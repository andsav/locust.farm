//! The storage seam between the state machine and durable storage.
//!
//! The state machine computes a [`Commit`]: the events, content objects, local
//! records and content removals that one operation produces. A store applies a
//! commit entirely or not at all. That single rule is what keeps an event, the
//! local state derived from it and the record that makes a retried request
//! idempotent from ever disagreeing after a crash.
//!
//! One dedicated thread owns the store. It runs the state machine, calls this
//! trait, and exchanges messages with the I/O tasks (local connections, peer
//! links) over channels. The trait is therefore synchronous, has no notion of
//! a runtime and needs no lock. The state machine rebuilds task, membership
//! and frontier state in memory by replaying [`Store::log`], so that state
//! cannot disagree with the log after a crash.
//!
//! What this trait reports as durable survives power loss, not only a process
//! crash (SQLite: WAL, `synchronous=FULL`, and `fullfsync` on macOS). The
//! daemon releases its own signed events to peers once a commit returns; a
//! commit lost after that would make it sign a second event at the same
//! position, which peers see as a forked history.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::ops::Bound;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::crypto::content_hash;
use crate::event::{AuthorPoint, Event};
use crate::id::{BlobHash, EventId, GoalId, PublicKey};

/// Content whose hash is known to match its bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blob {
    hash: BlobHash,
    bytes: Vec<u8>,
}

impl Blob {
    /// Wraps bytes with their plain BLAKE3 hash, which identifies them.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            hash: content_hash(&bytes),
            bytes,
        }
    }

    /// Accepts bytes received under a claimed hash only if they match it.
    pub fn verified(hash: BlobHash, bytes: Vec<u8>) -> Option<Self> {
        let blob = Self::new(bytes);
        (blob.hash == hash).then_some(blob)
    }

    /// The identity of these bytes.
    pub fn hash(&self) -> BlobHash {
        self.hash
    }

    /// The stored bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The stored bytes, without copying.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Namespaces for local, non-replicated records. Keys and values are opaque
/// byte strings to the store, the empty string included; the crate that owns
/// a space defines their encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Space {
    /// Daemon identity, endpoint key and signing watermarks.
    Identity = 0,
    /// Enrolled principals: keys, names, grants and credential digests.
    Agent = 1,
    /// Per-goal local settings: workspace binding, standing grants, halt state.
    Goal = 2,
    /// Known peers: contact hints and last successful synchronization.
    Peer = 3,
    /// Issued invitations and their redemption state.
    Invite = 4,
    /// Session-bound claims and their generations.
    Claim = 5,
    /// Event feed entries and explicit session context acknowledgments.
    Cursor = 6,
    /// Request keys with the digest and result of their first execution.
    Idempotency = 7,
    /// Obligations that cannot be rediscovered from the event log.
    Pending = 8,
    /// Session records of client adapters, keyed by instance.
    Session = 9,
    /// Which goals a held content object belongs to, withdrawn marks and
    /// retention.
    Blob = 10,
    /// Content keys per goal and key epoch.
    Key = 11,
    /// Owner-scoped local drafts, presentation revisions and immutable publications.
    Formation = 12,
}

/// One local record: its key and its value.
pub type LocalRecord = (Vec<u8>, Vec<u8>);

/// A change to one local record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalWrite {
    /// Sets the value of `key` in `space`, replacing any earlier value.
    Put {
        space: Space,
        key: Vec<u8>,
        value: Vec<u8>,
    },
    /// Removes `key` from `space`; an absent key is ignored.
    Delete { space: Space, key: Vec<u8> },
}

/// Everything one operation makes durable, applied as one step. Within it,
/// local writes take effect in order and removals take effect last.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Commit {
    /// Structurally valid events the caller has decided to retain. Those not
    /// already held are appended to their goal's log in this order.
    pub events: Vec<Event>,
    /// Content objects to hold. Objects already held are left untouched.
    pub blobs: Vec<Blob>,
    /// Applied in order after the events and blobs.
    pub local: Vec<LocalWrite>,
    /// Content objects to stop holding, applied last; absent objects are
    /// ignored. Events that name them are kept.
    pub drop_blobs: Vec<BlobHash>,
}

