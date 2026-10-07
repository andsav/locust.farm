//! Goal-scoped content reachability, derived from committed references.
//!
//! Only typed event roots may decode containers. A manifest file entry is an
//! opaque leaf even when its bytes happen to encode another manifest. Edges
//! exist only after authenticating the containing object for this goal.
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use locust_proto::api::{ApiError, ErrorCode};
use locust_proto::engine::Entropy;
use locust_proto::event::Body;
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey};
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

/// Content readiness remains separate from signed workspace authority.
#[derive(Clone, Debug)]
pub(in crate::node) enum ManifestState {
    Missing,
    Withdrawn,
    KeyMissing { epoch: u32 },
    Invalid { reason: String },
    Ready { manifest: Arc<Manifest>, epoch: u32 },
}
pub(in crate::node) enum FileState {
    Missing,
    Withdrawn,
    KeyMissing { epoch: u32 },
    Invalid { reason: String },
    Ready { size: u64 },
}
#[derive(Clone)]
struct CachedFile {
    epoch: u32,
    len: u64,
    size: u64,
    key_fingerprint: [u8; 32],
}
#[derive(Clone)]
struct CachedManifest {
    epoch: u32,
    len: u64,
    key_fingerprint: [u8; 32],
    decoded: Result<Arc<Manifest>, String>,
}
impl CachedManifest {
    fn state(&self) -> ManifestState {
        match &self.decoded {
            Ok(manifest) => ManifestState::Ready {
                manifest: manifest.clone(),
                epoch: self.epoch,
            },
            Err(reason) => ManifestState::Invalid {
                reason: reason.clone(),
            },
        }
    }
}
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct IndexStats {
    pub manifest_decodes: usize,
    pub cache_hits: usize,
    pub event_roots: usize,
    pub history_rebuilds: usize,
    pub file_validations: usize,
}
#[derive(Default)]
struct Graph {
    references: BTreeMap<BlobHash, BTreeSet<Reference>>,
    records: BTreeMap<BlobHash, BlobRecord>,
    expanded: BTreeSet<(BlobHash, Reference)>,
    wanted: BTreeSet<BlobHash>,
    root_events: BTreeSet<EventId>,
    governance_head: Option<EventId>,
    content_epoch: u32,
    // Decode facts are immutable content observations, never authority. On
    // withdrawal/key/membership change the references are rebuilt independently.
    manifests: RefCell<BTreeMap<BlobHash, CachedManifest>>,
    files: RefCell<BTreeMap<BlobHash, CachedFile>>,
    #[cfg(test)]
    stats: std::cell::Cell<IndexStats>,
}

