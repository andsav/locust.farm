//! Recoverable ordinary-directory updates. The caller persists the descriptor
//! together with its expected daemon binding and target before `execute`, then
//! commits the new binding and operation receipt before `mark_completed`.
//! Filesystem changes are serialized and recoverable, never multi-file atomic.
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, Metadata};
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use locust_proto::crypto::content_hash;
use locust_proto::id::BlobHash;
use rustix::fs::{self, AtFlags, FlockOperation, Mode, OFlags};
use serde::{Deserialize, Serialize};

use crate::files::{check_path, invalid};
use crate::{FileValue, UpdatePlan, WorkspaceError, safe_fs};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryIdentity {
    pub device: u64,
    pub inode: u64,
}
impl DirectoryIdentity {
    fn of(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

/// Exact local recovery identity; persist this opaque descriptor in the daemon.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateDescriptor {
    pub root: PathBuf,
    pub root_identity: DirectoryIdentity,
    pub recovery_directory: PathBuf,
    pub recovery_identity: DirectoryIdentity,
    pub plan_digest: BlobHash,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpdateReport {
    pub descriptor: UpdateDescriptor,
    pub applied_paths: Vec<String>,
    pub adopted_paths: Vec<String>,
    pub dirty_paths: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct FileIdentity {
    directory: DirectoryIdentity,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
    mode: u32,
}
impl FileIdentity {
    fn of(m: &Metadata) -> Self {
        Self {
            directory: DirectoryIdentity::of(m),
            size: m.len(),
            modified: (m.mtime(), m.mtime_nsec()),
            changed: (m.ctime(), m.ctime_nsec()),
            mode: m.mode(),
        }
    }
}
impl From<&safe_fs::Identity> for FileIdentity {
    fn from(identity: &safe_fs::Identity) -> Self {
        Self {
            directory: DirectoryIdentity {
                device: identity.dev,
                inode: identity.ino,
            },
            size: identity.size,
            modified: identity.modified,
            changed: identity.changed,
            mode: identity.mode,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Observation {
    File {
        identity: FileIdentity,
        digest: BlobHash,
        executable: bool,
    },
    Directory {
        identity: DirectoryIdentity,
        opaque: bool,
    },
}
impl Observation {
    // Renames can change ctime. The stable inode, complete bytes and mode must
    // still match, including mtime, size and every permission bit.
    fn renamed_matches(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::File {
                    identity: a,
                    digest: ad,
                    executable: ae,
                },
                Self::File {
                    identity: b,
                    digest: bd,
                    executable: be,
                },
            ) => {
                a.directory == b.directory
                    && a.size == b.size
                    && a.modified == b.modified
                    && a.mode == b.mode
                    && ad == bd
                    && ae == be
            }
            (
                Self::Directory {
                    identity: a,
                    opaque: ao,
                },
                Self::Directory {
                    identity: b,
                    opaque: bo,
                },
            ) => a == b && ao == bo,
            _ => false,
        }
    }
}
type Inventory = BTreeMap<String, Observation>;

#[derive(Clone, Debug, Serialize, Deserialize)]
enum Step {
    RemoveFile {
        path: String,
        artifact: String,
        expected: Observation,
    },
    RemoveDirectory {
        path: String,
        artifact: String,
        expected: Observation,
    },
    Install {
        path: String,
        artifact: String,
        expected: Observation,
    },
}
impl Step {
    fn path(&self) -> &str {
        match self {
            Self::RemoveFile { path, .. }
            | Self::RemoveDirectory { path, .. }
            | Self::Install { path, .. } => path,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct DurableChange {
    path: String,
    before: Option<crate::FileDigest>,
    after: Option<crate::FileDigest>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DurableUpdatePlan {
    changes: Vec<DurableChange>,
    adopted_paths: Vec<String>,
    dirty_paths: Vec<String>,
    untracked_paths: Vec<String>,
    final_files: BTreeMap<String, crate::FileDigest>,
    final_directories: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct DurablePlan {
    version: u32,
    root: PathBuf,
    root_identity: DirectoryIdentity,
    update: DurableUpdatePlan,
    before: Inventory,
    steps: Vec<Step>,
}
#[derive(Serialize, Deserialize)]
enum Phase {
    Intent {
        step: usize,
    },
    Done {
        step: usize,
        observed: Option<Observation>,
    },
}

/// Owns the cross-process lock for the checkout inode. Retain it while committing
/// daemon metadata. Dropping it leaves all recovery artifacts intact.
pub struct PreparedUpdate {
    descriptor: UpdateDescriptor,
    root: File,
    recovery: File,
    plan: DurablePlan,
}

fn conflict(path: &str, reason: &str) -> WorkspaceError {
    WorkspaceError::Conflict {
        path: path.into(),
        reason: reason.into(),
    }
}
fn json<T: Serialize>(value: &T) -> Result<Vec<u8>, WorkspaceError> {
    serde_json::to_vec(value).map_err(|e| invalid(e.to_string()))
}
fn decode<T: for<'a> Deserialize<'a>>(bytes: &[u8]) -> Result<T, WorkspaceError> {
    serde_json::from_slice(bytes).map_err(|e| invalid(e.to_string()))
}

// Open each absolute component without following links, including ancestors of
// the caller-selected checkout or recovery root.
fn absolute_directory(path: &Path) -> Result<File, WorkspaceError> {
    if !path.is_absolute() {
        return Err(invalid("checkout and recovery paths must be absolute"));
    }
    let mut directory = safe_fs::root(Path::new("/"))?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                let name = name
                    .to_str()
                    .ok_or_else(|| invalid("non-UTF-8 directory path"))?;
                directory = safe_fs::dir_at(&directory, name)?;
            }
            _ => return Err(invalid("directory path is not canonical")),
        }
    }
    Ok(directory)
}
fn canonical(path: &Path) -> Result<PathBuf, WorkspaceError> {
    let canonical = std::fs::canonicalize(path)?;
    if canonical != path {
        return Err(invalid(
            "directory path must be canonical and contain no symbolic links",
        ));
    }
    let _directory = absolute_directory(path)?;
    Ok(canonical)
}
fn lock(root: &File) -> Result<(), WorkspaceError> {
    fs::flock(root, FlockOperation::NonBlockingLockExclusive).map_err(|e| {
        if e == rustix::io::Errno::WOULDBLOCK {
            conflict("checkout", "another process is updating this checkout")
        } else {
            e.into()
        }
    })
}
fn validate_placement(
    descriptor: &UpdateDescriptor,
    managed_roots: &[PathBuf],
) -> Result<(), WorkspaceError> {
    if descriptor.root.parent() != descriptor.recovery_directory.parent()
        || descriptor.root == descriptor.recovery_directory
    {
        return Err(invalid(
            "recovery must be a private sibling of the checkout",
        ));
    }
    for managed in managed_roots
        .iter()
        .chain(std::iter::once(&descriptor.root))
    {
        let managed = canonical(managed)?;
        if descriptor.recovery_directory.starts_with(&managed)
            || managed.starts_with(&descriptor.recovery_directory)
        {
            return Err(invalid("recovery directory overlaps a managed checkout"));
        }
    }
    let root = absolute_directory(&descriptor.root)?;
    let recovery = absolute_directory(&descriptor.recovery_directory)?;
    if DirectoryIdentity::of(&root.metadata()?) != descriptor.root_identity
        || DirectoryIdentity::of(&recovery.metadata()?) != descriptor.recovery_identity
        || descriptor.root_identity.device != descriptor.recovery_identity.device
    {
        return Err(invalid("checkout or recovery identity/device changed"));
    }
    let metadata = recovery.metadata()?;
    if metadata.mode() & 0o777 != 0o700 || metadata.uid() != rustix::process::geteuid().as_raw() {
        return Err(invalid(
            "recovery directory is not private and owned by this principal",
        ));
    }
    Ok(())
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
                let child = safe_fs::dir_at_listed(dir, name, &listed)?;
                let opaque = name == ".git";
                result.insert(
                    path.clone(),
                    Observation::Directory {
                        identity: DirectoryIdentity::of(&child.metadata()?),
                        opaque,
                    },
                );
                if !opaque {
                    walk(&child, &path, result)?;
                }
            } else {
                let file = safe_fs::read_at_digest_listed(dir, name, &path, &listed)?
                    .ok_or_else(|| conflict(&path, "file disappeared during inventory"))?;
                result.insert(
                    path,
                    Observation::File {
                        identity: FileIdentity {
                            directory: DirectoryIdentity {
                                device: file.identity.dev,
                                inode: file.identity.ino,
                            },
                            size: file.identity.size,
                            modified: file.identity.modified,
                            changed: file.identity.changed,
                            mode: file.identity.mode,
                        },
                        digest: file.digest,
                        executable: file.executable,
                    },
                );
            }
        }
        Ok(())
    }
    let mut result = BTreeMap::new();
    walk(root, "", &mut result)?;
    Ok(result)
}
/// Reuse verified content digests from `expected` when a file's identity
/// (inode, size, mtime, ctime, mode) is unchanged, re-reading only files
/// whose identity differs. This avoids hashing the entire checkout on every
/// step while preserving interference detection: any content or metadata
/// change alters ctime, forcing a re-read.
fn revalidate_inventory(root: &File, expected: &Inventory) -> Result<Inventory, WorkspaceError> {
    fn walk(
        dir: &File,
        prefix: &str,
        expected: &Inventory,
        result: &mut Inventory,
    ) -> Result<(), WorkspaceError> {
        let listed = safe_fs::names(dir)?;
        for name in &listed {
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let stat = fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW)?;
            if fs::FileType::from_raw_mode(stat.st_mode) == fs::FileType::Directory {
                let child = safe_fs::dir_at_listed(dir, name, &listed)?;
                let opaque = name == ".git";
                result.insert(
                    path.clone(),
                    Observation::Directory {
                        identity: DirectoryIdentity::of(&child.metadata()?),
                        opaque,
                    },
                );
                if !opaque {
                    walk(&child, &path, expected, result)?;
                }
            } else {
                let stat_identity = FileIdentity::from(&safe_fs::Identity::from(&stat));
                match expected.get(&path) {
                    Some(Observation::File {
                        identity: exp,
                        digest,
                        executable,
                        ..
                    }) if stat_identity == *exp => {
                        result.insert(
                            path,
                            Observation::File {
                                identity: stat_identity,
                                digest: *digest,
                                executable: *executable,
                            },
                        );
                    }
                    _ => {
                        let file = safe_fs::read_at_digest_listed(dir, name, &path, &listed)?
                            .ok_or_else(|| {
                                conflict(&path, "file disappeared during revalidation")
                            })?;
                        result.insert(
                            path,
                            Observation::File {
                                identity: FileIdentity {
                                    directory: DirectoryIdentity {
                                        device: file.identity.dev,
                                        inode: file.identity.ino,
                                    },
                                    size: file.identity.size,
                                    modified: file.identity.modified,
                                    changed: file.identity.changed,
                                    mode: file.identity.mode,
                                },
                                digest: file.digest,
                                executable: file.executable,
                            },
                        );
                    }
                }
            }
        }
        Ok(())
    }
    let mut result = BTreeMap::new();
    walk(root, "", expected, &mut result)?;
    Ok(result)
}
fn observe_at(dir: &File, name: &str) -> Result<Option<Observation>, WorkspaceError> {
    let stat = match fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(s) => s,
        Err(e) if e == rustix::io::Errno::NOENT => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if fs::FileType::from_raw_mode(stat.st_mode) == fs::FileType::Directory {
        let child = safe_fs::dir_at(dir, name)?;
        if !safe_fs::names(&child)?.is_empty() {
            return Err(conflict(name, "staged directory is not empty"));
        }
        Ok(Some(Observation::Directory {
            identity: DirectoryIdentity::of(&child.metadata()?),
            opaque: false,
        }))
    } else {
        let file = safe_fs::read_at(dir, name, name)?
            .ok_or_else(|| conflict(name, "artifact disappeared"))?;
        Ok(Some(Observation::File {
            identity: FileIdentity::from(&file.identity),
            digest: content_hash(&file.bytes),
            executable: file.executable,
        }))
    }
}
// macOS fsync alone can leave acknowledged bytes in a drive's volatile
// cache. The existing rustix primitive requests its hardware flush barrier.
fn durable_file_sync(file: &File) -> Result<(), WorkspaceError> {
    file.sync_all()?;
    #[cfg(target_vendor = "apple")]
    fs::fcntl_fullfsync(file)?;
    Ok(())
}
fn private_write(dir: &File, name: &str, bytes: &[u8]) -> Result<(), WorkspaceError> {
    let mut file = File::from(fs::openat(
        dir,
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )?);
    file.write_all(bytes)?;
    durable_file_sync(&file)?;
    dir.sync_all()?;
    Ok(())
}
fn atomic_private_write(dir: &File, name: &str, bytes: &[u8]) -> Result<(), WorkspaceError> {
    let temporary = format!("{name}.tmp");
    match fs::unlinkat(dir, &temporary, AtFlags::empty()) {
        Ok(()) => {}
        Err(error) if error == rustix::io::Errno::NOENT => {}
        Err(error) => return Err(error.into()),
    }
    private_write(dir, &temporary, bytes)?;
    safe_fs::rename_new(dir, &temporary, dir, name)?;
    let file = File::from(fs::openat(
        dir,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    durable_file_sync(&file)
}
fn read_private(dir: &File, name: &str) -> Result<Vec<u8>, WorkspaceError> {
    let mut file = File::from(fs::openat(
        dir,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    let before = file.metadata()?;
    if !before.is_file() || before.nlink() != 1 || before.mode() & 0o777 != 0o600 {
        return Err(invalid("unsafe recovery metadata file"));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    if FileIdentity::of(&before) != FileIdentity::of(&file.metadata()?) {
        return Err(invalid("recovery metadata changed while reading"));
    }
    Ok(bytes)
}
fn file_matches(observation: &Observation, value: &FileValue) -> bool {
    matches!(observation,Observation::File{digest,executable,..} if *digest==content_hash(&value.bytes)&&*executable==value.executable)
}
fn file_digest_matches(observation: &Observation, digest: &crate::FileDigest) -> bool {
    matches!(observation,Observation::File{digest:d,executable,..} if *d==digest.digest&&*executable==digest.executable)
}
fn validate_final_layout(
    final_files: &BTreeMap<String, (BlobHash, bool)>,
    final_directories: &[String],
    before: &Inventory,
) -> Result<(), WorkspaceError> {
    for path in final_files.keys() {
        if !locust_proto::manifest::is_safe_path(path) {
            return Err(invalid("unsafe final layout path"));
        }
        for (other, observation) in before {
            if matches!(observation, Observation::Directory { opaque: true, .. })
                && (path == other || path.starts_with(&format!("{other}/")))
            {
                return Err(conflict(path, "protected directory collision"));
            }
        }
        for parent in ancestors(path) {
            if final_files.contains_key(&parent) {
                return Err(conflict(path, "file/directory prefix collision"));
            }
        }
    }
    for path in final_directories {
        validate_preserved_directory(path, before)?;
        for parent in ancestors(path) {
            if final_files.contains_key(&parent) {
                return Err(conflict(path, "preserved directory under a final file"));
            }
        }
    }
    Ok(())
}

/// Allow observed opaque Git metadata itself, never a path beneath it.
fn validate_preserved_directory(path: &str, before: &Inventory) -> Result<(), WorkspaceError> {
    let Some(Observation::Directory { opaque, .. }) = before.get(path) else {
        return Err(invalid(
            "preserved directory was not observed in the inventory",
        ));
    };
    if locust_proto::manifest::is_safe_path(path) {
        return Ok(());
    }
    let (parent, name) = path
        .rsplit_once('/')
        .map_or((None, path), |(parent, name)| (Some(parent), name));
    if *opaque
        && path.len() <= locust_proto::limits::MAX_PATH_BYTES
        && (name.eq_ignore_ascii_case(".git") || name.eq_ignore_ascii_case("git~1"))
        && parent.is_none_or(locust_proto::manifest::is_safe_path)
    {
        Ok(())
    } else {
        Err(invalid("unsafe preserved directory path"))
    }
}
fn validate_plan(plan: &UpdatePlan, before: &Inventory) -> Result<(), WorkspaceError> {
    let mut resulting: BTreeMap<String, (BlobHash, bool)> = before
        .iter()
        .filter_map(|(path, observation)| match observation {
            Observation::File {
                digest, executable, ..
            } => Some((path.clone(), (*digest, *executable))),
            _ => None,
        })
        .collect();
    let mut seen = BTreeSet::new();
    for change in &plan.changes {
        check_path(&change.path)?;
        if !seen.insert(&change.path) {
            return Err(invalid("duplicate transition path"));
        }
        match (&change.before, before.get(&change.path)) {
            (Some(value), Some(observation)) if file_matches(observation, value) => {}
            (None, None) | (None, Some(Observation::Directory { opaque: false, .. })) => {}
            _ => {
                return Err(conflict(
                    &change.path,
                    "planned preimage does not match the actual local file",
                ));
            }
        }
        if let Some(after) = &change.after {
            resulting.insert(
                change.path.clone(),
                (content_hash(&after.bytes), after.executable),
            );
        } else {
            resulting.remove(&change.path);
        }
    }
    let final_files: BTreeMap<_, _> = plan
        .final_files
        .iter()
        .map(|(path, digest)| (path.clone(), (digest.digest, digest.executable)))
        .collect();
    if resulting != final_files {
        return Err(invalid(
            "update plan omits or changes preserved local files",
        ));
    }
    validate_final_layout(&final_files, &plan.final_directories, before)
}
fn validate_durable_plan(
    plan: &DurableUpdatePlan,
    before: &Inventory,
) -> Result<(), WorkspaceError> {
    let mut resulting: BTreeMap<String, (BlobHash, bool)> = before
        .iter()
        .filter_map(|(path, observation)| match observation {
            Observation::File {
                digest, executable, ..
            } => Some((path.clone(), (*digest, *executable))),
            _ => None,
        })
        .collect();
    let mut seen = BTreeSet::new();
    for change in &plan.changes {
        check_path(&change.path)?;
        if !seen.insert(&change.path) {
            return Err(invalid("duplicate transition path"));
        }
        match (&change.before, before.get(&change.path)) {
            (Some(digest), Some(observation)) if file_digest_matches(observation, digest) => {}
            (None, None) | (None, Some(Observation::Directory { opaque: false, .. })) => {}
            _ => {
                return Err(conflict(
                    &change.path,
                    "planned preimage does not match the actual local file",
                ));
            }
        }
        if let Some(after) = &change.after {
            resulting.insert(change.path.clone(), (after.digest, after.executable));
        } else {
            resulting.remove(&change.path);
        }
    }
    let final_files: BTreeMap<_, _> = plan
        .final_files
        .iter()
        .map(|(path, digest)| (path.clone(), (digest.digest, digest.executable)))
        .collect();
    if resulting != final_files {
        return Err(invalid(
            "update plan omits or changes preserved local files",
        ));
    }
    validate_final_layout(&final_files, &plan.final_directories, before)
}
fn ancestors(path: &str) -> Vec<String> {
    path.match_indices('/')
        .map(|(index, _)| path[..index].to_owned())
        .collect()
}

fn durable_update_plan(update: &UpdatePlan) -> DurableUpdatePlan {
    DurableUpdatePlan {
        changes: update
            .changes
            .iter()
            .map(|change| DurableChange {
                path: change.path.clone(),
                before: change.before.as_ref().map(|value| crate::FileDigest {
                    digest: content_hash(&value.bytes),
                    executable: value.executable,
                    size: value.bytes.len() as u64,
                }),
                after: change.after.as_ref().map(|value| crate::FileDigest {
                    digest: content_hash(&value.bytes),
                    executable: value.executable,
                    size: value.bytes.len() as u64,
                }),
            })
            .collect(),
        adopted_paths: update.adopted_paths.clone(),
        dirty_paths: update.dirty_paths.clone(),
        untracked_paths: update.untracked_paths.clone(),
        final_files: update.final_files.clone(),
        final_directories: update.final_directories.clone(),
    }
}

fn remove_recovery_dir(parent: &File, name: &str) -> Result<(), WorkspaceError> {
    let dir = safe_fs::dir_at(parent, name)?;
    for child in safe_fs::names(&dir)? {
        let stat = fs::statat(&dir, &child, AtFlags::SYMLINK_NOFOLLOW)?;
        if fs::FileType::from_raw_mode(stat.st_mode) == fs::FileType::Directory {
            remove_recovery_dir(&dir, &child)?;
        } else {
            fs::unlinkat(&dir, &child, AtFlags::empty())?;
        }
    }
    fs::unlinkat(parent, name, AtFlags::REMOVEDIR)?;
    parent.sync_all()?;
    Ok(())
}

/// Freeze and durably stage an already-composed transition without changing any
/// checkout file. `managed_roots` must include every locally bound checkout.
pub fn prepare_update(
    root: &Path,
    managed_roots: &[PathBuf],
    update: &UpdatePlan,
) -> Result<PreparedUpdate, WorkspaceError> {
    prepare_update_checked(root, managed_roots, update, || Ok(()))
}

fn prepare_update_checked(
    root: &Path,
    managed_roots: &[PathBuf],
    update: &UpdatePlan,
    staged: impl FnOnce() -> Result<(), WorkspaceError>,
) -> Result<PreparedUpdate, WorkspaceError> {
    let root_path = canonical(root)?;
    let root = safe_fs::root(&root_path)?;
    lock(&root)?;
    let before = inventory(&root)?;
    validate_plan(update, &before)?;
    let parent_path = root_path
        .parent()
        .ok_or_else(|| invalid("filesystem root has no recovery sibling"))?;
    let parent = absolute_directory(parent_path)?;
    if parent.metadata()?.dev() != root.metadata()?.dev() {
        return Err(invalid(
            "checkout mount root has no sibling on its filesystem",
        ));
    }
    // Detect ancestor bindings before even creating a private directory.
    for managed in managed_roots {
        let managed = canonical(managed)?;
        if parent_path.starts_with(&managed) {
            return Err(invalid(
                "checkout's sibling recovery area lies inside a managed root",
            ));
        }
    }
    let root_identity = DirectoryIdentity::of(&root.metadata()?);
    let prefix = format!(
        ".locust-recovery-{}-{}-",
        root_identity.device, root_identity.inode
    );
    for name in safe_fs::names(&parent)? {
        if !name.starts_with(&prefix) {
            continue;
        }
        let prior = safe_fs::dir_at(&parent, &name)?;
        let completion = read_private(&prior, "completed.json")
            .and_then(|bytes| decode::<UpdateDescriptor>(&bytes));
        match completion {
            Ok(done)
                if done.root_identity == root_identity
                    && done.root == root_path
                    && done.recovery_directory == parent_path.join(&name)
                    && done.recovery_identity == DirectoryIdentity::of(&prior.metadata()?) => {}
            _ => {
                // Registration can commit before authorized.json is written.
                // An absent marker never proves an older journal is disposable.
                return Err(WorkspaceError::RecoveryRequired {
                    path: parent_path.join(name),
                    reason: "an earlier same-root operation is incomplete or unknown".into(),
                });
            }
        }
    }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = loop {
        let name = format!(
            "{prefix}{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        match fs::mkdirat(&parent, &name, Mode::from_raw_mode(0o700)) {
            Ok(()) => break name,
            Err(e) if e == rustix::io::Errno::EXIST => continue,
            Err(e) => return Err(e.into()),
        }
    };
    parent.sync_all()?;
    let recovery = safe_fs::dir_at(&parent, &name)?;
    let mut descriptor = UpdateDescriptor {
        root: root_path.clone(),
        root_identity: DirectoryIdentity::of(&root.metadata()?),
        recovery_directory: parent_path.join(&name),
        recovery_identity: DirectoryIdentity::of(&recovery.metadata()?),
        plan_digest: BlobHash([0; 32]),
    };
    validate_placement(&descriptor, managed_roots)?;
    let result = (|| {
        let mut steps = Vec::new();
        for (index, change) in update.changes.iter().enumerate() {
            if let Some(original) = &change.before {
                private_write(&recovery, &format!("original-{index}"), &original.bytes)?;
                steps.push(Step::RemoveFile {
                    path: change.path.clone(),
                    artifact: format!("moved-{index}"),
                    expected: before[&change.path].clone(),
                });
            }
            if let Some(after) = &change.after {
                private_write(&recovery, &format!("replacement-{index}"), &after.bytes)?;
                safe_fs::write_new(
                    &recovery,
                    &format!("new-{index}"),
                    &after.bytes,
                    after.executable,
                )?;
            }
        }
        let mut removed_dirs: Vec<_> = before
            .iter()
            .filter_map(|(path, observation)| {
                matches!(observation, Observation::Directory { opaque: false, .. }).then_some(path)
            })
            .filter(|path| {
                update
                    .final_files
                    .keys()
                    .any(|file| *path == file || path.starts_with(&format!("{file}/")))
            })
            .cloned()
            .collect();
        removed_dirs.sort_by_key(|path| std::cmp::Reverse(path.len()));
        for (index, path) in removed_dirs.into_iter().enumerate() {
            steps.push(Step::RemoveDirectory {
                expected: before[&path].clone(),
                path,
                artifact: format!("removed-directory-{index}"),
            });
        }
        let final_dirs: BTreeSet<_> = update
            .final_files
            .keys()
            .flat_map(|path| ancestors(path))
            .collect();
        for (index, path) in final_dirs.into_iter().enumerate() {
            if matches!(before.get(&path), Some(Observation::Directory { .. })) {
                continue;
            }
            let artifact = format!("directory-{index}");
            fs::mkdirat(&recovery, &artifact, Mode::from_raw_mode(0o755))?;
            recovery.sync_all()?;
            let expected = observe_at(&recovery, &artifact)?.unwrap();
            steps.push(Step::Install {
                path,
                artifact,
                expected,
            });
        }
        for (index, change) in update.changes.iter().enumerate() {
            if change.after.is_some() {
                let artifact = format!("new-{index}");
                let expected = observe_at(&recovery, &artifact)?.unwrap();
                steps.push(Step::Install {
                    path: change.path.clone(),
                    artifact,
                    expected,
                });
            }
        }
        // Probe the whole resulting layout, including preserved untracked files,
        // using this filesystem's case/Unicode comparison rules.
        fs::mkdirat(&recovery, "layout", Mode::from_raw_mode(0o700))?;
        let layout = safe_fs::dir_at(&recovery, "layout")?;
        for path in update.final_files.keys() {
            let (parent, leaf) = layout_parent(&layout, path, true)?;
            safe_fs::write_new(&parent, &leaf, &[], false)?;
        }
        for path in &update.final_directories {
            if update.final_files.contains_key(path) {
                return Err(conflict(path, "preserved directory collides with a file"));
            }
            let _directory = layout_directory(&layout, path, &before)?;
        }
        let plan = DurablePlan {
            version: 1,
            root: root_path,
            root_identity: descriptor.root_identity.clone(),
            update: durable_update_plan(update),
            before,
            steps,
        };
        let bytes = json(&plan)?;
        descriptor.plan_digest = content_hash(&bytes);
        private_write(&recovery, "plan.json", &bytes)?;
        staged()?;
        if inventory(&root)? != plan.before {
            return Err(conflict(
                "checkout",
                "local layout or files changed during preparation",
            ));
        }
        Ok(PreparedUpdate {
            descriptor: descriptor.clone(),
            root,
            recovery,
            plan,
        })
    })();
    match result {
        Ok(prepared) => Ok(prepared),
        Err(error) => match remove_recovery_dir(&parent, &name) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(WorkspaceError::RecoveryRequired {
                path: descriptor.recovery_directory,
                reason: format!("{error}; staging cleanup failed: {cleanup}"),
            }),
        },
    }
}
// Inventory and probe code may inspect preserved private paths, but mutations
// themselves always use checked TreeChange paths and safe_fs::parent.
fn layout_parent(root: &File, path: &str, create: bool) -> Result<(File, String), WorkspaceError> {
    let mut directory = root.try_clone()?;
    let mut parts = path.split('/').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            return Ok((directory, part.into()));
        }
        if create {
            match fs::mkdirat(&directory, part, Mode::from_raw_mode(0o755)) {
                Ok(()) => directory.sync_all()?,
                Err(e) if e == rustix::io::Errno::EXIST => {}
                Err(e) => return Err(e.into()),
            }
        }
        directory = safe_fs::dir_at(&directory, part)?;
    }
    Err(invalid("empty layout path"))
}
fn layout_directory(root: &File, path: &str, before: &Inventory) -> Result<File, WorkspaceError> {
    validate_preserved_directory(path, before)?;
    let (parent, leaf) = layout_parent(root, path, true)?;
    match fs::mkdirat(&parent, &leaf, Mode::from_raw_mode(0o755)) {
        Ok(()) => parent.sync_all()?,
        Err(e) if e == rustix::io::Errno::EXIST => {}
        Err(e) => return Err(e.into()),
    };
    safe_fs::dir_at(&parent, &leaf)
}

pub fn reopen_update(
    descriptor: &UpdateDescriptor,
    managed_roots: &[PathBuf],
) -> Result<PreparedUpdate, WorkspaceError> {
    validate_placement(descriptor, managed_roots)?;
    let root = absolute_directory(&descriptor.root)?;
    lock(&root)?;
    let recovery = absolute_directory(&descriptor.recovery_directory)?;
    let bytes = read_private(&recovery, "plan.json")?;
    if content_hash(&bytes) != descriptor.plan_digest {
        return Err(invalid(
            "recovery plan digest differs from the registered operation",
        ));
    }
    let plan: DurablePlan = decode(&bytes)?;
    if plan.version != 1
        || plan.root != descriptor.root
        || plan.root_identity != descriptor.root_identity
    {
        return Err(invalid("recovery plan does not match checkout identity"));
    }
    validate_durable_plan(&plan.update, &plan.before)?;
    Ok(PreparedUpdate {
        descriptor: descriptor.clone(),
        root,
        recovery,
        plan,
    })
}

/// Reconcile a filesystem completion marker after the caller has read an exact
/// durable Completed receipt from the daemon. No checkout content is scanned:
/// subsequent user edits belong to the new base and cannot revoke that receipt.
/// Failure to mark is optional bookkeeping, never failure of the durable receipt.
pub fn mark_update_completed(descriptor: &UpdateDescriptor) -> Result<(), WorkspaceError> {
    let prepared = reopen_update(descriptor, &[])?;
    if decode::<UpdateDescriptor>(&read_private(&prepared.recovery, "authorized.json")?)?
        != *descriptor
    {
        return Err(invalid("operation authorization identity changed"));
    }
    let phases = prepared.phases()?;
    if phases.len() != prepared.plan.steps.len() * 2 {
        return Err(invalid(
            "completion receipt names an unfinished filesystem journal",
        ));
    }
    for (step, pair) in phases.chunks_exact(2).enumerate() {
        match (&pair[0], &pair[1]) {
            (
                Phase::Intent { step: intended },
                Phase::Done {
                    step: done,
                    observed,
                },
            ) if *intended == step && *done == step => {
                if !prepared.step_result(&prepared.plan.steps[step], observed.as_ref())? {
                    return Err(invalid("completed recovery artifact identity changed"));
                }
            }
            _ => return Err(invalid("invalid completed journal transition")),
        }
    }
    prepared.write_completed_marker(descriptor)
}

impl PreparedUpdate {
    pub fn descriptor(&self) -> &UpdateDescriptor {
        &self.descriptor
    }
    fn validate_identity(&self) -> Result<(), WorkspaceError> {
        validate_placement(&self.descriptor, &[])
    }
    fn phases(&self) -> Result<Vec<Phase>, WorkspaceError> {
        let names = safe_fs::names(&self.recovery)?;
        let mut numbered: Vec<_> = names
            .iter()
            .filter_map(|name| {
                if name
                    .strip_prefix("phase-")
                    .and_then(|suffix| suffix.strip_suffix(".tmp"))
                    .is_some_and(|suffix| suffix.parse::<usize>().is_ok())
                {
                    return None;
                }
                name.strip_prefix("phase-")
                    .map(|suffix| (name, suffix.parse::<usize>()))
            })
            .collect();
        numbered.sort_by_key(|(_, index)| index.as_ref().copied().unwrap_or(usize::MAX));
        let mut result = Vec::new();
        for (expected, (name, index)) in numbered.into_iter().enumerate() {
            if index != Ok(expected) {
                return Err(invalid("non-contiguous or malformed recovery journal"));
            }
            result.push(decode(&read_private(&self.recovery, name)?)?);
        }
        Ok(result)
    }
    fn phase(&self, number: usize, phase: &Phase) -> Result<(), WorkspaceError> {
        atomic_private_write(&self.recovery, &format!("phase-{number}"), &json(phase)?)
    }
    fn authorization(&self, registered: &UpdateDescriptor) -> Result<(), WorkspaceError> {
        if registered != &self.descriptor {
            return Err(invalid(
                "registered operation descriptor differs from the prepared update",
            ));
        }
        self.validate_identity()?;
        match read_private(&self.recovery, "authorized.json") {
            Ok(bytes) => {
                if decode::<UpdateDescriptor>(&bytes)? != self.descriptor {
                    return Err(invalid("operation authorization identity changed"));
                }
                Ok(())
            }
            Err(WorkspaceError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                atomic_private_write(&self.recovery, "authorized.json", &json(registered)?)
            }
            Err(error) => Err(error),
        }
    }
    /// Caller assertion: `registered` was durably persisted together with the
    /// expected binding and target before this method. Files never execute.
    pub fn execute(
        &mut self,
        registered: &UpdateDescriptor,
    ) -> Result<UpdateReport, WorkspaceError> {
        self.execute_inner(registered)
            .map_err(|error| WorkspaceError::RecoveryRequired {
                path: self.descriptor.recovery_directory.clone(),
                reason: error.to_string(),
            })
    }
    fn execute_inner(
        &mut self,
        registered: &UpdateDescriptor,
    ) -> Result<UpdateReport, WorkspaceError> {
        self.authorization(registered)?;
        if content_hash(&read_private(&self.recovery, "plan.json")?) != self.descriptor.plan_digest
        {
            return Err(invalid("durable plan identity changed"));
        }
        for (index, change) in self.plan.update.changes.iter().enumerate() {
            for (prefix, digest) in [("original", &change.before), ("replacement", &change.after)] {
                if let Some(digest) = digest
                    && content_hash(&read_private(&self.recovery, &format!("{prefix}-{index}"))?)
                        != digest.digest
                {
                    return Err(conflict(&change.path, "durable recovery copy changed"));
                }
            }
        }
        let phases = self.phases()?;
        let mut expected = self.plan.before.clone();
        let mut completed = 0;
        let mut intended = false;
        for phase in &phases {
            match phase {
                Phase::Intent { step }
                    if *step == completed && !intended && *step < self.plan.steps.len() =>
                {
                    intended = true
                }
                Phase::Done { step, observed } if *step == completed && intended => {
                    let planned_step = self
                        .plan
                        .steps
                        .get(*step)
                        .ok_or_else(|| invalid("journal step outside plan"))?;
                    if !self.step_result(planned_step, observed.as_ref())? {
                        return Err(invalid(
                            "completed journal step or its recovery artifact changed",
                        ));
                    }
                    let path = planned_step.path();
                    if let Some(observation) = observed {
                        expected.insert(path.into(), observation.clone());
                    } else {
                        expected.remove(path);
                    }
                    completed += 1;
                    intended = false;
                }
                _ => return Err(invalid("invalid write-ahead journal transition")),
            }
        }
        let mut next_phase = phases.len();
        for index in completed..self.plan.steps.len() {
            self.validate_identity()?;
            let step = &self.plan.steps[index];
            let current = revalidate_inventory(&self.root, &expected)?;
            let mut after = current.clone();
            let path = step.path();
            let recovered = if intended {
                self.reconcile_step(step, &expected, &current)?
            } else {
                false
            };
            if !recovered {
                if current != expected {
                    return Err(conflict(
                        path,
                        "layout or preimage changed outside this operation",
                    ));
                }
                if !intended {
                    self.phase(next_phase, &Phase::Intent { step: index })?;
                    next_phase += 1;
                }
                // Validate immediately before destructive filesystem operations.
                if revalidate_inventory(&self.root, &expected)? != expected {
                    return Err(conflict(path, "checkout changed after write-ahead intent"));
                }
                self.perform(step, &expected)?;
                after = revalidate_inventory(&self.root, &expected)?;
            }
            let observed = after.get(path).cloned();
            let mut wanted = expected.clone();
            if let Some(value) = &observed {
                wanted.insert(path.into(), value.clone());
            } else {
                wanted.remove(path);
            }
            if after != wanted || !self.step_result(step, observed.as_ref())? {
                return Err(conflict(
                    path,
                    "external interference during mutation; moved originals retained",
                ));
            }
            self.phase(
                next_phase,
                &Phase::Done {
                    step: index,
                    observed,
                },
            )?;
            next_phase += 1;
            expected = after;
            intended = false;
        }
        if revalidate_inventory(&self.root, &expected)? != expected {
            return Err(conflict("checkout", "files changed after application"));
        }
        let final_files: BTreeMap<_, _> = expected
            .iter()
            .filter_map(|(path, observation)| match observation {
                Observation::File {
                    digest, executable, ..
                } => Some((path.clone(), (*digest, *executable))),
                _ => None,
            })
            .collect();
        let wanted: BTreeMap<_, _> = self
            .plan
            .update
            .final_files
            .iter()
            .map(|(path, digest)| (path.clone(), (digest.digest, digest.executable)))
            .collect();
        if final_files != wanted {
            return Err(conflict(
                "checkout",
                "mixed final result differs from prepared plan",
            ));
        }
        Ok(UpdateReport {
            descriptor: self.descriptor.clone(),
            applied_paths: self
                .plan
                .update
                .changes
                .iter()
                .map(|change| change.path.clone())
                .collect(),
            adopted_paths: self.plan.update.adopted_paths.clone(),
            dirty_paths: self.plan.update.dirty_paths.clone(),
        })
    }
    fn reconcile_step(
        &self,
        step: &Step,
        expected: &Inventory,
        current: &Inventory,
    ) -> Result<bool, WorkspaceError> {
        if current == expected {
            match step {
                Step::RemoveFile { artifact, .. } | Step::RemoveDirectory { artifact, .. }
                    if observe_at(&self.recovery, artifact)?.is_some() =>
                {
                    return Err(conflict(
                        step.path(),
                        "original and recovery positions are ambiguous",
                    ));
                }
                Step::Install {
                    artifact, expected, ..
                } if !observe_at(&self.recovery, artifact)?
                    .is_some_and(|actual| actual == *expected) =>
                {
                    return Err(conflict(step.path(), "staged replacement identity changed"));
                }
                _ => {}
            }
            return Ok(false);
        }
        let observed = current.get(step.path());
        let mut wanted = expected.clone();
        if let Some(value) = observed {
            wanted.insert(step.path().into(), value.clone());
        } else {
            wanted.remove(step.path());
        }
        if *current != wanted || !self.step_result(step, observed)? {
            return Err(conflict(
                step.path(),
                "interrupted operation has unknown or externally changed state",
            ));
        }
        Ok(true)
    }
    fn step_result(
        &self,
        step: &Step,
        observed: Option<&Observation>,
    ) -> Result<bool, WorkspaceError> {
        Ok(match step {
            Step::RemoveFile {
                artifact, expected, ..
            } => {
                observed.is_none()
                    && observe_at(&self.recovery, artifact)?
                        .is_some_and(|actual| expected.renamed_matches(&actual))
            }
            Step::RemoveDirectory {
                artifact, expected, ..
            } => {
                observed.is_none()
                    && observe_at(&self.recovery, artifact)?
                        .is_some_and(|actual| expected.renamed_matches(&actual))
            }
            Step::Install {
                artifact, expected, ..
            } => {
                observe_at(&self.recovery, artifact)?.is_none()
                    && observed.is_some_and(|actual| expected.renamed_matches(actual))
            }
        })
    }
    fn perform(&self, step: &Step, inventory: &Inventory) -> Result<(), WorkspaceError> {
        let mut parent = self.root.try_clone()?;
        for path in ancestors(step.path()) {
            let name = path.rsplit('/').next().unwrap();
            parent = safe_fs::dir_at(&parent, name)?;
            if !matches!(inventory.get(&path), Some(Observation::Directory {identity, opaque:false})
                if *identity == DirectoryIdentity::of(&parent.metadata()?))
            {
                return Err(conflict(&path, "ancestor identity changed before mutation"));
            }
        }
        let leaf = step.path().rsplit('/').next().unwrap().to_owned();
        match step {
            Step::RemoveFile {
                artifact, expected, ..
            } => {
                if !observe_at(&parent, &leaf)?.is_some_and(|current| current == *expected) {
                    return Err(conflict(
                        step.path(),
                        "file identity changed before removal",
                    ));
                }
                safe_fs::rename_new(&parent, &leaf, &self.recovery, artifact)
            }
            Step::RemoveDirectory {
                artifact, expected, ..
            } => {
                if !observe_at(&parent, &leaf)?.is_some_and(|current| current == *expected) {
                    return Err(conflict(
                        step.path(),
                        "directory identity changed before removal",
                    ));
                }
                // Moving preserves the removed inode for post-syscall identity
                // validation and restart; external replacement is never erased.
                safe_fs::rename_new(&parent, &leaf, &self.recovery, artifact)
            }
            Step::Install {
                artifact, expected, ..
            } => {
                if !observe_at(&self.recovery, artifact)?
                    .is_some_and(|current| current == *expected)
                {
                    return Err(conflict(
                        step.path(),
                        "staged identity changed before installation",
                    ));
                }
                safe_fs::rename_new(&self.recovery, artifact, &parent, &leaf)
            }
        }
    }
    /// Call only after the daemon atomically committed the updated binding and
    /// completed operation with this exact descriptor; artifacts stay retained.
    pub fn mark_completed(&mut self, registered: &UpdateDescriptor) -> Result<(), WorkspaceError> {
        self.execute(registered)?;
        self.write_completed_marker(registered)
    }
    fn write_completed_marker(&self, registered: &UpdateDescriptor) -> Result<(), WorkspaceError> {
        match read_private(&self.recovery, "completed.json") {
            Ok(bytes) if decode::<UpdateDescriptor>(&bytes)? == self.descriptor => Ok(()),
            Ok(_) => Err(invalid("completion identity changed")),
            Err(WorkspaceError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                atomic_private_write(&self.recovery, "completed.json", &json(registered)?)
            }
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TreeChange;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn value(text: &str) -> FileValue {
        FileValue {
            bytes: text.as_bytes().to_vec(),
            executable: false,
        }
    }
    fn fixture() -> (tempfile::TempDir, PathBuf, UpdatePlan) {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("checkout");
        std::fs::create_dir(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        std::fs::create_dir(root.join("folder")).unwrap();
        std::fs::create_dir(root.join("empty")).unwrap();
        let originals: BTreeMap<_, _> = [
            ("plain", "old"),
            ("gone", "delete"),
            ("parent", "file"),
            ("folder/a", "descendant"),
            ("keep", "dirty"),
            ("untracked", "local"),
        ]
        .into_iter()
        .map(|(path, text)| (path.to_owned(), value(text)))
        .collect();
        for (path, file) in &originals {
            std::fs::write(root.join(path), &file.bytes).unwrap();
        }
        let final_values: BTreeMap<_, _> = [
            ("plain", "new"),
            ("parent/child", "child"),
            ("folder", "replacement"),
            ("keep", "dirty"),
            ("untracked", "local"),
        ]
        .into_iter()
        .map(|(path, text)| (path.to_owned(), value(text)))
        .collect();
        let finals: BTreeMap<_, _> = final_values
            .iter()
            .map(|(path, v)| {
                (
                    path.clone(),
                    crate::FileDigest {
                        digest: content_hash(&v.bytes),
                        executable: v.executable,
                        size: v.bytes.len() as u64,
                    },
                )
            })
            .collect();
        let changes: BTreeSet<_> = originals.keys().chain(finals.keys()).cloned().collect();
        let changes = changes
            .into_iter()
            .filter(|path| {
                let orig = originals.get(path);
                let fin = finals.get(path).map(|d| (d.digest, d.executable));
                orig.map(|v| (content_hash(&v.bytes), v.executable)) != fin
            })
            .map(|path| TreeChange {
                before: originals.get(&path).cloned(),
                after: final_values.get(&path).cloned(),
                path,
            })
            .collect();
        let plan = UpdatePlan {
            changes,
            adopted_paths: vec!["untracked".into()],
            dirty_paths: vec!["keep".into()],
            untracked_paths: vec!["untracked".into()],
            final_files: finals,
            final_directories: vec!["empty".into()],
        };
        (temporary, root, plan)
    }
    fn assert_final(root: &Path, plan: &UpdatePlan) {
        for path in plan.final_files.keys() {
            let text = match path.as_str() {
                "plain" => "new",
                "parent/child" => "child",
                "folder" => "replacement",
                "keep" => "dirty",
                "untracked" => "local",
                _ => unreachable!("fixture final path {path}"),
            };
            assert_eq!(std::fs::read(root.join(path)).unwrap(), text.as_bytes());
        }
        assert!(!root.join("gone").exists());
        assert!(!root.join("folder/a").exists());
        assert!(root.join("empty").is_dir());
    }
    #[test]
    fn stages_outside_checkout_and_verifies_mixed_result_before_completion() {
        let (_temp, root, plan) = fixture();
        let mut prepared = prepare_update(&root, std::slice::from_ref(&root), &plan).unwrap();
        let descriptor = prepared.descriptor().clone();
        assert_eq!(std::fs::read(root.join("plain")).unwrap(), b"old");
        assert_eq!(
            descriptor.root.parent(),
            descriptor.recovery_directory.parent()
        );
        assert!(!descriptor.recovery_directory.starts_with(&root));
        assert_eq!(
            std::fs::metadata(&descriptor.recovery_directory)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(descriptor.recovery_directory.join("plan.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let report = prepared.execute(&descriptor).unwrap();
        assert_eq!(report.dirty_paths, vec!["keep"]);
        assert_eq!(report.adopted_paths, vec!["untracked"]);
        assert_final(&root, &plan);
        assert!(
            !descriptor
                .recovery_directory
                .join("completed.json")
                .exists()
        );
        drop(prepared);
        let mut reopened = reopen_update(&descriptor, std::slice::from_ref(&root)).unwrap();
        reopened.execute(&descriptor).unwrap();
        reopened.mark_completed(&descriptor).unwrap();
        reopened.mark_completed(&descriptor).unwrap();
        assert!(
            descriptor
                .recovery_directory
                .join("completed.json")
                .exists()
        );
        assert_eq!(
            std::fs::read(descriptor.recovery_directory.join("original-1")).unwrap(),
            b"descendant"
        );
        drop(reopened);
        let same = UpdatePlan {
            changes: vec![],
            adopted_paths: vec![],
            dirty_paths: vec!["keep".into()],
            untracked_paths: vec![],
            final_files: plan.final_files.clone(),
            final_directories: vec!["empty".into(), "parent".into()],
        };
        prepare_update(&root, std::slice::from_ref(&root), &same).unwrap();
    }
    #[test]
    fn committed_receipt_marker_does_not_revalidate_subsequent_user_edits() {
        let (_temp, root, plan) = fixture();
        let mut prepared = prepare_update(&root, std::slice::from_ref(&root), &plan).unwrap();
        let descriptor = prepared.descriptor().clone();
        assert!(mark_update_completed(&descriptor).is_err());
        prepared.execute(&descriptor).unwrap();
        drop(prepared);
        std::fs::write(root.join("plain"), b"user edits after daemon completion").unwrap();
        symlink("plain", root.join("user-symlink")).unwrap();
        mark_update_completed(&descriptor).unwrap();
        mark_update_completed(&descriptor).unwrap();
        assert_eq!(
            std::fs::read(root.join("plain")).unwrap(),
            b"user edits after daemon completion"
        );
        assert!(root.join("user-symlink").is_symlink());
        let mut mismatched = descriptor.clone();
        mismatched.plan_digest = BlobHash([0; 32]);
        assert!(mark_update_completed(&mismatched).is_err());
    }
    #[test]
    fn resumes_every_write_ahead_boundary_including_directory_transitions() {
        for mutate in [false, true] {
            let step_count = {
                let (_temp, root, plan) = fixture();
                prepare_update(&root, std::slice::from_ref(&root), &plan)
                    .unwrap()
                    .plan
                    .steps
                    .len()
            };
            for stop in 0..step_count {
                let (_temp, root, plan) = fixture();
                let prepared = prepare_update(&root, std::slice::from_ref(&root), &plan).unwrap();
                let descriptor = prepared.descriptor().clone();
                prepared.authorization(&descriptor).unwrap();
                let mut expected = prepared.plan.before.clone();
                for index in 0..=stop {
                    prepared
                        .phase(index * 2, &Phase::Intent { step: index })
                        .unwrap();
                    if index < stop || mutate {
                        prepared
                            .perform(&prepared.plan.steps[index], &expected)
                            .unwrap();
                        let current = inventory(&prepared.root).unwrap();
                        if index < stop {
                            prepared
                                .phase(
                                    index * 2 + 1,
                                    &Phase::Done {
                                        step: index,
                                        observed: current
                                            .get(prepared.plan.steps[index].path())
                                            .cloned(),
                                    },
                                )
                                .unwrap();
                        }
                        expected = current;
                    }
                }
                drop(prepared);
                let mut reopened = reopen_update(&descriptor, std::slice::from_ref(&root)).unwrap();
                reopened
                    .execute(&descriptor)
                    .unwrap_or_else(|error| panic!("boundary {stop}, mutated {mutate}: {error}"));
                assert_final(&root, &plan);
            }
        }
    }
    #[test]
    fn interrupted_journal_temp_is_not_a_committed_phase() {
        let (_temp, root, plan) = fixture();
        let mut prepared = prepare_update(&root, std::slice::from_ref(&root), &plan).unwrap();
        private_write(&prepared.recovery, "phase-0.tmp", b"partial").unwrap();
        let descriptor = prepared.descriptor().clone();
        prepared.execute(&descriptor).unwrap();
        assert_final(&root, &plan);
    }
    #[test]
    fn external_change_or_unknown_journal_never_authorizes_mutation() {
        for corrupt_journal in [false, true] {
            let (_temp, root, plan) = fixture();
            let prepared = prepare_update(&root, std::slice::from_ref(&root), &plan).unwrap();
            let descriptor = prepared.descriptor().clone();
            if corrupt_journal {
                private_write(&prepared.recovery, "phase-0", b"partial").unwrap();
            } else {
                std::fs::write(root.join("untracked"), b"external").unwrap();
            }
            drop(prepared);
            let mut reopened = reopen_update(&descriptor, std::slice::from_ref(&root)).unwrap();
            assert!(reopened.execute(&descriptor).is_err());
            assert_eq!(std::fs::read(root.join("plain")).unwrap(), b"old");
            assert_eq!(std::fs::read(root.join("folder/a")).unwrap(), b"descendant");
        }
    }
    #[test]
    fn external_writer_after_removal_preserves_both_writer_and_original() {
        let (_temp, root, plan) = fixture();
        let prepared = prepare_update(&root, std::slice::from_ref(&root), &plan).unwrap();
        let descriptor = prepared.descriptor().clone();
        prepared.authorization(&descriptor).unwrap();
        prepared.phase(0, &Phase::Intent { step: 0 }).unwrap();
        prepared
            .perform(&prepared.plan.steps[0], &prepared.plan.before)
            .unwrap();
        let path = prepared.plan.steps[0].path();
        std::fs::write(root.join(path), b"external replacement").unwrap();
        let path = path.to_owned();
        drop(prepared);
        let mut reopened = reopen_update(&descriptor, std::slice::from_ref(&root)).unwrap();
        assert!(reopened.execute(&descriptor).is_err());
        assert_eq!(
            std::fs::read(root.join(path)).unwrap(),
            b"external replacement"
        );
        assert_eq!(
            std::fs::read(descriptor.recovery_directory.join("moved-1")).unwrap(),
            b"descendant"
        );
    }
    #[test]
    fn rejects_overlapping_managed_roots_tampering_and_same_root_abandoned_update() {
        let (_temp, root, plan) = fixture();
        let parent = root.parent().unwrap().to_owned();
        assert!(prepare_update(&root, &[parent], &plan).is_err());
        let prepared = prepare_update(&root, std::slice::from_ref(&root), &plan).unwrap();
        let descriptor = prepared.descriptor().clone();
        drop(prepared);
        // Registration may have committed before the local authorization marker.
        assert!(
            !descriptor
                .recovery_directory
                .join("authorized.json")
                .exists()
        );
        assert!(matches!(
            prepare_update(&root, std::slice::from_ref(&root), &plan),
            Err(WorkspaceError::RecoveryRequired { .. })
        ));
        let prepared = reopen_update(&descriptor, std::slice::from_ref(&root)).unwrap();
        prepared.authorization(&descriptor).unwrap();
        drop(prepared);
        assert!(matches!(
            prepare_update(&root, std::slice::from_ref(&root), &plan),
            Err(WorkspaceError::RecoveryRequired { .. })
        ));
        let mut wrong = descriptor.clone();
        wrong.plan_digest = BlobHash([7; 32]);
        assert!(reopen_update(&wrong, std::slice::from_ref(&root)).is_err());
        wrong = descriptor.clone();
        wrong.root_identity.inode += 1;
        assert!(reopen_update(&wrong, std::slice::from_ref(&root)).is_err());
        std::fs::write(
            descriptor.recovery_directory.join("original-1"),
            b"changed backup",
        )
        .unwrap();
        let mut reopened = reopen_update(&descriptor, std::slice::from_ref(&root)).unwrap();
        assert!(reopened.execute(&descriptor).is_err());
        assert_eq!(std::fs::read(root.join("plain")).unwrap(), b"old");
    }
    #[test]
    fn refuses_links_and_complete_layout_file_directory_collision() {
        let (_temp, root, mut plan) = fixture();
        symlink("plain", root.join("link")).unwrap();
        assert!(prepare_update(&root, std::slice::from_ref(&root), &plan).is_err());
        std::fs::remove_file(root.join("link")).unwrap();
        std::fs::hard_link(root.join("plain"), root.join("linked")).unwrap();
        assert!(prepare_update(&root, std::slice::from_ref(&root), &plan).is_err());
        std::fs::remove_file(root.join("linked")).unwrap();
        plan.final_directories.push("folder".into());
        assert!(prepare_update(&root, std::slice::from_ref(&root), &plan).is_err());
        assert!(root.join("folder/a").is_file());
    }
    #[test]
    fn lock_child() {
        if let Some(root) = std::env::var_os("LOCUST_TEST_LOCK_ROOT") {
            let root = absolute_directory(Path::new(&root)).unwrap();
            assert!(lock(&root).is_err());
        }
    }
    #[test]
    fn checkout_inode_lock_serializes_independent_processes() {
        let (_temp, root, plan) = fixture();
        let _prepared = prepare_update(&root, std::slice::from_ref(&root), &plan).unwrap();
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["transaction::tests::lock_child", "--exact", "--nocapture"])
            .env("LOCUST_TEST_LOCK_ROOT", &root)
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "{}",
            String::from_utf8_lossy(&child.stderr)
        );
        assert!(prepare_update(&root, std::slice::from_ref(&root), &plan).is_err());
    }

    #[test]
    fn inventory_lists_each_directory_once_not_once_per_child() {
        let (_temp, root, _plan) = fixture();
        let dir = root.join("many");
        std::fs::create_dir(&dir).unwrap();
        for n in 0..200u32 {
            std::fs::write(dir.join(n.to_string()), n.to_string().as_bytes()).unwrap();
        }
        let checkout = safe_fs::root(&root).unwrap();
        safe_fs::NAMES_CALLS.with(|c| c.set(0));
        let observed = inventory(&checkout).unwrap();
        let calls = safe_fs::NAMES_CALLS.with(|c| c.get());
        assert_eq!(
            observed.len(),
            200 + 9,
            "all files and directories observed"
        );
        assert!(
            calls <= 5,
            "names called {calls} times for 4 directories + root; expected linear"
        );
    }

    #[test]
    fn durable_plan_stores_digests_not_preserved_file_bytes() {
        let (_temp, root, plan) = fixture();
        let preserved = vec![0x42u8; 2 * 1024 * 1024];
        std::fs::write(root.join("untracked"), &preserved).unwrap();
        let mut updated = plan.clone();
        updated.final_files.insert(
            "untracked".into(),
            crate::FileDigest {
                digest: content_hash(&preserved),
                executable: false,
                size: preserved.len() as u64,
            },
        );
        let mut prepared = prepare_update(&root, std::slice::from_ref(&root), &updated).unwrap();
        let plan_bytes = read_private(&prepared.recovery, "plan.json").unwrap();
        assert!(
            !plan_bytes
                .windows(64)
                .any(|window| window == &preserved[..window.len()]),
            "preserved file bytes serialized into the durable plan"
        );
        let decoded: DurablePlan = decode(&plan_bytes).unwrap();
        assert_eq!(decoded.update.final_files.len(), updated.final_files.len());
        let digest = content_hash(&preserved);
        assert_eq!(
            decoded.update.final_files["untracked"],
            crate::FileDigest {
                digest,
                executable: false,
                size: preserved.len() as u64,
            }
        );
        let descriptor = prepared.descriptor().clone();
        prepared.execute(&descriptor).unwrap();
        // The preserved large file is unchanged; the changed files match the fixture.
        assert_eq!(std::fs::read(root.join("untracked")).unwrap(), preserved);
        assert_eq!(std::fs::read(root.join("plain")).unwrap(), b"new");
    }

    #[test]
    fn failed_pre_registration_preparation_is_cleaned_up_for_safe_retry() {
        let (_temp, root, plan) = fixture();
        let mut recovery_dir = None;
        let failed = prepare_update_checked(&root, std::slice::from_ref(&root), &plan, || {
            let staged = std::fs::read_dir(root.parent().unwrap())?
                .map(|entry| entry.unwrap().path())
                .find(|path| path.join("plan.json").is_file())
                .expect("the immutable plan was written before interference");
            recovery_dir = Some(staged);
            std::fs::write(root.join("untracked"), b"external edit")?;
            Ok(())
        });
        assert!(matches!(failed, Err(WorkspaceError::Conflict { .. })));
        assert!(!recovery_dir.unwrap().exists());
        // A corrected plan preserves the edit and can immediately retry.
        let mut updated = plan.clone();
        updated.final_files.insert(
            "untracked".into(),
            crate::FileDigest {
                digest: content_hash(b"external edit"),
                executable: false,
                size: 13,
            },
        );
        let prepared = prepare_update(&root, std::slice::from_ref(&root), &updated).unwrap();
        // The new preparation succeeds and applies correctly.
        let descriptor = prepared.descriptor().clone();
        drop(prepared);
        let mut reopened = reopen_update(&descriptor, std::slice::from_ref(&root)).unwrap();
        reopened.execute(&descriptor).unwrap();
        assert_eq!(
            std::fs::read(root.join("untracked")).unwrap(),
            b"external edit"
        );
    }

    #[test]
    fn unknown_recovery_directory_is_never_deleted_automatically() {
        let (_temp, root, plan) = fixture();
        let identity = DirectoryIdentity::of(&std::fs::metadata(&root).unwrap());
        let recovery_dir = root.parent().unwrap().join(format!(
            ".locust-recovery-{}-{}-unknown",
            identity.device, identity.inode
        ));
        std::fs::create_dir(&recovery_dir).unwrap();
        std::fs::write(recovery_dir.join("outside-edit"), b"preserve").unwrap();
        assert!(matches!(
            prepare_update(&root, std::slice::from_ref(&root), &plan),
            Err(WorkspaceError::RecoveryRequired { .. })
        ));
        assert_eq!(
            std::fs::read(recovery_dir.join("outside-edit")).unwrap(),
            b"preserve"
        );
    }

    /// An in-memory object store standing in for the daemon, so unit tests can
    /// drive the public `plan_update` -> `prepare_update` -> `execute` flow.
    use locust_proto::manifest::{Entry, Manifest};
    #[derive(Default)]
    struct MemStore {
        objects: std::collections::HashMap<BlobHash, Vec<u8>>,
    }
    impl crate::BlobSink for MemStore {
        fn store(&mut self, plaintext: &[u8]) -> std::io::Result<BlobHash> {
            let hash = content_hash(plaintext);
            self.objects.insert(hash, plaintext.to_vec());
            Ok(hash)
        }
    }
    impl crate::BlobSource for MemStore {
        fn fetch(&mut self, hash: &BlobHash) -> std::io::Result<Option<Vec<u8>>> {
            Ok(self.objects.get(hash).cloned())
        }
    }
    fn manifest_entry(store: &mut MemStore, path: &str, bytes: &[u8], executable: bool) -> Entry {
        use crate::BlobSink;
        Entry {
            path: path.to_owned(),
            executable,
            size: bytes.len() as u64,
            content: store.store(bytes).unwrap(),
        }
    }
    fn stored_manifest(store: &mut MemStore, entries: Vec<Entry>) -> (BlobHash, Manifest) {
        use crate::BlobSink;
        let mut entries = entries;
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let manifest = Manifest { entries };
        (store.store(&manifest.encode().unwrap()).unwrap(), manifest)
    }

    /// An opaque `.git` directory preserved by the planner must survive
    /// plan/prepare/execute with its contents and identity intact, even though
    /// the manifest path rules reject `.git`. This exercises the narrow
    /// preserved-directory exception end to end.
    #[test]
    fn preserved_opaque_git_directory_survives_plan_prepare_execute() {
        use crate::plan_update;

        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("checkout");
        std::fs::create_dir(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        std::fs::write(root.join("plain"), b"old").unwrap();
        // Opaque Git metadata: the inventory records `.git` but never walks in.
        std::fs::create_dir(root.join(".git")).unwrap();
        std::fs::write(root.join(".git/config"), b"[core]\n\trepository = true\n").unwrap();
        std::fs::set_permissions(
            root.join(".git/config"),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        let git_identity = DirectoryIdentity::of(&std::fs::metadata(root.join(".git")).unwrap());
        let config_bytes = std::fs::read(root.join(".git/config")).unwrap();
        let config_mode = std::fs::metadata(root.join(".git/config"))
            .unwrap()
            .permissions()
            .mode();

        let mut store = MemStore::default();
        let base_entry = manifest_entry(&mut store, "plain", b"old", false);
        let (base_id, _) = stored_manifest(&mut store, vec![base_entry]);
        let target_entry = manifest_entry(&mut store, "plain", b"new", false);
        let (target_id, _) = stored_manifest(&mut store, vec![target_entry]);

        let plan = plan_update(&root, base_id, target_id, &mut store).unwrap();
        assert!(plan.final_directories.contains(&".git".to_owned()));
        assert!(!plan.final_files.contains_key(".git"));
        assert!(
            !plan.final_files.keys().any(|p| p.starts_with(".git/")),
            "nothing under .git may be a planned file"
        );

        let mut prepared = prepare_update(&root, std::slice::from_ref(&root), &plan).unwrap();
        let descriptor = prepared.descriptor().clone();
        prepared.execute(&descriptor).unwrap();

        // The managed file advanced; the opaque Git metadata is untouched.
        assert_eq!(std::fs::read(root.join("plain")).unwrap(), b"new");
        assert_eq!(
            std::fs::read(root.join(".git/config")).unwrap(),
            config_bytes
        );
        assert_eq!(
            std::fs::metadata(root.join(".git/config"))
                .unwrap()
                .permissions()
                .mode(),
            config_mode
        );
        assert_eq!(
            DirectoryIdentity::of(&std::fs::metadata(root.join(".git")).unwrap()),
            git_identity,
            "the .git directory identity survives"
        );
    }

    /// A preserved `.git` directory is allowed only when the inventory
    /// observed it as opaque; an unobserved reserved path, a path underneath
    /// `.git`, or an unsafe segment is still refused.
    #[test]
    fn preserved_directory_validation_refuses_unobserved_unsafe_and_nested_git() {
        let (_temp, root, plan) = fixture();

        // `.git` was never created in this fixture, so it is unobserved.
        let mut unobserved = plan.clone();
        unobserved.final_directories.push(".git".into());
        assert!(prepare_update(&root, std::slice::from_ref(&root), &unobserved).is_err());

        // A path underneath an opaque `.git` is never a preserved directory.
        std::fs::create_dir(root.join(".git")).unwrap();
        std::fs::write(root.join(".git/config"), b"x").unwrap();
        let before_git = inventory(&safe_fs::root(&root).unwrap()).unwrap();
        assert!(matches!(
            validate_preserved_directory(".git/config", &before_git),
            Err(WorkspaceError::Invalid(_))
        ));
        // The opaque `.git` itself is now allowed.
        validate_preserved_directory(".git", &before_git).unwrap();
        std::fs::remove_dir_all(root.join(".git")).unwrap();

        // Unsafe segments are still refused even when the path is observed.
        let mut unsafe_plan = plan.clone();
        unsafe_plan.final_directories.push("a/../b".into());
        assert!(prepare_update(&root, std::slice::from_ref(&root), &unsafe_plan).is_err());
    }

    #[test]
    fn opaque_directory_observations_cannot_authorize_unsafe_paths() {
        let (_temp, root, _) = fixture();
        std::fs::create_dir(root.join(".git")).unwrap();
        let mut before = inventory(&safe_fs::root(&root).unwrap()).unwrap();
        let opaque = before[".git"].clone();
        for path in ["nested/.git", "nested/git~1", "nested/.GIT"] {
            before.insert(path.into(), opaque.clone());
            validate_preserved_directory(path, &before).unwrap();
        }
        for path in [
            "/.git",
            "a/../b",
            ".git/child",
            ".git/child/.git",
            "nested//.git",
        ] {
            before.insert(path.into(), opaque.clone());
            assert!(
                validate_preserved_directory(path, &before).is_err(),
                "{path}"
            );
        }
        let long = format!(
            "{}.git",
            "a/".repeat(locust_proto::limits::MAX_PATH_BYTES / 2)
        );
        before.insert(long.clone(), opaque);
        assert!(validate_preserved_directory(&long, &before).is_err());
    }
}
