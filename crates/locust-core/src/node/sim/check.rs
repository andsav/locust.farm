//! What must hold once faults have stopped and the network has gone quiet.

use std::collections::{BTreeMap, BTreeSet};

use locust_proto::api::{
    BlobState, ErrorCode, Halt, Membership, PendingWork, Request, Response, Standing,
};
use locust_proto::event::{Body, EffectAction, Event, TaskId};
use locust_proto::id::{EffectId, EventId, PublicKey};
use locust_proto::store::Store;

use super::machine::Who;
use super::restore::{Marks, Restore};
use super::run::{Fail, PATIENCE, Run};
use super::scenario::{RESULT_TEXT, TASK_TEXT, TITLE};
use super::world::{Micros, SEC};
use crate::sync::ANTI_ENTROPY_MS;

/// The identifiers of every event machine `m` holds of the goal, read from
/// its store.
fn held(r: &Run, m: usize) -> BTreeSet<EventId> {
    let log = r.w.machines[m].store.log(&r.goal(), 0, usize::MAX);
    log.expect("a memory store reads")
        .into_iter()
        .map(|(_, event)| event.id())
        .collect()
}

fn same_events(r: &Run) -> bool {
    let first = held(r, 0);
    (1..r.w.machines.len()).all(|m| held(r, m) == first)
}

/// The machine that hosts the goal in every scenario.
const HOST: usize = 0;

/// Two or more records of one key at one position, each held by some
/// machine now.
#[derive(Debug)]
pub struct Reuse {
    pub key: PublicKey,
    pub seq: u64,
    pub records: BTreeSet<EventId>,
    /// The machine whose key it is.
    pub m: usize,
    /// Whether the plan claims that this cannot happen.
    pub claimed: bool,
}

impl Reuse {
    /// The reuse in words, for a report.
    pub fn describe(&self, r: &Run) -> String {
        let whose = if r.principals.contains(&self.key) {
            "agent"
        } else {
            "governance key"
        };
        let claim = if self.claimed {
            "claimed"
        } else {
            "outside the claims"
        };
        let holders: Vec<String> = self
            .records
            .iter()
            .map(|id| {
                let on: Vec<String> = (0..r.w.machines.len())
                    .filter(|m| {
                        let store = &r.w.machines[*m].store;
                        store.event(id).expect("a memory store reads").is_some()
                    })
                    .map(|m| format!("m{}", m + 1))
                    .collect();
                on.join("+")
            })
            .collect();
        format!(
            "{} records of m{}'s {whose} at position {}, held on {} ({claim})",
            self.records.len(),
            self.m + 1,
            self.seq,
            holders.join(" and ")
        )
    }
}

/// Every used position signed again: one at which a machine holds a record
/// while another machine holds a different one, which is the model's
/// `StoreNoFork`. A position whose record no machine holds any more is not
/// used, so signing it again is no reuse.
pub fn reuses(r: &Run) -> Vec<Reuse> {
    let goal = r.goal();
    let mut at: BTreeMap<(PublicKey, u64), BTreeSet<EventId>> = BTreeMap::new();
    for machine in &r.w.machines {
        let log = machine.store.log(&goal, 0, usize::MAX);
        for (_, event) in log.expect("a memory store reads") {
            let header = event.header();
            at.entry((header.author, header.seq))
                .or_default()
                .insert(event.id());
        }
    }
    at.into_iter()
        .filter(|(_, records)| records.len() > 1)
        .map(|((key, seq), records)| {
            // Every key that is no principal's is the governance key.
            let agent = r.principals.iter().position(|p| *p == key);
            let m = agent.unwrap_or(HOST);
            let claimed = claimed(r, m, agent.is_none(), &records);
            Reuse {
                key,
                seq,
                records,
                m,
                claimed,
            }
        })
        .collect()
}

