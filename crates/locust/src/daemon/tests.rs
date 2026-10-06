//! The shell against a scripted engine that records every call, over a real
//! Unix socket in a temporary state directory.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use locust_proto::API_VERSION;
use locust_proto::api::{
    ApiError, Caller, ClientHello, Credential, DaemonStatus, ErrorCode, PendingWork, Request,
    RequestFrame, Response, ResponseFrame, ServerHello, SessionSecret, WaitOutcome,
};
use locust_proto::client::{Client, ClientError};
use locust_proto::codec;
use locust_proto::engine::{ConnId, Engine, Parked, Step};
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey};
use locust_proto::limits::{MAX_HELLO_FRAME_BYTES, MAX_LOCAL_FRAME_BYTES};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::{EngineInit, home, run_with, system};
use crate::failure::Failure;
use crate::testdir::short_dir;
use crate::version;

const OWNER: Credential = Credential([1; 32]);
const AGENT: Credential = Credential([2; 32]);
const STRANGER: Credential = Credential([9; 32]);
const GOAL: GoalId = GoalId([0x11; 32]);
const OTHER_GOAL: GoalId = GoalId([0x22; 32]);

/// Bytes of the large answer the scripted engine gives to `blob.get`.
const LARGE: usize = 300_000;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Connect {
        conn: u64,
        session: bool,
    },
    Request {
        conn: u64,
        id: u64,
        name: &'static str,
    },
    Resume {
        conn: u64,
        id: u64,
        timed_out: bool,
    },
    Disconnect {
        conn: u64,
    },
}

#[derive(Default)]
struct Record {
    calls: Vec<Call>,
    /// The time passed into each call.
    times: Vec<u64>,
    /// Digest of the owner credential the shell handed the engine.
    owner_digest: Option<[u8; 32]>,
}

type Log = Arc<Mutex<Record>>;

struct Scripted {
    log: Log,
    changed: Vec<GoalId>,
    /// How many resumes after a change park the wait again.
    unconcerned: u32,
    stop: bool,
}

impl Scripted {
    fn note(&self, call: Call, now_ms: u64) {
        let mut record = self.log.lock().unwrap();
        record.calls.push(call);
        record.times.push(now_ms);
    }
}

impl Engine for Scripted {
    fn connect(&mut self, conn: ConnId, hello: &ClientHello, now_ms: u64) -> ServerHello {
        let caller = if hello.credential == OWNER {
            Caller::Owner
        } else if hello.credential == AGENT {
            Caller::Agent(PublicKey([2; 32]))
        } else {
            return ServerHello::Refused {
                error: ApiError::new(ErrorCode::Denied, "credential is not known"),
                api_version: API_VERSION,
                daemon_version: "scripted".to_string(),
            };
        };
        self.note(
            Call::Connect {
                conn: conn.0,
                session: hello.session.is_some(),
            },
            now_ms,
        );
        ServerHello::Welcome {
            api_version: API_VERSION,
            daemon_version: "scripted".to_string(),
            caller,
            max_blob_bytes: 1024,
        }
    }

    fn request(&mut self, conn: ConnId, frame: RequestFrame, now_ms: u64) -> Step {
        self.note(
            Call::Request {
                conn: conn.0,
                id: frame.id,
                name: frame.request.name(),
            },
            now_ms,
        );
        let result = match frame.request {
            Request::Status => Ok(Response::Status(DaemonStatus {
                daemon_version: "scripted".to_string(),
                endpoint: None,
                agents: Vec::new(),
                goals: Vec::new(),
            })),
            Request::Shutdown => {
                self.stop = true;
                Ok(Response::Done)
            }
            Request::Wait {
                goal, timeout_ms, ..
            } => {
                return Step::Park(Parked {
                    request_id: frame.id,
                    goal,
                    timeout_ms,
                });
            }
            Request::ContributionPublish {
                goal,
                summary: text,
                ..
            } => {
                self.changed.push(goal);
                // The event says how much text arrived.
                let mut event = [0u8; 32];
                event[..8].copy_from_slice(&(text.len() as u64).to_le_bytes());
                Ok(Response::Recorded {
                    event: EventId(event),
                })
            }
            Request::Pending { .. } => Ok(Response::Pending(PendingWork {
                // Tells the connections apart.
                revision: conn.0,
                ..PendingWork::default()
            })),
            Request::BlobGet { .. } => Ok(Response::Blob {
                bytes: vec![7; LARGE],
            }),
            _ => Err(ApiError::new(ErrorCode::Unavailable, "not scripted")),
        };
        Step::Reply(ResponseFrame {
            id: frame.id,
            result,
        })
    }

