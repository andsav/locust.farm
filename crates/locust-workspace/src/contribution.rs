//! An inert delta between two exact stored manifests. No command, hook,
//! executable configuration, Git object name or local path is encoded here.

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::PathBuf;

use locust_proto::id::BlobHash;
use locust_proto::manifest::{Entry, Manifest};
use locust_proto::seal::MAX_PLAINTEXT_BYTES;
use serde::Serialize;

use crate::{BlobSink, BlobSource, ExportError, select};

pub use locust_proto::contribution::{Change, Contribution};

/// A content adapter that can read existing snapshots and store a contribution.
pub trait BlobStore: BlobSource + BlobSink {}
impl<T: BlobSource + BlobSink + ?Sized> BlobStore for T {}

#[derive(Clone, Debug, Serialize)]
pub struct ContributionReport {
    pub contribution_id: BlobHash,
    pub contribution: Contribution,
    pub head_manifest: Manifest,
    /// Local provenance only; never needed to fetch, review or apply a patch.
    pub source_commit: Option<String>,
    pub left_out: Vec<String>,
}

#[derive(Debug)]
pub enum ContributionError {
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
    HeadChanged {
        expected: String,
        actual: String,
    },
    NoChanges,
    /// No successful integration is claimed. Original bytes and the inert
    /// plan remain here; retrying the same contribution can finish it.
    RecoveryRequired {
        path: PathBuf,
        reason: String,
    },
    Export(ExportError),
    Io(io::Error),
}

impl fmt::Display for ContributionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(reason) => write!(f, "invalid contribution: {reason}"),
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
            Self::HeadChanged { expected, actual } => {
                write!(f, "Git HEAD changed: expected {expected}, found {actual}")
            }
            Self::NoChanges => f.write_str("the reviewed scope contains no changes"),
            Self::RecoveryRequired { path, reason } => write!(
                f,
                "application incomplete: {reason}; recoverable files at {}; retry this contribution after resolving the conflict",
                path.display()
            ),
            Self::Export(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ContributionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Export(e) => Some(e),
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}
impl From<io::Error> for ContributionError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<ExportError> for ContributionError {
    fn from(value: ExportError) -> Self {
        Self::Export(value)
    }
}
impl From<rustix::io::Errno> for ContributionError {
    fn from(value: rustix::io::Errno) -> Self {
        Self::Io(value.into())
    }
}

pub(crate) fn invalid(reason: impl Into<String>) -> ContributionError {
    ContributionError::Invalid(reason.into())
}

pub(crate) fn check_path(path: &str) -> Result<(), ContributionError> {
    if !locust_proto::manifest::is_safe_path(path) {
        return Err(invalid(format!("unsafe path {path:?}")));
    }
    if select::is_denied(path) || path.split('/').any(|p| p.starts_with(".locust-apply-")) {
        return Err(ContributionError::Unsupported {
            path: path.into(),
            reason: "private or reserved workspace path".into(),
        });
    }
    Ok(())
}

impl From<locust_proto::contribution::ContributionError> for ContributionError {
    fn from(value: locust_proto::contribution::ContributionError) -> Self {
        match value {
            locust_proto::contribution::ContributionError::Invalid(reason) => Self::Invalid(reason),
            locust_proto::contribution::ContributionError::NoChanges => Self::NoChanges,
        }
    }
}

pub(crate) fn fetch(
    source: &mut dyn BlobSource,
    id: BlobHash,
) -> Result<Vec<u8>, ContributionError> {
    let bytes = source
        .fetch(&id)?
        .ok_or(ContributionError::MissingObject(id))?;
    if bytes.len() > MAX_PLAINTEXT_BYTES {
        return Err(invalid("object exceeds content-object limit"));
    }
    Ok(bytes)
}

pub(crate) fn manifest(
    source: &mut dyn BlobSource,
    id: BlobHash,
) -> Result<Manifest, ContributionError> {
    Manifest::decode(&fetch(source, id)?).map_err(|e| invalid(e.to_string()))
}

pub(crate) fn file_bytes(
    source: &mut dyn BlobSource,
    entry: &Entry,
) -> Result<Vec<u8>, ContributionError> {
    let bytes = fetch(source, entry.content)?;
    if bytes.len() as u64 != entry.size {
        return Err(ContributionError::WrongSize {
            path: entry.path.clone(),
            expected: entry.size,
            actual: bytes.len() as u64,
        });
    }
    Ok(bytes)
}

pub(crate) fn changes(base: &Manifest, head: &Manifest) -> Vec<Change> {
    let mut paths = BTreeMap::new();
    for e in &base.entries {
        paths.entry(e.path.clone()).or_insert((None, None)).0 = Some(e.clone());
    }
    for e in &head.entries {
        paths.entry(e.path.clone()).or_insert((None, None)).1 = Some(e.clone());
    }
    paths
        .into_iter()
        .filter_map(|(path, (before, after))| {
            (before != after).then_some(Change {
                path,
                before,
                after,
            })
        })
        .collect()
}

pub(crate) fn load(
    id: BlobHash,
    source: &mut dyn BlobSource,
) -> Result<(Contribution, Manifest, Manifest), ContributionError> {
    let contribution = Contribution::decode(&fetch(source, id)?)?;
    for change in &contribution.changes {
        check_path(&change.path)?;
    }
    let base = manifest(source, contribution.base)?;
    let head = manifest(source, contribution.head)?;
    if changes(&base, &head) != contribution.changes {
        return Err(invalid(
            "delta does not exactly match its base and head manifests",
        ));
    }
    Ok((contribution, base, head))
}

pub(crate) fn finish(
    base_id: BlobHash,
    base: &Manifest,
    head: Manifest,
    source_commit: Option<String>,
    left_out: Vec<String>,
    store: &mut dyn BlobStore,
) -> Result<ContributionReport, ContributionError> {
    let delta = changes(base, &head);
    for change in &delta {
        check_path(&change.path)?;
    }
    if delta.is_empty() {
        return Err(ContributionError::NoChanges);
    }
    let head_bytes = head.encode().map_err(|e| invalid(e.to_string()))?;
    if head_bytes.len() > MAX_PLAINTEXT_BYTES {
        return Err(invalid("head manifest exceeds content-object limit"));
    }
    let head_id = store.store(&head_bytes)?;
    let contribution = Contribution {
        version: 1,
        base: base_id,
        head: head_id,
        changes: delta,
    };
    let contribution_id = store.store(&contribution.encode()?)?;
    Ok(ContributionReport {
        contribution_id,
        contribution,
        head_manifest: head,
        source_commit,
        left_out,
    })
}
