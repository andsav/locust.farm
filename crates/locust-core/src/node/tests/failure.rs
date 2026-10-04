//! Failed durability never permits a second signature before reopening.
use super::*;
use locust_proto::engine::{PeerEngine, PeerInput};
use locust_proto::event::{AuthorPoint, Event};
use locust_proto::id::EndpointId;
use locust_proto::id::{BlobHash, EventId, GoalId};
use locust_proto::store::{Commit, LocalRecord, Space, Store, StoreError};
use std::cell::Cell;
use std::rc::Rc;

struct Failing {
    inner: MemStore,
    fail: Rc<Cell<Option<bool>>>,
}
impl Store for Failing {
    fn commit(&mut self, tx: &Commit) -> Result<(), StoreError> {
        if let Some(after) = self.fail.take() {
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
                grants: Grants { manage_goals: true },
                credential: credential(1).digest(),
            },
            None,
        )
        .unwrap() else {
            panic!()
        };
        let request = Request::GoalCreate {
            title: "Atomic creation".into(),
        };
        fail.set(Some(after));
        assert!(
            call(
                &mut node,
                ConnId(1),
                Some(agent),
                request.clone(),
                Some(IdempotencyKey([1; 16]))
            )
            .is_err()
        );
        assert_eq!(store.goals().unwrap().len(), usize::from(after));
        assert!(node.take_changed().is_empty());
        assert!(node.stop_requested());
        assert_eq!(
            code(call(
                &mut node,
                ConnId(1),
                Some(agent),
                request.clone(),
                None
            )),
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
            Some(agent),
            request,
            Some(IdempotencyKey([1; 16])),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(store.goals().unwrap(), vec![goal]);
        assert_eq!(store.log(&goal, 0, 256).unwrap().len(), 2);
        assert_eq!(reopened.goals[&goal].state().members.len(), 1);
    }
}
