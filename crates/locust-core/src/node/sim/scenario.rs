//! Three machines through the current local API: create a coordinator
//! organization, invite and join, open and offer work, authorize and start
//! an attempt, publish a contribution, review and select it, then exchange
//! findings offline, restart, sleep and wake, and add another participant.

use locust_proto::api::{ErrorCode, Membership, Request, Response};
use locust_proto::event::{ReviewVerdict, TaskId};
use locust_proto::id::{BlobHash, EventId, GoalId};
use locust_proto::limits::BLOB_CHUNK_BYTES;
use std::collections::BTreeMap;

use super::machine::Who;
use super::run::{Fail, Run};
use super::world::SEC;

pub const TITLE: &str = "Three Mac T1";
pub const TASK_TEXT: &str = "Return the text: T1 task completed.";
pub const RESULT_TEXT: &str = "T1 task completed.";

const M1: usize = 0;
const M2: usize = 1;
const M3: usize = 2;

/// Starts every daemon and enrolls one principal per machine.
pub fn setup(r: &mut Run) -> Result<(), Fail> {
    r.step = "setup";
    for m in 0..r.w.machines.len() {
        r.w.start(m);
        let request = Request::AgentEnroll {
            name: r.w.machines[m].name.clone(),
            credential: r.w.machines[m].agent.digest(),
        };
        let Response::AgentEnrolled { agent } = r.op(m, Who::Owner, request)? else {
            return r.fail("enroll: unexpected answer");
        };
        r.w.machines[m].principal = Some(agent);
        r.principals.push(agent);
        identity(r, m)?;
    }
    Ok(())
}

/// `status` shows the endpoint identity and the principal of the machine.
fn identity(r: &mut Run, m: usize) -> Result<(), Fail> {
    let Response::Status(status) = r.now(m, Who::Agent, Request::Status)? else {
        return r.fail("status: unexpected answer");
    };
    let machine = &r.w.machines[m];
    let agents: Vec<_> = status.agents.iter().map(|a| (a.agent, a.revoked)).collect();
    if status.endpoint != Some(machine.endpoint) || agents != [(r.principals[m], false)] {
        return r.fail(format!("m{} status lost its identity: {status:?}", m + 1));
    }
    Ok(())
}

pub fn finding(r: &mut Run, m: usize, text: &str) -> Result<EventId, Fail> {
    let request = Request::ContributionPublish {
        goal: r.goal(),

        attempt: None,
        generation: None,
        summary: text.into(),
        sources: Vec::new(),
        artifacts: Vec::new(),
    };
    let event = r.record(m, Who::Agent, "finding", request)?;
    r.findings.push((event, text.into()));
    if !r.shows_finding(m, event, text) {
        return r.fail(format!("m{} cannot read back its own finding", m + 1));
    }
    Ok(event)
}

/// What a restart must preserve, as the guide's step 4 lists it.
fn durable_view(r: &mut Run, m: usize) -> Result<String, Fail> {
    let goal = r.goal();
    identity(r, m)?;
    let Response::GoalStatus(mut status) = r.now(m, Who::Agent, Request::GoalStatus { goal })?
    else {
        return r.fail("goal status: unexpected answer");
    };
    // Which peers are connected, which computers the guard heard from and
    // whether the data was restored are what a restart may change.
    status.peers.clear();
    status.restored = None;
    for view in &mut status.guard {
        view.heard.clear();
        view.waiting.clear();
    }
    let board = r.now(m, Who::Agent, Request::Board { goal })?;
    let findings = r.now(m, Who::Agent, Request::Contributions { goal, task: None })?;
    Ok(format!("{status:?}\n{board:?}\n{findings:?}"))
}

