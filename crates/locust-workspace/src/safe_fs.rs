//! Descriptor-relative, no-follow reads and mutations. A received path never
//! reaches a shell or follows a symlink. Root selection is a local caller's
//! authority; every component beneath that root is opened separately.

#[cfg(test)]
use std::cell::Cell;
use std::fs::{File, Metadata, Permissions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use locust_proto::id::BlobHash;
use locust_proto::seal::MAX_PLAINTEXT_BYTES;
use rustix::fs::{self, AtFlags, Dir, Mode, OFlags};

use crate::WorkspaceError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Identity {
    pub dev: u64,
    pub ino: u64,
    pub size: u64,
    pub modified: (i64, i64),
    pub changed: (i64, i64),
    pub mode: u32,
}
impl From<&Metadata> for Identity {
    fn from(m: &Metadata) -> Self {
        Self {
            dev: m.dev(),
            ino: m.ino(),
            size: m.size(),
            modified: (m.mtime(), m.mtime_nsec()),
            changed: (m.ctime(), m.ctime_nsec()),
            mode: m.mode(),
        }
    }
}
impl From<&rustix::fs::Stat> for Identity {
    // Stat field widths and signedness differ between macOS and Linux.
    #[allow(clippy::unnecessary_cast)]
    fn from(stat: &rustix::fs::Stat) -> Self {
        Self {
            dev: stat.st_dev as u64,
            ino: stat.st_ino,
            size: stat.st_size as u64,
            modified: (stat.st_mtime as i64, stat.st_mtime_nsec as i64),
            changed: (stat.st_ctime as i64, stat.st_ctime_nsec as i64),
            mode: stat.st_mode as u32,
        }
    }
}

pub(crate) struct LocalFile {
    pub bytes: Vec<u8>,
    pub executable: bool,
    pub identity: Identity,
}

/// A locally observed file: identity and content digest without the full
/// bytes, for large preserved files that must not be loaded whole.
pub(crate) struct ObservedFile {
    pub identity: Identity,
    pub digest: BlobHash,
    pub executable: bool,
    pub size: u64,
}

pub(crate) fn unsupported(path: &str, reason: &str) -> WorkspaceError {
    WorkspaceError::Unsupported {
        path: path.into(),
        reason: reason.into(),
    }
}

pub(crate) fn root(path: &Path) -> Result<File, WorkspaceError> {
    Ok(File::from(fs::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?))
}

#[cfg(test)]
thread_local! {
    /// Test-only counter for `names` calls, used to verify that directory
    /// inventories reuse a single listing rather than re-listing per child.
    /// Thread-local so parallel tests do not interfere.
    pub(crate) static NAMES_CALLS: Cell<u64> = const { Cell::new(0) };
    /// Test-only counter for durable `write_new` syncs, used to verify that
    /// disposable layout probes take the non-durable path. Thread-local so
    /// parallel tests do not interfere.
    pub(crate) static SYNC_CALLS: Cell<u64> = const { Cell::new(0) };
}

pub(crate) fn names(dir: &File) -> Result<Vec<String>, WorkspaceError> {
    #[cfg(test)]
    NAMES_CALLS.with(|c| c.set(c.get() + 1));
    let mut result = Vec::new();
    for entry in Dir::read_from(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        result.push(
            std::str::from_utf8(name)
                .map_err(|_| unsupported("directory", "non-UTF-8 entry"))?
                .to_owned(),
        );
    }
    result.sort_unstable();
    Ok(result)
}

fn check_listed(listed: &[String], name: &str) -> Result<(), WorkspaceError> {
    if listed
        .binary_search_by(|entry| entry.as_str().cmp(name))
        .is_err()
    {
        return Err(WorkspaceError::Conflict {
            path: name.into(),
            reason: "filesystem case or normalization alias".into(),
        });
    }
    Ok(())
}

fn exact_name(dir: &File, name: &str) -> Result<(), WorkspaceError> {
    check_listed(&names(dir)?, name)
}

pub(crate) fn dir_at(parent: &File, name: &str) -> Result<File, WorkspaceError> {
    let file = File::from(
        fs::openat(
            parent,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            if error == rustix::io::Errno::LOOP || error == rustix::io::Errno::NOTDIR {
                unsupported(name, "symlink or non-directory ancestor")
            } else {
                error.into()
            }
        })?,
    );
    exact_name(parent, name)?;
    Ok(file)
}

