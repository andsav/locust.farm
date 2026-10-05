//! Named commands and fields generated from the authoritative request schema.
use crate::context_receipts::{Surface, operation_schema};
use crate::failure::Failure;
use clap::{Arg, ArgAction, ArgMatches, Command};
use locust_proto::api::OPERATIONS;
use serde_json::{Map, Value};
use std::{collections::BTreeMap, sync::OnceLock};
struct Field {
    name: &'static str,
    flag: &'static str,
    schema: Value,
    required: bool,
}
fn fields(operation: &str) -> &'static [Field] {
    static FIELDS: OnceLock<BTreeMap<&'static str, Vec<Field>>> = OnceLock::new();
    FIELDS
        .get_or_init(|| {
            OPERATIONS
                .iter()
                .map(|operation| {
                    let schema = operation_schema(operation.name, Surface::Cli)
                        .expect("registry has typed schema");
                    let required = schema["required"].as_array();
                    let fields = schema["properties"]
                        .as_object()
                        .map(|properties| {
                            properties
                                .iter()
                                .map(|(name, value)| {
                                    let name: &'static str =
                                        Box::leak(name.clone().into_boxed_str());
                                    let flag: &'static str =
                                        Box::leak(name.replace('_', "-").into_boxed_str());
                                    Field {
                                        name,
                                        flag,
                                        schema: resolve(value, &schema),
                                        required: required.is_some_and(|fields| {
                                            fields.iter().any(|field| field == name)
                                        }) && !nullable(value),
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    (operation.name, fields)
                })
                .collect()
        })
        .get(operation)
        .map(Vec::as_slice)
        .unwrap_or_default()
}
fn nullable(value: &Value) -> bool {
    value["type"]
        .as_array()
        .is_some_and(|types| types.iter().any(|kind| kind == "null"))
        || value["anyOf"]
            .as_array()
            .is_some_and(|branches| branches.iter().any(|branch| branch["type"] == "null"))
}
fn resolve(value: &Value, root: &Value) -> Value {
    if let Some(reference) = value["$ref"].as_str() {
        return root
            .pointer(reference.trim_start_matches('#'))
            .cloned()
            .unwrap_or_else(|| value.clone());
    }
    if nullable(value)
        && let Some(branches) = value["anyOf"].as_array()
        && let Some(branch) = branches.iter().find(|branch| branch["type"] != "null")
    {
        return resolve(branch, root);
    }
    if let Some(types) = value["type"].as_array() {
        let mut result = value.clone();
        result["type"] = types
            .iter()
            .find(|kind| *kind != "null")
            .cloned()
            .unwrap_or(Value::Null);
        return result;
    }
    value.clone()
}
fn operation(name: &'static str, api: &'static str) -> Command {
    let mut command = Command::new(name).about(
        OPERATIONS
            .iter()
            .find(|operation| operation.name == api)
            .unwrap()
            .summary,
    );
    for field in fields(api) {
        let positional = text_field(api) == Some(field.name);
        let required = field.required
            && !matches!(
                field.schema["type"].as_str(),
                Some("array" | "object" | "boolean")
            );
        let mut argument =
            Arg::new(field.name)
                .required(required)
                .help(if field.schema["type"] == "string" {
                    "Value, or - for standard input on text fields"
                } else {
                    "JSON value matching formation contract or the API schema"
                });
        if api == "context.acknowledge" && field.name == "receipt" {
            argument = argument
                .help("ctx: reference returned by context read with this credential and session");
        }
        if let Some(values) = field.schema["enum"].as_array() {
            let values: Vec<&'static str> = values
                .iter()
                .filter_map(Value::as_str)
                .map(|value| &*Box::leak(value.to_owned().into_boxed_str()))
                .collect();
            if !values.is_empty() {
                argument = argument.value_parser(clap::builder::PossibleValuesParser::new(values));
            }
        }
        if positional {
            argument = argument.allow_hyphen_values(true);
        } else {
            argument = argument.long(field.flag);
        }
        command = command.arg(argument);
    }
    if api == "wait" {
        // Statuses 20 and 21 accompany an `ok: true` result; see `cli::execute`.
        command = command.after_help(
            "Exit status: 0 when the goal changed since --seen, 20 when nothing changed before the timeout, 21 when nothing changed and no peer of the goal is reachable. 20 and 21 are not errors.",
        );
    }
    if api == "goal.create" {
        command = command.arg(
            Arg::new("formation")
                .long("formation")
                .conflicts_with("formation_json")
                .help("Bundled organization name from formation examples; no JSON required"),
        );
    }
    command
}
fn version_line() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(crate::version::line)
}
pub(super) fn command() -> Command {
    let mut command = Command::new("locust")
        .about("Local participant daemon and client")
        .version(version_line())
        .propagate_version(true)
        .subcommand_required(true)
        .arg(
            Arg::new("home")
                .long("home")
                .global(true)
                .help("Absolute state directory or LOCUST_HOME"),
        )
        .arg(
            Arg::new("credential")
                .long("credential")
                .global(true)
                .conflicts_with("owner")
                .help("Explicit credential file"),
        )
        .arg(
            Arg::new("owner")
                .long("owner")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Use this home's owner credential"),
        )
        .arg(
            Arg::new("as")
                .long("as")
                .global(true)
                .requires("owner")
                .help("Act as an enrolled principal"),
        )
        .arg(
            Arg::new("session")
                .long("session")
                .global(true)
                .help("Explicit execution session file"),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Print one JSON response envelope"),
        )
        .arg(
            Arg::new("idempotency-key")
                .long("idempotency-key")
                .global(true)
                .help("Caller-owned 16-byte retry key in hex"),
        )
        .subcommands(super::permissions::commands())
        .subcommand(super::invitations::command())
        .subcommands(super::workspace::commands())
        .subcommand(super::client::commands())
        .subcommand(super::package::commands())
        .subcommand(super::install::commands())
        .subcommand(super::service::commands())
        .subcommand(super::setup::commands())
        .subcommand(super::onboarding::up_command())
        .subcommand(super::formation::commands())
        .subcommand(super::farm::commands())
        .subcommand(Command::new("contract").about("Export API, event and MCP contracts offline"))
        .subcommand(super::doctor::command())
        .subcommand(
            Command::new("mcp")
                .about("Serve authenticated MCP tools")
                .arg(Arg::new("lifecycle-receipt").long("lifecycle-receipt")),
        )
        .subcommand(
            Command::new("session")
                .subcommand_required(true)
                .subcommand(
                    Command::new("create")
                        .about("Create or reuse a private session secret")
                        .arg(Arg::new("path").required(true)),
                ),
        )
        .subcommand(
            Command::new("call")
                .about("Call a typed API operation")
                .arg(Arg::new("operation").required(true))
                .arg(
                    Arg::new("fields")
                        .default_value("{}")
                        .allow_hyphen_values(true),
                ),
        );
    let mut groups: BTreeMap<&'static str, Vec<Command>> = BTreeMap::new();
    for api in OPERATIONS {
        // Offline authoring owns these names; authenticated inspection is available through call/MCP.
        if api.name.starts_with("invitation.")
            || api.name.starts_with("permission.")
            || api.name.starts_with("farm.")
            || api.name.starts_with("workspace.")
            || api.name == "inbox"
            || matches!(
                api.name,
                "formation.validate" | "formation.explain" | "agent.enroll" | "author.enroll"
            )
        {
            continue;
        }
        if let Some((group, leaf)) = api.name.split_once('.') {
            if let Some((subgroup, leaf)) = leaf.split_once('.') {
                let children = groups.entry(group).or_default();
                if let Some(index) = children
                    .iter()
                    .position(|command| command.get_name() == subgroup)
                {
                    let child = children.remove(index).subcommand(operation(leaf, api.name));
                    children.push(child);
                } else {
                    children.push(
                        Command::new(subgroup)
                            .subcommand_required(true)
                            .subcommand(operation(leaf, api.name)),
                    );
                }
            } else {
                groups
                    .entry(group)
                    .or_default()
                    .push(operation(leaf, api.name));
            }
        } else {
            command = command.subcommand(operation(api.name, api.name));
        }
    }
    groups
        .entry("goal")
        .or_default()
        .push(super::local_members::command());
    groups
        .entry("daemon")
        .or_default()
        .push(Command::new("run").about("Run the participant daemon"));
    groups
        .entry("agent")
        .or_default()
        .push(super::onboarding::add_command());
    groups.entry("agent").or_default().push(
        Command::new("enroll")
            .about("Enroll a principal and store its credential")
            .arg(Arg::new("name").required(true))
            .arg(
                Arg::new("manage-goals")
                    .long("manage-goals")
                    .action(ArgAction::SetTrue),
            ),
    );
    groups.entry("author").or_default().push(
        Command::new("enroll")
            .about("Enroll a private formation author and store its credential")
            .arg(Arg::new("name").required(true)),
    );
    for (name, children) in groups {
        if command
            .get_subcommands()
            .any(|existing| existing.get_name() == name)
        {
            command = command.mut_subcommand(name, |command| {
                command.subcommand_negates_reqs(true).subcommands(children)
            });
        } else {
            command = command.subcommand(
                Command::new(name)
                    .subcommand_required(true)
                    .subcommands(children),
            );
        }
    }
    command
}
pub(super) fn selected(matches: &ArgMatches) -> (String, &ArgMatches) {
    let mut fields = matches;
    let mut names = Vec::new();
    while let Some((name, child)) = fields.subcommand() {
        names.push(name);
        fields = child;
    }
    (names.join("."), fields)
}

/// Describe the actual command tree, including host-local commands which are
/// not daemon operations. The documentation exporter uses this same surface.
pub(super) fn contract() -> Value {
    fn describe(command: &Command) -> Value {
        serde_json::json!({
            "name": command.get_name(),
            "summary": command.get_about().map(ToString::to_string),
            "arguments": command.get_arguments().map(|argument| serde_json::json!({
                "name": argument.get_id().as_str(),
                "long": argument.get_long(),
                "short": argument.get_short(),
                "positional": argument.get_long().is_none() && argument.get_short().is_none(),
                "required": argument.is_required_set(),
                "action": format!("{:?}", argument.get_action()),
                "help": argument.get_help().map(ToString::to_string),
            })).collect::<Vec<_>>(),
            "commands": command.get_subcommands().map(describe).collect::<Vec<_>>(),
        })
    }
    describe(&command())
}
pub(super) fn values(operation: &str, matches: &ArgMatches) -> Result<Map<String, Value>, Failure> {
    if matches!(operation, "agent.enroll" | "author.enroll") {
        return Ok(Map::new());
    }
    let mut values = Map::new();
    for field in fields(operation) {
        let value = match matches.get_one::<String>(field.name) {
            Some(text) if field.schema["type"] == "string" => Value::String(text.clone()),
            Some(text) => serde_json::from_str(text)
                .map_err(|error| Failure::usage(format!("--{}: {error}", field.flag)))?,
            None if field.schema.get("default").is_some() => field.schema["default"].clone(),
            None => match field.schema["type"].as_str() {
                Some("array") if field.required => serde_json::json!([]),
                Some("object") if field.required => serde_json::json!({}),
                Some("boolean") => Value::Bool(false),
                _ => Value::Null,
            },
        };
        values.insert(field.name.into(), value);
    }
    if operation == "goal.create"
        && let Some(name) = matches.get_one::<String>("formation")
    {
        let preset = locust_proto::organization::presets()
            .into_iter()
            .find(|preset| preset.name == *name)
            .ok_or_else(|| {
                Failure::usage(format!("unknown formation {name}; use formation examples"))
            })?;
        values.insert(
            "formation_json".into(),
            Value::String(
                serde_json::to_string(&preset.formation).expect("bundled formation encodes"),
            ),
        );
    }
    Ok(values)
}
pub(super) fn text_field(operation: &str) -> Option<&'static str> {
    match operation {
        "task.open" | "attempt.report" | "review.record" | "check.attest" | "doc.revise" => {
            Some("text")
        }
        "contribution.publish" => Some("summary"),
        "formation.draft.create" | "formation.draft.update" => Some("source"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn long_flags(command: &Command, flags: &mut std::collections::BTreeSet<String>) {
        flags.extend(
            command
                .get_arguments()
                .filter_map(|argument| argument.get_long().map(String::from)),
        );
    }
    /// The packaged skill is read as instructions: a `locust ...` command it
    /// names must parse, and a flag named on its own must belong to a command
    /// group named in the same paragraph.
    #[test]
    fn the_skill_names_only_commands_and_flags_this_parser_accepts() {
        use clap::error::ErrorKind;
        let skill = include_str!("../../../../skills/locust/SKILL.md");
        let root = command();
        let mut checked = 0;
        for paragraph in skill.split("\n\n") {
            // Every second piece between backticks is a code span; one may
            // wrap across lines.
            let spans: Vec<Vec<&str>> = paragraph
                .split('`')
                .skip(1)
                .step_by(2)
                .map(|span| span.split_whitespace().collect())
                .collect();
            let mut flags = std::collections::BTreeSet::new();
            for words in &spans {
                if words.len() < 2 || words[0] != "locust" {
                    continue;
                }
                let named = words.join(" ");
                if let Err(error) = command().try_get_matches_from(words) {
                    // Naming a command without its required parts is fine.
                    assert!(
                        matches!(
                            error.kind(),
                            ErrorKind::MissingRequiredArgument | ErrorKind::MissingSubcommand
                        ),
                        "the skill names `{named}`: {error}"
                    );
                }
                long_flags(&root, &mut flags);
                let mut group = vec![root.find_subcommand(words[1]).expect("parsed above")];
                while let Some(command) = group.pop() {
                    long_flags(command, &mut flags);
                    group.extend(command.get_subcommands());
                }
                checked += 1;
            }
            for words in &spans {
                let Some(flag) = words.first().and_then(|word| word.strip_prefix("--")) else {
                    continue;
                };
                assert!(
                    flags.contains(flag),
                    "the skill names `{}` beside no command that takes it",
                    words.join(" ")
                );
                checked += 1;
            }
        }
        assert!(checked > 0, "no command or flag was found in the skill");
    }
    #[test]
    fn optional_sources_use_the_schema_default_in_named_commands() {
        let matches = command()
            .try_get_matches_from([
                "locust",
                "contribution",
                "publish",
                "--goal",
                &"01".repeat(32),
                "A finding",
            ])
            .unwrap();
        let (name, fields) = selected(&matches);
        assert_eq!(
            values(&name, fields).unwrap()["sources"],
            serde_json::json!([])
        );
        assert!(
            serde_json::from_value::<locust_proto::api::Request>(
                serde_json::json!({name.as_str(): values(&name, fields).unwrap()})
            )
            .is_ok()
        );
    }

    #[test]
    fn new_goal_uses_empty_bindings_and_optional_formation() {
        let matches = command()
            .try_get_matches_from(["locust", "goal", "create", "--title", "open"])
            .unwrap();
        let (name, fields) = selected(&matches);
        let request = serde_json::from_value::<locust_proto::api::Request>(
            serde_json::json!({name.as_str():values(&name,fields).unwrap()}),
        )
        .unwrap();
        assert!(
            matches!(request,locust_proto::api::Request::GoalCreate{formation_json:None,roles,inputs,..} if roles.is_empty() && inputs.is_empty())
        );
    }
    #[test]
    fn composed_scope_is_json_and_task_identity_remains_explicit() {
        let goal = "01".repeat(32);
        let task = format!("task:{}", "02".repeat(32));
        let scope = serde_json::json!({"task":task}).to_string();
        let matches = command()
            .try_get_matches_from([
                "locust", "scope", "close", "--goal", &goal, "--scope", &scope,
            ])
            .unwrap();
        let (name, fields) = selected(&matches);
        let request = serde_json::from_value::<locust_proto::api::Request>(
            serde_json::json!({name.as_str():values(&name,fields).unwrap()}),
        )
        .unwrap();
        assert!(matches!(
            request,
            locust_proto::api::Request::ScopeClose {
                scope: locust_proto::event::Scope::Task(_),
                expected: None,
                ..
            }
        ));
    }
    #[test]
    fn old_commands_are_removed_and_catalog_has_nested_commands() {
        assert!(
            command()
                .try_get_matches_from(["locust", "task", "claim"])
                .is_err()
        );
        let matches = command()
            .try_get_matches_from([
                "locust",
                "formation",
                "draft",
                "update",
                "--id",
                "draft",
                "--expected-revision",
                "1",
                "{",
            ])
            .unwrap();
        assert_eq!(selected(&matches).0, "formation.draft.update");
    }
}
