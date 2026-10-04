//! Authenticated peer connections and ordered [`locust_proto::sync::SyncMessage`] frames.
//!
//! The caller chooses endpoint identity, relays, contact hints and frame
//! admission limits. This crate decides neither membership nor which messages
//! a peer may receive. An authenticated endpoint is not an authorized member.
//!
//! The daemon must keep accepting connections while individual handshakes or
//! streams wait, and must apply its own admission and cancellation policy.
//! No application retries, deadlines or reconciliation policy live here.

#![forbid(unsafe_code)]

mod framing;

pub use framing::{FrameError, FrameLimits, FrameReceiver, FrameSender, FramedLink};

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

use iroh::endpoint::{
    Connection, ConnectionError, Incoming, NetReportConfig, PortmapperConfig, QuicTransportConfig,
    RecvStream, SendStream, VarInt, presets,
};
use iroh::{EndpointAddr, Watcher};
use locust_proto::id::EndpointId;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

/// Protocol identifier negotiated by the authenticated Iroh handshake.
pub const SYNC_ALPN: &[u8] = b"locust/sync/0";

/// A connection whose peer has been silent this long is closed: three missed
/// keep-alives. A dead process or a sleeping machine is noticed in this time.
pub const CONNECTION_IDLE: Duration = Duration::from_secs(15);
/// How often an otherwise idle connection is kept alive.
pub const KEEP_ALIVE: Duration = Duration::from_secs(5);

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

/// Relay infrastructure is always an explicit operator choice.
#[derive(Clone, PartialEq, Eq)]
pub enum RelayConfig {
    Disabled,
    N0,
    Custom(Vec<String>),
}

impl fmt::Debug for RelayConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str("Disabled"),
            Self::N0 => f.write_str("N0"),
            Self::Custom(urls) => f
                .debug_struct("Custom")
                .field("relay_count", &urls.len())
                .finish(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IpTransport {
    Default,
    Bind(SocketAddr),
    Disabled,
}

/// QUIC resource budgets apply before application admission as well as after.
/// They limit buffered bytes/streams, not tasks, attempts or operation time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransportBudget {
    pub max_bidirectional_streams: u32,
    pub stream_receive_bytes: u32,
    pub connection_receive_bytes: u32,
}

impl Default for TransportBudget {
    fn default() -> Self {
        // Two simultaneous exchanges can each buffer one largest contract
        // frame including its prefix. Application reads replenish credit;
        // larger exchanges still flow under backpressure.
        let frame = (locust_proto::limits::MAX_PEER_FRAME_BYTES + 4) as u32;
        Self {
            max_bidirectional_streams: 2,
            stream_receive_bytes: frame,
            connection_receive_bytes: 2 * frame,
        }
    }
}

/// Address publication and lookup. The daemon uses both; isolated probes
/// can disable them and use only explicit hints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lookup {
    pub local_network: bool,
    pub mainline: bool,
}

impl Lookup {
    pub const DISABLED: Self = Self {
        local_network: false,
        mainline: false,
    };
}

impl Default for Lookup {
    fn default() -> Self {
        Self {
            local_network: true,
            mainline: true,
        }
    }
}

/// Caller-owned endpoint identity and network choices.
#[derive(Clone)]
pub struct EndpointConfig {
    pub secret_key: [u8; 32],
    pub relays: RelayConfig,
    pub ip_transport: IpTransport,
    pub port_mapping: bool,
    pub lookup: Lookup,
    pub budget: TransportBudget,
}

impl fmt::Debug for EndpointConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EndpointConfig")
            .field("secret_key", &"<redacted>")
            .field(
                "relays",
                &match &self.relays {
                    RelayConfig::Disabled => "disabled",
                    RelayConfig::N0 => "n0",
                    RelayConfig::Custom(_) => "custom",
                },
            )
            .field("ip_transport", &self.ip_transport)
            .field("port_mapping", &self.port_mapping)
            .field("lookup", &self.lookup)
            .field("budget", &self.budget)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    LocalClosed,
    PeerClosed,
    PeerReset,
    TimedOut,
    TransportError,
}

impl From<&ConnectionError> for CloseReason {
    fn from(error: &ConnectionError) -> Self {
        match error {
            ConnectionError::LocallyClosed => Self::LocalClosed,
            ConnectionError::ApplicationClosed(_) | ConnectionError::ConnectionClosed(_) => {
                Self::PeerClosed
            }
            ConnectionError::Reset => Self::PeerReset,
            ConnectionError::TimedOut => Self::TimedOut,
            _ => Self::TransportError,
        }
    }
}

