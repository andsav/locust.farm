//! Tests of the node, as transcripts through the two seams of
//! `locust_proto::engine`.

mod daemon;

use locust_proto::API_VERSION;
use locust_proto::api::{
    ApiError, Caller, ClientHello, Credential, ErrorCode, Grants, Request, RequestFrame, Response,
    ServerHello, SessionSecret,
};
use locust_proto::engine::{ConnId, Engine, Entropy, Step};
use locust_proto::id::{IdempotencyKey, PublicKey};
use locust_proto::store::MemStore;

use super::Node;

/// Deterministic random bytes: a counter, so every draw differs and a test
/// run repeats exactly.
pub(super) struct Counting {
    next: u64,
}

impl Counting {
    /// A source whose draws differ from every other `seed`'s.
    pub fn new(seed: u8) -> Self {
        Self {
            next: u64::from(seed) << 32,
        }
    }
}

impl Entropy for Counting {
    fn fill(&mut self, bytes: &mut [u8]) {
        self.next += 1;
        let digest = locust_proto::crypto::content_hash(&self.next.to_le_bytes());
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = digest.0[index % 32] ^ (index / 32) as u8;
        }
    }
}

pub(super) type TestNode = Node<MemStore, Counting>;

/// The owner credential every test daemon is opened with.
pub(super) const OWNER: Credential = Credential([0xA0; 32]);

/// One daemon under test with its store, so it can be restarted.
pub(super) struct Daemon {
    pub node: TestNode,
    pub store: MemStore,
    pub seed: u8,
    next_conn: u64,
    next_request: u64,
}

impl Daemon {
    /// A fresh daemon whose random draws are seeded by `seed`.
    pub fn new(seed: u8) -> Self {
        let store = MemStore::new();
        let node = open(&store, seed);
        Self {
            node,
            store,
            seed,
            next_conn: 0,
            next_request: 0,
        }
    }

    /// Stops the daemon and starts it again over the same store.
    pub fn restart(&mut self) {
        // A restarted daemon must not repeat the random draws of its first
        // life; a real source never does.
        self.seed = self.seed.wrapping_add(100);
        self.node = open(&self.store, self.seed);
    }

    /// Opens a connection and returns the hello's answer.
    pub fn hello(
        &mut self,
        credential: Credential,
        session: Option<SessionSecret>,
    ) -> (ConnId, ServerHello) {
        self.next_conn += 1;
        let conn = ConnId(self.next_conn);
        let hello = ClientHello {
            api_version: API_VERSION,
            credential,
            session,
        };
        (conn, self.node.connect(conn, &hello, 0))
    }

    /// Opens a connection that must be welcomed.
    pub fn connect(&mut self, credential: Credential, session: Option<SessionSecret>) -> ConnId {
        let (conn, answer) = self.hello(credential, session);
        assert!(
            matches!(answer, ServerHello::Welcome { .. }),
            "refused: {answer:?}"
        );
        conn
    }

    pub fn owner(&mut self) -> ConnId {
        self.connect(OWNER, None)
    }

    /// Sends one frame and returns what the engine did with it.
    pub fn step(&mut self, conn: ConnId, frame: RequestFrame, now_ms: u64) -> Step {
        self.node.request(conn, frame, now_ms)
    }

    pub fn frame(&mut self, request: Request) -> RequestFrame {
        self.next_request += 1;
        RequestFrame {
            id: self.next_request,
            idempotency: None,
            on_behalf: None,
            request,
        }
    }

    /// Sends a frame that must be answered at once.
    pub fn send(&mut self, conn: ConnId, frame: RequestFrame) -> Result<Response, ApiError> {
        let id = frame.id;
        let request = frame.request.clone();
        match self.step(conn, frame, 1_000) {
            Step::Reply(reply) => {
                assert_eq!(reply.id, id);
                if let Ok(response) = &reply.result {
                    assert!(
                        request.is_answered_by(response),
                        "{} answered by {response:?}",
                        request.name()
                    );
                }
                reply.result
            }
            Step::Park(parked) => panic!("unexpectedly parked: {parked:?}"),
        }
    }

    /// Makes a request that must be answered at once.
    pub fn call(&mut self, conn: ConnId, request: Request) -> Result<Response, ApiError> {
        let frame = self.frame(request);
        self.send(conn, frame)
    }

    /// Makes a request that must succeed.
    pub fn ok(&mut self, conn: ConnId, request: Request) -> Response {
        let name = request.name();
        self.call(conn, request)
            .unwrap_or_else(|error| panic!("{name} failed: {error}"))
    }

    /// The owner makes a request on behalf of `principal`.
    pub fn on_behalf(
        &mut self,
        owner: ConnId,
        principal: PublicKey,
        request: Request,
    ) -> Result<Response, ApiError> {
        let mut frame = self.frame(request);
        frame.on_behalf = Some(principal);
        self.send(owner, frame)
    }

    /// Makes a request under an idempotency key.
    pub fn keyed(&mut self, conn: ConnId, key: u8, request: Request) -> Result<Response, ApiError> {
        let mut frame = self.frame(request);
        frame.idempotency = Some(IdempotencyKey([key; 16]));
        self.send(conn, frame)
    }

    /// Enrolls a principal named `name` whose credential is derived from
    /// `tag`, and returns its key.
    pub fn enroll(&mut self, name: &str, tag: u8, manage_goals: bool) -> PublicKey {
        let owner = self.owner();
        let response = self.ok(
            owner,
            Request::AgentEnroll {
                name: name.into(),
                grants: Grants { manage_goals },
                credential: credential(tag).digest(),
            },
        );
        self.node.disconnect(owner);
        match response {
            Response::AgentEnrolled { agent } => agent,
            other => panic!("unexpected answer: {other:?}"),
        }
    }
}

fn open(store: &MemStore, seed: u8) -> TestNode {
    Node::open(
        store.reopen(),
        Counting::new(seed),
        OWNER.digest(),
        "test".into(),
        0,
    )
    .expect("a memory store opens")
}

/// The credential tests enroll principals under.
pub(super) fn credential(tag: u8) -> Credential {
    Credential([tag; 32])
}

/// A session secret for tests.
pub(super) fn session(tag: u8) -> SessionSecret {
    SessionSecret([tag; 32])
}

pub(super) fn code<T: std::fmt::Debug>(result: Result<T, ApiError>) -> ErrorCode {
    result.expect_err("the request must be refused").code
}

pub(super) fn caller_of(answer: &ServerHello) -> Option<Caller> {
    match answer {
        ServerHello::Welcome { caller, .. } => Some(*caller),
        ServerHello::Refused { .. } => None,
    }
}

mod lifecycle;

mod failure;

mod content;

mod formal;
