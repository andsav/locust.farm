//! Goal-scoped plaintext access and durable content availability records.

use locust_proto::api::{ApiError, BlobState, BlobStatus, ErrorCode, Response};
use locust_proto::engine::Entropy;
use locust_proto::id::{BlobHash, GoalId};
use locust_proto::seal;
use locust_proto::store::{Blob, LocalWrite, Space, Store, StoreError};
use serde::{Deserialize, Serialize};

use super::{Plan, Planned, answer};
use crate::node::Node;
use crate::node::access::not_found;
use crate::node::callers::Actor;
use crate::node::commit::Tx;
use crate::node::entry::Entry;
use crate::node::records;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub(in crate::node) struct BlobRecord {
    pub wanted: bool,
    pub withdrawn: bool,
}

pub(in crate::node) fn blob_key(goal: &GoalId, hash: &BlobHash) -> Vec<u8> {
    [goal.0.as_slice(), hash.0.as_slice()].concat()
}

pub(in crate::node) fn blob_record<S: Store>(
    store: &S,
    goal: &GoalId,
    hash: &BlobHash,
) -> Result<Option<BlobRecord>, StoreError> {
    store
        .get(Space::Blob, &blob_key(goal, hash))?
        .map(|bytes| records::read(&bytes))
        .transpose()
}

pub(in crate::node) fn blob_write(
    goal: &GoalId,
    hash: &BlobHash,
    record: &BlobRecord,
) -> LocalWrite {
    records::put(Space::Blob, blob_key(goal, hash), record)
}

