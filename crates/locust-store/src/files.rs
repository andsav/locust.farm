//! The directory of large content objects, `blobs/` in the state directory.
//!
//! Names are a content hash in lowercase hex, plus a suffix for files that
//! are not held objects:
//!
//! - `<hash>`: a held object, once a database row names it;
//! - `<hash>.tmp`: an object being written, never valid after a restart;
//! - `<hash>.staged`: the partial copy of an object being received, whose
//!   length is the staged length.
//!
//! Every write that a caller is told is durable is synced first, file then
//! directory; `File::sync_all` and `sync_data` use F_FULLFSYNC on macOS.
//! Removals are durable only where the contract needs them to be (discarded
//! staging); any other leftover is collected on open.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, FileExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use locust_proto::crypto::ContentHasher;
use locust_proto::id::BlobHash;
use locust_proto::store::StoreError;

use crate::error::{corrupted, file};

const TEMPORARY: &str = ".tmp";
const STAGED: &str = ".staged";

/// What a file in the directory is, by its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Object,
    Temporary,
    Staged,
}

#[derive(Debug)]
pub(crate) struct Files {
    dir: PathBuf,
}

impl Files {
    /// Uses `dir`, creating it (and the state directory) owner-only if
    /// missing. Each newly created directory's entry is synced in its
    /// parent before this returns.
    pub(crate) fn create(dir: PathBuf) -> Result<Self, StoreError> {
        create_directories(&dir, sync_directory)?;
        Ok(Self { dir })
    }

    fn path(&self, hash: &BlobHash, suffix: &str) -> PathBuf {
        self.dir.join(format!("{hash}{suffix}"))
    }

    /// Makes the directory's entries (created, renamed or removed files)
    /// durable.
    pub(crate) fn sync(&self) -> Result<(), StoreError> {
        sync_directory(&self.dir).map_err(|error| file("sync", &self.dir, error))
    }

