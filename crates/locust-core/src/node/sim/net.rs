//! The simulated network: exchanges as ordered streams over connections.
//!
//! A stream is what one QUIC stream is to the daemon's shell: each side's
//! frames reach the other in the order sent or not at all, a side learns
//! that its last frame was delivered only from an acknowledgement, and an
//! abort on one side reaches the other as a reset. Different streams are
//! delayed independently. Frames are carried encoded and decoded again
//! under the reader's frame limit, as on the wire.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use locust_proto::engine::ExchangeId;

use super::conn::Conn;
use super::rng::Rng;
use super::world::{Ev, MS, Micros, World};

pub(super) enum Item {
    Frame(Vec<u8>),
    /// The side finished: everything before it was sent.
    Fin,
}

pub(super) struct Side {
    pub m: usize,
    pub boot: u32,
    /// The shell's number for the exchange. The accepting shell numbers it
    /// when the first thing arrives on the stream.
    pub exchange: Option<ExchangeId>,
    /// Sent and not yet arrived at the other side.
    pub out: VecDeque<Item>,
    /// When the latest thing this side sent arrives; later ones arrive later.
    pub arrive_at: Micros,
    /// When the latest local input of this side is delivered.
    pub local_at: Micros,
    /// The shell told the engine how the exchange ended, or is about to.
    pub ended: bool,
    /// The largest frame this side reads or writes: the hello limit on an
    /// accepted exchange until the engine admits it.
    pub limit: usize,
    /// The engine still owes answers, so the shell reads no further frame.
    pub read_blocked: bool,
    /// Set once the engine asked to finish.
    pub finishing: Option<Micros>,
    pub got_fin: bool,
    pub got_done: bool,
    pub sent_ending: bool,
    pub last_recv: Micros,
    /// When the latest frame this side sent was written.
    pub last_send: Micros,
}

pub(super) struct Stream {
    pub conn: usize,
    /// Side 0 dialed, side 1 accepted.
    pub sides: [Side; 2],
    /// The link fails once this many frames were delivered on the stream.
    pub cut_after: Option<u64>,
    pub delivered: u64,
}

/// Whether something sent can reach its receiver now.
pub(super) enum Path {
    Open,
    /// Not now: the receiver sleeps or the route is down. It is kept, as a
    /// reliable transport retransmits, until the route returns or an end
    /// gives up.
    Held,
    /// Never: an end of the connection is gone.
    Dead,
}

pub(super) struct Net {
    /// `blocked[from][to]`: how many faults stop packets from `from` to `to`.
    pub blocked: Vec<Vec<u32>>,
    pub base: Vec<Vec<Micros>>,
    pub conns: Vec<Conn>,
    pub streams: Vec<Stream>,
    /// Streams with a side whose shell has not ended it.
    pub live: BTreeSet<usize>,
    pub by_exchange: BTreeMap<(usize, u32, ExchangeId), (usize, usize)>,
    /// Arrivals waiting for their path to come back.
    pub held: Vec<Ev>,
    /// How often one delivery stalls for up to seconds, per 1024.
    pub stall: u64,
    /// While above zero, new streams are cut after a few frames.
    pub cut_storm: u32,
    /// How many new streams in 64 are cut after a few frames, all run long.
    pub cut_rate: u64,
    /// How many connection attempts in 64 fail although the path is fine.
    pub open_fail_rate: u64,
}

impl Net {
    pub fn new(machines: usize, rng: &mut Rng) -> Self {
        let mut base = vec![vec![0; machines]; machines];
        let pairs = (0..machines).flat_map(|a| (a + 1..machines).map(move |b| (a, b)));
        for (a, b) in pairs {
            // A direct local path, or a relayed one.
            let one_way = if rng.chance(1, 2) {
                rng.range(150, 2_500)
            } else {
                rng.range(15_000, 120_000)
            };
            base[a][b] = one_way;
            base[b][a] = one_way;
        }
        Self {
            blocked: vec![vec![0; machines]; machines],
            base,
            conns: Vec::new(),
            streams: Vec::new(),
            live: BTreeSet::new(),
            by_exchange: BTreeMap::new(),
            held: Vec::new(),
            stall: rng.pick(&[0, 4, 16, 64]),
            cut_storm: 0,
            cut_rate: 0,
            open_fail_rate: 0,
        }
    }

    pub fn reach(&self, from: usize, to: usize) -> bool {
        self.blocked[from][to] == 0
    }
}

impl World {
    pub(super) fn latency(&mut self, a: usize, b: usize) -> Micros {
        let base = self.net.base[a][b];
        let mut delay = base + self.rng.below(base + 1);
        if self.rng.chance(self.net.stall, 1024) {
            delay += self.rng.range(200, 6_000) * MS;
        }
        delay
    }

    pub(super) fn local_delay(&mut self) -> Micros {
        self.rng.range(20, 400)
    }
}
