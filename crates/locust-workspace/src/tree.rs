//! Git-independent frozen snapshots, plaintext tree algebra and local updates.
//! A caller supplies revision ancestry and authority separately: matching trees
//! do not prove that one revision is an ancestor of another.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use locust_proto::crypto::content_hash;
use locust_proto::id::BlobHash;
use locust_proto::manifest::{Entry, Manifest};
use locust_proto::seal::MAX_PLAINTEXT_BYTES;
use rustix::fs::{self, AtFlags, Mode};
use serde::{Deserialize, Serialize};

use crate::files::{check_path, file_bytes, invalid, manifest};
use crate::{BlobSource, BlobStore, WorkspaceError, safe_fs};

/// File equality includes authenticated plaintext and the executable bit.
/// Sealed object identifiers alone are insufficient across encryption epochs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileValue {
    pub bytes: Vec<u8>,
    pub executable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeChange {
    pub path: String,
    pub before: Option<FileValue>,
    pub after: Option<FileValue>,
}

/// A file's content digest and executable bit, without the plaintext bytes.
/// Used for preserved local files that are observed but never published as a
/// whole content object, so their size is not bounded by the object limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDigest {
    pub digest: BlobHash,
    pub executable: bool,
    pub size: u64,
}

/// Layout and equality surface shared by [`FileValue`] and [`FileDigest`].
pub trait TreeFile: PartialEq + Clone {
    fn executable(&self) -> bool;
    fn size(&self) -> u64;
}

impl TreeFile for FileValue {
    fn executable(&self) -> bool {
        self.executable
    }
    fn size(&self) -> u64 {
        self.bytes.len() as u64
    }
}

impl TreeFile for FileDigest {
    fn executable(&self) -> bool {
        self.executable
    }
    fn size(&self) -> u64 {
        self.size
    }
}

impl From<&FileValue> for FileDigest {
    fn from(value: &FileValue) -> Self {
        Self {
            digest: content_hash(&value.bytes),
            executable: value.executable,
            size: value.bytes.len() as u64,
        }
    }
}

/// Convert a full-value file map into a digest-only file map, so callers
/// that only need equality (such as [`three_way_tree`] against a local
/// observation) can avoid holding plaintext bytes for every file.
pub fn file_digests(files: &BTreeMap<String, FileValue>) -> BTreeMap<String, FileDigest> {
    files
        .iter()
        .map(|(path, value)| (path.clone(), FileDigest::from(value)))
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureMode {
    /// Capture modified/deleted managed paths and explicitly selected files.
    ManagedAndSelected,
    /// Capture exactly the supplied paths. An empty selection is invalid.
    Only,
}

/// All content is stored before this value is returned. Publication uses this
/// manifest identifier and never rereads the live capture directory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FrozenTree {
    pub manifest_id: BlobHash,
    pub manifest: Manifest,
    pub captured_paths: Vec<String>,
    pub changes: Vec<TreeChange>,
    /// A composition with no change reuses the current tree's identifier.
    pub already_included: bool,
}

/// A complete preflighted mixed local result. This is an inert plan: mutation
/// requires a separate durable transaction with repeated preimage checks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatePlan {
    pub changes: Vec<TreeChange>,
    pub adopted_paths: Vec<String>,
    pub dirty_paths: Vec<String>,
    pub untracked_paths: Vec<String>,
    pub final_files: BTreeMap<String, FileDigest>,
    /// Preserved directories, including empty and opaque Git directories.
    pub final_directories: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalTree {
    pub files: BTreeMap<String, FileDigest>,
    /// Includes preserved empty directories and opaque Git metadata directories.
    pub directories: Vec<String>,
}

/// Parse one exact relative file path per UTF-8 line. A final newline is
/// accepted; blank lines, duplicates and unsafe/private paths are refused.
/// No trimming, escaping, globbing or recursive directory expansion occurs.
pub fn parse_paths(bytes: &[u8]) -> Result<Vec<String>, WorkspaceError> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid("path list must be UTF-8"))?;
    let paths: Vec<String> = text.split_terminator('\n').map(str::to_owned).collect();
    selected_paths(&paths)
}