/// Stable categories deliberately omit peer-controlled close strings/URLs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportError {
    InvalidRelay,
    InvalidBind,
    InvalidBudget,
    InvalidEndpointId,
    Bind,
    Connect,
    Handshake,
    Connection(CloseReason),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "transport: {self:?}")
    }
}
impl std::error::Error for TransportError {}

#[derive(Debug, Clone)]
pub struct Endpoint(iroh::Endpoint);

impl Endpoint {
    pub async fn bind(config: EndpointConfig) -> Result<Self, TransportError> {
        let budget = config.budget;
        if budget.max_bidirectional_streams == 0
            || budget.stream_receive_bytes == 0
            || budget.connection_receive_bytes == 0
        {
            return Err(TransportError::InvalidBudget);
        }
        let relays = match config.relays {
            RelayConfig::Disabled => iroh::RelayMode::Disabled,
            RelayConfig::N0 => iroh::RelayMode::Default,
            RelayConfig::Custom(urls) => {
                if urls.is_empty() {
                    return Err(TransportError::InvalidRelay);
                }
                let urls = urls
                    .iter()
                    .map(|url| relay_url(url).ok_or(TransportError::InvalidRelay))
                    .collect::<Result<Vec<_>, _>>()?;
                iroh::RelayMode::Custom(urls.into_iter().collect())
            }
        };
        let mut report = NetReportConfig::minimal();
        report.https_probes = !matches!(relays, iroh::RelayMode::Disabled);
        let transport = QuicTransportConfig::builder()
            .max_concurrent_bidi_streams(budget.max_bidirectional_streams.into())
            .max_concurrent_uni_streams(0u32.into())
            .stream_receive_window(budget.stream_receive_bytes.into())
            .receive_window(budget.connection_receive_bytes.into())
            .datagram_receive_buffer_size(None)
            .datagram_send_buffer_size(0)
            .max_idle_timeout(Some(
                VarInt::from_u32(CONNECTION_IDLE.as_millis() as u32).into(),
            ))
            .keep_alive_interval(KEEP_ALIVE)
            .build();
        let mut builder = iroh::Endpoint::builder(presets::Minimal)
            .secret_key(iroh::SecretKey::from_bytes(&config.secret_key))
            .alpns(vec![SYNC_ALPN.to_vec()])
            .relay_mode(relays)
            .clear_address_lookup()
            .net_report_config(report)
            .transport_config(transport);
        if !config.port_mapping {
            builder = builder.portmapper_config(PortmapperConfig::Disabled);
        }
        if config.lookup.local_network {
            builder =
                builder.address_lookup(iroh_mdns_address_lookup::MdnsAddressLookup::builder());
        }
        if config.lookup.mainline {
            builder =
                builder.address_lookup(iroh_mainline_address_lookup::DhtAddressLookup::builder());
        }
        builder = match config.ip_transport {
            IpTransport::Default => builder,
            IpTransport::Disabled => builder.clear_ip_transports(),
            IpTransport::Bind(addr) => builder
                .clear_ip_transports()
                .bind_addr(addr)
                .map_err(|_| TransportError::InvalidBind)?,
        };
        builder
            .bind()
            .await
            .map(Self)
            .map_err(|_| TransportError::Bind)
    }

    pub fn id(&self) -> EndpointId {
        EndpointId(*self.0.id().as_bytes())
    }

    /// Public contact hints; unrecognized hints are ignored when dialing.
    pub fn hints(&self) -> Vec<String> {
        contact_hints(&self.0.addr())
    }

    /// Waits for a changed contact snapshot, or returns `None` on local close.
    /// Caller owns deadlines. Rechecking `previous` also avoids a lost update
    /// between taking a snapshot and starting this wait.
    pub async fn hints_changed(&self, previous: &[String]) -> Option<Vec<String>> {
        let mut watcher = self.0.watch_addr();
        loop {
            if self.0.is_closed() {
                return None;
            }
            let hints = contact_hints(&watcher.get());
            if hints != previous {
                return Some(hints);
            }
            tokio::select! {
                _ = self.0.closed() => return None,
                value = watcher.updated() => { value.ok()?; }
            }
        }
    }

    /// Relay readiness only. Without relays this waits until canceled.
    pub async fn online(&self) {
        self.0.online().await;
    }

