//! Private, unsigned and unsynced chat observations. Directory-relative opens
//! prevent a replaced marks path from redirecting reads or writes.
use std::fs::{File, TryLockError};
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::{Duration, Instant};

use locust_adapter::hooks::core::Marks;
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
}

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
        let lock = private_file(&self.0, &format!("{name}.lock"), true)?;
        loop {
            match lock.try_lock() {
                Ok(()) => break,
                Err(TryLockError::WouldBlock) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err(Failure::unavailable(
                            "hook time limit reached while locking chat marks",
                        ));
                    }
                    std::thread::sleep(remaining.min(Duration::from_millis(10)));
                }
                Err(TryLockError::Error(error)) => return Err(storage(error)),
            }
        }
        let marks = match private_file(&self.0, &format!("{name}.json"), false) {
            Ok(mut file) => {
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).map_err(storage)?;
                serde_json::from_slice(&bytes).map_err(storage)?
            }
            Err(error) if error.code == locust_proto::api::ErrorCode::NotFound => Marks::default(),
            Err(error) => return Err(error),
        };
        Ok(Chat {
            directory: self.0.try_clone().map_err(storage)?,
            name,
            _lock: lock,
            marks,
        })
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
    pub fn save(&self) -> Result<(), Failure> {
        let mut random = [0; 16];
        getrandom::fill(&mut random).map_err(storage)?;
        let name = format!(".{}.tmp", crypto::content_hash(&random));
        let result = (|| {
            let mut file = File::from(
                rustix::fs::openat(
                    &self.directory,
                    &name,
                    OFlags::WRONLY
                        | OFlags::CREATE
                        | OFlags::EXCL
                        | OFlags::NOFOLLOW
                        | OFlags::CLOEXEC,
                    Mode::from_raw_mode(0o600),
                )
                .map_err(storage)?,
            );
            let bytes = serde_json::to_vec(&self.marks).map_err(storage)?;
            file.write_all(&bytes).map_err(storage)?;
            file.sync_all().map_err(storage)?;
            rustix::fs::renameat(
                &self.directory,
                &name,
                &self.directory,
                format!("{}.json", self.name),
            )
            .map_err(storage)?;
            self.directory.sync_all().map_err(storage)
        })();
        if result.is_err() {
            let _ = rustix::fs::unlinkat(&self.directory, &name, rustix::fs::AtFlags::empty());
        }
        result
    }
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
        let chat = directory.chat(b"chat", deadline()).unwrap();
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
                    chat.marks.invocations.insert(index.to_string());
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
