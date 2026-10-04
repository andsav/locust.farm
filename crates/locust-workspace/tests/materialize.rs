//! Publication, retained private stages, and concurrent destination safety.

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

fn cause(result: &Result<(), MaterializeError>) -> &MaterializeError {
    let mut error = result.as_ref().unwrap_err();
    while let MaterializeError::Staging { error: inner, .. } = error {
        error = inner;
    }
    error
}
fn assert_retained_stages(parent: &Path) {
    assert!(!parent.join("out").exists());
    let names = list_dir(parent);
    assert!(!names.is_empty());
    assert!(
        names
            .iter()
            .all(|name| name.starts_with(".locust-apply-materialize-"))
    );
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
fn a_missing_object_retains_a_private_stage_without_a_destination() {
    let parent = tempfile::tempdir().unwrap();
    let mut store = MemBlobs::default();
    let mut manifest = manifest_of(&mut store, &FILES);
    let absent = BlobHash([7; 32]);
    manifest.entries[1].content = absent;

    let result = materialize(&manifest, &mut store, &parent.path().join("out"));
    assert!(
        matches!(cause(&result), MaterializeError::MissingObject { path, content } if path == "b/run.sh" && *content == absent),
        "{result:?}"
    );
    assert_retained_stages(parent.path());
}

#[test]
fn a_wrong_size_object_retains_a_private_stage_without_a_destination() {
    let parent = tempfile::tempdir().unwrap();
    let mut store = MemBlobs::default();
    let mut manifest = manifest_of(&mut store, &FILES);
    manifest.entries[2].size += 1;

    let result = materialize(&manifest, &mut store, &parent.path().join("out"));
    assert!(
        matches!(cause(&result), MaterializeError::WrongSize { path, expected: 7, actual: 6 } if path == "c.txt"),
        "{result:?}"
    );
    assert_retained_stages(parent.path());
}

#[test]
fn a_failing_source_retains_a_private_stage_without_a_destination() {
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
        matches!(cause(&result), MaterializeError::Source(_)),
        "{result:?}"
    );
    assert_retained_stages(parent.path());
}

#[test]
fn an_interrupted_run_retains_its_stage_and_the_next_run_leaves_it_alone() {
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
    assert_eq!(list_dir(parent.path()).len(), 2);
    assert!(parent.path().join(&leftover[0]).exists());
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
            matches!(cause(&result), MaterializeError::Collision(path) if path == colliding),
            "{files:?}: {result:?}"
        );
        assert_retained_stages(parent.path());
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
            matches!(cause(&result), MaterializeError::Collision(path) if path == composed),
            "{result:?}"
        );
        assert_retained_stages(parent.path());
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

