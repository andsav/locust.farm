use super::*;

use locust_proto::id::BlobHash;
use locust_proto::organization::WorkspacePolicy;

fn workspace_formation(reviews: u32) -> Formation {
    Formation {
        workspace: Some(WorkspacePolicy {
            integrator: Authority::Participant {
                key: testkit::keypair(4).public().to_string(),
            },
            completion: CompletionRule::Reviews {
                by: Selector::Members,
                count: reviews,
                exclude_author: true,
            },
        }),
        ..Formation::default()
    }
}

fn epoch(f: &mut Fixture, expected: Option<EventId>, checkpoint: WorkspaceCheckpoint) -> Context {
    let id = f.admin(Body::WorkspaceEpoch {
        expected_epoch: expected,
        rules: f.rules,
        checkpoint,
    });
    Context {
        scope: Scope::Workspace,
        round: id,
    }
}

fn proposal(
    f: &mut Fixture,
    worker: usize,
    context: Context,
    parent: Option<EventId>,
    sources: Vec<EventId>,
) -> EventId {
    let result_manifest = BlobHash([100 + f.events.len() as u8; 32]);
    f.worker(
        worker,
        Body::WorkspaceProposed {
            context,
            parent,
            result_manifest,
            sources,
        },
    )
}

fn integrate(
    f: &mut Fixture,
    context: Context,
    previous: Option<EventId>,
    subject: EventId,
    evidence: Vec<EventId>,
) -> EventId {
    f.worker(
        2,
        Body::ScopeDecided {
            context,
            previous,
            action: DecisionAction::Select { subject },
            evidence,
        },
    )
}

fn seed(f: &mut Fixture, context: Context) -> (EventId, EventId, EventId) {
    let candidate = proposal(f, 0, context, None, Vec::new());
    let review = f.review(1, context, candidate);
    let revision = integrate(f, context, None, candidate, vec![review]);
    (candidate, revision, review)
}

fn head(goal: &Goal) -> Option<EventId> {
    goal.state().workspace.as_ref().and_then(|w| w.head)
}

fn fork_body(f: &mut Fixture, original: EventId, key: u8, body: Body) -> EventId {
    let mut header = f.event(original).header().clone();
    header.at_ms += 20_000;
    header.body = body;
    let event = Event::sign(header, &testkit::keypair(key)).unwrap();
    let id = event.id();
    f.events.push(event);
    id
}

fn assert_workspace_replay_and_restart(f: &Fixture) {
    let expected = f.goal();
    for shift in 0..f.events.len() {
        let mut incoming = f.events.clone();
        incoming.rotate_left(shift);
        if shift % 2 == 1 {
            incoming.reverse();
        }
        let mut replica = Goal::new(f.id);
        for chunk in incoming.chunks(3) {
            replica.apply(chunk, &f.definitions);
            replica.apply(chunk, &f.definitions);
        }
        assert_eq!(replica.evaluation(), expected.evaluation(), "shift {shift}");
    }
    let mut store = MemStore::new();
    store
        .commit(&Commit {
            events: f.events.clone(),
            ..Commit::default()
        })
        .unwrap();
    assert_eq!(
        Goal::load(&store.reopen(), f.id, &f.definitions)
            .unwrap()
            .evaluation(),
        expected.evaluation()
    );
}

#[test]
fn workspace_seed_and_successor_bind_exact_parent_without_manifest_bytes() {
    let mut f = Fixture::new(workspace_formation(1));
    let context = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (first, revision, _) = seed(&mut f, context);
    let second = proposal(&mut f, 0, context, Some(revision), Vec::new());
    let review = f.review(1, context, second);
    let next = integrate(&mut f, context, Some(revision), second, vec![review]);
    let goal = f.goal();
    let workspace = goal.state().workspace.as_ref().unwrap();
    assert_eq!(workspace.epoch, context.round);
    assert!(workspace.ready && workspace.enabled);
    assert_eq!(workspace.checkpoint, None);
    assert_eq!(workspace.head, Some(next));
    assert_eq!(goal.standing(&revision), Some(Standing::Effective));
    assert_eq!(goal.standing(&next), Some(Standing::Effective));
    assert!(goal.state().workspace_lineage.contains(&revision));
    assert!(goal.state().workspace_lineage.contains(&next));
    assert_eq!(goal.state().workspace_revisions[&revision].proposal, first);
    assert_eq!(goal.state().workspace_revisions[&next].proposal, second);
    assert_workspace_replay_and_restart(&f);
}

