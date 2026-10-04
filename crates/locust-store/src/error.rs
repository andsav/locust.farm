//! What opening a state directory can fail with, and how SQLite and file
//! system errors map onto [`StoreError`].

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use locust_proto::store::StoreError;
use rusqlite::ErrorCode;

/// Why [`crate::SqliteStore::open`] could not open a state directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenError {
    /// Another open store holds the directory: a second daemon on the same
    /// state directory, or a second handle in this process.
    InUse(PathBuf),
    /// The database was written by a newer Locust. This binary does not know
    /// that schema and does not guess.
    NewerSchema { found: i64, known: i64 },
    /// The directory or database could not be opened, migrated or checked.
    Store(StoreError),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InUse(dir) => write!(
                f,
                "state directory {} is in use by another Locust store; is another daemon running on it?",
                dir.display()
            ),
            Self::NewerSchema { found, known } => write!(
                f,
                "the database has schema version {found}, newer than version {known} that this \
                 binary knows; run the Locust release that wrote it or a newer one"
            ),
            Self::Store(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for OpenError {}

impl From<StoreError> for OpenError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

/// An SQLite error as a store error: a damaged or foreign database file, or
/// a value of the wrong type in a row, is `Corrupted`; anything else failed.
pub(crate) fn sql(error: rusqlite::Error) -> StoreError {
    let damaged = matches!(
        error.sqlite_error_code(),
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase)
    ) || matches!(
        error,
        rusqlite::Error::FromSqlConversionFailure(..)
            | rusqlite::Error::InvalidColumnType(..)
            | rusqlite::Error::IntegralValueOutOfRange(..)
    );
    if damaged {
        StoreError::Corrupted(error.to_string())
    } else {
        StoreError::Failed(format!("sqlite: {error}"))
    }
}

/// A file system error while doing `action` on `path`.
pub(crate) fn file(action: &str, path: &Path, error: io::Error) -> StoreError {
    StoreError::Failed(format!("cannot {action} {}: {error}", path.display()))
}

pub(crate) fn corrupted(detail: impl fmt::Display) -> StoreError {
    StoreError::Corrupted(detail.to_string())
}
