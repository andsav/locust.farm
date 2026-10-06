use std::collections::BTreeMap;

#[path = "workspace_tests.rs"]
mod workspace_tests;

use locust_proto::event::*;
use locust_proto::id::{DefinitionHash, EndpointId, EventId, GoalId};
use locust_proto::organization::{
    Authority, CompletionRule, EvidenceKind, Formation, Prerequisite, Selector, Stage,
};
use locust_proto::store::{Commit, MemStore, Store};
use locust_proto::testkit::{self, Author};

use crate::goal::{Goal, Standing, Waiting};

struct Fixture {
    admin: Author,
    workers: Vec<Author>,
    events: Vec<Event>,
    definitions: BTreeMap<DefinitionHash, Formation>,
    id: GoalId,
    anchor: EventId,
    rules: EventId,
    admissions: Vec<EventId>,
}
impl Fixture {
    fn new(formation: Formation) -> Self {
        let inspected = crate::organization::inspect(&serde_json::to_string(&formation).unwrap());
        assert!(inspected.valid, "{:?}", inspected.diagnostics);
        let formation = inspected.normalized.unwrap();
        let mut admin = Author::new(1);
        let workers: Vec<_> = (2..=4).map(Author::new).collect();
        let genesis = admin.genesis_with(&formation);
        let id = genesis.header().goal;
        let mut anchor = genesis.id();
        let mut events = vec![genesis];
        let mut admissions = Vec::new();
        for principal in std::iter::once(admin.key.public())
            .chain(workers.iter().map(|worker| worker.key.public()))
        {
            let event = admin.event(
                id,
                Some(anchor),
                Body::MemberAdmitted {
                    member: principal,
                    endpoint: EndpointId(principal.0),
                },
            );
            anchor = event.id();
            admissions.push(event.id());
            events.push(event);
        }
        let roles = formation
            .roles
            .keys()
            .map(|name| {
                let mut principals = match name.as_str() {
                    "reviewer" => vec![workers[1].key.public(), workers[2].key.public()],
                    _ => vec![admin.key.public()],
                };
                principals.sort();
                (name.clone(), principals)
            })
            .collect();
        let (binding, _) = testkit::rules_binding(&id, 0, &formation, roles);
        let bound = admin.event(
            id,
            Some(anchor),
            Body::RulesBound {
                expected: None,
                binding,
            },
        );
        anchor = bound.id();
        let rules = bound.id();
        events.push(bound);
        Self {
            admin,
            workers,
            events,
            definitions: BTreeMap::from([(testkit::definition_hash(&formation), formation)]),
            id,
            anchor,
            rules,
            admissions,
        }
    }
    fn admin(&mut self, body: Body) -> EventId {
        let governance = body.is_governance();
        let event = self.admin.event(self.id, Some(self.anchor), body);
        let id = event.id();
        if governance {
            self.anchor = id;
        }
        self.events.push(event);
        id
    }
    fn worker(&mut self, index: usize, body: Body) -> EventId {
        let event = self.workers[index].event(self.id, Some(self.anchor), body);
        let id = event.id();
        self.events.push(event);
        id
    }
    fn context(&self) -> Context {
        Context {
            scope: Scope::Goal,
            round: self.rules,
        }
    }
    fn task(&mut self) -> Context {
        let id = self.worker(
            0,
            Body::TaskOpened {
                binding: TaskBinding {
                    rules: self.rules,
                    task_type: None,
                    inputs: BTreeMap::new(),
                    parent: None,
                    stage: None,
                },
            },
        );
        Context {
            scope: Scope::Task(TaskId::Authored(id)),
            round: id,
        }
    }
    fn publish(&mut self, index: usize, context: Context) -> EventId {
        self.worker(
            index,
            Body::ContributionPublished {
                context,
                attempt: None,
                sources: Vec::new(),
                artifacts: Vec::new(),
            },
        )
    }
    fn review(&mut self, index: usize, context: Context, subject: EventId) -> EventId {
        self.worker(
            index,
            Body::ReviewRecorded {
                context,
                subject,
                verdict: ReviewVerdict::Approve,
            },
        )
    }
    fn goal(&self) -> Goal {
        let mut goal = Goal::new(self.id);
        goal.apply(&self.events, &self.definitions);
        goal
    }
    fn event(&self, id: EventId) -> &Event {
        self.events.iter().find(|event| event.id() == id).unwrap()
    }
    fn fork(&mut self, id: EventId, key: u8) -> EventId {
        let mut header = self.event(id).header().clone();
        header.at_ms += 10_000;
        let event = Event::sign(header, &testkit::keypair(key)).unwrap();
        let id = event.id();
        self.events.push(event);
        id
    }
}
fn review_formation(count: u32) -> Formation {
    let mut formation = Formation::default();
    formation.decisions.completion = CompletionRule::Reviews {
        by: Selector::Members,
        count,
        exclude_author: true,
    };
    formation.decisions.selection = Some(Authority::Participant {
        key: testkit::keypair(1).public().to_string(),
    });
    formation
}

#[test]
fn open_taskless_work_needs_no_administrator_decision() {
    let mut f = Fixture::new(Formation::default());
    let context = f.context();
    let first = f.publish(0, context);
    let second = f.publish(1, context);
    let goal = f.goal();
    assert_eq!(goal.standing(&first), Some(Standing::Effective));
    assert_eq!(goal.standing(&second), Some(Standing::Effective));
    assert_eq!(goal.state().contributions.len(), 2);
    assert!(goal.state().tasks.is_empty());
    assert!(goal.state().decisions.is_empty());
    assert_eq!(goal.state().head, Some(f.rules));
}

