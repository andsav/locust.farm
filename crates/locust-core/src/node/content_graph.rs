//! Goal-scoped content reachability, derived from committed references.
//!
//! Only typed event roots may decode containers. A manifest file entry is an
//! opaque leaf even when its bytes happen to encode another manifest. Edges
//! exist only after authenticating the containing object for this goal.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use locust_proto::contribution::Contribution;
use locust_proto::engine::Entropy;
use locust_proto::event::Body;
use locust_proto::id::{BlobHash, GoalId, PublicKey};
use locust_proto::limits::MAX_BLOB_BYTES;
use locust_proto::manifest::Manifest;
use locust_proto::seal;
use locust_proto::store::{Commit, LocalWrite, Space, Store, StoreError};

use super::entry::Entry;
use super::requests::content::{BlobRecord, blob_record};
use super::{Node, entry, records};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Opaque,
    Manifest,
    Contribution,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Reference {
    kind: Kind,
    /// None while the signed root's anchor is not on the governance chain.
    epoch: Option<u32>,
    exact_epoch: bool,
    len: Option<u64>,
    /// Every containing object on this path must be readable too.
    read_epoch: u32,
    automatic: bool,
    local: bool,
}
impl Reference {
    fn root(kind: Kind, epoch: Option<u32>, len: Option<u64>) -> Self {
        Self {
            kind,
            epoch,
            exact_epoch: false,
            len,
            read_epoch: 0,
            automatic: true,
            local: false,
        }
    }
    fn admits(self, len: u64, epoch: u32) -> bool {
        self.len.is_none_or(|expected| expected == len)
            && self.epoch.is_some_and(|ceiling| {
                if self.exact_epoch {
                    epoch == ceiling
                } else {
                    epoch <= ceiling
                }
            })
    }
    fn child(self, parent_epoch: u32, kind: Kind, len: Option<u64>) -> Self {
        Self {
            kind,
            epoch: self.epoch.map(|ceiling| ceiling.min(parent_epoch)),
            exact_epoch: false,
            len,
            read_epoch: self.read_epoch.max(parent_epoch),
            automatic: true,
            local: false,
        }
    }
}

#[derive(Default)]
struct Graph {
    references: BTreeMap<BlobHash, BTreeSet<Reference>>,
    records: BTreeMap<BlobHash, BlobRecord>,
    expanded: BTreeSet<(BlobHash, Reference)>,
    wanted: BTreeSet<BlobHash>,
}

/// Missing lookup is a range query. Blob arrival expands only that object's
/// existing references, without rescanning goal history for every file.
#[derive(Default)]
pub(super) struct BlobIndex(BTreeMap<GoalId, Graph>);
impl BlobIndex {
    pub fn wanted(&self, goal: &GoalId) -> Option<&BTreeSet<BlobHash>> {
        self.0.get(goal).map(|graph| &graph.wanted)
    }
    pub fn names(&self, goal: &GoalId, hash: &BlobHash) -> bool {
        self.0
            .get(goal)
            .is_some_and(|graph| graph.references.contains_key(hash))
    }
    pub fn admits(&self, goal: &GoalId, hash: &BlobHash, len: u64, epoch: u32) -> bool {
        self.0
            .get(goal)
            .and_then(|graph| graph.references.get(hash))
            .is_some_and(|references| {
                references
                    .iter()
                    .any(|reference| reference.admits(len, epoch))
            })
    }
    pub fn admits_length(&self, goal: &GoalId, hash: &BlobHash, len: u64) -> bool {
        self.0
            .get(goal)
            .and_then(|graph| graph.references.get(hash))
            .is_some_and(|references| {
                references
                    .iter()
                    .any(|reference| reference.len.is_none_or(|expected| expected == len))
            })
    }
    pub fn readable(
        &self,
        goal: &GoalId,
        hash: &BlobHash,
        ceiling: u32,
        metadata: Option<(u64, u32)>,
    ) -> bool {
        self.0
            .get(goal)
            .and_then(|graph| graph.references.get(hash))
            .is_some_and(|references| {
                references.iter().any(|reference| {
                    reference.read_epoch <= ceiling
                        && metadata.is_none_or(|(len, epoch)| {
                            epoch <= ceiling && reference.admits(len, epoch)
                        })
                })
            })
    }
}

