//! Non-consuming observation of one goal for owners and agents.

use super::{Output, acting_agent, connection, presentation, print, resolve_goal, status};
use crate::failure::Failure;
use clap::{Arg, ArgMatches, Command};
use locust_proto::api::{Caller, Request, Response};
use locust_proto::id::IdempotencyKey;
use locust_proto::local;
use serde_json::json;

pub(super) fn command() -> Command {
    Command::new("watch")
        .about("Observe one goal until its next change or the explicit timeout; never acknowledges work or context")
        .arg(
            Arg::new("goal")
                .long("goal")
                .required(true)
                .help("Goal title, full identifier or unique prefix"),
        )
        .arg(
            Arg::new("timeout-ms")
                .long("timeout-ms")
                .default_value("30000")
                .value_parser(clap::value_parser!(u32))
                .help("Maximum wait in milliseconds; 0 checks immediately"),
        )
}

pub(super) fn run(matches: &ArgMatches, selected: &ArgMatches) -> Result<Output, Failure> {
    let home = connection::home(matches)?;
    let socket = local::socket_path(&home)?;
    let mut client = connection::open(matches, &home)?;
    // Preserve the old command's validation even though observation sends no retry key.
    matches
        .get_one::<String>("idempotency-key")
        .map(|key| {
            key.parse::<IdempotencyKey>()
                .map_err(|_| Failure::usage("--idempotency-key requires 16 bytes in hex"))
        })
        .transpose()?;
    let on_behalf = if matches.get_flag("owner") && matches.get_one::<String>("agent").is_some() {
        Some(acting_agent(&mut client, &socket, matches, None)?)
    } else {
        None
    };
    let display_principal = on_behalf.or(match client.caller() {
        Caller::Agent(principal) | Caller::Author(principal) => Some(principal),
        Caller::Owner => None,
    });
    let known = status(&mut client, &socket, on_behalf)?;
    let goal = resolve_goal(
        &mut client,
        &socket,
        selected.get_one::<String>("goal").expect("required goal"),
        on_behalf,
    )?;
    let response = client
        .call_with(Request::Pending { goal }, None, on_behalf)
        .map_err(|error| connection::client_error(error, &socket))?;
    let Response::Pending(initial) = response else {
        unreachable!("checked response")
    };
    let timeout_ms = *selected
        .get_one::<u32>("timeout-ms")
        .expect("default timeout");
    // Names and tasks for the view; none of this is read under --json.
    let (members, tasks) = if matches.get_flag("json") {
        (Vec::new(), Vec::new())
    } else {
        (
            super::members(&mut client, goal, on_behalf),
            super::board(&mut client, goal, on_behalf),
        )
    };
    let goals: Vec<_> = known.goals.iter().map(|summary| summary.goal).collect();
    let reader = presentation::Reader {
        goals: &goals,
        members: &members,
        tasks: &tasks,
        now_ms: super::now_ms(),
        ..presentation::Reader::new(super::voice(matches), display_principal, &known.agents)
    };
    if !matches.get_flag("json")
        && let Err(error) = print::stdout(format_args!(
            "{}\nObserving for up to {timeout_ms} ms without acknowledgment…\n",
            presentation::render(&Response::Pending(initial.clone()), Some(goal), &reader)
                .expect("pending renderer")
        ))
    {
        return Ok(Output {
            status: print::status(Err(error), 0),
            ..Output::success(json!(null), String::new())
        });
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
    let human = presentation::render(&response, Some(goal), &reader).expect("wait renderer");
    let human = format!("{human}\nObservation only: no work or context was acknowledged.");
    Ok(Output::success(
        json!({"goal":goal,"initial":initial,"result":response}),
        human,
    ))
}
