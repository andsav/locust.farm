//! Temporary state directories for tests.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// A private temporary directory with a short path, one level inside the
/// temporary directory that owns it, so the marks directory beside it is
/// removed with it. macOS limits a socket path to 103 bytes, and its default
/// temporary directory is already long.
pub(crate) struct ShortDir {
    path: PathBuf,
    _root: tempfile::TempDir,
}

impl ShortDir {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

pub(crate) fn short_dir() -> ShortDir {
    let root = tempfile::Builder::new()
        .prefix("lc")
        .tempdir_in("/tmp")
        .unwrap();
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = root.path().join("h");
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    ShortDir { path, _root: root }
}
