//! Frames exchanged between two daemons, and the rule that reconciles their
//! copies of a goal.
//!
//! # What the transport provides
//!
//! The transport is constructed from a 32-byte endpoint secret. It exposes to
//! the daemon the local endpoint identifier, the local contact hints, the
//! authenticated remote endpoint identifier of each link, and notifications
//! when a link comes up or goes down. Each exchange is one bidirectional
//! stream of length-prefixed frames ([`crate::codec`]), each one an encoded
//! [`SyncMessage`]. The transport decides nothing about membership: whether a
//! remote endpoint speaks for a member, which is whether a `MemberAdmitted`
//! decision of the goal binds a current member to it, is decided by the
//! daemon for every request.
//!
//! # Exchanges
//!
//! An exchange is one bidirectional stream opened by the initiator. A daemon
//! that wants something opens its own exchange, so two exchanges never share
//! a stream. The initiator sends [`SyncMessage::Hello`], then requests; the
//! responder answers each request in order and sends nothing unasked:
//!
//! - `Hello` is not answered when accepted. A `Hello` of another version is
//!   answered with `Refused(UnsupportedVersion)` and ends the exchange.
//! - `Join` is answered with the responder's `Frontier` once the key is
//!   admitted, also when the same key repeats it, and otherwise with
//!   `Refused(InvitationRefused)`.
//! - `Frontier(mine)` is answered with zero or more `Events` and `Inventory`
//!   frames, chosen by the reconciliation rule below for every author either
//!   side holds, then the responder's own `Frontier`. The initiator then
//!   applies the same rule to that frontier and pushes the `Events` the
//!   responder lacks. Pushed `Events` are not answered.
//! - `InventoryRequest` is answered with one `Inventory`.
//! - `EventRequest` is answered with one `Events` frame for each run of
//!   [`MAX_EVENTS_PER_BATCH`] requested identifiers, in order, carrying the
//!   events of that run the responder holds (possibly none). The initiator
//!   knows how many frames to read without the responder holding them all. An
//!   empty identifier list receives no answer.
//! - `KeyRequest` is answered with `Key`, with `Refused(NotAMember)` when the
//!   remote endpoint does not speak for a current member, or with
//!   `Refused(KeyUnavailable)` when the responder holds no key for that epoch.
//! - `BlobRequest` is answered with `BlobChunk` frames that run contiguously
//!   from `offset` to `total` (one empty chunk when `offset` equals `total`),
//!   or with `BlobUnavailable` when the responder does not serve the object or
//!   `offset` is past its end.
//! - `Done` is not answered; it ends the exchange.
//!
//! Before admission, frames are read at [`MAX_HELLO_FRAME_BYTES`]. `Hello`
//! identifies the goal; `Join` can establish admission. Ordinary requests from
//! an endpoint that does not speak for a member receive `Refused(NotAMember)`,
//! including requests naming an unknown goal. After membership is established,
//! the per-link limit rises to [`MAX_PEER_FRAME_BYTES`]. Dialed links can read
//! admission history at the peer limit, but neither side sends held frontiers,
//! inventory, headers, keys or content until it verifies the recipient's
//! current membership from canonical goal state. In particular, a joining
//! initiator sends an empty frontier while that verification is pending.
//!
//! A separate evidence exchange is `Hello`, `HaltProof([a, b])`, `Done`.
//! The receiver authenticates the sender as a historical contact of this known
//! goal (or its pending inviter), verifies both signatures and checks that the
//! two different events name the goal's coordinator at the same sequence. Once
//! that historical eligibility is established, the shell may raise only the
//! evidence frame limit to `2 * (MAX_HEADER_BYTES + 128)`. This permission grants
//! no ordinary membership, inventory, content or key access. It lets a halted
//! replica deliver coordinator equivocation to contacts whose admission the
//! fork excluded. Proofs are retained through the normal durable commit path.
//!
//! The responder advances its answer lazily, at most one frame per transport
//! credit, and intake waits until that complete answer drains. A frame that
//! [`SyncMessage::decode`] refuses is answered with that refusal and ends the
//! exchange. Request pipelining cannot accumulate materialized answers behind
//! a stalled writer.
//!
//! A stream that ends without `Done` is an aborted exchange, not a finished
//! one; what arrived before stays valid, since every event stands alone. The
//! sender of an exchange's last frame, a `Refused` in particular, waits under
//! the shared exchange idle deadline for the transport to acknowledge it before it drops the link,
//! because a frame queued behind a dropped link is not delivered. Stream
//! reset codes and connection close codes carry no protocol meaning and
//! are zero; reasons travel as `Refused` frames.
//!
//! # Reconciliation
//!
//! Each side states, per author, the [`AuthorFrontier`]: how many consecutive
//! positions it holds from zero and a running digest of exactly which events
//! it holds below that. Equal `(next_seq, digest)` means equal retained sets.
//! The rule for answering a peer's entry `(n, d)` for one author
//! ([`AuthorFrontier::is_prefix_of`]):
//!
//! - If I hold at least `n` positions and my digest over my points below `n`
//!   equals `d`, the peer holds exactly my prefix: I send my events at
//!   positions `n` and above.
//! - If my contiguous prefix is shorter and I hold no points beyond it, I
//!   defer inventory and send my frontier. The longer side verifies that
//!   prefix and sends its suffix, or requests inventory when it differs.
//! - Otherwise I send one page of `Inventory` for that author. The peer
//!   requests missing identifiers, sends what I lack, and requests later
//!   inventory pages as needed. Unequal-length forks still reconcile.
//!
//! An author missing from a frontier is held from nothing: the entry
//! `(0, EMPTY_LOG_DIGEST)`, a prefix of every history. Conflicting events at
//! one position (a forked author) make the digests differ even when the
//! counts agree, so a fork is always found and both versions reach both
//! sides. Live pushes are hints; a missed one loses nothing, because the next
//! exchange of frontiers finds the gap.
//!
//! # Content keys and transfer
//!
//! Every content object a goal's events name is sealed, and its
//! [`BlobHash`] is the plain BLAKE3 of the sealed bytes, so storage and
//! transfer verify objects without keys. The key epoch of an event is the
//! number of `MemberRemoved` decisions at or before its anchor. Keys travel
//! only as `Key` frames over a link whose remote endpoint speaks for a current
//! member; any member holding a key may serve it, a member admitted later is
//! given every earlier epoch so it can read history, and a removed member is
//! refused with `NotAMember`.
//!
//! An object travels as `BlobChunk` frames of at most [`BLOB_CHUNK_BYTES`].
//! The receiver stages the chunks durably ([`crate::store::Store::stage_blob`])
//! and, after a reconnect or restart, resumes with a `BlobRequest` at the
//! staged length. When the object is complete it is verified against its hash
//! before it is promoted ([`crate::store::Store::finish_blob`]); a mismatch
//! discards the staged bytes. The receiver checks advertised lengths against
//! signed associations before staging and checks sealed metadata once its
//! prefix is present. An unavailable nonzero resume gets one retry at zero;
//! unavailability alone never erases shared staging. An incompatible old stage
//! is replaced only by a zero-offset stream carrying admissible metadata.
//!
//! [`MAX_HELLO_FRAME_BYTES`]: crate::limits::MAX_HELLO_FRAME_BYTES
//! [`MAX_PEER_FRAME_BYTES`]: crate::limits::MAX_PEER_FRAME_BYTES

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::PROTOCOL_VERSION;
use crate::codec;
use crate::crypto::{ContentKey, domain, domain_hasher};
use crate::event::{AuthorPoint, WireEvent};
use crate::id::{BlobHash, EventId, GoalId, PublicKey};
use crate::invite::JoinRequest;
use crate::limits::{
    BLOB_CHUNK_BYTES, MAX_BLOB_BYTES, MAX_EVENTS_PER_BATCH, MAX_FRONTIER_AUTHORS, MAX_HEADER_BYTES,
    MAX_INVENTORY_POINTS,
};

