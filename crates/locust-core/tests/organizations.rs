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
                grants: Grants { manage_goals: true },
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
        let blueprint = locust_proto::organization::presets()
            .into_iter()
            .find(|p| p.name == preset)
            .unwrap()
            .blueprint;
        let roles = blueprint
            .roles
            .keys()
            .map(|role| (role.clone(), vec![self.principal]))
            .collect();
        let Response::GoalCreated { goal } = self.ok(
            self.agent,
            Request::GoalCreate {
                title: "Goal".into(),
                blueprint_json: Some(serde_json::to_string(&blueprint).unwrap()),
                roles,
                inputs: BTreeMap::new(),
            },
        ) else {
            panic!()
        };
        goal
    }
    fn grant(&mut self, goal: GoalId, flow: bool) {
        self.ok(
            self.owner,
            Request::GoalGrant {
                goal,
                agent: self.principal,
                grants: GoalGrants {
                    administer: true,
                    contribute: true,
                    execute: true,
                    review: true,
                    select: true,
                    flow,
                    takeover: true,
                },
            },
        );
    }
    fn task(&mut self, goal: GoalId) -> TaskId {
        TaskId::Authored(recorded(self.ok(
            self.agent,
            Request::TaskOpen {
                goal,
                text: "Work".into(),
                variation: None,
                inputs: BTreeMap::new(),
                parent: None,
            },
        )))
    }
    fn publish(&mut self, goal: GoalId, task: Option<TaskId>) -> EventId {
        recorded(self.ok(
            self.agent,
            Request::ContributionPublish {
                goal,
                task,
                attempt: None,
                generation: None,
                summary: "Evidence".into(),
                base: None,
                patch: None,
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
    h.grant(goal, false);
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
        ErrorCode::Conflict
    );
    assert_eq!(
        h.ok(h.agent, Request::Board { goal }),
        Response::Board(vec![])
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
    h.grant(goal, false);
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
fn pipeline_materializes_on_grant_and_completion_without_agent_polling() {
    let mut h = Harness::new();
    let goal = h.goal("pipeline");
    assert_eq!(
        h.ok(h.agent, Request::Board { goal }),
        Response::Board(vec![])
    );
    h.grant(goal, true);
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
fn scoped_selection_requires_completed_exact_contribution_and_cas() {
    let mut h = Harness::new();
    let goal = h.goal("independent-attempts");
    h.grant(goal, false);
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
    let Response::BlueprintDraft(draft) = h.ok(
        conn,
        Request::BlueprintDraftCreate {
            id: "design".into(),
            expected_revision: 0,
            source: "{\"schema_version\":1}".into(),
        },
    ) else {
        panic!()
    };
    assert_eq!(draft.owner, author);
    let error = h
        .request(
            conn,
            Request::BlueprintDraftUpdate {
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
            Request::BlueprintDraft {
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
            Request::BlueprintDraft {
                id: "design".into()
            }
        ),
        Response::BlueprintDraft(draft)
    );
}

#[test]
fn closure_gates_authoring_and_reopened_starts_record_the_exact_position() {
    use locust_proto::{
        event::Body,
        organization::{Authority, Blueprint},
    };
    let mut h = Harness::new();
    let mut blueprint = Blueprint::default();
    blueprint.decisions.closure = Some(Authority::Participant {
        key: h.principal.to_string(),
    });
    let Response::GoalCreated { goal } = h.ok(
        h.agent,
        Request::GoalCreate {
            title: "Causal closure".into(),
            blueprint_json: Some(serde_json::to_string(&blueprint).unwrap()),
            roles: BTreeMap::new(),
            inputs: BTreeMap::new(),
        },
    ) else {
        panic!()
    };
    h.grant(goal, false);
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
        ErrorCode::Denied
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
    h.grant(goal, false);
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
            variation: None,
            inputs: BTreeMap::new(),
            parent: Some(parent),
        },
    ));
    let blueprint = locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == "open")
        .unwrap()
        .blueprint;
    let new_rules = recorded(h.ok(
        h.agent,
        Request::RulesBind {
            goal,
            expected: old_rules,
            blueprint_json: serde_json::to_string(&blueprint).unwrap(),
            roles: BTreeMap::new(),
            inputs: BTreeMap::new(),
        },
    ));
    assert_ne!(new_rules, old_rules);
    let next_child = recorded(h.ok(
        h.agent,
        Request::TaskOpen {
            goal,
            text: "Next nested".into(),
            variation: None,
            inputs: BTreeMap::new(),
            parent: Some(parent),
        },
    ));
    let revised = recorded(h.ok(
        h.agent,
        Request::TaskRevise {
            goal,
            task: TaskId::Authored(child),
            expected_round: child,
            variation: None,
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
