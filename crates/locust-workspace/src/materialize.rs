//! Materialize exact manifest bytes into a new destination. Each invocation
//! owns a private unique staging directory; it never removes another run's
//! files. Received paths use descriptor-relative no-follow creation, and
//! NOREPLACE publication prevents a concurrent destination from being replaced.
//! Failed or interrupted staging directories are retained for explicit local
//! inspection and cleanup, with their path included in ordinary error reports.

use std::ffi::OsString;
use std::fmt;
use std::fs::File;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use locust_proto::id::BlobHash;
use locust_proto::manifest::{Manifest, ManifestError};
use rustix::fs::{self, AtFlags, Mode};

use crate::{BlobSource, ContributionError, safe_fs};

#[derive(Debug)]
pub enum MaterializeError {
    /// No publication occurred; the invocation's own staging files remain.
    Staging {
        path: PathBuf,
        error: Box<MaterializeError>,
    },
    /// Exact files were published, but syncing their directory failed.
    Published { path: PathBuf, error: io::Error },
    /// The manifest fails the contract's checks. Nothing was written.
    Manifest(ManifestError),
    /// The destination has no final name, such as `/` or `..`.
    InvalidDestination(PathBuf),
    /// Something already exists at the destination.
    DestinationExists(PathBuf),
    /// Creating this manifest path found the name already taken by another
    /// manifest path that this filesystem treats as the same name.
    Collision(String),
    /// The source does not have the object an entry names.
    MissingObject { path: String, content: BlobHash },
    /// The object's length is not the entry's size.
    WrongSize {
        path: String,
        expected: u64,
        actual: u64,
    },
    /// The source failed.
    Source(io::Error),
    /// A filesystem operation failed.
    Io { path: PathBuf, error: io::Error },
}

impl fmt::Display for MaterializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Staging { path, error } => write!(
                f,
                "materialization incomplete; staged files retained at {}: {error}",
                path.display()
            ),
            Self::Published { path, error } => write!(
                f,
                "files published at {}, but directory durability is unconfirmed: {error}",
                path.display()
            ),
            Self::Manifest(error) => write!(f, "invalid manifest: {error}"),
            Self::InvalidDestination(path) => {
                write!(
                    f,
                    "{} cannot be a destination: it has no final name",
                    path.display()
                )
            }
            Self::DestinationExists(path) => write!(f, "{} already exists", path.display()),
            Self::Collision(path) => write!(
                f,
                "{path} collides with another path that this filesystem treats as the same name"
            ),
            Self::MissingObject { path, content } => {
                write!(f, "{path}: object {content} is not available")
            }
            Self::WrongSize {
                path,
                expected,
                actual,
            } => write!(
                f,
                "{path}: object is {actual} bytes, manifest says {expected}"
            ),
            Self::Source(error) => write!(f, "reading an object failed: {error}"),
            Self::Io { path, error } => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for MaterializeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Staging { error, .. } => Some(error),
            Self::Published { error, .. } => Some(error),
            Self::Manifest(error) => Some(error),
            Self::Source(error) | Self::Io { error, .. } => Some(error),
            _ => None,
        }
    }
}

fn io_error(path: &Path, error: io::Error) -> MaterializeError {
    MaterializeError::Io {
        path: path.to_path_buf(),
        error,
    }
}

