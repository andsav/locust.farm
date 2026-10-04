//! Explicit local paths and blocking API connections.
use crate::{failure::Failure, secret};
use clap::ArgMatches;
use locust_proto::api::{Credential, ErrorCode, SessionSecret};
use locust_proto::client::{Client, ClientError};
use locust_proto::{API_VERSION, local};
use std::fs;
use std::os::unix::fs::PermissionsExt;
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
pub(super) fn read_secret(path: &Path) -> Result<[u8; 32], Failure> {
    let metadata = fs::metadata(path)
        .map_err(|error| Failure::invalid(format!("secret file {}: {error}", path.display())))?;
    let mode = metadata.permissions().mode() & 0o7777;
    if !metadata.is_file() || mode != local::SECRET_FILE_MODE {
        return Err(Failure::invalid(format!(
            "secret file {} must be a regular file with mode 0600 (found {mode:04o})",
            path.display()
        )));
    }
    secret::read(path)
        .map_err(|error| Failure::invalid(format!("secret file {}: {error}", path.display())))
}
pub(super) fn connect(socket: &Path) -> Result<UnixStream, Failure> {
    UnixStream::connect(socket).map_err(|error| {
        Failure::unavailable(format!(
            "daemon socket {} is not answering: {error}",
            socket.display()
        ))
    })
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
pub(super) fn client_error(error: ClientError, socket: &Path) -> Failure {
    match error {
        ClientError::Api(error) => error.into(),
        ClientError::Refused {
            error,
            api_version,
            daemon_version,
        } => Failure::new(
            error.code,
            format!(
                "daemon {daemon_version} at {} (API {api_version}; client API {API_VERSION}): {}",
                socket.display(),
                error.message
            ),
        ),
        ClientError::TooLarge => Failure::new(
            ErrorCode::LimitExceeded,
            "request exceeds the local API frame limit",
        ),
        ClientError::Protocol(detail) if detail.contains("another API version") => Failure::new(
            ErrorCode::UnsupportedVersion,
            format!(
                "daemon at {} welcomed another API version; client API is {API_VERSION}",
                socket.display()
            ),
        ),
        ClientError::Protocol(detail) => {
            Failure::internal(format!("daemon at {}: {detail}", socket.display()))
        }
        other => Failure::unavailable(format!(
            "daemon socket {} is not answering: {other}",
            socket.display()
        )),
    }
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