/// Why a store could not answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    /// The storage medium failed (I/O error, disk full). Nothing was applied,
    /// except that a failure while a mutation (commit, staged write, promotion
    /// or discard) was being made durable leaves its outcome unknown until the
    /// store is reopened; a store refuses further calls until recovery has made
    /// that state durable. A daemon whose mutation failed therefore stops and
    /// replays the log on its next start, and never signs again at a position
    /// it may already have used.
    Failed(String),
    /// Stored data failed an integrity check.
    Corrupted(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(detail) => write!(f, "storage failed: {detail}"),
            Self::Corrupted(detail) => write!(f, "stored data is corrupted: {detail}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Durable storage of events, content objects and local records. The
/// `conformance` module (feature `testkit`) states the behavior every
/// implementation shares.
pub trait Store {
    /// Applies the whole commit or none of it, as one durable step.
    ///
    /// Events not already held are appended to their goal's log in the order
    /// they appear in `commit.events`: the n-th event ever stored for a goal
    /// has position n, counted per goal from 1 with no gaps, so the caller
    /// knows every new position without reading back. Events and objects
    /// already held are left untouched, so replaying a commit changes
    /// nothing. An event may name content that is not held. Every content
    /// object is durable before any record that names it becomes visible.
    ///
    /// When this returns `Ok` the commit survives power loss (see the module
    /// documentation).
    fn commit(&mut self, commit: &Commit) -> Result<(), StoreError>;

    /// The event with this identifier, if held.
    fn event(&self, id: &EventId) -> Result<Option<Event>, StoreError>;

    /// Whether the event with this identifier is held, without reading it.
    fn has_event(&self, id: &EventId) -> Result<bool, StoreError>;

    /// At most `limit` events of a goal with a position greater than `after`,
    /// in position order, each with its position. Positions start at 1,
    /// follow local arrival order, strictly increase and never change, so a
    /// position is a durable cursor.
    fn log(&self, goal: &GoalId, after: u64, limit: usize)
    -> Result<Vec<(u64, Event)>, StoreError>;

    /// At most `limit` events of one author in a goal that come strictly
    /// after the cursor `after` in ascending (position, identifier) order, or
    /// from the first when `after` is `None`. Two events share a position only
    /// when the author signed conflicting histories; all of them are kept and
    /// returned. The cursor need not name a held event. Passing the point of
    /// the last event returned continues the listing, so paging visits every
    /// event exactly once however many share a position.
    fn author_log(
        &self,
        goal: &GoalId,
        author: &PublicKey,
        after: Option<AuthorPoint>,
        limit: usize,
    ) -> Result<Vec<Event>, StoreError>;

    /// Goals with at least one stored event, ascending.
    fn goals(&self) -> Result<Vec<GoalId>, StoreError>;

    /// The stored bytes of a held content object.
    fn blob(&self, hash: &BlobHash) -> Result<Option<Vec<u8>>, StoreError>;

    /// The stored length of a held content object, or `None` when it is not
    /// held.
    fn blob_len(&self, hash: &BlobHash) -> Result<Option<u64>, StoreError>;

    /// Up to `len` stored bytes of a held content object starting at byte
    /// `offset`: fewer at its end, none from its end on. `None` when the
    /// object is not held. Serves one transfer chunk without reading the
    /// whole object.
    fn blob_range(
        &self,
        hash: &BlobHash,
        offset: u64,
        len: usize,
    ) -> Result<Option<Vec<u8>>, StoreError>;

    /// Appends `bytes` to the partial copy of object `hash` being received,
    /// only when `offset` equals its staged length, and returns the staged
    /// length either way. A repeated or misplaced chunk therefore changes
    /// nothing and tells the caller where to resume. Staged bytes are not a
    /// held object until [`Store::finish_blob`] promotes them. Durable like a
    /// commit when it returns.
    fn stage_blob(&mut self, hash: &BlobHash, offset: u64, bytes: &[u8])
    -> Result<u64, StoreError>;

    /// The number of bytes staged for `hash`; 0 when none are.
    fn staged_len(&self, hash: &BlobHash) -> Result<u64, StoreError>;

    /// Reads at most `len` unverified staged bytes from `offset`, without
    /// promoting them. None means no staged copy; an exhausted range is
    /// empty. Allows restart-safe format checks before promotion.
    fn staged_range(
        &self,
        hash: &BlobHash,
        offset: u64,
        len: usize,
    ) -> Result<Option<Vec<u8>>, StoreError>;

    /// Durably discards only the staged copy. An existing held object is
    /// untouched. Repeating the discard is harmless.
    fn discard_staged_blob(&mut self, hash: &BlobHash) -> Result<(), StoreError>;

    /// Ends the receipt of object `hash`. If the staged bytes hash to `hash`
    /// they become a held object (an object already held is left untouched)
    /// and this returns `true`; otherwise they are discarded and this returns
    /// `false`. Either way nothing remains staged for `hash`. Durable like a
    /// commit when it returns.
    fn finish_blob(&mut self, hash: &BlobHash) -> Result<bool, StoreError>;

    /// The value of `key` in `space`, if set.
    fn get(&self, space: Space, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError>;

    /// Records in `space` whose key starts with `prefix`, ascending by key
    /// bytes.
    fn scan(&self, space: Space, prefix: &[u8]) -> Result<Vec<LocalRecord>, StoreError>;
}

/// Reference implementation held in memory. It defines the behavior every
/// store must match and backs tests and early integration. It is not durable:
/// its state lives as long as one of its handles.
#[derive(Debug, Default)]
pub struct MemStore {
    state: Arc<Mutex<MemState>>,
}

#[derive(Debug, Default)]
struct MemState {
    events: HashMap<EventId, Event>,
    logs: BTreeMap<GoalId, Vec<EventId>>,
    authors: BTreeSet<(GoalId, PublicKey, u64, EventId)>,
    blobs: HashMap<BlobHash, Vec<u8>>,
    staged: HashMap<BlobHash, Vec<u8>>,
    local: BTreeMap<(Space, Vec<u8>), Vec<u8>>,
}

impl MemStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Another handle on this store's state, as reopening a durable store
    /// after a restart would give: everything committed or staged through one
    /// handle is visible through every other. Lets tests restart a daemon, or
    /// run `conformance::run_reopen`, over the same state.
    pub fn reopen(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }

    fn state(&self) -> MutexGuard<'_, MemState> {
        // Writes never stop part-way (allocation failure aborts), so a lock
        // poisoned by a panicking caller still guards consistent state.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Store for MemStore {
    fn commit(&mut self, commit: &Commit) -> Result<(), StoreError> {
        let mut guard = self.state();
        let state = &mut *guard;
        for blob in &commit.blobs {
            state
                .blobs
                .entry(blob.hash)
                .or_insert_with(|| blob.bytes.clone());
        }
        for event in &commit.events {
            let id = event.id();
            if state.events.contains_key(&id) {
                continue;
            }
            let header = event.header();
            state.logs.entry(header.goal).or_default().push(id);
            state
                .authors
                .insert((header.goal, header.author, header.seq, id));
            state.events.insert(id, event.clone());
        }
        for write in &commit.local {
            match write {
                LocalWrite::Put { space, key, value } => {
                    state.local.insert((*space, key.clone()), value.clone());
                }
                LocalWrite::Delete { space, key } => {
                    state.local.remove(&(*space, key.clone()));
                }
            }
        }
        for hash in &commit.drop_blobs {
            state.blobs.remove(hash);
        }
        Ok(())
    }

    fn event(&self, id: &EventId) -> Result<Option<Event>, StoreError> {
        Ok(self.state().events.get(id).cloned())
    }

    fn has_event(&self, id: &EventId) -> Result<bool, StoreError> {
        Ok(self.state().events.contains_key(id))
    }

    fn log(
        &self,
        goal: &GoalId,
        after: u64,
        limit: usize,
    ) -> Result<Vec<(u64, Event)>, StoreError> {
        let state = self.state();
        let Some(log) = state.logs.get(goal) else {
            return Ok(Vec::new());
        };
        // The event at index i has position i + 1.
        let start = usize::try_from(after).map_or(log.len(), |after| after.min(log.len()));
        Ok(log[start..]
            .iter()
            .take(limit)
            .zip(after.saturating_add(1)..)
            .map(|(id, position)| (position, state.events[id].clone()))
            .collect())
    }

    fn author_log(
        &self,
        goal: &GoalId,
        author: &PublicKey,
        after: Option<AuthorPoint>,
        limit: usize,
    ) -> Result<Vec<Event>, StoreError> {
        let state = self.state();
        let first = match after {
            Some(point) => Bound::Excluded((*goal, *author, point.seq, point.id)),
            None => Bound::Included((*goal, *author, 0, EventId([0x00; 32]))),
        };
        let last = Bound::Included((*goal, *author, u64::MAX, EventId([0xff; 32])));
        Ok(state
            .authors
            .range((first, last))
            .take(limit)
            .map(|(_, _, _, id)| state.events[id].clone())
            .collect())
    }

    fn goals(&self) -> Result<Vec<GoalId>, StoreError> {
        Ok(self.state().logs.keys().copied().collect())
    }

    fn blob(&self, hash: &BlobHash) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.state().blobs.get(hash).cloned())
    }

    fn blob_len(&self, hash: &BlobHash) -> Result<Option<u64>, StoreError> {
        Ok(self.state().blobs.get(hash).map(|bytes| bytes.len() as u64))
    }

    fn blob_range(
        &self,
        hash: &BlobHash,
        offset: u64,
        len: usize,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        let state = self.state();
        let Some(bytes) = state.blobs.get(hash) else {
            return Ok(None);
        };
        let start = usize::try_from(offset).map_or(bytes.len(), |offset| offset.min(bytes.len()));
        let end = start.saturating_add(len).min(bytes.len());
        Ok(Some(bytes[start..end].to_vec()))
    }

    fn stage_blob(
        &mut self,
        hash: &BlobHash,
        offset: u64,
        bytes: &[u8],
    ) -> Result<u64, StoreError> {
        let mut state = self.state();
        let staged = state.staged.get(hash).map_or(0, Vec::len) as u64;
        if offset != staged || bytes.is_empty() {
            return Ok(staged);
        }
        state
            .staged
            .entry(*hash)
            .or_default()
            .extend_from_slice(bytes);
        Ok(staged + bytes.len() as u64)
    }

    fn staged_len(&self, hash: &BlobHash) -> Result<u64, StoreError> {
        Ok(self.state().staged.get(hash).map_or(0, Vec::len) as u64)
    }

    fn staged_range(
        &self,
        hash: &BlobHash,
        offset: u64,
        len: usize,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        let state = self.state();
        let Some(bytes) = state.staged.get(hash) else {
            return Ok(None);
        };
        let start = usize::try_from(offset).map_or(bytes.len(), |offset| offset.min(bytes.len()));
        let end = start.saturating_add(len).min(bytes.len());
        Ok(Some(bytes[start..end].to_vec()))
    }

    fn discard_staged_blob(&mut self, hash: &BlobHash) -> Result<(), StoreError> {
        self.state().staged.remove(hash);
        Ok(())
    }

    fn finish_blob(&mut self, hash: &BlobHash) -> Result<bool, StoreError> {
        let mut state = self.state();
        let staged = state.staged.remove(hash).unwrap_or_default();
        if content_hash(&staged) != *hash {
            return Ok(false);
        }
        state.blobs.entry(*hash).or_insert(staged);
        Ok(true)
    }

    fn get(&self, space: Space, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.state().local.get(&(space, key.to_vec())).cloned())
    }

    fn scan(&self, space: Space, prefix: &[u8]) -> Result<Vec<LocalRecord>, StoreError> {
        Ok(self
            .state()
            .local
            .range((space, prefix.to_vec())..)
            .take_while(|((entry_space, key), _)| *entry_space == space && key.starts_with(prefix))
            .map(|((_, key), value)| (key.clone(), value.clone()))
            .collect())
    }
}

