//! Person-facing commands that change membership, rules, or durable identity.

use super::{
    LocalClient, Output, acting_agent, confirm, connection, invitations, presentation, print,
    resolve_goal, roles, selectors, status,
};
use crate::failure::Failure;
use clap::{Arg, ArgAction, ArgMatches, Command};
use locust_proto::api::{
    Abilities, Caller, DaemonStatus, ErrorCode, GoalStatus, InvitationState, Level, Membership,
    Request, Response, Voice, shell_word, short,
};
use locust_proto::event::TaskId;
use locust_proto::id::{BlobHash, EventId, GoalId, IdempotencyKey, PublicKey};
use locust_proto::local;
use locust_proto::organization::{Authority, Formation, WorkspacePolicy};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn option(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name).long(name).required(true).help(help)
}

fn goal_option() -> Arg {
    option("goal", "Visible goal title, identifier or unique prefix")
}

fn formation(command: Command) -> Command {
    command
        .arg(
            Arg::new("formation")
                .long("formation")
                .conflicts_with("formation-json")
                .help("Bundled formation name"),
        )
        .arg(
            Arg::new("formation-json")
                .long("formation-json")
                .help("Complete formation JSON"),
        )
        .arg(
            Arg::new("inputs")
                .long("inputs")
                .help("Input blob hashes as JSON"),
        )
}

pub(super) fn commands() -> Vec<(&'static str, Command)> {
    vec![
        (
            "goal",
            confirm::flags(formation(
                Command::new("create")
                    .arg(option("title", "Goal title"))
                    .arg(name_arg()),
            )),
        ),
        (
            "goal",
            confirm::flags(role_args(
                Command::new("add")
                    .arg(goal_option())
                    .arg(level_arg(false))
                    .arg(name_arg()),
            )),
        ),
        (
            "goal",
            confirm::flags(
                invitations::ticket_input(Command::new("join"))
                    .arg(level_arg(false))
                    .arg(name_arg()),
            ),
        ),
        (
            "goal",
            confirm::flags(Command::new("leave").arg(goal_option())),
        ),
        (
            "goal",
            confirm::flags(
                role_args(Command::new("invite").arg(goal_option())).arg(
                    Arg::new("expires")
                        .long("expires")
                        .default_value("7d")
                        .help("Invitation lifetime, e.g. 12h or 7d"),
                ),
            ),
        ),
        (
            "member",
            confirm::flags(
                Command::new("remove")
                    .arg(goal_option())
                    .arg(option("member", "Member key, prefix or local name")),
            ),
        ),
        (
            "rules",
            confirm::flags(
                formation(Command::new("bind").arg(goal_option())).arg(
                    Arg::new("no-role")
                        .long("no-role")
                        .action(ArgAction::SetTrue)
                        .help("Keep current role holders when binding rules that need reviewers"),
                ),
            ),
        ),
        (
            "task",
            confirm::flags(
                Command::new("revise")
                    .arg(goal_option())
                    .arg(option("task", "Task title or typed identifier"))
                    .arg(Arg::new("task-type").long("task-type")),
            ),
        ),
        ("role", roles::command("give")),
        ("role", roles::command("take")),
        (
            "agent",
            Command::new("revoke").about("Disconnect an agent; reconnect keeps its name and work"),
        ),
        (
            "agent",
            Command::new("reconnect")
                .about("Connect a disconnected agent again with its existing name and key"),
        ),
    ]
}

fn name_arg() -> Arg {
    Arg::new("name")
        .long("name")
        .help("Name this agent carries in the goal; defaults to its enrolled name")
}
fn role_args(command: Command) -> Command {
    command
        .arg(
            Arg::new("role")
                .long("role")
                .conflicts_with("no-role")
                .help("Role given when the member joins"),
        )
        .arg(
            Arg::new("no-role")
                .long("no-role")
                .action(clap::ArgAction::SetTrue)
                .help("Join without the counting role"),
        )
}
fn member_name(
    client: &mut LocalClient,
    socket: &Path,
    args: &ArgMatches,
    agent: PublicKey,
) -> Result<String, Failure> {
    let name = match args.get_one::<String>("name") {
        Some(name) => name.clone(),
        None => {
            status(client, socket, None)?
                .agents
                .into_iter()
                .find(|candidate| candidate.agent == agent)
                .ok_or_else(|| Failure::new(ErrorCode::NotFound, "select an enrolled local agent"))?
                .name
        }
    };
    if !locust_proto::event::is_member_name(&name) {
        return Err(Failure::invalid(
            "a member's name must have 1 to 64 bytes, no outer spaces and no control characters, and cannot look like a key (8 to 64 hex digits)",
        ));
    }
    Ok(name)
}

fn level_arg(required: bool) -> Arg {
    Arg::new("level")
        .long("level")
        .value_parser(["read", "ask", "auto"])
        .required(required)
        .help("Local level for this agent in the goal")
}

pub(super) fn level_command() -> Command {
    Command::new("level").arg(goal_option()).arg(
        Arg::new("level")
            .required(true)
            .value_parser(["read", "ask", "auto"]),
    )
}

pub(super) fn allow_command() -> Command {
    Command::new("allow")
        .arg(goal_option())
        .arg(option("task", "Task title or typed identifier"))
        .arg(
            Arg::new("revoke")
                .long("revoke")
                .action(clap::ArgAction::SetTrue),
        )
}

pub(super) fn owns(operation: &str) -> bool {
    matches!(
        operation,
        "level"
            | "allow"
            | "role.give"
            | "role.take"
            | "goal.create"
            | "goal.add"
            | "goal.join"
            | "goal.leave"
            | "goal.invite"
            | "member.remove"
            | "rules.bind"
            | "task.revise"
            | "invitation.revoke"
            | "agent.revoke"
            | "agent.reconnect"
    )
}

fn value<'a>(args: &'a ArgMatches, name: &str) -> &'a str {
    args.get_one::<String>(name).expect("required argument")
}

fn now_ms() -> Result<u64, Failure> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Failure::internal("system clock precedes the Unix epoch"))?
        .as_millis() as u64)
}

fn call(
    client: &mut LocalClient,
    socket: &Path,
    request: Request,
    key: Option<IdempotencyKey>,
) -> Result<Response, Failure> {
    client
        .call_with(request, key, None)
        .map_err(|error| connection::client_error(error, socket))
}

fn observed(client: &mut LocalClient, socket: &Path, goal: GoalId) -> Result<GoalStatus, Failure> {
    let Response::GoalStatus(observed) = call(client, socket, Request::GoalStatus { goal }, None)?
    else {
        unreachable!("typed client checks response kind")
    };
    Ok(observed)
}

fn selected_level(args: &ArgMatches) -> Level {
    match args
        .get_one::<String>("level")
        .map(String::as_str)
        .unwrap_or("auto")
    {
        "read" => Level::Read,
        "ask" => Level::Ask,
        "auto" => Level::Auto,
        _ => unreachable!("clap validates level"),
    }
}

fn level_word(level: Level) -> &'static str {
    match level {
        Level::Read => "read",
        Level::Ask => "ask",
        Level::Auto => "auto",
    }
}

fn agent_abilities(status: &GoalStatus, agent: PublicKey) -> Result<&Abilities, Failure> {
    status
        .abilities
        .iter()
        .find(|abilities| abilities.agent == agent)
        .ok_or_else(|| {
            Failure::new(
                ErrorCode::NotFound,
                "this agent has no local standing in this goal",
            )
        })
}

