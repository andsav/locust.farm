//! Private, unsigned and unsynced chat observations. Directory-relative opens
//! prevent a replaced marks path from redirecting reads or writes.
use std::collections::BTreeSet;
use std::fs::{File, TryLockError};
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::{Duration, Instant};

use locust_adapter::hooks::core::{ClaimKey, Marks};
use locust_proto::api::{Credential, SessionSecret};
use locust_proto::crypto;
use rustix::fs::{Mode, OFlags};

use crate::failure::Failure;

pub(super) struct Directory(File);
pub(super) struct Chat {
    directory: File,
    name: String,
    _lock: File,
    pub marks: Marks,
    /// What the file holds, so an unchanged chat is never rewritten.
    saved: Marks,
}

/// The session's record of claims its chats ended themselves, newest last.
const RELEASED: &str = "released.json";
/// Enough for every claim a session's chats can hold between two callbacks.
const MAX_RELEASED: usize = 256;

impl Directory {
    pub fn open(
        home: &Path,
        credential: Credential,
        session: SessionSecret,
    ) -> Result<Self, Failure> {
        let mut directory = File::from(
            rustix::fs::open(
                home,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(storage)?,
        );
        check_directory(&directory)?;
        for name in [
            "hook-marks".to_owned(),
            locust_proto::id::BlobHash(credential.digest()).to_string(),
            session.instance().to_string(),
        ] {
            match rustix::fs::mkdirat(&directory, &name, Mode::from_raw_mode(0o700)) {
                Ok(()) => directory.sync_all().map_err(storage)?,
                Err(rustix::io::Errno::EXIST) => (),
                Err(error) => return Err(storage(error)),
            }
            directory = File::from(
                rustix::fs::openat(
                    &directory,
                    &name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(storage)?,
            );
            check_directory(&directory)?;
        }
        Ok(Self(directory))
    }

    pub fn chat(&self, identity: &[u8], deadline: Instant) -> Result<Chat, Failure> {
        let name = crypto::content_hash(identity).to_string();
        let lock = locked(&self.0, &format!("{name}.lock"), deadline)?;
        let marks: Marks = read(&self.0, &format!("{name}.json"))?.unwrap_or_default();
        Ok(Chat {
            directory: self.0.try_clone().map_err(storage)?,
            name,
            _lock: lock,
            saved: marks.clone(),
            marks,
        })
    }

    /// Claims any chat of this session ended by its own terminal write.
    pub fn released(&self) -> Result<BTreeSet<ClaimKey>, Failure> {
        Ok(read::<Vec<ClaimKey>>(&self.0, RELEASED)?
            .unwrap_or_default()
            .into_iter()
            .collect())
    }

    /// Add this chat's own releases to the session record, keeping the newest.
    pub fn share(&self, released: &BTreeSet<ClaimKey>, deadline: Instant) -> Result<(), Failure> {
        let _lock = locked(&self.0, "released.lock", deadline)?;
        let mut record: Vec<ClaimKey> = read(&self.0, RELEASED)?.unwrap_or_default();
        let before = record.clone();
        record.retain(|key| !released.contains(key));
        record.extend(released);
        let excess = record.len().saturating_sub(MAX_RELEASED);
        record.drain(..excess);
        if record == before {
            return Ok(());
        }
        replace(
            &self.0,
            RELEASED,
            &serde_json::to_vec(&record).map_err(storage)?,
        )
    }

    /// Exactly one waiting worker per execution session, across all its chats.
    pub fn try_wait(&self) -> Result<Option<File>, Failure> {
        let lock = private_file(&self.0, "waiting.lock", true)?;
        match lock.try_lock() {
            Ok(()) => Ok(Some(lock)),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(storage(error)),
        }
    }
}

impl Chat {
    /// Write the marks only when they changed since they were read or saved.
    pub fn save(&mut self) -> Result<(), Failure> {
        if self.marks == self.saved {
            return Ok(());
        }
        replace(
            &self.directory,
            &format!("{}.json", self.name),
            &serde_json::to_vec(&self.marks).map_err(storage)?,
        )?;
        self.saved = self.marks.clone();
        Ok(())
    }
}

fn locked(directory: &File, name: &str, deadline: Instant) -> Result<File, Failure> {
    let lock = private_file(directory, name, true)?;
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(lock),
            Err(TryLockError::WouldBlock) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(Failure::unavailable(
                        "hook time limit reached while locking hook marks",
                    ));
                }
                std::thread::sleep(remaining.min(Duration::from_millis(10)));
            }
            Err(TryLockError::Error(error)) => return Err(storage(error)),
        }
    }
}

