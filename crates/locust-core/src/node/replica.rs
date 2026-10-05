//! Durable reconciliation adapter. The driver selects one goal before each
//! call; received event batches use the node's single transactional write path.
use std::collections::BTreeSet;
use std::ops::Bound::{Excluded, Unbounded};

use locust_proto::crypto::ContentKey;
use locust_proto::engine::Entropy;
use locust_proto::event::{AuthorPoint, Body, Event, WireEvent};
use locust_proto::id::{BlobHash, EffectId, EndpointId, EventId, GoalId, PublicKey};
use locust_proto::limits::MAX_BLOB_BYTES;
use locust_proto::seal;
use locust_proto::store::{Store, StoreError};
use locust_proto::sync::{AuthorFrontier, Frontier, Refusal};

use super::commit::Tx;
use super::requests::content::blob_record;
use super::{Node, entry, local};
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
        !record.is_some_and(|record| record.withdrawn)
            && (self.names_content(entry, hash) || record.is_some())
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

    fn blob_length_admitted(&self, hash: &BlobHash, total: u64) -> bool {
        let goal = self.replica_id();
        self.blob_index.admits_length(&goal, hash, total)
            || (!self.names_content(&self.goals[&goal], hash)
                && blob_record(&self.store, &goal, hash)
                    .ok()
                    .flatten()
                    .is_some())
    }

    fn stage_bytes(
        &mut self,
        hash: &BlobHash,
        offset: u64,
        total: u64,
        bytes: &[u8],
    ) -> Result<Option<u64>, StoreError> {
        let mut staged = self.store.staged_len(hash)?;
        if offset < staged {
            let overlap = bytes.len().min((staged - offset) as usize);
            let held = self.store.staged_range(hash, offset, overlap)?;
            let matches = held.as_deref() == Some(&bytes[..overlap]);
            if staged > total || !matches {
                // An unavailable resume alone changes no shared staging.
                // A retry at zero may replace incompatible bytes only once
                // the new stream has supplied admissible sealed metadata.
                if offset != 0
                    || !self.blob_metadata_admitted(&self.replica_id(), hash, total, bytes)
                {
                    return Ok(None);
                }
                self.store.discard_staged_blob(hash)?;
                staged = 0;
            } else if overlap == bytes.len() {
                return Ok(Some(staged));
            } else {
                return self
                    .store
                    .stage_blob(hash, staged, &bytes[overlap..])
                    .map(Some);
            }
        }
        if offset != staged {
            return Ok(None);
        }
        self.store.stage_blob(hash, offset, bytes).map(Some)
    }
}

impl<S: Store, E: Entropy> Replica for Node<S, E> {
    fn next_delivery(
        &self,
        remote: EndpointId,
        after: Option<(EffectId, PublicKey)>,
    ) -> Option<(EffectId, PublicKey)> {
        if self.failed {
            return None;
        }
        self.goals[&self.replica_id()]
            .deliveries
            .iter()
            .find(|(key, record)| {
                after.is_none_or(|after| **key > after)
                    && record.endpoint == remote
                    && record.available
                    && !record.delivered
            })
            .map(|(key, _)| *key)
    }

    fn receive_delivery(
        &mut self,
        effect: EffectId,
        recipient: PublicKey,
    ) -> Result<bool, Refusal> {
        if self.failed {
            return Err(Refusal::ProtocolError);
        }
        // The inbox was committed atomically with the verified event projection.
        // No missing definition, foreign recipient, or disputed effect earns a receipt.
        Ok(self.goals[&self.replica_id()]
            .deliveries
            .get(&(effect, recipient))
            .is_some_and(|record| {
                record.available
                    && record.received
                    && self.principals.active(&recipient).is_some()
                    && self
                        .identity
                        .endpoint
                        .as_ref()
                        .is_some_and(|local| local.endpoint == record.endpoint)
            }))
    }

    fn receive_receipt(
        &mut self,
        remote: EndpointId,
        effect: EffectId,
        recipient: PublicKey,
    ) -> Result<(), Refusal> {
        if self.failed {
            return Err(Refusal::ProtocolError);
        }
        let goal = self.replica_id();
        let mut record = self.goals[&goal]
            .deliveries
            .get(&(effect, recipient))
            .filter(|record| record.endpoint == remote)
            .cloned()
            .ok_or(Refusal::ProtocolError)?;
        if !record.delivered {
            record.delivered = true;
            let mut tx = Tx::none();
            tx.local(super::delivery::write(goal, effect, recipient, &record))
                .touch(goal);
            self.land_once(tx).map_err(|_| Refusal::ProtocolError)?;
        }
        Ok(())
    }

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
                    .any(|join| join.administrator != genesis.administrator)
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
            .filter_map(|point| entry.goal.event(&point.id))
            .flat_map(|event| {
                let definition = match &event.header().body {
                    Body::RulesBound { binding, .. } => Some(binding.definition.object.key_epoch),
                    _ => None,
                };
                event
                    .header()
                    .payload
                    .map(|payload| payload.key_epoch)
                    .into_iter()
                    .chain(definition)
            })
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
    fn founding_blob(&self) -> Option<(BlobHash, u64)> {
        let goal = self.replica_id();
        let entry = &self.goals[&goal];
        let wanted = self.blob_index.wanted(&goal)?;
        entry
            .goal
            .authors()
            .flat_map(|author| entry.goal.points(author))
            .find_map(|point| {
                let event = entry.goal.event(&point.id)?;
                if !matches!(event.header().body, Body::Genesis(_))
                    || entry.goal.standing(&point.id) != Some(crate::goal::Standing::Effective)
                {
                    return None;
                }
                let hash = event.header().payload?.hash;
                wanted
                    .contains(&hash)
                    .then(|| {
                        self.store
                            .staged_len(&hash)
                            .ok()
                            .map(|offset| (hash, offset))
                    })
                    .flatten()
            })
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
        let set = self.blob_index.wanted(&self.replica_id())?;
        let hash = *set
            .range((after.map_or(Unbounded, Excluded), Unbounded))
            .next()?;
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
        if total < seal::OVERHEAD_BYTES as u64
            || total > MAX_BLOB_BYTES as u64
            || !self.blob_length_admitted(hash, total)
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
        let staged = match self.stage_bytes(hash, offset, total, bytes) {
            Ok(Some(staged)) => staged,
            Ok(None) => return Staged::Rejected,
            Err(_) => {
                self.failed = true;
                return Staged::Rejected;
            }
        };
        if staged < seal::OVERHEAD_BYTES as u64 {
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
        if staged > total || !self.blob_metadata_admitted(&self.replica_id(), hash, total, &prefix)
        {
            if self.store.discard_staged_blob(hash).is_err() {
                self.failed = true;
            }
            return Staged::Rejected;
        }
        if staged < total {
            return Staged::More(staged);
        }
        match self.store.finish_blob(hash) {
            Ok(true) => {
                // A received descendant is authorized by its path, never by
                // a new independent local publication record.
                let mut tx = Tx::none();
                tx.arrived.push(*hash);
                tx.touch(self.replica_id());
                if self.land(tx).is_err() {
                    return Staged::Rejected;
                }
                if self.content_arrived(*hash).is_err() {
                    self.failed = true;
                    Staged::Rejected
                } else {
                    Staged::Complete
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
