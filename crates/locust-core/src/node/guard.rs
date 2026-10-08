//! The restore guard: a key of this daemon signs nothing in a goal while
//! this daemon cannot show that it holds what the key already signed there.
//!
//! The marks (`Store::marks`, kept beside the data directory) name the last
//! record each key signed in each goal. A start compares them, and the
//! identity of the database file, with what the store holds (`guard_start`);
//! [`Node::hold`] then answers, for one key in one goal, whether it may sign
//! now. Every gate reads that answer and adds no condition of its own:
//! `next_place`, `drive_flow` and `plan_join` (through `admission_hold`).
//! A hold ends when the marked record is held again, when the computers the
//! goal names have been heard from (`guard_settle`), or on the owner's
//! `goal.continue`. Only the first is proof. The guard changes no signed byte
//! and no rule by which any daemon judges a record.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use locust_proto::api::{ApiError, GuardReason, GuardView, Halt};
use locust_proto::engine::Entropy;
use locust_proto::event::AuthorPoint;
use locust_proto::id::{EndpointId, GoalId, PublicKey};
use locust_proto::store::{Mark, MarkWrite, Marks, Store};

use super::Node;
use super::commit::Tx;
use super::entry::Entry;
use super::identity::Identity;
use super::local::{self, Restored};
use super::peers::historical_endpoints;

/// Most endpoints remembered per goal as callers.
const CALLERS: usize = 8;

pub(super) const CONFLICT: &str = "this agent has two records at one position in this goal and signs nothing more in it; the goal is not halted";

/// Why a key may not sign in a goal now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Hold {
    /// The marks name a record of the key that the store does not hold in
    /// the key's usable log.
    Behind { mark: AuthorPoint },
    /// This daemon's data is a copy of unknown age.
    Unheard,
    /// The key was just admitted here and the host's computer has not been
    /// heard from since.
    Admitted,
}

/// The guard's memory. Only the marks outlive a start, through the store.
#[derive(Debug, Default)]
pub(super) struct Guard {
    /// Read at the start, then updated by every commit that carries marks.
    marks: BTreeMap<(GoalId, PublicKey), Mark>,
    /// Computers heard from since this start, per goal.
    heard: BTreeMap<GoalId, BTreeSet<EndpointId>>,
    /// Endpoints bound to no current member that called about a goal whose
    /// governance key is held here, newest last.
    callers: BTreeMap<GoalId, VecDeque<EndpointId>>,
    /// Goals the marks name and the store does not hold, counted at a start
    /// that put the data back from a copy; 0 after an ordinary start.
    lost: u32,
}

impl Guard {
    pub fn apply(&mut self, writes: &[MarkWrite]) {
        apply(&mut self.marks, writes);
    }

    pub fn mark(&self, goal: &GoalId, key: &PublicKey) -> Option<&Mark> {
        self.marks.get(&(*goal, *key))
    }

    /// Endpoints to dial back about `goal`.
    pub fn callers(&self, goal: &GoalId) -> impl Iterator<Item = &EndpointId> {
        self.callers.get(goal).into_iter().flatten()
    }

    /// Forgets whom this daemon heard from in `goal`: an admission that has
    /// just landed waits for a hearing after it.
    pub fn unhear(&mut self, goal: &GoalId) {
        self.heard.remove(goal);
    }

    /// Goals this computer signed in that the copy it started from lacks.
    pub fn lost(&self) -> u32 {
        self.lost
    }
}

fn apply(marks: &mut BTreeMap<(GoalId, PublicKey), Mark>, writes: &[MarkWrite]) {
    for write in writes {
        match *write {
            MarkWrite::Set(mark) => {
                marks.insert((mark.goal, mark.key), mark);
            }
            MarkWrite::Clear { goal, key } => {
                marks.remove(&(goal, key));
            }
        }
    }
}

/// The marks of one goal.
fn of_goal(
    marks: &BTreeMap<(GoalId, PublicKey), Mark>,
    goal: GoalId,
) -> BTreeMap<(GoalId, PublicKey), Mark> {
    marks
        .range((goal, PublicKey([0x00; 32]))..=(goal, PublicKey([0xff; 32])))
        .map(|(key, mark)| (*key, *mark))
        .collect()
}

