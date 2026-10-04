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
use locust_proto::limits::{MAX_HEADER_BYTES, MAX_HELLO_FRAME_BYTES, MAX_PEER_FRAME_BYTES};
use locust_proto::sync::{Refusal, SyncMessage};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot, watch};
use tokio::task::JoinSet;
use tokio::time::Instant;

use super::connection::stopping;
use super::worker::{Job, NetworkOutput};
use crate::failure::Failure;

/// Maximum time between completed frames in either exchange direction.
/// This also bounds initial handshakes and connection admission; an admitted
/// exchange that keeps completing frames has no overall duration limit.
const IO_IDLE: Duration = Duration::from_secs(30);
/// Aggregate QUIC receive credit reserved for unauthenticated connections.
const UNADMITTED_RECEIVE_BYTES: usize = 64 * 1024 * 1024;
const EVIDENCE_FRAME_BYTES: usize = 2 * (MAX_HEADER_BYTES + 128);
fn admission_slots() -> usize {
    (UNADMITTED_RECEIVE_BYTES / TransportBudget::default().connection_receive_bytes as usize).max(1)
}
struct Connection {
    connection: PeerConnection,
    admitted: watch::Sender<bool>,
}

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
    limit: watch::Sender<usize>,
    connection_admitted: watch::Sender<bool>,
    connection: PeerConnection,
}

