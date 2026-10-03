//! Signed events: the durable, replicated record of a goal.
//!
//! An event is a [`Header`] encoded once, hashed and signed. The exact header
//! bytes are kept and relayed verbatim; nothing ever re-encodes a header in
//! order to verify it. User-written content (task text, notes, summaries)
//! lives in a detachable payload named by hash, so it can be withheld without
//! breaking the signed structure that membership, assignment and acceptance
//! depend on.
//!
//! [`Event::decode`] establishes structural validity only: the bytes are one
//! canonical header, internally consistent, and signed by the author it names.
//! Whether that author was a member, and whether the transition is allowed,
//! is decided by `locust-core` against the goal's history.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::PROTOCOL_VERSION;
use crate::codec;
use crate::crypto::{self, Keypair, domain};
use crate::id::{BlobHash, EndpointId, EventId, GoalId, PublicKey, Signature};
use crate::limits::{
    MAX_ARTIFACTS, MAX_DEPENDENCIES, MAX_HEADER_BYTES, MAX_PARENTS, MAX_PAYLOAD_BYTES,
};

/// The signed part of an event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    /// [`PROTOCOL_VERSION`]. Always the first byte on the wire.
    pub version: u8,
    pub goal: GoalId,
    /// The principal whose key signed this header.
    pub author: PublicKey,
    /// Position in the author's own log for this goal, starting at zero.
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
    /// Detachable user content for this event, if any.
    pub payload: Option<PayloadRef>,
    pub body: Body,
}

/// Names an event's detachable content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadRef {
    /// Digest of the stored bytes (ciphertext when `key_epoch` is set).
    pub hash: BlobHash,
    pub len: u32,
    /// Content-key epoch the bytes are sealed under; absent for plaintext.
    pub key_epoch: Option<u32>,
}

/// A position in one author's log.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorPoint {
    pub seq: u64,
    pub id: EventId,
}

/// The record that founds a goal. Its digest is the goal's identifier, so the
/// initial authority cannot be replaced under the same identifier.
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
pub enum Doc {
    Plan,
    Summary,
}

/// The typed, structural content of an event.
///
/// Variants are identified on the wire by declaration order: append new ones,
/// never reorder or remove. Variants up to and including `RevisionAccepted`
/// are coordinator decisions; the rest are contributions any current member
/// may author. Task, assignment, result, cancellation and revision are each
/// identified by the event that created them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Body {
    /// Founds the goal. Signed by the owner; the first decision in the chain.
    Genesis(Genesis),
    /// Admits a principal and binds it to the endpoint that may speak for it.
    MemberAdmitted {
        member: PublicKey,
        endpoint: EndpointId,
    },
    /// Removes a principal. Its contributions after `last_accepted` are kept
    /// as evidence but grant nothing.
    MemberRemoved {
        member: PublicKey,
        last_accepted: Option<AuthorPoint>,
    },
    /// Assigns a proposed task to a named member. A later assignment of the
    /// same task carries a higher `attempt` and supersedes this one.
    TaskAssigned {
        task: EventId,
        assignee: PublicKey,
        attempt: u32,
    },
    /// Withdraws an assignment's right to finalize. The executor answers with
    /// [`Body::CancelAcknowledged`]; until then the cancellation is only requested.
    CancelRequested { assignment: EventId },
    /// Accepts a submitted result, optionally advancing the accepted workspace
    /// head to the named manifest.
    ResultAccepted {
        result: EventId,
        head: Option<BlobHash>,
    },
    /// Declines a submitted result. The payload may say why.
    ResultRejected { result: EventId },
    /// Makes a document revision the accepted one. Valid only if the
    /// revision's `base` is the currently accepted revision.
    RevisionAccepted { revision: EventId },

    /// Proposes work. The payload is the task text; these fields are the
    /// authored policy every peer must see unchanged.
    TaskProposed {
        /// Manifest of the input snapshot the work starts from.
        input: Option<BlobHash>,
        depends_on: Vec<EventId>,
        /// Authored absolute deadline in Unix milliseconds; absent means none.
        deadline_ms: Option<u64>,
        /// Authored attempt budget; absent means unlimited.
        max_attempts: Option<u32>,
    },
    /// The assignee takes this exact assignment.
    AssignmentAccepted { assignment: EventId },
    /// The assignee will not do this assignment.
    AssignmentDeclined { assignment: EventId },
    /// Progress report; text in the payload.
    Progress { assignment: EventId },
    /// The executor's result. Completion is not acceptance: only
    /// [`Body::ResultAccepted`] changes the goal's accepted state.
    ResultSubmitted {
        assignment: EventId,
        /// Manifest the work was actually done against.
        base: Option<BlobHash>,
        /// Patch against `base`, if the result changes the workspace.
        patch: Option<BlobHash>,
        /// Further output objects.
        artifacts: Vec<BlobHash>,
    },
    /// The attempt ended without a result; reason in the payload.
    AttemptFailed { assignment: EventId },
    /// The executor's answer to a cancellation request.
    CancelAcknowledged {
        cancel: EventId,
        outcome: CancelOutcome,
    },
    /// An append-only note or finding, optionally about a task and optionally
    /// correcting an earlier note.
    Note {
        about: Option<EventId>,
        supersedes: Option<EventId>,
    },
    /// A proposed revision of a shared document, naming the accepted revision
    /// it was written against.
    Revision { doc: Doc, base: Option<EventId> },
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

    /// Stable name for logs, errors and command output.
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
        }
    }
}

