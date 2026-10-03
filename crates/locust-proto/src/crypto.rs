//! Hashing and signing primitives.
//!
//! Identifiers use BLAKE3 in key-derivation mode with a fixed context string
//! per purpose, so a digest computed for one purpose can never be presented
//! as another. Stored content uses plain BLAKE3 so it matches standard tools.
//! Signatures are Ed25519 over a domain-separated digest, never over raw
//! caller-supplied bytes.

use std::fmt;

use crate::id::{BlobHash, PublicKey, Signature};

/// Context strings. Changing one changes every identifier derived with it.
pub mod domain {
    pub const EVENT_ID: &str = "locust v0 event id";
    pub const EVENT_SIGNATURE: &str = "locust v0 event signature";
    pub const GOAL_ID: &str = "locust v0 goal id";
    pub const INVITE_SECRET: &str = "locust v0 invitation secret";
    pub const JOIN_SIGNATURE: &str = "locust v0 join signature";
}

/// Starts a domain-separated digest; feed it with `update` and `finalize`.
pub fn domain_hasher(context: &str) -> blake3::Hasher {
    blake3::Hasher::new_derive_key(context)
}

pub fn domain_hash(context: &str, bytes: &[u8]) -> [u8; 32] {
    *domain_hasher(context).update(bytes).finalize().as_bytes()
}

pub fn content_hash(bytes: &[u8]) -> BlobHash {
    BlobHash(*blake3::hash(bytes).as_bytes())
}

/// An Ed25519 signing key held by the daemon for one enrolled principal.
pub struct Keypair(ed25519_dalek::SigningKey);

impl Keypair {
    /// Builds the key from 32 secret bytes. The caller supplies randomness;
    /// this crate never generates any.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self(ed25519_dalek::SigningKey::from_bytes(&seed))
    }

    pub fn seed(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    pub fn public(&self) -> PublicKey {
        PublicKey(self.0.verifying_key().to_bytes())
    }

    /// Signs `digest` for the purpose named by `context`.
    pub fn sign(&self, context: &str, digest: &[u8; 32]) -> Signature {
        use ed25519_dalek::Signer;
        Signature(self.0.sign(&domain_hash(context, digest)).to_bytes())
    }
}

impl fmt::Debug for Keypair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Keypair({})", self.public())
    }
}

/// Checks a signature made by [`Keypair::sign`] with the same context and
/// digest. Malformed keys and non-canonical signatures fail.
pub fn verify(key: &PublicKey, context: &str, digest: &[u8; 32], signature: &Signature) -> bool {
    let Ok(key) = ed25519_dalek::VerifyingKey::from_bytes(&key.0) else {
        return false;
    };
    let signature = ed25519_dalek::Signature::from_bytes(&signature.0);
    key.verify_strict(&domain_hash(context, digest), &signature)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_are_bound_to_key_context_and_digest() {
        let key = Keypair::from_seed([1; 32]);
        let other = Keypair::from_seed([2; 32]);
        let digest = [9; 32];
        let signature = key.sign(domain::EVENT_SIGNATURE, &digest);

        assert!(verify(
            &key.public(),
            domain::EVENT_SIGNATURE,
            &digest,
            &signature
        ));
        assert!(!verify(
            &other.public(),
            domain::EVENT_SIGNATURE,
            &digest,
            &signature
        ));
        assert!(!verify(
            &key.public(),
            domain::JOIN_SIGNATURE,
            &digest,
            &signature
        ));
        assert!(!verify(
            &key.public(),
            domain::EVENT_SIGNATURE,
            &[8; 32],
            &signature
        ));
    }

    #[test]
    fn domains_separate_digests() {
        assert_ne!(
            domain_hash(domain::EVENT_ID, b"same"),
            domain_hash(domain::GOAL_ID, b"same")
        );
        assert_ne!(
            domain_hash(domain::EVENT_ID, b"same"),
            content_hash(b"same").0
        );
    }

    #[test]
    fn debug_never_prints_the_seed() {
        let key = Keypair::from_seed([0x5a; 32]);
        let printed = format!("{key:?}");
        assert!(printed.contains(&key.public().to_string()));
        assert!(!printed.contains(&"5a".repeat(32)));
    }
}