/// Behavior every [`Store`] must share. Run both suites against each
/// implementation: `conformance::run(MyStore::open_empty)` and
/// `conformance::run_reopen(|| MyStore::open(&path))`.
#[cfg(any(test, feature = "testkit"))]
pub mod conformance {
    use super::*;
    use crate::event::{Body, Context, Scope};
    use crate::testkit::{self, Author};

    fn contribution() -> Body {
        Body::ContributionPublished {
            context: Context {
                scope: Scope::Goal,
                round: EventId([1; 32]),
            },
            attempt: None,
            sources: Vec::new(),
            artifacts: Vec::new(),
        }
    }

    fn put(space: Space, key: &[u8], value: &[u8]) -> LocalWrite {
        LocalWrite::Put {
            space,
            key: key.to_vec(),
            value: value.to_vec(),
        }
    }

    fn point(event: &Event) -> AuthorPoint {
        AuthorPoint {
            seq: event.header().seq,
            id: event.id(),
        }
    }

    fn events(events: &[&Event]) -> Commit {
        Commit {
            events: events.iter().map(|event| (*event).clone()).collect(),
            ..Commit::default()
        }
    }

    fn positions(entries: &[(u64, Event)]) -> Vec<u64> {
        entries.iter().map(|(position, _)| *position).collect()
    }

