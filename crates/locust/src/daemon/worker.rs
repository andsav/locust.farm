//! The one thread that owns the engine.
//!
//! Connection tasks hand it [`Job`]s over a channel and it calls the engine
//! in the order they arrive, so the engine has a single writer and needs no
//! lock. Each connection can have at most one request outstanding, which
//! bounds the queue by the number of connections.
//!
//! The thread also holds the waits the engine parked. After every call into
//! the engine it asks which goals changed and revisits the waits on those
//! goals; a connection task tells it when a wait's time is up.

use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use crate::failure::Failure;
use locust_proto::api::{
    ApiError, ClientHello, ErrorCode, Request, RequestFrame, ResponseFrame, ServerHello,
};
use locust_proto::engine::{
    ConnId, Engine, ExchangeId, Parked, PeerEngine, PeerInput, PeerOutput, PeerTime, Step,
};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::watch;

/// Wall time for local requests and peer diagnostics: Unix milliseconds.
/// Peer scheduling also receives the worker's monotonic elapsed time.
pub(crate) type Clock = fn() -> u64;

/// What a connection task asks of the engine thread.
pub(crate) enum Job {
    /// A hello arrived. Its answer, and every later answer for the
    /// connection, goes to `answers`.
    Connect {
        conn: ConnId,
        hello: ClientHello,
        answers: UnboundedSender<Answer>,
    },
    /// A request arrived on a welcomed connection.
    Request { conn: ConnId, frame: RequestFrame },
    /// The network refreshed contact hints before creating an invitation.
    InviteReady { conn: ConnId, frame: RequestFrame },
    /// The time of the connection's parked request `request_id` is up.
    Expire { conn: ConnId, request_id: u64 },
    /// A welcomed connection ended.
    Disconnect { conn: ConnId },
    /// One transport input, serialized with local requests.
    Peer(PeerInput),
    /// A received frame waits until its outputs (including admission) are dispatched.
    PeerFrame {
        input: PeerInput,
        processed: tokio::sync::oneshot::Sender<()>,
    },
    /// Every connection has ended; the thread drops the engine and exits.
    Stop,
}

/// What the engine thread tells a connection task.
// A reply is the common case and is moved once per request; boxing it would
// add an allocation to every answer.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Answer {
    Hello(ServerHello),
    Reply(ResponseFrame),
    /// The request is parked. Its reply follows when its goal changes, or
    /// after the task reports that `timeout_ms` have passed since the
    /// request arrived.
    Parked {
        timeout_ms: u32,
    },
}

/// The running engine thread, as the rest of the shell holds it.
pub(crate) struct EngineThread {
    /// Where connection tasks send their jobs.
    pub(crate) jobs: Sender<Job>,
    /// Set to true when the daemon is to stop: by the engine thread once an
    /// owner asked for it, or by the accept loop on a signal.
    pub(crate) stop: watch::Sender<bool>,
    thread: Option<JoinHandle<Result<(), Failure>>>,
    pub(crate) endpoint_secret: Option<[u8; 32]>,
    pub(crate) outgoing: Option<tokio::sync::mpsc::UnboundedReceiver<NetworkOutput>>,
}

impl EngineThread {
    /// Starts the thread and builds the engine on it, so the engine itself
    /// never crosses threads. Returns once the engine exists, or with the
    /// reason it could not be built.
    #[cfg(test)]
    pub(crate) fn start<E, F>(make_engine: F, clock: Clock) -> Result<Self, Failure>
    where
        E: Engine + 'static,
        F: FnOnce() -> Result<E, String> + Send + 'static,
    {
        Self::start_inner(|| make_engine().map_err(Failure::internal), clock, None)
    }

    pub(crate) fn start_networked<E, F>(make_engine: F, clock: Clock) -> Result<Self, Failure>
    where
        E: Engine + PeerEngine + 'static,
        F: FnOnce() -> Result<E, Failure> + Send + 'static,
    {
        Self::start_inner(
            make_engine,
            clock,
            Some((E::endpoint_secret, E::peer, E::peer_readable)),
        )
    }