/// Whom this daemon must hear from in a goal.
#[derive(Debug)]
pub(super) struct Sources {
    /// Every other computer: an endpoint, other than this daemon's, that a
    /// current member is bound to.
    pub all: BTreeSet<EndpointId>,
    /// The host's computer, when this daemon does not host the goal.
    pub host: Option<EndpointId>,
}

/// What a start found, from the two facts the first table of the plan reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Start {
    /// Marks kept, the file last used, nothing ahead.
    Ordinary,
    /// Marks kept and the file last used, but a mark is ahead of the store:
    /// the file was overwritten in place, so every goal in it is restored.
    Overwritten,
    /// Marks kept, another file: the data directory was replaced by a copy.
    Replaced,
    /// Marks lost or copied, the file last used.
    MarksLost,
    /// Marks lost or copied, another file: a copy of unknown age.
    Unknown,
}

static NOBODY: BTreeSet<EndpointId> = BTreeSet::new();

impl<S: Store, E: Entropy> Node<S, E> {
    fn own_endpoint_id(&self) -> Option<EndpointId> {
        self.identity
            .endpoint
            .as_ref()
            .map(|record| record.endpoint)
    }

    /// Whether this daemon may sign with `key` in this goal now: `None` means
    /// it may.
    pub(super) fn hold(&self, entry: &Entry, key: &PublicKey) -> Option<Hold> {
        self.hold_with(entry, key, &self.guard.marks)
    }

    /// [`Node::hold`] for the key that signs admissions in this goal.
    pub(super) fn admission_hold(&self, entry: &Entry) -> Option<Hold> {
        let governance = entry.state().governance?;
        self.hold(entry, &governance)
    }

    /// The view a refusal of `key` carries. Where this daemon hosts the goal
    /// and the goal's own key is held, that key's: every agent there is held
    /// with it, and its view says when only the person can end the wait.
    /// An agent's own missing records are named only while the goal's key
    /// waits for another computer too. Otherwise the key's own hold. `None`
    /// exactly when [`Node::hold`] answers none.
    pub(super) fn hold_view(&self, entry: &Entry, key: &PublicKey) -> Option<GuardView> {
        self.hold(entry, key)?;
        let governance = self
            .hosted_governance(entry)
            .filter(|governance| governance != key)
            .and_then(|governance| self.guard_view(entry, &governance));
        match (self.own_hold(entry, key), governance) {
            (Some(Hold::Behind { .. }), Some(governance)) if !governance.waits_for_you() => {
                self.guard_view(entry, key)
            }
            (_, Some(governance)) => Some(governance),
            (_, None) => self.guard_view(entry, key),
        }
    }

    fn hold_with(
        &self,
        entry: &Entry,
        key: &PublicKey,
        marks: &BTreeMap<(GoalId, PublicKey), Mark>,
    ) -> Option<Hold> {
        // A forked key signs nothing anyway, and status calls it a conflict.
        if entry.goal.fork_point(key).is_some() {
            return None;
        }
        if let Some(behind) = behind(entry, key, marks) {
            return Some(behind);
        }
        // Where the host's own records are missing, this copy of who is in,
        // of the rules and of the goal's end is known to be old.
        if let Some(governance) = entry.state().governance
            && governance != *key
            && self.hosts(entry)
            && let Some(hold) = self.hold_with(entry, &governance, marks)
        {
            return Some(hold);
        }
        unheard(entry, key)
    }

    /// The hold on `key` for its own reason, not the governance key's.
    fn own_hold(&self, entry: &Entry, key: &PublicKey) -> Option<Hold> {
        if entry.goal.fork_point(key).is_some() {
            return None;
        }
        behind(entry, key, &self.guard.marks).or_else(|| unheard(entry, key))
    }

