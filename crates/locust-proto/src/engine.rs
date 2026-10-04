//! The seam between the daemon's I/O shell and the state machine.
//!
//! The shell owns sockets, timers and the clock. The engine owns every
//! decision and all state, and never blocks: each call returns at once. One
//! dedicated thread owns the engine and calls it in the order requests
//! arrive; the shell's I/O tasks exchange messages with that thread over
//! channels. That keeps the daemon single-writer without a lock and lets
//! the whole engine run inside a test with no runtime.
//!
//! The engine never reads a clock or a random source. The shell passes the
//! time into every call and supplies randomness through [`Entropy`].
//!
//! Two traits make the seam. [`Engine`] is the local socket's side: requests
//! in, answers out. [`PeerEngine`] is the transport's side: what happened on
//! the network in, what to open and send out. Both are plain values in and
//! out, so a test drives several engines against each other with no sockets.

use crate::api::{ClientHello, RequestFrame, ResponseFrame, ServerHello};
use crate::id::{EndpointId, GoalId};
use crate::sync::SyncMessage;

/// One accepted local connection, numbered by the shell. Numbers are never
/// reused while the daemon runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnId(pub u64);

/// A wait the engine cannot answer yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parked {
    /// The request frame's number, echoed in the eventual response.
    pub request_id: u64,
    /// The goal whose change will answer it.
    pub goal: GoalId,
    /// How long the caller asked to wait, from when the request arrived.
    pub timeout_ms: u32,
}

/// What the engine did with one request.
// A reply is the common case and is returned by value once per request;
// boxing it would add an allocation to every answer.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// The answer, ready to send.
    Reply(ResponseFrame),
    /// Nothing to report yet. The shell keeps the request and calls
    /// [`Engine::resume`] when the goal changes or the timeout passes. A
    /// connection has at most one parked request, because requests on a
    /// connection are answered in order.
    Park(Parked),
}

/// Random bytes for keys, salts and secrets, supplied by the shell.
pub trait Entropy {
    fn fill(&mut self, bytes: &mut [u8]);
}

/// The state machine as the local socket server sees it.
pub trait Engine {
    /// Answers a hello. After a welcome the engine remembers the connection
    /// and its caller until [`Engine::disconnect`].
    fn connect(&mut self, conn: ConnId, hello: &ClientHello, now_ms: u64) -> ServerHello;

    /// Handles one request from a welcomed connection.
    fn request(&mut self, conn: ConnId, frame: RequestFrame, now_ms: u64) -> Step;

    /// Revisits a parked wait, because its goal changed or, with `timed_out`,
    /// because its time is up. Returns the answer, or parks it again when the
    /// change did not concern this caller. With `timed_out` it always replies.
    fn resume(&mut self, conn: ConnId, parked: &Parked, timed_out: bool, now_ms: u64) -> Step;

    /// Goals that changed since this was last called, so the shell knows
    /// which parked waits to revisit. The shell calls it after every other
    /// call into the engine.
    fn take_changed(&mut self) -> Vec<GoalId>;

    /// Forgets a connection. Claims held by its session stay with the
    /// session; only the attachment ends.
    fn disconnect(&mut self, conn: ConnId);

    /// True once an owner asked the daemon to stop.
    fn stop_requested(&self) -> bool;
}

/// One exchange with a peer daemon: one bidirectional stream of
/// [`SyncMessage`] frames (see [`crate::sync`]). The side that creates an
/// exchange numbers it, so the two number spaces never collide. Numbers are
/// never reused while the daemon runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExchangeId {
    /// Opened by this daemon; numbered by the engine.
    Dialed(u64),
    /// Opened by a peer; numbered by the shell.
    Accepted(u64),
}

/// What the shell tells the engine about the network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeerInput {
    /// The transport is running under the engine's endpoint secret, or its
    /// contact hints changed. `hints` are what an invitation should carry.
    Endpoint {
        endpoint: EndpointId,
        hints: Vec<String>,
    },
    /// Whether the shell currently holds any authenticated connection to
    /// this endpoint. This is reachability, never membership or authority.
    Connection {
        endpoint: EndpointId,
        connected: bool,
    },
    /// An exchange the engine asked for is open and may be written to.
    Opened(ExchangeId),
    /// An exchange the engine asked for could not be opened. Nothing more
    /// arrives for it.
    OpenFailed(ExchangeId),
    /// A peer opened an exchange. `remote` is the endpoint the transport
    /// authenticated, which is not yet a member of anything.
    Accepted {
        exchange: ExchangeId,
        remote: EndpointId,
    },
    /// The next frame of an exchange, in order.
    Frame {
        exchange: ExchangeId,
        frame: SyncMessage,
    },
    /// The previous output frame was written successfully. The engine may
    /// produce the next frame; one notification follows each `Send`.
    Writable(ExchangeId),
    /// The shell finished the exchange as asked and the transport confirmed
    /// delivery of every queued frame. Nothing more arrives for it.
    Finished(ExchangeId),
    /// The exchange ended without confirmed completion: the peer ended it
    /// or the link failed. Nothing more arrives for it, and anything still
    /// queued for it was dropped. This is never a successful finish.
    Closed(ExchangeId),
    /// Nothing happened on the network. The shell sends this after local
    /// requests changed a goal and at least once a second, so the engine can
    /// start exchanges and act on its own deadlines.
    Poll,
}

/// What the engine asks of the transport. The shell carries out the outputs
/// of one call in order; frames for one exchange are sent in the order given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeerOutput {
    /// Open an exchange with this endpoint. `hints` may be empty: finding an
    /// endpoint by its key is the transport's job. Answered by
    /// [`PeerInput::Opened`] or [`PeerInput::OpenFailed`].
    Open {
        exchange: ExchangeId,
        endpoint: EndpointId,
        hints: Vec<String>,
    },
    /// Send one frame. Answered by `Writable` after the write completes, or
    /// `Closed` if it fails. Producers wait for capacity before emitting the
    /// next frame of a potentially large response.
    Send {
        exchange: ExchangeId,
        frame: SyncMessage,
    },
    /// The remote endpoint of this accepted exchange speaks for a member:
    /// read its later frames with the peer frame limit instead of the hello
    /// limit. Exchanges this daemon opened are read at the peer limit from
    /// the start.
    Admit(ExchangeId),
    /// End the exchange once everything sent on it was delivered, waiting
    /// under the shell's deadline for the transport's acknowledgement.
    /// Answered by [`PeerInput::Finished`] on success or [`PeerInput::Closed`]
    /// on failure.
    Finish(ExchangeId),
}

/// The state machine as the transport sees it.
pub trait PeerEngine {
    /// The 32-byte secret the transport's endpoint identity is derived from.
    /// The engine creates it on first start and keeps it in its store.
    fn endpoint_secret(&self) -> [u8; 32];

    /// Handles one input and appends what the transport should do to `out`.
    /// The shell calls [`Engine::take_changed`] afterwards, because a frame
    /// from a peer can answer a parked wait.
    fn peer(&mut self, input: PeerInput, now_ms: u64, out: &mut Vec<PeerOutput>);
}
