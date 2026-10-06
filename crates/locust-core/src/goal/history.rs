//! The events held for one goal: decoded once, indexed by identifier, with a
//! log per author that answers frontier and prefix questions by lookup.

use std::collections::BTreeMap;

use locust_proto::event::{AuthorPoint, Body, Event};
use locust_proto::id::{EventId, PublicKey};
use locust_proto::sync::{AuthorFrontier, EMPTY_LOG_DIGEST, Frontier, log_digest_step};

use super::ids::IdMap;

/// Where a held event lives in [`History::events`].
pub(super) type Slot = usize;

/// One author's held events.
#[derive(Clone, Default)]
pub(super) struct AuthorLog {
    /// Every held point, ascending by (position, identifier).
    pub points: Vec<AuthorPoint>,
    /// The slot of each point, in the same order.
    pub slots: Vec<Slot>,
    /// The running digest after each point below `next_seq`.
    digests: Vec<[u8; 32]>,
    /// The first position not held.
    next_seq: u64,
    /// How many leading points form the usable prefix: positions `0..usable`,
    /// each held once, each naming the one before it as `prev`.
    pub usable: usize,
    /// The lowest position holding two different events.
    pub fork: Option<u64>,
}

/// How a newly held event sits in its author's log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Placed {
    /// It extends the usable prefix and nothing is held after it.
    Tip,
    /// It extends the usable prefix and later events were waiting for it.
    Fills,
    /// It is past the usable prefix of a log without a fork.
    Beyond,
    /// Its position already held another event, or the log is forked.
    Forked,
}

impl AuthorLog {
    /// The digest over the first `count` points.
    fn digest_before(&self, count: usize) -> [u8; 32] {
        match count.checked_sub(1) {
            Some(last) => self.digests[last],
            None => EMPTY_LOG_DIGEST,
        }
    }

    /// The points below the first position not held.
    fn contiguous(&self) -> usize {
        self.digests.len()
    }

    /// The number of events past the usable prefix.
    pub fn waiting(&self) -> usize {
        self.points.len() - self.usable
    }

    /// The last event of the usable prefix.
    pub fn tip(&self) -> Option<EventId> {
        Some(self.points[self.usable.checked_sub(1)?].id)
    }

    /// Usable positions are exactly `0..usable`, with one event at each index.
    pub fn contains_usable(&self, point: AuthorPoint) -> bool {
        usize::try_from(point.seq)
            .ok()
            .and_then(|index| self.points[..self.usable].get(index))
            == Some(&point)
    }

    fn insert(&mut self, point: AuthorPoint, slot: Slot, events: &[Event]) -> Placed {
        let at = self.points.partition_point(|held| *held < point);
        let shares = |held: Option<&AuthorPoint>| held.is_some_and(|held| held.seq == point.seq);
        if shares(self.points.get(at)) || shares(at.checked_sub(1).and_then(|i| self.points.get(i)))
        {
            self.fork = Some(self.fork.map_or(point.seq, |fork| fork.min(point.seq)));
            // A fork inside the usable prefix ends the prefix at that position.
            self.usable = self.usable.min(point.seq as usize);
        }
        self.points.insert(at, point);
        self.slots.insert(at, slot);

        // Digests cover the points below the first missing position.
        self.digests.truncate(at);
        if at == self.digests.len() {
            while let Some(next) = self.points.get(self.digests.len()) {
                if next.seq > self.next_seq {
                    break;
                }
                self.next_seq = self.next_seq.max(next.seq + 1);
                let state = self.digest_before(self.digests.len());
                self.digests.push(log_digest_step(state, next));
            }
        }

        let before = self.usable;
        while let Some(next) = self.points.get(self.usable) {
            let index = self.usable;
            let forked = self
                .points
                .get(index + 1)
                .is_some_and(|after| after.seq == next.seq);
            let prev = events[self.slots[index]].header().prev;
            if next.seq != index as u64 || forked || prev != self.tip() {
                break;
            }
            self.usable += 1;
        }

        if self.fork.is_some() {
            Placed::Forked
        } else if at >= before && at < self.usable {
            if self.usable == self.points.len() && at + 1 == self.usable {
                Placed::Tip
            } else {
                Placed::Fills
            }
        } else {
            Placed::Beyond
        }
    }

    fn frontier(&self, author: PublicKey) -> AuthorFrontier {
        AuthorFrontier {
            author,
            next_seq: self.next_seq,
            digest: self.digest_before(self.contiguous()),
        }
    }
}

/// Every event held for one goal.
#[derive(Clone, Default)]
pub(super) struct History {
    /// Held events in arrival order.
    pub events: Vec<Event>,
    index: IdMap<Slot>,
    pub logs: BTreeMap<PublicKey, AuthorLog>,
    /// The author of a held genesis event.
    pub governance: Option<PublicKey>,
}

impl History {
    pub fn slot(&self, id: &EventId) -> Option<Slot> {
        self.index.get(id).copied()
    }

    pub fn get(&self, id: &EventId) -> Option<&Event> {
        Some(&self.events[self.slot(id)?])
    }

    pub fn log(&self, author: &PublicKey) -> Option<&AuthorLog> {
        self.logs.get(author)
    }

    /// Holds `event`. `None` if it was held already.
    pub fn insert(&mut self, event: &Event) -> Option<(Slot, Placed)> {
        let slot = self.events.len() as Slot;
        match self.index.entry(event.id()) {
            std::collections::hash_map::Entry::Occupied(_) => return None,
            std::collections::hash_map::Entry::Vacant(entry) => entry.insert(slot),
        };
        self.events.push(event.clone());
        let header = event.header();
        if let Body::Genesis(genesis) = &header.body {
            self.governance = Some(genesis.governance);
        }
        let point = AuthorPoint {
            seq: header.seq,
            id: event.id(),
        };
        let log = self.logs.entry(header.author).or_default();
        Some((slot, log.insert(point, slot, &self.events)))
    }

    pub fn frontier(&self) -> Frontier {
        Frontier {
            authors: self
                .logs
                .iter()
                .map(|(author, log)| log.frontier(*author))
                .collect(),
        }
    }

    pub fn frontier_of(&self, author: &PublicKey) -> AuthorFrontier {
        match self.logs.get(author) {
            Some(log) => log.frontier(*author),
            None => AuthorFrontier::from_points(*author, &[]),
        }
    }

    /// [`AuthorFrontier::is_prefix_of`] by lookup: the peer's count is at
    /// most ours, and our digest over the points below it equals theirs.
    pub fn extends(&self, theirs: &AuthorFrontier) -> bool {
        let Some(log) = self.logs.get(&theirs.author) else {
            return theirs.next_seq == 0 && theirs.digest == EMPTY_LOG_DIGEST;
        };
        let below = log
            .points
            .partition_point(|point| point.seq < theirs.next_seq);
        theirs.next_seq <= log.next_seq && log.digest_before(below) == theirs.digest
    }
}