    fn resume(&mut self, conn: ConnId, parked: &Parked, timed_out: bool, now_ms: u64) -> Step {
        self.note(
            Call::Resume {
                conn: conn.0,
                id: parked.request_id,
                timed_out,
            },
            now_ms,
        );
        let outcome = if timed_out {
            WaitOutcome::NoEvent
        } else if self.unconcerned > 0 {
            self.unconcerned -= 1;
            return Step::Park(*parked);
        } else {
            WaitOutcome::Work(Box::new(PendingWork {
                revision: 2,
                ..PendingWork::default()
            }))
        };
        Step::Reply(ResponseFrame {
            id: parked.request_id,
            result: Ok(Response::Waited(outcome)),
        })
    }

    fn take_changed(&mut self) -> Vec<GoalId> {
        std::mem::take(&mut self.changed)
    }

    fn disconnect(&mut self, conn: ConnId) {
        self.note(Call::Disconnect { conn: conn.0 }, 0);
    }

    fn stop_requested(&self) -> bool {
        self.stop
    }
}

/// A daemon running the scripted engine on its own thread.
struct Running {
    _dir: Option<tempfile::TempDir>,
    home: PathBuf,
    log: Log,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<JoinHandle<Result<(), Failure>>>,
}

impl Running {
    fn start() -> Self {
        Self::start_with(0)
    }

    fn start_with(unconcerned: u32) -> Self {
        let dir = short_dir();
        let mut running = Self::start_in(dir.path(), unconcerned);
        running._dir = Some(dir);
        running
    }

    fn start_in(home: &Path, unconcerned: u32) -> Self {
        let log = Log::default();
        let (shutdown, stopped) = tokio::sync::oneshot::channel::<()>();
        let (ready, listening) = mpsc::channel();
        let thread = thread::spawn({
            let (home, log) = (home.to_path_buf(), log.clone());
            move || {
                run_with(
                    &home,
                    move |init: EngineInit| {
                        log.lock().unwrap().owner_digest = Some(init.owner_digest);
                        Ok(Scripted {
                            log,
                            changed: Vec::new(),
                            unconcerned,
                            stop: false,
                        })
                    },
                    move |_socket| {
                        ready.send(()).unwrap();
                        Ok(async move {
                            let _ = stopped.await;
                        })
                    },
                )
            }
        });
        listening
            .recv_timeout(Duration::from_secs(10))
            .expect("the daemon did not start listening");
        Self {
            _dir: None,
            home: home.to_path_buf(),
            log,
            shutdown: Some(shutdown),
            thread: Some(thread),
        }
    }

    fn socket(&self) -> PathBuf {
        self.home.join("daemon.sock")
    }

    fn connect(&self) -> UnixStream {
        let stream = UnixStream::connect(self.socket()).unwrap();
        // A test that would hang fails instead.
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        stream
    }

    fn client(&self, credential: Credential) -> Client<UnixStream> {
        Client::open(self.connect(), credential, None).unwrap()
    }

    /// A connection past the hello, for writing frames by hand.
    fn raw(&self, credential: Credential) -> UnixStream {
        let mut stream = self.connect();
        send(
            &mut stream,
            &ClientHello {
                api_version: API_VERSION,
                credential,
                session: None,
            },
        );
        let answer: ServerHello = receive(&mut stream).unwrap();
        assert!(matches!(answer, ServerHello::Welcome { .. }));
        stream
    }

