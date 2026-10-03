//! Authenticated peer connections and ordered [`locust_proto::sync::SyncMessage`] frames.
//!
//! The caller chooses endpoint identity, relays, address lookup and frame
//! admission limits. This crate decides neither membership nor which messages
//! a peer may receive. An authenticated endpoint is not an authorized member.
//!
//! The daemon must keep accepting connections while individual handshakes or
//! streams wait, and must apply its own admission and cancellation policy.
//! No application retries, deadlines or reconciliation policy live here.

#![forbid(unsafe_code)]

mod framing;

pub use framing::{
    FrameError, FrameLimits, FrameReceiver, FrameSender, FramedLink, MemoryLink, memory_pair,
};

use iroh::EndpointAddr;
use iroh::endpoint::{
    BindError, Builder, ConnectError, ConnectingError, Connection, ConnectionError, Incoming,
    RecvStream, SendStream,
};
use locust_proto::id::EndpointId;
use std::time::Duration;

/// Protocol identifier negotiated by the authenticated Iroh handshake.
pub const SYNC_ALPN: &[u8] = b"locust/sync/0";

/// Transport category of an observed open network path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    /// A direct IP path.
    Direct,
    /// A path through an Iroh relay.
    Relay,
    /// Another transport, such as a caller-configured custom transport.
    Other,
}

/// An owned path observation that contains no IP addresses or relay URLs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathSnapshot {
    pub kind: PathKind,
    /// Whether Iroh selected this path for application data at snapshot time.
    pub selected: bool,
    /// QUIC's round-trip time estimate, sampled while creating the snapshot.
    pub rtt: Duration,
}

/// An endpoint with Locust's ALPN and otherwise caller-selected configuration.
#[derive(Debug, Clone)]
pub struct Endpoint(iroh::Endpoint);

impl Endpoint {
    /// Binds a caller-configured Iroh endpoint, accepting only [`SYNC_ALPN`].
    ///
    /// Use `iroh::endpoint::presets::Minimal` for no public address lookup or
    /// relay. Iroh's `N0` preset enables Number 0's relay and lookup services;
    /// selecting that preset is an explicit operator choice. Supply a persisted
    /// secret key on the builder to retain identity across process restarts.
    pub async fn bind(builder: Builder) -> Result<Self, BindError> {
        builder
            .alpns(vec![SYNC_ALPN.to_vec()])
            .bind()
            .await
            .map(Self)
    }

    pub fn id(&self) -> EndpointId {
        EndpointId(*self.0.id().as_bytes())
    }

    /// Current contact hints, including any configured relay address.
    pub fn addr(&self) -> EndpointAddr {
        self.0.addr()
    }

    /// Waits until at least one configured relay completes its connection
    /// handshake and registers this endpoint.
    ///
    /// This is relay readiness, not direct-path or address-lookup readiness.
    /// With relays disabled or unreachable, this waits indefinitely, even if
    /// direct connections work. The caller owns cancellation and any timeout.
    /// Returning does not guarantee that the relay remains connected afterward.
    pub async fn online(&self) {
        self.0.online().await;
    }

    pub async fn connect(
        &self,
        addr: impl Into<EndpointAddr>,
    ) -> Result<PeerConnection, ConnectError> {
        self.0.connect(addr, SYNC_ALPN).await.map(PeerConnection)
    }

    /// Receives the next connection attempt, without waiting for its handshake.
    ///
    /// Call [`IncomingConnection::accept`] separately so an unresponsive peer
    /// cannot prevent the application from accepting other attempts. `None`
    /// means this endpoint is closed.
    pub async fn accept(&self) -> Option<IncomingConnection> {
        self.0.accept().await.map(IncomingConnection)
    }

    /// Flushes endpoint shutdown and stops Iroh's background tasks.
    pub async fn close(&self) {
        self.0.close().await;
    }
}

/// A pending handshake. No remote identity is exposed before authentication.
#[derive(Debug)]
pub struct IncomingConnection(Incoming);

impl IncomingConnection {
    pub async fn accept(self) -> Result<PeerConnection, ConnectingError> {
        self.0.await.map(PeerConnection)
    }

    pub fn refuse(self) {
        self.0.refuse();
    }
}

/// One authenticated connection; each stream is a separate framed exchange.
#[derive(Debug, Clone)]
pub struct PeerConnection(Connection);

impl PeerConnection {
    /// Identity established by Iroh's handshake, never by peer message content.
    pub fn remote_id(&self) -> EndpointId {
        EndpointId(*self.0.remote_id().as_bytes())
    }

    /// Observes the connection's currently open paths without exposing their
    /// IP addresses or relay URLs.
    ///
    /// Path membership and selection are captured by Iroh at call time; RTT
    /// estimates are sampled while copying the paths. Paths can change as soon
    /// as this returns. Selection describes Iroh's application-data choice,
    /// not proof of the route taken by an individual frame.
    pub fn path_snapshot(&self) -> Vec<PathSnapshot> {
        self.0
            .paths()
            .iter()
            .map(|path| PathSnapshot {
                kind: if path.is_ip() {
                    PathKind::Direct
                } else if path.is_relay() {
                    PathKind::Relay
                } else {
                    PathKind::Other
                },
                selected: path.is_selected(),
                rtt: path.rtt(),
            })
            .collect()
    }

    /// Opens a framed exchange. The remote can accept it only after the first
    /// frame is sent; QUIC does not announce an empty newly opened stream.
    pub async fn open_link(&self, limits: FrameLimits) -> Result<IrohLink, ConnectionError> {
        let (send, recv) = self.0.open_bi().await?;
        Ok(FramedLink::new(
            self.remote_id(),
            recv,
            send,
            limits,
            Some(self.0.clone()),
        ))
    }

    pub async fn accept_link(&self, limits: FrameLimits) -> Result<IrohLink, ConnectionError> {
        let (send, recv) = self.0.accept_bi().await?;
        Ok(FramedLink::new(
            self.remote_id(),
            recv,
            send,
            limits,
            Some(self.0.clone()),
        ))
    }

    /// Closes every stream on this connection. Call [`Endpoint::close`] when
    /// shutting down the endpoint to flush the close notification.
    pub fn close(&self, reason: &[u8]) {
        self.0.close(0u32.into(), reason);
    }
}

pub type IrohLink = FramedLink<RecvStream, SendStream>;

#[cfg(test)]
mod tests;
