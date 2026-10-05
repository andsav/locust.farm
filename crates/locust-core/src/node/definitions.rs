//! Goal-scoped verified definitions. A catalog entry is not evidence that a
//! replica holds the encrypted object named by a signed rules binding.
use std::collections::BTreeMap;

use locust_proto::crypto::ContentKey;
use locust_proto::event::{Body, DefinitionRef, Event};
use locust_proto::id::{BlobHash, DefinitionHash, EventId, GoalId};
use locust_proto::organization::Formation;
use locust_proto::seal;
use locust_proto::store::{Blob, Commit, LocalWrite, Space, Store, StoreError};

use super::{entry, records};
use crate::goal::DefinitionLookup;

#[derive(Clone, Debug, Default)]
pub(super) struct Definitions {
    values: BTreeMap<DefinitionHash, Formation>,
    references: BTreeMap<EventId, DefinitionRef>,
}

impl DefinitionLookup for Definitions {
    fn definition(&self, hash: &DefinitionHash) -> Option<&Formation> {
        self.values.get(hash)
    }
}

impl Definitions {
    pub fn load<S: Store>(
        store: &S,
        goal: GoalId,
        keys: &BTreeMap<u32, ContentKey>,
        pending: &Commit,
    ) -> Result<Self, StoreError> {
        let mut definitions = Self::default();
        let mut cursor = 0;
        loop {
            let page = store.log(&goal, cursor, 256)?;
            if page.is_empty() {
                break;
            }
            for (position, event) in page {
                definitions.note(&event);
                cursor = position;
            }
        }
        definitions.updated(store, goal, keys, pending)
    }

    pub fn names(&self, hash: &BlobHash) -> bool {
        self.references
            .values()
            .any(|reference| reference.object.hash == *hash)
    }

    pub fn changed_by(
        &self,
        goal: GoalId,
        pending: &Commit,
        arrived: &[BlobHash],
    ) -> Result<bool, StoreError> {
        for write in &pending.local {
            let (space, key) = match write {
                LocalWrite::Put { space, key, .. } | LocalWrite::Delete { space, key } => {
                    (*space, key)
                }
            };
            if space == Space::Key && entry::key_subject(key)?.0 == goal {
                return Ok(true);
            }
        }
        Ok(pending.events.iter().any(|event| {
            event.header().goal == goal && matches!(event.header().body, Body::RulesBound { .. })
        }) || pending.blobs.iter().any(|blob| self.names(&blob.hash()))
            || pending.drop_blobs.iter().any(|hash| self.names(hash))
            || arrived.iter().any(|hash| self.names(hash)))
    }

    /// Only rule bindings, their objects and content keys can change the
    /// definition lookup. Ordinary work and local records never reload history.
    pub fn updated<S: Store>(
        &self,
        store: &S,
        goal: GoalId,
        keys: &BTreeMap<u32, ContentKey>,
        pending: &Commit,
    ) -> Result<Self, StoreError> {
        let mut keys = keys.clone();
        for write in &pending.local {
            let (space, key, value) = match write {
                LocalWrite::Put { space, key, value } => (*space, key, Some(value)),
                LocalWrite::Delete { space, key } => (*space, key, None),
            };
            if space == Space::Key {
                let (subject, epoch) = entry::key_subject(key)?;
                if subject == goal {
                    if let Some(value) = value {
                        keys.insert(epoch, records::read(value)?);
                    } else {
                        keys.remove(&epoch);
                    }
                }
            }
        }
        let mut updated = Self {
            values: BTreeMap::new(),
            references: self.references.clone(),
        };
        for event in &pending.events {
            if event.header().goal == goal {
                updated.note(event);
            }
        }
        for reference in updated.references.values() {
            if let Some(definition) = Self::read(store, goal, &keys, pending, reference)? {
                updated.values.insert(reference.semantic, definition);
            }
        }
        Ok(updated)
    }

    fn note(&mut self, event: &Event) {
        if let Body::RulesBound { binding, .. } = &event.header().body {
            self.references.insert(event.id(), binding.definition);
        }
    }

    fn read<S: Store>(
        store: &S,
        goal: GoalId,
        keys: &BTreeMap<u32, ContentKey>,
        pending: &Commit,
        reference: &DefinitionRef,
    ) -> Result<Option<Formation>, StoreError> {
        let Some(key) = keys.get(&reference.object.key_epoch) else {
            return Ok(None);
        };
        if pending.drop_blobs.contains(&reference.object.hash) {
            return Ok(None);
        }
        let blob = if let Some(blob) = pending
            .blobs
            .iter()
            .find(|blob| blob.hash() == reference.object.hash)
        {
            blob.clone()
        } else if let Some(bytes) = store.blob(&reference.object.hash)? {
            Blob::new(bytes)
        } else {
            return Ok(None);
        };
        if !reference.object.admits(&blob) {
            return Ok(None);
        }
        let Ok(bytes) = seal::open(&goal, key, blob.bytes()) else {
            return Ok(None);
        };
        let Ok(source) = std::str::from_utf8(&bytes) else {
            return Ok(None);
        };
        let inspected = crate::organization::inspect(source);
        if let (Some(definition), Some(hash)) = (inspected.normalized, inspected.semantic_hash)
            && inspected.valid
            && hash == reference.semantic.to_string()
        {
            return Ok(Some(definition));
        }
        Ok(None)
    }
}
