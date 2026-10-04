//! Held content objects: where each one lives and its row in `blobs`.
//!
//! An object of at most [`INLINE_MAX_BYTES`] is stored in its row, in the
//! same transaction as the rest of its commit. A larger one is a file in
//! `blobs/`, written and synced, renamed into place and the directory synced
//! before the transaction that adds its row commits, so a row never names a
//! file that a crash could lose. A file whose row never committed is an
//! orphan; [`collect_garbage`] removes it on open.

use locust_proto::id::BlobHash;
use locust_proto::store::{Blob, StoreError};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::columns::{blob, count, int};
use crate::error::{corrupted, sql};
use crate::files::{Files, Kind};

/// Largest object stored inline in the database; larger ones are files.
///
/// Measured on an Apple SSD with APFS (see the crate documentation): every
/// F_FULLFSYNC costs about 4 ms, and an inline object up to 512 KiB commits
/// in the one flush its commit pays anyway, while a file object adds two
/// (its own and its directory's). Reading favours files above about 64 KiB,
/// but by tens of microseconds. At 256 KiB every typical payload (task text,
/// notes, summaries) and small manifest commits in one flush; bigger
/// manifests, patches and artifacts go to files, where a range read touches
/// only the bytes it returns and the WAL does not carry them twice. An
/// inline object is read whole even for a range, which this bound keeps
/// small. Changing it needs a schema migration (see `schema::V1`).
pub const INLINE_MAX_BYTES: usize = 256 * 1024;

const HELD: &str = "SELECT 1 FROM blobs WHERE hash = ?1";
const LEN: &str = "SELECT len FROM blobs WHERE hash = ?1";
const READ: &str = "SELECT len, bytes FROM blobs WHERE hash = ?1";
const INSERT: &str =
    "INSERT INTO blobs (hash, len, bytes) VALUES (?1, ?2, ?3) ON CONFLICT (hash) DO NOTHING";
const DELETE: &str = "DELETE FROM blobs WHERE hash = ?1 RETURNING len";

/// Whether an object of this many bytes is a file.
pub(crate) fn is_file(len: u64) -> bool {
    len > INLINE_MAX_BYTES as u64
}

pub(crate) fn is_held(conn: &Connection, hash: &BlobHash) -> Result<bool, StoreError> {
    let mut statement = conn.prepare_cached(HELD).map_err(sql)?;
    statement.exists([hash.as_bytes()]).map_err(sql)
}

pub(crate) fn len(conn: &Connection, hash: &BlobHash) -> Result<Option<u64>, StoreError> {
    let mut statement = conn.prepare_cached(LEN).map_err(sql)?;
    let len = statement
        .query_row([hash.as_bytes()], |row| row.get::<_, i64>(0))
        .optional()
        .map_err(sql)?;
    len.map(|len| {
        u64::try_from(len).map_err(|_| corrupted(format_args!("object {hash} has length {len}")))
    })
    .transpose()
}

/// Writes the file of every large object in `blobs` that is not held yet,
/// then syncs the directory once. Runs before the commit's transaction.
pub(crate) fn install(conn: &Connection, files: &Files, blobs: &[Blob]) -> Result<(), StoreError> {
    let mut written: Vec<BlobHash> = Vec::new();
    for blob in blobs {
        let hash = blob.hash();
        if !is_file(blob.bytes().len() as u64) || written.contains(&hash) || is_held(conn, &hash)? {
            continue;
        }
        files.write(&hash, blob.bytes())?;
        written.push(hash);
    }
    if !written.is_empty() {
        files.sync()?;
    }
    Ok(())
}

/// Adds a row for every object in `blobs` not held yet, inside the caller's
/// transaction; a large object's file must already be installed.
pub(crate) fn insert(tx: &Connection, blobs: &[Blob]) -> Result<(), StoreError> {
    if blobs.is_empty() {
        return Ok(());
    }
    let mut insert = tx.prepare_cached(INSERT).map_err(sql)?;
    for blob in blobs {
        let len = blob.bytes().len() as u64;
        let inline = (!is_file(len)).then_some(blob.bytes());
        insert
            .execute(params![blob.hash().as_bytes(), int(len)?, inline])
            .map_err(sql)?;
    }
    Ok(())
}

