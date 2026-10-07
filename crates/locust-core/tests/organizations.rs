//! Current-model acceptance through the public Engine seam and durable Store.
use locust_core::node::Node;
use locust_proto::api::*;
use locust_proto::engine::*;
use locust_proto::event::{AttemptStatus, Scope, TaskId};
use locust_proto::id::*;
use locust_proto::store::{MemStore, Store};
use std::collections::BTreeMap;

struct Random(u64);
impl Entropy for Random {
    fn fill(&mut self, bytes: &mut [u8]) {
        self.0 += 1;
        let hash = locust_proto::crypto::content_hash(&self.0.to_le_bytes());
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = hash.0[i % 32];
        }
    }
}
fn preset_formation(name: &str) -> locust_proto::organization::Formation {
    locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == name)
        .unwrap()
        .formation
}

struct Harness {
    node: Node<MemStore, Random>,
    store: MemStore,
    count: u64,
    principal: PublicKey,
    agent: ConnId,
    owner: ConnId,
}
impl Harness {
    fn new() -> Self {
        let store = MemStore::new();
        let node = Node::open(
            store.reopen(),
            Random(0),
            Credential([1; 32]).digest(),
            "test".into(),
            0,
        )
        .unwrap();
        let mut h = Self {
            node,
            store,
            count: 0,
            principal: PublicKey([0; 32]),
            agent: ConnId(0),
            owner: ConnId(0),
        };
        h.node.peer(
            PeerInput::Endpoint {
                endpoint: EndpointId([9; 32]),
                hints: vec![],
            },
            PeerTime {
                unix_ms: 0,
                elapsed_ms: 0,
            },
            &mut vec![],
        );
        h.owner = h.connect(1, None);
        let Response::AgentEnrolled { agent } = h.ok(
            h.owner,
            Request::AgentEnroll {
                name: "agent".into(),
                credential: Credential([2; 32]).digest(),
            },
        ) else {
            panic!()
        };
        h.principal = agent;
        h.agent = h.connect(2, Some(3));
        h
    }
    fn connect(&mut self, credential: u8, session: Option<u8>) -> ConnId {
        self.count += 1;
        let conn = ConnId(self.count);
        assert!(matches!(
            self.node.connect(
                conn,
                &ClientHello {
                    api_version: locust_proto::API_VERSION,
                    credential: Credential([credential; 32]),
                    session: session.map(|n| SessionSecret([n; 32]))
                },
                0
            ),
            ServerHello::Welcome { .. }
        ));
        conn
    }
    fn request(&mut self, conn: ConnId, request: Request) -> Result<Response, ApiError> {
        self.count += 1;
        let Step::Reply(reply) = self.node.request(
            conn,
            RequestFrame {
                id: self.count,
                idempotency: None,
                on_behalf: None,
                request,
            },
            1000,
        ) else {
            panic!()
        };
        reply.result
    }
    fn ok(&mut self, conn: ConnId, request: Request) -> Response {
        let name = request.name();
        self.request(conn, request)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
    }
    fn goal(&mut self, preset: &str) -> GoalId {
        self.goal_from(preset_formation(preset))
    }
    fn goal_from(&mut self, formation: locust_proto::organization::Formation) -> GoalId {
        let Response::GoalCreated { goal } = self.ok(
            self.owner,
            Request::GoalCreate {
                name: "host".into(),
                agent: self.principal,
                title: "Goal".into(),
                formation_json: Some(serde_json::to_string(&formation).unwrap()),

                inputs: BTreeMap::new(),
            },
        ) else {
            panic!()
        };
        goal
    }
    fn task(&mut self, goal: GoalId) -> TaskId {
        TaskId::Authored(recorded(self.ok(
            self.agent,
            Request::TaskOpen {
                goal,
                text: "Work".into(),
                task_type: None,
                inputs: BTreeMap::new(),
                parent: None,
            },
        )))
    }
    fn publish(&mut self, goal: GoalId, task: Option<TaskId>) -> EventId {
        let claim = task.map(|task| {
            let Response::Claimed(claim) = self.ok(
                self.agent,
                Request::AttemptStart {
                    goal,
                    task,
                    offer: None,
                },
            ) else {
                panic!()
            };
            claim
        });
        recorded(self.ok(
            self.agent,
            Request::ContributionPublish {
                goal,
                attempt: claim.map(|claim| claim.attempt),
                generation: claim.map(|claim| claim.generation),
                summary: "Evidence".into(),
                sources: Vec::new(),
                artifacts: vec![],
            },
        ))
    }
    fn pending(&mut self, goal: GoalId) -> PendingWork {
        let Response::Pending(work) = self.ok(self.agent, Request::Pending { goal }) else {
            panic!()
        };
        work
    }
    fn restart(&mut self) {
        self.node = Node::open(
            self.store.reopen(),
            Random(10000),
            Credential([1; 32]).digest(),
            "test".into(),
            0,
        )
        .unwrap();
        self.owner = self.connect(1, None);
        self.agent = self.connect(2, Some(3));
    }
}
fn recorded(response: Response) -> EventId {
    let Response::Recorded { event } = response else {
        panic!("{response:?}")
    };
    event
}