/// Stops and starts the daemon of `m` over its store and compares what it
/// shows before and after. No simulated time passes in between.
fn restart(r: &mut Run, m: usize) -> Result<(), Fail> {
    r.up(m)?;
    r.w.chaos_point();
    r.up(m)?;
    let before = durable_view(r, m)?;
    r.w.stop(m, true);
    r.w.start(m);
    let after = durable_view(r, m)?;
    if before != after {
        return r.fail(format!(
            "m{} shows something else after a restart:\nbefore {before}\nafter  {after}",
            m + 1
        ));
    }
    Ok(())
}

/// The coordinator invites, machine `m` joins, and the first `expect`
/// machines come to list each other and the title.
pub fn join(r: &mut Run, m: usize, expect: usize) -> Result<(), Fail> {
    let goal = r.goal();
    let request = Request::GoalInvite {
        role: None,
        goal,
        expires_ms: r.w.wall_ms(M1) + 7 * 24 * 60 * 60 * 1_000,
    };
    let Response::Invited { ticket } = r.op(M1, Who::Owner, request)? else {
        return r.fail("invite: unexpected answer");
    };
    match r.op(
        m,
        Who::Owner,
        Request::GoalJoin {
            name: "member".into(),
            agent: r.principals[m],
            ticket,
            level: locust_proto::api::Level::Auto,
        },
    )? {
        Response::Joined {
            goal: joined,
            membership: Membership::Joining | Membership::Member,
            ..
        } if joined == goal => {}
        other => return r.fail(format!("join: unexpected answer {other:?}")),
    }
    r.wait("every member to list every principal and the title", |r| {
        (0..expect).all(|m| r.members(m) == Some(expect))
    })?;
    // A key just admitted signs once its computer has heard from the host's.
    r.wait("the new member to hear from the host's computer", |r| {
        r.signs(m)
    })?;
    set_ask(r, m)
}

fn task_state(r: &mut Run, m: usize) -> Option<(bool, Vec<EventId>, Option<EventId>)> {
    let task = r.task?;
    let board = r.board(m)?;
    let view = board.iter().find(|view| view.task == task)?;
    Some((view.completed, view.attempts.clone(), view.selected))
}

fn set_ask(r: &mut Run, m: usize) -> Result<(), Fail> {
    r.op(
        m,
        Who::Owner,
        Request::LevelSet {
            goal: r.goal(),
            agent: r.principals[m],
            level: locust_proto::api::Level::Ask,
        },
    )?;
    Ok(())
}

