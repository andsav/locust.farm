//! Local, non-replicated records: opaque byte keys and values per space.
//!
//! SQLite orders blobs by their bytes, then by length, which is the order of
//! Rust byte strings, so a prefix scan is one index range: from the prefix up
//! to the first key that no longer starts with it.

use locust_proto::store::{LocalRecord, LocalWrite, Space, StoreError};
use rusqlite::{Connection, params};

use crate::columns::blob;
use crate::error::sql;

const PUT: &str = "INSERT INTO local (space, key, value) VALUES (?1, ?2, ?3) \
     ON CONFLICT (space, key) DO UPDATE SET value = excluded.value";
const DELETE: &str = "DELETE FROM local WHERE space = ?1 AND key = ?2";
const GET: &str = "SELECT value FROM local WHERE space = ?1 AND key = ?2";
const SCAN_TO_END: &str =
    "SELECT key, value FROM local WHERE space = ?1 AND key >= ?2 ORDER BY key";
const SCAN_RANGE: &str =
    "SELECT key, value FROM local WHERE space = ?1 AND key >= ?2 AND key < ?3 ORDER BY key";

fn space_id(space: Space) -> i64 {
    i64::from(space as u8)
}

/// Applies the writes in order inside the caller's transaction.
pub(crate) fn apply(tx: &Connection, writes: &[LocalWrite]) -> Result<(), StoreError> {
    if writes.is_empty() {
        return Ok(());
    }
    let mut put = tx.prepare_cached(PUT).map_err(sql)?;
    let mut delete = tx.prepare_cached(DELETE).map_err(sql)?;
    for write in writes {
        match write {
            LocalWrite::Put { space, key, value } => {
                put.execute(params![space_id(*space), key.as_slice(), value.as_slice()])
            }
            LocalWrite::Delete { space, key } => {
                delete.execute(params![space_id(*space), key.as_slice()])
            }
        }
        .map_err(sql)?;
    }
    Ok(())
}

pub(crate) fn get(
    conn: &Connection,
    space: Space,
    key: &[u8],
) -> Result<Option<Vec<u8>>, StoreError> {
    let mut statement = conn.prepare_cached(GET).map_err(sql)?;
    let mut rows = statement
        .query(params![space_id(space), key])
        .map_err(sql)?;
    match rows.next().map_err(sql)? {
        Some(row) => Ok(Some(blob(row, 0)?.to_vec())),
        None => Ok(None),
    }
}

pub(crate) fn scan(
    conn: &Connection,
    space: Space,
    prefix: &[u8],
) -> Result<Vec<LocalRecord>, StoreError> {
    let end = prefix_end(prefix);
    let mut statement;
    let mut rows = match &end {
        Some(end) => {
            statement = conn.prepare_cached(SCAN_RANGE).map_err(sql)?;
            statement.query(params![space_id(space), prefix, end.as_slice()])
        }
        None => {
            statement = conn.prepare_cached(SCAN_TO_END).map_err(sql)?;
            statement.query(params![space_id(space), prefix])
        }
    }
    .map_err(sql)?;
    let mut records = Vec::new();
    while let Some(row) = rows.next().map_err(sql)? {
        records.push((blob(row, 0)?.to_vec(), blob(row, 1)?.to_vec()));
    }
    Ok(records)
}

/// The least key greater than every key that starts with `prefix`, or `None`
/// when no such key exists (the prefix is empty or all `0xff`).
fn prefix_end(prefix: &[u8]) -> Option<Vec<u8>> {
    let last = prefix.iter().rposition(|&byte| byte != 0xff)?;
    let mut end = prefix[..=last].to_vec();
    end[last] += 1;
    Some(end)
}

#[cfg(test)]
mod tests {
    use super::prefix_end;

    #[test]
    fn a_prefix_ends_at_its_successor_without_trailing_ff_bytes() {
        assert_eq!(prefix_end(b""), None);
        assert_eq!(prefix_end(&[0xff, 0xff]), None);
        assert_eq!(prefix_end(&[0x00]), Some(vec![0x01]));
        assert_eq!(prefix_end(&[0x00, 0xff]), Some(vec![0x01]));
        assert_eq!(prefix_end(b"b/"), Some(b"b0".to_vec()));
        assert_eq!(prefix_end(&[0x7f, 0xff, 0xff]), Some(vec![0x80]));
    }
}
