//! Explicit local paths and blocking API connections.
pub(super) use crate::connection::{client_error, connect, read_secret};
use crate::failure::Failure;
use clap::ArgMatches;
use locust_proto::api::{Credential, SessionSecret};
use locust_proto::client::Client;
use locust_proto::local;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

pub(super) fn home(matches: &ArgMatches) -> Result<PathBuf, Failure> {
    let configured = matches
        .get_one::<String>("home")
        .map(std::ffi::OsString::from)
        .or_else(|| std::env::var_os(local::HOME_ENV));
    let user_home = std::env::var_os("HOME");
    local::home_dir(configured.as_deref(), user_home.as_deref())
        .map_err(|error| path_error(error, matches, "home"))
}
pub(super) fn credential_path(matches: &ArgMatches, home: &Path) -> Result<PathBuf, Failure> {
    if matches.get_flag("owner") {
        return Ok(local::owner_credential_path(home));
    }
    let configured = matches
        .get_one::<String>("credential")
        .map(std::ffi::OsString::from)
        .or_else(|| std::env::var_os(local::CREDENTIAL_ENV));
    local::credential_path(configured.as_deref())
        .map_err(|error| path_error(error, matches, "credential"))
}
pub(super) fn session_path(matches: &ArgMatches) -> Result<Option<PathBuf>, Failure> {
    let configured = matches
        .get_one::<String>("session")
        .map(std::ffi::OsString::from)
        .or_else(|| std::env::var_os(local::SESSION_ENV));
    local::session_path(configured.as_deref())
        .map_err(|error| path_error(error, matches, "session"))
}
pub(super) fn open(matches: &ArgMatches, home: &Path) -> Result<Client<UnixStream>, Failure> {
    let socket = local::socket_path(home).map_err(|error| path_error(error, matches, "home"))?;
    let credential_path = credential_path(matches, home)?;
    let session_path = session_path(matches)?;
    let stream = connect(&socket)?;
    let credential = Credential(read_secret(&credential_path)?);
    let session = session_path
        .as_deref()
        .map(read_secret)
        .transpose()?
        .map(SessionSecret);
    Client::open(stream, credential, session).map_err(|error| client_error(error, &socket))
}
fn path_error(error: local::LocalError, matches: &ArgMatches, option: &str) -> Failure {
    match error {
        local::LocalError::NoCredential => Failure::usage(
            "select a credential with --credential <absolute-path> or LOCUST_CREDENTIAL, or use --owner for owner authority",
        ),
        local::LocalError::NotAbsolute(_) if matches.get_one::<String>(option).is_some() => {
            Failure::usage(format!("--{option} must be an absolute path"))
        }
        local::LocalError::SocketPathTooLong(_) if matches.get_one::<String>("home").is_some() => {
            Failure::usage(format!(
                "{}; choose a shorter --home directory",
                error.to_string().split(';').next().unwrap()
            ))
        }
        _ => error.into(),
    }
}
