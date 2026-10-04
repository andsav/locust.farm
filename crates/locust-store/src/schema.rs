//! The database schema and its single forward migration path.
//!
//! `PRAGMA user_version` records the schema version, 0 for a new file.
//! Opening applies every migration above the recorded version in order, each
//! in one transaction with its version bump, and refuses a database whose
//! version is newer than this binary knows.

use rusqlite::Connection;

use crate::error::{OpenError, corrupted, sql};

/// The schema version this binary writes.
pub(crate) const VERSION: i64 = MIGRATIONS.len() as i64;

/// Entry `n` migrates version `n` to `n + 1`. Append only: a released entry
/// is never edited, because databases in the field already ran it.
const MIGRATIONS: [&str; 1] = [V1];

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
///   Changing the limit therefore needs a migration that moves objects.
/// - `local`: non-replicated records, ordered by key bytes within a space.
pub(crate) const V1: &str = "
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

/// Brings the database to [`VERSION`].
pub(crate) fn migrate(conn: &mut Connection) -> Result<(), OpenError> {
    let found: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(sql)?;
    if found > VERSION {
        return Err(OpenError::NewerSchema {
            found,
            known: VERSION,
        });
    }
    let done = usize::try_from(found)
        .map_err(|_| corrupted(format_args!("schema version {found} is negative")))?;
    for (from, migration) in (0..).zip(MIGRATIONS).skip(done) {
        let tx = conn.transaction().map_err(sql)?;
        tx.execute_batch(migration).map_err(sql)?;
        tx.pragma_update(None, "user_version", from + 1)
            .map_err(sql)?;
        tx.commit().map_err(sql)?;
    }
    Ok(())
}