    /// Writes an object under its final name, durable except for the
    /// directory entry: the caller syncs the directory once for all the
    /// objects of a commit.
    pub(crate) fn write(&self, hash: &BlobHash, bytes: &[u8]) -> Result<(), StoreError> {
        let temporary = self.path(hash, TEMPORARY);
        let mut out = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|error| file("create", &temporary, error))?;
        out.write_all(bytes)
            .and_then(|()| out.sync_all())
            .map_err(|error| file("write", &temporary, error))?;
        let path = self.path(hash, "");
        fs::rename(&temporary, &path).map_err(|error| file("rename", &temporary, error))
    }

    /// The whole object, which the database says is `len` bytes.
    pub(crate) fn read(&self, hash: &BlobHash, len: u64) -> Result<Vec<u8>, StoreError> {
        let path = self.path(hash, "");
        let bytes = fs::read(&path).map_err(|error| missing_or_failed(&path, error))?;
        if bytes.len() as u64 != len {
            return Err(wrong_length(&path, bytes.len() as u64, len));
        }
        Ok(bytes)
    }

    /// `count` bytes of the object from `start`, reading only those.
    pub(crate) fn read_range(
        &self,
        hash: &BlobHash,
        len: u64,
        start: u64,
        count: usize,
    ) -> Result<Vec<u8>, StoreError> {
        let path = self.path(hash, "");
        let object = File::open(&path).map_err(|error| missing_or_failed(&path, error))?;
        let actual = object
            .metadata()
            .map_err(|error| file("inspect", &path, error))?
            .len();
        if actual != len {
            return Err(wrong_length(&path, actual, len));
        }
        let mut bytes = vec![0; count];
        object
            .read_exact_at(&mut bytes, start)
            .map_err(|error| file("read", &path, error))?;
        Ok(bytes)
    }

    /// Removes an object's file. A failure leaves an orphan that the next
    /// open collects, so it is not reported.
    pub(crate) fn remove(&self, hash: &BlobHash) {
        let _ = fs::remove_file(self.path(hash, ""));
    }

    /// The number of bytes staged for `hash`.
    pub(crate) fn staged_len(&self, hash: &BlobHash) -> Result<u64, StoreError> {
        let path = self.path(hash, STAGED);
        match fs::metadata(&path) {
            Ok(metadata) => Ok(metadata.len()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
            Err(error) => Err(file("inspect", &path, error)),
        }
    }

    pub(crate) fn staged_range(
        &self,
        hash: &BlobHash,
        offset: u64,
        len: usize,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        let Some(staged) = self.open_staged(hash)? else {
            return Ok(None);
        };
        let offset = offset.min(staged.len);
        let len = u64::try_from(len)
            .unwrap_or(u64::MAX)
            .min(staged.len - offset) as usize;
        let mut bytes = vec![0; len];
        staged
            .file
            .read_exact_at(&mut bytes, offset)
            .map_err(|error| file("read", &staged.path, error))?;
        Ok(Some(bytes))
    }

    /// Appends `bytes` to the staged copy of `hash` if `offset` is its
    /// length, durably, and returns the staged length either way.
    ///
    /// A crash during an append can leave part of that chunk staged. Those
    /// bytes are the ones sent, or else they fail the hash check that
    /// promotion makes, which discards them.
    pub(crate) fn stage(
        &self,
        hash: &BlobHash,
        offset: u64,
        bytes: &[u8],
    ) -> Result<u64, StoreError> {
        let path = self.path(hash, STAGED);
        let (staged, created) = match OpenOptions::new().write(true).open(&path) {
            Ok(staged) => (staged, false),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if offset != 0 || bytes.is_empty() {
                    return Ok(0);
                }
                let staged = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&path)
                    .map_err(|error| file("create", &path, error))?;
                (staged, true)
            }
            Err(error) => return Err(file("open", &path, error)),
        };
        let len = staged
            .metadata()
            .map_err(|error| file("inspect", &path, error))?
            .len();
        if offset != len || bytes.is_empty() {
            return Ok(len);
        }
        staged
            .write_all_at(bytes, len)
            .and_then(|()| staged.sync_data())
            .map_err(|error| file("write", &path, error))?;
        if created {
            self.sync()?;
        }
        Ok(len + bytes.len() as u64)
    }

    /// The staged copy of `hash`, if any bytes were staged.
    pub(crate) fn open_staged(&self, hash: &BlobHash) -> Result<Option<Staged>, StoreError> {
        let path = self.path(hash, STAGED);
        let staged = match File::open(&path) {
            Ok(staged) => staged,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(file("open", &path, error)),
        };
        let len = staged
            .metadata()
            .map_err(|error| file("inspect", &path, error))?
            .len();
        Ok(Some(Staged {
            file: staged,
            path,
            len,
        }))
    }

    /// Removes the staged copy of `hash`, durably.
    pub(crate) fn discard_staged(&self, hash: &BlobHash) -> Result<(), StoreError> {
        let path = self.path(hash, STAGED);
        match fs::remove_file(&path) {
            Ok(()) => self.sync(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(file("remove", &path, error)),
        }
    }

    /// Makes the staged copy of `hash` its object file, durably, without
    /// copying it. Its bytes were synced as they were staged.
    pub(crate) fn promote_staged(&self, hash: &BlobHash) -> Result<(), StoreError> {
        let staged = self.path(hash, STAGED);
        fs::rename(&staged, self.path(hash, "")).map_err(|error| file("rename", &staged, error))?;
        self.sync()
    }

    /// Every file this directory may own, by hash and kind. Names this crate
    /// never writes are left out, so they are never touched.
    pub(crate) fn entries(&self) -> Result<Vec<(BlobHash, Kind)>, StoreError> {
        let listing = fs::read_dir(&self.dir).map_err(|error| file("list", &self.dir, error))?;
        let mut entries = Vec::new();
        for entry in listing {
            let entry = entry.map_err(|error| file("list", &self.dir, error))?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let (hex, kind) = match name.split_once('.') {
                None => (name, Kind::Object),
                Some((hex, "tmp")) => (hex, Kind::Temporary),
                Some((hex, "staged")) => (hex, Kind::Staged),
                Some(_) => continue,
            };
            // Exactly the names `path` writes: lowercase hex of a hash.
            let lowercase_hex = hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
            if hex.len() == 2 * BlobHash::LEN
                && lowercase_hex
                && let Ok(hash) = hex.parse()
            {
                entries.push((hash, kind));
            }
        }
        Ok(entries)
    }

    /// Removes the file of `hash` of this kind; a failure is retried on the
    /// next open.
    pub(crate) fn remove_entry(&self, hash: &BlobHash, kind: Kind) {
        let suffix = match kind {
            Kind::Object => "",
            Kind::Temporary => TEMPORARY,
            Kind::Staged => STAGED,
        };
        let _ = fs::remove_file(self.path(hash, suffix));
    }
}

fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

/// Creates missing ancestors from the first existing directory down. The
/// sync operation is supplied so tests can inject a failed parent sync.
fn create_directories(
    dir: &Path,
    mut sync_parent: impl FnMut(&Path) -> io::Result<()>,
) -> Result<(), StoreError> {
    let mut missing = Vec::new();
    let mut ancestor = dir;
    loop {
        match fs::metadata(ancestor) {
            Ok(metadata) if metadata.is_dir() => {
                // The first existing ancestor may be the directory whose
                // creation succeeded but whose parent sync failed on the
                // previous attempt. Repair it before creating descendants.
                if ancestor.parent().is_some() {
                    let parent = parent_directory(ancestor);
                    sync_parent(parent).map_err(|error| file("sync", parent, error))?;
                }
                break;
            }
            Ok(_) => {
                return Err(file(
                    "create",
                    ancestor,
                    io::Error::from(io::ErrorKind::NotADirectory),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                missing.push(ancestor);
                ancestor = parent_directory(ancestor);
            }
            Err(error) => return Err(file("inspect", ancestor, error)),
        }
    }

    for path in missing.into_iter().rev() {
        match DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => {}
            // Another creator may have won the race. Its directory entry
            // must also be durable before our open succeeds.
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists && path.is_dir() => {}
            Err(error) => return Err(file("create", path, error)),
        }
        let parent = parent_directory(path);
        sync_parent(parent).map_err(|error| file("sync", parent, error))?;
    }
    Ok(())
}

fn parent_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

/// An open staged copy, read once to promote it.
pub(crate) struct Staged {
    file: File,
    path: PathBuf,
    /// Bytes staged.
    pub(crate) len: u64,
}

impl Staged {
    /// All staged bytes; for copies small enough to hold in memory.
    pub(crate) fn read(mut self) -> Result<Vec<u8>, StoreError> {
        let mut bytes = Vec::with_capacity(usize::try_from(self.len).unwrap_or(0));
        io::Read::read_to_end(&mut self.file, &mut bytes)
            .map_err(|error| file("read", &self.path, error))?;
        Ok(bytes)
    }

    /// The content hash of the staged bytes, read in pieces.
    pub(crate) fn hash(self) -> Result<BlobHash, StoreError> {
        content_hash_of(&self.file).map_err(|error| file("read", &self.path, error))
    }
}

/// The plain BLAKE3 of everything `reader` yields, as
/// [`locust_proto::crypto::content_hash`] computes it over one slice, read in
/// 64 KiB pieces so an object is never held in memory whole.
pub(crate) fn content_hash_of(mut reader: impl io::Read) -> io::Result<BlobHash> {
    let mut hasher = ContentHasher::new();
    let mut piece = vec![0u8; 64 * 1024];
    loop {
        match reader.read(&mut piece) {
            Ok(0) => return Ok(hasher.finish()),
            Ok(count) => hasher.update(&piece[..count]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

fn missing_or_failed(path: &Path, error: io::Error) -> StoreError {
    if error.kind() == io::ErrorKind::NotFound {
        corrupted(format_args!(
            "held object file {} is missing",
            path.display()
        ))
    } else {
        file("read", path, error)
    }
}

fn wrong_length(path: &Path, actual: u64, recorded: u64) -> StoreError {
    corrupted(format_args!(
        "object file {} has {actual} bytes, the database records {recorded}",
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_proto::crypto::content_hash;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn newly_created_ancestors_sync_their_parents_in_order() {
        let root = tempfile::tempdir().unwrap();
        let state = root.path().join("ancestor/state");
        let blobs = state.join("blobs");
        let mut synced = Vec::new();
        create_directories(&blobs, |parent| {
            synced.push(parent.to_path_buf());
            sync_directory(parent)
        })
        .unwrap();
        assert_eq!(
            synced,
            [
                parent_directory(root.path()),
                root.path(),
                &root.path().join("ancestor"),
                &state
            ]
        );
        for path in [root.path().join("ancestor"), state.clone(), blobs.clone()] {
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        // Reopening repairs only the existing leaf's parent entry.
        synced.clear();
        create_directories(&blobs, |parent| {
            synced.push(parent.to_path_buf());
            sync_directory(parent)
        })
        .unwrap();
        assert_eq!(synced, [state]);
    }

    #[test]
    fn a_failed_parent_sync_stops_directory_creation_and_is_reported() {
        for fail_at in 0..3 {
            let root = tempfile::tempdir().unwrap();
            let paths = [
                root.path().join("ancestor"),
                root.path().join("ancestor/state"),
                root.path().join("ancestor/state/blobs"),
            ];
            let mut syncs = 0;
            let error = create_directories(&paths[2], |parent| {
                if parent == parent_directory(root.path()) {
                    return sync_directory(parent);
                }
                let index = syncs;
                syncs += 1;
                if index == fail_at {
                    Err(io::Error::other("injected parent sync failure"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            let parent = parent_directory(&paths[fail_at]);
            assert_eq!(
                error,
                StoreError::Failed(format!(
                    "cannot sync {}: injected parent sync failure",
                    parent.display()
                ))
            );
            assert_eq!(syncs, fail_at + 1);
            for (index, path) in paths.iter().enumerate() {
                assert_eq!(path.exists(), index <= fail_at);
            }
            // The retry repairs the closest surviving ancestor, then creates
            // and syncs the remaining descendants. Earlier ancestors need no
            // additional sync because the first attempt stopped at the failure.
            let mut repaired = Vec::new();
            create_directories(&paths[2], |parent| {
                repaired.push(parent.to_path_buf());
                sync_directory(parent)
            })
            .unwrap();
            assert_eq!(
                repaired,
                paths[fail_at..]
                    .iter()
                    .map(|path| parent_directory(path).to_path_buf())
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn an_existing_directory_still_propagates_a_failed_publication_repair() {
        let root = tempfile::tempdir().unwrap();
        let error = create_directories(root.path(), |_| {
            Err(io::Error::other("injected repair failure"))
        })
        .unwrap_err();
        assert_eq!(
            error,
            StoreError::Failed(format!(
                "cannot sync {}: injected repair failure",
                parent_directory(root.path()).display()
            ))
        );
    }

    #[test]
    fn a_file_in_the_directory_hierarchy_is_reported_without_syncing() {
        let root = tempfile::tempdir().unwrap();
        let blocker = root.path().join("state");
        fs::write(&blocker, b"not a directory").unwrap();
        let result = create_directories(&blocker.join("blobs"), |_| {
            panic!("no directory was created")
        });
        assert!(matches!(result, Err(StoreError::Failed(_))));
        assert_eq!(fs::read(blocker).unwrap(), b"not a directory");
    }

    #[test]
    fn hashing_in_pieces_equals_hashing_the_whole_object() {
        for len in [0, 1, 64 * 1024, 64 * 1024 + 1, 3 * 1024 * 1024 + 7] {
            let bytes: Vec<u8> = (0..len).map(|n| (n % 251) as u8).collect();
            assert_eq!(
                content_hash_of(bytes.as_slice()).unwrap(),
                content_hash(&bytes),
                "{len} bytes"
            );
        }
    }
}
