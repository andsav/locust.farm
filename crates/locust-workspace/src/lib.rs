//! Workspace snapshots.
//!
//! This crate runs inside the trusted CLI, never inside the daemon, and never
//! talks to the daemon itself. Snapshot operations:
//!
//! - [`export()`] reads the files of one named Git commit under an export
//!   root, leaves out credentials and private material, stores each file and
//!   then the [`Manifest`] through a [`BlobSink`], and reports what it shared,
//!   left out and refused.
//! - [`materialize()`] writes the files a manifest names, read through a
//!   [`BlobSource`], into a new directory that appears only once every file is
//!   written and checked.
//!
//! The CLI implements [`BlobSink`] and [`BlobSource`] over the local API; the
//! daemon seals content on store and opens it on read, so this crate handles
//! plaintext only. A manifest entry names whatever identifier the sink
//! returned for that file, and its size is the plaintext size.
//!
//! Git objects are read with plumbing commands under an isolated
//! configuration (see `git.rs`): no checkout, filter, text conversion or hook
//! runs, and nothing a tree or a received manifest names is ever executed.
//! Only regular files and the executable bit are carried; a symlink, a
//! submodule or any other entry is an explicit error.
//!
//! [`capture_seed`] and [`capture_tree`] freeze ordinary directory snapshots.
//! [`compose_trees`] combines exact manifest trees; [`plan_update`] preserves
//! compatible local edits. [`prepare_update`] and [`reopen_update`] use durable
//! journals outside the managed root for interrupted file transitions.
//!
//! Unix only (macOS and Linux): materialization sets modes through
//! `std::os::unix`; contribution mutations use descriptor-relative `rustix`
//! filesystem operations. See `docs/crates.md`.
//!
//! [`Manifest`]: locust_proto::manifest::Manifest

#![forbid(unsafe_code)]

mod disposition;
mod export;
mod files;
mod git;
mod materialize;
mod review;
mod safe_fs;
mod select;
mod transaction;
mod tree;

use std::io;

use locust_proto::id::BlobHash;

pub use disposition::{
    CheckoutDisposition, CheckoutFacts, SessionOwnership, UpdateBlocker, checkout_disposition,
};
pub use export::{ExportError, ExportReport, export};
pub use files::{BlobStore, WorkspaceError};
pub use materialize::{MaterializeError, materialize};
pub use review::{ChangeReview, FileSummary, review_changes};
pub use transaction::{
    DirectoryIdentity, PreparedUpdate, UpdateDescriptor, UpdateReport, mark_update_completed,
    prepare_update, reopen_update,
};
pub use tree::{
    CaptureMode, FileDigest, FileValue, FrozenTree, LocalTree, TreeChange, TreeFile, UpdatePlan,
    capture_seed, capture_tree, compose_trees, diff_trees, file_digests, inspect_local_tree,
    inspect_tree, parse_paths, plan_update, three_way_tree,
};

/// Stores content objects for an export.
pub trait BlobSink {
    /// Stores `plaintext` and returns the identifier the daemon assigned to
    /// it, which is what a manifest entry names.
    fn store(&mut self, plaintext: &[u8]) -> io::Result<BlobHash>;
}

/// Reads content objects for a materialization.
pub trait BlobSource {
    /// Returns the plaintext of the object `hash` names, or `None` if the
    /// object is not available.
    fn fetch(&mut self, hash: &BlobHash) -> io::Result<Option<Vec<u8>>>;
}