/// Like `dir_at` but reuses a directory listing already obtained with
/// `names`, avoiding a full re-list per child.
pub(crate) fn dir_at_listed(
    parent: &File,
    name: &str,
    listed: &[String],
) -> Result<File, WorkspaceError> {
    let file = File::from(
        fs::openat(
            parent,
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| {
            if error == rustix::io::Errno::LOOP || error == rustix::io::Errno::NOTDIR {
                unsupported(name, "symlink or non-directory ancestor")
            } else {
                error.into()
            }
        })?,
    );
    check_listed(listed, name)?;
    Ok(file)
}

pub(crate) fn parent(
    root: &File,
    path: &str,
    create: bool,
) -> Result<(File, String), WorkspaceError> {
    let mut dir = root.try_clone()?;
    let mut parts = path.split('/').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            return Ok((dir, part.to_owned()));
        }
        if create {
            match fs::mkdirat(&dir, part, Mode::from_raw_mode(0o755)) {
                Ok(()) => {
                    dir.sync_all()?;
                }
                Err(e) if e == rustix::io::Errno::EXIST => {}
                Err(e) => return Err(e.into()),
            }
        }
        dir = dir_at(&dir, part)?;
        match fs::statat(&dir, ".git", AtFlags::SYMLINK_NOFOLLOW) {
            Ok(_) => {
                return Err(unsupported(
                    path,
                    "nested Git repository or submodule ancestor",
                ));
            }
            Err(error) if error == rustix::io::Errno::NOENT => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(unsupported(path, "empty path"))
}

pub(crate) fn read(root: &File, path: &str) -> Result<Option<LocalFile>, WorkspaceError> {
    match parent(root, path, false) {
        Ok((dir, name)) => read_at(&dir, &name, path),
        Err(WorkspaceError::Io(e)) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub(crate) fn read_at(
    parent: &File,
    name: &str,
    label: &str,
) -> Result<Option<LocalFile>, WorkspaceError> {
    read_at_listed(parent, name, label, &names(parent)?)
}

/// Like `read_at` but reuses a directory listing already obtained with
/// `names`, avoiding a full re-list per child.
pub(crate) fn read_at_listed(
    parent: &File,
    name: &str,
    label: &str,
    listed: &[String],
) -> Result<Option<LocalFile>, WorkspaceError> {
    let fd = match fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(e) if e == rustix::io::Errno::NOENT => return Ok(None),
        Err(e) if e == rustix::io::Errno::LOOP => return Err(unsupported(label, "symbolic link")),
        Err(e) => return Err(e.into()),
    };
    check_listed(listed, name)?;
    let mut file = File::from(fd);
    let before = file.metadata()?;
    if !before.is_file() {
        return Err(unsupported(
            label,
            "directory, submodule or special file; select regular files explicitly",
        ));
    }
    if before.nlink() != 1 {
        return Err(unsupported(label, "hardlinked file"));
    }
    if before.len() > MAX_PLAINTEXT_BYTES as u64 {
        return Err(unsupported(label, "file exceeds the content-object limit"));
    }
    let identity = Identity::from(&before);
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_PLAINTEXT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let current = fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?;
    if bytes.len() as u64 != before.len()
        || Identity::from(&file.metadata()?) != identity
        || current.st_ino != before.ino()
        || current.st_dev as u64 != before.dev()
    {
        return Err(WorkspaceError::Conflict {
            path: label.into(),
            reason: "file changed while it was read".into(),
        });
    }
    Ok(Some(LocalFile {
        bytes,
        executable: before.mode() & 0o111 != 0,
        identity,
    }))
}

/// Reads a file's identity and content digest without loading its full bytes,
/// streaming through a fixed buffer. No content-object limit is imposed: this
/// is a local observation of a preserved file, not a publishable object.
/// Reuses a sorted directory listing obtained with `names`.
pub(crate) fn read_at_digest_listed(
    parent: &File,
    name: &str,
    label: &str,
    listed: &[String],
) -> Result<Option<ObservedFile>, WorkspaceError> {
    let fd = match fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(e) if e == rustix::io::Errno::NOENT => return Ok(None),
        Err(e) if e == rustix::io::Errno::LOOP => return Err(unsupported(label, "symbolic link")),
        Err(e) => return Err(e.into()),
    };
    check_listed(listed, name)?;
    let mut file = File::from(fd);
    let before = file.metadata()?;
    if !before.is_file() {
        return Err(unsupported(
            label,
            "directory, submodule or special file; select regular files explicitly",
        ));
    }
    if before.nlink() != 1 {
        return Err(unsupported(label, "hardlinked file"));
    }
    let identity = Identity::from(&before);
    let size = before.len();
    let executable = before.mode() & 0o111 != 0;
    let mut hasher = locust_proto::crypto::ContentHasher::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finish();
    let current = fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?;
    if Identity::from(&file.metadata()?) != identity
        || current.st_ino != before.ino()
        || current.st_dev as u64 != before.dev()
    {
        return Err(WorkspaceError::Conflict {
            path: label.into(),
            reason: "file changed while it was read".into(),
        });
    }
    Ok(Some(ObservedFile {
        identity,
        digest,
        executable,
        size,
    }))
}

