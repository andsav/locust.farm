//! Export: the files of one commit under an export root become a stored
//! manifest.

use std::fmt;
use std::io;
use std::path::Path;

use locust_proto::id::BlobHash;
use locust_proto::limits::MAX_BLOB_BYTES;
use locust_proto::manifest::{Entry, Manifest, ManifestError, is_safe_path};

use crate::{BlobSink, git, select};

/// What an export shared, left out and refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportReport {
    /// The stored manifest, entries in ascending byte order of path.
    pub manifest: Manifest,
    /// The manifest's identifier, as the sink assigned it. This identifies
    /// the shared snapshot.
    pub manifest_id: BlobHash,
    /// Full object name of the exported commit; provenance only.
    pub commit: String,
    /// Number of files in the manifest.
    pub files: usize,
    /// Sum of the files' sizes.
    pub total_bytes: u64,
    /// Paths the default-deny selection left out.
    pub left_out: Vec<String>,
    /// Paths a manifest cannot carry: not UTF-8 (shown lossily), or refused
    /// by [`is_safe_path`].
    pub refused: Vec<String>,
}

#[derive(Debug)]
pub enum ExportError {
    /// The export root is not inside a Git work tree.
    NotInWorkTree,
    /// A Git command failed or answered unexpectedly, including a commit
    /// name that resolves to no commit or an export root the commit lacks.
    Git(String),
    /// A selected path is a symlink, a submodule or another entry that is
    /// not a regular file. Nothing was stored.
    Unsupported { path: String, mode: String },
    /// A selected file is larger than one content object may be. Nothing was
    /// stored.
    TooLarge { path: String, size: u64 },
    /// The selected files do not form a valid manifest: too many entries,
    /// or a malformed tree. Nothing was stored.
    Manifest(ManifestError),
    /// The encoded manifest would be larger than one content object may be.
    /// Nothing was stored.
    ManifestTooLarge { bytes: usize },
    /// The sink failed to store an object.
    Sink(io::Error),
    /// Running Git failed.
    Io(io::Error),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInWorkTree => f.write_str("the export root is not inside a Git work tree"),
            Self::Git(detail) => write!(f, "git: {detail}"),
            Self::Unsupported { path, mode } => {
                let kind = match mode.as_str() {
                    "120000" => "a symbolic link".to_owned(),
                    "160000" => "a submodule".to_owned(),
                    other => format!("mode {other}"),
                };
                write!(f, "{path} is {kind}; only regular files can be exported")
            }
            Self::TooLarge { path, size } => write!(
                f,
                "{path} is {size} bytes, over the {MAX_BLOB_BYTES}-byte object limit"
            ),
            Self::Manifest(error) => {
                write!(f, "the selected files are not a valid manifest: {error}")
            }
            Self::ManifestTooLarge { bytes } => write!(
                f,
                "the manifest encodes to {bytes} bytes, over the {MAX_BLOB_BYTES}-byte object limit"
            ),
            Self::Sink(error) => write!(f, "storing an object failed: {error}"),
            Self::Io(error) => write!(f, "running git failed: {error}"),
        }
    }
}

impl std::error::Error for ExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Manifest(error) => Some(error),
            Self::Sink(error) | Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ExportError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Exports the files of `commit` that lie under `root`, a directory inside a
/// Git work tree, with paths relative to `root`.
///
/// Only the committed tree is read: uncommitted and untracked files are not
/// exported. Paths a manifest cannot carry are refused and paths the
/// default-deny selection matches are left out, both reported; any other
/// entry that is not a regular file, or is over the object size limit, fails
/// the export before anything is stored. Each file is then stored through
/// `sink`, and finally the encoded manifest.
pub fn export(
    root: &Path,
    commit: &str,
    sink: &mut dyn BlobSink,
) -> Result<ExportReport, ExportError> {
    let prefix = git::work_tree_prefix(root)?;
    let commit = git::resolve_commit(root, commit)?;

    let mut selected = Vec::new();
    let mut left_out = Vec::new();
    let mut refused = Vec::new();
    for item in git::list_tree(root, &commit, &prefix)? {
        let path = match String::from_utf8(item.path) {
            Ok(path) if is_safe_path(&path) => path,
            Ok(path) => {
                refused.push(path);
                continue;
            }
            Err(error) => {
                refused.push(String::from_utf8_lossy(error.as_bytes()).into_owned());
                continue;
            }
        };
        if select::is_denied(&path) {
            left_out.push(path);
            continue;
        }
        let executable = match item.mode.as_str() {
            "100644" => false,
            "100755" => true,
            _ => {
                return Err(ExportError::Unsupported {
                    path,
                    mode: item.mode,
                });
            }
        };
        if item.size > MAX_BLOB_BYTES as u64 {
            return Err(ExportError::TooLarge {
                path,
                size: item.size,
            });
        }
        // The content identifier is filled in once the file is stored.
        let entry = Entry {
            path,
            executable,
            size: item.size,
            content: BlobHash([0; 32]),
        };
        selected.push((entry, item.object));
    }
    selected.sort_unstable_by(|a, b| a.0.path.cmp(&b.0.path));
    let (entries, objects): (Vec<_>, Vec<_>) = selected.into_iter().unzip();
    let mut manifest = Manifest { entries };

    // Check the manifest before moving any bytes. Identifiers have a fixed
    // width, so the encoded size is already final.
    let planned = manifest.encode().map_err(ExportError::Manifest)?.len();
    if planned > MAX_BLOB_BYTES {
        return Err(ExportError::ManifestTooLarge { bytes: planned });
    }

    let mut blobs = git::Blobs::open(root)?;
    for (entry, object) in manifest.entries.iter_mut().zip(&objects) {
        let bytes = blobs.read(object, entry.size)?;
        entry.content = sink.store(&bytes).map_err(ExportError::Sink)?;
    }
    drop(blobs);

    let encoded = manifest.encode().map_err(ExportError::Manifest)?;
    let manifest_id = sink.store(&encoded).map_err(ExportError::Sink)?;
    Ok(ExportReport {
        files: manifest.entries.len(),
        total_bytes: manifest.entries.iter().map(|entry| entry.size).sum(),
        manifest,
        manifest_id,
        commit,
        left_out,
        refused,
    })
}
