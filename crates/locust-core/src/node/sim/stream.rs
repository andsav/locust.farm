//! What the shell's outputs do to streams, and how what was sent arrives.

use std::collections::VecDeque;

use locust_proto::codec;
use locust_proto::engine::{ExchangeId, PeerEngine, PeerInput};
use locust_proto::id::EndpointId;
use locust_proto::limits::{MAX_HELLO_FRAME_BYTES, MAX_PEER_FRAME_BYTES};
use locust_proto::sync::{Refusal, SyncMessage};

use super::machine::{Machine, Power};
use super::net::{Item, Path, Side, Stream};
use super::world::{Ev, MS, World};
use super::{EVIDENCE_FRAME_BYTES, IO_IDLE_MS};

impl World {
    fn side(&mut self, stream: usize, side: usize) -> &mut Side {
        &mut self.net.streams[stream].sides[side]
    }

    /// Queues a shell-local input of one side of a stream, after every
    /// earlier one of that side.
    fn local(&mut self, stream: usize, side: usize, input: PeerInput) {
        let delay = self.local_delay();
        let now = self.now;
        let s = self.side(stream, side);
        let at = (now + delay).max(s.local_at + 1);
        s.local_at = at;
        let (m, boot) = (s.m, s.boot);
        self.schedule(at, Ev::Local { m, boot, input });
    }

    fn schedule_arrival(&mut self, stream: usize, to: usize) {
        let from = 1 - to;
        let (fm, tm) = {
            let sides = &self.net.streams[stream].sides;
            (sides[from].m, sides[to].m)
        };
        let delay = self.latency(fm, tm);
        let now = self.now;
        let s = self.side(stream, from);
        let at = (now + delay).max(s.arrive_at + 1);
        s.arrive_at = at;
        self.schedule(at, Ev::Arrive { stream, to });
    }

    fn lookup(&self, m: usize, exchange: ExchangeId) -> Option<(usize, usize)> {
        let key = (m, self.machines[m].boot, exchange);
        let (stream, side) = *self.net.by_exchange.get(&key)?;
        (!self.net.streams[stream].sides[side].ended).then_some((stream, side))
    }

    /// `PeerOutput::Open`: over the first connection the shell believes
    /// alive, else by connecting first.
    pub(super) fn open(&mut self, m: usize, exchange: ExchangeId, endpoint: EndpointId) {
        let to = self
            .machines
            .iter()
            .position(|other| other.boot > 0 && other.endpoint == endpoint)
            .filter(|to| *to != m);
        if let Some(conn) = to.and_then(|to| self.believed_conn(m, to)) {
            self.new_stream(conn, m, exchange);
            return;
        }
        let delay = match to {
            Some(to) => self.latency(m, to) + self.latency(to, m),
            None => self.rng.range(1, 2_000) * MS,
        } + self.local_delay();
        // The attempt gives up quickly when something answers that nobody
        // listens, and at the shell's connect deadline when nothing answers.
        let patience = if self.rng.chance(1, 3) {
            self.rng.range(50, 2_000)
        } else {
            self.rng.range(2_000, IO_IDLE_MS)
        };
        let boot = self.machines[m].boot;
        let ev = Ev::Connect {
            m,
            boot,
            exchange,
            to,
            deadline: self.now + delay + patience * MS,
        };
        self.schedule(self.now + delay, ev);
    }

    pub(super) fn new_stream(&mut self, conn: usize, m: usize, exchange: ExchangeId) {
        let other = self.net.conns[conn].other(m);
        let now = self.now;
        let side = |m: usize, boot: u32, exchange: Option<ExchangeId>| Side {
            m,
            boot,
            exchange,
            out: VecDeque::new(),
            arrive_at: now,
            local_at: now,
            ended: false,
            limit: match exchange {
                Some(_) => MAX_PEER_FRAME_BYTES,
                None => MAX_HELLO_FRAME_BYTES,
            },
            read_blocked: false,
            finishing: None,
            got_fin: false,
            got_done: false,
            sent_ending: false,
            last_recv: now,
            last_send: now,
        };
        let storm = self.net.cut_storm > 0 && self.rng.chance(1, 2);
        let cut = storm || self.rng.chance(self.net.cut_rate, 64);
        let cut_after = cut.then(|| self.rng.below(12));
        let boot = self.machines[m].boot;
        let stream = self.net.streams.len();
        self.net.streams.push(Stream {
            conn,
            sides: [
                side(m, boot, Some(exchange)),
                side(other, self.machines[other].boot, None),
            ],
            cut_after,
            delivered: 0,
        });
        self.net.live.insert(stream);
        self.net
            .by_exchange
            .insert((m, boot, exchange), (stream, 0));
        self.local(stream, 0, PeerInput::Opened(exchange));
    }