/// Why bytes were not accepted as a structurally valid event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventError {
    TooLarge,
    /// The bytes do not decode as a header.
    Malformed,
    /// The bytes decode, but are not the one encoding of that header.
    NotCanonical,
    UnsupportedVersion(u8),
    /// `seq` and `prev` disagree.
    BadSequence,
    /// Too many parents, dependencies or artifacts, or parents out of order.
    BadReferences,
    /// A genesis event that does not match its own goal, or a later event
    /// with no anchor.
    BadAnchor,
    PayloadTooLarge,
    /// The signing key is not the header's author.
    AuthorMismatch,
    BadSignature,
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
            Self::BadSequence => f.write_str("sequence number and previous event disagree"),
            Self::BadReferences => f.write_str("references are out of order or too many"),
            Self::BadAnchor => f.write_str("anchor does not fit the event kind"),
            Self::PayloadTooLarge => f.write_str("payload exceeds the admitted size"),
            Self::AuthorMismatch => f.write_str("signing key is not the header's author"),
            Self::BadSignature => f.write_str("signature does not verify"),
        }
    }
}

impl std::error::Error for EventError {}

impl Header {
    /// Checks everything that can be checked without the goal's history.
    pub fn check(&self) -> Result<(), EventError> {
        if self.version != PROTOCOL_VERSION {
            return Err(EventError::UnsupportedVersion(self.version));
        }
        if (self.seq == 0) != self.prev.is_none() {
            return Err(EventError::BadSequence);
        }
        if self.parents.len() > MAX_PARENTS || !self.parents.is_sorted_by(|a, b| a < b) {
            return Err(EventError::BadReferences);
        }
        if let Some(payload) = &self.payload
            && payload.len as usize > MAX_PAYLOAD_BYTES
        {
            return Err(EventError::PayloadTooLarge);
        }
        match &self.body {
            Body::Genesis(genesis) => {
                let founds_this_goal = self.seq == 0
                    && self.anchor.is_none()
                    && self.parents.is_empty()
                    && self.author == genesis.owner
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
    #[serde(with = "codec::bytes")]
    pub header: Vec<u8>,
    pub signature: Signature,
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
    /// Encodes and signs a new event.
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

    /// Validates bytes received from a peer or a client.
    pub fn decode(header_bytes: &[u8], signature: Signature) -> Result<Self, EventError> {
        let header = decode_header(header_bytes)?;
        // One header has exactly one encoding, so one identifier.
        let canonical = codec::encode(&header).map_err(|_| EventError::Malformed)?;
        if canonical != header_bytes {
            return Err(EventError::NotCanonical);
        }
        header.check()?;
        let id = event_id(header_bytes);
        if !crypto::verify(&header.author, domain::EVENT_SIGNATURE, &id.0, &signature) {
            return Err(EventError::BadSignature);
        }
        Ok(Self {
            id,
            header,
            bytes: canonical.into_boxed_slice(),
            signature,
        })
    }

    /// Rebuilds an event from bytes this daemon validated with
    /// [`Event::decode`] before persisting them. Skips the canonical-form and
    /// signature checks; never use it on input from outside the local store.
    pub fn from_stored(header_bytes: Vec<u8>, signature: Signature) -> Result<Self, EventError> {
        let header = decode_header(&header_bytes)?;
        Ok(Self {
            id: event_id(&header_bytes),
            header,
            bytes: header_bytes.into_boxed_slice(),
            signature,
        })
    }

    pub fn from_wire(wire: &WireEvent) -> Result<Self, EventError> {
        Self::decode(&wire.header, wire.signature)
    }

    pub fn to_wire(&self) -> WireEvent {
        WireEvent {
            header: self.bytes.to_vec(),
            signature: self.signature,
        }
    }

    pub fn id(&self) -> EventId {
        self.id
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The exact bytes that were hashed and signed.
    pub fn header_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn signature(&self) -> Signature {
        self.signature
    }
}

fn decode_header(bytes: &[u8]) -> Result<Header, EventError> {
    if bytes.len() > MAX_HEADER_BYTES {
        return Err(EventError::TooLarge);
    }
    match bytes.first() {
        Some(&PROTOCOL_VERSION) => {}
        Some(&version) => return Err(EventError::UnsupportedVersion(version)),
        None => return Err(EventError::Malformed),
    }
    codec::decode(bytes).map_err(|_| EventError::Malformed)
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
        let stored = Event::from_stored(wire.header, wire.signature).unwrap();
        assert_eq!(stored, event);
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
        let error = Event::decode(&bytes, event.signature()).unwrap_err();
        assert!(matches!(
            error,
            EventError::NotCanonical | EventError::Malformed
        ));

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
        let mut owner = Author::new(1);
        let genesis = owner.genesis();
        let goal = genesis.header().goal;
        let valid = Header {
            version: PROTOCOL_VERSION,
            goal,
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
        };
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
                    key_epoch: None,
                });
            }),
            EventError::PayloadTooLarge
        );
        assert_eq!(
            Event::sign(valid, &testkit::keypair(2)),
            Err(EventError::AuthorMismatch)
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
        let header = |goal| Header {
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
        assert!(Event::sign(header(genesis.goal_id()), &key).is_ok());
        assert_eq!(
            Event::sign(header(GoalId([0; 32])), &key),
            Err(EventError::BadAnchor)
        );

        let other = Genesis {
            salt: [1; 16],
            ..genesis
        };
        assert_ne!(other.goal_id(), genesis.goal_id());
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
                key_epoch: None,
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
