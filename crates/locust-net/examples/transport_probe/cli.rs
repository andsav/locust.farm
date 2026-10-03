use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use iroh::RelayUrl;
use locust_proto::id::EndpointId;

use super::{Failure, Result};

pub const USAGE: &str = "Usage: transport_probe listen|connect --mode direct|relay|auto --timeout-ms N [--bind IP:PORT] [--relay-url URL ... | --n0-relays] [--peer ENDPOINT_ID --peer-addr IP:PORT ... --peer-relay URL] [--expect-peer ENDPOINT_ID] [--wait-for-route direct|relay]\nRelay and auto require an explicit relay choice. --wait-for-route is auto-only and polls selected paths every 50ms within the total deadline. No discovery, identity files, or public infrastructure by default. Contact records are private network hints.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Direct,
    Relay,
    Auto,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Relay => "relay",
            Self::Auto => "auto",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Listen,
    Connect,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Listen => "listen",
            Self::Connect => "connect",
        }
    }
}

#[derive(Debug)]
pub struct Config {
    pub role: Role,
    pub mode: Mode,
    pub timeout: Duration,
    pub bind: Option<SocketAddr>,
    pub relays: Vec<RelayUrl>,
    pub n0_relays: bool,
    pub peer: Option<EndpointId>,
    pub peer_addrs: Vec<SocketAddr>,
    pub peer_relay: Option<RelayUrl>,
    pub expect_peer: Option<EndpointId>,
    pub wait_route: Option<Mode>,
}

impl Config {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut args = args.into_iter();
        let role = match args.next().as_deref() {
            Some("listen") => Role::Listen,
            Some("connect") => Role::Connect,
            _ => return Err(Failure("invalid_role")),
        };
        let mut mode = None;
        let mut timeout = None;
        let mut bind = None;
        let mut relays = Vec::new();
        let mut n0_relays = false;
        let mut peer = None;
        let mut peer_addrs = Vec::new();
        let mut peer_relay = None;
        let mut expect_peer = None;
        let mut wait_route = None;
        while let Some(flag) = args.next() {
            if flag == "--n0-relays" {
                if n0_relays {
                    return Err(Failure("duplicate_option"));
                }
                n0_relays = true;
                continue;
            }
            if !matches!(
                flag.as_str(),
                "--mode"
                    | "--timeout-ms"
                    | "--bind"
                    | "--relay-url"
                    | "--peer"
                    | "--peer-addr"
                    | "--peer-relay"
                    | "--expect-peer"
                    | "--wait-for-route"
            ) {
                return Err(Failure("unknown_option"));
            }
            let value = args.next().ok_or(Failure("missing_value"))?;
            match flag.as_str() {
                "--mode" => set_once(
                    &mut mode,
                    match value.as_str() {
                        "direct" => Mode::Direct,
                        "relay" => Mode::Relay,
                        "auto" => Mode::Auto,
                        _ => return Err(Failure("invalid_mode")),
                    },
                )?,
                "--timeout-ms" => {
                    let milliseconds: u64 =
                        value.parse().map_err(|_| Failure("invalid_timeout"))?;
                    if milliseconds == 0 {
                        return Err(Failure("zero_timeout"));
                    }
                    set_once(&mut timeout, Duration::from_millis(milliseconds))?;
                }
                "--bind" => set_once(&mut bind, socket(&value, true)?)?,
                "--relay-url" => relays.push(relay_url(&value)?),
                "--peer" => set_once(&mut peer, endpoint_id(&value)?)?,
                "--peer-addr" => peer_addrs.push(socket(&value, false)?),
                "--peer-relay" => set_once(&mut peer_relay, relay_url(&value)?)?,
                "--expect-peer" => set_once(&mut expect_peer, endpoint_id(&value)?)?,
                "--wait-for-route" => set_once(
                    &mut wait_route,
                    match value.as_str() {
                        "direct" => Mode::Direct,
                        "relay" => Mode::Relay,
                        _ => return Err(Failure("invalid_wait_route")),
                    },
                )?,
                _ => return Err(Failure("unknown_option")),
            }
        }
        let mode = mode.ok_or(Failure("mode_required"))?;
        let timeout = timeout.ok_or(Failure("timeout_required"))?;
        if mode != Mode::Auto && wait_route.is_some() {
            return Err(Failure("wait_route_requires_auto"));
        }
        if n0_relays && !relays.is_empty() {
            return Err(Failure("conflicting_relays"));
        }
        if mode == Mode::Direct && (n0_relays || !relays.is_empty() || peer_relay.is_some()) {
            return Err(Failure("direct_relay_conflict"));
        }
        if mode != Mode::Direct && !n0_relays && relays.is_empty() {
            return Err(Failure("relay_choice_required"));
        }
        if mode == Mode::Relay && (bind.is_some() || !peer_addrs.is_empty()) {
            return Err(Failure("relay_ip_conflict"));
        }
        match role {
            Role::Listen if peer.is_some() || !peer_addrs.is_empty() || peer_relay.is_some() => {
                return Err(Failure("listener_peer_conflict"));
            }
            Role::Connect => {
                if expect_peer.is_some() {
                    return Err(Failure("connector_expect_conflict"));
                }
                if peer.is_none() {
                    return Err(Failure("peer_required"));
                }
                if peer_addrs.is_empty() && peer_relay.is_none() {
                    return Err(Failure("peer_hint_required"));
                }
                if mode == Mode::Relay && peer_relay.is_none() {
                    return Err(Failure("peer_relay_required"));
                }
            }
            Role::Listen => {}
        }
        Ok(Self {
            role,
            mode,
            timeout,
            bind,
            relays,
            n0_relays,
            peer,
            peer_addrs,
            peer_relay,
            expect_peer,
            wait_route,
        })
    }
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> Result<()> {
    if slot.is_some() {
        return Err(Failure("duplicate_option"));
    }
    *slot = Some(value);
    Ok(())
}

