//! Published size limits.
//!
//! Byte limits are applied to a frame, a header or a ticket before it is
//! buffered or decoded. Count limits (parents, entries, hints) are applied to
//! the decoded value before it is acted on or retained; the byte limit of the
//! enclosing frame bounds what decoding can allocate. Violations are rejected
//! and reported, including for input from admitted members; nothing is
//! silently truncated.

/// Largest signed header, in bytes.
pub const MAX_HEADER_BYTES: usize = 16 * 1024;

/// Largest number of causal parents one event may name.
pub const MAX_PARENTS: usize = 64;

/// Largest number of output objects one result may name.
pub const MAX_ARTIFACTS: usize = 64;

/// Largest event payload (task text, note, result summary, document
/// revision), counted as stored bytes.
pub const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;

/// Ceiling for one stored content object. Operators may lower it; the daemon
/// reports the effective value in its hello.
pub const MAX_BLOB_BYTES: usize = 64 * 1024 * 1024;

/// Bytes of a content object carried by one peer frame. An object travels as
/// a run of chunks, so an interrupted transfer resumes at an offset.
pub const BLOB_CHUNK_BYTES: usize = 1024 * 1024;

/// Largest number of events in one sync batch.
pub const MAX_EVENTS_PER_BATCH: usize = 256;

/// Largest number of authors one frontier may list.
pub const MAX_FRONTIER_AUTHORS: usize = 4096;

/// Largest number of log positions in one inventory frame, and of
/// identifiers in one event request.
pub const MAX_INVENTORY_POINTS: usize = 4096;

/// Largest frame accepted before the sender is identified: a local hello, or
/// any peer frame from an endpoint not yet known to speak for a member.
pub const MAX_HELLO_FRAME_BYTES: usize = 4 * 1024;

/// Largest frame on a peer link once the remote endpoint speaks for a member.
/// Holds a full event batch or one content chunk.
pub const MAX_PEER_FRAME_BYTES: usize = MAX_EVENTS_PER_BATCH * (MAX_HEADER_BYTES + 128);

/// Largest frame on the local socket after the hello. The local API carries a
/// whole content object in one frame.
pub const MAX_LOCAL_FRAME_BYTES: usize = MAX_BLOB_BYTES + 64 * 1024;

/// Largest encoded invitation, in bytes; a ticket is twice that in hex plus
/// its prefix.
pub const MAX_INVITATION_BYTES: usize = 4 * 1024;

/// Largest number of entries in one workspace manifest.
pub const MAX_MANIFEST_ENTRIES: usize = 100_000;

/// Longest relative path in a manifest, in bytes.
pub const MAX_PATH_BYTES: usize = 1024;

/// Largest client-owned detail blob in one session record.
pub const MAX_SESSION_DETAIL_BYTES: usize = 64 * 1024;