    /// A goal founded by `owner`, followed by `count` notes of the owner.
    fn straight_log(owner: &mut Author, count: usize) -> Vec<Event> {
        let genesis = owner.genesis(testkit::keypair(9).public());
        let goal = genesis.header().goal;
        let mut log = vec![genesis.clone()];
        for _ in 0..count {
            log.push(owner.event(goal, Some(genesis.id()), contribution()));
        }
        log
    }

    /// Runs every check, each against a fresh store from `empty`.
    pub fn run<S: Store>(empty: impl Fn() -> S) {
        an_empty_store_answers_with_nothing(empty());
        a_commit_becomes_visible_as_a_whole(empty());
        replaying_a_commit_keeps_every_position(empty());
        the_log_is_a_stable_cursor(empty());
        positions_follow_commit_order_in_each_goal(empty());
        author_logs_keep_conflicting_events(empty());
        the_author_log_limit_applies_after_the_cursor(empty());
        author_log_paging_visits_every_event_at_one_position_once(empty());
        local_records_are_scoped_and_ordered(empty());
        local_keys_are_opaque_bytes_and_values_may_be_empty(empty());
        several_blobs_commit_together(empty());
        dropped_blobs_go_last_and_leave_events_alone(empty());
        held_blobs_are_served_by_range(empty());
        staging_resumes_and_promotes_only_matching_bytes(empty());
    }

    /// Runs the checks that span a restart. `open` returns a store over one
    /// persistent state, empty on the first call. The suite drops each store
    /// before opening the next, so an implementation may hold an exclusive
    /// lock on its state.
    pub fn run_reopen<S: Store>(open: impl Fn() -> S) {
        let mut owner = Author::new(1);
        let mut member = Author::new(2);
        let first = straight_log(&mut owner, 2);
        let goal = first[0].header().goal;
        let other = straight_log(&mut member, 1);
        let other_goal = other[0].header().goal;
        let blob = Blob::new(b"kept across a restart".to_vec());
        let partial = b"staged then resumed".to_vec();
        let partial_hash = content_hash(&partial);
        let discarded_hash = content_hash(b"discarded before promotion");
        let initial = Commit {
            events: [first.clone(), other.clone()].concat(),
            blobs: vec![blob.clone()],
            local: vec![put(Space::Key, &[0x00, 0xff], b"")],
            drop_blobs: Vec::new(),
        };

        {
            let mut store = open();
            assert_eq!(store.goals(), Ok(Vec::new()), "the first open is empty");
            store.commit(&initial).unwrap();
            assert_eq!(store.stage_blob(&partial_hash, 0, &partial[..6]), Ok(6));
            store.stage_blob(&discarded_hash, 0, b"discarded").unwrap();
        }

        let next = owner.event(goal, Some(first[0].id()), contribution());
        {
            let mut store = open();
            let log = store.log(&goal, 0, 10).unwrap();
            assert_eq!(positions(&log), [1, 2, 3]);
            let held: Vec<Event> = log.into_iter().map(|(_, event)| event).collect();
            assert_eq!(held, first);
            assert_eq!(positions(&store.log(&other_goal, 0, 10).unwrap()), [1, 2]);
            let mut goals = vec![goal, other_goal];
            goals.sort();
            assert_eq!(store.goals(), Ok(goals));
            assert_eq!(
                store.author_log(&goal, &owner.key.public(), Some(point(&first[0])), 10),
                Ok(first[1..].to_vec())
            );
            assert_eq!(store.has_event(&other[1].id()), Ok(true));
            assert_eq!(store.blob(&blob.hash()), Ok(Some(blob.bytes().to_vec())));
            assert_eq!(store.get(Space::Key, &[0x00, 0xff]), Ok(Some(Vec::new())));
            assert_eq!(store.staged_len(&partial_hash), Ok(6));
            assert_eq!(
                store.staged_range(&partial_hash, 0, 6),
                Ok(Some(partial[..6].to_vec()))
            );

            // Replay keeps positions; a new event takes the next one.
            store.commit(&initial).unwrap();
            store.commit(&events(&[&first[2], &next])).unwrap();
            assert_eq!(
                store.stage_blob(&partial_hash, 6, &partial[6..]),
                Ok(partial.len() as u64)
            );
            store.discard_staged_blob(&discarded_hash).unwrap();
        }

        {
            let mut store = open();
            let log = store.log(&goal, 2, 10).unwrap();
            assert_eq!(positions(&log), [3, 4]);
            assert_eq!(log[1].1, next);
            assert_eq!(store.staged_len(&partial_hash), Ok(partial.len() as u64));
            assert_eq!(store.staged_len(&discarded_hash), Ok(0));
            assert_eq!(store.staged_range(&discarded_hash, 0, 20), Ok(None));
            assert_eq!(store.blob_len(&discarded_hash), Ok(None));
            assert_eq!(store.finish_blob(&partial_hash), Ok(true));
        }

        let store = open();
        assert_eq!(store.blob(&partial_hash), Ok(Some(partial)));
        assert_eq!(store.staged_len(&partial_hash), Ok(0));
        assert_eq!(positions(&store.log(&goal, 0, 10).unwrap()), [1, 2, 3, 4]);
    }

