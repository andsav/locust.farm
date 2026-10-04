//! Regressions over the public held-event interface.
use super::{Exclusion, Goal, Halt, Standing, TaskState};
use locust_proto::event::{AuthorPoint, Body, Event};
use locust_proto::id::{BlobHash, EndpointId};
use locust_proto::store::{Commit, MemStore, Store};
use locust_proto::testkit::Author;

fn transcript() -> (Author, Author, Vec<Event>) {
    transcript_with_budget(None)
}

fn transcript_with_budget(max_attempts: Option<u32>) -> (Author, Author, Vec<Event>) {
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
            max_attempts,
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

/// Concrete replay of GoalLog's IR12 symbolic events 1..9. These signed
/// contributions enter through the held-event seam: the local task.submit API
/// already refuses submission after acceptance, but peers may send it.
#[test]
fn tla_ir12_later_submission_changes_display_without_changing_acceptance() {
    let (mut coordinator, mut worker, mut events) = transcript_with_budget(Some(2));
    let goal_id = events[0].header().goal;
    let task = events[3].id();
    let assignment = events[4].id();
    let accepted_result = events[6].id();
    let accept = coordinator.event(
        goal_id,
        Some(assignment),
        Body::ResultAccepted {
            result: accepted_result,
            head: Some(BlobHash([8; 32])),
        },
    );
    events.push(accept.clone());
    let mut chronological = held(&events);
    assert_eq!(
        chronological.state().task(&task).unwrap().result,
        Some(accepted_result)
    );
    let later = worker.event(
        goal_id,
        Some(accept.id()),
        Body::ResultSubmitted {
            assignment,
            base: None,
            patch: None,
            artifacts: vec![],
        },
    );
    events.push(later.clone());
    chronological.apply(std::slice::from_ref(&later));
    let view = chronological.state().task(&task).unwrap();
    assert_eq!(view.state, TaskState::Accepted);
    assert_eq!(view.accepted, Some(accepted_result));
    // Current behavior, not a proposed accepted-result display guarantee.
    assert_eq!(view.result, Some(later.id()));
    assert_eq!(view.accepted_head, Some(BlobHash([8; 32])));
    assert_eq!(
        chronological.standing(&later.id()),
        Some(Standing::Effective)
    );

    // Acceptance and the later submission arrive before their prerequisites;
    // duplicates are harmless, and the incremental view agrees at every prefix.
    let mut scrambled = Goal::new(goal_id);
    let mut store = MemStore::new();
    for index in [7, 8, 6, 4, 8, 3, 5, 2, 1, 0, 7] {
        let event = events[index].clone();
        store
            .commit(&Commit {
                events: vec![event.clone()],
                ..Commit::default()
            })
            .unwrap();
        scrambled.apply(&[event]);
        assert_eq!(scrambled.folded, super::fold::fold(&scrambled.history).0);
    }
    assert_same(&chronological, &scrambled, &events);
    assert_same(
        &chronological,
        &Goal::load(&store.reopen(), goal_id).unwrap(),
        &events,
    );
}

/// Concrete replay of GoalLog's IR5 events 1..21: three accepted heads, with
/// the middle task proposed by M; M then equivocates at author position zero.
#[test]
fn tla_ir5_member_fork_rolls_back_dependent_accepted_heads() {
    let mut coordinator = Author::new(1);
    let mut worker = Author::new(2);
    let mut proposer = Author::new(3);
    let (genesis, own) = coordinator.found_goal(EndpointId([1; 32]));
    let goal_id = genesis.header().goal;
    let worker_admission = coordinator.event(
        goal_id,
        Some(own.id()),
        Body::MemberAdmitted {
            member: worker.key.public(),
            endpoint: EndpointId([2; 32]),
        },
    );
    let proposer_admission = coordinator.event(
        goal_id,
        Some(worker_admission.id()),
        Body::MemberAdmitted {
            member: proposer.key.public(),
            endpoint: EndpointId([3; 32]),
        },
    );
    let mut anchor = proposer_admission.id();
    let mut base = None;
    let mut tasks = Vec::new();
    let mut results = Vec::new();
    let mut acceptances = Vec::new();
    let mut events = vec![genesis, own, worker_admission, proposer_admission];
    for (index, head) in [BlobHash([9; 32]), BlobHash([14; 32]), BlobHash([19; 32])]
        .into_iter()
        .enumerate()
    {
        let author = if index == 1 {
            &mut proposer
        } else {
            &mut coordinator
        };
        let proposal = author.event(
            goal_id,
            Some(anchor),
            Body::TaskProposed {
                input: None,
                depends_on: tasks.last().copied().into_iter().collect(),
                deadline_ms: None,
                max_attempts: Some(2),
            },
        );
        let assignment = coordinator.event(
            goal_id,
            Some(anchor),
            Body::TaskAssigned {
                task: proposal.id(),
                assignee: worker.key.public(),
                attempt: 1,
            },
        );
        let take = worker.event(
            goal_id,
            Some(assignment.id()),
            Body::AssignmentAccepted {
                assignment: assignment.id(),
            },
        );
        let result = worker.event(
            goal_id,
            Some(assignment.id()),
            Body::ResultSubmitted {
                assignment: assignment.id(),
                base,
                patch: None,
                artifacts: vec![],
            },
        );
        let accept = coordinator.event(
            goal_id,
            Some(assignment.id()),
            Body::ResultAccepted {
                result: result.id(),
                head: Some(head),
            },
        );
        tasks.push(proposal.id());
        results.push(result.id());
        acceptances.push(accept.id());
        anchor = accept.id();
        base = Some(head);
        events.extend([proposal, assignment, take, result, accept]);
    }
    let mut chronological = held(&events);
    assert_eq!(chronological.state().accepted_heads.len(), 3);
    for task in &tasks {
        assert_eq!(
            chronological.state().task(task).unwrap().state,
            TaskState::Accepted
        );
    }

    let mut conflicting = events[9].header().clone();
    conflicting.body = Body::Note {
        about: None,
        supersedes: None,
    };
    let fork = Event::sign(conflicting, &proposer.key).unwrap();
    events.push(fork.clone());
    chronological.apply(&[fork]);
    // The coordinator did not fork: replay continues, but M's proposal is
    // unusable. The final acceptance no longer matches the accepted base.
    assert_eq!(chronological.halt(), None);
    assert_eq!(
        chronological.standing(&events[9].id()),
        Some(Standing::Excluded(Exclusion::Forked))
    );
    assert!(chronological.state().task(&tasks[1]).is_none());
    assert_eq!(chronological.state().accepted_heads.len(), 1);
    assert_eq!(
        chronological.state().accepted_head(),
        Some(BlobHash([9; 32]))
    );
    assert_eq!(
        chronological.state().task(&tasks[2]).unwrap().state,
        TaskState::Submitted
    );
    assert_eq!(
        chronological.standing(&results[2]),
        Some(Standing::Effective)
    );
    for accept in &acceptances[1..] {
        assert!(matches!(
            chronological.standing(accept),
            Some(Standing::Excluded(_))
        ));
    }

    let removal = coordinator.event(
        goal_id,
        Some(anchor),
        Body::MemberRemoved {
            member: proposer.key.public(),
            last_accepted: None,
        },
    );
    events.push(removal.clone());
    chronological.apply(&[removal]);
    assert!(!chronological.state().is_member(&proposer.key.public()));
    assert_eq!(chronological.state().accepted_heads.len(), 1);
    assert_eq!(
        chronological.state().task(&tasks[2]).unwrap().state,
        TaskState::Submitted
    );

    let mut scrambled = Goal::new(goal_id);
    for event in events.iter().rev() {
        scrambled.apply(std::slice::from_ref(event));
        assert_eq!(scrambled.folded, super::fold::fold(&scrambled.history).0);
    }
    assert_same(&chronological, &scrambled, &events);
}
