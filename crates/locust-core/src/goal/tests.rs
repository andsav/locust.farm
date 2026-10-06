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

/// A goal as one host's computer and three other members see it. The
/// computer holds two keys: the goal's governance key, which signs the first
/// record, admissions, removals, rules and stage steps and is no member, and
/// the host's agent (`admin`, key 1), an ordinary member that the formation's
/// `lead` role and selection authority name.
struct Fixture {
    governance: Author,
    admin: Author,
    workers: Vec<Author>,
    events: Vec<Event>,
    definitions: BTreeMap<DefinitionHash, Formation>,
    id: GoalId,
    anchor: EventId,
    rules: EventId,
    admissions: Vec<EventId>,
}
/// The seed of the fixture's governance key; no member of any fixture uses it.
const GOVERNANCE: u8 = 10;
impl Fixture {
    fn new(formation: Formation) -> Self {
        let inspected = crate::organization::inspect(&serde_json::to_string(&formation).unwrap());
        assert!(inspected.valid, "{:?}", inspected.diagnostics);
        Self::founded(inspected.normalized.unwrap())
    }
    /// Founds a goal without asking whether its formation is valid, so a
    /// test can watch replay refuse one.
    fn founded(formation: Formation) -> Self {
        let mut governance = Author::new(GOVERNANCE);
        let admin = Author::new(1);
        let workers: Vec<_> = (2..=4).map(Author::new).collect();
        let genesis = governance.genesis_with(admin.key.public(), &formation);
        let id = genesis.header().goal;
        let mut anchor = genesis.id();
        let mut events = vec![genesis];
        let mut admissions = Vec::new();
        for principal in std::iter::once(admin.key.public())
            .chain(workers.iter().map(|worker| worker.key.public()))
        {
            let event = governance.event(
                id,
                Some(anchor),
                Body::MemberAdmitted {
                    name: "member".into(),
                    role: None,
                    member: principal,
                    endpoint: EndpointId(principal.0),
                },
            );
            anchor = event.id();
            admissions.push(event.id());
            events.push(event);
        }
        let roles: BTreeMap<String, Vec<_>> = formation
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
        let (binding, _) = testkit::rules_binding(&id, 0, &formation);
        let bound = governance.event(
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
        for (role, holders) in roles {
            let event = governance.event(id, Some(anchor), Body::RoleHolders { role, holders });
            anchor = event.id();
            events.push(event);
        }
        Self {
            governance,
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
    /// What the host's computer signs: governance and stage steps with the
    /// goal's key, everything else as the host's agent.
    fn host(&mut self, body: Body) -> EventId {
        if body.host_may_sign() {
            self.governance(body)
        } else {
            self.agent(body)
        }
    }
    /// Signs with the goal's governance key, whatever the body.
    fn governance(&mut self, body: Body) -> EventId {
        let governance = body.is_governance();
        let event = self.governance.event(self.id, Some(self.anchor), body);
        let id = event.id();
        if governance {
            self.anchor = id;
        }
        self.events.push(event);
        id
    }
    /// Signs as the host's agent, whatever the body.
    fn agent(&mut self, body: Body) -> EventId {
        let event = self.admin.event(self.id, Some(self.anchor), body);
        let id = event.id();
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
    /// The same history run forward, one record at a time in reverse, and
    /// reloaded from a store; all three agree.
    fn replays(&self) -> [Goal; 3] {
        let forward = self.goal();
        let mut reversed = Goal::new(self.id);
        for event in self.events.iter().rev() {
            reversed.apply(std::slice::from_ref(event), &self.definitions);
        }
        let mut store = MemStore::new();
        store
            .commit(&Commit {
                events: self.events.clone(),
                ..Commit::default()
            })
            .unwrap();
        let reloaded = Goal::load(&store.reopen(), self.id, &self.definitions).unwrap();
        assert_eq!(reversed.evaluation(), forward.evaluation());
        assert_eq!(reloaded.evaluation(), forward.evaluation());
        [forward, reversed, reloaded]
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
fn replay_exposes_the_pinned_rule_behind_each_denied_action() {
    use locust_proto::api::Rule;
    use locust_proto::organization::StartRule;
    let worker = Selector::Participant {
        key: testkit::keypair(2).public().to_string(),
    };
    let host = Authority::Participant {
        key: testkit::keypair(1).public().to_string(),
    };
    let mut formation = Formation::default();
    formation.work.propose = worker.clone();
    formation.work.publish = worker.clone();
    formation.work.starts = vec![
        StartRule::Independent { by: worker.clone() },
        StartRule::Offered {
            by: worker.clone(),
            to: Selector::Members,
        },
    ];
    formation.decisions.completion = CompletionRule::All {
        rules: vec![
            CompletionRule::Declaration { by: worker.clone() },
            CompletionRule::Reviews {
                by: worker.clone(),
                count: 1,
                exclude_author: true,
            },
            CompletionRule::Check {
                name: "build".into(),
                by: worker.clone(),
            },
        ],
    };
    formation.decisions.selection = Some(host.clone());
    formation.decisions.finish = Some(host);
    let mut f = Fixture::new(formation);
    let task = f.task();
    let goal_context = f.context();
    let subject = f.publish(0, goal_context);
    let attempt = f.worker(
        0,
        Body::AttemptStarted {
            context: task,
            offer: None,
            closure: None,
        },
    );
    let baseline = f.goal();
    assert_eq!(baseline.standing(&attempt), Some(Standing::Effective));
    let task_binding = TaskBinding {
        rules: f.rules,
        task_type: None,
        inputs: BTreeMap::new(),
        parent: None,
        stage: None,
    };
    let denied = [
        (
            Rule::Propose,
            Body::TaskOpened {
                binding: task_binding,
            },
        ),
        (
            Rule::Publish,
            Body::ContributionPublished {
                context: goal_context,
                attempt: None,
                sources: vec![],
                artifacts: vec![],
            },
        ),
        (
            Rule::Start,
            Body::AttemptStarted {
                context: task,
                offer: None,
                closure: None,
            },
        ),
        (
            Rule::Offer,
            Body::WorkOffered {
                context: task,
                recipient: f.workers[0].key.public(),
            },
        ),
        (
            Rule::Declare,
            Body::CompletionDeclared {
                context: goal_context,
                subject,
            },
        ),
        (
            Rule::Review,
            Body::ReviewRecorded {
                context: goal_context,
                subject,
                verdict: ReviewVerdict::Approve,
            },
        ),
        (
            Rule::Attest,
            Body::CheckAttested {
                context: goal_context,
                subject,
                name: "build".into(),
                passed: true,
            },
        ),
        (
            Rule::Select,
            Body::ScopeDecided {
                context: goal_context,
                previous: None,
                action: DecisionAction::Select { subject },
                evidence: vec![],
            },
        ),
        (
            Rule::Finish,
            Body::ScopeDecided {
                context: goal_context,
                previous: None,
                action: DecisionAction::Close,
                evidence: vec![],
            },
        ),
        (Rule::Cancel, Body::CancelRequested { attempt }),
    ];
    for (rule, body) in denied {
        let candidate = Author::new(3).event(f.id, Some(f.anchor), body);
        let id = candidate.id();
        let mut trial = baseline.clone();
        trial.apply(&[candidate], &f.definitions);
        assert!(
            matches!(trial.standing(&id), Some(Standing::Excluded(_))),
            "{rule:?}"
        );
        let refusal = trial
            .rule_refusal(&id)
            .unwrap_or_else(|| panic!("missing rule metadata for {rule:?}"));
        assert_eq!(refusal.rule, rule);
        assert_ne!(refusal.qualifies, Selector::Nobody);
    }
}

#[test]
fn open_taskless_work_needs_no_host_decision() {
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
    let decision = f.host(Body::ScopeDecided {
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
    let decision = f.host(Body::ScopeDecided {
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
    let first = f.host(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject: a },
        evidence: vec![ar],
    });
    let second = f.host(Body::ScopeDecided {
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
    assert!(goal.evaluation().host_halt.is_none());
    assert_eq!(goal.standing(&open), Some(Standing::Effective));
}

#[test]
fn a_selection_cannot_substitute_another_task_or_count_unlisted_reviews() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let other = f.task();
    let candidate = f.publish(0, other);
    let review = f.review(1, other, candidate);
    let wrong = f.host(Body::ScopeDecided {
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
    let incomplete = f.host(Body::ScopeDecided {
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
    f.host(Body::MemberRemoved {
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
    f.host(Body::MemberAdmitted {
        name: "member".into(),
        role: None,
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
    f.host(Body::MemberRemoved {
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
    f.host(Body::MemberRemoved {
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
    let revised = f.host(Body::TaskRevised {
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
    let created = f.host(Body::EffectMaterialized {
        effect: first.effect.clone(),
    });
    // A duplicated authored materialization is still one logical task.
    f.host(Body::EffectMaterialized {
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
    let wrong = f.host(Body::EffectMaterialized {
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
    let wrong = f.host(Body::EffectMaterialized { effect });
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
    let decision = f.host(Body::ScopeDecided {
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
    let decision = f.host(Body::ScopeDecided {
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
    let decision = f.host(Body::ScopeDecided {
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
    let selected = f.host(Body::ScopeDecided {
        context: first,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![],
    });
    let other_selected = f.host(Body::ScopeDecided {
        context: other,
        previous: None,
        action: DecisionAction::Select {
            subject: other_subject,
        },
        evidence: vec![],
    });
    let closed = f.host(Body::ScopeDecided {
        context: first,
        previous: None,
        action: DecisionAction::Close,
        evidence: vec![],
    });
    let reopened = f.host(Body::ScopeDecided {
        context: first,
        previous: Some(closed),
        action: DecisionAction::Reopen,
        evidence: vec![],
    });
    let replacement = f.host(Body::ScopeDecided {
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
    let competing = f.host(Body::ScopeDecided {
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
        let removal = f.host(Body::MemberRemoved {
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
        let signed = Event::sign(header, &f.governance.key).unwrap();
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
    let decision = f.host(Body::ScopeDecided {
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
fn scope_proof_cannot_retain_evidence_past_the_host_cutoff() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let subject = f.publish(0, context);
    let review = f.review(1, context, subject);
    f.host(Body::MemberRemoved {
        member: f.workers[1].key.public(),
        admission: f.admissions[2],
        last_accepted: None,
    });
    let decision = f.host(Body::ScopeDecided {
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
    let previous = f.host(Body::ScopeDecided {
        context: first,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![review],
    });
    let second = f.task();
    let subject = f.publish(0, second);
    let review = f.review(1, second, subject);
    let wrong = f.host(Body::ScopeDecided {
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
    let selection = f.host(Body::ScopeDecided {
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
    let (binding, _) = testkit::rules_binding(&f.id, 0, &Formation::default());
    f.host(Body::RulesBound {
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
    let close = f.host(Body::ScopeDecided {
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
    let reopen = f.host(Body::ScopeDecided {
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
    let close = f.host(Body::ScopeDecided {
        context: another,
        previous: None,
        action: DecisionAction::Close,
        evidence: vec![],
    });
    let subject = f.publish(0, context);
    let select = f.host(Body::ScopeDecided {
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
    let revised = f.host(Body::TaskRevised {
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
    f.host(Body::ScopeDecided {
        context: first_context,
        previous: None,
        action: DecisionAction::Select {
            subject: first_revision,
        },
        evidence: vec![first_review],
    });
    // Second governance round: bind new rules, then revise and select again.
    let (binding, _) = testkit::rules_binding(&f.id, 0, &review_formation(1));
    let second_rules = f.host(Body::RulesBound {
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
    f.host(Body::ScopeDecided {
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
    let materialized = f.host(Body::EffectMaterialized {
        effect: research.effect.clone(),
    });
    // The upstream research stage is materialized but unfinished.
    let task_id = TaskId::Derived(research.id);
    // Revise the unfinished upstream stage before any completion evidence.
    let EffectAction::OpenTask { binding, .. } = &research.effect.action else {
        unreachable!()
    };
    let revised = f.host(Body::TaskRevised {
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
fn revised_stage_round_keeps_its_runner_as_creator_despite_a_member_copy_of_the_effect() {
    use crate::goal::Exclusion;
    let mut formation = Formation::default();
    formation.decisions.completion = CompletionRule::Declaration {
        by: Selector::Members,
    };
    formation.flow.insert(
        "research".into(),
        Stage {
            task_type: None,
            requires: Vec::new(),
            recipients: Selector::Members,
        },
    );
    let mut f = Fixture::new(formation);
    let governance = f.governance.key.public();
    let stage = f
        .goal()
        .evaluation()
        .desired_effects
        .values()
        .next()
        .unwrap()
        .clone();
    assert_eq!(stage.runner, governance);
    // A member signs the stage's effect at the start of its own log, below
    // every position the governance key has left.
    let copy = f.worker(
        0,
        Body::EffectMaterialized {
            effect: stage.effect.clone(),
        },
    );
    let materialized = f.host(Body::EffectMaterialized {
        effect: stage.effect.clone(),
    });
    assert_eq!(f.event(copy).header().seq, 0);
    assert!(f.event(materialized).header().seq > 0);
    let task = TaskId::Derived(stage.id);
    let EffectAction::OpenTask { binding, .. } = &stage.effect.action else {
        unreachable!()
    };
    let revised = f.host(Body::TaskRevised {
        task,
        expected_round: materialized,
        binding: binding.clone(),
    });
    let context = Context {
        scope: Scope::Task(task),
        round: revised,
    };
    let subject = f.publish(0, context);
    let by_member = f.worker(1, Body::CompletionDeclared { context, subject });
    let by_key = f.governance(Body::CompletionDeclared { context, subject });
    for goal in f.replays() {
        assert_eq!(
            goal.standing(&copy),
            Some(Standing::Excluded(Exclusion::Precondition(
                "effect signer is not its configured runner"
            )))
        );
        assert_eq!(goal.standing(&materialized), Some(Standing::Effective));
        assert_eq!(goal.standing(&revised), Some(Standing::Effective));
        assert_eq!(
            goal.effective_rules(context, &f.definitions)
                .unwrap()
                .creator,
            Some(governance)
        );
        assert_eq!(goal.standing(&by_member), Some(Standing::Effective));
        assert_eq!(
            goal.standing(&by_key),
            Some(Standing::Excluded(Exclusion::NotAMember))
        );
    }
}

#[test]
fn the_governance_key_is_never_a_member_and_its_ordinary_work_is_excluded() {
    use crate::goal::Exclusion;
    let mut f = Fixture::new(review_formation(1));
    let governance = f.governance.key.public();
    let admission = f.host(Body::MemberAdmitted {
        name: "member".into(),
        role: None,
        member: governance,
        endpoint: EndpointId(governance.0),
    });
    let context = f.context();
    let task = f.governance(Body::TaskOpened {
        binding: TaskBinding {
            rules: f.rules,
            task_type: None,
            inputs: BTreeMap::new(),
            parent: None,
            stage: None,
        },
    });
    let subject = f.publish(0, context);
    let result = f.governance(Body::ContributionPublished {
        context,
        attempt: None,
        sources: vec![],
        artifacts: vec![],
    });
    let review = f.governance(Body::ReviewRecorded {
        context,
        subject,
        verdict: ReviewVerdict::Approve,
    });
    let approval = f.review(1, context, subject);
    let pick = f.governance(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![approval],
    });
    for goal in f.replays() {
        assert_eq!(
            goal.standing(&admission),
            Some(Standing::Excluded(Exclusion::Precondition(
                "the goal's signing key is not a member"
            )))
        );
        assert!(!goal.state().members.contains_key(&governance));
        assert_eq!(goal.state().head, Some(admission));
        for id in [task, result, review, pick] {
            assert_eq!(
                goal.standing(&id),
                Some(Standing::Excluded(Exclusion::NotAMember))
            );
        }
        assert_eq!(goal.standing(&subject), Some(Standing::Effective));
        assert!(goal.evaluation().host_halt.is_none());
    }
}

#[test]
fn a_stage_step_signed_by_the_governance_key_is_effective_and_one_by_the_hosts_agent_is_not() {
    use crate::goal::Exclusion;
    let mut f = Fixture::new(pipeline());
    let stage = f
        .goal()
        .evaluation()
        .desired_effects
        .values()
        .next()
        .unwrap()
        .clone();
    assert_eq!(stage.runner, f.governance.key.public());
    let by_agent = f.agent(Body::EffectMaterialized {
        effect: stage.effect.clone(),
    });
    let by_key = f.host(Body::EffectMaterialized {
        effect: stage.effect.clone(),
    });
    assert_eq!(f.event(by_key).header().author, f.governance.key.public());
    for goal in f.replays() {
        assert_eq!(
            goal.standing(&by_agent),
            Some(Standing::Excluded(Exclusion::Precondition(
                "effect signer is not its configured runner"
            )))
        );
        assert_eq!(goal.standing(&by_key), Some(Standing::Effective));
        assert_eq!(goal.state().tasks.len(), 1);
        assert!(goal.evaluation().desired_effects.is_empty());
    }
}

#[test]
fn removing_the_hosts_agent_is_excluded_and_it_stays_a_member() {
    use crate::goal::Exclusion;
    let mut f = Fixture::new(review_formation(1));
    let host = f.admin.key.public();
    let removal = f.host(Body::MemberRemoved {
        member: host,
        admission: f.admissions[0],
        last_accepted: None,
    });
    let context = f.context();
    let subject = f.publish(0, context);
    let review = f.host(Body::ReviewRecorded {
        context,
        subject,
        verdict: ReviewVerdict::Approve,
    });
    let later = f.host(Body::MemberAdmitted {
        name: "member".into(),
        role: None,
        member: testkit::keypair(9).public(),
        endpoint: EndpointId([9; 32]),
    });
    for goal in f.replays() {
        assert_eq!(
            goal.standing(&removal),
            Some(Standing::Excluded(Exclusion::Precondition(
                "the host's agent is not removed"
            )))
        );
        assert!(goal.state().is_member(&host));
        assert_eq!(goal.state().epoch, 0);
        assert_eq!(goal.standing(&review), Some(Standing::Effective));
        assert_eq!(goal.standing(&later), Some(Standing::Effective));
        assert_eq!(goal.state().head, Some(later));
    }
}

#[test]
fn a_fork_in_the_governance_log_retracts_later_governance_and_preserves_prefix_work() {
    use crate::goal::{Exclusion, Halt};
    // Once with two admissions at one position, once with a stage step
    // against an admission.
    for stage_step in [false, true] {
        let mut f = Fixture::new(if stage_step {
            pipeline()
        } else {
            review_formation(1)
        });
        let context = f.context();
        let subject = f.publish(0, context);
        let first = f.host(Body::MemberAdmitted {
            name: "member".into(),
            role: None,
            member: testkit::keypair(8).public(),
            endpoint: EndpointId([8; 32]),
        });
        let fork_seq = f.event(first).header().seq;
        let later = f.host(Body::MemberAdmitted {
            name: "member".into(),
            role: None,
            member: testkit::keypair(9).public(),
            endpoint: EndpointId([9; 32]),
        });
        let mut goal = f.goal();
        for id in [first, later] {
            assert_eq!(goal.standing(&id), Some(Standing::Effective));
        }
        let fork = if stage_step {
            let step = goal
                .evaluation()
                .desired_effects
                .values()
                .next()
                .unwrap()
                .effect
                .clone();
            let mut header = f.event(first).header().clone();
            header.body = Body::EffectMaterialized { effect: step };
            let event = Event::sign(header, &f.governance.key).unwrap();
            let id = event.id();
            f.events.push(event);
            id
        } else {
            f.fork(first, GOVERNANCE)
        };
        goal.apply(&[f.event(fork).clone()], &f.definitions);
        assert!(
            matches!(goal.evaluation().host_halt, Some(Halt::Fork { seq, .. }) if seq == fork_seq)
        );
        assert_eq!(goal.state().head, Some(f.rules));
        for id in [first, later] {
            assert_eq!(
                goal.standing(&id),
                Some(Standing::Excluded(Exclusion::AfterHalt))
            );
        }
        assert!(
            !goal
                .state()
                .members
                .contains_key(&testkit::keypair(8).public())
        );
        assert!(
            !goal
                .state()
                .members
                .contains_key(&testkit::keypair(9).public())
        );
        assert_eq!(goal.standing(&subject), Some(Standing::Effective));
        assert!(goal.next(&f.governance.key.public()).is_none());
        // Members whose own log has no record anchored in the cut suffix can
        // still work. A member with such a record stays pending forever (see
        // retracted_anchor_keeps_same_goal_author_descendants_pending_even_at_surviving_head).
        f.anchor = f.rules;
        let clean = f.publish(2, context);
        goal.apply(&[f.event(clean).clone()], &f.definitions);
        assert_eq!(goal.standing(&clean), Some(Standing::Effective));
        for replay in f.replays() {
            assert_eq!(goal.evaluation(), replay.evaluation());
        }
    }
}

#[test]
fn a_review_fork_by_the_hosts_agent_costs_what_a_members_fork_costs() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.context();
    let subject = f.publish(0, context);
    let review = f.host(Body::ReviewRecorded {
        context,
        subject,
        verdict: ReviewVerdict::Approve,
    });
    let later = f.host(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![review],
    });
    let admissions: Vec<_> = [8u8, 9]
        .into_iter()
        .map(|n| {
            f.host(Body::MemberAdmitted {
                name: "member".into(),
                role: None,
                member: testkit::keypair(n).public(),
                endpoint: EndpointId([n; 32]),
            })
        })
        .collect();
    let mut goal = f.goal();
    assert_eq!(goal.standing(&later), Some(Standing::Effective));
    let fork = f.fork(review, 1);
    goal.apply(&[f.event(fork).clone()], &f.definitions);
    assert!(goal.evaluation().host_halt.is_none());
    for id in &admissions {
        assert_eq!(goal.standing(id), Some(Standing::Effective));
    }
    assert_eq!(goal.state().head, Some(admissions[1]));
    assert!(goal.next(&f.governance.key.public()).is_some());
    assert!(goal.next(&f.admin.key.public()).is_none());
    assert!(goal.standing(&review).unwrap().is_pending());
    assert!(goal.standing(&fork).unwrap().is_pending());
    assert!(goal.standing(&later).unwrap().is_pending());
    assert_eq!(goal.standing(&subject), Some(Standing::Effective));
    for replay in f.replays() {
        assert_eq!(goal.evaluation(), replay.evaluation());
    }
}

#[test]
fn a_stage_task_names_the_governance_key_as_its_creator() {
    use locust_proto::organization::{StartRule, TaskType, WorkRules};
    let staged = |by: Selector| {
        let mut formation = Formation::default();
        formation
            .roles
            .insert("lead".into(), locust_proto::organization::Role::default());
        formation.task_types.insert(
            "offered".into(),
            TaskType {
                work: Some(WorkRules {
                    starts: vec![StartRule::Offered {
                        by,
                        to: Selector::Members,
                    }],
                    ..Default::default()
                }),
                decisions: None,
            },
        );
        formation.flow.insert(
            "research".into(),
            Stage {
                task_type: Some("offered".into()),
                requires: Vec::new(),
                recipients: Selector::Members,
            },
        );
        formation
    };
    for (by, offers) in [
        (Selector::TaskCreator, 4),
        (Selector::Members, 4),
        (
            Selector::Role {
                name: "lead".into(),
            },
            0,
        ),
    ] {
        let mut f = Fixture::new(staged(by.clone()));
        let governance = f.governance.key.public();
        let stage = f
            .goal()
            .evaluation()
            .desired_effects
            .values()
            .find(|desired| matches!(desired.effect.action, EffectAction::OpenTask { .. }))
            .unwrap()
            .clone();
        let opened = f.host(Body::EffectMaterialized {
            effect: stage.effect.clone(),
        });
        for goal in f.replays() {
            let context = Context {
                scope: Scope::Task(TaskId::Derived(stage.id)),
                round: opened,
            };
            assert_eq!(
                goal.effective_rules(context, &f.definitions)
                    .unwrap()
                    .creator,
                Some(governance),
                "{by:?}"
            );
            let offered: Vec<_> = goal
                .evaluation()
                .desired_effects
                .values()
                .filter(|desired| matches!(desired.effect.action, EffectAction::Offer { .. }))
                .collect();
            assert_eq!(offered.len(), offers, "{by:?}");
            assert!(offered.iter().all(|desired| desired.runner == governance));
        }
    }
}

#[test]
fn rules_whose_stage_task_names_the_task_creator_are_excluded_in_replay() {
    use crate::goal::Exclusion;
    use locust_proto::organization::{DecisionRules, TaskType};
    let mut formation = Formation::default();
    formation.task_types.insert(
        "owned".into(),
        TaskType {
            work: None,
            decisions: Some(DecisionRules {
                completion: CompletionRule::Declaration {
                    by: Selector::TaskCreator,
                },
                ..Default::default()
            }),
        },
    );
    formation.flow.insert(
        "research".into(),
        Stage {
            task_type: Some("owned".into()),
            requires: Vec::new(),
            recipients: Selector::Members,
        },
    );
    let inspected = crate::organization::inspect(&serde_json::to_string(&formation).unwrap());
    assert!(
        inspected
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "selector_scope")
    );
    let f = Fixture::founded(formation);
    for goal in f.replays() {
        assert_eq!(
            goal.standing(&f.rules),
            Some(Standing::Excluded(Exclusion::InvalidDefinition))
        );
        assert!(goal.state().current_rules.is_none());
        assert!(goal.evaluation().desired_effects.is_empty());
        assert!(goal.state().tasks.is_empty());
    }
}

#[test]
fn retracted_anchor_keeps_same_goal_author_descendants_pending_even_at_surviving_head() {
    let mut f = Fixture::new(Formation::default());
    let context = f.context();
    let member = testkit::keypair(8).public();
    let host_work = f.host(Body::MemberAdmitted {
        name: "member".into(),
        role: None,
        member,
        endpoint: EndpointId(member.0),
    });
    let member = testkit::keypair(9).public();
    let retracted = f.host(Body::MemberAdmitted {
        name: "member".into(),
        role: None,
        member,
        endpoint: EndpointId(member.0),
    });
    let poisoned = f.publish(0, context);
    let mut goal = f.goal();
    assert_eq!(goal.standing(&poisoned), Some(Standing::Effective));
    let fork = f.fork(host_work, GOVERNANCE);
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

/// Authorizing every retained event after a removal traverses the cutoff
/// ancestry once per tenure, not once per event. A deterministic counter
/// counts the shared branch walk independently of membership lookups, while
/// the exact cutoff, fork, gap and readmission semantics are unchanged.
#[test]
fn retained_events_authorize_with_one_cutoff_traversal_per_tenure() {
    let mut f = Fixture::new(Formation::default());
    let context = f.context();
    // A long same-author branch retained by an exact cutoff.
    const RETAINED: usize = 64;
    let mut published = Vec::with_capacity(RETAINED);
    for _ in 0..RETAINED {
        published.push(f.publish(0, context));
    }
    let after = f.publish(0, context);
    let fork = f.fork(published[0], 2);
    let point = AuthorPoint {
        seq: f.event(published[RETAINED - 1]).header().seq,
        id: published[RETAINED - 1],
    };
    f.host(Body::MemberRemoved {
        member: f.workers[0].key.public(),
        admission: f.admissions[1],
        last_accepted: Some(point),
    });
    let goal = f.goal();
    // Every retained contribution is effective; the post-cutoff and fork are not.
    for id in &published {
        assert_eq!(goal.standing(id), Some(Standing::Effective));
    }
    assert!(matches!(goal.standing(&after), Some(Standing::Excluded(_))));
    assert!(matches!(goal.standing(&fork), Some(Standing::Excluded(_))));
    assert!(!goal.state().is_member(&f.workers[0].key.public()));
    // One traversal for the single removed tenure, regardless of how many
    // retained events authorized against it.
    assert_eq!(goal.cutoff_traversals(), 1);

    // Readmission does not backdate eligibility to the post-cutoff event.
    f.host(Body::MemberAdmitted {
        name: "member".into(),
        role: None,
        member: f.workers[0].key.public(),
        endpoint: EndpointId([2; 32]),
    });
    let goal = f.goal();
    assert!(goal.state().is_member(&f.workers[0].key.public()));
    assert!(matches!(goal.standing(&after), Some(Standing::Excluded(_))));
    // The fresh fold re-traverses once for the removed tenure.
    assert_eq!(goal.cutoff_traversals(), 1);
}

/// A missing cutoff ancestor caches a pending result only for this fold:
/// once it arrives, the next fold re-traverses and retains the branch.
#[test]
fn missing_cutoff_ancestor_waits_and_retraverses_on_arrival() {
    let mut f = Fixture::new(Formation::default());
    let context = f.context();
    let first = f.publish(0, context);
    let last = f.publish(0, context);
    let point = AuthorPoint {
        seq: f.event(last).header().seq,
        id: last,
    };
    f.host(Body::MemberRemoved {
        member: f.workers[0].key.public(),
        admission: f.admissions[1],
        last_accepted: Some(point),
    });
    // Drop the cutoff point from history so its ancestry is missing.
    let mut events: Vec<Event> = f
        .events
        .iter()
        .filter(|event| event.id() != last)
        .cloned()
        .collect();
    let mut goal = Goal::new(f.id);
    goal.apply(&events, &f.definitions);
    assert!(goal.standing(&first).unwrap().is_pending());
    // The missing ancestor is traversed once and the pending result is reused.
    assert_eq!(goal.cutoff_traversals(), 1);
    // Restore the missing ancestor and refold: the branch is retained.
    events.push(f.event(last).clone());
    goal.apply(&[f.event(last).clone()], &f.definitions);
    assert_eq!(goal.standing(&first), Some(Standing::Effective));
    assert_eq!(goal.standing(&last), Some(Standing::Effective));
    // The fresh fold re-traverses once and caches the now-complete ancestry.
    assert_eq!(goal.cutoff_traversals(), 1);
}

fn preset_formation(name: &str) -> Formation {
    locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == name)
        .unwrap()
        .formation
}
fn publish_body(context: Context) -> Body {
    Body::ContributionPublished {
        context,
        attempt: None,
        sources: vec![],
        artifacts: vec![],
    }
}
fn rebind(f: &mut Fixture, formation: Formation) -> EventId {
    let formation = crate::organization::inspect(&serde_json::to_string(&formation).unwrap())
        .normalized
        .unwrap();
    let (binding, _) = testkit::rules_binding(&f.id, f.goal().state().epoch, &formation);
    f.definitions
        .insert(testkit::definition_hash(&formation), formation);
    let id = f.host(Body::RulesBound {
        expected: Some(f.rules),
        binding,
    });
    f.rules = id;
    id
}
fn alone(formation: Formation) -> Fixture {
    let formation = crate::organization::inspect(&serde_json::to_string(&formation).unwrap())
        .normalized
        .unwrap();
    let mut governance = Author::new(GOVERNANCE);
    let admin = Author::new(1);
    let genesis = governance.genesis_with(admin.key.public(), &formation);
    let id = genesis.header().goal;
    let admission = governance.event(
        id,
        Some(genesis.id()),
        Body::MemberAdmitted {
            member: admin.key.public(),
            endpoint: EndpointId([1; 32]),
            name: "Maple".into(),
            role: None,
        },
    );
    let (binding, _) = testkit::rules_binding(&id, 0, &formation);
    let bound = governance.event(
        id,
        Some(admission.id()),
        Body::RulesBound {
            expected: None,
            binding,
        },
    );
    let rules = bound.id();
    let admissions = vec![admission.id()];
    Fixture {
        governance,
        admin,
        workers: (2..=4).map(Author::new).collect(),
        events: vec![genesis, admission, bound],
        definitions: BTreeMap::from([(testkit::definition_hash(&formation), formation)]),
        id,
        anchor: rules,
        rules,
        admissions,
    }
}
fn admit_worker(f: &mut Fixture, worker: usize, role: Option<&str>) -> EventId {
    let key = f.workers[worker].key.public();
    f.host(Body::MemberAdmitted {
        member: key,
        endpoint: EndpointId(key.0),
        name: format!("Worker {worker}"),
        role: role.map(str::to_owned),
    })
}

#[test]
fn role_holders_are_read_at_each_events_governance_position() {
    let mut formation = review_formation(1);
    formation
        .roles
        .insert("reviewer".into(), Default::default());
    formation.decisions.completion = CompletionRule::Reviews {
        by: Selector::Role {
            name: "reviewer".into(),
        },
        count: 1,
        exclude_author: true,
    };
    let mut f = Fixture::new(formation);
    let context = f.task();
    let subject = f.publish(0, context);
    let old = f.review(1, context, subject);
    f.host(Body::RoleHolders {
        role: "reviewer".into(),
        holders: vec![f.workers[2].key.public()],
    });
    let refused = f.review(1, context, subject);
    let new = f.review(2, context, subject);
    for goal in f.replays() {
        assert_eq!(goal.standing(&old), Some(Standing::Effective));
        assert!(matches!(
            goal.standing(&refused),
            Some(Standing::Excluded(_))
        ));
        assert_eq!(goal.standing(&new), Some(Standing::Effective));
        assert!(goal.state().contributions[&subject].approved);
    }
}
#[test]
fn a_member_given_reviewer_can_review_a_result_in_a_task_opened_before() {
    let mut f = alone(preset_formation("review-panel"));
    admit_worker(&mut f, 0, None);
    admit_worker(&mut f, 1, None);
    let context = f.task();
    let subject = f.publish(0, context);
    f.host(Body::RoleHolders {
        role: "reviewer".into(),
        holders: vec![f.workers[1].key.public()],
    });
    let review = f.review(1, context, subject);
    for goal in f.replays() {
        assert_eq!(goal.standing(&review), Some(Standing::Effective));
    }
}
#[test]
fn a_role_list_outlives_the_binding_that_declared_it_and_old_tasks_read_it() {
    let mut f = Fixture::new(preset_formation("directed"));
    let context = f.task();
    let subject = f.publish(0, context);
    rebind(&mut f, preset_formation("peer-review"));
    f.host(Body::RoleHolders {
        role: "reviewer".into(),
        holders: vec![f.workers[1].key.public()],
    });
    f.host(Body::RoleHolders {
        role: "lead".into(),
        holders: vec![f.workers[2].key.public()],
    });
    let review = f.review(1, context, subject);
    let pick = f.worker(
        2,
        Body::ScopeDecided {
            context,
            previous: None,
            action: DecisionAction::Select { subject },
            evidence: vec![review],
        },
    );
    let refused = f.agent(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![review],
    });
    rebind(&mut f, preset_formation("directed"));
    for goal in f.replays() {
        assert_eq!(goal.standing(&pick), Some(Standing::Effective));
        assert!(matches!(
            goal.standing(&refused),
            Some(Standing::Excluded(_))
        ));
        assert_eq!(goal.state().roles["lead"], vec![f.workers[2].key.public()]);
        assert_eq!(
            goal.state().roles["reviewer"],
            vec![f.workers[1].key.public()]
        );
    }
}
#[test]
fn decisions_by_successive_authorities_follow_governance_chronology() {
    let mut f = Fixture::new(preset_formation("independent-attempts"));
    let context = f.task();
    let first = f.publish(0, context);
    let second = f.publish(1, context);
    let first_done = f.worker(
        0,
        Body::CompletionDeclared {
            context,
            subject: first,
        },
    );
    let second_done = f.worker(
        1,
        Body::CompletionDeclared {
            context,
            subject: second,
        },
    );
    for _ in 0..4 {
        f.agent(publish_body(f.context()));
    }
    let old = f.agent(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject: first },
        evidence: vec![first_done],
    });
    f.host(Body::RoleHolders {
        role: "lead".into(),
        holders: vec![f.workers[2].key.public()],
    });
    let new = f.worker(
        2,
        Body::ScopeDecided {
            context,
            previous: Some(old),
            action: DecisionAction::Select { subject: second },
            evidence: vec![second_done],
        },
    );
    for goal in f.replays() {
        assert_eq!(goal.standing(&new), Some(Standing::Effective));
        assert_eq!(
            goal.state().task_round(context).unwrap().selected,
            Some(second)
        );
    }
}
#[test]
fn removal_drops_the_member_from_every_role_and_an_empty_role_falls_to_the_host() {
    let mut f = Fixture::new(preset_formation("directed"));
    for role in ["lead", "reviewer"] {
        f.host(Body::RoleHolders {
            role: role.into(),
            holders: vec![f.workers[1].key.public()],
        });
    }
    rebind(&mut f, preset_formation("peer-review"));
    f.host(Body::MemberRemoved {
        member: f.workers[1].key.public(),
        admission: f.admissions[2],
        last_accepted: None,
    });
    for goal in f.replays() {
        for role in ["lead", "reviewer"] {
            assert_eq!(goal.state().roles[role], vec![f.admin.key.public()]);
        }
    }
}
#[test]
fn role_holders_must_be_distinct_admitted_and_non_empty() {
    let mut f = Fixture::new(Formation::default());
    let a = f.admin.key.public();
    let b = f.workers[0].key.public();
    let mut unsorted = vec![a, b];
    unsorted.sort();
    unsorted.reverse();
    for holders in [
        vec![],
        vec![a, a],
        unsorted,
        vec![testkit::keypair(99).public()],
    ] {
        let record = f.host(Body::RoleHolders {
            role: "reader".into(),
            holders,
        });
        for goal in f.replays() {
            assert_eq!(
                goal.standing(&record),
                Some(Standing::Excluded(super::Exclusion::Precondition(
                    "role holders must name distinct admitted members in ascending order"
                )))
            );
        }
    }
}
#[test]
fn rules_bound_fills_unheld_declared_roles_with_the_host_and_requires_one_holder_per_authority() {
    let mut f = alone(preset_formation("directed"));
    assert_eq!(f.goal().state().roles["lead"], vec![f.admin.key.public()]);
    admit_worker(&mut f, 0, None);
    let mut holders = vec![f.admin.key.public(), f.workers[0].key.public()];
    holders.sort();
    f.host(Body::RoleHolders {
        role: "lead".into(),
        holders,
    });
    let binding = rebind(&mut f, preset_formation("directed"));
    for goal in f.replays() {
        assert_eq!(
            goal.standing(&binding),
            Some(Standing::Excluded(super::Exclusion::Precondition(
                "a role that picks or closes must have exactly one holder"
            )))
        );
    }
}
#[test]
fn a_members_latest_review_counts_and_a_later_reject_withdraws_only_its_own_approval() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let subject = f.publish(0, context);
    f.review(1, context, subject);
    assert!(f.goal().state().contributions[&subject].approved);
    let rejected = f.worker(
        1,
        Body::ReviewRecorded {
            context,
            subject,
            verdict: ReviewVerdict::Reject,
        },
    );
    for goal in f.replays() {
        assert!(!goal.state().contributions[&subject].approved);
        assert!(!goal.state().task_round(context).unwrap().completed);
        assert_eq!(
            goal.latest_reviews(subject, &f.definitions)[&f.workers[1].key.public()],
            rejected
        );
    }
    f.review(2, context, subject);
    for goal in f.replays() {
        assert!(goal.state().contributions[&subject].approved);
    }
}
#[test]
fn a_reject_after_a_pinned_approval_leaves_the_decision_and_its_subject_selected() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let subject = f.publish(0, context);
    let review = f.review(1, context, subject);
    let decision = f.agent(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![review],
    });
    f.worker(
        1,
        Body::ReviewRecorded {
            context,
            subject,
            verdict: ReviewVerdict::Reject,
        },
    );
    for goal in f.replays() {
        assert_eq!(goal.standing(&decision), Some(Standing::Effective));
        assert_eq!(
            goal.state().task_round(context).unwrap().selected,
            Some(subject)
        );
    }
}
#[test]
fn a_forked_reviewers_latest_effective_review_is_the_last_before_the_fork() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let subject = f.publish(0, context);
    f.review(1, context, subject);
    let reject = f.worker(
        1,
        Body::ReviewRecorded {
            context,
            subject,
            verdict: ReviewVerdict::Reject,
        },
    );
    f.fork(reject, 3);
    for goal in f.replays() {
        assert!(goal.state().contributions[&subject].approved);
    }
}
#[test]
fn check_counts_one_passing_attestation_by_a_named_member() {
    let mut formation = Formation::default();
    formation.decisions.completion = CompletionRule::Check {
        name: "build".into(),
        by: Selector::Members,
    };
    let mut f = Fixture::new(formation);
    let context = f.task();
    let subject = f.publish(0, context);
    f.worker(
        1,
        Body::CheckAttested {
            context,
            subject,
            name: "build".into(),
            passed: true,
        },
    );
    assert!(f.goal().state().contributions[&subject].approved);
    f.worker(
        1,
        Body::CheckAttested {
            context,
            subject,
            name: "build".into(),
            passed: false,
        },
    );
    for goal in f.replays() {
        assert!(!goal.state().contributions[&subject].approved);
    }
}
#[test]
fn a_review_the_rule_does_not_ask_for_is_an_opinion_and_counts_for_nothing() {
    let mut f = Fixture::new(Formation::default());
    let context = f.task();
    let subject = f.publish(0, context);
    let approve = f.review(0, context, subject);
    let reject = f.worker(
        1,
        Body::ReviewRecorded {
            context,
            subject,
            verdict: ReviewVerdict::Reject,
        },
    );
    for goal in f.replays() {
        assert_eq!(goal.standing(&approve), Some(Standing::Effective));
        assert_eq!(goal.standing(&reject), Some(Standing::Effective));
        assert!(!goal.state().contributions[&subject].approved);
    }
    f.worker(0, Body::CompletionDeclared { context, subject });
    for goal in f.replays() {
        assert!(goal.state().contributions[&subject].approved);
    }
}
#[test]
fn a_record_naming_an_unheld_host_record_withdraws_no_other_members_result() {
    let mut f = Fixture::new(Formation::default());
    let context = f.task();
    let subject = f.publish(0, context);
    f.worker(0, Body::CompletionDeclared { context, subject });
    let unheld = EventId([9; 32]);
    let stray: Vec<EventId> = [
        Body::ReviewRecorded {
            context,
            subject,
            verdict: ReviewVerdict::Reject,
        },
        Body::CompletionDeclared { context, subject },
    ]
    .into_iter()
    .map(|body| {
        let event = f.workers[1].event(f.id, Some(unheld), body);
        let id = event.id();
        f.events.push(event);
        id
    })
    .collect();
    for goal in f.replays() {
        for id in &stray {
            assert_eq!(goal.standing(id), Some(Standing::Pending(Waiting::Anchor)));
        }
        assert!(goal.state().contributions[&subject].approved);
    }
}
#[test]
fn the_only_members_result_counts_as_posted_and_a_second_admission_ends_that() {
    let mut f = alone(preset_formation("peer-review"));
    let context = f.context();
    let first = f.agent(publish_body(context));
    admit_worker(&mut f, 0, None);
    let second = f.agent(publish_body(context));
    for goal in f.replays() {
        assert!(goal.state().contributions[&first].approved);
        assert_eq!(
            goal.state().contributions[&first].evidence,
            std::collections::BTreeSet::from([first])
        );
        assert!(!goal.state().contributions[&second].approved);
    }
    f.review(0, context, second);
    for goal in f.replays() {
        assert!(goal.state().contributions[&second].approved);
    }
}
#[test]
fn removing_the_last_other_member_makes_the_hosts_results_count_again() {
    let mut f = alone(preset_formation("peer-review"));
    let admission = admit_worker(&mut f, 0, None);
    let context = f.context();
    let before = f.agent(publish_body(context));
    f.host(Body::MemberRemoved {
        member: f.workers[0].key.public(),
        admission,
        last_accepted: None,
    });
    let after = f.agent(publish_body(context));
    for goal in f.replays() {
        assert!(!goal.state().contributions[&before].approved);
        assert!(goal.state().contributions[&after].approved);
    }
}
#[test]
fn nobody_but_the_host_agent_is_ever_the_only_member() {
    let mut f = alone(preset_formation("peer-review"));
    admit_worker(&mut f, 0, None);
    let context = f.context();
    let subject = f.publish(0, context);
    let removal = f.host(Body::MemberRemoved {
        member: f.admin.key.public(),
        admission: f.admissions[0],
        last_accepted: None,
    });
    for goal in f.replays() {
        assert!(!goal.state().contributions[&subject].approved);
        assert!(matches!(
            goal.standing(&removal),
            Some(Standing::Excluded(_))
        ));
    }
}
#[test]
fn the_hosts_agent_can_anchor_no_further_back_than_its_own_latest_anchor() {
    let mut f = alone(preset_formation("peer-review"));
    let old = f.anchor;
    admit_worker(&mut f, 0, None);
    let context = f.context();
    f.agent(publish_body(context));
    f.anchor = old;
    let bad = f.agent(publish_body(context));
    for goal in f.replays() {
        assert_eq!(
            goal.standing(&bad),
            Some(Standing::Excluded(super::Exclusion::AnchorRegressed))
        );
    }
    let mut f = alone(preset_formation("peer-review"));
    let old = f.anchor;
    admit_worker(&mut f, 0, None);
    f.anchor = old;
    let context = f.context();
    let historical = f.agent(publish_body(context));
    for goal in f.replays() {
        assert!(goal.state().contributions[&historical].approved);
    }
}

#[test]
fn an_ex_holder_anchored_before_the_change_still_counts_and_after_it_is_excluded() {
    let mut f = Fixture::new(preset_formation("directed"));
    let context = f.task();
    let subject = f.publish(0, context);
    let before = f.anchor;
    f.host(Body::RoleHolders {
        role: "reviewer".into(),
        holders: vec![f.admin.key.public()],
    });
    let after = f.anchor;
    f.anchor = before;
    let old = f.review(1, context, subject);
    f.anchor = after;
    let late = f.review(1, context, subject);
    for goal in f.replays() {
        assert_eq!(goal.standing(&old), Some(Standing::Effective));
        assert!(matches!(goal.standing(&late), Some(Standing::Excluded(_))));
        assert!(goal.state().contributions[&subject].approved);
    }
}
#[test]
fn stage_recipients_and_review_requests_follow_roles_at_the_materialization_anchor() {
    let mut formation = preset_formation("pipeline");
    formation.roles.insert("workers".into(), Default::default());
    formation.flow.get_mut("draft").unwrap().recipients = Selector::Role {
        name: "workers".into(),
    };
    let mut f = alone(formation);
    let newcomer = admit_worker(&mut f, 0, Some("workers"));
    let desired = f
        .goal()
        .evaluation()
        .desired_effects
        .values()
        .find(|e| e.effect.transition == "stage:draft")
        .unwrap()
        .clone();
    assert_eq!(
        desired.recipients,
        std::collections::BTreeSet::from([f.admin.key.public(), f.workers[0].key.public()])
    );
    let opened = f.host(Body::EffectMaterialized {
        effect: desired.effect.clone(),
    });
    f.host(Body::RoleHolders {
        role: "workers".into(),
        holders: vec![f.admin.key.public()],
    });
    for goal in f.replays() {
        assert_eq!(goal.standing(&newcomer), Some(Standing::Effective));
        assert_eq!(goal.standing(&opened), Some(Standing::Effective));
    }
    let context = Context {
        scope: Scope::Task(TaskId::Derived(desired.id)),
        round: opened,
    };
    let subject = f.publish(0, context);
    let later = admit_worker(&mut f, 1, None);
    let goal = f.goal();
    assert!(goal.evaluation().desired_effects.values().any(|effect| matches!(effect.effect.action,EffectAction::RequestReview { subject:target,recipient,.. } if target==subject && recipient==f.workers[1].key.public())));
    assert_eq!(goal.standing(&later), Some(Standing::Effective));
    let request = goal
        .evaluation()
        .desired_effects
        .values()
        .find(|effect| matches!(effect.effect.action, EffectAction::RequestReview { subject: target, recipient, .. }
            if target == subject && recipient == f.workers[1].key.public()))
        .unwrap()
        .effect
        .clone();
    let signed = f.host(Body::EffectMaterialized { effect: request });
    f.host(Body::MemberRemoved {
        member: f.workers[1].key.public(),
        admission: later,
        last_accepted: None,
    });
    for goal in f.replays() {
        assert_eq!(goal.standing(&signed), Some(Standing::Effective));
    }
}
#[test]
fn a_stage_does_not_open_on_an_approval_its_member_withdrew() {
    for evidence in [EvidenceKind::Review, EvidenceKind::Completion] {
        let mut formation = preset_formation("pipeline");
        formation.flow.get_mut("ship").unwrap().requires[0].evidence = evidence;
        let mut f = Fixture::new(formation);
        let stage = f
            .goal()
            .evaluation()
            .desired_effects
            .values()
            .find(|e| e.effect.transition == "stage:draft")
            .unwrap()
            .clone();
        let opened = f.host(Body::EffectMaterialized {
            effect: stage.effect,
        });
        let context = Context {
            scope: Scope::Task(TaskId::Derived(stage.id)),
            round: opened,
        };
        let subject = f.publish(0, context);
        f.review(1, context, subject);
        let ready = f
            .goal()
            .evaluation()
            .desired_effects
            .values()
            .find(|e| e.effect.transition == "stage:ship")
            .unwrap()
            .clone();
        f.worker(
            1,
            Body::ReviewRecorded {
                context,
                subject,
                verdict: ReviewVerdict::Reject,
            },
        );
        assert!(
            !f.goal()
                .evaluation()
                .desired_effects
                .values()
                .any(|e| e.effect.transition == "stage:ship")
        );
        // A recorded stage keeps the approvals its own record pinned.
        let pinned = f.host(Body::EffectMaterialized {
            effect: ready.effect,
        });
        for goal in f.replays() {
            assert_eq!(goal.standing(&pinned), Some(Standing::Effective));
        }
    }
}
#[test]
fn a_withdrawn_approval_signs_no_second_request_to_the_same_reviewer() {
    let mut f = Fixture::new(review_formation(1));
    let context = f.task();
    let subject = f.publish(0, context);
    let requests: Vec<_> = f
        .goal()
        .evaluation()
        .desired_effects
        .values()
        .cloned()
        .collect();
    for request in requests {
        f.worker(
            0,
            Body::EffectMaterialized {
                effect: request.effect,
            },
        );
    }
    f.review(1, context, subject);
    f.worker(
        1,
        Body::ReviewRecorded {
            context,
            subject,
            verdict: ReviewVerdict::Reject,
        },
    );
    assert!(f.goal().evaluation().desired_effects.is_empty());
    f.host(Body::MemberAdmitted {
        member: testkit::keypair(8).public(),
        endpoint: EndpointId([8; 32]),
        name: "New reviewer".into(),
        role: None,
    });
    let goal = f.goal();
    assert_eq!(goal.evaluation().desired_effects.len(), 1);
    let approval = f.review(2, context, subject);
    f.agent(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![approval],
    });
    f.worker(
        2,
        Body::ReviewRecorded {
            context,
            subject,
            verdict: ReviewVerdict::Reject,
        },
    );
    for goal in f.replays() {
        assert!(goal.evaluation().desired_effects.is_empty());
    }
}
#[test]
fn review_requests_skip_results_that_count_and_tasks_that_are_not_open() {
    let mut formation = review_formation(1);
    formation.decisions.finish = formation.decisions.selection.clone();
    let mut f = Fixture::new(formation);
    let context = f.task();
    let subject = f.publish(0, context);
    f.review(1, context, subject);
    assert!(f.goal().evaluation().desired_effects.is_empty());
    let other = f.publish(0, context);
    assert!(!f.goal().evaluation().desired_effects.is_empty());
    f.agent(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Close,
        evidence: vec![],
    });
    for goal in f.replays() {
        assert!(!goal.state().contributions[&other].approved);
        assert!(goal.evaluation().desired_effects.is_empty());
    }
}
#[test]
fn all_counts_only_when_every_part_does() {
    let mut formation = review_formation(1);
    let review = formation.decisions.completion.clone();
    formation.decisions.completion = CompletionRule::All {
        rules: vec![
            review,
            CompletionRule::Check {
                name: "build".into(),
                by: Selector::Members,
            },
        ],
    };
    let mut f = Fixture::new(formation);
    let context = f.task();
    let subject = f.publish(0, context);
    f.review(1, context, subject);
    assert!(!f.goal().state().contributions[&subject].approved);
    f.worker(
        1,
        Body::CheckAttested {
            context,
            subject,
            name: "build".into(),
            passed: true,
        },
    );
    for goal in f.replays() {
        assert!(goal.state().contributions[&subject].approved);
    }
}
#[test]
fn any_counts_when_one_part_does() {
    let mut formation = review_formation(1);
    let review = formation.decisions.completion.clone();
    formation.decisions.completion = CompletionRule::Any {
        rules: vec![
            review,
            CompletionRule::Check {
                name: "build".into(),
                by: Selector::Members,
            },
        ],
    };
    let mut f = Fixture::new(formation);
    let context = f.task();
    let subject = f.publish(0, context);
    f.worker(
        1,
        Body::CheckAttested {
            context,
            subject,
            name: "build".into(),
            passed: true,
        },
    );
    for goal in f.replays() {
        assert!(goal.state().contributions[&subject].approved);
    }
}

#[test]
fn a_rules_change_strands_no_task_and_revise_moves_one_to_the_current_rules() {
    let mut f = Fixture::new(preset_formation("directed"));
    let context = f.task();
    let subject = f.publish(0, context);
    let old_rules = f.rules;
    rebind(&mut f, preset_formation("peer-review"));
    let current = f.rules;
    let child = f.worker(
        0,
        Body::TaskOpened {
            binding: TaskBinding {
                rules: old_rules,
                task_type: None,
                inputs: BTreeMap::new(),
                parent: Some(context),
                stage: None,
            },
        },
    );
    let child_context = Context {
        scope: Scope::Task(TaskId::Authored(child)),
        round: child,
    };
    let child_subject = f.publish(0, child_context);
    f.review(1, context, subject);
    f.review(1, child_context, child_subject);
    let Scope::Task(task) = context.scope else {
        unreachable!()
    };
    let revised = f.host(Body::TaskRevised {
        task,
        expected_round: context.round,
        binding: TaskBinding {
            rules: current,
            task_type: None,
            inputs: BTreeMap::new(),
            parent: None,
            stage: None,
        },
    });
    let newer = Context {
        scope: Scope::Task(task),
        round: revised,
    };
    let new_subject = f.publish(0, newer);
    let review = f.agent(Body::ReviewRecorded {
        context: newer,
        subject: new_subject,
        verdict: ReviewVerdict::Approve,
    });
    for goal in f.replays() {
        assert_eq!(goal.standing(&review), Some(Standing::Effective));
        for id in [subject, child_subject, new_subject] {
            assert!(goal.state().contributions[&id].approved);
        }
    }
}

#[test]
fn an_opinion_never_opens_a_stage_that_requires_a_counting_review() {
    let mut formation = preset_formation("pipeline");
    formation.task_types.clear();
    formation.flow.get_mut("draft").unwrap().task_type = None;
    formation.flow.get_mut("ship").unwrap().requires[0].evidence = EvidenceKind::Review;
    let mut f = Fixture::new(formation);
    let stage = f
        .goal()
        .evaluation()
        .desired_effects
        .values()
        .find(|e| e.effect.transition == "stage:draft")
        .unwrap()
        .clone();
    let opened = f.host(Body::EffectMaterialized {
        effect: stage.effect,
    });
    let context = Context {
        scope: Scope::Task(TaskId::Derived(stage.id)),
        round: opened,
    };
    let subject = f.publish(0, context);
    let opinion = f.review(1, context, subject);
    for goal in f.replays() {
        assert_eq!(goal.standing(&opinion), Some(Standing::Effective));
        assert!(
            !goal
                .evaluation()
                .desired_effects
                .values()
                .any(|e| e.effect.transition == "stage:ship")
        );
    }
}

#[test]
fn a_signed_automatic_offer_and_its_acceptance_follow_each_stage_round() {
    use locust_proto::organization::StartRule;
    let mut formation = Formation::default();
    formation.work.starts = vec![StartRule::Offered {
        by: Selector::TaskCreator,
        to: Selector::Members,
    }];
    formation.flow.insert(
        "draft".into(),
        Stage {
            task_type: None,
            requires: vec![],
            recipients: Selector::Members,
        },
    );
    let mut f = Fixture::new(formation);
    let step = f
        .goal()
        .evaluation()
        .desired_effects
        .values()
        .next()
        .unwrap()
        .clone();
    let opened = f.host(Body::EffectMaterialized {
        effect: step.effect,
    });
    let task = TaskId::Derived(step.id);
    let mut round = opened;
    for revise in [false, true] {
        if revise {
            let binding = f.goal().state().tasks[&task].rounds[&round].binding.clone();
            round = f.host(Body::TaskRevised {
                task,
                expected_round: round,
                binding,
            });
        }
        let member = f.workers[0].key.public();
        let offer = f.goal().evaluation().desired_effects.values()
            .find(|step| matches!(step.effect.action, EffectAction::Offer { context, recipient } if context.round == round && recipient == member)).unwrap().clone();
        let offered = f.host(Body::EffectMaterialized {
            effect: offer.effect,
        });
        let accepted = f.worker(
            0,
            Body::AttemptStarted {
                context: Context {
                    scope: Scope::Task(task),
                    round,
                },
                offer: Some(offered),
                closure: None,
            },
        );
        for goal in f.replays() {
            assert_eq!(goal.standing(&offered), Some(Standing::Effective));
            assert_eq!(goal.standing(&accepted), Some(Standing::Effective));
            assert_eq!(goal.state().tasks[&task].current_round, round);
            assert_eq!(
                goal.state().effects[&offer.id].runner,
                f.governance.key.public()
            );
        }
    }
}

#[test]
fn stage_revisions_reject_task_creator_work_and_completion_rules_in_replay() {
    use locust_proto::organization::{DecisionRules, StartRule, TaskType, WorkRules};
    for (work, completion) in [
        (
            Some(WorkRules {
                starts: vec![StartRule::Independent {
                    by: Selector::TaskCreator,
                }],
                ..Default::default()
            }),
            None,
        ),
        (
            Some(WorkRules {
                starts: vec![StartRule::Offered {
                    by: Selector::Members,
                    to: Selector::TaskCreator,
                }],
                ..Default::default()
            }),
            None,
        ),
        (
            None,
            Some(CompletionRule::All {
                rules: vec![CompletionRule::Declaration {
                    by: Selector::TaskCreator,
                }],
            }),
        ),
    ] {
        let mut formation = Formation::default();
        formation.task_types.insert(
            "unusable".into(),
            TaskType {
                work,
                decisions: completion.map(|completion| DecisionRules {
                    completion,
                    ..Default::default()
                }),
            },
        );
        formation.flow.insert(
            "draft".into(),
            Stage {
                task_type: None,
                requires: vec![],
                recipients: Selector::Members,
            },
        );
        let mut f = Fixture::new(formation);
        let step = f
            .goal()
            .evaluation()
            .desired_effects
            .values()
            .next()
            .unwrap()
            .clone();
        let opened = f.host(Body::EffectMaterialized {
            effect: step.effect,
        });
        let task = TaskId::Derived(step.id);
        let mut binding = f.goal().state().tasks[&task].rounds[&opened]
            .binding
            .clone();
        binding.task_type = Some("unusable".into());
        let revised = f.host(Body::TaskRevised {
            task,
            expected_round: opened,
            binding,
        });
        for goal in f.replays() {
            assert!(matches!(
                goal.standing(&revised),
                Some(Standing::Excluded(_))
            ));
            assert_eq!(goal.state().tasks[&task].current_round, opened);
        }
    }
}

#[test]
fn a_fork_between_two_stage_steps_retracts_them_in_every_replay_order() {
    let mut formation = Formation::default();
    for name in ["first", "second"] {
        formation.flow.insert(
            name.into(),
            Stage {
                task_type: None,
                requires: vec![],
                recipients: Selector::Members,
            },
        );
    }
    let mut f = Fixture::new(formation);
    let steps: Vec<_> = f
        .goal()
        .evaluation()
        .desired_effects
        .values()
        .cloned()
        .collect();
    let first = f.host(Body::EffectMaterialized {
        effect: steps[0].effect.clone(),
    });
    let mut header = f.event(first).header().clone();
    header.body = Body::EffectMaterialized {
        effect: steps[1].effect.clone(),
    };
    let second = Event::sign(header, &f.governance.key).unwrap();
    let second_id = second.id();
    f.events.push(second);
    for goal in f.replays() {
        assert!(goal.evaluation().host_halt.is_some());
        assert_eq!(goal.state().head, Some(f.rules));
        assert!(goal.state().effects.is_empty());
        for id in [first, second_id] {
            assert_eq!(
                goal.standing(&id),
                Some(Standing::Pending(Waiting::ForkProof))
            );
        }
    }
}

#[test]
fn role_admissions_keep_the_host_when_an_earlier_formation_has_not_arrived() {
    let mut f = alone(preset_formation("review-panel"));
    let first_definition = *f.definitions.keys().next().unwrap();
    admit_worker(&mut f, 0, Some("reviewer"));
    admit_worker(&mut f, 1, Some("reviewer"));
    rebind(&mut f, preset_formation("directed"));
    let context = f.context();
    let subject = f.publish(0, context);
    let approval = f.agent(Body::ReviewRecorded {
        context,
        subject,
        verdict: ReviewVerdict::Approve,
    });
    let mut expected = vec![
        f.admin.key.public(),
        f.workers[0].key.public(),
        f.workers[1].key.public(),
    ];
    expected.sort();
    let earlier = f.definitions.remove(&first_definition).unwrap();
    for goal in f.replays() {
        assert_eq!(goal.state().roles["reviewer"], expected);
        assert_eq!(goal.standing(&approval), Some(Standing::Effective));
    }
    f.definitions.insert(first_definition, earlier);
    for goal in f.replays() {
        assert_eq!(goal.state().roles["reviewer"], expected);
        assert_eq!(goal.standing(&approval), Some(Standing::Effective));
    }
}

#[test]
fn a_binding_names_an_unadmitted_participant_in_its_refusal() {
    let mut f = Fixture::new(Formation::default());
    let mut formation = Formation::default();
    formation.decisions.selection = Some(Authority::Participant {
        key: testkit::keypair(99).public().to_string(),
    });
    let binding = rebind(&mut f, formation);
    for goal in f.replays() {
        assert_eq!(
            goal.standing(&binding),
            Some(Standing::Excluded(super::Exclusion::Precondition(
                "the participant that picks, closes or accepts file changes must be a member of the goal"
            )))
        );
    }
}

#[test]
fn reviews_checks_and_declarations_cannot_use_an_anchor_before_their_subject() {
    for kind in ["review", "check", "declaration"] {
        let mut formation = preset_formation("directed");
        let by = Selector::Role {
            name: "reviewer".into(),
        };
        formation.decisions.completion = match kind {
            "review" => CompletionRule::Reviews {
                by,
                count: 1,
                exclude_author: false,
            },
            "check" => CompletionRule::Check {
                name: "build".into(),
                by,
            },
            _ => CompletionRule::Declaration { by },
        };
        let mut f = Fixture::new(formation);
        let context = f.task();
        let old_anchor = f.anchor;
        f.host(Body::RoleHolders {
            role: "reviewer".into(),
            holders: vec![f.admin.key.public()],
        });
        let subject = f.publish(0, context);
        f.anchor = old_anchor;
        let witness = f.worker(
            1,
            match kind {
                "review" => Body::ReviewRecorded {
                    context,
                    subject,
                    verdict: ReviewVerdict::Approve,
                },
                "check" => Body::CheckAttested {
                    context,
                    subject,
                    name: "build".into(),
                    passed: true,
                },
                _ => Body::CompletionDeclared { context, subject },
            },
        );
        for goal in f.replays() {
            assert_eq!(
                goal.standing(&witness),
                Some(Standing::Excluded(super::Exclusion::Precondition(
                    "evidence is anchored before its subject"
                ))),
                "{kind}"
            );
            assert!(!goal.state().contributions[&subject].approved, "{kind}");
        }
    }
}

#[test]
fn automatic_offers_are_wanted_only_for_an_open_stage_round() {
    use locust_proto::organization::StartRule;
    for end in ["completed", "selected", "closed"] {
        let mut formation = Formation::default();
        formation.work.starts = vec![StartRule::Offered {
            by: Selector::TaskCreator,
            to: Selector::Members,
        }];
        formation.decisions.selection = Some(Authority::Participant {
            key: testkit::keypair(1).public().to_string(),
        });
        formation.decisions.finish = formation.decisions.selection.clone();
        formation.flow.insert(
            "draft".into(),
            Stage {
                task_type: None,
                requires: vec![],
                recipients: Selector::Members,
            },
        );
        let mut f = alone(formation);
        let stage = f
            .goal()
            .evaluation()
            .desired_effects
            .values()
            .next()
            .unwrap()
            .clone();
        let opened = f.host(Body::EffectMaterialized {
            effect: stage.effect,
        });
        let context = Context {
            scope: Scope::Task(TaskId::Derived(stage.id)),
            round: opened,
        };
        let offer = f
            .goal()
            .evaluation()
            .desired_effects
            .values()
            .find(|effect| matches!(effect.effect.action, EffectAction::Offer { .. }))
            .unwrap()
            .effect
            .clone();
        let signed = f.host(Body::EffectMaterialized { effect: offer });
        let subject = f.agent(publish_body(context));
        if end == "closed" {
            f.agent(Body::ScopeDecided {
                context,
                previous: None,
                action: DecisionAction::Close,
                evidence: vec![],
            });
        } else {
            let declaration = f.agent(Body::CompletionDeclared { context, subject });
            if end == "selected" {
                f.agent(Body::ScopeDecided {
                    context,
                    previous: None,
                    action: DecisionAction::Select { subject },
                    evidence: vec![declaration],
                });
            }
        }
        admit_worker(&mut f, 0, None);
        for goal in f.replays() {
            assert!(!goal.task_available(context), "{end}");
            assert_eq!(goal.standing(&signed), Some(Standing::Effective), "{end}");
            assert!(
                !goal
                    .evaluation()
                    .desired_effects
                    .values()
                    .any(|effect| matches!(effect.effect.action, EffectAction::Offer { .. })),
                "{end}"
            );
        }
    }
}

#[test]
fn a_new_lead_reopens_after_the_old_leads_later_log_position() {
    let mut formation = review_formation(1);
    formation.roles.insert("lead".into(), Default::default());
    formation.decisions.finish = Some(Authority::Role {
        name: "lead".into(),
    });
    let mut f = Fixture::new(formation);
    let context = f.task();
    let subject = f.publish(0, context);
    for _ in 0..3 {
        f.agent(Body::ReviewRecorded {
            context,
            subject,
            verdict: ReviewVerdict::Reject,
        });
    }
    let closed = f.agent(Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Close,
        evidence: vec![],
    });
    f.host(Body::RoleHolders {
        role: "lead".into(),
        holders: vec![f.workers[1].key.public()],
    });
    let reopened = f.worker(
        1,
        Body::ScopeDecided {
            context,
            previous: Some(closed),
            action: DecisionAction::Reopen,
            evidence: vec![],
        },
    );
    assert!(f.event(closed).header().seq > f.event(reopened).header().seq);
    let attempt = f.worker(
        0,
        Body::AttemptStarted {
            context,
            offer: None,
            closure: Some(reopened),
        },
    );
    for goal in f.replays() {
        assert_eq!(goal.standing(&attempt), Some(Standing::Effective));
        assert!(!goal.state().task_round(context).unwrap().closed);
        assert!(goal.evaluation().desired_effects.values().any(|effect|
            matches!(effect.effect.action, EffectAction::RequestReview { subject: target, .. } if target == subject)));
    }
}

#[test]
fn neither_of_two_lead_holders_can_pick() {
    let mut f = Fixture::new(preset_formation("directed"));
    let context = f.task();
    let subject = f.publish(0, context);
    let review = f.review(1, context, subject);
    let mut holders = vec![f.admin.key.public(), f.workers[0].key.public()];
    holders.sort();
    f.host(Body::RoleHolders {
        role: "lead".into(),
        holders,
    });
    let body = Body::ScopeDecided {
        context,
        previous: None,
        action: DecisionAction::Select { subject },
        evidence: vec![review],
    };
    let first = f.agent(body.clone());
    let second = f.worker(0, body);
    for goal in f.replays() {
        for id in [first, second] {
            assert!(matches!(goal.standing(&id), Some(Standing::Excluded(_))));
        }
        assert_eq!(goal.state().task_round(context).unwrap().selected, None);
    }
}
