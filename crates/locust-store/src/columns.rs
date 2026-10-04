//! Reading typed values out of rows and integers into and out of SQLite.
//!
//! SQLite integers are signed 64-bit. The contract keeps every value the
//! store indexes at or below `i64::MAX` (sequence numbers by
//! `Header::check`, positions and lengths by what fits on a disk), so a value
//! outside that range is either a caller asking past the end or a damaged row.

use locust_proto::store::StoreError;
use rusqlite::Row;
use rusqlite::types::ValueRef;

use crate::error::{corrupted, sql};

/// Borrows the blob in column `index` without copying it.
pub(crate) fn blob<'row>(row: &'row Row<'_>, index: usize) -> Result<&'row [u8], StoreError> {
    match row.get_ref(index).map_err(sql)? {
        ValueRef::Blob(bytes) => Ok(bytes),
        other => Err(corrupted(format_args!(
            "column {index} holds {:?}, not a blob",
            other.data_type()
        ))),
    }
}

/// A fixed-size identifier stored in column `index`.
pub(crate) fn fixed<const N: usize>(row: &Row<'_>, index: usize) -> Result<[u8; N], StoreError> {
    let bytes = blob(row, index)?;
    bytes.try_into().map_err(|_| {
        corrupted(format_args!(
            "column {index} holds {} bytes, not {N}",
            bytes.len()
        ))
    })
}

/// A count, position or length stored in column `index`.
pub(crate) fn count(row: &Row<'_>, index: usize) -> Result<u64, StoreError> {
    let value: i64 = row.get(index).map_err(sql)?;
    u64::try_from(value).map_err(|_| corrupted(format_args!("column {index} holds {value}")))
}

/// A count, position or length as an SQLite integer, for a value this store
/// writes. Only damaged input can exceed the range.
pub(crate) fn int(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value)
        .map_err(|_| StoreError::Failed(format!("{value} exceeds the store's integer range")))
}

/// A row limit as an SQLite integer; larger limits mean "all".
pub(crate) fn limit(limit: usize) -> i64 {
    i64::try_from(limit).unwrap_or(i64::MAX)
}
