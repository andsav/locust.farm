use std::fmt;
use std::io;

use iroh::endpoint::{ReadError, RecvStream, SendStream, WriteError};
use locust_proto::codec::{self, CodecError};
use locust_proto::id::EndpointId;
use locust_proto::limits::{MAX_HELLO_FRAME_BYTES, MAX_PEER_FRAME_BYTES};
use locust_proto::sync::{Refusal, SyncMessage};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::CloseReason;

// Retain ordinary control frames, but release chunk/batch-sized allocations
// after use. This affects allocation reuse only, never admitted frame sizes.
const RETAIN_BUFFER_BYTES: usize = 64 * 1024;
const READ_GROWTH_BYTES: usize = 8 * 1024;

/// Explicit per-direction limits, excluding the four-byte prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameLimits {
    pub max_send_bytes: usize,
    pub max_receive_bytes: usize,
}

impl FrameLimits {
    /// Use until the daemon establishes membership for this exchange.
    pub const fn hello() -> Self {
        Self {
            max_send_bytes: MAX_HELLO_FRAME_BYTES,
            max_receive_bytes: MAX_HELLO_FRAME_BYTES,
        }
    }

    /// Only for an endpoint the daemon has already bound to a member.
    pub const fn peer() -> Self {
        Self {
            max_send_bytes: MAX_PEER_FRAME_BYTES,
            max_receive_bytes: MAX_PEER_FRAME_BYTES,
        }
    }
}

#[derive(Debug)]
pub enum FrameError {
    Io(io::Error),
    Codec(CodecError),
    Protocol(Refusal),
    TooLarge {
        length: usize,
        maximum: usize,
    },
    Truncated,
    PeerReset {
        code: u64,
    },
    ConnectionLost(CloseReason),
    LocalClosed,
    /// Failed/canceled writing, or a previously finished/failed half.
    Unusable,
}

impl FrameError {
    fn from_io(error: io::Error) -> Self {
        if let Some(error) = error
            .get_ref()
            .and_then(|error| error.downcast_ref::<ReadError>())
        {
            match error {
                ReadError::Reset(code) => {
                    return Self::PeerReset {
                        code: code.into_inner(),
                    };
                }
                ReadError::ConnectionLost(reason) => return Self::connection(reason),
                ReadError::ClosedStream => return Self::LocalClosed,
                _ => {}
            }
        }
        if let Some(error) = error
            .get_ref()
            .and_then(|error| error.downcast_ref::<WriteError>())
        {
            match error {
                WriteError::Stopped(code) => {
                    return Self::PeerReset {
                        code: code.into_inner(),
                    };
                }
                WriteError::ConnectionLost(reason) => return Self::connection(reason),
                WriteError::ClosedStream => return Self::LocalClosed,
                _ => {}
            }
        }
        match error.kind() {
            io::ErrorKind::UnexpectedEof => Self::Truncated,
            io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset => {
                Self::PeerReset { code: 0 }
            }
            _ => Self::Io(error),
        }
    }

    fn connection(error: &iroh::endpoint::ConnectionError) -> Self {
        let reason = CloseReason::from(error);
        if reason == CloseReason::LocalClosed {
            Self::LocalClosed
        } else {
            Self::ConnectionLost(reason)
        }
    }
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "frame I/O: {:?}", error.kind()),
            Self::Codec(error) => write!(f, "frame encoding: {error}"),
            Self::Protocol(refusal) => write!(f, "invalid sync message: {refusal}"),
            Self::TooLarge { length, maximum } => {
                write!(f, "frame length {length} exceeds admitted size {maximum}")
            }
            Self::Truncated => f.write_str("frame stream ended inside a frame"),
            Self::PeerReset { code } => write!(f, "peer aborted frame stream ({code})"),
            Self::ConnectionLost(reason) => write!(f, "frame connection lost: {reason:?}"),
            Self::LocalClosed => f.write_str("frame connection closed locally"),
            Self::Unusable => f.write_str("frame stream half is no longer usable"),
        }
    }
}

impl std::error::Error for FrameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Codec(error) => Some(error),
            _ => None,
        }
    }
}

/// Ordered framing. The daemon interprets EOF without `Done` as an abort.
#[derive(Debug)]
pub struct FramedLink<R, W> {
    receiver: FrameReceiver<R>,
    sender: FrameSender<W>,
}

