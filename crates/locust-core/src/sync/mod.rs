//! Reconciliation between two daemons as state machines over frames.
//!
//! One exchange is one stream; its grammar is the module documentation of
//! [`locust_proto::sync`]. Both ends are state machines with a frame in and
//! frames out:
//!
//! - [`Responder`] serves an exchange a peer opened. Until the remote
//!   endpoint is known to speak for a member it reads only `Hello` and
//!   `Join`; any other request is refused with `NotAMember`, also for a goal
//!   it does not know.
//! - [`Initiator`] runs an exchange this daemon opened: `Hello`, its
//!   `Frontier`, the answer, the pushes and requests the reconciliation rule
//!   calls for, then founding content and keys, bulk content and remaining
//!   keys, then `Done`.
//! - [`Driver`] owns every exchange of the daemon: which to open, retries
//!   with backoff, anti-entropy, admission and finishing. It turns
//!   [`PeerInput`](locust_proto::engine::PeerInput) into
//!   [`PeerOutput`](locust_proto::engine::PeerOutput) and asks the node only
//!   about its goals, through [`Host`].
//!
//! The machines never hold a goal. They borrow one through [`Replica`],
//! which the node implements over its goal and store, so this module is
//! tested with in-memory fakes. Responses retain cursors over history and content, materializing at most one
//! outgoing frame until the transport reports capacity. Inventory pages and
//! requests obey the wire limits; complete histories and transfers are never
//! collected into outgoing frame vectors.

mod batch;
mod driver;
mod initiator;
mod outbox;
mod responder;
#[cfg(test)]
mod tests;

pub use driver::{
    ANTI_ENTROPY_MS, Driver, Ended, Host, Joining, MAX_BACKOFF_MS, MIN_BACKOFF_MS, Report,
};
pub use initiator::Initiator;
pub use responder::Responder;

use locust_proto::crypto::ContentKey;
use locust_proto::event::{AuthorPoint, WireEvent};
use locust_proto::id::{BlobHash, EffectId, EndpointId, EventId, PublicKey};
use locust_proto::sync::{AuthorFrontier, Frontier, Refusal};

/// What [`Replica::stage`] made of one received chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Staged {
    /// Staged; the object is not complete yet. Carries the staged length,
    /// where the next chunk is expected.
    More(u64),
    /// The object is held now, verified against its hash, or was already
    /// held. Later chunks of this transfer are ignored.
    Complete,
    /// Not wanted, inconsistent with what the goal says about it, or complete
    /// but not matching its hash, in which case the staged bytes were
    /// discarded. Later chunks of this transfer are ignored.
    Rejected,
}

/// One goal as reconciliation sees it. The node implements it over its goal
/// and store; tests implement it over [`locust_proto::store::MemStore`].
///
/// Reads are infallible: a replica that cannot read its store answers as if
/// it held nothing more, and the next exchange finds what this one missed.
pub trait Replica {
    /// Next committed, currently authorized recipient outbox item for this endpoint.
    fn next_delivery(
        &self,
        _remote: EndpointId,
        _after: Option<(EffectId, PublicKey)>,
    ) -> Option<(EffectId, PublicKey)> {
        None
    }

    /// Whether this daemon durably holds the addressed local recipient inbox item.
    fn receive_delivery(
        &mut self,
        _effect: EffectId,
        _recipient: PublicKey,
    ) -> Result<bool, Refusal> {
        Ok(false)
    }

    /// Commit a positive receipt from the authenticated recipient endpoint.
    fn receive_receipt(
        &mut self,
        _remote: EndpointId,
        _effect: EffectId,
        _recipient: PublicKey,
    ) -> Result<(), Refusal> {
        Err(Refusal::ProtocolError)
    }

    /// What this replica holds: one entry per author it holds events of,
    /// strictly ascending by author, at most
    /// [`MAX_FRONTIER_AUTHORS`](locust_proto::limits::MAX_FRONTIER_AUTHORS).
    fn frontier(&self) -> Frontier;

    /// The author whose events are sent before any other author's: the
    /// goal's governance key, once a first record is held. A receiver
    /// screens every other author by the admissions in that key's log. The
    /// frontier frame itself keeps its ascending order.
    fn first_author(&self) -> Option<PublicKey> {
        None
    }

    /// [`AuthorFrontier::is_prefix_of`] applied to this replica's points of
    /// `theirs.author`, answered by lookup of the running digest.
    fn extends(&self, theirs: &AuthorFrontier) -> bool;

    /// Every retained point of `author`, strictly ascending by (position,
    /// identifier); empty for an author not held. Paging is the caller's.
    fn points(&self, author: &PublicKey) -> &[AuthorPoint];

    /// The held event `id` as it travels, or `None` when it is not held.
    fn wire_event(&self, id: &EventId) -> Option<WireEvent>;

    /// Verifies, screens, commits and applies received events, and returns
    /// how many were new. Events already held are skipped. An error ends the
    /// exchange with that refusal: `ProtocolError` for an event that does not
    /// decode or names another goal.
    fn receive(&mut self, events: Vec<WireEvent>) -> Result<usize, Refusal>;

    /// The content key of `epoch`, when held.
    fn key(&self, epoch: u32) -> Option<ContentKey>;

    /// Epochs of held events whose key is not held, ascending.
    fn wanted_keys(&self) -> Vec<u32>;

    /// Keeps `key` for `epoch` only if it opens a held payload of that epoch
    /// (the epoch's first decision, or the genesis payload for epoch 0), and
    /// says whether it did. A key that cannot be checked yet is refused; a
    /// later exchange offers it again.
    fn offer_key(&mut self, epoch: u32, key: ContentKey) -> bool;

    /// The missing founding payload, fetched before bulk history so its key
    /// and goal title can become usable immediately. None when already held.
    fn founding_blob(&self) -> Option<(BlobHash, u64)> {
        None
    }

    /// At most `limit` content objects the goal's held events name and this
    /// replica lacks, payloads of held events first, each with its staged
    /// length, where a transfer resumes.
    fn wanted_blobs(&self, limit: usize) -> Vec<(BlobHash, u64)>;

    /// Next missing object in hash order, strictly after the cursor. An
    /// unavailable object advances the cursor too, so no prefix can starve
    /// later content. Production implementations should avoid materializing
    /// the entire wanted set.
    fn next_wanted_blob(&self, after: Option<BlobHash>) -> Option<(BlobHash, u64)> {
        self.wanted_blobs(usize::MAX)
            .into_iter()
            .filter(|(hash, _)| after.is_none_or(|after| *hash > after))
            .min_by_key(|(hash, _)| *hash)
    }

    /// The stored length of an object this goal serves, or `None` when it
    /// does not hold it, withdrew it, or no held event of the goal names it.
    fn blob_len(&self, hash: &BlobHash) -> Option<u64>;

    /// Up to `len` stored bytes of an object this goal serves from `offset`;
    /// `None` when [`Replica::blob_len`] is `None`.
    fn blob_range(&self, hash: &BlobHash, offset: u64, len: usize) -> Option<Vec<u8>>;

    /// Stages one received chunk of `hash`, whose whole stored length is
    /// `total`, and verifies and promotes the object once it is complete.
    fn stage(&mut self, hash: &BlobHash, offset: u64, total: u64, bytes: &[u8]) -> Staged;
}
