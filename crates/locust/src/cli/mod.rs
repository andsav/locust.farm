//! Command-line client: explicit authority, one JSON envelope, typed API.
// A print macro panics when its reader has gone away; write through `print`.
#![deny(clippy::print_stdout, clippy::print_stderr)]
mod args;
mod client;
mod confirm;
mod connection;
mod doctor;
mod farm;
mod formation;
mod hook;
mod install;
mod invitations;
mod onboarding;
mod only_you;
mod package;
mod presentation;
mod print;
mod roles;
mod selectors;
mod service;
mod setup;
mod watch;
mod workspace;

use crate::{daemon, failure::Failure, secret};
use clap::{ArgMatches, error::ErrorKind};
use locust_proto::api::{
    Audience, Credential, DaemonStatus, ErrorCode, Membership, OPERATIONS, Request, Response,
    SessionSecret, Voice, WaitOutcome,
};
use locust_proto::client::Client;
use locust_proto::id::{GoalId, IdempotencyKey, PublicKey};
use locust_proto::local;
use selectors::{resolve_goal, validate_goal};
use serde_json::{Map, Value, json};
use std::io::{self, Read};
use std::path::Path;

type LocalClient = Client<std::os::unix::net::UnixStream>;
#[derive(Debug)]
struct Output {
    result: Value,
    human: String,
    status: u8,
    ok: bool,
}
impl Output {
    fn success(result: Value, human: String) -> Self {
        Self {
            result,
            human,
            status: 0,
            ok: true,
        }
    }
}

