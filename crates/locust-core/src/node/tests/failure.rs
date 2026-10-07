//! Failed durability never permits a second signature before reopening.
use super::*;
use locust_proto::engine::{PeerEngine, PeerInput};
use locust_proto::event::{AuthorPoint, Event};
use locust_proto::id::EndpointId;
use locust_proto::id::{BlobHash, EventId, GoalId};
use locust_proto::store::{Commit, LocalRecord, Marks, Space, Store, StoreError};
use std::cell::Cell;
use std::rc::Rc;

struct Failing {
    inner: MemStore,
    fail: Rc<Cell<Option<bool>>>,
    effect_only: bool,
}
impl Store for Failing {
    fn commit(&mut self, tx: &Commit) -> Result<(), StoreError> {
        if (!self.effect_only
            || tx.events.iter().any(|event| {
                matches!(
                    event.header().body,
                    locust_proto::event::Body::EffectMaterialized { .. }
                )
            }))
            && let Some(after) = self.fail.take()
        {
            if after {
                self.inner.commit(tx)?;
            }
            return Err(StoreError::Failed("injected durability failure".into()));
        }
        self.inner.commit(tx)
    }
    fn event(&self, id: &EventId) -> Result<Option<Event>, StoreError> {
        self.inner.event(id)
    }
    fn has_event(&self, id: &EventId) -> Result<bool, StoreError> {
        self.inner.has_event(id)
    }
    fn log(
        &self,
        goal: &GoalId,
        after: u64,
        limit: usize,
    ) -> Result<Vec<(u64, Event)>, StoreError> {
        self.inner.log(goal, after, limit)
    }
    fn author_log(
        &self,
        goal: &GoalId,
        author: &PublicKey,
        after: Option<AuthorPoint>,
        limit: usize,
    ) -> Result<Vec<Event>, StoreError> {
        self.inner.author_log(goal, author, after, limit)
    }
    fn goals(&self) -> Result<Vec<GoalId>, StoreError> {
        self.inner.goals()
    }
    fn blob(&self, hash: &BlobHash) -> Result<Option<Vec<u8>>, StoreError> {
        self.inner.blob(hash)
    }
    fn blob_len(&self, hash: &BlobHash) -> Result<Option<u64>, StoreError> {
        self.inner.blob_len(hash)
    }
    fn blob_range(
        &self,
        hash: &BlobHash,
        offset: u64,
        len: usize,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        self.inner.blob_range(hash, offset, len)
    }
    fn stage_blob(
        &mut self,
        hash: &BlobHash,
        offset: u64,
        bytes: &[u8],
    ) -> Result<u64, StoreError> {
        self.inner.stage_blob(hash, offset, bytes)
    }
    fn staged_len(&self, hash: &BlobHash) -> Result<u64, StoreError> {
        self.inner.staged_len(hash)
    }
    fn staged_range(
        &self,
        hash: &BlobHash,
        offset: u64,
        len: usize,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        self.inner.staged_range(hash, offset, len)
    }
    fn discard_staged_blob(&mut self, hash: &BlobHash) -> Result<(), StoreError> {
        self.inner.discard_staged_blob(hash)
    }
    fn finish_blob(&mut self, hash: &BlobHash) -> Result<bool, StoreError> {
        self.inner.finish_blob(hash)
    }
    fn get(&self, space: Space, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        self.inner.get(space, key)
    }
    fn scan(&self, space: Space, prefix: &[u8]) -> Result<Vec<LocalRecord>, StoreError> {
        self.inner.scan(space, prefix)
    }
    fn marks(&self) -> Result<Marks, StoreError> {
        self.inner.marks()
    }
}
fn call(
    node: &mut impl Engine,
    conn: ConnId,
    principal: Option<PublicKey>,
    request: Request,
    key: Option<IdempotencyKey>,
) -> Result<Response, ApiError> {
    match node.request(
        conn,
        RequestFrame {
            id: 1,
            idempotency: key,
            on_behalf: principal,
            request,
        },
        100,
    ) {
        Step::Reply(reply) => reply.result,
        Step::Park(_) => panic!(),
    }
}

