//! The asynchronous transport shell. Membership and reconciliation stay in
//! the engine; each stream has one reader and one ordered writer.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

use locust_net::{
    Endpoint, EndpointConfig, FrameError, FrameLimits, IpTransport, Lookup, PeerConnection,
    PeerLink, PeerReceiver, PeerSender, RelayConfig, TransportBudget,
};
use locust_proto::engine::{ExchangeId, PeerInput, PeerOutput};
use locust_proto::id::EndpointId;
use locust_proto::limits::MAX_PEER_FRAME_BYTES;
use locust_proto::sync::{Refusal, SyncMessage};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinSet;

use super::connection::stopping;
use super::worker::{Job, NetworkOutput};
use crate::failure::Failure;

/// A deadline for one stalled transport operation, not an exchange-duration
/// limit. An exchange that continues making progress has no overall cutoff.
const IO_IDLE: Duration = Duration::from_secs(30);

pub(crate) async fn bind(secret_key: [u8; 32]) -> Result<Endpoint, Failure> {
    let relays = match std::env::var("LOCUST_RELAY") {
        Err(std::env::VarError::NotPresent) => RelayConfig::N0,
        Ok(value) if value == "n0" => RelayConfig::N0,
        Ok(value) if value == "none" => RelayConfig::Disabled,
        Ok(value) => RelayConfig::Custom(vec![value]),
        Err(_) => return Err(Failure::usage("LOCUST_RELAY must be UTF-8")),
    };
    let lookup = match std::env::var("LOCUST_LOOKUP").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("all") => Lookup::default(),
        Ok("local") => Lookup {
            local_network: true,
            mainline: false,
        },
        Ok("mainline") => Lookup {
            local_network: false,
            mainline: true,
        },
        Ok("none") => Lookup::DISABLED,
        _ => {
            return Err(Failure::usage(
                "LOCUST_LOOKUP must be all, local, mainline or none",
            ));
        }
    };
    let ip_transport = match std::env::var("LOCUST_BIND") {
        Err(std::env::VarError::NotPresent) => IpTransport::Default,
        Ok(value) => IpTransport::Bind(
            value
                .parse()
                .map_err(|_| Failure::usage("LOCUST_BIND must be an IP socket address"))?,
        ),
        Err(_) => return Err(Failure::usage("LOCUST_BIND must be UTF-8")),
    };
    Endpoint::bind(EndpointConfig {
        secret_key,
        relays,
        lookup,
        ip_transport,
        port_mapping: true,
        budget: TransportBudget::default(),
    })
    .await
    .map_err(|error| Failure::unavailable(format!("peer endpoint could not start: {error}")))
}

enum Command {
    Send(SyncMessage),
    Finish,
    Refuse(Refusal),
}

struct Exchange {
    commands: mpsc::UnboundedSender<Command>,
    admitted: watch::Sender<bool>,
}

enum Event {
    Connected {
        connection: PeerConnection,
        opening: Option<ExchangeId>,
    },
    Link {
        link: Box<PeerLink>,
        connection: PeerConnection,
        exchange: Option<ExchangeId>,
    },
    OpenFailed(ExchangeId),
    ConnectionClosed(EndpointId),
    Ended(ExchangeId),
}

