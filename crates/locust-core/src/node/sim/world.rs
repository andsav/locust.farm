//! The simulated world: machines, one clock, one queue of future inputs.
//!
//! Simulated true time is counted in microseconds from the start of the
//! run. Every input a shell would hand its engine is an entry in one queue,
//! keyed by the time it happens; the world always takes the earliest. The
//! times come from the seeded generator, so the seed fixes the whole order.

use std::collections::BTreeMap;

use locust_proto::api::{ApiError, Request, Response};
use locust_proto::engine::{Engine, ExchangeId, PeerEngine, PeerInput, PeerOutput};

use super::chaos::{Chaos, Fault};
use super::machine::{Machine, Power, Who};
use super::net::Net;
use super::rng::Rng;
use super::{EPOCH_MS, POLL_MS};

/// Simulated true time in microseconds since the run began.
pub type Micros = u64;

pub const MS: Micros = 1_000;
pub const SEC: Micros = 1_000_000;

/// One future input or simulator action.
#[derive(Debug)]
pub(super) enum Ev {
    /// The shell's one-second poll of machine `m`, started at `boot`.
    Poll { m: usize, boot: u32 },
    /// An input the machine's own shell produces: `Opened`, `OpenFailed`,
    /// `Writable`, `Finished`, `Closed` or `Connection`.
    Local {
        m: usize,
        boot: u32,
        input: PeerInput,
    },
    /// A connection attempt for a dialed exchange is tried again. It
    /// succeeds as soon as the other machine can be reached and fails at
    /// `deadline`.
    Connect {
        m: usize,
        boot: u32,
        exchange: ExchangeId,
        to: Option<usize>,
        deadline: Micros,
    },
    /// The next thing side `1 - to` of a stream sent reaches side `to`.
    Arrive { stream: usize, to: usize },
    /// The acknowledgement of side `to`'s finished stream reaches it.
    FinAck { stream: usize, to: usize },
    /// The other side's abort of a stream reaches side `to`.
    Reset { stream: usize, to: usize },
    /// A graceful close of a connection reaches its end `to`.
    ConnClose { conn: usize, to: usize },
    /// A woken machine notices that its old connections are dead.
    WakeClose { m: usize, boot: u32, before: usize },
    /// The fault injector's next decision.
    Chaos,
    /// A fault ends.
    Heal(Fault),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub events: u64,
    pub polls: u64,
    pub frames: u64,
    pub opened: u64,
    pub open_failed: u64,
    pub finished: u64,
    pub closed: u64,
    pub oversized: u64,
    pub requests: u64,
    pub request_errors: u64,
    pub faults: u64,
}

pub struct World {
    pub now: Micros,
    pub machines: Vec<Machine>,
    pub(super) net: Net,
    queue: BTreeMap<(Micros, u64), Ev>,
    seq: u64,
    /// Inputs for machines that are asleep, delivered when they wake.
    pub(super) deferred: Vec<Ev>,
    /// Network timing.
    pub(super) rng: Rng,
    pub(super) chaos: Chaos,
    /// A running hash of everything delivered, to compare two runs of a seed.
    pub digest: u64,
    /// Human-readable lines of the same, when asked for.
    pub log: Option<Vec<String>>,
    pub stats: Stats,
}

impl World {
    pub fn new(seed: u64, machines: usize) -> Self {
        let root = Rng::new(seed);
        let mut rng = root.fork(1);
        let net = Net::new(machines, &mut rng);
        Self {
            now: 0,
            machines: (0..machines).map(|i| Machine::new(i, &root)).collect(),
            net,
            queue: BTreeMap::new(),
            seq: 0,
            deferred: Vec::new(),
            rng,
            chaos: Chaos::new(root.fork(2)),
            digest: seed,
            log: None,
            stats: Stats::default(),
        }
    }

    /// The wall clock of machine `m` in Unix milliseconds, as its shell
    /// passes it into every engine call.
    pub fn wall_ms(&self, m: usize) -> u64 {
        let true_ms = (EPOCH_MS + self.now / MS) as i64;
        (true_ms + self.machines[m].clock_offset_ms) as u64
    }

    pub(super) fn schedule(&mut self, at: Micros, ev: Ev) {
        self.seq += 1;
        self.queue.insert((at.max(self.now), self.seq), ev);
    }

    pub(super) fn note(&mut self, m: usize, tag: u64, line: impl FnOnce() -> String) {
        let mut z = self.digest ^ self.now.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (m as u64).wrapping_add(tag << 8)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        self.digest = z ^ (z >> 29);
        if self.log.is_some() {
            let text = format!(
                "[{:>10.3}s {}] {}",
                self.now as f64 / SEC as f64,
                self.machines.get(m).map_or("--", |m| m.name.as_str()),
                line()
            );
            if let Some(log) = self.log.as_mut() {
                log.push(text);
            }
        }
    }

    /// Starts the daemon process of machine `m` and its shell: the endpoint
    /// announcement and the poll timer.
    pub fn start(&mut self, m: usize) {
        let now_ms = self.wall_ms(m);
        self.machines[m].start(now_ms);
        let boot = self.machines[m].boot;
        self.note(m, 1, || format!("process started (boot {boot})"));
        // A restart shows local callers exactly what they could read when
        // the process ended, however it ended.
        if let Some(before) = self.machines[m].last_view.take() {
            let after = self.machines[m].visible(now_ms);
            assert!(
                after.as_ref() == Some(&before),
                "m{} shows something else after a restart:\nbefore {before}\nafter  {after:?}",
                m + 1
            );
        }
        let endpoint = self.machines[m].endpoint;
        self.feed(
            m,
            PeerInput::Endpoint {
                endpoint,
                hints: Vec::new(),
            },
        );
        let first = self.now + self.rng.range(1, POLL_MS) * MS;
        self.schedule_poll(m, first);
    }