fn tree_path(path: &str) -> Result<(), WorkspaceError> {
    check_path(path)?;
    if path.split('/').any(|component| {
        let name = component.to_ascii_lowercase();
        name == ".locust"
            || name.starts_with(".locust-workspace-")
            || name.starts_with(".locust-recovery-")
    }) {
        return Err(safe_fs::unsupported(
            path,
            "reserved local workspace metadata",
        ));
    }
    Ok(())
}

fn selected_paths(paths: &[String]) -> Result<Vec<String>, WorkspaceError> {
    let mut paths = paths.to_vec();
    paths.sort();
    for path in &paths {
        tree_path(path)?;
    }
    if paths.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid("selected paths must be unique"));
    }
    Ok(paths)
}

fn checked_manifest(source: &mut dyn BlobSource, id: BlobHash) -> Result<Manifest, WorkspaceError> {
    let tree = manifest(source, id)?;
    for entry in &tree.entries {
        tree_path(&entry.path)?;
    }
    Ok(tree)
}

fn values(
    tree: &Manifest,
    source: &mut dyn BlobSource,
    cache: &mut BTreeMap<BlobHash, Vec<u8>>,
) -> Result<BTreeMap<String, FileValue>, WorkspaceError> {
    tree.entries
        .iter()
        .map(|entry| {
            let bytes = match cache.get(&entry.content) {
                Some(bytes) if bytes.len() as u64 == entry.size => bytes.clone(),
                _ => {
                    let bytes = file_bytes(source, entry)?;
                    cache.insert(entry.content, bytes.clone());
                    bytes
                }
            };
            Ok((
                entry.path.clone(),
                FileValue {
                    bytes,
                    executable: entry.executable,
                },
            ))
        })
        .collect()
}

/// Decode a canonical tree and read/validate every file through the caller's
/// authenticated object source. This performs no capture or storage writes.
pub fn inspect_tree(
    manifest_id: BlobHash,
    source: &mut dyn BlobSource,
) -> Result<(Manifest, BTreeMap<String, FileValue>), WorkspaceError> {
    let tree = checked_manifest(source, manifest_id)?;
    let files = values(&tree, source, &mut BTreeMap::new())?;
    Ok((tree, files))
}

/// Derive exact plaintext/mode changes for review, including across different
/// content-encryption epochs. Every referenced file is validated first.
pub fn diff_trees(
    base_id: BlobHash,
    result_id: BlobHash,
    source: &mut dyn BlobSource,
) -> Result<Vec<TreeChange>, WorkspaceError> {
    let base = checked_manifest(source, base_id)?;
    let result = checked_manifest(source, result_id)?;
    let mut cache = BTreeMap::new();
    let before = values(&base, source, &mut cache)?;
    let after = values(&result, source, &mut cache)?;
    Ok(delta(&before, &after))
}

fn delta(
    before: &BTreeMap<String, FileValue>,
    after: &BTreeMap<String, FileValue>,
) -> Vec<TreeChange> {
    let paths: BTreeSet<_> = before.keys().chain(after.keys()).collect();
    paths
        .into_iter()
        .filter_map(|path| {
            let old = before.get(path);
            let new = after.get(path);
            (old != new).then(|| TreeChange {
                path: path.clone(),
                before: old.cloned(),
                after: new.cloned(),
            })
        })
        .collect()
}