#[test]
fn unknown_definition_waits_and_refresh_uses_exact_hash() {
    let mut f = Fixture::new(Formation::default());
    let subject = f.publish(0, f.context());
    let mut goal = Goal::new(f.id);
    goal.apply(&f.events, &BTreeMap::new());
    assert!(goal.standing(&subject).unwrap().is_pending());
    let changes = goal.refresh(&f.definitions);
    assert!(changes.judged.contains(&subject));
    assert_eq!(goal.standing(&subject), Some(Standing::Effective));
    let hash = *f.definitions.keys().next().unwrap();
    let mut wrong = Formation::default();
    wrong.context.guidance = "not the pinned definition".into();
    goal.refresh(&BTreeMap::from([(hash, wrong)]));
    assert!(!goal.standing(&subject).unwrap().is_effective());
}

#[test]
fn independent_attempts_and_contributions_survive_reverse_arrival_and_restart() {
    let mut f = Fixture::new(Formation::default());
    let context = f.task();
    let a = f.worker(
        0,
        Body::AttemptStarted {
            context,
            offer: None,
            closure: None,
        },
    );
    let b = f.worker(
        1,
        Body::AttemptStarted {
            context,
            offer: None,
            closure: None,
        },
    );
    f.publish(0, context);
    f.publish(1, context);
    let expected = f.goal();
    assert_eq!(expected.state().attempts.len(), 2);
    let mut reversed = Goal::new(f.id);
    for event in f.events.iter().rev() {
        reversed.apply(std::slice::from_ref(event), &f.definitions);
    }
    assert_eq!(reversed.evaluation(), expected.evaluation());
    assert!(reversed.standing(&a).unwrap().is_effective());
    assert!(reversed.standing(&b).unwrap().is_effective());
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
fn threshold_counts_distinct_non_author_principals_on_the_exact_subject() {
    let mut f = Fixture::new(review_formation(2));
    let context = f.task();
    let first = f.publish(0, context);
    let second = f.publish(0, context);
    let self_review = f.review(0, context, first);
    f.review(1, context, first);
    f.review(1, context, first);
    let goal = f.goal();
    assert!(!goal.state().contributions[&first].approved);
    assert!(matches!(
        goal.standing(&self_review),
        Some(Standing::Excluded(_))
    ));
    f.review(2, context, first);
    let goal = f.goal();
    assert!(goal.state().contributions[&first].approved);
    assert!(!goal.state().contributions[&second].approved);
}

#[test]
fn member_fork_retracts_unpinned_approval_but_scoped_decision_retains_exact_proof() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let subject = f.publish(0, context);
    let review = f.review(1, context, subject);
    let mut before = f.goal();
    assert!(before.state().contributions[&subject].approved);
    let decision = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![review],
    });
    f.fork(review, 3);
    let changes = before.apply(&f.events, &f.definitions);
    assert!(changes.judged.contains(&review));
    assert_eq!(
        before.standing(&review),
        Some(Standing::Pending(Waiting::ForkProof))
    );
    assert!(!before.state().contributions[&subject].approved);
    assert_eq!(before.standing(&decision), Some(Standing::Effective));
    assert_eq!(
        before.state().task_round(context).unwrap().selected,
        Some(subject)
    );
    assert!(before.evaluation().retained.contains(&review));
}

#[test]
fn a_scoped_decision_waits_for_its_exact_missing_review() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let subject = f.publish(0, context);
    let review = f.review(1, context, subject);
    let decision = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![review],
    });
    let mut goal = Goal::new(f.id);
    let without: Vec<_> = f
        .events
        .iter()
        .filter(|event| event.id() != review)
        .cloned()
        .collect();
    goal.apply(&without, &f.definitions);
    assert!(goal.standing(&decision).unwrap().is_pending());
    goal.apply(std::slice::from_ref(f.event(review)), &f.definitions);
    assert_eq!(goal.standing(&decision), Some(Standing::Effective));
}

#[test]
fn scope_equivocation_halts_only_that_stream_and_keeps_other_work() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let a = f.publish(0, context);
    let b = f.publish(0, context);
    let ar = f.review(1, context, a);
    let br = f.review(1, context, b);
    let first = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject: a },
        evidence: vec![ar],
    });
    let second = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject: b },
        evidence: vec![br],
    });
    let open = f.publish(2, f.context());
    let goal = f.goal();
    assert_eq!(goal.standing(&first), Some(Standing::Disputed));
    assert_eq!(goal.standing(&second), Some(Standing::Disputed));
    assert_eq!(goal.evaluation().scope_halts.len(), 1);
    assert!(goal.evaluation().admin_halt.is_none());
    assert_eq!(goal.standing(&open), Some(Standing::Effective));
}

#[test]
fn a_selection_cannot_substitute_another_task_or_count_unlisted_reviews() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let other = f.task();
    let candidate = f.publish(0, other);
    let review = f.review(1, other, candidate);
    let wrong = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject: candidate },
        evidence: vec![review],
    });
    assert!(!f.goal().standing(&wrong).unwrap().is_effective());
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let candidate = f.publish(0, context);
    f.review(1, context, candidate);
    let incomplete = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject: candidate },
        evidence: vec![],
    });
    assert_eq!(
        f.goal().standing(&incomplete),
        Some(Standing::Pending(Waiting::Evidence))
    );
}

#[test]
fn removal_retains_only_exact_cutoff_ancestry_and_readmission_does_not_backdate() {
    let mut f = Fixture::new(Formation::default());
    let context = f.context();
    let first = f.publish(0, context);
    let last = f.publish(0, context);
    let after = f.publish(0, context);
    let fork = f.fork(first, 2);
    let point = AuthorPoint {
        seq: f.event(last).header().seq,
        id: last,
    };
    f.admin(Body::MemberRemoved {
        member: f.workers[0].key.public(),
        admission: f.admissions[1],
        last_accepted: Some(point),
    });
    let goal = f.goal();
    assert_eq!(goal.standing(&first), Some(Standing::Effective));
    assert_eq!(goal.standing(&last), Some(Standing::Effective));
    assert!(matches!(goal.standing(&fork), Some(Standing::Excluded(_))));
    assert!(matches!(goal.standing(&after), Some(Standing::Excluded(_))));
    assert!(!goal.state().is_member(&f.workers[0].key.public()));
    f.admin(Body::MemberAdmitted {
        member: f.workers[0].key.public(),
        endpoint: EndpointId([2; 32]),
    });
    let goal = f.goal();
    assert!(goal.state().is_member(&f.workers[0].key.public()));
    assert!(matches!(goal.standing(&after), Some(Standing::Excluded(_))));
}