/// Where the plan claims that a restored machine signs at no used position
/// (the simulator bullet of G1's tests in
/// `docs/host-safety-and-ending-plan.md`). A reuse is left out when one of
/// its records was signed after an event that the claim excludes and before
/// the machine's next restore, which the claim judges afresh:
///
/// - the machine's owner sent `goal.continue`, for any key;
/// - on a member's machine, an agent's key of which a restore with the
///   marks kept finds no mark (an earlier restore lost the marks and wrote
///   them again from a copy in which the key had no record, and it has
///   signed nothing here since): no mark names its last record or carries
///   that the goal was a copy of unknown age;
/// - for an agent's key on a member's machine, a restore with the marks
///   lost (residual 5: the host's computer may lack that agent's last
///   records while another member holds them);
/// - for any agent's key, a restore with the marks kept in a run where a
///   member was removed after the backup was taken (residual 5: the record
///   may have reached only the removed computer, and is given up once every
///   other computer has answered);
/// - for an agent's key on a member's machine, a restore with the marks
///   kept whose copy predates an admission that the host's computer held
///   and then lost to a restore of its own (the fourth case of risk (5):
///   the member's copy does not wait for the computer it does not know, and
///   no computer in the know could answer for it, so the give-up lowers the
///   mark).
///
/// It stays claimed for the governance key whatever the marks and the
/// members admitted or removed since, and for the host's agent whenever
/// the marks were lost, since that key is held with the governance key.
fn claimed(r: &Run, m: usize, governance: bool, records: &BTreeSet<EventId>) -> bool {
    let next = |at: Micros| {
        r.w.restores
            .iter()
            .find(|restore| restore.m == m && restore.at > at)
            .map(|restore| &restore.before)
    };
    let signed_after = |at: Micros, before: &BTreeSet<EventId>| {
        let until = next(at);
        records
            .iter()
            .any(|id| !before.contains(id) && until.is_none_or(|until| until.contains(id)))
    };
    let continued = r
        .continued
        .iter()
        .any(|continued| continued.m == m && signed_after(continued.at, &continued.before));
    let excluded =
        r.w.restores
            .iter()
            .filter(|restore| restore.m == m && signed_after(restore.at, &restore.before))
            .any(|restore| {
                !governance
                    && match restore.marks {
                        Marks::Lost => m != HOST,
                        Marks::Kept => {
                            r.removals.iter().any(|at| *at > restore.taken)
                                || (m != HOST
                                    && (host_missed_the_same_admission(r, restore)
                                        || !restore.marked.contains(&(r.goal(), r.principals[m]))))
                        }
                    }
            });
    !continued && !excluded
}

/// The fourth case of risk (5): `restore` put a member's computer back with
/// its marks kept to a copy that predates an admission, and the host's
/// computer, which held that admission, was itself restored to a copy
/// without it. The member's copy does not wait for the computer it does not
/// know, and no computer in the know could answer for it, so the give-up
/// lowers the mark. A host copy without the admission whose restore came
/// before the admission existed is no such case: the host learns of the
/// admission again and can answer for the admitted member.
pub(super) fn host_missed_the_same_admission(r: &Run, restore: &Restore) -> bool {
    let goal = r.goal();
    let admissions: BTreeSet<EventId> =
        r.w.machines
            .iter()
            .flat_map(|machine| {
                machine
                    .store
                    .log(&goal, 0, usize::MAX)
                    .expect("a memory store reads")
            })
            .filter(|(_, event)| matches!(event.header().body, Body::MemberAdmitted { .. }))
            .map(|(_, event)| event.id())
            .collect();
    admissions.iter().any(|admission| {
        !restore.copy.contains(admission)
            && r.w.restores.iter().any(|host| {
                host.m == HOST && host.before.contains(admission) && !host.copy.contains(admission)
            })
    })
}

/// One broken invariant.
#[derive(Clone, Debug)]
pub struct Violation {
    pub text: String,
    /// It follows from a position signed again outside the claims: the
    /// reused key's own halt, or records that stand on its reused records
    /// (and the deliveries of their effects) not being effective. Anything
    /// else, such as a record missing somewhere, never does.
    pub forked: bool,
}

/// What a fork outside the claims explains: each record that stands on a
/// reused record (the key's records from the reused position on, then every
/// record whose previous record, typed dependency or effect evidence is one
/// of these, or that acknowledges the delivery of an effect one of these
/// materializes), those effects, and the reused keys.
struct Forked {
    keys: BTreeSet<PublicKey>,
    events: BTreeSet<EventId>,
    effects: BTreeSet<EffectId>,
}

