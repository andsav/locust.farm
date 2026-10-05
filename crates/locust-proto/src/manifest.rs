//! Workspace manifests: the reviewed list of files that makes up a shared
//! snapshot. A manifest's content hash identifies the snapshot; a source Git
//! commit is provenance only.
//!
//! The first release carries regular files and an executable bit. Symlinks,
//! hardlinks, special files and submodules cannot be expressed here, so they
//! cannot be shared or materialized by accident.
//!
//! Like every content object of a goal, the files a manifest lists and the
//! manifest itself are stored sealed (see [`crate::seal`]). An entry names
//! its file by the hash of the stored, sealed object and gives the plaintext
//! size; events name a manifest by the hash of its sealed object as well.
//! [`Manifest::decode`] accepts only the one canonical encoding, so a decoded
//! manifest's [`Manifest::plain_digest`] is always the hash of the bytes it came from.
//!
//! [`is_safe_path`] refuses paths that are unsafe or ambiguous on a supported
//! platform. Case folding and Unicode normalization depend on the destination
//! filesystem and cannot be predicted from a manifest, so the materializer
//! creates every file with create-new semantics in a fresh destination and
//! reports a collision instead of overwriting.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::codec::{self, CodecError};
use crate::crypto::content_hash;
use crate::id::BlobHash;
use crate::limits::{MAX_MANIFEST_ENTRIES, MAX_PATH_BYTES};

/// One regular file of a snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Entry {
    /// Relative path with `/` separators; see [`is_safe_path`].
    pub path: String,
    /// The file is materialized with its executable bits set.
    pub executable: bool,
    /// Size of the file's plaintext, in bytes.
    pub size: u64,
    /// Hash of the stored object holding the file: its sealed bytes.
    pub content: BlobHash,
}

/// A snapshot: the files it contains and nothing else.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Manifest {
    /// Entries in strictly ascending byte order of `path`.
    pub entries: Vec<Entry>,
}

/// Why bytes or a value were not accepted as a manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManifestError {
    /// The bytes do not decode as a manifest.
    Malformed,
    /// The bytes decode, but are not the one encoding of that manifest, so
    /// they do not hash to its identifier.
    NotCanonical,
    /// More than [`MAX_MANIFEST_ENTRIES`] entries.
    TooManyEntries,
    /// A path breaks a rule of [`is_safe_path`].
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
            Self::NotCanonical => "manifest is not in its canonical encoding",
            Self::TooManyEntries => "manifest has too many entries",
            Self::UnsafePath => "manifest contains an unsafe or unsupported path",
            Self::Unordered => "manifest entries are not sorted and unique",
            Self::FileUnderFile => "manifest uses one path as both file and directory",
        })
    }
}

impl std::error::Error for ManifestError {}

/// Longest path component, in bytes: the common filesystem limit.
const MAX_COMPONENT_BYTES: usize = 255;

/// True if `path` is a relative path that stays inside its root and names
/// the same file on every supported platform.
///
/// The path must be non-empty and at most [`MAX_PATH_BYTES`] long. Each
/// `/`-separated component is refused if it:
///
/// - is empty, `.` or `..`;
/// - is `.git`, or `git~1` (its Windows short name), ASCII case-insensitively;
/// - is longer than 255 bytes;
/// - contains a byte below 0x20, 0x7f, `\`, or `:` (which names a Windows
///   drive or alternate data stream);
/// - ends with `.` or a space, which Windows strips;
/// - contains a C1 control (U+0080..=U+009F) or an invisible formatting
///   character (U+200C..=U+200F, U+202A..=U+202E, U+2066..=U+206F, U+FEFF),
///   which some filesystems ignore and which hide a name from a reviewer.
///
/// Case and normalization folding cannot be predicted from a manifest, so
/// they are not refused here: the materializer creates every file with
/// create-new semantics in a fresh destination and reports a collision.
pub fn is_safe_path(path: &str) -> bool {
    !path.is_empty() && path.len() <= MAX_PATH_BYTES && path.split('/').all(is_safe_component)
}

fn is_safe_component(component: &str) -> bool {
    let bytes = component.as_bytes();
    let Some(&last) = bytes.last() else {
        return false;
    };
    bytes.len() <= MAX_COMPONENT_BYTES
        && component != "."
        && component != ".."
        && last != b'.'
        && last != b' '
        && !component.eq_ignore_ascii_case(".git")
        && !component.eq_ignore_ascii_case("git~1")
        && !bytes
            .iter()
            .any(|&byte| byte < 0x20 || byte == 0x7f || byte == b'\\' || byte == b':')
        && (component.is_ascii() || !component.chars().any(is_hidden_char))
}

