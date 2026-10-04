//! A deterministic network of drivers: carries `PeerOutput` between them the
//! way the shell and transport would, with read limits, cut-offs and
//! offline nodes.

use std::collections::{HashMap, VecDeque};

use locust_proto::codec;
use locust_proto::engine::{ExchangeId, PeerInput, PeerOutput};
use locust_proto::limits::{MAX_HELLO_FRAME_BYTES, MAX_PEER_FRAME_BYTES};
use locust_proto::sync::SyncMessage;

use super::host::TestHost;
use crate::sync::Driver;

pub struct Node {
    pub host: TestHost,
    pub driver: Driver,
    pub online: bool,
}

/// One side of a stream: a node and its number for the exchange.
type Side = (usize, ExchangeId);

struct Stream {
    sides: [Side; 2],
    /// The acceptor's frames are read at the peer limit.
    admitted: bool,
    closed: bool,
    delivered: usize,
    finished: [bool; 2],
}

enum Msg {
    Input(usize, PeerInput),
    /// An encoded frame for one side of a stream.
    Frame(usize, u8, Vec<u8>),
    /// Everything a side sent before finishing was delivered.
    Close(usize, usize),
}

#[derive(Default)]
pub struct Net {
    pub nodes: Vec<Node>,
    pub now_ms: u64,
    /// Drop every stream after this many delivered frames.
    pub cut: Option<usize>,
    /// Every frame delivered, by receiving node.
    pub log: Vec<(usize, SyncMessage)>,
    /// Frames the reader refused for exceeding its limit.
    pub oversized: usize,
    streams: Vec<Stream>,
    by_side: HashMap<Side, usize>,
    queue: VecDeque<Msg>,
    next_accepted: u64,
}

impl Net {
    pub fn new(hosts: Vec<TestHost>) -> Self {
        Self {
            nodes: hosts
                .into_iter()
                .map(|host| Node {
                    host,
                    driver: Driver::new(),
                    online: true,
                })
                .collect(),
            now_ms: 1_000_000,
            ..Self::default()
        }
    }

    pub fn host(&mut self, node: usize) -> &mut TestHost {
        &mut self.nodes[node].host
    }

    /// Restarts a node: its streams close and it gets a new driver, as
    /// after a daemon restart.
    pub fn restart(&mut self, node: usize) {
        let online = self.nodes[node].online;
        self.set_online(node, false);
        self.settle();
        self.nodes[node].driver = Driver::new();
        self.nodes[node].online = online;
    }

    /// Takes a node off the network, closing its streams.
    pub fn set_online(&mut self, node: usize, online: bool) {
        self.nodes[node].online = online;
        if !online {
            for stream in 0..self.streams.len() {
                if self.streams[stream].sides.iter().any(|side| side.0 == node) {
                    self.close(stream);
                }
            }
        }
    }

    /// Polls nodes in the given order, then delivers until nothing moves.
    pub fn poll(&mut self, order: &[usize]) {
        for &node in order {
            self.input(node, PeerInput::Poll);
        }
        self.settle();
    }

    /// Polls nodes in the given order and delivers nothing yet.
    pub fn poll_without_settling(&mut self, order: &[usize]) {
        for &node in order {
            self.input(node, PeerInput::Poll);
        }
    }

    /// Advances the clock and polls every node.
    pub fn tick(&mut self, ms: u64) {
        self.now_ms += ms;
        let order: Vec<usize> = (0..self.nodes.len()).collect();
        self.poll(&order);
    }

    /// Ticks a second at a time until no replica changed for longer than
    /// an anti-entropy period.
    pub fn quiesce(&mut self) {
        let mut quiet_since = self.now_ms;
        let mut last = self.revisions();
        for _ in 0..2_000 {
            self.tick(1_000);
            let now = self.revisions();
            if now != last {
                last = now;
                quiet_since = self.now_ms;
            } else if self.now_ms - quiet_since > crate::sync::ANTI_ENTROPY_MS + 2_000 {
                return;
            }
        }
        panic!("the network never went quiet");
    }

    fn revisions(&self) -> Vec<u64> {
        self.nodes
            .iter()
            .flat_map(|node| node.host.replicas.values().map(|replica| replica.revision))
            .collect()
    }

