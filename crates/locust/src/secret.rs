//! Credential and session secret files: exactly 32 secret bytes, mode 0600.
//!
//! Whoever generates a secret writes its file before telling the daemon
//! about it, so a secret the daemon knows is never lost. [`read_or_create`] makes
//! the file appear under its name whole or not at all, and never replaces
//! one that is already there.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

use locust_proto::local::{self, HOME_MODE, SECRET_FILE_BYTES, SECRET_FILE_MODE};

/// A fresh secret from the operating system's random source.
pub fn generate() -> io::Result<[u8; SECRET_FILE_BYTES]> {
    let mut secret = [0u8; SECRET_FILE_BYTES];
    getrandom::fill(&mut secret).map_err(io::Error::other)?;
    Ok(secret)
}

/// The secret a file holds. A file that is not exactly the secret is refused
/// as invalid data.
pub fn read(path: &Path) -> io::Result<[u8; SECRET_FILE_BYTES]> {
    let contents = fs::read(path)?;
    local::secret_from_file(&contents)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// The permission bits of a file or directory.
#[cfg(test)]
pub fn mode(path: &Path) -> io::Result<u32> {
    Ok(fs::metadata(path)?.permissions().mode() & 0o7777)
}

/// Creates a directory only its owner may enter, and its missing parents.
pub fn create_private_dir(path: &Path) -> io::Result<()> {
    create_private_dir_with_sync(path, &mut sync_directory)
}

fn create_private_dir_with_sync(
    path: &Path,
    sync: &mut impl FnMut(&Path) -> io::Result<()>,
) -> io::Result<()> {
    if path.as_os_str().is_empty() {
        return Ok(());
    }
    if path.is_dir() {
        // A previous attempt may have created this ancestor but failed its
        // parent sync. Repair that entry before acknowledging the retry.
        return if path.parent().is_some() {
            sync(parent(path))
        } else {
            Ok(()) // The filesystem root has no parent entry to publish.
        };
    }
    let parent = parent(path);
    create_private_dir_with_sync(parent, sync)?;
    match DirBuilder::new().mode(HOME_MODE).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && path.is_dir() => {}
        Err(error) => return Err(error),
    }
    sync(parent)
}

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

/// Publishes a complete private secret without replacing an existing file.
fn create_with_sync(
    path: &Path,
    secret: &[u8; SECRET_FILE_BYTES],
    sync: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<bool> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path names no file"))?;
    let mut staging = name.to_os_string();
    staging.push(format!(".{}.new", std::process::id()));
    let staging = path.with_file_name(staging);

    // A leftover from an interrupted run of a process with the same number.
    let _ = fs::remove_file(&staging);
    let written = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(SECRET_FILE_MODE)
        .open(&staging)
        .and_then(|mut file| {
            file.set_permissions(fs::Permissions::from_mode(SECRET_FILE_MODE))?;
            file.write_all(secret)?;
            file.sync_all()
        });
    // A hard link fails when the name is taken, where a rename would replace.
    let linked = written.and_then(|()| fs::hard_link(&staging, path));
    let _ = fs::remove_file(&staging);
    let created = match linked {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
        Err(error) => return Err(error),
    };
    // File data was synced before linking. Persist the final directory entry
    // (and staging cleanup) before the daemon can learn this credential.
    sync(parent(path))?;
    Ok(created)
}

/// The secret in the file at `path`, generating and storing one first when
/// the file does not exist. Two processes that race end up with the same
/// secret: the one whose file appeared first.
pub fn read_or_create(path: &Path) -> io::Result<[u8; SECRET_FILE_BYTES]> {
    read_or_create_with_sync(path, sync_directory)
}