/// C1 controls and invisible formatting characters.
fn is_hidden_char(c: char) -> bool {
    matches!(
        c,
        '\u{80}'..='\u{9f}'
            | '\u{200c}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{206f}'
            | '\u{feff}'
    )
}

/// True if some entry's path continues another entry's path with `/`.
///
/// One pass over entries sorted by path, with no hashing. All paths that
/// start with a given path form one contiguous run right after it, so
/// `chain` holds the earlier paths that are string prefixes of the current
/// one, and leaving a run pops its path. Only the top of the chain needs
/// checking: if a shorter path in the chain were a file the current path
/// lies under, the top, a longer prefix of the current path, would lie under
/// it too and would have been reported when it was reached.
fn nests_under_file(entries: &[Entry]) -> bool {
    let mut chain: Vec<&str> = Vec::new();
    for entry in entries {
        let path = entry.path.as_str();
        while chain.last().is_some_and(|top| !path.starts_with(top)) {
            chain.pop();
        }
        if let Some(top) = chain.last()
            && path.as_bytes().get(top.len()) == Some(&b'/')
        {
            return true;
        }
        chain.push(path);
    }
    false
}

/// Bytes in the longest variable-length `u64`.
const MAX_VARINT_BYTES: usize = 10;

/// The entry count at the start of an encoded manifest, read without
/// decoding any entry. It is postcard's length prefix, read as postcard reads
/// a `usize` on a 64-bit target: base-128 groups, least significant first,
/// the high bit set on every byte but the last, at most ten bytes, and the
/// tenth carrying only the top bit.
fn announced_entries(bytes: &[u8]) -> Result<u64, ManifestError> {
    let mut count = 0u64;
    for (index, &byte) in bytes.iter().take(MAX_VARINT_BYTES).enumerate() {
        count |= u64::from(byte & 0x7f) << (7 * index);
        if byte & 0x80 == 0 {
            return if index == MAX_VARINT_BYTES - 1 && byte > 1 {
                Err(ManifestError::Malformed)
            } else {
                Ok(count)
            };
        }
    }
    Err(ManifestError::Malformed)
}