    fn calls(&self) -> Vec<Call> {
        self.log.lock().unwrap().calls.clone()
    }

    /// Waits until the engine has seen `call`.
    fn saw(&self, call: &Call) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !self.calls().contains(call) {
            assert!(Instant::now() < deadline, "the engine never saw {call:?}");
            thread::sleep(Duration::from_millis(2));
        }
    }

    /// Stops the daemon the way a signal does and returns how it ended.
    fn stop(&mut self) -> Result<(), Failure> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.join()
    }

    fn join(&mut self) -> Result<(), Failure> {
        self.thread.take().unwrap().join().unwrap()
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if self.thread.is_some() {
            let _ = self.stop();
        }
    }
}

fn send<T: Serialize>(stream: &mut UnixStream, value: &T) {
    let mut bytes = Vec::new();
    codec::encode_frame(value, &mut bytes).unwrap();
    stream.write_all(&bytes).unwrap();
}

fn receive<T: DeserializeOwned>(stream: &mut UnixStream) -> Option<T> {
    codec::read_frame(stream, MAX_LOCAL_FRAME_BYTES)
        .unwrap()
        .map(|bytes| codec::decode(&bytes).unwrap())
}

/// True if the daemon closed the connection: nothing more can be read.
fn closed(stream: &mut UnixStream) -> bool {
    let mut byte = [0u8; 1];
    match stream.read(&mut byte) {
        Ok(read) => read == 0,
        Err(error) => error.kind() == std::io::ErrorKind::ConnectionReset,
    }
}

fn request(id: u64, request: Request) -> RequestFrame {
    RequestFrame {
        id,
        idempotency: None,
        on_behalf: None,
        request,
    }
}

fn wait(timeout_ms: u32) -> Request {
    Request::Wait {
        goal: GOAL,
        seen: 1,
        timeout_ms,
    }
}

fn note(goal: GoalId, text: &str) -> Request {
    Request::ContributionPublish {
        goal,
        attempt: None,
        generation: None,
        sources: Vec::new(),
        artifacts: vec![],
        summary: text.to_string(),
    }
}

fn resumes(calls: &[Call]) -> Vec<&Call> {
    calls
        .iter()
        .filter(|call| matches!(call, Call::Resume { .. }))
        .collect()
}

#[test]
fn a_hello_is_answered_by_the_engine_and_its_end_is_reported() {
    let before = system::now_ms();
    let daemon = Running::start();
    let client = daemon.client(OWNER);
    assert_eq!(client.caller(), Caller::Owner);
    assert_eq!(client.daemon_version(), "scripted");
    assert_eq!(client.max_blob_bytes(), 1024);
    drop(client);
    daemon.saw(&Call::Disconnect { conn: 1 });

    let session = Some(SessionSecret([0x6b; 32]));
    let client = Client::open(daemon.connect(), AGENT, session).unwrap();
    assert_eq!(client.caller(), Caller::Agent(PublicKey([2; 32])));
    drop(client);
    daemon.saw(&Call::Disconnect { conn: 2 });

    assert_eq!(
        daemon.calls(),
        [
            Call::Connect {
                conn: 1,
                session: false
            },
            Call::Disconnect { conn: 1 },
            Call::Connect {
                conn: 2,
                session: true
            },
            Call::Disconnect { conn: 2 },
        ]
    );
    // The shell passed the system clock into the hello.
    let time = daemon.log.lock().unwrap().times[0];
    assert!((before..=system::now_ms()).contains(&time));
}

