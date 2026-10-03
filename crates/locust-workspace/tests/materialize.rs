//! Materialization failures: each leaves no destination behind.

mod support;

use std::fs;
use std::io;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use locust_proto::id::BlobHash;
use locust_proto::manifest::{Entry, Manifest, ManifestError};
use locust_workspace::{BlobSource, MaterializeError, materialize};
use support::{File, MemBlobs, list_dir, manifest_of, read_tree};

const FILES: [File; 3] = [
    ("a.txt", b"first\n", false),
    ("b/run.sh", b"#!/bin/sh\n", true),
    ("c.txt", b"third\n", false),
];

/// Hands out `remaining` objects, then fails: by panicking if `panic` is
/// set, otherwise with an error.
struct Failing<'a> {
    store: &'a mut MemBlobs,
    remaining: usize,
    panic: bool,
}

impl BlobSource for Failing<'_> {
    fn fetch(&mut self, hash: &BlobHash) -> io::Result<Option<Vec<u8>>> {
        if self.remaining == 0 {
            if self.panic {
                panic!("simulated interruption");
            }
            return Err(io::Error::other("simulated source failure"));
        }
        self.remaining -= 1;
        self.store.fetch(hash)
    }
}

fn is_case_insensitive(dir: &Path) -> bool {
    fs::write(dir.join("probe"), b"").unwrap();
    let insensitive = dir.join("PROBE").exists();
    fs::remove_file(dir.join("probe")).unwrap();
    insensitive
}

#[test]
fn an_existing_destination_is_refused_and_left_alone() {
    let parent = tempfile::tempdir().unwrap();
    let mut store = MemBlobs::default();
    let manifest = manifest_of(&mut store, &FILES);

    let destination = parent.path().join("out");
    fs::create_dir(&destination).unwrap();
    let result = materialize(&manifest, &mut store, &destination);
    assert!(
        matches!(result, Err(MaterializeError::DestinationExists(_))),
        "{result:?}"
    );
    assert!(list_dir(&destination).is_empty());

    let dangling = parent.path().join("dangling");
    std::os::unix::fs::symlink(parent.path().join("nowhere"), &dangling).unwrap();
    let result = materialize(&manifest, &mut store, &dangling);
    assert!(
        matches!(result, Err(MaterializeError::DestinationExists(_))),
        "{result:?}"
    );
    assert!(!parent.path().join("nowhere").exists());
    assert_eq!(list_dir(parent.path()), ["dangling", "out"]);
}

#[test]
fn a_missing_object_fails_and_leaves_nothing() {
    let parent = tempfile::tempdir().unwrap();
    let mut store = MemBlobs::default();
    let mut manifest = manifest_of(&mut store, &FILES);
    let absent = BlobHash([7; 32]);
    manifest.entries[1].content = absent;

    let result = materialize(&manifest, &mut store, &parent.path().join("out"));
    assert!(
        matches!(&result, Err(MaterializeError::MissingObject { path, content }) if path == "b/run.sh" && *content == absent),
        "{result:?}"
    );
    assert!(list_dir(parent.path()).is_empty());
}

#[test]
fn an_object_of_the_wrong_size_fails_and_leaves_nothing() {
    let parent = tempfile::tempdir().unwrap();
    let mut store = MemBlobs::default();
    let mut manifest = manifest_of(&mut store, &FILES);
    manifest.entries[2].size += 1;

    let result = materialize(&manifest, &mut store, &parent.path().join("out"));
    assert!(
        matches!(&result, Err(MaterializeError::WrongSize { path, expected: 7, actual: 6 }) if path == "c.txt"),
        "{result:?}"
    );
    assert!(list_dir(parent.path()).is_empty());
}

#[test]
fn a_failing_source_leaves_nothing() {
    let parent = tempfile::tempdir().unwrap();
    let mut store = MemBlobs::default();
    let manifest = manifest_of(&mut store, &FILES);
    let mut source = Failing {
        store: &mut store,
        remaining: 1,
        panic: false,
    };

    let result = materialize(&manifest, &mut source, &parent.path().join("out"));
    assert!(
        matches!(result, Err(MaterializeError::Source(_))),
        "{result:?}"
    );
    assert!(list_dir(parent.path()).is_empty());
}

