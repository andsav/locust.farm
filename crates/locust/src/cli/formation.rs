//! Offline organization authoring; deliberately independent of daemon state.
use super::Output;
use crate::failure::Failure;
use clap::{Arg, ArgMatches, Command};
use locust_core::organization::inspect;
use locust_proto::api::ErrorCode;
use locust_proto::organization::{OPERATIONS, contract, presets, schema};
use serde_json::{Value, json};

pub(super) fn commands() -> Command {
    let mut command = Command::new("formation")
        .about("Author local formations and inspect organization definitions offline")
        .subcommand_required(true)
        .arg_required_else_help(true);
    for operation in OPERATIONS {
        let mut child = Command::new(operation.name).about(operation.summary);
        child = match operation.input {
            "none" => child,
            "example_name" => child.arg(
                Arg::new("name")
                    .required(true)
                    .help("Name from formation examples"),
            ),
            "json_path_or_stdin" => child.arg(
                Arg::new("source")
                    .required(true)
                    .help("JSON document path, or - for standard input")
                    .allow_hyphen_values(true),
            ),
            "two_json_paths" => child
                .arg(
                    Arg::new("before")
                        .required(true)
                        .help("Before JSON document path"),
                )
                .arg(
                    Arg::new("after")
                        .required(true)
                        .help("After JSON document path"),
                ),
            _ => unreachable!("authoring catalog has an unsupported input shape"),
        };
        command = command.subcommand(child);
    }
    command
}
fn rendered(result: Value) -> Result<Output, Failure> {
    let human = serde_json::to_string_pretty(&result)
        .map_err(|error| Failure::internal(format!("cannot render formation: {error}")))?;
    Ok(Output::success(result, human))
}
pub(super) fn run(operation: &str, args: &ArgMatches) -> Result<Output, Failure> {
    match operation {
        "formation.contract" => rendered(contract()),
        "formation.schema" => rendered(schema()),
        "formation.examples" => rendered(json!(
            presets()
                .into_iter()
                .map(|preset| json!({"name":preset.name,"description":preset.description}))
                .collect::<Vec<_>>()
        )),
        "formation.example" => {
            let name = args.get_one::<String>("name").expect("required name");
            let preset = presets()
                .into_iter()
                .find(|preset| &preset.name == name)
                .ok_or_else(|| {
                    Failure::new(
                        ErrorCode::NotFound,
                        format!(
                            "unknown formation example {name}; use formation examples to list names"
                        ),
                    )
                })?;
            rendered(json!(preset.formation))
        }
        "formation.diff" => {
            let read = |name| {
                let path = args
                    .get_one::<String>(name)
                    .expect("required document path");
                std::fs::read_to_string(path)
                    .map_err(|error| Failure::invalid(format!("{path}: {error}")))
            };
            let diff = locust_core::organization::diff::compare(&read("before")?, &read("after")?);
            let valid = diff.valid;
            let status = if valid {
                0
            } else if diff
                .before
                .diagnostics
                .iter()
                .chain(&diff.after.diagnostics)
                .any(|diagnostic| diagnostic.code == "unsupported_version")
            {
                10
            } else {
                6
            };
            let mut output = rendered(json!(diff))?;
            output.ok = valid;
            output.status = status;
            Ok(output)
        }
        "formation.validate" | "formation.explain" | "formation.normalize" => {
            let path = args.get_one::<String>("source").expect("required source");
            let source = if path == "-" {
                super::stdin_text()?
            } else {
                std::fs::read_to_string(path)
                    .map_err(|error| Failure::invalid(format!("{path}: {error}")))?
            };
            let inspection = inspect(&source);
            if operation == "formation.normalize" && inspection.valid {
                return rendered(json!(
                    inspection
                        .normalized
                        .expect("valid inspection has normalized formation")
                ));
            }
            let status = if inspection.valid {
                0
            } else if inspection
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "unsupported_version")
            {
                10
            } else {
                6
            };
            let valid = inspection.valid;
            let mut output = rendered(json!(inspection))?;
            output.ok = valid;
            output.status = status;
            Ok(output)
        }
        _ => Err(Failure::usage("unknown formation operation")),
    }
}
