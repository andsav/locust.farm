//! Sealed content objects: the only form in which a goal's content is stored
//! and transferred.
//!
//! Every content object an event names (a payload, a manifest, a file, a
//! patch) is the output of [`seal`]. Its identity is the plain BLAKE3 hash of
//! the sealed bytes ([`crate::crypto::content_hash`]), so a store, a relay or
//! a peer verifies, deduplicates and resumes a transfer of an object without
//! holding any key. Plaintext exists only inside a member's daemon and on
//! its local API.
//!
//! # Layout
//!
//! ```text
//! offset  bytes  field
//!      0      1  format      FORMAT
//!      1      4  epoch       key epoch, little-endian u32
//!      5     24  nonce       derived from the content, see below
//!     29      n  ciphertext  as long as the plaintext
//!   29+n     16  tag         Poly1305
//! ```
//!
//! The header is in the clear so that a holder of the bytes learns which
//! epoch key opens them ([`epoch_of`]) from the object alone: a manifest
//! entry or a patch reference is a bare hash with nowhere else to say it.
//!
//! # Construction
//!
//! `key` is the goal's [`ContentKey`] for `epoch`.
//!
//! ```text
//! enc_key   = BLAKE3 derive_key(context = domain::SEAL_KEY,   material = key)
//! nonce_key = BLAKE3 derive_key(context = domain::SEAL_NONCE, material = key)
//! aad       = format (1 byte) || epoch (4 bytes, LE) || goal (32 bytes)
//! nonce     = first 24 bytes of BLAKE3 keyed_hash(nonce_key, aad || plaintext)
//! ciphertext || tag = XChaCha20-Poly1305(enc_key, nonce, aad, plaintext)
//! ```
//!
//! The nonce is a keyed hash of everything the cipher is given, so sealing is
//! deterministic and needs no randomness or counter: the same content in the
//! same goal and epoch always gives the same bytes and therefore one
//! identity. Covering the associated data as well as the plaintext means two
//! different inputs never share a nonce, even if one key were wrongly used
//! for two epochs or two goals; a shared nonce would reuse the Poly1305
//! one-time key and allow forgery.
//!
//! [`open`] recomputes the nonce from what it decrypted and refuses a
//! mismatch. So exactly one byte string opens to a given content under a
//! given goal, epoch and key, and no key holder can mint a second identity
//! for it. The same check commits the bytes to the key: making one object
//! open to different content under two keys needs a collision on 192 bits of
//! keyed BLAKE3 (about 2^96 work), which XChaCha20-Poly1305 alone does not
//! prevent.
//!
//! # What sealing protects
//!
//! Confidentiality and integrity of content against anyone without the epoch
//! key, including relays and holders that are not members. An object opens
//! only for the goal and epoch it was sealed for, and any change to any byte
//! is detected.
//!
//! # What it does not protect
//!
//! - Equality. Two objects with the same content in the same goal and epoch
//!   are the same bytes. An observer who can also get a member to seal
//!   content of the observer's choosing can confirm a guess of another
//!   object's whole content.
//! - Length and epoch. The plaintext length is the stored length minus
//!   [`OVERHEAD_BYTES`], and the epoch is readable by anyone.
//! - Authorship. Every holder of the epoch key can seal. Who introduced an
//!   object is established by the signed event that names its hash.
//! - Earlier epochs. A removed member keeps the keys it was given and can
//!   still open what was sealed under them.
//!
//! The epoch read by [`epoch_of`] is unauthenticated until [`open`] succeeds;
//! use it only to choose a key.

use std::fmt;

use chacha20poly1305::{AeadInOut, KeyInit, XChaCha20Poly1305};

use crate::crypto::{ContentKey, domain, domain_hash};
use crate::id::GoalId;
use crate::limits::MAX_BLOB_BYTES;

/// Format byte of the construction described in the module documentation. A
/// different cipher, derivation or layout takes a different byte, and an
/// object carrying an unknown byte is refused rather than guessed at.
pub const FORMAT: u8 = 1;

/// Offset of the format byte.
pub const FORMAT_OFFSET: usize = 0;

