//! Shared plaintext file validation and content adapters.

use std::fmt;
use std::io;
use std::path::PathBuf;

use locust_proto::id::BlobHash;
use locust_proto::manifest::{Entry, Manifest};
use locust_proto::seal::MAX_PLAINTEXT_BYTES;

use crate::{BlobSink, BlobSource, ExportError, select};

/// A content adapter that can read existing snapshots and store a contribution.
pub trait BlobStore: BlobSource + BlobSink {}
impl<T: BlobSource + BlobSink + ?Sized> BlobStore for T {}

#[derive(Debug)]
pub enum WorkspaceError {
    Invalid(String),
    MissingObject(BlobHash),
    WrongSize {
        path: String,
        expected: u64,
        actual: u64,
    },
    Unsupported {
        path: String,
        reason: String,
    },
    Conflict {
        path: String,
        reason: String,
    },
    /// No successful integration is claimed. Original bytes and the inert
    /// plan remain here; retrying the same contribution can finish it.
    RecoveryRequired {
        path: PathBuf,
        reason: String,
    },
    Export(ExportError),
    Io(io::Error),
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(reason) => write!(f, "invalid workspace: {reason}"),
            Self::MissingObject(id) => write!(f, "object {id} is unavailable"),
            Self::WrongSize {
                path,
                expected,
                actual,
            } => write!(f, "{path}: expected {expected} bytes, received {actual}"),
            Self::Unsupported { path, reason } => write!(f, "{path}: unsupported {reason}"),
            Self::Conflict { path, reason } => write!(
                f,
                "{path}: local conflict: {reason}; no integration recorded"
            ),
            Self::RecoveryRequired { path, reason } => write!(
                f,
                "application incomplete: {reason}; recoverable files at {}; resume the recorded workspace operation after resolving the conflict",
                path.display()
            ),
            Self::Export(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for WorkspaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Export(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}
impl From<io::Error> for WorkspaceError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<ExportError> for WorkspaceError {
    fn from(value: ExportError) -> Self {
        Self::Export(value)
    }
}
impl From<rustix::io::Errno> for WorkspaceError {
    fn from(value: rustix::io::Errno) -> Self {
        Self::Io(value.into())
    }
}

pub(crate) fn invalid(reason: impl Into<String>) -> WorkspaceError {
    WorkspaceError::Invalid(reason.into())
}

pub(crate) fn check_path(path: &str) -> Result<(), WorkspaceError> {
    if !locust_proto::manifest::is_safe_path(path) {
        return Err(invalid(format!("unsafe path {path:?}")));
    }
    if select::is_denied(path) {
        return Err(WorkspaceError::Unsupported {
            path: path.into(),
            reason: "private or reserved workspace path".into(),
        });
    }
    Ok(())
}

pub(crate) fn fetch(source: &mut dyn BlobSource, id: BlobHash) -> Result<Vec<u8>, WorkspaceError> {
    let bytes = source
        .fetch(&id)?
        .ok_or(WorkspaceError::MissingObject(id))?;
    if bytes.len() > MAX_PLAINTEXT_BYTES {
        return Err(invalid("object exceeds content-object limit"));
    }
    Ok(bytes)
}

pub(crate) fn manifest(
    source: &mut dyn BlobSource,
    id: BlobHash,
) -> Result<Manifest, WorkspaceError> {
    Manifest::decode(&fetch(source, id)?).map_err(|e| invalid(e.to_string()))
}

pub(crate) fn file_bytes(
    source: &mut dyn BlobSource,
    entry: &Entry,
) -> Result<Vec<u8>, WorkspaceError> {
    let bytes = fetch(source, entry.content)?;
    if bytes.len() as u64 != entry.size {
        return Err(WorkspaceError::WrongSize {
            path: entry.path.clone(),
            expected: entry.size,
            actual: bytes.len() as u64,
        });
    }
    Ok(bytes)
}