fn read<T: serde::de::DeserializeOwned>(
    directory: &File,
    name: &str,
) -> Result<Option<T>, Failure> {
    match private_file(directory, name, false) {
        Ok(mut file) => {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).map_err(storage)?;
            Ok(Some(serde_json::from_slice(&bytes).map_err(storage)?))
        }
        Err(error) if error.code == locust_proto::api::ErrorCode::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Replace one private file through a synced temporary file and rename.
fn replace(directory: &File, name: &str, bytes: &[u8]) -> Result<(), Failure> {
    let mut random = [0; 16];
    getrandom::fill(&mut random).map_err(storage)?;
    let temporary = format!(".{}.tmp", crypto::content_hash(&random));
    let result = (|| {
        let mut file = File::from(
            rustix::fs::openat(
                directory,
                &temporary,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )
            .map_err(storage)?,
        );
        file.write_all(bytes).map_err(storage)?;
        file.sync_all().map_err(storage)?;
        rustix::fs::renameat(directory, &temporary, directory, name).map_err(storage)?;
        directory.sync_all().map_err(storage)
    })();
    if result.is_err() {
        let _ = rustix::fs::unlinkat(directory, &temporary, rustix::fs::AtFlags::empty());
    }
    result
}

fn private_file(directory: &File, name: &str, create: bool) -> Result<File, Failure> {
    let flags = OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
    let opened = if create {
        match rustix::fs::openat(
            directory,
            name,
            flags | OFlags::CREATE | OFlags::EXCL,
            Mode::from_raw_mode(0o600),
        ) {
            Err(rustix::io::Errno::EXIST) => {
                rustix::fs::openat(directory, name, flags, Mode::empty())
            }
            result => result,
        }
    } else {
        rustix::fs::openat(directory, name, flags, Mode::empty())
    };
    let file = File::from(opened.map_err(|error| {
        if error == rustix::io::Errno::NOENT {
            Failure::new(locust_proto::api::ErrorCode::NotFound, "no hook marks")
        } else {
            storage(error)
        }
    })?);
    let metadata = file.metadata().map_err(storage)?;
    if !metadata.is_file()
        || metadata.uid() != rustix::process::getuid().as_raw()
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(Failure::invalid(
            "hook marks must be private owned regular files",
        ));
    }
    Ok(file)
}

