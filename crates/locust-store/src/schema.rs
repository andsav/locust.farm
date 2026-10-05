//! One current database format, initialized directly without migrations.
//! An empty unmarked database is new. Every other format is refused.
use crate::error::{OpenError, sql};
use rusqlite::Connection;

pub(crate) const VERSION: i64 = 6;

/// Every table is `STRICT`, so a value of the wrong type is refused when it
/// is written rather than misread later.
///
/// - `goals`: one row per goal with an event; `last_position` is the position
///   of its newest event, so the next one is known without searching the log.
/// - `events`: the log, append only. `id` answers point lookups and
///   `has_event` from its index alone; `events_by_position` serves `log`
///   (and keeps positions unique per goal); `events_by_author` serves
///   `author_log` in (seq, id) order. The header is the last column, so
///   reading the indexed columns never touches its overflow pages.
/// - `blobs`: held content objects. An object is a file under `blobs/`
///   exactly when it is longer than `INLINE_MAX_BYTES` (262144), and then
///   `bytes` is NULL; the CHECK keeps that rule and the stored length honest.
///   Changing the limit requires a distinct current schema; existing objects are not converted.
/// - `local`: non-replicated records, ordered by key bytes within a space.
pub(crate) const CURRENT: &str = "
CREATE TABLE goals (
    goal BLOB NOT NULL PRIMARY KEY,
    last_position INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

CREATE TABLE events (
    id BLOB NOT NULL UNIQUE,
    goal BLOB NOT NULL,
    position INTEGER NOT NULL,
    author BLOB NOT NULL,
    seq INTEGER NOT NULL,
    signature BLOB NOT NULL,
    header BLOB NOT NULL
) STRICT;
CREATE UNIQUE INDEX events_by_position ON events (goal, position);
CREATE INDEX events_by_author ON events (goal, author, seq, id);

CREATE TABLE blobs (
    hash BLOB NOT NULL PRIMARY KEY,
    len INTEGER NOT NULL,
    bytes BLOB,
    CHECK (CASE WHEN len > 262144 THEN bytes IS NULL ELSE length(bytes) IS len END)
) STRICT;

CREATE TABLE local (
    space INTEGER NOT NULL,
    key BLOB NOT NULL,
    value BLOB NOT NULL,
    PRIMARY KEY (space, key)
) STRICT, WITHOUT ROWID;
";

/// Reject unsupported markers and unmarked databases containing schema objects.
pub(crate) fn check(conn: &Connection) -> Result<(), OpenError> {
    let found: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(sql)?;
    if found == VERSION {
        return Ok(());
    }
    let empty = if found == 0 {
        conn.query_row(
            "SELECT NOT EXISTS(SELECT 1 FROM sqlite_schema)",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(sql)?
    } else {
        false
    };
    if empty {
        Ok(())
    } else {
        Err(OpenError::UnsupportedSchema {
            found,
            known: VERSION,
        })
    }
}

/// Atomically create the current tables and marker only in an empty database.
pub(crate) fn initialize(conn: &mut Connection) -> Result<(), OpenError> {
    let tx = conn.transaction().map_err(sql)?;
    check(&tx)?;
    let found: i64 = tx
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(sql)?;
    if found == 0 {
        tx.execute_batch(CURRENT).map_err(sql)?;
        tx.pragma_update(None, "user_version", VERSION)
            .map_err(sql)?;
    }
    tx.commit().map_err(sql)?;
    Ok(())
}