impl<R, W> FramedLink<R, W> {
    pub(crate) fn new(peer: EndpointId, reader: R, writer: W, limits: FrameLimits) -> Self {
        Self {
            receiver: FrameReceiver {
                peer,
                reader,
                maximum: limits.max_receive_bytes,
                state: ReceiveState::header(limits.max_receive_bytes),
                buffer: Vec::new(),
            },
            sender: FrameSender {
                peer,
                writer,
                maximum: limits.max_send_bytes,
                usable: true,
                buffer: Vec::new(),
            },
        }
    }

    pub fn remote_id(&self) -> EndpointId {
        self.receiver.remote_id()
    }
    pub fn into_split(self) -> (FrameSender<W>, FrameReceiver<R>) {
        (self.sender, self.receiver)
    }
    pub fn set_send_limit(&mut self, maximum: usize) {
        self.sender.set_send_limit(maximum);
    }
    pub fn set_receive_limit(&mut self, maximum: usize) {
        self.receiver.set_receive_limit(maximum);
    }
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> FramedLink<R, W> {
    pub async fn send(&mut self, message: &SyncMessage) -> Result<(), FrameError> {
        self.sender.send(message).await
    }
    pub async fn recv(&mut self) -> Result<Option<SyncMessage>, FrameError> {
        self.receiver.recv().await
    }
    /// Ends the sending direction without waiting for delivery. For a final
    /// network frame use `finish_acknowledged` before dropping the connection.
    pub async fn finish(&mut self) -> Result<(), FrameError> {
        self.sender.finish().await
    }
}

impl FramedLink<RecvStream, SendStream> {
    pub async fn finish_acknowledged(&mut self) -> Result<(), FrameError> {
        self.sender.finish_acknowledged().await
    }
    pub fn abort_send(&mut self) -> Result<(), FrameError> {
        self.sender.abort()
    }
    pub fn abort_receive(&mut self) -> Result<(), FrameError> {
        self.receiver.abort()
    }
}

pub struct FrameSender<W> {
    peer: EndpointId,
    writer: W,
    maximum: usize,
    usable: bool,
    buffer: Vec<u8>,
}

impl<W> fmt::Debug for FrameSender<W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrameSender")
            .field("peer", &self.peer)
            .field("maximum", &self.maximum)
            .field("usable", &self.usable)
            .field("buffered_bytes", &self.buffer.len())
            .finish_non_exhaustive()
    }
}

impl<W> FrameSender<W> {
    pub fn remote_id(&self) -> EndpointId {
        self.peer
    }
    /// Applies to the next send. A canceled send remains unusable.
    pub fn set_send_limit(&mut self, maximum: usize) {
        self.maximum = maximum;
    }
}

impl<W: AsyncWrite + Unpin> FrameSender<W> {
    /// Success is local byte acceptance, not peer consumption or persistence.
    /// Canceling after writing starts poisons this half; reset it explicitly.
    /// Encoding/size rejection before writing leaves it usable.
    pub async fn send(&mut self, message: &SyncMessage) -> Result<(), FrameError> {
        if !self.usable {
            return Err(FrameError::Unusable);
        }
        self.buffer.clear();
        codec::encode_frame(message, &mut self.buffer).map_err(FrameError::Codec)?;
        let length = self.buffer.len() - codec::FRAME_PREFIX_BYTES;
        if length > self.maximum {
            release_large(&mut self.buffer);
            return Err(FrameError::TooLarge {
                length,
                maximum: self.maximum,
            });
        }
        self.usable = false;
        let result = self
            .writer
            .write_all(&self.buffer)
            .await
            .map_err(FrameError::from_io);
        release_large(&mut self.buffer);
        result?;
        self.usable = true;
        Ok(())
    }

    pub async fn finish(&mut self) -> Result<(), FrameError> {
        if !self.usable {
            return Err(FrameError::Unusable);
        }
        self.usable = false;
        self.writer.shutdown().await.map_err(FrameError::from_io)
    }
}

impl FrameSender<SendStream> {
    /// Waits for QUIC acknowledgement of all bytes and FIN. This establishes
    /// transport delivery only. Caller owns cancellation/deadlines; starting
    /// this operation makes the sender unusable even if canceled.
    pub async fn finish_acknowledged(&mut self) -> Result<(), FrameError> {
        self.finish().await?;
        match self.writer.stopped().await {
            Ok(None) => Ok(()),
            Ok(Some(code)) => Err(FrameError::PeerReset {
                code: code.into_inner(),
            }),
            Err(error) => Err(FrameError::from_io(error.into())),
        }
    }