fn check_directory(file: &File) -> Result<(), Failure> {
    let metadata = file.metadata().map_err(storage)?;
    if !metadata.is_dir()
        || metadata.uid() != rustix::process::getuid().as_raw()
        || metadata.mode() & 0o7777 != 0o700
    {
        return Err(Failure::invalid(
            "hook marks need private owned directories",
        ));
    }
    Ok(())
}
fn storage(error: impl std::fmt::Display) -> Failure {
    Failure::internal(format!("hook marks: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(5)
    }

    fn home() -> tempfile::TempDir {
        let directory = tempfile::Builder::new()
            .prefix("lh.")
            .tempdir_in("/tmp")
            .unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        directory
    }

    #[test]
    fn private_marks_persist_per_chat_and_one_waiter_per_session() {
        let home = home();
        let directory =
            Directory::open(home.path(), Credential([1; 32]), SessionSecret([2; 32])).unwrap();
        let mut chat = directory.chat(b"first", deadline()).unwrap();
        chat.marks.used_locust = true;
        chat.save().unwrap();
        drop(chat);
        assert!(
            directory
                .chat(b"first", deadline())
                .unwrap()
                .marks
                .used_locust
        );
        assert!(
            !directory
                .chat(b"second", deadline())
                .unwrap()
                .marks
                .used_locust
        );
        let wait = directory.try_wait().unwrap().unwrap();
        assert!(directory.try_wait().unwrap().is_none());
        drop(wait);
        assert!(directory.try_wait().unwrap().is_some());
        let other =
            Directory::open(home.path(), Credential([1; 32]), SessionSecret([3; 32])).unwrap();
        assert!(!other.chat(b"first", deadline()).unwrap().marks.used_locust);
    }

    #[test]
    fn linked_and_nonprivate_marks_are_refused_without_touching_target() {
        let home = home();
        let outside = home.path().join("outside");
        std::fs::write(&outside, "untouched").unwrap();
        symlink(&outside, home.path().join("hook-marks")).unwrap();
        assert!(Directory::open(home.path(), Credential([1; 32]), SessionSecret([2; 32])).is_err());
        assert_eq!(std::fs::read_to_string(outside).unwrap(), "untouched");
        std::fs::remove_file(home.path().join("hook-marks")).unwrap();
        let directory =
            Directory::open(home.path(), Credential([1; 32]), SessionSecret([2; 32])).unwrap();
        let mut chat = directory.chat(b"chat", deadline()).unwrap();
        chat.marks.used_locust = true;
        chat.save().unwrap();
        let entry = format!("{}.json", crypto::content_hash(b"chat"));
        let file = private_file(&directory.0, &entry, false).unwrap();
        file.set_permissions(std::fs::Permissions::from_mode(0o644))
            .unwrap();
        drop(chat);
        assert!(directory.chat(b"chat", deadline()).is_err());
    }

    #[test]
    fn concurrent_chat_updates_do_not_lose_marks() {
        let home = home();
        std::thread::scope(|scope| {
            for index in 0..8 {
                let home = home.path();
                scope.spawn(move || {
                    let directory =
                        Directory::open(home, Credential([1; 32]), SessionSecret([2; 32])).unwrap();
                    let mut chat = directory.chat(b"same", deadline()).unwrap();
                    chat.marks.invocations.push_back(index.to_string());
                    chat.save().unwrap();
                });
            }
        });
        let directory =
            Directory::open(home.path(), Credential([1; 32]), SessionSecret([2; 32])).unwrap();
        assert_eq!(
            directory
                .chat(b"same", deadline())
                .unwrap()
                .marks
                .invocations
                .len(),
            8
        );
    }

    #[test]
    fn unchanged_marks_are_never_rewritten() {
        let home = home();
        let directory =
            Directory::open(home.path(), Credential([1; 32]), SessionSecret([2; 32])).unwrap();
        let mut chat = directory.chat(b"steady", deadline()).unwrap();
        chat.save().unwrap();
        let entry = format!("{}.json", crypto::content_hash(b"steady"));
        assert!(matches!(
            private_file(&directory.0, &entry, false),
            Err(Failure {
                code: locust_proto::api::ErrorCode::NotFound,
                ..
            })
        ));
        chat.marks.worker = true;
        chat.save().unwrap();
        let first = private_file(&directory.0, &entry, false)
            .unwrap()
            .metadata()
            .unwrap()
            .ino();
        chat.save().unwrap();
        drop(chat);
        let mut chat = directory.chat(b"steady", deadline()).unwrap();
        chat.save().unwrap();
        let second = private_file(&directory.0, &entry, false)
            .unwrap()
            .metadata()
            .unwrap()
            .ino();
        // A rewrite renames a fresh file into place; an unchanged inode shows none.
        assert_eq!(first, second);
    }

    #[test]
    fn the_session_release_record_merges_chats_and_keeps_only_the_newest() {
        let home = home();
        let directory =
            Directory::open(home.path(), Credential([1; 32]), SessionSecret([2; 32])).unwrap();
        assert!(directory.released().unwrap().is_empty());
        let key = |n: u32| ClaimKey {
            goal: locust_proto::id::GoalId([1; 32]),
            attempt: locust_proto::id::EventId([2; 32]),
            generation: n,
        };
        directory
            .share(&BTreeSet::from([key(1)]), deadline())
            .unwrap();
        directory
            .share(&BTreeSet::from([key(2)]), deadline())
            .unwrap();
        assert_eq!(
            directory.released().unwrap(),
            BTreeSet::from([key(1), key(2)])
        );
        for n in 3..(MAX_RELEASED as u32 + 10) {
            directory
                .share(&BTreeSet::from([key(n)]), deadline())
                .unwrap();
        }
        let record = directory.released().unwrap();
        assert_eq!(record.len(), MAX_RELEASED);
        assert!(!record.contains(&key(1)));
        assert!(record.contains(&key(MAX_RELEASED as u32 + 9)));
    }

    #[test]
    fn contended_chat_lock_respects_the_invocation_deadline() {
        let home = home();
        let directory =
            Directory::open(home.path(), Credential([1; 32]), SessionSecret([2; 32])).unwrap();
        let _held = directory.chat(b"same", deadline()).unwrap();
        let result = directory.chat(b"same", Instant::now() + Duration::from_millis(20));
        assert!(matches!(
            result,
            Err(Failure {
                code: locust_proto::api::ErrorCode::Unavailable,
                ..
            })
        ));
    }
}
