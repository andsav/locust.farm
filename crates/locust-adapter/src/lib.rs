//! Client lifecycle adapters.
//!
//! This crate knows how each supported coding client is configured, launched,
//! observed and woken: per-run configuration, readiness, session binding,
//! hooks and diagnostics. It talks to the daemon only through the local API
//! in [`locust_proto::api`] and keeps its records in the store's session
//! space; it adds no second collaboration store or protocol. Logic common to
//! all clients stays separate from each client's argument, configuration and
//! event formats.
//!
//! Depends on `locust-proto` only. Owner: the client lifecycle stream; see
//! `docs/workstreams.md`.

#![forbid(unsafe_code)]

pub mod config;

pub mod delivery;
pub mod managed;
