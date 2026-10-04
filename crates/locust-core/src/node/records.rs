//! Encoding of the node's local records.
//!
//! Every local record is postcard through [`locust_proto::codec`]. A record
//! type lives next to the code that owns it; this module only holds what they
//! share: building a write, reading a value back, and composing keys.

use locust_proto::codec;
use locust_proto::store::{LocalWrite, Space, StoreError};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// A write that sets `key` in `space` to the encoding of `value`.
pub(super) fn put<T: Serialize + ?Sized>(space: Space, key: Vec<u8>, value: &T) -> LocalWrite {
    LocalWrite::Put {
        space,
        key,
        // The node's own records are plain data that always encodes.
        value: codec::encode(value).expect("local records encode"),
    }
}

/// A write that removes `key` from `space`.
pub(super) fn delete(space: Space, key: Vec<u8>) -> LocalWrite {
    LocalWrite::Delete { space, key }
}

/// Decodes a stored record. Bytes this node wrote always decode, so a failure
/// is damage to the store.
pub(super) fn read<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, StoreError> {
    codec::decode(bytes).map_err(|_| StoreError::Corrupted("a local record does not decode".into()))
}

/// A key made of a one-byte tag followed by fixed-width parts.
pub(super) fn key(tag: u8, parts: &[&[u8]]) -> Vec<u8> {
    let mut key = Vec::with_capacity(1 + parts.iter().map(|part| part.len()).sum::<usize>());
    key.push(tag);
    for part in parts {
        key.extend_from_slice(part);
    }
    key
}

/// The fixed-width part of `key` that starts at `offset`, if it is there.
pub(super) fn part<const N: usize>(key: &[u8], offset: usize) -> Option<[u8; N]> {
    key.get(offset..offset + N)?.try_into().ok()
}

/// The error for a key this node could not have written.
pub(super) fn bad_key() -> StoreError {
    StoreError::Corrupted("a local record has a malformed key".into())
}
