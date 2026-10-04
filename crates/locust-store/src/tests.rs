use std::cell::Cell;

use locust_proto::store::conformance;
use rusqlite::Connection;
use tempfile::TempDir;

use crate::{INLINE_MAX_BYTES, OpenError, SqliteStore};

fn scratch() -> (TempDir, SqliteStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path()).unwrap();
    (dir, store)
}

#[test]
fn the_store_meets_the_contract() {
    let root = tempfile::tempdir().unwrap();
    let next = Cell::new(0);
    conformance::run(|| {
        next.set(next.get() + 1);
        SqliteStore::open(root.path().join(next.get().to_string())).unwrap()
    });
}

#[test]
fn a_reopened_store_meets_the_contract() {
    let dir = tempfile::tempdir().unwrap();
    conformance::run_reopen(|| SqliteStore::open(dir.path()).unwrap());
}

#[test]
fn the_store_can_move_to_its_own_thread() {
    fn owned_by_one_thread<T: Send + 'static>() {}
    owned_by_one_thread::<SqliteStore>();
}

#[test]
fn a_second_open_of_one_state_directory_fails_until_the_first_is_dropped() {
    let (dir, store) = scratch();
    let second = SqliteStore::open(dir.path());
    assert_eq!(
        second.unwrap_err(),
        OpenError::InUse(dir.path().to_path_buf())
    );
    drop(store);
    assert!(SqliteStore::open(dir.path()).is_ok());
}

#[test]
fn a_database_from_a_newer_schema_is_refused() {
    let (dir, store) = scratch();
    drop(store);
    let raw = Connection::open(dir.path().join("locust.db")).unwrap();
    raw.pragma_update(None, "user_version", 2).unwrap();
    drop(raw);
    assert_eq!(
        SqliteStore::open(dir.path()).unwrap_err(),
        OpenError::NewerSchema { found: 2, known: 1 }
    );
}

#[test]
fn the_schema_states_the_inline_limit_the_code_uses() {
    assert!(crate::schema::V1.contains(&format!("len > {INLINE_MAX_BYTES} ")));
}
