//! Materialize: a manifest becomes files in a new directory.
//!
//! Files are written into a staging directory beside the destination, named
//! `.<destination name>.locust-partial`, which is renamed into place only
//! once every file is written and its size checked. A process that stops
//! part way leaves only the staging directory, which the next run for the
//! same destination removes; the destination itself never appears
//! incomplete. One materialization per destination runs at a time.
//!
//! Every directory and file is created new inside the fresh staging
//! directory, so nothing is followed: a name the filesystem already holds
//! under another spelling (case folding, Unicode normalization) is reported
//! as a collision. Files are not synced to disk one by one; the guarantee
//! covers an interrupted process, not a power loss, matching what Git does
//! for a checkout.

use std::collections::HashSet;
use std::ffi::OsString;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use locust_proto::id::BlobHash;
use locust_proto::manifest::{Manifest, ManifestError};

use crate::BlobSource;

#[derive(Debug)]
pub enum MaterializeError {
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

/// Writes the files `manifest` names, read from `source`, into
/// `destination`, which must not exist yet; its parent must.
///
/// On any error the destination does not exist afterwards.
pub fn materialize(
    manifest: &Manifest,
    source: &mut dyn BlobSource,
    destination: &Path,
) -> Result<(), MaterializeError> {
    manifest.check().map_err(MaterializeError::Manifest)?;
    let staging = staging_path(destination)?;
    ensure_absent(destination)?;
    remove_leftover(&staging)?;
    fs::create_dir(&staging).map_err(|error| io_error(&staging, error))?;

    let result = write_files(manifest, source, &staging).and_then(|()| {
        ensure_absent(destination)?;
        fs::rename(&staging, destination).map_err(|error| io_error(destination, error))
    });
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn staging_path(destination: &Path) -> Result<PathBuf, MaterializeError> {
    let name = destination
        .file_name()
        .ok_or_else(|| MaterializeError::InvalidDestination(destination.to_path_buf()))?;
    let mut staging = OsString::from(".");
    staging.push(name);
    staging.push(".locust-partial");
    Ok(destination.with_file_name(staging))
}

fn ensure_absent(destination: &Path) -> Result<(), MaterializeError> {
    match fs::symlink_metadata(destination) {
        Ok(_) => Err(MaterializeError::DestinationExists(
            destination.to_path_buf(),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(destination, error)),
    }
}

/// Removes what an interrupted run left at the staging path. A symlink there
/// is removed, never followed.
fn remove_leftover(staging: &Path) -> Result<(), MaterializeError> {
    let removed = match fs::symlink_metadata(staging) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(staging),
        Ok(_) => fs::remove_file(staging),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    };
    removed.map_err(|error| io_error(staging, error))
}

fn write_files(
    manifest: &Manifest,
    source: &mut dyn BlobSource,
    staging: &Path,
) -> Result<(), MaterializeError> {
    let mut directories = HashSet::new();
    for entry in &manifest.entries {
        for (end, _) in entry.path.match_indices('/') {
            let directory = &entry.path[..end];
            if directories.insert(directory) {
                create(directory, staging, |path| fs::create_dir(path))?;
            }
        }

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

        let mode = if entry.executable { 0o777 } else { 0o666 };
        let mut file = create(&entry.path, staging, |path| {
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(mode)
                .open(path)
        })?;
        file.write_all(&bytes)
            .map_err(|error| io_error(&staging.join(&entry.path), error))?;
    }
    Ok(())
}

/// Creates the manifest path `relative` inside `staging` with `make`, which
/// must fail if the name exists. An existing name is a collision, because
/// this run has not created it under this spelling.
fn create<T>(
    relative: &str,
    staging: &Path,
    make: impl FnOnce(&Path) -> io::Result<T>,
) -> Result<T, MaterializeError> {
    let path = staging.join(relative);
    make(&path).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            MaterializeError::Collision(relative.to_owned())
        } else {
            MaterializeError::Io { path, error }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

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
    fn the_staging_directory_sits_beside_the_destination() {
        assert_eq!(
            staging_path(Path::new("/work/out")).unwrap(),
            Path::new("/work/.out.locust-partial")
        );
        assert_eq!(
            staging_path(Path::new("out")).unwrap(),
            Path::new(".out.locust-partial")
        );
        for invalid in ["/", "..", "work/.."] {
            assert!(matches!(
                staging_path(Path::new(invalid)),
                Err(MaterializeError::InvalidDestination(_))
            ));
        }
    }

    #[test]
    fn a_leftover_symlink_at_the_staging_path_is_removed_not_followed() {
        let parent = tempfile::tempdir().unwrap();
        let elsewhere = parent.path().join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        fs::write(elsewhere.join("keep.txt"), b"keep").unwrap();
        let destination = parent.path().join("out");
        std::os::unix::fs::symlink(&elsewhere, staging_path(&destination).unwrap()).unwrap();

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

        assert_eq!(fs::read(elsewhere.join("keep.txt")).unwrap(), b"keep");
        assert!(!elsewhere.join("a.txt").exists());
        assert!(fs::symlink_metadata(&destination).unwrap().is_dir());
        assert_eq!(fs::read(destination.join("a.txt")).unwrap(), b"hello");
    }
}