#[test]
fn concurrent_materializations_publish_one_exact_manifest_without_mixing() {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::mpsc;
    use std::time::Duration;
    struct Paused {
        store: MemBlobs,
        calls: usize,
        ready: mpsc::Sender<()>,
        resume: mpsc::Receiver<()>,
    }
    impl BlobSource for Paused {
        fn fetch(&mut self, hash: &BlobHash) -> io::Result<Option<Vec<u8>>> {
            self.calls += 1;
            if self.calls == 2 {
                self.ready.send(()).unwrap();
                self.resume.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            self.store.fetch(hash)
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("out");
    let mut a_store = MemBlobs::default();
    let a = manifest_of(&mut a_store, &[("a1", b"A1", false), ("a2", b"A2", false)]);
    let mut b_store = MemBlobs::default();
    let b = manifest_of(&mut b_store, &[("b1", b"B1", false), ("b2", b"B2", false)]);
    let (ready, received) = mpsc::channel();
    let (a_resume, a_wait) = mpsc::channel();
    let (b_resume, b_wait) = mpsc::channel();
    let a_destination = destination.clone();
    let a_ready = ready.clone();
    let a_thread = std::thread::spawn(move || {
        materialize(
            &a,
            &mut Paused {
                store: a_store,
                calls: 0,
                ready: a_ready,
                resume: a_wait,
            },
            &a_destination,
        )
    });
    let b_destination = destination.clone();
    let b_thread = std::thread::spawn(move || {
        materialize(
            &b,
            &mut Paused {
                store: b_store,
                calls: 0,
                ready,
                resume: b_wait,
            },
            &b_destination,
        )
    });
    received.recv_timeout(Duration::from_secs(5)).unwrap();
    received.recv_timeout(Duration::from_secs(5)).unwrap();
    let stages = list_dir(temp.path());
    assert_eq!(stages.len(), 2);
    for stage in &stages {
        assert_eq!(
            fs::metadata(temp.path().join(stage))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(list_dir(&temp.path().join(stage)).len(), 1);
    }
    a_resume.send(()).unwrap();
    b_resume.send(()).unwrap();
    let a_result = a_thread.join().unwrap();
    let b_result = b_thread.join().unwrap();
    assert_ne!(a_result.is_ok(), b_result.is_ok());
    let winner = if a_result.is_ok() {
        ["a1", "a2"]
    } else {
        ["b1", "b2"]
    };
    assert_eq!(list_dir(&destination), winner);
    let loser = if a_result.is_err() {
        a_result
    } else {
        b_result
    };
    assert!(matches!(
        cause(&loser),
        MaterializeError::DestinationExists(_)
    ));
    assert_eq!(
        list_dir(temp.path()).len(),
        2,
        "winner destination and loser stage remain"
    );
}

#[test]
fn a_destination_created_during_fetch_is_never_replaced() {
    struct CreateDestination {
        store: MemBlobs,
        destination: std::path::PathBuf,
    }
    impl BlobSource for CreateDestination {
        fn fetch(&mut self, hash: &BlobHash) -> io::Result<Option<Vec<u8>>> {
            fs::create_dir(&self.destination)?;
            self.store.fetch(hash)
        }
    }
    let parent = tempfile::tempdir().unwrap();
    let destination = parent.path().join("out");
    let mut store = MemBlobs::default();
    let manifest = manifest_of(&mut store, &[("a", b"approved", false)]);
    let result = materialize(
        &manifest,
        &mut CreateDestination {
            store,
            destination: destination.clone(),
        },
        &destination,
    );
    assert!(
        matches!(cause(&result), MaterializeError::DestinationExists(_)),
        "{result:?}"
    );
    assert!(
        list_dir(&destination).is_empty(),
        "concurrently created empty directory was replaced"
    );
    let MaterializeError::Staging { path, .. } = result.unwrap_err() else {
        panic!("stage not reported");
    };
    assert_eq!(fs::read(path.join("a")).unwrap(), b"approved");
}

#[test]
fn a_staging_ancestor_replaced_with_a_symlink_is_never_followed() {
    struct ReplaceAncestor {
        store: MemBlobs,
        parent: std::path::PathBuf,
        outside: std::path::PathBuf,
        calls: usize,
    }
    impl BlobSource for ReplaceAncestor {
        fn fetch(&mut self, hash: &BlobHash) -> io::Result<Option<Vec<u8>>> {
            self.calls += 1;
            if self.calls == 2 {
                let stage = fs::read_dir(&self.parent)?
                    .map(|item| item.unwrap().path())
                    .find(|path| {
                        path.file_name()
                            .unwrap()
                            .to_string_lossy()
                            .starts_with(".locust-apply-materialize-")
                    })
                    .unwrap();
                fs::rename(stage.join("nested"), stage.join("original-nested"))?;
                std::os::unix::fs::symlink(&self.outside, stage.join("nested"))?;
            }
            self.store.fetch(hash)
        }
    }
    let parent = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("keep"), b"private").unwrap();
    let mut store = MemBlobs::default();
    let manifest = manifest_of(
        &mut store,
        &[("nested/a", b"A", false), ("nested/b", b"B", false)],
    );
    let result = materialize(
        &manifest,
        &mut ReplaceAncestor {
            store,
            parent: parent.path().into(),
            outside: outside.path().into(),
            calls: 0,
        },
        &parent.path().join("out"),
    );
    assert!(result.is_err());
    assert!(!parent.path().join("out").exists());
    assert_eq!(list_dir(outside.path()), ["keep"]);
    assert_eq!(fs::read(outside.path().join("keep")).unwrap(), b"private");
}
