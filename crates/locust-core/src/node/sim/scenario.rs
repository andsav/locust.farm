//! The sequence of `docs/t1-run.md` for three machines, through the local
//! API only: found a goal, invite, join, propose and assign a task,
//! authorize, claim, submit, accept, write while the coordinator is
//! offline, restart each daemon, sleep and wake a laptop, add the third
//! participant, exchange without the coordinator.

use locust_proto::api::{ErrorCode, Grants, Membership, Request, Response, TaskState};
use locust_proto::id::{BlobHash, EventId, GoalId};
use locust_proto::limits::BLOB_CHUNK_BYTES;

use super::machine::Who;
use super::run::{Fail, Run};
use super::world::SEC;

pub const TITLE: &str = "Three Mac T1";
pub const TASK_TEXT: &str = "Return the text: T1 task completed.";
pub const RESULT_TEXT: &str = "T1 task completed.";

const M1: usize = 0;
const M2: usize = 1;
const M3: usize = 2;

/// Starts every daemon and enrolls one principal per machine with
/// `--manage-goals`, as the guide's setup does.
pub fn setup(r: &mut Run) -> Result<(), Fail> {
    r.step = "setup";
    for m in 0..r.w.machines.len() {
        r.w.start(m);
        let request = Request::AgentEnroll {
            name: r.w.machines[m].name.clone(),
            grants: Grants { manage_goals: true },
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

pub fn note(r: &mut Run, m: usize, text: &str) -> Result<EventId, Fail> {
    let request = Request::NoteAdd {
        goal: r.goal(),
        about: None,
        supersedes: None,
        text: text.into(),
    };
    let event = r.record(m, Who::Agent, "note", request)?;
    r.notes.push((event, text.into()));
    if !r.shows_note(m, event, text) {
        return r.fail(format!("m{} cannot read back its own note", m + 1));
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
    // Which peers are connected is the one thing a restart may change.
    status.peers.clear();
    let board = r.now(m, Who::Agent, Request::Board { goal })?;
    let notes = r.now(m, Who::Agent, Request::Notes { goal, about: None })?;
    Ok(format!("{status:?}\n{board:?}\n{notes:?}"))
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
        goal,
        expires_ms: None,
    };
    let Response::Invited { ticket } = r.op(M1, Who::Agent, request)? else {
        return r.fail("invite: unexpected answer");
    };
    match r.op(m, Who::Agent, Request::GoalJoin { ticket })? {
        Response::Joined {
            goal: joined,
            membership: Membership::Joining | Membership::Member,
            ..
        } if joined == goal => {}
        other => return r.fail(format!("join: unexpected answer {other:?}")),
    }
    r.wait("every member to list every principal and the title", |r| {
        (0..expect).all(|m| r.members(m) == Some(expect))
    })
}

fn task_state(r: &mut Run, m: usize) -> Option<(TaskState, Option<EventId>, Option<EventId>)> {
    let task = r.task?;
    let board = r.board(m)?;
    let view = board.iter().find(|view| view.task == task)?;
    Some((view.state, view.assignment, view.result))
}

/// Propose, assign, authorize, claim, submit and accept one task. Some
/// seeds have a second session of the assignee take the claim over first.
fn task(r: &mut Run) -> Result<(), Fail> {
    let goal = r.goal();
    r.step = "propose and assign";
    let propose = Request::TaskPropose {
        goal,
        text: TASK_TEXT.into(),
        input: None,
        depends_on: Vec::new(),
        deadline_ms: None,
        max_attempts: None,
    };
    let task = r.record(M1, Who::Agent, "task", propose)?;
    r.task = Some(task);
    let assign = Request::TaskAssign {
        goal,
        task,
        assignee: r.principals[M2],
    };
    let assignment = r.record(M1, Who::Agent, "assignment", assign)?;
    r.assignment = Some(assignment);

    r.step = "authorize and claim";
    r.wait("m2's board to show the assignment", |r| {
        task_state(r, M2).is_some_and(|(_, held, _)| held == Some(assignment))
    })?;
    let takeover = r.rng.chance(1, 3);
    let authorize = Request::TaskAuthorize {
        goal,
        assignment,
        takeover,
    };
    r.op(M2, Who::Owner, authorize)?;
    let claim = Request::TaskClaim { goal, assignment };
    let Response::Claimed(first) = r.op(M2, Who::Session(0), claim.clone())? else {
        return r.fail("claim: unexpected answer");
    };
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
        let request = Request::TaskTakeover { goal, assignment };
        let Response::Claimed(taken) = r.op(M2, Who::Session(1), request)? else {
            return r.fail("takeover: unexpected answer");
        };
        if taken.generation != generation + 1 || taken.instance == first.instance {
            return r.fail(format!("takeover of {first:?} gave {taken:?}"));
        }
        // The fenced session's write and its claim are refused, and leave
        // nothing behind (checked on every error by `World::call`).
        let stale = submit(goal, assignment, generation, artifacts.clone());
        match r.ask(M2, Who::Session(0), stale)? {
            Err(error) if error.code == ErrorCode::Superseded => {}
            other => return r.fail(format!("a fenced submit answered {other:?}")),
        }
        match r.ask(M2, Who::Session(0), claim)? {
            Err(error) if error.code == ErrorCode::ClaimHeld => {}
            other => return r.fail(format!("a fenced claim answered {other:?}")),
        }
        holder = 1;
        generation = taken.generation;
    }

    r.step = "submit";
    let request = submit(goal, assignment, generation, artifacts);
    let result = r.record(M2, Who::Session(holder), "result", request)?;
    r.result = Some(result);
    r.generation = Some(generation);

    r.step = "accept";
    r.wait("m1 to show the submitted text", |r| {
        r.event_text(M1, result).as_deref() == Some(RESULT_TEXT)
    })?;
    let accept = Request::ResultAccept {
        goal,
        result,
        head: None,
    };
    r.record(M1, Who::Agent, "acceptance", accept)?;
    let Response::Pending(work) = r.op(M1, Who::Agent, Request::Pending { goal })? else {
        return r.fail("pending: unexpected answer");
    };
    if !work.to_review.is_empty() {
        return r.fail(format!("m1 still has work to review: {work:?}"));
    }
    r.wait(
        "both boards to show the accepted result and its text",
        |r| {
            [M1, M2].into_iter().all(|m| {
                task_state(r, m) == Some((TaskState::Accepted, Some(assignment), Some(result)))
                    && r.event_text(m, result).as_deref() == Some(RESULT_TEXT)
            })
        },
    )
}

fn submit(goal: GoalId, assignment: EventId, generation: u32, artifacts: Vec<BlobHash>) -> Request {
    Request::TaskSubmit {
        goal,
        assignment,
        generation,
        summary: RESULT_TEXT.into(),
        base: None,
        patch: None,
        artifacts,
    }
}

pub fn create(r: &mut Run) -> Result<(), Fail> {
    r.step = "create the goal";
    let create = Request::GoalCreate {
        title: TITLE.into(),
    };
    let Response::GoalCreated { goal } = r.op(M1, Who::Agent, create)? else {
        return r.fail("create: unexpected answer");
    };
    r.goal = Some(goal);
    Ok(())
}

/// The whole guide. Faults, when enabled, land between and during steps.
pub fn t1(r: &mut Run) -> Result<(), Fail> {
    create(r)?;
    r.step = "m2 joins";
    join(r, M2, 2)?;
    task(r)?;

    r.step = "write while m1 is offline";
    r.hold_stopped(M1)?;
    let offline = note(r, M2, "M2 wrote this while M1 was offline")?;
    r.release(M1);
    r.wait("m1 to show the note written while it was offline", |r| {
        r.shows_note(M1, offline, "M2 wrote this while M1 was offline")
    })?;

    r.step = "restart m2, then m1";
    restart(r, M2)?;
    restart(r, M1)?;

    r.step = "sleep and wake m2";
    r.hold_asleep(M2)?;
    let during = note(r, M1, "M1 wrote this while M2 was asleep")?;
    // Longer than every timeout: backoff tops out at a minute.
    let asleep = match r.rng.below(3) {
        0 => r.rng.range(95, 300),
        1 => r.rng.range(300, 1_800),
        _ => r.rng.range(1_800, 14_400),
    };
    r.w.run_for(asleep * SEC);
    r.release(M2);
    r.wait("m2 to show the note written while it slept", |r| {
        r.shows_note(M2, during, "M1 wrote this while M2 was asleep")
    })?;

    r.step = "m3 joins";
    join(r, M3, 3)?;
    let (assignment, result) = (r.assignment, r.result);
    r.wait(
        "m3 to show the earlier task, its result and its text",
        |r| {
            task_state(r, M3) == Some((TaskState::Accepted, assignment, result))
                && result.is_some_and(|id| r.event_text(M3, id).as_deref() == Some(RESULT_TEXT))
        },
    )?;

    r.step = "exchange without the coordinator";
    r.hold_stopped(M1)?;
    r.up(M3)?;
    r.w.stop(M3, true);
    r.w.start(M3);
    let from_m2 = note(r, M2, "M2 to M3 without the coordinator")?;
    let from_m3 = note(r, M3, "M3 to M2 without the coordinator")?;
    r.wait("m2 and m3 to show each other's notes without m1", |r| {
        r.shows_note(M2, from_m3, "M3 to M2 without the coordinator")
            && r.shows_note(M3, from_m2, "M2 to M3 without the coordinator")
    })?;
    r.release(M1);
    r.wait("m1 to catch up with both notes", |r| {
        r.shows_note(M1, from_m2, "M2 to M3 without the coordinator")
            && r.shows_note(M1, from_m3, "M3 to M2 without the coordinator")
    })?;
    r.step = "restart m3 once more";
    restart(r, M3)
}
