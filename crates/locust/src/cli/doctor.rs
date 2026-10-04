//! Readiness checks, in the published order, without creating local state.
use super::connection;
use crate::daemon::home::lock_holder;
use crate::failure::Failure;
use clap::{Arg, ArgMatches, Command};
mod profile;
#[cfg(test)]
mod tests;
use locust_proto::api::{Credential, Request, SessionSecret};
use locust_proto::client::Client;
use locust_proto::local;
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub(super) fn command() -> Command {
    Command::new("doctor")
        .about("Check daemon readiness and a selected installation or enrolled client profile")
        .arg(Arg::new("prefix").long("prefix").help("Software prefix; defaults to the selected onboarding journal"))
        .arg(Arg::new("client").long("client").value_parser(["codex", "claude", "pi", "droid", "shell"]).help("Inspect this enrolled client in the selected profile"))
        .arg(Arg::new("profile-home").long("profile-home").requires("client").help("Selected client profile home (defaults to HOME)"))
        .arg(Arg::new("workspace").long("workspace").requires("client").help("Workspace to inspect (defaults to the enrolled workspace)"))
        .arg(Arg::new("service").long("service").value_parser(["launchd", "systemd", "none"]).help("Service manager to inspect; defaults to this platform when inspecting a client"))
        .arg(Arg::new("service-profile-home").long("service-profile-home").help("Service manager profile home (defaults to HOME)"))
        .arg(Arg::new("log-dir").long("log-dir").help("Service logs directory (defaults to daemon home/logs)"))
}

fn check(checks: &mut Vec<Value>, name: &str, result: Result<String, Failure>) {
    let (ok, detail) = match result {
        Ok(detail) => (true, detail),
        Err(error) => (false, error.to_string()),
    };
    let recovery = if ok {
        None
    } else {
        Some(match name {
            "state_directory" => {
                "Inspect the selected daemon home and restore its user ownership and mode 0700, then rerun doctor."
            }
            "socket_path" => "Select a shorter absolute daemon home with --home.",
            "daemon_lock" | "socket_connection" => {
                "Start the daemon for this --home with locust up, or inspect its service and logs if it is already running."
            }
            "hello" => {
                "Check the credential selection and use the same installed release for the daemon and CLI; inspect the daemon logs before retrying."
            }
            "status" => {
                "Inspect the selected daemon logs, correct the reported API error, and rerun doctor."
            }
            "credential_file" => {
                "Restore the original selected credential file with mode 0600, or choose --owner for owner diagnostics."
            }
            "session_file" => {
                "Restore the original selected session file with mode 0600; do not replace an enrolled session identity."
            }
            _ => "Review the reported selection and rerun doctor.",
        })
    };
    checks.push(json!({"name": name, "ok": ok, "detail": detail, "recovery": recovery}));
}
fn private_home(home: &Path) -> Result<String, Failure> {
    let metadata = fs::metadata(home)
        .map_err(|error| Failure::invalid(format!("{}: {error}", home.display())))?;
    let mode = metadata.permissions().mode() & 0o7777;
    if metadata.is_dir() && mode == local::HOME_MODE {
        Ok(format!("{} mode {mode:04o}", home.display()))
    } else {
        Err(Failure::invalid(format!(
            "{} must be a directory with mode 0700 (found {mode:04o})",
            home.display()
        )))
    }
}
pub(super) fn run(matches: &ArgMatches, home: &Path) -> (Value, String, u8, bool) {
    let args = matches
        .subcommand_matches("doctor")
        .expect("doctor arguments");
    let selected =
        args.get_one::<String>("client").is_some() || args.get_one::<String>("prefix").is_some();
    let mut checks = Vec::new();
    check(&mut checks, "state_directory", private_home(home));
    let socket = local::socket_path(home).map_err(Failure::from);
    check(
        &mut checks,
        "socket_path",
        socket
            .as_ref()
            .map(|path| path.display().to_string())
            .map_err(Clone::clone),
    );
    check(
        &mut checks,
        "daemon_lock",
        lock_holder(home)
            .map_err(|error| {
                Failure::unavailable(format!(
                    "daemon lock {}: {error}",
                    local::lock_path(home).display()
                ))
            })
            .and_then(|holder| {
                holder
                    .map(|pid| format!("held by process {pid}"))
                    .ok_or_else(|| Failure::unavailable("daemon lock is not held"))
            }),
    );
    let stream = socket
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|path| connection::connect(path));
    check(
        &mut checks,
        "socket_connection",
        stream
            .as_ref()
            .map(|_| "socket accepted a connection".to_owned())
            .map_err(Clone::clone),
    );
    let credential = connection::credential_path(matches, home)
        .or_else(|error| {
            if selected
                && error.usage
                && matches.get_one::<String>("credential").is_none()
                && std::env::var_os(local::CREDENTIAL_ENV).is_none()
            {
                Ok(local::owner_credential_path(home))
            } else {
                Err(error)
            }
        })
        .and_then(|path| connection::read_secret(&path).map(Credential));
    let session_path = connection::session_path(matches);
    let session = session_path
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|path| {
            path.as_deref()
                .map(connection::read_secret)
                .transpose()
                .map(|secret| secret.map(SessionSecret))
        });
    let client = stream.and_then(|stream| {
        let credential = credential.as_ref().map_err(Clone::clone)?;
        let session = *session.as_ref().map_err(Clone::clone)?;
        Client::open(stream, *credential, session).map_err(|error| {
            connection::client_error(error, socket.as_ref().expect("connected socket"))
        })
    });
    check(
        &mut checks,
        "hello",
        client
            .as_ref()
            .map(|client| {
                format!(
                    "API {} daemon {}",
                    locust_proto::API_VERSION,
                    client.daemon_version()
                )
            })
            .map_err(Clone::clone),
    );
    let status = client.and_then(|mut client| {
        client.call(Request::Status).map_err(|error| {
            connection::client_error(error, socket.as_ref().expect("connected socket"))
        })
    });
    check(
        &mut checks,
        "status",
        status.map(|_| "status succeeded".to_owned()),
    );
    check(
        &mut checks,
        "credential_file",
        credential.map(|_| "readable, 32 bytes, mode 0600".to_owned()),
    );
    match session_path {
        Ok(None) => {}
        _ => check(
            &mut checks,
            "session_file",
            session.map(|_| "readable, 32 bytes, mode 0600".to_owned()),
        ),
    }
    let profile = profile::inspect(args, home, &mut checks);
    let ok = checks.iter().all(|check| check["ok"] == true);
    let human = checks
        .iter()
        .map(|check| {
            format!(
                "{} {}: {}{}",
                if check["ok"] == true { "ok" } else { "failed" },
                check["name"].as_str().unwrap(),
                check["detail"].as_str().unwrap(),
                check["recovery"]
                    .as_str()
                    .map(|recovery| format!("\n  Next: {recovery}"))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let human = if profile.is_null() {
        human
    } else {
        format!(
            "{human}\nNative client discovery: unverified. Start a fresh chat in the selected profile and verify Locust tools and the bound launcher."
        )
    };
    (
        json!({"checks": checks,"profile":profile,"model_ready":false}),
        human,
        u8::from(!ok),
        ok,
    )
}
