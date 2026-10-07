//! Fault injection, chosen by the seed: stops and restarts, sleep, routes
//! lost one way or both, exchanges cut mid-way, and a store put back from
//! an older backup ([`super::restore`]).
//!
//! Faults start at seeded moments while the scenario runs, so they land
//! between its steps and during them. Each ends by itself after a seeded
//! time. The scenario can also ask for a fault at an exact point.

use super::machine::Power;
use super::restore::Marks;
use super::rng::Rng;
use super::world::{Ev, MS, Micros, SEC, World};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// The process is not running; healing starts it again.
    Down { m: usize },
    /// The machine sleeps; healing wakes it.
    Asleep { m: usize },
    /// The process is not running and its store was put back from an older
    /// backup, with its marks kept or lost; healing starts it again. Waking
    /// from sleep is never this.
    Restored { m: usize, marks: Marks },
    /// Packets from the first machine of each pair to the second are lost.
    Partition { pairs: Vec<(usize, usize)> },
    /// New exchanges are cut after a few frames.
    CutStorm,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Fault {
    id: u64,
    kind: Kind,
}

pub struct Chaos {
    rng: Rng,
    pub enabled: bool,
    /// Faults still to inject.
    pub budget: u32,
    /// Mean simulated time between two faults.
    pub mean_gap_ms: u64,
    active: Vec<Fault>,
    next_id: u64,
    /// When the latest fault ended.
    pub last_heal: Micros,
    /// Every fault injected, for the report of a failing seed.
    pub history: Vec<String>,
    /// Faults still allowed over the whole run, when a replay limits them
    /// to find the fewest that still fail.
    pub cap: Option<u32>,
    /// True while a next decision is queued.
    chain: bool,
    /// Also step wall clocks while machines run. Off unless asked for.
    pub clock_steps: bool,
    /// Backups and restores are drawn from their own generator, so a run
    /// with them turned off is the run it was before they existed.
    pub backup_rng: Rng,
    /// Whether this run takes backups and restores machines at all.
    pub allow_restores: bool,
    /// Whether injected faults include restores now. The scenario turns this
    /// on where nobody waits for a write to arrive.
    pub restores: bool,
}

impl Chaos {
    pub fn new(rng: Rng) -> Self {
        Self {
            enabled: false,
            budget: 0,
            mean_gap_ms: 20_000,
            active: Vec::new(),
            next_id: 0,
            last_heal: 0,
            history: Vec::new(),
            cap: None,
            chain: false,
            clock_steps: false,
            backup_rng: rng.fork(0xBAC0),
            allow_restores: false,
            restores: false,
            rng,
        }
    }

    /// True while no injected fault is in effect.
    pub fn quiet(&self) -> bool {
        self.active.is_empty()
    }

    /// A new fault in effect.
    pub(super) fn fault(&mut self, kind: Kind) -> Fault {
        self.next_id += 1;
        let fault = Fault {
            id: self.next_id,
            kind,
        };
        self.active.push(fault.clone());
        fault
    }
}

impl World {
    /// Turns fault injection on with `budget` faults, about `mean_gap_ms`
    /// of simulated time apart.
    pub fn enable_chaos(&mut self, budget: u32, mean_gap_ms: u64) {
        self.chaos.enabled = budget > 0;
        self.chaos.budget = budget;
        self.chaos.mean_gap_ms = mean_gap_ms;
        if budget > 0 && !self.chaos.chain {
            self.schedule_chaos();
        }
    }

    fn schedule_chaos(&mut self) {
        let mean = self.chaos.mean_gap_ms;
        let gap = self.chaos.rng.range(mean / 8, mean * 2);
        self.chaos.chain = true;
        self.schedule(self.now + gap * MS, Ev::Chaos);
    }

    pub(super) fn chaos_tick(&mut self) {
        self.chaos.chain = false;
        if !self.chaos.enabled || self.chaos.budget == 0 {
            return;
        }
        if self.chaos.allow_restores {
            let m = self.chaos.backup_rng.below(self.machines.len() as u64) as usize;
            if self.chaos.backup_rng.chance(1, 2) {
                self.backup(m);
            }
        }
        self.inject();
        self.schedule_chaos();
    }

    /// Possibly injects a fault at exactly this point of the scenario.
    pub fn chaos_point(&mut self) {
        if self.chaos.enabled && self.chaos.budget > 0 && self.chaos.rng.chance(1, 5) {
            self.inject();
        }
    }

    fn candidates(&self) -> Vec<usize> {
        (0..self.machines.len())
            .filter(|m| self.machines[*m].running() && !self.machines[*m].held)
            .collect()
    }