#[test]
fn workspace_has_no_implicit_administrator_integrator() {
    let mut f = Fixture::new(Formation::default());
    let context = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let candidate = proposal(&mut f, 0, context, None, Vec::new());
    let decision = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject: candidate },
        evidence: Vec::new(),
    });
    let goal = f.goal();
    assert!(!goal.standing(&decision).unwrap().is_effective());
    assert_eq!(head(&goal), None);
    assert!(!goal.state().workspace.as_ref().unwrap().enabled);
}

#[test]
fn workspace_wrong_integrator_and_stale_parent_do_not_advance() {
    let mut f = Fixture::new(workspace_formation(1));
    let context = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let candidate = proposal(&mut f, 0, context, None, Vec::new());
    let stale = proposal(&mut f, 0, context, None, Vec::new());
    let review = f.review(1, context, candidate);
    let wrong = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject: candidate },
        evidence: vec![review],
    });
    let revision = integrate(&mut f, context, None, candidate, vec![review]);
    let stale_review = f.review(1, context, stale);
    let stale_decision = integrate(&mut f, context, Some(revision), stale, vec![stale_review]);
    let goal = f.goal();
    assert!(!goal.standing(&wrong).unwrap().is_effective());
    assert!(!goal.standing(&stale_decision).unwrap().is_effective());
    assert_eq!(head(&goal), Some(revision));
}

#[test]
fn workspace_review_evidence_cannot_move_to_another_candidate() {
    let mut f = Fixture::new(workspace_formation(1));
    let context = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let source = proposal(&mut f, 0, context, None, Vec::new());
    let candidate = proposal(&mut f, 0, context, None, vec![source]);
    let old_review = f.review(1, context, source);
    let decision = integrate(&mut f, context, None, candidate, vec![old_review]);
    assert!(!f.goal().standing(&decision).unwrap().is_effective());
    assert_eq!(head(&f.goal()), None);
}

#[test]
fn workspace_nested_source_authors_are_ineligible_but_independent_review_counts() {
    let mut f = Fixture::new(workspace_formation(1));
    let context = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let direct = proposal(&mut f, 1, context, None, Vec::new());
    let nested = proposal(&mut f, 2, context, None, vec![direct]);
    let candidate = proposal(&mut f, 0, context, None, vec![nested]);
    let self_review = f.review(0, context, candidate);
    let direct_review = f.review(1, context, candidate);
    let nested_review = f.review(2, context, candidate);
    let before = f.goal();
    for review in [self_review, direct_review, nested_review] {
        assert!(!before.standing(&review).unwrap().is_effective());
    }
    assert!(!before.state().workspace_proposals[&candidate].approved);
    let independent = f.admin(Body::ReviewRecorded {
        context,
        subject: candidate,
        verdict: ReviewVerdict::Approve,
    });
    let revision = integrate(&mut f, context, None, candidate, vec![independent]);
    let goal = f.goal();
    assert_eq!(goal.standing(&revision), Some(Standing::Effective));
    assert_eq!(head(&goal), Some(revision));
    assert!(
        goal.state().workspace_proposals[&candidate]
            .source_authors
            .is_superset(
                &[f.workers[1].key.public(), f.workers[2].key.public()]
                    .into_iter()
                    .collect()
            )
    );
    assert_workspace_replay_and_restart(&f);
}

#[test]
fn workspace_parent_history_does_not_permanently_exclude_old_authors() {
    let mut f = Fixture::new(workspace_formation(1));
    let context = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (_, revision, _) = seed(&mut f, context);
    let candidate = proposal(&mut f, 1, context, Some(revision), Vec::new());
    let old_author_review = f.review(0, context, candidate);
    let next = integrate(
        &mut f,
        context,
        Some(revision),
        candidate,
        vec![old_author_review],
    );
    let goal = f.goal();
    assert_eq!(goal.standing(&old_author_review), Some(Standing::Effective));
    assert_eq!(head(&goal), Some(next));
}

