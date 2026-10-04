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

use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use locust_proto::api::{
    ApiError, ClientHello, ErrorCode, RequestFrame, ResponseFrame, ServerHello,
};
use locust_proto::engine::{ConnId, Engine, Parked, PeerEngine, PeerInput, PeerOutput, Step};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::watch;

/// The time passed into every engine call: Unix milliseconds.
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
    thread: Option<JoinHandle<()>>,
    pub(crate) endpoint_secret: Option<[u8; 32]>,
    pub(crate) outgoing: Option<tokio::sync::mpsc::UnboundedReceiver<NetworkOutput>>,
}

impl EngineThread {
    /// Starts the thread and builds the engine on it, so the engine itself
    /// never crosses threads. Returns once the engine exists, or with the
    /// reason it could not be built.
    #[cfg(test)]
    pub(crate) fn start<E, F>(make_engine: F, clock: Clock) -> Result<Self, String>
    where
        E: Engine + 'static,
        F: FnOnce() -> Result<E, String> + Send + 'static,
    {
        Self::start_inner(make_engine, clock, None)
    }

    pub(crate) fn start_networked<E, F>(make_engine: F, clock: Clock) -> Result<Self, String>
    where
        E: Engine + PeerEngine + 'static,
        F: FnOnce() -> Result<E, String> + Send + 'static,
    {
        Self::start_inner(make_engine, clock, Some((E::endpoint_secret, E::peer)))
    }

    fn start_inner<E, F>(
        make_engine: F,
        clock: Clock,
        peer: Option<PeerHooks<E>>,
    ) -> Result<Self, String>
    where
        E: Engine + 'static,
        F: FnOnce() -> Result<E, String> + Send + 'static,
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
                        let endpoint_secret = peer.map(|(secret, _)| secret(&engine));
                        let _ = ready.send(Ok(endpoint_secret));
                        Worker::new(engine, clock, stop, peer.map(|(_, handle)| handle), output)
                            .run(&inbox);
                    }
                    Err(reason) => {
                        let _ = ready.send(Err(reason));
                    }
                }
            })
            .map_err(|error| format!("the engine thread could not be started: {error}"))?;
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
                Err("the engine thread ended before the engine was built".to_string())
            }
        }
    }

    /// Lets the thread finish the jobs already queued, drops the engine and
    /// waits for the thread to end.
    pub(crate) fn shutdown(mut self) {
        self.join();
    }

    fn join(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = self.jobs.send(Job::Stop);
            let _ = thread.join();
        }
    }
}

impl Drop for EngineThread {
    fn drop(&mut self) {
        // Startup errors must release the store before StateDir releases its lock.
        self.join();
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
}

type PeerHandler<E> = fn(&mut E, PeerInput, u64, &mut Vec<PeerOutput>);
type PeerHooks<E> = (fn(&E) -> [u8; 32], PeerHandler<E>);

struct Worker<E> {
    peer: Option<PeerHandler<E>>,
    output: UnboundedSender<NetworkOutput>,
    frames: Vec<PeerOutput>,
    engine: E,
    clock: Clock,
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
        peer: Option<PeerHandler<E>>,
        output: UnboundedSender<NetworkOutput>,
    ) -> Self {
        Self {
            engine,
            peer,
            output,
            frames: Vec::new(),
            clock,
            stop,
            stop_announced: false,
            conns: BTreeMap::new(),
            parked: 0,
            due: Vec::new(),
        }
    }

    fn run(mut self, inbox: &Receiver<Job>) {
        while let Ok(job) = inbox.recv() {
            match job {
                Job::Connect {
                    conn,
                    hello,
                    answers,
                } => self.connect(conn, &hello, answers),
                Job::Request { conn, frame } => {
                    let step = self.engine.request(conn, frame, (self.clock)());
                    self.deliver(conn, step);
                    self.network(PeerInput::Poll);
                }
                Job::Peer(input) => self.network(input),
                Job::PeerFrame { input, processed } => {
                    self.network(input);
                    let _ = self.output.send(NetworkOutput::Processed(processed));
                }
                Job::Expire { conn, request_id } => self.expire(conn, request_id),
                Job::Disconnect { conn } => self.disconnect(conn),
                Job::Stop => break,
            }
            self.settle();
        }
    }

    fn network(&mut self, input: PeerInput) {
        if let Some(peer) = self.peer {
            peer(&mut self.engine, input, (self.clock)(), &mut self.frames);
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
