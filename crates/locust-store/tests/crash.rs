//! Unclean stops. The test binary runs itself again as a child that commits
//! in a loop and reports each acknowledged commit on stdout; the parent kills
//! it with SIGKILL, or lets it exit without closing the store, then reopens
//! the state directory and checks every acknowledged commit.
//!
//! A killed process keeps whatever the kernel already accepted, so these
//! tests cover atomicity and ordering across a process crash; they cannot
//! simulate a power loss.

use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use locust_proto::crypto::content_hash;
use locust_proto::event::{Body, Event};
use locust_proto::id::{BlobHash, GoalId};
use locust_proto::local::marks_dir;
use locust_proto::store::{Blob, Commit, LocalWrite, Space, Store};
use locust_proto::testkit::{Author, sealed_payload};
use locust_store::{INLINE_MAX_BYTES, OpenError, SqliteStore};

/// Set only in the child: the state directory to commit into.
const CHILD_DIR: &str = "LOCUST_STORE_TEST_CHILD_DIR";
/// Set only in a child that should exit, without closing the store, after
/// this many commits.
const CHILD_EXIT_AFTER: &str = "LOCUST_STORE_TEST_CHILD_EXIT_AFTER";

/// The history both processes agree on: commit `n` stores event `n` (the
/// genesis for 0), its payload and its input object, large for odd `n` and
/// inline for even `n`, and one local record.
struct History {
    author: Author,
    events: Vec<Event>,
}

impl History {
    fn new() -> Self {
        let mut author = Author::new(1);
        let genesis = author.genesis(locust_proto::testkit::keypair(9).public());
        Self {
            author,
            events: vec![genesis],
        }
    }

    fn goal(&self) -> GoalId {
        self.events[0].header().goal
    }

    fn input(n: usize) -> Blob {
        let len = if n % 2 == 1 {
            INLINE_MAX_BYTES + 1 + n
        } else {
            64 + n
        };
        Blob::new((0..len).map(|i| (i * 7 + n) as u8).collect())
    }

    fn payload(&self, n: usize) -> Blob {
        sealed_payload(&self.goal(), 0, format!("task {n}").as_bytes()).1
    }

    fn event(&mut self, n: usize) -> &Event {
        while self.events.len() <= n {
            let next = self.events.len();
            let goal = self.goal();
            let anchor = Some(self.events[0].id());
            let (payload, _) = sealed_payload(&goal, 0, format!("task {next}").as_bytes());
            let body = Body::ContributionPublished {
                context: locust_proto::event::Context {
                    scope: locust_proto::event::Scope::Goal,
                    round: self.events[0].id(),
                },
                attempt: None,
                sources: Vec::new(),
                artifacts: vec![],
            };
            let event = self.author.event_with(goal, anchor, body, Some(payload));
            self.events.push(event);
        }
        &self.events[n]
    }

    fn record(n: usize) -> Vec<u8> {
        format!("commit/{n:06}").into_bytes()
    }

    fn commit(&mut self, n: usize) -> Commit {
        let event = self.event(n).clone();
        let blobs = if n == 0 {
            Vec::new()
        } else {
            vec![self.payload(n), Self::input(n)]
        };
        Commit {
            events: vec![event],
            blobs,
            local: vec![LocalWrite::Put {
                space: Space::Cursor,
                key: Self::record(n),
                value: n.to_le_bytes().to_vec(),
            }],
            marks: Vec::new(),
        }
    }
}

/// The child's side; does nothing unless run as a child.
#[test]
#[ignore = "run by the crash tests as a child process"]
fn child_commits_until_stopped() {
    let Some(dir) = env::var_os(CHILD_DIR) else {
        return;
    };
    let exit_after: Option<usize> = env::var(CHILD_EXIT_AFTER).ok().map(|n| n.parse().unwrap());
    let mut store = SqliteStore::open(&dir, &marks_dir(Path::new(&dir))).unwrap();
    let mut history = History::new();
    let start = store.log(&history.goal(), 0, usize::MAX).unwrap().len();
    let mut out = std::io::stdout().lock();
    for n in start..start + exit_after.unwrap_or(100_000) {
        store.commit(&history.commit(n)).unwrap();
        writeln!(out, "ack {n}").unwrap();
        out.flush().unwrap();
    }
    // Leaves without dropping the store: no close, no checkpoint.
    std::process::exit(0);
}