fn unavailable(message: &'static str) -> ApiError {
    ApiError::new(ErrorCode::Unavailable, message)
}

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn blob_put(&self, actor: &Actor, goal: GoalId, bytes: Vec<u8>) -> Plan {
        let (entry, _) = self.member(actor, &goal)?;
        let epoch = entry.state().epoch;
        let key = entry
            .keys
            .get(&epoch)
            .ok_or_else(|| unavailable("the current content key has not arrived"))?;
        let sealed = seal::seal(&goal, epoch, key, &bytes).map_err(|_| {
            ApiError::new(
                ErrorCode::LimitExceeded,
                "the sealed content exceeds the content limit",
            )
        })?;
        let blob = Blob::new(sealed);
        let hash = blob.hash();
        let mut tx = Tx::none();
        tx.commit.blobs.push(blob);
        tx.local(blob_write(&goal, &hash, &BlobRecord::default()))
            .touch(goal);
        Ok(Planned {
            response: Response::BlobStored { hash },
            tx,
        })
    }

    pub(super) fn blob_get(&self, actor: &Actor, goal: GoalId, hash: BlobHash) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let record = blob_record(&self.store, &goal, &hash)?;
        if !self.names_content(entry, &hash) && record.is_none() {
            return Err(not_found("nothing in this goal names that content"));
        }
        if record.is_some_and(|record| record.withdrawn) {
            return Err(unavailable("the content was withdrawn on this daemon"));
        }
        if !self.content_path_readable(entry, &hash, actor.principal.as_ref(), None) {
            return Err(crate::node::access::denied(
                "the content path is outside this principal's membership",
            ));
        }
        let bytes = self
            .store
            .blob(&hash)?
            .ok_or_else(|| unavailable("the content has not arrived"))?;
        let epoch = seal::epoch_of(&bytes)
            .map_err(|_| ApiError::new(ErrorCode::Corrupted, "the stored content is not sealed"))?;
        if !self.blob_metadata_admitted(&goal, &hash, bytes.len() as u64, &bytes) {
            return Err(unavailable(
                "the content does not match this goal's references",
            ));
        }
        if !self.content_path_readable(
            entry,
            &hash,
            actor.principal.as_ref(),
            Some((bytes.len() as u64, epoch)),
        ) {
            return Err(crate::node::access::denied(
                "the content epoch is outside this principal's membership",
            ));
        }
        let key = entry
            .keys
            .get(&epoch)
            .ok_or_else(|| unavailable("the content key has not arrived"))?;
        let plain = seal::open(&goal, key, &bytes).map_err(|_| {
            ApiError::new(
                ErrorCode::Corrupted,
                "the stored content failed authentication",
            )
        })?;
        answer(Response::Blob { bytes: plain })
    }

    /// A missing content read is the one read that schedules future work.
    /// Called before reporting Unavailable.
    pub(in crate::node) fn note_blob_want(
        &mut self,
        actor: &Actor,
        goal: GoalId,
        hash: BlobHash,
    ) -> Result<(), ApiError> {
        let entry = self.readable(actor, &goal)?;
        let record = blob_record(&self.store, &goal, &hash)?;
        if !self.names_content(entry, &hash) && record.is_none() {
            return Ok(());
        }
        if !self.content_path_readable(entry, &hash, actor.principal.as_ref(), None)
            || (!entry.names(&hash) && record.is_none() && self.blob_index.names(&goal, &hash))
            || record.is_some_and(|record| record.withdrawn || record.wanted)
            || self.store.blob_len(&hash)?.is_some()
        {
            return Ok(());
        }
        let mut tx = Tx::none();
        tx.local(blob_write(
            &goal,
            &hash,
            &BlobRecord {
                wanted: true,
                withdrawn: false,
            },
        ))
        .touch(goal);
        self.land(tx)
    }

    pub(super) fn blob_stat(&self, actor: &Actor, goal: GoalId, hashes: Vec<BlobHash>) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let states = hashes
            .into_iter()
            .map(|hash| {
                self.read_content_state(entry, &hash, actor.principal.as_ref())
                    .map(|state| BlobStatus { hash, state })
            })
            .collect::<Result<Vec<_>, _>>()?;
        answer(Response::BlobStates(states))
    }

    pub(super) fn blob_withdraw(&self, actor: &Actor, goal: GoalId, hash: BlobHash) -> Plan {
        let (entry, _) = self.member(actor, &goal)?;
        if !self.names_content(entry, &hash) && blob_record(&self.store, &goal, &hash)?.is_none() {
            return Err(not_found("nothing in this goal names that content"));
        }
        let mut tx = Tx::none();
        tx.local(blob_write(
            &goal,
            &hash,
            &BlobRecord {
                wanted: false,
                withdrawn: true,
            },
        ))
        .touch(goal);
        // Retain bytes as evidence. Both peer serving and local reads consult
        // the goal's withdrawal record, so another goal is never affected.
        Ok(Planned {
            response: Response::Done,
            tx,
        })
    }

    pub(in crate::node) fn read_content_state(
        &self,
        entry: &Entry,
        hash: &BlobHash,
        reader: Option<&locust_proto::id::PublicKey>,
    ) -> Result<BlobState, ApiError> {
        if !self.names_content(entry, hash)
            && blob_record(&self.store, &entry.id(), hash)?.is_none()
        {
            return Ok(BlobState::Unknown);
        }
        if !self.content_path_readable(entry, hash, reader, None) {
            return Ok(BlobState::Unavailable);
        }
        if let Some(prefix) = self.store.blob_range(hash, 0, seal::OVERHEAD_BYTES)?
            && let Ok(epoch) = seal::epoch_of(&prefix)
            && !self.content_path_readable(
                entry,
                hash,
                reader,
                Some((self.store.blob_len(hash)?.unwrap_or(0), epoch)),
            )
        {
            return Ok(BlobState::Unavailable);
        }
        self.content_state(entry, hash)
    }

    pub(in crate::node) fn content_state(
        &self,
        entry: &Entry,
        hash: &BlobHash,
    ) -> Result<BlobState, ApiError> {
        let record = blob_record(&self.store, &entry.id(), hash)?;
        if !self.names_content(entry, hash) && record.is_none() {
            return Ok(BlobState::Unknown);
        }
        if record.is_some_and(|record| record.withdrawn) {
            return Ok(BlobState::Unavailable);
        }
        if let Some(prefix) = self.store.blob_range(hash, 0, seal::OVERHEAD_BYTES)? {
            let epoch = seal::epoch_of(&prefix).map_err(|_| {
                ApiError::new(ErrorCode::Corrupted, "the stored content is not sealed")
            })?;
            let len = self.store.blob_len(hash)?.unwrap_or(0);
            if entry.keys.contains_key(&epoch)
                && self.blob_metadata_admitted(&entry.id(), hash, len, &prefix)
            {
                return Ok(BlobState::Held);
            }
        }
        Ok(if self.reachable(&entry.id()) {
            BlobState::Requested
        } else {
            BlobState::Unavailable
        })
    }
}