    /// The one function that names whom this daemon must hear from, and the
    /// place host replacement plugs into.
    pub(super) fn guard_sources(&self, entry: &Entry) -> Sources {
        let own = self.own_endpoint_id();
        let state = entry.state();
        let all = state
            .members
            .values()
            .filter(|member| member.is_active())
            .map(|member| member.endpoint)
            .filter(|endpoint| Some(*endpoint) != own)
            .collect();
        let host = if self.hosts(entry) {
            None
        } else {
            state
                .host
                .and_then(|host| state.members.get(&host))
                .map(|member| member.endpoint)
                .or_else(|| entry.local.joins.values().next().map(|join| join.endpoint))
                .filter(|endpoint| Some(*endpoint) != own)
        };
        Sources { all, host }
    }

    /// Computers heard from in `goal` since this start.
    pub(super) fn heard(&self, goal: &GoalId) -> &BTreeSet<EndpointId> {
        self.guard.heard.get(goal).unwrap_or(&NOBODY)
    }

    /// The governance log held here admits a member on another computer.
    fn shared_in(&self, entry: &Entry) -> bool {
        let own = self.own_endpoint_id();
        historical_endpoints(entry)
            .into_iter()
            .any(|endpoint| own.is_none_or(|own| endpoint != own))
    }

    /// Whether a mark written for `goal` should say the goal was shared: it
    /// said so before, or the governance log held here admits another
    /// computer. Set on every mark a commit carries, after its events are
    /// applied, so the admission that first shares a goal carries the bit.
    pub(super) fn shared(&self, mark: &Mark) -> bool {
        mark.shared
            || self
                .guard
                .mark(&mark.goal, &mark.key)
                .is_some_and(|kept| kept.shared)
            || self
                .goals
                .get(&mark.goal)
                .is_some_and(|entry| self.shared_in(entry))
    }

    /// The keys of this daemon in a goal: the governance key where this
    /// daemon hosts it, and every local principal that is, was or is
    /// becoming a member, or that signed there.
    pub(super) fn local_keys(&self, entry: &Entry) -> BTreeSet<PublicKey> {
        let marked = of_goal(&self.guard.marks, entry.id());
        let mut keys: BTreeSet<_> = entry
            .state()
            .members
            .keys()
            .chain(entry.local.part.keys())
            .chain(entry.local.joins.keys())
            .chain(entry.local.unheard.iter())
            .chain(marked.keys().map(|(_, key)| key))
            .filter(|key| self.principals.holds(key))
            .copied()
            .collect();
        if self.hosts(entry)
            && let Some(governance) = entry.state().governance
        {
            keys.insert(governance);
        }
        keys
    }

    /// The marks of a goal whose key this daemon signs with: a local
    /// principal's, or the governance key where this daemon hosts the goal.
    /// Any other mark was written by a data directory this one replaced. It
    /// is kept, never lowered, and never read here: it holds that
    /// directory's key if that directory is put back.
    fn own_marks(&self, entry: &Entry) -> BTreeMap<(GoalId, PublicKey), Mark> {
        let hosted = self.hosted_governance(entry);
        of_goal(&self.guard.marks, entry.id())
            .into_iter()
            .filter(|((_, key), _)| self.principals.holds(key) || hosted == Some(*key))
            .collect()
    }

    /// What lowers the mark of `key` to the end of the key's usable log: the
    /// record the table gives up is then no longer asked for.
    fn lowered(entry: &Entry, key: &PublicKey, mark: &Mark) -> MarkWrite {
        let points = entry.goal.points(key);
        let usable = entry.goal.usable(key) as usize;
        match usable.checked_sub(1).map(|last| points[last]) {
            Some(point) => MarkWrite::Set(Mark { point, ..*mark }),
            None => MarkWrite::Clear {
                goal: entry.id(),
                key: *key,
            },
        }
    }

