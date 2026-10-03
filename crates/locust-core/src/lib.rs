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
//! Depends on `locust-proto` only. Owner: the core stream; see
//! `docs/workstreams.md`.

#![forbid(unsafe_code)]