fn seal_tree(
    files: &BTreeMap<String, FileValue>,
    reuse: &[(&Manifest, &BTreeMap<String, FileValue>)],
    store: &mut dyn BlobStore,
) -> Result<(BlobHash, Manifest), WorkspaceError> {
    check_layout(files)?;
    let mut entries = Vec::with_capacity(files.len());
    for (path, file) in files {
        tree_path(path)?;
        let old = reuse.iter().find_map(|(tree, values)| {
            (values.get(path).is_some_and(|old| old.bytes == file.bytes))
                .then(|| {
                    tree.entries
                        .binary_search_by(|entry| entry.path.cmp(path))
                        .ok()
                        .map(|index| tree.entries[index].content)
                })
                .flatten()
        });
        let content = match old {
            Some(content) => content,
            None => {
                if file.bytes.len() > MAX_PLAINTEXT_BYTES {
                    return Err(WorkspaceError::Unsupported {
                        path: path.clone(),
                        reason: "file exceeds the content-object limit".into(),
                    });
                }
                store.store(&file.bytes)?
            }
        };
        entries.push(Entry {
            path: path.clone(),
            content,
            executable: file.executable,
            size: file.bytes.len() as u64,
        });
    }
    let tree = Manifest { entries };
    let bytes = tree.encode().map_err(|error| invalid(error.to_string()))?;
    if bytes.len() > MAX_PLAINTEXT_BYTES {
        return Err(invalid("manifest exceeds content-object limit"));
    }
    Ok((store.store(&bytes)?, tree))
}

/// Capture a Git-free seed or full replacement using only explicit regular
/// file selections. An empty seed requires an explicit empty request.
pub fn capture_seed(
    root: &Path,
    paths: &[String],
    explicit_empty: bool,
    store: &mut dyn BlobStore,
) -> Result<FrozenTree, WorkspaceError> {
    let selected = selected_paths(paths)?;
    if selected.is_empty() != explicit_empty {
        return Err(invalid(
            "provide file selections or explicitly request an empty seed",
        ));
    }
    let root = safe_fs::root(root)?;
    let mut captured = BTreeMap::new();
    for path in &selected {
        let file = safe_fs::read(&root, path)?
            .ok_or_else(|| invalid(format!("selected file {path:?} is absent")))?;
        captured.insert(path.clone(), file);
    }
    recheck_captured(&root, &captured)?;
    let files = captured
        .into_iter()
        .map(|(path, file)| (path, local_value(file)))
        .collect();
    let (manifest_id, manifest) = seal_tree(&files, &[], store)?;
    Ok(FrozenTree {
        manifest_id,
        manifest,
        captured_paths: selected,
        changes: delta(&BTreeMap::new(), &files),
        already_included: false,
    })
}

fn local_value(file: safe_fs::LocalFile) -> FileValue {
    FileValue {
        bytes: file.bytes,
        executable: file.executable,
    }
}

fn recheck_captured(
    root: &File,
    captured: &BTreeMap<String, safe_fs::LocalFile>,
) -> Result<(), WorkspaceError> {
    for (path, file) in captured {
        if safe_fs::read(root, path)?.as_ref().map(|now| &now.identity) != Some(&file.identity) {
            return Err(WorkspaceError::Conflict {
                path: path.clone(),
                reason: "selected file changed during capture".into(),
            });
        }
    }
    Ok(())
}

