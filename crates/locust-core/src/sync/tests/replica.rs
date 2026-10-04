//! A minimal replica over `MemStore`: events per author, payload objects and
//! keys, with no goal semantics.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use locust_proto::crypto::ContentKey;
use locust_proto::event::{AuthorPoint, Event, PayloadRef, WireEvent};
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey};
use locust_proto::seal;
use locust_proto::store::{Blob, Commit, MemStore, Store};
use locust_proto::sync::{AuthorFrontier, Frontier, Refusal};

use crate::sync::{Replica, Staged};

pub struct TestReplica {
    pub goal: GoalId,
    pub store: MemStore,
    points: BTreeMap<PublicKey, Vec<AuthorPoint>>,
    payloads: BTreeMap<BlobHash, PayloadRef>,
    keys: BTreeMap<u32, ContentKey>,
    /// Objects wanted besides payloads, with their stored lengths.
    pub wants: BTreeMap<BlobHash, u64>,
    /// Objects served with one byte flipped.
    pub corrupt: HashSet<BlobHash>,
    /// Raised by every new event, object or key.
    pub revision: u64,
    /// Events received from peers that were new.
    pub received: usize,
}

impl TestReplica {
    /// Replays what `store` holds of `goal`.
    pub fn open(goal: GoalId, store: MemStore) -> Self {
        let mut replica = Self {
            goal,
            store,
            points: BTreeMap::new(),
            payloads: BTreeMap::new(),
            keys: BTreeMap::new(),
            wants: BTreeMap::new(),
            corrupt: HashSet::new(),
            revision: 0,
            received: 0,
        };
        let held: Vec<Event> = replica
            .store
            .log(&goal, 0, usize::MAX)
            .unwrap()
            .into_iter()
            .map(|(_, event)| event)
            .collect();
        replica.index(&held);
        replica
    }

    pub fn new(goal: GoalId) -> Self {
        Self::open(goal, MemStore::new())
    }

    /// Commits events written locally.
    pub fn insert(&mut self, events: &[Event]) {
        let fresh: Vec<Event> = events
            .iter()
            .filter(|event| !self.store.has_event(&event.id()).unwrap())
            .cloned()
            .collect();
        self.store
            .commit(&Commit {
                events: fresh.clone(),
                ..Commit::default()
            })
            .unwrap();
        self.index(&fresh);
    }

    pub fn add_blob(&mut self, blob: Blob) {
        self.store
            .commit(&Commit {
                blobs: vec![blob],
                ..Commit::default()
            })
            .unwrap();
        self.revision += 1;
    }

    pub fn set_key(&mut self, epoch: u32, key: ContentKey) {
        self.keys.insert(epoch, key);
    }

    pub fn ids(&self) -> BTreeSet<EventId> {
        self.points
            .values()
            .flatten()
            .map(|point| point.id)
            .collect()
    }

    pub fn holds_blob(&self, hash: &BlobHash) -> bool {
        self.store.blob_len(hash).unwrap().is_some()
    }

    fn index(&mut self, events: &[Event]) {
        for event in events {
            let header = event.header();
            let point = AuthorPoint {
                seq: header.seq,
                id: event.id(),
            };
            let points = self.points.entry(header.author).or_default();
            if let Err(at) = points.binary_search(&point) {
                points.insert(at, point);
                self.revision += 1;
            }
            if let Some(payload) = header.payload {
                self.payloads.insert(payload.hash, payload);
            }
        }
    }

    fn expected_len(&self, hash: &BlobHash) -> Option<u64> {
        self.payloads
            .get(hash)
            .map(|payload| u64::from(payload.len))
            .or_else(|| self.wants.get(hash).copied())
    }
}

impl Replica for TestReplica {
    fn frontier(&self) -> Frontier {
        Frontier {
            authors: self
                .points
                .iter()
                .map(|(author, points)| AuthorFrontier::from_points(*author, points))
                .collect(),
        }
    }

    fn extends(&self, theirs: &AuthorFrontier) -> bool {
        theirs.is_prefix_of(self.points(&theirs.author))
    }

    fn points(&self, author: &PublicKey) -> &[AuthorPoint] {
        self.points.get(author).map_or(&[], Vec::as_slice)
    }

    fn wire_event(&self, id: &EventId) -> Option<WireEvent> {
        self.store.event(id).unwrap().map(|event| event.to_wire())
    }

    /// Also checks what every frame must keep: each author's log ascending.
    fn receive(&mut self, events: Vec<WireEvent>) -> Result<usize, Refusal> {
        let mut last: BTreeMap<PublicKey, AuthorPoint> = BTreeMap::new();
        let mut fresh = Vec::new();
        for wire in &events {
            let event = Event::from_wire(wire).map_err(|_| Refusal::ProtocolError)?;
            let header = event.header();
            if header.goal != self.goal {
                return Err(Refusal::ProtocolError);
            }
            let point = AuthorPoint {
                seq: header.seq,
                id: event.id(),
            };
            if let Some(previous) = last.insert(header.author, point) {
                assert!(previous < point, "a frame kept an author's log ascending");
            }
            if !self.store.has_event(&event.id()).unwrap() {
                fresh.push(event);
            }
        }
        let count = fresh.len();
        self.received += count;
        self.insert(&fresh);
        Ok(count)
    }

    fn key(&self, epoch: u32) -> Option<ContentKey> {
        self.keys.get(&epoch).copied()
    }

    fn wanted_keys(&self) -> Vec<u32> {
        let epochs: BTreeSet<u32> = self
            .payloads
            .values()
            .map(|payload| payload.key_epoch)
            .filter(|epoch| !self.keys.contains_key(epoch))
            .collect();
        epochs.into_iter().collect()
    }

    fn offer_key(&mut self, epoch: u32, key: ContentKey) -> bool {
        let opens = self
            .payloads
            .values()
            .filter(|payload| payload.key_epoch == epoch)
            .filter_map(|payload| self.store.blob(&payload.hash).unwrap())
            .any(|sealed| seal::open(&self.goal, &key, &sealed).is_ok());
        if opens {
            self.keys.insert(epoch, key);
            self.revision += 1;
        }
        opens
    }

    fn wanted_blobs(&self, limit: usize) -> Vec<(BlobHash, u64)> {
        self.payloads
            .keys()
            .chain(self.wants.keys())
            .filter(|hash| !self.holds_blob(hash))
            .map(|hash| (*hash, self.store.staged_len(hash).unwrap()))
            .take(limit)
            .collect()
    }

    fn blob_len(&self, hash: &BlobHash) -> Option<u64> {
        self.store.blob_len(hash).unwrap()
    }

    fn blob_range(&self, hash: &BlobHash, offset: u64, len: usize) -> Option<Vec<u8>> {
        let mut bytes = self.store.blob_range(hash, offset, len).unwrap()?;
        if self.corrupt.contains(hash)
            && let Some(byte) = bytes.first_mut()
        {
            *byte ^= 0xff;
        }
        Some(bytes)
    }

    fn stage(&mut self, hash: &BlobHash, offset: u64, total: u64, bytes: &[u8]) -> Staged {
        if self.expected_len(hash) != Some(total) {
            return Staged::Rejected;
        }
        if self.holds_blob(hash) {
            return Staged::Complete;
        }
        let staged = self.store.stage_blob(hash, offset, bytes).unwrap();
        if staged < total {
            return Staged::More(staged);
        }
        if self.store.finish_blob(hash).unwrap() {
            self.revision += 1;
            Staged::Complete
        } else {
            Staged::Rejected
        }
    }
}
