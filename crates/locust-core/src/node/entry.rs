//! One goal as the node holds it: the replicated history with this daemon's
//! own records about it.
//!
//! `Space::Key` holds the content key of each epoch, keyed by goal and epoch.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use locust_proto::crypto::ContentKey;
use locust_proto::event::{Event, TaskId};
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey};
use locust_proto::seal;
use locust_proto::store::{Blob, LocalWrite, Space, Store, StoreError};

use super::feed::Feed;
use super::local::Local;
use super::records;
use super::sessions::ClaimRecord;
use crate::goal::{Goal, State};

const KEY: u8 = b'k';

/// A goal held or being joined.
pub(super) struct Entry {
    pub goal: Goal,
    pub definitions: super::definitions::Definitions,
    pub local: Local,
    /// Content keys by epoch.
    pub keys: BTreeMap<u32, ContentKey>,
    /// Claims by assignment.
    pub claims: BTreeMap<EventId, ClaimRecord>,
    pub feed: Feed,
    pub context: super::context::Acknowledgments,
    pub deliveries: BTreeMap<(locust_proto::id::EffectId, PublicKey), super::delivery::Delivery>,
    /// Effects that could not be materialized in the latest pass; retried on
    /// the next change or startup, never persisted as replicated state.
    pub failed_effects: BTreeSet<locust_proto::id::EffectId>,
    /// Every content object a held event names.
    named: HashSet<BlobHash>,
}

pub(super) fn key_write(goal: &GoalId, epoch: u32, key: &ContentKey) -> LocalWrite {
    records::put(
        Space::Key,
        records::key(KEY, &[&goal.0, &epoch.to_be_bytes()]),
        key,
    )
}

/// The goal and epoch a `Space::Key` key is about.
pub(super) fn key_subject(key: &[u8]) -> Result<(GoalId, u32), StoreError> {
    let goal = records::part(key, 1).ok_or_else(records::bad_key)?;
    let epoch = records::part(key, 1 + GoalId::LEN).ok_or_else(records::bad_key)?;
    Ok((GoalId(goal), u32::from_be_bytes(epoch)))
}

impl Entry {
    /// An entry around `goal` with no local records yet.
    pub fn new(goal: Goal) -> Self {
        Self {
            goal,
            definitions: super::definitions::Definitions::default(),
            local: Local::default(),
            keys: BTreeMap::new(),
            claims: BTreeMap::new(),
            feed: Feed::default(),
            context: super::context::Acknowledgments::default(),
            deliveries: BTreeMap::new(),
            failed_effects: BTreeSet::new(),
            named: HashSet::new(),
        }
    }

    /// Notes the content objects that newly held events name.
    pub fn note_named(&mut self, events: &[Event]) {
        for event in events {
            self.named.extend(event.header().blobs());
        }
    }

    /// True if a held event names the content object `hash`.
    pub fn names(&self, hash: &BlobHash) -> bool {
        self.named.contains(hash)
    }

    pub fn id(&self) -> GoalId {
        self.goal.id()
    }

    pub fn state(&self) -> &State {
        self.goal.state()
    }

    /// True if `key` is a current member by the decisions applied here.
    pub fn is_member(&self, key: &PublicKey) -> bool {
        self.state().is_member(key)
    }

    /// The revision a caller sees: never zero.
    pub fn revision(&self) -> u64 {
        self.local.revision.max(1)
    }

    pub fn may_read_epoch(&self, reader: Option<&PublicKey>, epoch: u32) -> bool {
        reader.is_none_or(|reader| {
            self.goal
                .read_epoch(reader)
                .is_some_and(|last| epoch <= last)
        })
    }

    /// The text of `event`: its payload opened with the key of the epoch it
    /// was sealed under. Absent when there is no payload, the payload or the
    /// key is not held, or the bytes are not text.
    pub fn text<S: Store>(
        &self,
        store: &S,
        event: &Event,
        reader: Option<&PublicKey>,
    ) -> Option<String> {
        let payload = event.header().payload?;
        if !self.may_read_epoch(reader, payload.key_epoch) {
            return None;
        }
        let record =
            super::requests::content::blob_record(store, &self.id(), &payload.hash).ok()?;
        if record.is_some_and(|record| record.withdrawn) {
            return None;
        }
        let key = self.keys.get(&payload.key_epoch)?;
        let sealed = Blob::new(store.blob(&payload.hash).ok()??);
        if !payload.admits(&sealed) {
            return None;
        }
        let plain = seal::open(&self.id(), key, sealed.bytes()).ok()?;
        String::from_utf8(plain).ok()
    }

    /// A task's title as the board shows it: the first line of the text that
    /// opened it. Every view that names a task prints this, never the whole text.
    pub fn task_title<S: Store>(
        &self,
        store: &S,
        task: &TaskId,
        reader: Option<&PublicKey>,
    ) -> Option<String> {
        let created = self.state().tasks.get(task)?.created;
        let event = self.goal.event(&created)?;
        self.text(store, event, reader)
            .map(|text| text.lines().next().unwrap_or_default().to_owned())
    }
}
