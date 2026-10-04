//! Concrete Engine traces for the bounded Sessions model. All identities and
//! storage are synthetic; no production credential, daemon or network is used.

use super::lifecycle::{authorize, offered, setup};
use super::*;
use locust_proto::event::AttemptStatus;
use locust_proto::id::{EventId, GoalId};
use locust_proto::store::Store;

fn progress(goal: GoalId, attempt: EventId, generation: u32, text: &str) -> Request {
    Request::AttemptReport {
        goal,
        attempt,
        generation,
        status: AttemptStatus::Progress,
        text: text.into(),
    }
}

#[test]
fn tla_sessions_aba_delayed_write_and_idempotent_retry_survive_reopen() {
    // Sessions.WitnessABADelayedReject: authorize -> claim A1 -> delay A1 ->
    // takeover B2 -> takeover A3 -> reject A1. A committed keyed write also
    // demonstrates that retry may return old success without signing again.
    let (mut daemon, principal, owner, a, goal) = setup();
    let (task, offer) = offered(&mut daemon, a, goal, principal);
    authorize(&mut daemon, owner, goal, task, principal);
    let Response::Claimed(first) = daemon.ok(
        a,
        Request::AttemptStart {
            goal,
            task,
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    let attempt = first.attempt;
    assert_eq!(first.generation, 1);
    let committed = progress(goal, attempt, 1, "committed before takeover");
    let recorded = daemon.keyed(a, 61, committed.clone()).unwrap();
    let mut delayed = daemon.frame(progress(goal, attempt, 1, "delayed A1"));
    delayed.idempotency = Some(IdempotencyKey([62; 16]));
    let b = daemon.connect(credential(1), Some(session(2)));
    let Response::Claimed(second) = daemon.ok(b, Request::AttemptTakeover { goal, attempt }) else {
        panic!()
    };
    assert_eq!(second.generation, 2);
    let Response::Claimed(third) = daemon.ok(a, Request::AttemptTakeover { goal, attempt }) else {
        panic!()
    };
    assert_eq!(third.generation, 3);
    assert_eq!(first.instance, third.instance);
    let before = daemon.store.log(&goal, 0, 256).unwrap();
    assert_eq!(code(daemon.send(a, delayed.clone())), ErrorCode::Superseded);
    assert_eq!(
        code(daemon.call(b, progress(goal, attempt, 2, "delayed B2"))),
        ErrorCode::Superseded
    );
    assert_eq!(daemon.keyed(a, 61, committed.clone()).unwrap(), recorded);
    assert_eq!(daemon.store.log(&goal, 0, 256).unwrap(), before);

    daemon.restart();
    let a = daemon.connect(credential(1), Some(session(1)));
    let Response::Claimed(recovered) = daemon.ok(
        a,
        Request::AttemptStart {
            goal,
            task,
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    assert_eq!(recovered, third);
    assert_eq!(code(daemon.send(a, delayed)), ErrorCode::Superseded);
    assert_eq!(daemon.keyed(a, 61, committed).unwrap(), recorded);
    assert_eq!(daemon.store.log(&goal, 0, 256).unwrap(), before);

    // A refused delayed request did not reserve its idempotency key. The
    // current generation may commit once, and its retry authors no duplicate.
    let current = progress(goal, attempt, 3, "current A3");
    let fresh = daemon.keyed(a, 62, current.clone()).unwrap();
    assert_ne!(fresh, recorded);
    assert_eq!(daemon.keyed(a, 62, current).unwrap(), fresh);
    assert_eq!(
        daemon.store.log(&goal, 0, 256).unwrap().len(),
        before.len() + 1
    );
}