/// Freeze the complete result of a default or exact-subset capture against an
/// immutable base. New files are included only by explicit selection.
pub fn capture_tree(
    root: &Path,
    base_id: BlobHash,
    paths: &[String],
    mode: CaptureMode,
    store: &mut dyn BlobStore,
) -> Result<FrozenTree, WorkspaceError> {
    let selected = selected_paths(paths)?;
    if mode == CaptureMode::Only && selected.is_empty() {
        return Err(invalid("subset capture requires a path selection"));
    }
    let base = checked_manifest(store, base_id)?;
    let before = values(&base, store, &mut BTreeMap::new())?;
    let root = safe_fs::root(root)?;
    // Explicit directory selections are always invalid, even if they replace a
    // formerly managed file. Managed parent deletions are inferred separately.
    for path in &selected {
        match safe_fs::read(&root, path) {
            Err(error @ WorkspaceError::Unsupported { .. }) if before.contains_key(path) => {
                if !replaced_ancestor(&root, path, selected.iter())? {
                    return Err(error);
                }
            }
            result => {
                let _ = result?;
            }
        }
    }
    let scope: BTreeSet<String> = selected
        .iter()
        .cloned()
        .chain(
            before
                .keys()
                .filter(|_| mode == CaptureMode::ManagedAndSelected)
                .cloned(),
        )
        .collect();
    let mut observations = BTreeMap::new();
    for path in &scope {
        let local = capture_local(&root, path, &scope, &before)?;
        if local.is_none() && !before.contains_key(path) {
            return Err(invalid(format!(
                "selected path {path:?} is absent from both base and local files"
            )));
        }
        observations.insert(path.clone(), local);
    }
    // A second observation of every selected or managed preimage precedes the
    // first store, so even a sink callback cannot retarget this frozen capture.
    for (path, first) in &observations {
        let now = capture_local(&root, path, &scope, &before)?;
        if first.as_ref().map(|file| &file.identity) != now.as_ref().map(|file| &file.identity) {
            return Err(WorkspaceError::Conflict {
                path: path.clone(),
                reason: "file changed during capture".into(),
            });
        }
    }
    let mut after = before.clone();
    for (path, file) in observations {
        match file {
            Some(file) => {
                after.insert(path, local_value(file));
            }
            None => {
                after.remove(&path);
            }
        }
    }
    let changes = delta(&before, &after);
    let captured_paths = selected
        .into_iter()
        .chain(changes.iter().map(|change| change.path.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let (manifest_id, manifest) = if changes.is_empty() {
        (base_id, base)
    } else {
        seal_tree(&after, &[(&base, &before)], store)?
    };
    Ok(FrozenTree {
        manifest_id,
        manifest,
        captured_paths,
        changes,
        already_included: false,
    })
}

fn capture_local(
    root: &File,
    path: &str,
    scope: &BTreeSet<String>,
    before: &BTreeMap<String, FileValue>,
) -> Result<Option<safe_fs::LocalFile>, WorkspaceError> {
    match safe_fs::read(root, path) {
        Err(error @ WorkspaceError::Unsupported { .. }) if before.contains_key(path) => {
            // A selected file can replace an implicit managed directory.
            if replaced_ancestor(root, path, scope.iter())? {
                return Ok(None);
            }
            // A managed file replaced by a directory is a deletion. Opening
            // that exact directory verifies its type without listing or reading
            // descendants. Its new files still require explicit selections.
            if let Ok((parent, leaf)) = safe_fs::parent(root, path, false)
                && safe_fs::dir_at(&parent, &leaf).is_ok()
            {
                return Ok(None);
            }
            Err(error)
        }
        result => result,
    }
}

fn replaced_ancestor<'a>(
    root: &File,
    path: &str,
    scope: impl Iterator<Item = &'a String>,
) -> Result<bool, WorkspaceError> {
    for ancestor in scope.filter(|ancestor| path.starts_with(&format!("{ancestor}/"))) {
        if safe_fs::read(root, ancestor)?.is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Deterministic per-path three-way composition. Absence is a value. No text
/// merge or conflict markers are generated; any conflict returns no tree.
pub fn three_way_tree<V: TreeFile>(
    base: &BTreeMap<String, V>,
    proposed: &BTreeMap<String, V>,
    current: &BTreeMap<String, V>,
) -> Result<BTreeMap<String, V>, WorkspaceError> {
    let paths: BTreeSet<_> = base
        .keys()
        .chain(proposed.keys())
        .chain(current.keys())
        .collect();
    let mut result = BTreeMap::new();
    for path in paths {
        let b = base.get(path);
        let l = proposed.get(path);
        let c = current.get(path);
        let value = if l == b || l == c {
            c
        } else if c == b {
            l
        } else {
            return Err(WorkspaceError::Conflict {
                path: path.clone(),
                reason: "incompatible concurrent changes".into(),
            });
        };
        if let Some(value) = value {
            result.insert(path.clone(), value.clone());
        }
    }
    check_layout(&result)?;
    Ok(result)
}

fn check_layout<V: TreeFile>(files: &BTreeMap<String, V>) -> Result<(), WorkspaceError> {
    Manifest {
        entries: files
            .iter()
            .map(|(path, file)| Entry {
                path: path.clone(),
                executable: file.executable(),
                size: file.size(),
                content: BlobHash([0; 32]),
            })
            .collect(),
    }
    .check()
    .map_err(|error| invalid(error.to_string()))
}

/// Compose one source proposal onto the current accepted tree. The caller
/// verifies accepted lineage ancestry and records proposal source provenance.
pub fn compose_trees(
    base_id: BlobHash,
    proposed_id: BlobHash,
    current_id: BlobHash,
    store: &mut dyn BlobStore,
) -> Result<FrozenTree, WorkspaceError> {
    let base = checked_manifest(store, base_id)?;
    let proposed = checked_manifest(store, proposed_id)?;
    let current = checked_manifest(store, current_id)?;
    let mut cache = BTreeMap::new();
    let b = values(&base, store, &mut cache)?;
    let l = values(&proposed, store, &mut cache)?;
    let c = values(&current, store, &mut cache)?;
    let result = three_way_tree(&b, &l, &c)?;
    let changes = delta(&c, &result);
    let already_included = changes.is_empty();
    let (manifest_id, manifest) = if already_included {
        (current_id, current)
    } else {
        seal_tree(
            &result,
            &[(&current, &c), (&proposed, &l), (&base, &b)],
            store,
        )?
    };
    Ok(FrozenTree {
        manifest_id,
        manifest,
        captured_paths: changes.iter().map(|change| change.path.clone()).collect(),
        changes,
        already_included,
    })
}

#[derive(Default, PartialEq, Eq)]
struct Inventory {
    files: BTreeMap<String, FileDigest>,
    identities: BTreeMap<String, safe_fs::Identity>,
    directories: BTreeMap<String, safe_fs::Identity>,
}

fn inventory(root: &File) -> Result<Inventory, WorkspaceError> {
    fn walk(dir: &File, prefix: &str, result: &mut Inventory) -> Result<(), WorkspaceError> {
        let listed = safe_fs::names(dir)?;
        for name in &listed {
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let stat = fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW)?;
            if fs::FileType::from_raw_mode(stat.st_mode) == fs::FileType::Directory {
                let next = safe_fs::dir_at_listed(dir, name, &listed)?;
                result
                    .directories
                    .insert(path.clone(), safe_fs::Identity::from(&next.metadata()?));
                if !name.eq_ignore_ascii_case(".git") {
                    walk(&next, &path, result)?;
                }
            } else {
                let file = safe_fs::read_at_digest_listed(dir, name, &path, &listed)?
                    .ok_or_else(|| invalid("local file vanished during inspection"))?;
                result
                    .identities
                    .insert(path.clone(), file.identity.clone());
                result.files.insert(
                    path,
                    FileDigest {
                        digest: file.digest,
                        executable: file.executable,
                        size: file.size,
                    },
                );
            }
        }
        Ok(())
    }
    let mut result = Inventory::default();
    walk(root, "", &mut result)?;
    Ok(result)
}

/// Read-only local status observation. No objects are stored and no layout
/// probes are created. A changed, unsupported or unreadable path is an error,
/// rather than evidence that the directory is clean or safe to reset.
pub fn inspect_local_tree(root: &Path) -> Result<LocalTree, WorkspaceError> {
    let directory = safe_fs::root(root)?;
    let first = inventory(&directory)?;
    if first != inventory(&directory)? {
        return Err(WorkspaceError::Conflict {
            path: root.display().to_string(),
            reason: "local layout changed during inspection".into(),
        });
    }
    Ok(LocalTree {
        files: first.files,
        directories: first.directories.into_keys().collect(),
    })
}

/// Inspect all local paths, compute the whole mixed transition and preflight
/// aliases on the checkout's filesystem. No checkout contents are mutated.
/// Unsupported links/special files and unknown inspection results fail closed.
pub fn plan_update(
    root: &Path,
    base_id: BlobHash,
    target_id: BlobHash,
    source: &mut dyn BlobSource,
) -> Result<UpdatePlan, WorkspaceError> {
    let base = checked_manifest(source, base_id)?;
    let target = checked_manifest(source, target_id)?;
    let mut cache = BTreeMap::new();
    let b = values(&base, source, &mut cache)?;
    let c = values(&target, source, &mut cache)?;
    let root_dir = safe_fs::root(root)?;
    let local = inventory(&root_dir)?;
    let b_digests: BTreeMap<_, _> = b
        .iter()
        .map(|(path, value)| {
            (
                path.clone(),
                FileDigest {
                    digest: content_hash(&value.bytes),
                    executable: value.executable,
                    size: value.bytes.len() as u64,
                },
            )
        })
        .collect();
    let c_digests: BTreeMap<_, _> = c
        .iter()
        .map(|(path, value)| {
            (
                path.clone(),
                FileDigest {
                    digest: content_hash(&value.bytes),
                    executable: value.executable,
                    size: value.bytes.len() as u64,
                },
            )
        })
        .collect();
    let final_files = three_way_tree(&b_digests, &local.files, &c_digests)?;
    let mut final_directories = Vec::new();
    for path in local.directories.keys() {
        let replaced = final_files
            .keys()
            .any(|file| path == file || path.starts_with(&format!("{file}/")));
        if replaced {
            if !b.keys().any(|file| file.starts_with(&format!("{path}/"))) {
                return Err(WorkspaceError::Conflict {
                    path: path.clone(),
                    reason: "incoming file collides with an unrelated local directory".into(),
                });
            }
        } else {
            final_directories.push(path.clone());
        }
    }
    probe_layout(root, &root_dir, &final_files, &final_directories)?;
    if local != inventory(&root_dir)? {
        return Err(WorkspaceError::Conflict {
            path: root.display().to_string(),
            reason: "local layout changed during update preflight".into(),
        });
    }
    let adopted_paths = c_digests
        .iter()
        .filter(|(path, value)| {
            !b_digests.contains_key(*path) && local.files.get(*path) == Some(value)
        })
        .map(|(path, _)| path.clone())
        .collect();
    let dirty_paths = b_digests
        .keys()
        .chain(c_digests.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|path| final_files.get(*path) != c_digests.get(*path))
        .cloned()
        .collect();
    let untracked_paths = final_files
        .keys()
        .filter(|path| !b_digests.contains_key(*path) && !c_digests.contains_key(*path))
        .cloned()
        .collect();
    let changes = plan_changes(&root_dir, &local.files, &final_files, &c)?;
    Ok(UpdatePlan {
        changes,
        adopted_paths,
        dirty_paths,
        untracked_paths,
        final_files,
        final_directories,
    })
}

fn plan_changes(
    root: &File,
    local: &BTreeMap<String, FileDigest>,
    final_files: &BTreeMap<String, FileDigest>,
    target: &BTreeMap<String, FileValue>,
) -> Result<Vec<TreeChange>, WorkspaceError> {
    let paths: BTreeSet<_> = local.keys().chain(final_files.keys()).collect();
    let mut changes = Vec::new();
    for path in paths {
        let before_digest = local.get(path);
        let after_digest = final_files.get(path);
        if before_digest == after_digest {
            continue;
        }
        let before = match before_digest {
            Some(_) => {
                let file = safe_fs::read(root, path)?.ok_or_else(|| WorkspaceError::Conflict {
                    path: path.clone(),
                    reason: "local file vanished during planning".into(),
                })?;
                Some(FileValue {
                    bytes: file.bytes,
                    executable: file.executable,
                })
            }
            None => None,
        };
        let after = match after_digest {
            Some(_) => Some(
                target
                    .get(path)
                    .ok_or_else(|| WorkspaceError::Conflict {
                        path: path.clone(),
                        reason: "target manifest lacks a planned file".into(),
                    })?
                    .clone(),
            ),
            None => None,
        };
        changes.push(TreeChange {
            path: path.clone(),
            before,
            after,
        });
    }
    Ok(changes)
}

/// Check actual case/Unicode name semantics using a disposable private sibling
/// skeleton. Every file is create-new; no writable source or object hardlinks
/// exist. The sibling must be on the same device as the checkout.
fn probe_layout(
    root_path: &Path,
    root: &File,
    files: &BTreeMap<String, FileDigest>,
    directories: &[String],
) -> Result<(), WorkspaceError> {
    let canonical = std::fs::canonicalize(root_path)?;
    let parent_path = canonical
        .parent()
        .ok_or_else(|| invalid("checkout has no sibling recovery location"))?;
    let parent = safe_fs::root(parent_path)?;
    if parent.metadata()?.dev() != root.metadata()?.dev() {
        return Err(invalid(
            "checkout has no same-device sibling recovery location",
        ));
    }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = loop {
        let name = format!(
            ".locust-apply-layout-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        match fs::mkdirat(&parent, &name, Mode::from_raw_mode(0o700)) {
            Ok(()) => break name,
            Err(error) if error == rustix::io::Errno::EXIST => continue,
            Err(error) => return Err(error.into()),
        }
    };
    let probe = safe_fs::dir_at(&parent, &name)?;
    let result = (|| {
        for path in directories {
            let (dir, leaf) = probe_parent(&probe, path)?;
            match fs::mkdirat(&dir, &leaf, Mode::from_raw_mode(0o700)) {
                Ok(()) => {}
                Err(error) if error == rustix::io::Errno::EXIST => {}
                Err(error) => return Err(error.into()),
            }
            let _ = safe_fs::dir_at(&dir, &leaf)?;
        }
        for path in files.keys() {
            let path = path.as_str();
            let (dir, leaf) = probe_parent(&probe, path)?;
            safe_fs::write_new(&dir, &leaf, &[], false).map_err(|error| match error {
                WorkspaceError::Io(ref io) if io.kind() == io::ErrorKind::AlreadyExists => {
                    WorkspaceError::Conflict {
                        path: path.to_owned(),
                        reason: "filesystem case, normalization or directory alias".into(),
                    }
                }
                error => error,
            })?;
        }
        Ok(())
    })();
    remove_probe(&probe)?;
    fs::unlinkat(&parent, &name, AtFlags::REMOVEDIR)?;
    result
}

// Layout probes must retain private paths and an opaque .git directory. They
// use the same no-follow descriptor operations but no Git-ancestor exclusion.
fn probe_parent(root: &File, path: &str) -> Result<(File, String), WorkspaceError> {
    let mut dir = root.try_clone()?;
    let mut parts = path.split('/').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            return Ok((dir, part.into()));
        }
        match fs::mkdirat(&dir, part, Mode::from_raw_mode(0o700)) {
            Ok(()) => {}
            Err(error) if error == rustix::io::Errno::EXIST => {}
            Err(error) => return Err(error.into()),
        }
        dir = safe_fs::dir_at(&dir, part)?;
    }
    Err(invalid("empty layout path"))
}

fn remove_probe(dir: &File) -> Result<(), WorkspaceError> {
    for name in safe_fs::names(dir)? {
        let stat = fs::statat(dir, &name, AtFlags::SYMLINK_NOFOLLOW)?;
        if fs::FileType::from_raw_mode(stat.st_mode) == fs::FileType::Directory {
            remove_probe(&safe_fs::dir_at(dir, &name)?)?;
            fs::unlinkat(dir, &name, AtFlags::REMOVEDIR)?;
        } else {
            fs::unlinkat(dir, &name, AtFlags::empty())?;
        }
    }
    Ok(())
}
