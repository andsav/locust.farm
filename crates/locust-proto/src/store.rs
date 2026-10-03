//! The storage seam between the state machine and durable storage.
//!
//! The state machine computes a [`Commit`]: the events, content objects and
//! local records that one operation produces. A store applies a commit
//! entirely or not at all. That single rule is what keeps an event, the local
//! state derived from it and the record that makes a retried request
//! idempotent from ever disagreeing after a crash.
//!
//! The trait is synchronous and has no notion of a runtime. One owner calls it
//! from one thread; nothing here needs a lock.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use crate::crypto::content_hash;
use crate::event::Event;
use crate::id::{BlobHash, EventId, GoalId, PublicKey};
use crate::sync::{AuthorFrontier, Frontier};

/// Content whose hash is known to match its bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blob {
    hash: BlobHash,
    bytes: Vec<u8>,
}

impl Blob {
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

    pub fn hash(&self) -> BlobHash {
        self.hash
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Namespaces for local, non-replicated records. Keys and values are opaque
/// to the store; the crate that owns a space defines their encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Space {
    /// Daemon identity, endpoint key and signing watermarks.
    Identity = 0,
    /// Enrolled principals: keys, names, grants and credential digests.
    Agent = 1,
    /// Per-goal local settings: export root, standing grants, halt state.
    Goal = 2,
    /// Known peers: contact hints and last successful synchronization.
    Peer = 3,
    /// Issued invitations and their redemption state.
    Invite = 4,
    /// Session-bound claims and their generations.
    Claim = 5,
    /// Consumer positions in a goal's event feed.
    Cursor = 6,
    /// Request keys with the digest and result of their first execution.
    Idempotency = 7,
    /// Obligations that cannot be rediscovered from the event log.
    Pending = 8,
    /// Managed client launches and session bindings.
    Session = 9,
}

/// One local record: its key and its value.
pub type LocalRecord = (Vec<u8>, Vec<u8>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalWrite {
    Put {
        space: Space,
        key: Vec<u8>,
        value: Vec<u8>,
    },
    Delete {
        space: Space,
        key: Vec<u8>,
    },
}

/// Everything one operation makes durable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Commit {
    /// Structurally valid events the caller has decided to retain.
    pub events: Vec<Event>,
    pub blobs: Vec<Blob>,
    /// Applied in order after the events and blobs.
    pub local: Vec<LocalWrite>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    /// The storage medium failed (I/O error, disk full). Nothing was applied.
    Unavailable(String),
    /// Stored data failed an integrity check.
    Corrupted(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(detail) => write!(f, "storage unavailable: {detail}"),
            Self::Corrupted(detail) => write!(f, "stored data is corrupted: {detail}"),
        }
    }
}

impl std::error::Error for StoreError {}

pub trait Store {
    /// Applies the whole commit or none of it. When this returns `Ok` the
    /// commit is durable, and every content object was durable before any
    /// record that names it became visible. Events and content already
    /// present are left untouched, so replaying a commit changes nothing.
    fn commit(&mut self, commit: Commit) -> Result<(), StoreError>;

    fn event(&self, id: &EventId) -> Result<Option<Event>, StoreError>;

    /// Events of a goal in local arrival order, each with its position.
    /// Positions start at 1, strictly increase and never change, so a position
    /// is a durable cursor. Returns events with a position greater than `after`.
    fn log(&self, goal: &GoalId, after: u64, limit: usize)
    -> Result<Vec<(u64, Event)>, StoreError>;

    /// One author's events from `from_seq`, ascending by sequence number and
    /// then identifier. Two events share a sequence number only when the
    /// author signed conflicting histories; both are returned.
    fn author_log(
        &self,
        goal: &GoalId,
        author: &PublicKey,
        from_seq: u64,
        limit: usize,
    ) -> Result<Vec<Event>, StoreError>;

    /// For every author in the goal, how many consecutive events are held
    /// counting from sequence zero.
    fn frontier(&self, goal: &GoalId) -> Result<Frontier, StoreError>;

    /// Goals with at least one stored event, ascending.
    fn goals(&self) -> Result<Vec<GoalId>, StoreError>;

    fn blob(&self, hash: &BlobHash) -> Result<Option<Vec<u8>>, StoreError>;