/// Offset of the key epoch, a little-endian `u32`.
pub const EPOCH_OFFSET: usize = 1;

/// Offset of the nonce.
pub const NONCE_OFFSET: usize = 5;

/// Length of the XChaCha20 nonce.
pub const NONCE_BYTES: usize = 24;

/// Offset of the ciphertext, which is exactly as long as the plaintext. The
/// tag follows it and ends the object.
pub const CIPHERTEXT_OFFSET: usize = NONCE_OFFSET + NONCE_BYTES;

/// Length of the Poly1305 tag.
pub const TAG_BYTES: usize = 16;

/// Bytes a sealed object is longer than its plaintext: header, nonce and tag.
/// Also the length of the smallest sealed object, the empty plaintext.
pub const OVERHEAD_BYTES: usize = CIPHERTEXT_OFFSET + TAG_BYTES;

/// Largest plaintext [`seal`] accepts: its sealed form is exactly
/// [`MAX_BLOB_BYTES`], the ceiling for one stored object.
pub const MAX_PLAINTEXT_BYTES: usize = MAX_BLOB_BYTES - OVERHEAD_BYTES;

/// Associated data: the format byte and epoch as they appear in the header,
/// then the goal.
const AAD_BYTES: usize = NONCE_OFFSET + GoalId::LEN;

/// Why content was not sealed, or bytes were not opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SealError {
    /// The first byte is not [`FORMAT`]: the bytes are not a sealed object,
    /// or were sealed by a construction this version does not read.
    UnknownFormat,
    /// The bytes are empty or shorter than [`OVERHEAD_BYTES`]. An object cut
    /// short at or beyond that length is [`SealError::NotAuthentic`] instead,
    /// because its last bytes are then not its tag.
    Truncated,
    /// The plaintext exceeds [`MAX_PLAINTEXT_BYTES`], or the sealed bytes
    /// exceed [`MAX_BLOB_BYTES`]. Nothing was encrypted or decrypted.
    TooLarge,
    /// Authentication failed: the key or the goal is not the one the object
    /// was sealed with, or a byte was changed. These cases cannot be told
    /// apart.
    NotAuthentic,
    /// The object authenticates under this key, but its nonce is not the one
    /// derived from its content, so [`seal`] did not produce it. Only a key
    /// holder can make such an object; accepting it would give one content a
    /// second identity.
    NotCanonical,
}

impl fmt::Display for SealError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnknownFormat => "bytes are not a sealed object of a known format",
            Self::Truncated => "sealed object is cut short",
            Self::TooLarge => "content exceeds the admitted size",
            Self::NotAuthentic => "sealed object does not authenticate for this goal and key",
            Self::NotCanonical => "sealed object is not the canonical sealing of its content",
        })
    }
}

impl std::error::Error for SealError {}

/// The stored length of a plaintext of `plaintext_len` bytes: always
/// [`OVERHEAD_BYTES`] more. `None` only when that overflows.
///
/// Check limits with this before sealing. Every limit on a content object
/// ([`crate::limits::MAX_PAYLOAD_BYTES`], [`MAX_BLOB_BYTES`], an operator's
/// lower ceiling) and every length on the wire or in the store counts sealed
/// bytes.
pub const fn sealed_len(plaintext_len: u64) -> Option<u64> {
    plaintext_len.checked_add(OVERHEAD_BYTES as u64)
}

/// The plaintext length of a sealed object of `sealed_len` bytes. `None` when
/// that is shorter than any sealed object.
pub const fn plaintext_len(sealed_len: u64) -> Option<u64> {
    sealed_len.checked_sub(OVERHEAD_BYTES as u64)
}