    /// Ends the holds of `goal` whose rule in the second table of the plan is
    /// met: deletes `UNHEARD` records and clears `unheard` in the `RESTORED`
    /// record, and lowers a mark where the table gives up its record. In a
    /// goal this daemon hosts it never clears `unheard`: only `goal.continue`
    /// does. A settle that ends a hold touches the goal, so landing it runs
    /// the goal's flow and a waiting step is signed at once.
    pub(super) fn guard_settle(&self, goal: GoalId, tx: &mut Tx) {
        let Some(entry) = self.goals.get(&goal) else {
            return;
        };
        let sources = self.guard_sources(entry);
        let heard = self.heard(&goal);
        let hosts = self.hosts(entry);
        let governance = entry.state().governance;
        let mut marks = self.own_marks(entry);
        let mut ended = false;
        let mut lowered_keys = BTreeSet::new();

        // The governance key first, since an agent's give-up waits on it.
        // It never gives up a record by itself: the records it is missing
        // say who the other computers are. Only a goal never shared, by the
        // mark and by this copy, needs no other computer.
        if let Some(governance) = governance.filter(|_| hosts)
            && let Some(mark) = marks.get(&(goal, governance)).copied()
            && behind(entry, &governance, &marks).is_some()
            && !mark.shared
            && !self.shared_in(entry)
        {
            let write = Self::lowered(entry, &governance, &mark);
            apply(&mut marks, &[write]);
            tx.commit.marks.push(write);
            lowered_keys.insert(governance);
            ended = true;
        }
        let heard_all = sources.all.iter().all(|endpoint| heard.contains(endpoint));
        let governance_held = hosts
            && governance
                .is_some_and(|governance| self.hold_with(entry, &governance, &marks).is_some());
        if heard_all && !governance_held {
            let agents: Vec<_> = marks
                .iter()
                .filter(|((_, key), _)| Some(*key) != governance)
                .map(|((_, key), mark)| (*key, *mark))
                .collect();
            for (key, mark) in agents {
                if behind(entry, &key, &marks).is_some() {
                    let write = Self::lowered(entry, &key, &mark);
                    apply(&mut marks, &[write]);
                    tx.commit.marks.push(write);
                    lowered_keys.insert(key);
                    ended = true;
                }
            }
        }

        let host_heard = sources.host.is_some_and(|host| heard.contains(&host));
        if let Some(restored) = entry.local.restored
            && restored.unheard
            && !hosts
        {
            let mut others = sources
                .all
                .iter()
                .filter(|endpoint| Some(**endpoint) != sources.host)
                .peekable();
            let others_heard =
                others.peek().is_some() && others.all(|endpoint| heard.contains(endpoint));
            if host_heard || others_heard {
                tx.local(local::restored_write(
                    &goal,
                    &Restored {
                        unheard: false,
                        ..restored
                    },
                ));
                // Rewrite each mark that still says the goal was unheard, so
                // the marks file no longer makes the next start read a copy
                // of unknown age. The give-ups above rewrite their own.
                for ((_, key), mark) in &marks {
                    if mark.unheard && !lowered_keys.contains(key) {
                        tx.commit.marks.push(MarkWrite::Set(*mark));
                    }
                }
                ended = true;
            }
        }
        if host_heard {
            for key in &entry.local.unheard {
                tx.local(local::unheard_delete(&goal, key));
                ended = true;
            }
        }
        if ended {
            tx.touch(goal);
        }
    }

    /// Writes the goal's `RESTORED` record. Where this daemon hosts the goal
    /// it also revokes every pending invitation: a copy cannot know which
    /// tickets were used or revoked after it was taken. An earlier record's
    /// count and `unheard` are kept.
    pub(super) fn restore_found(
        &self,
        entry: &Entry,
        unheard: bool,
        now_ms: u64,
        tx: &mut Tx,
    ) -> Result<(), ApiError> {
        let goal = entry.id();
        let revoked = if self.hosts(entry) {
            self.revoke_pending(goal, now_ms, tx)?
        } else {
            0
        };
        let before = entry.local.restored;
        tx.local(local::restored_write(
            &goal,
            &Restored {
                revoked: before
                    .map_or(0, |before| before.revoked)
                    .saturating_add(revoked),
                unheard: unheard || before.is_some_and(|before| before.unheard),
            },
        ))
        .touch(goal);
        Ok(())
    }

