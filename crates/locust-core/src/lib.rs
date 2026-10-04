//! The Locust state machine.
//!
//! This crate decides what a goal's history means: which events are
//! authorized, what state tasks, membership and documents are in, what a
//! local request should append, and what a peer is missing. It performs no
//! I/O and reads no clock or random source; callers pass in the time, any
//! random bytes and a [`locust_proto::store::Store`]. The same code path
//! validates events written locally and events received from peers.
//!
//! Because nothing here blocks or spawns, several nodes can run inside one
//! test and exchange messages in any order, which is how delayed, duplicated,
//! reordered and partitioned delivery is exercised deterministically.
//!
//! Three parts, each depending only on the ones before it:
//!
//! - [`goal`]: what one goal's held events mean. The state of a goal is a
//!   function of the set of events held, never of the order they arrived in.
//! - [`sync`]: reconciliation between two daemons, as state machines with
//!   frames in and frames out.
//! - [`node`]: the daemon's state machine. It owns the store, the principals
//!   and every goal, and implements the two seams of
//!   [`locust_proto::engine`].
//!
//! Depends on `locust-proto` only. Owner: the core stream; see
//! `docs/workstreams.md`.

#![forbid(unsafe_code)]

pub mod goal;
pub mod node;
pub mod organization;
pub mod sync;
