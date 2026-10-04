//! Readiness checks, in the published order, without creating local state.
use super::connection;
use crate::daemon::home::lock_holder;
use crate::failure::Failure;
use clap::ArgMatches;
use locust_proto::api::{Credential, Request, SessionSecret};
use locust_proto::client::Client;
use locust_proto::local;
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn check(checks: &mut Vec<Value>, name: &str, result: Result<String, Failure>) {
    let (ok, detail) = match result {
        Ok(detail) => (true, detail),
        Err(error) => (false, error.to_string()),
    };
    checks.push(json!({"name": name, "ok": ok, "detail": detail}));
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
    let ok = checks.iter().all(|check| check["ok"] == true);
    let human = checks
        .iter()
        .map(|check| {
            format!(
                "{} {}: {}",
                if check["ok"] == true { "ok" } else { "failed" },
                check["name"].as_str().unwrap(),
                check["detail"].as_str().unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    (json!({"checks": checks}), human, u8::from(!ok), ok)
}