/// The log digest of an author holding nothing: 32 zero bytes, where every
/// running chain starts.
pub const EMPTY_LOG_DIGEST: [u8; 32] = [0; 32];

/// Extends a running log digest by one point:
/// `domain_hash(LOG_DIGEST, state || seq as u64 little-endian || id)`.
///
/// A holder that keeps the state after every point of an author answers the
/// digest of any prefix by lookup instead of rehashing.
pub fn log_digest_step(state: [u8; 32], point: &AuthorPoint) -> [u8; 32] {
    let mut hasher = domain_hasher(domain::LOG_DIGEST);
    hasher.update(&state);
    hasher.update(&point.seq.to_le_bytes());
    hasher.update(&point.id.0);
    *hasher.finalize().as_bytes()
}

/// The running digest of `points` in the order given, starting from
/// [`EMPTY_LOG_DIGEST`]: the fold of [`log_digest_step`].
pub fn log_digest<'a>(points: impl IntoIterator<Item = &'a AuthorPoint>) -> [u8; 32] {
    points.into_iter().fold(EMPTY_LOG_DIGEST, log_digest_step)
}

/// How much of one author's log a peer holds, and exactly which events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorFrontier {
    /// The author this entry is about.
    pub author: PublicKey,
    /// Number of consecutive positions held, counted from zero. A position
    /// holding several conflicting events counts once.
    pub next_seq: u64,
    /// [`log_digest`] over every retained event of `author` with a position
    /// below `next_seq`, in ascending (position, identifier) order. Two
    /// entries with equal `next_seq` and `digest` describe equal retained sets.
    pub digest: [u8; 32],
}

impl AuthorFrontier {
    /// The entry for `author` from its retained points in ascending
    /// (position, identifier) order, as [`crate::store::Store::author_log`]
    /// returns them. Counting stops at the first missing position; points
    /// past it are not covered. Points in another order give a meaningless
    /// entry, never a panic.
    pub fn from_points(author: PublicKey, points: &[AuthorPoint]) -> Self {
        let mut next_seq = 0u64;
        let mut digest = EMPTY_LOG_DIGEST;
        for point in points {
            if point.seq == next_seq {
                next_seq += 1;
            } else if next_seq.checked_sub(1) != Some(point.seq) {
                // A missing position: nothing after it is consecutive.
                break;
            }
            digest = log_digest_step(digest, point);
        }
        Self {
            author,
            next_seq,
            digest,
        }
    }

    /// The reconciliation rule, applied by the holder of `mine` (its points
    /// of this author, ascending as for [`AuthorFrontier::from_points`]) to
    /// this entry, which a peer sent.
    ///
    /// True when the holder holds at least `next_seq` positions and its digest
    /// over its points below `next_seq` equals `digest`: the peer holds
    /// exactly that prefix, so the holder sends its events at positions
    /// `next_seq` and above. False when the histories diverge or the holder is
    /// behind: the holder sends an `Inventory` of `mine`.
    ///
    /// This computes the prefix digest from the points. A holder that keeps
    /// the [`log_digest_step`] state after every point answers the same
    /// question by looking up the state after its last point below `next_seq`.
    pub fn is_prefix_of(&self, mine: &[AuthorPoint]) -> bool {
        let below = mine.partition_point(|point| point.seq < self.next_seq);
        Self::from_points(self.author, &mine[..below]) == *self
    }
}