    /// Reads the marks and decides what this start is, then writes, in one
    /// commit synced like any other before the first exchange: a `RESTORED`
    /// record for each goal the start finds restored, the marks again from
    /// the store where they are lost or short of it, the `RESTORED` record
    /// deleted where the start is ordinary and nothing is held, and the
    /// database file's identity when it changed.
    pub(super) fn guard_start(&mut self, found: Marks, now_ms: u64) -> Result<(), ApiError> {
        if let Some(kept) = &found.kept {
            self.guard.marks = kept
                .iter()
                .map(|mark| ((mark.goal, mark.key), *mark))
                .collect();
        }
        let mut tx = Tx::none();
        if self.identity.file != Some(found.file) {
            tx.local(Identity::file_write(&found.file));
        }
        // A new store holds nothing and no key of it has signed. Marks found
        // beside it are kept: they name keys of the data directory this one
        // replaced, and are what holds those keys if that directory is put
        // back from an older copy. No key of the new store signs under them
        // (`own_marks`).
        let new =
            self.identity.file.is_none() && self.goals.values().all(|entry| entry.goal.is_empty());
        if new {
            return self.land_once(tx);
        }
        let same_file = self
            .identity
            .file
            .is_some_and(|file| file.same(&found.file));
        // Only a mark of a key this daemon signs with can show the file was
        // overwritten. A goal the store does not hold at all is left out: its
        // marks stay as the only memory of it. So is a goal that already
        // holds its `RESTORED` record: it was found restored, and may still
        // be catching up. Either would otherwise make every later start look
        // overwritten, and find restored a goal made since.
        let ahead = || {
            self.goals
                .values()
                .filter(|entry| entry.local.restored.is_none())
                .any(|entry| {
                    self.own_marks(entry)
                        .values()
                        .any(|mark| !entry.goal.holds(&mark.point.id))
                })
        };
        let start = match (found.kept.is_some(), same_file) {
            (true, true) if ahead() => Start::Overwritten,
            (true, true) => Start::Ordinary,
            (true, false) => Start::Replaced,
            (false, true) => Start::MarksLost,
            (false, false) => Start::Unknown,
        };
        // The goals the kept marks name and this copy lacks were lost with
        // it. The number lasts until the next ordinary start.
        self.guard.lost = match start {
            Start::Replaced | Start::Overwritten => {
                let lost: BTreeSet<_> = self
                    .guard
                    .marks
                    .keys()
                    .map(|(goal, _)| *goal)
                    .filter(|goal| !self.goals.contains_key(goal))
                    .collect();
                u32::try_from(lost.len()).unwrap_or(u32::MAX)
            }
            Start::Ordinary | Start::MarksLost | Start::Unknown => 0,
        };
        // Kept marks written while a goal was unheard keep that memory when
        // the store does not: a start with the marks lost rewrites them from
        // the copy, and the `RESTORED` record that said unheard is lost with
        // the copy the marks describe. A goal the store holds whose kept
        // marks say unheard while its `RESTORED` record does not is a copy
        // of unknown age, whatever else this start found. A goal where no
        // local key has any record in the copy carries no mark, so the bit
        // cannot keep the memory there.
        let marked_unheard: BTreeSet<GoalId> = if found.kept.is_some() {
            self.goals
                .values()
                .filter(|entry| {
                    !entry
                        .local
                        .restored
                        .is_some_and(|restored| restored.unheard)
                        && self.own_marks(entry).values().any(|mark| mark.unheard)
                })
                .map(|entry| entry.id())
                .collect()
        } else {
            BTreeSet::new()
        };
        for entry in self.goals.values() {
            let goal = entry.id();
            let marked = marked_unheard.contains(&goal);
            match start {
                Start::Ordinary if !marked => {}
                Start::Overwritten if !marked && entry.local.restored.is_some() => {}
                Start::Ordinary | Start::Overwritten | Start::Replaced => {
                    self.restore_found(entry, marked, now_ms, &mut tx)?
                }
                Start::Unknown => self.restore_found(entry, true, now_ms, &mut tx)?,
                // A behind hold is remembered only by the marks: losing them
                // while a restore is caught up must not make the goal
                // ordinary.
                Start::MarksLost => {
                    if let Some(restored) = entry.local.restored {
                        tx.local(local::restored_write(
                            &goal,
                            &Restored {
                                unheard: true,
                                ..restored
                            },
                        ))
                        .touch(goal);
                    }
                }
            }
            for key in self.local_keys(entry) {
                let Some(tip) = entry.goal.points(&key).last().copied() else {
                    // Lost marks stay lost until something is written to
                    // them. Without a write here, a start whose keys have
                    // signed nothing would leave them lost, and every later
                    // start would read the restore as still being caught up.
                    if found.kept.is_none() {
                        tx.commit.marks.push(MarkWrite::Clear { goal, key });
                    }
                    continue;
                };
                let mark = Mark {
                    goal,
                    key,
                    point: tip,
                    shared: false,
                    unheard: false,
                };
                match self.guard.mark(&goal, &key) {
                    None => tx.commit.marks.push(MarkWrite::Set(mark)),
                    Some(kept) if kept.point.seq < tip.seq => {
                        tx.commit.marks.push(MarkWrite::Set(Mark {
                            shared: kept.shared,
                            ..mark
                        }))
                    }
                    Some(_) => {}
                }
            }
        }
        if start == Start::Ordinary {
            let mut marks = self.guard.marks.clone();
            apply(&mut marks, &tx.commit.marks);
            for entry in self.goals.values() {
                // A goal the marks say are unheard was found restored above;
                // its `RESTORED` write must not be deleted in this commit.
                if marked_unheard.contains(&entry.id()) {
                    continue;
                }
                if entry.local.restored.is_some()
                    && self
                        .local_keys(entry)
                        .iter()
                        .all(|key| self.hold_with(entry, key, &marks).is_none())
                {
                    tx.local(local::restored_delete(&entry.id()))
                        .touch(entry.id());
                }
            }
        }
        self.land_once(tx)
    }

