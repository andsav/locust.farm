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

use crate::api::{ClientHello, RequestFrame, ResponseFrame, ServerHello};
use crate::id::GoalId;

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
