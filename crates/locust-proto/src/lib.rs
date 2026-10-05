//! The Locust contract: identifiers, signed events, the local API, peer sync
//! frames and the storage seam.
//!
//! Every library crate in the workspace depends on this crate and on no other
//! workspace crate; only the `locust` binary wires them together. Everything
//! here is plain data and pure functions: no I/O beyond `std::io` traits, no
//! async runtime, no global state. See `docs/crates.md`.

#![forbid(unsafe_code)]

pub mod api;
pub mod client;
pub mod codec;
pub mod contribution;
pub mod crypto;
pub mod engine;
pub mod event;
pub mod farm;
pub mod id;
pub mod invite;
pub mod limits;
pub mod local;
pub mod manifest;
pub mod organization;
pub mod seal;
pub mod store;
pub mod sync;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;
#[cfg(test)]
mod vectors;

/// Version byte carried by every signed header and invitation. A peer that
/// sees another value reports an unsupported version instead of guessing.
pub const PROTOCOL_VERSION: u8 = 5;

/// Version of the local daemon API spoken over the Unix socket.
pub const API_VERSION: u16 = 5;
