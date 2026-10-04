//! Named commands and API fields; generic call uses the API deserializer.
use crate::failure::Failure;
use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::{Map, Value, json};

#[derive(Clone, Copy)]
enum Kind {
    String,
    Number,
    List,
    Flag,
    Bool,
    Text,
}
#[derive(Clone, Copy)]
struct Field {
    key: &'static str,
    flag: &'static str,
    kind: Kind,
    required: bool,
}
const fn string(key: &'static str, flag: &'static str, required: bool) -> Field {
    Field {
        key,
        flag,
        kind: Kind::String,
        required,
    }
}
const fn number(key: &'static str, flag: &'static str, required: bool) -> Field {
    Field {
        key,
        flag,
        kind: Kind::Number,
        required,
    }
}
const fn list(key: &'static str, flag: &'static str) -> Field {
    Field {
        key,
        flag,
        kind: Kind::List,
        required: false,
    }
}
const fn flag(key: &'static str, name: &'static str) -> Field {
    Field {
        key,
        flag: name,
        kind: Kind::Flag,
        required: false,
    }
}
const fn text(key: &'static str) -> Field {
    Field {
        key,
        flag: key,
        kind: Kind::Text,
        required: true,
    }
}
const GOAL: Field = string("goal", "goal", true);
const ASSIGNMENT: Field = string("assignment", "assignment", true);
const GENERATION: Field = number("generation", "generation", true);
fn fields(operation: &str) -> Vec<Field> {
    match operation {
        "agent.grant" => vec![
            string("agent", "agent", true),
            Field {
                key: "manage_goals",
                flag: "manage-goals",
                kind: Kind::Bool,
                required: true,
            },
        ],
        "goal.create" => vec![string("title", "title", true)],
        "goal.join" => vec![string("ticket", "ticket", true)],
        "goal.invite" => vec![GOAL, number("expires_ms", "expires-ms", false)],
        "goal.status" | "goal.leave" | "board" | "pending" => vec![GOAL],
        "task.show" => vec![GOAL, string("task", "task", true)],
        "event.show" => vec![GOAL, string("event", "event", true)],
        "task.propose" => vec![
            GOAL,
            text("text"),
            string("input", "input", false),
            list("depends_on", "depends-on"),
            number("deadline_ms", "deadline-ms", false),
            number("max_attempts", "max-attempts", false),
        ],
        "task.assign" => vec![
            GOAL,
            string("task", "task", true),
            string("assignee", "assignee", true),
        ],
        "task.claim" | "task.takeover" | "task.decline" | "task.cancel" => vec![GOAL, ASSIGNMENT],
        "task.authorize" => vec![GOAL, ASSIGNMENT, flag("takeover", "takeover")],
        "task.progress" => vec![GOAL, ASSIGNMENT, GENERATION, text("text")],
        "task.fail" => vec![GOAL, ASSIGNMENT, GENERATION, text("reason")],
        "task.submit" => vec![
            GOAL,
            ASSIGNMENT,
            GENERATION,
            text("summary"),
            string("base", "base", false),
            string("patch", "patch", false),
            list("artifacts", "artifacts"),
        ],
        "result.accept" => vec![
            GOAL,
            string("result", "result", true),
            string("head", "head", false),
        ],
        "result.reject" => vec![GOAL, string("result", "result", true), text("reason")],
        "note.add" => vec![
            GOAL,
            text("text"),
            string("about", "about", false),
            string("supersedes", "supersedes", false),
        ],
        "notes" => vec![GOAL, string("about", "about", false)],
        "wait" => vec![
            GOAL,
            number("seen", "seen", true),
            number("timeout_ms", "timeout-ms", true),
        ],
        "events" => vec![
            GOAL,
            number("after", "after", false),
            number("limit", "limit", true),
        ],
        "doc.read" => vec![GOAL, string("doc", "doc", true)],
        "doc.revise" => vec![
            GOAL,
            string("doc", "doc", true),
            string("base", "base", false),
            text("text"),
        ],
        "doc.accept" => vec![GOAL, string("revision", "revision", true)],
        _ => vec![],
    }
}
fn version_line() -> &'static str {
    static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    VERSION.get_or_init(crate::version::line)
}
fn operation_help(api: &str) -> &'static str {
    if api == "daemon.run" {
        return "Run the participant daemon in the foreground";
    }
    locust_proto::api::OPERATIONS
        .iter()
        .find(|op| op.name == api)
        .expect("named command has an API operation")
        .summary
}
fn group_help(name: &str) -> &'static str {
    match name {
        "daemon" => "Start or stop the participant daemon",
        "agent" => "Enroll principals and manage daemon-wide grants",
        "goal" => "Create, join and inspect collaborative goals",
        "task" => "Propose, assign and carry out tasks",
        "note" => "Add notes to a goal",
        "event" => "Inspect a recorded event",
        "result" => "Accept or reject submitted results",
        "doc" => "Read, revise and accept shared documents",
        _ => unreachable!("named command group"),
    }
}
fn field_help(key: &str) -> &'static str {
    match key {
        "goal" => "Goal identifier or unique hexadecimal prefix of at least 8 characters",
        "ticket" => "Invitation ticket, or - to read it from standard input",
        "expires_ms" => "Expiry as absolute Unix time in milliseconds; must be in the future",
        "deadline_ms" => "Task deadline as absolute Unix time in milliseconds",
        "agent" => "Full enrolled principal key",
        "manage_goals" => "Set goal-management authority explicitly to true or false",
        "title" => "Goal title",
        "assignment" => "Full assignment event identifier",
        "generation" => "Current claim generation",
        "task" => "Full task proposal event identifier",
        "event" => "Full event identifier",
        "assignee" => "Full principal key of the task assignee",
        "depends_on" => "Task dependencies; repeat or separate identifiers with commas",
        "max_attempts" => "Maximum task attempts",
        "input" => "Input content object hash",
        "takeover" => "Also authorize taking over an existing claim",
        "base" => "Base identifier or content hash for this operation",
        "patch" => "Patch content object hash",
        "artifacts" => "Artifact content hashes; repeat or separate with commas",
        "result" => "Full submission event identifier",
        "head" => "Accepted result content hash",
        "about" => "Event identifier the note concerns",
        "supersedes" => "Earlier note event identifier this note replaces",
        "seen" => "Last observed goal change counter",
        "timeout_ms" => "Maximum wait duration in milliseconds",
        "after" => "Return events after this local position",
        "limit" => "Maximum number of events to return",
        "doc" => "Document identifier",
        "revision" => "Full revision event identifier",
        "text" | "reason" | "summary" => "Text, or - to read standard input",
        _ => unreachable!("named command field"),
    }
}
fn operation(name: &'static str, api: &'static str) -> Command {
    let mut command = Command::new(name).about(operation_help(api));
    for field in fields(api) {
        let mut arg = Arg::new(field.key)
            .required(field.required)
            .help(field_help(field.key));
        arg = match field.kind {
            Kind::Flag => arg.long(field.flag).action(ArgAction::SetTrue),
            Kind::Bool => arg.long(field.flag).value_parser(["true", "false"]),
            Kind::List => arg
                .long(field.flag)
                .action(ArgAction::Append)
                .value_delimiter(','),
            Kind::Text => arg
                .allow_hyphen_values(true)
                .help("Text, or - to read standard input"),
            _ => arg.long(field.flag),
        };
        command = command.arg(arg);
    }
    command
}
fn group(name: &'static str, children: &[(&'static str, &'static str)]) -> Command {
    let mut command = Command::new(name)
        .about(group_help(name))
        .subcommand_required(true);
    for &(name, api) in children {
        command = command.subcommand(operation(name, api));
    }
    command
}
pub(super) fn command() -> Command {
    Command::new("locust")
        .about("Local participant daemon and client")
        .version(version_line())
        .propagate_version(true)
        .subcommand_required(true)
        .arg(
            Arg::new("home")
                .long("home")
                .global(true)
                .help("Absolute state directory (or LOCUST_HOME; defaults to ~/.locust)"),
        )
        .arg(
            Arg::new("credential")
                .long("credential")
                .help("Absolute credential file (or LOCUST_CREDENTIAL)")
                .global(true)
                .conflicts_with("owner"),
        )
        .arg(
            Arg::new("owner")
                .long("owner")
                .help("Use this daemon home's owner credential")
                .global(true)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("as")
                .long("as")
                .global(true)
                .requires("owner")
                .help("Act as an enrolled name or principal key (requires --owner)"),
        )
        .arg(
            Arg::new("session")
                .long("session")
                .global(true)
                .help("Absolute execution session secret file (or LOCUST_SESSION)"),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .help("Print one JSON response envelope")
                .global(true)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("idempotency-key")
                .long("idempotency-key")
                .help("Retry key: 16 bytes in hexadecimal, scoped to the caller")
                .global(true),
        )
        .subcommand(group(
            "daemon",
            &[("run", "daemon.run"), ("stop", "daemon.stop")],
        ))
        .subcommand(operation("status", "status"))
        .subcommand(
            Command::new("doctor").about("Check local paths, credentials and daemon readiness"),
        )
        .subcommand(
            Command::new("agent")
                .about(group_help("agent"))
                .subcommand_required(true)
                .subcommand(
                    Command::new("enroll")
                        .about(
                            "Enroll a named principal; use agent grant to change existing grants",
                        )
                        .arg(
                            Arg::new("name")
                                .required(true)
                                .help("Local principal name: a-z, 0-9 and hyphens"),
                        )
                        .arg(
                            Arg::new("manage-goals")
                                .long("manage-goals")
                                .help("Allow creating goals, inviting, joining and leaving")
                                .action(ArgAction::SetTrue),
                        ),
                )
                .subcommand(operation("grant", "agent.grant")),
        )
        .subcommand(group(
            "goal",
            &[
                ("create", "goal.create"),
                ("invite", "goal.invite"),
                ("join", "goal.join"),
                ("status", "goal.status"),
                ("leave", "goal.leave"),
            ],
        ))
        .subcommand(group(
            "task",
            &[
                ("propose", "task.propose"),
                ("assign", "task.assign"),
                ("claim", "task.claim"),
                ("submit", "task.submit"),
                ("show", "task.show"),
                ("authorize", "task.authorize"),
                ("takeover", "task.takeover"),
                ("cancel", "task.cancel"),
                ("decline", "task.decline"),
                ("progress", "task.progress"),
                ("fail", "task.fail"),
            ],
        ))
        .subcommand(group("note", &[("add", "note.add")]))
        .subcommand(operation("notes", "notes"))
        .subcommand(group("event", &[("show", "event.show")]))
        .subcommand(group(
            "result",
            &[("accept", "result.accept"), ("reject", "result.reject")],
        ))
        .subcommand(operation("board", "board"))
        .subcommand(operation("pending", "pending"))
        .subcommand(operation("wait", "wait"))
        .subcommand(operation("events", "events"))
        .subcommand(group(
            "doc",
            &[
                ("read", "doc.read"),
                ("revise", "doc.revise"),
                ("accept", "doc.accept"),
            ],
        ))
        .subcommand(
            Command::new("session")
                .about("Manage local execution session secrets")
                .subcommand_required(true)
                .subcommand(
                    Command::new("create")
                        .about("Create or reuse a private execution session secret")
                        .arg(
                            Arg::new("path")
                                .required(true)
                                .help("Absolute path for the session secret"),
                        ),
                ),
        )
        .subcommand(
            Command::new("call")
                .about("Call any typed API operation with JSON fields")
                .arg(
                    Arg::new("operation")
                        .required(true)
                        .help("API operation name, such as goal.status"),
                )
                .arg(
                    Arg::new("fields")
                        .help("JSON object, or - to read standard input")
                        .default_value("{}")
                        .allow_hyphen_values(true),
                ),
        )
}
pub(super) fn selected(matches: &ArgMatches) -> (String, &ArgMatches) {
    let (name, child) = matches.subcommand().expect("clap requires a command");
    match child.subcommand() {
        Some((leaf, fields)) => (format!("{name}.{leaf}"), fields),
        None => (name.to_owned(), child),
    }
}
pub(super) fn values(operation: &str, matches: &ArgMatches) -> Result<Map<String, Value>, Failure> {
    let mut values = Map::new();
    for field in fields(operation) {
        let value = match field.kind {
            Kind::Flag => Some(json!(matches.get_flag(field.key))),
            Kind::Bool => matches
                .get_one::<String>(field.key)
                .map(|v| json!(v == "true")),
            Kind::List => Some(json!(
                matches
                    .get_many::<String>(field.key)
                    .map(|v| v.collect::<Vec<_>>())
                    .unwrap_or_default()
            )),
            Kind::Number => matches
                .get_one::<String>(field.key)
                .map(|v| {
                    v.parse::<u64>().map(Value::from).map_err(|_| {
                        Failure::usage(format!("--{} requires an unsigned integer", field.flag))
                    })
                })
                .transpose()?,
            _ => matches
                .get_one::<String>(field.key)
                .map(|v| Value::String(v.clone())),
        };
        if let Some(value) = value {
            values.insert(field.key.to_owned(), value);
        }
    }
    if operation == "agent.grant" {
        let manage_goals = values.remove("manage_goals").expect("required grant value");
        values.insert("grants".to_owned(), json!({ "manage_goals": manage_goals }));
    }
    Ok(values)
}
pub(super) fn text_field(operation: &str) -> Option<&'static str> {
    fields(operation)
        .iter()
        .find(|field| matches!(field.kind, Kind::Text))
        .map(|field| field.key)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_t1_named_command_has_the_expected_typed_request() {
        command().debug_assert();
        let goal = "03".repeat(32);
        let event = "04".repeat(32);
        let agent = "05".repeat(32);
        let cases = vec![
            (vec!["status"], "status"),
            (vec!["daemon", "stop"], "daemon.stop"),
            (vec!["goal", "create", "--title", "a goal"], "goal.create"),
            (
                vec!["goal", "invite", "--goal", &goal, "--expires-ms", "123"],
                "goal.invite",
            ),
            // A ticket is validated by Request::check, after deserialization.
            (vec!["goal", "join", "--ticket", "ticket-text"], "goal.join"),
            (vec!["goal", "status", "--goal", &goal], "goal.status"),
            (
                vec!["note", "add", "--goal", &goal, "some text"],
                "note.add",
            ),
            (vec!["notes", "--goal", &goal], "notes"),
            (
                vec!["task", "propose", "--goal", &goal, "some text"],
                "task.propose",
            ),
            (
                vec![
                    "task",
                    "assign",
                    "--goal",
                    &goal,
                    "--task",
                    &event,
                    "--assignee",
                    &agent,
                ],
                "task.assign",
            ),
            (
                vec!["task", "claim", "--goal", &goal, "--assignment", &event],
                "task.claim",
            ),
            (
                vec![
                    "task",
                    "submit",
                    "--goal",
                    &goal,
                    "--assignment",
                    &event,
                    "--generation",
                    "1",
                    "summary",
                ],
                "task.submit",
            ),
            (
                vec!["event", "show", "--goal", &goal, "--event", &event],
                "event.show",
            ),
            (
                vec!["result", "accept", "--goal", &goal, "--result", &event],
                "result.accept",
            ),
            (vec!["board", "--goal", &goal], "board"),
            (vec!["pending", "--goal", &goal], "pending"),
        ];
        for (arguments, expected) in cases {
            let matches = command()
                .try_get_matches_from(std::iter::once("locust").chain(arguments))
                .unwrap();
            let (operation, selected) = selected(&matches);
            assert_eq!(operation, expected);
            let request =
                super::super::request(&operation, values(&operation, selected).unwrap()).unwrap();
            assert_eq!(request.name(), expected);
        }
    }
    #[test]
    fn text_and_global_flags_can_appear_before_or_after_field_flags() {
        let goal = "03".repeat(32);
        for arguments in [
            vec!["locust", "--json", "note", "add", "--goal", &goal, "-"],
            vec![
                "locust",
                "note",
                "add",
                "some text",
                "--goal",
                &goal,
                "--json",
            ],
        ] {
            let matches = command().try_get_matches_from(arguments).unwrap();
            assert!(matches.get_flag("json"));
            let (operation, selected) = selected(&matches);
            assert_eq!(values(&operation, selected).unwrap()["goal"], goal);
        }
    }
}
