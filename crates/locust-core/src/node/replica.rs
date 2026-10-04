//! Durable reconciliation adapter. The driver selects one goal before each
//! call; received event batches use the node's single transactional write path.
use std::collections::BTreeSet;

use locust_proto::crypto::ContentKey;
use locust_proto::engine::Entropy;
use locust_proto::event::{AuthorPoint, Body, Event, WireEvent};
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey};
use locust_proto::limits::MAX_BLOB_BYTES;
use locust_proto::seal;
use locust_proto::store::{Space, Store};
use locust_proto::sync::{AuthorFrontier, Frontier, Refusal};

use super::commit::Tx;
use super::requests::content::{BlobRecord, blob_record, blob_write};
use super::{Node, entry, local, records};
use crate::sync::{Replica, Staged};

impl<S: Store, E: Entropy> Node<S, E> {
    fn replica_id(&self) -> GoalId {
        self.replica_goal.expect("Host selected a goal")
    }

    fn allowed_blob(&self, hash: &BlobHash) -> bool {
        let entry = &self.goals[&self.replica_id()];
        let Ok(record) = blob_record(&self.store, &entry.id(), hash) else {
            return false;
        };
        !record.is_some_and(|record| record.withdrawn) && (entry.names(hash) || record.is_some())
    }

    /// Checks goal-scoped reference metadata without reading full objects.
    /// A conflicting reference never invalidates another matching reference.
    pub(in crate::node) fn blob_metadata_admitted(
        &self,
        goal: &GoalId,
        hash: &BlobHash,
        len: u64,
        prefix: &[u8],
    ) -> bool {
        let Ok(epoch) = seal::epoch_of(prefix) else {
            return false;
        };
        if len < seal::OVERHEAD_BYTES as u64 || len > MAX_BLOB_BYTES as u64 {
            return false;
        }
        let Some(entry) = self.goals.get(goal) else {
            return false;
        };
        if !entry.names(hash) {
            return epoch <= entry.state().epoch
                && blob_record(&self.store, goal, hash)
                    .ok()
                    .flatten()
                    .is_some();
        }
        entry
            .goal
            .authors()
            .flat_map(|author| entry.goal.points(author))
            .filter_map(|point| entry.goal.event(&point.id))
            .any(|event| {
                let header = event.header();
                if header.payload.is_some_and(|payload| {
                    payload.hash == *hash
                        && u64::from(payload.len) == len
                        && payload.key_epoch == epoch
                }) {
                    return true;
                }
                header
                    .blobs()
                    .into_iter()
                    .skip(usize::from(header.payload.is_some()))
                    .any(|named| named == *hash)
                    && entry
                        .goal
                        .epoch_of(&event.id())
                        .is_some_and(|event_epoch| epoch <= event_epoch)
            })
    }

    fn held_metadata_admitted(&self, hash: &BlobHash) -> bool {
        let Some(len) = self.store.blob_len(hash).ok().flatten() else {
            return false;
        };
        let Some(prefix) = self
            .store
            .blob_range(hash, 0, seal::OVERHEAD_BYTES)
            .ok()
            .flatten()
        else {
            return false;
        };
        self.blob_metadata_admitted(&self.replica_id(), hash, len, &prefix)
    }

    fn named_blobs(&self) -> impl Iterator<Item = BlobHash> + '_ {
        let entry = &self.goals[&self.replica_id()];
        entry
            .goal
            .authors()
            .flat_map(move |author| entry.goal.points(author))
            .filter_map(move |point| entry.goal.event(&point.id))
            .flat_map(|event| event.header().blobs())
    }
}