impl Forked {
    fn of(r: &Run) -> Self {
        let mut from: BTreeMap<PublicKey, u64> = BTreeMap::new();
        for reuse in reuses(r).into_iter().filter(|reuse| !reuse.claimed) {
            let seq = from.entry(reuse.key).or_insert(reuse.seq);
            *seq = (*seq).min(reuse.seq);
        }
        let goal = r.goal();
        let mut all: BTreeMap<EventId, Event> = BTreeMap::new();
        for machine in &r.w.machines {
            let log = machine.store.log(&goal, 0, usize::MAX);
            for (_, event) in log.expect("a memory store reads") {
                all.entry(event.id()).or_insert(event);
            }
        }
        let mut events: BTreeSet<EventId> = all
            .iter()
            .filter(|(_, event)| {
                let header = event.header();
                from.get(&header.author)
                    .is_some_and(|seq| header.seq >= *seq)
            })
            .map(|(id, _)| *id)
            .collect();
        let mut effects = BTreeSet::new();
        loop {
            effects.extend(events.iter().filter_map(|id| match &all[id].header().body {
                Body::EffectMaterialized { effect } => Some(effect.id(goal)),
                _ => None,
            }));
            let more: Vec<EventId> = all
                .iter()
                .filter(|(id, event)| {
                    !events.contains(*id)
                        && (stands_on(event).iter().any(|on| events.contains(on))
                            || matches!(
                                &event.header().body,
                                Body::DeliveryAcknowledged { effect } if effects.contains(effect)
                            ))
                })
                .map(|(id, _)| *id)
                .collect();
            if more.is_empty() {
                break;
            }
            events.extend(more);
        }
        Self {
            keys: from.into_keys().collect(),
            events,
            effects,
        }
    }
}

/// The records `event` stands on: its author's previous record, its typed
/// dependencies, and for a materialized effect its evidence and subject.
fn stands_on(event: &Event) -> Vec<EventId> {
    let header = event.header();
    let mut on: Vec<EventId> = header.prev.into_iter().collect();
    on.extend(header.body.dependencies());
    if let Body::EffectMaterialized { effect } = &header.body {
        on.extend(effect.evidence.iter().copied());
        if let EffectAction::RequestReview { subject, .. } = effect.action {
            on.push(subject);
        }
    }
    on
}

