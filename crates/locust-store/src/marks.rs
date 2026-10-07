//! The marks file, `marks` in the marks directory beside the state
//! directory: the last record each key signed in each goal on this daemon.
//!
//! A header of 32 bytes, then records of 112 bytes at fixed slots. Integers
//! are little-endian.
//!
//! | Header bytes | Holds |
//! |---|---|
//! | 0..8 | [`MAGIC`] |
//! | 8..16 | the inode number of this file |
//! | 16 | 1 when the file system reports a creation time, else 0 |
//! | 17..24 | zero |
//! | 24..32 | this file's creation time in milliseconds, else zero |
//!
//! | Record bytes | Holds |
//! |---|---|
//! | 0..32 | goal |
//! | 32..64 | key |
//! | 64..72 | the record's sequence number |
//! | 72..104 | the record's event id |
//! | 104 | 1 for a mark, 0 for a cleared slot (all else zero) |
//! | 105 | 1 when the mark is shared, else 0 |
//! | 106 | 1 when the mark was written while its goal was unheard, else 0 |
//! | 107..108 | zero |
//! | 108..112 | the first four bytes of BLAKE3 over bytes 0..108 |
//!
//! A changed mark is rewritten in its slot, a cleared one is overwritten as
//! a cleared slot, and a new one takes a cleared slot or is appended. Every
//! write of a commit lands before the file is synced once.
//!
//! The header names the file's own identity, read after the file was
//! created, so a copy of the file, or an older one put back into the same
//! directory, does not match its header. That, a header that does not read
//! or a record whose checksum fails makes the marks lost. A lost file is
//! left in place until the next write, which builds the whole replacement
//! under a temporary name, syncs it, renames it over the old one and syncs
//! the directory. Replacing it at open instead would let a crash before the
//! first write turn lost marks into kept, empty ones.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::{FileExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use locust_proto::crypto::content_hash;
use locust_proto::event::AuthorPoint;
use locust_proto::id::{EventId, GoalId, PublicKey};
use locust_proto::local::owner_only;
use locust_proto::store::{FileId, Mark, MarkWrite, StoreError};

use crate::error::file;
use crate::files::{create_directories, sync_directory, sync_file};

const FILE: &str = "marks";
const TEMPORARY: &str = "marks.tmp";
const MAGIC: [u8; 8] = *b"LOCUSTMK";
const HEADER: usize = 32;
const RECORD: usize = 112;
const SUMMED: usize = RECORD - 4;
/// Mode of the marks directory and of the marks file: the marks name goals,
/// keys and counts, so only their owner may read them.
const DIRECTORY_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;

/// The open marks file of one marks directory. Owned by the store.
#[derive(Debug)]
pub(crate) struct MarksFile {
    dir: PathBuf,
    /// `None` while the marks are lost or the file is missing: the next
    /// write replaces it.
    file: Option<File>,
    slots: BTreeMap<(GoalId, PublicKey), u64>,
    cleared: Vec<u64>,
    len: u64,
}

/// Uses the marks directory `dir`, creating it owner-only if missing, and
/// reads its marks: ascending by goal and key, or `None` when they are lost.
/// A directory or file that others may read is refused, as the state
/// directory is.
pub(crate) fn open(dir: &Path) -> Result<(MarksFile, Option<Vec<Mark>>), StoreError> {
    create_directories(dir, sync_directory)?;
    let metadata = fs::metadata(dir).map_err(|error| file("inspect", dir, error))?;
    private(
        dir,
        "directory",
        metadata.permissions().mode(),
        DIRECTORY_MODE,
    )?;
    let mut marks = MarksFile {
        dir: dir.to_owned(),
        file: None,
        slots: BTreeMap::new(),
        cleared: Vec::new(),
        len: 0,
    };
    let path = dir.join(FILE);
    let mut found = match OpenOptions::new().read(true).write(true).open(&path) {
        Ok(found) => found,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((marks, None)),
        Err(error) => return Err(file("open", &path, error)),
    };
    let metadata = found
        .metadata()
        .map_err(|error| file("inspect", &path, error))?;
    private(&path, "file", metadata.permissions().mode(), FILE_MODE)?;
    let mut bytes = Vec::new();
    found
        .read_to_end(&mut bytes)
        .map_err(|error| file("read", &path, error))?;
    let id = FileId::of(&path).map_err(|error| file("inspect", &path, error))?;
    let Some(slots) = decode(&bytes, id) else {
        return Ok((marks, None));
    };
    let (mut kept, mut held, mut cleared) = (BTreeMap::new(), BTreeMap::new(), Vec::new());
    for (slot, mark) in (0..).zip(&slots) {
        match mark {
            Some(mark) => {
                if kept.insert((mark.goal, mark.key), *mark).is_some() {
                    return Ok((marks, None));
                }
                held.insert((mark.goal, mark.key), slot);
            }
            None => cleared.push(slot),
        }
    }
    marks.slots = held;
    marks.cleared = cleared;
    marks.len = slots.len() as u64;
    marks.file = Some(found);
    Ok((marks, Some(kept.into_values().collect())))
}