enum Event {
    Connected {
        connection: PeerConnection,
        opening: Option<ExchangeId>,
        permit: OwnedSemaphorePermit,
        deadline: Instant,
    },
    Link {
        link: Box<PeerLink>,
        connection: PeerConnection,
        exchange: Option<ExchangeId>,
        admission: watch::Sender<bool>,
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
    let mut connections: HashMap<EndpointId, Vec<Connection>> = HashMap::new();
    let mut exchanges: HashMap<ExchangeId, Exchange> = HashMap::new();
    let permits = Arc::new(Semaphore::new(admission_slots()));
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
                        let known = connections.get(&peer).and_then(|items| items.iter().find(|item| !item.connection.is_closed())).map(|item| (item.connection.clone(), item.admitted.clone()));
                        let events = events.clone();
                        if let Some((connection, admission)) = known {
                            tasks.spawn(open_link(connection, exchange, admission, events));
                        } else {
                            let Ok(permit) = permits.clone().try_acquire_owned() else {
                                let _ = jobs.send(Job::Peer(PeerInput::OpenFailed(exchange)));
                                continue;
                            };
                            let deadline = Instant::now() + IO_IDLE;
                            let endpoint = endpoint.clone();
                            tasks.spawn(async move {
                                match tokio::time::timeout(IO_IDLE, endpoint.connect(peer, &hints)).await {
                                    Ok(Ok(connection)) => { let _ = events.send(Event::Connected { connection, opening: Some(exchange), permit, deadline }); }
                                    _ => { let _ = events.send(Event::OpenFailed(exchange)); }
                                }
                            });
                        }
                    }
                    PeerOutput::Admit(exchange) => {
                        if let Some(link) = exchanges.get(&exchange) { link.limit.send_replace(MAX_PEER_FRAME_BYTES); link.connection_admitted.send_replace(true); }
                    }
                    PeerOutput::Evidence(exchange) => {
                        if let Some(link) = exchanges.get(&exchange) { link.limit.send_modify(|limit| *limit = (*limit).max(EVIDENCE_FRAME_BYTES)); }
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
                Event::Connected { connection, opening, permit, deadline } => {
                    let peer = connection.remote_id();
                    let held = connections.entry(peer).or_default();
                    held.retain(|item| !item.connection.is_closed());
                    // Keep the two connections needed by simultaneous dials.
                    // Later exchanges reuse them; extra connections close.
                    if held.len() >= 2 {
                        connection.close();
                        if let Some(exchange) = opening { let _ = jobs.send(Job::Peer(PeerInput::OpenFailed(exchange))); }
                        continue;
                    }
                    let (admission, admitted) = watch::channel(false);
                    held.push(Connection { connection: connection.clone(), admitted: admission.clone() });
                    let _ = jobs.send(Job::Peer(PeerInput::Connection { endpoint: peer, connected: true }));
                    super::log(format_args!("locust: peer {peer} paths {:?}", connection.path_snapshot()));
                    tasks.spawn(accept_links(connection.clone(), admission.clone(), events.clone(), stop.clone()));
                    let guarded = connection.clone();
                    tasks.spawn(async move { admission_guard(guarded, admitted, permit, deadline).await; });
                    if let Some(exchange) = opening { tasks.spawn(open_link(connection, exchange, admission, events.clone())); }
                }
                Event::Link { link, connection, exchange, admission } => {
                    let dialed = exchange.is_some();
                    let exchange = exchange.unwrap_or_else(|| { accepted += 1; ExchangeId::Accepted(accepted) });
                    let (commands, receiver) = mpsc::unbounded_channel();
                    let (limit, limits) = watch::channel(if dialed { MAX_PEER_FRAME_BYTES } else { MAX_HELLO_FRAME_BYTES });
                    exchanges.insert(exchange, Exchange { commands, limit, connection_admitted: admission, connection: connection.clone() });
                    let input = if dialed { PeerInput::Opened(exchange) } else {
                        PeerInput::Accepted { exchange, remote: link.remote_id() }
                    };
                    let _ = jobs.send(Job::Peer(input));
                    tasks.spawn(exchange_task(*link, connection, exchange, receiver, limits, jobs.clone(), events.clone(), stop.clone()));
                }
                Event::OpenFailed(exchange) => { let _ = jobs.send(Job::Peer(PeerInput::OpenFailed(exchange))); }
                Event::ConnectionClosed(peer) => {
                    let held = connections.entry(peer).or_default();
                    held.retain(|item| !item.connection.is_closed());
                    if held.is_empty() {
                        connections.remove(&peer);
                        let _ = jobs.send(Job::Peer(PeerInput::Connection { endpoint: peer, connected: false }));
                    }
                }
                Event::Ended(exchange) => {
                    if let Some(link) = exchanges.remove(&exchange) && !*link.connection_admitted.borrow() { link.connection.close(); }
                }
            },
            connection = endpoint.accept() => match connection {
                Some(incoming) => {
                    let Ok(permit) = permits.clone().try_acquire_owned() else { incoming.refuse(); continue; };
                    let deadline = Instant::now() + IO_IDLE;
                    let events = events.clone();
                    tasks.spawn(async move {
                        if let Ok(Ok(connection)) = tokio::time::timeout(IO_IDLE, incoming.accept()).await {
                            let _ = events.send(Event::Connected { connection, opening: None, permit, deadline });
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
                if let Err(error) = result { super::log(format_args!("locust: peer I/O task ended: {error}")); }
            }
        }
    }
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    for connection in connections.into_values().flatten() {
        connection.connection.close();
    }
    endpoint.close().await;
}

async fn open_link(
    connection: PeerConnection,
    exchange: ExchangeId,
    admission: watch::Sender<bool>,
    events: mpsc::UnboundedSender<Event>,
) {
    match tokio::time::timeout(IO_IDLE, connection.open_link(FrameLimits::peer())).await {
        Ok(Ok(link)) => {
            let _ = events.send(Event::Link {
                link: Box::new(link),
                connection,
                exchange: Some(exchange),
                admission,
            });
        }
        _ => {
            let _ = events.send(Event::OpenFailed(exchange));
        }
    }
}

async fn accept_links(
    connection: PeerConnection,
    admission: watch::Sender<bool>,
    events: mpsc::UnboundedSender<Event>,
    mut stop: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            () = stopping(&mut stop) => break,
            _ = connection.closed() => break,
            link = connection.accept_link(FrameLimits::hello()) => match link {
                Ok(link) => { let _ = events.send(Event::Link { link: Box::new(link), connection: connection.clone(), exchange: None, admission: admission.clone() }); }
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
    admission: watch::Receiver<usize>,
    jobs: Sender<Job>,
    events: mpsc::UnboundedSender<Event>,
    mut stop: watch::Receiver<bool>,
) {
    let (mut sender, mut receiver) = link.into_split();
    let (progress, activity) = watch::channel(Instant::now());
    let idle = idle_deadline(activity, IO_IDLE);
    tokio::pin!(idle);
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
            progress.clone(),
        );
        let reader = read_frames(&mut receiver, exchange, admission, jobs.clone(), progress);
        tokio::pin!(writer, reader);
        tokio::select! {
            biased;
            result = &mut writer => result,
            result = &mut reader => match result {
                ReadEnd::Refuse(reason) => {
                    // Decode/prefix failures still get an explicit protocol reply.
                    let _ = reject.send(reason);
                    tokio::select! { _ = &mut writer => {}, () = stopping(&mut stop) => {}, () = &mut idle => {} }
                    false
                }
                ReadEnd::Eof { done } if done || ending.load(Ordering::Acquire) => {
                    // A half-close after Done can precede our final delivery ack.
                    tokio::select! { result = &mut writer => result, () = stopping(&mut stop) => false, () = &mut idle => false }
                }
                ReadEnd::Eof { .. } | ReadEnd::Failed => false,
            },
            () = stopping(&mut stop) => false,
            () = &mut idle => false,
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
    admission: watch::Receiver<usize>,
    jobs: Sender<Job>,
    ending: Arc<AtomicBool>,
    mut rejected: oneshot::Receiver<Refusal>,
    progress: watch::Sender<Instant>,
) -> bool {
    loop {
        let command = tokio::select! {
            biased;
            reason = &mut rejected => match reason {
                Ok(reason) => Command::Refuse(reason),
                Err(_) => break,
            },
            command = commands.recv() => match command {
                Some(command) => command,
                _ => break,
            },
        };
        sender.set_send_limit(*admission.borrow());
        match command {
            Command::Refuse(reason) => {
                if matches!(sender.send(&SyncMessage::Refused(reason)).await, Ok(())) {
                    let _ = sender.finish_acknowledged().await;
                }
                // This exchange failed even if its refusal reached the peer.
                return false;
            }
            Command::Send(frame) => {
                if matches!(frame, SyncMessage::Done | SyncMessage::Refused(_)) {
                    ending.store(true, Ordering::Release);
                }
                if !matches!(sender.send(&frame).await, Ok(())) {
                    let _ = sender.abort();
                    return false;
                }
                progress.send_replace(Instant::now());
                if jobs.send(Job::Peer(PeerInput::Writable(exchange))).is_err() {
                    return false;
                }
            }
            Command::Finish => {
                ending.store(true, Ordering::Release);
                let done = matches!(sender.finish_acknowledged().await, Ok(()));
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
    admission: watch::Receiver<usize>,
    jobs: Sender<Job>,
    progress: watch::Sender<Instant>,
) -> ReadEnd {
    let mut done = false;
    loop {
        receiver.set_receive_limit(*admission.borrow());
        let frame = match receiver.recv().await {
            Ok(Some(frame)) => frame,
            Ok(None) => return ReadEnd::Eof { done },
            Err(FrameError::Protocol(reason)) => return ReadEnd::Refuse(reason),
            Err(FrameError::TooLarge { .. }) => return ReadEnd::Refuse(Refusal::LimitExceeded),
            _ => return ReadEnd::Failed,
        };
        progress.send_replace(Instant::now());
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
        if applied.await.is_err() {
            return ReadEnd::Failed;
        }
    }
}

async fn idle_deadline(mut progress: watch::Receiver<Instant>, idle: Duration) {
    loop {
        let deadline = *progress.borrow_and_update() + idle;
        tokio::select! {
            () = tokio::time::sleep_until(deadline) => return,
            changed = progress.changed() => if changed.is_err() { return; },
        }
    }
}

async fn admission_guard(
    connection: PeerConnection,
    mut admitted: watch::Receiver<bool>,
    permit: OwnedSemaphorePermit,
    deadline: Instant,
) {
    loop {
        if *admitted.borrow_and_update() {
            drop(permit);
            return;
        }
        tokio::select! {
            _ = connection.closed() => return,
            () = tokio::time::sleep_until(deadline) => { connection.close(); return; },
            changed = admitted.changed() => if changed.is_err() { connection.close(); return; },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn either_direction_progress_keeps_an_exchange_alive_until_both_stall() {
        let (progress, activity) = watch::channel(Instant::now());
        let timer = idle_deadline(activity, Duration::from_millis(100));
        tokio::pin!(timer);
        for _ in 0..8 {
            assert!(
                tokio::time::timeout(Duration::from_millis(30), &mut timer)
                    .await
                    .is_err()
            );
            // A completed send or receive uses this same signal; no idle
            // writer waiting for commands can independently end the reader.
            progress.send_replace(Instant::now());
        }
        tokio::time::timeout(Duration::from_secs(2), &mut timer)
            .await
            .unwrap();
    }

    // These exercise the production exchange reader/writer over real loopback
    // Iroh streams. The fake engine only acknowledges frames and requests a
    // clean finish; this is not a large-object replication test.
    #[tokio::test]
    #[ignore = "real Iroh one-way traffic runs beyond the production 30-second idle interval"]
    async fn send_only_completed_frames_survive_production_idle_interval() {
        one_way_exchange_past_idle(true).await;
    }

    #[tokio::test]
    #[ignore = "real Iroh one-way traffic runs beyond the production 30-second idle interval"]
    async fn receive_only_completed_frames_survive_production_idle_interval() {
        one_way_exchange_past_idle(false).await;
    }

    fn idle_test_chunk(index: usize, count: usize) -> SyncMessage {
        SyncMessage::BlobChunk {
            hash: locust_proto::id::BlobHash([73; 32]),
            offset: index as u64 * 1024,
            total: count as u64 * 1024,
            bytes: vec![index as u8; 1024],
        }
    }

    async fn one_way_exchange_past_idle(sending: bool) {
        use crate::daemon::durable_tests::local_endpoint;
        let seed = if sending { 151 } else { 153 };
        let a = local_endpoint([seed; 32]).await.unwrap();
        let b = local_endpoint([seed + 1; 32]).await.unwrap();
        let hints = b.hints();
        let (outgoing, incoming) = tokio::join!(a.connect(b.id(), &hints), async {
            b.accept().await.unwrap().accept().await
        });
        let outgoing = outgoing.unwrap();
        let incoming = incoming.unwrap();
        let interval = Duration::from_secs(2);
        let count = (IO_IDLE.as_secs() / interval.as_secs()) as usize + 3;
        let span = interval * (count as u32 - 1);
        assert!(span > IO_IDLE);
        let exchange = ExchangeId::Dialed(71);
        let (commands, command_rx) = mpsc::unbounded_channel();
        let (jobs, job_rx) = std::sync::mpsc::channel();
        let engine_commands = commands.clone();
        let engine = std::thread::spawn(move || {
            let mut frames = Vec::new();
            let mut writable = 0;
            while let Ok(job) = job_rx.recv() {
                match job {
                    Job::PeerFrame {
                        input:
                            PeerInput::Frame {
                                exchange: id,
                                frame,
                            },
                        processed,
                    } => {
                        assert_eq!(id, exchange);
                        let done = frame == SyncMessage::Done;
                        frames.push(frame);
                        processed.send(()).unwrap();
                        if done {
                            assert!(!sending);
                            engine_commands.send(Command::Finish).unwrap();
                        }
                    }
                    Job::Peer(PeerInput::Writable(id)) => {
                        assert_eq!(id, exchange);
                        writable += 1;
                    }
                    Job::Peer(PeerInput::Finished(id)) => {
                        assert_eq!(id, exchange);
                        return (frames, writable);
                    }
                    Job::Peer(PeerInput::Closed(_)) => {
                        panic!("active one-way exchange closed before its clean finish")
                    }
                    _ => panic!("unexpected fake-engine job"),
                }
            }
            panic!("exchange ended without a terminal engine notification");
        });
        let (events, mut event_rx) = mpsc::unbounded_channel();
        let (_admission, limits) = watch::channel(MAX_PEER_FRAME_BYTES);
        let (_stop, stop) = watch::channel(false);
        let (finished, keep_remote_open) = oneshot::channel();
        let started = Instant::now();
        let transfer = async {
            let remote = async {
                if sending {
                    let mut link = incoming.accept_link(FrameLimits::peer()).await.unwrap();
                    for index in 0..count {
                        assert_eq!(
                            link.recv().await.unwrap(),
                            Some(idle_test_chunk(index, count)),
                            "lost or reordered sent chunk {index}"
                        );
                    }
                    assert!(started.elapsed() >= span);
                    assert_eq!(link.recv().await.unwrap(), Some(SyncMessage::Done));
                    assert_eq!(link.recv().await.unwrap(), None);
                    keep_remote_open.await.unwrap();
                } else {
                    let mut link = incoming.open_link(FrameLimits::peer()).await.unwrap();
                    for index in 0..count {
                        if index > 0 {
                            tokio::time::sleep(interval).await;
                        }
                        link.send(&idle_test_chunk(index, count)).await.unwrap();
                    }
                    assert!(started.elapsed() >= span);
                    link.send(&SyncMessage::Done).await.unwrap();
                    link.finish_acknowledged().await.unwrap();
                    // The production writer sent no application frames while
                    // its reader kept receiving, then cleanly half-closed.
                    assert_eq!(link.recv().await.unwrap(), None);
                    keep_remote_open.await.unwrap();
                }
            };
            let local = async {
                let link = if sending {
                    outgoing.open_link(FrameLimits::peer()).await.unwrap()
                } else {
                    outgoing.accept_link(FrameLimits::peer()).await.unwrap()
                };
                let feeder = async {
                    if sending {
                        for index in 0..count {
                            if index > 0 {
                                tokio::time::sleep(interval).await;
                            }
                            commands
                                .send(Command::Send(idle_test_chunk(index, count)))
                                .unwrap();
                        }
                        commands.send(Command::Send(SyncMessage::Done)).unwrap();
                        commands.send(Command::Finish).unwrap();
                    }
                };
                let exchange_run = exchange_task(
                    link,
                    outgoing.clone(),
                    exchange,
                    command_rx,
                    limits,
                    jobs,
                    events,
                    stop,
                );
                tokio::join!(feeder, exchange_run);
                finished.send(()).unwrap();
            };
            tokio::join!(remote, local);
        };
        // Test-harness watchdog only. Production exchanges have no total
        // duration limit while either direction completes frames.
        tokio::time::timeout(IO_IDLE * 3, transfer)
            .await
            .expect("loopback exchange test stalled");
        assert!(started.elapsed() >= span);
        assert!(matches!(event_rx.recv().await, Some(Event::Ended(id)) if id == exchange));
        let (frames, writable) = engine.join().unwrap();
        if sending {
            assert!(
                frames.is_empty(),
                "send-only exchange received application data"
            );
            assert_eq!(writable, count + 1);
        } else {
            let expected: Vec<_> = (0..count)
                .map(|index| idle_test_chunk(index, count))
                .chain([SyncMessage::Done])
                .collect();
            assert_eq!(frames, expected);
            assert_eq!(writable, 0);
        }
        a.close().await;
        b.close().await;
    }

    #[tokio::test]
    async fn unauthenticated_connection_deadline_releases_its_resource_permit() {
        use crate::daemon::durable_tests::local_endpoint;
        let a = local_endpoint([141; 32]).await.unwrap();
        let b = local_endpoint([142; 32]).await.unwrap();
        let hints = b.hints();
        let (outgoing, incoming) = tokio::join!(a.connect(b.id(), &hints), async {
            b.accept().await.unwrap().accept().await
        });
        let outgoing = outgoing.unwrap();
        let incoming = incoming.unwrap();
        let permits = Arc::new(Semaphore::new(1));
        let permit = permits.clone().try_acquire_owned().unwrap();
        assert!(permits.clone().try_acquire_owned().is_err());
        let (_admission, admitted) = watch::channel(false);
        admission_guard(
            incoming,
            admitted,
            permit,
            Instant::now() + Duration::from_millis(100),
        )
        .await;
        tokio::time::timeout(Duration::from_secs(2), outgoing.closed())
            .await
            .unwrap();
        assert_eq!(permits.available_permits(), 1);
        assert!(
            admission_slots() * TransportBudget::default().connection_receive_bytes as usize
                <= UNADMITTED_RECEIVE_BYTES
        );
        a.close().await;
        b.close().await;
    }
}