/// Open, offer, authorize, start, publish, review and select one task. Some
/// seeds have a second session of the assignee take the claim over first.
fn task(r: &mut Run) -> Result<(), Fail> {
    let goal = r.goal();
    r.step = "open and offer";
    let open = Request::TaskOpen {
        goal,
        text: TASK_TEXT.into(),
        task_type: None,
        inputs: BTreeMap::new(),
        parent: None,
    };
    let task = TaskId::Authored(r.record(M1, Who::Agent, "task", open)?);
    r.task = Some(task);
    let offer = r.record(
        M1,
        Who::Agent,
        "offer",
        Request::WorkOffer {
            goal,
            task,
            recipient: r.principals[M2],
        },
    )?;

    r.step = "authorize and start";
    r.wait("m2 to show the offered work", |r| {
        matches!(r.read(M2, Request::Pending {goal}), Some(Response::Pending(work))
            if work.ask_first.iter().any(|item| item.task == task))
    })?;
    let takeover = r.rng.chance(1, 3);
    r.op(
        M2,
        Who::Owner,
        Request::TaskAllow {
            goal,
            task,
            agent: r.principals[M2],
        },
    )?;
    let claim = Request::AttemptStart {
        goal,
        task: Some(task),
        offer: Some(offer),
    };
    let Response::Claimed(first) = r.op(M2, Who::Session(0), claim.clone())? else {
        return r.fail("claim: unexpected answer");
    };
    let attempt = first.attempt;
    r.attempt = Some(attempt);
    if first.generation != 1 || first.task != task {
        return r.fail(format!("the first claim is {first:?}"));
    }
    // Retrying after a lost answer must give the same claim back.
    let Response::Claimed(again) = r.op(M2, Who::Session(0), claim.clone())? else {
        return r.fail("claim: unexpected answer");
    };
    if again != first {
        return r.fail(format!(
            "a repeated claim changed: {first:?} then {again:?}"
        ));
    }
    // A quarter of the seeds attach an artifact of two chunks, so that
    // content moves in more than one frame and can be cut part-way.
    let mut artifacts = Vec::new();
    if r.rng.chance(1, 4) {
        let mut bytes = vec![0u8; BLOB_CHUNK_BYTES + r.rng.range(1, 100_000) as usize];
        r.rng.fill(&mut bytes);
        let put = Request::BlobPut {
            goal,
            bytes: bytes.clone(),
        };
        let Response::BlobStored { hash } = r.op(M2, Who::Agent, put)? else {
            return r.fail("blob put: unexpected answer");
        };
        r.artifact = Some((hash, bytes));
        artifacts.push(hash);
    }
    let mut holder = 0;
    let mut generation = first.generation;
    if takeover {
        r.step = "take the claim over";
        let request = Request::AttemptTakeover { goal, attempt };
        let Response::Claimed(taken) = r.op(M2, Who::Session(1), request)? else {
            return r.fail("takeover: unexpected answer");
        };
        if taken.generation != generation + 1 || taken.instance == first.instance {
            return r.fail(format!("takeover of {first:?} gave {taken:?}"));
        }
        // The fenced session's write and its claim are refused, and leave
        // nothing behind (checked on every error by `World::call`).
        let stale = submit(goal, attempt, generation, artifacts.clone());
        match r.ask(M2, Who::Session(0), stale)? {
            Err(error) if error.code == ErrorCode::Superseded => {}
            other => return r.fail(format!("a fenced submit answered {other:?}")),
        }
        match r.ask(M2, Who::Session(0), claim)? {
            Err(error) if matches!(error.code, ErrorCode::ClaimHeld | ErrorCode::Conflict) => {}
            other => return r.fail(format!("a fenced claim answered {other:?}")),
        }
        holder = 1;
        generation = taken.generation;
    }

    r.step = "submit";
    let request = submit(goal, attempt, generation, artifacts);
    let result = r.record(M2, Who::Session(holder), "result", request)?;
    r.result = Some(result);
    r.generation = Some(generation);
    r.record(
        M2,
        Who::Session(holder),
        "attempt completed",
        Request::AttemptReport {
            goal,
            attempt,
            generation,
            status: locust_proto::event::AttemptStatus::Completed,
            text: "Execution finished".into(),
        },
    )?;

    r.step = "review and select";
    r.wait("m1 to show the submitted text", |r| {
        r.event_text(M1, result).as_deref() == Some(RESULT_TEXT)
    })?;
    r.record(
        M1,
        Who::Agent,
        "review",
        Request::ReviewRecord {
            goal,
            subject: result,
            verdict: ReviewVerdict::Approve,
            text: "Verified task output".into(),
        },
    )?;
    r.record(
        M1,
        Who::Agent,
        "selection",
        Request::ScopeSelect {
            goal,
            subject: result,
            expected: None,
        },
    )?;
    r.wait(
        "both boards to show the reviewed and selected contribution",
        |r| {
            [M1, M2].into_iter().all(|m| {
                task_state(r, m) == Some((true, vec![attempt], Some(result)))
                    && r.event_text(m, result).as_deref() == Some(RESULT_TEXT)
            })
        },
    )
}

fn submit(goal: GoalId, attempt: EventId, generation: u32, artifacts: Vec<BlobHash>) -> Request {
    Request::ContributionPublish {
        goal,
        attempt: Some(attempt),
        generation: Some(generation),
        summary: RESULT_TEXT.into(),
        sources: Vec::new(),
        artifacts,
    }
}