    /// Records that `endpoint` was heard from in `goal` and ends the holds
    /// that waited for it. Settles on every hearing, not only the first: a
    /// rule can be met by records that arrived since.
    pub(super) fn guard_heard(&mut self, goal: GoalId, endpoint: EndpointId) {
        if !self.goals.contains_key(&goal) {
            return;
        }
        self.guard.heard.entry(goal).or_default().insert(endpoint);
        let mut tx = Tx::none();
        self.guard_settle(goal, &mut tx);
        let _ = self.land(tx);
    }

    /// Remembers an endpoint bound to no current member that called about a
    /// goal this daemon hosts while its governance key is held, so it is
    /// dialed back: it may hold the host's later records.
    pub(super) fn guard_caller(&mut self, goal: GoalId, endpoint: EndpointId) {
        let Some(entry) = self.goals.get(&goal) else {
            return;
        };
        let member = entry
            .state()
            .members
            .values()
            .any(|member| member.is_active() && member.endpoint == endpoint);
        if member
            || !self.hosts(entry)
            || self.admission_hold(entry).is_none()
            || Some(endpoint) == self.own_endpoint_id()
        {
            return;
        }
        let callers = self.guard.callers.entry(goal).or_default();
        callers.retain(|known| *known != endpoint);
        callers.push_back(endpoint);
        if callers.len() > CALLERS {
            callers.pop_front();
        }
    }

    /// Callers are dropped once the hold that asked for them ends. Run by
    /// every landing that touches the goal, since each way a hold ends
    /// lands: records arriving, a settle, or `goal.continue`.
    pub(super) fn forget_callers(&mut self, goal: GoalId) {
        if self
            .goals
            .get(&goal)
            .is_none_or(|entry| self.admission_hold(entry).is_none())
        {
            self.guard.callers.remove(&goal);
        }
    }

    /// Why `key`, or the goal itself, cannot advance here: the goal's
    /// authority conflict first; else this agent's own fork; else a hold
    /// that waits for this daemon to catch up.
    pub(super) fn halt(&self, entry: &Entry, key: Option<&PublicKey>) -> Option<Halt> {
        if let Some(halt) = entry.halted() {
            return Some(halt);
        }
        let key = key?;
        if entry.goal.fork_point(key).is_some() && entry.state().governance.as_ref() != Some(key) {
            return Some(Halt::SignerConflict);
        }
        match self.hold(entry, key) {
            Some(Hold::Behind { .. } | Hold::Unheard) => Some(Halt::SignerRecovery),
            Some(Hold::Admitted) | None => None,
        }
    }

