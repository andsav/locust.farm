//! Opening the database: the lock and the durability settings. The crate
//! documentation explains what each setting buys and what it costs.

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, ErrorCode};

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
    let mode: String = conn
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        .map_err(|error| match error.sqlite_error_code() {
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => {
                OpenError::InUse(dir.to_path_buf())
            }
            _ => sql(error).into(),
        })?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(OpenError::Store(locust_proto::store::StoreError::Failed(
            format!("the database refused write-ahead logging (journal mode {mode})"),
        )));
    }
    conn.pragma_update(None, "synchronous", "FULL")
        .map_err(sql)?;
    // F_FULLFSYNC on macOS; SQLite ignores it where the platform lacks it.
    conn.pragma_update(None, "fullfsync", "ON").map_err(sql)?;
    conn.set_prepared_statement_cache_capacity(STATEMENT_CACHE);
    Ok(conn)
}
