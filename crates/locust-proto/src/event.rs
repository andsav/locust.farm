//! Signed events: the durable, replicated record of a goal.
//!
//! An event is a [`Header`] encoded once, hashed and signed. The exact header
//! bytes are kept and relayed verbatim; nothing ever re-encodes a header in
//! order to verify it. User-written content (task text, notes, summaries)
//! lives in a detachable payload named by hash, so it can be withheld without
//! breaking the signed structure that membership, assignment and acceptance
//! depend on. Every payload is sealed under the goal's content key (see
//! [`crate::seal`]); its reference names the sealed bytes, so storage and
//! transfer verify a payload without holding the key.
//!
//! A goal is founded by two events committed together: the owner's genesis,
//! which founds authority and admits nobody, and the coordinator's first
//! decision, [`Body::MemberAdmitted`] for itself and its own endpoint.
//!
//! [`Event::decode`] establishes structural validity only: the bytes are one
//! canonical header, internally consistent, and signed by the author it names.
//! Whether that author was a member, and whether the transition is allowed,
//! is decided by `locust-core` against the goal's history.
//!
//! Two events by one author at one position, including two genesis events of
//! one goal, are conflicting histories. Both are kept as attributed evidence;
//! neither replaces the other, and a store holds and serves both.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::PROTOCOL_VERSION;
use crate::codec::{self, CodecError};
use crate::crypto::{self, Keypair, domain};
use crate::id::{BlobHash, EndpointId, EventId, GoalId, PublicKey, Signature};
use crate::limits::{
    MAX_ARTIFACTS, MAX_DEPENDENCIES, MAX_HEADER_BYTES, MAX_PARENTS, MAX_PAYLOAD_BYTES,
};
use crate::seal;
use crate::store::Blob;

/// The signed part of an event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    /// [`PROTOCOL_VERSION`]. Always the first byte on the wire.
    pub version: u8,
    /// The goal this event belongs to. Every event binds its goal.
    pub goal: GoalId,
    /// The principal whose key signed this header.
    pub author: PublicKey,
    /// Position in the author's own log for this goal, starting at zero and
    /// at most `i64::MAX`, so every store can index it as a signed 64-bit
    /// integer.
    pub seq: u64,
    /// The author's previous event in this goal; absent exactly when `seq` is zero.
    pub prev: Option<EventId>,
    /// For a coordinator decision: the decision it succeeds, which makes the
    /// decisions of a goal one hash-linked chain. For a contribution: the
    /// latest decision the author had applied when writing. Absent only on
    /// the genesis event.
    pub anchor: Option<EventId>,
    /// Further events this one causally follows, strictly ascending.
    pub parents: Vec<EventId>,
    /// The author's wall clock in Unix milliseconds. Diagnostic only: it never
    /// orders events, grants authority or resolves a conflict.
    pub at_ms: u64,
    /// Detachable, sealed user content for this event, if any.
    pub payload: Option<PayloadRef>,
    /// The typed, structural content.
    pub body: Body,
}

/// Names an event's detachable content. Every payload is sealed, so `hash`
/// and `len` describe the stored, sealed bytes: storage and transfer verify a
/// payload without holding its key, and plaintext is not expressible.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadRef {
    /// Plain BLAKE3 digest of the stored (sealed) bytes.
    pub hash: BlobHash,
    /// Length of the stored (sealed) bytes; at most [`MAX_PAYLOAD_BYTES`].
    pub len: u32,
    /// The content-key epoch the bytes are sealed under. It must equal the
    /// event's epoch: the number of [`Body::MemberRemoved`] decisions in the
    /// decision chain up to and including the event's anchor, so zero for
    /// genesis. A removal therefore rotates the key with no extra decision.
    /// `locust-core` checks it against the chain; a stale epoch is refused.
    pub key_epoch: u32,
}

impl PayloadRef {
    /// True if `blob` is the object this reference names: the same hash, the
    /// same stored length, and sealed under the epoch the signed header
    /// claims. A daemon checks this before it keeps bytes as an event's
    /// payload, so an author cannot claim the current epoch while sealing
    /// under a key a removed member still holds.
    pub fn admits(&self, blob: &Blob) -> bool {
        blob.hash() == self.hash
            && u64::try_from(blob.bytes().len()) == Ok(u64::from(self.len))
            && seal::epoch_of(blob.bytes()) == Ok(self.key_epoch)
    }
}

