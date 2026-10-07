use std::cell::Cell;
use std::fmt::Debug;
use std::fs;
use std::os::unix::fs::{FileExt, PermissionsExt};
use std::path::{Path, PathBuf};

use locust_proto::PROTOCOL_VERSION;
use locust_proto::crypto::content_hash;
use locust_proto::event::{AuthorPoint, Body, Context, Event, Header, PayloadRef, Scope};
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey};
use locust_proto::limits::{
    BLOB_CHUNK_BYTES, MAX_ARTIFACTS, MAX_EVENTS_PER_BATCH, MAX_PARENTS, MAX_PAYLOAD_BYTES,
};
use locust_proto::local::marks_dir;
use locust_proto::store::{
    Blob, Commit, FileId, LocalWrite, Mark, MarkWrite, Marks, Space, Store, StoreError, conformance,
};
use locust_proto::testkit::{Author, keypair};
use rusqlite::{Connection, params};
use tempfile::TempDir;

use crate::{INLINE_MAX_BYTES, OpenError, SqliteStore};

/// A state directory and its marks directory, side by side in one temporary
/// directory that removes both.
struct Scratch {
    data: PathBuf,
    marks: PathBuf,
    root: TempDir,
}

impl Scratch {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("state");
        Self {
            marks: marks_dir(&data),
            data,
            root,
        }
    }

    fn path(&self) -> &Path {
        &self.data
    }

    fn open(&self) -> Result<SqliteStore, OpenError> {
        SqliteStore::open(&self.data, &self.marks)
    }

    fn marks_file(&self) -> PathBuf {
        self.marks.join("marks")
    }
}

fn scratch() -> (Scratch, SqliteStore) {
    let dir = Scratch::new();
    let store = dir.open().unwrap();
    (dir, store)
}

fn reopen(dir: &Scratch) -> SqliteStore {
    dir.open().unwrap()
}

/// A connection that bypasses the store, for damaging its files on purpose.
/// The store must be closed.
fn raw(dir: &Scratch) -> Connection {
    Connection::open(dir.path().join("locust.db")).unwrap()
}

fn contribution() -> Body {
    Body::ContributionPublished {
        context: Context {
            scope: Scope::Goal,
            round: EventId([0; 32]),
        },
        attempt: None,
        sources: Vec::new(),
        artifacts: vec![],
    }
}

fn put(space: Space, key: &[u8], value: &[u8]) -> LocalWrite {
    LocalWrite::Put {
        space,
        key: key.to_vec(),
        value: value.to_vec(),
    }
}

/// An object of `len` bytes whose content differs from other lengths'.
fn object(len: usize) -> Blob {
    Blob::new((0..len).map(|n| (n % 251) as u8 ^ len as u8).collect())
}