#[test]
fn a_hello_the_engine_refuses_is_answered_and_closed() {
    let daemon = Running::start();
    let Err(ClientError::Refused { error, .. }) = Client::open(daemon.connect(), STRANGER, None)
    else {
        panic!("the stranger was welcomed");
    };
    assert_eq!(error.code, ErrorCode::Denied);

    let mut stream = daemon.connect();
    send(
        &mut stream,
        &ClientHello {
            api_version: API_VERSION,
            credential: STRANGER,
            session: None,
        },
    );
    let answer: ServerHello = receive(&mut stream).unwrap();
    assert!(matches!(answer, ServerHello::Refused { .. }));
    assert!(closed(&mut stream));

    // A later connection is served, and the refused ones were never
    // attached, so nothing reports their end.
    let mut client = daemon.client(OWNER);
    client.call(Request::Status).unwrap();
    drop(client);
    daemon.saw(&Call::Disconnect { conn: 3 });
    assert_eq!(
        daemon.calls(),
        [
            Call::Connect {
                conn: 3,
                session: false
            },
            Call::Request {
                conn: 3,
                id: 1,
                name: "status"
            },
            Call::Disconnect { conn: 3 },
        ]
    );
}

#[test]
fn a_hello_the_shell_cannot_read_is_refused_without_asking_the_engine() {
    let daemon = Running::start();
    let refusal = |frame: &[u8]| {
        let mut stream = daemon.connect();
        stream.write_all(frame).unwrap();
        let Some(ServerHello::Refused {
            error,
            api_version,
            daemon_version,
        }) = receive(&mut stream)
        else {
            panic!("the hello was not refused");
        };
        assert_eq!(api_version, API_VERSION);
        assert_eq!(daemon_version, version::daemon());
        assert!(closed(&mut stream));
        error
    };

    // A client of another API version, whatever follows its version.
    let mut other = Vec::new();
    codec::encode_frame(&(API_VERSION + 1, [0u8; 7]), &mut other).unwrap();
    let error = refusal(&other);
    assert_eq!(error.code, ErrorCode::UnsupportedVersion);
    assert!(error.message.contains(&(API_VERSION + 1).to_string()));

    // This version, but not a hello.
    let mut garbage = Vec::new();
    codec::encode_frame(&(API_VERSION, 7u8), &mut garbage).unwrap();
    assert_eq!(refusal(&garbage).code, ErrorCode::Invalid);

    // A hello frame above the hello limit is refused before it is read.
    let oversized = (MAX_HELLO_FRAME_BYTES as u32 + 1).to_le_bytes();
    assert_eq!(refusal(&oversized).code, ErrorCode::Invalid);

    // A connection that closes before its hello is simply gone.
    drop(daemon.connect());

    let mut client = daemon.client(OWNER);
    client.call(Request::Status).unwrap();
    assert!(
        daemon
            .calls()
            .iter()
            .all(|call| !matches!(call, Call::Connect { conn, .. } if *conn != 5))
    );
}

#[test]
fn a_request_is_answered_under_its_id() {
    let before = system::now_ms();
    let daemon = Running::start();
    let mut client = daemon.client(OWNER);
    let Response::Status(status) = client.call(Request::Status).unwrap() else {
        panic!("status was answered with something else");
    };
    assert_eq!(status.daemon_version, "scripted");
    let Err(ClientError::Api(error)) = client.call(Request::Sessions) else {
        panic!("an unscripted request succeeded");
    };
    assert_eq!(error.code, ErrorCode::Unavailable);
    // An error leaves the connection usable.
    client.call(Request::Status).unwrap();

    let mut stream = daemon.raw(AGENT);
    send(&mut stream, &request(77, Request::Status));
    let answer: ResponseFrame = receive(&mut stream).unwrap();
    assert_eq!(answer.id, 77);
    assert!(matches!(answer.result, Ok(Response::Status(_))));

    let record = daemon.log.lock().unwrap();
    assert_eq!(
        record.calls[1..4],
        [
            Call::Request {
                conn: 1,
                id: 1,
                name: "status"
            },
            Call::Request {
                conn: 1,
                id: 2,
                name: "sessions"
            },
            Call::Request {
                conn: 1,
                id: 3,
                name: "status"
            },
        ]
    );
    let now = system::now_ms();
    assert!(
        record
            .times
            .iter()
            .all(|time| (before..=now).contains(time))
    );
}

