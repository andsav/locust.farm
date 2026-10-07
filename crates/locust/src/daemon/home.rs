//! The state directory as a running daemon holds it: created private,
//! locked against a second daemon, with the owner's credential in place and
//! the socket bound. Paths and modes come from `locust_proto::local`.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use locust_proto::api::Credential;
use locust_proto::local::{self, HOME_MODE, SECRET_FILE_MODE};

use crate::failure::Failure;
use crate::secret;

/// The locked state directory of one running daemon. Dropping it removes
/// the socket file and releases the lock, in that order.
pub(crate) struct StateDir {
    home: PathBuf,
    socket: PathBuf,
    /// True once this daemon bound the socket, so only its own is removed.
    bound: bool,
    owner: Credential,
    /// Holds the lock for as long as it is open.
    _lock: File,
}

impl StateDir {
    /// Prepares `home` for a daemon: checks that the socket path fits,
    /// creates the directory with mode 0700 if it is missing, takes the
    /// lock, and creates the owner's credential on first start.
    pub(crate) fn open(home: &Path) -> Result<Self, Failure> {
        let socket = local::socket_path(home)?;
        if !home.exists() {
            secret::create_private_dir(home)
                .and_then(|()| fs::set_permissions(home, fs::Permissions::from_mode(HOME_MODE)))
                .map_err(|error| {
                    Failure::invalid(format!(
                        "state directory {} cannot be created: {error}",
                        home.display()
                    ))
                })?;
        }
        check_private(home)?;
        let lock = lock(home)?;
        let owner_path = local::owner_credential_path(home);
        let owner = secret::read_or_create(&owner_path).map_err(|error| {
            Failure::invalid(format!(
                "owner credential {}: {error}",
                owner_path.display()
            ))
        })?;
        Ok(Self {
            home: home.to_path_buf(),
            socket,
            bound: false,
            owner: Credential(owner),
            _lock: lock,
        })
    }

    pub(crate) fn home(&self) -> &Path {
        &self.home
    }

    pub(crate) fn socket(&self) -> &Path {
        &self.socket
    }

    /// The owner's credential, which the engine recognizes by its digest.
    pub(crate) fn owner(&self) -> Credential {
        self.owner
    }

    /// Binds the socket. A socket file left by a daemon that did not stop
    /// cleanly is removed first: this daemon holds the lock, so no running
    /// daemon owns it.
    pub(crate) fn listen(&mut self) -> Result<UnixListener, Failure> {
        let bind = || -> io::Result<UnixListener> {
            match fs::remove_file(&self.socket) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
                _ => {}
            }
            let listener = UnixListener::bind(&self.socket)?;
            listener.set_nonblocking(true)?;
            Ok(listener)
        };
        let listener = bind().map_err(|error| {
            Failure::unavailable(format!(
                "socket {} cannot be bound: {error}",
                self.socket.display()
            ))
        })?;
        self.bound = true;
        Ok(listener)
    }
}

impl Drop for StateDir {
    fn drop(&mut self) {
        if self.bound {
            let _ = fs::remove_file(&self.socket);
        }
        // The lock is released when `_lock` closes, after this.
    }
}

/// Refuses a state directory that anyone but its owner can enter.
fn check_private(home: &Path) -> Result<(), Failure> {
    let metadata = fs::metadata(home).map_err(|error| {
        Failure::invalid(format!("state directory {}: {error}", home.display()))
    })?;
    if !metadata.is_dir() {
        return Err(Failure::invalid(format!(
            "state directory {} is not a directory",
            home.display()
        )));
    }
    local::owner_only(
        "state directory",
        home,
        metadata.permissions().mode(),
        HOME_MODE,
    )
    .map_err(Failure::invalid)
}

/// Takes the daemon lock, or reports that another daemon runs here.
fn lock(home: &Path) -> Result<File, Failure> {
    let path = local::lock_path(home);
    let failed =
        |error: io::Error| Failure::internal(format!("daemon lock {}: {error}", path.display()));
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(SECRET_FILE_MODE)
        .open(&path)
        .map_err(failed)?;
    match file.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            let mut holder = String::new();
            let _ = file.read_to_string(&mut holder);
            let holder = match holder.trim() {
                "" => String::new(),
                process => format!(" (process {process})"),
            };
            return Err(Failure::unavailable(format!(
                "a daemon is already running on {}{holder}",
                home.display()
            )));
        }
        Err(TryLockError::Error(error)) => return Err(failed(error)),
    }
    // The holder's process number, for the message above and for `doctor`.
    file.set_len(0)
        .and_then(|()| file.seek(SeekFrom::Start(0)))
        .and_then(|_| writeln!(file, "{}", std::process::id()))
        .map_err(failed)?;
    Ok(file)
}

