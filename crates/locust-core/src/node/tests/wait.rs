//! Waiting against a goal's revision, a counter that never falls.
use super::authorization::join_local;
use super::lifecycle::{finding, setup};
use super::*;
use locust_proto::api::{ResponseFrame, WaitOutcome};
use locust_proto::id::GoalId;

fn revision(daemon: &mut Daemon, conn: ConnId, goal: GoalId) -> u64 {
    let Response::Pending(work) = daemon.ok(conn, Request::Pending { goal }) else {
        panic!()
    };
    work.revision
}

fn wait(daemon: &mut Daemon, conn: ConnId, goal: GoalId, seen: u64, timeout_ms: u32) -> Step {
    let frame = daemon.frame(Request::Wait {
        goal,
        seen,
        timeout_ms,
    });
    daemon.step(conn, frame, 100)
}

#[test]
fn a_seen_ahead_of_the_revision_is_refused_and_names_the_revision() {
    let (mut d, _, _, a, goal) = setup();
    let current = revision(&mut d, a, goal);
    for (seen, timeout_ms) in [(current + 1, 0), (current + 1, 1000), (u64::MAX, 1000)] {
        let Step::Reply(reply) = wait(&mut d, a, goal, seen, timeout_ms) else {
            panic!("a wait ahead of the revision parked")
        };
        let error = reply.result.unwrap_err();
        assert_eq!(error.code, ErrorCode::Invalid);
        assert!(
            error.message.contains(&format!("revision {current}")),
            "{}",
            error.message
        );
    }
    assert!(d.node.conns[&a].parked.is_none());
    assert_eq!(revision(&mut d, a, goal), current);
    // What the caller could have seen is answered as before.
    assert!(matches!(
        wait(&mut d, a, goal, 0, 1000),
        Step::Reply(ResponseFrame {
            result: Ok(Response::Waited(WaitOutcome::Work(_))),
            ..
        })
    ));
    assert!(matches!(
        wait(&mut d, a, goal, current, 0),
        Step::Reply(ResponseFrame {
            result: Ok(Response::Waited(WaitOutcome::NoEvent)),
            ..
        })
    ));
    assert!(matches!(
        wait(&mut d, a, goal, current, 1000),
        Step::Park(_)
    ));
}

/// The refusal above is only right while this holds: every revision a caller
/// was given stays at or below the goal's revision for the life of the store.
#[test]
fn the_revision_never_falls_across_restart_leaving_removal_and_rejoining() {
    let (mut d, _, _, a, goal) = setup();
    let (member, m) = join_local(&mut d, a, goal, 2);
    let mut given = vec![revision(&mut d, m, goal)];
    let mut observe = |d: &mut Daemon, conn: ConnId| {
        let now = revision(d, conn, goal);
        assert!(now >= *given.last().unwrap(), "{now} after {given:?}");
        for seen in &given {
            assert!(
                matches!(
                    wait(d, conn, goal, *seen, 0),
                    Step::Reply(ResponseFrame { result: Ok(_), .. })
                ),
                "seen {seen} was refused at revision {now}"
            );
        }
        given.push(now);
        now
    };
    let before = observe(&mut d, m);
    d.ok(a, finding(goal, "changed"));
    assert!(observe(&mut d, m) > before);

    d.restart();
    let a = d.connect(credential(1), Some(session(1)));
    let m = d.connect(credential(2), None);
    observe(&mut d, a);
    observe(&mut d, m);

    let owner = d.owner();
    d.ok(
        owner,
        Request::GoalLeave {
            goal,
            agent: member,
        },
    );
    observe(&mut d, m);
    d.ok(owner, Request::MemberRemove { goal, member });
    observe(&mut d, m);
    d.restart();
    let m = d.connect(credential(2), None);
    observe(&mut d, m);
    let owner = d.owner();

    let Response::Invited { ticket } = d.ok(
        owner,
        Request::GoalInvite {
            goal,
            expires_ms: 604_801_000,
        },
    ) else {
        panic!()
    };
    d.ok(
        owner,
        Request::GoalJoin {
            agent: member,
            ticket,
        },
    );
    observe(&mut d, m);
    d.restart();
    let m = d.connect(credential(2), None);
    observe(&mut d, m);
}
