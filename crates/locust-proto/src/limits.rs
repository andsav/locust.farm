//! Published size limits. Input is checked against these before it is
//! allocated, decoded or retained, including input from admitted members.
//! Violations are rejected and reported; nothing is silently truncated.

/// Largest signed header, in bytes.
pub const MAX_HEADER_BYTES: usize = 16 * 1024;

/// Largest number of causal parents one event may name.
pub const MAX_PARENTS: usize = 64;

/// Largest number of task dependencies one proposal may name.
pub const MAX_DEPENDENCIES: usize = 64;

/// Largest event payload (task text, note, result summary, document revision).
pub const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;

/// Default ceiling for one stored content object. Operators may lower or raise
/// it; the daemon reports the effective value.
pub const DEFAULT_MAX_BLOB_BYTES: usize = 64 * 1024 * 1024;

/// Largest number of events in one sync batch.
pub const MAX_EVENTS_PER_BATCH: usize = 256;

/// Largest frame on the local API socket and on a peer link. A frame carries
/// at most one content object plus a small envelope.
pub const MAX_FRAME_BYTES: usize = DEFAULT_MAX_BLOB_BYTES + 64 * 1024;

/// Largest number of entries in one workspace manifest.
pub const MAX_MANIFEST_ENTRIES: usize = 100_000;

/// Longest relative path in a manifest, in bytes.
pub const MAX_PATH_BYTES: usize = 1024;
