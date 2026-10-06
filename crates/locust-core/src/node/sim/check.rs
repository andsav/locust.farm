//! What must hold once faults have stopped and the network has gone quiet.

use std::collections::{BTreeMap, BTreeSet};

use locust_proto::api::{BlobState, ErrorCode, Membership, Request, Response, Standing};
use locust_proto::id::EventId;
use locust_proto::store::Store;

use super::machine::Who;
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

/// Every broken invariant, in words. Empty when all hold. Reads through the
/// local API as each machine's principal, and the stores for event sets.
/// `deep` also reads the artifact's bytes back instead of asking whether it
/// is held, which is slow enough to do once.
pub fn violations(r: &mut Run, deep: bool) -> Vec<String> {
    let mut bad = Vec::new();
    let goal = r.goal();
    let everyone: BTreeSet<_> = r.principals.iter().copied().collect();
    let reference = held(r, 0);
    let expected_notes: BTreeMap<EventId, Option<String>> = r
        .findings
        .iter()
        .map(|(id, text)| (*id, Some(text.clone())))
        .collect();
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
        for acked in &r.acked {
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
                    bad.push(format!("{name} status lists the goal as {entry:?}"));
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
            bad.push(format!(
                "{name} shows findings {findings:?}, expected {expected_notes:?}"
            ));
        }
        if let (Some(task), Some(result)) = (r.task, r.result) {
            match r.read(m, Request::Task { goal, task }) {
                Some(Response::Task(detail)) if detail.text.as_deref() == Some(TASK_TEXT) => {}
                other => bad.push(format!("{name} cannot read the task text: {other:?}")),
            }
            if r.event_text(m, result).as_deref() != Some(RESULT_TEXT) {
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
        if let Some(hash) = r.artifact.as_ref().map(|(hash, _)| *hash) {
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
                    bad.push(format!(
                        "{name} holds a {} event that is {:?}",
                        view.kind, view.standing
                    ));
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
        match r.read(m, Request::Pending { goal }) {
            Some(Response::Pending(work))
                if work.ask_first.is_empty()
                    && work.to_start.is_empty()
                    && work.claimed.is_empty()
                    && work.held_elsewhere.is_empty()
                    && work.to_acknowledge.is_empty()
                    && work.deliveries.iter().all(|delivery| delivery.acknowledged)
                    && work
                        .to_review
                        .iter()
                        .all(|item| expected_notes.contains_key(&item.subject)) => {}
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
    bad
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
    let mut last = Vec::new();
    let mut looked = 0;
    loop {
        // Review requests remain durable until the intended local recipient
        // explicitly acknowledges them, including requests whose review is done.
        for m in 0..r.w.machines.len() {
            let goal = r.goal();
            if let Some(Response::Pending(work)) = r.read(m, Request::Pending { goal }) {
                for delivery in work
                    .deliveries
                    .iter()
                    .filter(|delivery| !delivery.acknowledged)
                {
                    r.record(
                        m,
                        Who::Agent,
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
            last = violations(r, false);
            if last.is_empty() {
                break;
            }
        }
        if r.w.now - healed >= PATIENCE {
            if last.is_empty() {
                last = violations(r, false);
            }
            return r.fail(format!(
                "not quiet {} s after the last fault ended: {}",
                PATIENCE / SEC,
                last.join("; ")
            ));
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
        return r.fail(format!(
            "an invariant broke while quiet: {}",
            later.join("; ")
        ));
    }
    Ok(converged)
}