impl Manifest {
    /// Checks the rules every manifest meets: at most
    /// [`MAX_MANIFEST_ENTRIES`] entries, every path safe ([`is_safe_path`]),
    /// paths strictly ascending in byte order, and no path both a file and a
    /// directory.
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
        if nests_under_file(&self.entries) {
            return Err(ManifestError::FileUnderFile);
        }
        Ok(())
    }

    /// The canonical encoding: the plaintext that is sealed and stored, and
    /// that [`Manifest::plain_digest`] hashes. Refuses a manifest that fails
    /// [`Manifest::check`].
    pub fn encode(&self) -> Result<Vec<u8>, ManifestError> {
        self.check()?;
        codec::encode(self).map_err(|_| ManifestError::Malformed)
    }

    /// Decodes a manifest's plaintext encoding. Refuses an announced entry
    /// count above [`MAX_MANIFEST_ENTRIES`] before decoding any entry, then
    /// accepts only the one canonical encoding and runs [`Manifest::check`].
    /// For every byte string it accepts,
    /// `Manifest::decode(bytes)?.plain_digest()? == content_hash(bytes).0`.
    pub fn decode(bytes: &[u8]) -> Result<Self, ManifestError> {
        if announced_entries(bytes)? > MAX_MANIFEST_ENTRIES as u64 {
            return Err(ManifestError::TooManyEntries);
        }
        let manifest: Self = codec::decode_canonical(bytes).map_err(|error| match error {
            CodecError::NotCanonical => ManifestError::NotCanonical,
            _ => ManifestError::Malformed,
        })?;
        manifest.check()?;
        Ok(manifest)
    }

    /// BLAKE3 of the canonical plaintext encoding: the same in every goal and
    /// key epoch, so two snapshots can be compared for equal content. It is
    /// never the identifier an event carries. Events name the manifest's
    /// stored object by the hash of its sealed bytes, which depends on the
    /// goal and epoch; comparing the two never matches.
    pub fn plain_digest(&self) -> Result<[u8; 32], ManifestError> {
        Ok(content_hash(&self.encode()?).0)
    }

    /// The sum of the entries' plaintext sizes, or `None` if it overflows
    /// `u64`.
    pub fn total_size(&self) -> Option<u64> {
        self.entries
            .iter()
            .try_fold(0u64, |total, entry| total.checked_add(entry.size))
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

    /// Asserts each path is refused, alone and inside a directory, and that
    /// a manifest holding it fails its check.
    fn assert_refused(paths: &[&str]) {
        for path in paths {
            assert!(!is_safe_path(path), "{path:?} should be refused");
            assert!(!is_safe_path(&format!("dir/{path}")), "{path:?} nested");
            assert_eq!(manifest(&[path]).check(), Err(ManifestError::UnsafePath));
        }
    }

    fn assert_allowed(paths: &[&str]) {
        for path in paths {
            assert!(is_safe_path(path), "{path:?} should be allowed");
        }
    }

    #[test]
    fn a_valid_manifest_round_trips_and_has_a_stable_id() {
        let manifest = manifest(&["Cargo.toml", "src/lib.rs", "src/main.rs"]);
        let bytes = manifest.encode().unwrap();
        assert_eq!(Manifest::decode(&bytes), Ok(manifest.clone()));
        assert_eq!(manifest.plain_digest().unwrap(), content_hash(&bytes).0);
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
    fn a_component_longer_than_255_bytes_is_refused() {
        let long = "a".repeat(256);
        let wide = "é".repeat(128);
        assert_refused(&[&long, &wide]);
        assert_allowed(&[&"a".repeat(255), &"é".repeat(127)]);
        assert!(is_safe_path(&format!(
            "{}/{}",
            "a".repeat(255),
            "b".repeat(255)
        )));
    }

    #[test]
    fn a_colon_is_refused() {
        assert_refused(&["a:b", "C:", "notes.txt:hidden", ".git::$INDEX_ALLOCATION"]);
        assert_allowed(&["a;b", "a=b"]);
    }

    #[test]
    fn a_trailing_dot_or_space_is_refused() {
        assert_refused(&["a.", "a ", "...", "dir./x", "dir /x", ".git."]);
        assert_allowed(&[".a", " a", "a.b", "a b", ".env"]);
    }

    #[test]
    fn c1_controls_and_invisible_formatting_characters_are_refused() {
        for c in [
            '\u{80}', '\u{9f}', '\u{200c}', '\u{200d}', '\u{200f}', '\u{202a}', '\u{202e}',
            '\u{2066}', '\u{206f}', '\u{feff}',
        ] {
            assert_refused(&[&format!("a{c}b"), &format!("{c}")]);
        }
        for c in ['\u{a1}', '\u{2010}', '\u{2070}', 'é', '日'] {
            assert_allowed(&[&format!("a{c}b")]);
        }
    }

    #[test]
    fn the_windows_short_name_of_git_is_refused() {
        assert_refused(&["git~1", "GIT~1", "Git~1/config"]);
        assert_allowed(&["git~2", "git~10", "git", "agit~1", "git~1a"]);
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
        // Paths sorting between a file and its would-be children.
        assert_eq!(
            manifest(&["a", "a b", "a.b/c", "a/x"]).check(),
            Err(ManifestError::FileUnderFile)
        );
        assert_eq!(manifest(&["a b", "a.b/c", "a/x"]).check(), Ok(()));
        assert_eq!(
            manifest(&["a/b", "a/b.txt", "a/b/c/d"]).check(),
            Err(ManifestError::FileUnderFile)
        );
        assert_eq!(
            manifest(&["a", "b", "b/c", "c"]).check(),
            Err(ManifestError::FileUnderFile)
        );
        assert_eq!(manifest(&["a", "ab/c", "b"]).check(), Ok(()));
    }

    #[test]
    fn the_file_under_file_check_handles_many_deep_paths() {
        // Before the one-pass check, each of these paths hashed ~400 prefixes
        // of up to 800 bytes.
        let deep = "d/".repeat(400);
        let files: Vec<String> = (0..2_000).map(|n| format!("{deep}f{n:05}")).collect();
        let mut manifest = Manifest {
            entries: files.iter().map(|path| entry(path)).collect(),
        };
        assert_eq!(manifest.check(), Ok(()));

        // A file that is also a directory, deep inside the run.
        let child = entry(&format!("{deep}f01000/child"));
        manifest.entries.insert(1_001, child);
        assert_eq!(manifest.check(), Err(ManifestError::FileUnderFile));
        manifest.entries.remove(1_001);

        // A shallow file that is an ancestor of every other entry.
        manifest.entries.insert(0, entry(&"d/".repeat(200)[..399]));
        assert_eq!(manifest.check(), Err(ManifestError::FileUnderFile));
    }

    #[test]
    fn decode_checks_what_it_decodes() {
        let unordered = codec::encode(&manifest(&["b", "a"])).unwrap();
        assert_eq!(Manifest::decode(&unordered), Err(ManifestError::Unordered));
        assert_eq!(Manifest::decode(&[0xff]), Err(ManifestError::Malformed));
        assert_eq!(Manifest::decode(&[]), Err(ManifestError::Malformed));
    }

    /// Lane B's B-R4 reproduction: an overlong entry count decoded to an empty
    /// manifest whose identifier was not the hash of the stored bytes.
    #[test]
    fn an_overlong_encoding_of_the_empty_manifest_is_refused() {
        assert_eq!(
            Manifest::decode(&[0x80, 0x00]),
            Err(ManifestError::NotCanonical)
        );
        let empty = Manifest::decode(&[0x00]).unwrap();
        assert_eq!(empty.plain_digest(), Ok(content_hash(&[0x00]).0));
    }

    #[test]
    fn an_entry_count_above_the_limit_is_refused_before_entries_are_decoded() {
        // Only the count is present: decoding entries would fail as malformed.
        let over = codec::encode(&(MAX_MANIFEST_ENTRIES + 1)).unwrap();
        assert_eq!(Manifest::decode(&over), Err(ManifestError::TooManyEntries));
        let at_limit = codec::encode(&MAX_MANIFEST_ENTRIES).unwrap();
        assert_eq!(Manifest::decode(&at_limit), Err(ManifestError::Malformed));

        let top_bit = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01];
        assert_eq!(
            Manifest::decode(&top_bit),
            Err(ManifestError::TooManyEntries)
        );
        let past_u64 = [0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x02];
        assert_eq!(Manifest::decode(&past_u64), Err(ManifestError::Malformed));
        let unterminated = [0x80; 10];
        assert_eq!(
            Manifest::decode(&unterminated),
            Err(ManifestError::Malformed)
        );
    }

    #[test]
    fn every_accepted_byte_string_hashes_to_the_decoded_identifier() {
        let samples = [
            Manifest::default(),
            manifest(&["a"]),
            Manifest {
                entries: vec![
                    Entry {
                        path: "bin/run".to_string(),
                        executable: true,
                        size: 300,
                        content: BlobHash([0x80; 32]),
                    },
                    Entry {
                        path: "src/lib.rs".to_string(),
                        executable: false,
                        size: u64::MAX,
                        content: content_hash(b"lib"),
                    },
                ],
            },
        ];
        let mut accepted = 0;
        let mut not_canonical = 0;
        let mut probe = |bytes: &[u8]| match Manifest::decode(bytes) {
            Ok(manifest) => {
                assert_eq!(
                    manifest.plain_digest(),
                    Ok(content_hash(bytes).0),
                    "{bytes:02x?}"
                );
                accepted += 1;
            }
            Err(ManifestError::NotCanonical) => not_canonical += 1,
            Err(_) => {}
        };
        for sample in &samples {
            let bytes = sample.encode().unwrap();
            probe(&bytes);
            for index in 0..bytes.len() {
                // Every value at every position.
                for value in 0..=u8::MAX {
                    let mut mutated = bytes.clone();
                    mutated[index] = value;
                    probe(&mutated);
                }
                // The byte dropped.
                let mut shorter = bytes.clone();
                shorter.remove(index);
                probe(&shorter);
                // A final varint byte rewritten in an overlong two-byte form.
                if bytes[index] < 0x80 {
                    let mut overlong = bytes.clone();
                    overlong.splice(index..=index, [bytes[index] | 0x80, 0x00]);
                    probe(&overlong);
                }
            }
        }
        assert!(accepted > samples.len());
        assert!(not_canonical > 0);
    }

    #[test]
    fn total_size_sums_plaintext_sizes_without_overflowing() {
        assert_eq!(Manifest::default().total_size(), Some(0));
        let mut manifest = manifest(&["a", "b"]);
        manifest.entries[0].size = 40;
        manifest.entries[1].size = 2;
        assert_eq!(manifest.total_size(), Some(42));
        manifest.entries[1].size = u64::MAX;
        assert_eq!(manifest.total_size(), None);
    }
}