    fn has_blob(&self, hash: &BlobHash) -> Result<bool, StoreError>;

    fn get(&self, space: Space, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError>;

    /// Records in `space` whose key starts with `prefix`, ascending by key.
    fn scan(&self, space: Space, prefix: &[u8]) -> Result<Vec<LocalRecord>, StoreError>;
}

/// Reference implementation held in memory. It defines the behavior every
/// store must match and backs tests and early integration; it is not durable.
#[derive(Clone, Debug, Default)]
pub struct MemStore {
    events: HashMap<EventId, Event>,
    logs: BTreeMap<GoalId, Vec<EventId>>,
    authors: BTreeSet<(GoalId, PublicKey, u64, EventId)>,
    blobs: HashMap<BlobHash, Vec<u8>>,
    local: BTreeMap<(Space, Vec<u8>), Vec<u8>>,
}

impl MemStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn author_range(
        &self,
        goal: &GoalId,
        author: &PublicKey,
        from_seq: u64,
    ) -> impl Iterator<Item = &(GoalId, PublicKey, u64, EventId)> {
        let first = (*goal, *author, from_seq, EventId([0x00; 32]));
        let last = (*goal, *author, u64::MAX, EventId([0xff; 32]));
        self.authors.range(first..=last)
    }
}

impl Store for MemStore {
    fn commit(&mut self, commit: Commit) -> Result<(), StoreError> {
        for blob in commit.blobs {
            self.blobs.entry(blob.hash).or_insert(blob.bytes);
        }
        for event in commit.events {
            let id = event.id();
            if self.events.contains_key(&id) {
                continue;
            }
            let header = event.header();
            self.logs.entry(header.goal).or_default().push(id);
            self.authors
                .insert((header.goal, header.author, header.seq, id));
            self.events.insert(id, event);
        }
        for write in commit.local {
            match write {
                LocalWrite::Put { space, key, value } => {
                    self.local.insert((space, key), value);
                }
                LocalWrite::Delete { space, key } => {
                    self.local.remove(&(space, key));
                }
            }
        }
        Ok(())
    }

    fn event(&self, id: &EventId) -> Result<Option<Event>, StoreError> {
        Ok(self.events.get(id).cloned())
    }

    fn log(
        &self,
        goal: &GoalId,
        after: u64,
        limit: usize,
    ) -> Result<Vec<(u64, Event)>, StoreError> {
        let Some(log) = self.logs.get(goal) else {
            return Ok(Vec::new());
        };
        let skip = usize::try_from(after).unwrap_or(usize::MAX);
        Ok(log
            .iter()
            .enumerate()
            .skip(skip)
            .take(limit)
            .map(|(index, id)| (index as u64 + 1, self.events[id].clone()))
            .collect())
    }

    fn author_log(
        &self,
        goal: &GoalId,
        author: &PublicKey,
        from_seq: u64,
        limit: usize,
    ) -> Result<Vec<Event>, StoreError> {
        Ok(self
            .author_range(goal, author, from_seq)
            .take(limit)
            .map(|(_, _, _, id)| self.events[id].clone())
            .collect())
    }

    fn frontier(&self, goal: &GoalId) -> Result<Frontier, StoreError> {
        let first = (*goal, PublicKey([0x00; 32]), 0, EventId([0x00; 32]));
        let last = (*goal, PublicKey([0xff; 32]), u64::MAX, EventId([0xff; 32]));
        let mut authors: Vec<AuthorFrontier> = Vec::new();
        for (_, author, seq, _) in self.authors.range(first..=last) {
            match authors.last_mut() {
                Some(entry) if entry.author == *author => {
                    if *seq == entry.next_seq {
                        entry.next_seq += 1;
                    }
                }
                _ => authors.push(AuthorFrontier {
                    author: *author,
                    next_seq: u64::from(*seq == 0),
                }),
            }
        }
        Ok(Frontier { authors })
    }

    fn goals(&self) -> Result<Vec<GoalId>, StoreError> {
        Ok(self.logs.keys().copied().collect())
    }

