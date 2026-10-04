//! Atomic profile overlays with recoverable originals and no-follow components.
use crate::failure::Failure;
use locust_proto::api::ErrorCode;
use rustix::fs::{self as fs, AtFlags, Mode, OFlags, RenameFlags};
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

fn io(error: impl std::fmt::Display) -> Failure {
    Failure::internal(format!("profile filesystem operation failed: {error}"))
}
pub(super) fn directory(path: &Path) -> Result<File, Failure> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err(Failure::usage(
            "selected directories must be absolute paths without . or .. components",
        ));
    }
    // Selection of the root is explicit local authority. Components beneath it
    // are opened separately with NOFOLLOW; the root's final entry is no-follow.
    Ok(File::from(
        fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(io)?,
    ))
}
pub(super) fn private_file(parent: &File, name: &str) -> Result<File, Failure> {
    let file = File::from(
        fs::openat(
            parent,
            name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(io)?,
    );
    file.sync_all().map_err(io)?;
    parent.sync_all().map_err(io)?;
    Ok(file)
}
pub(super) fn lock(parent: &File, name: &str) -> Result<File, Failure> {
    let file = File::from(
        fs::openat(
            parent,
            name,
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(io)?,
    );
    let m = file.metadata().map_err(io)?;
    if !m.is_file()
        || m.nlink() != 1
        || m.mode() & 0o777 != 0o600
        || m.uid() != parent.metadata().map_err(io)?.uid()
    {
        return Err(Failure::invalid(
            "launcher lock must be an owned private regular file",
        ));
    }
    file.try_lock().map_err(|_| {
        Failure::new(
            ErrorCode::Conflict,
            "another local launcher owns this session or profile",
        )
    })?;
    Ok(file)
}
struct Snapshot {
    bytes: Vec<u8>,
    identity: (u64, u64),
    mode: u32,
}
fn snapshot(parent: &File, name: &str) -> Result<Option<Snapshot>, Failure> {
    let fd = match fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(e) => return Err(io(e)),
    };
    let mut file = File::from(fd);
    let m = file.metadata().map_err(io)?;
    if !m.is_file() || m.nlink() != 1 {
        return Err(Failure::invalid(
            "profile configuration must be a regular file without hardlinks",
        ));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(io)?;
    let after = file.metadata().map_err(io)?;
    if m.len() != after.len()
        || m.mtime() != after.mtime()
        || m.mtime_nsec() != after.mtime_nsec()
        || m.ctime() != after.ctime()
        || m.ctime_nsec() != after.ctime_nsec()
    {
        return Err(Failure::new(
            ErrorCode::Conflict,
            "profile file changed while reading",
        ));
    }
    Ok(Some(Snapshot {
        bytes,
        identity: (m.dev(), m.ino()),
        mode: m.mode() & 0o7777,
    }))
}
fn read(parent: &File, name: &str) -> Result<Option<Vec<u8>>, Failure> {
    Ok(snapshot(parent, name)?.map(|s| s.bytes))
}

pub(super) struct Overlay {
    parent: File,
    name: String,
    backup: String,
    staged: String,
    pub baseline: Option<Vec<u8>>,
    written: Option<Vec<u8>>,
    baseline_mode: u32,
    baseline_identity: Option<(u64, u64)>,
    restore_staged: String,
    pub path: PathBuf,
}
impl Overlay {
    pub fn inspect(profile: &Path, relative: &Path, nonce: &str) -> Result<Self, Failure> {
        let mut parent = directory(profile)?;
        let parts: Vec<_> = relative.components().collect();
        for part in &parts[..parts.len() - 1] {
            let Component::Normal(name) = part else {
                return Err(Failure::invalid("unsafe profile path"));
            };
            match fs::mkdirat(&parent, *name, Mode::from_raw_mode(0o700)) {
                Ok(()) => parent.sync_all().map_err(io)?,
                Err(rustix::io::Errno::EXIST) => (),
                Err(e) => return Err(io(e)),
            }
            parent = File::from(
                fs::openat(
                    &parent,
                    *name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(io)?,
            );
        }
        let name = relative
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Failure::invalid("invalid profile file"))?
            .to_owned();
        let original = snapshot(&parent, &name)?;
        let baseline_mode = original.as_ref().map_or(0o600, |s| s.mode);
        let baseline_identity = original.as_ref().map(|s| s.identity);
        let baseline = original.map(|s| s.bytes);
        Ok(Self {
            parent,
            name,
            backup: format!(".locust-{nonce}.original"),
            staged: format!(".locust-{nonce}.overlay"),
            baseline,
            written: None,
            baseline_mode,
            baseline_identity,
            restore_staged: format!(".locust-{nonce}.restore"),
            path: profile.join(relative),
        })
    }
    fn original_matches(&self, name: &str) -> Result<bool, Failure> {
        Ok(snapshot(&self.parent, name)?.is_some_and(|s| {
            Some(s.identity) == self.baseline_identity && Some(&s.bytes) == self.baseline.as_ref()
        }))
    }
    pub fn apply(&mut self, bytes: Vec<u8>) -> Result<(), Failure> {
        if read(&self.parent, &self.name)? != self.baseline {
            return Err(Failure::new(
                ErrorCode::Conflict,
                "profile changed before overlay publication",
            ));
        }
        let mut staged = private_file(&self.parent, &self.staged)?;
        staged.write_all(&bytes).map_err(io)?;
        staged.sync_all().map_err(io)?;
        if self.baseline.is_some() {
            fs::renameat_with(
                &self.parent,
                &self.name,
                &self.parent,
                &self.backup,
                RenameFlags::NOREPLACE,
            )
            .map_err(io)?;
            let backup = File::from(
                fs::openat(
                    &self.parent,
                    &self.backup,
                    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(io)?,
            );
            backup
                .set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(io)?;
            backup.sync_all().map_err(io)?;
            if !self.original_matches(&self.backup)? {
                return Err(Failure::new(
                    ErrorCode::Conflict,
                    format!(
                        "profile changed during publication; original retained beside {} as {}",
                        self.path.display(),
                        self.backup
                    ),
                ));
            }
        }
        if let Err(e) = fs::renameat_with(
            &self.parent,
            &self.staged,
            &self.parent,
            &self.name,
            RenameFlags::NOREPLACE,
        ) {
            if self.baseline.is_some() {
                let _ = fs::renameat_with(
                    &self.parent,
                    &self.backup,
                    &self.parent,
                    &self.name,
                    RenameFlags::NOREPLACE,
                );
            }
            return Err(Failure::new(
                ErrorCode::Conflict,
                format!(
                    "profile publication conflicted ({e}); recovery files retained beside {}",
                    self.path.display()
                ),
            ));
        }
        self.written = Some(bytes);
        self.parent.sync_all().map_err(io)
    }
    pub fn restore(&mut self) -> Result<(), Failure> {
        let Some(expected) = self.written.as_ref() else {
            return Ok(());
        };
        if read(&self.parent, &self.name)?.as_ref() != Some(expected) {
            return Err(Failure::new(
                ErrorCode::Conflict,
                format!(
                    "profile changed during client run; preserved edits and original backup beside {} as {}",
                    self.path.display(),
                    self.backup
                ),
            ));
        }
        if self.baseline.is_some() && !self.original_matches(&self.backup)? {
            return Err(Failure::new(
                ErrorCode::Conflict,
                format!(
                    "original profile backup changed; retained overlay and recovery files beside {}",
                    self.path.display()
                ),
            ));
        }
        // Move the exact owned overlay aside before restoration. A concurrent
        // writer is never overwritten: restore uses NOREPLACE.
        fs::renameat_with(
            &self.parent,
            &self.name,
            &self.parent,
            &self.staged,
            RenameFlags::NOREPLACE,
        )
        .map_err(io)?;
        if read(&self.parent, &self.staged)?.as_ref() != Some(expected) {
            let _ = fs::renameat_with(
                &self.parent,
                &self.staged,
                &self.parent,
                &self.name,
                RenameFlags::NOREPLACE,
            );
            return Err(Failure::new(
                ErrorCode::Conflict,
                "concurrent profile edit retained; original backup requires local recovery",
            ));
        }
        if self.baseline.is_some() {
            fs::renameat_with(
                &self.parent,
                &self.backup,
                &self.parent,
                &self.restore_staged,
                RenameFlags::NOREPLACE,
            )
            .map_err(io)?;
            if !self.original_matches(&self.restore_staged)? {
                return Err(Failure::new(
                    ErrorCode::Conflict,
                    "original profile backup changed during restoration; recovery files retained",
                ));
            }
            let backup = File::from(
                fs::openat(
                    &self.parent,
                    &self.restore_staged,
                    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(io)?,
            );
            fs::renameat_with(
                &self.parent,
                &self.restore_staged,
                &self.parent,
                &self.name,
                RenameFlags::NOREPLACE,
            )
            .map_err(io)?;
            backup
                .set_permissions(std::fs::Permissions::from_mode(self.baseline_mode))
                .map_err(io)?;
            if !self.original_matches(&self.name)? {
                return Err(Failure::new(
                    ErrorCode::Conflict,
                    "profile changed during original publication; recovery overlay retained",
                ));
            }
        }
        fs::unlinkat(&self.parent, &self.staged, AtFlags::empty()).map_err(io)?;
        self.written = None;
        self.parent.sync_all().map_err(io)
    }
}