#[test]
fn a_parked_wait_is_answered_when_its_goal_changes() {
    let daemon = Running::start();
    let mut waiter = daemon.client(AGENT);
    let waiting = thread::spawn(move || waiter.call(wait(60_000)));
    daemon.saw(&Call::Request {
        conn: 1,
        id: 1,
        name: "wait",
    });

    // A change to another goal does not concern the wait. The second
    // request is answered after the engine thread settled the first.
    let mut writer = daemon.client(OWNER);
    writer.call(note(OTHER_GOAL, "elsewhere")).unwrap();
    writer.call(Request::Status).unwrap();
    assert!(resumes(&daemon.calls()).is_empty());
    assert!(!waiting.is_finished());

    writer.call(note(GOAL, "here")).unwrap();
    let outcome = waiting.join().unwrap().unwrap();
    assert!(matches!(outcome, Response::Waited(WaitOutcome::Work(_))));
    writer.call(Request::Status).unwrap();
    assert_eq!(
        resumes(&daemon.calls()),
        [&Call::Resume {
            conn: 1,
            id: 1,
            timed_out: false
        }]
    );
}

#[test]
fn a_change_that_does_not_concern_the_caller_parks_the_wait_again() {
    let daemon = Running::start_with(1);
    let mut waiter = daemon.client(AGENT);
    let waiting = thread::spawn(move || waiter.call(wait(60_000)));
    daemon.saw(&Call::Request {
        conn: 1,
        id: 1,
        name: "wait",
    });

    let mut writer = daemon.client(OWNER);
    writer.call(note(GOAL, "not for the waiter")).unwrap();
    writer.call(Request::Status).unwrap();
    assert_eq!(resumes(&daemon.calls()).len(), 1);
    assert!(!waiting.is_finished());

    writer.call(note(GOAL, "for the waiter")).unwrap();
    let outcome = waiting.join().unwrap().unwrap();
    assert!(matches!(outcome, Response::Waited(WaitOutcome::Work(_))));
    assert_eq!(resumes(&daemon.calls()).len(), 2);
}

#[test]
fn a_parked_wait_is_answered_when_its_time_is_up() {
    let daemon = Running::start();
    let mut client = daemon.client(AGENT);
    let started = Instant::now();
    let outcome = client.call(wait(80)).unwrap();
    let elapsed = started.elapsed();
    assert_eq!(outcome, Response::Waited(WaitOutcome::NoEvent));
    assert!(elapsed >= Duration::from_millis(80), "{elapsed:?}");
    assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
    assert_eq!(
        resumes(&daemon.calls()),
        [&Call::Resume {
            conn: 1,
            id: 1,
            timed_out: true
        }]
    );

    // The connection goes on, and a later change finds nothing parked.
    client.call(note(GOAL, "after")).unwrap();
    client.call(Request::Status).unwrap();
    assert_eq!(resumes(&daemon.calls()).len(), 1);
}

#[test]
fn a_parked_connection_that_disconnects_is_dropped() {
    let daemon = Running::start();
    let mut stream = daemon.raw(AGENT);
    send(&mut stream, &request(5, wait(60_000)));
    daemon.saw(&Call::Request {
        conn: 1,
        id: 5,
        name: "wait",
    });
    drop(stream);
    daemon.saw(&Call::Disconnect { conn: 1 });

    let mut writer = daemon.client(OWNER);
    writer.call(note(GOAL, "nobody waits")).unwrap();
    writer.call(Request::Status).unwrap();
    assert!(resumes(&daemon.calls()).is_empty());
}