#[test]
fn workspace_missing_source_waits_then_counts_full_source_author_set() {
    let mut f = Fixture::new(workspace_formation(1));
    let context = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let source = proposal(&mut f, 1, context, None, Vec::new());
    let candidate = proposal(&mut f, 0, context, None, vec![source]);
    let review = f.admin(Body::ReviewRecorded {
        context,
        subject: candidate,
        verdict: ReviewVerdict::Approve,
    });
    let revision = integrate(&mut f, context, None, candidate, vec![review]);
    let without: Vec<_> = f
        .events
        .iter()
        .filter(|e| e.id() != source)
        .cloned()
        .collect();
    let mut goal = Goal::new(f.id);
    goal.apply(&without, &f.definitions);
    assert!(!goal.standing(&revision).unwrap().is_effective());
    assert_eq!(head(&goal), None);
    goal.apply(std::slice::from_ref(f.event(source)), &f.definitions);
    assert_eq!(head(&goal), Some(revision));
    assert!(
        goal.state().workspace_proposals[&candidate]
            .source_authors
            .contains(&f.workers[1].key.public())
    );
}

#[test]
fn workspace_sources_require_workspace_proposals_not_advisory_task_citations() {
    let mut f = Fixture::new(workspace_formation(1));
    let context = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let source = f.publish(1, f.context());
    let candidate = proposal(&mut f, 0, context, None, vec![source]);
    let review = f.admin(Body::ReviewRecorded {
        context,
        subject: candidate,
        verdict: ReviewVerdict::Approve,
    });
    let revision = integrate(&mut f, context, None, candidate, vec![review]);
    assert!(!f.goal().standing(&revision).unwrap().is_effective());
    assert_eq!(head(&f.goal()), None);
}

#[test]
fn workspace_competing_successors_retract_the_head_without_halting_governance() {
    let mut f = Fixture::new(workspace_formation(1));
    let context = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (_, first, _) = seed(&mut f, context);
    let candidate = proposal(&mut f, 0, context, None, Vec::new());
    let review = f.review(1, context, candidate);
    let second = integrate(&mut f, context, None, candidate, vec![review]);
    let unrelated = f.publish(0, f.context());
    let goal = f.goal();
    assert_eq!(goal.standing(&first), Some(Standing::Disputed));
    assert_eq!(goal.standing(&second), Some(Standing::Disputed));
    assert_eq!(head(&goal), None);
    assert!(goal.evaluation().admin_halt.is_none());
    assert_eq!(goal.standing(&unrelated), Some(Standing::Effective));
}

#[test]
fn workspace_epoch_reconciles_one_selection_branch_and_fences_late_forks() {
    let mut f = Fixture::new(workspace_formation(1));
    let old = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (_, chosen, _) = seed(&mut f, old);
    let competing = f.fork(chosen, 4);
    let new = epoch(
        &mut f,
        Some(old.round),
        WorkspaceCheckpoint::Revision(chosen),
    );
    let candidate = proposal(&mut f, 0, new, Some(chosen), Vec::new());
    let review = f.review(1, new, candidate);
    let revision = integrate(&mut f, new, None, candidate, vec![review]);
    let without: Vec<_> = f
        .events
        .iter()
        .filter(|e| e.id() != competing)
        .cloned()
        .collect();
    let mut goal = Goal::new(f.id);
    goal.apply(&without, &f.definitions);
    assert_eq!(head(&goal), Some(revision));
    goal.apply(std::slice::from_ref(f.event(competing)), &f.definitions);
    assert_eq!(head(&goal), Some(revision));
    let workspace = goal.state().workspace.as_ref().unwrap();
    assert_eq!(workspace.epoch, new.round);
    assert_eq!(workspace.checkpoint, Some(chosen));
    assert!(workspace.ready);
    assert!(goal.state().workspace_lineage.contains(&chosen));
    assert!(!goal.state().workspace_lineage.contains(&competing));
    assert_workspace_replay_and_restart(&f);
}

#[test]
fn workspace_checkpoint_does_not_waive_an_unrelated_integrator_log_fork() {
    let mut f = Fixture::new(workspace_formation(1));
    let old = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (candidate, selected, _) = seed(&mut f, old);
    fork_body(
        &mut f,
        selected,
        4,
        Body::ReviewRecorded {
            context: old,
            subject: candidate,
            verdict: ReviewVerdict::Approve,
        },
    );
    let new = epoch(
        &mut f,
        Some(old.round),
        WorkspaceCheckpoint::Revision(selected),
    );
    let goal = f.goal();
    let workspace = goal.state().workspace.as_ref().unwrap();
    assert_eq!(workspace.epoch, new.round);
    assert!(!workspace.ready);
    assert_eq!(head(&goal), None);
    assert!(goal.evaluation().admin_halt.is_none());
}

