//! The restore fault: a stopped machine's store goes back to a backup taken
//! earlier in the run, with its marks kept (the data directory alone was
//! replaced by a copy) or lost (the whole computer was put back, so its
//! older marks file reads as lost).
//!
//! Backups are taken of a machine only once the scenario says that it is a
//! member and has finished its own part of the guide that lives in local
//! records alone (a claim, a session, an allowance): a copy older than that
//! brings back older local settings, which the guard does not cover and the
//! checks would read as a broken claim. A backup is a copy of the records
//! and never shares the marks. Restoring makes another copy of it, so a
//! backup can be restored more than once, and drops every later backup of
//! that machine: those copies are of a history the restore abandoned.

use std::collections::BTreeSet;

use locust_proto::id::{BlobHash, EventId};
use locust_proto::store::{MemStore, Store};

use super::chaos::Kind;
use super::machine::Power;
use super::world::{Ev, MS, Micros, SEC, World};
use crate::node::tests::snapshot_all;

/// What a restore does with the marks beside the data directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marks {
    /// The marks directory survived: the copy is started with the marks as
    /// they are now.
    Kept,
    /// The marks were put back with the data, and read as lost.
    Lost,
}

/// A copy of one machine's records, taken while the run went on.
pub struct Backup {
    pub taken: Micros,
    store: MemStore,
}

/// One restore that happened, as the checks need it afterwards.
#[derive(Clone, Debug)]
pub struct Restore {
    pub m: usize,
    pub at: Micros,
    /// When the backup put back was taken.
    pub taken: Micros,
    pub marks: Marks,
    /// Every record some machine held just before the restore. A record not
    /// in it was signed after the restore.
    pub before: BTreeSet<EventId>,
    /// The records the restored copy holds: the backup as it was taken.
    pub copy: BTreeSet<EventId>,
    /// The records only this machine held, which no machine holds now.
    pub lost: BTreeSet<EventId>,
    /// The content objects only this machine held. A record can outlive its
    /// content: content is fetched after its record, so a copy can hold a
    /// record without the text it names.
    pub lost_blobs: BTreeSet<BlobHash>,
}

impl World {
    /// Every record machine `m` holds, in every goal.
    pub fn events_of(&self, m: usize) -> BTreeSet<EventId> {
        let store = &self.machines[m].store;
        let goals = store.goals().expect("a memory store reads");
        goals
            .iter()
            .flat_map(|goal| store.log(goal, 0, usize::MAX).expect("log"))
            .map(|(_, event)| event.id())
            .collect()
    }

    /// Every record some machine holds.
    pub fn everywhere(&self) -> BTreeSet<EventId> {
        (0..self.machines.len())
            .flat_map(|m| self.events_of(m))
            .collect()
    }

    /// Records lost everywhere by a restore. The checks leave them out of
    /// what every machine must hold.
    pub fn lost(&self) -> BTreeSet<EventId> {
        self.restores
            .iter()
            .flat_map(|restore| restore.lost.iter().copied())
            .collect()
    }

    /// Content objects lost everywhere by a restore.
    pub fn lost_blobs(&self) -> BTreeSet<BlobHash> {
        self.restores
            .iter()
            .flat_map(|restore| restore.lost_blobs.iter().copied())
            .collect()
    }

    /// The content objects machine `m` holds of those its records name.
    fn blobs_of(store: &MemStore) -> BTreeSet<BlobHash> {
        let goals = store.goals().expect("a memory store reads");
        goals
            .iter()
            .flat_map(|goal| store.log(goal, 0, usize::MAX).expect("log"))
            .flat_map(|(_, event)| event.header().blobs())
            .filter(|hash| {
                store
                    .blob_len(hash)
                    .expect("a memory store reads")
                    .is_some()
            })
            .collect()
    }

    /// Takes a backup of machine `m` now, if the scenario allows one.
    pub fn backup(&mut self, m: usize) {
        if !self.chaos.allow_restores || !self.ready[m] {
            return;
        }
        let store = snapshot_all(&self.machines[m].store);
        let taken = self.now;
        self.note(m, 9, || "backed up".into());
        self.backups[m].push(Backup { taken, store });
    }

    /// A backup of each machine that allows one, at a seeded half of the
    /// points the scenario offers.
    pub fn backups(&mut self) {
        for m in 0..self.machines.len() {
            if self.chaos.allow_restores && self.ready[m] && self.chaos.backup_rng.chance(1, 2) {
                self.backup(m);
            }
        }
    }

    /// Machines a restore may hit: running, not kept by the scenario, with a
    /// backup.
    pub(super) fn restorable(&self) -> Vec<usize> {
        (0..self.machines.len())
            .filter(|m| {
                let machine = &self.machines[*m];
                machine.running() && !machine.held && !self.backups[*m].is_empty()
            })
            .collect()
    }