/// A position in one author's log: a sequence number and the event there.
/// Ordered by position, then identifier, which is the order author logs and
/// inventories use; conflicting events at one position stay distinct points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AuthorPoint {
    /// Position in the author's log.
    pub seq: u64,
    /// The event at that position.
    pub id: EventId,
}

/// The record that founds a goal. Its digest is the goal's identifier, so the
/// initial authority cannot be replaced under the same identifier. In
/// protocol version 0 the owner is the coordinator: [`Header::check`] refuses
/// a genesis event whose record names two different keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Genesis {
    /// Root authority; signs the genesis event.
    pub owner: PublicKey,
    /// Signs every later decision.
    pub coordinator: PublicKey,
    /// Random bytes that make two goals with the same keys distinct.
    pub salt: [u8; 16],
}

impl Genesis {
    /// The identifier of the goal this record founds: a domain-separated
    /// digest of the owner, the coordinator and the salt.
    pub fn goal_id(&self) -> GoalId {
        let mut hasher = crypto::domain_hasher(domain::GOAL_ID);
        hasher.update(&self.owner.0);
        hasher.update(&self.coordinator.0);
        hasher.update(&self.salt);
        GoalId(*hasher.finalize().as_bytes())
    }
}

/// How an executor answered a cancellation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelOutcome {
    /// Work stopped before producing a result.
    Stopped,
    /// The attempt had already finished.
    Completed,
    /// The executor cannot tell what took effect.
    Uncertain,
}

/// A replaceable shared document. Notes and findings are append-only
/// [`Body::Note`] events instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Doc {
    /// The goal's plan.
    Plan,
    /// The goal's summary of what was done and found.
    Summary,
}

/// The typed, structural content of an event.
///
/// Variants are identified on the wire by declaration order: append new ones,
/// never reorder or remove. Adding a variant requires a new
/// [`PROTOCOL_VERSION`]: an older peer cannot decode the new index, and it
/// cannot skip the event either, because the author's later events chain to
/// it through `prev`. Variants up to and including `RevisionAccepted` are
/// coordinator decisions; the rest are contributions any current member may
/// author. Task, assignment, result, cancellation and revision are each
/// identified by the event that created them. JSON renders each variant under
/// its [`Body::kind`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Body {
    /// Founds the goal. Signed by the owner; the first decision in the chain.
    /// It founds authority and admits nobody: the coordinator's first
    /// decision is [`Body::MemberAdmitted`] for itself, and goal creation
    /// commits both events together.
    Genesis(Genesis),
    /// Admits a principal and binds it to the endpoint that may speak for it.
    MemberAdmitted {
        /// The admitted principal.
        member: PublicKey,
        /// The only endpoint that may synchronize on the member's behalf.
        endpoint: EndpointId,
    },
    /// Removes a principal. Its contributions after `last_accepted` are kept
    /// as evidence but grant nothing. Events anchored at or after this
    /// decision use the next key epoch (see [`PayloadRef::key_epoch`]).
    MemberRemoved {
        /// The removed principal.
        member: PublicKey,
        /// The last point of the member's log that still counts.
        last_accepted: Option<AuthorPoint>,
    },
    /// Assigns a proposed task to a named member. A later assignment of the
    /// same task carries a higher `attempt` and supersedes this one.
    TaskAssigned {
        /// The [`Body::TaskProposed`] event.
        task: EventId,
        /// The member who is to do the work.
        assignee: PublicKey,
        /// One more than the attempt of the task's previous assignment.
        attempt: u32,
    },
    /// Withdraws an assignment's right to finalize. The executor answers with
    /// [`Body::CancelAcknowledged`]; until then the cancellation is only requested.
    CancelRequested {
        /// The [`Body::TaskAssigned`] event being cancelled.
        assignment: EventId,
    },
    /// Accepts a submitted result. With `head`, the named manifest becomes
    /// the accepted workspace head; that is valid only if the result's `base`
    /// equals the currently accepted head, or no head is accepted yet, so an
    /// accepted head is never computed against a superseded one.
    ResultAccepted {
        /// The [`Body::ResultSubmitted`] event.
        result: EventId,
        /// The manifest that becomes the accepted workspace head, if any.
        head: Option<BlobHash>,
    },
    /// Declines a submitted result. The payload may say why.
    ResultRejected {
        /// The [`Body::ResultSubmitted`] event.
        result: EventId,
    },
    /// Makes a document revision the accepted one. Valid only if the
    /// revision's `base` is the currently accepted revision.
    RevisionAccepted {
        /// The [`Body::Revision`] event.
        revision: EventId,
    },

    /// Proposes work. The payload is the task text; these fields are the
    /// authored policy every peer must see unchanged.
    TaskProposed {
        /// Manifest of the input snapshot the work starts from.
        input: Option<BlobHash>,
        /// Tasks this one depends on, named by their [`Body::TaskProposed`]
        /// events; at most [`MAX_DEPENDENCIES`].
        depends_on: Vec<EventId>,
        /// Authored absolute deadline in Unix milliseconds; absent means none.
        deadline_ms: Option<u64>,
        /// Authored attempt budget; absent means unlimited.
        max_attempts: Option<u32>,
    },
    /// The assignee takes this exact assignment.
    AssignmentAccepted {
        /// The [`Body::TaskAssigned`] event.
        assignment: EventId,
    },
    /// The assignee will not do this assignment.
    AssignmentDeclined {
        /// The [`Body::TaskAssigned`] event.
        assignment: EventId,
    },
    /// Progress report; text in the payload.
    Progress {
        /// The [`Body::TaskAssigned`] event.
        assignment: EventId,
    },
    /// The executor's result. Completion is not acceptance: only
    /// [`Body::ResultAccepted`] changes the goal's accepted state.
    ResultSubmitted {
        /// The [`Body::TaskAssigned`] event.
        assignment: EventId,
        /// Manifest the work was actually done against.
        base: Option<BlobHash>,
        /// Patch against `base`, if the result changes the workspace.
        patch: Option<BlobHash>,
        /// Further output objects; at most [`MAX_ARTIFACTS`].
        artifacts: Vec<BlobHash>,
    },
    /// The attempt ended without a result; reason in the payload.
    AttemptFailed {
        /// The [`Body::TaskAssigned`] event.
        assignment: EventId,
    },
    /// The executor's answer to a cancellation request.
    CancelAcknowledged {
        /// The [`Body::CancelRequested`] event.
        cancel: EventId,
        /// What the executor reports took effect.
        outcome: CancelOutcome,
    },
    /// An append-only note or finding, optionally about a task and optionally
    /// correcting an earlier note.
    Note {
        /// The event, usually a task, the note is about.
        about: Option<EventId>,
        /// The earlier [`Body::Note`] this one corrects.
        supersedes: Option<EventId>,
    },
    /// A proposed revision of a shared document, naming the accepted revision
    /// it was written against.
    Revision {
        /// The document revised.
        doc: Doc,
        /// The accepted [`Body::Revision`] this one was written against;
        /// absent when none was accepted yet.
        base: Option<EventId>,
    },
    /// The author asks the coordinator to remove it from the goal; the
    /// payload may say why. Membership ends only with a
    /// [`Body::MemberRemoved`] decision.
    LeaveRequested,
}