pub(crate) fn write_new(
    dir: &File,
    name: &str,
    bytes: &[u8],
    executable: bool,
) -> Result<(), WorkspaceError> {
    let file = create_exclusive(dir, name, bytes, executable)?;
    file.sync_all()?;
    dir.sync_all()?;
    #[cfg(test)]
    SYNC_CALLS.with(|c| c.set(c.get() + 1));
    Ok(())
}

/// Exclusive, no-follow creation without flushing, for disposable layout probes.
/// Recovery data and actual replacements must use [`write_new`].
pub(crate) fn probe_write_new(
    dir: &File,
    name: &str,
    bytes: &[u8],
    executable: bool,
) -> Result<(), WorkspaceError> {
    let _file = create_exclusive(dir, name, bytes, executable)?;
    Ok(())
}

fn create_exclusive(
    dir: &File,
    name: &str,
    bytes: &[u8],
    executable: bool,
) -> Result<File, WorkspaceError> {
    let fd = fs::openat(
        dir,
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )?;
    let mut file = File::from(fd);
    file.write_all(bytes)?;
    file.set_permissions(Permissions::from_mode(if executable {
        0o755
    } else {
        0o644
    }))?;
    Ok(file)
}

pub(crate) fn rename_new(
    from: &File,
    old: &str,
    to: &File,
    new: &str,
) -> Result<(), WorkspaceError> {
    fs::renameat_with(from, old, to, new, fs::RenameFlags::NOREPLACE)?;
    from.sync_all()?;
    to.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `write_new` is the durable path: each call performs a hardware flush and
    /// increments the test-only sync counter.
    #[test]
    fn write_new_is_durable_and_counts_syncs() {
        let temp = tempfile::tempdir().unwrap();
        let dir = root(temp.path()).unwrap();
        SYNC_CALLS.with(|c| c.set(0));
        write_new(&dir, "durable", b"bytes", false).unwrap();
        assert_eq!(SYNC_CALLS.with(|c| c.get()), 1, "write_new must sync once");
        assert_eq!(
            std::fs::read(temp.path().join("durable")).unwrap(),
            b"bytes"
        );
    }

    /// `probe_write_new` shares exclusivity, privacy and no-follow guarantees
    /// with `write_new` but skips the hardware flush, so the sync counter is
    /// unchanged. This is the disposable path layout probes use.
    #[test]
    fn probe_write_new_skips_durable_sync() {
        let temp = tempfile::tempdir().unwrap();
        let dir = root(temp.path()).unwrap();
        SYNC_CALLS.with(|c| c.set(0));
        probe_write_new(&dir, "probe", b"bytes", true).unwrap();
        assert_eq!(
            SYNC_CALLS.with(|c| c.get()),
            0,
            "probe writes must not sync"
        );
        let meta = std::fs::metadata(temp.path().join("probe")).unwrap();
        assert!(meta.is_file());
        assert_eq!(
            meta.permissions().mode() & 0o111,
            0o111,
            "executable bit preserved"
        );
    }

    /// Collision validation is preserved on the non-durable path: a second
    /// exclusive create of the same name fails with `AlreadyExists`, the same
    /// error `probe_layout` maps to a layout conflict.
    #[test]
    fn probe_write_new_reports_collisions_like_write_new() {
        let temp = tempfile::tempdir().unwrap();
        let dir = root(temp.path()).unwrap();
        probe_write_new(&dir, "shared", b"", false).unwrap();
        let probe_err = probe_write_new(&dir, "shared", b"", false).unwrap_err();
        assert!(
            matches!(probe_err, WorkspaceError::Io(ref io) if io.kind() == std::io::ErrorKind::AlreadyExists),
            "probe collision must surface as AlreadyExists, got {probe_err:?}"
        );
        write_new(&dir, "shared2", b"", false).unwrap();
        let durable_err = write_new(&dir, "shared2", b"", false).unwrap_err();
        assert!(
            matches!(durable_err, WorkspaceError::Io(ref io) if io.kind() == std::io::ErrorKind::AlreadyExists)
        );
    }
}