#[test]
fn workspace_continuation_cannot_inherit_an_excluded_signer_branch() {
    let mut f = Fixture::new(workspace_formation(1));
    let old = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (_, chosen, _) = seed(&mut f, old);
    let excluded = f.fork(chosen, 4);
    let new = epoch(
        &mut f,
        Some(old.round),
        WorkspaceCheckpoint::Revision(chosen),
    );
    let candidate = proposal(&mut f, 0, new, Some(chosen), Vec::new());
    let review = f.review(1, new, candidate);
    let original = integrate(&mut f, new, None, candidate, vec![review]);
    let mut header = f.event(original).header().clone();
    header.prev = Some(excluded);
    let event = Event::sign(header, &testkit::keypair(4)).unwrap();
    let wrong_branch = event.id();
    *f.events
        .iter_mut()
        .find(|event| event.id() == original)
        .unwrap() = event;
    let goal = f.goal();
    assert!(goal.standing(&wrong_branch).unwrap().is_pending());
    assert_eq!(head(&goal), Some(chosen));
    assert_workspace_replay_and_restart(&f);
}

#[test]
fn task_selection_cannot_inherit_a_workspace_checkpoint_signer_pin() {
    let mut formation = workspace_formation(1);
    formation.decisions.completion = CompletionRule::Reviews {
        by: Selector::Members,
        count: 1,
        exclude_author: true,
    };
    formation.decisions.selection = Some(Authority::Participant {
        key: testkit::keypair(4).public().to_string(),
    });
    let mut f = Fixture::new(formation);
    let old = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (_, chosen, _) = seed(&mut f, old);
    epoch(
        &mut f,
        Some(old.round),
        WorkspaceCheckpoint::Revision(chosen),
    );
    f.fork(chosen, 4);
    let task = f.task();
    let candidate = f.publish(0, task);
    let review = f.review(1, task, candidate);
    let decision = f.worker(
        2,
        Body::ScopeDecided {
            context: task,
            previous: None,
            action: DecisionAction::Select { subject: candidate },
            evidence: vec![review],
        },
    );
    let goal = f.goal();
    assert!(goal.standing(&decision).unwrap().is_pending());
    assert_eq!(goal.state().task_round(task).unwrap().selected, None);
    assert_eq!(head(&goal), Some(chosen));
    assert_workspace_replay_and_restart(&f);
}

#[test]
fn workspace_checkpoint_proof_pins_do_not_leak_to_ordinary_work() {
    let mut f = Fixture::new(workspace_formation(1));
    let old = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (_, chosen, review) = seed(&mut f, old);
    let unrelated = f.publish(1, f.context());
    f.fork(review, 3);
    let new = epoch(
        &mut f,
        Some(old.round),
        WorkspaceCheckpoint::Revision(chosen),
    );
    let goal = f.goal();
    assert_eq!(goal.state().workspace.as_ref().unwrap().epoch, new.round);
    assert_eq!(head(&goal), Some(chosen));
    assert!(!goal.standing(&unrelated).unwrap().is_effective());
    assert!(!goal.state().contributions.contains_key(&unrelated));
    assert_workspace_replay_and_restart(&f);
}

#[test]
fn workspace_missing_or_wrong_kind_checkpoint_fences_prior_epoch_but_not_membership() {
    for checkpoint_is_missing in [false, true] {
        let mut f = Fixture::new(workspace_formation(1));
        let old = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
        let (candidate, _, _) = seed(&mut f, old);
        let checkpoint = if checkpoint_is_missing {
            EventId([99; 32])
        } else {
            candidate
        };
        let blocked = epoch(
            &mut f,
            Some(old.round),
            WorkspaceCheckpoint::Revision(checkpoint),
        );
        f.admin(Body::MemberRemoved {
            member: f.workers[1].key.public(),
            admission: f.admissions[2],
            last_accepted: None,
        });
        let goal = f.goal();
        let workspace = goal.state().workspace.as_ref().unwrap();
        assert_eq!(workspace.epoch, blocked.round);
        assert!(!workspace.ready);
        assert_eq!(workspace.head, None);
        assert!(!goal.state().is_member(&f.workers[1].key.public()));
        assert!(goal.evaluation().admin_halt.is_none());
    }
}

