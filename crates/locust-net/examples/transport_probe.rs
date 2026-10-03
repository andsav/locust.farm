//! Two-machine transport fixture, not a daemon protocol implementation.
//!
//! Contact records contain intentional network contact hints. Keep those records
//! private; sanitized path and result records are suitable for shared evidence.

#[path = "transport_probe/cli.rs"]
mod cli;
#[path = "transport_probe/exchange.rs"]
mod exchange;
#[path = "transport_probe/output.rs"]
mod output;

use std::cell::Cell;
use std::io::{self, Write};
use std::process::ExitCode;

use iroh::endpoint::{NetReportConfig, PortmapperConfig, presets};
use iroh::{EndpointAddr, RelayMap, RelayMode};
use locust_net::{Endpoint, FrameLimits, PathKind, PeerConnection};
use locust_proto::limits::MAX_HELLO_FRAME_BYTES;
use tokio::time::Instant;

use cli::{Config, Mode, Role};
use output::Output;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Failure(&'static str);

type Result<T> = std::result::Result<T, Failure>;

// Parsing/help precede accepting a caller deadline and use ordinary CLI I/O.
fn parse_record(line: impl AsRef<str>) -> Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    writeln!(stdout, "{}", line.as_ref()).map_err(|_| Failure("output"))?;
    stdout.flush().map_err(|_| Failure("output"))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = match std::env::args_os()
        .skip(1)
        .map(|arg| arg.into_string())
        .collect()
    {
        Ok(args) => args,
        Err(_) => {
            eprintln!("transport_probe: invalid_utf8");
            if parse_record("record=result status=failure phase=parse error=invalid_utf8").is_err()
            {
                eprintln!("transport_probe: output");
            }
            return ExitCode::from(2);
        }
    };
    if args == ["--help"] || args == ["-h"] {
        println!("{}", cli::USAGE);
        return ExitCode::SUCCESS;
    }
    let config = match Config::parse(args) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("transport_probe: {}\n{}", error.0, cli::USAGE);
            if parse_record(format!(
                "record=result status=failure phase=parse error={}",
                error.0
            ))
            .is_err()
            {
                eprintln!("transport_probe: output");
            }
            return ExitCode::from(2);
        }
    };
    let Some(deadline) = Instant::now().checked_add(config.timeout) else {
        eprintln!("transport_probe: timeout_out_of_range");
        return ExitCode::from(2);
    };
    let output = match Output::start() {
        Ok(output) => output,
        Err(_) => return ExitCode::from(1),
    };
    let phase = Cell::new("bind");
    let mut endpoint = None;
    // One caller-selected deadline covers binding, relay readiness, handshake,
    // streams, and the complete exchange. Dropping the work future cancels it.
    let outcome = tokio::select! {
        result = run(&config, &mut endpoint, &phase, &output) => result,
        _ = tokio::time::sleep_until(deadline) => Err(Failure("timeout")),
        signal = tokio::signal::ctrl_c() => {
            match signal {
                Ok(()) => Err(Failure("cancelled")),
                Err(_) => Err(Failure("signal")),
            }
        }
    };
    if let Some(endpoint) = endpoint {
        endpoint.close().await;
    }
    let (status, error, code) = match outcome {
        Ok(()) => ("success", "none", 0),
        Err(Failure("timeout")) => ("failure", "timeout", 124),
        Err(Failure("cancelled")) => ("failure", "cancelled", 130),
        Err(error) => ("failure", error.0, 1),
    };
    // A separate output grace uses the same caller-selected duration. A blocked
    // terminal may prevent the final record, but cannot hold this process open.
    let flushed = tokio::time::timeout(config.timeout, async {
        if code != 0 {
            // Never print raw transport errors, peer-controlled frames or argv.
            output
                .error(format!(
                    "transport_probe: phase={} error={error}",
                    phase.get()
                ))
                .await?;
        }
        output
            .record(format!(
                "record=result status={status} mode={} phase={} error={error}",
                config.mode.as_str(),
                phase.get()
            ))
            .await
    })
    .await;
    if !matches!(flushed, Ok(Ok(()))) && code == 0 {
        return ExitCode::from(1);
    }
    ExitCode::from(code)
}

