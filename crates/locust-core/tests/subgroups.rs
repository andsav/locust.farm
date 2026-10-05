//! Separate-goal subgroup isolation through authenticated public Engine requests.
use locust_core::node::Node;
use locust_proto::api::*;
use locust_proto::engine::*;
use locust_proto::event::{ReviewVerdict, Scope};
use locust_proto::id::*;
use locust_proto::store::MemStore;
use std::collections::BTreeMap;

struct Random(u64);
impl Entropy for Random {
    fn fill(&mut self, bytes: &mut [u8]) {
        self.0 += 1;
        let hash = locust_proto::crypto::content_hash(&self.0.to_le_bytes());
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = hash.0[index % 32];
        }
    }
}
struct Harness {
    node: Node<MemStore, Random>,
    store: MemStore,
    sequence: u64,
}
impl Harness {
    fn new() -> Self {
        let store = MemStore::new();
        let node = Node::open(
            store.reopen(),
            Random(0),
            Credential([1; 32]).digest(),
            "subgroups".into(),
            0,
        )
        .unwrap();
        let mut h = Self {
            node,
            store,
            sequence: 0,
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
        h.connect(1);
        h
    }
    fn connect(&mut self, tag: u8) -> ConnId {
        let conn = ConnId(u64::from(tag));
        assert!(matches!(
            self.node.connect(
                conn,
                &ClientHello {
                    api_version: locust_proto::API_VERSION,
                    credential: Credential([tag; 32]),
                    session: None
                },
                0
            ),
            ServerHello::Welcome { .. }
        ));
        conn
    }
    fn call(&mut self, conn: ConnId, request: Request) -> Result<Response, ApiError> {
        self.sequence += 1;
        let Step::Reply(reply) = self.node.request(
            conn,
            RequestFrame {
                id: self.sequence,
                idempotency: None,
                on_behalf: None,
                request,
            },
            1000,
        ) else {
            panic!("expected direct reply")
        };
        reply.result
    }
    fn ok(&mut self, conn: ConnId, request: Request) -> Response {
        let name = request.name();
        self.call(conn, request)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
    }
    fn enroll(&mut self, tag: u8) -> (PublicKey, ConnId) {
        let Response::AgentEnrolled { agent } = self.ok(
            ConnId(1),
            Request::AgentEnroll {
                name: format!("member-{tag}"),
                grants: Grants { manage_goals: true },
                credential: Credential([tag; 32]).digest(),
            },
        ) else {
            panic!()
        };
        (agent, self.connect(tag))
    }
    fn goal(&mut self, conn: ConnId, creator: PublicKey, preset: &str) -> GoalId {
        let formation = locust_proto::organization::presets()
            .into_iter()
            .find(|p| p.name == preset)
            .unwrap()
            .formation;
        let roles = formation
            .roles
            .keys()
            .map(|name| (name.clone(), vec![creator]))
            .collect();
        let Response::GoalCreated { goal } = self.ok(
            conn,
            Request::GoalCreate {
                title: format!("{preset} goal"),
                formation_json: Some(serde_json::to_string(&formation).unwrap()),
                roles,
                inputs: BTreeMap::new(),
            },
        ) else {
            panic!()
        };
        self.grant(goal, creator);
        goal
    }
    fn grant(&mut self, goal: GoalId, agent: PublicKey) {
        self.ok(
            ConnId(1),
            Request::GoalGrant {
                goal,
                agent,
                grants: GoalGrants {
                    administer: true,
                    contribute: true,
                    execute: true,
                    review: true,
                    select: true,
                    flow: false,
                    takeover: false,
                },
            },
        );
    }
    fn join(&mut self, issuer: ConnId, member: ConnId, goal: GoalId, principal: PublicKey) {
        let Response::Invited { ticket } = self.ok(
            issuer,
            Request::GoalInvite {
                goal,
                expires_ms: None,
            },
        ) else {
            panic!()
        };
        assert!(matches!(
            self.ok(member, Request::GoalJoin { ticket }),
            Response::Joined {
                membership: Membership::Member,
                ..
            }
        ));
        self.grant(goal, principal);
    }
    fn put(&mut self, conn: ConnId, goal: GoalId, bytes: &[u8]) -> BlobHash {
        let Response::BlobStored { hash } = self.ok(
            conn,
            Request::BlobPut {
                goal,
                bytes: bytes.to_vec(),
            },
        ) else {
            panic!()
        };
        hash
    }
    fn get(&mut self, conn: ConnId, goal: GoalId, hash: BlobHash) -> Vec<u8> {
        let Response::Blob { bytes } = self.ok(conn, Request::BlobGet { goal, hash }) else {
            panic!()
        };
        bytes
    }
    fn publish(
        &mut self,
        conn: ConnId,
        goal: GoalId,
        artifact: BlobHash,
        summary: &str,
    ) -> EventId {
        let Response::Recorded { event } = self.ok(
            conn,
            Request::ContributionPublish {
                goal,
                task: None,
                attempt: None,
                generation: None,
                summary: summary.into(),
                sources: Vec::new(),
                artifacts: vec![artifact],
            },
        ) else {
            panic!()
        };
        event
    }
    fn contributions(&mut self, conn: ConnId, goal: GoalId) -> Vec<ContributionView> {
        let Response::Contributions(items) =
            self.ok(conn, Request::Contributions { goal, task: None })
        else {
            panic!()
        };
        items
    }
    fn restart(&mut self) {
        self.node = Node::open(
            self.store.reopen(),
            Random(10000),
            Credential([1; 32]).digest(),
            "subgroups".into(),
            0,
        )
        .unwrap();
        for tag in [1, 2, 3] {
            self.connect(tag);
        }
    }
}

#[test]
fn separate_goal_exports_only_selected_bytes_and_returns_a_fresh_parent_candidate() {
    let mut h = Harness::new();
    let (bridge, parent_member) = h.enroll(2);
    let (child_principal, child_member) = h.enroll(3);
    let parent = h.goal(parent_member, bridge, "coordinator");
    let child = h.goal(child_member, child_principal, "open");
    h.join(child_member, parent_member, child, bridge);

    let private = h.put(parent_member, parent, b"private parent deliberation");
    let selected = h.put(parent_member, parent, b"selected contract for subgroup");
    let private_event = h.publish(parent_member, parent, private, "Private draft");
    let selected_event = h.publish(parent_member, parent, selected, "Selected scope");
    h.ok(
        parent_member,
        Request::ReviewRecord {
            goal: parent,
            subject: selected_event,
            verdict: ReviewVerdict::Approve,
            text: "Approved exact scope".into(),
        },
    );
    let Response::Recorded {
        event: parent_selection,
    } = h.ok(
        parent_member,
        Request::ScopeSelect {
            goal: parent,
            subject: selected_event,
            expected: None,
        },
    )
    else {
        panic!()
    };

    for request in [
        Request::BlobGet {
            goal: parent,
            hash: private,
        },
        Request::BlobGet {
            goal: parent,
            hash: selected,
        },
        Request::Contributions {
            goal: parent,
            task: None,
        },
    ] {
        assert_eq!(
            h.call(child_member, request).unwrap_err().code,
            ErrorCode::NotFound
        );
    }
    let selected_view = h
        .contributions(parent_member, parent)
        .into_iter()
        .find(|c| c.selected)
        .unwrap();
    assert_eq!(selected_view.contribution, selected_event);
    assert_eq!(selected_view.artifacts, vec![selected]);
    let bytes = h.get(parent_member, parent, selected_view.artifacts[0]);
    let exported = h.put(parent_member, child, &bytes);
    assert_ne!(exported, selected, "sealed identity is goal-bound");
    let export_event = h.publish(
        parent_member,
        child,
        exported,
        "Explicit selected context export",
    );
    assert_eq!(
        h.get(child_member, child, exported),
        b"selected contract for subgroup"
    );
    assert!(
        h.call(
            child_member,
            Request::BlobGet {
                goal: child,
                hash: private
            }
        )
        .is_err()
    );
    assert!(
        h.call(
            child_member,
            Request::BlobGet {
                goal: child,
                hash: selected
            }
        )
        .is_err()
    );
    let child_history = h.contributions(child_member, child);
    assert_eq!(child_history.len(), 1);
    assert_eq!(child_history[0].contribution, export_event);
    assert!(
        !child_history
            .iter()
            .any(|c| c.contribution == private_event || c.contribution == selected_event)
    );

    let child_result = h.put(child_member, child, b"subgroup answer");
    let child_finding = h.publish(
        child_member,
        child,
        child_result,
        "Independent subgroup finding",
    );
    let returned_bytes = h.get(parent_member, child, child_result);
    let returned = h.put(parent_member, parent, &returned_bytes);
    assert_ne!(returned, child_result);
    let parent_candidate = h.publish(parent_member, parent, returned, "Returned subgroup finding");
    let candidate = h
        .contributions(parent_member, parent)
        .into_iter()
        .find(|c| c.contribution == parent_candidate)
        .unwrap();
    assert_eq!(candidate.context.scope, Scope::Goal);
    assert_eq!(candidate.artifacts, vec![returned]);
    assert!(!candidate.approved && !candidate.selected && candidate.evidence.is_empty());
    assert_ne!(parent_candidate, child_finding);
    assert_eq!(
        h.call(
            parent_member,
            Request::ScopeSelect {
                goal: parent,
                subject: parent_candidate,
                expected: Some(parent_selection)
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::Conflict
    );

    h.restart();
    assert_eq!(h.get(parent_member, parent, returned), b"subgroup answer");
    assert_eq!(
        h.call(
            child_member,
            Request::BlobGet {
                goal: parent,
                hash: returned
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::NotFound
    );
    let candidate = h
        .contributions(parent_member, parent)
        .into_iter()
        .find(|c| c.contribution == parent_candidate)
        .unwrap();
    assert!(!candidate.approved && !candidate.selected);
    h.ok(
        parent_member,
        Request::ReviewRecord {
            goal: parent,
            subject: parent_candidate,
            verdict: ReviewVerdict::Approve,
            text: "Parent review of returned bytes".into(),
        },
    );
    h.ok(
        parent_member,
        Request::ScopeSelect {
            goal: parent,
            subject: parent_candidate,
            expected: Some(parent_selection),
        },
    );
    assert!(
        h.contributions(parent_member, parent)
            .iter()
            .any(|c| c.contribution == parent_candidate && c.approved && c.selected)
    );
}