#[test]
fn open_findings_need_no_task_and_completion_does_not_create_a_selection() {
    let mut h = Harness::new();
    let goal = h.goal("open");
    let finding = h.publish(goal, None);
    h.ok(
        h.agent,
        Request::CompletionDeclare {
            goal,
            subject: finding,
        },
    );
    let Response::Contributions(contributions) =
        h.ok(h.agent, Request::Contributions { goal, task: None })
    else {
        panic!()
    };
    assert_eq!(contributions.len(), 1);
    assert!(contributions[0].approved);
    assert!(!contributions[0].selected);
    assert_eq!(contributions[0].context.scope, Scope::Goal);
    assert_eq!(
        h.request(
            h.agent,
            Request::ScopeSelect {
                goal,
                subject: finding,
                expected: None
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::NotEligible
    );
    h.restart();
    assert_eq!(
        h.ok(h.agent, Request::Contributions { goal, task: None }),
        Response::Contributions(contributions)
    );
}

#[test]
fn independent_attempts_and_local_aba_takeover_remain_distinct() {
    let mut h = Harness::new();
    let goal = h.goal("open");
    let task = h.task(goal);
    let Response::Claimed(a) = h.ok(
        h.agent,
        Request::AttemptStart {
            goal,
            task,
            offer: None,
        },
    ) else {
        panic!()
    };
    let b_conn = h.connect(2, Some(4));
    let Response::Pending(work) = h.ok(b_conn, Request::Pending { goal }) else {
        panic!()
    };
    assert!(work.to_start.iter().any(|item| item.task == task));
    let Response::Pending(work) = h.ok(h.agent, Request::Pending { goal }) else {
        panic!()
    };
    assert!(!work.to_start.iter().any(|item| item.task == task));
    let Response::Claimed(b) = h.ok(
        b_conn,
        Request::AttemptStart {
            goal,
            task,
            offer: None,
        },
    ) else {
        panic!()
    };
    assert_ne!(a.attempt, b.attempt);
    let Response::Claimed(taken) = h.ok(
        b_conn,
        Request::AttemptTakeover {
            goal,
            attempt: a.attempt,
        },
    ) else {
        panic!()
    };
    assert_eq!(taken.generation, 2);
    let Response::Claimed(back) = h.ok(
        h.agent,
        Request::AttemptTakeover {
            goal,
            attempt: a.attempt,
        },
    ) else {
        panic!()
    };
    assert_eq!(back.generation, 3);
    assert_eq!(
        h.request(
            h.agent,
            Request::AttemptReport {
                goal,
                attempt: a.attempt,
                generation: 1,
                status: AttemptStatus::Progress,
                text: "late".into()
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::Superseded
    );
    h.restart();
    h.ok(
        h.agent,
        Request::ContributionPublish {
            goal,
            attempt: Some(a.attempt),
            generation: Some(3),
            summary: "Finished independent work; awaiting declaration".into(),
            sources: vec![],
            artifacts: vec![],
        },
    );
    h.ok(
        h.agent,
        Request::AttemptReport {
            goal,
            attempt: a.attempt,
            generation: 3,
            status: AttemptStatus::Completed,
            text: "finished".into(),
        },
    );
    let Response::Task(detail) = h.ok(h.agent, Request::Task { goal, task }) else {
        panic!()
    };
    assert_eq!(detail.view.attempts.len(), 2);
    assert!(!detail.view.completed);
}

#[test]
fn pipeline_materializes_without_any_setting() {
    let mut h = Harness::new();
    // One member drives both stages alone, so the draft uses the default rules.
    let mut formation = preset_formation("pipeline");
    formation.flow.get_mut("draft").unwrap().task_type = None;
    formation.task_types.clear();
    let goal = h.goal_from(formation);
    let Response::Board(board) = h.ok(h.agent, Request::Board { goal }) else {
        panic!()
    };
    assert_eq!(board.len(), 1);
    assert!(matches!(board[0].task, TaskId::Derived(_)));
    let contribution = h.publish(goal, Some(board[0].task));
    h.ok(
        h.agent,
        Request::CompletionDeclare {
            goal,
            subject: contribution,
        },
    );
    let Response::Board(board) = h.ok(h.agent, Request::Board { goal }) else {
        panic!()
    };
    assert_eq!(board.len(), 2);
    let deliveries = h.pending(goal).deliveries;
    assert_eq!(deliveries.len(), 2);
    let count = h.store.log(&goal, 0, 1000).unwrap().len();
    h.restart();
    assert_eq!(h.store.log(&goal, 0, 1000).unwrap().len(), count);
    assert_eq!(h.pending(goal).deliveries, deliveries);
    h.ok(
        h.agent,
        Request::DeliveryAcknowledge {
            goal,
            effect: deliveries[0].effect,
        },
    );
    h.restart();
    assert!(
        h.pending(goal)
            .deliveries
            .iter()
            .find(|d| d.effect == deliveries[0].effect)
            .unwrap()
            .acknowledged
    );
}

#[test]
fn stage_steps_are_signed_while_the_hosts_agent_is_disconnected() {
    use locust_proto::event::Body;
    let mut h = Harness::new();
    let goal = h.goal("open");
    // Disconnect the host's agent before the stages exist: the first stage's
    // task must open with nobody but the governance key to sign it.
    h.ok(h.owner, Request::AgentRevoke { agent: h.principal });
    let mut formation = preset_formation("pipeline");
    formation.flow.get_mut("draft").unwrap().task_type = None;
    formation.task_types.clear();
    let Response::GoalStatus(status) = h.ok(h.owner, Request::GoalStatus { goal }) else {
        panic!()
    };
    assert!(status.hosted_here);
    assert_eq!(status.host, Some(h.principal));
    assert_ne!(status.governance, h.principal);
    let rules = recorded(h.ok(
        h.owner,
        Request::RulesBind {
            no_role: false,
            goal,
            expected: status.current_rules.unwrap(),
            formation_json: serde_json::to_string(&formation).unwrap(),

            inputs: BTreeMap::new(),
        },
    ));
    assert_eq!(
        h.store.event(&rules).unwrap().unwrap().header().author,
        status.governance
    );
    let Response::GoalStatus(status) = h.ok(h.owner, Request::GoalStatus { goal }) else {
        panic!()
    };
    assert!(status.halted.is_none());
    assert!(status.stalled.is_empty(), "{:?}", status.stalled);
    let Response::Board(board) = h.ok(h.owner, Request::Board { goal }) else {
        panic!()
    };
    assert_eq!(board.len(), 1);
    assert!(matches!(board[0].task, TaskId::Derived(_)));
    assert!(board[0].by_host);
    assert_eq!(board[0].creator, status.governance);
    let Response::Events(events) = h.ok(
        h.owner,
        Request::Events {
            goal,
            after: None,
            limit: 100,
        },
    ) else {
        panic!()
    };
    let steps: Vec<_> = events
        .iter()
        .filter(|event| event.kind == "effect_materialized")
        .collect();
    assert_eq!(steps.len(), 1);
    assert!(steps[0].by_host);
    assert_eq!(steps[0].author, status.governance);
    assert_eq!(steps[0].standing, Standing::Effective);
    // No record in the goal is the disconnected agent's.
    assert!(events.iter().all(|event| event.author != h.principal));
    let step = h.store.event(&steps[0].event).unwrap().unwrap();
    assert!(matches!(
        step.header().body,
        Body::EffectMaterialized { .. }
    ));
    assert_eq!(step.header().author, status.governance);
    // The disconnected agent itself signs nothing.
    assert_eq!(
        h.request(
            h.agent,
            Request::AttemptStart {
                goal,
                task: board[0].task,
                offer: None,
            },
        )
        .unwrap_err()
        .code,
        ErrorCode::Denied
    );
    // Connected again, the agent takes the step's task and the completed
    // draft opens the next stage, signed by the governance key as well.
    h.ok(h.owner, Request::AgentReconnect { agent: h.principal });
    h.agent = h.connect(2, Some(5));
    let contribution = h.publish(goal, Some(board[0].task));
    h.ok(
        h.agent,
        Request::CompletionDeclare {
            goal,
            subject: contribution,
        },
    );
    let Response::Board(board) = h.ok(h.agent, Request::Board { goal }) else {
        panic!()
    };
    assert_eq!(board.len(), 2);
    assert!(board.iter().all(|task| task.by_host));
    h.restart();
    let Response::Events(events) = h.ok(
        h.owner,
        Request::Events {
            goal,
            after: None,
            limit: 100,
        },
    ) else {
        panic!()
    };
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == "effect_materialized")
            .filter(|event| event.by_host && event.author == status.governance)
            .count(),
        2
    );
}

#[test]
fn scoped_selection_requires_completed_exact_contribution_and_cas() {
    let mut h = Harness::new();
    let goal = h.goal("independent-attempts");
    let task = h.task(goal);
    let first = h.publish(goal, Some(task));
    let second = h.publish(goal, Some(task));
    assert_eq!(
        h.request(
            h.agent,
            Request::ScopeSelect {
                goal,
                subject: first,
                expected: None
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::Conflict
    );
    h.ok(
        h.agent,
        Request::CompletionDeclare {
            goal,
            subject: first,
        },
    );
    h.ok(
        h.agent,
        Request::CompletionDeclare {
            goal,
            subject: second,
        },
    );
    let selection = recorded(h.ok(
        h.agent,
        Request::ScopeSelect {
            goal,
            subject: first,
            expected: None,
        },
    ));
    assert_eq!(
        h.request(
            h.agent,
            Request::ScopeSelect {
                goal,
                subject: second,
                expected: None
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::Conflict
    );
    h.ok(
        h.agent,
        Request::ScopeSelect {
            goal,
            subject: second,
            expected: Some(selection),
        },
    );
    h.restart();
    let Response::Task(task) = h.ok(h.agent, Request::Task { goal, task }) else {
        panic!()
    };
    assert_eq!(task.view.selected, Some(second));
}

#[test]
fn author_credential_is_private_cas_capability_without_goal_or_session_access() {
    let mut h = Harness::new();
    let goal = h.goal("open");
    let Response::AuthorEnrolled { author } = h.ok(
        h.owner,
        Request::AuthorEnroll {
            name: "designer".into(),
            credential: Credential([5; 32]).digest(),
        },
    ) else {
        panic!()
    };
    assert_ne!(author, h.principal);
    let conn = h.connect(5, None);
    let Response::FormationDraft(draft) = h.ok(
        conn,
        Request::FormationDraftCreate {
            id: "design".into(),
            expected_revision: 0,
            source: "{\"schema_version\":2}".into(),
        },
    ) else {
        panic!()
    };
    assert_eq!(draft.owner, author);
    let error = h
        .request(
            conn,
            Request::FormationDraftUpdate {
                id: "design".into(),
                expected_revision: 0,
                source: "{}".into(),
            },
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
    assert!(error.details_json.unwrap().contains("current_draft"));
    assert_eq!(
        h.request(conn, Request::GoalStatus { goal })
            .unwrap_err()
            .code,
        ErrorCode::Denied
    );
    assert_eq!(
        h.request(
            h.agent,
            Request::FormationDraft {
                id: "design".into()
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::NotFound
    );
    h.restart();
    let conn = h.connect(5, None);
    assert_eq!(
        h.ok(
            conn,
            Request::FormationDraft {
                id: "design".into()
            }
        ),
        Response::FormationDraft(draft)
    );
}

#[test]
fn closure_gates_authoring_and_reopened_starts_record_the_exact_position() {
    use locust_proto::{
        event::Body,
        organization::{Authority, Formation},
    };
    let mut h = Harness::new();
    let mut formation = Formation::default();
    formation.decisions.finish = Some(Authority::Participant {
        key: h.principal.to_string(),
    });
    let Response::GoalCreated { goal } = h.ok(
        h.owner,
        Request::GoalCreate {
            name: "host".into(),
            agent: h.principal,
            title: "Causal closure".into(),
            formation_json: Some(serde_json::to_string(&formation).unwrap()),

            inputs: BTreeMap::new(),
        },
    ) else {
        panic!()
    };
    let task = h.task(goal);
    let closed = recorded(h.ok(
        h.agent,
        Request::ScopeClose {
            goal,
            scope: Scope::Task(task),
            expected: None,
        },
    ));
    assert_eq!(
        h.request(
            h.agent,
            Request::AttemptStart {
                goal,
                task,
                offer: None
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::Conflict
    );
    let reopened = recorded(h.ok(
        h.agent,
        Request::ScopeReopen {
            goal,
            scope: Scope::Task(task),
            expected: Some(closed),
        },
    ));
    let Response::Claimed(claim) = h.ok(
        h.agent,
        Request::AttemptStart {
            goal,
            task,
            offer: None,
        },
    ) else {
        panic!()
    };
    let event = h.store.event(&claim.attempt).unwrap().unwrap();
    assert!(
        matches!(event.header().body, Body::AttemptStarted { closure: Some(position), .. } if position == reopened)
    );
    h.restart();
    let Response::Claimed(retried) = h.ok(
        h.agent,
        Request::AttemptStart {
            goal,
            task,
            offer: None,
        },
    ) else {
        panic!()
    };
    assert_eq!(retried.attempt, claim.attempt);
}

#[test]
fn nested_task_creation_and_revision_keep_parent_pin_after_default_amendment() {
    use locust_proto::event::Body;
    let mut h = Harness::new();
    let goal = h.goal("open");
    let parent = h.task(goal);
    let old_rules = h
        .store
        .log(&goal, 0, 100)
        .unwrap()
        .iter()
        .find_map(|(_, event)| {
            matches!(event.header().body, Body::RulesBound { .. }).then_some(event.id())
        })
        .unwrap();
    let child = recorded(h.ok(
        h.agent,
        Request::TaskOpen {
            goal,
            text: "Nested".into(),
            task_type: None,
            inputs: BTreeMap::new(),
            parent: Some(parent),
        },
    ));
    let formation = locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == "open")
        .unwrap()
        .formation;
    let new_rules = recorded(h.ok(
        h.owner,
        Request::RulesBind {
            no_role: false,
            goal,
            expected: old_rules,
            formation_json: serde_json::to_string(&formation).unwrap(),

            inputs: BTreeMap::new(),
        },
    ));
    assert_ne!(new_rules, old_rules);
    let next_child = recorded(h.ok(
        h.agent,
        Request::TaskOpen {
            goal,
            text: "Next nested".into(),
            task_type: None,
            inputs: BTreeMap::new(),
            parent: Some(parent),
        },
    ));
    let revised = recorded(h.ok(
        h.owner,
        Request::TaskRevise {
            goal,
            task: TaskId::Authored(child),
            expected_round: child,
            task_type: None,
        },
    ));
    for id in [next_child, revised] {
        let event = h.store.event(&id).unwrap().unwrap();
        let (Body::TaskOpened { binding } | Body::TaskRevised { binding, .. }) =
            &event.header().body
        else {
            panic!()
        };
        assert_eq!(binding.rules, old_rules);
    }
    h.restart();
    let Response::Board(tasks) = h.ok(h.agent, Request::Board { goal }) else {
        panic!()
    };
    assert_eq!(tasks.len(), 3);
}