#[test]
fn missing_or_invalid_retention_cutoff_does_not_reopen_membership() {
    let mut f = Fixture::new(Formation::default());
    let subject = f.publish(0, f.context());
    f.admin(Body::MemberRemoved {
        member: f.workers[0].key.public(),
        admission: f.admissions[1],
        last_accepted: Some(AuthorPoint {
            seq: 0,
            id: EventId([9; 32]),
        }),
    });
    let goal = f.goal();
    assert!(!goal.state().is_member(&f.workers[0].key.public()));
    assert!(goal.standing(&subject).unwrap().is_pending());
    let mut f = Fixture::new(Formation::default());
    let subject = f.publish(0, f.context());
    let wrong = f.publish(1, f.context());
    f.admin(Body::MemberRemoved {
        member: f.workers[0].key.public(),
        admission: f.admissions[1],
        last_accepted: Some(AuthorPoint { seq: 0, id: wrong }),
    });
    let goal = f.goal();
    assert!(!goal.state().is_member(&f.workers[0].key.public()));
    assert!(matches!(
        goal.standing(&subject),
        Some(Standing::Excluded(_))
    ));
}

#[test]
fn active_round_revision_never_reinterprets_old_evidence() {
    let mut f = Fixture::new(Formation::default());
    let old = f.task();
    let first = f.publish(0, old);
    let Scope::Task(task) = old.scope else {
        unreachable!()
    };
    let revised = f.admin(Body::TaskRevised {
        task,
        expected_round: old.round,
        binding: TaskBinding {
            rules: f.rules,
            task_type: None,
            inputs: BTreeMap::new(),
            parent: None,
            stage: None,
        },
    });
    let stale = f.publish(0, old);
    let current = Context {
        scope: old.scope,
        round: revised,
    };
    let fresh = f.publish(0, current);
    let goal = f.goal();
    assert_eq!(goal.standing(&first), Some(Standing::Effective));
    assert!(matches!(goal.standing(&stale), Some(Standing::Excluded(_))));
    assert_eq!(goal.standing(&fresh), Some(Standing::Effective));
    assert_eq!(goal.current_context(old.scope), Some(current));
}

fn pipeline() -> Formation {
    let mut formation = Formation::default();
    formation.flow.insert(
        "research".into(),
        Stage {
            task_type: None,
            requires: Vec::new(),
            recipients: Selector::Members,
        },
    );
    formation.flow.insert(
        "build".into(),
        Stage {
            task_type: None,
            requires: vec![Prerequisite {
                stage: "research".into(),
                evidence: EvidenceKind::Completion,
            }],
            recipients: Selector::Members,
        },
    );
    formation
}

#[test]
fn daemon_effects_advance_configured_stages_and_deduplicate_logical_work() {
    let mut f = Fixture::new(pipeline());
    let initial = f.goal();
    assert_eq!(initial.evaluation().desired_effects.len(), 1);
    let first = initial
        .evaluation()
        .desired_effects
        .values()
        .next()
        .unwrap()
        .clone();
    let created = f.admin(Body::EffectMaterialized {
        effect: first.effect.clone(),
    });
    // A duplicated authored materialization is still one logical task.
    f.admin(Body::EffectMaterialized {
        effect: first.effect,
    });
    let goal = f.goal();
    assert_eq!(goal.state().effects.len(), 1);
    assert_eq!(goal.state().tasks.len(), 1);
    assert!(goal.evaluation().desired_effects.is_empty());
    let context = Context {
        scope: Scope::Task(TaskId::Derived(first.id)),
        round: created,
    };
    let subject = f.publish(0, context);
    f.worker(0, Body::CompletionDeclared { context, subject });
    let goal = f.goal();
    assert_eq!(goal.evaluation().desired_effects.len(), 1);
    let next = goal.evaluation().desired_effects.values().next().unwrap();
    assert_eq!(next.effect.transition, "stage:build");
    let mut reverse = Goal::new(f.id);
    let mut events = f.events.clone();
    events.reverse();
    reverse.apply(&events, &f.definitions);
    assert_eq!(reverse.evaluation(), goal.evaluation());
}

#[test]
fn ordinary_publication_automatically_requires_review_delivery_by_its_author() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let subject = f.publish(0, context);
    let goal = f.goal();
    let pending: Vec<_> = goal.evaluation().desired_effects.values().collect();
    assert_eq!(pending.len(), 3);
    assert!(
        pending
            .iter()
            .all(|effect| effect.runner == f.workers[0].key.public())
    );
    let desired = pending[0].clone();
    let wrong = f.admin(Body::EffectMaterialized {
        effect: desired.effect.clone(),
    });
    assert!(matches!(
        f.goal().standing(&wrong),
        Some(Standing::Excluded(_))
    ));
    let materialized = f.worker(
        0,
        Body::EffectMaterialized {
            effect: desired.effect.clone(),
        },
    );
    assert_eq!(f.goal().standing(&materialized), Some(Standing::Effective));
    assert!(matches!(desired.effect.trigger,Trigger::Contribution(id) if id==subject));
}

