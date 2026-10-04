//! A deterministic, seeded simulation of several computers in one process.
//!
//! Each [`machine::Machine`] is the production [`Node`](super::Node) over
//! `MemStore`, with the real reconciliation driver inside it. The simulator
//! stands where the daemon's shell and the transport stand: it calls the
//! two seams of `locust_proto::engine` and carries `PeerOutput` between
//! machines over a network it controls. Nothing here reads a clock or a
//! random source; one seed fixes every latency, fault and secret, so a
//! failing seed replays exactly, in this process or another.
//!
//! What is simulated, and how closely:
//!
//! - **Machines** ([`machine`]): running, stopped (restart is `Node::open`
//!   over `MemStore::reopen` with the same owner credential) or asleep (the
//!   process does not run while time passes; on waking its wall clock has
//!   jumped and its connections die). Each has its own wall-clock offset.
//!   Every restart must show local callers what they could read before.
//! - **The shell** ([`world`], [`conn`]): the one-second `Poll`, a `Poll`
//!   after every local request, `Writable` after each `Send`, `Finished`
//!   only on the other end's acknowledgement, one 30-second idle deadline
//!   per exchange reset by a frame in either direction, the same deadline
//!   on connects and on admitting a connection, a paused reader while the
//!   engine owes answers, at most two connections per peer, and reuse of
//!   the first one believed alive, as in
//!   `crates/locust/src/daemon/network.rs`.
//! - **The network** ([`net`], [`stream`]): per ordered pair of machines,
//!   packets pass or not. An exchange is an ordered stream; streams are
//!   delayed independently, can be cut after any frame, or fail to open.
//!   What was sent while a route is down is kept and arrives when it
//!   returns, unless an end gave up first.
//! - **Faults** ([`chaos`]) chosen by the seed, between and during steps.
//! - **Scenarios** ([`scenario`], [`storm`]) through `Engine::connect` and
//!   `Engine::request` only, with real credentials and sessions.
//! - **Invariants** ([`check`]) once faults stop and the network is quiet.
//!
//! [`seed`] runs one seed or many and makes a failing seed smaller;
//! [`tests`] holds what cargo runs and lists the environment variables.
//!
//! The scheduler is a queue of future inputs ordered by seeded delivery
//! times. Inputs of one stream direction keep their order; inputs of
//! different streams and machines interleave as their latencies fall.
//!
//! What is not simulated is listed at the top of [`tests`].

mod chaos;
mod check;
mod conn;
mod machine;
mod net;
mod rng;
mod run;
mod scenario;
mod seed;
mod storm;
mod stream;
mod tests;
mod trace;
mod world;

/// Unix milliseconds of simulated true time zero.
const EPOCH_MS: u64 = 1_790_000_000_000;

/// The shell polls the peer engine this often
/// (`crates/locust/src/daemon/network.rs`).
const POLL_MS: u64 = 1_000;

/// The shell's deadline for one stalled transport operation (`IO_IDLE`).
const IO_IDLE_MS: u64 = 30_000;

/// How long a connection survives without hearing from its other end: the
/// transport's idle timeout. Assumed, not read from the product, which
/// leaves it at the transport's default.
const CONN_IDLE_MS: u64 = 30_000;

/// The frame limit the shell grants for one fork proof
/// (`EVIDENCE_FRAME_BYTES` in the network shell).
const EVIDENCE_FRAME_BYTES: usize = 2 * (locust_proto::limits::MAX_HEADER_BYTES + 128);