impl Body {
    /// True for events only the goal's authority may author.
    pub fn is_decision(&self) -> bool {
        matches!(
            self,
            Self::Genesis(_)
                | Self::MemberAdmitted { .. }
                | Self::MemberRemoved { .. }
                | Self::TaskAssigned { .. }
                | Self::CancelRequested { .. }
                | Self::ResultAccepted { .. }
                | Self::ResultRejected { .. }
                | Self::RevisionAccepted { .. }
        )
    }

    /// Stable name for logs, errors and command output; equal to the
    /// variant's JSON tag.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Genesis(_) => "genesis",
            Self::MemberAdmitted { .. } => "member_admitted",
            Self::MemberRemoved { .. } => "member_removed",
            Self::TaskAssigned { .. } => "task_assigned",
            Self::CancelRequested { .. } => "cancel_requested",
            Self::ResultAccepted { .. } => "result_accepted",
            Self::ResultRejected { .. } => "result_rejected",
            Self::RevisionAccepted { .. } => "revision_accepted",
            Self::TaskProposed { .. } => "task_proposed",
            Self::AssignmentAccepted { .. } => "assignment_accepted",
            Self::AssignmentDeclined { .. } => "assignment_declined",
            Self::Progress { .. } => "progress",
            Self::ResultSubmitted { .. } => "result_submitted",
            Self::AttemptFailed { .. } => "attempt_failed",
            Self::CancelAcknowledged { .. } => "cancel_acknowledged",
            Self::Note { .. } => "note",
            Self::Revision { .. } => "revision",
            Self::LeaveRequested => "leave_requested",
        }
    }
}

