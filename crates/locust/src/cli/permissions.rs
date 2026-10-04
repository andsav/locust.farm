//! Human permission controls, attention inbox and non-consuming observation.

use clap::{Arg, ArgMatches, Command};
use locust_proto::api::{Caller, GoalPermission, Request, Response};
use locust_proto::id::IdempotencyKey;
use locust_proto::local;
use serde_json::json;

use super::{Output, connection, presentation, resolve_goal, resolve_principal, status};
use crate::failure::Failure;

fn goal() -> Arg {
    Arg::new("goal")
        .long("goal")
        .required(true)
        .help("Goal title, full identifier or unique prefix")
}

fn agent() -> Arg {
    Arg::new("agent")
        .long("agent")
        .required(true)
        .help("Enrolled local name or full principal key")
}

fn change(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .arg(goal())
        .arg(agent())
        .arg(
            Arg::new("task")
                .long("task")
                .help("Task title or typed task:/effect: identifier or unique prefix"),
        )
        .arg(
            Arg::new("permissions")
                .num_args(1..)
                .required_unless_present("task")
                .value_parser([
                    "administer",
                    "contribute",
                    "execute",
                    "review",
                    "select",
                    "flow",
                    "takeover",
                ])
                .help("Independent local permission names; other permissions stay unchanged"),
        )
}

pub(super) fn commands() -> [Command; 3] {
    [
        Command::new("permission").about("Inspect or change explicit local permissions; membership and client tool approval are separate")
            .subcommand_required(true)
            .subcommand(Command::new("inspect").about("Show standing permissions and task-specific execution authorizations").arg(goal()).arg(agent()))
            .subcommand(change("allow", "Allow only the named permissions; --task requires execute, optionally takeover"))
            .subcommand(change("revoke", "Revoke named permissions or one task's authorizations; does not stop a client process")),
        Command::new("inbox").about("Show the owner's local participants needing permission, action, or review across goals without acknowledgment"),
        Command::new("watch").about("Observe one goal until its next change or the explicit timeout; never acknowledges work or context")
            .arg(goal())
            .arg(Arg::new("timeout-ms").long("timeout-ms").default_value("30000")
                .value_parser(clap::value_parser!(u32)).help("Maximum wait in milliseconds; 0 checks immediately")),
    ]
}

