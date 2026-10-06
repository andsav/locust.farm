//! Person-facing commands that change membership, rules, or durable identity.

use super::{
    LocalClient, Output, acting_agent, confirm, connection, invitations, presentation, print,
    resolve_goal, selectors, status,
};
use crate::failure::Failure;
use clap::{Arg, ArgMatches, Command};
use locust_proto::api::{
    Caller, DaemonStatus, ErrorCode, GoalStatus, InvitationState, Membership, Request, Response,
};
use locust_proto::event::TaskId;
use locust_proto::id::{BlobHash, EventId, GoalId, IdempotencyKey, PublicKey};
use locust_proto::local;
use locust_proto::organization::Formation;
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
        .arg(
            Arg::new("roles")
                .long("roles")
                .help("Role bindings as JSON"),
        )
}

pub(super) fn commands() -> Vec<(&'static str, Command)> {
    vec![
        (
            "goal",
            confirm::flags(formation(
                Command::new("create").arg(option("title", "Goal title")),
            )),
        ),
        (
            "goal",
            confirm::flags(Command::new("add").arg(goal_option())),
        ),
        (
            "goal",
            confirm::flags(invitations::ticket_input(Command::new("join"))),
        ),
        (
            "goal",
            confirm::flags(Command::new("leave").arg(goal_option())),
        ),
        (
            "goal",
            confirm::flags(
                Command::new("invite").arg(goal_option()).arg(
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
            confirm::flags(formation(Command::new("bind").arg(goal_option()))),
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
        ("agent", confirm::flags(Command::new("revoke"))),
    ]
}

pub(super) fn owns(operation: &str) -> bool {
    matches!(
        operation,
        "goal.create"
            | "goal.add"
            | "goal.join"
            | "goal.leave"
            | "goal.invite"
            | "member.remove"
            | "rules.bind"
            | "task.revise"
            | "invitation.revoke"
            | "agent.revoke"
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
    if matches!(operation, "goal.add" | "agent.revoke")
        && (!matches.get_flag("owner") || matches.get_one::<String>("agent").is_none())
    {
        return Err(Failure::usage(format!(
            "{operation} requires --owner --agent NAME"
        )));
    }
    if matches.get_one::<String>("agent").is_some()
        && matches!(
            operation,
            "goal.invite" | "member.remove" | "rules.bind" | "task.revise"
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
        "goal.add" => goal_add(matches, args, &mut client, &socket),
        "goal.join" => goal_join(matches, args, &mut client, &socket, owner),
        "goal.leave" => goal_leave(matches, args, &mut client, &socket, owner),
        "goal.invite" => goal_invite(matches, args, &mut client, &socket, owner),
        "member.remove" => member_remove(matches, args, &mut client, &socket, owner),
        "rules.bind" => rules_bind(matches, args, &mut client, &socket, owner),
        "task.revise" => task_revise(matches, args, &mut client, &socket, owner),
        "invitation.revoke" => invitation_revoke(matches, args, &mut client, &socket, owner),
        "agent.revoke" => agent_revoke(matches, args, &mut client, &socket),
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
        let parsed: Formation = serde_json::from_str(source)
            .map_err(|error| Failure::usage(format!("--formation-json: {error}")))?;
        return Ok(("custom".into(), Some(source.clone()), parsed));
    }
    Ok(("open".into(), None, Formation::default()))
}

fn short_goal(goal: GoalId) -> String {
    goal.to_string().chars().take(8).collect()
}

struct CreateSelection<'a> {
    agent: PublicKey,
    title: &'a str,
    formation_name: &'a str,
    formation_json: &'a Option<String>,
    formation: &'a Formation,
    roles: &'a BTreeMap<String, Vec<PublicKey>>,
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
            "roles":selected.roles,"inputs":selected.inputs}),
        human: format!(
            "Start \"{}\" with {} as the host's agent, using {}. {}",
            presentation::safe(selected.title),
            name_for(&known, selected.agent),
            presentation::safe(selected.formation_name),
            presentation::counts_when(&selected.formation.decisions.completion)
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
    let roles: BTreeMap<String, Vec<PublicKey>> = json_map(args, "roles")?;
    let inputs: BTreeMap<String, BlobHash> = json_map(args, "inputs")?;
    let agent = if owner {
        acting_agent(client, socket, matches, None)?
    } else {
        current_agent(client)?
    };
    if owner {
        let selected = CreateSelection {
            agent,
            title,
            formation_name: &formation_name,
            formation_json: &formation_json,
            formation: &formation,
            roles: &roles,
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
            formation_json,
            roles,
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
            "Started \"{}\" ({}). Host: you. This computer keeps who is in and the rules.",
            presentation::safe(title),
            short_goal(goal)
        ),
    ))
}

fn add_plan(
    client: &mut LocalClient,
    socket: &Path,
    goal: GoalId,
    agent: PublicKey,
) -> Result<confirm::Plan, Failure> {
    let known = status(client, socket, None)?;
    let selected = known
        .agents
        .iter()
        .find(|candidate| candidate.agent == agent && !candidate.revoked && !candidate.author_only)
        .ok_or_else(|| {
            Failure::new(ErrorCode::NotFound, "select an active enrolled local agent")
        })?;
    let goal_status = observed(client, socket, goal)?;
    let host_here = known
        .agents
        .iter()
        .any(|candidate| candidate.agent == goal_status.host && !candidate.revoked)
        && goal_status
            .members
            .iter()
            .any(|member| member.member == goal_status.host && member.local);
    if !host_here {
        return Err(Failure::new(
            ErrorCode::Denied,
            "this goal's host is not an active local agent; request an invitation from its owner",
        ));
    }
    let standing = known
        .goals
        .iter()
        .find(|entry| entry.goal == goal && entry.member == agent)
        .map(|entry| entry.membership);
    let joined = standing == Some(Membership::Member);
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
    Ok(confirm::Plan {
        command: "goal add",
        review: json!({"goal":goal,"title":goal_status.title,"host":goal_status.host,
            "agent":agent,"name":selected.name,"already_member":joined}),
        human: format!(
            "Add {} to \"{}\" ({}). This shares the whole goal's history and content. Local files and private chats stay here.",
            presentation::safe(&selected.name),
            presentation::safe(title),
            short_goal(goal)
        ),
        warning: None,
        again: String::new(),
    })
}

fn add_retry_key(observed: &GoalStatus, agent: PublicKey, expires_ms: u64) -> IdempotencyKey {
    let bytes = serde_json::to_vec(&(
        "locust-goal-add-v2",
        observed.goal,
        observed.host,
        observed.governance_head,
        agent,
        expires_ms,
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
    let plan = add_plan(client, socket, goal, agent)?;
    if let Some(output) = reviewed(matches, args, &plan, || {
        add_plan(client, socket, goal, agent)
    })? {
        return Ok(output);
    }
    let name = plan.review["name"].as_str().unwrap_or("agent");
    let title = plan.review["title"].as_str().unwrap_or("this goal");
    if plan.review["already_member"] == true {
        return Ok(Output::success(
            json!({"goal":goal,"agent":agent,"membership":"member","changed":false}),
            format!(
                "{} is already a member of \"{}\".",
                presentation::safe(name),
                presentation::safe(title)
            ),
        ));
    }
    let goal_status = observed(client, socket, goal)?;
    let now = now_ms()?;
    let expires_ms = (now / 86_400_000 + 2) * 86_400_000;
    let Response::Invited { ticket } = call(
        client,
        socket,
        Request::GoalInvite { goal, expires_ms },
        Some(add_retry_key(&goal_status, agent, expires_ms)),
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
    if preview.goal != goal || preview.governance != goal_status.host {
        return Err(Failure::new(
            ErrorCode::Conflict,
            "local invitation differs from the reviewed goal",
        ));
    }
    let Response::Joined {
        goal: joined_goal,
        membership,
        ..
    } = call(client, socket, Request::GoalJoin { agent, ticket }, None)?
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
        json!({"goal":goal,"agent":agent,"membership":"member","changed":true}),
        format!(
            "{} joined \"{}\".",
            presentation::safe(name),
            presentation::safe(title)
        ),
    ))
}

fn join_plan(
    client: &mut LocalClient,
    socket: &Path,
    preview: &locust_proto::api::InvitationPreview,
    agent: PublicKey,
) -> Result<confirm::Plan, Failure> {
    let known = status(client, socket, None)?;
    let standing = known
        .goals
        .iter()
        .find(|entry| entry.goal == preview.goal && entry.member == agent)
        .map(|entry| entry.membership);
    Ok(confirm::Plan {
        command: "goal join",
        review: json!({"invitation_review":preview.review,"goal":preview.goal,
            "title":preview.goal_title,"agent":agent,"standing":standing}),
        human: format!(
            "Join \"{}\" ({}) as {}. This shares what this agent posts with the goal's members. {}",
            presentation::safe(preview.goal_title.as_deref().unwrap_or("this goal")),
            short_goal(preview.goal),
            name_for(&known, agent),
            preview.sharing_facts.join(" ")
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
    if owner {
        let plan = join_plan(client, socket, &preview, agent)?;
        if args
            .get_one::<String>("ticket")
            .is_some_and(|source| source == "-")
            && args.get_one::<String>("confirm").is_none()
        {
            return Ok(plan.shown());
        }
        if let Some(output) = reviewed(matches, args, &plan, || {
            join_plan(client, socket, &preview, agent)
        })? {
            return Ok(output);
        }
    }
    let response = call(
        client,
        socket,
        Request::GoalJoin { agent, ticket },
        idempotency(matches)?,
    )?;
    let result = json!(response);
    let Response::Joined { membership, .. } = response else {
        unreachable!("typed response")
    };
    let name = if owner {
        name_for(&status(client, socket, None)?, agent)
    } else {
        agent.to_string()
    };
    let title = presentation::safe(preview.goal_title.as_deref().unwrap_or("this goal"));
    let human = if membership == Membership::Member {
        format!("{name} joined \"{title}\".")
    } else {
        format!(
            "Joining \"{title}\" as {name}. Admission comes from the host's computer; locust --owner status shows it."
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
            short_goal(goal),
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
    Ok(confirm::Plan {
        command: "goal invite",
        review: json!({"goal":goal,"title":observed.title,"host":observed.host,
            "duration":typed_duration,"issued_invitations":issued,
            "pending_invitations":pending}),
        human: format!(
            "Invite someone to \"{}\" ({}). The ticket shares this goal's history and content with whoever presents it. Expires after {}. Invitations issued: {issued}. Pending invitations: {pending}.",
            presentation::safe(observed.title.as_deref().unwrap_or("this goal")),
            short_goal(goal),
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
    if owner {
        let plan = invite_plan(client, socket, goal, duration)?;
        if let Some(output) = reviewed(matches, args, &plan, || {
            invite_plan(client, socket, goal, duration)
        })? {
            return Ok(output);
        }
    }
    let expires_ms = now_ms()?
        .checked_add(lifetime)
        .ok_or_else(|| Failure::usage("--expires is too large"))?;
    let response = call(
        client,
        socket,
        Request::GoalInvite { goal, expires_ms },
        idempotency(matches)?,
    )?;
    let Response::Invited { ticket } = &response else {
        unreachable!("typed response")
    };
    let warning = format!(
        "Anyone who presents this ticket is admitted while this computer is on, until {}. Send it privately. Stop admission: locust --owner invitation revoke --goal {} --all",
        presentation::utc(expires_ms),
        short_goal(goal)
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
    let present = observed.members.iter().any(|entry| entry.member == member);
    Ok(confirm::Plan {
        command: "member remove",
        review: json!({"goal":goal,"title":observed.title,"host":observed.host,
            "member":member,"member_present":present}),
        human: format!(
            "Remove member {} from \"{}\" ({}). Copies already received cannot be retracted.",
            member.to_string().chars().take(8).collect::<String>(),
            presentation::safe(observed.title.as_deref().unwrap_or("this goal")),
            short_goal(goal)
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
    let title = plan
        .as_ref()
        .and_then(|plan| plan.review["title"].as_str())
        .unwrap_or("this goal");
    Ok(Output::success(
        json!(response),
        format!(
            "Removed {} from \"{}\". Copies already received cannot be retracted.",
            member.to_string().chars().take(8).collect::<String>(),
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
    roles: &BTreeMap<String, Vec<PublicKey>>,
    inputs: &BTreeMap<String, BlobHash>,
) -> Result<confirm::Plan, Failure> {
    let observed = observed(client, socket, goal)?;
    let current = observed
        .current_rules
        .ok_or_else(|| Failure::unavailable("current rules have not arrived"))?;
    Ok(confirm::Plan {
        command: "rules bind",
        review: json!({"goal":goal,"title":observed.title,"host":observed.host,
            "current_rules":current,"formation":formation_name,
            "formation_json":formation_json,"roles":roles,"inputs":inputs}),
        human: format!(
            "Bind \"{}\" ({}) to {}. Open tasks keep their old rules until revised.",
            presentation::safe(observed.title.as_deref().unwrap_or("this goal")),
            short_goal(goal),
            presentation::safe(formation_name)
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
    let source =
        source.ok_or_else(|| Failure::usage("rules bind needs --formation or --formation-json"))?;
    let roles: BTreeMap<String, Vec<PublicKey>> = json_map(args, "roles")?;
    let inputs: BTreeMap<String, BlobHash> = json_map(args, "inputs")?;
    let plan = if owner {
        let plan = rules_plan(client, socket, goal, &name, &source, &roles, &inputs)?;
        if let Some(output) = reviewed(matches, args, &plan, || {
            rules_plan(client, socket, goal, &name, &source, &roles, &inputs)
        })? {
            return Ok(output);
        }
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
            goal,
            expected: current,
            formation_json: source,
            roles,
            inputs,
        },
        idempotency(matches)?,
    )?;
    let title = plan
        .as_ref()
        .and_then(|plan| plan.review["title"].as_str())
        .unwrap_or("this goal");
    Ok(Output::success(
        json!(response),
        format!(
            "\"{}\" now follows {}. Open tasks keep their old rules until revised.",
            presentation::safe(title),
            presentation::safe(&name)
        ),
    ))
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

fn revoke_plan(
    client: &mut LocalClient,
    socket: &Path,
    agent: PublicKey,
) -> Result<confirm::Plan, Failure> {
    let known = status(client, socket, None)?;
    let selected = known
        .agents
        .iter()
        .find(|entry| entry.agent == agent)
        .ok_or_else(|| Failure::new(ErrorCode::NotFound, "agent is not enrolled"))?;
    let mut hosted = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for entry in &known.goals {
        if seen.insert(entry.goal) {
            let goal = observed(client, socket, entry.goal)?;
            if goal.host == agent {
                hosted.push(json!({"goal":entry.goal,"title":goal.title}));
            }
        }
    }
    let names: Vec<_> = hosted
        .iter()
        .filter_map(|item| item["title"].as_str())
        .map(presentation::safe)
        .collect();
    Ok(confirm::Plan {
        command: "agent revoke",
        review: json!({"agent":agent,"name":selected.name,"already_revoked":selected.revoked,
            "hosted_goals":hosted}),
        human: format!(
            "Disconnect {}. The name stays taken.",
            presentation::safe(&selected.name)
        ),
        warning: if names.is_empty() {
            None
        } else {
            Some(format!(
                "Goals {} hosts freeze for everyone: nobody joins and the rules cannot change.",
                presentation::safe(&selected.name)
            ))
        },
        again: String::new(),
    })
}

fn agent_revoke(
    matches: &ArgMatches,
    args: &ArgMatches,
    client: &mut LocalClient,
    socket: &Path,
) -> Result<Output, Failure> {
    let agent = acting_agent(client, socket, matches, None)?;
    let plan = revoke_plan(client, socket, agent)?;
    if let Some(output) = reviewed(matches, args, &plan, || revoke_plan(client, socket, agent))? {
        return Ok(output);
    }
    let response = call(
        client,
        socket,
        Request::AgentRevoke { agent },
        idempotency(matches)?,
    )?;
    let name = plan.review["name"].as_str().unwrap_or("agent");
    Ok(Output::success(
        json!(response),
        format!(
            "{} is disconnected. The name stays taken.",
            presentation::safe(name)
        ),
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
            short_goal(goal)
        ));
    }
    Ok(Output::success(json!(response), human))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitation_expiry_requires_a_finite_positive_duration() {
        assert_eq!(duration_ms("1h").unwrap(), 3_600_000);
        assert_eq!(duration_ms("7d").unwrap(), 604_800_000);
        for value in ["never", "0h", "1m", "3", "999999999999999999999d", "🦋"] {
            assert!(duration_ms(value).is_err(), "{value}");
        }
    }
}