fn endpoint_id(value: &str) -> Result<EndpointId> {
    let id: EndpointId = value.parse().map_err(|_| Failure("invalid_endpoint_id"))?;
    iroh::EndpointId::from_bytes(id.as_bytes()).map_err(|_| Failure("invalid_endpoint_id"))?;
    Ok(id)
}

fn socket(value: &str, bind: bool) -> Result<SocketAddr> {
    let addr: SocketAddr = value.parse().map_err(|_| Failure("invalid_address"))?;
    if !bind && (addr.ip().is_unspecified() || addr.ip().is_multicast() || addr.port() == 0) {
        return Err(Failure("invalid_peer_address"));
    }
    Ok(addr)
}

fn relay_url(value: &str) -> Result<RelayUrl> {
    let url: RelayUrl = value.parse().map_err(|_| Failure("invalid_relay_url"))?;
    let host = url.host_str().ok_or(Failure("invalid_relay_url"))?;
    // HTTP is reserved for a relay running on this host; ordinary relays use TLS.
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
        return Err(Failure("invalid_relay_url"));
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Config> {
        Config::parse(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn direct_listener_requires_explicit_mode_and_nonzero_timeout() {
        let parsed = parse(&[
            "listen",
            "--mode",
            "direct",
            "--timeout-ms",
            "42",
            "--bind",
            "127.0.0.1:0",
        ])
        .unwrap();
        assert_eq!(parsed.mode, Mode::Direct);
        assert_eq!(parsed.timeout, Duration::from_millis(42));
        assert_eq!(parsed.bind.unwrap().port(), 0);
        for args in [
            vec!["listen"],
            vec!["listen", "--mode", "direct"],
            vec!["listen", "--mode", "bogus", "--timeout-ms", "10"],
            vec!["listen", "--mode", "direct", "--timeout-ms", "0"],
            vec!["listen", "--mode", "direct", "--timeout-ms", "-1"],
            vec![
                "listen",
                "--mode",
                "direct",
                "--timeout-ms",
                "10",
                "--mode",
                "direct",
            ],
            vec![
                "listen",
                "--mode",
                "direct",
                "--timeout-ms",
                "10",
                "--secret",
                "x",
            ],
        ] {
            assert!(parse(&args).is_err());
        }
    }

    #[test]
    fn parses_explicit_peer_and_repeated_hints() {
        let id = iroh::SecretKey::generate().public().to_string();
        let parsed = parse(&[
            "connect",
            "--mode",
            "direct",
            "--timeout-ms",
            "20",
            "--peer",
            &id,
            "--peer-addr",
            "127.0.0.1:1",
            "--peer-addr",
            "[::1]:2",
        ])
        .unwrap();
        assert_eq!(parsed.peer_addrs.len(), 2);
        assert_eq!(parsed.peer.unwrap().to_string(), id);
    }

    #[test]
    fn rejects_conflicting_and_missing_route_configuration() {
        for args in [
            vec!["listen", "--mode", "relay", "--timeout-ms", "10"],
            vec!["listen", "--mode", "auto", "--timeout-ms", "10"],
            vec![
                "listen",
                "--mode",
                "direct",
                "--timeout-ms",
                "10",
                "--n0-relays",
            ],
            vec![
                "listen",
                "--mode",
                "relay",
                "--timeout-ms",
                "10",
                "--n0-relays",
                "--bind",
                "127.0.0.1:0",
            ],
            vec![
                "listen",
                "--mode",
                "relay",
                "--timeout-ms",
                "10",
                "--n0-relays",
                "--relay-url",
                "https://example.com",
            ],
            vec!["connect", "--mode", "direct", "--timeout-ms", "10"],
        ] {
            assert!(parse(&args).is_err());
        }
        let parsed = parse(&[
            "listen",
            "--mode",
            "auto",
            "--timeout-ms",
            "10",
            "--relay-url",
            "https://example.com",
            "--relay-url",
            "https://example.org",
        ])
        .unwrap();
        assert_eq!(parsed.relays.len(), 2);
    }

    #[test]
    fn relay_urls_reject_secrets_and_non_http_schemes() {
        for invalid in [
            "wat",
            "ftp://example.com",
            "http://example.com",
            "https://user:secret@example.com",
            "https://example.com?token=secret",
            "https://example.com#secret",
            "https://example.com/\n",
        ] {
            assert_eq!(relay_url(invalid), Err(Failure("invalid_relay_url")));
        }
        for valid in [
            "https://example.com",
            "http://127.0.0.1:3340",
            "http://[::1]:3340",
            "http://localhost:3340",
        ] {
            assert!(relay_url(valid).is_ok());
        }
    }

    #[test]
    fn peer_hints_reject_unspecified_addresses_and_bad_keys() {
        for invalid in ["0.0.0.0:42", "127.0.0.1:0", "224.0.0.1:42", "hostname:42"] {
            assert!(socket(invalid, false).is_err());
        }
        assert!(endpoint_id("wrong").is_err());
        assert!(endpoint_id(&"gg".repeat(32)).is_err());
        assert!(endpoint_id(&"ab".repeat(31)).is_err());
    }

    #[test]
    fn wait_for_route_requires_auto_and_concrete_path() {
        let parsed = parse(&[
            "listen",
            "--mode",
            "auto",
            "--timeout-ms",
            "10",
            "--n0-relays",
            "--wait-for-route",
            "direct",
        ])
        .unwrap();
        assert_eq!(parsed.wait_route, Some(Mode::Direct));
        assert!(
            parse(&[
                "listen",
                "--mode",
                "direct",
                "--timeout-ms",
                "10",
                "--wait-for-route",
                "direct"
            ])
            .is_err()
        );
        assert!(
            parse(&[
                "listen",
                "--mode",
                "auto",
                "--timeout-ms",
                "10",
                "--n0-relays",
                "--wait-for-route",
                "auto"
            ])
            .is_err()
        );
    }
}