    fn start_inner<E, F>(
        make_engine: F,
        clock: Clock,
        peer: Option<PeerHooks<E>>,
    ) -> Result<Self, Failure>
    where
        E: Engine + 'static,
        F: FnOnce() -> Result<E, Failure> + Send + 'static,
    {
        let (jobs, inbox) = mpsc::channel();
        let (stop, _) = watch::channel(false);
        let (ready, built) = mpsc::channel();
        let (output, outgoing) = tokio::sync::mpsc::unbounded_channel();
        let thread = thread::Builder::new()
            .name("locust-engine".to_string())
            .spawn({
                let stop = stop.clone();
                move || match make_engine() {
                    Ok(engine) => {
                        let endpoint_secret = peer.map(|(secret, _, _)| secret(&engine));
                        let _ = ready.send(Ok(endpoint_secret));
                        Worker::new(
                            engine,
                            clock,
                            stop,
                            peer.map(|(_, handle, readable)| (handle, readable)),
                            output,
                        )
                        .run(&inbox)
                    }
                    Err(reason) => {
                        let _ = ready.send(Err(reason.clone()));
                        Err(reason)
                    }
                }
            })
            .map_err(|error| {
                Failure::internal(format!("the engine thread could not be started: {error}"))
            })?;
        match built.recv() {
            Ok(Ok(endpoint_secret)) => Ok(Self {
                jobs,
                stop,
                thread: Some(thread),
                endpoint_secret,
                outgoing: Some(outgoing),
            }),
            Ok(Err(reason)) => {
                let _ = thread.join();
                Err(reason)
            }
            Err(_) => {
                let _ = thread.join();
                Err(Failure::internal(
                    "the engine thread ended before the engine was built",
                ))
            }
        }
    }

    /// Lets the thread finish the jobs already queued, drops the engine and
    /// waits for the thread to end.
    pub(crate) fn shutdown(mut self) -> Result<(), Failure> {
        self.join()
    }

    fn join(&mut self) -> Result<(), Failure> {
        if let Some(thread) = self.thread.take() {
            let _ = self.jobs.send(Job::Stop);
            return thread
                .join()
                .map_err(|_| Failure::internal("the engine thread panicked"))?;
        }
        Ok(())
    }
}

impl Drop for EngineThread {
    fn drop(&mut self) {
        // Startup errors must release the store before StateDir releases its lock.
        let _ = self.join();
    }
}

/// A welcomed connection as the engine thread knows it.
struct Attached {
    answers: UnboundedSender<Answer>,
    /// The wait the engine parked for this connection, if any.
    parked: Option<Parked>,
}

// Frames already own their buffers; keep each dispatched action allocation-free.
#[allow(clippy::large_enum_variant)]
pub(crate) enum NetworkOutput {
    Action(PeerOutput),
    Processed(tokio::sync::oneshot::Sender<()>),
    Invite { conn: ConnId, frame: RequestFrame },
}

type PeerHandler<E> = fn(&mut E, PeerInput, PeerTime, &mut Vec<PeerOutput>);
type PeerReader<E> = fn(&E, ExchangeId) -> bool;
type PeerHooks<E> = (fn(&E) -> [u8; 32], PeerHandler<E>, PeerReader<E>);

struct Worker<E> {
    peer: Option<(PeerHandler<E>, PeerReader<E>)>,
    output: UnboundedSender<NetworkOutput>,
    frames: Vec<PeerOutput>,
    blocked: HashMap<ExchangeId, tokio::sync::oneshot::Sender<()>>,
    engine: E,
    clock: Clock,
    started: std::time::Instant,
    stop: watch::Sender<bool>,
    stop_announced: bool,
    conns: BTreeMap<ConnId, Attached>,
    /// How many connections have a parked wait.
    parked: usize,
    /// Connections to revisit, reused between changes.
    due: Vec<(ConnId, Parked)>,
}

impl<E: Engine> Worker<E> {
    fn new(
        engine: E,
        clock: Clock,
        stop: watch::Sender<bool>,
        peer: Option<(PeerHandler<E>, PeerReader<E>)>,
        output: UnboundedSender<NetworkOutput>,
    ) -> Self {
        Self {
            engine,
            peer,
            output,
            frames: Vec::new(),
            blocked: HashMap::new(),
            clock,
            started: std::time::Instant::now(),
            stop,
            stop_announced: false,
            conns: BTreeMap::new(),
            parked: 0,
            due: Vec::new(),
        }
    }