    fn an_empty_store_answers_with_nothing<S: Store>(mut store: S) {
        let goal = GoalId([1; 32]);
        let hash = BlobHash([1; 32]);
        assert_eq!(store.event(&EventId([1; 32])), Ok(None));
        assert_eq!(store.has_event(&EventId([1; 32])), Ok(false));
        assert_eq!(store.log(&goal, 0, 10), Ok(Vec::new()));
        assert_eq!(
            store.author_log(&goal, &PublicKey([1; 32]), None, 10),
            Ok(Vec::new())
        );
        assert_eq!(store.goals(), Ok(Vec::new()));
        assert_eq!(store.blob(&hash), Ok(None));
        assert_eq!(store.blob_len(&hash), Ok(None));
        assert_eq!(store.blob_range(&hash, 0, 10), Ok(None));
        assert_eq!(store.staged_len(&hash), Ok(0));
        assert_eq!(store.finish_blob(&hash), Ok(false));
        assert_eq!(store.get(Space::Claim, b"key"), Ok(None));
        assert_eq!(store.scan(Space::Claim, b""), Ok(Vec::new()));
    }

    fn a_commit_becomes_visible_as_a_whole<S: Store>(mut store: S) {
        let mut owner = Author::new(1);
        let genesis = owner.genesis(testkit::keypair(9).public());
        let goal = genesis.header().goal;
        let blob = Blob::new(b"task text".to_vec());
        store
            .commit(&Commit {
                events: vec![genesis.clone()],
                blobs: vec![blob.clone()],
                local: vec![put(Space::Goal, b"g", b"settings")],
                drop_blobs: Vec::new(),
            })
            .unwrap();

        assert_eq!(store.event(&genesis.id()), Ok(Some(genesis.clone())));
        assert_eq!(store.has_event(&genesis.id()), Ok(true));
        assert_eq!(store.log(&goal, 0, 10), Ok(vec![(1, genesis)]));
        assert_eq!(store.goals(), Ok(vec![goal]));
        assert_eq!(store.blob(&blob.hash()), Ok(Some(b"task text".to_vec())));
        assert_eq!(store.blob_len(&blob.hash()), Ok(Some(9)));
        assert_eq!(store.get(Space::Goal, b"g"), Ok(Some(b"settings".to_vec())));
    }

    fn replaying_a_commit_keeps_every_position<S: Store>(mut store: S) {
        let mut owner = Author::new(1);
        let log = straight_log(&mut owner, 3);
        let goal = log[0].header().goal;
        let first = Commit {
            blobs: vec![Blob::new(b"x".to_vec())],
            ..events(&[&log[0], &log[1]])
        };
        store.commit(&first).unwrap();
        store.commit(&events(&[&log[2]])).unwrap();
        let before = store.log(&goal, 0, 10).unwrap();
        assert_eq!(positions(&before), [1, 2, 3]);

        store.commit(&first).unwrap();
        assert_eq!(store.log(&goal, 0, 10).unwrap(), before);
        // Held events in a commit take no position; new ones take the next.
        store.commit(&events(&[&log[1], &log[3], &log[0]])).unwrap();
        let after = store.log(&goal, 0, 10).unwrap();
        assert_eq!(after[..3], before[..]);
        assert_eq!(after[3], (4, log[3].clone()));
        assert_eq!(after.len(), 4);
    }

    fn the_log_is_a_stable_cursor<S: Store>(mut store: S) {
        let mut owner = Author::new(1);
        let log = straight_log(&mut owner, 4);
        let goal = log[0].header().goal;
        for event in &log {
            store.commit(&events(&[event])).unwrap();
        }

        assert_eq!(
            positions(&store.log(&goal, 0, 10).unwrap()),
            [1, 2, 3, 4, 5]
        );
        assert_eq!(positions(&store.log(&goal, 0, 2).unwrap()), [1, 2]);
        let tail = store.log(&goal, 3, 10).unwrap();
        assert_eq!(positions(&tail), [4, 5]);
        assert_eq!(tail[0].1, log[3]);
        assert_eq!(positions(&store.log(&goal, 3, 1).unwrap()), [4]);
        assert_eq!(store.log(&goal, 5, 10), Ok(Vec::new()));
        assert_eq!(store.log(&goal, u64::MAX, 10), Ok(Vec::new()));
        assert_eq!(store.log(&goal, 0, 0), Ok(Vec::new()));
        assert_eq!(store.log(&GoalId([9; 32]), 0, 10), Ok(Vec::new()));
    }

    fn positions_follow_commit_order_in_each_goal<S: Store>(mut store: S) {
        let mut first_owner = Author::new(1);
        let mut second_owner = Author::new(2);
        let a = straight_log(&mut first_owner, 3);
        let b = straight_log(&mut second_owner, 2);
        let (goal_a, goal_b) = (a[0].header().goal, b[0].header().goal);
        store
            .commit(&events(&[&a[0], &b[0], &a[1], &b[1], &a[2]]))
            .unwrap();
        // A held event and a repeat within the commit take no position.
        store
            .commit(&events(&[&a[2], &b[2], &a[3], &b[2]]))
            .unwrap();

        let log_a = store.log(&goal_a, 0, 10).unwrap();
        let log_b = store.log(&goal_b, 0, 10).unwrap();
        assert_eq!(positions(&log_a), [1, 2, 3, 4]);
        assert_eq!(positions(&log_b), [1, 2, 3]);
        let held = |log: Vec<(u64, Event)>| -> Vec<Event> {
            log.into_iter().map(|(_, event)| event).collect()
        };
        assert_eq!(held(log_a), a);
        assert_eq!(held(log_b), b);
        let mut goals = vec![goal_a, goal_b];
        goals.sort();
        assert_eq!(store.goals(), Ok(goals));
    }

