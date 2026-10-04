//! Filling `Events` frames up to the count and byte limits.

use locust_proto::event::WireEvent;
#[cfg(test)]
use locust_proto::limits::{MAX_EVENTS_PER_BATCH, MAX_PEER_FRAME_BYTES};
#[cfg(test)]
use locust_proto::sync::SyncMessage;

/// Encoded bytes of an `Events` frame around its events: the variant index
/// and the count, at most two varint bytes for [`MAX_EVENTS_PER_BATCH`].
#[cfg(test)]
const FRAME_OVERHEAD: usize = 1 + 2;

/// Bytes the events of one frame may take.
#[cfg(test)]
const BUDGET: usize = MAX_PEER_FRAME_BYTES - FRAME_OVERHEAD;

/// The encoded size of one event inside a frame: the header's length varint,
/// the header and the 64-byte signature.
pub(crate) fn encoded_len(event: &WireEvent) -> usize {
    let len = event.header.len();
    let varint = (usize::BITS - (len | 1).leading_zeros()).div_ceil(7) as usize;
    varint + len + 64
}

/// Events waiting to go out in one frame.
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct Batch {
    events: Vec<WireEvent>,
    bytes: usize,
}

#[cfg(test)]
impl Batch {
    /// Adds `event`, first sending the frame being filled when the event
    /// would take it past either limit. Callers add each author's events in
    /// ascending order, which frames then keep.
    pub(crate) fn push(&mut self, event: WireEvent, out: &mut Vec<SyncMessage>) {
        let size = encoded_len(&event);
        if self.events.len() == MAX_EVENTS_PER_BATCH || self.bytes + size > BUDGET {
            self.flush(out);
        }
        self.bytes += size;
        self.events.push(event);
    }

    /// Sends the frame being filled, if it holds anything.
    pub(crate) fn flush(&mut self, out: &mut Vec<SyncMessage>) {
        if !self.events.is_empty() {
            out.push(SyncMessage::Events(std::mem::take(&mut self.events)));
            self.bytes = 0;
        }
    }
}