    fn run(mut self, inbox: &Receiver<Job>) -> Result<(), Failure> {
        while let Ok(job) = inbox.recv() {
            match job {
                Job::Connect {
                    conn,
                    hello,
                    answers,
                } => self.connect(conn, &hello, answers),
                Job::Request { conn, frame }
                    if self.peer.is_some()
                        && matches!(frame.request, Request::GoalInvite { .. }) =>
                {
                    let id = frame.id;
                    if self
                        .output
                        .send(NetworkOutput::Invite { conn, frame })
                        .is_err()
                    {
                        self.deliver(
                            conn,
                            Step::Reply(ResponseFrame {
                                id,
                                result: Err(ApiError::new(
                                    ErrorCode::Unavailable,
                                    "the peer transport is unavailable",
                                )),
                            }),
                        );
                    }
                }
                Job::Request { conn, frame } | Job::InviteReady { conn, frame } => {
                    if !self.conns.contains_key(&conn) {
                        continue;
                    }
                    let step = self.engine.request(conn, frame, (self.clock)());
                    self.deliver(conn, step);
                    self.network(PeerInput::Poll);
                }
                Job::Peer(input) => self.network(input),
                Job::PeerFrame { input, processed } => {
                    let exchange = match &input {
                        PeerInput::Frame { exchange, .. } => *exchange,
                        _ => unreachable!("only frames await processing"),
                    };
                    self.network(input);
                    self.blocked.insert(exchange, processed);
                }
                Job::Expire { conn, request_id } => self.expire(conn, request_id),
                Job::Disconnect { conn } => self.disconnect(conn),
                Job::Stop => break,
            }
            self.settle();
            if let Some((_, readable)) = self.peer {
                let ready: Vec<_> = self
                    .blocked
                    .keys()
                    .copied()
                    .filter(|exchange| readable(&self.engine, *exchange))
                    .collect();
                for exchange in ready {
                    if let Some(processed) = self.blocked.remove(&exchange) {
                        let _ = self.output.send(NetworkOutput::Processed(processed));
                    }
                }
            }
        }
        self.engine
            .failure()
            .map_or(Ok(()), |error| Err(error.into()))
    }

    fn network(&mut self, input: PeerInput) {
        if let Some((peer, _)) = self.peer {
            peer(
                &mut self.engine,
                input,
                PeerTime {
                    unix_ms: (self.clock)(),
                    elapsed_ms: u64::try_from(self.started.elapsed().as_millis())
                        .unwrap_or(u64::MAX),
                },
                &mut self.frames,
            );
            for output in self.frames.drain(..) {
                if self.output.send(NetworkOutput::Action(output)).is_err() {
                    break;
                }
            }
        }
    }

    fn connect(&mut self, conn: ConnId, hello: &ClientHello, answers: UnboundedSender<Answer>) {
        let answer = self.engine.connect(conn, hello, (self.clock)());
        let welcomed = matches!(answer, ServerHello::Welcome { .. });
        let delivered = answers.send(Answer::Hello(answer)).is_ok();
        if welcomed && delivered {
            self.conns.insert(
                conn,
                Attached {
                    answers,
                    parked: None,
                },
            );
        } else if welcomed {
            // The task went away before its welcome; it will not report the end.
            self.engine.disconnect(conn);
        }
    }

    /// Sends a reply to its connection, or records that its request is parked.
    fn deliver(&mut self, conn: ConnId, step: Step) {
        let Some(attached) = self.conns.get_mut(&conn) else {
            return;
        };
        match step {
            Step::Reply(frame) => {
                if attached.parked.take().is_some() {
                    self.parked -= 1;
                }
                // A task that is gone reports its own end.
                let _ = attached.answers.send(Answer::Reply(frame));
            }
            Step::Park(parked) => {
                if attached.parked.replace(parked).is_none() {
                    self.parked += 1;
                    let _ = attached.answers.send(Answer::Parked {
                        timeout_ms: parked.timeout_ms,
                    });
                }
            }
        }
    }