#[test]
fn effect_cannot_change_recipients_and_fork_retraction_removes_outbox_projection() {
    let mut f = Fixture::new(pipeline());
    let mut effect = f
        .goal()
        .evaluation()
        .desired_effects
        .values()
        .next()
        .unwrap()
        .effect
        .clone();
    let EffectAction::OpenTask { recipients, .. } = &mut effect.action else {
        unreachable!()
    };
    recipients.clear();
    let wrong = f.admin(Body::EffectMaterialized { effect });
    assert!(matches!(
        f.goal().standing(&wrong),
        Some(Standing::Excluded(_))
    ));
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let subject = f.publish(0, context);
    let desired = f
        .goal()
        .evaluation()
        .desired_effects
        .values()
        .next()
        .unwrap()
        .clone();
    let event = f.worker(
        0,
        Body::EffectMaterialized {
            effect: desired.effect,
        },
    );
    let mut goal = f.goal();
    assert!(goal.state().effects.contains_key(&desired.id));
    f.fork(subject, 2);
    let changes = goal.apply(&f.events, &f.definitions);
    assert!(changes.judged.contains(&event));
    assert!(goal.state().effects.is_empty());
}

#[test]
fn shared_document_selection_requires_the_same_exact_review_evidence() {
    let mut f = Fixture::new(review_formation(1));
    let context = Context {
        scope: Scope::Document(Doc::Plan),
        round: f.rules,
    };
    let revision = f.worker(
        0,
        Body::DocumentRevised {
            context,
            doc: Doc::Plan,
            base: None,
        },
    );
    let goal = f.goal();
    assert!(!goal.state().revisions[&revision].approved);
    assert!(!goal.evaluation().desired_effects.is_empty());
    let review = f.review(1, context, revision);
    let decision = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject: revision },
        evidence: vec![review],
    });
    let goal = f.goal();
    assert!(goal.state().revisions[&revision].approved);
    assert_eq!(goal.standing(&decision), Some(Standing::Effective));
    assert_eq!(goal.state().documents[&Doc::Plan].selected, Some(revision));
    let mut f = Fixture::new(review_formation(1));
    let context = Context {
        scope: Scope::Document(Doc::Summary),
        round: f.rules,
    };
    let revision = f.worker(
        0,
        Body::DocumentRevised {
            context,
            doc: Doc::Summary,
            base: None,
        },
    );
    let decision = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject: revision },
        evidence: vec![],
    });
    assert_eq!(
        f.goal().standing(&decision),
        Some(Standing::Pending(Waiting::Evidence))
    );
}

#[test]
fn same_slot_scope_authority_equivocation_is_explicitly_disputed() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let subject = f.publish(0, context);
    let review = f.review(1, context, subject);
    let decision = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![review],
    });
    let fork = f.fork(decision, 1);
    let goal = f.goal();
    assert_eq!(goal.standing(&decision), Some(Standing::Disputed));
    assert_eq!(goal.standing(&fork), Some(Standing::Disputed));
    assert_eq!(goal.evaluation().scope_halts.len(), 1);
    assert_eq!(goal.standing(&subject), Some(Standing::Effective));
}

#[test]
fn usable_prefix_lookup_matches_scan_across_gaps_forks_and_duplicate_arrival() {
    let mut f = Fixture::new(Formation::default());
    let mut subjects = Vec::new();
    for _ in 0..8 {
        subjects.push(f.publish(0, f.context()));
        f.publish(1, f.context());
    }
    f.fork(subjects[3], 2);
    for shift in 0..f.events.len() {
        let mut incoming = f.events.clone();
        incoming.rotate_left(shift);
        if shift % 2 == 1 {
            incoming.reverse();
        }
        let mut history = super::history::History::default();
        for event in &incoming {
            history.insert(event);
            assert!(history.insert(event).is_none());
            for candidate in &f.events {
                if let Some(log) = history.log(&candidate.header().author) {
                    let point = AuthorPoint {
                        seq: candidate.header().seq,
                        id: candidate.id(),
                    };
                    let scanned = log.points[..log.usable]
                        .iter()
                        .any(|held| held.id == point.id);
                    assert_eq!(log.contains_usable(point), scanned);
                    assert!(!log.contains_usable(AuthorPoint {
                        seq: u64::MAX,
                        id: candidate.id(),
                    }));
                }
            }
        }
    }
}

#[test]
fn decision_successors_keep_author_scope_purpose_and_predecessor_separate() {
    let mut formation = review_formation(1);
    formation.decisions.completion = CompletionRule::Contribution {
        by: Selector::Members,
    };
    formation.decisions.finish = formation.decisions.selection.clone();
    let mut f = Fixture::new(formation);
    let first = f.task();
    let other = f.task();
    let subject = f.publish(0, first);
    let other_subject = f.publish(0, other);
    let selected = f.admin(Body::ScopeDecided {
        context: first,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![],
    });
    let other_selected = f.admin(Body::ScopeDecided {
        context: other,
        previous: None,
        action: DecisionAction::Select {
            subject: other_subject,
        },
        evidence: vec![],
    });
    let closed = f.admin(Body::ScopeDecided {
        context: first,
        previous: None,
        action: DecisionAction::Close,
        evidence: vec![],
    });
    let reopened = f.admin(Body::ScopeDecided {
        context: first,
        previous: Some(closed),
        action: DecisionAction::Reopen,
        evidence: vec![],
    });
    let replacement = f.admin(Body::ScopeDecided {
        context: first,
        previous: Some(selected),
        action: DecisionAction::Select { subject },
        evidence: vec![],
    });
    let unauthorized = f.worker(
        1,
        Body::ScopeDecided {
            context: first,
            previous: None,
            action: DecisionAction::Select { subject },
            evidence: vec![],
        },
    );
    let mut before = f.goal();
    for id in [selected, other_selected, closed, reopened, replacement] {
        assert_eq!(before.standing(&id), Some(Standing::Effective));
    }
    assert!(matches!(
        before.standing(&unauthorized),
        Some(Standing::Excluded(_))
    ));
    let competing = f.admin(Body::ScopeDecided {
        context: first,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![],
    });
    before.apply(std::slice::from_ref(f.event(competing)), &f.definitions);
    for id in [selected, competing, replacement] {
        assert_eq!(before.standing(&id), Some(Standing::Disputed));
    }
    for id in [other_selected, closed, reopened] {
        assert_eq!(before.standing(&id), Some(Standing::Effective));
    }
    let expected = f.goal();
    assert_eq!(before.evaluation(), expected.evaluation());
    for shift in 0..f.events.len() {
        let mut incoming = f.events.clone();
        incoming.rotate_left(shift);
        if shift % 2 == 1 {
            incoming.reverse();
        }
        let mut actual = Goal::new(f.id);
        for batch in incoming.chunks(3) {
            actual.apply(batch, &f.definitions);
        }
        assert_eq!(actual.evaluation(), expected.evaluation());
    }
}

