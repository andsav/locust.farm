//! Maps keyed by event identifier.
//!
//! An event identifier is already a uniform digest, so hashing it again buys
//! nothing: the map takes its hash from the identifier's first eight bytes.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use locust_proto::id::EventId;

/// A map from event identifier to `V`.
pub(crate) type IdMap<V> = HashMap<EventId, V, BuildHasherDefault<IdHasher>>;

/// Reads the hash out of the identifier's own bytes.
#[derive(Clone, Copy, Default)]
pub(crate) struct IdHasher(u64);

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        // An identifier arrives as one 32-byte write; anything shorter is
        // folded in so the hasher stays correct for any key.
        match bytes.first_chunk::<8>() {
            Some(head) => self.0 ^= u64::from_le_bytes(*head),
            None => {
                for &byte in bytes {
                    self.0 = self.0.rotate_left(8) ^ u64::from(byte);
                }
            }
        }
    }

    // The length prefix of a fixed-size array carries no information.
    fn write_usize(&mut self, _: usize) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identifier_map_finds_what_was_put_in() {
        let mut map: IdMap<u32> = IdMap::default();
        for n in 0..=255u8 {
            let mut bytes = [n; 32];
            bytes[31] = n.wrapping_mul(7);
            map.insert(EventId(bytes), u32::from(n));
        }
        assert_eq!(map.len(), 256);
        let mut probe = [9u8; 32];
        probe[31] = 63;
        assert_eq!(map.get(&EventId(probe)), Some(&9));
        assert_eq!(map.get(&EventId([9; 32])), None);
    }
}