#[test]
fn workspace_pending_checkpoint_can_arrive_after_structural_fencing() {
    let mut f = Fixture::new(workspace_formation(1));
    let old = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (_, chosen, _) = seed(&mut f, old);
    let new = epoch(
        &mut f,
        Some(old.round),
        WorkspaceCheckpoint::Revision(chosen),
    );
    let without: Vec<_> = f
        .events
        .iter()
        .filter(|e| e.id() != chosen)
        .cloned()
        .collect();
    let mut goal = Goal::new(f.id);
    goal.apply(&without, &f.definitions);
    assert_eq!(goal.state().workspace.as_ref().unwrap().epoch, new.round);
    assert!(!goal.state().workspace.as_ref().unwrap().ready);
    assert_eq!(head(&goal), None);
    goal.apply(std::slice::from_ref(f.event(chosen)), &f.definitions);
    assert!(goal.state().workspace.as_ref().unwrap().ready);
    assert_eq!(head(&goal), Some(chosen));
}

#[test]
fn workspace_checkpoint_does_not_waive_missing_candidate_review_evidence() {
    let mut f = Fixture::new(workspace_formation(1));
    let old = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (_, chosen, review) = seed(&mut f, old);
    let new = epoch(
        &mut f,
        Some(old.round),
        WorkspaceCheckpoint::Revision(chosen),
    );
    let without: Vec<_> = f
        .events
        .iter()
        .filter(|event| event.id() != review)
        .cloned()
        .collect();
    let mut goal = Goal::new(f.id);
    goal.apply(&without, &f.definitions);
    assert_eq!(goal.state().workspace.as_ref().unwrap().epoch, new.round);
    assert!(!goal.state().workspace.as_ref().unwrap().ready);
    assert_eq!(head(&goal), None);
    goal.apply(std::slice::from_ref(f.event(review)), &f.definitions);
    assert_eq!(head(&goal), Some(chosen));
    assert!(goal.state().workspace.as_ref().unwrap().ready);
}

#[test]
fn workspace_check_attestations_bind_the_final_candidate_not_its_source() {
    let mut formation = workspace_formation(1);
    formation.workspace.as_mut().unwrap().completion = CompletionRule::All {
        rules: vec![
            CompletionRule::Reviews {
                by: Selector::Members,
                count: 1,
                exclude_author: true,
            },
            CompletionRule::Check {
                name: "unit".into(),
                by: Selector::Members,
            },
        ],
    };
    let mut f = Fixture::new(formation);
    let context = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let source = proposal(&mut f, 0, context, None, Vec::new());
    let candidate = proposal(&mut f, 0, context, None, vec![source]);
    let review = f.review(1, context, candidate);
    f.worker(
        1,
        Body::CheckAttested {
            context,
            subject: source,
            name: "unit".into(),
            passed: true,
        },
    );
    assert!(!f.goal().state().workspace_proposals[&candidate].approved);
    let check = f.worker(
        1,
        Body::CheckAttested {
            context,
            subject: candidate,
            name: "unit".into(),
            passed: true,
        },
    );
    let revision = integrate(&mut f, context, None, candidate, vec![review, check]);
    assert!(f.goal().state().workspace_proposals[&candidate].approved);
    assert_eq!(head(&f.goal()), Some(revision));
}

