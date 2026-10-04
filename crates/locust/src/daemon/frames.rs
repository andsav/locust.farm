//! Frames on one local connection, read and written through two buffers the
//! connection keeps for its whole life: in the steady state no frame
//! allocates.

use std::io;

use locust_proto::codec::{self, FRAME_PREFIX_BYTES};
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

/// Capacity a buffer starts with and falls back to after carrying a frame
/// that needed more, so one large content object does not pin its size for
/// the life of the connection.
const KEPT_BUFFER_BYTES: usize = 64 * 1024;

/// Smallest spare capacity worth offering to a read.
const READ_CHUNK_BYTES: usize = 4 * 1024;

pub(crate) struct Frames {
    stream: UnixStream,
    /// Bytes received and not yet consumed. A frame handed out by
    /// [`Frames::read`] stays at the front until the next read.
    inbound: Vec<u8>,
    /// Length of the frame at the front of `inbound` that was handed out.
    handed_out: usize,
    outbound: Vec<u8>,
}

impl Frames {
    pub(crate) fn new(stream: UnixStream) -> Self {
        Self {
            stream,
            inbound: Vec::with_capacity(READ_CHUNK_BYTES),
            handed_out: 0,
            outbound: Vec::with_capacity(READ_CHUNK_BYTES),
        }
    }

    /// Forgets the frame the last read handed out.
    fn discard(&mut self) {
        if self.handed_out > 0 {
            self.inbound.drain(..self.handed_out);
            self.handed_out = 0;
            if self.inbound.capacity() > KEPT_BUFFER_BYTES
                && self.inbound.len() <= KEPT_BUFFER_BYTES
            {
                self.inbound.shrink_to(KEPT_BUFFER_BYTES);
            }
        }
    }

    /// Receives more bytes into `inbound`; zero means the peer closed.
    async fn receive(&mut self) -> io::Result<usize> {
        if self.inbound.capacity() - self.inbound.len() < FRAME_PREFIX_BYTES {
            self.inbound.reserve(READ_CHUNK_BYTES);
        }
        self.stream.read_buf(&mut self.inbound).await
    }

    /// The payload of the next frame, or `None` when the peer closed between
    /// frames. A frame that announces more than `max` bytes is refused as
    /// invalid data before anything is allocated for it. Dropping this
    /// future loses nothing: what was received stays buffered.
    pub(crate) async fn read(&mut self, max: usize) -> io::Result<Option<&[u8]>> {
        self.discard();
        while self.inbound.len() < FRAME_PREFIX_BYTES {
            if self.receive().await? == 0 {
                return if self.inbound.is_empty() {
                    Ok(None)
                } else {
                    Err(io::ErrorKind::UnexpectedEof.into())
                };
            }
        }
        let mut prefix = [0u8; FRAME_PREFIX_BYTES];
        prefix.copy_from_slice(&self.inbound[..FRAME_PREFIX_BYTES]);
        let end = FRAME_PREFIX_BYTES + codec::frame_len(prefix, max)?;
        if end > self.inbound.len() {
            self.inbound.reserve(end - self.inbound.len());
        }
        while self.inbound.len() < end {
            if self.receive().await? == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
        }
        self.handed_out = end;
        Ok(Some(&self.inbound[FRAME_PREFIX_BYTES..end]))
    }

    /// Waits until the peer closes or sends something while no frame is
    /// expected from it. Returns false when it closed or failed. What it
    /// sent is kept for the next read.
    pub(crate) async fn peer_stays(&mut self) -> bool {
        self.discard();
        matches!(self.receive().await, Ok(received) if received > 0)
    }

    /// Encodes one frame carrying `value` into the outbound buffer and
    /// returns its payload length. Nothing is sent until [`Frames::flush`].
    pub(crate) fn encode<T: Serialize>(&mut self, value: &T) -> io::Result<usize> {
        self.outbound.clear();
        codec::encode_frame(value, &mut self.outbound)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        Ok(self.outbound.len() - FRAME_PREFIX_BYTES)
    }

    /// Sends the encoded frame.
    pub(crate) async fn flush(&mut self) -> io::Result<()> {
        let sent = self.stream.write_all(&self.outbound).await;
        self.outbound.clear();
        if self.outbound.capacity() > KEPT_BUFFER_BYTES {
            self.outbound.shrink_to(KEPT_BUFFER_BYTES);
        }
        sent
    }

    /// Encodes and sends one frame.
    pub(crate) async fn write<T: Serialize>(&mut self, value: &T) -> io::Result<()> {
        self.encode(value)?;
        self.flush().await
    }
}