pub(super) fn run() -> u8 {
    let arguments: Vec<_> = std::env::args_os().collect();
    let mcp_mode = mcp_invocation(&arguments);
    let hook_mode = command_invocation(&arguments, "hook");
    let json_mode = !mcp_mode && arguments.iter().any(|arg| arg == "--json");
    let matches = match args::command().try_get_matches_from(arguments) {
        Ok(matches) => matches,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            let written = if mcp_mode {
                print::stderr(&error)
            } else if json_mode {
                let key = if error.kind() == ErrorKind::DisplayVersion {
                    "version"
                } else {
                    "help"
                };
                print::stdout(format_args!(
                    "{}\n",
                    json!({"ok": true, "result": {key: error.to_string().trim_end()}})
                ))
            } else {
                print::stdout(&error)
            };
            return print::status(written, 0);
        }
        Err(_) if hook_mode => {
            if std::env::var_os("LOCUST_HOOKS").is_none_or(|value| value != "off") {
                let _ = print::stdout(format_args!("Locust context was NOT injected\n"));
            }
            return 0;
        }
        Err(error) => return print_failure(Failure::usage(error.to_string()), json_mode),
    };
    if let Some(("mcp", selected)) = matches.subcommand() {
        return match run_mcp(&matches, selected) {
            Ok(()) => 0,
            Err(error) => print_failure(error, false),
        };
    }
    if let Some(("hook", selected)) = matches.subcommand() {
        return hook::run(&matches, selected);
    }
    match execute(&matches) {
        Ok(output) => {
            let written = if matches.get_flag("json") {
                print::stdout(format_args!(
                    "{}\n",
                    json!({"ok": output.ok, "result": output.result})
                ))
            } else if !output.human.is_empty() {
                print::stdout(format_args!("{}\n", output.human))
            } else {
                Ok(())
            };
            print::status(written, output.status)
        }
        Err(error) if matches.get_flag("owner") && !matches.get_flag("json") => {
            print_failure(error.for_person(), false)
        }
        Err(error) => print_failure(error, matches.get_flag("json")),
    }
}
// Classify parse failures without confusing an option value or command body
// named "mcp" with the transport subcommand. Successful parsing uses Clap's
// actual selected subcommand for dispatch.
fn mcp_invocation(arguments: &[std::ffi::OsString]) -> bool {
    command_invocation(arguments, "mcp")
}
fn command_invocation(arguments: &[std::ffi::OsString], command: &str) -> bool {
    let mut arguments = arguments.iter().skip(1);
    while let Some(argument) = arguments.next() {
        let Some(argument) = argument.to_str() else {
            return false;
        };
        if matches!(
            argument,
            "--home" | "--credential" | "--agent" | "--session" | "--idempotency-key"
        ) {
            arguments.next();
        } else if argument == "--" {
            return arguments.next().is_some_and(|argument| argument == command);
        } else if !argument.starts_with('-') {
            return argument == command;
        }
    }
    false
}
fn run_mcp(matches: &ArgMatches, selected: &ArgMatches) -> Result<(), Failure> {
    if matches.get_flag("owner")
        || matches.get_one::<String>("agent").is_some()
        || matches.get_flag("json")
        || matches.get_one::<String>("idempotency-key").is_some()
    {
        return Err(Failure::usage(
            "mcp does not accept --owner, --agent, --json or --idempotency-key",
        ));
    }
    let home = connection::home(matches)?;
    let credential = connection::credential_path(matches, &home)?;
    let session = connection::session_path(matches)?;
    crate::mcp::run(crate::mcp::Config {
        home,
        credential,
        session,
        lifecycle_receipt: selected
            .get_one::<String>("lifecycle-receipt")
            .map(std::path::PathBuf::from),
    })
}
fn print_failure(error: Failure, json_mode: bool) -> u8 {
    let written = if json_mode {
        print::stdout(format_args!(
            "{}\n",
            json!({"ok": false, "error": {"code": error.code.as_str(), "message": error.message, "details": error.details_json.as_deref().and_then(|text| serde_json::from_str::<Value>(text).ok())}})
        ))
    } else {
        print::stderr(format_args!("locust: {error}{}\n", role_details(&error)))
    };
    print::status(written, error.exit_status())
}
/// The role a refusal is about, and the roles the goal has, which the daemon
/// carries only as details: a person reads them after the sentence.
fn role_details(error: &Failure) -> String {
    let Some(details) = error
        .details_json
        .as_deref()
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
    else {
        return String::new();
    };
    let mut tail = String::new();
    if let Some(role) = details.get("role").and_then(Value::as_str) {
        tail.push_str(&format!(" Role: {}", presentation::safe(role)));
    }
    if let Some(roles) = details.get("roles").and_then(Value::as_array) {
        let roles: Vec<_> = roles
            .iter()
            .filter_map(Value::as_str)
            .map(presentation::safe)
            .collect();
        if !roles.is_empty() {
            tail.push_str(&format!(" Roles here: {}", roles.join(", ")));
        }
    }
    tail
}
fn stdin_text() -> Result<String, Failure> {
    let mut text = String::new();
    io::stdin()
        .read_to_string(&mut text)
        .map_err(|error| Failure::invalid(format!("standard input: {error}")))?;
    Ok(text)
}
fn execute(matches: &ArgMatches) -> Result<Output, Failure> {
    if matches.get_one::<String>("agent").is_some() && !matches.get_flag("owner") {
        return Err(Failure::usage("--agent requires --owner"));
    }
    let (operation, selected) = args::selected(matches);
    if operation.starts_with("farm.") {
        return farm::run(matches, &operation, selected);
    }
    if operation == "contract" {
        let mut contract = locust_proto::api::contract();
        contract["cli"] = args::contract();
        let human = if matches.get_flag("json") {
            String::new()
        } else {
            serde_json::to_string_pretty(&contract)
                .map_err(|error| Failure::internal(format!("cannot render contract: {error}")))?
        };
        return Ok(Output::success(contract, human));
    }
    if matches!(
        operation.as_str(),
        "formation.contract"
            | "formation.schema"
            | "formation.examples"
            | "formation.example"
            | "formation.validate"
            | "formation.explain"
            | "formation.normalize"
            | "formation.diff"
    ) {
        return formation::run(&operation, selected);
    }
    if operation == "up" || operation == "agent.add" {
        return onboarding::run(matches, &operation, selected);
    }
    if operation.starts_with("setup.") {
        return setup::run(&operation, selected);
    }
    if operation.starts_with("service.") {
        return service::run(&operation, selected);
    }
    if operation.starts_with("install.") {
        return install::run(&operation, selected);
    }
    if operation.starts_with("package.") {
        return package::run(&operation, selected);
    }
    if operation.starts_with("workspace.") {
        return workspace::run(matches, &operation, selected);
    }
    if operation.starts_with("client.") {
        return client::run(matches, &operation, selected);
    }
    if only_you::owns(&operation) {
        return only_you::run(matches, &operation, selected);
    }
    if operation.starts_with("invitation.") {
        return invitations::run(matches, &operation, selected);
    }
    if operation == "watch" {
        return watch::run(matches, selected);
    }
    let named_enrollment = matches!(operation.as_str(), "agent.enroll" | "author.enroll");
    let author_enrollment = operation == "author.enroll";
    let generic_call = operation == "call";
    if operation == "session.create" {
        return create_session(selected.get_one::<String>("path").expect("required path"));
    }
    let home = connection::home(matches)?;
    if operation == "daemon.run" {
        daemon::run(&home)?;
        return Ok(Output::success(json!("done"), String::new()));
    }
    if operation == "doctor" {
        let (result, human, status, ok) = doctor::run(matches, &home);
        return Ok(Output {
            result,
            human,
            status,
            ok,
        });
    }
    let idempotency = matches
        .get_one::<String>("idempotency-key")
        .map(|value| {
            value
                .parse::<IdempotencyKey>()
                .map_err(|_| Failure::usage("--idempotency-key requires 16 bytes in hex"))
        })
        .transpose()?;
    let (operation, mut fields) = if operation == "call" {
        let operation = selected
            .get_one::<String>("operation")
            .expect("required operation")
            .clone();
        let text = selected
            .get_one::<String>("fields")
            .expect("default fields");
        let text = if text == "-" {
            stdin_text()?
        } else {
            text.clone()
        };
        let fields: Value = serde_json::from_str(&text)
            .map_err(|error| Failure::usage(format!("request JSON: {error}")))?;
        let fields = fields
            .as_object()
            .cloned()
            .ok_or_else(|| Failure::usage("request fields must be one JSON object"))?;
        (operation, fields)
    } else {
        let fields = args::values(&operation, selected)?;
        (operation, fields)
    };
    if !generic_call
        && let Some(field) = args::text_field(&operation)
        && fields.get(field).and_then(Value::as_str) == Some("-")
    {
        fields.insert(field.to_owned(), Value::String(stdin_text()?));
    }
    let receipts =
        if !generic_call && matches!(operation.as_str(), "context.read" | "context.acknowledge") {
            let credential = Credential(connection::read_secret(&connection::credential_path(
                matches, &home,
            )?)?);
            let session = connection::session_path(matches)?
                .as_deref()
                .map(connection::read_secret)
                .transpose()?
                .map(locust_proto::api::SessionSecret);
            Some(crate::context_receipts::Cache::new(
                &home,
                credential,
                session,
                crate::context_receipts::Surface::Cli,
            ))
        } else {
            None
        };
    if !generic_call && operation == "context.acknowledge" {
        let reference = fields
            .get("receipt")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                Failure::usage("--receipt requires the reference returned by context read")
            })?;
        fields.insert(
            "receipt".into(),
            serde_json::to_value(receipts.as_ref().unwrap().load(reference)?)
                .map_err(|error| Failure::internal(error.to_string()))?,
        );
    }
    if generic_call {
        request(&operation, fields.clone())?
            .check()
            .map_err(Failure::from)?;
    }
    let mut client = connection::open(matches, &home)?;
    let socket = local::socket_path(&home)?;
    let mut on_behalf = None;
    let meta = OPERATIONS.iter().find(|item| item.name == operation);
    if !generic_call && matches.get_one::<String>("agent").is_some() {
        match meta.map(|item| item.audience) {
            Some(Audience::Host) => {
                return Err(Failure::usage(
                    "host commands are the host's own and name no agent; drop --agent",
                ));
            }
            Some(Audience::Owner) if !args::has_field(&operation, "agent") => {
                return Err(Failure::usage(
                    "this owner command names no agent; drop --agent",
                ));
            }
            _ => {}
        }
    }
    let credential_path = if named_enrollment {
        let name = selected.get_one::<String>("name").expect("required name");
        if !locust_proto::api::is_agent_name(name) {
            return Err(Failure::usage(
                "principal name must contain 1 to 32 a-z, 0-9 or - characters",
            ));
        }
        let path = if author_enrollment {
            home.join("authors").join(format!("{name}.credential"))
        } else {
            local::agent_credential_path(&home, name)?
        };
        secret::create_private_dir(path.parent().expect("agent parent"))
            .map_err(|error| Failure::invalid(format!("credential directory: {error}")))?;
        let credential = Credential(secret::read_or_create(&path).map_err(|error| {
            Failure::invalid(format!("agent credential {}: {error}", path.display()))
        })?);
        connection::read_secret(&path)?;
        fields = json!({"name": name, "credential": credential.digest()})
            .as_object()
            .unwrap()
            .clone();
        Some(path)
    } else {
        None
    };
    if !generic_call && let Some(Value::String(goal)) = fields.get("goal") {
        let goal = resolve_goal(&mut client, &socket, goal, None)?;
        fields.insert("goal".to_owned(), json!(goal));
    }
    if !generic_call {
        let meta = meta.ok_or_else(|| Failure::usage(format!("unknown operation {operation}")))?;
        let goal = fields
            .get("goal")
            .and_then(|value| serde_json::from_value::<GoalId>(value.clone()).ok());
        if matches.get_flag("owner") {
            match meta.audience {
                Audience::Host => {}
                Audience::Owner if args::has_field(&operation, "agent") => {
                    fields.insert(
                        "agent".into(),
                        json!(acting_agent(&mut client, &socket, matches, goal)?),
                    );
                }
                Audience::Agent | Audience::Author
                    if matches.get_one::<String>("agent").is_some()
                        || (!meta.read_only && operation != "session.drop") =>
                {
                    on_behalf = Some(acting_agent(&mut client, &socket, matches, goal)?);
                }
                _ => {}
            }
        } else if args::has_field(&operation, "agent") {
            let agent = match client.caller() {
                locust_proto::api::Caller::Agent(agent)
                | locust_proto::api::Caller::Author(agent) => agent,
                locust_proto::api::Caller::Owner => unreachable!("owner handled above"),
            };
            fields.insert("agent".into(), json!(agent));
        }
        if let Some(goal) = goal {
            for field in ["member", "recipient"] {
                if let Some(Value::String(value)) = fields.get(field) {
                    let member = selectors::resolve_member(&mut client, &socket, goal, value)?;
                    fields.insert(field.to_owned(), json!(member));
                }
            }
        }
        if !named_enrollment {
            validate_fields(&operation, &fields)?;
        }
        selectors::resolve_fields(&mut client, &socket, &mut fields, on_behalf)?;
    }
    let request = request(&operation, fields)?;
    let response_goal = request.goal();
    request.check().map_err(Failure::from)?;
    if matches!(
        request,
        Request::AttemptStart { .. }
            | Request::AttemptTakeover { .. }
            | Request::ContributionPublish {
                attempt: Some(_),
                ..
            }
            | Request::AttemptReport { .. }
    ) && connection::session_path(matches)?.is_none()
    {
        return Err(Failure::invalid(
            "this operation requires an explicit --session file or LOCUST_SESSION",
        ));
    }
    let response = client
        .call_with(request, idempotency, on_behalf)
        .map_err(|error| connection::client_error(error, &socket))?;
    let exit_status = match response {
        Response::Waited(WaitOutcome::NoEvent) => 20,
        Response::Waited(WaitOutcome::Disconnected) => 21,
        _ => 0,
    };
    let response_principal = on_behalf.or(match client.caller() {
        locust_proto::api::Caller::Agent(key) | locust_proto::api::Caller::Author(key) => Some(key),
        locust_proto::api::Caller::Owner => None,
    });
    let known = if matches.get_flag("json") {
        None
    } else if let Response::Status(status) = &response {
        Some(status.clone())
    } else {
        status(&mut client, &socket, on_behalf).ok()
    };
    let (members, tasks) = match (&known, &response) {
        (None, _) => (Vec::new(), Vec::new()),
        (Some(_), Response::Pending(_) | Response::Waited(WaitOutcome::Work(_))) => {
            match response_goal {
                Some(goal) => (
                    members(&mut client, goal, on_behalf),
                    board(&mut client, goal, on_behalf),
                ),
                None => (Vec::new(), Vec::new()),
            }
        }
        (Some(_), Response::GoalStatus(view)) => {
            (Vec::new(), board(&mut client, view.goal, on_behalf))
        }
        _ => (Vec::new(), Vec::new()),
    };
    let goals = known
        .as_ref()
        .map(|known| {
            known
                .goals
                .iter()
                .map(|summary| summary.goal)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let names = known.map(|known| known.agents).unwrap_or_default();
    let reader = presentation::Reader {
        goals: &goals,
        members: &members,
        tasks: &tasks,
        now_ms: now_ms(),
        ..presentation::Reader::new(voice(matches), response_principal, &names)
    };
    let mut result = match receipts {
        Some(cache) => cache.present(&response)?,
        None => {
            serde_json::to_value(&response).map_err(|error| Failure::internal(error.to_string()))?
        }
    };
    let human = if matches.get_flag("json") {
        String::new()
    } else if let Response::GoalStatus(view) = &response {
        let formation = roles::current_formation(&mut client, &socket, view)
            .ok()
            .flatten();
        presentation::goal_status(view, formation.as_ref(), &reader)
    } else if matches!(response, Response::Context(_)) && !generic_call {
        serde_json::to_string_pretty(&result).expect("response encodes")
    } else {
        presentation::render(&response, response_goal, &reader)
            .unwrap_or_else(|| human(&response, credential_path.as_deref()))
    };
    if let Some(path) = credential_path {
        let tag = if author_enrollment {
            "author_enrolled"
        } else {
            "agent_enrolled"
        };
        result[tag]["credential_path"] = json!(path);
    }
    Ok(Output {
        result,
        human,
        status: exit_status,
        ok: true,
    })
}
fn request(operation: &str, fields: Map<String, Value>) -> Result<Request, Failure> {
    if fields.is_empty()
        && let Ok(request) = serde_json::from_value(json!(operation))
    {
        return Ok(request);
    }
    serde_json::from_value(json!({operation: fields}))
        .map_err(|error| Failure::usage(format!("{operation}: {error}")))
}
fn validate_fields(operation: &str, fields: &Map<String, Value>) -> Result<(), Failure> {
    let mut fields = fields.clone();
    if let Some(Value::String(goal)) = fields.get("goal") {
        validate_goal(goal)?;
        fields.insert("goal".to_owned(), json!(GoalId([0; 32])));
    }
    for name in ["member", "recipient"] {
        if let Some(Value::String(value)) = fields.get(name) {
            if value.parse::<PublicKey>().is_err()
                && !selectors::prefix(value)
                && !locust_proto::api::is_agent_name(value)
            {
                return Err(Failure::usage(format!(
                    "--{} requires a local name, full member key or at least 8 hex characters",
                    if name == "recipient" { "member" } else { name }
                )));
            }
            fields.insert(name.to_owned(), json!(PublicKey([0; 32])));
        }
    }
    selectors::validate_fields(&mut fields)?;
    request(operation, fields)?.check().map_err(Failure::from)
}
fn status(
    client: &mut LocalClient,
    socket: &Path,
    on_behalf: Option<PublicKey>,
) -> Result<DaemonStatus, Failure> {
    match client
        .call_with(Request::Status, None, on_behalf)
        .map_err(|error| connection::client_error(error, socket))?
    {
        Response::Status(status) => Ok(status),
        _ => unreachable!("Client checks response kinds"),
    }
}
/// Whose words a view is in: the person's with `--owner`, else the agent's.
pub(super) fn voice(matches: &ArgMatches) -> Voice {
    if matches.get_flag("owner") {
        Voice::Person
    } else {
        Voice::Agent
    }
}
pub(super) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}
/// The goal's members, for their names in it; none when the read fails,
/// since a view never fails for want of a name.
pub(super) fn members(
    client: &mut LocalClient,
    goal: GoalId,
    on_behalf: Option<PublicKey>,
) -> Vec<locust_proto::api::MemberView> {
    match client.call_with(Request::GoalStatus { goal }, None, on_behalf) {
        Ok(Response::GoalStatus(view)) => view.members,
        _ => Vec::new(),
    }
}
/// The goal's tasks, so that a task's prefix is unique among them.
pub(super) fn board(
    client: &mut LocalClient,
    goal: GoalId,
    on_behalf: Option<PublicKey>,
) -> Vec<locust_proto::api::TaskView> {
    match client.call_with(Request::Board { goal }, None, on_behalf) {
        Ok(Response::Board(tasks)) => tasks,
        _ => Vec::new(),
    }
}
pub(super) fn acting_agent(
    client: &mut LocalClient,
    socket: &Path,
    matches: &ArgMatches,
    goal: Option<GoalId>,
) -> Result<PublicKey, Failure> {
    if !matches.get_flag("owner") {
        return Err(Failure::usage("--agent requires --owner"));
    }
    if let Some(key) = matches
        .get_one::<String>("agent")
        .and_then(|named| named.parse::<PublicKey>().ok())
    {
        // An exact key needs no lookup. The daemon returns the reconnect
        // instruction if this principal is disconnected.
        return Ok(key);
    }
    let managing_connection = matches.subcommand().is_some_and(|(command, args)| {
        command == "agent" && matches!(args.subcommand_name(), Some("revoke" | "reconnect"))
    });
    let known = status(client, socket, None)?;
    if let Some(named) = matches.get_one::<String>("agent") {
        let key = named.parse::<PublicKey>().ok();
        return known
            .agents
            .iter()
            .find(|agent| agent.name == *named || key == Some(agent.agent))
            .map(|agent| {
                if agent.revoked && !managing_connection {
                    Err(Failure::usage(disconnected_agent(agent)))
                } else {
                    Ok(agent.agent)
                }
            })
            .transpose()?
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::NotFound,
                    format!("no enrolled agent named {}", presentation::safe(named)),
                )
            });
    }
    let eligible: Vec<_> = known
        .agents
        .iter()
        .filter(|agent| !agent.revoked && !agent.author_only)
        .filter(|agent| {
            goal.is_none_or(|goal| {
                known.goals.iter().any(|entry| {
                    entry.goal == goal
                        && entry.member == agent.agent
                        && entry.membership == Membership::Member
                })
            })
        })
        .collect();
    match eligible.as_slice() {
        [agent] => Ok(agent.agent),
        [] => {
            let disconnected: Vec<_> = known
                .agents
                .iter()
                .filter(|agent| agent.revoked && !agent.author_only)
                .filter(|agent| {
                    goal.is_none_or(|goal| {
                        known.goals.iter().any(|entry| {
                            entry.goal == goal
                                && entry.member == agent.agent
                                && entry.membership == Membership::Member
                        })
                    })
                })
                .map(disconnected_agent)
                .collect();
            if !disconnected.is_empty() {
                return Err(Failure::usage(disconnected.join("\n")));
            }
            if let Some(goal) = goal {
                let title = known
                    .goals
                    .iter()
                    .find(|entry| entry.goal == goal)
                    .and_then(|entry| entry.title.as_deref())
                    .map(presentation::safe)
                    .unwrap_or_else(|| goal.to_string());
                Err(Failure::usage(format!(
                    "none of your agents is in {title}; add one with locust --owner goal add --goal {} --agent NAME",
                    locust_proto::api::short(
                        &goal.to_string(),
                        &known
                            .goals
                            .iter()
                            .map(|entry| entry.goal.to_string())
                            .collect::<Vec<_>>()
                    )
                )))
            } else {
                Err(Failure::usage(
                    "connect an agent first: locust --owner up --help",
                ))
            }
        }
        many => Err(Failure::usage(format!(
            "name the agent: --agent {} or --agent {}",
            many[0].name, many[1].name
        ))),
    }
}
pub(super) fn disconnected_agent(agent: &locust_proto::api::AgentView) -> String {
    format!(
        "{} is disconnected. Connect it again: locust --owner agent reconnect --agent {}",
        presentation::safe(&agent.name),
        locust_proto::api::shell_word(&agent.name)
    )
}
fn create_session(path: &str) -> Result<Output, Failure> {
    let path = local::session_path(Some(std::ffi::OsStr::new(path)))
        .map_err(|error| match error {
            local::LocalError::NotAbsolute(_) => {
                Failure::usage("session create PATH must be an absolute path")
            }
            other => other.into(),
        })?
        .ok_or_else(|| Failure::usage("session create requires an absolute path"))?;
    secret::create_private_dir(
        path.parent()
            .ok_or_else(|| Failure::usage("session path names no file"))?,
    )
    .map_err(|error| Failure::invalid(format!("session directory: {error}")))?;
    let secret =
        SessionSecret(secret::read_or_create(&path).map_err(|error| {
            Failure::invalid(format!("session file {}: {error}", path.display()))
        })?);
    connection::read_secret(&path)?;
    Ok(Output::success(
        json!({"instance": secret.instance(), "session_path": path}),
        format!("session {}\n{}", secret.instance(), path.display()),
    ))
}
fn human(response: &Response, credential_path: Option<&Path>) -> String {
    match response {
        Response::Done => "done".to_owned(),
        Response::Recorded { event } => event.to_string(),
        Response::GoalCreated { goal } => goal.to_string(),
        Response::AgentEnrolled { agent } | Response::AuthorEnrolled { author: agent } => {
            match credential_path {
                Some(path) => format!("{agent}\n{}", path.display()),
                None => agent.to_string(),
            }
        }
        Response::Invited { ticket } => ticket.as_str().to_owned(),
        Response::Joined {
            goal,
            membership,
            level,
            ..
        } => format!(
            "goal {goal}\nmembership {}\nlevel {}",
            stable_name(membership),
            stable_name(level)
        ),
        Response::Claimed(claim) => format!(
            "attempt {}\ngeneration {}\ninstance {}",
            claim.attempt, claim.generation, claim.instance
        ),
        _ => serde_json::to_string_pretty(response).expect("response JSON is serializable"),
    }
}

fn stable_name(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .expect("public enum is serializable")
        .as_str()
        .expect("public enum uses a string tag")
        .to_owned()
}

// Exercise generated client registration against the real parser without
// exposing CLI construction in production code.
#[cfg(test)]
pub(crate) fn command_for_test() -> clap::Command {
    args::command()
}