#[test]
fn workspace_explicit_restoration_recovers_consecutive_invalid_attempts_and_retains_seed() {
    let mut f = Fixture::new(workspace_formation(1));
    let initial = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (seed_proposal, selected, _) = seed(&mut f, initial);
    let retained = epoch(
        &mut f,
        Some(initial.round),
        WorkspaceCheckpoint::Revision(selected),
    );
    let missing = epoch(
        &mut f,
        Some(retained.round),
        WorkspaceCheckpoint::Revision(EventId([99; 32])),
    );
    let wrong = epoch(
        &mut f,
        Some(missing.round),
        WorkspaceCheckpoint::Revision(seed_proposal),
    );
    let repaired = epoch(
        &mut f,
        Some(wrong.round),
        WorkspaceCheckpoint::RetainBefore {
            epoch: missing.round,
        },
    );
    let goal = f.goal();
    let workspace = goal.state().workspace.as_ref().unwrap();
    assert_eq!(workspace.epoch, repaired.round);
    assert_eq!(workspace.checkpoint, Some(selected));
    assert!(workspace.ready);
    assert_eq!(workspace.head, Some(selected));
    let candidate = proposal(&mut f, 0, repaired, Some(selected), Vec::new());
    let review = f.review(1, repaired, candidate);
    let next = integrate(&mut f, repaired, None, candidate, vec![review]);
    assert_eq!(head(&f.goal()), Some(next));
    assert_workspace_replay_and_restart(&f);
}

#[test]
fn workspace_explicit_restoration_recovers_an_invalid_first_epoch() {
    let mut f = Fixture::new(workspace_formation(1));
    let blocked = epoch(
        &mut f,
        None,
        WorkspaceCheckpoint::Revision(EventId([99; 32])),
    );
    let repaired = epoch(
        &mut f,
        Some(blocked.round),
        WorkspaceCheckpoint::RetainBefore {
            epoch: blocked.round,
        },
    );
    let (_, selected, _) = seed(&mut f, repaired);
    assert_eq!(head(&f.goal()), Some(selected));
    assert_workspace_replay_and_restart(&f);
}

#[test]
fn workspace_unseeded_handoff_fences_an_undelivered_old_seed() {
    let mut f = Fixture::new(workspace_formation(1));
    let old = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (_, old_seed, _) = seed(&mut f, old);
    let new = epoch(&mut f, Some(old.round), WorkspaceCheckpoint::Unseeded);
    let (_, selected, _) = seed(&mut f, new);
    let without: Vec<_> = f
        .events
        .iter()
        .filter(|e| e.id() != old_seed)
        .cloned()
        .collect();
    let mut goal = Goal::new(f.id);
    goal.apply(&without, &f.definitions);
    goal.apply(std::slice::from_ref(f.event(old_seed)), &f.definitions);
    assert_eq!(head(&goal), Some(selected));
    assert!(!goal.state().workspace_lineage.contains(&old_seed));
    assert_workspace_replay_and_restart(&f);
}

#[test]
fn workspace_ordinary_unseeded_handoff_cannot_erase_a_retained_revision() {
    let mut f = Fixture::new(workspace_formation(1));
    let initial = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (_, selected, _) = seed(&mut f, initial);
    let retained = epoch(
        &mut f,
        Some(initial.round),
        WorkspaceCheckpoint::Revision(selected),
    );
    let invalid = epoch(&mut f, Some(retained.round), WorkspaceCheckpoint::Unseeded);
    let (_, bad_selection, _) = seed(&mut f, invalid);
    let goal = f.goal();
    assert_eq!(
        goal.state().workspace.as_ref().unwrap().epoch,
        invalid.round
    );
    assert!(!goal.state().workspace.as_ref().unwrap().ready);
    assert!(!goal.standing(&bad_selection).unwrap().is_effective());
    assert_eq!(head(&goal), None);
}

#[test]
fn workspace_reconciliation_excludes_old_revision_bases_from_current_lineage() {
    let mut f = Fixture::new(workspace_formation(1));
    let old = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (_, selected, _) = seed(&mut f, old);
    let other = proposal(&mut f, 0, old, None, Vec::new());
    let review = f.review(1, old, other);
    let excluded = integrate(&mut f, old, None, other, vec![review]);
    let new = epoch(
        &mut f,
        Some(old.round),
        WorkspaceCheckpoint::Revision(selected),
    );
    let stale = proposal(&mut f, 0, new, Some(excluded), Vec::new());
    let stale_review = f.review(1, new, stale);
    let bad = integrate(&mut f, new, None, stale, vec![stale_review]);
    let goal = f.goal();
    assert_eq!(head(&goal), Some(selected));
    assert!(!goal.state().workspace_lineage.contains(&excluded));
    assert!(!goal.standing(&bad).unwrap().is_effective());
}