/// Keeps accepting peers while handshakes, engine work and other streams wait.
pub(crate) async fn serve(
    endpoint: Endpoint,
    jobs: Sender<Job>,
    mut output: mpsc::UnboundedReceiver<NetworkOutput>,
    mut stop: watch::Receiver<bool>,
) {
    let (events, mut incoming) = mpsc::unbounded_channel();
    let mut tasks = JoinSet::new();
    let mut connections: HashMap<EndpointId, Vec<PeerConnection>> = HashMap::new();
    let mut exchanges: HashMap<ExchangeId, Exchange> = HashMap::new();
    let mut accepted = 0u64;
    let mut hints = endpoint.hints();
    let mut poll = tokio::time::interval(Duration::from_secs(1));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let _ = jobs.send(Job::Peer(PeerInput::Endpoint {
        endpoint: endpoint.id(),
        hints: hints.clone(),
    }));
    loop {
        tokio::select! {
            biased;
            () = stopping(&mut stop) => break,
            Some(action) = output.recv() => match action {
                NetworkOutput::Processed(done) => { let _ = done.send(()); }
                NetworkOutput::Action(action) => match action {
                    PeerOutput::Open { exchange, endpoint: peer, hints } => {
                        let known = connections.get(&peer).and_then(|items| items.iter().find(|item| !item.is_closed())).cloned();
                        let events = events.clone();
                        if let Some(connection) = known {
                            tasks.spawn(open_link(connection, exchange, events));
                        } else {
                            let endpoint = endpoint.clone();
                            tasks.spawn(async move {
                                match tokio::time::timeout(IO_IDLE, endpoint.connect(peer, &hints)).await {
                                    Ok(Ok(connection)) => { let _ = events.send(Event::Connected { connection, opening: Some(exchange) }); }
                                    _ => { let _ = events.send(Event::OpenFailed(exchange)); }
                                }
                            });
                        }
                    }
                    PeerOutput::Admit(exchange) => {
                        if let Some(link) = exchanges.get(&exchange) { link.admitted.send_replace(true); }
                    }
                    PeerOutput::Send { exchange, frame } => {
                        if exchanges.get(&exchange).is_none_or(|link| link.commands.send(Command::Send(frame)).is_err()) {
                            let _ = jobs.send(Job::Peer(PeerInput::Closed(exchange)));
                        }
                    }
                    PeerOutput::Finish(exchange) => {
                        if exchanges.get(&exchange).is_none_or(|link| link.commands.send(Command::Finish).is_err()) {
                            let _ = jobs.send(Job::Peer(PeerInput::Closed(exchange)));
                        }
                    }
                }
            },
            Some(event) = incoming.recv() => match event {
                Event::Connected { connection, opening } => {
                    let peer = connection.remote_id();
                    let held = connections.entry(peer).or_default();
                    held.retain(|connection| !connection.is_closed());
                    held.push(connection.clone());
                    let _ = jobs.send(Job::Peer(PeerInput::Connection { endpoint: peer, connected: true }));
                    eprintln!("locust: peer {peer} paths {:?}", connection.path_snapshot());
                    tasks.spawn(accept_links(connection.clone(), events.clone(), stop.clone()));
                    if let Some(exchange) = opening { tasks.spawn(open_link(connection, exchange, events.clone())); }
                }
                Event::Link { link, connection, exchange } => {
                    let dialed = exchange.is_some();
                    let exchange = exchange.unwrap_or_else(|| { accepted += 1; ExchangeId::Accepted(accepted) });
                    let (commands, receiver) = mpsc::unbounded_channel();
                    let (admitted, admission) = watch::channel(dialed);
                    exchanges.insert(exchange, Exchange { commands, admitted });
                    let input = if dialed { PeerInput::Opened(exchange) } else {
                        PeerInput::Accepted { exchange, remote: link.remote_id() }
                    };
                    let _ = jobs.send(Job::Peer(input));
                    tasks.spawn(exchange_task(*link, connection, exchange, receiver, admission, jobs.clone(), events.clone(), stop.clone()));
                }
                Event::OpenFailed(exchange) => { let _ = jobs.send(Job::Peer(PeerInput::OpenFailed(exchange))); }
                Event::ConnectionClosed(peer) => {
                    let held = connections.entry(peer).or_default();
                    held.retain(|connection| !connection.is_closed());
                    if held.is_empty() {
                        connections.remove(&peer);
                        let _ = jobs.send(Job::Peer(PeerInput::Connection { endpoint: peer, connected: false }));
                    }
                }
                Event::Ended(exchange) => { exchanges.remove(&exchange); }
            },
            connection = endpoint.accept() => match connection {
                Some(incoming) => {
                    let events = events.clone();
                    tasks.spawn(async move {
                        if let Ok(Ok(connection)) = tokio::time::timeout(IO_IDLE, incoming.accept()).await {
                            let _ = events.send(Event::Connected { connection, opening: None });
                        }
                    });
                }
                None => break,
            },
            changed = endpoint.hints_changed(&hints) => match changed {
                Some(changed) => {
                    hints = changed;
                    let _ = jobs.send(Job::Peer(PeerInput::Endpoint { endpoint: endpoint.id(), hints: hints.clone() }));
                }
                None => break,
            },
            _ = poll.tick() => { let _ = jobs.send(Job::Peer(PeerInput::Poll)); }
            Some(result) = tasks.join_next(), if !tasks.is_empty() => {
                if let Err(error) = result { eprintln!("locust: peer I/O task ended: {error}"); }
            }
        }
    }
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    for connection in connections.into_values().flatten() {
        connection.close();
    }
    endpoint.close().await;
}

