//! Optional macOS syscall-level recovery regression. Portable failure/order
//! coverage also lives in the library's unit tests. Build the retained
//! research/evidence/t1-candidate-review/verify-store-flush-interposer.c helper
//! and pass its absolute dylib path as LOCUST_STORE_TEST_INTERPOSER:
//!
//! cargo test -p locust-store --test recovery_flush recovery_flushes -- --ignored
//!
//! The helper exits only the owned writer child and fails flushes only in the
//! owned recovery child. This tests real SQLite syscall ordering, not power loss.

#![cfg(target_os = "macos")]

use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use locust_proto::event::Body;
use locust_proto::store::{Blob, Commit, Store};
use locust_proto::testkit::Author;
use locust_store::{INLINE_MAX_BYTES, SqliteStore};

const DIR: &str = "LOCUST_STORE_RECOVERY_DIR";
const STEP: &str = "LOCUST_STORE_RECOVERY_STEP";
const ARM: &str = "LOCUST_STORE_RECOVERY_ARM";

fn blob() -> Blob {
    Blob::new(vec![7; INLINE_MAX_BYTES + 1])
}

#[test]
#[ignore = "invoked by recovery_flushes as an owned subprocess"]
fn recovery_child() {
    let Ok(step) = env::var(STEP) else { return };
    let dir = env::var(DIR).unwrap();
    let arm = env::var(ARM).unwrap();
    if step == "write" {
        let mut store = SqliteStore::open(&dir).unwrap();
        let mut author = Author::new(1);
        let genesis = author.genesis(locust_proto::testkit::keypair(9).public());
        let goal = genesis.header().goal;
        store
            .commit(&Commit {
                events: vec![genesis.clone()],
                blobs: vec![blob()],
                ..Commit::default()
            })
            .unwrap();
        let note = author.event(
            goal,
            Some(genesis.id()),
            Body::ContributionPublished {
                context: locust_proto::event::Context {
                    scope: locust_proto::event::Scope::Goal,
                    round: genesis.id(),
                },
                attempt: None,
                sources: Vec::new(),
                artifacts: vec![],
            },
        );
        fs::write(&arm, b"armed").unwrap();
        let _ = store.commit(&Commit {
            events: vec![note],
            drop_blobs: vec![blob().hash()],
            ..Commit::default()
        });
        panic!("writer kill injection never fired");
    }
    if step == "fail-recovery" {
        let result = SqliteStore::open(&dir);
        assert!(
            result.is_err(),
            "open must fail when the recovered WAL cannot be synced"
        );
        assert!(
            Path::new(&dir)
                .join("blobs")
                .join(blob().hash().to_string())
                .exists(),
            "failed recovery must not collect a file based on unflushed metadata"
        );
        assert!(
            Path::new(&format!("{arm}.fired")).exists(),
            "recovery fault never fired"
        );
    } else {
        assert_eq!(step, "read");
        let store = SqliteStore::open(&dir).unwrap();
        eprintln!("[recovery-test] open returned");
        let goal = Author::new(1)
            .genesis(locust_proto::testkit::keypair(9).public())
            .header()
            .goal;
        assert_eq!(store.log(&goal, 0, 10).unwrap().len(), 2);
        assert_eq!(store.blob_len(&blob().hash()), Ok(None));
        assert!(
            !Path::new(&dir)
                .join("blobs")
                .join(blob().hash().to_string())
                .exists()
        );
    }
}

fn child(dir: &Path, arm: &Path, step: &str, fault: Option<&str>) -> Output {
    let dylib = env::var("LOCUST_STORE_TEST_INTERPOSER")
        .expect("build verify-store-flush-interposer.c and set LOCUST_STORE_TEST_INTERPOSER");
    let mut command = Command::new(env::current_exe().unwrap());
    command
        .args([
            "recovery_child",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(DIR, dir)
        .env(STEP, step)
        .env(ARM, arm)
        .env("DYLD_INSERT_LIBRARIES", dylib)
        .env("PROBE_TRACE", "1")
        .env_remove("PROBE_KILL_SUB")
        .env_remove("PROBE_KILL_ARM")
        .env_remove("PROBE_FAIL_SUB")
        .env_remove("PROBE_FAIL_ARM");
    if let Some(fault) = fault {
        command
            .env(format!("PROBE_{fault}_SUB"), "locust.db-wal")
            .env(format!("PROBE_{fault}_ARM"), arm);
    }
    command.output().unwrap()
}

#[test]
#[ignore = "requires the macOS verify-store flush interposer; see module documentation"]
fn recovery_flushes_before_exposure_and_refuses_a_failed_barrier() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("state");
    let arm = root.path().join("fault");
    let fired = root.path().join("fault.fired");
    let writer = child(&dir, &arm, "write", Some("KILL"));
    assert_eq!(
        writer.status.code(),
        Some(77),
        "{}",
        String::from_utf8_lossy(&writer.stderr)
    );
    assert!(
        String::from_utf8_lossy(&writer.stderr)
            .contains("[verify-store] injected exit 77 before F_FULLFSYNC")
    );
    assert!(fired.exists());
    fs::remove_file(&fired).unwrap();

    let failed = child(&dir, &arm, "fail-recovery", Some("FAIL"));
    assert!(
        failed.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&failed.stdout),
        String::from_utf8_lossy(&failed.stderr)
    );
    assert!(
        String::from_utf8_lossy(&failed.stderr)
            .contains("[verify-store] injected EIO before F_FULLFSYNC")
    );
    assert!(fired.exists());
    fs::remove_file(&arm).unwrap();
    fs::remove_file(&fired).unwrap();

    let reader = child(&dir, &arm, "read", None);
    assert!(
        reader.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&reader.stdout),
        String::from_utf8_lossy(&reader.stderr)
    );
    let stderr = String::from_utf8_lossy(&reader.stderr);
    let flush = stderr
        .lines()
        .position(|line| line.contains("F_FULLFSYNC") && line.ends_with("locust.db-wal -> 0"))
        .expect("recovered WAL must be explicitly synced");
    let returned = stderr
        .lines()
        .position(|line| line == "[recovery-test] open returned")
        .unwrap();
    let collected = stderr
        .lines()
        .position(|line| line.contains("unlink") && line.contains(&blob().hash().to_string()))
        .expect("the recovered deletion must collect the old object");
    assert!(flush < collected && collected < returned, "{stderr}");
}