/// Seals `plaintext` for one goal under that goal's key for `epoch`.
///
/// Deterministic: the same four inputs always give the same bytes, and a
/// change to any of them changes the nonce, the ciphertext and the tag. The
/// result is [`sealed_len`] of the plaintext length. The caller supplies the
/// key that belongs to `epoch`; nothing here can check that pairing.
///
/// Fails only with [`SealError::TooLarge`], before encrypting anything.
pub fn seal(
    goal: &GoalId,
    epoch: u32,
    key: &ContentKey,
    plaintext: &[u8],
) -> Result<Vec<u8>, SealError> {
    if plaintext.len() > MAX_PLAINTEXT_BYTES {
        return Err(SealError::TooLarge);
    }
    let aad = associated_data(goal, epoch);
    let nonce = derive_nonce(key, &aad, plaintext);

    // One allocation: the plaintext is copied into place and encrypted there.
    let mut sealed = Vec::with_capacity(OVERHEAD_BYTES + plaintext.len());
    sealed.extend_from_slice(&aad[..NONCE_OFFSET]);
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(plaintext);
    let tag = cipher(key)
        .encrypt_inout_detached(
            (&nonce).into(),
            &aad,
            (&mut sealed[CIPHERTEXT_OFFSET..]).into(),
        )
        // The cipher's own limit is far above the ceiling checked above.
        .map_err(|_| SealError::TooLarge)?;
    sealed.extend_from_slice(&tag);
    Ok(sealed)
}

/// Opens bytes sealed for `goal`, with the key of the epoch [`epoch_of`]
/// reports for them.
///
/// `Ok(plaintext)` means `sealed` is exactly what [`seal`] returns for this
/// goal, that epoch, this key and that plaintext. Never panics, whatever the
/// bytes; nothing is decrypted unless the tag verifies. Establishing the
/// first guarantee costs one keyed hash of the plaintext after decryption.
pub fn open(goal: &GoalId, key: &ContentKey, sealed: &[u8]) -> Result<Vec<u8>, SealError> {
    let parts = parts(sealed)?;
    if sealed.len() > MAX_BLOB_BYTES {
        return Err(SealError::TooLarge);
    }
    let aad = associated_data(goal, parts.epoch);
    let mut plaintext = parts.ciphertext.to_vec();
    cipher(key)
        .decrypt_inout_detached(
            parts.nonce.into(),
            &aad,
            plaintext.as_mut_slice().into(),
            parts.tag.into(),
        )
        .map_err(|_| SealError::NotAuthentic)?;
    // The nonce is public and the tag has already verified, so this
    // comparison has nothing to hide from a timing observer.
    if derive_nonce(key, &aad, &plaintext) != *parts.nonce {
        return Err(SealError::NotCanonical);
    }
    Ok(plaintext)
}

/// Reads the key epoch from sealed bytes without a key, so the caller can
/// look up (or request) the key before calling [`open`].
///
/// Checks the format byte and the minimum length only. The value is whatever
/// the bytes say: it is authenticated when [`open`] succeeds, not before.
pub fn epoch_of(sealed: &[u8]) -> Result<u32, SealError> {
    parts(sealed).map(|parts| parts.epoch)
}

/// The fields of a sealed object, borrowed from its bytes.
struct Parts<'a> {
    epoch: u32,
    nonce: &'a [u8; NONCE_BYTES],
    ciphertext: &'a [u8],
    tag: &'a [u8; TAG_BYTES],
}

/// Splits sealed bytes by the layout. Checks the format byte first, so bytes
/// of another format are never reported as merely short.
fn parts(sealed: &[u8]) -> Result<Parts<'_>, SealError> {
    match sealed.first() {
        Some(&FORMAT) => {}
        Some(_) => return Err(SealError::UnknownFormat),
        None => return Err(SealError::Truncated),
    }
    let (prefix, rest) = sealed
        .split_first_chunk::<NONCE_OFFSET>()
        .ok_or(SealError::Truncated)?;
    let (nonce, rest) = rest
        .split_first_chunk::<NONCE_BYTES>()
        .ok_or(SealError::Truncated)?;
    let (ciphertext, tag) = rest
        .split_last_chunk::<TAG_BYTES>()
        .ok_or(SealError::Truncated)?;
    let [_format, epoch @ ..] = *prefix;
    Ok(Parts {
        epoch: u32::from_le_bytes(epoch),
        nonce,
        ciphertext,
        tag,
    })
}