fn read_or_create_with_sync(
    path: &Path,
    mut sync: impl FnMut(&Path) -> io::Result<()>,
) -> io::Result<[u8; SECRET_FILE_BYTES]> {
    match read(path) {
        Ok(secret) => {
            // A failed publication sync leaves the complete file visible.
            // Its visibility alone does not make a retry durable.
            sync(parent(path))?;
            Ok(secret)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let secret = generate()?;
            if create_with_sync(path, &secret, &mut sync)? {
                Ok(secret)
            } else {
                read(path)
            }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create(path: &Path, secret: &[u8; SECRET_FILE_BYTES]) -> io::Result<bool> {
        create_with_sync(path, secret, sync_directory)
    }

    #[test]
    fn a_created_secret_file_is_exactly_the_secret_and_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("owner.credential");
        assert!(create(&path, &[7; 32]).unwrap());
        assert_eq!(fs::read(&path).unwrap(), [7; 32]);
        assert_eq!(mode(&path).unwrap(), 0o600);
        assert_eq!(read(&path).unwrap(), [7; 32]);
        // Nothing but the file itself is left behind.
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn an_existing_secret_file_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("worker.credential");
        assert!(create(&path, &[1; 32]).unwrap());
        assert!(!create(&path, &[2; 32]).unwrap());
        assert_eq!(read(&path).unwrap(), [1; 32]);
        assert_eq!(read_or_create(&path).unwrap(), [1; 32]);
    }

    #[test]
    fn a_missing_secret_is_generated_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.secret");
        let first = read_or_create(&path).unwrap();
        assert_ne!(first, [0; 32]);
        assert_eq!(read_or_create(&path).unwrap(), first);
        assert_eq!(mode(&path).unwrap(), 0o600);
    }

    #[test]
    fn a_file_that_is_not_exactly_a_secret_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("short.credential");
        fs::write(&path, [7; 31]).unwrap();
        assert_eq!(read(&path).unwrap_err().kind(), io::ErrorKind::InvalidData);
        // It is not silently replaced either.
        assert_eq!(
            read_or_create(&path).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            read(&dir.path().join("absent")).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn a_private_directory_is_created_with_its_parents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state/agents");
        create_private_dir(&path).unwrap();
        assert_eq!(mode(&path).unwrap(), 0o700);
        // Creating it again is not an error.
        create_private_dir(&path).unwrap();
    }

    #[test]
    fn directory_sync_failure_is_not_a_durable_secret_acknowledgement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent.credential");
        let error = create_with_sync(&path, &[7; 32], |parent| {
            assert_eq!(parent, dir.path());
            assert_eq!(read(&path).unwrap(), [7; 32]);
            assert_eq!(fs::read_dir(parent).unwrap().count(), 1);
            Err(io::Error::other("injected directory sync failure"))
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "injected directory sync failure");
        // Retrying syncs the surviving entry without replacing its secret.
        assert!(
            !create_with_sync(&path, &[8; 32], |parent| {
                assert_eq!(parent, dir.path());
                sync_directory(parent)
            })
            .unwrap()
        );
        assert_eq!(read(&path).unwrap(), [7; 32]);
    }

    #[test]
    fn read_or_create_repairs_a_failed_publication_before_returning_the_secret() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("retry.credential");
        let error = read_or_create_with_sync(&path, |_| {
            Err(io::Error::other("injected publication sync failure"))
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "injected publication sync failure");
        let original = read(&path).unwrap();
        let mut syncs = 0;
        let retried = read_or_create_with_sync(&path, |parent| {
            assert_eq!(parent, dir.path());
            syncs += 1;
            sync_directory(parent)
        })
        .unwrap();
        assert_eq!(retried, original);
        assert_eq!(syncs, 1);
        assert_eq!(mode(&path).unwrap(), SECRET_FILE_MODE);
        // A failed repair is still an error even though the secret is readable.
        assert!(
            read_or_create_with_sync(&path, |_| Err(io::Error::other("repair failed"))).is_err()
        );
        assert_eq!(read(&path).unwrap(), original);
    }

    #[test]
    fn directory_retry_repairs_the_first_unsynced_ancestor_before_descendants() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("state");
        let leaf = parent.join("agents");
        create_private_dir_with_sync(&leaf, &mut |directory| {
            if directory == root.path() {
                Err(io::Error::other("injected ancestor sync failure"))
            } else {
                sync_directory(directory)
            }
        })
        .unwrap_err();
        assert!(parent.is_dir());
        assert!(!leaf.exists());
        let mut synced = Vec::new();
        create_private_dir_with_sync(&leaf, &mut |directory| {
            synced.push(directory.to_path_buf());
            sync_directory(directory)
        })
        .unwrap();
        assert_eq!(synced, [root.path(), parent.as_path()]);
        assert!(leaf.is_dir());
    }
}