#[test]
fn a_frame_over_the_limit_closes_only_its_connection() {
    let daemon = Running::start();
    let mut offender = daemon.raw(AGENT);
    let mut bystander = daemon.client(OWNER);

    let announced = MAX_LOCAL_FRAME_BYTES as u32 + 1;
    offender.write_all(&announced.to_le_bytes()).unwrap();
    assert!(closed(&mut offender));
    daemon.saw(&Call::Disconnect { conn: 1 });

    bystander.call(Request::Status).unwrap();
    daemon.client(AGENT).call(Request::Status).unwrap();
}

#[test]
fn a_frame_that_does_not_decode_closes_only_its_connection() {
    let daemon = Running::start();
    let mut offender = daemon.raw(AGENT);
    let mut bystander = daemon.client(OWNER);

    let mut garbage = Vec::new();
    codec::write_frame(&mut garbage, &[0xff; 9]).unwrap();
    offender.write_all(&garbage).unwrap();
    assert!(closed(&mut offender));
    daemon.saw(&Call::Disconnect { conn: 1 });
    // The engine never saw a request from it.
    assert!(
        daemon
            .calls()
            .iter()
            .all(|call| !matches!(call, Call::Request { conn: 1, .. }))
    );

    bystander.call(Request::Status).unwrap();

    // A frame cut short by the peer closing ends the connection too.
    let mut truncated = daemon.raw(AGENT);
    truncated.write_all(&[200, 0, 0, 0, 1, 2, 3]).unwrap();
    drop(truncated);
    daemon.saw(&Call::Disconnect { conn: 3 });
    bystander.call(Request::Status).unwrap();
}

#[test]
fn two_connections_interleave() {
    let daemon = Running::start();
    let mut first = daemon.client(OWNER);
    let mut second = daemon.client(AGENT);
    let revision = |client: &mut Client<UnixStream>| match client
        .call(Request::Pending { goal: GOAL })
        .unwrap()
    {
        Response::Pending(pending) => pending.revision,
        other => panic!("unexpected answer {other:?}"),
    };
    for _ in 0..3 {
        assert_eq!(revision(&mut first), 1);
        assert_eq!(revision(&mut second), 2);
    }
    let order: Vec<(u64, u64)> = daemon
        .calls()
        .iter()
        .filter_map(|call| match call {
            Call::Request { conn, id, .. } => Some((*conn, *id)),
            _ => None,
        })
        .collect();
    assert_eq!(order, [(1, 1), (2, 1), (1, 2), (2, 2), (1, 3), (2, 3)]);
}

#[test]
fn requests_sent_together_are_answered_in_order() {
    let daemon = Running::start();
    let mut stream = daemon.raw(AGENT);
    let mut bytes = Vec::new();
    for id in [10, 11, 12] {
        codec::encode_frame(&request(id, Request::Status), &mut bytes).unwrap();
    }
    // The last frame arrives in two pieces.
    let (together, rest) = bytes.split_at(bytes.len() - 3);
    stream.write_all(together).unwrap();
    for id in [10, 11] {
        let answer: ResponseFrame = receive(&mut stream).unwrap();
        assert_eq!(answer.id, id);
    }
    stream.write_all(rest).unwrap();
    let answer: ResponseFrame = receive(&mut stream).unwrap();
    assert_eq!(answer.id, 12);

    // A request sent while a wait is parked is answered after the wait.
    let mut bytes = Vec::new();
    codec::encode_frame(&request(20, wait(60)), &mut bytes).unwrap();
    codec::encode_frame(&request(21, Request::Status), &mut bytes).unwrap();
    stream.write_all(&bytes).unwrap();
    let answer: ResponseFrame = receive(&mut stream).unwrap();
    assert_eq!(
        (answer.id, answer.result),
        (20, Ok(Response::Waited(WaitOutcome::NoEvent)))
    );
    let answer: ResponseFrame = receive(&mut stream).unwrap();
    assert_eq!(answer.id, 21);
}

