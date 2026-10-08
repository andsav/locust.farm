//! Opening the database: the lock and the durability settings. The crate
//! documentation explains what each setting buys and what it costs.

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, ErrorCode, OpenFlags};

use crate::error::{OpenError, sql};

/// Prepared statements kept per connection; the store uses fewer than this.
const STATEMENT_CACHE: usize = 32;

/// Opens `database`, takes its exclusive lock and makes every commit durable
/// against power loss. Fails with [`OpenError::InUse`] naming `dir` when
/// another connection holds the lock.
pub(crate) fn open(database: &Path, dir: &Path) -> Result<Connection, OpenError> {
    let conn = Connection::open(database).map_err(sql)?;
    // Fail at once on a held lock; rusqlite would otherwise wait 5 seconds.
    conn.busy_timeout(Duration::ZERO).map_err(sql)?;
    // Must precede the first access, which takes the lock and keeps the WAL
    // index in this process's memory.
    conn.pragma_update(None, "locking_mode", "EXCLUSIVE")
        .map_err(sql)?;
    // That first access fails while any other connection has the database
    // open, an sqlite3 shell as much as another store.
    first_read(&conn, dir)?;
    crate::schema::check(&conn)?;
    check_protocol(&conn)?;
    let mode: String = conn
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        .map_err(|error| in_use(error, dir))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(OpenError::Store(locust_proto::store::StoreError::Failed(
            format!("the database refused write-ahead logging (journal mode {mode})"),
        )));
    }
    conn.pragma_update(None, "synchronous", "FULL")
        .map_err(sql)?;
    // F_FULLFSYNC on macOS; SQLite ignores it where the platform lacks it.
    conn.pragma_update(None, "fullfsync", "ON").map_err(sql)?;
    recover(&conn)?;
    conn.set_prepared_statement_cache_capacity(STATEMENT_CACHE);
    Ok(conn)
}

/// A killed writer may leave a complete commit record in the kernel cache
/// without having synced it. SQLite can recover and read that record. Make
/// recovered state durable before the caller publishes it or collects files
/// based on it. FULL checkpoint syncs the WAL before copying it to the main
/// database, then syncs the database. The exclusive connection owns recovery.
fn recover(conn: &Connection) -> Result<(), OpenError> {
    #[cfg(test)]
    crate::faults::check(crate::faults::Point::Recovery, Path::new(""))
        .map_err(|error| crate::error::file("recover", Path::new("database"), error))?;
    let (busy, logged, checkpointed): (i64, i64, i64) = conn
        .query_row("PRAGMA wal_checkpoint(FULL)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(sql)?;
    if busy != 0 || logged != checkpointed {
        return Err(OpenError::Store(locust_proto::store::StoreError::Failed(
            "database recovery checkpoint did not complete".to_owned(),
        )));
    }
    Ok(())
}

/// Read the existing format before creating directories or configuring writes.
/// SQLite may need its existing WAL metadata to read a recovered database.
pub(crate) fn preflight(database: &Path, dir: &Path) -> Result<(), OpenError> {
    if !database.exists() {
        return Ok(());
    }
    let conn =
        Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(sql)?;
    conn.busy_timeout(Duration::ZERO).map_err(sql)?;
    first_read(&conn, dir)?;
    crate::schema::check(&conn)?;
    check_protocol(&conn)
}

/// Reads once with lock-aware error mapping, before the shared checks.
fn first_read(conn: &Connection, dir: &Path) -> Result<(), OpenError> {
    conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
        .map(drop)
        .map_err(|error| in_use(error, dir))
}

/// A lock another connection holds is [`OpenError::InUse`], not a failure.
fn in_use(error: rusqlite::Error, dir: &Path) -> OpenError {
    match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => {
            OpenError::InUse(dir.to_path_buf())
        }
        _ => sql(error).into(),
    }
}

/// Reject incompatible signed events before initialization or garbage collection.
pub(crate) fn check_protocol(conn: &Connection) -> Result<(), OpenError> {
    crate::schema::check(conn)?;
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'events')",
            [],
            |row| row.get(0),
        )
        .map_err(sql)?;
    if !exists {
        return Ok(());
    }
    let mut statement = conn
        .prepare("SELECT DISTINCT substr(header, 1, 1) FROM events")
        .map_err(sql)?;
    let versions = statement
        .query_map([], |row| row.get::<_, Vec<u8>>(0))
        .map_err(sql)?;
    for version in versions {
        let bytes = version.map_err(sql)?;
        let found = *bytes
            .first()
            .ok_or_else(|| crate::error::corrupted("stored event has an empty header"))?;
        if found != locust_proto::PROTOCOL_VERSION {
            return Err(OpenError::UnsupportedProtocolVersion {
                found,
                known: locust_proto::PROTOCOL_VERSION,
            });
        }
    }
    Ok(())
}
