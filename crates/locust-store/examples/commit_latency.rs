//! Prints what the SQLite store costs on this machine: commit latency for one
//! event per commit, with and without a mark, and for 256 events per commit,
//! and the rate of replaying a goal's feed from position 1. Built by
//! `cargo test`, never run by it.
//!
//! ```sh
//! cargo run --release -p locust-store --example commit_latency [-- <dir>]
//! ```
//!
//! The state directory and its marks directory are made in a temporary
//! directory inside `<dir>`, by default the system's temporary directory, so
//! another disk can be measured.

use std::env;
use std::time::{Duration, Instant};

use locust_proto::PROTOCOL_VERSION;
use locust_proto::event::{AuthorPoint, Body, Event, Header, PayloadRef};
use locust_proto::id::{BlobHash, EventId, GoalId};
use locust_proto::limits::{MAX_ARTIFACTS, MAX_EVENTS_PER_BATCH, MAX_PARENTS, MAX_PAYLOAD_BYTES};
use locust_proto::local::marks_dir;
use locust_proto::store::{Commit, Mark, MarkWrite, Store};
use locust_proto::testkit::{Author, keypair};
use locust_store::SqliteStore;

const SINGLE_COMMITS: usize = 200;
const BATCH_COMMITS: usize = 20;

fn main() {
    let dir = match env::args_os().nth(1) {
        Some(parent) => tempfile::tempdir_in(parent),
        None => tempfile::tempdir(),
    }
    .unwrap();
    let state = dir.path().join("state");
    println!("state directory {}", state.display());
    let mut store = SqliteStore::open(&state, &marks_dir(&state)).unwrap();

    let mut owner = Author::new(1);
    let genesis = owner.genesis(locust_proto::testkit::keypair(9).public());
    let goal = genesis.header().goal;
    let round = genesis.id();
    let anchor = Some(round);
    commit(&mut store, vec![genesis], Vec::new());
    let mut note = || {
        owner.event(
            goal,
            anchor,
            Body::ContributionPublished {
                context: locust_proto::event::Context {
                    scope: locust_proto::event::Scope::Goal,
                    round,
                },
                attempt: None,
                sources: Vec::new(),
                artifacts: vec![],
            },
        )
    };

    // Alternated, so both see the same disk and the same log size.
    let (mut single, mut marked) = (Vec::new(), Vec::new());
    for _ in 0..SINGLE_COMMITS {
        single.push(commit(&mut store, vec![note()], Vec::new()));
        let event = note();
        let mark = MarkWrite::Set(Mark {
            goal,
            key: event.header().author,
            point: AuthorPoint {
                seq: event.header().seq,
                id: event.id(),
            },
            shared: true,
            unheard: false,
        });
        marked.push(commit(&mut store, vec![event], vec![mark]));
    }
    report("1 event per commit", single, 1);
    report("1 event and 1 mark per commit", marked, 1);

    let batches: Vec<Duration> = (0..BATCH_COMMITS)
        .map(|_| {
            commit(
                &mut store,
                (0..MAX_EVENTS_PER_BATCH).map(|_| note()).collect(),
                Vec::new(),
            )
        })
        .collect();
    report("256 events per commit", batches, MAX_EVENTS_PER_BATCH);

    let mut largest = Largest::new();
    let size = largest.next().header_bytes().len();
    let batches: Vec<Duration> = (0..BATCH_COMMITS)
        .map(|_| {
            let batch = (0..MAX_EVENTS_PER_BATCH).map(|_| largest.next()).collect();
            commit(&mut store, batch, Vec::new())
        })
        .collect();
    report(
        &format!("256 largest ({size}-byte) headers per commit"),
        batches,
        MAX_EVENTS_PER_BATCH,
    );

    let start = Instant::now();
    let (mut after, mut events) = (0, 0);
    loop {
        let page = store.log(&goal, after, MAX_EVENTS_PER_BATCH).unwrap();
        let Some((last, _)) = page.last() else { break };
        after = *last;
        events += page.len();
    }
    let elapsed = start.elapsed();
    println!(
        "replay from position 1, pages of 256: {events} events in {elapsed:.2?} ({:.0} events/s)",
        events as f64 / elapsed.as_secs_f64()
    );
}

fn commit(store: &mut SqliteStore, events: Vec<Event>, marks: Vec<MarkWrite>) -> Duration {
    let commit = Commit {
        events,
        marks,
        ..Commit::default()
    };
    let start = Instant::now();
    store.commit(&commit).unwrap();
    start.elapsed()
}

fn report(what: &str, mut samples: Vec<Duration>, events: usize) {
    samples.sort();
    let at = |fraction: f64| samples[((samples.len() - 1) as f64 * fraction) as usize];
    let median = at(0.5);
    println!(
        "{what}: median {median:.2?}, p90 {:.2?}, max {:.2?} over {} commits ({:.1?} per event at the median)",
        at(0.9),
        at(1.0),
        samples.len(),
        median / events as u32
    );
}

/// Events of one author whose headers are as large as the contract admits.
struct Largest {
    key: locust_proto::crypto::Keypair,
    seq: u64,
    prev: EventId,
}

impl Largest {
    fn new() -> Self {
        Self {
            key: keypair(2),
            seq: 1,
            prev: EventId([0x11; 32]),
        }
    }

    fn next(&mut self) -> Event {
        let header = Header {
            version: PROTOCOL_VERSION,
            goal: GoalId([7; 32]),
            author: self.key.public(),
            seq: self.seq,
            prev: Some(self.prev),
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
                context: locust_proto::event::Context {
                    scope: locust_proto::event::Scope::Goal,
                    round: EventId([0xaa; 32]),
                },
                attempt: Some(EventId([0xcc; 32])),
                artifacts: (0..MAX_ARTIFACTS)
                    .map(|a| BlobHash([a as u8; 32]))
                    .collect(),
            },
        };
        let event = Event::sign(header, &self.key).unwrap();
        self.seq += 1;
        self.prev = event.id();
        event
    }
}