#[test]
fn removal_payload_uses_rotated_epoch_and_rejects_old_epoch() {
    for epoch in [0, 1] {
        let mut f = Fixture::new(Formation::default());
        let removal = f.admin(Body::MemberRemoved {
            member: f.workers[0].key.public(),
            admission: f.admissions[1],
            last_accepted: None,
        });
        let mut header = f.event(removal).header().clone();
        header.payload = Some(PayloadRef {
            hash: locust_proto::id::BlobHash([42; 32]),
            len: locust_proto::seal::OVERHEAD_BYTES as u32,
            key_epoch: epoch,
        });
        f.events.pop();
        let signed = Event::sign(header, &f.admin.key).unwrap();
        let id = signed.id();
        f.events.push(signed);
        let goal = f.goal();
        if epoch == 1 {
            assert_eq!(goal.standing(&id), Some(Standing::Effective));
            assert_eq!(goal.state().epoch, 1);
            assert!(!goal.state().is_member(&f.workers[0].key.public()));
        } else {
            assert_eq!(
                goal.standing(&id),
                Some(Standing::Excluded(crate::goal::Exclusion::BadEpoch))
            );
            assert_eq!(goal.state().epoch, 0);
        }
    }
}

#[test]
fn only_delivery_recipients_can_acknowledge_and_ack_does_not_start_work() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    f.publish(0, context);
    let goal = f.goal();
    let desired = goal
        .evaluation()
        .desired_effects
        .values()
        .find(|effect| effect.recipients.contains(&f.workers[1].key.public()))
        .unwrap()
        .clone();
    f.worker(
        0,
        Body::EffectMaterialized {
            effect: desired.effect,
        },
    );
    let unauthorized = f.worker(0, Body::DeliveryAcknowledged { effect: desired.id });
    let authorized = f.worker(1, Body::DeliveryAcknowledged { effect: desired.id });
    let goal = f.goal();
    assert!(matches!(
        goal.standing(&unauthorized),
        Some(Standing::Excluded(_))
    ));
    assert_eq!(goal.standing(&authorized), Some(Standing::Effective));
    assert_eq!(goal.state().effects[&desired.id].acknowledged.len(), 1);
    assert!(goal.state().attempts.is_empty());
}

#[test]
fn incompatible_proof_branches_dispute_only_their_scope() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let subject = f.publish(0, context);
    let review = f.review(1, context, subject);
    let sibling = f.fork(review, 3);
    let decision = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![review, sibling],
    });
    let goal = f.goal();
    assert_eq!(goal.standing(&decision), Some(Standing::Disputed));
    assert_eq!(goal.evaluation().scope_halts.len(), 1);
    assert_eq!(goal.standing(&subject), Some(Standing::Effective));
}

#[test]
fn scope_proof_cannot_retain_evidence_past_the_administrator_cutoff() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let subject = f.publish(0, context);
    let review = f.review(1, context, subject);
    f.admin(Body::MemberRemoved {
        member: f.workers[1].key.public(),
        admission: f.admissions[2],
        last_accepted: None,
    });
    let decision = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![review],
    });
    let goal = f.goal();
    assert!(matches!(
        goal.standing(&review),
        Some(Standing::Excluded(_))
    ));
    assert!(matches!(
        goal.standing(&decision),
        Some(Standing::Excluded(_))
    ));
    assert!(goal.state().decisions.is_empty());
}

#[test]
fn selection_predecessor_cannot_cross_task_scopes() {
    let mut f = Fixture::new(review_formation(1));
    let first = f.task();
    let subject = f.publish(0, first);
    let review = f.review(1, first, subject);
    let previous = f.admin(Body::ScopeDecided {
        context: first,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![review],
    });
    let second = f.task();
    let subject = f.publish(0, second);
    let review = f.review(1, second, subject);
    let wrong = f.admin(Body::ScopeDecided {
        context: second,
        previous: Some(previous),
        action: DecisionAction::Select { subject },
        evidence: vec![review],
    });
    let goal = f.goal();
    assert_eq!(goal.standing(&previous), Some(Standing::Effective));
    assert!(matches!(goal.standing(&wrong), Some(Standing::Excluded(_))));
}