    /// Aborts even a canceled/failed send. Version 0 reset codes are zero;
    /// protocol refusal reasons travel in frames.
    pub fn abort(&mut self) -> Result<(), FrameError> {
        self.usable = false;
        self.buffer = Vec::new();
        self.writer
            .reset(0u32.into())
            .map_err(|_| FrameError::LocalClosed)
    }
}

#[derive(Debug)]
enum ReceiveState {
    Header {
        bytes: [u8; 4],
        filled: usize,
        maximum: usize,
    },
    Payload {
        length: usize,
    },
    Ended,
    Failed,
}

impl ReceiveState {
    fn header(maximum: usize) -> Self {
        Self::Header {
            bytes: [0; 4],
            filled: 0,
            maximum,
        }
    }
}

pub struct FrameReceiver<R> {
    peer: EndpointId,
    reader: R,
    maximum: usize,
    state: ReceiveState,
    buffer: Vec<u8>,
}

impl<R> fmt::Debug for FrameReceiver<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrameReceiver")
            .field("peer", &self.peer)
            .field("maximum", &self.maximum)
            .field("state", &self.state)
            .field("buffered_bytes", &self.buffer.len())
            .finish_non_exhaustive()
    }
}

impl<R> FrameReceiver<R> {
    pub fn remote_id(&self) -> EndpointId {
        self.peer
    }
    /// Applies to the next prefix. A partially read prefix or payload keeps
    /// its original limit, including across cancellation of `recv`.
    pub fn set_receive_limit(&mut self, maximum: usize) {
        self.maximum = maximum;
        if let ReceiveState::Header {
            filled: 0,
            maximum: active,
            ..
        } = &mut self.state
        {
            *active = maximum;
        }
    }
}

impl<R: AsyncRead + Unpin> FrameReceiver<R> {
    /// Cancel-safe: received bytes and prefix admission stay in this receiver.
    /// A malformed/truncated/oversized frame poisons this half; the other half
    /// remains available to send an explicit refusal.
    pub async fn recv(&mut self) -> Result<Option<SyncMessage>, FrameError> {
        let result = self.read_message().await;
        if result.is_err() {
            self.state = ReceiveState::Failed;
            self.buffer = Vec::new();
        }
        result
    }

    async fn read_message(&mut self) -> Result<Option<SyncMessage>, FrameError> {
        loop {
            match &mut self.state {
                ReceiveState::Header {
                    bytes,
                    filled,
                    maximum,
                } => {
                    let count = self
                        .reader
                        .read(&mut bytes[*filled..])
                        .await
                        .map_err(FrameError::from_io)?;
                    if count == 0 {
                        if *filled == 0 {
                            self.state = ReceiveState::Ended;
                            return Ok(None);
                        }
                        return Err(FrameError::Truncated);
                    }
                    *filled += count;
                    if *filled == bytes.len() {
                        let length = codec::frame_len(*bytes, *maximum).map_err(|error| {
                            FrameError::TooLarge {
                                length: error.length,
                                maximum: error.max,
                            }
                        })?;
                        self.buffer.clear();
                        self.state = ReceiveState::Payload { length };
                    }
                }
                ReceiveState::Payload { length } => {
                    if self.buffer.len() == *length {
                        let message =
                            SyncMessage::decode(&self.buffer).map_err(FrameError::Protocol)?;
                        release_large(&mut self.buffer);
                        self.state = ReceiveState::header(self.maximum);
                        return Ok(Some(message));
                    }
                    let remaining = *length - self.buffer.len();
                    if self.buffer.len() == self.buffer.capacity() {
                        self.buffer.reserve(remaining.min(READ_GROWTH_BYTES));
                    }
                    // Grow only as bytes arrive, and never consume a later frame.
                    let count = (&mut self.reader)
                        .take(remaining as u64)
                        .read_buf(&mut self.buffer)
                        .await
                        .map_err(FrameError::from_io)?;
                    if count == 0 {
                        return Err(FrameError::Truncated);
                    }
                }
                ReceiveState::Ended => return Ok(None),
                ReceiveState::Failed => return Err(FrameError::Unusable),
            }
        }
    }
}

impl FrameReceiver<RecvStream> {
    pub fn abort(&mut self) -> Result<(), FrameError> {
        self.state = ReceiveState::Failed;
        self.buffer = Vec::new();
        self.reader
            .stop(0u32.into())
            .map_err(|_| FrameError::LocalClosed)
    }
}

fn release_large(buffer: &mut Vec<u8>) {
    if buffer.capacity() > RETAIN_BUFFER_BYTES {
        *buffer = Vec::new();
    } else {
        buffer.clear();
    }
}