#[test]
fn failed_commit_before_or_after_durability_fences_node_and_reopen_resolves_outcome() {
    for after in [false, true] {
        let store = MemStore::new();
        let fail = Rc::new(Cell::new(None));
        let mut node = Node::open(
            Failing {
                inner: store.reopen(),
                fail: fail.clone(),
                effect_only: false,
            },
            Counting::new(31),
            OWNER.digest(),
            "test".into(),
            0,
        )
        .unwrap();
        node.peer(
            PeerInput::Endpoint {
                endpoint: EndpointId([31; 32]),
                hints: vec![],
            },
            locust_proto::engine::PeerTime {
                unix_ms: 0,
                elapsed_ms: 0,
            },
            &mut Vec::new(),
        );
        let hello = ClientHello {
            api_version: API_VERSION,
            credential: OWNER,
            session: None,
        };
        node.connect(ConnId(1), &hello, 0);
        let Response::AgentEnrolled { agent } = call(
            &mut node,
            ConnId(1),
            None,
            Request::AgentEnroll {
                name: "creator".into(),
                credential: credential(1).digest(),
            },
            None,
        )
        .unwrap() else {
            panic!()
        };
        let request = Request::GoalCreate {
            name: "host".into(),
            agent,
            title: "Atomic creation".into(),
            formation_json: Some("{\"schema_version\":2}".into()),

            inputs: Default::default(),
        };
        fail.set(Some(after));
        assert!(
            call(
                &mut node,
                ConnId(1),
                None,
                request.clone(),
                Some(IdempotencyKey([1; 16]))
            )
            .is_err()
        );
        assert_eq!(store.goals().unwrap().len(), usize::from(after));
        assert!(node.take_changed().is_empty());
        assert!(node.stop_requested());
        assert_eq!(
            code(call(&mut node, ConnId(1), None, request.clone(), None)),
            ErrorCode::Internal
        );
        let mut reopened = Node::open(
            store.reopen(),
            Counting::new(32),
            OWNER.digest(),
            "test".into(),
            0,
        )
        .unwrap();
        reopened.connect(ConnId(1), &hello, 0);
        let Response::GoalCreated { goal } = call(
            &mut reopened,
            ConnId(1),
            None,
            request,
            Some(IdempotencyKey([1; 16])),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(store.goals().unwrap(), vec![goal]);
        assert_eq!(store.log(&goal, 0, 256).unwrap().len(), 3);
        assert_eq!(reopened.goals[&goal].state().members.len(), 1);
    }
}

#[test]
fn exhausted_revision_refuses_before_changing_the_goal_projection() {
    let (mut daemon, _, _, agent, goal) = super::lifecycle::setup();
    let mut tx = crate::node::commit::Tx::none();
    tx.local(crate::node::local::revision_write(&goal, u64::MAX));
    daemon.node.land(tx).unwrap();
    let before = daemon.store.log(&goal, 0, usize::MAX).unwrap();
    let refused = daemon.call(
        agent,
        Request::TaskOpen {
            goal,
            text: "must not appear".into(),
            task_type: None,
            inputs: Default::default(),
            parent: None,
        },
    );
    assert_eq!(code(refused), ErrorCode::Conflict);
    assert!(daemon.node.goals[&goal].state().tasks.is_empty());
    assert_eq!(daemon.store.log(&goal, 0, usize::MAX).unwrap(), before);
}

#[test]
fn effect_and_recipient_records_commit_atomically_and_uncertain_commit_requires_reopen() {
    use locust_proto::event::Body;
    for after in [false, true] {
        let (store, goal, _principal) = super::delivery::unmaterialized();
        let fail = Rc::new(Cell::new(None));
        let mut node = Node::open(
            Failing {
                inner: store.reopen(),
                fail: fail.clone(),
                effect_only: true,
            },
            Counting::new(83),
            OWNER.digest(),
            "test".into(),
            0,
        )
        .unwrap();
        let hello = ClientHello {
            api_version: API_VERSION,
            credential: OWNER,
            session: None,
        };
        node.connect(ConnId(1), &hello, 0);
        fail.set(Some(after));
        let expected = node.goals[&goal].state().current_rules.unwrap();
        let result = call(
            &mut node,
            ConnId(1),
            None,
            super::delivery::pipeline_request(goal, expected),
            None,
        );
        assert!(result.is_err());
        assert!(node.stop_requested());
        let signed = store
            .log(&goal, 0, usize::MAX)
            .unwrap()
            .iter()
            .filter(|(_, event)| matches!(event.header().body, Body::EffectMaterialized { .. }))
            .count();
        assert_eq!(signed, usize::from(after));
        assert_eq!(
            store.scan(Space::Pending, &[]).unwrap().len(),
            if after { 2 } else { 0 }
        );
        // No tentative delivery is visible after an uncertain commit.
        assert!(node.goals[&goal].deliveries.is_empty());
        let reopened = Node::open(
            store.reopen(),
            Counting::new(84),
            OWNER.digest(),
            "test".into(),
            0,
        )
        .unwrap();
        let signed = store
            .log(&goal, 0, usize::MAX)
            .unwrap()
            .iter()
            .filter(|(_, event)| matches!(event.header().body, Body::EffectMaterialized { .. }))
            .count();
        assert_eq!(signed, 1);
        assert_eq!(reopened.goals[&goal].deliveries.len(), 2);
        assert!(
            reopened.goals[&goal]
                .deliveries
                .values()
                .all(|record| record.available)
        );
    }
}

#[test]
fn recipient_inbox_commit_failure_never_issues_a_positive_receipt_before_reopen() {
    use crate::sync::{Host, Replica};
    for after in [false, true] {
        let fixture = super::delivery::failure_fixture();
        let fail = Rc::new(Cell::new(None));
        let mut recipient = Node::open(
            Failing {
                inner: fixture.recipient.reopen(),
                fail: fail.clone(),
                effect_only: false,
            },
            Counting::new(91),
            OWNER.digest(),
            "test".into(),
            0,
        )
        .unwrap();
        fail.set(Some(after));
        assert!(
            recipient
                .replica(&fixture.goal)
                .unwrap()
                .receive(fixture.incoming.clone())
                .is_err()
        );
        assert!(recipient.stop_requested());
        assert!(recipient.goals[&fixture.goal].deliveries.is_empty());
        assert!(recipient.replica(&fixture.goal).is_none());
        assert!(Replica::receive_delivery(&mut recipient, fixture.effect, fixture.target).is_err());
        assert_eq!(
            fixture.recipient.scan(Space::Pending, &[]).unwrap().len(),
            if after { 2 } else { 0 }
        );
        let mut reopened = Node::open(
            fixture.recipient.reopen(),
            Counting::new(92),
            OWNER.digest(),
            "test".into(),
            0,
        )
        .unwrap();
        assert_eq!(
            reopened
                .replica(&fixture.goal)
                .unwrap()
                .receive_delivery(fixture.effect, fixture.target)
                .unwrap(),
            after
        );
        reopened
            .replica(&fixture.goal)
            .unwrap()
            .receive(fixture.incoming.clone())
            .unwrap();
        assert!(
            reopened
                .replica(&fixture.goal)
                .unwrap()
                .receive_delivery(fixture.effect, fixture.target)
                .unwrap()
        );
        assert_eq!(
            fixture.recipient.scan(Space::Pending, &[]).unwrap().len(),
            2
        );
        let conn = ConnId(2);
        reopened.connect(
            conn,
            &ClientHello {
                api_version: API_VERSION,
                credential: credential(2),
                session: None,
            },
            0,
        );
        let Response::Pending(work) = call(
            &mut reopened,
            conn,
            None,
            Request::Pending { goal: fixture.goal },
            None,
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(work.deliveries.len(), 1);
        assert!(work.deliveries[0].received);
        assert!(!work.deliveries[0].acknowledged);
        assert!(work.claimed.is_empty());
    }
}

#[test]
fn sender_receipt_commit_failure_keeps_memory_pending_and_reopen_resolves_durability() {
    use crate::sync::{Host, Replica};
    for after in [false, true] {
        let fixture = super::delivery::failure_fixture();
        let mut recipient = Node::open(
            fixture.recipient.reopen(),
            Counting::new(93),
            OWNER.digest(),
            "test".into(),
            0,
        )
        .unwrap();
        recipient
            .replica(&fixture.goal)
            .unwrap()
            .receive(fixture.incoming.clone())
            .unwrap();
        assert!(
            recipient
                .replica(&fixture.goal)
                .unwrap()
                .receive_delivery(fixture.effect, fixture.target)
                .unwrap()
        );
        let fail = Rc::new(Cell::new(None));
        let mut sender = Node::open(
            Failing {
                inner: fixture.sender.reopen(),
                fail: fail.clone(),
                effect_only: false,
            },
            Counting::new(94),
            OWNER.digest(),
            "test".into(),
            0,
        )
        .unwrap();
        assert!(
            !sender.goals[&fixture.goal].deliveries[&(fixture.effect, fixture.target)].delivered
        );
        let event_count = fixture
            .sender
            .log(&fixture.goal, 0, usize::MAX)
            .unwrap()
            .len();
        fail.set(Some(after));
        assert!(
            sender
                .replica(&fixture.goal)
                .unwrap()
                .receive_receipt(fixture.recipient_endpoint, fixture.effect, fixture.target)
                .is_err()
        );
        assert!(sender.stop_requested());
        assert!(
            !sender.goals[&fixture.goal].deliveries[&(fixture.effect, fixture.target)].delivered
        );
        assert!(sender.replica(&fixture.goal).is_none());
        assert!(Replica::next_delivery(&sender, fixture.recipient_endpoint, None).is_none());
        let mut reopened = Node::open(
            fixture.sender.reopen(),
            Counting::new(95),
            OWNER.digest(),
            "test".into(),
            0,
        )
        .unwrap();
        assert_eq!(
            reopened.goals[&fixture.goal].deliveries[&(fixture.effect, fixture.target)].delivered,
            after
        );
        assert_eq!(
            reopened
                .replica(&fixture.goal)
                .unwrap()
                .next_delivery(fixture.recipient_endpoint, None)
                .is_some(),
            !after
        );
        reopened
            .replica(&fixture.goal)
            .unwrap()
            .receive_receipt(fixture.recipient_endpoint, fixture.effect, fixture.target)
            .unwrap();
        reopened
            .replica(&fixture.goal)
            .unwrap()
            .receive_receipt(fixture.recipient_endpoint, fixture.effect, fixture.target)
            .unwrap();
        assert!(
            reopened.goals[&fixture.goal].deliveries[&(fixture.effect, fixture.target)].delivered
        );
        assert_eq!(
            fixture
                .sender
                .log(&fixture.goal, 0, usize::MAX)
                .unwrap()
                .len(),
            event_count
        );
        assert_eq!(fixture.sender.scan(Space::Pending, &[]).unwrap().len(), 2);
    }
}

#[test]
fn cancellation_acknowledgment_and_terminal_report_commit_together() {
    use super::lifecycle::{authorize, event, offered, setup};
    use locust_proto::event::{AttemptStatus, CancelOutcome};

    for after in [false, true] {
        let (mut d, principal, owner, agent, goal) = setup();
        let (task, offer) = offered(&mut d, agent, goal, principal);
        authorize(&mut d, owner, goal, task, principal);
        let Response::Claimed(claim) = d.ok(
            agent,
            Request::AttemptStart {
                goal,
                task,
                offer: Some(offer),
            },
        ) else {
            panic!()
        };
        let cancel = event(d.ok(
            agent,
            Request::AttemptCancel {
                goal,
                attempt: claim.attempt,
            },
        ));
        let before = d.store.log(&goal, 0, 1000).unwrap().len();
        let fail = Rc::new(Cell::new(None));
        let open = || {
            Node::open(
                Failing {
                    inner: d.store.reopen(),
                    fail: fail.clone(),
                    effect_only: false,
                },
                Counting::new(91),
                OWNER.digest(),
                "test".into(),
                0,
            )
            .unwrap()
        };
        let hello = ClientHello {
            api_version: API_VERSION,
            credential: credential(1),
            session: Some(session(1)),
        };
        let mut node = open();
        node.connect(ConnId(1), &hello, 0);
        let request = Request::CancelAcknowledge {
            goal,
            cancel,
            generation: Some(1),
            outcome: CancelOutcome::Stopped,
        };
        let key = Some(IdempotencyKey([71; 16]));
        fail.set(Some(after));
        assert!(call(&mut node, ConnId(1), None, request.clone(), key).is_err());
        assert_eq!(
            d.store.log(&goal, 0, 1000).unwrap().len(),
            before + if after { 2 } else { 0 }
        );
        assert_eq!(
            code(call(&mut node, ConnId(1), None, request.clone(), key)),
            ErrorCode::Internal
        );
        let mut node = open();
        node.connect(ConnId(1), &hello, 0);
        let response = call(&mut node, ConnId(1), None, request.clone(), key).unwrap();
        assert_eq!(d.store.log(&goal, 0, 1000).unwrap().len(), before + 2);
        assert_eq!(
            node.goals[&goal].state().attempts[&claim.attempt].status,
            Some(AttemptStatus::Abandoned)
        );
        assert_eq!(
            call(&mut node, ConnId(1), None, request, key).unwrap(),
            response
        );
        assert_eq!(d.store.log(&goal, 0, 1000).unwrap().len(), before + 2);
    }
}
