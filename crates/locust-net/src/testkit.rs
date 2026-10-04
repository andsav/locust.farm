//! Byte-stream fixtures, with caller-supplied, unauthenticated identities.
use std::io;

use locust_proto::id::EndpointId;
use tokio::io::DuplexStream;

use crate::{FrameLimits, FramedLink};

pub type MemoryLink = FramedLink<DuplexStream, DuplexStream>;

/// Each direction owns its whole duplex stream. Dropping a sender gives EOF;
/// dropping a receiver wakes/fails the corresponding writer without retaining
/// an unused read half in the opposite sender. `capacity` is byte backpressure.
pub fn memory_pair(
    left: EndpointId,
    right: EndpointId,
    left_limits: FrameLimits,
    right_limits: FrameLimits,
    capacity: usize,
) -> io::Result<(MemoryLink, MemoryLink)> {
    if capacity == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "memory link capacity must be nonzero",
        ));
    }
    let (left_write, right_read) = tokio::io::duplex(capacity);
    let (right_write, left_read) = tokio::io::duplex(capacity);
    Ok((
        FramedLink::new(right, left_read, left_write, left_limits),
        FramedLink::new(left, right_read, right_write, right_limits),
    ))
}
