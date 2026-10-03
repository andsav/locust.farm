//! Peer transport.
//!
//! This crate turns "reach that endpoint" into an authenticated, ordered
//! stream of frames carrying [`locust_proto::sync::SyncMessage`]. It owns the
//! endpoint, relay and address configuration, connection reuse and an
//! in-memory link for tests. It does not decide who is a member or what to
//! send: the binary feeds received frames to the state machine and writes
//! back what the state machine returns.
//!
//! Depends on `locust-proto` only. Owner: the transport stream; see
//! `docs/workstreams.md`.

#![forbid(unsafe_code)]
