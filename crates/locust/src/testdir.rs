//! Temporary state directories for tests.

use std::fs;
use std::os::unix::fs::PermissionsExt;

/// A private temporary directory with a short path. macOS limits a socket
/// path to 103 bytes, and its default temporary directory is already long.
pub(crate) fn short_dir() -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix("lc")
        .tempdir_in("/tmp")
        .unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    dir
}