    fn blob(&self, hash: &BlobHash) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.blobs.get(hash).cloned())
    }

    fn has_blob(&self, hash: &BlobHash) -> Result<bool, StoreError> {
        Ok(self.blobs.contains_key(hash))
    }

    fn get(&self, space: Space, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.local.get(&(space, key.to_vec())).cloned())
    }

    fn scan(&self, space: Space, prefix: &[u8]) -> Result<Vec<LocalRecord>, StoreError> {
        Ok(self
            .local
            .range((space, prefix.to_vec())..)
            .take_while(|((entry_space, key), _)| *entry_space == space && key.starts_with(prefix))
            .map(|((_, key), value)| (key.clone(), value.clone()))
            .collect())
    }
}

/// Behavior every [`Store`] must share. Run it against each implementation:
/// `conformance::run(MyStore::open_empty)`.
#[cfg(any(test, feature = "testkit"))]
pub mod conformance {
    use super::*;
    use crate::event::Body;
    use crate::testkit::Author;

    fn note() -> Body {
        Body::Note {
            about: None,
            supersedes: None,
        }
    }

    fn put(space: Space, key: &[u8], value: &[u8]) -> LocalWrite {
        LocalWrite::Put {
            space,
            key: key.to_vec(),
            value: value.to_vec(),
        }
    }

    /// Runs every check, each against a fresh store from `empty`.
    pub fn run<S: Store>(empty: impl Fn() -> S) {
        an_empty_store_answers_with_nothing(empty());
        a_commit_becomes_visible_as_a_whole(empty());
        replaying_a_commit_changes_nothing(empty());
        the_log_is_a_stable_cursor(empty());
        author_logs_keep_conflicting_events(empty());
        the_frontier_counts_consecutive_events(empty());
        local_records_are_scoped_and_ordered(empty());
    }

    fn an_empty_store_answers_with_nothing<S: Store>(store: S) {
        let goal = GoalId([1; 32]);
        assert_eq!(store.event(&EventId([1; 32])), Ok(None));
        assert_eq!(store.log(&goal, 0, 10), Ok(Vec::new()));
        assert_eq!(
            store.author_log(&goal, &PublicKey([1; 32]), 0, 10),
            Ok(Vec::new())
        );
        assert_eq!(store.frontier(&goal), Ok(Frontier::default()));
        assert_eq!(store.goals(), Ok(Vec::new()));
        assert_eq!(store.blob(&BlobHash([1; 32])), Ok(None));
        assert_eq!(store.has_blob(&BlobHash([1; 32])), Ok(false));
        assert_eq!(store.get(Space::Claim, b"key"), Ok(None));
        assert_eq!(store.scan(Space::Claim, b""), Ok(Vec::new()));
    }

    fn a_commit_becomes_visible_as_a_whole<S: Store>(mut store: S) {
        let mut owner = Author::new(1);
        let genesis = owner.genesis();
        let goal = genesis.header().goal;
        let blob = Blob::new(b"task text".to_vec());
        store
            .commit(Commit {
                events: vec![genesis.clone()],
                blobs: vec![blob.clone()],
                local: vec![put(Space::Goal, b"g", b"settings")],
            })
            .unwrap();

        assert_eq!(store.event(&genesis.id()), Ok(Some(genesis.clone())));
        assert_eq!(store.log(&goal, 0, 10), Ok(vec![(1, genesis)]));
        assert_eq!(store.goals(), Ok(vec![goal]));
        assert_eq!(store.blob(&blob.hash()), Ok(Some(b"task text".to_vec())));
        assert_eq!(store.has_blob(&blob.hash()), Ok(true));
        assert_eq!(store.get(Space::Goal, b"g"), Ok(Some(b"settings".to_vec())));
    }

    fn replaying_a_commit_changes_nothing<S: Store>(mut store: S) {
        let mut owner = Author::new(1);
        let genesis = owner.genesis();
        let goal = genesis.header().goal;
        let second = owner.event(goal, Some(genesis.id()), note());
        let commit = Commit {
            events: vec![genesis, second],
            blobs: vec![Blob::new(b"x".to_vec())],
            local: Vec::new(),
        };
        store.commit(commit.clone()).unwrap();
        let before = store.log(&goal, 0, 10).unwrap();
        store.commit(commit).unwrap();
        assert_eq!(store.log(&goal, 0, 10).unwrap(), before);
        assert_eq!(before.len(), 2);
    }