/// The names in the object directory, sorted.
fn object_files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir.join("blobs"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

fn is_corrupted<T: Debug>(result: Result<T, StoreError>) -> bool {
    matches!(result, Err(StoreError::Corrupted(_)))
}

fn positions(log: &[(u64, Event)]) -> Vec<u64> {
    log.iter().map(|(position, _)| *position).collect()
}

#[test]
fn the_store_meets_the_contract() {
    let root = tempfile::tempdir().unwrap();
    let next = Cell::new(0);
    conformance::run(|| {
        next.set(next.get() + 1);
        let dir = root.path().join(next.get().to_string());
        SqliteStore::open(&dir, &marks_dir(&dir)).unwrap()
    });
}

#[test]
fn a_reopened_store_meets_the_contract() {
    let dir = Scratch::new();
    conformance::run_reopen(|| dir.open().unwrap());
}

#[test]
fn the_store_can_move_to_its_own_thread() {
    fn owned_by_one_thread<T: Send + 'static>() {}
    owned_by_one_thread::<SqliteStore>();
}

#[test]
fn a_second_open_of_one_state_directory_fails_until_the_first_is_dropped() {
    let (dir, store) = scratch();
    let second = dir.open();
    assert_eq!(
        second.unwrap_err(),
        OpenError::InUse(dir.path().to_path_buf())
    );
    drop(store);
    assert!(dir.open().is_ok());
}

#[test]
fn the_database_is_opened_for_power_loss_durability_under_an_exclusive_lock() {
    let (dir, mut store) = scratch();
    store
        .commit(&Commit {
            local: vec![put(Space::Goal, b"g", b"1")],
            ..Commit::default()
        })
        .unwrap();
    let pragma = |name: &str| -> String {
        store
            .connection()
            .pragma_query_value(None, name, |row| row.get::<_, rusqlite::types::Value>(0))
            .map(|value| format!("{value:?}"))
            .unwrap()
    };
    assert_eq!(pragma("journal_mode"), r#"Text("wal")"#);
    assert_eq!(pragma("synchronous"), "Integer(2)", "FULL");
    assert_eq!(pragma("fullfsync"), "Integer(1)");
    assert_eq!(pragma("locking_mode"), r#"Text("exclusive")"#);
    // The WAL index lives in process memory: no shared-memory file.
    assert!(dir.path().join("locust.db-wal").exists());
    assert!(!dir.path().join("locust.db-shm").exists());
}

#[test]
fn every_unsupported_schema_is_refused_without_mutating_state() {
    for version in (-1..crate::schema::VERSION).chain([crate::schema::VERSION + 1, 999]) {
        let dir = Scratch::new();
        fs::create_dir(dir.path()).unwrap();
        let path = dir.path().join("locust.db");
        let raw = Connection::open(&path).unwrap();
        raw.execute_batch(
            "CREATE TABLE preserved(value TEXT); INSERT INTO preserved VALUES ('untouched');",
        )
        .unwrap();
        raw.pragma_update(None, "user_version", version).unwrap();
        drop(raw);
        let before = fs::read(&path).unwrap();
        assert_eq!(
            dir.open().unwrap_err(),
            OpenError::UnsupportedSchema {
                found: version,
                known: crate::schema::VERSION
            }
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(
            fs::read_dir(dir.path()).unwrap().count(),
            1,
            "refusal created files for schema {version}"
        );
        assert!(!dir.marks.exists());
    }
}

#[test]
fn fresh_initialization_is_atomic_and_current_state_reopens() {
    let mut conn = Connection::open_in_memory().unwrap();
    // Allow the schema root and first table, then exhaust the database pages.
    // SQLite must roll back the earlier table and leave the marker uncommitted.
    conn.pragma_update(None, "max_page_count", 2).unwrap();
    assert!(crate::schema::initialize(&mut conn).is_err());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM sqlite_schema", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    conn.pragma_update(None, "max_page_count", 100).unwrap();
    crate::schema::initialize(&mut conn).unwrap();
    assert_eq!(
        conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        crate::schema::VERSION
    );
    let (dir, mut store) = scratch();
    store
        .commit(&Commit {
            local: vec![put(Space::Identity, b"persisted", b"current")],
            ..Commit::default()
        })
        .unwrap();
    drop(store);
    let store = reopen(&dir);
    assert_eq!(
        store.get(Space::Identity, b"persisted").unwrap(),
        Some(b"current".to_vec())
    );
}

#[test]
fn the_schema_states_the_inline_limit_the_code_uses() {
    assert!(crate::schema::CURRENT.contains(&format!("len > {INLINE_MAX_BYTES} ")));
}

#[test]
fn a_commit_that_fails_part_way_leaves_nothing() {
    let (dir, mut store) = scratch();
    let mut owner = Author::new(1);
    let genesis = owner.genesis(locust_proto::testkit::keypair(9).public());
    let goal = genesis.header().goal;
    let first = owner.event(goal, Some(genesis.id()), contribution());
    let small = object(100);
    let large = object(INLINE_MAX_BYTES + 1);
    // Refuses one local write, after the commit's events and objects are in.
    store
        .connection()
        .execute_batch(
            "CREATE TEMP TRIGGER refuse BEFORE INSERT ON local \
             WHEN NEW.key = CAST('refuse' AS BLOB) \
             BEGIN SELECT RAISE(ABORT, 'refused by the test'); END;",
        )
        .unwrap();
    let failing = Commit {
        events: vec![genesis.clone(), first.clone()],
        blobs: vec![small.clone(), large.clone()],
        local: vec![
            put(Space::Goal, b"first", b"1"),
            put(Space::Goal, b"refuse", b""),
        ],
        marks: Vec::new(),
    };

    assert!(matches!(store.commit(&failing), Err(StoreError::Failed(_))));
    assert_eq!(store.goals(), Ok(Vec::new()));
    assert_eq!(store.has_event(&genesis.id()), Ok(false));
    assert_eq!(store.event(&first.id()), Ok(None));
    assert_eq!(store.log(&goal, 0, 10), Ok(Vec::new()));
    assert_eq!(store.blob_len(&small.hash()), Ok(None));
    assert_eq!(store.blob_len(&large.hash()), Ok(None));
    assert_eq!(store.get(Space::Goal, b"first"), Ok(None));
    // The large object's file went in before the transaction failed.
    assert_eq!(object_files(dir.path()), [large.hash().to_string()]);

    // The store carries on, and the failed commit took no position.
    let second = owner.event(goal, Some(genesis.id()), contribution());
    store
        .commit(&Commit {
            events: vec![genesis.clone(), second.clone()],
            ..Commit::default()
        })
        .unwrap();
    assert_eq!(store.log(&goal, 0, 10), Ok(vec![(1, genesis), (2, second)]));
    // The file no row names is removed on the next open.
    drop(store);
    let store = reopen(&dir);
    assert_eq!(object_files(dir.path()), Vec::<String>::new());
    assert_eq!(store.blob_len(&large.hash()), Ok(None));
}

#[test]
fn a_failure_while_committing_stops_the_store_until_it_is_reopened() {
    let (dir, mut store) = scratch();
    // A deferred foreign key is checked by COMMIT itself, so this commit
    // fails at the step whose outcome a real I/O error would leave unknown.
    store
        .connection()
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TEMP TABLE parent (id INTEGER PRIMARY KEY);
             CREATE TEMP TABLE child (
                 parent INTEGER REFERENCES parent (id) DEFERRABLE INITIALLY DEFERRED
             );
             CREATE TEMP TRIGGER fail_at_commit AFTER INSERT ON local
             WHEN NEW.key = CAST('fail at commit' AS BLOB)
             BEGIN INSERT INTO child VALUES (1); END;",
        )
        .unwrap();
    let kept = put(Space::Goal, b"kept", b"1");
    store
        .commit(&Commit {
            local: vec![kept],
            ..Commit::default()
        })
        .unwrap();

    let failing = Commit {
        local: vec![put(Space::Goal, b"fail at commit", b"")],
        ..Commit::default()
    };
    assert!(matches!(store.commit(&failing), Err(StoreError::Failed(_))));
    assert!(matches!(
        store.get(Space::Goal, b"kept"),
        Err(StoreError::Failed(_))
    ));
    assert!(matches!(
        store.commit(&Commit::default()),
        Err(StoreError::Failed(_))
    ));

    drop(store);
    let store = reopen(&dir);
    assert_eq!(store.get(Space::Goal, b"kept"), Ok(Some(b"1".to_vec())));
    assert_eq!(store.get(Space::Goal, b"fail at commit"), Ok(None));
}

#[test]
fn a_damaged_event_row_is_reported_as_corrupted_never_as_another_event() {
    let (dir, mut store) = scratch();
    let mut owner = Author::new(1);
    let genesis = owner.genesis(locust_proto::testkit::keypair(9).public());
    let goal = genesis.header().goal;
    let author = owner.key.public();
    let first = owner.event(goal, Some(genesis.id()), contribution());
    let second = owner.event(goal, Some(genesis.id()), contribution());
    store
        .commit(&Commit {
            events: vec![genesis.clone(), first.clone(), second.clone()],
            ..Commit::default()
        })
        .unwrap();
    drop(store);

    // `first` now carries the header of `second`.
    raw(&dir)
        .execute(
            "UPDATE events SET header = (SELECT header FROM events WHERE id = ?2) WHERE id = ?1",
            params![first.id().as_bytes(), second.id().as_bytes()],
        )
        .unwrap();
    let store = reopen(&dir);
    assert!(is_corrupted(store.event(&first.id())));
    assert!(is_corrupted(store.log(&goal, 0, 10)));
    assert!(is_corrupted(store.author_log(&goal, &author, None, 10)));
    // Rows around it still read, and presence is answered from the index.
    assert_eq!(store.event(&second.id()), Ok(Some(second.clone())));
    assert_eq!(store.log(&goal, 0, 1), Ok(vec![(1, genesis.clone())]));
    assert_eq!(store.has_event(&first.id()), Ok(true));
    drop(store);

    // `second` is now indexed under another goal and another position.
    let other = GoalId([9; 32]);
    raw(&dir)
        .execute(
            "UPDATE events SET goal = ?2, seq = 7 WHERE id = ?1",
            params![second.id().as_bytes(), other.as_bytes()],
        )
        .unwrap();
    let store = reopen(&dir);
    assert!(is_corrupted(store.log(&other, 0, 10)));
    assert!(is_corrupted(store.author_log(&other, &author, None, 10)));
}

#[test]
fn a_missing_or_truncated_object_file_is_reported_as_corrupted() {
    let (dir, mut store) = scratch();
    let missing = object(INLINE_MAX_BYTES + 1);
    let truncated = object(INLINE_MAX_BYTES + 2);
    store
        .commit(&Commit {
            blobs: vec![missing.clone(), truncated.clone()],
            ..Commit::default()
        })
        .unwrap();
    let blobs = dir.path().join("blobs");
    fs::remove_file(blobs.join(missing.hash().to_string())).unwrap();
    fs::File::options()
        .write(true)
        .open(blobs.join(truncated.hash().to_string()))
        .unwrap()
        .set_len(10)
        .unwrap();

    assert!(is_corrupted(store.blob(&missing.hash())));
    assert!(is_corrupted(store.blob_range(&missing.hash(), 0, 4)));
    assert!(is_corrupted(store.blob(&truncated.hash())));
    assert!(is_corrupted(store.blob_range(&truncated.hash(), 0, 4)));
}

#[test]
fn objects_above_the_inline_limit_are_files_served_by_range() {
    let (dir, mut store) = scratch();
    let at_limit = object(INLINE_MAX_BYTES);
    let above = object(INLINE_MAX_BYTES + 1);
    let large = object(3 * BLOB_CHUNK_BYTES + 5);
    store
        .commit(&Commit {
            blobs: vec![
                at_limit.clone(),
                above.clone(),
                large.clone(),
                above.clone(),
            ],
            ..Commit::default()
        })
        .unwrap();

    // Only the objects above the limit are files.
    assert_eq!(
        object_files(dir.path()),
        sorted(vec![above.hash().to_string(), large.hash().to_string()])
    );
    for blob in [&at_limit, &above, &large] {
        assert_eq!(store.blob(&blob.hash()), Ok(Some(blob.bytes().to_vec())));
        assert_eq!(
            store.blob_len(&blob.hash()),
            Ok(Some(blob.bytes().len() as u64))
        );
    }

    let bytes = large.bytes();
    let len = bytes.len();
    let chunk = BLOB_CHUNK_BYTES;
    let range = |offset: usize, max: usize| {
        store
            .blob_range(&large.hash(), offset as u64, max)
            .unwrap()
            .unwrap()
    };
    assert_eq!(range(0, 1), bytes[..1]);
    assert_eq!(range(0, chunk), bytes[..chunk]);
    assert_eq!(range(chunk - 3, chunk), bytes[chunk - 3..2 * chunk - 3]);
    assert_eq!(range(3 * chunk, chunk), bytes[3 * chunk..]);
    assert_eq!(range(len - 1, 10), bytes[len - 1..]);
    assert_eq!(range(len, 10), Vec::<u8>::new());
    assert_eq!(range(len + 1, 10), Vec::<u8>::new());
    assert_eq!(range(0, usize::MAX), bytes);
    assert_eq!(
        store.blob_range(&large.hash(), u64::MAX, usize::MAX),
        Ok(Some(Vec::new()))
    );
    assert_eq!(
        store.blob_range(&at_limit.hash(), INLINE_MAX_BYTES as u64 - 1, 10),
        Ok(Some(at_limit.bytes()[INLINE_MAX_BYTES - 1..].to_vec()))
    );

    // Every object survives reopen.
    drop(store);
    let store = reopen(&dir);
    for blob in [&at_limit, &above, &large] {
        assert_eq!(store.blob(&blob.hash()), Ok(Some(blob.bytes().to_vec())));
    }
}

#[test]
fn staging_a_large_object_resumes_after_reopen_and_promotes_it_durably() {
    let (dir, mut store) = scratch();
    let large = object(2 * BLOB_CHUNK_BYTES + 3);
    let (hash, bytes, chunk) = (large.hash(), large.bytes(), BLOB_CHUNK_BYTES);
    assert_eq!(
        store.stage_blob(&hash, 0, &bytes[..chunk]),
        Ok(chunk as u64)
    );
    assert_eq!(
        store.stage_blob(&hash, 5, &bytes[5..chunk]),
        Ok(chunk as u64)
    );
    drop(store);

    let mut store = reopen(&dir);
    assert_eq!(store.staged_len(&hash), Ok(chunk as u64));
    assert_eq!(store.blob_len(&hash), Ok(None));
    for start in (chunk..bytes.len()).step_by(chunk) {
        let end = (start + chunk).min(bytes.len());
        assert_eq!(
            store.stage_blob(&hash, start as u64, &bytes[start..end]),
            Ok(end as u64)
        );
    }
    assert_eq!(object_files(dir.path()), [format!("{hash}.staged")]);
    assert_eq!(store.finish_blob(&hash), Ok(true));
    assert_eq!(object_files(dir.path()), [hash.to_string()]);
    assert_eq!(store.staged_len(&hash), Ok(0));
    assert_eq!(store.blob(&hash), Ok(Some(bytes.to_vec())));

    // Receiving it again leaves the held object as it was.
    assert_eq!(store.stage_blob(&hash, 0, bytes), Ok(bytes.len() as u64));
    assert_eq!(store.finish_blob(&hash), Ok(true));
    assert_eq!(object_files(dir.path()), [hash.to_string()]);
    drop(store);
    assert_eq!(reopen(&dir).blob(&hash), Ok(Some(bytes.to_vec())));
}

#[test]
fn a_large_staged_copy_that_does_not_match_its_hash_is_discarded() {
    let (dir, mut store) = scratch();
    let promised = object(INLINE_MAX_BYTES + 10).hash();
    let sent = object(INLINE_MAX_BYTES + 11);
    assert_eq!(
        store.stage_blob(&promised, 0, sent.bytes()),
        Ok(sent.bytes().len() as u64)
    );
    assert_eq!(store.finish_blob(&promised), Ok(false));
    assert_eq!(store.staged_len(&promised), Ok(0));
    assert_eq!(store.blob_len(&promised), Ok(None));
    assert_eq!(object_files(dir.path()), Vec::<String>::new());
}

#[test]
fn leftovers_in_the_object_directory_are_collected_on_open() {
    let (dir, mut store) = scratch();
    let held = object(INLINE_MAX_BYTES + 1).hash();
    let inline = object(10);
    store
        .commit(&Commit {
            blobs: vec![object(INLINE_MAX_BYTES + 1), inline.clone()],
            ..Commit::default()
        })
        .unwrap();
    drop(store);
    let orphan = object(INLINE_MAX_BYTES + 2).hash();
    let resumable = object(INLINE_MAX_BYTES + 3).hash();
    // Not a name this crate writes (it writes lowercase hex).
    let foreign = object(INLINE_MAX_BYTES + 4)
        .hash()
        .to_string()
        .to_uppercase();
    let blobs = dir.path().join("blobs");
    for name in [
        orphan.to_string(),
        format!("{orphan}.tmp"),
        format!("{held}.staged"),
        format!("{resumable}.staged"),
        inline.hash().to_string(),
        format!("{orphan}.other"),
        foreign.clone(),
        "notes.txt".to_owned(),
    ] {
        fs::write(blobs.join(name), b"leftover").unwrap();
    }

    let store = reopen(&dir);
    assert_eq!(
        object_files(dir.path()),
        sorted(vec![
            held.to_string(),
            format!("{held}.staged"),
            format!("{resumable}.staged"),
            format!("{orphan}.other"),
            foreign.clone(),
            "notes.txt".to_owned(),
        ])
    );
    assert_eq!(store.staged_len(&resumable), Ok(8));
    assert_eq!(store.staged_len(&held), Ok(8));
    assert_eq!(
        store.blob(&inline.hash()),
        Ok(Some(inline.bytes().to_vec()))
    );
}

#[test]
fn startup_collection_keeps_unreferenced_durable_content_and_interrupted_transfers() {
    let (dir, mut store) = scratch();
    let retained = object(INLINE_MAX_BYTES + 101);
    let transfer = object(3 * BLOB_CHUNK_BYTES + 7);
    let chunk = BLOB_CHUNK_BYTES;
    // The store does not infer eviction from local policy metadata. Removing
    // a pointer (or recording withdrawal/leave above this layer) cannot erase
    // an acknowledged object or an independently durable transfer prefix.
    store
        .commit(&Commit {
            blobs: vec![retained.clone()],
            local: vec![put(Space::Blob, b"policy", retained.hash().as_bytes())],
            ..Commit::default()
        })
        .unwrap();
    store
        .commit(&Commit {
            local: vec![LocalWrite::Delete {
                space: Space::Blob,
                key: b"policy".to_vec(),
            }],
            ..Commit::default()
        })
        .unwrap();
    store
        .stage_blob(&transfer.hash(), 0, &transfer.bytes()[..chunk])
        .unwrap();
    drop(store);

    let orphan = object(INLINE_MAX_BYTES + 102);
    fs::write(
        dir.path().join("blobs").join(orphan.hash().to_string()),
        orphan.bytes(),
    )
    .unwrap();
    let mut store = reopen(&dir);
    assert_eq!(store.get(Space::Blob, b"policy").unwrap(), None);
    assert_eq!(
        store.blob(&retained.hash()).unwrap(),
        Some(retained.bytes().to_vec())
    );
    assert_eq!(store.staged_len(&transfer.hash()).unwrap(), chunk as u64);
    assert!(
        !dir.path()
            .join("blobs")
            .join(orphan.hash().to_string())
            .exists()
    );
    for start in (chunk..transfer.bytes().len()).step_by(chunk) {
        let end = (start + chunk).min(transfer.bytes().len());
        store
            .stage_blob(
                &transfer.hash(),
                start as u64,
                &transfer.bytes()[start..end],
            )
            .unwrap();
    }
    assert!(store.finish_blob(&transfer.hash()).unwrap());
    drop(store);
    let store = reopen(&dir);
    assert_eq!(
        store.blob(&retained.hash()).unwrap(),
        Some(retained.bytes().to_vec())
    );
    assert_eq!(
        store.blob(&transfer.hash()).unwrap(),
        Some(transfer.bytes().to_vec())
    );
}

/// The largest header the contract admits: every list at its limit and
/// every integer at its widest encoding.
fn largest_header(n: usize, prev: EventId) -> Header {
    Header {
        version: PROTOCOL_VERSION,
        goal: GoalId([7; 32]),
        author: keypair(1).public(),
        seq: i64::MAX as u64 - (MAX_EVENTS_PER_BATCH - 1 - n) as u64,
        prev: Some(prev),
        anchor: Some(EventId([0xaa; 32])),
        parents: (0..MAX_PARENTS).map(|p| EventId([p as u8; 32])).collect(),
        at_ms: u64::MAX,
        payload: Some(PayloadRef {
            hash: BlobHash([0xbb; 32]),
            len: MAX_PAYLOAD_BYTES as u32,
            key_epoch: u32::MAX,
        }),
        body: Body::ContributionPublished {
            sources: vec![],
            context: Context {
                scope: Scope::Goal,
                round: EventId([0xaa; 32]),
            },
            attempt: Some(EventId([0xcc; 32])),
            artifacts: (0..MAX_ARTIFACTS)
                .map(|a| BlobHash([a as u8; 32]))
                .collect(),
        },
    }
}

#[test]
fn a_batch_of_256_largest_headers_commits_in_one_step() {
    let (dir, mut store) = scratch();
    let key = keypair(1);
    let mut batch: Vec<Event> = Vec::with_capacity(MAX_EVENTS_PER_BATCH);
    let mut prev = EventId([0x11; 32]);
    for n in 0..MAX_EVENTS_PER_BATCH {
        let event = Event::sign(largest_header(n, prev), &key).unwrap();
        prev = event.id();
        batch.push(event);
    }
    assert!(batch.iter().all(|event| event.header_bytes().len() > 4096));
    let goal = GoalId([7; 32]);
    store
        .commit(&Commit {
            events: batch.clone(),
            ..Commit::default()
        })
        .unwrap();
    drop(store);

    let store = reopen(&dir);
    let log = store.log(&goal, 0, usize::MAX).unwrap();
    assert_eq!(
        positions(&log),
        (1..=MAX_EVENTS_PER_BATCH as u64).collect::<Vec<_>>()
    );
    assert!(log.iter().map(|(_, event)| event).eq(batch.iter()));
    // Paged by author cursor up to the largest sequence number.
    let author = key.public();
    let mut paged = Vec::new();
    let mut after = None;
    loop {
        let page = store.author_log(&goal, &author, after, 100).unwrap();
        let Some(last) = page.last() else { break };
        after = Some(AuthorPoint {
            seq: last.header().seq,
            id: last.id(),
        });
        paged.extend(page);
    }
    assert_eq!(paged, batch);
    assert_eq!(after.map(|point| point.seq), Some(i64::MAX as u64));
}

#[test]
fn recovery_barrier_failure_prevents_open_and_garbage_collection() {
    use crate::faults::{self, Point};
    let (dir, store) = scratch();
    drop(store);
    let orphan = dir.path().join("blobs").join(object(42).hash().to_string());
    fs::write(&orphan, b"orphan").unwrap();
    faults::arm(Point::Recovery, Path::new(""));
    assert!(matches!(
        dir.open(),
        Err(OpenError::Store(StoreError::Failed(_)))
    ));
    faults::assert_fired();
    assert!(orphan.exists(), "failed recovery must not collect files");
    let store = reopen(&dir);
    assert!(!orphan.exists());
    assert_eq!(store.goals(), Ok(Vec::new()));
}

#[test]
fn a_failed_staging_flush_is_fenced_and_repaired_before_reopen_succeeds() {
    use crate::faults::{self, Point};
    let (dir, mut store) = scratch();
    let blob = object(INLINE_MAX_BYTES + 17);
    let hash = blob.hash();
    let path = dir.path().join("blobs").join(format!("{hash}.staged"));
    store.stage_blob(&hash, 0, &blob.bytes()[..10]).unwrap();
    faults::arm(Point::FileSync, &path);
    assert!(matches!(
        store.stage_blob(&hash, 10, &blob.bytes()[10..]),
        Err(StoreError::Failed(_))
    ));
    faults::assert_fired();
    assert!(matches!(
        store.finish_blob(&hash),
        Err(StoreError::Failed(_))
    ));
    assert!(matches!(
        store.staged_len(&hash),
        Err(StoreError::Failed(_))
    ));
    drop(store);

    faults::arm(Point::FileSync, &path);
    assert!(matches!(
        dir.open(),
        Err(OpenError::Store(StoreError::Failed(_)))
    ));
    faults::assert_fired();
    let mut store = reopen(&dir);
    let trace = faults::take_trace();
    assert!(trace.contains(&(Point::FileSync, path.clone())));
    assert!(trace.contains(&(Point::DirectorySync, dir.path().join("blobs"))));
    assert_eq!(store.staged_len(&hash), Ok(blob.bytes().len() as u64));
    assert_eq!(
        store.stage_blob(&hash, blob.bytes().len() as u64, &[]),
        Ok(blob.bytes().len() as u64)
    );
    faults::arm(Point::FileSync, &path);
    assert!(matches!(
        store.finish_blob(&hash),
        Err(StoreError::Failed(_))
    ));
    faults::assert_fired();
    assert!(matches!(store.blob_len(&hash), Err(StoreError::Failed(_))));
    drop(store);
    let mut store = reopen(&dir);
    assert_eq!(store.blob_len(&hash), Ok(None));
    assert_eq!(store.staged_len(&hash), Ok(blob.bytes().len() as u64));
    assert_eq!(store.finish_blob(&hash), Ok(true));
    assert_eq!(store.blob(&hash), Ok(Some(blob.bytes().to_vec())));
}

#[test]
fn uncertain_stage_and_discard_directory_updates_require_recovery() {
    use crate::faults::{self, Point};
    let (dir, mut store) = scratch();
    let blob = object(31);
    let hash = blob.hash();
    let blobs = dir.path().join("blobs");
    faults::arm(Point::DirectorySync, &blobs);
    assert!(matches!(
        store.stage_blob(&hash, 0, blob.bytes()),
        Err(StoreError::Failed(_))
    ));
    faults::assert_fired();
    assert!(matches!(
        store.stage_blob(&hash, 0, blob.bytes()),
        Err(StoreError::Failed(_))
    ));
    drop(store);

    faults::arm(Point::DirectorySync, &blobs);
    assert!(matches!(
        dir.open(),
        Err(OpenError::Store(StoreError::Failed(_)))
    ));
    faults::assert_fired();
    let mut store = reopen(&dir);
    assert_eq!(store.stage_blob(&hash, 0, blob.bytes()), Ok(31));
    faults::arm(Point::DirectorySync, &blobs);
    assert!(matches!(
        store.discard_staged_blob(&hash),
        Err(StoreError::Failed(_))
    ));
    faults::assert_fired();
    assert!(matches!(
        store.discard_staged_blob(&hash),
        Err(StoreError::Failed(_))
    ));
    drop(store);

    faults::arm(Point::DirectorySync, &blobs);
    assert!(dir.open().is_err());
    faults::assert_fired();
    let mut store = reopen(&dir);
    assert_eq!(store.discard_staged_blob(&hash), Ok(()));
    assert_eq!(store.staged_len(&hash), Ok(0));
    drop(store);
    assert_eq!(reopen(&dir).staged_len(&hash), Ok(0));
}

#[test]
fn promotion_preserves_staging_when_the_row_insert_fails() {
    let (dir, mut store) = scratch();
    let blob = object(INLINE_MAX_BYTES + 5);
    let hash = blob.hash();
    store.stage_blob(&hash, 0, blob.bytes()).unwrap();
    store
        .connection()
        .execute_batch(
            "CREATE TEMP TRIGGER refuse BEFORE INSERT ON blobs
         BEGIN SELECT RAISE(ABORT, 'injected promotion insert failure'); END;",
        )
        .unwrap();
    let error = store.finish_blob(&hash).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("injected promotion insert failure")
    );
    assert_eq!(store.staged_len(&hash), Ok(blob.bytes().len() as u64));
    assert_eq!(store.blob_len(&hash), Ok(None));
    drop(store);
    let mut store = reopen(&dir);
    assert_eq!(store.staged_len(&hash), Ok(blob.bytes().len() as u64));
    assert_eq!(store.finish_blob(&hash), Ok(true));
    assert_eq!(store.blob(&hash), Ok(Some(blob.bytes().to_vec())));
}

#[test]
fn promotion_preserves_staging_when_the_database_commit_fails() {
    let (dir, mut store) = scratch();
    let blob = object(INLINE_MAX_BYTES + 7);
    let hash = blob.hash();
    store.stage_blob(&hash, 0, blob.bytes()).unwrap();
    store
        .connection()
        .execute_batch(
            "PRAGMA foreign_keys = ON;
         CREATE TEMP TABLE parent (id INTEGER PRIMARY KEY);
         CREATE TEMP TABLE child (
             parent INTEGER REFERENCES parent (id) DEFERRABLE INITIALLY DEFERRED
         );
         CREATE TEMP TRIGGER fail_at_commit AFTER INSERT ON blobs
         BEGIN INSERT INTO child VALUES (1); END;",
        )
        .unwrap();
    let error = store.finish_blob(&hash).unwrap_err();
    assert!(error.to_string().contains("FOREIGN KEY constraint failed"));
    assert!(matches!(
        store.staged_len(&hash),
        Err(StoreError::Failed(_))
    ));
    drop(store);
    let mut store = reopen(&dir);
    assert_eq!(store.blob_len(&hash), Ok(None));
    assert_eq!(store.staged_len(&hash), Ok(blob.bytes().len() as u64));
    assert_eq!(store.finish_blob(&hash), Ok(true));
    assert_eq!(store.blob(&hash), Ok(Some(blob.bytes().to_vec())));
}

#[test]
fn promotion_file_and_directory_flush_failures_preserve_recoverable_staging() {
    use crate::faults::{self, Point};
    for point in [Point::FileSync, Point::DirectorySync] {
        let (dir, mut store) = scratch();
        let blob = object(INLINE_MAX_BYTES + 9);
        let hash = blob.hash();
        store.stage_blob(&hash, 0, blob.bytes()).unwrap();
        let target = match point {
            Point::FileSync => dir.path().join("blobs").join(format!("{hash}.tmp")),
            Point::DirectorySync => dir.path().join("blobs"),
            _ => unreachable!(),
        };
        faults::arm(point, &target);
        assert!(matches!(
            store.finish_blob(&hash),
            Err(StoreError::Failed(_))
        ));
        faults::assert_fired();
        assert!(matches!(store.blob_len(&hash), Err(StoreError::Failed(_))));
        drop(store);
        let mut store = reopen(&dir);
        assert_eq!(store.blob_len(&hash), Ok(None));
        assert_eq!(store.staged_len(&hash), Ok(blob.bytes().len() as u64));
        assert_eq!(store.finish_blob(&hash), Ok(true));
        assert_eq!(store.blob(&hash), Ok(Some(blob.bytes().to_vec())));
    }
}

#[test]
fn interrupted_promotion_cleanup_keeps_staging_independent_of_held_bytes() {
    use crate::faults::{self, Point};
    let (dir, mut store) = scratch();
    let blob = object(INLINE_MAX_BYTES + 11);
    let hash = blob.hash();
    let staged = dir.path().join("blobs").join(format!("{hash}.staged"));
    store.stage_blob(&hash, 0, blob.bytes()).unwrap();
    faults::arm(Point::Discard, &staged);
    assert!(matches!(
        store.finish_blob(&hash),
        Err(StoreError::Failed(_))
    ));
    faults::assert_fired();
    drop(store);
    let mut store = reopen(&dir);
    let len = blob.bytes().len() as u64;
    assert_eq!(store.staged_len(&hash), Ok(len));
    assert_eq!(store.blob(&hash), Ok(Some(blob.bytes().to_vec())));
    assert_eq!(store.stage_blob(&hash, len, b"extra"), Ok(len + 5));
    assert_eq!(store.blob(&hash), Ok(Some(blob.bytes().to_vec())));
    assert_eq!(store.finish_blob(&hash), Ok(false));
    assert_eq!(store.blob(&hash), Ok(Some(blob.bytes().to_vec())));
}

#[test]
fn redundant_staging_survives_reopen_for_inline_and_file_objects() {
    for len in [19, INLINE_MAX_BYTES + 1] {
        let (dir, mut store) = scratch();
        let blob = object(len);
        let hash = blob.hash();
        store
            .commit(&Commit {
                blobs: vec![blob.clone()],
                ..Commit::default()
            })
            .unwrap();
        store.stage_blob(&hash, 0, blob.bytes()).unwrap();
        drop(store);
        let mut store = reopen(&dir);
        assert_eq!(store.staged_len(&hash), Ok(len as u64));
        assert_eq!(store.finish_blob(&hash), Ok(true));
        assert_eq!(store.staged_len(&hash), Ok(0));
        assert_eq!(store.blob(&hash), Ok(Some(blob.bytes().to_vec())));
    }
}

#[test]
fn incompatible_event_protocol_refuses_open_before_collecting_or_rewriting_state() {
    let (dir, mut store) = scratch();
    let mut author = locust_proto::testkit::Author::new(71);
    let genesis = author.genesis(locust_proto::testkit::keypair(9).public());
    let held = object(INLINE_MAX_BYTES + 13);
    let pending = object(57);
    store
        .commit(&Commit {
            events: vec![genesis.clone()],
            blobs: vec![held.clone()],
            local: vec![LocalWrite::Put {
                space: Space::Identity,
                key: b"sentinel".to_vec(),
                value: b"identity".to_vec(),
            }],
            marks: Vec::new(),
        })
        .unwrap();
    store
        .stage_blob(&pending.hash(), 0, &pending.bytes()[..23])
        .unwrap();
    // Version is the first header byte. The compatibility check must run
    // before decoding or rewriting any row, including identity records.
    let mut old_header = genesis.header_bytes().to_vec();
    old_header[0] = locust_proto::PROTOCOL_VERSION.wrapping_sub(1);
    store
        .connection()
        .execute("UPDATE events SET header = ?1", [&old_header])
        .unwrap();
    drop(store);
    let orphan = dir
        .path()
        .join("blobs")
        .join(format!("{}.tmp", object(91).hash()));
    fs::write(&orphan, b"uncollected evidence").unwrap();
    let before = object_files(dir.path());
    assert!(
        matches!(dir.open(), Err(OpenError::UnsupportedProtocolVersion { found, known })
        if found == old_header[0] && known == locust_proto::PROTOCOL_VERSION)
    );
    assert_eq!(object_files(dir.path()), before);
    assert_eq!(fs::read(&orphan).unwrap(), b"uncollected evidence");
    assert_eq!(
        fs::read(dir.path().join("blobs").join(held.hash().to_string())).unwrap(),
        held.bytes()
    );
    assert_eq!(
        fs::read(
            dir.path()
                .join("blobs")
                .join(format!("{}.staged", pending.hash()))
        )
        .unwrap(),
        &pending.bytes()[..23]
    );
    let conn = rusqlite::Connection::open_with_flags(
        locust_proto::local::database_path(dir.path()),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let header: Vec<u8> = conn
        .query_row("SELECT header FROM events", [], |row| row.get(0))
        .unwrap();
    let identity: Vec<u8> = conn
        .query_row("SELECT value FROM local", [], |row| row.get(0))
        .unwrap();
    assert_eq!(header, old_header);
    assert_eq!(identity, b"identity");
}

#[test]
fn locked_connection_rechecks_event_protocol_after_preflight() {
    let (dir, mut store) = scratch();
    let mut author = locust_proto::testkit::Author::new(91);
    let genesis = author.genesis(locust_proto::testkit::keypair(9).public());
    let goal = genesis.header().goal;
    let first = author.event(goal, Some(genesis.id()), contribution());
    store
        .commit(&Commit {
            events: vec![genesis.clone(), first.clone()],
            ..Commit::default()
        })
        .unwrap();
    drop(store);
    let database = locust_proto::local::database_path(dir.path());
    crate::connection::preflight(&database, dir.path()).unwrap();
    // A change after the read-only preflight is caught by the locked check.
    let mut old_header = genesis.header_bytes().to_vec();
    old_header[0] = locust_proto::PROTOCOL_VERSION.wrapping_sub(1);
    raw(&dir)
        .execute("UPDATE events SET header = ?1", [&old_header])
        .unwrap();
    assert!(
        matches!(crate::connection::open(&database, dir.path()), Err(OpenError::UnsupportedProtocolVersion { found, known })
        if found == old_header[0] && known == locust_proto::PROTOCOL_VERSION)
    );
}

fn mark(goal: u8, key: u8, seq: u64) -> Mark {
    Mark {
        goal: GoalId([goal; 32]),
        key: PublicKey([key; 32]),
        point: AuthorPoint {
            seq,
            id: EventId([seq as u8; 32]),
        },
        shared: seq % 2 == 1,
        unheard: false,
    }
}

fn marking(writes: Vec<MarkWrite>) -> Commit {
    Commit {
        marks: writes,
        ..Commit::default()
    }
}

/// The steps of a fault trace that touched the marks directory.
fn marks_steps(dir: &Scratch) -> Vec<(crate::faults::Point, PathBuf)> {
    crate::faults::take_trace()
        .into_iter()
        .filter(|(_, path)| path.starts_with(&dir.marks))
        .collect()
}

/// Changes one byte of the last record in place, as a torn write leaves it.
fn tear_last_record(path: &Path) {
    let file = fs::OpenOptions::new().write(true).open(path).unwrap();
    let len = file.metadata().unwrap().len();
    file.write_all_at(&[0xa5], len - 40).unwrap();
}

/// Copies every file directly inside `from` into a new directory `to`.
fn copy_files(from: &Path, to: &Path) {
    fs::create_dir(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}

#[test]
fn marks_survive_reopen_and_a_torn_record_makes_them_lost() {
    let (dir, mut store) = scratch();
    let (first, second, raised) = (mark(1, 1, 0), mark(1, 2, 3), mark(1, 1, 1));
    store
        .commit(&marking(vec![
            MarkWrite::Set(second),
            MarkWrite::Set(first),
        ]))
        .unwrap();
    let file = store.marks().unwrap().file;
    drop(store);
    assert_eq!(
        reopen(&dir).marks(),
        Ok(Marks {
            file,
            kept: Some(vec![first, second]),
        })
    );

    tear_last_record(&dir.marks_file());
    let mut store = reopen(&dir);
    assert_eq!(store.marks(), Ok(Marks { file, kept: None }));
    // The next write replaces the file; the marks that were lost stay lost.
    store
        .commit(&marking(vec![MarkWrite::Set(raised)]))
        .unwrap();
    assert_eq!(store.marks().unwrap().kept, None, "what the open found");
    drop(store);
    assert_eq!(reopen(&dir).marks().unwrap().kept, Some(vec![raised]));
}

#[test]
fn a_marks_unheard_bit_survives_reopen_and_a_nonzero_reserved_byte_is_lost() {
    let (dir, mut store) = scratch();
    let unheard = Mark {
        unheard: true,
        ..mark(1, 1, 0)
    };
    let ordinary = mark(1, 2, 0);
    store
        .commit(&marking(vec![
            MarkWrite::Set(unheard),
            MarkWrite::Set(ordinary),
        ]))
        .unwrap();
    drop(store);
    assert_eq!(
        reopen(&dir).marks().unwrap().kept,
        Some(vec![unheard, ordinary])
    );

    // A nonzero byte past the used ones reads as a torn write, checksum
    // notwithstanding.
    let mut file = fs::read(dir.marks_file()).unwrap();
    let slot = 32;
    file[slot + 107] = 1;
    let sum = content_hash(&file[slot..slot + 108]);
    file[slot + 108..slot + 112].copy_from_slice(&sum.0[..4]);
    fs::write(dir.marks_file(), file).unwrap();
    assert_eq!(reopen(&dir).marks().unwrap().kept, None);
}

#[test]
fn marks_in_a_copied_directory_are_not_kept() {
    let (dir, mut store) = scratch();
    let kept = mark(1, 1, 4);
    store.commit(&marking(vec![MarkWrite::Set(kept)])).unwrap();
    drop(store);
    let copy = dir.root.path().join("copy.marks");
    copy_files(&dir.marks, &copy);
    assert_eq!(
        fs::read(copy.join("marks")).unwrap(),
        fs::read(dir.marks_file()).unwrap()
    );
    let store = SqliteStore::open(dir.path(), &copy).unwrap();
    assert_eq!(store.marks().unwrap().kept, None);
    drop(store);
    assert_eq!(reopen(&dir).marks().unwrap().kept, Some(vec![kept]));
}

#[test]
fn an_older_marks_file_put_back_into_its_directory_reads_as_lost() {
    let (dir, mut store) = scratch();
    let (older, newer) = (mark(1, 1, 2), mark(1, 1, 3));
    store.commit(&marking(vec![MarkWrite::Set(older)])).unwrap();
    let saved = dir.root.path().join("saved");
    fs::copy(dir.marks_file(), &saved).unwrap();
    store.commit(&marking(vec![MarkWrite::Set(newer)])).unwrap();
    drop(store);

    let directory = FileId::of(&dir.marks).unwrap();
    fs::remove_file(dir.marks_file()).unwrap();
    fs::copy(&saved, dir.marks_file()).unwrap();
    assert_eq!(FileId::of(&dir.marks).unwrap(), directory);
    assert_eq!(reopen(&dir).marks().unwrap().kept, None);
}

#[test]
fn a_failed_commit_writes_no_mark() {
    let (dir, mut store) = scratch();
    let kept = mark(1, 1, 0);
    store.commit(&marking(vec![MarkWrite::Set(kept)])).unwrap();
    let writes = vec![
        MarkWrite::Set(mark(1, 1, 1)),
        MarkWrite::Set(mark(2, 2, 0)),
        MarkWrite::Clear {
            goal: kept.goal,
            key: kept.key,
        },
    ];
    // Refused inside the transaction: the store carries on.
    store
        .connection()
        .execute_batch(
            "CREATE TEMP TRIGGER refuse BEFORE INSERT ON local \
             WHEN NEW.key = CAST('refuse' AS BLOB) \
             BEGIN SELECT RAISE(ABORT, 'refused by the test'); END;",
        )
        .unwrap();
    crate::faults::take_trace();
    let refused = Commit {
        local: vec![put(Space::Goal, b"refuse", b"")],
        marks: writes.clone(),
        ..Commit::default()
    };
    assert!(matches!(store.commit(&refused), Err(StoreError::Failed(_))));
    assert_eq!(marks_steps(&dir), []);
    assert_eq!(store.get(Space::Goal, b"refuse"), Ok(None));
    drop(store);
    let store = reopen(&dir);
    assert_eq!(store.marks().unwrap().kept, Some(vec![kept]));

    // Failed while committing, with an unknown outcome: the store stops.
    store
        .connection()
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TEMP TABLE parent (id INTEGER PRIMARY KEY);
             CREATE TEMP TABLE child (
                 parent INTEGER REFERENCES parent (id) DEFERRABLE INITIALLY DEFERRED
             );
             CREATE TEMP TRIGGER fail_at_commit AFTER INSERT ON local
             WHEN NEW.key = CAST('fail at commit' AS BLOB)
             BEGIN INSERT INTO child VALUES (1); END;",
        )
        .unwrap();
    let mut store = store;
    let failing = Commit {
        local: vec![put(Space::Goal, b"fail at commit", b"")],
        marks: writes,
        ..Commit::default()
    };
    assert!(matches!(store.commit(&failing), Err(StoreError::Failed(_))));
    assert_eq!(marks_steps(&dir), []);
    drop(store);
    assert_eq!(reopen(&dir).marks().unwrap().kept, Some(vec![kept]));
}

#[test]
fn a_failed_mark_sync_breaks_the_store_and_nothing_is_released() {
    use crate::faults::{self, Point};
    let (dir, mut store) = scratch();
    store
        .commit(&marking(vec![MarkWrite::Set(mark(1, 1, 0))]))
        .unwrap();
    let mut owner = Author::new(1);
    let genesis = owner.genesis(keypair(9).public());
    faults::arm(Point::FileSync, &dir.marks_file());
    let signed = Commit {
        events: vec![genesis.clone()],
        marks: vec![MarkWrite::Set(mark(1, 1, 1))],
        ..Commit::default()
    };
    // The caller releases a signed record only once its commit returns.
    assert!(matches!(store.commit(&signed), Err(StoreError::Failed(_))));
    faults::assert_fired();
    assert!(matches!(
        store.has_event(&genesis.id()),
        Err(StoreError::Failed(_))
    ));
    assert!(matches!(store.marks(), Err(StoreError::Failed(_))));
    assert!(matches!(
        store.commit(&Commit::default()),
        Err(StoreError::Failed(_))
    ));
    drop(store);
    // The database committed before the marks were written.
    assert_eq!(reopen(&dir).has_event(&genesis.id()), Ok(true));
}

#[test]
fn a_commit_with_marks_syncs_the_marks_file_once_and_one_without_syncs_none() {
    use crate::faults::Point;
    let (dir, mut store) = scratch();
    let first = mark(1, 1, 0);
    store.commit(&marking(vec![MarkWrite::Set(first)])).unwrap();
    crate::faults::take_trace();
    store
        .commit(&Commit {
            local: vec![put(Space::Goal, b"g", b"1")],
            marks: vec![
                MarkWrite::Set(mark(1, 1, 1)),
                MarkWrite::Set(mark(1, 2, 0)),
                MarkWrite::Set(mark(2, 1, 0)),
                MarkWrite::Clear {
                    goal: first.goal,
                    key: first.key,
                },
            ],
            ..Commit::default()
        })
        .unwrap();
    assert_eq!(marks_steps(&dir), [(Point::FileSync, dir.marks_file())]);
    store
        .commit(&Commit {
            local: vec![put(Space::Goal, b"g", b"2")],
            ..Commit::default()
        })
        .unwrap();
    assert_eq!(marks_steps(&dir), []);
    drop(store);
    let mut store = reopen(&dir);
    assert_eq!(
        store.marks().unwrap().kept,
        Some(vec![mark(1, 2, 0), mark(2, 1, 0)])
    );
    // A new mark takes the slot the cleared one left.
    let len = fs::metadata(dir.marks_file()).unwrap().len();
    store
        .commit(&marking(vec![MarkWrite::Set(mark(3, 3, 0))]))
        .unwrap();
    assert_eq!(fs::metadata(dir.marks_file()).unwrap().len(), len);
    drop(store);
    assert_eq!(
        reopen(&dir).marks().unwrap().kept,
        Some(vec![mark(1, 2, 0), mark(2, 1, 0), mark(3, 3, 0)])
    );
}

#[test]
fn creating_the_marks_file_syncs_its_directory() {
    use crate::faults::Point;
    let (dir, mut store) = scratch();
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&dir.marks), 0o700);
    assert!(!dir.marks_file().exists(), "no file until the first mark");
    let created = [
        (Point::FileSync, dir.marks.join("marks.tmp")),
        (Point::DirectorySync, dir.marks.clone()),
    ];
    crate::faults::take_trace();
    store
        .commit(&marking(vec![MarkWrite::Set(mark(1, 1, 0))]))
        .unwrap();
    assert_eq!(marks_steps(&dir), created);
    assert_eq!(mode(&dir.marks_file()), 0o600);
    drop(store);

    // A file that does not read is replaced the same way.
    tear_last_record(&dir.marks_file());
    let mut store = reopen(&dir);
    crate::faults::take_trace();
    store
        .commit(&marking(vec![MarkWrite::Set(mark(1, 1, 1))]))
        .unwrap();
    assert_eq!(marks_steps(&dir), created);
}

#[test]
fn a_copied_database_file_has_another_identity() {
    let (dir, mut store) = scratch();
    let kept = mark(1, 1, 0);
    store
        .commit(&Commit {
            local: vec![put(Space::Goal, b"g", b"1")],
            marks: vec![MarkWrite::Set(kept)],
            ..Commit::default()
        })
        .unwrap();
    let file = store.marks().unwrap().file;
    drop(store);
    assert_eq!(reopen(&dir).marks().unwrap().file, file);

    let copy = dir.root.path().join("copy");
    copy_files(dir.path(), &copy);
    let store = SqliteStore::open(&copy, &dir.marks).unwrap();
    assert_eq!(store.get(Space::Goal, b"g"), Ok(Some(b"1".to_vec())));
    let found = store.marks().unwrap();
    assert_ne!(found.file, file);
    assert_eq!(
        found.kept,
        Some(vec![kept]),
        "the marks are beside, not copied"
    );
}