    fn author_logs_keep_conflicting_events<S: Store>(mut store: S) {
        let mut owner = Author::new(1);
        let genesis = owner.genesis(testkit::keypair(9).public());
        let goal = genesis.header().goal;
        // The same key continues its log twice from the same point.
        let mut twin = Author::new(1);
        assert_eq!(twin.genesis(testkit::keypair(9).public()), genesis);
        let first = owner.event(goal, Some(genesis.id()), contribution());
        let conflicting = twin.event(
            goal,
            Some(genesis.id()),
            Body::ContributionPublished {
                context: Context {
                    scope: Scope::Goal,
                    round: genesis.id(),
                },
                attempt: None,
                sources: Vec::new(),
                artifacts: Vec::new(),
            },
        );
        let third = owner.event(goal, Some(genesis.id()), contribution());
        store
            .commit(&events(&[&third, &conflicting, &genesis, &first]))
            .unwrap();

        let author = genesis.header().author;
        let mut at_one = vec![first, conflicting];
        at_one.sort_by_key(Event::id);
        let mut expected = vec![genesis.clone()];
        expected.extend(at_one.clone());
        expected.push(third.clone());
        assert_eq!(store.author_log(&goal, &author, None, 10), Ok(expected));

        let after_genesis = Some(point(&genesis));
        at_one.push(third);
        assert_eq!(
            store.author_log(&goal, &author, after_genesis, 10),
            Ok(at_one.clone())
        );
        assert_eq!(
            store.author_log(&goal, &author, after_genesis, 2),
            Ok(at_one[..2].to_vec())
        );
        // Resuming between the two events at one position.
        assert_eq!(
            store.author_log(&goal, &author, Some(point(&at_one[0])), 10),
            Ok(at_one[1..].to_vec())
        );
        assert_eq!(
            store.author_log(&goal, &PublicKey([9; 32]), None, 10),
            Ok(Vec::new())
        );
    }

    fn the_author_log_limit_applies_after_the_cursor<S: Store>(mut store: S) {
        let mut owner = Author::new(1);
        let log = straight_log(&mut owner, 5);
        let goal = log[0].header().goal;
        let author = owner.key.public();
        store
            .commit(&Commit {
                events: log.clone(),
                ..Commit::default()
            })
            .unwrap();

        assert_eq!(
            store.author_log(&goal, &author, Some(point(&log[2])), 2),
            Ok(log[3..5].to_vec())
        );
        assert_eq!(
            store.author_log(&goal, &author, Some(point(&log[5])), 2),
            Ok(Vec::new())
        );
        assert_eq!(store.author_log(&goal, &author, None, 0), Ok(Vec::new()));
        // A cursor that names no held event: everything after its point.
        let before_third = AuthorPoint {
            seq: 2,
            id: EventId([0x00; 32]),
        };
        assert_eq!(
            store.author_log(&goal, &author, Some(before_third), 2),
            Ok(log[2..4].to_vec())
        );
        let after_third = AuthorPoint {
            seq: 2,
            id: EventId([0xff; 32]),
        };
        assert_eq!(
            store.author_log(&goal, &author, Some(after_third), 1),
            Ok(log[3..4].to_vec())
        );
        let past_everything = AuthorPoint {
            seq: u64::MAX,
            id: EventId([0xff; 32]),
        };
        assert_eq!(
            store.author_log(&goal, &author, Some(past_everything), 10),
            Ok(Vec::new())
        );
    }

    /// Pages through one author's log `limit` events at a time from `after`.
    /// Bounded, so a store that repeats a page fails instead of looping.
    fn pages<S: Store>(
        store: &S,
        goal: &GoalId,
        author: &PublicKey,
        mut after: Option<AuthorPoint>,
        limit: usize,
    ) -> Vec<Vec<Event>> {
        let mut pages = Vec::new();
        for _ in 0..16 {
            let page = store.author_log(goal, author, after, limit).unwrap();
            let Some(last) = page.last() else {
                return pages;
            };
            after = Some(point(last));
            pages.push(page);
        }
        panic!("paging an author log did not end");
    }

    /// Lane B's B-R3 reproduction: with 257 conflicting events at one
    /// position and pages of 256, paging by position repeated the first page
    /// or skipped the last event.
    fn author_log_paging_visits_every_event_at_one_position_once<S: Store>(mut store: S) {
        let mut owner = Author::new(1);
        let root = owner.genesis(testkit::keypair(9).public());
        let goal = root.header().goal;
        let author = owner.key.public();
        let mut variants: Vec<Event> = (0..257u16)
            .map(|n| {
                let mut twin = Author::new(1);
                twin.genesis(testkit::keypair(9).public());
                let mut about = [0; 32];
                about[..2].copy_from_slice(&n.to_le_bytes());
                twin.event(
                    goal,
                    Some(root.id()),
                    Body::ContributionPublished {
                        context: Context {
                            scope: Scope::Goal,
                            round: EventId(about),
                        },
                        attempt: None,
                        sources: Vec::new(),
                        artifacts: Vec::new(),
                    },
                )
            })
            .collect();
        let mut commit = Commit {
            events: variants.clone(),
            ..Commit::default()
        };
        commit.events.insert(0, root.clone());
        store.commit(&commit).unwrap();
        variants.sort_by_key(Event::id);

        let from_one = pages(&store, &goal, &author, Some(point(&root)), 256);
        let sizes: Vec<usize> = from_one.iter().map(Vec::len).collect();
        assert_eq!(sizes, [256, 1]);
        assert_eq!(from_one.concat(), variants);

        let from_start = pages(&store, &goal, &author, None, 256);
        let sizes: Vec<usize> = from_start.iter().map(Vec::len).collect();
        assert_eq!(sizes, [256, 2]);
        let mut everything = vec![root];
        everything.extend(variants);
        assert_eq!(from_start.concat(), everything);
    }

