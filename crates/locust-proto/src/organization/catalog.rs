//! Owner-scoped local authoring records. Source and semantic identities differ.
use super::Formation;
use crate::id::PublicKey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub id: String,
    pub owner: PublicKey,
    pub revision: u64,
    /// Exact authored bytes as UTF-8, including whitespace and incomplete JSON.
    pub source: String,
    pub source_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Presentation {
    pub draft_id: String,
    pub owner: PublicKey,
    pub revision: u64,
    /// Opaque JSON layout metadata; never included in semantic identity.
    pub data_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Publication {
    pub id: String,
    pub owner: PublicKey,
    pub draft_id: String,
    pub draft_revision: u64,
    pub source: String,
    pub source_hash: String,
    pub normalized_json: String,
    pub semantic_hash: String,
}

/// Identity of exact source bytes, distinct from normalized semantic identity.
pub fn source_hash(source: &str) -> String {
    let mut hash = blake3::Hasher::new_derive_key("locust formation source v1");
    hash.update(source.as_bytes());
    hash.finalize().to_hex().to_string()
}

impl Publication {
    /// Decode the normalized semantic definition from its wire-safe JSON source.
    pub fn normalized(&self) -> Result<Formation, serde_json::Error> {
        serde_json::from_str(&self.normalized_json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec;
    fn round_trip<T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(
        value: &T,
    ) {
        assert_eq!(
            codec::decode_canonical::<T>(&codec::encode(value).unwrap()).unwrap(),
            *value
        );
    }
    #[test]
    fn every_catalog_record_round_trips_the_binary_codec() {
        let source = "{\"schema_version\":2}".to_owned();
        let owner = PublicKey([7; 32]);
        round_trip(&Draft {
            id: "draft".into(),
            owner,
            revision: 1,
            source_hash: source_hash(&source),
            source: source.clone(),
        });
        round_trip(&Presentation {
            draft_id: "draft".into(),
            owner,
            revision: 0,
            data_json: "{\"x\":1}".into(),
        });
        let normalized = Formation::default();
        round_trip(&Publication {
            id: "published".into(),
            owner,
            draft_id: "draft".into(),
            draft_revision: 1,
            source_hash: source_hash(&source),
            source,
            normalized_json: serde_json::to_string(&normalized).unwrap(),
            semantic_hash: super::super::semantic_hash(&normalized),
        });
    }
}