/// Adds the row of a large object whose file is already in place.
pub(crate) fn insert_file(tx: &Connection, hash: &BlobHash, len: u64) -> Result<(), StoreError> {
    let mut insert = tx.prepare_cached(INSERT).map_err(sql)?;
    insert
        .execute(params![hash.as_bytes(), int(len)?, None::<&[u8]>])
        .map_err(sql)?;
    Ok(())
}

/// Deletes the rows of `hashes` inside the caller's transaction and returns
/// the objects that were files, to remove once the transaction commits.
pub(crate) fn delete(tx: &Connection, hashes: &[BlobHash]) -> Result<Vec<BlobHash>, StoreError> {
    if hashes.is_empty() {
        return Ok(Vec::new());
    }
    let mut delete = tx.prepare_cached(DELETE).map_err(sql)?;
    let mut removed = Vec::new();
    for hash in hashes {
        let mut rows = delete.query([hash.as_bytes()]).map_err(sql)?;
        if let Some(row) = rows.next().map_err(sql)?
            && is_file(count(row, 0)?)
        {
            removed.push(*hash);
        }
    }
    Ok(removed)
}

/// The whole object.
pub(crate) fn read(
    conn: &Connection,
    files: &Files,
    hash: &BlobHash,
) -> Result<Option<Vec<u8>>, StoreError> {
    let mut statement = conn.prepare_cached(READ).map_err(sql)?;
    let mut rows = statement.query([hash.as_bytes()]).map_err(sql)?;
    let Some(row) = rows.next().map_err(sql)? else {
        return Ok(None);
    };
    let len = count(row, 0)?;
    if is_file(len) {
        files.read(hash, len).map(Some)
    } else {
        inline(row, hash, len).map(|bytes| Some(bytes.to_vec()))
    }
}

/// Up to `max` bytes of the object from `offset`, as `Store::blob_range`
/// specifies. A file object is read only where the range lies; an inline one
/// is at most [`INLINE_MAX_BYTES`].
pub(crate) fn read_range(
    conn: &Connection,
    files: &Files,
    hash: &BlobHash,
    offset: u64,
    max: usize,
) -> Result<Option<Vec<u8>>, StoreError> {
    let mut statement = conn.prepare_cached(READ).map_err(sql)?;
    let mut rows = statement.query([hash.as_bytes()]).map_err(sql)?;
    let Some(row) = rows.next().map_err(sql)? else {
        return Ok(None);
    };
    let len = count(row, 0)?;
    let start = offset.min(len);
    let take = usize::try_from(len - start).map_or(max, |rest| rest.min(max));
    if take == 0 {
        return Ok(Some(Vec::new()));
    }
    if is_file(len) {
        return files.read_range(hash, len, start, take).map(Some);
    }
    // `start + take <= len`, which `inline` checked against the bytes.
    let start = start as usize;
    Ok(Some(inline(row, hash, len)?[start..start + take].to_vec()))
}

/// The bytes of an inline object, which must be `len` long.
fn inline<'row>(row: &'row Row<'_>, hash: &BlobHash, len: u64) -> Result<&'row [u8], StoreError> {
    let bytes = blob(row, 1)?;
    if bytes.len() as u64 != len {
        return Err(corrupted(format_args!(
            "inline object {hash} has {} bytes, its row records {len}",
            bytes.len()
        )));
    }
    Ok(bytes)
}

/// Removes what a crash or a failed commit can leave in `blobs/`: temporary
/// files, object files no row names, and staged copies of objects already
/// held (promotion committed, removal lost). Staged copies of objects not
/// held are kept: they are transfers to resume.
pub(crate) fn collect_garbage(conn: &Connection, files: &Files) -> Result<(), StoreError> {
    for (hash, kind) in files.entries()? {
        let orphan = match kind {
            Kind::Temporary => true,
            Kind::Object => !len(conn, &hash)?.is_some_and(is_file),
            Kind::Staged => is_held(conn, &hash)?,
        };
        if orphan {
            files.remove_entry(&hash, kind);
        }
    }
    Ok(())
}
