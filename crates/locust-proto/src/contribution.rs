//! Canonical inert patches bound to two sealed workspace manifests.
//! This content format is independent of the signed-event and API versions.
use crate::codec;
use crate::id::BlobHash;
use crate::limits::MAX_MANIFEST_ENTRIES;
use crate::manifest::Entry;
use serde::{Deserialize, Serialize};
use std::fmt;
const MAGIC: &[u8] = b"locust-contribution\0";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContributionError {
    Invalid(String),
    NoChanges,
}
impl fmt::Display for ContributionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(reason) => write!(f, "invalid contribution: {reason}"),
            Self::NoChanges => f.write_str("contribution contains no changes"),
        }
    }
}
impl std::error::Error for ContributionError {}
fn invalid(reason: impl Into<String>) -> ContributionError {
    ContributionError::Invalid(reason.into())
}
fn check_path(path: &str) -> Result<(), ContributionError> {
    if crate::manifest::is_safe_path(path) {
        Ok(())
    } else {
        Err(invalid("unsafe contribution path"))
    }
}

/// One path's exact before and after entries. A missing entry is an addition
/// or deletion; all other metadata is bound by the two stored manifests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub before: Option<Entry>,
    pub after: Option<Entry>,
}

/// Version 1 of the contribution blob format. Identifiers name sealed objects,
/// including `base` and `head`; they are never hashes of plaintext bytes here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contribution {
    pub version: u32,
    pub base: BlobHash,
    pub head: BlobHash,
    #[serde(deserialize_with = "read_changes")]
    pub changes: Vec<Change>,
}

fn read_changes<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Change>, D::Error> {
    struct Changes;
    impl<'de> serde::de::Visitor<'de> for Changes {
        type Value = Vec<Change>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("at most two manifests' worth of changed paths")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let max = MAX_MANIFEST_ENTRIES * 2;
            if seq.size_hint().is_some_and(|n| n > max) {
                return Err(serde::de::Error::custom("too many changes"));
            }
            let mut result = Vec::new();
            while let Some(change) = seq.next_element()? {
                if result.len() == max {
                    return Err(serde::de::Error::custom("too many changes"));
                }
                result.push(change);
            }
            Ok(result)
        }
    }
    d.deserialize_seq(Changes)
}

impl Contribution {
    pub fn check(&self) -> Result<(), ContributionError> {
        if self.version != 1 {
            return Err(invalid("unsupported format version"));
        }
        if self.base == self.head {
            return Err(invalid("base and head must differ"));
        }
        if self.changes.is_empty() {
            return Err(ContributionError::NoChanges);
        }
        if self.changes.len() > MAX_MANIFEST_ENTRIES * 2
            || !self.changes.is_sorted_by(|a, b| a.path < b.path)
        {
            return Err(invalid("changed paths must be bounded, sorted and unique"));
        }
        for change in &self.changes {
            check_path(&change.path)?;
            if change.before == change.after {
                return Err(invalid("unchanged path in delta"));
            }
            for entry in [&change.before, &change.after].into_iter().flatten() {
                if entry.path != change.path
                    || entry.size > (crate::seal::MAX_PLAINTEXT_BYTES) as u64
                {
                    return Err(invalid("entry path or size is invalid"));
                }
            }
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, ContributionError> {
        self.check()?;
        let mut bytes = MAGIC.to_vec();
        bytes.extend(codec::encode(self).map_err(|e| invalid(e.to_string()))?);
        if bytes.len() > crate::seal::MAX_PLAINTEXT_BYTES {
            return Err(invalid("contribution exceeds content-object limit"));
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ContributionError> {
        if bytes.len() > crate::seal::MAX_PLAINTEXT_BYTES {
            return Err(invalid("contribution exceeds content-object limit"));
        }
        let bytes = bytes
            .strip_prefix(MAGIC)
            .ok_or_else(|| invalid("not a contribution blob"))?;
        let value: Self = codec::decode_canonical(bytes).map_err(|e| invalid(e.to_string()))?;
        value.check()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Contribution {
        let entry = Entry {
            path: "a.rs".into(),
            executable: false,
            size: 3,
            content: BlobHash([3; 32]),
        };
        Contribution {
            version: 1,
            base: BlobHash([1; 32]),
            head: BlobHash([2; 32]),
            changes: vec![Change {
                path: "a.rs".into(),
                before: None,
                after: Some(entry),
            }],
        }
    }
    #[test]
    fn version_one_encoding_is_pinned() {
        let bytes = crate::id::hex_to_vec(concat!(
            "6c6f637573742d636f6e747269627574696f6e00", // tag, including NUL
            "01",                                       // version
            "0101010101010101010101010101010101010101010101010101010101010101",
            "0202020202020202020202020202020202020202020202020202020202020202",
            "0104612e7273000104612e72730003", // one addition of a.rs, mode false, size 3
            "0303030303030303030303030303030303030303030303030303030303030303"
        ))
        .unwrap();
        assert_eq!(sample().encode().unwrap(), bytes);
        assert_eq!(Contribution::decode(&bytes).unwrap(), sample());
    }
    #[test]
    fn tagged_canonical_roundtrip_and_trailing_bytes() {
        let value = sample();
        let bytes = value.encode().unwrap();
        assert_eq!(Contribution::decode(&bytes).unwrap(), value);
        assert!(Contribution::decode(&bytes[MAGIC.len()..]).is_err());
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(Contribution::decode(&trailing).is_err());
        let mut overlong = bytes;
        overlong.splice(MAGIC.len()..MAGIC.len() + 1, [0x81, 0]);
        assert!(Contribution::decode(&overlong).is_err());
    }
    #[test]
    fn malformed_reference_metadata_cannot_be_encoded() {
        let mut value = sample();
        value.changes[0].after.as_mut().unwrap().path = "different".into();
        assert!(value.encode().is_err());
        let mut value = sample();
        value.changes[0].path = "../outside".into();
        assert!(value.encode().is_err());
        let mut value = sample();
        value.changes[0].after.as_mut().unwrap().size = crate::limits::MAX_BLOB_BYTES as u64;
        assert!(value.encode().is_err());
        let mut value = sample();
        value.changes.push(value.changes[0].clone());
        assert!(value.encode().is_err());
    }
}