/// Writes the files `manifest` names into a new `destination` whose parent
/// exists. Concurrent calls use independent stages; at most one can publish.
/// Errors before publication retain only this call's stage, leaving an
/// independently created destination untouched.
pub fn materialize(
    manifest: &Manifest,
    source: &mut dyn BlobSource,
    destination: &Path,
) -> Result<(), MaterializeError> {
    manifest.check().map_err(MaterializeError::Manifest)?;
    destination
        .file_name()
        .ok_or_else(|| MaterializeError::InvalidDestination(destination.to_path_buf()))?;
    let parent_path = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent =
        safe_fs::root(parent_path).map_err(|error| filesystem_error(parent_path, error))?;
    let leaf = destination.file_name().expect("validated final name");
    ensure_absent(&parent, leaf, destination)?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let (name, staging_path) = loop {
        let name = OsString::from(format!(
            ".locust-apply-materialize-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let path = parent_path.join(&name);
        match fs::mkdirat(&parent, &name, Mode::from_raw_mode(0o700)) {
            Ok(()) => break (name, path),
            Err(error) if error == rustix::io::Errno::EXIST => continue,
            Err(error) => return Err(io_error(&path, error.into())),
        }
    };
    let result = (|| {
        let staging = File::from(
            fs::openat(
                &parent,
                &name,
                fs::OFlags::RDONLY
                    | fs::OFlags::DIRECTORY
                    | fs::OFlags::NOFOLLOW
                    | fs::OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|error| io_error(&staging_path, error.into()))?,
        );
        write_files(manifest, source, &staging, &staging_path)?;
        let original = staging
            .metadata()
            .map_err(|error| io_error(&staging_path, error))?;
        let current = fs::statat(&parent, &name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|error| io_error(&staging_path, error.into()))?;
        if original.ino() != current.st_ino || original.dev() != current.st_dev as u64 {
            return Err(io_error(
                &staging_path,
                io::Error::other("staging directory changed before publication"),
            ));
        }
        fs::renameat_with(&parent, &name, &parent, leaf, fs::RenameFlags::NOREPLACE).map_err(
            |error| {
                if error == rustix::io::Errno::EXIST {
                    MaterializeError::DestinationExists(destination.to_path_buf())
                } else {
                    io_error(destination, error.into())
                }
            },
        )?;
        Ok(())
    })();
    result.map_err(|error| MaterializeError::Staging {
        path: staging_path,
        error: Box::new(error),
    })?;
    parent
        .sync_all()
        .map_err(|error| MaterializeError::Published {
            path: destination.to_path_buf(),
            error,
        })
}

fn ensure_absent(
    parent: &File,
    name: &std::ffi::OsStr,
    destination: &Path,
) -> Result<(), MaterializeError> {
    match fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(_) => Err(MaterializeError::DestinationExists(
            destination.to_path_buf(),
        )),
        Err(error) if error == rustix::io::Errno::NOENT => Ok(()),
        Err(error) => Err(io_error(destination, error.into())),
    }
}
fn filesystem_error(path: &Path, error: ContributionError) -> MaterializeError {
    match error {
        ContributionError::Conflict { path, .. } | ContributionError::Unsupported { path, .. } => {
            MaterializeError::Collision(path)
        }
        error => io_error(path, io::Error::other(error)),
    }
}
fn write_files(
    manifest: &Manifest,
    source: &mut dyn BlobSource,
    staging: &File,
    label: &Path,
) -> Result<(), MaterializeError> {
    for entry in &manifest.entries {
        let bytes = source
            .fetch(&entry.content)
            .map_err(MaterializeError::Source)?
            .ok_or_else(|| MaterializeError::MissingObject {
                path: entry.path.clone(),
                content: entry.content,
            })?;
        if bytes.len() as u64 != entry.size {
            return Err(MaterializeError::WrongSize {
                path: entry.path.clone(),
                expected: entry.size,
                actual: bytes.len() as u64,
            });
        }
        let (parent, leaf) = safe_fs::parent(staging, &entry.path, true)
            .map_err(|error| filesystem_error(&label.join(&entry.path), error))?;
        safe_fs::write_new(&parent, &leaf, &bytes, entry.executable).map_err(|error| {
            if matches!(&error, ContributionError::Io(error) if error.kind() == io::ErrorKind::AlreadyExists) { MaterializeError::Collision(entry.path.clone()) }
            else { filesystem_error(&label.join(&entry.path), error) }
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;

    use locust_proto::crypto::content_hash;
    use locust_proto::manifest::Entry;

    use super::*;

    struct Objects(HashMap<BlobHash, Vec<u8>>);

    impl BlobSource for Objects {
        fn fetch(&mut self, hash: &BlobHash) -> io::Result<Option<Vec<u8>>> {
            Ok(self.0.get(hash).cloned())
        }
    }

    #[test]
    fn invalid_destination_names_are_rejected_before_creation() {
        for invalid in ["/", "..", "work/.."] {
            assert!(matches!(
                materialize(
                    &Manifest { entries: vec![] },
                    &mut Objects(HashMap::new()),
                    Path::new(invalid)
                ),
                Err(MaterializeError::InvalidDestination(_))
            ));
        }
    }

    #[test]
    fn a_legacy_leftover_symlink_is_preserved_and_never_followed() {
        let parent = tempfile::tempdir().unwrap();
        let elsewhere = parent.path().join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        fs::write(elsewhere.join("keep.txt"), b"keep").unwrap();
        let destination = parent.path().join("out");
        std::os::unix::fs::symlink(&elsewhere, parent.path().join(".out.locust-partial")).unwrap();

        let bytes = b"hello".to_vec();
        let manifest = Manifest {
            entries: vec![Entry {
                path: "a.txt".into(),
                executable: false,
                size: 5,
                content: content_hash(&bytes),
            }],
        };
        let mut source = Objects(HashMap::from([(content_hash(&bytes), bytes)]));
        materialize(&manifest, &mut source, &destination).unwrap();

        assert!(
            fs::symlink_metadata(parent.path().join(".out.locust-partial"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(elsewhere.join("keep.txt")).unwrap(), b"keep");
        assert!(!elsewhere.join("a.txt").exists());
        assert!(fs::symlink_metadata(&destination).unwrap().is_dir());
        assert_eq!(fs::read(destination.join("a.txt")).unwrap(), b"hello");
    }
}