/// Every broken invariant. Empty when all hold. Reads through the local API
/// as each machine's principal, and the stores for event sets. `deep` also
/// reads the artifact's bytes back instead of asking whether it is held,
/// which is slow enough to do once.
pub fn violations(r: &mut Run, deep: bool) -> Vec<Violation> {
    let mut bad = Vec::new();
    let goal = r.goal();
    let forked = Forked::of(r);
    // The violations that follow from the fork outside the claims.
    let mut excused = Vec::new();
    let everyone: BTreeSet<_> = r.principals.iter().copied().collect();
    let reference = held(r, 0);
    // A record only a restored machine held is held nowhere now, and so is
    // a text only it held.
    let lost = r.w.lost();
    let lost_blobs = r.w.lost_blobs();
    let text_lost = |id: &EventId| {
        r.w.machines.iter().any(|machine| {
            let event = machine.store.event(id).expect("a memory store reads");
            event.is_some_and(|event| {
                event
                    .header()
                    .blobs()
                    .iter()
                    .any(|hash| lost_blobs.contains(hash))
            })
        })
    };
    let expected_notes: BTreeMap<EventId, Option<String>> = r
        .findings
        .iter()
        .filter(|(id, _)| !lost.contains(id))
        .map(|(id, text)| (*id, Some(text.clone()).filter(|_| !text_lost(id))))
        .collect();
    let task_text = match r.task {
        Some(TaskId::Authored(id)) if text_lost(&id) => None,
        _ => Some(TASK_TEXT),
    };
    let result_text = Some(RESULT_TEXT).filter(|_| !r.result.is_some_and(|id| text_lost(&id)));
    let mut boards = Vec::new();
    let mut heads = Vec::new();
    for m in 0..r.w.machines.len() {
        let name = r.w.machines[m].name.clone();
        if !r.w.machines[m].running() {
            bad.push(format!("{name} is not running"));
            continue;
        }
        let events = held(r, m);
        if events != reference {
            bad.push(format!(
                "{name} holds {} events and m1 holds {}",
                events.len(),
                reference.len()
            ));
        }
        for acked in r.acked.iter().filter(|acked| !lost.contains(&acked.event)) {
            if !events.contains(&acked.event) {
                bad.push(format!(
                    "{name} lacks the {} acknowledged on m{}",
                    acked.what,
                    acked.machine + 1
                ));
            }
        }
        match r.read(m, Request::GoalStatus { goal }) {
            Some(Response::GoalStatus(status)) => {
                let members: BTreeSet<_> = status.members.iter().map(|v| v.member).collect();
                if members != everyone {
                    bad.push(format!("{name} lists {} members", members.len()));
                }
                for view in &status.members {
                    let at = r.principals.iter().position(|p| *p == view.member);
                    let right = at.map(|at| (r.w.machines[at].endpoint, at == m));
                    if right != Some((view.endpoint, view.local)) {
                        bad.push(format!("{name} binds a member wrongly: {view:?}"));
                    }
                }
                if status.title.as_deref() != Some(TITLE) {
                    bad.push(format!("{name} shows the title {:?}", status.title));
                }
                if let Some(halt) = status.halted {
                    bad.push(format!("{name} reports the goal halted: {halt:?}"));
                }
                if !status.guard.is_empty() {
                    bad.push(format!("{name} is still catching up: {:?}", status.guard));
                }
                if status.host != Some(r.principals[0]) {
                    bad.push(format!("{name} names another host"));
                }
                heads.push(status.governance_head);
            }
            other => bad.push(format!("{name} cannot show the goal: {other:?}")),
        }
        match r.read(m, Request::Status) {
            Some(Response::Status(status)) => {
                let entry = status.goals.iter().find(|summary| summary.goal == goal);
                let fine = entry.is_some_and(|summary| {
                    summary.membership == Membership::Member && summary.halted.is_none()
                });
                if !fine {
                    let text = format!("{name} status lists the goal as {entry:?}");
                    let halted_by_the_fork = entry.is_some_and(|summary| {
                        summary.membership == Membership::Member
                            && summary.halted == Some(Halt::SignerConflict)
                            && forked.keys.contains(&summary.member)
                    });
                    if halted_by_the_fork {
                        excused.push(text);
                    } else {
                        bad.push(text);
                    }
                }
            }
            other => bad.push(format!("{name} cannot show its status: {other:?}")),
        }
        let findings = r.finding_views(m).map(|findings| {
            findings
                .into_iter()
                .map(|view| (view.contribution, view.text))
                .collect::<BTreeMap<_, _>>()
        });
        if findings.as_ref() != Some(&expected_notes) {
            let text = format!("{name} shows findings {findings:?}, expected {expected_notes:?}");
            let only_forked = findings.as_ref().is_some_and(|found| {
                found
                    .keys()
                    .chain(expected_notes.keys())
                    .filter(|id| found.get(*id) != expected_notes.get(*id))
                    .all(|id| forked.events.contains(id))
            });
            if only_forked {
                excused.push(text);
            } else {
                bad.push(text);
            }
        }
        if let (Some(task), Some(result)) = (r.task, r.result) {
            match r.read(m, Request::Task { goal, task }) {
                Some(Response::Task(detail)) if detail.text.as_deref() == task_text => {}
                other => bad.push(format!("{name} cannot read the task text: {other:?}")),
            }
            if r.event_text(m, result).as_deref() != result_text {
                bad.push(format!("{name} cannot read the result text"));
            }
            let board = r.board(m).unwrap_or_default();
            let done = board.iter().any(|view| {
                view.task == task
                    && view.completed
                    && r.attempt
                        .is_some_and(|attempt| view.attempts.contains(&attempt))
                    && view.selected == Some(result)
            });
            if !done {
                bad.push(format!(
                    "{name} board does not show the reviewed and selected task: {board:?}"
                ));
            }
            boards.push(board);
        }
        let artifact = r.artifact.as_ref().map(|(hash, _)| *hash);
        if let Some(hash) = artifact.filter(|hash| !lost_blobs.contains(hash)) {
            let hashes = vec![hash];
            let held = match r.read(m, Request::BlobStat { goal, hashes }) {
                Some(Response::BlobStates(states)) => {
                    states.iter().all(|status| status.state == BlobState::Held)
                }
                _ => false,
            };
            // BlobGet explicitly records a want; BlobStat is a read-only inventory.
            let fetched = r.read(m, Request::BlobGet { goal, hash });
            let same = !deep
                || match fetched {
                    Some(Response::Blob { bytes }) => {
                        r.artifact.as_ref().is_some_and(|(_, sent)| *sent == bytes)
                    }
                    _ => false,
                };
            if !held || !same {
                bad.push(format!("{name} cannot read the artifact (held {held})"));
            }
        }
        let acked: Vec<_> = r
            .acked
            .iter()
            .filter(|acked| !lost.contains(&acked.event))
            .map(|acked| (acked.what, acked.event))
            .collect();
        for (what, event) in acked {
            if !matches!(
                r.read(m, Request::Event { goal, event }),
                Some(Response::Event(_))
            ) {
                bad.push(format!("{name} cannot show the acknowledged {what}"));
            }
        }
        let feed = Request::Events {
            goal,
            after: Some(0),
            limit: 256,
        };
        match r.read(m, feed) {
            Some(Response::Events(views)) => {
                for view in views.iter().filter(|v| v.standing != Standing::Effective) {
                    let text = format!(
                        "{name} holds a {} event that is {:?}",
                        view.kind, view.standing
                    );
                    if forked.events.contains(&view.event) {
                        excused.push(text);
                    } else {
                        bad.push(text);
                    }
                }
                let listed: BTreeSet<_> = views.iter().map(|view| view.event).collect();
                if listed != events {
                    bad.push(format!("{name} feed does not cover its stored event set"));
                }
                if views
                    .windows(2)
                    .any(|pair| pair[0].position >= pair[1].position)
                {
                    bad.push(format!("{name} feed positions do not advance"));
                }
            }
            other => bad.push(format!("{name} cannot list events: {other:?}")),
        }
        // Excused, a delivery of an effect that stands on the fork, or a
        // review of a record that does, is left waiting.
        let settled = |work: &PendingWork, excused: bool| {
            work.ask_first.is_empty()
                && work.to_start.is_empty()
                && work.claimed.is_empty()
                && work.held_elsewhere.is_empty()
                && work.to_acknowledge.is_empty()
                && work.deliveries.iter().all(|delivery| {
                    delivery.acknowledged || (excused && forked.effects.contains(&delivery.effect))
                })
                && work.to_review.iter().all(|item| {
                    expected_notes.contains_key(&item.subject)
                        || (excused && forked.events.contains(&item.subject))
                })
        };
        match r.read(m, Request::Pending { goal }) {
            Some(Response::Pending(work)) if settled(&work, false) => {}
            Some(Response::Pending(work)) if settled(&work, true) => {
                excused.push(format!("{name} still has pending work: {work:?}"));
            }
            other => bad.push(format!("{name} still has pending work: {other:?}")),
        }
    }
    if boards.windows(2).any(|pair| pair[0] != pair[1]) {
        bad.push(format!("boards differ between machines: {boards:?}"));
    }
    if heads.windows(2).any(|pair| pair[0] != pair[1]) {
        bad.push(format!(
            "governance heads differ between machines: {heads:?}"
        ));
    }
    claims(r, &mut bad);
    for reuse in reuses(r).into_iter().filter(|reuse| reuse.claimed) {
        bad.push(format!(
            "a_restored_machine_signs_at_no_used_position_unless_its_owner_continued: {}",
            reuse.describe(r)
        ));
    }
    let plain = bad.into_iter().map(|text| Violation {
        text,
        forked: false,
    });
    let explained = excused
        .into_iter()
        .map(|text| Violation { text, forked: true });
    plain.chain(explained).collect()
}

