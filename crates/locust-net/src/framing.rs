use std::fmt;
use std::io;

use iroh::endpoint::{Connection, RecvStream, SendStream, WriteError};
use locust_proto::codec::{self, CodecError};
use locust_proto::id::EndpointId;
use locust_proto::sync::SyncMessage;
use tokio::io::{
    AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream, ReadHalf, WriteHalf,
};

/// Explicit, local per-frame admission limits, excluding the four-byte prefix.
///
/// There is no implicit default or peer negotiation. The caller selects these
/// from its configured policy and reports refusals to the peer when possible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameLimits {
    pub max_send_bytes: u32,
    pub max_receive_bytes: u32,
}

#[derive(Debug)]
pub enum FrameError {
    Io(io::Error),
    Codec(CodecError),
    TooLarge {
        length: usize,
        maximum: u32,
    },
    /// A previous failure or canceled send made this half unsafe to reuse,
    /// or the sending half has already been finished.
    Unusable,
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "frame I/O: {error}"),
            Self::Codec(error) => write!(f, "frame encoding: {error}"),
            Self::TooLarge { length, maximum } => {
                write!(f, "frame length {length} exceeds admitted size {maximum}")
            }
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

/// An ordered exchange with the same framing for Iroh and in-memory streams.
#[derive(Debug)]
pub struct FramedLink<R, W> {
    receiver: FrameReceiver<R>,
    sender: FrameSender<W>,
}

impl<R, W> FramedLink<R, W> {
    pub(crate) fn new(
        peer: EndpointId,
        reader: R,
        writer: W,
        limits: FrameLimits,
        connection: Option<Connection>,
    ) -> Self {
        Self {
            receiver: FrameReceiver {
                peer,
                reader,
                maximum: limits.max_receive_bytes,
                state: ReceiveState::Header {
                    bytes: [0; 4],
                    filled: 0,
                },
                _connection: connection.clone(),
            },
            sender: FrameSender {
                peer,
                writer,
                maximum: limits.max_send_bytes,
                usable: true,
                _connection: connection,
            },
        }
    }

    pub fn remote_id(&self) -> EndpointId {
        self.receiver.remote_id()
    }

    /// Separates send and receive so either direction may make progress while
    /// the other waits. Each half retains the peer identity and connection.
    pub fn into_split(self) -> (FrameSender<W>, FrameReceiver<R>) {
        (self.sender, self.receiver)
    }
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> FramedLink<R, W> {
    pub async fn send(&mut self, message: &SyncMessage) -> Result<(), FrameError> {
        self.sender.send(message).await
    }

    pub async fn recv(&mut self) -> Result<Option<SyncMessage>, FrameError> {
        self.receiver.recv().await
    }

    /// Ends only the sending direction. The remote may still send a response.
    pub async fn finish(&mut self) -> Result<(), FrameError> {
        self.sender.finish().await
    }
}

impl FramedLink<RecvStream, SendStream> {
    /// Finishes sending and waits for the peer's QUIC acknowledgement.
    ///
    /// See [`FrameSender::finish_acknowledged`] for acknowledgement and
    /// cancellation semantics. The receiving direction remains available.
    pub async fn finish_acknowledged(&mut self) -> Result<(), FrameError> {
        self.sender.finish_acknowledged().await
    }
}

#[derive(Debug)]
pub struct FrameSender<W> {
    peer: EndpointId,
    writer: W,
    maximum: u32,
    usable: bool,
    // Streams must keep the authenticated connection alive after the caller
    // drops its PeerConnection. Memory links need no transport guard.
    _connection: Option<Connection>,
}

impl<W> FrameSender<W> {
    pub fn remote_id(&self) -> EndpointId {
        self.peer
    }
}

impl<W: AsyncWrite + Unpin> FrameSender<W> {
    /// Sends one complete encoded frame. Success means the local transport
    /// accepted the bytes, not that the remote processed or persisted them.
    ///
    /// Canceling this future after it starts writing poisons this sending half:
    /// no later frame can be appended to a potentially partial frame. A size or
    /// encoding rejection before writing leaves the half usable.
    pub async fn send(&mut self, message: &SyncMessage) -> Result<(), FrameError> {
        if !self.usable {
            return Err(FrameError::Unusable);
        }
        let payload = codec::encode(message).map_err(FrameError::Codec)?;
        if payload.len() > self.maximum as usize {
            return Err(FrameError::TooLarge {
                length: payload.len(),
                maximum: self.maximum,
            });
        }
        // Use the contract's framing implementation rather than a second wire
        // encoder. Only the async I/O lives in this crate.
        let mut wire = Vec::with_capacity(4 + payload.len());
        codec::write_frame(&mut wire, &payload).map_err(FrameError::Io)?;
        self.usable = false;
        self.writer.write_all(&wire).await.map_err(FrameError::Io)?;
        self.usable = true;
        Ok(())
    }