async fn open_link(
    connection: PeerConnection,
    exchange: ExchangeId,
    events: mpsc::UnboundedSender<Event>,
) {
    match tokio::time::timeout(IO_IDLE, connection.open_link(FrameLimits::peer())).await {
        Ok(Ok(link)) => {
            let _ = events.send(Event::Link {
                link: Box::new(link),
                connection,
                exchange: Some(exchange),
            });
        }
        _ => {
            let _ = events.send(Event::OpenFailed(exchange));
        }
    }
}

async fn accept_links(
    connection: PeerConnection,
    events: mpsc::UnboundedSender<Event>,
    mut stop: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            () = stopping(&mut stop) => break,
            _ = connection.closed() => break,
            link = connection.accept_link(FrameLimits::hello()) => match link {
                Ok(link) => { let _ = events.send(Event::Link { link: Box::new(link), connection: connection.clone(), exchange: None }); }
                Err(_) => break,
            }
        }
    }
    let _ = events.send(Event::ConnectionClosed(connection.remote_id()));
}

#[allow(clippy::too_many_arguments)]
async fn exchange_task(
    link: PeerLink,
    _connection: PeerConnection,
    exchange: ExchangeId,
    commands: mpsc::UnboundedReceiver<Command>,
    admission: watch::Receiver<bool>,
    jobs: Sender<Job>,
    events: mpsc::UnboundedSender<Event>,
    mut stop: watch::Receiver<bool>,
) {
    let (mut sender, mut receiver) = link.into_split();
    let ending = Arc::new(AtomicBool::new(false));
    let (reject, rejected) = oneshot::channel();
    let complete = {
        let writer = write_frames(
            &mut sender,
            exchange,
            commands,
            admission.clone(),
            jobs.clone(),
            ending.clone(),
            rejected,
        );
        let reader = read_frames(&mut receiver, exchange, admission, jobs.clone());
        tokio::pin!(writer, reader);
        tokio::select! {
            biased;
            result = &mut writer => result,
            result = &mut reader => match result {
                ReadEnd::Refuse(reason) => {
                    // Decode/prefix failures still get an explicit protocol reply.
                    let _ = reject.send(reason);
                    tokio::select! { _ = &mut writer => {}, () = stopping(&mut stop) => {} }
                    false
                }
                ReadEnd::Eof { done } if done || ending.load(Ordering::Acquire) => {
                    // A half-close after Done can precede our final delivery ack.
                    tokio::select! { result = &mut writer => result, () = stopping(&mut stop) => false }
                }
                ReadEnd::Eof { .. } | ReadEnd::Failed => false,
            },
            () = stopping(&mut stop) => false,
        }
    };
    if !complete {
        // Cancellation may interrupt a partial send; never leave that half usable.
        let _ = sender.abort();
        let _ = receiver.abort();
    }
    let input = if complete {
        PeerInput::Finished(exchange)
    } else {
        PeerInput::Closed(exchange)
    };
    let _ = jobs.send(Job::Peer(input));
    let _ = events.send(Event::Ended(exchange));
}

