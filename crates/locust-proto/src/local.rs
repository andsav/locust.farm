//! Local conventions shared by the daemon, CLI, bridge, hooks and installer:
//! where the state directory is, what lives in it and which environment
//! variables select a credential and a session.
//!
//! This is the one definition of those names. Everything here is a constant
//! or a pure function: the caller reads the environment and the file system
//! and passes the values in, so every process resolves the same paths from
//! the same inputs and the rules can be tested without touching either.
//!
//! Layout of the state directory, `$LOCUST_HOME` or `~/.locust`, mode 0700:
//!
//! | Path | Holds |
//! |---|---|
//! | `daemon.sock` | The daemon's Unix socket |
//! | `daemon.lock` | Lock held by the one running daemon |
//! | `locust.db` | The store's database |
//! | `blobs/` | Content objects kept as files |
//! | `logs/` | Daemon and adapter logs |
//! | `owner.credential` | The owner's credential |
//! | `agents/<name>.credential` | One enrolled principal's credential |
//! | `sessions/<instance>.secret` | One execution session's secret |
//!
//! Beside the state directory, never inside it, is the marks directory
//! ([`marks_dir`]): the state directory's path with [`MARKS_SUFFIX`] added,
//! `~/.locust.marks` by default. It holds the store's marks, so a copy of the
//! state directory does not carry them.
//!
//! A credential or secret file holds exactly the 32 secret bytes, nothing
//! else, and has mode 0600. Whoever generates a secret writes its file before
//! telling the daemon about it, so a secret the daemon knows is never lost.

use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::api::is_agent_name;
use crate::id::InstanceId;

/// Environment variable that names the state directory. Must be an absolute
/// path. Unset or empty selects `~/.locust`.
pub const HOME_ENV: &str = "LOCUST_HOME";

/// Environment variable that names the credential file a client presents.
/// There is no default: a client without it has no credential, and never
/// falls back to the owner's.
pub const CREDENTIAL_ENV: &str = "LOCUST_CREDENTIAL";

/// Environment variable that names the session secret file a client
/// presents. Unset or empty means the client is not an execution session.
pub const SESSION_ENV: &str = "LOCUST_SESSION";

/// Name of the default state directory inside the user's home directory.
pub const DEFAULT_HOME: &str = ".locust";

/// Mode of the state directory: only its owner may enter it.
pub const HOME_MODE: u32 = 0o700;

/// Mode of every credential and secret file.
pub const SECRET_FILE_MODE: u32 = 0o600;

/// Exact size of a credential or secret file, in bytes.
pub const SECRET_FILE_BYTES: usize = 32;

/// The daemon's Unix socket.
pub const SOCKET_FILE: &str = "daemon.sock";

/// Lock held by the one daemon that runs on a state directory.
pub const LOCK_FILE: &str = "daemon.lock";

/// The store's database.
pub const DATABASE_FILE: &str = "locust.db";

/// Directory of content objects kept as files.
pub const BLOBS_DIR: &str = "blobs";

/// Directory of daemon and adapter logs.
pub const LOGS_DIR: &str = "logs";

/// Added to the last part of the state directory's path to name the marks
/// directory beside it.
pub const MARKS_SUFFIX: &str = ".marks";

/// The owner's credential.
pub const OWNER_CREDENTIAL_FILE: &str = "owner.credential";

/// Directory of enrolled principals' credentials, `<name>.credential` each.
pub const AGENTS_DIR: &str = "agents";

/// Directory of session secrets, `<instance>.secret` each.
pub const SESSIONS_DIR: &str = "sessions";

/// Longest socket path, in bytes. macOS limits a Unix socket path to 104
/// bytes including its terminator, the tightest of the supported platforms;
/// applying it everywhere keeps one state directory valid on all of them.
pub const MAX_SOCKET_PATH_BYTES: usize = 103;

/// Why a local convention could not be applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalError {
    /// Neither `LOCUST_HOME` nor the user's home directory is known.
    NoHome,
    /// The named environment variable holds a relative path. Processes with
    /// different working directories would resolve it differently.
    NotAbsolute(&'static str),
    /// The socket path would be this many bytes, more than
    /// [`MAX_SOCKET_PATH_BYTES`]. Choose a shorter `LOCUST_HOME`.
    SocketPathTooLong(usize),
    /// The name fails [`is_agent_name`], so it cannot name a credential file.
    BadAgentName,
    /// `LOCUST_CREDENTIAL` is not set.
    NoCredential,
    /// A credential or secret file is not exactly [`SECRET_FILE_BYTES`] long.
    BadSecretFile,
}

