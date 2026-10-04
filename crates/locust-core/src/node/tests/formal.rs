//! Concrete Engine traces for the bounded Sessions model. All identities and
//! storage are synthetic; no production credential, daemon or network is used.

use super::lifecycle::{event, setup};
use super::*;
use locust_proto::id::{EventId, GoalId};
use locust_proto::store::Store;

fn progress(goal: GoalId, assignment: EventId, generation: u32, text: &str) -> Request {
    Request::TaskProgress {
        goal,
        assignment,
        generation,
        text: text.into(),
    }
}

#[test]
fn tla_sessions_aba_delayed_write_and_idempotent_retry_survive_reopen() {
    // Sessions.WitnessABADelayedReject: authorize -> claim A1 -> delay A1 ->
    // takeover B2 -> takeover A3 -> reject A1. A committed keyed write also
    // demonstrates that retry may return old success without signing again.
    let (mut daemon, principal, owner, a, goal) = setup();
    let task = event(daemon.ok(
        a,
        Request::TaskPropose {
            goal,
            text: "Sessions model trace".into(),
            input: None,
            depends_on: vec![],
            deadline_ms: None,
            max_attempts: Some(1),
        },
    ));
    let assignment = event(daemon.ok(
        a,
        Request::TaskAssign {
            goal,
            task,
            assignee: principal,
        },
    ));
    daemon.ok(
        owner,
        Request::TaskAuthorize {
            goal,
            assignment,
            takeover: true,
        },
    );
    let Response::Claimed(first) = daemon.ok(a, Request::TaskClaim { goal, assignment }) else {
        panic!()
    };
    assert_eq!(first.generation, 1);
    let committed = progress(goal, assignment, 1, "committed before takeover");
    let recorded = daemon.keyed(a, 61, committed.clone()).unwrap();
    let mut delayed = daemon.frame(progress(goal, assignment, 1, "delayed A1"));
    delayed.idempotency = Some(IdempotencyKey([62; 16]));
    let b = daemon.connect(credential(1), Some(session(2)));
    let Response::Claimed(second) = daemon.ok(b, Request::TaskTakeover { goal, assignment }) else {
        panic!()
    };
    assert_eq!(second.generation, 2);
    let Response::Claimed(third) = daemon.ok(a, Request::TaskTakeover { goal, assignment }) else {
        panic!()
    };
    assert_eq!(third.generation, 3);
    assert_eq!(first.instance, third.instance);
    let before = daemon.store.log(&goal, 0, 256).unwrap();
    assert_eq!(code(daemon.send(a, delayed.clone())), ErrorCode::Superseded);
    assert_eq!(
        code(daemon.call(b, progress(goal, assignment, 2, "delayed B2"))),
        ErrorCode::Superseded
    );
    assert_eq!(daemon.keyed(a, 61, committed.clone()).unwrap(), recorded);
    assert_eq!(daemon.store.log(&goal, 0, 256).unwrap(), before);

    daemon.restart();
    let a = daemon.connect(credential(1), Some(session(1)));
    let Response::Claimed(recovered) = daemon.ok(a, Request::TaskClaim { goal, assignment }) else {
        panic!()
    };
    assert_eq!(recovered, third);
    assert_eq!(code(daemon.send(a, delayed)), ErrorCode::Superseded);
    assert_eq!(daemon.keyed(a, 61, committed).unwrap(), recorded);
    assert_eq!(daemon.store.log(&goal, 0, 256).unwrap(), before);

    // A refused delayed request did not reserve its idempotency key. The
    // current generation may commit once, and its retry authors no duplicate.
    let current = progress(goal, assignment, 3, "current A3");
    let fresh = daemon.keyed(a, 62, current.clone()).unwrap();
    assert_ne!(fresh, recorded);
    assert_eq!(daemon.keyed(a, 62, current).unwrap(), fresh);
    assert_eq!(
        daemon.store.log(&goal, 0, 256).unwrap().len(),
        before.len() + 1
    );
}