#[test]
fn workspace_composition_can_retain_a_source_based_on_the_chosen_checkpoint_branch() {
    let mut f = Fixture::new(workspace_formation(1));
    let old = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (_, chosen, _) = seed(&mut f, old);
    let source = proposal(&mut f, 0, old, Some(chosen), Vec::new());
    f.fork(chosen, 4);
    let new = epoch(
        &mut f,
        Some(old.round),
        WorkspaceCheckpoint::Revision(chosen),
    );
    let candidate = proposal(&mut f, 0, new, Some(chosen), vec![source]);
    let review = f.review(1, new, candidate);
    let revision = integrate(&mut f, new, None, candidate, vec![review]);
    let goal = f.goal();
    assert_eq!(head(&goal), Some(revision));
    assert_eq!(goal.standing(&revision), Some(Standing::Effective));
    assert_workspace_replay_and_restart(&f);
}

#[test]
fn workspace_composition_cannot_retain_a_source_based_on_an_excluded_checkpoint_sibling() {
    let mut f = Fixture::new(workspace_formation(1));
    let old = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (_, chosen, _) = seed(&mut f, old);
    let other = proposal(&mut f, 0, old, None, Vec::new());
    let other_review = f.review(1, old, other);
    let excluded = integrate(&mut f, old, None, other, vec![other_review]);
    let source = proposal(&mut f, 0, old, Some(excluded), Vec::new());
    let new = epoch(
        &mut f,
        Some(old.round),
        WorkspaceCheckpoint::Revision(chosen),
    );
    let candidate = proposal(&mut f, 0, new, Some(chosen), vec![source]);
    let review = f.review(1, new, candidate);
    let revision = integrate(&mut f, new, None, candidate, vec![review]);
    let goal = f.goal();
    assert!(!goal.standing(&revision).unwrap().is_effective());
    assert_eq!(head(&goal), Some(chosen));
    assert!(!goal.state().workspace_lineage.contains(&excluded));
    assert_workspace_replay_and_restart(&f);
}

#[test]
fn workspace_pinned_epoch_survives_unrelated_rules_rebinding_then_explicitly_disables() {
    let mut f = Fixture::new(workspace_formation(1));
    let initial = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let candidate = proposal(&mut f, 0, initial, None, Vec::new());
    let replacement = Formation::default();
    let hash = testkit::definition_hash(&replacement);
    f.definitions.insert(hash, replacement.clone());
    let (binding, _) = testkit::rules_binding(&f.id, 0, &replacement, BTreeMap::new());
    let rebound = f.admin(Body::RulesBound {
        expected: Some(f.rules),
        binding,
    });
    let review = f.review(1, initial, candidate);
    let selected = integrate(&mut f, initial, None, candidate, vec![review]);
    let before = f.goal();
    assert_eq!(head(&before), Some(selected));
    assert_eq!(
        before.state().workspace.as_ref().unwrap().epoch,
        initial.round
    );
    assert!(before.state().workspace.as_ref().unwrap().enabled);
    f.rules = rebound;
    let disabled = epoch(
        &mut f,
        Some(initial.round),
        WorkspaceCheckpoint::Revision(selected),
    );
    let goal = f.goal();
    let workspace = goal.state().workspace.as_ref().unwrap();
    assert_eq!(workspace.epoch, disabled.round);
    assert_eq!(workspace.head, Some(selected));
    assert!(workspace.ready);
    assert!(!workspace.enabled);
    assert_workspace_replay_and_restart(&f);
}

#[test]
fn workspace_membership_cutoff_preserves_exact_earlier_review_but_rejects_later_evidence() {
    let mut f = Fixture::new(workspace_formation(1));
    let context = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let (_, selected, retained_review) = seed(&mut f, context);
    let next = proposal(&mut f, 0, context, Some(selected), Vec::new());
    let rejected_review = f.review(1, context, next);
    let rejected = integrate(&mut f, context, Some(selected), next, vec![rejected_review]);
    let cutoff = AuthorPoint {
        seq: f.event(retained_review).header().seq,
        id: retained_review,
    };
    f.admin(Body::MemberRemoved {
        member: f.workers[1].key.public(),
        admission: f.admissions[2],
        last_accepted: Some(cutoff),
    });
    let goal = f.goal();
    assert_eq!(goal.standing(&selected), Some(Standing::Effective));
    assert!(!goal.standing(&rejected_review).unwrap().is_effective());
    assert!(!goal.standing(&rejected).unwrap().is_effective());
    assert_eq!(head(&goal), Some(selected));
    let handoff = epoch(
        &mut f,
        Some(context.round),
        WorkspaceCheckpoint::Revision(selected),
    );
    assert_eq!(
        f.goal().state().workspace.as_ref().unwrap().epoch,
        handoff.round
    );
    assert_eq!(head(&f.goal()), Some(selected));
    assert_workspace_replay_and_restart(&f);
}

