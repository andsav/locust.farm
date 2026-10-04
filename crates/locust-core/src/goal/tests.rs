//! Regressions over the public held-event interface.
use super::{Goal, Halt, Standing, TaskState};
use locust_proto::event::{AuthorPoint, Body, Event};
use locust_proto::id::EndpointId;
use locust_proto::store::{Commit, MemStore, Store};
use locust_proto::testkit::Author;

fn transcript() -> (Author, Author, Vec<Event>) {
    let mut coordinator = Author::new(1);
    let mut worker = Author::new(2);
    let (genesis, own) = coordinator.found_goal(EndpointId([1; 32]));
    let goal = genesis.header().goal;
    let admission = coordinator.event(
        goal,
        Some(own.id()),
        Body::MemberAdmitted {
            member: worker.key.public(),
            endpoint: EndpointId([2; 32]),
        },
    );
    let proposal = coordinator.event(
        goal,
        Some(admission.id()),
        Body::TaskProposed {
            input: None,
            depends_on: vec![],
            deadline_ms: None,
            max_attempts: None,
        },
    );
    let assignment = coordinator.event(
        goal,
        Some(admission.id()),
        Body::TaskAssigned {
            task: proposal.id(),
            assignee: worker.key.public(),
            attempt: 1,
        },
    );
    let take = worker.event(
        goal,
        Some(assignment.id()),
        Body::AssignmentAccepted {
            assignment: assignment.id(),
        },
    );
    let result = worker.event(
        goal,
        Some(assignment.id()),
        Body::ResultSubmitted {
            assignment: assignment.id(),
            base: None,
            patch: None,
            artifacts: vec![],
        },
    );
    (
        coordinator,
        worker,
        vec![genesis, own, admission, proposal, assignment, take, result],
    )
}

fn held(events: &[Event]) -> Goal {
    let mut goal = Goal::new(events[0].header().goal);
    goal.apply(events);
    goal
}

fn assert_same(a: &Goal, b: &Goal, events: &[Event]) {
    assert_eq!(a.state(), b.state());
    assert_eq!(a.halt(), b.halt());
    assert_eq!(a.frontier(), b.frontier());
    for event in events {
        assert_eq!(
            a.standing(&event.id()),
            b.standing(&event.id()),
            "{}",
            event.header().body.kind()
        );
    }
}

#[test]
fn duplicate_admission_cannot_rebind_an_active_member() {
    let (mut author, _, mut events) = transcript();
    let goal = events[0].header().goal;
    let duplicate = author.event(
        goal,
        Some(events[4].id()),
        Body::MemberAdmitted {
            member: author.key.public(),
            endpoint: EndpointId([9; 32]),
        },
    );
    events.push(duplicate.clone());
    let goal = held(&events);
    assert_eq!(
        goal.state().members[&author.key.public()],
        EndpointId([1; 32])
    );
    assert!(matches!(
        goal.standing(&duplicate.id()),
        Some(Standing::Excluded(_))
    ));
}

#[test]
fn removal_permanently_fences_open_assignment_even_after_readmission() {
    let (mut author, worker, mut events) = transcript();
    let goal_id = events[0].header().goal;
    let result = events[6].clone();
    let remove = author.event(
        goal_id,
        Some(events[4].id()),
        Body::MemberRemoved {
            member: worker.key.public(),
            last_accepted: Some(AuthorPoint {
                seq: result.header().seq,
                id: result.id(),
            }),
        },
    );
    let accept = author.event(
        goal_id,
        Some(remove.id()),
        Body::ResultAccepted {
            result: result.id(),
            head: None,
        },
    );
    events.extend([remove, accept.clone()]);
    let goal = held(&events);
    assert!(goal.state().assignment(&events[4].id()).unwrap().revoked);
    assert_eq!(goal.state().tasks[0].accepted, None);
    assert_eq!(goal.standing(&result.id()), Some(Standing::Effective));
    assert!(matches!(
        goal.standing(&accept.id()),
        Some(Standing::Excluded(_))
    ));
    let readmit = author.event(
        goal_id,
        Some(accept.id()),
        Body::MemberAdmitted {
            member: worker.key.public(),
            endpoint: EndpointId([3; 32]),
        },
    );
    let resurrect = author.event(
        goal_id,
        Some(readmit.id()),
        Body::ResultAccepted {
            result: result.id(),
            head: None,
        },
    );
    events.extend([readmit, resurrect.clone()]);
    let goal = held(&events);
    assert!(goal.state().is_member(&worker.key.public()));
    assert_eq!(goal.state().tasks[0].accepted, None);
    assert!(matches!(
        goal.standing(&resurrect.id()),
        Some(Standing::Excluded(_))
    ));
}

#[test]
fn progress_after_submission_keeps_work_submitted() {
    let (_, mut worker, mut events) = transcript();
    let progress = worker.event(
        events[0].header().goal,
        Some(events[4].id()),
        Body::Progress {
            assignment: events[4].id(),
        },
    );
    events.push(progress);
    let goal = held(&events);
    assert_eq!(goal.state().tasks[0].state, TaskState::Submitted);
    assert_eq!(goal.state().tasks[0].result, Some(events[6].id()));
}