impl fmt::Display for LocalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoHome => write!(f, "{HOME_ENV} is not set and no home directory is known"),
            Self::NotAbsolute(variable) => write!(f, "{variable} must be an absolute path"),
            Self::SocketPathTooLong(bytes) => write!(
                f,
                "socket path is {bytes} bytes, more than {MAX_SOCKET_PATH_BYTES}; set {HOME_ENV} to a shorter directory"
            ),
            Self::BadAgentName => {
                f.write_str("a principal's name is 1 to 32 characters from a-z, 0-9 and -")
            }
            Self::NoCredential => write!(f, "{CREDENTIAL_ENV} is not set"),
            Self::BadSecretFile => write!(
                f,
                "a credential or secret file must hold exactly {SECRET_FILE_BYTES} bytes"
            ),
        }
    }
}

impl std::error::Error for LocalError {}

/// The value of an environment variable, with unset and empty meaning the
/// same thing.
fn set(value: Option<&OsStr>) -> Option<&OsStr> {
    value.filter(|value| !value.is_empty())
}

fn absolute(value: &OsStr, variable: &'static str) -> Result<PathBuf, LocalError> {
    let path = Path::new(value);
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Err(LocalError::NotAbsolute(variable))
    }
}

/// The state directory: `locust_home` (the value of [`HOME_ENV`]) when set,
/// otherwise [`DEFAULT_HOME`] inside `user_home` (the value of `HOME`).
/// Either must be absolute.
pub fn home_dir(
    locust_home: Option<&OsStr>,
    user_home: Option<&OsStr>,
) -> Result<PathBuf, LocalError> {
    if let Some(home) = set(locust_home) {
        return absolute(home, HOME_ENV);
    }
    let user_home = set(user_home).ok_or(LocalError::NoHome)?;
    Ok(absolute(user_home, "HOME")?.join(DEFAULT_HOME))
}

/// The daemon's socket inside `home`. Refused when the path is too long to
/// bind or connect to; see [`MAX_SOCKET_PATH_BYTES`].
pub fn socket_path(home: &Path) -> Result<PathBuf, LocalError> {
    let path = home.join(SOCKET_FILE);
    let bytes = path.as_os_str().as_encoded_bytes().len();
    if bytes > MAX_SOCKET_PATH_BYTES {
        return Err(LocalError::SocketPathTooLong(bytes));
    }
    Ok(path)
}

/// The lock file of the daemon that runs on `home`.
pub fn lock_path(home: &Path) -> PathBuf {
    home.join(LOCK_FILE)
}

/// The store's database inside `home`.
pub fn database_path(home: &Path) -> PathBuf {
    home.join(DATABASE_FILE)
}

/// The directory of content objects inside `home`.
pub fn blobs_dir(home: &Path) -> PathBuf {
    home.join(BLOBS_DIR)
}

/// The directory of logs inside `home`.
pub fn logs_dir(home: &Path) -> PathBuf {
    home.join(LOGS_DIR)
}

/// The marks directory of `home`: beside it, never inside it, so a copy of
/// `home` does not carry it. `home`'s path with [`MARKS_SUFFIX`] added to its
/// last part. A trailing `/` or `/.` is not a part: it would put the suffix
/// inside `home`.
pub fn marks_dir(home: &Path) -> PathBuf {
    match (home.parent(), home.file_name()) {
        (Some(parent), Some(name)) => {
            let mut name = name.to_owned();
            name.push(MARKS_SUFFIX);
            parent.join(name)
        }
        _ => {
            let mut path = home.as_os_str().to_owned();
            path.push(MARKS_SUFFIX);
            PathBuf::from(path)
        }
    }
}

/// The owner's credential file inside `home`. A client uses it only when the
/// person at the keyboard asks to act as the owner.
pub fn owner_credential_path(home: &Path) -> PathBuf {
    home.join(OWNER_CREDENTIAL_FILE)
}

/// The credential file of the enrolled principal `name` inside `home`. The
/// name becomes a file name, so one that fails [`is_agent_name`] is refused.
pub fn agent_credential_path(home: &Path, name: &str) -> Result<PathBuf, LocalError> {
    if !is_agent_name(name) {
        return Err(LocalError::BadAgentName);
    }
    Ok(home.join(AGENTS_DIR).join(format!("{name}.credential")))
}

/// The secret file of the session with handle `instance` inside `home`.
pub fn session_secret_path(home: &Path, instance: &InstanceId) -> PathBuf {
    home.join(SESSIONS_DIR).join(format!("{instance}.secret"))
}