#[test]
fn large_frames_pass_in_both_directions() {
    let daemon = Running::start();
    let mut client = daemon.client(AGENT);
    let text = "x".repeat(LARGE);
    let Response::Recorded { event } = client.call(note(GOAL, &text)).unwrap() else {
        panic!("the note was not recorded");
    };
    assert_eq!(event.0[..8], (LARGE as u64).to_le_bytes());

    let get = Request::BlobGet {
        goal: GOAL,
        hash: BlobHash([3; 32]),
    };
    assert_eq!(
        client.call(get).unwrap(),
        Response::Blob {
            bytes: vec![7; LARGE]
        }
    );
    // Small frames follow on the same connection.
    client.call(Request::Status).unwrap();
}

#[test]
fn daemon_stop_ends_the_daemon_and_removes_the_socket() {
    let mut daemon = Running::start();
    let socket = daemon.socket();
    let owner_digest = daemon.log.lock().unwrap().owner_digest.unwrap();
    assert!(socket.exists());
    assert!(home::lock_holder(&daemon.home).unwrap().is_some());

    let mut waiter = daemon.client(AGENT);
    let waiting = thread::spawn(move || waiter.call(wait(60_000)));
    daemon.saw(&Call::Request {
        conn: 1,
        id: 1,
        name: "wait",
    });
    let mut idle = daemon.raw(AGENT);

    // The request that stops the daemon is itself answered.
    let mut owner = daemon.client(OWNER);
    assert_eq!(owner.call(Request::Shutdown).unwrap(), Response::Done);
    daemon.join().unwrap();

    assert!(!socket.exists());
    assert_eq!(home::lock_holder(&daemon.home).unwrap(), None);
    // A parked wait and an idle connection were closed, not answered.
    assert!(matches!(
        waiting.join().unwrap(),
        Err(ClientError::Closed | ClientError::Io(_))
    ));
    assert!(closed(&mut idle));
    assert!(UnixStream::connect(&socket).is_err());

    // The same directory starts again with the same owner credential.
    let again = Running::start_in(&daemon.home, 0);
    assert_eq!(again.log.lock().unwrap().owner_digest, Some(owner_digest));
    again.client(OWNER).call(Request::Status).unwrap();
}

#[test]
fn the_shutdown_signal_ends_the_daemon_and_removes_the_socket() {
    let mut daemon = Running::start();
    let socket = daemon.socket();
    let mut idle = daemon.raw(AGENT);
    daemon.stop().unwrap();
    assert!(!socket.exists());
    assert_eq!(home::lock_holder(&daemon.home).unwrap(), None);
    assert!(closed(&mut idle));
    // The engine was told about the connection that was still open.
    assert!(daemon.calls().contains(&Call::Disconnect { conn: 1 }));
}

#[test]
fn a_second_daemon_on_the_same_directory_is_refused() {
    let daemon = Running::start();
    let refused = run_with(
        &daemon.home,
        |_init| -> Result<Scripted, String> { panic!("the engine of a refused daemon was built") },
        |_socket| Ok(std::future::ready(())),
    )
    .unwrap_err();
    assert_eq!(refused.code, ErrorCode::Unavailable);
    assert_eq!(refused.exit_status(), 8);
    assert!(
        refused.message.contains(&daemon.home.display().to_string()),
        "{}",
        refused.message
    );
    // The running daemon kept its socket.
    daemon.client(OWNER).call(Request::Status).unwrap();
}

#[test]
fn an_engine_that_cannot_start_leaves_no_socket_and_no_lock() {
    let dir = short_dir();
    let failed = run_with(
        dir.path(),
        |_init| -> Result<Scripted, String> { Err("the store is damaged".to_string()) },
        |_socket| Ok(std::future::ready(())),
    )
    .unwrap_err();
    assert_eq!(failed.code, ErrorCode::Internal);
    assert!(failed.message.contains("the store is damaged"));
    assert!(!dir.path().join("daemon.sock").exists());
    assert_eq!(home::lock_holder(dir.path()).unwrap(), None);
}
