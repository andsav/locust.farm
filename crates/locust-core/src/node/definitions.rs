//! Goal-scoped verified definitions. A catalog entry is not evidence that a
//! replica holds the encrypted object named by a signed rules binding.
use std::collections::BTreeMap;

use locust_proto::crypto::ContentKey;
use locust_proto::event::{Body, Event};
use locust_proto::id::{DefinitionHash, GoalId};
use locust_proto::organization::Formation;
use locust_proto::seal;
use locust_proto::store::{Blob, Commit, LocalWrite, Space, Store, StoreError};

use super::{entry, records};
use crate::goal::DefinitionLookup;

#[derive(Clone, Debug, Default)]
pub(super) struct Definitions(BTreeMap<DefinitionHash, Formation>);

impl DefinitionLookup for Definitions {
    fn definition(&self, hash: &DefinitionHash) -> Option<&Formation> {
        self.0.get(hash)
    }
}

impl Definitions {
    pub fn load<S: Store>(
        store: &S,
        goal: GoalId,
        keys: &BTreeMap<u32, ContentKey>,
        pending: &Commit,
    ) -> Result<Self, StoreError> {
        let mut keys = keys.clone();
        for write in &pending.local {
            if let LocalWrite::Put {
                space: Space::Key,
                key,
                value,
            } = write
            {
                let (subject, epoch) = entry::key_subject(key)?;
                if subject == goal {
                    keys.insert(epoch, records::read(value)?);
                }
            }
        }
        let mut definitions = Self::default();
        let mut cursor = 0;
        loop {
            let page = store.log(&goal, cursor, 256)?;
            if page.is_empty() {
                break;
            }
            for (position, event) in page {
                definitions.read(store, goal, &keys, pending, &event)?;
                cursor = position;
            }
        }
        for event in &pending.events {
            if event.header().goal == goal {
                definitions.read(store, goal, &keys, pending, event)?;
            }
        }
        Ok(definitions)
    }

    fn read<S: Store>(
        &mut self,
        store: &S,
        goal: GoalId,
        keys: &BTreeMap<u32, ContentKey>,
        pending: &Commit,
        event: &Event,
    ) -> Result<(), StoreError> {
        let Body::RulesBound { binding, .. } = &event.header().body else {
            return Ok(());
        };
        let reference = &binding.definition;
        let Some(key) = keys.get(&reference.object.key_epoch) else {
            return Ok(());
        };
        if pending.drop_blobs.contains(&reference.object.hash) {
            return Ok(());
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
            return Ok(());
        };
        if !reference.object.admits(&blob) {
            return Ok(());
        }
        let Ok(bytes) = seal::open(&goal, key, blob.bytes()) else {
            return Ok(());
        };
        let Ok(source) = std::str::from_utf8(&bytes) else {
            return Ok(());
        };
        let inspected = crate::organization::inspect(source);
        if let (Some(definition), Some(hash)) = (inspected.normalized, inspected.semantic_hash)
            && inspected.valid
            && hash == reference.semantic.to_string()
        {
            self.0.insert(reference.semantic, definition);
        }
        Ok(())
    }
}