/// Missing lookup is a range query. Blob arrival expands only that object's
/// existing references, without rescanning goal history for every file.
#[derive(Default)]
pub(super) struct BlobIndex(BTreeMap<GoalId, Graph>);
impl BlobIndex {
    #[cfg(test)]
    pub fn stats(&self, goal: &GoalId) -> IndexStats {
        self.0
            .get(goal)
            .map_or_else(IndexStats::default, |graph| graph.stats.get())
    }
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
    fn event_roots(
        &mut self,
        entry: &Entry,
        event: &locust_proto::event::Event,
        queue: &mut VecDeque<(BlobHash, Reference)>,
    ) {
        if !self.root_events.insert(event.id()) {
            return;
        }
        #[cfg(test)]
        self.stats.update(|mut stats| {
            stats.event_roots += 1;
            stats
        });
        let header = event.header();
        let epoch = entry.goal.epoch_of(&event.id());
        // Once signed metadata names a formerly local object, its broad local
        // association may no longer weaken exact payload/epoch constraints.
        for hash in header.blobs() {
            if let Some(references) = self.references.get_mut(&hash) {
                references.retain(|reference| !reference.local);
            }
        }
        if let Some(payload) = header.payload {
            let mut reference = Reference::root(
                Kind::Opaque,
                Some(payload.key_epoch),
                Some(u64::from(payload.len)),
            );
            reference.exact_epoch = true;
            self.add(payload.hash, reference, queue);
        }
        for hash in header
            .blobs()
            .into_iter()
            .filter(|hash| header.payload.is_none_or(|payload| payload.hash != *hash))
        {
            if !matches!(&header.body, Body::RulesBound { binding, .. } if binding.definition.object.hash == hash)
            {
                self.add(hash, Reference::root(Kind::Opaque, epoch, None), queue);
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
                self.add(payload.hash, reference, queue);
            }
            Body::TaskOpened { binding } | Body::TaskRevised { binding, .. } => {
                for hash in binding.inputs.values() {
                    self.add(*hash, Reference::root(Kind::Manifest, epoch, None), queue);
                }
            }
            Body::WorkspaceProposed {
                result_manifest, ..
            } => {
                self.add(
                    *result_manifest,
                    Reference::root(Kind::Manifest, epoch, None),
                    queue,
                );
            }
            Body::EffectMaterialized { effect } => {
                if let locust_proto::event::EffectAction::OpenTask { binding, .. } = &effect.action
                {
                    for hash in binding.inputs.values() {
                        self.add(*hash, Reference::root(Kind::Manifest, epoch, None), queue);
                    }
                }
            }
            _ => {}
        }
    }
    fn roots<S: Store>(entry: &Entry, store: &S, old: Option<Self>) -> Result<Self, StoreError> {
        let mut graph = Self::default();
        if let Some(old) = old {
            graph.manifests = old.manifests;
            graph.files = old.files;
            #[cfg(test)]
            {
                graph.stats = old.stats;
            }
        }
        graph.governance_head = entry.state().head;
        graph.content_epoch = entry.state().epoch;
        #[cfg(test)]
        graph.stats.update(|mut stats| {
            stats.history_rebuilds += 1;
            stats
        });
        let mut queue = VecDeque::new();
        for (key, value) in store.scan(Space::Blob, &entry.id().0)? {
            let (goal, hash) = subject(&key)?;
            if goal == entry.id() {
                graph.records.insert(hash, records::read(&value)?);
            }
        }
        for author in entry.goal.authors() {
            for point in entry.goal.points(author) {
                if let Some(event) = entry.goal.event(&point.id) {
                    graph.event_roots(entry, event, &mut queue);
                }
            }
        }
        for (hash, record) in graph.records.clone() {
            graph.local(entry, hash, record, &mut queue);
        }
        graph.expand(entry, store, queue)?;
        Ok(graph)
    }
    fn manifest<S: Store>(
        &self,
        entry: &Entry,
        store: &S,
        hash: BlobHash,
    ) -> Result<ManifestState, StoreError> {
        let Some(len) = store.blob_len(&hash)? else {
            return Ok(ManifestState::Missing);
        };
        if len < seal::OVERHEAD_BYTES as u64 || len > MAX_BLOB_BYTES as u64 {
            return Ok(ManifestState::Invalid {
                reason: "manifest exceeds the content-object bounds".into(),
            });
        }
        if let Some(cached) = self.manifests.borrow().get(&hash)
            && cached.len == len
            && entry.keys.get(&cached.epoch).is_some_and(|key| {
                locust_proto::crypto::domain_hash("locust:manifest-cache:key:v1", &key.0)
                    == cached.key_fingerprint
            })
        {
            #[cfg(test)]
            self.stats.update(|mut stats| {
                stats.cache_hits += 1;
                stats
            });
            return Ok(cached.state());
        }
        let Some(bytes) = store.blob(&hash)? else {
            return Ok(ManifestState::Missing);
        };
        if locust_proto::crypto::content_hash(&bytes) != hash {
            return Err(StoreError::Corrupted(
                "a content graph object does not match its hash".into(),
            ));
        }
        let epoch = match seal::epoch_of(&bytes) {
            Ok(epoch) => epoch,
            Err(error) => {
                return Ok(ManifestState::Invalid {
                    reason: error.to_string(),
                });
            }
        };
        let Some(key) = entry.keys.get(&epoch) else {
            return Ok(ManifestState::KeyMissing { epoch });
        };
        let plain = match seal::open(&entry.id(), key, &bytes) {
            Ok(plain) => plain,
            Err(error) => {
                return Ok(ManifestState::Invalid {
                    reason: error.to_string(),
                });
            }
        };
        #[cfg(test)]
        self.stats.update(|mut stats| {
            stats.manifest_decodes += 1;
            stats
        });
        let decoded = Manifest::decode(&plain)
            .map(Arc::new)
            .map_err(|error| error.to_string());
        let cached = CachedManifest {
            epoch,
            len: bytes.len() as u64,
            key_fingerprint: locust_proto::crypto::domain_hash(
                "locust:manifest-cache:key:v1",
                &key.0,
            ),
            decoded,
        };
        let state = cached.state();
        self.manifests.borrow_mut().insert(hash, cached);
        Ok(state)
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
            let ManifestState::Ready { manifest, epoch } = self.manifest(entry, store, hash)?
            else {
                continue;
            };
            let Some(len) = store.blob_len(&hash)? else {
                continue;
            };
            if !reference.admits(len, epoch) {
                continue;
            }
            self.expanded.insert((hash, reference));
            for file in &manifest.entries {
                // Existing object limits apply; malformed lengths cannot
                // authorize inflated requests or recursive manifest sniffing.
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
        let old = self.blob_index.0.remove(&goal);
        let old_wanted = old
            .as_ref()
            .map(|graph| graph.wanted.clone())
            .unwrap_or_default();
        let graph = Graph::roots(entry, &self.store, old)?;
        if graph.wanted.iter().any(|hash| !old_wanted.contains(hash)) {
            self.outbound.insert(goal);
        }
        self.blob_index.0.insert(goal, graph);
        Ok(())
    }
    pub(super) fn update_blob_index(&mut self, commit: &Commit) -> Result<(), StoreError> {
        // Ordinary work events append roots incrementally. Governance head or
        // content-key changes can alter old anchors; withdrawal removes parent
        // paths. Those invalidate authorization, retaining only decode facts.
        let mut rebuild = BTreeSet::new();
        let event_goals: BTreeSet<_> = commit
            .events
            .iter()
            .map(|event| event.header().goal)
            .collect();
        for goal in &event_goals {
            let Some(entry) = self.goals.get(goal) else {
                continue;
            };
            if self.blob_index.0.get(goal).is_none_or(|graph| {
                graph.governance_head != entry.state().head
                    || graph.content_epoch != entry.state().epoch
            }) {
                rebuild.insert(*goal);
            }
        }
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
        for goal in &rebuild {
            self.rebuild_content_goal(*goal)?;
        }
        for event in &commit.events {
            let goal = event.header().goal;
            if rebuild.contains(&goal) {
                continue;
            }
            let Some(entry) = self.goals.get(&goal) else {
                continue;
            };
            let Some(held) = entry.goal.event(&event.id()) else {
                continue;
            };
            let graph = self.blob_index.0.entry(goal).or_default();
            let mut queue = VecDeque::new();
            graph.event_roots(entry, held, &mut queue);
            if graph.expand(entry, &self.store, queue)? {
                self.outbound.insert(goal);
            }
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
        // A definition blob can unblock governance without a new event.
        let changed: Vec<_> = self
            .blob_index
            .0
            .iter()
            .filter_map(|(goal, graph)| {
                self.goals
                    .get(goal)
                    .filter(|entry| {
                        graph.governance_head != entry.state().head
                            || graph.content_epoch != entry.state().epoch
                    })
                    .map(|_| *goal)
            })
            .collect();
        for goal in changed {
            self.rebuild_content_goal(goal)?;
        }
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
    /// Reads a typed container through the current serving authorization, then
    /// reuses only immutable authenticated decode facts. Cache existence never
    /// grants goal association, membership, keys or withdrawal permission.
    pub(in crate::node) fn workspace_manifest(
        &self,
        entry: &Entry,
        hash: BlobHash,
        reader: Option<&PublicKey>,
    ) -> Result<ManifestState, ApiError> {
        let record = blob_record(&self.store, &entry.id(), &hash)?;
        if record.is_some_and(|record| record.withdrawn) {
            return Ok(ManifestState::Withdrawn);
        }
        if !self.names_content(entry, &hash) && record.is_none() {
            return Err(ApiError::new(
                ErrorCode::NotFound,
                "nothing in this goal names that manifest",
            ));
        }
        if !self.content_path_readable(entry, &hash, reader, None) {
            return Err(ApiError::new(
                ErrorCode::Denied,
                "the manifest path is outside this principal's membership",
            ));
        }
        let Some(graph) = self.blob_index.0.get(&entry.id()) else {
            return Ok(ManifestState::Missing);
        };
        let Some(len) = self.store.blob_len(&hash)? else {
            return Ok(ManifestState::Missing);
        };
        let cached_epoch = graph
            .manifests
            .borrow()
            .get(&hash)
            .filter(|cached| cached.len == len)
            .map(|cached| cached.epoch);
        let epoch = if let Some(epoch) = cached_epoch {
            epoch
        } else {
            let Some(bytes) = self.store.blob(&hash)? else {
                return Ok(ManifestState::Missing);
            };
            match seal::epoch_of(&bytes) {
                Ok(epoch) => epoch,
                Err(error) => {
                    return Ok(ManifestState::Invalid {
                        reason: error.to_string(),
                    });
                }
            }
        };
        if !self.blob_index.admits(&entry.id(), &hash, len, epoch)
            && (self.names_content(entry, &hash) || epoch > entry.state().epoch || record.is_none())
        {
            return Ok(ManifestState::Invalid {
                reason: "manifest does not match goal references".into(),
            });
        }
        if !self.content_path_readable(entry, &hash, reader, Some((len, epoch))) {
            return Err(ApiError::new(
                ErrorCode::Denied,
                "the manifest epoch is outside this principal's membership",
            ));
        }
        let state = graph.manifest(entry, &self.store, hash)?;
        Ok(state)
    }
    /// Cache only authenticated immutable file facts. Existence, withdrawal,
    /// current membership paths and the exact content key are checked each time.
    pub(in crate::node) fn workspace_file(
        &self,
        entry: &Entry,
        hash: BlobHash,
        reader: Option<&PublicKey>,
    ) -> Result<FileState, ApiError> {
        if blob_record(&self.store, &entry.id(), &hash)?.is_some_and(|record| record.withdrawn) {
            return Ok(FileState::Withdrawn);
        }
        let Some(len) = self.store.blob_len(&hash)? else {
            return Ok(FileState::Missing);
        };
        let graph = self.blob_index.0.get(&entry.id());
        let cached = graph.and_then(|graph| {
            graph
                .files
                .borrow()
                .get(&hash)
                .filter(|cached| cached.len == len)
                .cloned()
        });
        let authorize = |epoch| {
            if self.content_path_readable(entry, &hash, reader, Some((len, epoch))) {
                Ok(())
            } else {
                Err(super::access::denied(
                    "workspace file is outside this principal's membership",
                ))
            }
        };
        let fingerprint = |key: &locust_proto::crypto::ContentKey| {
            locust_proto::crypto::domain_hash("locust:file-cache:key:v1", &key.0)
        };
        if let Some(cached) = cached {
            authorize(cached.epoch)?;
            let Some(key) = entry.keys.get(&cached.epoch) else {
                return Ok(FileState::KeyMissing {
                    epoch: cached.epoch,
                });
            };
            if fingerprint(key) == cached.key_fingerprint {
                return Ok(FileState::Ready { size: cached.size });
            }
        }
        let Some(bytes) = self.store.blob(&hash)? else {
            return Ok(FileState::Missing);
        };
        if locust_proto::crypto::content_hash(&bytes) != hash {
            return Ok(FileState::Invalid {
                reason: "sealed file hash does not match manifest".into(),
            });
        }
        let epoch = match seal::epoch_of(&bytes) {
            Ok(epoch) => epoch,
            Err(error) => {
                return Ok(FileState::Invalid {
                    reason: error.to_string(),
                });
            }
        };
        authorize(epoch)?;
        let Some(key) = entry.keys.get(&epoch) else {
            return Ok(FileState::KeyMissing { epoch });
        };
        let plain = match seal::open(&entry.id(), key, &bytes) {
            Ok(plain) => plain,
            Err(error) => {
                return Ok(FileState::Invalid {
                    reason: error.to_string(),
                });
            }
        };
        let size = plain.len() as u64;
        if let Some(graph) = graph {
            #[cfg(test)]
            graph.stats.update(|mut stats| {
                stats.file_validations += 1;
                stats
            });
            graph.files.borrow_mut().insert(
                hash,
                CachedFile {
                    epoch,
                    len,
                    size,
                    key_fingerprint: fingerprint(key),
                },
            );
        }
        Ok(FileState::Ready { size })
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