/// A goal's identifier cut to its shortest unique prefix among the goals
/// this daemon holds, so the printed line runs as printed. When the daemon
/// cannot be read the whole identifier is printed, which always runs.
pub(super) fn cut_goal(client: &mut LocalClient, socket: &Path, goal: GoalId) -> String {
    match status(client, socket, None) {
        Ok(known) => short(
            &goal.to_string(),
            &known
                .goals
                .iter()
                .map(|summary| summary.goal.to_string())
                .collect::<Vec<_>>(),
        ),
        Err(_) => goal.to_string(),
    }
}

/// A task's identifier cut among the goal's tasks, whole when they cannot
/// be read.
fn cut_task(client: &mut LocalClient, socket: &Path, goal: GoalId, task: TaskId) -> String {
    match call(client, socket, Request::Board { goal }, None) {
        Ok(Response::Board(tasks)) => short(
            &task.to_string(),
            &tasks
                .iter()
                .map(|view| view.task.to_string())
                .collect::<Vec<_>>(),
        ),
        _ => task.to_string(),
    }
}

fn level_set(
    matches: &ArgMatches,
    args: &ArgMatches,
    client: &mut LocalClient,
    socket: &Path,
) -> Result<Output, Failure> {
    let goal = resolve_goal(client, socket, value(args, "goal"), None)?;
    let agent = acting_agent(client, socket, matches, Some(goal))?;
    let before = observed(client, socket, goal)?;
    let prior = agent_abilities(&before, agent)?;
    let level = selected_level(args);
    let old = prior.level;
    let changed = old != level;
    let after = if changed {
        let Response::Abilities(after) = call(
            client,
            socket,
            Request::LevelSet { goal, agent, level },
            idempotency(matches)?,
        )?
        else {
            unreachable!("typed response")
        };
        after
    } else {
        prior.clone()
    };
    let title = presentation::safe(before.title.as_deref().unwrap_or("this goal"));
    let name = presentation::safe(&after.name);
    let mut human = format!(
        "{name} in \"{title}\": {}.\n{}",
        level_word(after.level),
        presentation::standing_line(&after, Voice::Person)
    );
    if after.level == Level::Auto && !after.hosted_here {
        human.push_str(", so tasks other members wrote run here unasked");
    }
    // The Undo line is for the person; under --json nothing prints it, so
    // the read that cuts the identifier is skipped.
    if changed && !matches.get_flag("json") {
        human.push_str(&format!(
            "\nUndo: {}",
            locust_proto::api::level_command(
                &cut_goal(client, socket, goal),
                &after.name,
                level_word(old)
            )
        ));
    }
    Ok(Output::success(
        json!({"goal":goal,"agent":agent,"level":after.level,"abilities":after,"changed":changed}),
        human,
    ))
}

fn task_allow(
    matches: &ArgMatches,
    args: &ArgMatches,
    client: &mut LocalClient,
    socket: &Path,
) -> Result<Output, Failure> {
    let goal = resolve_goal(client, socket, value(args, "goal"), None)?;
    let agent = acting_agent(client, socket, matches, Some(goal))?;
    let task = selectors::resolve_task(client, socket, goal, value(args, "task"), None)?;
    let detail = task_detail(client, socket, goal, task)?;
    let before = observed(client, socket, goal)?;
    let prior = agent_abilities(&before, agent)?;
    let visibly_allowed = prior.allowed_tasks.contains(&task);
    let revoke = args.get_flag("revoke");
    // A closed or finished task hides its stored allowance from Abilities.
    // Revocation must reach the daemon even when no allowance is visible here.
    let (after, changed, was_allowed) = if revoke {
        let Response::TaskDisallowed {
            abilities,
            changed,
            was_allowed,
        } = call(
            client,
            socket,
            Request::TaskDisallow { goal, agent, task },
            idempotency(matches)?,
        )?
        else {
            unreachable!("typed response")
        };
        (abilities, changed, was_allowed)
    } else if !visibly_allowed {
        let Response::Abilities(abilities) = call(
            client,
            socket,
            Request::TaskAllow { goal, agent, task },
            idempotency(matches)?,
        )?
        else {
            unreachable!("typed response")
        };
        (abilities, true, false)
    } else {
        (prior.clone(), false, false)
    };
    let task_title = presentation::safe(detail.view.title.as_deref().unwrap_or("this task"));
    let goal_title = presentation::safe(before.title.as_deref().unwrap_or("this goal"));
    let name = presentation::safe(&after.name);
    // The printed lines are for the person; under --json nothing prints
    // them, so the reads that cut the identifiers are skipped.
    let human_wanted = !matches.get_flag("json");
    let goal_cut = if human_wanted {
        cut_goal(client, socket, goal)
    } else {
        String::new()
    };
    let mut human = if revoke {
        format!("{name} may no longer take \"{task_title}\". A running attempt is not stopped.")
    } else if after.level == Level::Read {
        // The allowance is stored, but at read nothing lets the agent act on it.
        format!(
            "\"{task_title}\" is allowed for {name}, but at read it only reads. It takes the task once it is set to ask:\n  {}",
            locust_proto::api::level_command(&goal_cut, &after.name, "ask")
        )
    } else {
        format!("{name} may take \"{task_title}\" in \"{goal_title}\" until the host revises it.")
    };
    let takeable = !detail.view.closed && !detail.view.completed && detail.view.selected.is_none();
    if human_wanted && changed && (!revoke || (was_allowed && visibly_allowed && takeable)) {
        human.push_str(&format!(
            "\nUndo: {}",
            locust_proto::api::allow_command(
                &goal_cut,
                &cut_task(client, socket, goal, task),
                &after.name,
                !revoke
            )
        ));
    }
    Ok(Output::success(
        json!({"goal":goal,"agent":agent,"task":task,"allowed":!revoke,"abilities":after,"changed":changed}),
        human,
    ))
}

fn idempotency(matches: &ArgMatches) -> Result<Option<IdempotencyKey>, Failure> {
    matches
        .get_one::<String>("idempotency-key")
        .map(|text| {
            text.parse::<IdempotencyKey>()
                .map_err(|_| Failure::usage("--idempotency-key requires 16 bytes in hex"))
        })
        .transpose()
}

fn reviewed(
    matches: &ArgMatches,
    args: &ArgMatches,
    plan: &confirm::Plan,
    fresh: impl FnOnce() -> Result<confirm::Plan, Failure>,
) -> Result<Option<Output>, Failure> {
    match confirm::decide(matches, args, plan)? {
        confirm::Decision::Show => Ok(Some(plan.shown())),
        confirm::Decision::Proceed => {
            let id = plan.id();
            confirm::bound(&id, &fresh()?)?;
            Ok(None)
        }
    }
}

fn name_for(known: &DaemonStatus, agent: PublicKey) -> String {
    known
        .agents
        .iter()
        .find(|known| known.agent == agent)
        .map(|known| presentation::safe(&known.name))
        .unwrap_or_else(|| agent.to_string())
}

fn current_agent(client: &LocalClient) -> Result<PublicKey, Failure> {
    match client.caller() {
        Caller::Agent(agent) | Caller::Author(agent) => Ok(agent),
        _ => Err(Failure::usage("this request requires --owner and an agent")),
    }
}

