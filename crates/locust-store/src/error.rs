//! What opening a state directory can fail with, and how SQLite and file
//! system errors map onto [`StoreError`].

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use locust_proto::store::StoreError;
use rusqlite::ErrorCode;

/// Why [`crate::SqliteStore::open`] could not open a state directory. Each
/// says only what happened; the caller says what to do next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenError {
    /// Another open store holds the directory's database: another program,
    /// or a second handle in this process.
    InUse(PathBuf),
    /// The database is not this binary's current format. No conversion is performed.
    UnsupportedSchema { found: i64, known: i64 },
    /// Persisted signed events use another protocol. Their bytes cannot be
    /// migrated in place without invalidating event identifiers/signatures.
    UnsupportedProtocolVersion { found: u8, known: u8 },
    /// The marks directory, or the marks file in it, lets someone other than
    /// its owner in: its permission bits are `mode` and must be `wanted`.
    MarksNotPrivate {
        path: PathBuf,
        mode: u32,
        wanted: u32,
    },
    /// The directory or database could not be opened, initialized or checked.
    Store(StoreError),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InUse(dir) => write!(
                f,
                "another program has the database in {} open",
                dir.display()
            ),
            Self::UnsupportedSchema { found, known } => write!(
                f,
                "the database has unsupported schema version {found}; this binary supports only version {known}"
            ),
            Self::UnsupportedProtocolVersion { found, known } => write!(
                f,
                "the state directory contains event protocol version {found}, but this binary supports version {known}"
            ),
            Self::MarksNotPrivate { path, mode, wanted } => write!(
                f,
                "{} has mode {mode:04o}; it must be {wanted:04o}",
                path.display()
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