/// The violations in words, for a failure, and whether every one follows
/// from the fork outside the claims.
fn failure(violations: &[Violation]) -> (String, bool) {
    let text: Vec<&str> = violations.iter().map(|v| v.text.as_str()).collect();
    let forked = !violations.is_empty() && violations.iter().all(|v| v.forked);
    (text.join("; "), forked)
}

/// Claims and generations: the claim lives only where it was taken, at the
/// generation its holder was told, and a finished attempt can neither be
/// claimed again nor written to.
fn claims(r: &mut Run, bad: &mut Vec<String>) {
    let (Some(attempt), Some(generation)) = (r.attempt, r.generation) else {
        return;
    };
    let goal = r.goal();
    for m in 0..r.w.machines.len() {
        let Some(node) = r.w.machines[m].node.as_ref() else {
            continue;
        };
        let record = node
            .goals
            .get(&goal)
            .and_then(|entry| entry.claims.get(&attempt));
        let found = record.map(|claim| claim.generation);
        let expected = (m == 1).then_some(generation);
        if found != expected {
            bad.push(format!(
                "m{} holds claim generation {found:?}, expected {expected:?}",
                m + 1
            ));
        }
    }
    let claim = Request::AttemptTakeover { goal, attempt };
    let progress = Request::AttemptReport {
        goal,
        attempt,
        status: locust_proto::event::AttemptStatus::Progress,
        generation,
        text: "after the end".into(),
    };
    for (session, request) in [
        (0, claim.clone()),
        (1, claim),
        (0, progress.clone()),
        (1, progress),
    ] {
        if !r.w.machines[1].running() {
            break;
        }
        let name = request.name();
        match r.w.call(1, Who::Session(session), request) {
            Err(error) if matches!(error.code, ErrorCode::Superseded | ErrorCode::Conflict) => {}
            other => bad.push(format!("{name} on the finished attempt answered {other:?}")),
        }
    }
}