    /// `PeerOutput::Send`. Sending on an exchange the shell no longer has
    /// answers `Closed`, as the daemon's shell does.
    pub(super) fn send(&mut self, m: usize, exchange: ExchangeId, frame: SyncMessage) {
        let Some((stream, side)) = self.lookup(m, exchange) else {
            return self.stray(m, exchange);
        };
        let bytes = codec::encode(&frame).expect("every frame the engine sends encodes");
        let ending = matches!(frame, SyncMessage::Done | SyncMessage::Refused(_));
        let now = self.now;
        let s = self.side(stream, side);
        if bytes.len() > s.limit {
            // The writer refuses a frame over its limit and gives up.
            self.stats.oversized += 1;
            return self.abort_side(stream, side);
        }
        s.out.push_back(Item::Frame(bytes));
        s.last_send = now;
        s.sent_ending |= ending;
        self.schedule_arrival(stream, 1 - side);
        self.local(stream, side, PeerInput::Writable(exchange));
    }

    /// `PeerOutput::Admit`: the exchange reads and writes at the peer frame
    /// limit, and its connection is kept past the admission deadline.
    pub(super) fn admit(&mut self, m: usize, exchange: ExchangeId) {
        if let Some((stream, side)) = self.lookup(m, exchange) {
            self.side(stream, side).limit = MAX_PEER_FRAME_BYTES;
            let conn = self.net.streams[stream].conn;
            let end = self.net.conns[conn].end_of(m);
            self.net.conns[conn].admitted[end] = true;
        }
    }

    /// `PeerOutput::Evidence`: room for one fork proof, and nothing else.
    pub(super) fn evidence(&mut self, m: usize, exchange: ExchangeId) {
        if let Some((stream, side)) = self.lookup(m, exchange) {
            let s = self.side(stream, side);
            s.limit = s.limit.max(EVIDENCE_FRAME_BYTES);
        }
    }

    /// `PeerOutput::Finish`: the end of this side's frames goes out, and
    /// `Finished` follows only once the other end acknowledged it.
    pub(super) fn finish(&mut self, m: usize, exchange: ExchangeId) {
        let Some((stream, side)) = self.lookup(m, exchange) else {
            return self.stray(m, exchange);
        };
        let now = self.now;
        let s = self.side(stream, side);
        s.out.push_back(Item::Fin);
        s.finishing = Some(now);
        s.sent_ending = true;
        self.schedule_arrival(stream, 1 - side);
    }

    fn stray(&mut self, m: usize, exchange: ExchangeId) {
        let at = self.now + self.local_delay();
        let boot = self.machines[m].boot;
        let input = PeerInput::Closed(exchange);
        self.schedule(at, Ev::Local { m, boot, input });
    }

    /// Whether what side `from` sent can reach side `to` now.
    pub(super) fn path(&self, stream: usize, from: usize, to: usize) -> Path {
        let st = &self.net.streams[stream];
        let (f, t) = (&st.sides[from], &st.sides[to]);
        let (fm, tm) = (&self.machines[f.m], &self.machines[t.m]);
        let gone = |m: &Machine, s: &Side| m.boot != s.boot || m.power == Power::Stopped;
        if gone(fm, f) || gone(tm, t) || !self.net.conns[st.conn].alive_both() {
            return Path::Dead;
        }
        if fm.power == Power::Asleep || tm.power == Power::Asleep || !self.net.reach(f.m, t.m) {
            return Path::Held;
        }
        Path::Open
    }

    /// The next thing side `1 - to` sent reaches side `to`.
    pub(super) fn arrive(&mut self, stream: usize, to: usize) {
        let from = 1 - to;
        if self.net.streams[stream].sides[from].out.is_empty() {
            return;
        }
        match self.path(stream, from, to) {
            Path::Dead => return self.side(stream, from).out.clear(),
            Path::Held => return self.net.held.push(Ev::Arrive { stream, to }),
            Path::Open => {}
        }
        if self.side(stream, to).read_blocked && !self.side(stream, to).ended {
            return self.net.held.push(Ev::Arrive { stream, to });
        }
        let st = &self.net.streams[stream];
        if st.cut_after.is_some_and(|cut| st.delivered >= cut) {
            return self.cut(stream);
        }
        let item = self.side(stream, from).out.pop_front().expect("not empty");
        let (fm, tm) = (self.side(stream, from).m, self.side(stream, to).m);
        if self.side(stream, to).ended {
            // The reader is gone: the sender's stream is stopped.
            let at = self.now + self.latency(tm, fm);
            return self.schedule(at, Ev::Reset { stream, to: from });
        }
        let now = self.now;
        let exchange = match self.side(stream, to).exchange {
            Some(exchange) => exchange,
            None => {
                self.machines[tm].next_accepted += 1;
                let exchange = ExchangeId::Accepted(self.machines[tm].next_accepted);
                let boot = self.machines[tm].boot;
                self.net
                    .by_exchange
                    .insert((tm, boot, exchange), (stream, to));
                let s = self.side(stream, to);
                s.exchange = Some(exchange);
                s.last_recv = now;
                s.last_send = now;
                let remote = self.machines[fm].endpoint;
                self.feed(tm, PeerInput::Accepted { exchange, remote });
                exchange
            }
        };
        match item {
            Item::Frame(bytes) => {
                let s = self.side(stream, to);
                if bytes.len() > s.limit {
                    // The reader refuses it; its writer says so and gives up.
                    self.stats.oversized += 1;
                    let refusal = SyncMessage::Refused(Refusal::LimitExceeded);
                    let bytes = codec::encode(&refusal).expect("a refusal encodes");
                    self.side(stream, to).out.push_back(Item::Frame(bytes));
                    self.schedule_arrival(stream, from);
                    return self.end_side(stream, to, false);
                }
                let frame = SyncMessage::decode(&bytes).expect("every frame sent decodes");
                s.last_recv = now;
                s.got_done |= matches!(frame, SyncMessage::Done | SyncMessage::Refused(_));
                // The shell reads the next frame only once the engine says
                // it may; `feed` asks after every call.
                s.read_blocked = true;
                self.net.streams[stream].delivered += 1;
                self.stats.frames += 1;
                self.feed(tm, PeerInput::Frame { exchange, frame });
            }
            Item::Fin => {
                let at = self.now + self.latency(tm, fm);
                self.schedule(at, Ev::FinAck { stream, to: from });
                let s = self.side(stream, to);
                s.got_fin = true;
                if !(s.got_done || s.sent_ending) {
                    self.abort_side(stream, to);
                }
            }
        }
    }