impl<S: Store, E: Entropy> Replica for Node<S, E> {
    fn frontier(&self) -> Frontier {
        self.goals[&self.replica_id()].goal.frontier()
    }
    fn extends(&self, theirs: &AuthorFrontier) -> bool {
        self.goals[&self.replica_id()].goal.extends(theirs)
    }
    fn points(&self, author: &PublicKey) -> &[AuthorPoint] {
        self.goals[&self.replica_id()].goal.points(author)
    }
    fn wire_event(&self, id: &EventId) -> Option<WireEvent> {
        self.goals[&self.replica_id()]
            .goal
            .event(id)
            .map(Event::to_wire)
    }
    fn receive(&mut self, events: Vec<WireEvent>) -> Result<usize, Refusal> {
        if self.failed {
            return Err(Refusal::ProtocolError);
        }
        let goal = self.replica_id();
        let mut decoded = Vec::with_capacity(events.len());
        for wire in events {
            let event = Event::from_wire(&wire).map_err(|_| Refusal::ProtocolError)?;
            if event.header().goal != goal {
                return Err(Refusal::ProtocolError);
            }
            if let Body::Genesis(genesis) = &event.header().body
                && self.goals[&goal]
                    .local
                    .joins
                    .values()
                    .any(|join| join.coordinator != genesis.coordinator)
            {
                return Err(Refusal::InvitationRefused);
            }
            decoded.push(event);
        }
        let screened = self.goals[&goal].goal.screen(decoded);
        let count = screened.len();
        if count > 0 {
            let mut tx = Tx::none();
            tx.commit.events = screened;
            self.land(tx).map_err(|_| Refusal::ProtocolError)?;
        }
        Ok(count)
    }
    fn key(&self, epoch: u32) -> Option<ContentKey> {
        self.goals[&self.replica_id()].keys.get(&epoch).copied()
    }
    fn wanted_keys(&self) -> Vec<u32> {
        let entry = &self.goals[&self.replica_id()];
        entry
            .goal
            .authors()
            .flat_map(|author| entry.goal.points(author))
            .filter_map(|point| entry.goal.event(&point.id)?.header().payload)
            .map(|payload| payload.key_epoch)
            .filter(|epoch| !entry.keys.contains_key(epoch))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    fn offer_key(&mut self, epoch: u32, key: ContentKey) -> bool {
        let goal = self.replica_id();
        let entry = &self.goals[&goal];
        let proof = entry
            .goal
            .authors()
            .flat_map(|author| entry.goal.points(author))
            .filter_map(|point| entry.goal.event(&point.id))
            .filter(|event| {
                matches!(
                    entry.goal.standing(&event.id()),
                    Some(crate::goal::Standing::Effective)
                )
            })
            .find_map(|event| {
                let valid = if epoch == 0 {
                    matches!(event.header().body, Body::Genesis(_))
                } else {
                    matches!(event.header().body, Body::MemberRemoved { .. })
                        && entry.goal.epoch_of(&event.id()) == Some(epoch)
                };
                valid
                    .then_some(event.header().payload)
                    .flatten()
                    .filter(|payload| payload.key_epoch == epoch)
            });
        let Some(proof) = proof else {
            return false;
        };
        let Some(sealed) = self.store.blob(&proof.hash).ok().flatten() else {
            return false;
        };
        if seal::epoch_of(&sealed).ok() != Some(epoch) {
            return false;
        }
        let Ok(plain) = seal::open(&goal, &key, &sealed) else {
            return false;
        };
        let mut tx = Tx::none();
        tx.local(entry::key_write(&goal, epoch, &key)).touch(goal);
        if epoch == 0
            && let Ok(title) = String::from_utf8(plain)
        {
            tx.local(local::title_write(&goal, &title));
        }
        self.land(tx).is_ok()
    }
    fn wanted_blobs(&self, limit: usize) -> Vec<(BlobHash, u64)> {
        let mut result = Vec::new();
        let mut after = None;
        while result.len() < limit {
            let Some(item) = self.next_wanted_blob(after) else {
                break;
            };
            after = Some(item.0);
            result.push(item);
        }
        result
    }
    fn next_wanted_blob(&self, after: Option<BlobHash>) -> Option<(BlobHash, u64)> {
        let goal = self.replica_id();
        let explicit = self.store.scan(Space::Blob, &goal.0).ok()?;
        let wanted = explicit.iter().filter_map(|(key, value)| {
            let record: BlobRecord = records::read(value).ok()?;
            if !record.wanted || record.withdrawn {
                return None;
            }
            records::part(key, GoalId::LEN).map(BlobHash)
        });
        let hash = self
            .named_blobs()
            .chain(wanted)
            .filter(|hash| after.is_none_or(|after| *hash > after))
            .filter(|hash| self.allowed_blob(hash))
            .filter(|hash| self.store.blob_len(hash).ok() == Some(None))
            .min()?;
        Some((hash, self.store.staged_len(&hash).ok()?))
    }
    fn blob_len(&self, hash: &BlobHash) -> Option<u64> {
        if !self.allowed_blob(hash) || !self.held_metadata_admitted(hash) {
            return None;
        }
        self.store.blob_len(hash).ok().flatten()
    }
    fn blob_range(&self, hash: &BlobHash, offset: u64, len: usize) -> Option<Vec<u8>> {
        if !self.allowed_blob(hash) || !self.held_metadata_admitted(hash) {
            return None;
        }
        self.store.blob_range(hash, offset, len).ok().flatten()
    }
    fn stage(&mut self, hash: &BlobHash, offset: u64, total: u64, bytes: &[u8]) -> Staged {
        if self.failed {
            return Staged::Rejected;
        }
        if !self.allowed_blob(hash) {
            return Staged::Rejected;
        }
        if total > MAX_BLOB_BYTES as u64
            || offset
                .checked_add(bytes.len() as u64)
                .is_none_or(|end| end > total)
        {
            return Staged::Rejected;
        }
        if self.store.blob_len(hash).ok().flatten().is_some() {
            return if self.held_metadata_admitted(hash) {
                Staged::Complete
            } else {
                Staged::Rejected
            };
        }
        let Ok(staged) = self.store.stage_blob(hash, offset, bytes) else {
            self.failed = true;
            return Staged::Rejected;
        };
        if staged < total {
            return Staged::More(staged);
        }
        let prefix = match self.store.staged_range(hash, 0, seal::OVERHEAD_BYTES) {
            Ok(Some(prefix)) => prefix,
            Ok(None) => return Staged::Rejected,
            Err(_) => {
                self.failed = true;
                return Staged::Rejected;
            }
        };
        if staged != total || !self.blob_metadata_admitted(&self.replica_id(), hash, total, &prefix)
        {
            if self.store.discard_staged_blob(hash).is_err() {
                self.failed = true;
            }
            return Staged::Rejected;
        }
        match self.store.finish_blob(hash) {
            Ok(true) => {
                let mut tx = Tx::none();
                tx.local(blob_write(&self.replica_id(), hash, &BlobRecord::default()))
                    .touch(self.replica_id());
                if self.land(tx).is_ok() {
                    Staged::Complete
                } else {
                    Staged::Rejected
                }
            }
            Ok(false) => Staged::Rejected,
            Err(_) => {
                self.failed = true;
                Staged::Rejected
            }
        }
    }
}

#[cfg(test)]
#[path = "replica_tests.rs"]
mod tests;