    fn inject(&mut self) {
        if self.chaos.cap == Some(0) {
            return;
        }
        if self.chaos.allow_restores && self.chaos.restores && self.chaos.backup_rng.chance(1, 4) {
            let targets = self.restorable();
            if !targets.is_empty() {
                let m = self.chaos.backup_rng.pick(&targets);
                let (kind, heal_ms, text) = self.restore(m);
                return self.begin(kind, heal_ms, text);
            }
        }
        let targets = self.candidates();
        let roll = self.chaos.rng.below(100);
        let (kind, heal_ms, text) = match roll {
            0..30 if !targets.is_empty() => {
                let m = self.chaos.rng.pick(&targets);
                let graceful = self.chaos.rng.chance(1, 2);
                let down = match self.chaos.rng.below(3) {
                    0 => self.chaos.rng.range(0, 2_000),
                    1 => self.chaos.rng.range(2_000, 40_000),
                    _ => self.chaos.rng.range(40_000, 200_000),
                };
                self.stop(m, graceful);
                let how = if graceful { "stopped" } else { "killed" };
                (
                    Kind::Down { m },
                    down,
                    format!("m{} {how} for {down} ms", m + 1),
                )
            }
            30..50 if !targets.is_empty() => {
                let m = self.chaos.rng.pick(&targets);
                let asleep = match self.chaos.rng.below(4) {
                    0 => self.chaos.rng.range(1_000, 20_000),
                    1 | 2 => self.chaos.rng.range(90_000, 600_000),
                    _ => self.chaos.rng.range(600_000, 3 * 3_600_000),
                };
                self.sleep(m);
                (
                    Kind::Asleep { m },
                    asleep,
                    format!("m{} asleep for {asleep} ms", m + 1),
                )
            }
            50..75 => {
                let n = self.machines.len();
                let a = self.chaos.rng.below(n as u64) as usize;
                let b = (a + 1 + self.chaos.rng.below(n as u64 - 1) as usize) % n;
                let (pairs, shape) = match self.chaos.rng.below(3) {
                    0 => (vec![(a, b)], format!("m{} cannot reach m{}", a + 1, b + 1)),
                    1 => (
                        vec![(a, b), (b, a)],
                        format!("m{} and m{} are partitioned", a + 1, b + 1),
                    ),
                    _ => (
                        (0..n)
                            .filter(|o| *o != a)
                            .flat_map(|o| [(a, o), (o, a)])
                            .collect(),
                        format!("m{} is isolated", a + 1),
                    ),
                };
                let lasts = match self.chaos.rng.below(3) {
                    0 => self.chaos.rng.range(500, 20_000),
                    1 => self.chaos.rng.range(20_000, 120_000),
                    _ => self.chaos.rng.range(120_000, 400_000),
                };
                for (from, to) in &pairs {
                    self.set_blocked(*from, *to, true);
                }
                (
                    Kind::Partition { pairs },
                    lasts,
                    format!("{shape} for {lasts} ms"),
                )
            }
            75..90 => {
                let live: Vec<usize> = self.net.live.iter().copied().collect();
                if live.is_empty() {
                    return;
                }
                let stream = self.chaos.rng.pick(&live);
                let after = self.net.streams[stream].delivered + self.chaos.rng.below(4);
                self.net.streams[stream].cut_after = Some(after);
                self.record(format!("stream {stream} cut after {after} frames"));
                return;
            }
            90..95 if self.chaos.clock_steps && !targets.is_empty() => {
                // The wall clock of a running machine is set forward or back.
                let m = self.chaos.rng.pick(&targets);
                let step = self.chaos.rng.range(1_000, 600_000) as i64;
                let step = if self.chaos.rng.chance(1, 2) {
                    step
                } else {
                    -step
                };
                self.machines[m].clock_offset_ms += step;
                self.record(format!("m{} clock stepped by {step} ms", m + 1));
                return;
            }
            _ => {
                let lasts = self.chaos.rng.range(5_000, 90_000);
                self.net.cut_storm += 1;
                (
                    Kind::CutStorm,
                    lasts,
                    format!("new exchanges cut for {lasts} ms"),
                )
            }
        };
        self.begin(kind, heal_ms, text);
    }

    pub(super) fn record(&mut self, text: String) {
        self.chaos.budget = self.chaos.budget.saturating_sub(1);
        self.chaos.cap = self.chaos.cap.map(|cap| cap.saturating_sub(1));
        self.stats.faults += 1;
        let line = format!("{:>10.3}s {text}", self.now as f64 / SEC as f64);
        self.note(usize::MAX, 7, || format!("FAULT {text}"));
        self.chaos.history.push(line);
    }

    /// Ends one fault, if it is still in effect.
    pub(super) fn heal(&mut self, fault: Fault) {
        let Some(index) = self.chaos.active.iter().position(|f| *f == fault) else {
            return;
        };
        self.chaos.active.remove(index);
        self.chaos.last_heal = self.now;
        self.note(usize::MAX, 8, || format!("HEAL {:?}", fault.kind));
        match fault.kind {
            Kind::Down { m } => {
                if self.machines[m].power == Power::Stopped && !self.machines[m].held {
                    self.start(m);
                }
            }
            Kind::Asleep { m } => {
                if !self.machines[m].held {
                    self.wake(m);
                }
            }
            Kind::Restored { m, .. } => self.heal_restored(m),
            Kind::Partition { pairs } => {
                for (from, to) in pairs {
                    self.set_blocked(from, to, false);
                }
            }
            Kind::CutStorm => self.net.cut_storm = self.net.cut_storm.saturating_sub(1),
        }
    }

    /// Stops injecting and ends every fault still in effect, now.
    pub fn calm(&mut self) {
        self.chaos.enabled = false;
        for fault in self.chaos.active.clone() {
            self.heal(fault);
        }
        for stream in &mut self.net.streams {
            stream.cut_after = None;
        }
        self.net.cut_rate = 0;
        self.net.open_fail_rate = 0;
        self.chaos.last_heal = self.now;
    }
}