#[test]
fn accepted_fork_branch_is_readable_only_in_its_selected_scope() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let subject = f.publish(0, context);
    let review = f.review(1, context, subject);
    let selection = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![review],
    });
    let unrelated = f.publish(0, f.context());
    f.fork(context.round, 2);
    let goal = f.goal();
    assert_eq!(goal.standing(&selection), Some(Standing::Effective));
    assert_eq!(
        goal.standing(&subject),
        Some(Standing::Pending(Waiting::ForkProof))
    );
    assert_eq!(
        goal.standing(&unrelated),
        Some(Standing::Pending(Waiting::ForkProof))
    );
    assert!(!goal.state().contributions.contains_key(&subject));
    assert!(goal.state().tasks.is_empty());
    let key = ScopeKey {
        context,
        purpose: DecisionPurpose::Selection,
    };
    let accepted = goal.selection(&key).unwrap();
    assert_eq!(accepted.decision, selection);
    assert_eq!(accepted.subject.id(), subject);
    assert_eq!(accepted.task.as_ref().unwrap().selected, Some(subject));
    assert_eq!(accepted.task.as_ref().unwrap().binding.rules, f.rules);
    assert_eq!(goal.state().selections.len(), 1);
    let Scope::Task(task) = context.scope else {
        unreachable!()
    };
    assert_eq!(
        goal.selected_task(task).unwrap().current_round,
        context.round
    );
    assert_eq!(
        goal.selected_rules(context, &f.definitions).unwrap().rules,
        f.rules
    );
    assert!(goal.effective_rules(context, &f.definitions).is_none());
    assert!(!goal.can_start(context, f.workers[0].key.public(), None, &f.definitions));
    for shift in 0..f.events.len() {
        let mut incoming = f.events.clone();
        incoming.rotate_left(shift);
        if shift % 2 == 1 {
            incoming.reverse();
        }
        let mut replica = Goal::new(f.id);
        for chunk in incoming.chunks(3) {
            replica.apply(chunk, &f.definitions);
        }
        assert_eq!(replica.evaluation(), goal.evaluation());
    }
}

#[test]
fn nested_tasks_inherit_parent_authority_and_reject_widening() {
    use locust_proto::organization::{DecisionRules, TaskType, WorkRules};
    let owner = Author::new(2).key.public();
    let restricted = WorkRules {
        propose: Selector::Participant {
            key: owner.to_string(),
        },
        publish: Selector::Participant {
            key: owner.to_string(),
        },
        starts: vec![locust_proto::organization::StartRule::Independent {
            by: Selector::TaskCreator,
        }],
    };
    let mut formation = Formation::default();
    formation.task_types.insert(
        "restricted".into(),
        TaskType {
            work: Some(restricted),
            decisions: Some(DecisionRules {
                completion: CompletionRule::Reviews {
                    by: Selector::Members,
                    count: 2,
                    exclude_author: true,
                },
                ..Default::default()
            }),
        },
    );
    formation.task_types.insert(
        "wide".into(),
        TaskType {
            work: Some(WorkRules::default()),
            decisions: Some(DecisionRules::default()),
        },
    );
    formation.task_types.insert(
        "weak".into(),
        TaskType {
            work: None,
            decisions: Some(DecisionRules::default()),
        },
    );
    let mut f = Fixture::new(formation);
    let binding = |task_type: Option<&str>, parent| TaskBinding {
        rules: f.rules,
        task_type: task_type.map(str::to_owned),
        inputs: BTreeMap::new(),
        parent,
        stage: None,
    };
    let parent_binding = binding(Some("restricted"), None);
    let parent_id = f.worker(
        0,
        Body::TaskOpened {
            binding: parent_binding,
        },
    );
    let parent = Context {
        scope: Scope::Task(TaskId::Authored(parent_id)),
        round: parent_id,
    };
    let binding = |task_type: Option<&str>| TaskBinding {
        rules: f.rules,
        task_type: task_type.map(str::to_owned),
        inputs: BTreeMap::new(),
        parent: Some(parent),
        stage: None,
    };
    let inherited = binding(None);
    let wide = binding(Some("wide"));
    let weak = binding(Some("weak"));
    let child = f.worker(
        0,
        Body::TaskOpened {
            binding: inherited.clone(),
        },
    );
    let outsider = f.worker(1, Body::TaskOpened { binding: inherited });
    let wide = f.worker(0, Body::TaskOpened { binding: wide });
    let weak = f.worker(0, Body::TaskOpened { binding: weak });
    let goal = f.goal();
    assert_eq!(goal.standing(&child), Some(Standing::Effective));
    for rejected in [outsider, wide, weak] {
        assert!(matches!(
            goal.standing(&rejected),
            Some(Standing::Excluded(_))
        ));
    }
    let effective = goal
        .effective_rules(
            Context {
                scope: Scope::Task(TaskId::Authored(child)),
                round: child,
            },
            &f.definitions,
        )
        .unwrap();
    assert_eq!(
        effective.work.propose,
        Selector::Participant {
            key: owner.to_string()
        }
    );
    assert!(matches!(
        effective.decisions.completion,
        CompletionRule::Reviews { count: 2, .. }
    ));
}

#[test]
fn nested_task_keeps_parent_rules_after_future_defaults_change() {
    let mut f = Fixture::new(Formation::default());
    let parent = f.task();
    let old_rules = f.rules;
    let (binding, _) = testkit::rules_binding(&f.id, 0, &Formation::default(), BTreeMap::new());
    f.admin(Body::RulesBound {
        expected: Some(old_rules),
        binding,
    });
    let child = f.worker(
        0,
        Body::TaskOpened {
            binding: TaskBinding {
                rules: old_rules,
                task_type: None,
                inputs: BTreeMap::new(),
                parent: Some(parent),
                stage: None,
            },
        },
    );
    assert_eq!(f.goal().standing(&child), Some(Standing::Effective));
}