#[allow(clippy::too_many_arguments)]
async fn write_frames(
    sender: &mut PeerSender,
    exchange: ExchangeId,
    mut commands: mpsc::UnboundedReceiver<Command>,
    admission: watch::Receiver<bool>,
    jobs: Sender<Job>,
    ending: Arc<AtomicBool>,
    mut rejected: oneshot::Receiver<Refusal>,
) -> bool {
    loop {
        let command = tokio::select! {
            biased;
            reason = &mut rejected => match reason {
                Ok(reason) => Command::Refuse(reason),
                Err(_) => break,
            },
            command = tokio::time::timeout(IO_IDLE, commands.recv()) => match command {
                Ok(Some(command)) => command,
                _ => break,
            },
        };
        if *admission.borrow() {
            sender.set_send_limit(MAX_PEER_FRAME_BYTES);
        }
        match command {
            Command::Refuse(reason) => {
                if matches!(
                    tokio::time::timeout(IO_IDLE, sender.send(&SyncMessage::Refused(reason))).await,
                    Ok(Ok(()))
                ) {
                    let _ = tokio::time::timeout(IO_IDLE, sender.finish_acknowledged()).await;
                }
                // This exchange failed even if its refusal reached the peer.
                return false;
            }
            Command::Send(frame) => {
                if matches!(frame, SyncMessage::Done | SyncMessage::Refused(_)) {
                    ending.store(true, Ordering::Release);
                }
                if !matches!(
                    tokio::time::timeout(IO_IDLE, sender.send(&frame)).await,
                    Ok(Ok(()))
                ) {
                    let _ = sender.abort();
                    return false;
                }
                if jobs.send(Job::Peer(PeerInput::Writable(exchange))).is_err() {
                    return false;
                }
            }
            Command::Finish => {
                ending.store(true, Ordering::Release);
                let done = matches!(
                    tokio::time::timeout(IO_IDLE, sender.finish_acknowledged()).await,
                    Ok(Ok(()))
                );
                if !done {
                    let _ = sender.abort();
                }
                return done;
            }
        }
    }
    let _ = sender.abort();
    false
}

enum ReadEnd {
    Eof { done: bool },
    Refuse(Refusal),
    Failed,
}

async fn read_frames(
    receiver: &mut PeerReceiver,
    exchange: ExchangeId,
    admission: watch::Receiver<bool>,
    jobs: Sender<Job>,
) -> ReadEnd {
    let mut done = false;
    loop {
        if *admission.borrow() {
            receiver.set_receive_limit(MAX_PEER_FRAME_BYTES);
        }
        let frame = match tokio::time::timeout(IO_IDLE, receiver.recv()).await {
            Ok(Ok(Some(frame))) => frame,
            Ok(Ok(None)) => return ReadEnd::Eof { done },
            Ok(Err(FrameError::Protocol(reason))) => return ReadEnd::Refuse(reason),
            Ok(Err(FrameError::TooLarge { .. })) => return ReadEnd::Refuse(Refusal::LimitExceeded),
            _ => return ReadEnd::Failed,
        };
        done |= matches!(frame, SyncMessage::Done | SyncMessage::Refused(_));
        let (processed, applied) = oneshot::channel();
        if jobs
            .send(Job::PeerFrame {
                input: PeerInput::Frame { exchange, frame },
                processed,
            })
            .is_err()
        {
            return ReadEnd::Failed;
        }
        // The dispatcher applies Admit before acknowledging this frame. The
        // next prefix cannot accidentally retain the old pre-admission limit.
        if !matches!(tokio::time::timeout(IO_IDLE, applied).await, Ok(Ok(()))) {
            return ReadEnd::Failed;
        }
    }
}
