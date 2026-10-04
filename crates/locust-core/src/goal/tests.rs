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

/// Reuse the historical version-0 GoalLog IR12 events 1..9 against version 1.
/// Signed peer contributions enter through the held-event seam; the corrected
/// runtime keeps the accepted display while retaining the late result as
/// evidence. This does not establish conformance with the old model outcome.
#[test]
fn tla_ir12_later_submission_preserves_the_accepted_display() {
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
    assert_eq!(view.result, Some(accepted_result));
    assert_eq!(
        chronological
            .state()
            .assignment(&assignment)
            .unwrap()
            .result,
        Some(later.id())
    );
    assert!(
        chronological
            .state()
            .results
            .iter()
            .any(|result| result.id == later.id())
    );
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

/// Reuse the historical version-0 GoalLog IR5 events 1..21 against version 1.
/// The canonical decisions select M's original proposal and preserve all three
/// accepted heads across its later fork. The final explicit empty removal
/// cutoff is a separate coordinator action, not the old member-fork outcome.
#[test]
fn tla_ir5_member_fork_preserves_the_committed_accepted_heads() {
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
    assert_eq!(chronological.halt(), None);
    assert_eq!(
        chronological.standing(&events[9].id()),
        Some(Standing::Effective)
    );
    assert_eq!(
        chronological.standing(&events[19].id()),
        Some(Standing::Excluded(Exclusion::Forked))
    );
    assert_eq!(chronological.state().accepted_heads.len(), 3);
    assert_eq!(
        chronological.state().accepted_head(),
        Some(BlobHash([19; 32]))
    );
    for (task, result) in tasks.iter().zip(&results) {
        let task = chronological.state().task(task).unwrap();
        assert_eq!(task.state, TaskState::Accepted);
        assert_eq!(task.accepted, Some(*result));
    }
    for accept in &acceptances {
        assert_eq!(chronological.standing(accept), Some(Standing::Effective));
    }

    // Preserve historical event 21: last_accepted=None intentionally fences
    // every contribution of M's tenure. That explicit coordinator cutoff
    // can withdraw the proposal on which later acceptances depend; branch
    // commitments do not bypass membership-removal policy.

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

#[test]
fn canonical_acceptance_pins_transitive_author_ancestry_across_arrivals_and_reopen() {
    use locust_proto::id::BlobHash;
    let (mut coordinator, mut worker, mut events) = transcript();
    let id = events[0].header().goal;
    let accepted = coordinator.event(
        id,
        Some(events[4].id()),
        Body::ResultAccepted {
            result: events[6].id(),
            head: Some(BlobHash([42; 32])),
        },
    );
    let mut fork_header = events[5].header().clone();
    fork_header.at_ms += 42;
    let fork = Event::sign(fork_header, &worker.key).unwrap();
    let suffix = worker.event(
        id,
        Some(accepted.id()),
        Body::Note {
            about: None,
            supersedes: None,
        },
    );
    events.extend([accepted, fork.clone(), suffix.clone()]);
    let expected = held(&events);
    assert_eq!(expected.state().accepted_head(), Some(BlobHash([42; 32])));
    assert_eq!(
        expected.standing(&events[5].id()),
        Some(Standing::Effective)
    );
    assert_eq!(
        expected.standing(&events[6].id()),
        Some(Standing::Effective)
    );
    assert!(matches!(
        expected.standing(&fork.id()),
        Some(Standing::Excluded(super::Exclusion::Forked))
    ));
    assert!(matches!(
        expected.standing(&suffix.id()),
        Some(Standing::Excluded(super::Exclusion::Forked))
    ));
    for seed in 1..65u64 {
        let mut order = events.clone();
        let mut random = seed;
        for i in (1..order.len()).rev() {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            order.swap(i, random as usize % (i + 1));
        }
        let mut goal = Goal::new(id);
        let mut store = MemStore::new();
        for event in order {
            store
                .commit(&Commit {
                    events: vec![event.clone()],
                    ..Commit::default()
                })
                .unwrap();
            goal.apply(&[event]);
            assert_eq!(goal.folded, super::fold::fold(&goal.history).0);
        }
        assert_same(&expected, &goal, &events);
        assert_same(&expected, &Goal::load(&store, id).unwrap(), &events);
    }
}

#[test]
fn late_submission_is_evidence_without_replacing_the_accepted_task_result() {
    let (mut coordinator, mut worker, mut events) = transcript();
    let id = events[0].header().goal;
    let accepted = coordinator.event(
        id,
        Some(events[4].id()),
        Body::ResultAccepted {
            result: events[6].id(),
            head: None,
        },
    );
    let late = worker.event(
        id,
        Some(accepted.id()),
        Body::ResultSubmitted {
            assignment: events[4].id(),
            base: None,
            patch: None,
            artifacts: vec![],
        },
    );
    events.extend([accepted, late.clone()]);
    let goal = held(&events);
    assert_eq!(goal.state().tasks[0].result, Some(events[6].id()));
    assert_eq!(goal.state().tasks[0].accepted, Some(events[6].id()));
    assert_eq!(goal.standing(&late.id()), Some(Standing::Effective));
    assert!(
        goal.state()
            .results
            .iter()
            .any(|result| result.id == late.id())
    );
}

#[test]
fn canonical_dependencies_survive_variant_and_waiting_quotas_in_the_same_batch() {
    for waiting in [false, true] {
        let (mut coordinator, worker, events) = transcript();
        let id = events[0].header().goal;
        let mut goal = held(&events[..5]);
        let count = if waiting {
            super::MAX_WAITING_PER_AUTHOR
        } else {
            super::MAX_FORK_VARIANTS
        };
        let mut fill = Vec::new();
        for i in 0..count {
            let mut header = events[5].header().clone();
            header.at_ms += 100 + i as u64;
            if waiting {
                header.seq = 100 + i as u64;
                header.prev = Some(events[5].id());
            }
            fill.push(Event::sign(header, &worker.key).unwrap());
        }
        let fill = goal.screen(fill);
        assert_eq!(fill.len(), count);
        goal.apply(&fill);
        let accepted = coordinator.event(
            id,
            Some(events[4].id()),
            Body::ResultAccepted {
                result: events[6].id(),
                head: None,
            },
        );
        let batch = goal.screen(vec![events[6].clone(), events[5].clone(), accepted.clone()]);
        assert!(batch.iter().any(|event| event.id() == events[5].id()));
        assert!(batch.iter().any(|event| event.id() == events[6].id()));
        goal.apply(&batch);
        assert_eq!(goal.state().tasks[0].accepted, Some(events[6].id()));
        assert_eq!(goal.state().head, Some(accepted.id()));
    }
}

#[test]
fn broken_chain_or_unauthorized_decision_cannot_pin_a_member_branch() {
    for unauthorized in [false, true] {
        let (mut coordinator, worker, mut events) = transcript();
        let id = events[0].header().goal;
        let mut fork_header = events[5].header().clone();
        fork_header.at_ms += 1;
        events.push(Event::sign(fork_header, &worker.key).unwrap());
        let decision = if unauthorized {
            let mut outsider = Author::new(44);
            outsider.event(
                id,
                Some(events[4].id()),
                Body::ResultAccepted {
                    result: events[6].id(),
                    head: None,
                },
            )
        } else {
            coordinator.event(
                id,
                Some(events[2].id()),
                Body::ResultAccepted {
                    result: events[6].id(),
                    head: None,
                },
            )
        };
        events.push(decision);
        let goal = held(&events);
        assert!(matches!(
            goal.standing(&events[5].id()),
            Some(Standing::Excluded(super::Exclusion::Forked))
        ));
        assert!(matches!(
            goal.standing(&events[6].id()),
            Some(Standing::Excluded(super::Exclusion::Forked))
        ));
        assert_eq!(goal.state().tasks[0].accepted, None);
    }
}

#[test]
fn noncanonical_claimed_decisions_do_not_bypass_dependency_retention_quotas() {
    for unauthorized in [false, true] {
        let (mut coordinator, worker, events) = transcript();
        let id = events[0].header().goal;
        let mut goal = held(&events[..5]);
        let fill: Vec<_> = (0..super::MAX_FORK_VARIANTS)
            .map(|i| {
                let mut header = events[5].header().clone();
                header.at_ms += 100 + i as u64;
                Event::sign(header, &worker.key).unwrap()
            })
            .collect();
        goal.apply(&goal.screen(fill));
        let decision = if unauthorized {
            let mut outsider = Author::new(88);
            outsider.event(
                id,
                Some(events[4].id()),
                Body::ResultAccepted {
                    result: events[6].id(),
                    head: None,
                },
            )
        } else {
            coordinator.event(
                id,
                Some(events[2].id()),
                Body::ResultAccepted {
                    result: events[6].id(),
                    head: None,
                },
            )
        };
        let kept = goal.screen(vec![decision, events[6].clone(), events[5].clone()]);
        assert!(!kept.iter().any(|event| event.id() == events[5].id()));
    }
}

#[test]
fn invalid_canonical_reference_does_not_resurrect_a_forked_branch() {
    use locust_proto::id::EventId;
    let (mut coordinator, worker, mut events) = transcript();
    let id = events[0].header().goal;
    let mut fork_header = events[5].header().clone();
    fork_header.at_ms += 1;
    let fork = Event::sign(fork_header, &worker.key).unwrap();
    let mut bad_header = events.pop().unwrap().header().clone();
    bad_header.body = Body::ResultSubmitted {
        assignment: EventId([99; 32]),
        base: None,
        patch: None,
        artifacts: vec![],
    };
    let invalid = Event::sign(bad_header, &worker.key).unwrap();
    let decision = coordinator.event(
        id,
        Some(events[4].id()),
        Body::ResultAccepted {
            result: invalid.id(),
            head: None,
        },
    );
    events.extend([fork, invalid, decision.clone()]);
    let expected = held(&events);
    assert_eq!(
        expected.standing(&events[5].id()),
        Some(Standing::Excluded(super::Exclusion::Forked))
    );
    assert!(matches!(
        expected.standing(&decision.id()),
        Some(Standing::Excluded(_))
    ));
    for seed in 1..65u64 {
        let mut order = events.clone();
        let mut random = seed;
        for i in (1..order.len()).rev() {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            order.swap(i, random as usize % (i + 1));
        }
        let mut goal = Goal::new(id);
        let mut store = MemStore::new();
        for event in order {
            store
                .commit(&Commit {
                    events: vec![event.clone()],
                    ..Commit::default()
                })
                .unwrap();
            goal.apply(&[event]);
            assert_eq!(goal.folded, super::fold::fold(&goal.history).0);
        }
        assert_same(&expected, &goal, &events);
        assert_same(
            &expected,
            &Goal::load(&store.reopen(), id).unwrap(),
            &events,
        );
    }
}

#[test]
fn wrong_kind_reference_retains_its_header_without_exempting_ancestry() {
    let (mut coordinator, mut worker, events) = transcript();
    let id = events[0].header().goal;
    let mut goal = held(&events);
    let mut fork_header = events[5].header().clone();
    fork_header.at_ms += 1;
    goal.apply(&[Event::sign(fork_header, &worker.key).unwrap()]);
    let mut suffix = Vec::new();
    for _ in 0..(super::MAX_WAITING_PER_AUTHOR * 2) {
        suffix.push(worker.event(
            id,
            Some(events[4].id()),
            Body::Note {
                about: None,
                supersedes: None,
            },
        ));
    }
    let target = suffix.last().unwrap().id();
    let decision = coordinator.event(
        id,
        Some(events[4].id()),
        Body::RevisionAccepted { revision: target },
    );
    suffix.push(decision.clone());
    for reversed in [false, true] {
        let mut batch = suffix.clone();
        if reversed {
            batch.reverse();
        }
        let kept = goal.screen(batch);
        assert!(kept.iter().any(|event| event.id() == target));
        assert!(kept.len() <= super::MAX_WAITING_PER_AUTHOR + 2);
        let mut projected = super::history::History::default();
        for event in goal.history.events.iter().chain(&kept) {
            projected.insert(event);
        }
        let selection = super::commitments::Commitments::build(
            &projected,
            &super::chain::Chain::build(&projected),
        );
        assert!(
            !selection
                .pins
                .keys()
                .any(|(author, _)| *author == worker.key.public())
        );
        assert_eq!(selection.required.len(), 2); // Coordinator task and the invalid direct target.
    }
}

#[test]
fn wrong_kind_reference_with_missing_ancestry_is_refused_without_stalling() {
    use locust_proto::id::EventId;
    let (mut coordinator, worker, mut events) = transcript();
    let id = events[0].header().goal;
    let mut header = events[6].header().clone();
    header.seq = 100;
    header.prev = Some(EventId([99; 32]));
    header.body = Body::Note {
        about: None,
        supersedes: None,
    };
    let target = Event::sign(header, &worker.key).unwrap();
    let decision = coordinator.event(
        id,
        Some(events[4].id()),
        Body::ResultAccepted {
            result: target.id(),
            head: None,
        },
    );
    let next = coordinator.event(
        id,
        Some(decision.id()),
        Body::MemberAdmitted {
            member: Author::new(44).key.public(),
            endpoint: EndpointId([44; 32]),
        },
    );
    events.extend([target, decision.clone(), next.clone()]);
    let goal = held(&events);
    assert!(matches!(
        goal.standing(&decision.id()),
        Some(Standing::Excluded(_))
    ));
    assert_eq!(goal.standing(&next.id()), Some(Standing::Effective));
    assert_eq!(goal.state().head, Some(next.id()));
    let mut store = MemStore::new();
    store
        .commit(&Commit {
            events: events.clone(),
            ..Commit::default()
        })
        .unwrap();
    assert_same(&goal, &Goal::load(&store.reopen(), id).unwrap(), &events);
}

#[test]
fn incomplete_canonical_branch_waits_then_recovers_through_a_full_variant_quota() {
    let (mut coordinator, worker, events) = transcript();
    let id = events[0].header().goal;
    let mut goal = held(&events[..5]);
    let variants: Vec<_> = (0..super::MAX_FORK_VARIANTS)
        .map(|i| {
            let mut header = events[5].header().clone();
            header.at_ms += 100 + i as u64;
            Event::sign(header, &worker.key).unwrap()
        })
        .collect();
    goal.apply(&goal.screen(variants));
    let accepted = coordinator.event(
        id,
        Some(events[4].id()),
        Body::ResultAccepted {
            result: events[6].id(),
            head: None,
        },
    );
    let after = coordinator.event(
        id,
        Some(accepted.id()),
        Body::MemberAdmitted {
            member: Author::new(44).key.public(),
            endpoint: EndpointId([44; 32]),
        },
    );
    goal.apply(&[events[6].clone(), accepted.clone(), after.clone()]);
    assert_eq!(
        goal.standing(&accepted.id()),
        Some(Standing::Pending(super::Waiting::Reference))
    );
    assert!(matches!(
        goal.standing(&after.id()),
        Some(Standing::Pending(_))
    ));
    assert_eq!(goal.state().head, Some(events[4].id()));
    let kept = goal.screen(vec![events[5].clone()]);
    assert_eq!(kept.len(), 1);
    goal.apply(&kept);
    assert_eq!(goal.standing(&accepted.id()), Some(Standing::Effective));
    assert_eq!(goal.standing(&after.id()), Some(Standing::Effective));
    assert_eq!(goal.state().tasks[0].accepted, Some(events[6].id()));
    let all = goal.history.events.clone();
    let mut store = MemStore::new();
    store
        .commit(&Commit {
            events: all.clone(),
            ..Commit::default()
        })
        .unwrap();
    assert_same(&goal, &Goal::load(&store.reopen(), id).unwrap(), &all);
}