#[test]
fn starts_bind_causal_closure_without_rejecting_concurrent_offline_work() {
    let mut formation = Formation::default();
    formation.decisions.finish = Some(Authority::Participant {
        key: Author::new(1).key.public().to_string(),
    });
    let mut f = Fixture::new(formation);
    let context = f.task();
    let close = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Close,
        evidence: vec![],
    });
    // This worker's signed ancestry has not observed the concurrent close.
    let concurrent = f.worker(
        0,
        Body::AttemptStarted {
            context,
            offer: None,
            closure: None,
        },
    );
    let observed_close = f.worker(
        1,
        Body::AttemptStarted {
            context,
            offer: None,
            closure: Some(close),
        },
    );
    let omitted_close = f.worker(
        1,
        Body::AttemptStarted {
            context,
            offer: None,
            closure: None,
        },
    );
    let reopen = f.admin(Body::ScopeDecided {
        context,
        previous: Some(close),
        action: DecisionAction::Reopen,
        evidence: vec![],
    });
    let reopened = f.worker(
        1,
        Body::AttemptStarted {
            context,
            offer: None,
            closure: Some(reopen),
        },
    );
    let regressed = f.worker(
        1,
        Body::AttemptStarted {
            context,
            offer: None,
            closure: Some(close),
        },
    );
    let goal = f.goal();
    for accepted in [concurrent, reopened] {
        assert_eq!(goal.standing(&accepted), Some(Standing::Effective));
    }
    for rejected in [observed_close, omitted_close, regressed] {
        assert!(matches!(
            goal.standing(&rejected),
            Some(Standing::Excluded(_))
        ));
    }
    let mut missing = Goal::new(f.id);
    missing.apply(
        &f.events
            .iter()
            .filter(|event| event.id() != reopen)
            .cloned()
            .collect::<Vec<_>>(),
        &f.definitions,
    );
    assert!(matches!(
        missing.standing(&reopened),
        Some(Standing::Pending(_))
    ));
    missing.apply(&[f.event(reopen).clone()], &f.definitions);
    assert_eq!(missing.evaluation(), goal.evaluation());
    for reverse in [false, true] {
        let mut events = f.events.clone();
        if reverse {
            events.reverse();
        }
        let mut store = MemStore::new();
        store
            .commit(&Commit {
                events,
                ..Commit::default()
            })
            .unwrap();
        assert_eq!(
            Goal::load(&store.reopen(), f.id, &f.definitions)
                .unwrap()
                .evaluation(),
            goal.evaluation()
        );
    }
}

#[test]
fn start_cannot_use_another_scope_or_selection_as_closure_position() {
    let mut formation = Formation::default();
    let authority = Authority::Participant {
        key: Author::new(1).key.public().to_string(),
    };
    formation.decisions.finish = Some(authority.clone());
    formation.decisions.selection = Some(authority);
    formation.decisions.completion = CompletionRule::Contribution {
        by: Selector::Members,
    };
    let mut f = Fixture::new(formation);
    let context = f.task();
    let another = f.task();
    let close = f.admin(Body::ScopeDecided {
        context: another,
        previous: None,
        action: DecisionAction::Close,
        evidence: vec![],
    });
    let subject = f.publish(0, context);
    let select = f.admin(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![subject],
    });
    let wrong_scope = f.worker(
        1,
        Body::AttemptStarted {
            context,
            offer: None,
            closure: Some(close),
        },
    );
    let wrong_purpose = f.worker(
        1,
        Body::AttemptStarted {
            context,
            offer: None,
            closure: Some(select),
        },
    );
    let goal = f.goal();
    for rejected in [wrong_scope, wrong_purpose] {
        assert!(matches!(
            goal.standing(&rejected),
            Some(Standing::Excluded(_))
        ));
    }
}

#[test]
fn new_child_cannot_reuse_parent_round_superseded_at_its_anchor() {
    let mut f = Fixture::new(Formation::default());
    let parent = f.task();
    let revised = f.admin(Body::TaskRevised {
        task: match parent.scope {
            Scope::Task(task) => task,
            _ => unreachable!(),
        },
        expected_round: parent.round,
        binding: TaskBinding {
            rules: f.rules,
            task_type: None,
            inputs: BTreeMap::new(),
            parent: None,
            stage: None,
        },
    });
    let child = f.worker(
        0,
        Body::TaskOpened {
            binding: TaskBinding {
                rules: f.rules,
                task_type: None,
                inputs: BTreeMap::new(),
                parent: Some(parent),
                stage: None,
            },
        },
    );
    let current = f.worker(
        0,
        Body::TaskOpened {
            binding: TaskBinding {
                rules: f.rules,
                task_type: None,
                inputs: BTreeMap::new(),
                parent: Some(Context {
                    round: revised,
                    ..parent
                }),
                stage: None,
            },
        },
    );
    let goal = f.goal();
    assert!(matches!(goal.standing(&child), Some(Standing::Excluded(_))));
    assert_eq!(goal.standing(&current), Some(Standing::Effective));
}

#[test]
fn document_selection_follows_governance_chronology_not_scopekey_hash_order() {
    let mut f = Fixture::new(review_formation(1));
    let doc = Doc::Plan;
    let first_rules = f.rules;
    // First governance round: revise and select the document.
    let first_context = Context {
        scope: Scope::Document(doc),
        round: first_rules,
    };
    let first_revision = f.worker(
        0,
        Body::DocumentRevised {
            context: first_context,
            doc,
            base: None,
        },
    );
    let first_review = f.review(1, first_context, first_revision);
    f.admin(Body::ScopeDecided {
        context: first_context,
        previous: None,
        action: DecisionAction::Select {
            subject: first_revision,
        },
        evidence: vec![first_review],
    });
    // Second governance round: bind new rules, then revise and select again.
    let (binding, _) = testkit::rules_binding(&f.id, 0, &review_formation(1), BTreeMap::new());
    let second_rules = f.admin(Body::RulesBound {
        expected: Some(first_rules),
        binding,
    });
    let second_context = Context {
        scope: Scope::Document(doc),
        round: second_rules,
    };
    let second_revision = f.worker(
        0,
        Body::DocumentRevised {
            context: second_context,
            doc,
            base: Some(first_revision),
        },
    );
    let second_review = f.review(1, second_context, second_revision);
    f.admin(Body::ScopeDecided {
        context: second_context,
        previous: None,
        action: DecisionAction::Select {
            subject: second_revision,
        },
        evidence: vec![second_review],
    });
    let goal = f.goal();
    let first_key = ScopeKey {
        context: first_context,
        purpose: DecisionPurpose::Selection,
    };
    let second_key = ScopeKey {
        context: second_context,
        purpose: DecisionPurpose::Selection,
    };
    // The test is meaningful only when ScopeKey hash order is opposite to
    // governance chronology; otherwise the bug and the fix agree.
    assert!(
        second_key < first_key,
        "test requires opposite ScopeKey hash order to catch the bug"
    );
    // The latest governance round's selection wins, not the last by hash.
    assert_eq!(
        goal.state().documents[&doc].selected,
        Some(second_revision),
        "latest governance round's document selection should win"
    );
    // Both selections are retained in scoped history.
    assert!(goal.state().selections.contains_key(&first_key));
    assert!(goal.state().selections.contains_key(&second_key));
}

