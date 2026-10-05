//! Command-line client: explicit authority, one JSON envelope, typed API.
// A print macro panics when its reader has gone away; write through `print`.
#![deny(clippy::print_stdout, clippy::print_stderr)]
mod args;
mod client;
mod connection;
mod doctor;
mod farm;
mod formation;
mod install;
mod invitations;
mod local_members;
mod onboarding;
mod package;
mod permissions;
mod presentation;
mod print;
mod selectors;
mod service;
mod setup;
mod workspace;

use crate::{daemon, failure::Failure, secret};
use clap::{ArgMatches, error::ErrorKind};
use locust_proto::api::{
    Credential, DaemonStatus, ErrorCode, Grants, Request, Response, SessionSecret, WaitOutcome,
};
use locust_proto::client::Client;
use locust_proto::id::{GoalId, IdempotencyKey, PublicKey};
use locust_proto::local;
use selectors::{resolve_goal, validate_goal};
use serde_json::{Map, Value, json};
use std::io::{self, Read};
use std::path::Path;

type LocalClient = Client<std::os::unix::net::UnixStream>;
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
        Err(error) => return print_failure(Failure::usage(error.to_string()), json_mode),
    };
    if let Some(("mcp", selected)) = matches.subcommand() {
        return match run_mcp(&matches, selected) {
            Ok(()) => 0,
            Err(error) => print_failure(error, false),
        };
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
        Err(error) => print_failure(error, matches.get_flag("json")),
    }
}
// Classify parse failures without confusing an option value or command body
// named "mcp" with the transport subcommand. Successful parsing uses Clap's
// actual selected subcommand for dispatch.
fn mcp_invocation(arguments: &[std::ffi::OsString]) -> bool {
    let mut arguments = arguments.iter().skip(1);
    while let Some(argument) = arguments.next() {
        let Some(argument) = argument.to_str() else {
            return false;
        };
        if matches!(
            argument,
            "--home" | "--credential" | "--as" | "--session" | "--idempotency-key"
        ) {
            arguments.next();
        } else if argument == "--" {
            return arguments.next().is_some_and(|argument| argument == "mcp");
        } else if !argument.starts_with('-') {
            return argument == "mcp";
        }
    }
    false
}
fn run_mcp(matches: &ArgMatches, selected: &ArgMatches) -> Result<(), Failure> {
    if matches.get_flag("owner")
        || matches.get_one::<String>("as").is_some()
        || matches.get_flag("json")
        || matches.get_one::<String>("idempotency-key").is_some()
    {
        return Err(Failure::usage(
            "mcp does not accept --owner, --as, --json or --idempotency-key",
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
        print::stderr(format_args!("locust: {error}\n"))
    };
    print::status(written, error.exit_status())
}
fn stdin_text() -> Result<String, Failure> {
    let mut text = String::new();
    io::stdin()
        .read_to_string(&mut text)
        .map_err(|error| Failure::invalid(format!("standard input: {error}")))?;
    Ok(text)
}
fn execute(matches: &ArgMatches) -> Result<Output, Failure> {
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
    if operation == "goal.add-local" {
        return local_members::run(matches, selected);
    }
    if operation.starts_with("invitation.") {
        return invitations::run(matches, &operation, selected);
    }
    if operation.starts_with("permission.") || matches!(operation.as_str(), "inbox" | "watch") {
        return permissions::run(matches, &operation, selected);
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
    if !generic_call
        && operation == "goal.join"
        && fields.get("ticket").and_then(Value::as_str) == Some("-")
    {
        fields.insert(
            "ticket".to_owned(),
            Value::String(stdin_text()?.trim_end_matches(['\r', '\n']).to_owned()),
        );
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
    if !named_enrollment {
        if generic_call {
            request(&operation, fields.clone())?
                .check()
                .map_err(Failure::from)?;
        } else {
            validate_fields(&operation, &fields)?;
        }
    }
    let mut client = connection::open(matches, &home)?;
    let socket = local::socket_path(&home)?;
    let on_behalf = matches
        .get_one::<String>("as")
        .map(|name| resolve_principal(&mut client, &socket, name))
        .transpose()?;
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
        fields = if author_enrollment { json!({"name":name,"credential":credential.digest()}) } else { json!({"name": name, "grants": Grants { manage_goals: selected.get_flag("manage-goals") }, "credential": credential.digest()}) }.as_object().unwrap().clone();
        Some(path)
    } else {
        None
    };
    if !generic_call && let Some(Value::String(goal)) = fields.get("goal") {
        let goal = resolve_goal(&mut client, &socket, goal, on_behalf)?;
        fields.insert("goal".to_owned(), json!(goal));
    }
    if !generic_call {
        for field in ["agent", "member", "recipient", "principal"] {
            if let Some(Value::String(value)) = fields.get(field) {
                let principal = resolve_principal(&mut client, &socket, value)?;
                fields.insert(field.to_owned(), json!(principal));
            }
        }
    }
    if !generic_call {
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
        locust_proto::api::Caller::Agent(key)
        | locust_proto::api::Caller::Viewer(key)
        | locust_proto::api::Caller::Author(key) => Some(key),
        locust_proto::api::Caller::Owner => None,
    });
    let names = if matches.get_flag("json") {
        Vec::new()
    } else if let Response::Status(status) = &response {
        status.agents.clone()
    } else {
        status(&mut client, &socket, on_behalf)
            .map(|status| status.agents)
            .unwrap_or_default()
    };
    let mut result = match receipts {
        Some(cache) => cache.present(&response)?,
        None => {
            serde_json::to_value(&response).map_err(|error| Failure::internal(error.to_string()))?
        }
    };
    let human = if matches.get_flag("json") {
        String::new()
    } else if matches!(response, Response::Context(_)) && !generic_call {
        serde_json::to_string_pretty(&result).expect("response encodes")
    } else {
        presentation::render(&response, &names, response_goal, response_principal)
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
    for name in ["agent", "member", "recipient", "principal"] {
        if let Some(Value::String(value)) = fields.get(name) {
            if value.parse::<PublicKey>().is_err() && !locust_proto::api::is_agent_name(value) {
                return Err(Failure::usage(format!(
                    "--{name} requires an enrolled name or a full principal key"
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
fn resolve_principal(
    client: &mut LocalClient,
    socket: &Path,
    principal: &str,
) -> Result<PublicKey, Failure> {
    if let Ok(key) = principal.parse() {
        return Ok(key);
    }
    if !locust_proto::api::is_agent_name(principal) {
        return Err(Failure::usage(
            "principal requires an enrolled name or a full principal key",
        ));
    }
    status(client, socket, None)?
        .agents
        .into_iter()
        .find(|agent| agent.name == principal)
        .map(|agent| agent.agent)
        .ok_or_else(|| {
            Failure::new(
                ErrorCode::NotFound,
                format!("principal {principal} is not enrolled"),
            )
        })
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
            administrator,
            membership,
        } => format!(
            "goal {goal}\nadministrator {administrator}\nmembership {}",
            stable_name(membership)
        ),
        Response::Claimed(claim) => format!(
            "attempt {}\ngeneration {}\ninstance {}",
            claim.attempt, claim.generation, claim.instance
        ),
        Response::Status(status) => {
            let mut lines = vec![format!("daemon {}", status.daemon_version)];
            if let Some(endpoint) = status.endpoint {
                lines.push(format!("endpoint {endpoint}"));
            }
            lines.extend(status.agents.iter().map(|agent| {
                format!(
                    "agent {} {}{}",
                    agent.name,
                    agent.agent,
                    if agent.revoked { " revoked" } else { "" }
                )
            }));
            lines.extend(status.goals.iter().map(|goal| {
                format!(
                    "goal {} member {} {} {}{}",
                    goal.goal,
                    goal.member,
                    stable_name(&goal.membership),
                    goal.title.as_deref().unwrap_or(""),
                    goal.halted
                        .as_ref()
                        .map(|halt| format!(" halted {}", stable_name(halt)))
                        .unwrap_or_default()
                )
            }));
            lines.join("\n")
        }
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