    fn local_records_are_scoped_and_ordered<S: Store>(mut store: S) {
        store
            .commit(&Commit {
                local: vec![
                    put(Space::Claim, b"b/2", b"two"),
                    put(Space::Claim, b"a/1", b"one"),
                    put(Space::Claim, b"b/1", b"old"),
                    put(Space::Claim, b"b/1", b"new"),
                    put(Space::Cursor, b"b/1", b"other space"),
                    put(Space::Claim, b"c", b"gone"),
                    LocalWrite::Delete {
                        space: Space::Claim,
                        key: b"c".to_vec(),
                    },
                    LocalWrite::Delete {
                        space: Space::Claim,
                        key: b"never set".to_vec(),
                    },
                ],
                ..Commit::default()
            })
            .unwrap();

        assert_eq!(store.get(Space::Claim, b"b/1"), Ok(Some(b"new".to_vec())));
        assert_eq!(store.get(Space::Claim, b"c"), Ok(None));
        assert_eq!(
            store.scan(Space::Claim, b"b/"),
            Ok(vec![
                (b"b/1".to_vec(), b"new".to_vec()),
                (b"b/2".to_vec(), b"two".to_vec()),
            ])
        );
        assert_eq!(store.scan(Space::Claim, b"").unwrap().len(), 3);
        assert_eq!(
            store.scan(Space::Cursor, b""),
            Ok(vec![(b"b/1".to_vec(), b"other space".to_vec())])
        );
    }

    fn local_keys_are_opaque_bytes_and_values_may_be_empty<S: Store>(mut store: S) {
        let keys: [&[u8]; 8] = [
            b"",
            &[0x00],
            &[0x00, 0x00],
            &[0x00, 0xff],
            &[0x7f],
            &[0xff],
            &[0xff, 0x00],
            &[0xff, 0xff],
        ];
        // Written out of order; every value is its key's length in bytes.
        let mut local: Vec<LocalWrite> = keys
            .iter()
            .rev()
            .map(|key| put(Space::Key, key, &vec![0xee; key.len()]))
            .collect();
        local.push(put(Space::Blob, &[0xff], b"neighbouring space"));
        store
            .commit(&Commit {
                local,
                ..Commit::default()
            })
            .unwrap();

        let records = |pairs: &[&[u8]]| -> Vec<LocalRecord> {
            pairs
                .iter()
                .map(|key| (key.to_vec(), vec![0xee; key.len()]))
                .collect()
        };
        // An empty value is a value, not an absent record.
        assert_eq!(store.get(Space::Key, b""), Ok(Some(Vec::new())));
        assert_eq!(store.get(Space::Key, &[0x00]), Ok(Some(vec![0xee])));
        assert_eq!(store.get(Space::Key, &[0x00, 0x01]), Ok(None));
        assert_eq!(store.scan(Space::Key, b""), Ok(records(&keys)));
        assert_eq!(store.scan(Space::Key, &[0x00]), Ok(records(&keys[1..4])));
        assert_eq!(store.scan(Space::Key, &[0xff]), Ok(records(&keys[5..])));
        assert_eq!(
            store.scan(Space::Key, &[0xff, 0xff]),
            Ok(records(&keys[7..]))
        );
        assert_eq!(store.scan(Space::Key, &[0xff, 0xff, 0xff]), Ok(Vec::new()));
        assert_eq!(
            store.scan(Space::Blob, b""),
            Ok(vec![(vec![0xff], b"neighbouring space".to_vec())])
        );
    }

    fn several_blobs_commit_together<S: Store>(mut store: S) {
        let blobs = [
            Blob::new(b"one".to_vec()),
            Blob::new(b"two".to_vec()),
            Blob::new(Vec::new()),
            Blob::new(b"one".to_vec()),
        ];
        store
            .commit(&Commit {
                blobs: blobs.to_vec(),
                ..Commit::default()
            })
            .unwrap();
        for blob in &blobs {
            assert_eq!(store.blob(&blob.hash()), Ok(Some(blob.bytes().to_vec())));
            assert_eq!(
                store.blob_len(&blob.hash()),
                Ok(Some(blob.bytes().len() as u64))
            );
        }
    }

    fn dropped_blobs_go_last_and_leave_events_alone<S: Store>(mut store: S) {
        let kept = Blob::new(b"kept".to_vec());
        let dropped = Blob::new(b"dropped".to_vec());
        let transient = Blob::new(b"transient".to_vec());
        let mut owner = Author::new(1);
        let genesis = owner.genesis(testkit::keypair(9).public());
        let goal = genesis.header().goal;
        let task = owner.event(
            goal,
            Some(genesis.id()),
            Body::ContributionPublished {
                context: Context {
                    scope: Scope::Goal,
                    round: genesis.id(),
                },
                attempt: None,
                sources: Vec::new(),
                artifacts: vec![dropped.hash()],
            },
        );
        store
            .commit(&Commit {
                events: vec![genesis, task.clone()],
                blobs: vec![kept.clone(), dropped.clone()],
                ..Commit::default()
            })
            .unwrap();

        store
            .commit(&Commit {
                drop_blobs: vec![dropped.hash(), BlobHash([9; 32])],
                ..Commit::default()
            })
            .unwrap();
        assert_eq!(store.blob(&dropped.hash()), Ok(None));
        assert_eq!(store.blob_len(&dropped.hash()), Ok(None));
        assert_eq!(store.blob(&kept.hash()), Ok(Some(b"kept".to_vec())));
        assert_eq!(store.event(&task.id()), Ok(Some(task)));

        // Removal is applied after the commit's own content.
        store
            .commit(&Commit {
                blobs: vec![transient.clone()],
                drop_blobs: vec![transient.hash()],
                ..Commit::default()
            })
            .unwrap();
        assert_eq!(store.blob_len(&transient.hash()), Ok(None));

        // A dropped object can be held again.
        store
            .commit(&Commit {
                blobs: vec![dropped.clone()],
                ..Commit::default()
            })
            .unwrap();
        assert_eq!(store.blob(&dropped.hash()), Ok(Some(b"dropped".to_vec())));
    }