    fn expire(&mut self, conn: ConnId, request_id: u64) {
        let parked = match self.conns.get(&conn).and_then(|attached| attached.parked) {
            Some(parked) if parked.request_id == request_id => parked,
            // Already answered by a change.
            _ => return,
        };
        let step = match self.engine.resume(conn, &parked, true, (self.clock)()) {
            Step::Reply(frame) => Step::Reply(frame),
            Step::Park(_) => Step::Reply(ResponseFrame {
                id: request_id,
                result: Err(ApiError::new(
                    ErrorCode::Internal,
                    "a wait whose time was up was not answered",
                )),
            }),
        };
        self.deliver(conn, step);
    }

    fn disconnect(&mut self, conn: ConnId) {
        if let Some(attached) = self.conns.remove(&conn) {
            if attached.parked.is_some() {
                self.parked -= 1;
            }
            self.engine.disconnect(conn);
        }
    }

    /// Runs after every job: revisits the parked waits of goals that
    /// changed, until nothing changes any more, and passes on a request to
    /// stop.
    fn settle(&mut self) {
        loop {
            let changed = self.engine.take_changed();
            if changed.is_empty() || self.parked == 0 {
                break;
            }
            self.due.clear();
            self.due
                .extend(self.conns.iter().filter_map(|(conn, attached)| {
                    attached
                        .parked
                        .filter(|parked| changed.contains(&parked.goal))
                        .map(|parked| (*conn, parked))
                }));
            for index in 0..self.due.len() {
                let (conn, parked) = self.due[index];
                let step = self.engine.resume(conn, &parked, false, (self.clock)());
                self.deliver(conn, step);
            }
        }
        if !self.stop_announced && self.engine.stop_requested() {
            self.stop_announced = true;
            self.stop.send_replace(true);
        }
    }
}

