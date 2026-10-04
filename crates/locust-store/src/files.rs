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
    /// missing.
    pub(crate) fn create(dir: PathBuf) -> Result<Self, StoreError> {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .map_err(|error| file("create", &dir, error))?;
        Ok(Self { dir })
    }

    fn path(&self, hash: &BlobHash, suffix: &str) -> PathBuf {
        self.dir.join(format!("{hash}{suffix}"))
    }

    /// Makes the directory's entries (created, renamed or removed files)
    /// durable.
    pub(crate) fn sync(&self) -> Result<(), StoreError> {
        File::open(&self.dir)
            .and_then(|dir| dir.sync_all())
            .map_err(|error| file("sync", &self.dir, error))
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