    /// The governance key, where this daemon hosts the goal.
    pub(super) fn hosted_governance(&self, entry: &Entry) -> Option<PublicKey> {
        entry.state().governance.filter(|_| self.hosts(entry))
    }

    /// The view of `key`'s own hold in a goal, if it has one.
    pub(super) fn guard_view(&self, entry: &Entry, key: &PublicKey) -> Option<GuardView> {
        let reason = match self.own_hold(entry, key)? {
            Hold::Behind { mark } => GuardReason::Behind {
                held: entry.goal.usable(key),
                signed: mark.seq.saturating_add(1),
            },
            Hold::Unheard => GuardReason::Unheard,
            Hold::Admitted => GuardReason::Admitted,
        };
        let sources = self.guard_sources(entry);
        let heard = self.heard(&entry.id());
        Some(GuardView {
            key: *key,
            by_host: entry.state().governance.as_ref() == Some(key),
            reason,
            heard: heard.iter().copied().collect(),
            waiting: sources
                .all
                .iter()
                .chain(sources.host.as_ref())
                .filter(|endpoint| !heard.contains(endpoint))
                .copied()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
        })
    }

    /// The views of `keys`' own holds, and of the governance key's where
    /// this daemon hosts the goal.
    pub(super) fn guard_views(
        &self,
        entry: &Entry,
        keys: impl IntoIterator<Item = PublicKey>,
    ) -> Vec<GuardView> {
        let mut keys: BTreeSet<_> = keys.into_iter().collect();
        keys.extend(self.hosted_governance(entry));
        keys.iter()
            .filter_map(|key| self.guard_view(entry, key))
            .collect()
    }

    /// The owner's word that this daemon has caught up in a goal. Deletes
    /// every `UNHEARD` record, clears `unheard` in the `RESTORED` record and
    /// lowers every mark that is ahead to the end of its key's usable log.
    /// Answers how many keys were held and are not now.
    pub(super) fn guard_continue(&self, entry: &Entry, tx: &mut Tx) -> u32 {
        let goal = entry.id();
        let keys = self.local_keys(entry);
        let held = keys
            .iter()
            .filter(|key| self.hold(entry, key).is_some())
            .count();
        for key in &entry.local.unheard {
            tx.local(local::unheard_delete(&goal, key));
        }
        if let Some(restored) = entry.local.restored {
            tx.local(local::restored_write(
                &goal,
                &Restored {
                    unheard: false,
                    ..restored
                },
            ));
        }
        let was_unheard = entry
            .local
            .restored
            .is_some_and(|restored| restored.unheard);
        for (key, mark) in self
            .own_marks(entry)
            .into_iter()
            .map(|((_, key), mark)| (key, mark))
        {
            if behind(entry, &key, &self.guard.marks).is_some() {
                tx.commit.marks.push(Self::lowered(entry, &key, &mark));
            } else if was_unheard && mark.unheard {
                // Rewrite the mark so the bit it was written with while the
                // goal was unheard reads false again.
                tx.commit.marks.push(MarkWrite::Set(mark));
            }
        }
        tx.touch(goal);
        u32::try_from(held).unwrap_or(u32::MAX)
    }
}

/// A forked log is never behind: its usable prefix ends at the fork, and
/// the key signs nothing there anyway.
fn behind(
    entry: &Entry,
    key: &PublicKey,
    marks: &BTreeMap<(GoalId, PublicKey), Mark>,
) -> Option<Hold> {
    let mark = marks.get(&(entry.id(), *key))?;
    (entry.goal.fork_point(key).is_none() && !entry.goal.holds_usable(key, mark.point))
        .then_some(Hold::Behind { mark: mark.point })
}

fn unheard(entry: &Entry, key: &PublicKey) -> Option<Hold> {
    if entry
        .local
        .restored
        .is_some_and(|restored| restored.unheard)
    {
        Some(Hold::Unheard)
    } else if entry.local.unheard.contains(key) {
        Some(Hold::Admitted)
    } else {
        None
    }
}