#[test]
fn first_fork_evidence_survives_a_full_waiting_set_in_both_orders() {
    for fork_first in [false, true] {
        let mut author = Author::new(1);
        let (genesis, admission) = author.found_goal(EndpointId([1; 32]));
        let id = genesis.header().goal;
        let mut header = admission.header().clone();
        header.at_ms += 99;
        let fork = Event::sign(header, &author.key).unwrap();
        let mut goal = held(&[
            genesis,
            if fork_first {
                fork.clone()
            } else {
                admission.clone()
            },
        ]);
        let _missing = author.event(
            id,
            Some(admission.id()),
            Body::Note {
                about: None,
                supersedes: None,
            },
        );
        let backlog = (0..super::MAX_WAITING_PER_AUTHOR)
            .map(|_| {
                author.event(
                    id,
                    Some(admission.id()),
                    Body::Note {
                        about: None,
                        supersedes: None,
                    },
                )
            })
            .collect();
        let kept = goal.screen(backlog);
        assert_eq!(kept.len(), super::MAX_WAITING_PER_AUTHOR);
        goal.apply(&kept);
        let kept = goal.screen(vec![if fork_first { admission } else { fork }]);
        assert_eq!(kept.len(), 1);
        goal.apply(&kept);
        assert!(matches!(goal.halt(), Some(Halt::Fork { seq:1, events }) if events.len() == 2));
    }
}

#[test]
fn arrival_permutations_append_and_restart_match_full_replay() {
    let (mut coordinator, _, mut events) = transcript();
    let accept = coordinator.event(
        events[0].header().goal,
        Some(events[4].id()),
        Body::ResultAccepted {
            result: events[6].id(),
            head: None,
        },
    );
    events.push(accept);
    let expected = held(&events);
    let id = expected.id();
    let mut append = Goal::new(id);
    for event in &events {
        append.apply(std::slice::from_ref(event));
        let fresh = super::fold::fold(&append.history).0;
        assert_eq!(append.folded, fresh);
    }
    assert_same(&expected, &append, &events);
    for seed in 0..32u64 {
        let mut order = events.clone();
        let mut random = seed + 1;
        for i in (1..order.len()).rev() {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            order.swap(i, random as usize % (i + 1));
        }
        let mut actual = Goal::new(id);
        let mut store = MemStore::new();
        for event in order {
            store
                .commit(&Commit {
                    events: vec![event.clone()],
                    ..Commit::default()
                })
                .unwrap();
            actual.apply(&[event]);
            assert_eq!(actual.folded, super::fold::fold(&actual.history).0);
        }
        assert_same(&expected, &actual, &events);
        assert_same(
            &expected,
            &Goal::load(&store.reopen(), id).unwrap(),
            &events,
        );
    }
}

#[test]
fn missing_referenced_contribution_stalls_then_unblocks_decisions() {
    let (_, _, events) = transcript();
    let mut goal = held(&events[..3]);
    goal.apply(&events[4..]);
    assert_eq!(goal.state().head, Some(events[2].id()));
    assert!(goal.next(&events[0].header().author).is_none());
    let changes = goal.apply(&events[3..4]);
    assert!(changes.refolded);
    assert_eq!(goal.state().tasks[0].state, TaskState::Submitted);
    assert!(changes.judged.contains(&events[4].id()));
}

#[test]
fn removal_readmission_and_fork_replay_are_independent_of_arrival_order() {
    let (mut author, mut worker, mut events) = transcript();
    let id = events[0].header().goal;
    let remove = author.event(
        id,
        Some(events[4].id()),
        Body::MemberRemoved {
            member: worker.key.public(),
            last_accepted: None,
        },
    );
    let readmit = author.event(
        id,
        Some(remove.id()),
        Body::MemberAdmitted {
            member: worker.key.public(),
            endpoint: EndpointId([4; 32]),
        },
    );
    let assign = author.event(
        id,
        Some(readmit.id()),
        Body::TaskAssigned {
            task: events[3].id(),
            assignee: worker.key.public(),
            attempt: 2,
        },
    );
    let take = worker.event(
        id,
        Some(assign.id()),
        Body::AssignmentAccepted {
            assignment: assign.id(),
        },
    );
    let submit = worker.event(
        id,
        Some(assign.id()),
        Body::ResultSubmitted {
            assignment: assign.id(),
            base: None,
            patch: None,
            artifacts: vec![],
        },
    );
    let accept = author.event(
        id,
        Some(assign.id()),
        Body::ResultAccepted {
            result: submit.id(),
            head: None,
        },
    );
    events.extend([remove, readmit, assign, take, submit, accept]);
    for with_fork in [false, true] {
        let mut events = events.clone();
        if with_fork {
            let mut header = events[2].header().clone();
            header.at_ms += 100;
            events.push(Event::sign(header, &author.key).unwrap());
        }
        let expected = held(&events);
        if !with_fork {
            assert_eq!(expected.state().tasks[0].state, TaskState::Accepted);
        }
        for seed in 1..65u64 {
            let mut order = events.clone();
            let mut random = seed;
            for i in (1..order.len()).rev() {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                order.swap(i, random as usize % (i + 1));
            }
            let mut actual = Goal::new(id);
            for event in order {
                actual.apply(&[event]);
                assert_eq!(actual.folded, super::fold::fold(&actual.history).0);
            }
            assert_same(&expected, &actual, &events);
        }
    }
}