    pub async fn finish(&mut self) -> Result<(), FrameError> {
        if !self.usable {
            return Err(FrameError::Unusable);
        }
        self.usable = false;
        self.writer.shutdown().await.map_err(FrameError::Io)
    }
}

impl FrameSender<SendStream> {
    /// Finishes sending and waits until the peer acknowledges all stream data
    /// and its end at the QUIC transport layer. A peer stop is an I/O error.
    ///
    /// Success does not establish application consumption or persistence.
    /// The caller owns the deadline and cancellation. Once finishing starts,
    /// this sender cannot be reused, including if this future is canceled.
    pub async fn finish_acknowledged(&mut self) -> Result<(), FrameError> {
        self.finish().await?;
        match self.writer.stopped().await {
            Ok(None) => Ok(()),
            Ok(Some(code)) => Err(FrameError::Io(WriteError::Stopped(code).into())),
            Err(error) => Err(FrameError::Io(error.into())),
        }
    }
}

#[derive(Debug)]
enum ReceiveState {
    Header { bytes: [u8; 4], filled: usize },
    Payload { bytes: Vec<u8>, filled: usize },
    Ended,
    Failed,
}

#[derive(Debug)]
pub struct FrameReceiver<R> {
    peer: EndpointId,
    reader: R,
    maximum: u32,
    state: ReceiveState,
    _connection: Option<Connection>,
}

impl<R> FrameReceiver<R> {
    pub fn remote_id(&self) -> EndpointId {
        self.peer
    }
}

impl<R: AsyncRead + Unpin> FrameReceiver<R> {
    /// Reads one frame, returning `None` only on EOF between frames.
    ///
    /// Cancel-safe: partial prefix and payload progress stays in this receiver.
    /// A truncated, oversized or malformed frame poisons the receiving half.
    /// The sending half remains available for an explicit protocol refusal.
    pub async fn recv(&mut self) -> Result<Option<SyncMessage>, FrameError> {
        let result = self.read_message().await;
        if result.is_err() {
            self.state = ReceiveState::Failed;
        }
        result
    }

    async fn read_message(&mut self) -> Result<Option<SyncMessage>, FrameError> {
        loop {
            match &mut self.state {
                ReceiveState::Header { bytes, filled } => {
                    let count = self
                        .reader
                        .read(&mut bytes[*filled..])
                        .await
                        .map_err(FrameError::Io)?;
                    if count == 0 {
                        if *filled == 0 {
                            self.state = ReceiveState::Ended;
                            return Ok(None);
                        }
                        return Err(FrameError::Io(io::ErrorKind::UnexpectedEof.into()));
                    }
                    *filled += count;
                    if *filled == bytes.len() {
                        let length = u32::from_le_bytes(*bytes);
                        if length > self.maximum {
                            return Err(FrameError::TooLarge {
                                length: length as usize,
                                maximum: self.maximum,
                            });
                        }
                        self.state = ReceiveState::Payload {
                            bytes: vec![0; length as usize],
                            filled: 0,
                        };
                    }
                }
                ReceiveState::Payload { bytes, filled } => {
                    if *filled == bytes.len() {
                        let message = codec::decode(bytes).map_err(FrameError::Codec)?;
                        self.state = ReceiveState::Header {
                            bytes: [0; 4],
                            filled: 0,
                        };
                        return Ok(Some(message));
                    }
                    let count = self
                        .reader
                        .read(&mut bytes[*filled..])
                        .await
                        .map_err(FrameError::Io)?;
                    if count == 0 {
                        return Err(FrameError::Io(io::ErrorKind::UnexpectedEof.into()));
                    }
                    *filled += count;
                }
                ReceiveState::Ended => return Ok(None),
                ReceiveState::Failed => return Err(FrameError::Unusable),
            }
        }
    }
}

pub type MemoryLink = FramedLink<ReadHalf<DuplexStream>, WriteHalf<DuplexStream>>;

/// Creates a byte-stream twin using the production encoder and framing.
///
/// Identities here are supplied by the test harness, **not authenticated**.
/// Capacity controls byte backpressure and is also caller-selected; it must be
/// nonzero. Each endpoint has independent send and receive admission limits.
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
    let (left_stream, right_stream) = tokio::io::duplex(capacity);
    let (left_read, left_write) = tokio::io::split(left_stream);
    let (right_read, right_write) = tokio::io::split(right_stream);
    Ok((
        FramedLink::new(right, left_read, left_write, left_limits, None),
        FramedLink::new(left, right_read, right_write, right_limits, None),
    ))
}