/// Why bytes were not accepted as a structurally valid event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventError {
    /// The header bytes exceed [`MAX_HEADER_BYTES`].
    TooLarge,
    /// The bytes do not decode as a header.
    Malformed,
    /// The bytes decode, but are not the one encoding of that header.
    NotCanonical,
    /// The header's version byte is not [`PROTOCOL_VERSION`].
    UnsupportedVersion(u8),
    /// `seq` and `prev` disagree, or `seq` exceeds `i64::MAX`.
    BadSequence,
    /// Too many parents, dependencies or artifacts, or parents out of order.
    BadReferences,
    /// A genesis event that does not found the goal it names under this
    /// version's rules, or a later event with no anchor.
    BadAnchor,
    /// The payload reference names more than [`MAX_PAYLOAD_BYTES`].
    /// The payload length is outside what a sealed payload can have: shorter
    /// than an empty sealed object, or above the payload limit.
    BadPayloadLength,
    /// The signing key is not the header's author.
    AuthorMismatch,
    /// The signature does not verify under the header's author.
    BadSignature,
    /// Stored bytes do not hash to the identifier they were stored under.
    IdMismatch,
}

impl fmt::Display for EventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => f.write_str("header exceeds the admitted size"),
            Self::Malformed => f.write_str("bytes are not an event header"),
            Self::NotCanonical => f.write_str("header is not in its canonical encoding"),
            Self::UnsupportedVersion(version) => {
                write!(f, "protocol version {version} is not supported")
            }
            Self::BadSequence => {
                f.write_str("sequence number is out of range or disagrees with the previous event")
            }
            Self::BadReferences => f.write_str("references are out of order or too many"),
            Self::BadAnchor => f.write_str("anchor does not fit the event kind"),
            Self::BadPayloadLength => {
                f.write_str("payload length is not that of an admitted sealed object")
            }
            Self::AuthorMismatch => f.write_str("signing key is not the header's author"),
            Self::BadSignature => f.write_str("signature does not verify"),
            Self::IdMismatch => f.write_str("stored bytes do not match their event identifier"),
        }
    }
}

impl std::error::Error for EventError {}

/// Largest sequence number: stores index positions as signed 64-bit integers.
const MAX_SEQ: u64 = i64::MAX as u64;

impl Header {
    /// Checks everything that can be checked without the goal's history: the
    /// version; `seq` at most `i64::MAX` and `prev` present exactly when
    /// `seq` is not zero; parent, dependency and artifact counts and parent
    /// order; the payload size; and that a genesis event founds the goal it
    /// names (position zero, no anchor or parents, signed by the owner, who
    /// in version 0 is also the coordinator) while every other event has an
    /// anchor.
    pub fn check(&self) -> Result<(), EventError> {
        if self.version != PROTOCOL_VERSION {
            return Err(EventError::UnsupportedVersion(self.version));
        }
        if self.seq > MAX_SEQ || (self.seq == 0) != self.prev.is_none() {
            return Err(EventError::BadSequence);
        }
        if self.parents.len() > MAX_PARENTS || !self.parents.is_sorted_by(|a, b| a < b) {
            return Err(EventError::BadReferences);
        }
        if let Some(payload) = &self.payload
            && !(seal::OVERHEAD_BYTES..=MAX_PAYLOAD_BYTES).contains(&(payload.len as usize))
        {
            return Err(EventError::BadPayloadLength);
        }
        match &self.body {
            Body::Genesis(genesis) => {
                // Version 0: the owner is the coordinator.
                let founds_this_goal = self.seq == 0
                    && self.anchor.is_none()
                    && self.parents.is_empty()
                    && self.author == genesis.owner
                    && genesis.owner == genesis.coordinator
                    && self.goal == genesis.goal_id();
                if !founds_this_goal {
                    return Err(EventError::BadAnchor);
                }
            }
            _ if self.anchor.is_none() => return Err(EventError::BadAnchor),
            Body::TaskProposed { depends_on, .. } if depends_on.len() > MAX_DEPENDENCIES => {
                return Err(EventError::BadReferences);
            }
            Body::ResultSubmitted { artifacts, .. } if artifacts.len() > MAX_ARTIFACTS => {
                return Err(EventError::BadReferences);
            }
            _ => {}
        }
        Ok(())
    }

    /// Every content object this header names, payload first.
    pub fn blobs(&self) -> Vec<BlobHash> {
        let mut blobs: Vec<BlobHash> = self.payload.iter().map(|payload| payload.hash).collect();
        match &self.body {
            Body::ResultAccepted { head, .. } => blobs.extend(head),
            Body::TaskProposed { input, .. } => blobs.extend(input),
            Body::ResultSubmitted {
                base,
                patch,
                artifacts,
                ..
            } => {
                blobs.extend(base);
                blobs.extend(patch);
                blobs.extend(artifacts);
            }
            _ => {}
        }
        blobs
    }
}