pub(super) fn run(
    matches: &ArgMatches,
    operation: &str,
    args: &ArgMatches,
) -> Result<Output, Failure> {
    if matches!(operation, "goal.add" | "agent.revoke" | "agent.reconnect")
        && (!matches.get_flag("owner") || matches.get_one::<String>("agent").is_none())
    {
        return Err(Failure::usage(format!(
            "{operation} requires --owner --agent NAME"
        )));
    }
    if matches.get_one::<String>("agent").is_some()
        && matches!(
            operation,
            "goal.invite"
                | "member.remove"
                | "rules.bind"
                | "task.revise"
                | "role.give"
                | "role.take"
        )
    {
        return Err(Failure::usage(
            "host commands are the host's own and name no agent; drop --agent",
        ));
    }
    let home = connection::home(matches)?;
    let socket = local::socket_path(&home)?;
    let mut client = connection::open(matches, &home)?;
    let owner = client.caller() == Caller::Owner;
    match operation {
        "goal.create" => goal_create(matches, args, &mut client, &socket, owner),
        "role.give" | "role.take" => roles::run(matches, operation, args, &mut client, &socket),
        "level" => level_set(matches, args, &mut client, &socket),
        "allow" => task_allow(matches, args, &mut client, &socket),
        "goal.add" => goal_add(matches, args, &mut client, &socket),
        "goal.join" => goal_join(matches, args, &mut client, &socket, owner),
        "goal.leave" => goal_leave(matches, args, &mut client, &socket, owner),
        "goal.invite" => goal_invite(matches, args, &mut client, &socket, owner),
        "member.remove" => member_remove(matches, args, &mut client, &socket, owner),
        "rules.bind" => rules_bind(matches, args, &mut client, &socket, owner),
        "task.revise" => task_revise(matches, args, &mut client, &socket, owner),
        "invitation.revoke" => invitation_revoke(matches, args, &mut client, &socket, owner),
        "agent.revoke" => agent_revoke(matches, &mut client, &socket),
        "agent.reconnect" => agent_reconnect(matches, &mut client, &socket),
        _ => Err(Failure::usage("unknown person command")),
    }
}

fn json_map<T: serde::de::DeserializeOwned>(args: &ArgMatches, name: &str) -> Result<T, Failure> {
    serde_json::from_str(
        args.get_one::<String>(name)
            .map(String::as_str)
            .unwrap_or("{}"),
    )
    .map_err(|error| Failure::usage(format!("--{}: {error}", name.replace('_', "-"))))
}