#[test]
fn stage_prerequisite_resolves_revised_upstream_round() {
    let mut f = Fixture::new(pipeline());
    let initial = f.goal();
    let research = initial
        .evaluation()
        .desired_effects
        .values()
        .next()
        .unwrap()
        .clone();
    let materialized = f.admin(Body::EffectMaterialized {
        effect: research.effect.clone(),
    });
    // The upstream research stage is materialized but unfinished.
    let task_id = TaskId::Derived(research.id);
    // Revise the unfinished upstream stage before any completion evidence.
    let EffectAction::OpenTask { binding, .. } = &research.effect.action else {
        unreachable!()
    };
    let revised = f.admin(Body::TaskRevised {
        task: task_id,
        expected_round: materialized,
        binding: binding.clone(),
    });
    // Complete the new round: publish and declare completion there.
    let new_context = Context {
        scope: Scope::Task(task_id),
        round: revised,
    };
    let subject = f.publish(0, new_context);
    f.worker(
        0,
        Body::CompletionDeclared {
            context: new_context,
            subject,
        },
    );
    let goal = f.goal();
    // The downstream build stage now materializes from the revised round.
    assert_eq!(
        goal.evaluation().desired_effects.len(),
        1,
        "downstream stage should be desired after upstream completion"
    );
    let build = goal.evaluation().desired_effects.values().next().unwrap();
    assert_eq!(build.effect.transition, "stage:build");
}

#[test]
fn host_review_fork_retracts_later_governance_but_preserves_prefix_work() {
    use crate::goal::{Exclusion, Halt};
    let mut f = Fixture::new(review_formation(1));
    let context = f.context();
    let subject = f.publish(0, context);
    let review = f.admin(Body::ReviewRecorded {
        context,
        subject,
        verdict: ReviewVerdict::Approve,
    });
    let fork_seq = f.event(review).header().seq;
    let late_members = [testkit::keypair(8).public(), testkit::keypair(9).public()];
    let admissions: Vec<_> = late_members
        .iter()
        .map(|member| {
            f.admin(Body::MemberAdmitted {
                member: *member,
                endpoint: EndpointId(member.0),
            })
        })
        .collect();
    let mut goal = f.goal();
    assert_eq!(goal.standing(&review), Some(Standing::Effective));
    for id in &admissions {
        assert_eq!(goal.standing(id), Some(Standing::Effective));
    }
    let fork = f.fork(review, 1);
    goal.apply(&[f.event(fork).clone()], &f.definitions);
    assert!(
        matches!(goal.evaluation().admin_halt, Some(Halt::Fork { seq, .. }) if seq == fork_seq)
    );
    assert_eq!(goal.state().head, Some(f.rules));
    for id in admissions {
        assert_eq!(
            goal.standing(&id),
            Some(Standing::Excluded(Exclusion::AfterHalt))
        );
    }
    for member in late_members {
        assert!(!goal.state().members.contains_key(&member));
    }
    assert_eq!(goal.standing(&subject), Some(Standing::Effective));
    assert!(goal.next(&f.admin.key.public()).is_none());
    // The halt is not a blanket exclusion of every member's future work.
    f.anchor = f.rules;
    let clean = f.publish(2, context);
    goal.apply(&[f.event(clean).clone()], &f.definitions);
    assert_eq!(goal.standing(&clean), Some(Standing::Effective));
    assert_eq!(goal.evaluation(), f.goal().evaluation());
}

#[test]
fn retracted_anchor_keeps_same_goal_author_descendants_pending_even_at_surviving_head() {
    let mut f = Fixture::new(Formation::default());
    let context = f.context();
    let host_work = f.admin(Body::ContributionPublished {
        context,
        attempt: None,
        sources: vec![],
        artifacts: vec![],
    });
    let member = testkit::keypair(9).public();
    let retracted = f.admin(Body::MemberAdmitted {
        member,
        endpoint: EndpointId(member.0),
    });
    let poisoned = f.publish(0, context);
    let mut goal = f.goal();
    assert_eq!(goal.standing(&poisoned), Some(Standing::Effective));
    let fork = f.fork(host_work, 1);
    goal.apply(&[f.event(fork).clone()], &f.definitions);
    assert_eq!(
        goal.standing(&poisoned),
        Some(Standing::Pending(Waiting::Anchor))
    );
    assert_eq!(f.event(poisoned).header().anchor, Some(retracted));
    f.anchor = f.rules;
    for _ in 0..3 {
        // next() permits signing: semantic ancestor validity is checked by the fold.
        assert!(goal.next(&f.workers[0].key.public()).is_some());
        let descendant = f.publish(0, context);
        assert_eq!(f.event(descendant).header().anchor, Some(f.rules));
        goal.apply(&[f.event(descendant).clone()], &f.definitions);
        assert_eq!(
            goal.standing(&descendant),
            Some(Standing::Pending(Waiting::Anchor))
        );
    }
    let clean = f.publish(1, context);
    goal.apply(&[f.event(clean).clone()], &f.definitions);
    assert_eq!(goal.standing(&clean), Some(Standing::Effective));
    assert_eq!(goal.evaluation(), f.goal().evaluation());
}