/// What a peer holds of a goal: an entry per author, strictly ascending by
/// author.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frontier {
    /// At most [`MAX_FRONTIER_AUTHORS`] entries, strictly ascending by author.
    pub authors: Vec<AuthorFrontier>,
}

impl Frontier {
    /// The entry for `author`. An author that is not listed is held from
    /// nothing: `next_seq` 0 and [`EMPTY_LOG_DIGEST`], which is what
    /// [`AuthorFrontier::from_points`] gives for no points.
    pub fn get(&self, author: &PublicKey) -> AuthorFrontier {
        self.authors
            .binary_search_by(|entry| entry.author.cmp(author))
            .map_or_else(
                |_| AuthorFrontier::from_points(*author, &[]),
                |index| self.authors[index],
            )
    }

    /// The first position of `author` this frontier lacks.
    pub fn next_seq(&self, author: &PublicKey) -> u64 {
        self.get(author).next_seq
    }
}

/// Why a peer will not serve a request. Refusals are always sent, never
/// expressed by silence. Rendered in text as `snake_case` names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    /// The `Hello` names a protocol version this daemon does not speak.
    UnsupportedVersion,
    /// The remote endpoint does not speak for a current member of the goal.
    /// Also the answer for a goal the responder does not know, so the two
    /// cannot be told apart.
    NotAMember,
    /// The invitation is unknown, expired or bound to another key.
    InvitationRefused,
    /// A frame, batch or object exceeded a published or configured limit.
    LimitExceeded,
    /// A frame did not decode, was inconsistent, or broke the exchange order.
    ProtocolError,
    /// The responder holds no content key for the requested epoch; another
    /// member may.
    KeyUnavailable,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedVersion => "the peer does not speak this protocol version",
            Self::NotAMember => "the endpoint does not speak for a member of the goal",
            Self::InvitationRefused => "the invitation is unknown, expired or bound to another key",
            Self::LimitExceeded => "a frame, batch or object exceeds a limit",
            Self::ProtocolError => "a frame is malformed or out of order",
            Self::KeyUnavailable => "the peer holds no key for that epoch",
        })
    }
}

impl std::error::Error for Refusal {}

/// One frame of an exchange; see the module documentation for which frame
/// answers which. Variants are identified by declaration index, so they are
/// only ever appended. `Hello` stays the first variant and `version` its
/// first field in every protocol version, so a peer always reads the version
/// before anything else.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncMessage {
    /// Opens an exchange about one goal; the initiator's first frame.
    Hello { version: u8, goal: GoalId },
    /// Redeems an invitation. Eligible historical nonmembers may also send
    /// [`SyncMessage::HaltProof`].
    Join(JoinRequest),
    /// The responder will not serve the request this answers.
    Refused(Refusal),
    /// What the sender holds of the goal.
    Frontier(Frontier),
    /// At most [`MAX_EVENTS_PER_BATCH`] events, each header at most
    /// [`MAX_HEADER_BYTES`], in an order where each author's log is ascending.
    Events(Vec<WireEvent>),
    /// Asks for the responder's points of `author` strictly after `after` in
    /// (position, identifier) order, or from the first when `after` is `None`.
    InventoryRequest {
        author: PublicKey,
        after: Option<AuthorPoint>,
    },
    /// At most [`MAX_INVENTORY_POINTS`] points of `author` the sender holds,
    /// strictly ascending by (position, identifier). `more` says further
    /// points follow the last one; ask for them with an `InventoryRequest`
    /// after it.
    Inventory {
        author: PublicKey,
        points: Vec<AuthorPoint>,
        more: bool,
    },
    /// Asks for at most [`MAX_INVENTORY_POINTS`] events by identifier.
    EventRequest(Vec<EventId>),
    /// Asks for the goal's content key of one epoch.
    KeyRequest { epoch: u32 },
    /// The goal's content key of one epoch. Debug and text forms never show
    /// the key.
    Key { epoch: u32, key: ContentKey },
    /// Asks for a stored object from byte `offset`, which resumes an
    /// interrupted transfer.
    BlobRequest { hash: BlobHash, offset: u64 },
    /// At most [`BLOB_CHUNK_BYTES`] stored bytes of object `hash` starting at
    /// `offset`. `total` is the object's whole stored length, at most
    /// [`MAX_BLOB_BYTES`], and the chunk ends at or before it.
    BlobChunk {
        hash: BlobHash,
        offset: u64,
        total: u64,
        #[serde(with = "codec::bytes")]
        bytes: Vec<u8>,
    },
    /// The responder does not hold, has withdrawn, or cannot serve the asked
    /// range of this object.
    BlobUnavailable(BlobHash),
    /// Ends the exchange.
    Done,
    /// Exactly two conflicting coordinator events for this known goal. This
    /// proof-only frame may reach a historical participant without admitting
    /// it to content, keys, inventory or ordinary event reconciliation. The
    /// receiver validates both signatures and the same coordinator position.
    HaltProof([WireEvent; 2]),
}

/// Declaration index of [`SyncMessage::Hello`], its first encoded byte.
const HELLO: u8 = 0;