/// What the tag and the nonce bind besides the content: the header bytes
/// before the nonce, then the goal. Its first [`NONCE_OFFSET`] bytes are
/// written to the object as they are.
fn associated_data(goal: &GoalId, epoch: u32) -> [u8; AAD_BYTES] {
    let mut aad = [0u8; AAD_BYTES];
    aad[FORMAT_OFFSET] = FORMAT;
    aad[EPOCH_OFFSET..NONCE_OFFSET].copy_from_slice(&epoch.to_le_bytes());
    aad[NONCE_OFFSET..].copy_from_slice(goal.as_bytes());
    aad
}

/// The cipher under the encryption key derived from the epoch key. The epoch
/// key itself never keys a cipher or a hash directly.
fn cipher(key: &ContentKey) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new(&domain_hash(domain::SEAL_KEY, &key.0).into())
}

/// The nonce for one input: a keyed hash, under its own derived key, of
/// everything the cipher is given. `aad` has a fixed length, so the two parts
/// cannot be confused.
fn derive_nonce(key: &ContentKey, aad: &[u8; AAD_BYTES], plaintext: &[u8]) -> [u8; NONCE_BYTES] {
    let nonce_key = domain_hash(domain::SEAL_NONCE, &key.0);
    let mut nonce = [0u8; NONCE_BYTES];
    // The first bytes of the extended output are the first bytes of the hash.
    blake3::Hasher::new_keyed(&nonce_key)
        .update(aad)
        .update(plaintext)
        .finalize_xof()
        .fill(&mut nonce);
    nonce
}

#[cfg(test)]
mod tests {
    use chacha20poly1305::aead::{Aead, Payload};

    use super::*;
    use crate::crypto::content_hash;
    use crate::id::{Hex, hex_to_vec};
    use crate::limits::BLOB_CHUNK_BYTES;

    // Golden vectors for format 1, answering lane B's request B-3 for
    // fixtures. They pin the sealed-object format the way `vectors.rs` pins
    // events: a change that makes one fail changes what stored goals and
    // existing peers understand, so it needs a new format byte, not an
    // updated constant. The goal is the published test goal; the epoch has
    // four distinct bytes so its byte order is pinned too.
    const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const GOAL: &str = "0e3b678262546bfdce982c2a35432985908cf52de085300e2a1635d70907fd99";
    const EPOCH: u32 = 0x0102_0304;

    const ENC_KEY: &str = "82f5d7a15ddf8539e572f22f3bd2081ee932679af82eb128a02bc3ee66395395";
    const NONCE_KEY: &str = "594ec2264c95630b4e122cdb6e254f64981dbf7d43573b6422bc8a4013465e3f";

    const EMPTY_SEALED: &str = "0104030201ac7aa52c46f3181fa2dff0edad63e88b4506894829c2b7eeb9b69dce82e52e14539ccf4e5bef0b72";
    const EMPTY_HASH: &str = "733255901be6a3bcef066d591ab1e1db6c451f422c1be0cda52dd6b60f99fb41";

    /// The plaintext is the bytes 0, 1, .. 149: two full ChaCha20 blocks and
    /// a partial one, ending in a partial Poly1305 block.
    const PATTERN_LEN: u8 = 150;
    const PATTERN_SEALED: &str = "0104030201448d59207e299c027e99d371b47303cf37ee6c29e524ac1c563460b7c9b6e45b689c5a783abf41c04027cd1451f1cae6e194602a1700ee1fbb7af57e4f71807f5b810fffcb30144781ec1a92a7113cb54f128703b5fa75740bcd5fd8ce492a845c9c322a525c94de32ce8f642cb13210dcc969b46f3c885ac1183100b8a21616bdcd7bd64c4d0fa4b0e3b80b880a46adc71798584d23690599326876aac15f29380f255d8eb20fb88c0ba99ef100e6f6ee57ddaa66036c43d9c5c43eb8f4";
    const PATTERN_HASH: &str = "8b755f3ed311570384fc16282f61e553391daacb5e4e7d64d04d8462fb9e4902";