#[test]
fn workspace_administrator_epoch_fork_retracts_the_governance_suffix() {
    let mut f = Fixture::new(workspace_formation(1));
    let context = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    seed(&mut f, context);
    f.fork(context.round, 1);
    let goal = f.goal();
    assert!(goal.evaluation().admin_halt.is_some());
    assert!(goal.state().workspace.is_none());
    assert_eq!(head(&goal), None);
    assert_workspace_replay_and_restart(&f);
}

#[test]
fn workspace_checkpoint_cannot_substitute_another_goal_selection() {
    let mut f = Fixture::new(workspace_formation(1));
    let initial = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let mut other = Fixture::new(workspace_formation(2));
    let other_context = epoch(&mut other, None, WorkspaceCheckpoint::Unseeded);
    let candidate = proposal(&mut other, 0, other_context, None, Vec::new());
    let first_review = other.review(1, other_context, candidate);
    let second_review = other.review(2, other_context, candidate);
    let foreign = integrate(
        &mut other,
        other_context,
        None,
        candidate,
        vec![first_review, second_review],
    );
    assert_ne!(f.id, other.id);
    f.events.extend(other.events);
    let blocked = epoch(
        &mut f,
        Some(initial.round),
        WorkspaceCheckpoint::Revision(foreign),
    );
    let goal = f.goal();
    assert_eq!(
        goal.state().workspace.as_ref().unwrap().epoch,
        blocked.round
    );
    assert!(!goal.state().workspace.as_ref().unwrap().ready);
    assert_eq!(head(&goal), None);
}

fn host_integrator_competition(same_position: bool) {
    let mut formation = workspace_formation(1);
    formation.workspace.as_mut().unwrap().integrator = Authority::Participant {
        key: testkit::keypair(1).public().to_string(),
    };
    let mut f = Fixture::new(formation);
    let context = epoch(&mut f, None, WorkspaceCheckpoint::Unseeded);
    let first = proposal(&mut f, 0, context, None, vec![]);
    let second = proposal(&mut f, 0, context, None, vec![]);
    let first_review = f.review(1, context, first);
    let second_review = f.review(1, context, second);
    let decision = |subject, review| Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![review],
    };
    let a = f.admin(decision(first, first_review));
    assert_eq!(head(&f.goal()), Some(a));
    let b = if same_position {
        fork_body(&mut f, a, 1, decision(second, second_review))
    } else {
        f.admin(decision(second, second_review))
    };
    let member = testkit::keypair(9).public();
    let admission = f.admin(Body::MemberAdmitted {
        member,
        endpoint: EndpointId(member.0),
    });
    let goal = f.goal();
    assert_eq!(head(&goal), None);
    if same_position {
        assert_eq!(f.event(a).header().seq, f.event(b).header().seq);
        assert!(goal.evaluation().admin_halt.is_some());
        assert!(!goal.standing(&admission).unwrap().is_effective());
        assert!(!goal.state().members.contains_key(&member));
        assert!(goal.next(&f.admin.key.public()).is_none());
    } else {
        assert_eq!(f.event(b).header().seq, f.event(a).header().seq + 1);
        assert_eq!(f.event(b).header().prev, Some(a));
        assert_eq!(goal.standing(&a), Some(Standing::Disputed));
        assert_eq!(goal.standing(&b), Some(Standing::Disputed));
        assert!(goal.evaluation().admin_halt.is_none());
        assert_eq!(goal.standing(&admission), Some(Standing::Effective));
        assert!(goal.state().members[&member].is_active());
    }
    assert_workspace_replay_and_restart(&f);
}

#[test]
fn host_integrator_acceptances_at_same_log_position_halt_governance() {
    host_integrator_competition(true);
}

#[test]
fn host_integrator_acceptances_at_distinct_log_positions_dispute_only_workspace() {
    host_integrator_competition(false);
}