fn subject(key: &[u8]) -> Result<(GoalId, BlobHash), StoreError> {
    if key.len() != GoalId::LEN + BlobHash::LEN {
        return Err(records::bad_key());
    }
    Ok((
        GoalId(records::part(key, 0).ok_or_else(records::bad_key)?),
        BlobHash(records::part(key, GoalId::LEN).ok_or_else(records::bad_key)?),
    ))
}
impl Graph {
    fn add(
        &mut self,
        hash: BlobHash,
        reference: Reference,
        queue: &mut VecDeque<(BlobHash, Reference)>,
    ) {
        if self
            .records
            .get(&hash)
            .is_some_and(|record| record.withdrawn)
        {
            return;
        }
        if self.references.entry(hash).or_default().insert(reference) {
            queue.push_back((hash, reference));
        }
    }
    fn local(
        &mut self,
        entry: &Entry,
        hash: BlobHash,
        record: BlobRecord,
        queue: &mut VecDeque<(BlobHash, Reference)>,
    ) {
        self.records.insert(hash, record);
        // A generic local record must not override a signed payload's exact
        // length/epoch. BlobPut objects not directly named are opaque roots.
        if !entry.names(&hash) && !record.withdrawn {
            if let Some(references) = self.references.get_mut(&hash) {
                references.retain(|reference| !reference.local);
            }
            let mut reference = Reference::root(Kind::Opaque, Some(entry.state().epoch), None);
            reference.local = true;
            reference.automatic = record.wanted;
            self.add(hash, reference, queue);
        }
    }
    fn roots<S: Store>(entry: &Entry, store: &S) -> Result<Self, StoreError> {
        let mut graph = Self::default();
        let mut queue = VecDeque::new();
        for (key, value) in store.scan(Space::Blob, &entry.id().0)? {
            let (goal, hash) = subject(&key)?;
            if goal == entry.id() {
                graph.records.insert(hash, records::read(&value)?);
            }
        }
        for author in entry.goal.authors() {
            for point in entry.goal.points(author) {
                let Some(event) = entry.goal.event(&point.id) else {
                    continue;
                };
                let header = event.header();
                let epoch = entry.goal.epoch_of(&event.id());
                if let Some(payload) = header.payload {
                    let mut reference = Reference::root(
                        Kind::Opaque,
                        Some(payload.key_epoch),
                        Some(u64::from(payload.len)),
                    );
                    reference.exact_epoch = true;
                    graph.add(payload.hash, reference, &mut queue);
                }
                // Every body reference keeps its ordinary opaque association;
                // typed roots additionally authorize parsing that exact type.
                for hash in header
                    .blobs()
                    .into_iter()
                    .filter(|hash| header.payload.is_none_or(|payload| payload.hash != *hash))
                {
                    if !matches!(&header.body, Body::RulesBound { binding, .. } if binding.definition.object.hash == hash)
                    {
                        graph.add(hash, Reference::root(Kind::Opaque, epoch, None), &mut queue);
                    }
                }
                match &header.body {
                    Body::RulesBound { binding, .. } => {
                        let payload = binding.definition.object;
                        let mut reference = Reference::root(
                            Kind::Opaque,
                            Some(payload.key_epoch),
                            Some(u64::from(payload.len)),
                        );
                        reference.exact_epoch = true;
                        graph.add(payload.hash, reference, &mut queue);
                    }
                    Body::TaskOpened { binding } | Body::TaskRevised { binding, .. } => {
                        for hash in binding.inputs.values() {
                            graph.add(
                                *hash,
                                Reference::root(Kind::Manifest, epoch, None),
                                &mut queue,
                            );
                        }
                    }
                    Body::ContributionPublished { base, patch, .. } => {
                        if let Some(hash) = base {
                            graph.add(
                                *hash,
                                Reference::root(Kind::Manifest, epoch, None),
                                &mut queue,
                            );
                        }
                        if let Some(hash) = patch {
                            graph.add(
                                *hash,
                                Reference::root(Kind::Contribution, epoch, None),
                                &mut queue,
                            );
                        }
                    }
                    Body::EffectMaterialized { effect } => {
                        if let locust_proto::event::EffectAction::OpenTask { binding, .. } =
                            &effect.action
                        {
                            for hash in binding.inputs.values() {
                                graph.add(
                                    *hash,
                                    Reference::root(Kind::Manifest, epoch, None),
                                    &mut queue,
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        for (hash, record) in graph.records.clone() {
            graph.local(entry, hash, record, &mut queue);
        }
        graph.expand(entry, store, queue)?;
        Ok(graph)
    }
    fn refresh<S: Store>(&mut self, store: &S, hash: BlobHash) -> Result<bool, StoreError> {
        let missing = self
            .references
            .get(&hash)
            .is_some_and(|references| references.iter().any(|reference| reference.automatic))
            && !self
                .records
                .get(&hash)
                .is_some_and(|record| record.withdrawn)
            && store.blob_len(&hash)?.is_none();
        if missing {
            Ok(self.wanted.insert(hash))
        } else {
            self.wanted.remove(&hash);
            Ok(false)
        }
    }
    fn expand<S: Store>(
        &mut self,
        entry: &Entry,
        store: &S,
        mut queue: VecDeque<(BlobHash, Reference)>,
    ) -> Result<bool, StoreError> {
        let mut discovered = false;
        while let Some((hash, reference)) = queue.pop_front() {
            discovered |= self.refresh(store, hash)?;
            if reference.kind == Kind::Opaque || self.expanded.contains(&(hash, reference)) {
                continue;
            }
            let Some(bytes) = store.blob(&hash)? else {
                continue;
            };
            if locust_proto::crypto::content_hash(&bytes) != hash {
                return Err(StoreError::Corrupted(
                    "a content graph object does not match its hash".into(),
                ));
            }
            let Ok(epoch) = seal::epoch_of(&bytes) else {
                continue;
            };
            if !reference.admits(bytes.len() as u64, epoch) {
                continue;
            }
            let Some(key) = entry.keys.get(&epoch) else {
                continue;
            };
            let Ok(plain) = seal::open(&entry.id(), key, &bytes) else {
                continue;
            };
            self.expanded.insert((hash, reference));
            match reference.kind {
                Kind::Manifest => {
                    let Ok(manifest) = Manifest::decode(&plain) else {
                        continue;
                    };
                    for file in manifest.entries {
                        // A malformed size never creates an unbounded request;
                        // another independently valid reference can still win.
                        let Some(len) =
                            seal::sealed_len(file.size).filter(|len| *len <= MAX_BLOB_BYTES as u64)
                        else {
                            continue;
                        };
                        self.add(
                            file.content,
                            reference.child(epoch, Kind::Opaque, Some(len)),
                            &mut queue,
                        );
                    }
                }
                Kind::Contribution => {
                    let Ok(contribution) = Contribution::decode(&plain) else {
                        continue;
                    };
                    for manifest in [contribution.base, contribution.head] {
                        self.add(
                            manifest,
                            reference.child(epoch, Kind::Manifest, None),
                            &mut queue,
                        );
                    }
                }
                Kind::Opaque => {}
            }
        }
        Ok(discovered)
    }
}

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn rebuild_blob_index(&mut self) -> Result<(), StoreError> {
        self.blob_index = BlobIndex::default();
        let goals: Vec<_> = self.goals.keys().copied().collect();
        for goal in goals {
            self.rebuild_content_goal(goal)?;
        }
        Ok(())
    }
    fn rebuild_content_goal(&mut self, goal: GoalId) -> Result<(), StoreError> {
        let Some(entry) = self.goals.get(&goal) else {
            return Ok(());
        };
        let graph = Graph::roots(entry, &self.store)?;
        if graph.wanted.iter().any(|hash| {
            self.blob_index
                .wanted(&goal)
                .is_none_or(|old| !old.contains(hash))
        }) {
            self.outbound.insert(goal);
        }
        self.blob_index.0.insert(goal, graph);
        Ok(())
    }
    pub(super) fn update_blob_index(&mut self, commit: &Commit) -> Result<(), StoreError> {
        // History or key changes can change existing root epochs. Withdrawal
        // removes path authority, so recompute reachability rather than leave
        // stale child references. Blob arrival takes the incremental path.
        let mut rebuild: BTreeSet<_> = commit
            .events
            .iter()
            .map(|event| event.header().goal)
            .collect();
        let mut locals = Vec::new();
        for write in &commit.local {
            let (space, key, value) = match write {
                LocalWrite::Put { space, key, value } => (*space, key, Some(value)),
                LocalWrite::Delete { space, key } => (*space, key, None),
            };
            if space == Space::Key {
                rebuild.insert(entry::key_subject(key)?.0);
            }
            if space == Space::Blob {
                let (goal, hash) = subject(key)?;
                let record = value
                    .map(|value| records::read::<BlobRecord>(value))
                    .transpose()?;
                if record.is_none()
                    || record.is_some_and(|record| record.withdrawn)
                    || self
                        .blob_index
                        .0
                        .get(&goal)
                        .and_then(|graph| graph.records.get(&hash))
                        .is_some_and(|record| record.withdrawn)
                {
                    rebuild.insert(goal);
                } else if let Some(record) = record {
                    locals.push((goal, hash, record));
                }
            }
        }
        for hash in &commit.drop_blobs {
            for (goal, graph) in &self.blob_index.0 {
                if graph.references.contains_key(hash) {
                    rebuild.insert(*goal);
                }
            }
        }
        for goal in &rebuild {
            self.rebuild_content_goal(*goal)?;
        }
        for (goal, hash, record) in locals {
            if rebuild.contains(&goal) {
                continue;
            }
            let Some(entry) = self.goals.get(&goal) else {
                continue;
            };
            let graph = self.blob_index.0.entry(goal).or_default();
            let mut queue = VecDeque::new();
            graph.local(entry, hash, record, &mut queue);
            let was_wanted = graph.wanted.contains(&hash);
            graph.expand(entry, &self.store, queue)?;
            if graph.wanted.contains(&hash) && !was_wanted {
                self.outbound.insert(goal);
            }
        }
        for blob in &commit.blobs {
            self.content_arrived(blob.hash())?;
        }
        Ok(())
    }
    pub(super) fn content_arrived(&mut self, hash: BlobHash) -> Result<(), StoreError> {
        for (goal, graph) in &mut self.blob_index.0 {
            let Some(references) = graph.references.get(&hash) else {
                continue;
            };
            let queue = references
                .iter()
                .map(|reference| (hash, *reference))
                .collect();
            if graph.expand(&self.goals[goal], &self.store, queue)? {
                self.outbound.insert(*goal);
            }
        }
        Ok(())
    }
    pub(in crate::node) fn names_content(&self, entry: &Entry, hash: &BlobHash) -> bool {
        entry.names(hash) || self.blob_index.names(&entry.id(), hash)
    }
    pub(in crate::node) fn content_path_readable(
        &self,
        entry: &Entry,
        hash: &BlobHash,
        reader: Option<&PublicKey>,
        metadata: Option<(u64, u32)>,
    ) -> bool {
        let Some(reader) = reader else {
            return true;
        };
        let Some(ceiling) = entry.goal.read_epoch(reader) else {
            return false;
        };
        if self.blob_index.names(&entry.id(), hash) {
            self.blob_index
                .readable(&entry.id(), hash, ceiling, metadata)
        } else {
            metadata.is_none_or(|(_, epoch)| epoch <= ceiling)
        }
    }
    /// Metadata-only admission retains the any-valid-reference rule. Explicit
    /// local associations remain opaque and cannot cause container traversal.
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
        if self.blob_index.admits(goal, hash, len, epoch) {
            return true;
        }
        let Some(entry) = self.goals.get(goal) else {
            return false;
        };
        !self.names_content(entry, hash)
            && epoch <= entry.state().epoch
            && blob_record(&self.store, goal, hash)
                .ok()
                .flatten()
                .is_some_and(|record| !record.withdrawn)
    }
}