    fn key() -> ContentKey {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&hex_to_vec(KEY).unwrap());
        ContentKey(bytes)
    }

    fn goal() -> GoalId {
        GOAL.parse().unwrap()
    }

    fn pattern() -> Vec<u8> {
        (0..PATTERN_LEN).collect()
    }

    fn nonce_of(sealed: &[u8]) -> &[u8] {
        &sealed[NONCE_OFFSET..CIPHERTEXT_OFFSET]
    }

    /// The construction written out from the specification with literal
    /// values and other entry points of the same primitives, sharing nothing
    /// with the implementation. `nonce` replaces the derived nonce.
    fn reference(
        goal: &[u8; 32],
        epoch: u32,
        key: &[u8; 32],
        plaintext: &[u8],
        nonce: Option<[u8; 24]>,
    ) -> Vec<u8> {
        let enc_key = blake3::derive_key("locust v0 seal key", key);
        let nonce_key = blake3::derive_key("locust v0 seal nonce", key);

        let mut aad = vec![0x01];
        aad.extend_from_slice(&epoch.to_le_bytes());
        aad.extend_from_slice(goal);
        assert_eq!(aad.len(), 37);

        let mut hashed = aad.clone();
        hashed.extend_from_slice(plaintext);
        let mut derived = [0u8; 24];
        derived.copy_from_slice(&blake3::keyed_hash(&nonce_key, &hashed).as_bytes()[..24]);
        let nonce = nonce.unwrap_or(derived);

        let ciphertext_and_tag = XChaCha20Poly1305::new(&enc_key.into())
            .encrypt(
                (&nonce).into(),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .unwrap();

        let mut sealed = aad[..5].to_vec();
        sealed.extend_from_slice(&nonce);
        sealed.extend_from_slice(&ciphertext_and_tag);
        sealed
    }

    #[test]
    fn golden_vectors() {
        let (goal, key) = (goal(), key());
        assert_eq!(
            Hex(&blake3::derive_key("locust v0 seal key", &key.0)).to_string(),
            ENC_KEY
        );
        assert_eq!(
            Hex(&blake3::derive_key("locust v0 seal nonce", &key.0)).to_string(),
            NONCE_KEY
        );
        for (plaintext, sealed_hex, hash_hex) in [
            (Vec::new(), EMPTY_SEALED, EMPTY_HASH),
            (pattern(), PATTERN_SEALED, PATTERN_HASH),
        ] {
            let sealed = seal(&goal, EPOCH, &key, &plaintext).unwrap();
            assert_eq!(Hex(&sealed).to_string(), sealed_hex);
            assert_eq!(content_hash(&sealed).to_string(), hash_hex);
            assert_eq!(reference(&goal.0, EPOCH, &key.0, &plaintext, None), sealed);

            let published = hex_to_vec(sealed_hex).unwrap();
            assert_eq!(epoch_of(&published), Ok(EPOCH));
            assert_eq!(open(&goal, &key, &published), Ok(plaintext));
        }
    }

    #[test]
    fn the_implementation_is_the_construction_written_out() {
        let (goal, key) = (GoalId([0xa5; 32]), ContentKey([0xc3; 32]));
        // Up to several BLAKE3 chunks and many ChaCha20 blocks.
        for len in [0usize, 1, 64, 150, 1024, 5000] {
            let plaintext: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
            for epoch in [0, 1, u32::MAX] {
                assert_eq!(
                    seal(&goal, epoch, &key, &plaintext).unwrap(),
                    reference(&goal.0, epoch, &key.0, &plaintext, None),
                    "length {len}, epoch {epoch}"
                );
            }
        }
    }

    #[test]
    fn the_layout_is_format_epoch_nonce_ciphertext_tag() {
        assert_eq!(FORMAT, 1);
        assert_eq!(
            (FORMAT_OFFSET, EPOCH_OFFSET, NONCE_OFFSET, CIPHERTEXT_OFFSET),
            (0, 1, 5, 29)
        );
        assert_eq!((NONCE_BYTES, TAG_BYTES, OVERHEAD_BYTES), (24, 16, 45));

        let sealed = seal(&goal(), EPOCH, &key(), b"abc").unwrap();
        assert_eq!(sealed.len(), OVERHEAD_BYTES + 3);
        assert_eq!(sealed[FORMAT_OFFSET], FORMAT);
        assert_eq!(sealed[EPOCH_OFFSET..NONCE_OFFSET], [0x04, 0x03, 0x02, 0x01]);
        assert_eq!(epoch_of(&sealed), Ok(EPOCH));
        assert_ne!(sealed[CIPHERTEXT_OFFSET..CIPHERTEXT_OFFSET + 3], *b"abc");
    }

    #[test]
    fn content_round_trips_from_empty_to_a_whole_transfer_chunk() {
        let (goal, key) = (goal(), key());
        let sealed_chunk = BLOB_CHUNK_BYTES - OVERHEAD_BYTES;
        let lengths = [
            // Empty, then around a Poly1305 block, a ChaCha20 block and a
            // BLAKE3 chunk.
            0,
            1,
            15,
            16,
            17,
            63,
            64,
            65,
            1023,
            1024,
            1025,
            // The sealed bytes fill exactly one transfer chunk, then spill
            // one byte into a second; the plaintext is exactly one chunk.
            sealed_chunk,
            sealed_chunk + 1,
            BLOB_CHUNK_BYTES,
        ];
        for len in lengths {
            let plaintext: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
            let sealed = seal(&goal, 3, &key, &plaintext).unwrap();
            assert_eq!(sealed_len(len as u64), Some(sealed.len() as u64));
            assert_eq!(plaintext_len(sealed.len() as u64), Some(len as u64));
            assert_eq!(epoch_of(&sealed), Ok(3));
            assert!(
                open(&goal, &key, &sealed).as_deref() == Ok(plaintext.as_slice()),
                "length {len} did not round trip"
            );
        }
    }

    #[test]
    fn sealing_is_deterministic() {
        let (goal, key) = (goal(), key());
        let first = seal(&goal, EPOCH, &key, b"the same content").unwrap();
        let again = seal(&goal, EPOCH, &key, b"the same content").unwrap();
        assert_eq!(again, first);
        assert_eq!(content_hash(&again), content_hash(&first));

        // Other content takes another nonce, so no keystream is ever reused.
        let other = seal(&goal, EPOCH, &key, b"the same content.").unwrap();
        assert_ne!(nonce_of(&other), nonce_of(&first));
    }

    #[test]
    fn another_goal_epoch_or_key_gives_other_bytes_that_do_not_open_here() {
        let plaintext = pattern();
        let (goal, key) = (goal(), key());
        let other_goal = GoalId([0x11; 32]);
        let other_key = ContentKey([0x22; 32]);

        let sealed = seal(&goal, 7, &key, &plaintext).unwrap();
        let for_other_goal = seal(&other_goal, 7, &key, &plaintext).unwrap();
        let for_other_epoch = seal(&goal, 8, &key, &plaintext).unwrap();
        let for_other_key = seal(&goal, 7, &other_key, &plaintext).unwrap();

        // Not only the header differs: nonce, ciphertext and tag all do.
        let tag_at = sealed.len() - TAG_BYTES;
        for other in [&for_other_goal, &for_other_epoch, &for_other_key] {
            assert_ne!(nonce_of(other), nonce_of(&sealed));
            assert_ne!(
                other[CIPHERTEXT_OFFSET..tag_at],
                sealed[CIPHERTEXT_OFFSET..tag_at]
            );
            assert_ne!(other[tag_at..], sealed[tag_at..]);
        }

        let refused = Err(SealError::NotAuthentic);
        assert_eq!(open(&other_goal, &key, &sealed), refused);
        assert_eq!(open(&goal, &key, &for_other_goal), refused);
        assert_eq!(open(&goal, &other_key, &sealed), refused);
        assert_eq!(open(&goal, &key, &for_other_key), refused);

        // An object carries its epoch, so the only way to present it under
        // another one is to relabel it.
        for (object, epoch) in [(&sealed, 8u32), (&for_other_epoch, 7)] {
            let mut relabelled = object.clone();
            relabelled[EPOCH_OFFSET..NONCE_OFFSET].copy_from_slice(&epoch.to_le_bytes());
            assert_eq!(epoch_of(&relabelled), Ok(epoch));
            assert_eq!(open(&goal, &key, &relabelled), refused);
        }

        assert_eq!(open(&goal, &key, &sealed).unwrap(), plaintext);
        assert_eq!(open(&other_goal, &key, &for_other_goal).unwrap(), plaintext);
        assert_eq!(open(&goal, &key, &for_other_epoch).unwrap(), plaintext);
        assert_eq!(open(&goal, &other_key, &for_other_key).unwrap(), plaintext);
    }

    #[test]
    fn a_key_wrongly_used_for_two_epochs_or_goals_never_repeats_a_nonce() {
        // A repeated nonce under one key would reuse the Poly1305 one-time
        // key for two different messages. The nonce therefore covers the
        // epoch and goal as well as the plaintext.
        let key = key();
        let sealed = seal(&goal(), 0, &key, b"").unwrap();
        let next_epoch = seal(&goal(), 1, &key, b"").unwrap();
        let next_goal = seal(&GoalId([0x11; 32]), 0, &key, b"").unwrap();
        assert_ne!(nonce_of(&next_epoch), nonce_of(&sealed));
        assert_ne!(nonce_of(&next_goal), nonce_of(&sealed));
        assert_ne!(nonce_of(&next_goal), nonce_of(&next_epoch));
    }

    #[test]
    fn every_change_to_any_single_byte_is_rejected() {
        let (goal, key) = (goal(), key());
        let sealed = seal(&goal, EPOCH, &key, b"twenty bytes of text").unwrap();
        assert_eq!(sealed.len(), OVERHEAD_BYTES + 20);
        for index in 0..sealed.len() {
            // The format byte, each epoch and nonce byte, each ciphertext
            // byte and each tag byte, set to every other value.
            let expected = if index == FORMAT_OFFSET {
                SealError::UnknownFormat
            } else {
                SealError::NotAuthentic
            };
            for change in 1..=u8::MAX {
                let mut tampered = sealed.clone();
                tampered[index] ^= change;
                assert_eq!(
                    open(&goal, &key, &tampered),
                    Err(expected),
                    "byte {index} changed by {change:#04x}"
                );
            }
        }
    }

    #[test]
    fn truncation_at_every_length_is_rejected_without_panic() {
        let (goal, key) = (goal(), key());
        let sealed = seal(&goal, EPOCH, &key, &pattern()).unwrap();
        for len in 0..sealed.len() {
            let cut = &sealed[..len];
            if len < OVERHEAD_BYTES {
                assert_eq!(open(&goal, &key, cut), Err(SealError::Truncated));
                assert_eq!(epoch_of(cut), Err(SealError::Truncated));
            } else {
                assert_eq!(open(&goal, &key, cut), Err(SealError::NotAuthentic));
                assert_eq!(epoch_of(cut), Ok(EPOCH));
            }
        }

        let mut extended = sealed.clone();
        extended.push(0);
        assert_eq!(open(&goal, &key, &extended), Err(SealError::NotAuthentic));
        assert!(open(&goal, &key, &sealed[1..]).is_err());
    }

    #[test]
    fn bytes_of_another_format_are_refused_whatever_their_length() {
        let (goal, key) = (goal(), key());
        for bytes in [&[0u8][..], &[2], &[0; OVERHEAD_BYTES], &[0xff; 200]] {
            assert_eq!(open(&goal, &key, bytes), Err(SealError::UnknownFormat));
            assert_eq!(epoch_of(bytes), Err(SealError::UnknownFormat));
        }
        // A plaintext manifest or note is not mistaken for a sealed object
        // unless it happens to start with the format byte; then it fails
        // authentication.
        assert_eq!(
            open(
                &goal,
                &key,
                b"plain text, never sealed, longer than the overhead"
            ),
            Err(SealError::UnknownFormat)
        );
        assert_eq!(
            open(&goal, &key, &[FORMAT; OVERHEAD_BYTES]),
            Err(SealError::NotAuthentic)
        );
    }

    #[test]
    fn the_epoch_is_read_without_a_key() {
        for epoch in [0, 1, 0x0102_0304, u32::MAX] {
            let sealed = seal(&goal(), epoch, &key(), b"content").unwrap();
            assert_eq!(epoch_of(&sealed), Ok(epoch));
        }
        assert_eq!(epoch_of(&[]), Err(SealError::Truncated));
        assert_eq!(epoch_of(&[FORMAT]), Err(SealError::Truncated));
        assert_eq!(
            epoch_of(&[FORMAT; OVERHEAD_BYTES - 1]),
            Err(SealError::Truncated)
        );
        // It is only what the bytes say.
        assert_eq!(epoch_of(&[FORMAT; OVERHEAD_BYTES]), Ok(0x0101_0101));
    }

    #[test]
    fn only_the_bytes_seal_produces_open() {
        let (goal, key) = (goal(), key());
        let plaintext = pattern();
        let canonical = seal(&goal, EPOCH, &key, &plaintext).unwrap();

        // A key holder encrypts the same content correctly but under a nonce
        // of its choosing: authentic, and a second identity for the content.
        let chosen = reference(&goal.0, EPOCH, &key.0, &plaintext, Some([0x77; 24]));
        assert_ne!(content_hash(&chosen), content_hash(&canonical));
        assert_eq!(open(&goal, &key, &chosen), Err(SealError::NotCanonical));

        // Likewise under the nonce that belongs to other content.
        let mut borrowed = [0u8; 24];
        borrowed.copy_from_slice(nonce_of(&seal(&goal, EPOCH, &key, b"other").unwrap()));
        let moved = reference(&goal.0, EPOCH, &key.0, &plaintext, Some(borrowed));
        assert_eq!(open(&goal, &key, &moved), Err(SealError::NotCanonical));

        // So whatever opens is reproduced exactly by sealing its plaintext.
        let opened = open(&goal, &key, &canonical).unwrap();
        let epoch = epoch_of(&canonical).unwrap();
        assert_eq!(seal(&goal, epoch, &key, &opened).unwrap(), canonical);
    }

    #[test]
    fn stored_and_plain_lengths_differ_by_the_overhead() {
        assert_eq!(sealed_len(0), Some(45));
        assert_eq!(sealed_len(150), Some(195));
        assert_eq!(plaintext_len(45), Some(0));
        assert_eq!(plaintext_len(44), None);
        assert_eq!(plaintext_len(0), None);
        assert_eq!(sealed_len(u64::MAX - 45), Some(u64::MAX));
        assert_eq!(sealed_len(u64::MAX - 44), None);
        assert_eq!(
            sealed_len(MAX_PLAINTEXT_BYTES as u64),
            Some(MAX_BLOB_BYTES as u64)
        );
    }

    #[test]
    fn content_above_the_ceiling_is_refused() {
        let (goal, key) = (goal(), key());
        let too_long = vec![0u8; MAX_PLAINTEXT_BYTES + 1];
        assert_eq!(seal(&goal, 0, &key, &too_long), Err(SealError::TooLarge));

        let mut too_long = vec![0u8; MAX_BLOB_BYTES + 1];
        too_long[FORMAT_OFFSET] = FORMAT;
        assert_eq!(open(&goal, &key, &too_long), Err(SealError::TooLarge));
    }

    #[test]
    fn no_key_material_appears_in_debug_or_display_output() {
        let key = ContentKey([0x5a; 32]);
        let sealed = seal(&goal(), 0, &key, b"content").unwrap();
        let wrong_key = open(&goal(), &ContentKey([0x5b; 32]), &sealed);

        // An error is a bare variant, so it has no room for key bytes.
        assert_eq!(std::mem::size_of::<SealError>(), 1);
        let mut printed = format!("{key:?} {wrong_key:?}");
        for error in [
            SealError::UnknownFormat,
            SealError::Truncated,
            SealError::TooLarge,
            SealError::NotAuthentic,
            SealError::NotCanonical,
        ] {
            printed.push_str(&format!(" {error:?} {error}"));
        }

        for secret in [
            key.0,
            domain_hash(domain::SEAL_KEY, &key.0),
            domain_hash(domain::SEAL_NONCE, &key.0),
        ] {
            let hex = Hex(&secret[..4]).to_string();
            let list = format!("{:?}", &secret[..4]);
            assert!(!printed.contains(&hex), "{printed}");
            assert!(
                !printed.contains(list.trim_matches(['[', ']'])),
                "{printed}"
            );
        }
    }
}