fn spawn_child(dir: &Path, exit_after: Option<usize>) -> Child {
    let mut command = Command::new(env::current_exe().unwrap());
    command
        .args([
            "child_commits_until_stopped",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_DIR, dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if let Some(n) = exit_after {
        command.env(CHILD_EXIT_AFTER, n.to_string());
    }
    command.spawn().unwrap()
}

/// The commit number in an acknowledgement. The first one shares its line
/// with the test harness's "test ... " prefix.
fn acknowledged(line: &str) -> Option<usize> {
    line.rsplit_once("ack ")?.1.parse().ok()
}

/// Reopens `dir` and checks that at least the first `acknowledged` commits
/// are held, that the log is exactly a prefix of the history at dense
/// positions, that every held event's objects and record are held with the
/// right bytes, and that no file is left that no row names. Returns the
/// number of commits held.
fn check(dir: &Path, acknowledged: usize) -> usize {
    let store = SqliteStore::open(dir, &marks_dir(dir)).unwrap();
    let mut history = History::new();
    let log = store.log(&history.goal(), 0, usize::MAX).unwrap();
    assert!(
        log.len() >= acknowledged,
        "{acknowledged} commits were acknowledged, {} are held",
        log.len()
    );
    for (n, (position, event)) in log.iter().enumerate() {
        assert_eq!(*position, n as u64 + 1, "positions are dense");
        assert_eq!(event, history.event(n), "commit {n} holds its own event");
        for hash in event.header().blobs() {
            let bytes = store.blob(&hash).unwrap();
            let bytes = bytes.unwrap_or_else(|| panic!("commit {n} names a missing object"));
            assert_eq!(
                content_hash(&bytes),
                hash,
                "commit {n} holds intact objects"
            );
        }
        assert_eq!(
            store.get(Space::Cursor, &History::record(n)),
            Ok(Some(n.to_le_bytes().to_vec())),
            "commit {n} holds its local record"
        );
    }
    for entry in fs::read_dir(dir.join("blobs")).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        let hash: BlobHash = name
            .parse()
            .unwrap_or_else(|_| panic!("{name} was left in the object directory"));
        assert!(
            store.blob_len(&hash).unwrap().is_some(),
            "{name} is an orphan"
        );
    }
    log.len()
}

/// One commit takes from about 4 ms (one flush) to about 12 ms (three, when
/// it installs a large object) on the machine this was written on. Killing
/// the child this long after an acknowledgement spreads the kills over the
/// file install, the transaction and the gap between commits; killing at
/// once would always land in the gap.
fn kill_delay(round: u32) -> Duration {
    Duration::from_micros(1_300 * u64::from(round))
}

#[test]
fn acknowledged_commits_survive_a_process_killed_mid_commit() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("state");
    let mut held = 0;
    for round in 0..10 {
        let mut child = spawn_child(&dir, None);
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut acked = held;
        let mut seen = 0;
        while seen < 2 + round % 3 {
            let line = lines
                .next()
                .expect("the child stopped before acknowledging enough commits")
                .unwrap();
            if let Some(n) = acknowledged(&line) {
                acked = n + 1;
                seen += 1;
            }
        }
        if round == 0 {
            assert!(matches!(
                SqliteStore::open(&dir, &marks_dir(&dir)),
                Err(OpenError::InUse(_))
            ));
        }
        thread::sleep(kill_delay(round));
        child.kill().unwrap();
        child.wait().unwrap();
        for line in lines {
            if let Some(n) = acknowledged(&line.unwrap()) {
                acked = n + 1;
            }
        }
        held = check(&dir, acked);
    }
}

#[test]
fn a_process_that_exits_without_closing_the_store_loses_no_commit() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("state");
    let mut child = spawn_child(&dir, Some(12));
    let output = BufReader::new(child.stdout.take().unwrap());
    let acked = output
        .lines()
        .filter_map(|line| acknowledged(&line.unwrap()))
        .count();
    assert!(child.wait().unwrap().success());
    assert_eq!(acked, 12);
    // The store was never closed, so its commits are still in the WAL.
    let wal = fs::metadata(dir.join("locust.db-wal")).unwrap();
    assert!(wal.len() > 0);
    assert_eq!(check(&dir, acked), 12);
}