    fn held_blobs_are_served_by_range<S: Store>(mut store: S) {
        let blob = Blob::new(b"abcdefghij".to_vec());
        store
            .commit(&Commit {
                blobs: vec![blob.clone()],
                ..Commit::default()
            })
            .unwrap();
        let hash = blob.hash();
        let range = |offset, len| store.blob_range(&hash, offset, len).unwrap();

        assert_eq!(store.blob_len(&hash), Ok(Some(10)));
        assert_eq!(range(0, 4), Some(b"abcd".to_vec()));
        assert_eq!(range(4, 4), Some(b"efgh".to_vec()));
        assert_eq!(range(8, 4), Some(b"ij".to_vec()));
        assert_eq!(range(0, usize::MAX), Some(b"abcdefghij".to_vec()));
        assert_eq!(range(2, 0), Some(Vec::new()));
        assert_eq!(range(10, 4), Some(Vec::new()));
        assert_eq!(range(11, 4), Some(Vec::new()));
        assert_eq!(range(u64::MAX, usize::MAX), Some(Vec::new()));
        assert_eq!(store.blob_range(&BlobHash([9; 32]), 0, 4), Ok(None));
        assert_eq!(store.blob_len(&BlobHash([9; 32])), Ok(None));
    }

    fn staging_resumes_and_promotes_only_matching_bytes<S: Store>(mut store: S) {
        let object = b"0123456789".to_vec();
        let hash = content_hash(&object);

        assert_eq!(store.staged_len(&hash), Ok(0));
        assert_eq!(store.stage_blob(&hash, 0, b"0123"), Ok(4));
        assert_eq!(store.staged_range(&hash, 0, 2), Ok(Some(b"01".to_vec())));
        assert_eq!(
            store.staged_range(&hash, 2, usize::MAX),
            Ok(Some(b"23".to_vec()))
        );
        assert_eq!(
            store.staged_range(&hash, u64::MAX, usize::MAX),
            Ok(Some(Vec::new()))
        );
        assert_eq!(store.staged_range(&BlobHash([9; 32]), 0, 2), Ok(None));
        // A repeated, overlapping or early chunk changes nothing.
        assert_eq!(store.stage_blob(&hash, 0, b"0123"), Ok(4));
        assert_eq!(store.stage_blob(&hash, 2, b"23456"), Ok(4));
        assert_eq!(store.stage_blob(&hash, 9, b"9"), Ok(4));
        assert_eq!(store.stage_blob(&hash, 4, b""), Ok(4));
        assert_eq!(store.staged_len(&hash), Ok(4));
        // Staged bytes are not an object yet.
        assert_eq!(store.blob_len(&hash), Ok(None));
        assert_eq!(store.blob(&hash), Ok(None));

        // Another object stages independently.
        let tampered = content_hash(b"what was promised");
        assert_eq!(store.stage_blob(&tampered, 0, b"what was sent"), Ok(13));

        assert_eq!(store.stage_blob(&hash, 4, b"456789"), Ok(10));
        assert_eq!(store.finish_blob(&hash), Ok(true));
        assert_eq!(store.blob(&hash), Ok(Some(object.clone())));
        assert_eq!(store.staged_len(&hash), Ok(0));

        // Bytes that do not match the hash are discarded, never held.
        assert_eq!(store.finish_blob(&tampered), Ok(false));
        assert_eq!(store.staged_len(&tampered), Ok(0));
        assert_eq!(store.blob_len(&tampered), Ok(None));

        store.stage_blob(&hash, 0, b"bad staged copy").unwrap();
        store.discard_staged_blob(&hash).unwrap();
        store.discard_staged_blob(&hash).unwrap();
        assert_eq!(store.staged_range(&hash, 0, 10), Ok(None));
        assert_eq!(store.blob(&hash), Ok(Some(object.clone())));
        // Receiving an object again leaves the held one untouched.
        assert_eq!(store.stage_blob(&hash, 0, &object), Ok(10));
        assert_eq!(store.finish_blob(&hash), Ok(true));
        assert_eq!(store.blob(&hash), Ok(Some(object)));
        assert_eq!(store.staged_len(&hash), Ok(0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_memory_store_meets_the_contract() {
        conformance::run(MemStore::new);
    }

    #[test]
    fn a_reopened_memory_store_meets_the_contract() {
        let store = MemStore::new();
        conformance::run_reopen(|| store.reopen());
    }

    #[test]
    fn handles_of_one_memory_store_share_state_and_new_stores_do_not() {
        let mut store = MemStore::new();
        let handle = store.reopen();
        let blob = Blob::new(b"shared".to_vec());
        store
            .commit(&Commit {
                blobs: vec![blob.clone()],
                ..Commit::default()
            })
            .unwrap();
        assert_eq!(handle.blob_len(&blob.hash()), Ok(Some(6)));
        assert_eq!(MemStore::new().blob_len(&blob.hash()), Ok(None));
    }

    #[test]
    fn a_blob_is_only_accepted_under_its_own_hash() {
        let blob = Blob::new(b"content".to_vec());
        assert_eq!(blob.hash(), content_hash(b"content"));
        assert_eq!(Blob::verified(blob.hash(), b"content".to_vec()), Some(blob));
        assert_eq!(Blob::verified(BlobHash([0; 32]), b"content".to_vec()), None);
    }
}
