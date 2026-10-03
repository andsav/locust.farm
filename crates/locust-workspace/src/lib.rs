//! Workspace snapshots and patches.
//!
//! This crate runs inside the trusted CLI or client adapter, never inside the
//! daemon: it reads the files a participant chose to share, builds a
//! [`locust_proto::manifest::Manifest`], materializes a manifest into a
//! separate directory, and produces and applies patches bound to a manifest.
//! It reads Git objects directly with isolated configuration and never runs
//! filters, hooks or anything a received tree names.
//!
//! Depends on `locust-proto` only. Owner: the workspace stream; see
//! `docs/workstreams.md`.

#![forbid(unsafe_code)]
