//! Shared protected credentials and authenticated local API error handling.
use crate::{failure::Failure, secret};
use locust_proto::api::ErrorCode;
use locust_proto::client::ClientError;
use locust_proto::{API_VERSION, local};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::Path;

pub(crate) fn read_secret(path: &Path) -> Result<[u8; 32], Failure> {
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
pub(crate) fn connect(socket: &Path) -> Result<UnixStream, Failure> {
    UnixStream::connect(socket).map_err(|error| {
        Failure::unavailable(format!(
            "daemon socket {} is not answering: {error}",
            socket.display()
        ))
    })
}
pub(crate) fn client_error(error: ClientError, socket: &Path) -> Failure {
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
