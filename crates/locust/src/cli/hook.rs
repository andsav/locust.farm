//! The hook command selects a native adapter; all decisions belong to the core.
use clap::{Arg, ArgMatches, Command};
use locust_adapter::hooks::{self, Event, Parsed, core};
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

/// `LOCUST_HOOKS=off` silences every hook. `LOCUST_HOOKS=unattended` says no
/// person types in this harness, so an idle worker may wait at its turn end.
pub(super) fn run(matches: &ArgMatches, selected: &ArgMatches) -> u8 {
    let setting = std::env::var_os("LOCUST_HOOKS");
    if setting.as_deref().is_some_and(|value| value == "off") {
        return 0;
    }
    let harness = selected.get_one::<String>("harness").expect("required");
    let adapter = hooks::ADAPTERS
        .iter()
        .find(|adapter| adapter.harness == harness)
        .expect("validated");
    let event = selected.get_one::<String>("event").expect("required");
    let mut bytes = Vec::new();
    let value: Option<Value> = io::stdin()
        .read_to_end(&mut bytes)
        .ok()
        .and_then(|_| serde_json::from_slice(&bytes).ok());
    let mut parsed = value.as_ref().map_or(Parsed::Ignored, |value| {
        hooks::parse_input(adapter, event, value)
    });
    if let Parsed::Input(input) = &mut parsed
        && let Event::Stop { unattended } = &mut input.event
    {
        *unattended |= setting
            .as_deref()
            .is_some_and(|value| value == "unattended");
    }
    let native_event_name = match &parsed {
        Parsed::Input(input) => input.native_event_name.clone(),
        Parsed::Invalid {
            native_event_name, ..
        } => native_event_name.clone(),
        Parsed::Ignored => String::new(),
    };
    let config = (|| {
        if matches.get_flag("owner")
            || matches.get_one::<String>("agent").is_some()
            || matches.get_flag("json")
            || matches.get_one::<String>("idempotency-key").is_some()
        {
            return Err(Failure::usage("hooks require an agent execution session"));
        }
        let home = connection::home(matches)?;
        Ok(crate::hook::Config {
            credential: connection::credential_path(matches, &home)?,
            session: connection::session_path(matches)?
                .ok_or_else(|| Failure::usage("hooks require a session"))?,
            home,
            stop_timeout_seconds: adapter.stop_timeout_seconds,
        })
    })();
    let outcome = crate::hook::run(config, parsed);
    // An outcome the adapter cannot carry is a fault of this build; the chat
    // already heard from Locust, so it gets the core's failure line instead.
    let envelope = hooks::envelope(adapter, event, &native_event_name, &outcome)
        .or_else(|_| hooks::envelope(adapter, event, &native_event_name, &core::failure()));
    if let Ok(Some(value)) = envelope {
        let _ = print::stdout(format_args!("{value}\n"));
    }
    // A broken integration must not make the native tool or turn fail.
    0
}