pub(super) fn run(
    matches: &ArgMatches,
    operation: &str,
    selected: &ArgMatches,
) -> Result<Output, Failure> {
    let home = connection::home(matches)?;
    let socket = local::socket_path(&home)?;
    let mut client = connection::open(matches, &home)?;
    let idempotency = matches
        .get_one::<String>("idempotency-key")
        .map(|key| {
            key.parse::<IdempotencyKey>()
                .map_err(|_| Failure::usage("--idempotency-key requires 16 bytes in hex"))
        })
        .transpose()?;
    let on_behalf = matches
        .get_one::<String>("as")
        .map(|name| resolve_principal(&mut client, &socket, name))
        .transpose()?;
    let display_principal = on_behalf.or(match client.caller() {
        Caller::Agent(principal) | Caller::Viewer(principal) | Caller::Author(principal) => {
            Some(principal)
        }
        Caller::Owner => None,
    });
    if operation != "watch" && on_behalf.is_some() {
        return Err(Failure::usage(
            "permission and inbox are owner controls; use --owner without --as",
        ));
    }
    let known = status(&mut client, &socket, on_behalf)?;
    if operation == "inbox" {
        let response = client
            .call(Request::Inbox)
            .map_err(|error| connection::client_error(error, &socket))?;
        return output(response, &known.agents, None, None);
    }
    let goal = resolve_goal(
        &mut client,
        &socket,
        selected.get_one::<String>("goal").expect("required goal"),
        on_behalf,
    )?;
    if operation == "watch" {
        let response = client
            .call_with(Request::Pending { goal }, None, on_behalf)
            .map_err(|error| connection::client_error(error, &socket))?;
        let Response::Pending(initial) = response else {
            unreachable!("checked response")
        };
        let timeout_ms = *selected
            .get_one::<u32>("timeout-ms")
            .expect("default timeout");
        if !matches.get_flag("json") {
            println!(
                "{}",
                presentation::render(
                    &Response::Pending(initial.clone()),
                    &known.agents,
                    Some(goal),
                    display_principal
                )
                .expect("pending renderer")
            );
            println!("Observing for up to {timeout_ms} ms without acknowledgment…");
        }
        let response = client
            .call_with(
                Request::Wait {
                    goal,
                    seen: initial.revision,
                    timeout_ms,
                },
                None,
                on_behalf,
            )
            .map_err(|error| connection::client_error(error, &socket))?;
        let human = presentation::render(&response, &known.agents, Some(goal), display_principal)
            .expect("wait renderer");
        let human = format!("{human}\nObservation only: no work or context was acknowledged.");
        return Ok(Output::success(
            json!({"goal":goal,"initial":initial,"result":response}),
            human,
        ));
    }
    let agent = resolve_principal(
        &mut client,
        &socket,
        selected.get_one::<String>("agent").expect("required agent"),
    )?;
    let request = if operation == "permission.inspect" {
        Request::Permissions { goal, agent }
    } else {
        let permissions = selected
            .get_many::<String>("permissions")
            .into_iter()
            .flatten()
            .map(|value| {
                serde_json::from_value::<GoalPermission>(json!(value))
                    .expect("clap checks permissions")
            })
            .collect::<Vec<_>>();
        let task = selected
            .get_one::<String>("task")
            .map(|task| super::selectors::resolve_task(&mut client, &socket, goal, task, None))
            .transpose()?;
        match (operation, task) {
            ("permission.allow", Some(task)) => {
                if !permissions.contains(&GoalPermission::Execute)
                    || permissions.iter().any(|permission| {
                        !matches!(
                            permission,
                            GoalPermission::Execute | GoalPermission::Takeover
                        )
                    })
                {
                    return Err(Failure::usage(
                        "task authorization requires explicit execute and optional takeover only; use a standing permission for the other categories",
                    ));
                }
                Request::PermissionTaskAllow {
                    goal,
                    agent,
                    task,
                    takeover: permissions.contains(&GoalPermission::Takeover),
                }
            }
            ("permission.revoke", Some(task)) => {
                if !permissions.is_empty() {
                    return Err(Failure::usage(
                        "--task revokes that task's complete execution authorization; omit the standing permission names",
                    ));
                }
                Request::PermissionTaskRevoke { goal, agent, task }
            }
            ("permission.allow", None) => Request::PermissionAllow {
                goal,
                agent,
                permissions,
            },
            ("permission.revoke", None) => Request::PermissionRevoke {
                goal,
                agent,
                permissions,
            },
            _ => return Err(Failure::usage("unknown permission command")),
        }
    };
    request.check().map_err(Failure::from)?;
    let response = client
        .call_with(request, idempotency, None)
        .map_err(|error| connection::client_error(error, &socket))?;
    output(response, &known.agents, Some(goal), None)
}

fn output(
    response: Response,
    names: &[locust_proto::api::AgentView],
    goal: Option<locust_proto::id::GoalId>,
    principal: Option<locust_proto::id::PublicKey>,
) -> Result<Output, Failure> {
    let human =
        presentation::render(&response, names, goal, principal).expect("permission renderer");
    let result =
        serde_json::to_value(response).map_err(|error| Failure::internal(error.to_string()))?;
    Ok(Output::success(result, human))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_controls_need_no_json_and_keep_categories_separate() {
        let command = || Command::new("locust").subcommands(commands());
        let args = command()
            .try_get_matches_from([
                "locust",
                "permission",
                "allow",
                "--goal",
                "01010101",
                "--agent",
                "worker",
                "execute",
                "review",
            ])
            .unwrap();
        let (_, selected) = super::super::args::selected(&args);
        assert_eq!(
            selected
                .get_many::<String>("permissions")
                .unwrap()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["execute", "review"]
        );
        assert!(
            command()
                .try_get_matches_from([
                    "locust",
                    "permission",
                    "revoke",
                    "--goal",
                    "01010101",
                    "--agent",
                    "worker",
                    "--task",
                    "task:123"
                ])
                .is_ok()
        );
        assert!(
            command()
                .try_get_matches_from([
                    "locust",
                    "permission",
                    "allow",
                    "--goal",
                    "01010101",
                    "--agent",
                    "worker",
                    "all"
                ])
                .is_err()
        );
    }
}