fn event_id(header_bytes: &[u8]) -> EventId {
    EventId(crypto::domain_hash(domain::EVENT_ID, header_bytes))
}

/// An event as it travels and as it is stored: the exact signed bytes and the
/// signature over their identifier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireEvent {
    /// The exact header bytes that were hashed and signed.
    #[serde(with = "codec::bytes")]
    pub header: Vec<u8>,
    /// The author's signature over the identifier of `header`.
    pub signature: Signature,
}

impl WireEvent {
    /// The identifier these bytes have if they are a valid event. Hashing is
    /// far cheaper than [`Event::decode`], so a receiver drops identifiers it
    /// already holds before paying for decoding and signature verification.
    pub fn id(&self) -> EventId {
        event_id(&self.header)
    }
}

/// A structurally valid event: decoded, canonical and signed by its author.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    id: EventId,
    header: Header,
    bytes: Box<[u8]>,
    signature: Signature,
}

impl Event {
    /// Encodes and signs a new event. Refuses a header whose author is not
    /// `key`'s principal, one that fails [`Header::check`], and one whose
    /// encoding exceeds [`MAX_HEADER_BYTES`].
    pub fn sign(header: Header, key: &Keypair) -> Result<Self, EventError> {
        if header.author != key.public() {
            return Err(EventError::AuthorMismatch);
        }
        header.check()?;
        let bytes = codec::encode(&header).map_err(|_| EventError::Malformed)?;
        if bytes.len() > MAX_HEADER_BYTES {
            return Err(EventError::TooLarge);
        }
        let id = event_id(&bytes);
        let signature = key.sign(domain::EVENT_SIGNATURE, &id.0);
        Ok(Self {
            id,
            header,
            bytes: bytes.into_boxed_slice(),
            signature,
        })
    }

    /// Validates bytes received from a peer or a client: size, version byte,
    /// the one canonical encoding (through [`codec::decode_canonical`]),
    /// [`Header::check`] and the author's signature, in that order.
    pub fn decode(header_bytes: &[u8], signature: Signature) -> Result<Self, EventError> {
        check_size_and_version(header_bytes)?;
        // One header has exactly one encoding, so one identifier.
        let header: Header =
            codec::decode_canonical(header_bytes).map_err(|error| match error {
                CodecError::NotCanonical => EventError::NotCanonical,
                _ => EventError::Malformed,
            })?;
        header.check()?;
        let id = event_id(header_bytes);
        if !crypto::verify(&header.author, domain::EVENT_SIGNATURE, &id.0, &signature) {
            return Err(EventError::BadSignature);
        }
        Ok(Self {
            id,
            header,
            bytes: header_bytes.into(),
            signature,
        })
    }

    /// Rebuilds an event that this daemon validated with [`Event::decode`] or
    /// [`Event::sign`] before persisting it under `id`. Runs
    /// [`Header::check`] and refuses bytes that no longer hash to `id`
    /// ([`EventError::IdMismatch`]), so a damaged record is reported instead
    /// of served under the wrong identifier. Skips the canonical re-encode and
    /// the signature check, which the bytes passed before they were stored;
    /// never use it on input from outside the local store.
    pub fn from_stored(
        id: EventId,
        header_bytes: Vec<u8>,
        signature: Signature,
    ) -> Result<Self, EventError> {
        check_size_and_version(&header_bytes)?;
        let header: Header = codec::decode(&header_bytes).map_err(|_| EventError::Malformed)?;
        header.check()?;
        if event_id(&header_bytes) != id {
            return Err(EventError::IdMismatch);
        }
        Ok(Self {
            id,
            header,
            bytes: header_bytes.into_boxed_slice(),
            signature,
        })
    }

    /// Validates an event received in a sync frame; see [`Event::decode`].
    pub fn from_wire(wire: &WireEvent) -> Result<Self, EventError> {
        Self::decode(&wire.header, wire.signature)
    }

    /// The exact signed bytes and signature, for relaying or storing.
    pub fn to_wire(&self) -> WireEvent {
        WireEvent {
            header: self.bytes.to_vec(),
            signature: self.signature,
        }
    }

    /// The domain-separated digest of the exact header bytes.
    pub fn id(&self) -> EventId {
        self.id
    }