/// Ends every fault, runs until every invariant holds, then confirms that
/// nothing changes for two more anti-entropy periods. Returns how long
/// after the last fault ended the invariants first held.
pub fn settle(r: &mut Run) -> Result<Micros, Fail> {
    r.step = "quiesce";
    r.w.calm();
    for m in 0..r.w.machines.len() {
        r.release(m);
    }
    let healed = r.w.now;
    let mut looked = 0;
    loop {
        // Review requests remain durable until the intended local recipient
        // explicitly acknowledges them, including requests whose review is done.
        r.tend();
        for m in 0..r.w.machines.len() {
            let goal = r.goal();
            if let Some(Response::Pending(work)) = r.read(m, Request::Pending { goal }) {
                for delivery in work
                    .deliveries
                    .iter()
                    .filter(|delivery| !delivery.acknowledged)
                {
                    r.attempt_write(
                        m,
                        "delivery acknowledgement",
                        Request::DeliveryAcknowledge {
                            goal,
                            effect: delivery.effect,
                        },
                    )?;
                }
            }
        }
        // The stores are compared every second; the full reading through
        // the API follows once they agree, then every few seconds.
        if same_events(r) && (looked == 0 || r.w.now >= looked + 5 * SEC) {
            looked = r.w.now;
            if violations(r, false).is_empty() {
                break;
            }
        }
        if r.w.now - healed >= PATIENCE {
            let (text, forked) = failure(&violations(r, false));
            return Err(Fail {
                step: r.step,
                what: format!(
                    "not quiet {} s after the last fault ended: {text}",
                    PATIENCE / SEC
                ),
                forked,
            });
        }
        r.w.run_for(SEC);
    }
    let converged = r.w.now - healed;
    let before: Vec<_> = (0..r.w.machines.len()).map(|m| held(r, m)).collect();
    r.w.run_for((2 * ANTI_ENTROPY_MS + 10_000) * 1_000);
    let after: Vec<_> = (0..r.w.machines.len()).map(|m| held(r, m)).collect();
    if before != after {
        return r.fail("events changed after the network was quiet");
    }
    let later = violations(r, true);
    if !later.is_empty() {
        let (text, forked) = failure(&later);
        return Err(Fail {
            step: r.step,
            what: format!("an invariant broke while quiet: {text}"),
            forked,
        });
    }
    Ok(converged)
}
