//! Finite lifecycle experiments using the existing deterministic simulator.
use super::{machine::Who, run::Run, scenario, world::SEC};
use locust_proto::{
    api::{Membership, Request, Response},
    store::Store,
};

fn joined(seed: u64) -> Run {
    let mut r = Run::new(seed, 3);
    r.faults = false;
    r.w.net.stall = 0;
    scenario::setup(&mut r).unwrap();
    scenario::create(&mut r).unwrap();
    scenario::join(&mut r, 1, 2).unwrap();
    scenario::join(&mut r, 2, 3).unwrap();
    r.w.run_for(60 * SEC);
    r
}

#[test]
fn offline_removed_member_retries_refused_peers_and_fresh_ticket_returns_stale_membership() {
    let mut r = joined(8101);
    let goal = r.goal();
    let member = r.principals[2];
    r.w.stop(2, true);
    let removal = r
        .record(
            0,
            Who::Owner,
            "remove",
            Request::MemberRemove { goal, member },
        )
        .unwrap();
    r.w.run_for(120 * SEC);
    assert!(r.w.machines[1].store.event(&removal).unwrap().is_some());
    let Response::Invited { ticket } = r
        .op(
            0,
            Who::Owner,
            Request::GoalInvite {
                goal,
                expires_ms: r.w.wall_ms(0) + 7 * 24 * 60 * 60 * 1_000,
            },
        )
        .unwrap()
    else {
        panic!()
    };
    r.w.start(2);
    r.w.log = Some(vec![]);
    let before = r.w.net.streams.len();
    r.w.run_for(600 * SEC);
    let mut attempts = [0; 2];
    for stream in &r.w.net.streams[before..] {
        if stream.sides[0].m == 2 {
            attempts[stream.sides[1].m] += 1;
        }
    }
    let refused =
        r.w.log
            .as_ref()
            .unwrap()
            .iter()
            .filter(|s| s.contains("m3]") && s.contains("Refused("))
            .count();
    eprintln!("removed member: attempts={attempts:?}, refusals={refused}");
    assert!(attempts.iter().all(|n| *n > 2));
    assert_eq!(refused, attempts.iter().sum::<usize>());
    assert!(
        r.w.log
            .as_ref()
            .unwrap()
            .iter()
            .filter(|s| s.contains("m3]") && s.contains("Refused("))
            .all(|s| s.contains("NotAMember"))
    );
    assert!(r.w.machines[2].store.event(&removal).unwrap().is_none());
    assert_eq!(r.members(2), Some(3));
    let count = r.w.machines[0]
        .store
        .log(&goal, 0, usize::MAX)
        .unwrap()
        .len();
    assert!(matches!(
        r.op(
            2,
            Who::Owner,
            Request::GoalJoin {
                agent: member,
                ticket
            }
        )
        .unwrap(),
        Response::Joined {
            membership: Membership::Member,
            ..
        }
    ));
    r.w.run_for(120 * SEC);
    assert_eq!(
        r.w.machines[0]
            .store
            .log(&goal, 0, usize::MAX)
            .unwrap()
            .len(),
        count
    );
    assert!(!r.w.machines[0].node.as_ref().unwrap().goals[&goal].is_member(&member));
}

#[test]
fn leave_is_visible_in_events_and_replication_and_rotated_keys_continue_until_removal() {
    let mut r = joined(8102);
    let goal = r.goal();
    let member = r.principals[1];
    let leave = r
        .record(
            1,
            Who::Owner,
            "leave",
            Request::GoalLeave {
                goal,
                agent: member,
            },
        )
        .unwrap();
    r.w.run_for(60 * SEC);
    let Response::Events(events) = r
        .op(
            0,
            Who::Owner,
            Request::Events {
                goal,
                after: None,
                limit: 256,
            },
        )
        .unwrap()
    else {
        panic!()
    };
    assert!(events.iter().any(|event| event.event == leave));
    let Response::Event(detail) = r
        .op(0, Who::Owner, Request::Event { goal, event: leave })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(detail.view.author, member);
    assert!(matches!(
        detail.body,
        locust_proto::event::Body::LeaveRequested { .. }
    ));
    assert!(
        r.w.machines[0].node.as_ref().unwrap().goals[&goal]
            .state()
            .leave_requests
            .contains(&leave)
    );
    assert_eq!(r.members(0), Some(3));
    let other = r.principals[2];
    r.record(
        0,
        Who::Owner,
        "rotate",
        Request::MemberRemove {
            goal,
            member: other,
        },
    )
    .unwrap();
    let finding = scenario::finding(&mut r, 0, "after leave and rotation").unwrap();
    r.w.run_for(60 * SEC);
    assert!(r.w.machines[1].store.event(&finding).unwrap().is_some());
    assert!(
        r.w.machines[1].node.as_ref().unwrap().goals[&goal]
            .keys
            .contains_key(&1)
    );
    assert!(r.shows_finding(1, finding, "after leave and rotation"));
    r.record(
        0,
        Who::Owner,
        "remove leaver",
        Request::MemberRemove { goal, member },
    )
    .unwrap();
    let later = scenario::finding(&mut r, 0, "after removal").unwrap();
    r.w.run_for(120 * SEC);
    assert!(r.w.machines[1].store.event(&later).unwrap().is_none());
    assert!(
        !r.w.machines[1].node.as_ref().unwrap().goals[&goal]
            .keys
            .contains_key(&2)
    );
}

#[test]
fn idle_three_member_goal_keeps_exchanging_for_a_simulated_hour() {
    let mut r = joined(8103);
    let goal = r.goal();
    let logs: Vec<_> =
        r.w.machines
            .iter()
            .map(|m| m.store.log(&goal, 0, usize::MAX).unwrap().len())
            .collect();
    let before = r.w.net.streams.len();
    let requests = r.w.stats.requests;
    let mut counts = [[0usize; 3]; 3];
    let mut times = [
        [Vec::new(), Vec::new(), Vec::new()],
        [Vec::new(), Vec::new(), Vec::new()],
        [Vec::new(), Vec::new(), Vec::new()],
    ];
    let mut cursor = before;
    for second in 1..=3600 {
        r.w.run_for(SEC);
        for stream in &r.w.net.streams[cursor..] {
            let a = stream.sides[0].m;
            let b = stream.sides[1].m;
            counts[a][b] += 1;
            times[a][b].push(second);
        }
        cursor = r.w.net.streams.len();
    }
    eprintln!(
        "idle hour: ordered-pair exchanges={counts:?}, total={}",
        cursor - before
    );
    assert_eq!(r.w.stats.requests, requests);
    for a in 0..3 {
        assert_eq!(
            r.w.machines[a]
                .store
                .log(&goal, 0, usize::MAX)
                .unwrap()
                .len(),
            logs[a]
        );
        for b in 0..3 {
            if a == b {
                continue;
            }
            assert_eq!(counts[a][b], 120, "{counts:?}");
            let gaps: Vec<_> = times[a][b].windows(2).map(|w| w[1] - w[0]).collect();
            eprintln!(
                "{a}->{b}: min={}s max={}s",
                gaps.iter().min().unwrap(),
                gaps.iter().max().unwrap()
            );
            assert!(gaps.iter().all(|gap| *gap == 30));
            assert!(*times[a][b].last().unwrap() > 3560);
        }
    }
}