/// Whether a running daemon holds the lock of `home`, and its process
/// number as the lock file records it. Taking the lock is the test, so a
/// lock left by a daemon that died does not count.
pub(crate) fn lock_holder(home: &Path) -> io::Result<Option<String>> {
    let mut file = File::open(local::lock_path(home))?;
    match file.try_lock() {
        Ok(()) => {
            file.unlock()?;
            Ok(None)
        }
        Err(TryLockError::WouldBlock) => {
            let mut holder = String::new();
            file.read_to_string(&mut holder)?;
            Ok(Some(holder.trim().to_string()))
        }
        Err(TryLockError::Error(error)) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use locust_proto::api::ErrorCode;

    use super::*;
    use crate::testdir::short_dir;

    #[test]
    fn a_missing_state_directory_is_created_private_with_the_owner_credential() {
        let dir = short_dir();
        let home = dir.path().join("state");
        let state = StateDir::open(&home).unwrap();
        assert_eq!(secret::mode(&home).unwrap(), 0o700);
        let credential = local::owner_credential_path(&home);
        assert_eq!(fs::read(&credential).unwrap().len(), 32);
        assert_eq!(secret::mode(&credential).unwrap(), 0o600);
        assert_eq!(state.owner().0.as_slice(), fs::read(&credential).unwrap());
        assert_eq!(state.home(), home);
        assert_eq!(state.socket(), home.join("daemon.sock"));
    }

    #[test]
    fn the_owner_credential_survives_a_restart() {
        let dir = short_dir();
        let first = StateDir::open(dir.path()).unwrap().owner();
        let second = StateDir::open(dir.path()).unwrap().owner();
        assert_eq!(first.0, second.0);
    }

    #[test]
    fn a_second_daemon_on_the_same_directory_is_refused_until_the_first_is_gone() {
        let dir = short_dir();
        let first = StateDir::open(dir.path()).unwrap();
        let Err(refused) = StateDir::open(dir.path()) else {
            panic!("the second daemon was not refused");
        };
        assert_eq!(refused.code, ErrorCode::Unavailable);
        assert_eq!(refused.exit_status(), 8);
        assert!(refused.message.contains(&dir.path().display().to_string()));
        assert!(refused.message.contains(&std::process::id().to_string()));
        assert_eq!(
            lock_holder(dir.path()).unwrap(),
            Some(std::process::id().to_string())
        );

        drop(first);
        assert_eq!(lock_holder(dir.path()).unwrap(), None);
        StateDir::open(dir.path()).unwrap();
    }

    #[test]
    fn the_socket_is_bound_over_a_stale_file_and_removed_on_drop() {
        let dir = short_dir();
        let socket = dir.path().join("daemon.sock");
        fs::write(&socket, b"left by a daemon that was killed").unwrap();
        let mut state = StateDir::open(dir.path()).unwrap();
        let listener = state.listen().unwrap();
        assert!(std::os::unix::net::UnixStream::connect(&socket).is_ok());
        drop(listener);
        drop(state);
        assert!(!socket.exists());
        // The lock file stays; only the lock is released.
        assert!(local::lock_path(dir.path()).exists());
    }

    #[test]
    fn a_state_directory_that_was_never_bound_keeps_another_daemons_socket() {
        let dir = short_dir();
        let mut first = StateDir::open(dir.path()).unwrap();
        let _listener = first.listen().unwrap();
        // The refused daemon must not remove the running daemon's socket.
        assert!(StateDir::open(dir.path()).is_err());
        assert!(first.socket().exists());
    }

    #[test]
    fn a_socket_path_that_is_too_long_is_refused_before_anything_is_created() {
        let dir = short_dir();
        let home = dir.path().join("d".repeat(100));
        let Err(refused) = StateDir::open(&home) else {
            panic!("the long path was not refused");
        };
        assert!(refused.message.contains("socket path is"));
        assert!(!home.exists());
    }

    #[test]
    fn a_state_directory_others_can_enter_is_refused() {
        let dir = short_dir();
        let home = dir.path().join("open");
        fs::create_dir(&home).unwrap();
        fs::set_permissions(&home, fs::Permissions::from_mode(0o755)).unwrap();
        let Err(refused) = StateDir::open(&home) else {
            panic!("the open directory was not refused");
        };
        assert!(refused.message.contains("0755"), "{}", refused.message);
        assert!(!local::lock_path(&home).exists());
    }

    #[test]
    fn an_owner_credential_of_the_wrong_size_is_reported_not_replaced() {
        let dir = short_dir();
        let credential = local::owner_credential_path(dir.path());
        fs::write(&credential, b"not a secret").unwrap();
        let Err(refused) = StateDir::open(dir.path()) else {
            panic!("the damaged credential was accepted");
        };
        assert!(refused.message.contains("owner.credential"));
        assert_eq!(fs::read(&credential).unwrap(), b"not a secret");
    }
}