#[test]
fn an_interrupted_run_leaves_no_destination_and_the_next_run_replaces_its_leftover() {
    let parent = tempfile::tempdir().unwrap();
    let destination = parent.path().join("out");
    let mut store = MemBlobs::default();
    let manifest = manifest_of(&mut store, &FILES);

    // A panic stands in for the process being killed: no cleanup runs.
    let mut source = Failing {
        store: &mut store,
        remaining: 2,
        panic: true,
    };
    let interrupted = catch_unwind(AssertUnwindSafe(|| {
        materialize(&manifest, &mut source, &destination)
    }));
    assert!(interrupted.is_err());
    assert!(!destination.exists());
    let leftover = list_dir(parent.path());
    assert_eq!(leftover.len(), 1, "{leftover:?}");
    assert!(!read_tree(&parent.path().join(&leftover[0])).is_empty());

    materialize(&manifest, &mut store, &destination).unwrap();
    assert_eq!(list_dir(parent.path()), ["out"]);
    let expected = FILES
        .iter()
        .map(|&(path, contents, executable)| (path.to_owned(), (contents.to_vec(), executable)))
        .collect();
    assert_eq!(read_tree(&destination), expected);
}

#[test]
fn names_this_filesystem_folds_together_are_collisions() {
    let parent = tempfile::tempdir().unwrap();
    if !is_case_insensitive(parent.path()) {
        eprintln!("skipped: this filesystem is case-sensitive");
        return;
    }
    let mut store = MemBlobs::default();
    let cases: [(&[File], &str); 3] = [
        (
            &[("README", b"upper", false), ("readme", b"lower", false)],
            "readme",
        ),
        (
            &[("Src/a.rs", b"a", false), ("src/b.rs", b"b", false)],
            "src",
        ),
        (
            &[("Docs", b"file", false), ("docs/x.md", b"x", false)],
            "docs",
        ),
    ];
    for (files, colliding) in cases {
        let manifest = manifest_of(&mut store, files);
        let result = materialize(&manifest, &mut store, &parent.path().join("out"));
        assert!(
            matches!(&result, Err(MaterializeError::Collision(path)) if path == colliding),
            "{files:?}: {result:?}"
        );
        assert!(list_dir(parent.path()).is_empty());
    }

    // Normalization-insensitive filesystems (APFS) also fold composed and
    // decomposed forms of one name.
    let composed = "caf\u{e9}";
    fs::write(parent.path().join(composed), b"").unwrap();
    let folds_normalization = parent.path().join("cafe\u{301}").exists();
    fs::remove_file(parent.path().join(composed)).unwrap();
    if folds_normalization {
        let manifest = manifest_of(
            &mut store,
            &[(composed, b"nfc", false), ("cafe\u{301}", b"nfd", false)],
        );
        let result = materialize(&manifest, &mut store, &parent.path().join("out"));
        assert!(
            matches!(&result, Err(MaterializeError::Collision(path)) if path == composed),
            "{result:?}"
        );
        assert!(list_dir(parent.path()).is_empty());
    }
}

#[test]
fn an_invalid_manifest_is_refused_before_anything_is_written() {
    let parent = tempfile::tempdir().unwrap();
    let mut store = MemBlobs::default();
    for path in ["../escape.txt", "/etc/passwd", ".git/config", "a//b"] {
        let manifest = Manifest {
            entries: vec![Entry {
                path: path.to_owned(),
                executable: false,
                size: 1,
                content: BlobHash([1; 32]),
            }],
        };
        let result = materialize(&manifest, &mut store, &parent.path().join("out"));
        assert!(
            matches!(
                result,
                Err(MaterializeError::Manifest(ManifestError::UnsafePath))
            ),
            "{path}: {result:?}"
        );
    }
    assert!(list_dir(parent.path()).is_empty());
    assert!(!parent.path().parent().unwrap().join("escape.txt").exists());
}
