//! Durable storage: the SQLite implementation of
//! [`locust_proto::store::Store`].
//!
//! One database file holds the event log, its indexes, small content objects
//! and the local records; large content objects are files made durable before
//! the transaction that references them commits. A commit is one transaction.
//! The implementation must pass `locust_proto::store::conformance::run` and
//! add crash tests around every durability boundary.
//!
//! Depends on `locust-proto` only. Owner: the storage stream; see
//! `docs/workstreams.md`.

#![forbid(unsafe_code)]