    pub async fn connect(
        &self,
        peer: EndpointId,
        hints: &[String],
    ) -> Result<PeerConnection, TransportError> {
        let id = iroh::EndpointId::from_bytes(peer.as_bytes())
            .map_err(|_| TransportError::InvalidEndpointId)?;
        let mut addr = EndpointAddr::new(id);
        for hint in hints {
            if let Ok(socket) = hint.parse::<SocketAddr>() {
                if !socket.ip().is_unspecified()
                    && !socket.ip().is_multicast()
                    && socket.port() != 0
                {
                    addr = addr.with_ip_addr(socket);
                }
            } else if let Some(relay) = relay_url(hint) {
                addr = addr.with_relay_url(relay);
            }
        }
        self.0
            .connect(addr, SYNC_ALPN)
            .await
            .map(PeerConnection)
            .map_err(|_| TransportError::Connect)
    }

    /// Obtain attempts before awaiting individual handshakes, so a stalled
    /// handshake cannot block admission of other attempts.
    pub async fn accept(&self) -> Option<IncomingConnection> {
        self.0.accept().await.map(IncomingConnection)
    }
    pub async fn close(&self) {
        self.0.close().await;
    }

    /// Waits until local endpoint shutdown starts.
    pub async fn closed(&self) {
        self.0.closed().await;
    }
}

fn contact_hints(addr: &EndpointAddr) -> Vec<String> {
    let mut hints: Vec<String> = addr
        .ip_addrs()
        .map(ToString::to_string)
        .chain(addr.relay_urls().map(ToString::to_string))
        .collect();
    hints.sort();
    hints
}

fn relay_url(value: &str) -> Option<iroh::RelayUrl> {
    let url: iroh::RelayUrl = value.parse().ok()?;
    let host = url.host_str()?;
    let local = host.eq_ignore_ascii_case("localhost")
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    if !(url.scheme() == "https" || url.scheme() == "http" && local)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || value.chars().any(char::is_whitespace)
    {
        return None;
    }
    Some(url)
}

/// A pending handshake. No remote identity is exposed before authentication.
#[derive(Debug)]
pub struct IncomingConnection(Incoming);

impl IncomingConnection {
    pub async fn accept(self) -> Result<PeerConnection, TransportError> {
        self.0
            .await
            .map(PeerConnection)
            .map_err(|_| TransportError::Handshake)
    }

    pub fn refuse(self) {
        self.0.refuse();
    }
}

/// One authenticated connection; each stream is a separate framed exchange.
#[derive(Debug, Clone)]
pub struct PeerConnection(Connection);

impl PeerConnection {
    /// Whether the transport has already observed this connection closing.
    pub fn is_closed(&self) -> bool {
        self.0.close_reason().is_some()
    }

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
    /// Use `FrameLimits::hello()` unless the daemon has already established
    /// this endpoint speaks for a member. Authentication alone is insufficient.
    pub async fn open_link(&self, limits: FrameLimits) -> Result<IrohLink, TransportError> {
        let (send, recv) = self
            .0
            .open_bi()
            .await
            .map_err(|error| TransportError::Connection(CloseReason::from(&error)))?;
        Ok(FramedLink::new(self.remote_id(), recv, send, limits))
    }

    /// An inbound exchange starts at `FrameLimits::hello()`. Only the daemon
    /// may raise its limit after establishing membership from Hello/Join.
    /// Explicit limits allow stricter operator policies.
    pub async fn accept_link(&self, limits: FrameLimits) -> Result<IrohLink, TransportError> {
        let (send, recv) = self
            .0
            .accept_bi()
            .await
            .map_err(|error| TransportError::Connection(CloseReason::from(&error)))?;
        Ok(FramedLink::new(self.remote_id(), recv, send, limits))
    }

    /// Closes every stream on this connection. Call [`Endpoint::close`] when
    /// shutting down the endpoint to flush the close notification.
    pub fn close(&self) {
        self.0.close(0u32.into(), b"");
    }

    /// Waits for local/remote closure without exposing peer-controlled text.
    pub async fn closed(&self) -> CloseReason {
        CloseReason::from(&self.0.closed().await)
    }
}

/// Types usable by daemon tasks without depending on the transport provider.
pub type PeerLink = FramedLink<RecvStream, SendStream>;
pub type PeerSender = FrameSender<SendStream>;
pub type PeerReceiver = FrameReceiver<RecvStream>;
/// Compatibility name for existing transport qualification code.
pub type IrohLink = PeerLink;

#[cfg(test)]
mod tests;