impl SyncMessage {
    /// Decodes one received frame and checks what decoding cannot: a `Hello`
    /// of another protocol version is refused with `UnsupportedVersion`,
    /// before the rest of the frame is decoded; a count above a limit (events
    /// per batch, header size, frontier authors, inventory points, event
    /// request size, chunk size, object size) with `LimitExceeded`; bytes that
    /// do not decode, a frontier not strictly ascending by author, an
    /// inventory not strictly ascending by point, or a chunk that ends past
    /// its object's length with `ProtocolError`.
    ///
    /// The caller has already refused a frame longer than the read limit, which
    /// bounds what decoding can allocate.
    pub fn decode(bytes: &[u8]) -> Result<Self, Refusal> {
        if let [HELLO, version, ..] = bytes
            && *version != PROTOCOL_VERSION
        {
            return Err(Refusal::UnsupportedVersion);
        }
        let message: Self = codec::decode(bytes).map_err(|_| Refusal::ProtocolError)?;
        message.check()?;
        Ok(message)
    }

    fn check(&self) -> Result<(), Refusal> {
        match self {
            Self::Hello { version, .. } if *version != PROTOCOL_VERSION => {
                Err(Refusal::UnsupportedVersion)
            }
            Self::Frontier(frontier) if frontier.authors.len() > MAX_FRONTIER_AUTHORS => {
                Err(Refusal::LimitExceeded)
            }
            Self::Frontier(frontier)
                if !frontier.authors.is_sorted_by(|a, b| a.author < b.author) =>
            {
                Err(Refusal::ProtocolError)
            }
            Self::Events(events)
                if events.len() > MAX_EVENTS_PER_BATCH
                    || events
                        .iter()
                        .any(|event| event.header.len() > MAX_HEADER_BYTES) =>
            {
                Err(Refusal::LimitExceeded)
            }
            Self::HaltProof(events)
                if events
                    .iter()
                    .any(|event| event.header.len() > MAX_HEADER_BYTES) =>
            {
                Err(Refusal::LimitExceeded)
            }
            Self::Inventory { points, .. } if points.len() > MAX_INVENTORY_POINTS => {
                Err(Refusal::LimitExceeded)
            }
            Self::Inventory { points, .. }
                if !points.is_sorted_by(|a, b| (a.seq, a.id) < (b.seq, b.id)) =>
            {
                Err(Refusal::ProtocolError)
            }
            Self::EventRequest(ids) if ids.len() > MAX_INVENTORY_POINTS => {
                Err(Refusal::LimitExceeded)
            }
            Self::BlobChunk { total, bytes, .. }
                if bytes.len() > BLOB_CHUNK_BYTES || *total > MAX_BLOB_BYTES as u64 =>
            {
                Err(Refusal::LimitExceeded)
            }
            Self::BlobChunk {
                offset,
                total,
                bytes,
                ..
            } if offset
                .checked_add(bytes.len() as u64)
                .is_none_or(|end| end > *total) =>
            {
                Err(Refusal::ProtocolError)
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{content_hash, domain_hash};
    use crate::event::{Body, Event};
    use crate::id::{EndpointId, Signature};
    use crate::limits::{MAX_HELLO_FRAME_BYTES, MAX_PEER_FRAME_BYTES};
    use crate::store::{Commit, MemStore, Store};
    use crate::testkit::{self, Author};

    fn point(event: &Event) -> AuthorPoint {
        AuthorPoint {
            seq: event.header().seq,
            id: event.id(),
        }
    }

    fn at(seq: u64, id: u8) -> AuthorPoint {
        AuthorPoint {
            seq,
            id: EventId([id; 32]),
        }
    }

    fn note() -> Body {
        Body::Note {
            about: None,
            supersedes: None,
        }
    }

    /// One author's points in a goal, as a holder reads them from its store.
    fn points_in(store: &MemStore, goal: &GoalId, author: &PublicKey) -> Vec<AuthorPoint> {
        store
            .author_log(goal, author, None, usize::MAX)
            .unwrap()
            .iter()
            .map(point)
            .collect()
    }

    fn commit(store: &mut MemStore, events: &[Event]) {
        store
            .commit(&Commit {
                events: events.to_vec(),
                ..Commit::default()
            })
            .unwrap();
    }

    #[test]
    fn the_digest_step_is_the_documented_hash() {
        let state = [7; 32];
        let point = AuthorPoint {
            seq: 0x0102_0304_0506_0708,
            id: EventId([9; 32]),
        };
        let mut input = state.to_vec();
        input.extend_from_slice(&[8, 7, 6, 5, 4, 3, 2, 1]);
        input.extend_from_slice(&[9; 32]);
        assert_eq!(
            log_digest_step(state, &point),
            domain_hash(domain::LOG_DIGEST, &input)
        );
    }

    #[test]
    fn the_fold_equals_the_step_by_step_chain() {
        let points = [at(0, 1), at(1, 2), at(1, 3), at(2, 4)];
        let mut state = EMPTY_LOG_DIGEST;
        for point in &points {
            state = log_digest_step(state, point);
        }
        assert_eq!(log_digest(&points), state);
        assert_eq!(log_digest(&[]), EMPTY_LOG_DIGEST);
    }

    #[test]
    fn the_digest_is_order_sensitive_and_binds_position_and_identifier() {
        let digest = log_digest(&[at(1, 2), at(1, 3)]);
        assert_ne!(digest, log_digest(&[at(1, 3), at(1, 2)]));
        assert_ne!(digest, log_digest(&[at(1, 2), at(2, 3)]));
        assert_ne!(digest, log_digest(&[at(1, 2), at(1, 4)]));
        assert_ne!(log_digest(&[at(0, 1)]), EMPTY_LOG_DIGEST);
    }

    #[test]
    fn a_frontier_counts_consecutive_positions_from_zero() {
        let author = PublicKey([1; 32]);
        let entry = |points: &[AuthorPoint]| AuthorFrontier::from_points(author, points);

        assert_eq!(entry(&[]).next_seq, 0);
        assert_eq!(entry(&[]).digest, EMPTY_LOG_DIGEST);
        let straight = [at(0, 1), at(1, 2), at(2, 3)];
        assert_eq!(entry(&straight).next_seq, 3);
        assert_eq!(entry(&straight).digest, log_digest(&straight));

        // Nothing at position zero: nothing is consecutive.
        assert_eq!(entry(&[at(1, 2), at(2, 3)]), entry(&[]));
    }

    #[test]
    fn a_fork_counts_once_and_both_branches_are_digested() {
        let author = PublicKey([1; 32]);
        let forked = [at(0, 1), at(1, 2), at(1, 3), at(2, 4)];
        let entry = AuthorFrontier::from_points(author, &forked);
        assert_eq!(entry.next_seq, 3);
        assert_eq!(entry.digest, log_digest(&forked));

        // A fork at the last held position is covered too.
        let tip = [at(0, 1), at(1, 2), at(1, 3)];
        assert_eq!(AuthorFrontier::from_points(author, &tip).next_seq, 2);
        assert_eq!(
            AuthorFrontier::from_points(author, &tip).digest,
            log_digest(&tip)
        );
    }

    #[test]
    fn points_past_a_gap_are_not_covered() {
        let author = PublicKey([1; 32]);
        let below = [at(0, 1), at(1, 2), at(1, 3)];
        let mut held = below.to_vec();
        held.extend([at(3, 5), at(4, 6)]);
        let entry = AuthorFrontier::from_points(author, &held);
        assert_eq!(entry.next_seq, 2);
        assert_eq!(entry, AuthorFrontier::from_points(author, &below));
    }

    #[test]
    fn frontiers_built_from_a_store_count_consecutive_events_per_author() {
        let mut owner = Author::new(1);
        let mut member = Author::new(2);
        let genesis = owner.genesis();
        let goal = genesis.header().goal;
        let anchor = Some(genesis.id());
        let owner_first = owner.event(goal, anchor, note());
        let owner_second = owner.event(goal, anchor, note());
        let owner_third = owner.event(goal, anchor, note());
        let member_first = member.event(goal, anchor, note());
        let member_second = member.event(goal, anchor, note());
        let (owner_key, member_key) = (owner.key.public(), member.key.public());

        let frontier = |store: &MemStore| {
            let mut authors: Vec<AuthorFrontier> = [owner_key, member_key]
                .iter()
                .map(|author| {
                    AuthorFrontier::from_points(*author, &points_in(store, &goal, author))
                })
                .collect();
            authors.sort_by_key(|entry| entry.author);
            Frontier { authors }
        };

        // The owner's third event and the member's second arrive ahead of a gap.
        let mut store = MemStore::new();
        commit(
            &mut store,
            &[
                genesis.clone(),
                owner_first.clone(),
                owner_third,
                member_second,
            ],
        );
        let before = frontier(&store);
        assert_eq!(before.next_seq(&owner_key), 2);
        assert_eq!(before.next_seq(&member_key), 0);
        assert_eq!(
            before.get(&owner_key).digest,
            log_digest(&[point(&genesis), point(&owner_first)])
        );
        assert_eq!(before.get(&member_key).digest, EMPTY_LOG_DIGEST);

        commit(&mut store, &[owner_second, member_first]);
        let after = frontier(&store);
        assert_eq!(after.next_seq(&owner_key), 4);
        assert_eq!(after.next_seq(&member_key), 2);
    }

    #[test]
    fn an_unlisted_author_is_held_from_nothing() {
        let entry = |author: u8, next_seq: u64| AuthorFrontier {
            author: PublicKey([author; 32]),
            next_seq,
            digest: [author; 32],
        };
        let frontier = Frontier {
            authors: vec![entry(1, 4), entry(3, 9)],
        };
        assert_eq!(frontier.next_seq(&PublicKey([1; 32])), 4);
        assert_eq!(frontier.next_seq(&PublicKey([2; 32])), 0);
        assert_eq!(frontier.next_seq(&PublicKey([3; 32])), 9);
        assert_eq!(frontier.get(&PublicKey([3; 32])), entry(3, 9));
        assert_eq!(
            frontier.get(&PublicKey([2; 32])),
            AuthorFrontier::from_points(PublicKey([2; 32]), &[])
        );
    }

    /// Lane B's B-R3 reproduction, turned into the required behavior: two
    /// stores with the same genesis and different signed decisions at
    /// position 1 used to advertise identical frontiers and never learn of
    /// the fork.
    #[test]
    fn equal_length_divergent_histories_are_detected_and_reconciled() {
        let mut owner = Author::new(1);
        let mut twin = Author::new(1);
        let root = owner.genesis();
        assert_eq!(twin.genesis(), root);
        let goal = root.header().goal;
        let author = root.header().author;
        let admit = |signer: &mut Author, endpoint: u8| {
            signer.event(
                goal,
                Some(root.id()),
                Body::MemberAdmitted {
                    member: testkit::keypair(3).public(),
                    endpoint: EndpointId([endpoint; 32]),
                },
            )
        };
        let left_decision = admit(&mut owner, 1);
        let right_decision = admit(&mut twin, 2);
        assert_ne!(left_decision.id(), right_decision.id());

        let mut left = MemStore::new();
        let mut right = MemStore::new();
        commit(&mut left, &[root.clone(), left_decision.clone()]);
        commit(&mut right, &[root.clone(), right_decision.clone()]);
        let left_points = points_in(&left, &goal, &author);
        let right_points = points_in(&right, &goal, &author);
        let left_entry = AuthorFrontier::from_points(author, &left_points);
        let right_entry = AuthorFrontier::from_points(author, &right_points);

        assert_eq!(left_entry.next_seq, 2);
        assert_eq!(right_entry.next_seq, 2);
        assert_ne!(left_entry, right_entry);
        // Neither side holds exactly the other's prefix, so the rule tells
        // each to send an inventory instead of an empty suffix.
        assert!(!right_entry.is_prefix_of(&left_points));
        assert!(!left_entry.is_prefix_of(&right_points));

        // Each inventory names the event the other lacks. Once both hold the
        // fork, they agree and nothing more is sent.
        commit(&mut left, &[right_decision]);
        commit(&mut right, &[left_decision]);
        let left_points = points_in(&left, &goal, &author);
        let right_points = points_in(&right, &goal, &author);
        assert_eq!(left_points, right_points);
        let agreed = AuthorFrontier::from_points(author, &left_points);
        assert_eq!(agreed, AuthorFrontier::from_points(author, &right_points));
        assert_eq!(agreed.next_seq, 2);
        assert!(agreed.is_prefix_of(&right_points));
    }

    #[test]
    fn a_peer_that_is_simply_behind_is_recognized_by_the_prefix_digest() {
        let mut writer = Author::new(1);
        let root = writer.genesis();
        let goal = root.header().goal;
        let author = writer.key.public();
        let mut events = vec![root.clone()];
        for _ in 0..4 {
            events.push(writer.event(goal, Some(root.id()), note()));
        }
        let ahead: Vec<AuthorPoint> = events.iter().map(point).collect();
        let behind = &ahead[..3];
        let ahead_entry = AuthorFrontier::from_points(author, &ahead);
        let behind_entry = AuthorFrontier::from_points(author, behind);

        // The holder ahead finds the peer's entry is its own prefix and sends
        // positions 3 and 4. A holder keeping every chain state answers by
        // lookup: the state after its third point.
        assert!(behind_entry.is_prefix_of(&ahead));
        let states: Vec<[u8; 32]> = ahead
            .iter()
            .scan(EMPTY_LOG_DIGEST, |state, point| {
                *state = log_digest_step(*state, point);
                Some(*state)
            })
            .collect();
        assert_eq!(behind_entry.digest, states[2]);
        assert_eq!(ahead_entry.digest, states[4]);

        // The holder behind cannot vouch for positions it lacks.
        assert!(!ahead_entry.is_prefix_of(behind));
        // Equal entries: nothing is missing on either side.
        assert!(ahead_entry.is_prefix_of(&ahead));
        // A peer that lists nothing holds the empty prefix of everything.
        assert!(Frontier::default().get(&author).is_prefix_of(&ahead));

        // A fork below the peer's length is divergence, not a prefix.
        let mut twin = Author::new(1);
        twin.genesis();
        let sibling = twin.event(
            goal,
            Some(root.id()),
            Body::Note {
                about: Some(root.id()),
                supersedes: None,
            },
        );
        let mut forked = ahead.clone();
        forked.push(point(&sibling));
        forked.sort_by_key(|point| (point.seq, point.id));
        assert!(!behind_entry.is_prefix_of(&forked));
    }

    #[test]
    fn decode_round_trips_every_frame() {
        let join: JoinRequest = codec::decode(&[3; 192]).unwrap();
        let key: ContentKey = codec::decode(&[4; 32]).unwrap();
        let messages = [
            SyncMessage::Hello {
                version: PROTOCOL_VERSION,
                goal: GoalId([1; 32]),
            },
            SyncMessage::Join(join),
            SyncMessage::Refused(Refusal::KeyUnavailable),
            SyncMessage::Frontier(Frontier {
                authors: vec![AuthorFrontier::from_points(PublicKey([1; 32]), &[at(0, 1)])],
            }),
            SyncMessage::Events(vec![WireEvent {
                header: vec![0; 10],
                signature: Signature([2; 64]),
            }]),
            SyncMessage::InventoryRequest {
                author: PublicKey([1; 32]),
                after: Some(at(4, 4)),
            },
            SyncMessage::Inventory {
                author: PublicKey([1; 32]),
                points: vec![at(0, 1), at(1, 2), at(1, 3)],
                more: true,
            },
            SyncMessage::EventRequest(vec![EventId([5; 32])]),
            SyncMessage::KeyRequest { epoch: 2 },
            SyncMessage::Key { epoch: 2, key },
            SyncMessage::BlobRequest {
                hash: BlobHash([6; 32]),
                offset: 9,
            },
            SyncMessage::BlobChunk {
                hash: BlobHash([6; 32]),
                offset: 9,
                total: 12,
                bytes: vec![1, 2, 3],
            },
            SyncMessage::BlobUnavailable(BlobHash([6; 32])),
            SyncMessage::Done,
            SyncMessage::HaltProof(std::array::from_fn(|_| WireEvent {
                header: vec![0; 10],
                signature: Signature([2; 64]),
            })),
        ];
        for (index, message) in messages.into_iter().enumerate() {
            let bytes = codec::encode(&message).unwrap();
            // Declaration order is the wire tag.
            assert_eq!(usize::from(bytes[0]), index);
            assert_eq!(SyncMessage::decode(&bytes), Ok(message));
        }
    }

    #[test]
    fn another_hello_version_is_refused_before_the_rest_is_decoded() {
        let hello = SyncMessage::Hello {
            version: PROTOCOL_VERSION,
            goal: GoalId([1; 32]),
        };
        let mut bytes = codec::encode(&hello).unwrap();
        bytes[1] = PROTOCOL_VERSION + 1;
        assert_eq!(
            SyncMessage::decode(&bytes),
            Err(Refusal::UnsupportedVersion)
        );
        // A later version may lay out the rest of its hello differently.
        assert_eq!(
            SyncMessage::decode(&[HELLO, PROTOCOL_VERSION + 1, 0xff]),
            Err(Refusal::UnsupportedVersion)
        );
    }

    #[test]
    fn undecodable_frames_are_protocol_errors() {
        let mut done = codec::encode(&SyncMessage::Done).unwrap();
        done.push(0);
        assert_eq!(SyncMessage::decode(&done), Err(Refusal::ProtocolError));
        assert_eq!(SyncMessage::decode(&[]), Err(Refusal::ProtocolError));
        assert_eq!(SyncMessage::decode(&[0x7f]), Err(Refusal::ProtocolError));
        let hello = codec::encode(&SyncMessage::Hello {
            version: PROTOCOL_VERSION,
            goal: GoalId([1; 32]),
        })
        .unwrap();
        assert_eq!(
            SyncMessage::decode(&hello[..hello.len() - 1]),
            Err(Refusal::ProtocolError)
        );
    }

    fn decoded(message: &SyncMessage) -> Result<SyncMessage, Refusal> {
        SyncMessage::decode(&codec::encode(message).unwrap())
    }

    #[test]
    fn event_batches_are_limited_in_count_and_header_size() {
        let event = |len: usize| WireEvent {
            header: vec![0; len],
            signature: Signature([0; 64]),
        };
        let full = SyncMessage::Events(vec![event(1); MAX_EVENTS_PER_BATCH]);
        assert!(decoded(&full).is_ok());
        let crowded = SyncMessage::Events(vec![event(1); MAX_EVENTS_PER_BATCH + 1]);
        assert_eq!(decoded(&crowded), Err(Refusal::LimitExceeded));

        assert!(decoded(&SyncMessage::Events(vec![event(MAX_HEADER_BYTES)])).is_ok());
        let oversized = SyncMessage::Events(vec![event(1), event(MAX_HEADER_BYTES + 1)]);
        assert_eq!(decoded(&oversized), Err(Refusal::LimitExceeded));
    }

    #[test]
    fn frontiers_are_limited_and_strictly_ascending() {
        let entry = |n: u32| {
            let mut author = [0; 32];
            author[..4].copy_from_slice(&n.to_be_bytes());
            AuthorFrontier::from_points(PublicKey(author), &[])
        };
        let full = (0..MAX_FRONTIER_AUTHORS as u32).map(entry).collect();
        assert!(decoded(&SyncMessage::Frontier(Frontier { authors: full })).is_ok());
        let crowded = (0..=MAX_FRONTIER_AUTHORS as u32).map(entry).collect();
        assert_eq!(
            decoded(&SyncMessage::Frontier(Frontier { authors: crowded })),
            Err(Refusal::LimitExceeded)
        );

        for authors in [vec![entry(2), entry(1)], vec![entry(1), entry(1)]] {
            assert_eq!(
                decoded(&SyncMessage::Frontier(Frontier { authors })),
                Err(Refusal::ProtocolError)
            );
        }
    }

    #[test]
    fn inventories_are_limited_and_strictly_ascending() {
        let inventory = |points: Vec<AuthorPoint>| SyncMessage::Inventory {
            author: PublicKey([1; 32]),
            points,
            more: false,
        };
        let full: Vec<AuthorPoint> = (0..MAX_INVENTORY_POINTS as u64)
            .map(|seq| AuthorPoint {
                seq,
                id: EventId([0; 32]),
            })
            .collect();
        assert!(decoded(&inventory(full.clone())).is_ok());
        let mut crowded = full;
        crowded.push(at(MAX_INVENTORY_POINTS as u64, 0));
        assert_eq!(decoded(&inventory(crowded)), Err(Refusal::LimitExceeded));

        for points in [
            vec![at(1, 1), at(0, 2)],
            vec![at(1, 3), at(1, 2)],
            vec![at(1, 2), at(1, 2)],
        ] {
            assert_eq!(decoded(&inventory(points)), Err(Refusal::ProtocolError));
        }
    }

    #[test]
    fn event_requests_are_limited() {
        let request = |count| SyncMessage::EventRequest(vec![EventId([1; 32]); count]);
        assert!(decoded(&request(MAX_INVENTORY_POINTS)).is_ok());
        assert_eq!(
            decoded(&request(MAX_INVENTORY_POINTS + 1)),
            Err(Refusal::LimitExceeded)
        );
    }

    #[test]
    fn chunks_are_limited_and_stay_inside_their_object() {
        let chunk = |offset: u64, total: u64, len: usize| SyncMessage::BlobChunk {
            hash: BlobHash([1; 32]),
            offset,
            total,
            bytes: vec![0; len],
        };
        let max = MAX_BLOB_BYTES as u64;
        assert!(decoded(&chunk(0, max, BLOB_CHUNK_BYTES)).is_ok());
        assert!(decoded(&chunk(max - 1, max, 1)).is_ok());
        assert!(decoded(&chunk(5, 5, 0)).is_ok());
        assert_eq!(
            decoded(&chunk(0, max, BLOB_CHUNK_BYTES + 1)),
            Err(Refusal::LimitExceeded)
        );
        assert_eq!(decoded(&chunk(0, max + 1, 1)), Err(Refusal::LimitExceeded));
        assert_eq!(decoded(&chunk(4, 5, 2)), Err(Refusal::ProtocolError));
        assert_eq!(decoded(&chunk(6, 5, 0)), Err(Refusal::ProtocolError));
        assert_eq!(
            decoded(&chunk(u64::MAX, max, 1)),
            Err(Refusal::ProtocolError)
        );
    }

    #[test]
    fn the_largest_frames_fit_their_read_limits() {
        let chunk = SyncMessage::BlobChunk {
            hash: BlobHash([0xff; 32]),
            offset: u64::MAX,
            total: u64::MAX,
            bytes: vec![0xff; BLOB_CHUNK_BYTES],
        };
        assert!(codec::encode(&chunk).unwrap().len() <= MAX_PEER_FRAME_BYTES);

        let batch = SyncMessage::Events(vec![
            WireEvent {
                header: vec![0xff; MAX_HEADER_BYTES],
                signature: Signature([0xff; 64]),
            };
            MAX_EVENTS_PER_BATCH
        ]);
        let bytes = codec::encode(&batch).unwrap();
        assert!(bytes.len() <= MAX_PEER_FRAME_BYTES);
        assert_eq!(SyncMessage::decode(&bytes), Ok(batch));

        let widest = |seq| AuthorFrontier {
            author: PublicKey([0xff; 32]),
            next_seq: seq,
            digest: [0xff; 32],
        };
        let frontier = SyncMessage::Frontier(Frontier {
            authors: vec![widest(u64::MAX); MAX_FRONTIER_AUTHORS],
        });
        assert!(codec::encode(&frontier).unwrap().len() <= MAX_PEER_FRAME_BYTES);
        let inventory = SyncMessage::Inventory {
            author: PublicKey([0xff; 32]),
            points: vec![at(u64::MAX, 0xff); MAX_INVENTORY_POINTS],
            more: true,
        };
        assert!(codec::encode(&inventory).unwrap().len() <= MAX_PEER_FRAME_BYTES);

        // What a non-member may send, and what it is answered with before
        // admission, fits the hello limit.
        let join: JoinRequest = codec::decode(&[0xff; 192]).unwrap();
        for message in [
            SyncMessage::Hello {
                version: PROTOCOL_VERSION,
                goal: GoalId([0xff; 32]),
            },
            SyncMessage::Join(join),
            SyncMessage::Refused(Refusal::InvitationRefused),
        ] {
            assert!(codec::encode(&message).unwrap().len() <= MAX_HELLO_FRAME_BYTES);
        }
    }

    #[test]
    fn a_chunk_carries_its_bytes_as_one_run() {
        let message = SyncMessage::BlobChunk {
            hash: BlobHash([0; 32]),
            offset: 0,
            total: 300,
            bytes: vec![7; 300],
        };
        let frame = codec::encode(&message).unwrap();
        // Variant index, 32-byte hash, offset, two-byte total, two-byte
        // length, then the bytes.
        assert_eq!(frame.len(), 1 + 32 + 1 + 2 + 2 + 300);
        assert_eq!(SyncMessage::decode(&frame), Ok(message));
    }

    #[test]
    fn a_key_frame_never_shows_the_key() {
        let key: ContentKey = codec::decode(&[0x5a; 32]).unwrap();
        let message = SyncMessage::Key { epoch: 3, key };

        let printed = format!("{message:?}");
        assert!(printed.contains("epoch: 3"));
        assert!(!printed.contains("5a5a") && !printed.contains("90, 90"));
        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("<redacted>"));
        assert!(!json.contains("5a5a") && !json.contains("90,90"));

        // The binary frame between members does carry it.
        let bytes = codec::encode(&message).unwrap();
        assert_eq!(&bytes[bytes.len() - 32..], &[0x5a; 32]);
        assert_eq!(SyncMessage::decode(&bytes), Ok(message));
    }

    #[test]
    fn refusals_render_in_snake_case() {
        for (refusal, name) in [
            (Refusal::UnsupportedVersion, "unsupported_version"),
            (Refusal::NotAMember, "not_a_member"),
            (Refusal::InvitationRefused, "invitation_refused"),
            (Refusal::LimitExceeded, "limit_exceeded"),
            (Refusal::ProtocolError, "protocol_error"),
            (Refusal::KeyUnavailable, "key_unavailable"),
        ] {
            assert_eq!(
                serde_json::to_string(&refusal).unwrap(),
                format!("\"{name}\"")
            );
            assert!(!refusal.to_string().is_empty());
        }
    }

    #[test]
    fn a_staged_transfer_is_verified_only_when_complete() {
        // The transfer rule end to end over the storage seam: chunks are
        // staged, a reconnect resumes at the staged length, and the object is
        // verified before it is held.
        let object: Vec<u8> = (0..=255u8).cycle().take(2 * 100 + 7).collect();
        let hash = content_hash(&object);
        let serve = |offset: usize| SyncMessage::BlobChunk {
            hash,
            offset: offset as u64,
            total: object.len() as u64,
            bytes: object[offset..object.len().min(offset + 100)].to_vec(),
        };

        let mut store = MemStore::new();
        let receive = |store: &mut MemStore, message: SyncMessage| match decoded(&message) {
            Ok(SyncMessage::BlobChunk {
                hash,
                offset,
                bytes,
                ..
            }) => store.stage_blob(&hash, offset, &bytes).unwrap(),
            other => panic!("unexpected {other:?}"),
        };
        assert_eq!(receive(&mut store, serve(0)), 100);
        // The link drops; a new exchange resumes where staging ended.
        let resume = store.reopen().staged_len(&hash).unwrap();
        assert_eq!(resume, 100);
        assert_eq!(receive(&mut store, serve(resume as usize)), 200);
        assert_eq!(receive(&mut store, serve(200)), object.len() as u64);
        assert_eq!(store.blob_len(&hash), Ok(None));
        assert_eq!(store.finish_blob(&hash), Ok(true));
        assert_eq!(store.blob(&hash), Ok(Some(object)));
    }
}