impl<E> Drop for Worker<E> {
    fn drop(&mut self) {
        // A failed engine thread cannot leave a live socket serving nobody.
        self.stop.send_replace(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_proto::id::GoalId;
    use locust_proto::sync::SyncMessage;

    struct Fixture {
        remaining: usize,
        failed: bool,
    }
    impl Engine for Fixture {
        fn connect(&mut self, _: ConnId, _: &ClientHello, _: u64) -> ServerHello {
            ServerHello::Welcome {
                api_version: locust_proto::API_VERSION,
                daemon_version: "fixture".into(),
                caller: locust_proto::api::Caller::Owner,
                max_blob_bytes: 1024,
            }
        }
        fn request(&mut self, _: ConnId, frame: RequestFrame, _: u64) -> Step {
            Step::Reply(ResponseFrame {
                id: frame.id,
                result: Err(ApiError::new(ErrorCode::Invalid, "fixture answered")),
            })
        }
        fn resume(&mut self, _: ConnId, _: &Parked, _: bool, _: u64) -> Step {
            panic!("unused")
        }
        fn take_changed(&mut self) -> Vec<GoalId> {
            vec![]
        }
        fn disconnect(&mut self, _: ConnId) {}
        fn stop_requested(&self) -> bool {
            self.failed
        }
        fn failure(&self) -> Option<ApiError> {
            self.failed
                .then(|| ApiError::new(ErrorCode::Corrupted, "fixture durability failure"))
        }
    }
    impl PeerEngine for Fixture {
        fn endpoint_secret(&self) -> [u8; 32] {
            [0; 32]
        }
        fn peer(&mut self, input: PeerInput, _: PeerTime, out: &mut Vec<PeerOutput>) {
            let exchange = match input {
                PeerInput::Frame { exchange, .. } => {
                    self.remaining = 2;
                    exchange
                }
                PeerInput::Writable(exchange) => {
                    self.remaining -= 1;
                    exchange
                }
                _ => return,
            };
            if self.remaining > 0 {
                out.push(PeerOutput::Send {
                    exchange,
                    frame: SyncMessage::Events(vec![]),
                });
            }
        }
        fn peer_readable(&self, _: ExchangeId) -> bool {
            self.remaining == 0
        }
    }

    #[tokio::test]
    async fn request_acknowledgement_waits_for_the_whole_response_to_drain() {
        let mut thread = EngineThread::start_networked(
            || {
                Ok(Fixture {
                    remaining: 0,
                    failed: false,
                })
            },
            || 0,
        )
        .unwrap();
        let mut output = thread.outgoing.take().unwrap();
        let exchange = ExchangeId::Accepted(1);
        let (processed, _applied) = tokio::sync::oneshot::channel();
        thread
            .jobs
            .send(Job::PeerFrame {
                input: PeerInput::Frame {
                    exchange,
                    frame: SyncMessage::Frontier(Default::default()),
                },
                processed,
            })
            .unwrap();
        for _ in 0..2 {
            assert!(matches!(
                output.recv().await,
                Some(NetworkOutput::Action(PeerOutput::Send { .. }))
            ));
            assert!(
                output.try_recv().is_err(),
                "reader was released before response completion"
            );
            thread
                .jobs
                .send(Job::Peer(PeerInput::Writable(exchange)))
                .unwrap();
        }
        assert!(matches!(
            output.recv().await,
            Some(NetworkOutput::Processed(_))
        ));
        thread.shutdown().unwrap();
    }

    #[test]
    fn typed_startup_and_terminal_engine_failures_survive_thread_cleanup() {
        let error = EngineThread::start_networked::<Fixture, _>(
            || Err(Failure::new(ErrorCode::Corrupted, "damaged store")),
            || 0,
        )
        .err()
        .unwrap();
        assert_eq!(error.code, ErrorCode::Corrupted);
        let thread = EngineThread::start_networked(
            || {
                Ok(Fixture {
                    remaining: 0,
                    failed: true,
                })
            },
            || 0,
        )
        .unwrap();
        thread.jobs.send(Job::Peer(PeerInput::Poll)).unwrap();
        let error = thread.shutdown().unwrap_err();
        assert_eq!(error.code, ErrorCode::Corrupted);
    }
    #[tokio::test]
    async fn pending_invite_readiness_does_not_block_other_requests_or_peer_progress() {
        use locust_proto::api::Credential;
        let mut thread = EngineThread::start_networked(
            || {
                Ok(Fixture {
                    remaining: 0,
                    failed: false,
                })
            },
            || 123,
        )
        .unwrap();
        let mut output = thread.outgoing.take().unwrap();
        let (answers, mut replies) = tokio::sync::mpsc::unbounded_channel();
        thread
            .jobs
            .send(Job::Connect {
                conn: ConnId(1),
                hello: ClientHello {
                    api_version: locust_proto::API_VERSION,
                    credential: Credential([1; 32]),
                    session: None,
                },
                answers,
            })
            .unwrap();
        assert!(matches!(replies.recv().await, Some(Answer::Hello(_))));
        let frame = |id, request| RequestFrame {
            id,
            idempotency: None,
            on_behalf: None,
            request,
        };
        thread
            .jobs
            .send(Job::Request {
                conn: ConnId(1),
                frame: frame(
                    1,
                    Request::GoalInvite {
                        goal: GoalId([1; 32]),
                        expires_ms: None,
                    },
                ),
            })
            .unwrap();
        let Some(NetworkOutput::Invite {
            conn,
            frame: invite,
        }) = output.recv().await
        else {
            panic!("invite readiness")
        };
        assert!(
            replies.try_recv().is_err(),
            "invite answered before refreshed hints"
        );
        thread
            .jobs
            .send(Job::Request {
                conn,
                frame: frame(2, Request::Status),
            })
            .unwrap();
        let reply = tokio::time::timeout(std::time::Duration::from_secs(2), replies.recv())
            .await
            .unwrap();
        assert!(matches!(
            reply,
            Some(Answer::Reply(ResponseFrame { id: 2, .. }))
        ));
        thread
            .jobs
            .send(Job::Peer(PeerInput::Frame {
                exchange: ExchangeId::Accepted(1),
                frame: SyncMessage::Frontier(Default::default()),
            }))
            .unwrap();
        let action = tokio::time::timeout(std::time::Duration::from_secs(2), output.recv())
            .await
            .unwrap();
        assert!(matches!(
            action,
            Some(NetworkOutput::Action(PeerOutput::Send { .. }))
        ));
        thread
            .jobs
            .send(Job::InviteReady {
                conn,
                frame: invite,
            })
            .unwrap();
        assert!(matches!(
            replies.recv().await,
            Some(Answer::Reply(ResponseFrame { id: 1, .. }))
        ));
        thread.shutdown().unwrap();
    }
}
