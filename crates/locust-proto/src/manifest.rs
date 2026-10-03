//! Workspace manifests: the reviewed list of files that makes up a shared
//! snapshot. A manifest's content hash identifies the snapshot; a source Git
//! commit is provenance only.
//!
//! The first release carries regular files and an executable bit. Symlinks,
//! hardlinks, special files and submodules cannot be expressed here, so they
//! cannot be shared or materialized by accident.

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::codec;
use crate::crypto::content_hash;
use crate::id::BlobHash;
use crate::limits::{MAX_MANIFEST_ENTRIES, MAX_PATH_BYTES};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Relative path with `/` separators.
    pub path: String,
    pub executable: bool,
    pub size: u64,
    pub content: BlobHash,
}

/// Entries in strictly ascending byte order of `path`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub entries: Vec<Entry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestError {
    Malformed,
    TooManyEntries,
    /// A path is absolute, empty, too long, or has a `.`, `..`, `.git`,
    /// empty, backslash or control-character component.
    UnsafePath,
    /// Entries are not sorted, or a path appears twice.
    Unordered,
    /// One path is both a file and a directory.
    FileUnderFile,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Malformed => "bytes are not a manifest",
            Self::TooManyEntries => "manifest has too many entries",
            Self::UnsafePath => "manifest contains an unsafe or unsupported path",
            Self::Unordered => "manifest entries are not sorted and unique",
            Self::FileUnderFile => "manifest uses one path as both file and directory",
        })
    }
}

impl std::error::Error for ManifestError {}

/// True if `path` is a relative path that stays inside its root on every
/// supported platform. Case collisions are checked at materialization, where
/// the target filesystem is known.
pub fn is_safe_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_PATH_BYTES
        && path.split('/').all(|component| {
            !component.is_empty()
                && component != "."
                && component != ".."
                && !component.eq_ignore_ascii_case(".git")
                && !component
                    .bytes()
                    .any(|byte| byte < 0x20 || byte == 0x7f || byte == b'\\')
        })
}

impl Manifest {
    pub fn check(&self) -> Result<(), ManifestError> {
        if self.entries.len() > MAX_MANIFEST_ENTRIES {
            return Err(ManifestError::TooManyEntries);
        }
        if !self.entries.iter().all(|entry| is_safe_path(&entry.path)) {
            return Err(ManifestError::UnsafePath);
        }
        if !self.entries.is_sorted_by(|a, b| a.path < b.path) {
            return Err(ManifestError::Unordered);
        }
        let files: HashSet<&str> = self
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect();
        let nested_under_file = self.entries.iter().any(|entry| {
            entry
                .path
                .match_indices('/')
                .any(|(end, _)| files.contains(&entry.path[..end]))
        });
        if nested_under_file {
            return Err(ManifestError::FileUnderFile);
        }
        Ok(())
    }

    /// The bytes that are stored and hashed.
    pub fn encode(&self) -> Result<Vec<u8>, ManifestError> {
        self.check()?;
        codec::encode(self).map_err(|_| ManifestError::Malformed)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ManifestError> {
        let manifest: Self = codec::decode(bytes).map_err(|_| ManifestError::Malformed)?;
        manifest.check()?;
        Ok(manifest)
    }

    /// The snapshot identifier: the content hash of the encoded manifest.
    pub fn id(&self) -> Result<BlobHash, ManifestError> {
        Ok(content_hash(&self.encode()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> Entry {
        Entry {
            path: path.to_string(),
            executable: false,
            size: 1,
            content: content_hash(path.as_bytes()),
        }
    }

    fn manifest(paths: &[&str]) -> Manifest {
        Manifest {
            entries: paths.iter().copied().map(entry).collect(),
        }
    }

    #[test]
    fn a_valid_manifest_round_trips_and_has_a_stable_id() {
        let manifest = manifest(&["Cargo.toml", "src/lib.rs", "src/main.rs"]);
        let bytes = manifest.encode().unwrap();
        assert_eq!(Manifest::decode(&bytes), Ok(manifest.clone()));
        assert_eq!(manifest.id().unwrap(), content_hash(&bytes));
    }

    #[test]
    fn unsafe_paths_are_refused() {
        for path in [
            "",
            "/etc/passwd",
            "a//b",
            "a/./b",
            "../a",
            "a/..",
            ".git/config",
            "sub/.GIT/hooks/pre-commit",
            "a\\b",
            "a\nb",
            "trailing/",
        ] {
            assert!(!is_safe_path(path), "{path:?} should be refused");
            assert_eq!(manifest(&[path]).check(), Err(ManifestError::UnsafePath));
        }
        assert!(is_safe_path(".github/workflows/ci.yml"));
        assert!(is_safe_path("docs/.gitignore"));
        assert!(!is_safe_path(&"a".repeat(MAX_PATH_BYTES + 1)));
    }

    #[test]
    fn order_uniqueness_and_file_directory_clashes_are_checked() {
        assert_eq!(manifest(&["b", "a"]).check(), Err(ManifestError::Unordered));
        assert_eq!(manifest(&["a", "a"]).check(), Err(ManifestError::Unordered));
        assert_eq!(
            manifest(&["a", "a.txt", "a/b"]).check(),
            Err(ManifestError::FileUnderFile)
        );
        assert_eq!(manifest(&["a.txt", "a/b", "a/c"]).check(), Ok(()));
    }

    #[test]
    fn decode_checks_what_it_decodes() {
        let unordered = codec::encode(&manifest(&["b", "a"])).unwrap();
        assert_eq!(Manifest::decode(&unordered), Err(ManifestError::Unordered));
        assert_eq!(Manifest::decode(&[0xff]), Err(ManifestError::Malformed));
    }
}