    /// The transport's acknowledgement of everything side `to` sent.
    pub(super) fn fin_ack(&mut self, stream: usize, to: usize) {
        match self.path(stream, 1 - to, to) {
            Path::Dead => {}
            Path::Held => self.net.held.push(Ev::FinAck { stream, to }),
            Path::Open => self.end_side(stream, to, true),
        }
    }

    pub(super) fn reset(&mut self, stream: usize, to: usize) {
        match self.path(stream, 1 - to, to) {
            Path::Dead => {}
            Path::Held => self.net.held.push(Ev::Reset { stream, to }),
            Path::Open => self.end_side(stream, to, false),
        }
    }

    /// The shell of one side ends the exchange: `Finished` when the
    /// transport confirmed delivery, `Closed` otherwise.
    pub(super) fn end_side(&mut self, stream: usize, side: usize, finished: bool) {
        let s = self.side(stream, side);
        if s.ended {
            return;
        }
        s.ended = true;
        if !finished {
            s.out.clear();
        }
        if let Some(exchange) = s.exchange {
            let input = if finished {
                PeerInput::Finished(exchange)
            } else {
                PeerInput::Closed(exchange)
            };
            self.local(stream, side, input);
        }
        if self.net.streams[stream].sides.iter().all(|s| s.ended) {
            self.net.live.remove(&stream);
        }
        // The shell closes a connection whose exchange ended before the
        // engine admitted anything on it, unless an exchange it opened is
        // still in flight on it.
        let (m, conn) = (self.side(stream, side).m, self.net.streams[stream].conn);
        let end = self.net.conns[conn].end_of(m);
        if !self.net.conns[conn].admitted[end] && !self.dialing_on(conn, m) {
            self.close_conn(conn, end);
        }
    }

    /// Whether machine `m` has an exchange it opened in flight on `conn`.
    fn dialing_on(&self, conn: usize, m: usize) -> bool {
        let boot = self.machines[m].boot;
        self.net.live.iter().any(|stream| {
            let st = &self.net.streams[*stream];
            let opener = &st.sides[0];
            st.conn == conn && opener.m == m && opener.boot == boot && !opener.ended
        })
    }

    /// After every engine call the shell asks which paused exchanges may be
    /// read again.
    pub(super) fn refresh_readable(&mut self, m: usize) {
        let boot = self.machines[m].boot;
        let mut released = false;
        for stream in self.net.live.iter().copied().collect::<Vec<_>>() {
            for side in 0..2 {
                let s = &self.net.streams[stream].sides[side];
                let (Some(exchange), true) = (s.exchange, s.read_blocked) else {
                    continue;
                };
                if s.m != m || s.boot != boot || s.ended {
                    continue;
                }
                let node = self.machines[m].node.as_ref().expect("a running machine");
                if node.peer_readable(exchange) {
                    self.side(stream, side).read_blocked = false;
                    released = true;
                }
            }
        }
        if released {
            self.release_held();
        }
    }

    /// One side gives up on the exchange; the other learns by a reset.
    pub(super) fn abort_side(&mut self, stream: usize, side: usize) {
        self.end_side(stream, side, false);
        let (m, other) = (self.side(stream, side).m, self.side(stream, 1 - side).m);
        let at = self.now + self.latency(m, other);
        self.schedule(
            at,
            Ev::Reset {
                stream,
                to: 1 - side,
            },
        );
    }

    /// The link under one stream fails: both sides see it close.
    pub(super) fn cut(&mut self, stream: usize) {
        self.end_side(stream, 0, false);
        self.end_side(stream, 1, false);
    }

    /// Queues again everything that waited for a path, in the order it
    /// waited. Called whenever a route or a machine comes back.
    pub(super) fn release_held(&mut self) {
        for ev in std::mem::take(&mut self.net.held) {
            let at = self.now + self.local_delay();
            self.schedule(at, ev);
        }
    }
}