    pub fn settle(&mut self) {
        for _ in 0..1_000_000 {
            let Some(msg) = self.queue.pop_front() else {
                return;
            };
            self.deliver(msg);
        }
        panic!("delivery never settled");
    }

    fn input(&mut self, node: usize, input: PeerInput) {
        if !self.nodes[node].online {
            return;
        }
        let mut out = Vec::new();
        let Node { host, driver, .. } = &mut self.nodes[node];
        driver.handle(host, input, self.now_ms, &mut out);
        for output in out {
            self.output(node, output);
        }
    }

    fn output(&mut self, node: usize, output: PeerOutput) {
        match output {
            PeerOutput::Open {
                exchange, endpoint, ..
            } => {
                let target = self
                    .nodes
                    .iter()
                    .position(|other| other.online && other.host.endpoint == endpoint);
                let Some(target) = target else {
                    self.queue
                        .push_back(Msg::Input(node, PeerInput::OpenFailed(exchange)));
                    return;
                };
                let accepted = ExchangeId::Accepted(self.next_accepted);
                self.next_accepted += 1;
                let stream = self.streams.len();
                self.streams.push(Stream {
                    sides: [(node, exchange), (target, accepted)],
                    admitted: false,
                    closed: false,
                    delivered: 0,
                    finished: [false; 2],
                });
                self.by_side.insert((node, exchange), stream);
                self.by_side.insert((target, accepted), stream);
                let remote = self.nodes[node].host.endpoint;
                self.queue.push_back(Msg::Input(
                    target,
                    PeerInput::Accepted {
                        exchange: accepted,
                        remote,
                    },
                ));
                self.queue
                    .push_back(Msg::Input(node, PeerInput::Opened(exchange)));
            }
            PeerOutput::Send { exchange, frame } => {
                let stream = self.by_side[&(node, exchange)];
                let to = if self.streams[stream].sides[0].0 == node {
                    1
                } else {
                    0
                };
                let bytes = codec::encode(&frame).unwrap();
                self.queue.push_back(Msg::Frame(stream, to, bytes));
            }
            PeerOutput::Admit(exchange) => {
                let stream = self.by_side[&(node, exchange)];
                assert_eq!(self.streams[stream].sides[1], (node, exchange));
                self.streams[stream].admitted = true;
            }
            PeerOutput::Finish(exchange) => {
                if let Some(stream) = self.by_side.get(&(node, exchange)) {
                    let side = usize::from(self.streams[*stream].sides[1].0 == node);
                    self.queue.push_back(Msg::Close(*stream, side));
                }
            }
        }
    }

    fn deliver(&mut self, msg: Msg) {
        match msg {
            Msg::Input(node, input) => self.input(node, input),
            Msg::Close(stream, side) => {
                if !self.streams[stream].closed {
                    let (node, exchange) = self.streams[stream].sides[side];
                    self.input(node, PeerInput::Finished(exchange));
                    self.streams[stream].finished[side] = true;
                    if self.streams[stream].finished == [true; 2] {
                        self.close(stream);
                    }
                }
            }
            Msg::Frame(stream, to, bytes) => {
                let state = &mut self.streams[stream];
                if state.closed {
                    return;
                }
                if self.cut.is_some_and(|cut| state.delivered >= cut) {
                    self.close(stream);
                    return;
                }
                let limit = if to == 1 && !state.admitted {
                    MAX_HELLO_FRAME_BYTES
                } else {
                    MAX_PEER_FRAME_BYTES
                };
                if bytes.len() > limit {
                    self.oversized += 1;
                    self.close(stream);
                    return;
                }
                state.delivered += 1;
                let (sender, sending) = state.sides[1 - usize::from(to)];
                let (node, exchange) = state.sides[usize::from(to)];
                let frame = SyncMessage::decode(&bytes).expect("every frame sent decodes");
                self.log.push((node, frame.clone()));
                self.input(node, PeerInput::Frame { exchange, frame });
                self.queue
                    .push_back(Msg::Input(sender, PeerInput::Writable(sending)));
            }
        }
    }

    fn close(&mut self, stream: usize) {
        let state = &mut self.streams[stream];
        if state.closed {
            return;
        }
        state.closed = true;
        for (node, exchange) in state.sides {
            self.queue
                .push_back(Msg::Input(node, PeerInput::Closed(exchange)));
        }
    }
}