pub fn create(r: &mut Run) -> Result<(), Fail> {
    r.step = "create the goal";
    let create = Request::GoalCreate {
        name: "host".into(),
        agent: r.principals[M1],
        title: TITLE.into(),
        formation_json: Some(
            serde_json::to_string(
                &locust_proto::organization::presets()
                    .into_iter()
                    .find(|preset| preset.name == "directed")
                    .unwrap()
                    .formation,
            )
            .unwrap(),
        ),

        inputs: BTreeMap::new(),
    };
    let Response::GoalCreated { goal } = r.op(M1, Who::Owner, create)? else {
        return r.fail("create: unexpected answer");
    };
    r.goal = Some(goal);
    set_ask(r, M1)?;
    r.w.ready[M1] = true;
    Ok(())
}

/// The whole guide. Faults, when enabled, land between and during steps.
pub fn t1(r: &mut Run) -> Result<(), Fail> {
    create(r)?;
    r.step = "m2 joins";
    join(r, M2, 2)?;
    r.w.backups();
    task(r)?;
    // From here on a copy of m2 holds its claim at its last generation.
    r.w.ready[M2] = true;
    r.w.backups();

    r.step = "write while m1 is offline";
    r.hold_stopped(M1)?;
    let offline = finding(r, M2, "M2 wrote this while M1 was offline")?;
    r.release(M1);
    r.wait("m1 to show the finding written while it was offline", |r| {
        r.shows_finding(M1, offline, "M2 wrote this while M1 was offline")
    })?;

    r.step = "restart m2, then m1";
    restart(r, M2)?;
    restart(r, M1)?;

    r.step = "sleep and wake m2";
    r.hold_asleep(M2)?;
    let during = finding(r, M1, "M1 wrote this while M2 was asleep")?;
    // Longer than every timeout: backoff tops out at a minute.
    let asleep = match r.rng.below(3) {
        0 => r.rng.range(95, 300),
        1 => r.rng.range(300, 1_800),
        _ => r.rng.range(1_800, 14_400),
    };
    r.w.run_for(asleep * SEC);
    r.release(M2);
    r.wait("m2 to show the finding written while it slept", |r| {
        r.shows_finding(M2, during, "M1 wrote this while M2 was asleep")
    })?;

    r.w.backups();
    let restored = r.w.restore_point();

    r.step = "m3 joins";
    join(r, M3, 3)?;
    if let Some(m) = restored {
        // The guide signs with it next, without the coordinator.
        r.step = "catch up after a restore";
        r.wait("the restored machine to sign again", |r| r.signs(m))?;
    }
    r.w.ready[M3] = true;
    r.w.backups();
    let (attempt, result) = (r.attempt, r.result);
    r.wait(
        "m3 to show the earlier task, its result and its text",
        |r| {
            task_state(r, M3) == Some((true, attempt.into_iter().collect(), result))
                && result.is_some_and(|id| r.event_text(M3, id).as_deref() == Some(RESULT_TEXT))
        },
    )?;

    r.step = "exchange without the coordinator";
    r.hold_stopped(M1)?;
    r.up(M3)?;
    r.w.stop(M3, true);
    r.w.start(M3);
    let from_m2 = finding(r, M2, "M2 to M3 without the coordinator")?;
    let from_m3 = finding(r, M3, "M3 to M2 without the coordinator")?;
    r.wait("m2 and m3 to show each other's findings without m1", |r| {
        r.shows_finding(M2, from_m3, "M3 to M2 without the coordinator")
            && r.shows_finding(M3, from_m2, "M2 to M3 without the coordinator")
    })?;
    r.release(M1);
    r.wait("m1 to catch up with both findings", |r| {
        r.shows_finding(M1, from_m2, "M2 to M3 without the coordinator")
            && r.shows_finding(M1, from_m3, "M3 to M2 without the coordinator")
    })?;
    r.step = "restart m3 once more";
    restart(r, M3)
}