async fn run(
    config: &Config,
    endpoint_slot: &mut Option<Endpoint>,
    phase: &Cell<&'static str>,
    output: &Output,
) -> Result<()> {
    output.record(format!(
        "record=metadata version={} protocol_version={} iroh_version=1.3.0 os={} arch={} mode={} role={} timeout_ms={} output_grace_ms={} fixture=transport discovery=disabled port_mapping=disabled requested_route={} https_probes={}",
        env!("CARGO_PKG_VERSION"),
        locust_proto::PROTOCOL_VERSION,
        std::env::consts::OS,
        std::env::consts::ARCH,
        config.mode.as_str(),
        config.role.as_str(),
        config.timeout.as_millis(),
        config.timeout.as_millis(),
        config.wait_route.unwrap_or(config.mode).as_str(),
        if config.mode == Mode::Direct { "disabled" } else { "enabled" }
    )).await?;
    let mut builder = iroh::Endpoint::builder(presets::Minimal)
        .clear_address_lookup()
        .portmapper_config(PortmapperConfig::Disabled)
        .net_report_config(net_report_policy(config.mode));
    builder = builder.relay_mode(if config.n0_relays {
        RelayMode::Default
    } else if !config.relays.is_empty() {
        RelayMode::Custom(config.relays.iter().cloned().collect::<RelayMap>())
    } else {
        RelayMode::Disabled
    });
    if config.mode == Mode::Relay {
        builder = builder.clear_ip_transports();
    } else if let Some(bind) = config.bind {
        builder = builder
            .clear_ip_transports()
            .bind_addr(bind)
            .map_err(|_| Failure("bind_config"))?;
    }
    *endpoint_slot = Some(Endpoint::bind(builder).await.map_err(|_| Failure("bind"))?);
    let endpoint = endpoint_slot.as_ref().ok_or(Failure("bind"))?;
    output
        .record(format!("record=local endpoint_id={}", endpoint.id()))
        .await?;
    if config.mode != Mode::Direct {
        phase.set("online");
        endpoint.online().await;
    }
    let contact = endpoint.addr();
    for addr in contact.ip_addrs() {
        output
            .record(format!(
                "record=contact endpoint_id={} direct_addr={addr}",
                endpoint.id()
            ))
            .await?;
    }
    for relay in contact.relay_urls() {
        output
            .record(format!(
                "record=contact endpoint_id={} relay_url={relay}",
                endpoint.id()
            ))
            .await?;
    }
    output
        .record(format!(
            "record=ready endpoint_id={} mode={}",
            endpoint.id(),
            config.mode.as_str()
        ))
        .await?;
    let connection = match config.role {
        Role::Listen => {
            phase.set("accept");
            let incoming = endpoint.accept().await.ok_or(Failure("endpoint_closed"))?;
            phase.set("handshake");
            let connection = incoming.accept().await.map_err(|_| Failure("handshake"))?;
            if config
                .expect_peer
                .is_some_and(|expected| expected != connection.remote_id())
            {
                connection.close(b"unexpected peer");
                return Err(Failure("unexpected_peer"));
            }
            connection
        }
        Role::Connect => {
            phase.set("handshake");
            let peer = config.peer.ok_or(Failure("peer_required"))?;
            let id = iroh::EndpointId::from_bytes(peer.as_bytes())
                .map_err(|_| Failure("invalid_peer"))?;
            let mut addr = EndpointAddr::new(id);
            for direct in &config.peer_addrs {
                addr = addr.with_ip_addr(*direct);
            }
            if let Some(relay) = &config.peer_relay {
                addr = addr.with_relay_url(relay.clone());
            }
            let connection = endpoint
                .connect(addr)
                .await
                .map_err(|_| Failure("connect"))?;
            if connection.remote_id() != peer {
                return Err(Failure("unexpected_peer"));
            }
            connection
        }
    };
    output
        .record(format!("record=peer peer_id={}", connection.remote_id()))
        .await?;
    if let Some(route) = config.wait_route {
        phase.set("route_wait");
        loop {
            let desired = match route {
                Mode::Direct => PathKind::Direct,
                Mode::Relay => PathKind::Relay,
                Mode::Auto => return Err(Failure("invalid_wait_route")),
            };
            if connection
                .path_snapshot()
                .iter()
                .any(|path| path.selected && path.kind == desired)
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }
    phase.set("route");
    let expected_route = config.wait_route.unwrap_or(config.mode);
    observe(&connection, expected_route, "before", output).await?;
    phase.set("exchange");
    let limits = FrameLimits {
        max_send_bytes: MAX_HELLO_FRAME_BYTES as u32,
        max_receive_bytes: MAX_HELLO_FRAME_BYTES as u32,
    };
    match config.role {
        Role::Listen => {
            let mut link = connection
                .accept_link(limits)
                .await
                .map_err(|_| Failure("stream"))?;
            exchange::listen(&mut link).await?;
            phase.set("route");
            observe(&connection, expected_route, "after", output).await?;
            phase.set("exchange");
            finish(link, false).await?;
        }
        Role::Connect => {
            let mut link = connection
                .open_link(limits)
                .await
                .map_err(|_| Failure("stream"))?;
            exchange::connect(&mut link).await?;
            phase.set("route");
            observe(&connection, expected_route, "after", output).await?;
            phase.set("exchange");
            finish(link, true).await?;
        }
    }
    phase.set("complete");
    Ok(())
}

async fn finish(link: locust_net::IrohLink, connecting: bool) -> Result<()> {
    let (mut sender, mut receiver) = link.into_split();
    // Drain the peer's completion/EOF while waiting for our FIN to be ACKed.
    // Transport acknowledgment and validated application messages are distinct
    // evidence; neither claims membership or persistence.
    tokio::try_join!(
        async {
            sender
                .finish_acknowledged()
                .await
                .map_err(|_| Failure("delivery"))
        },
        exchange::completion(&mut receiver, connecting),
    )?;
    Ok(())
}

async fn observe(
    connection: &PeerConnection,
    mode: Mode,
    phase: &str,
    output: &Output,
) -> Result<()> {
    let paths = connection.path_snapshot();
    for path in &paths {
        let kind = match path.kind {
            PathKind::Direct => "direct",
            PathKind::Relay => "relay",
            PathKind::Other => "other",
        };
        output
            .record(format!(
                "record=path phase={phase} kind={kind} selected={} rtt_us={}",
                path.selected,
                path.rtt.as_micros()
            ))
            .await?;
    }
    let selected: Vec<_> = paths.iter().filter(|path| path.selected).collect();
    if selected.is_empty() {
        return Err(Failure("route_unobserved"));
    }
    if selected.iter().any(|path| match mode {
        Mode::Direct => path.kind != PathKind::Direct,
        Mode::Relay => path.kind != PathKind::Relay,
        Mode::Auto => false,
    }) {
        return Err(Failure("wrong_route"));
    }
    Ok(())
}

fn net_report_policy(mode: Mode) -> NetReportConfig {
    let mut config = NetReportConfig::minimal();
    // Forced relay has no IP transports and thus no QAD. HTTPS probes are
    // required to select a home relay and complete Endpoint::online().
    config.https_probes = mode != Mode::Direct;
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_and_auto_can_select_home_relay_without_captive_portal_checks() {
        for mode in [Mode::Relay, Mode::Auto] {
            let config = net_report_policy(mode);
            assert!(config.https_probes);
            assert!(!config.captive_portal_check);
        }
        let direct = net_report_policy(Mode::Direct);
        assert!(!direct.https_probes);
        assert!(!direct.captive_portal_check);
    }
}