impl MarksFile {
    /// Applies `writes` in order and syncs the file once. No I/O when there
    /// are none. A failure leaves the file's contents unknown.
    pub(crate) fn write(&mut self, writes: &[MarkWrite]) -> Result<(), StoreError> {
        if writes.is_empty() {
            return Ok(());
        }
        let replacing = self.file.is_none();
        let target = self.dir.join(if replacing { TEMPORARY } else { FILE });
        let out = match &mut self.file {
            Some(out) => out,
            missing @ None => missing.insert(create(&target)?),
        };
        for write in writes {
            let (slot, record) = match write {
                MarkWrite::Set(mark) => {
                    let slot = *self.slots.entry((mark.goal, mark.key)).or_insert_with(|| {
                        self.cleared.pop().unwrap_or_else(|| {
                            self.len += 1;
                            self.len - 1
                        })
                    });
                    (slot, encode(Some(mark)))
                }
                MarkWrite::Clear { goal, key } => {
                    let Some(slot) = self.slots.remove(&(*goal, *key)) else {
                        continue;
                    };
                    self.cleared.push(slot);
                    (slot, encode(None))
                }
            };
            out.write_all_at(&record, offset(slot))
                .map_err(|error| file("write", &target, error))?;
        }
        sync_file(out, &target).map_err(|error| file("sync", &target, error))?;
        if replacing {
            let path = self.dir.join(FILE);
            fs::rename(&target, &path).map_err(|error| file("rename", &target, error))?;
            sync_directory(&self.dir).map_err(|error| file("sync", &self.dir, error))?;
        }
        Ok(())
    }
}

/// Refuses a marks directory or file whose `mode` lets anyone but its owner
/// in, by the rule the state directory follows.
fn private(path: &Path, what: &str, mode: u32, wanted: u32) -> Result<(), StoreError> {
    owner_only(&format!("marks {what}"), path, mode, wanted).map_err(StoreError::Failed)
}

/// A new, empty marks file at `path` whose header names its own identity.
/// A temporary file left by an earlier attempt is reused at the file's own
/// mode, whatever mode it was left with.
fn create(path: &Path) -> Result<File, StoreError> {
    let out = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .mode(FILE_MODE)
        .open(path)
        .map_err(|error| file("create", path, error))?;
    out.set_permissions(fs::Permissions::from_mode(FILE_MODE))
        .map_err(|error| file("protect", path, error))?;
    let id = FileId::of(path).map_err(|error| file("inspect", path, error))?;
    let mut header = [0; HEADER];
    header[..8].copy_from_slice(&MAGIC);
    header[8..16].copy_from_slice(&id.ino.to_le_bytes());
    if let Some(created) = id.created_ms {
        header[16] = 1;
        header[24..32].copy_from_slice(&created.to_le_bytes());
    }
    out.write_all_at(&header, 0)
        .map_err(|error| file("write", path, error))?;
    Ok(out)
}

fn offset(slot: u64) -> u64 {
    HEADER as u64 + slot * RECORD as u64
}

fn checksum(record: &[u8]) -> [u8; 4] {
    let hash = content_hash(&record[..SUMMED]);
    [hash.0[0], hash.0[1], hash.0[2], hash.0[3]]
}

/// A mark's record, or a cleared slot's for `None`.
fn encode(mark: Option<&Mark>) -> [u8; RECORD] {
    let mut record = [0; RECORD];
    if let Some(mark) = mark {
        record[..32].copy_from_slice(&mark.goal.0);
        record[32..64].copy_from_slice(&mark.key.0);
        record[64..72].copy_from_slice(&mark.point.seq.to_le_bytes());
        record[72..104].copy_from_slice(&mark.point.id.0);
        record[104] = 1;
        record[105] = u8::from(mark.shared);
        record[106] = u8::from(mark.unheard);
    }
    let sum = checksum(&record);
    record[SUMMED..].copy_from_slice(&sum);
    record
}

/// Every slot of a file whose identity is `id`, `None` for a cleared one;
/// `None` when anything in it does not read.
fn decode(bytes: &[u8], id: FileId) -> Option<Vec<Option<Mark>>> {
    let (header, records) = bytes.split_at_checked(HEADER)?;
    let created = match header[16] {
        0 => None,
        1 => Some(u64::from_le_bytes(header[24..32].try_into().ok()?)),
        _ => return None,
    };
    let named = FileId {
        ino: u64::from_le_bytes(header[8..16].try_into().ok()?),
        created_ms: created,
    };
    if header[..8] != MAGIC || header[17..24] != [0; 7] || !named.same(&id) {
        return None;
    }
    if created.is_none() && header[24..32] != [0; 8] {
        return None;
    }
    if !records.len().is_multiple_of(RECORD) {
        return None;
    }
    records.chunks_exact(RECORD).map(decode_record).collect()
}

/// `Some(None)` for a cleared slot.
fn decode_record(record: &[u8]) -> Option<Option<Mark>> {
    if record[SUMMED..] != checksum(record) || record[107..SUMMED] != [0; 1] {
        return None;
    }
    let bit = |byte: u8| match byte {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    };
    let (Some(shared), Some(unheard)) = (bit(record[105]), bit(record[106])) else {
        return None;
    };
    match record[104] {
        0 if record[..SUMMED].iter().all(|byte| *byte == 0) => Some(None),
        1 => Some(Some(Mark {
            goal: GoalId(record[..32].try_into().ok()?),
            key: PublicKey(record[32..64].try_into().ok()?),
            point: AuthorPoint {
                seq: u64::from_le_bytes(record[64..72].try_into().ok()?),
                id: EventId(record[72..104].try_into().ok()?),
            },
            shared,
            unheard,
        })),
        _ => None,
    }
}