    /// Stops a machine, puts one of its backups in place of its store, and
    /// returns the fault that starts it again and its words.
    pub(super) fn restore(&mut self, m: usize) -> (Kind, u64, String) {
        let rng = &mut self.chaos.backup_rng;
        let k = rng.below(self.backups[m].len() as u64) as usize;
        let marks = if rng.chance(1, 2) {
            Marks::Kept
        } else {
            Marks::Lost
        };
        let graceful = rng.chance(1, 2);
        let down = match rng.below(3) {
            0 => rng.range(0, 2_000),
            1 => rng.range(2_000, 40_000),
            _ => rng.range(40_000, 200_000),
        };
        let lost = self.put_back(m, k, marks, graceful);
        let taken = self.backups[m][k].taken;
        let how = if graceful { "stopped" } else { "killed" };
        let text = format!(
            "m{} {how} and restored to its backup from {:.1} s with its marks {}, \
             {} records lost everywhere, down for {down} ms",
            m + 1,
            taken as f64 / SEC as f64,
            match marks {
                Marks::Kept => "kept",
                Marks::Lost => "lost",
            },
            lost,
        );
        (Kind::Restored { m, marks }, down, text)
    }

    /// Stops machine `m` and puts its backup `k` in place of its store,
    /// with the marks kept or lost. Returns how many records no machine
    /// holds now. The machine stays stopped.
    pub fn put_back(&mut self, m: usize, k: usize, marks: Marks, graceful: bool) -> usize {
        self.stop(m, graceful);
        // What a restored machine shows is older, not what it showed when it
        // stopped.
        self.machines[m].last_view = None;
        let before = self.everywhere();
        let others: BTreeSet<EventId> = (0..self.machines.len())
            .filter(|n| *n != m)
            .flat_map(|n| self.events_of(n))
            .collect();
        let backup = &self.backups[m][k];
        let taken = backup.taken;
        let mut copy = snapshot_all(&backup.store);
        let kept: BTreeSet<EventId> = {
            let goals = copy.goals().expect("a memory store reads");
            goals
                .iter()
                .flat_map(|goal| copy.log(goal, 0, usize::MAX).expect("log"))
                .map(|(_, event)| event.id())
                .collect()
        };
        let lost: BTreeSet<EventId> = self
            .events_of(m)
            .into_iter()
            .filter(|id| !kept.contains(id) && !others.contains(id))
            .collect();
        let other_blobs: BTreeSet<BlobHash> = (0..self.machines.len())
            .filter(|n| *n != m)
            .flat_map(|n| Self::blobs_of(&self.machines[n].store))
            .collect();
        let copy_blobs = Self::blobs_of(&copy);
        let lost_blobs: BTreeSet<BlobHash> = Self::blobs_of(&self.machines[m].store)
            .into_iter()
            .filter(|hash| !copy_blobs.contains(hash) && !other_blobs.contains(hash))
            .collect();
        if marks == Marks::Kept {
            copy = copy.with_marks(self.machines[m].store.marks_handle());
        }
        self.machines[m].store = copy;
        self.backups[m].truncate(k + 1);
        let count = lost.len();
        self.restores.push(Restore {
            m,
            at: self.now,
            taken,
            marks,
            before,
            copy: kept,
            lost,
            lost_blobs,
        });
        count
    }

    /// At a point the scenario offers, possibly restores one machine now.
    /// Returns the machine restored. The guide offers one before the third
    /// machine joins, so that a restored host is asked for an admission,
    /// a governance signature, while it may still be catching up.
    pub fn restore_point(&mut self) -> Option<usize> {
        if !self.chaos.enabled || !self.chaos.allow_restores || self.chaos.cap == Some(0) {
            return None;
        }
        if !self.chaos.backup_rng.chance(1, 2) {
            return None;
        }
        let targets = self.restorable();
        if targets.is_empty() {
            return None;
        }
        let m = self.chaos.backup_rng.pick(&targets);
        let (kind, heal_ms, text) = self.restore(m);
        self.begin(kind, heal_ms, text);
        Some(m)
    }

    /// The restore fault heals as `Down` does: the process starts again,
    /// now over the restored store.
    pub(super) fn heal_restored(&mut self, m: usize) {
        if self.machines[m].power == Power::Stopped && !self.machines[m].held {
            self.start(m);
        }
    }

    /// Records a fault in effect and schedules its end.
    pub(super) fn begin(&mut self, kind: Kind, heal_ms: u64, text: String) {
        let fault = self.chaos.fault(kind);
        self.record(text);
        self.schedule(self.now + heal_ms * MS, Ev::Heal(fault));
    }
}