/// The credential file a client presents: the value of [`CREDENTIAL_ENV`].
/// Unset is an error rather than a default, so losing the variable can never
/// widen what a client may do.
pub fn credential_path(locust_credential: Option<&OsStr>) -> Result<PathBuf, LocalError> {
    let value = set(locust_credential).ok_or(LocalError::NoCredential)?;
    absolute(value, CREDENTIAL_ENV)
}

/// The session secret file a client presents: the value of [`SESSION_ENV`],
/// or `None` when the client is not an execution session.
pub fn session_path(locust_session: Option<&OsStr>) -> Result<Option<PathBuf>, LocalError> {
    set(locust_session)
        .map(|value| absolute(value, SESSION_ENV))
        .transpose()
}

/// The secret a credential or session secret file holds. The file is exactly
/// the 32 secret bytes; anything else is refused rather than trimmed or
/// padded.
pub fn secret_from_file(contents: &[u8]) -> Result<[u8; SECRET_FILE_BYTES], LocalError> {
    contents.try_into().map_err(|_| LocalError::BadSecretFile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::SessionSecret;

    fn os(text: &str) -> Option<&OsStr> {
        Some(OsStr::new(text))
    }

    #[test]
    fn the_state_directory_defaults_to_dot_locust_in_the_home_directory() {
        assert_eq!(
            home_dir(None, os("/Users/ada")),
            Ok(PathBuf::from("/Users/ada/.locust"))
        );
        // An empty variable is an unset variable.
        assert_eq!(
            home_dir(os(""), os("/Users/ada")),
            Ok(PathBuf::from("/Users/ada/.locust"))
        );
    }

    #[test]
    fn locust_home_overrides_the_default() {
        assert_eq!(
            home_dir(os("/srv/locust"), os("/Users/ada")),
            Ok(PathBuf::from("/srv/locust"))
        );
        // It does not need a home directory to exist.
        assert_eq!(
            home_dir(os("/srv/locust"), None),
            Ok(PathBuf::from("/srv/locust"))
        );
    }

    #[test]
    fn a_state_directory_that_depends_on_the_working_directory_is_refused() {
        assert_eq!(
            home_dir(os("locust-state"), os("/Users/ada")),
            Err(LocalError::NotAbsolute(HOME_ENV))
        );
        assert_eq!(
            home_dir(os("./state"), None),
            Err(LocalError::NotAbsolute(HOME_ENV))
        );
        assert_eq!(
            home_dir(None, os("ada")),
            Err(LocalError::NotAbsolute("HOME"))
        );
        assert_eq!(home_dir(None, None), Err(LocalError::NoHome));
        assert_eq!(home_dir(os(""), os("")), Err(LocalError::NoHome));
    }

    #[test]
    fn every_path_is_inside_the_state_directory() {
        let home = Path::new("/Users/ada/.locust");
        assert_eq!(
            socket_path(home),
            Ok(PathBuf::from("/Users/ada/.locust/daemon.sock"))
        );
        assert_eq!(
            lock_path(home),
            PathBuf::from("/Users/ada/.locust/daemon.lock")
        );
        assert_eq!(
            database_path(home),
            PathBuf::from("/Users/ada/.locust/locust.db")
        );
        assert_eq!(blobs_dir(home), PathBuf::from("/Users/ada/.locust/blobs"));
        assert_eq!(logs_dir(home), PathBuf::from("/Users/ada/.locust/logs"));
        assert_eq!(
            owner_credential_path(home),
            PathBuf::from("/Users/ada/.locust/owner.credential")
        );
        assert_eq!(
            agent_credential_path(home, "worker-2"),
            Ok(PathBuf::from(
                "/Users/ada/.locust/agents/worker-2.credential"
            ))
        );
    }

    #[test]
    fn the_marks_directory_is_beside_the_home_and_never_inside_it() {
        let default = home_dir(None, os("/Users/ada")).unwrap();
        assert_eq!(
            marks_dir(&default),
            PathBuf::from("/Users/ada/.locust.marks")
        );
        for home in ["/srv/locust", "/srv/locust/", "/srv/locust/."] {
            let marks = marks_dir(Path::new(home));
            assert_eq!(marks, PathBuf::from("/srv/locust.marks"), "{home}");
            assert!(!marks.starts_with(home), "{home}");
            assert_eq!(marks.parent(), Path::new(home).parent(), "{home}");
        }
    }

    #[test]
    fn a_session_secret_file_is_named_by_the_public_handle() {
        let secret = SessionSecret([0x6b; 32]);
        let path = session_secret_path(Path::new("/h"), &secret.instance());
        assert_eq!(
            path,
            PathBuf::from(format!("/h/sessions/{}.secret", secret.instance()))
        );
        let name = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(name.len(), 2 * InstanceId::LEN + ".secret".len());
        assert!(!name.contains("6b6b"));
    }

    #[test]
    fn a_socket_path_longer_than_macos_allows_is_refused() {
        // "/" + directory + "/daemon.sock" is exactly the limit.
        let fitting = "d".repeat(MAX_SOCKET_PATH_BYTES - SOCKET_FILE.len() - 2);
        let home = PathBuf::from(format!("/{fitting}"));
        let path = socket_path(&home).unwrap();
        assert_eq!(path.as_os_str().len(), MAX_SOCKET_PATH_BYTES);

        let home = PathBuf::from(format!("/{fitting}d"));
        assert_eq!(
            socket_path(&home),
            Err(LocalError::SocketPathTooLong(MAX_SOCKET_PATH_BYTES + 1))
        );
        // Only the socket has the limit; other paths in the same directory work.
        assert_eq!(lock_path(&home), home.join("daemon.lock"));

        // The limit counts bytes, not characters.
        let home = PathBuf::from(format!("/{}", "é".repeat(50)));
        assert_eq!(
            socket_path(&home),
            Err(LocalError::SocketPathTooLong(
                1 + 100 + 1 + SOCKET_FILE.len()
            ))
        );
    }

    #[test]
    fn a_principal_name_that_could_leave_the_agents_directory_is_refused() {
        let home = Path::new("/h");
        for name in [
            "",
            "..",
            "../owner",
            "a/b",
            "Worker",
            "worker.1",
            "wörker",
            "worker ",
            &"a".repeat(33),
        ] {
            assert_eq!(
                agent_credential_path(home, name),
                Err(LocalError::BadAgentName),
                "{name:?}"
            );
        }
        assert!(agent_credential_path(home, &"a".repeat(32)).is_ok());
        assert!(agent_credential_path(home, "0-a").is_ok());
    }

    #[test]
    fn a_missing_credential_variable_is_an_error_not_the_owner() {
        assert_eq!(credential_path(None), Err(LocalError::NoCredential));
        assert_eq!(credential_path(os("")), Err(LocalError::NoCredential));
        assert_eq!(
            credential_path(os("agents/worker.credential")),
            Err(LocalError::NotAbsolute(CREDENTIAL_ENV))
        );
        assert_eq!(
            credential_path(os("/h/agents/worker.credential")),
            Ok(PathBuf::from("/h/agents/worker.credential"))
        );
    }

    #[test]
    fn a_missing_session_variable_means_no_session() {
        assert_eq!(session_path(None), Ok(None));
        assert_eq!(session_path(os("")), Ok(None));
        assert_eq!(
            session_path(os("sessions/x.secret")),
            Err(LocalError::NotAbsolute(SESSION_ENV))
        );
        assert_eq!(
            session_path(os("/h/sessions/x.secret")),
            Ok(Some(PathBuf::from("/h/sessions/x.secret")))
        );
    }

    #[test]
    fn a_secret_file_is_exactly_the_secret() {
        assert_eq!(secret_from_file(&[7; 32]), Ok([7; 32]));
        assert_eq!(secret_from_file(&[7; 31]), Err(LocalError::BadSecretFile));
        assert_eq!(secret_from_file(&[7; 33]), Err(LocalError::BadSecretFile));
        assert_eq!(secret_from_file(b""), Err(LocalError::BadSecretFile));
        // A hex or newline-terminated rendering is not the secret.
        let hex = format!("{}\n", "07".repeat(32));
        assert_eq!(
            secret_from_file(hex.as_bytes()),
            Err(LocalError::BadSecretFile)
        );
    }

    #[test]
    fn the_published_names_and_modes_are_stable() {
        assert_eq!(
            [HOME_ENV, CREDENTIAL_ENV, SESSION_ENV],
            ["LOCUST_HOME", "LOCUST_CREDENTIAL", "LOCUST_SESSION"]
        );
        assert_eq!(DEFAULT_HOME, ".locust");
        assert_eq!(MARKS_SUFFIX, ".marks");
        assert_eq!((HOME_MODE, SECRET_FILE_MODE), (0o700, 0o600));
        assert_eq!(MAX_SOCKET_PATH_BYTES + 1, 104);
    }

    #[test]
    fn errors_say_what_to_change() {
        assert_eq!(
            LocalError::SocketPathTooLong(120).to_string(),
            "socket path is 120 bytes, more than 103; set LOCUST_HOME to a shorter directory"
        );
        assert_eq!(
            LocalError::NotAbsolute(SESSION_ENV).to_string(),
            "LOCUST_SESSION must be an absolute path"
        );
        assert_eq!(
            LocalError::NoCredential.to_string(),
            "LOCUST_CREDENTIAL is not set"
        );
    }
}
