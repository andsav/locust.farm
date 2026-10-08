//! A start whose store or node cannot open: each failure says what happened
//! and then one thing to do, and nothing stored is moved or deleted.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use locust_proto::api::ErrorCode;
use locust_proto::local;
use locust_proto::store::{Commit, LocalWrite, Space, Store, StoreError};
use locust_store::{OpenError, SqliteStore};

use super::durable_tests::local_endpoint;
use super::{home, run_networked_with, store_open_failure};
use crate::failure::Failure;
use crate::testdir::short_dir;

const LOSS: &str = "A new data folder starts with no goals. Goals you host cannot continue \
    from it, and each goal you joined needs its ticket again.";

#[test]
fn each_store_open_failure_says_one_next_step() {
    let home = Path::new("/srv/locust");
    let marks = Path::new("/srv/locust.marks");
    let other_version = format!(
        "The data folder /srv/locust was made by another Locust version, and no version \
         converts it. Start the version that made it, or move the folder aside, do not delete \
         it, and start with a new one. {LOSS}"
    );
    let cases = [
        (
            OpenError::InUse(home.to_path_buf()),
            ErrorCode::Unavailable,
            "another program has the database in /srv/locust open. Close it, then start again."
                .to_owned(),
        ),
        (
            OpenError::UnsupportedSchema { found: 3, known: 7 },
            ErrorCode::UnsupportedVersion,
            format!(
                "the database has unsupported schema version 3; this binary supports only \
                 version 7. {other_version}"
            ),
        ),
        (
            OpenError::UnsupportedProtocolVersion { found: 6, known: 7 },
            ErrorCode::UnsupportedVersion,
            format!(
                "the state directory contains event protocol version 6, but this binary \
                 supports version 7. {other_version}"
            ),
        ),
        (
            OpenError::Store(StoreError::Corrupted("stored event 1a2b: bad id".into())),
            ErrorCode::Corrupted,
            format!(
                "stored data is corrupted: stored event 1a2b: bad id. Move /srv/locust aside \
                 and do not delete it: it holds your keys and every record. {LOSS}"
            ),
        ),
        (
            OpenError::Store(StoreError::Failed("cannot create /srv/locust/blobs".into())),
            ErrorCode::Internal,
            "storage failed: cannot create /srv/locust/blobs. Check that the disk has space \
             and that you can read and write /srv/locust and /srv/locust.marks, then start \
             again."
                .to_owned(),
        ),
        (
            OpenError::MarksNotPrivate {
                path: marks.to_path_buf(),
                mode: 0o755,
                wanted: 0o700,
            },
            ErrorCode::Internal,
            "/srv/locust.marks has mode 0755; it must be 0700. Run chmod 700 \
             /srv/locust.marks, then start again."
                .to_owned(),
        ),
    ];
    for (error, code, message) in cases {
        // What happened says no more than that; the next step is added once.
        let happened = error.to_string();
        let failure = store_open_failure(home, marks, error);
        assert_eq!(failure.message, message);
        assert!(failure.message.starts_with(&format!("{happened}. ")));
        assert!(!happened.contains("again") && !happened.contains("aside"));
        assert_eq!(failure.code, code);
        assert_eq!(
            failure.exit_status(),
            crate::failure::exit_status(code),
            "{message}"
        );
    }
}

/// Starts the production assembly on `home` with the marks in `marks` and
/// returns its failure. It must fail before it listens.
fn refused_start(home: &Path, marks: &Path) -> Failure {
    let failure = run_networked_with(
        home,
        marks,
        std::convert::identity,
        local_endpoint,
        |_socket| -> std::io::Result<std::future::Ready<()>> {
            panic!("a daemon whose store cannot open started listening")
        },
    )
    .unwrap_err();
    assert!(!home.join("daemon.sock").exists());
    assert_eq!(home::lock_holder(home).unwrap(), None);
    failure
}

fn marks_of(home: &Path) -> PathBuf {
    local::marks_dir(home).unwrap()
}

#[test]
fn a_marks_directory_others_may_enter_names_the_chmod_that_fixes_it() {
    let dir = short_dir();
    let marks = marks_of(dir.path());
    fs::create_dir(&marks).unwrap();
    fs::set_permissions(&marks, fs::Permissions::from_mode(0o755)).unwrap();
    let failure = refused_start(dir.path(), &marks);
    assert_eq!(failure.code, ErrorCode::Internal);
    assert_eq!(
        failure.message,
        format!(
            "{marks} has mode 0755; it must be 0700. Run chmod 700 {marks}, then start again.",
            marks = marks.display()
        )
    );
    let mode = fs::metadata(&marks).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755);
}

#[test]
fn a_marks_directory_that_cannot_be_made_names_both_directories() {
    let dir = short_dir();
    let marks = marks_of(dir.path());
    fs::write(&marks, b"not a directory").unwrap();
    let failure = refused_start(dir.path(), &marks);
    assert_eq!(failure.code, ErrorCode::Internal);
    assert!(
        failure.message.ends_with(&format!(
            ". Check that the disk has space and that you can read and write {} and {}, then \
             start again.",
            dir.path().display(),
            marks.display()
        )),
        "{}",
        failure.message
    );
    assert_eq!(fs::read(&marks).unwrap(), b"not a directory");
}

#[test]
fn a_database_another_program_has_open_says_to_close_it() {
    let dir = short_dir();
    let marks = marks_of(dir.path());
    drop(SqliteStore::open(dir.path(), &marks).unwrap());
    // An sqlite3 shell, say, that read the database and stayed open.
    let other = rusqlite::Connection::open(local::database_path(dir.path())).unwrap();
    other
        .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
        .unwrap();
    let failure = refused_start(dir.path(), &marks);
    assert_eq!(failure.code, ErrorCode::Unavailable);
    assert_eq!(failure.exit_status(), 8);
    assert_eq!(
        failure.message,
        format!(
            "another program has the database in {} open. Close it, then start again.",
            dir.path().display()
        )
    );
}

#[test]
fn a_record_the_node_cannot_read_says_to_move_the_folder_aside() {
    let dir = short_dir();
    let marks = marks_of(dir.path());
    let mut store = SqliteStore::open(dir.path(), &marks).unwrap();
    store
        .commit(&Commit {
            local: vec![LocalWrite::Put {
                space: Space::Identity,
                key: b"not a key this node writes".to_vec(),
                value: vec![1],
            }],
            ..Commit::default()
        })
        .unwrap();
    drop(store);
    let database = local::database_path(dir.path());
    let before = fs::read(&database).unwrap();
    let failure = refused_start(dir.path(), &marks);
    assert_eq!(failure.code, ErrorCode::Corrupted);
    assert_eq!(
        failure.message,
        format!(
            "stored data is corrupted: a local record has a malformed key. Move {} aside and do \
             not delete it: it holds your keys and every record. {LOSS}",
            dir.path().display()
        )
    );
    assert_eq!(fs::read(&database).unwrap(), before);
}
