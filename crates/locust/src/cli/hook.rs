//! The hook command selects a native adapter; all decisions belong to the core.
use clap::{Arg, ArgMatches, Command};
use locust_adapter::hooks::{self, Event, NativeInput};
use serde_json::Value;
use std::io::{self, Read};

use super::{connection, print};
use crate::failure::Failure;

pub(super) fn command() -> Command {
    Command::new("hook")
        .about("Read local work at a native chat hook; never sign or act")
        .arg(
            Arg::new("event")
                .required(true)
                .value_parser(["start", "stop", "tool"]),
        )
        .arg(
            Arg::new("harness")
                .long("harness")
                .required(true)
                .value_parser(clap::builder::PossibleValuesParser::new(
                    hooks::ADAPTERS.iter().map(|adapter| adapter.harness),
                )),
        )
}

pub(super) fn run(matches: &ArgMatches, selected: &ArgMatches) -> u8 {
    if std::env::var_os("LOCUST_HOOKS").is_some_and(|value| value == "off") {
        return 0;
    }
    let harness = selected.get_one::<String>("harness").expect("required");
    let adapter = hooks::ADAPTERS
        .iter()
        .find(|adapter| adapter.harness == harness)
        .expect("validated");
    let event = selected.get_one::<String>("event").expect("required");
    let mut input = NativeInput {
        client: adapter.client,
        chat: hooks::ChatIdentity {
            session_id: String::new(),
            agent_id: None,
        },
        event: match event.as_str() {
            "start" => Event::Start,
            "stop" => Event::Stop,
            _ => Event::Tool { own_call: None },
        },
        native_event_name: adapter
            .events
            .iter()
            .find(|mapping| mapping.event == event)
            .expect("registered")
            .native_name
            .to_owned(),
        compacted: false,
        stop_hook_active: false,
    };
    let result = (|| {
        if matches.get_flag("owner")
            || matches.get_one::<String>("agent").is_some()
            || matches.get_flag("json")
            || matches.get_one::<String>("idempotency-key").is_some()
        {
            return Err(Failure::usage("hooks require an agent execution session"));
        }
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|error| Failure::invalid(error.to_string()))?;
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|error| Failure::invalid(error.to_string()))?;
        input = hooks::parse_input(adapter.client, event, &value)
            .map_err(|error| Failure::invalid(error.to_string()))?;
        let home = connection::home(matches)?;
        let credential = connection::credential_path(matches, &home)?;
        let session = connection::session_path(matches)?
            .ok_or_else(|| Failure::usage("hooks require a session"))?;
        crate::hook::run(
            crate::hook::Config {
                home,
                credential,
                session,
                chat: serde_json::to_vec(&input.chat)
                    .map_err(|error| Failure::internal(error.to_string()))?,
                wait_limit_ms: adapter.wait_limit_ms,
                hook_timeout_ms: adapter
                    .stop_timeout_seconds
                    .saturating_mul(1000)
                    .saturating_sub(1000),
            },
            input.event.clone(),
        )
    })();
    let outcome = result.unwrap_or_else(|_| hooks::core::failure());
    match hooks::envelope(&input, &outcome) {
        Ok(Some(value)) => {
            let _ = print::stdout(format_args!("{value}\n"));
        }
        Ok(None) => (),
        Err(_) => {
            let _ = print::stdout(format_args!("Locust context was NOT injected\n"));
        }
    }
    // A broken integration must not make the native tool or turn fail.
    0
}