    fn the_log_is_a_stable_cursor<S: Store>(mut store: S) {
        let mut owner = Author::new(1);
        let genesis = owner.genesis();
        let goal = genesis.header().goal;
        let mut events = vec![genesis.clone()];
        for _ in 0..4 {
            events.push(owner.event(goal, Some(genesis.id()), note()));
        }
        for event in &events {
            store
                .commit(Commit {
                    events: vec![event.clone()],
                    ..Commit::default()
                })
                .unwrap();
        }

        let positions = |entries: &[(u64, Event)]| -> Vec<u64> {
            entries.iter().map(|(position, _)| *position).collect()
        };
        assert_eq!(
            positions(&store.log(&goal, 0, 10).unwrap()),
            [1, 2, 3, 4, 5]
        );
        assert_eq!(positions(&store.log(&goal, 0, 2).unwrap()), [1, 2]);
        let tail = store.log(&goal, 3, 10).unwrap();
        assert_eq!(positions(&tail), [4, 5]);
        assert_eq!(tail[0].1, events[3]);
        assert_eq!(store.log(&goal, 5, 10), Ok(Vec::new()));
        assert_eq!(store.log(&goal, u64::MAX, 10), Ok(Vec::new()));
        assert_eq!(store.log(&GoalId([9; 32]), 0, 10), Ok(Vec::new()));
    }

    fn author_logs_keep_conflicting_events<S: Store>(mut store: S) {
        let mut owner = Author::new(1);
        let genesis = owner.genesis();
        let goal = genesis.header().goal;
        // The same key continues its log twice from the same point.
        let mut twin = Author::new(1);
        assert_eq!(twin.genesis(), genesis);
        let first = owner.event(goal, Some(genesis.id()), note());
        let conflicting = twin.event(
            goal,
            Some(genesis.id()),
            Body::Note {
                about: Some(genesis.id()),
                supersedes: None,
            },
        );
        let third = owner.event(goal, Some(genesis.id()), note());
        store
            .commit(Commit {
                events: vec![
                    third.clone(),
                    conflicting.clone(),
                    genesis.clone(),
                    first.clone(),
                ],
                ..Commit::default()
            })
            .unwrap();

        let author = genesis.header().author;
        let mut at_one = vec![first, conflicting];
        at_one.sort_by_key(Event::id);
        let mut expected = vec![genesis];
        expected.extend(at_one.clone());
        expected.push(third.clone());
        assert_eq!(store.author_log(&goal, &author, 0, 10), Ok(expected));

        at_one.push(third);
        assert_eq!(store.author_log(&goal, &author, 1, 10), Ok(at_one.clone()));
        assert_eq!(
            store.author_log(&goal, &author, 1, 2),
            Ok(at_one[..2].to_vec())
        );
        assert_eq!(
            store.author_log(&goal, &PublicKey([9; 32]), 0, 10),
            Ok(Vec::new())
        );
    }

    fn the_frontier_counts_consecutive_events<S: Store>(mut store: S) {
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

        // The owner's third event and the member's second arrive ahead of a gap.
        store
            .commit(Commit {
                events: vec![genesis, owner_first, owner_third, member_second],
                ..Commit::default()
            })
            .unwrap();
        let frontier = store.frontier(&goal).unwrap();
        assert_eq!(frontier.next_seq(&owner.key.public()), 2);
        assert_eq!(frontier.next_seq(&member.key.public()), 0);
        assert!(frontier.authors.is_sorted_by(|a, b| a.author < b.author));

        store
            .commit(Commit {
                events: vec![owner_second, member_first],
                ..Commit::default()
            })
            .unwrap();
        let frontier = store.frontier(&goal).unwrap();
        assert_eq!(frontier.next_seq(&owner.key.public()), 4);
        assert_eq!(frontier.next_seq(&member.key.public()), 2);
    }

    fn local_records_are_scoped_and_ordered<S: Store>(mut store: S) {
        store
            .commit(Commit {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_memory_store_meets_the_contract() {
        conformance::run(MemStore::new);
    }

    #[test]
    fn a_blob_is_only_accepted_under_its_own_hash() {
        let blob = Blob::new(b"content".to_vec());
        assert_eq!(blob.hash(), content_hash(b"content"));
        assert_eq!(Blob::verified(blob.hash(), b"content".to_vec()), Some(blob));
        assert_eq!(Blob::verified(BlobHash([0; 32]), b"content".to_vec()), None);
    }
}