fn checked_formation(source: &str) -> Result<Formation, Failure> {
    let inspection = locust_core::organization::inspect(source);
    if !inspection.valid {
        return Err(Failure::invalid(
            inspection
                .diagnostics
                .iter()
                .map(|diagnostic| {
                    format!(
                        "{}: {} {}",
                        diagnostic.code, diagnostic.message, diagnostic.correction
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ));
    }
    Ok(inspection.normalized.expect("valid formation"))
}

fn chosen_formation(args: &ArgMatches) -> Result<(String, Option<String>, Formation), Failure> {
    if let Some(name) = args.get_one::<String>("formation") {
        let preset = locust_proto::organization::presets()
            .into_iter()
            .find(|preset| preset.name == *name)
            .ok_or_else(|| {
                Failure::usage(format!("unknown formation {name}; use formation examples"))
            })?;
        let source = serde_json::to_string(&preset.formation)
            .map_err(|error| Failure::internal(error.to_string()))?;
        return Ok((name.clone(), Some(source), preset.formation));
    }
    if let Some(source) = args.get_one::<String>("formation-json") {
        let parsed = checked_formation(source)?;
        return Ok(("custom".into(), Some(source.clone()), parsed));
    }
    let preset = locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == "peer-review")
        .expect("peer-review preset");
    Ok(("peer-review".into(), None, preset.formation))
}

/// A goal's identifier as a label beside its title, never as a command:
/// eight characters, read from nothing.
fn goal_label(goal: GoalId) -> String {
    short(&goal.to_string(), &[])
}

/// The cut of `goal` among the goals a status read already listed.
fn cut_goal_among(known: &DaemonStatus, goal: GoalId) -> String {
    short(
        &goal.to_string(),
        &known
            .goals
            .iter()
            .map(|summary| summary.goal.to_string())
            .collect::<Vec<_>>(),
    )
}

struct CreateSelection<'a> {
    agent: PublicKey,
    title: &'a str,
    formation_name: &'a str,
    formation_json: &'a Option<String>,
    formation: &'a Formation,
    name: &'a str,
    inputs: &'a BTreeMap<String, BlobHash>,
}

fn create_plan(
    client: &mut LocalClient,
    socket: &Path,
    selected: &CreateSelection<'_>,
) -> Result<confirm::Plan, Failure> {
    let known = status(client, socket, None)?;
    let titled: std::collections::BTreeSet<_> = known
        .goals
        .iter()
        .filter(|goal| goal.title.as_deref() == Some(selected.title))
        .map(|goal| goal.goal)
        .collect();
    Ok(confirm::Plan {
        command: "goal create",
        review: json!({"agent":selected.agent,"title":selected.title,"already_titled":titled.len(),
            "formation":selected.formation_name,"formation_json":selected.formation_json,
            "name":selected.name,"inputs":selected.inputs}),
        human: format!(
            "Start \"{}\" with the {} rules. Host: you. {} joins as {}. {}{}",
            presentation::safe(selected.title),
            presentation::safe(selected.formation_name),
            name_for(&known, selected.agent),
            presentation::safe(selected.name),
            presentation::counts_when(&selected.formation.decisions.completion),
            roles::initial_reviewers(selected.formation, selected.agent, selected.name)
        ),
        warning: None,
        again: String::new(),
    })
}

fn goal_create(
    matches: &ArgMatches,
    args: &ArgMatches,
    client: &mut LocalClient,
    socket: &Path,
    owner: bool,
) -> Result<Output, Failure> {
    let title = value(args, "title");
    let (formation_name, formation_json, formation) = chosen_formation(args)?;
    let inputs: BTreeMap<String, BlobHash> = json_map(args, "inputs")?;
    let agent = if owner {
        acting_agent(client, socket, matches, None)?
    } else {
        current_agent(client)?
    };
    let name = member_name(client, socket, args, agent)?;
    if owner {
        let selected = CreateSelection {
            agent,
            title,
            formation_name: &formation_name,
            formation_json: &formation_json,
            formation: &formation,
            name: &name,
            inputs: &inputs,
        };
        let plan = create_plan(client, socket, &selected)?;
        if let Some(output) = reviewed(matches, args, &plan, || {
            create_plan(client, socket, &selected)
        })? {
            return Ok(output);
        }
    }
    let response = call(
        client,
        socket,
        Request::GoalCreate {
            agent,
            title: title.into(),
            name: name.clone(),
            formation_json,
            inputs,
        },
        idempotency(matches)?,
    )?;
    let result = json!(response);
    let Response::GoalCreated { goal } = response else {
        unreachable!("typed response")
    };
    Ok(Output::success(
        result,
        format!(
            "Started \"{}\" ({}). Host: you. {}{}. The goal runs from this computer.",
            presentation::safe(title),
            goal_label(goal),
            presentation::safe(&name),
            if formation.roles.is_empty() {
                " is at level auto"
            } else {
                " holds every role, at level auto"
            }
        ),
    ))
}

fn already_member_output(
    client: &mut LocalClient,
    socket: &Path,
    goal: GoalId,
    agent: PublicKey,
    requested_name: Option<&str>,
    role: Option<&str>,
) -> Result<Output, Failure> {
    let known = status(client, socket, None)?;
    let view = observed(client, socket, goal)?;
    let member = view
        .members
        .iter()
        .find(|member| member.member == agent)
        .ok_or_else(|| Failure::unavailable("the member's admission has not arrived"))?;
    let level = view
        .abilities
        .iter()
        .find(|ability| ability.agent == agent)
        .map_or(Level::Auto, |ability| ability.level);
    let mut human = format!(
        "{} is already in \"{}\" as {} · {}.",
        name_for(&known, agent),
        presentation::safe(view.title.as_deref().unwrap_or("this goal")),
        presentation::safe(&member.name),
        level_word(level)
    );
    if requested_name.is_some_and(|name| name != member.name) {
        human.push_str(" A name cannot change.");
    }
    if let Some(role) = role
        && !view
            .roles
            .get(role)
            .is_some_and(|holders| holders.contains(&agent))
    {
        // Only the host's person gives roles; elsewhere the command would
        // be refused, so say who acts instead.
        if view.hosted_here && client.caller() == Caller::Owner {
            human.push('\n');
            human.push_str(&roles::role_command_line(
                "Give the role",
                "give",
                &cut_goal_among(&known, goal),
                &selectors::member_key_prefix(&view.members, agent),
                role,
            ));
        } else {
            human.push_str(&format!(
                " It does not hold {}; {}",
                presentation::safe(role),
                match &view.host_name {
                    Some(host) => format!(
                        "the host, {}'s owner, gives roles.",
                        presentation::chosen_name(host)
                    ),
                    None => "the host gives roles.".to_owned(),
                }
            ));
        }
    }
    Ok(Output::success(
        json!({"goal":goal,"agent":agent,"name":member.name,
        "membership":"member","level":level,"changed":false}),
        human,
    ))
}

fn add_plan(
    client: &mut LocalClient,
    socket: &Path,
    goal: GoalId,
    agent: PublicKey,
    level: Level,
    args: &ArgMatches,
) -> Result<confirm::Plan, Failure> {
    let known = status(client, socket, None)?;
    let selected = known
        .agents
        .iter()
        .find(|candidate| candidate.agent == agent && !candidate.author_only)
        .ok_or_else(|| {
            Failure::new(ErrorCode::NotFound, "select an active enrolled local agent")
        })?;
    if selected.revoked {
        return Err(Failure::usage(super::disconnected_agent(selected)));
    }
    let goal_status = observed(client, socket, goal)?;
    if !goal_status.hosted_here {
        return Err(Failure::new(
            ErrorCode::Denied,
            "this goal is hosted on another computer; request an invitation from its host",
        ));
    }
    let standing = known
        .goals
        .iter()
        .find(|entry| entry.goal == goal && entry.member == agent)
        .map(|entry| entry.membership);
    let joined = standing == Some(Membership::Member);
    let current_level = goal_status
        .abilities
        .iter()
        .find(|abilities| abilities.agent == agent)
        .map(|abilities| abilities.level);
    if goal_status
        .members
        .iter()
        .any(|member| member.member == agent)
        && !joined
    {
        return Err(Failure::new(
            ErrorCode::Conflict,
            "this agent's local membership is not active; inspect its departure or admission before rejoining",
        ));
    }
    let title = goal_status.title.as_deref().unwrap_or("this goal");
    let name = args
        .get_one::<String>("name")
        .cloned()
        .unwrap_or_else(|| selected.name.clone());
    if !locust_proto::event::is_member_name(&name) {
        return Err(Failure::invalid(
            "a member's name must have 1 to 64 bytes, no outer spaces and no control characters, and cannot look like a key (8 to 64 hex digits)",
        ));
    }
    let role = roles::selected_role(client, socket, args, &goal_status)?;
    let role_words = role
        .as_deref()
        .map(|role| format!(", a {}", presentation::safe(role)))
        .unwrap_or_default();
    Ok(confirm::Plan {
        command: "goal add",
        review: json!({"goal":goal,"title":goal_status.title,"host":goal_status.host,
            "agent":agent,"name":name,"role":role,"already_member":joined,"level":level,"current_level":current_level}),
        human: format!(
            "Add {} to \"{}\" ({}) as {}{}, at level {}. This shares the whole goal's history and content. Local files and private chats stay here. Level: read (reads and reports), ask (posts and asks before tasks), auto (takes tasks on its own) [selected: {}].",
            presentation::safe(&selected.name),
            presentation::safe(title),
            cut_goal_among(&known, goal),
            presentation::safe(&name),
            role_words,
            level_word(level),
            level_word(level)
        ),
        warning: None,
        again: String::new(),
    })
}

fn add_retry_key(
    observed: &GoalStatus,
    agent: PublicKey,
    expires_ms: u64,
    role: Option<&str>,
) -> IdempotencyKey {
    let bytes = serde_json::to_vec(&(
        "locust-goal-add-v2",
        observed.goal,
        observed.governance,
        observed.governance_head,
        agent,
        expires_ms,
        role,
    ))
    .expect("public retry selection encodes");
    IdempotencyKey(
        locust_proto::crypto::content_hash(&bytes).0[..16]
            .try_into()
            .unwrap(),
    )
}

fn goal_add(
    matches: &ArgMatches,
    args: &ArgMatches,
    client: &mut LocalClient,
    socket: &Path,
) -> Result<Output, Failure> {
    let goal = resolve_goal(client, socket, value(args, "goal"), None)?;
    let agent = acting_agent(client, socket, matches, None)?;
    let level = selected_level(args);
    let plan = add_plan(client, socket, goal, agent, level, args)?;
    if plan.review["already_member"] == true {
        return already_member_output(
            client,
            socket,
            goal,
            agent,
            args.get_one::<String>("name").map(String::as_str),
            plan.review["role"].as_str(),
        );
    }
    if let Some(output) = reviewed(matches, args, &plan, || {
        add_plan(client, socket, goal, agent, level, args)
    })? {
        return Ok(output);
    }
    let name = plan.review["name"].as_str().unwrap_or("agent");
    let role: Option<String> = serde_json::from_value(plan.review["role"].clone())
        .map_err(|error| Failure::internal(error.to_string()))?;
    let title = plan.review["title"].as_str().unwrap_or("this goal");
    let goal_status = observed(client, socket, goal)?;
    let now = now_ms()?;
    let expires_ms = (now / 86_400_000 + 2) * 86_400_000;
    let retry_key = add_retry_key(&goal_status, agent, expires_ms, role.as_deref());
    let Response::Invited { ticket } = call(
        client,
        socket,
        Request::GoalInvite {
            goal,
            expires_ms,
            role,
        },
        Some(retry_key),
    )?
    else {
        unreachable!("typed response")
    };
    let Response::InvitationInspected { preview } = call(
        client,
        socket,
        Request::InvitationInspect {
            ticket: ticket.clone(),
        },
        None,
    )?
    else {
        unreachable!("typed response")
    };
    if preview.goal != goal || preview.governance != goal_status.governance {
        return Err(Failure::new(
            ErrorCode::Conflict,
            "local invitation differs from the reviewed goal",
        ));
    }
    let Response::Joined {
        goal: joined_goal,
        membership,
        level: joined_level,
        ..
    } = call(
        client,
        socket,
        Request::GoalJoin {
            agent,
            name: name.into(),
            ticket,
            level,
        },
        None,
    )?
    else {
        unreachable!("typed response")
    };
    if joined_goal != goal
        || membership != Membership::Member
        || !observed(client, socket, goal)?
            .members
            .iter()
            .any(|member| member.member == agent && member.local)
    {
        return Err(Failure::unavailable(
            "local admission is incomplete; inspect status and repeat goal add",
        ));
    }
    Ok(Output::success(
        json!({"goal":goal,"agent":agent,"membership":"member","level":joined_level,"changed":true}),
        format!(
            "{} joined \"{}\" · {}.",
            presentation::safe(name),
            presentation::safe(title),
            level_word(joined_level)
        ),
    ))
}

fn join_plan(
    client: &mut LocalClient,
    socket: &Path,
    preview: &locust_proto::api::InvitationPreview,
    agent: PublicKey,
    level: Level,
    name: &str,
) -> Result<confirm::Plan, Failure> {
    let known = status(client, socket, None)?;
    let standing = known
        .goals
        .iter()
        .find(|entry| entry.goal == preview.goal && entry.member == agent)
        .map(|entry| entry.membership);
    let joining_facts = format!(
        "Host: {}. {}{}",
        presentation::safe(&preview.host_name),
        preview.sharing_facts.join(" "),
        preview
            .role
            .as_deref()
            .map(|role| format!(" Joins as a {}.", presentation::safe(role)))
            .unwrap_or_default()
    );
    Ok(confirm::Plan {
        command: "goal join",
        review: json!({"invitation_review":preview.review,"goal":preview.goal,
            "title":preview.goal_title,"agent":agent,"name":name,"role":preview.role,"host_name":preview.host_name,"standing":standing,"level":level}),
        human: format!(
            "Join \"{}\" ({}) as {}. This shares what this agent posts with the goal's members. {} Level: read (reads and reports), ask (posts and asks before tasks), auto (takes tasks on its own, so tasks other members wrote run here unasked) [selected: {}]. What a level allows also depends on the goal's rules, which arrive after admission.",
            presentation::safe(preview.goal_title.as_deref().unwrap_or("this goal")),
            cut_goal_among(&known, preview.goal),
            presentation::safe(name),
            joining_facts,
            level_word(level)
        ),
        warning: None,
        again: String::new(),
    })
}

fn goal_join(
    matches: &ArgMatches,
    args: &ArgMatches,
    client: &mut LocalClient,
    socket: &Path,
    owner: bool,
) -> Result<Output, Failure> {
    let ticket = invitations::read_ticket(args)?;
    let now = now_ms()?;
    let preview = locust_proto::invite::Invitation::from_ticket(ticket.as_str())
        .map_err(locust_proto::api::ApiError::from)
        .map_err(Failure::from)?
        .preview(now)
        .map_err(locust_proto::api::ApiError::from)
        .map_err(Failure::from)?;
    let agent = if owner {
        acting_agent(client, socket, matches, None)?
    } else {
        current_agent(client)?
    };
    let level = selected_level(args);
    let name = member_name(client, socket, args, agent)?;
    let plan = join_plan(client, socket, &preview, agent, level, &name)?;
    if plan.review["standing"] == "member" {
        return already_member_output(
            client,
            socket,
            preview.goal,
            agent,
            args.get_one::<String>("name").map(String::as_str),
            preview.role.as_deref(),
        );
    }
    if owner {
        if args
            .get_one::<String>("ticket")
            .is_some_and(|source| source == "-")
            && args.get_one::<String>("confirm").is_none()
        {
            return Ok(plan.shown());
        }
        if let Some(output) = reviewed(matches, args, &plan, || {
            join_plan(client, socket, &preview, agent, level, &name)
        })? {
            return Ok(output);
        }
    }
    let response = call(
        client,
        socket,
        Request::GoalJoin {
            agent,
            name: name.clone(),
            ticket,
            level,
        },
        idempotency(matches)?,
    )?;
    let result = json!(response);
    let Response::Joined {
        membership,
        level: joined_level,
        ..
    } = response
    else {
        unreachable!("typed response")
    };
    let name = presentation::safe(&name);
    let title = presentation::safe(preview.goal_title.as_deref().unwrap_or("this goal"));
    let human = if membership == Membership::Member {
        format!("{name} joined \"{title}\" · {}.", level_word(joined_level))
    } else if plan.review["standing"] == "joining" {
        // A re-run of a waiting join: the host may have signed the earlier
        // name already, and an admitted name stays.
        format!(
            "Joining \"{title}\" as {name} ({}), unless the host's computer already admitted it under the earlier name; an admitted name stays. Admission comes from the host's computer; locust --owner status shows it.",
            level_word(joined_level)
        )
    } else {
        format!(
            "Joining \"{title}\" as {name} ({}). Admission comes from the host's computer; locust --owner status shows it.",
            level_word(joined_level)
        )
    };
    Ok(Output::success(result, human))
}

fn leave_plan(
    client: &mut LocalClient,
    socket: &Path,
    goal: GoalId,
    agent: PublicKey,
) -> Result<confirm::Plan, Failure> {
    let observed = observed(client, socket, goal)?;
    if observed.host == Some(agent) {
        return Err(Failure::new(
            ErrorCode::Conflict,
            "the host's agent cannot leave its own goal",
        ));
    }
    let known = status(client, socket, None)?;
    let standing = known
        .goals
        .iter()
        .find(|entry| entry.goal == goal && entry.member == agent)
        .map(|entry| entry.membership);
    Ok(confirm::Plan {
        command: "goal leave",
        review: json!({"goal":goal,"title":observed.title,"host":observed.host,
            "agent":agent,"standing":standing}),
        human: format!(
            "{} leaves \"{}\" ({}). Coming back needs a new invitation; copies already received stay with the goal.",
            name_for(&known, agent),
            presentation::safe(observed.title.as_deref().unwrap_or("this goal")),
            cut_goal_among(&known, goal),
        ),
        warning: None,
        again: String::new(),
    })
}

fn goal_leave(
    matches: &ArgMatches,
    args: &ArgMatches,
    client: &mut LocalClient,
    socket: &Path,
    owner: bool,
) -> Result<Output, Failure> {
    let goal = resolve_goal(client, socket, value(args, "goal"), None)?;
    let agent = if owner {
        acting_agent(client, socket, matches, Some(goal))?
    } else {
        current_agent(client)?
    };
    let plan = if owner {
        let plan = leave_plan(client, socket, goal, agent)?;
        if let Some(output) = reviewed(matches, args, &plan, || {
            leave_plan(client, socket, goal, agent)
        })? {
            return Ok(output);
        }
        Some(plan)
    } else {
        None
    };
    let response = call(
        client,
        socket,
        Request::GoalLeave { goal, agent },
        idempotency(matches)?,
    )?;
    let known = if owner {
        Some(status(client, socket, None)?)
    } else {
        None
    };
    let name = known
        .as_ref()
        .map_or_else(|| agent.to_string(), |known| name_for(known, agent));
    let title = plan
        .as_ref()
        .and_then(|plan| plan.review["title"].as_str())
        .unwrap_or("this goal");
    Ok(Output::success(
        json!(response),
        format!(
            "{} left \"{}\". Copies already received stay with the goal.",
            name,
            presentation::safe(title)
        ),
    ))
}

fn duration_ms(text: &str) -> Result<u64, Failure> {
    if text == "never" {
        return Err(Failure::usage(
            "an invitation needs an expiry; the longest is up to you, e.g. --expires 30d",
        ));
    }
    let (number, multiplier) = if let Some(number) = text.strip_suffix('h') {
        (number, 3_600_000)
    } else if let Some(number) = text.strip_suffix('d') {
        (number, 86_400_000)
    } else {
        return Err(Failure::usage(
            "--expires requires Nh or Nd, e.g. 12h or 7d",
        ));
    };
    let count = number
        .parse::<u64>()
        .map_err(|_| Failure::usage("--expires requires Nh or Nd, e.g. 12h or 7d"))?;
    if count == 0 {
        return Err(Failure::usage("--expires must be at least 1h"));
    }
    count
        .checked_mul(multiplier)
        .ok_or_else(|| Failure::usage("--expires is too large"))
}

fn invite_plan(
    client: &mut LocalClient,
    socket: &Path,
    goal: GoalId,
    typed_duration: &str,
    args: &ArgMatches,
) -> Result<confirm::Plan, Failure> {
    let observed = observed(client, socket, goal)?;
    let Response::Invitations { invitations } =
        call(client, socket, Request::GoalInvitations { goal }, None)?
    else {
        unreachable!("typed response")
    };
    let pending = invitations
        .iter()
        .filter(|invitation| invitation.state == InvitationState::Pending)
        .count();
    let issued = invitations.len();
    let role = roles::selected_role(client, socket, args, &observed)?;
    let role_words = role
        .as_deref()
        .map(|role| format!(" as a {}", presentation::safe(role)))
        .unwrap_or_default();
    Ok(confirm::Plan {
        command: "goal invite",
        review: json!({"goal":goal,"title":observed.title,"host":observed.host,
            "duration":typed_duration,"role":role,"issued_invitations":issued,
            "pending_invitations":pending}),
        human: format!(
            "Invite someone to \"{}\" ({}){}. The ticket shares this goal's history and content with whoever presents it. Expires after {}. Invitations issued: {issued}. Pending invitations: {pending}.",
            presentation::safe(observed.title.as_deref().unwrap_or("this goal")),
            goal_label(goal),
            role_words,
            typed_duration
        ),
        warning: None,
        again: String::new(),
    })
}

fn goal_invite(
    matches: &ArgMatches,
    args: &ArgMatches,
    client: &mut LocalClient,
    socket: &Path,
    owner: bool,
) -> Result<Output, Failure> {
    let duration = value(args, "expires");
    let lifetime = duration_ms(duration)?;
    let goal = resolve_goal(client, socket, value(args, "goal"), None)?;
    let role = if owner {
        let plan = invite_plan(client, socket, goal, duration, args)?;
        if let Some(output) = reviewed(matches, args, &plan, || {
            invite_plan(client, socket, goal, duration, args)
        })? {
            return Ok(output);
        }
        serde_json::from_value(plan.review["role"].clone())
            .map_err(|error| Failure::internal(error.to_string()))?
    } else {
        args.get_one::<String>("role").cloned()
    };
    let expires_ms = now_ms()?
        .checked_add(lifetime)
        .ok_or_else(|| Failure::usage("--expires is too large"))?;
    let response = call(
        client,
        socket,
        Request::GoalInvite {
            goal,
            expires_ms,
            role,
        },
        idempotency(matches)?,
    )?;
    let Response::Invited { ticket } = &response else {
        unreachable!("typed response")
    };
    let warning = format!(
        "Anyone who presents this ticket is admitted while this computer is on, until {}. Send it privately. Stop admission: locust --owner invitation revoke --goal {} --all",
        presentation::utc(expires_ms),
        cut_goal(client, socket, goal)
    );
    if !matches.get_flag("json") {
        print::stderr(format_args!("{warning}\n")).map_err(|error| {
            Failure::internal(format!("cannot show invitation warning: {error}"))
        })?;
    }
    let mut result = json!(response);
    result["warning"] = json!(warning);
    Ok(Output::success(result, ticket.as_str().to_owned()))
}

fn removal_plan(
    client: &mut LocalClient,
    socket: &Path,
    goal: GoalId,
    member: PublicKey,
) -> Result<confirm::Plan, Failure> {
    let observed = observed(client, socket, goal)?;
    if observed.host == Some(member) {
        return Err(Failure::new(
            ErrorCode::Conflict,
            "the host's agent cannot be removed from its own goal",
        ));
    }
    let present = observed.members.iter().any(|entry| entry.member == member);
    let label = presentation::member_label(member, &observed.members);
    Ok(confirm::Plan {
        command: "member remove",
        review: json!({"goal":goal,"title":observed.title,"host":observed.host,
            "member":member,"member_present":present,"member_label":label}),
        human: format!(
            "Remove member {label} from \"{}\" ({}). Copies already received cannot be retracted.",
            presentation::safe(observed.title.as_deref().unwrap_or("this goal")),
            goal_label(goal)
        ),
        warning: None,
        again: String::new(),
    })
}

fn member_remove(
    matches: &ArgMatches,
    args: &ArgMatches,
    client: &mut LocalClient,
    socket: &Path,
    owner: bool,
) -> Result<Output, Failure> {
    let goal = resolve_goal(client, socket, value(args, "goal"), None)?;
    let member = selectors::resolve_member(client, socket, goal, value(args, "member"))?;
    // The plan is read before the write, where the member's label still is;
    // an agent's request goes to the daemon's own check without a plan.
    let plan = if owner {
        let plan = removal_plan(client, socket, goal, member)?;
        if let Some(output) = reviewed(matches, args, &plan, || {
            removal_plan(client, socket, goal, member)
        })? {
            return Ok(output);
        }
        Some(plan)
    } else {
        None
    };
    let response = call(
        client,
        socket,
        Request::MemberRemove { goal, member },
        idempotency(matches)?,
    )?;
    let (label, title) = match &plan {
        Some(plan) => (
            plan.review["member_label"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            plan.review["title"].as_str().unwrap_or("this goal"),
        ),
        None => (presentation::member_label(member, &[]), "this goal"),
    };
    Ok(Output::success(
        json!(response),
        format!(
            "Removed {label} from \"{}\". Copies already received cannot be retracted.",
            presentation::safe(title)
        ),
    ))
}

fn rules_plan(
    client: &mut LocalClient,
    socket: &Path,
    goal: GoalId,
    formation_name: &str,
    formation_json: &str,
    inputs: &BTreeMap<String, BlobHash>,
    no_role: bool,
) -> Result<confirm::Plan, Failure> {
    let observed = observed(client, socket, goal)?;
    let current = observed
        .current_rules
        .ok_or_else(|| Failure::unavailable("current rules have not arrived"))?;
    let mut formation: Formation = serde_json::from_str(formation_json)
        .map_err(|error| Failure::invalid(format!("formation definition: {error}")))?;
    let workspace = observed
        .workspace
        .as_ref()
        .filter(|workspace| workspace.enabled);
    let goal_cut = cut_goal(client, socket, goal);
    let mut file_lines = String::new();
    let mut file_changes = String::new();
    if let Some(workspace) = workspace {
        if formation.workspace.is_none() {
            let host = observed
                .host
                .ok_or_else(|| Failure::unavailable("the host's agent has not arrived"))?;
            formation.workspace = Some(WorkspacePolicy {
                integrator: Authority::Participant {
                    key: host.to_string(),
                },
                completion: formation.decisions.completion.clone(),
            });
        }
        let source = serde_json::to_string(&formation)
            .map_err(|error| Failure::internal(error.to_string()))?;
        let inspected = locust_core::organization::inspect(&source);
        if inspected
            .diagnostics
            .iter()
            .any(|item| item.code == "selector_scope" && item.path.starts_with("/workspace"))
        {
            return Err(Failure::invalid(
                "These rules cannot apply to the shared files. Give the files a rule in the formation's workspace part.",
            ));
        }
        let old = roles::current_formation(client, socket, &observed)?;
        if old
            .as_ref()
            .and_then(|old| {
                old.workspace
                    .as_ref()
                    .map(|policy| policy.completion != old.decisions.completion)
            })
            .unwrap_or(false)
        {
            file_lines.push_str("\nThis replaces the rule you gave the shared files.");
        }
        file_lines.push_str("\nShared files will follow these rules too. ");
        file_lines.push_str(&presentation::counts_when(
            &formation
                .workspace
                .as_ref()
                .expect("policy added")
                .completion,
        ));
        let Response::WorkspaceProposals(proposals) =
            call(client, socket, Request::WorkspaceProposals { goal }, None)?
        else {
            unreachable!("typed response")
        };
        let stranded: Vec<_> = proposals
            .iter()
            .filter(|proposal| {
                Some(proposal.context.round) == workspace.epoch
                    && proposal.integrated_as.is_empty()
                    && proposal.standing == locust_proto::api::Standing::Effective
                    && !proposal.stale
            })
            .collect();
        let changes = stranded
            .iter()
            .filter(|proposal| proposal.parent.is_some())
            .count();
        match changes {
            0 => {}
            1 => file_changes.push_str(
                "\n1 file change that has not landed must be proposed again by its author.",
            ),
            n => file_changes.push_str(&format!(
                "\n{n} file changes that have not landed must be proposed again by their authors."
            )),
        }
        if stranded.iter().any(|proposal| proposal.parent.is_none()) {
            file_changes.push_str(&format!("\nThe first files have not landed. Share them again with locust --owner workspace init --goal {goal_cut} and the same seed options."));
        }
        file_lines.push_str(&file_changes);
    }
    let source =
        serde_json::to_string(&formation).map_err(|error| Failure::internal(error.to_string()))?;
    checked_formation(&source)?;
    let counting_role = roles::counting_role(&formation);
    let mut role_lines = String::new();
    if let Some(role) = &counting_role {
        let holders = observed
            .roles
            .get(role)
            .cloned()
            .unwrap_or_else(|| observed.host.into_iter().collect());
        let unheld: Vec<_> = observed
            .members
            .iter()
            .filter(|member| !holders.contains(&member.member))
            .collect();
        // The daemon gives no role where earlier rules, which open tasks
        // still follow, let one holder act alone.
        let alone = observed.acting_alone.contains(role);
        if no_role || alone {
            // The bind creates an absent list as the host's agent alone.
            let mut lists = observed.roles.clone();
            lists.entry(role.clone()).or_insert_with(|| holders.clone());
            let missing = roles::missing_reviewers(&formation, role, &lists)
                .map(|missing| format!("{missing} more reviewers are needed. "))
                .unwrap_or_default();
            if alone && !no_role {
                role_lines.push_str(&format!(
                    "\nUnder the goal's earlier rules, which open tasks still follow, one {} acts alone, so this change gives the role to no one.",
                    presentation::safe(role)
                ));
            }
            role_lines.push_str(&format!("\n{missing}The role's holders stay as they are."));
            for member in unheld {
                role_lines.push('\n');
                role_lines.push_str(&roles::role_command_line(
                    "Give the role",
                    "give",
                    &goal_cut,
                    &selectors::member_key_prefix(&observed.members, member.member),
                    role,
                ));
            }
        } else if let Some(member) = unheld.first() {
            role_lines.push_str(&format!(
                "\nEveryone in the goal becomes a {}.\n{}",
                presentation::safe(role),
                roles::role_command_line(
                    "Undo for one member",
                    "take",
                    &goal_cut,
                    &selectors::member_key_prefix(&observed.members, member.member),
                    role,
                )
            ));
        }
    }
    Ok(confirm::Plan {
        command: "rules bind",
        review: json!({"goal":goal,"title":observed.title,"host":observed.host,
            "current_rules":current,"formation":formation_name,
            "formation_json":source,"inputs":inputs,"no_role":no_role,
            "members":observed.members,"roles":observed.roles,
            "role_changes":role_lines,"file_changes":file_changes,
            "workspace_epoch":workspace.and_then(|workspace| workspace.epoch),
            "workspace_policy":workspace.and(formation.workspace.as_ref())}),
        human: format!(
            "Bind \"{}\" ({goal_cut}) to {}. Open tasks keep their old rules until revised.{}{}",
            presentation::safe(observed.title.as_deref().unwrap_or("this goal")),
            presentation::safe(formation_name),
            file_lines,
            role_lines
        ),
        warning: None,
        again: String::new(),
    })
}

fn rules_bind(
    matches: &ArgMatches,
    args: &ArgMatches,
    client: &mut LocalClient,
    socket: &Path,
    owner: bool,
) -> Result<Output, Failure> {
    let goal = resolve_goal(client, socket, value(args, "goal"), None)?;
    let (name, source, _) = chosen_formation(args)?;
    let mut source =
        source.ok_or_else(|| Failure::usage("rules bind needs --formation or --formation-json"))?;
    let inputs: BTreeMap<String, BlobHash> = json_map(args, "inputs")?;
    let plan = if owner {
        let plan = rules_plan(
            client,
            socket,
            goal,
            &name,
            &source,
            &inputs,
            args.get_flag("no-role"),
        )?;
        if let Some(output) = reviewed(matches, args, &plan, || {
            rules_plan(
                client,
                socket,
                goal,
                &name,
                &source,
                &inputs,
                args.get_flag("no-role"),
            )
        })? {
            return Ok(output);
        }
        source = plan.review["formation_json"]
            .as_str()
            .expect("reviewed formation")
            .into();
        Some(plan)
    } else {
        None
    };
    let current = if let Some(plan) = &plan {
        serde_json::from_value::<EventId>(plan.review["current_rules"].clone())
            .map_err(|error| Failure::internal(format!("reviewed rules: {error}")))?
    } else {
        observed(client, socket, goal)?
            .current_rules
            .unwrap_or(EventId([0; 32]))
    };
    let response = call(
        client,
        socket,
        Request::RulesBind {
            no_role: args.get_flag("no-role"),
            goal,
            expected: current,
            formation_json: source,
            inputs,
        },
        idempotency(matches)?,
    )?;
    let title = plan
        .as_ref()
        .and_then(|plan| plan.review["title"].as_str())
        .unwrap_or("this goal");
    let mut human = format!(
        "\"{}\" now follows {}. Open tasks keep their old rules until revised.",
        presentation::safe(title),
        presentation::safe(&name)
    );
    if let Some(plan) = &plan
        && !plan.review["workspace_policy"].is_null()
    {
        human.push_str(" The shared files follow them too.");
    }
    if let Some(plan) = &plan {
        for field in ["file_changes", "role_changes"] {
            if let Some(text) = plan.review[field].as_str() {
                human.push_str(text);
            }
        }
    }
    Ok(Output::success(json!(response), human))
}

fn task_detail(
    client: &mut LocalClient,
    socket: &Path,
    goal: GoalId,
    task: TaskId,
) -> Result<locust_proto::api::TaskDetail, Failure> {
    let Response::Task(detail) = call(client, socket, Request::Task { goal, task }, None)? else {
        unreachable!("typed response")
    };
    Ok(detail)
}

fn revision_plan(
    client: &mut LocalClient,
    socket: &Path,
    goal: GoalId,
    task: TaskId,
    task_type: &Option<String>,
) -> Result<confirm::Plan, Failure> {
    let observed = observed(client, socket, goal)?;
    let detail = task_detail(client, socket, goal, task)?;
    let title = detail.view.title.as_deref().unwrap_or("this task");
    let under_parent = detail.parent.is_some();
    Ok(confirm::Plan {
        command: "task revise",
        review: json!({"goal":goal,"goal_title":observed.title,"task":task,
            "task_title":detail.view.title,"round":detail.view.context.round,
            "parent":detail.parent,"task_type":task_type}),
        human: if under_parent {
            format!(
                "Revise \"{}\" under its parent task's rules. Attempts on its old round are superseded.",
                presentation::safe(title)
            )
        } else {
            format!(
                "Revise \"{}\" under the current rules. Attempts on its old round are superseded.",
                presentation::safe(title)
            )
        },
        warning: None,
        again: String::new(),
    })
}

fn task_revise(
    matches: &ArgMatches,
    args: &ArgMatches,
    client: &mut LocalClient,
    socket: &Path,
    owner: bool,
) -> Result<Output, Failure> {
    let goal = resolve_goal(client, socket, value(args, "goal"), None)?;
    let task = selectors::resolve_task(client, socket, goal, value(args, "task"), None)?;
    let task_type = args.get_one::<String>("task-type").cloned();
    let plan = if owner {
        let plan = revision_plan(client, socket, goal, task, &task_type)?;
        if let Some(output) = reviewed(matches, args, &plan, || {
            revision_plan(client, socket, goal, task, &task_type)
        })? {
            return Ok(output);
        }
        Some(plan)
    } else {
        None
    };
    let round = if let Some(plan) = &plan {
        serde_json::from_value::<EventId>(plan.review["round"].clone())
            .map_err(|error| Failure::internal(format!("reviewed round: {error}")))?
    } else {
        task_detail(client, socket, goal, task)?.view.context.round
    };
    let response = call(
        client,
        socket,
        Request::TaskRevise {
            goal,
            task,
            expected_round: round,
            task_type,
        },
        idempotency(matches)?,
    )?;
    let title = plan
        .as_ref()
        .and_then(|plan| plan.review["task_title"].as_str())
        .unwrap_or("this task");
    let subtask = plan
        .as_ref()
        .is_some_and(|plan| !plan.review["parent"].is_null());
    let human = if subtask {
        format!(
            "\"{}\" has a new round under its parent task's rules. Attempts on its old round are superseded.",
            presentation::safe(title)
        )
    } else {
        format!(
            "\"{}\" follows the current rules now. Attempts on its old round are superseded.",
            presentation::safe(title)
        )
    };
    Ok(Output::success(json!(response), human))
}

/// The goals this agent was started with. Disconnecting it stops no host
/// command in them; the goal's own key signs those.
fn hosted_goals(
    client: &mut LocalClient,
    socket: &Path,
    known: &DaemonStatus,
    agent: PublicKey,
) -> Result<Vec<serde_json::Value>, Failure> {
    let mut hosted = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for entry in &known.goals {
        if seen.insert(entry.goal) {
            let goal = observed(client, socket, entry.goal)?;
            if goal.host == Some(agent) {
                hosted.push(json!({"goal":entry.goal,"title":goal.title}));
            }
        }
    }
    Ok(hosted)
}

/// Applies at once, like `level` and `allow`: `agent reconnect` undoes it.
fn agent_revoke(
    matches: &ArgMatches,
    client: &mut LocalClient,
    socket: &Path,
) -> Result<Output, Failure> {
    let agent = acting_agent(client, socket, matches, None)?;
    let known = status(client, socket, None)?;
    let selected = known
        .agents
        .iter()
        .find(|entry| entry.agent == agent)
        .ok_or_else(|| Failure::new(ErrorCode::NotFound, "agent is not enrolled"))?;
    let name = presentation::safe(&selected.name);
    let hosted = hosted_goals(client, socket, &known, agent)?;
    let changed = !selected.revoked;
    call(
        client,
        socket,
        Request::AgentRevoke { agent },
        idempotency(matches)?,
    )?;
    Ok(Output::success(
        json!({"agent":agent,"name":selected.name,"connected":false,"changed":changed,
            "hosted_goals":hosted}),
        format!(
            "{name} is disconnected. The name stays taken.\nUndo: locust --owner agent reconnect --agent {}",
            shell_word(&selected.name)
        ),
    ))
}

/// Connects a disconnected agent again under the name and key it had.
fn agent_reconnect(
    matches: &ArgMatches,
    client: &mut LocalClient,
    socket: &Path,
) -> Result<Output, Failure> {
    let agent = acting_agent(client, socket, matches, None)?;
    let known = status(client, socket, None)?;
    let selected = known
        .agents
        .iter()
        .find(|entry| entry.agent == agent)
        .ok_or_else(|| Failure::new(ErrorCode::NotFound, "agent is not enrolled"))?;
    let name = presentation::safe(&selected.name);
    let changed = selected.revoked;
    let human = if changed {
        call(
            client,
            socket,
            Request::AgentReconnect { agent },
            idempotency(matches)?,
        )?;
        format!(
            "{name} is connected again.\nUndo: locust --owner agent revoke --agent {}",
            shell_word(&selected.name)
        )
    } else {
        format!("{name} is not disconnected. Nothing changed.")
    };
    Ok(Output::success(
        json!({"agent":agent,"name":selected.name,"connected":true,"changed":changed}),
        human,
    ))
}

fn invitation_revoke(
    matches: &ArgMatches,
    args: &ArgMatches,
    client: &mut LocalClient,
    socket: &Path,
    owner: bool,
) -> Result<Output, Failure> {
    if matches.get_one::<String>("agent").is_some() {
        return Err(Failure::usage(
            "host commands are the host's own and name no agent; drop --agent",
        ));
    }
    let goal = resolve_goal(client, socket, value(args, "goal"), None)?;
    let (title, was_pending) = if owner {
        let observed = observed(client, socket, goal)?;
        let Response::Invitations { invitations } =
            call(client, socket, Request::GoalInvitations { goal }, None)?
        else {
            unreachable!("typed response")
        };
        let was_pending = args.get_one::<String>("invitation").is_some_and(|id| {
            invitations
                .iter()
                .any(|item| item.invitation == *id && item.state == InvitationState::Pending)
        });
        (observed.title, was_pending)
    } else {
        (None, false)
    };
    let response = call(
        client,
        socket,
        Request::InvitationRevoke {
            goal,
            invitation: args.get_one::<String>("invitation").cloned(),
        },
        idempotency(matches)?,
    )?;
    let count = match &response {
        Response::InvitationsRevoked { count } => *count,
        Response::InvitationRevoked { .. } => u32::from(was_pending),
        _ => unreachable!("typed response"),
    };
    let title = presentation::safe(title.as_deref().unwrap_or("this goal"));
    let mut human =
        format!("Stopped admission to \"{title}\": {count} invitations revoked. Members stay.");
    if count > 0 {
        human.push_str(&format!(
            "\nInvite again: locust --owner goal invite --goal {}",
            cut_goal(client, socket, goal)
        ));
    }
    Ok(Output::success(json!(response), human))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_retry_keys_distinguish_the_invited_role() {
        let view = GoalStatus {
            guard: vec![],
            restored: None,
            goal: GoalId([1; 32]),
            title: None,
            governance: PublicKey([2; 32]),
            hosted_here: true,
            host: Some(PublicKey([3; 32])),
            host_name: Some("Host".into()),
            roles: Default::default(),
            deciding: Default::default(),
            acting_alone: Default::default(),
            governance_head: None,
            current_rules: None,
            scope_halts: vec![],
            members: vec![],
            halted: None,
            workspace: None,
            abilities: vec![],
            stalled: vec![],
            peers: vec![],
        };
        let agent = PublicKey([4; 32]);
        let plain = add_retry_key(&view, agent, 123, None);
        let reviewer = add_retry_key(&view, agent, 123, Some("reviewer"));
        assert_ne!(plain, reviewer);
        assert_ne!(reviewer, add_retry_key(&view, agent, 123, Some("lead")));
        assert_eq!(reviewer, add_retry_key(&view, agent, 123, Some("reviewer")));
    }

    #[test]
    fn invitation_expiry_requires_a_finite_positive_duration() {
        assert_eq!(duration_ms("1h").unwrap(), 3_600_000);
        assert_eq!(duration_ms("7d").unwrap(), 604_800_000);
        for value in ["never", "0h", "1m", "3", "999999999999999999999d", "🦋"] {
            assert!(duration_ms(value).is_err(), "{value}");
        }
    }
}