    /// The decoded header.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The exact bytes that were hashed and signed.
    pub fn header_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The author's signature over [`Event::id`].
    pub fn signature(&self) -> Signature {
        self.signature
    }
}

/// Refuses oversized bytes, and reports another version from the first byte
/// before anything else is decoded.
fn check_size_and_version(bytes: &[u8]) -> Result<(), EventError> {
    if bytes.len() > MAX_HEADER_BYTES {
        return Err(EventError::TooLarge);
    }
    match bytes.first() {
        Some(&PROTOCOL_VERSION) => Ok(()),
        Some(&version) => Err(EventError::UnsupportedVersion(version)),
        None => Err(EventError::Malformed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::content_hash;
    use crate::testkit::{self, Author};

    fn note(author: &mut Author, goal: GoalId, anchor: EventId) -> Event {
        author.event(
            goal,
            Some(anchor),
            Body::Note {
                about: None,
                supersedes: None,
            },
        )
    }

    /// A note header by `key` after `genesis`, for tests that change one field.
    fn note_header(key: &Keypair, genesis: &Event) -> Header {
        Header {
            version: PROTOCOL_VERSION,
            goal: genesis.header().goal,
            author: key.public(),
            seq: 1,
            prev: Some(genesis.id()),
            anchor: Some(genesis.id()),
            parents: Vec::new(),
            at_ms: 0,
            payload: None,
            body: Body::Note {
                about: None,
                supersedes: None,
            },
        }
    }

    #[test]
    fn a_signed_event_round_trips_through_the_wire() {
        let mut owner = Author::new(1);
        let genesis = owner.genesis();
        let event = note(&mut owner, genesis.header().goal, genesis.id());

        let wire = event.to_wire();
        let frame = codec::encode(&wire).unwrap();
        let received = Event::from_wire(&codec::decode(&frame).unwrap()).unwrap();

        assert_eq!(received, event);
        assert_eq!(received.header().seq, 1);
        assert_eq!(received.header().prev, Some(genesis.id()));
        let stored = Event::from_stored(event.id(), wire.header, wire.signature).unwrap();
        assert_eq!(stored, event);
    }

    #[test]
    fn a_wire_event_knows_its_identifier_before_it_is_verified() {
        let mut owner = Author::new(1);
        let genesis = owner.genesis();
        let event = note(&mut owner, genesis.header().goal, genesis.id());
        assert_eq!(event.to_wire().id(), event.id());
        assert_eq!(genesis.to_wire().id(), genesis.id());
    }

    #[test]
    fn any_change_to_the_bytes_or_signature_is_rejected() {
        let mut owner = Author::new(1);
        let genesis = owner.genesis();
        let event = note(&mut owner, genesis.header().goal, genesis.id());

        let mut bytes = event.header_bytes().to_vec();
        *bytes.last_mut().unwrap() ^= 1;
        assert!(Event::decode(&bytes, event.signature()).is_err());

        let mut signature = event.signature();
        signature.0[0] ^= 1;
        assert_eq!(
            Event::decode(event.header_bytes(), signature),
            Err(EventError::BadSignature)
        );

        let stranger = testkit::keypair(9);
        let forged = stranger.sign(domain::EVENT_SIGNATURE, &event.id().0);
        assert_eq!(
            Event::decode(event.header_bytes(), forged),
            Err(EventError::BadSignature)
        );
    }

    #[test]
    fn only_the_canonical_encoding_is_accepted() {
        let mut owner = Author::new(1);
        let genesis = owner.genesis();
        let event = note(&mut owner, genesis.header().goal, genesis.id());

        // `seq` follows version (1 byte), goal (32) and author (32). Rewrite
        // its one-byte encoding of 1 as an overlong two-byte encoding.
        let mut bytes = event.header_bytes().to_vec();
        assert_eq!(bytes[65], 1);
        bytes.splice(65..66, [0x81, 0x00]);
        assert_eq!(
            Event::decode(&bytes, event.signature()),
            Err(EventError::NotCanonical)
        );

        let mut trailing = event.header_bytes().to_vec();
        trailing.push(0);
        assert_eq!(
            Event::decode(&trailing, event.signature()),
            Err(EventError::Malformed)
        );
    }

    #[test]
    fn another_protocol_version_is_reported_as_unsupported() {
        let mut owner = Author::new(1);
        let genesis = owner.genesis();
        let mut bytes = genesis.header_bytes().to_vec();
        bytes[0] = PROTOCOL_VERSION + 1;
        assert_eq!(
            Event::decode(&bytes, genesis.signature()),
            Err(EventError::UnsupportedVersion(PROTOCOL_VERSION + 1))
        );
    }

    #[test]
    fn structural_rules_are_enforced_when_signing() {
        let key = testkit::keypair(1);
        let genesis = Author::new(1).genesis();
        let valid = note_header(&key, &genesis);
        assert!(Event::sign(valid.clone(), &key).is_ok());

        let sign = |change: fn(&mut Header)| {
            let mut header = valid.clone();
            change(&mut header);
            Event::sign(header, &key).unwrap_err()
        };
        assert_eq!(sign(|h| h.prev = None), EventError::BadSequence);
        assert_eq!(sign(|h| h.seq = 0), EventError::BadSequence);
        assert_eq!(sign(|h| h.anchor = None), EventError::BadAnchor);
        assert_eq!(
            sign(|h| h.parents = vec![EventId([2; 32]), EventId([1; 32])]),
            EventError::BadReferences
        );
        assert_eq!(
            sign(|h| h.parents = vec![EventId([1; 32]), EventId([1; 32])]),
            EventError::BadReferences
        );
        assert_eq!(
            sign(|h| {
                h.payload = Some(PayloadRef {
                    hash: content_hash(b""),
                    len: u32::MAX,
                    key_epoch: 0,
                });
            }),
            EventError::BadPayloadLength
        );
        assert_eq!(
            Event::sign(valid, &testkit::keypair(2)),
            Err(EventError::AuthorMismatch)
        );
    }

    #[test]
    fn a_sequence_number_beyond_the_signed_64_bit_range_is_refused() {
        let key = testkit::keypair(1);
        let genesis = Author::new(1).genesis();
        let at = |seq| Header {
            seq,
            ..note_header(&key, &genesis)
        };
        assert!(Event::sign(at(i64::MAX as u64), &key).is_ok());
        assert_eq!(
            Event::sign(at(i64::MAX as u64 + 1), &key),
            Err(EventError::BadSequence)
        );
        assert_eq!(
            Event::sign(at(u64::MAX), &key),
            Err(EventError::BadSequence)
        );
    }

    #[test]
    fn genesis_must_found_the_goal_it_names() {
        let key = testkit::keypair(1);
        let genesis = Genesis {
            owner: key.public(),
            coordinator: key.public(),
            salt: [0; 16],
        };
        let header = |genesis: Genesis, goal| Header {
            version: PROTOCOL_VERSION,
            goal,
            author: key.public(),
            seq: 0,
            prev: None,
            anchor: None,
            parents: Vec::new(),
            at_ms: 0,
            payload: None,
            body: Body::Genesis(genesis),
        };
        assert!(Event::sign(header(genesis, genesis.goal_id()), &key).is_ok());
        assert_eq!(
            Event::sign(header(genesis, GoalId([0; 32])), &key),
            Err(EventError::BadAnchor)
        );

        let other = Genesis {
            salt: [1; 16],
            ..genesis
        };
        assert_ne!(other.goal_id(), genesis.goal_id());

        // Version 0: the owner is the coordinator.
        let split = Genesis {
            coordinator: testkit::keypair(2).public(),
            ..genesis
        };
        assert_eq!(
            Event::sign(header(split, split.goal_id()), &key),
            Err(EventError::BadAnchor)
        );
    }

    #[test]
    fn two_genesis_events_of_one_goal_are_distinct_conflicting_events() {
        // The goal identifier pins the genesis record, not the event: the
        // owner can sign a second genesis event for the same goal.
        let genesis = Author::new(1).genesis();
        let mut header = genesis.header().clone();
        header.at_ms += 1;
        let sibling = Event::sign(header, &testkit::keypair(1)).unwrap();
        assert_eq!(sibling.header().goal, genesis.header().goal);
        assert_eq!(sibling.header().seq, genesis.header().seq);
        assert_ne!(sibling.id(), genesis.id());
        // Both stay structurally valid; neither replaces the other.
        for event in [&genesis, &sibling] {
            assert_eq!(Event::from_wire(&event.to_wire()).as_ref(), Ok(event));
        }
    }

    #[test]
    fn from_stored_refuses_bytes_under_the_wrong_identifier() {
        let mut owner = Author::new(1);
        let genesis = owner.genesis();
        let event = note(&mut owner, genesis.header().goal, genesis.id());
        let wire = event.to_wire();
        assert_eq!(
            Event::from_stored(genesis.id(), wire.header, wire.signature),
            Err(EventError::IdMismatch)
        );
    }

    #[test]
    fn from_stored_runs_the_structural_check() {
        let key = testkit::keypair(1);
        let genesis = Author::new(1).genesis();
        let signature = genesis.signature();
        let stored = |header: Header| {
            let bytes = codec::encode(&header).unwrap();
            Event::from_stored(event_id(&bytes), bytes, signature)
        };
        assert_eq!(
            stored(Header {
                seq: i64::MAX as u64 + 1,
                ..note_header(&key, &genesis)
            }),
            Err(EventError::BadSequence)
        );
        assert_eq!(
            stored(Header {
                anchor: None,
                ..note_header(&key, &genesis)
            }),
            Err(EventError::BadAnchor)
        );
        assert_eq!(
            Event::from_stored(genesis.id(), Vec::new(), signature),
            Err(EventError::Malformed)
        );
    }

    #[test]
    fn a_payload_reference_admits_only_its_own_stored_bytes() {
        let goal = GoalId([1; 32]);
        let (payload, blob) = testkit::sealed_payload(&goal, 2, b"task text");
        assert!(payload.admits(&blob));
        assert!(
            !PayloadRef {
                len: payload.len - 1,
                ..payload
            }
            .admits(&blob)
        );
        let (_, other) = testkit::sealed_payload(&goal, 2, b"other text");
        assert!(!payload.admits(&other));
        // The header's epoch claim must match the epoch the object declares.
        assert!(
            !PayloadRef {
                key_epoch: 3,
                ..payload
            }
            .admits(&blob)
        );
    }

    #[test]
    fn a_payload_shorter_than_an_empty_sealed_object_is_refused() {
        let mut owner = Author::new(1);
        let genesis = owner.genesis();
        let header = |len| Header {
            version: PROTOCOL_VERSION,
            goal: genesis.header().goal,
            author: owner.key.public(),
            seq: 1,
            prev: Some(genesis.id()),
            anchor: Some(genesis.id()),
            parents: Vec::new(),
            at_ms: 0,
            payload: Some(PayloadRef {
                hash: BlobHash([1; 32]),
                len,
                key_epoch: 0,
            }),
            body: Body::Note {
                about: None,
                supersedes: None,
            },
        };
        let floor = seal::OVERHEAD_BYTES as u32;
        assert_eq!(header(floor).check(), Ok(()));
        assert_eq!(header(floor - 1).check(), Err(EventError::BadPayloadLength));
    }

    #[test]
    fn the_json_tag_of_every_body_is_its_kind() {
        for body in testkit::every_body() {
            let tag = match serde_json::to_value(&body).unwrap() {
                serde_json::Value::String(tag) => tag,
                serde_json::Value::Object(map) if map.len() == 1 => {
                    map.into_iter().next().unwrap().0
                }
                other => panic!("unexpected rendering {other}"),
            };
            assert_eq!(tag, body.kind());
        }
        assert_eq!(
            serde_json::to_value(CancelOutcome::Uncertain).unwrap(),
            "uncertain"
        );
        assert_eq!(serde_json::to_value(Doc::Plan).unwrap(), "plan");
    }

    #[test]
    fn decisions_are_exactly_the_variants_before_task_proposed() {
        let bodies = testkit::every_body();
        let first_contribution = bodies
            .iter()
            .position(|body| body.kind() == "task_proposed")
            .unwrap();
        for (index, body) in bodies.iter().enumerate() {
            assert_eq!(body.is_decision(), index < first_contribution, "{body:?}");
        }
        assert!(!Body::LeaveRequested.is_decision());
    }

    #[test]
    fn blobs_lists_every_named_object() {
        let header = Header {
            version: PROTOCOL_VERSION,
            goal: GoalId([0; 32]),
            author: PublicKey([0; 32]),
            seq: 1,
            prev: Some(EventId([0; 32])),
            anchor: Some(EventId([0; 32])),
            parents: Vec::new(),
            at_ms: 0,
            payload: Some(PayloadRef {
                hash: BlobHash([1; 32]),
                len: 3,
                key_epoch: 0,
            }),
            body: Body::ResultSubmitted {
                assignment: EventId([0; 32]),
                base: Some(BlobHash([2; 32])),
                patch: Some(BlobHash([3; 32])),
                artifacts: vec![BlobHash([4; 32])],
            },
        };
        assert_eq!(
            header.blobs(),
            [1, 2, 3, 4].map(|byte| BlobHash([byte; 32])).to_vec()
        );
    }
}