    /// Sets the one pending poll of machine `m`; an earlier one still queued
    /// is ignored when its time comes.
    pub(super) fn schedule_poll(&mut self, m: usize, at: Micros) {
        let boot = self.machines[m].boot;
        self.machines[m].next_poll = at;
        self.schedule(at, Ev::Poll { m, boot });
    }

    /// Hands one input to the engine of machine `m`, as its shell would, and
    /// carries out what the engine asks for, in order.
    pub(super) fn feed(&mut self, m: usize, input: PeerInput) {
        let now_ms = self.wall_ms(m);
        let tag = super::trace::input_tag(&input);
        self.note(m, tag, || format!("<- {}", super::trace::brief(&input)));
        let mut out = Vec::new();
        let node = self.machines[m].node.as_mut().expect("a running machine");
        node.peer(input, now_ms, &mut out);
        Engine::take_changed(node);
        for output in out {
            match output {
                PeerOutput::Open {
                    exchange, endpoint, ..
                } => self.open(m, exchange, endpoint),
                PeerOutput::Send { exchange, frame } => self.send(m, exchange, frame),
                PeerOutput::Admit(exchange) => self.admit(m, exchange),
                PeerOutput::Finish(exchange) => self.finish(m, exchange),
                PeerOutput::Evidence(exchange) => self.evidence(m, exchange),
            }
        }
        self.refresh_readable(m);
    }

    /// One local request on machine `m`, as one CLI command. After it the
    /// shell polls the peer engine, as the daemon's worker does. Checks that
    /// a request answered with an error left no event behind.
    pub fn call(&mut self, m: usize, who: Who, request: Request) -> Result<Response, ApiError> {
        let name = request.name();
        let before = self.machines[m].event_count();
        let now_ms = self.wall_ms(m);
        let result = self.machines[m].call(who, request, now_ms);
        self.stats.requests += 1;
        if let Err(error) = &result {
            self.stats.request_errors += 1;
            let after = self.machines[m].event_count();
            assert_eq!(
                before, after,
                "{name} answered {error} and still left an event behind"
            );
        }
        let ok = result.is_ok();
        self.note(m, 2 + u64::from(ok), || match &result {
            Ok(_) => format!("{who:?} {name}: ok"),
            Err(error) => format!("{who:?} {name}: {error}"),
        });
        if self.machines[m].running() {
            self.feed(m, PeerInput::Poll);
        }
        result
    }

    /// Takes the earliest queued entry and carries it out. False when the
    /// queue is empty, which only happens when no machine runs.
    pub fn step(&mut self) -> bool {
        let Some((&key, _)) = self.queue.iter().next() else {
            return false;
        };
        let ev = self.queue.remove(&key).expect("the key was just read");
        self.now = key.0;
        self.stats.events += 1;
        match ev {
            Ev::Poll { m, boot } => {
                let machine = &self.machines[m];
                if machine.boot != boot
                    || machine.power != Power::Running
                    || machine.next_poll != self.now
                {
                    return true;
                }
                self.stats.polls += 1;
                self.shell_timers(m);
                self.feed(m, PeerInput::Poll);
                self.schedule_poll(m, self.now + POLL_MS * MS);
            }
            Ev::Local { m, boot, input } => {
                let machine = &self.machines[m];
                if machine.boot != boot || machine.power == Power::Stopped {
                    return true;
                }
                if machine.power == Power::Asleep {
                    self.deferred.push(Ev::Local { m, boot, input });
                    return true;
                }
                match &input {
                    PeerInput::Opened(_) => self.stats.opened += 1,
                    PeerInput::OpenFailed(_) => self.stats.open_failed += 1,
                    PeerInput::Finished(_) => self.stats.finished += 1,
                    PeerInput::Closed(_) => self.stats.closed += 1,
                    _ => {}
                }
                self.feed(m, input);
            }
            Ev::Connect {
                m,
                boot,
                exchange,
                to,
                deadline,
            } => self.connect(m, boot, exchange, to, deadline),
            Ev::Arrive { stream, to } => self.arrive(stream, to),
            Ev::FinAck { stream, to } => self.fin_ack(stream, to),
            Ev::Reset { stream, to } => self.reset(stream, to),
            Ev::ConnClose { conn, to } => self.conn_close(conn, to),
            Ev::WakeClose { m, boot, before } => self.wake_close(m, boot, before),
            Ev::Chaos => self.chaos_tick(),
            Ev::Heal(fault) => self.heal(fault),
        }
        true
    }

    /// Runs the world until simulated time `until`.
    pub fn run_until(&mut self, until: Micros) {
        while let Some((&(at, _), _)) = self.queue.iter().next() {
            if at > until {
                break;
            }
            self.step();
        }
        self.now = self.now.max(until);
    }

    pub fn run_for(&mut self, duration: Micros) {
        self.run_until(self.now + duration);
    }
}
