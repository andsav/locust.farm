//! One simulated computer: the real node over a reopenable store, with the
//! things the daemon's shell and the user's files would hold around it.

use locust_proto::API_VERSION;
use locust_proto::api::{
    ApiError, ClientHello, Credential, ErrorCode, Request, RequestFrame, Response, ServerHello,
    SessionSecret,
};
use locust_proto::crypto::Keypair;
use locust_proto::engine::{ConnId, Engine, PeerEngine, Step};
use locust_proto::id::{EndpointId, PublicKey};
use locust_proto::store::{MemStore, Store};

use super::rng::{Rng, SimEntropy};
use crate::node::Node;

pub type SimNode = Node<MemStore, SimEntropy>;

/// What the computer is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    /// The daemon process runs.
    Running,
    /// No daemon process: nothing is delivered, exchanges to it fail to open.
    Stopped,
    /// The process exists but does not run while simulated time passes.
    Asleep,
}

/// Which secret files a local request presents, as the CLI would.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    /// `--owner`: the owner credential.
    Owner,
    /// `LOCUST_CREDENTIAL`: the enrolled principal, no session.
    Agent,
    /// The principal with `LOCUST_SESSION`: an execution session. The number
    /// selects one of the machine's session secret files.
    Session(usize),
}

pub struct Machine {
    pub name: String,
    /// The handle that outlives the process, standing for the disk.
    pub store: MemStore,
    /// The daemon process; `None` while stopped.
    pub node: Option<SimNode>,
    pub power: Power,
    /// How many times the process was started. Shell numbering restarts
    /// with it, and inputs addressed to an earlier start are dropped.
    pub boot: u32,
    /// This computer's wall clock minus simulated true time.
    pub clock_offset_ms: i64,
    /// The scenario itself keeps this machine stopped or asleep; fault
    /// injection leaves it alone until released.
    pub held: bool,
    pub owner: Credential,
    pub agent: Credential,
    pub sessions: [SessionSecret; 2],
    pub principal: Option<PublicKey>,
    pub endpoint: EndpointId,
    /// Accepted exchanges, numbered by the shell from 1 at each start.
    pub next_accepted: u64,
    /// When the shell's one pending poll fires, in simulated microseconds.
    pub next_poll: u64,
    /// What local callers could read when the process last stopped.
    pub last_view: Option<String>,
    /// Whether `last_view` shows each goal's revision. Not while a goal
    /// still holds its restore record: the start that forgets it touches
    /// the goal.
    pub last_revisions: bool,
    next_conn: u64,
    next_request: u64,
    entropy: Rng,
}

impl Machine {
    pub fn new(index: usize, rng: &Rng) -> Self {
        let mut secrets = rng.fork(0x5EC0 + index as u64);
        Self {
            name: format!("m{}", index + 1),
            store: MemStore::new(),
            node: None,
            power: Power::Stopped,
            boot: 0,
            clock_offset_ms: 0,
            held: false,
            owner: Credential(secrets.bytes()),
            agent: Credential(secrets.bytes()),
            sessions: [
                SessionSecret(secrets.bytes()),
                SessionSecret(secrets.bytes()),
            ],
            principal: None,
            endpoint: EndpointId([0; 32]),
            next_accepted: 0,
            next_poll: 0,
            last_view: None,
            last_revisions: true,
            next_conn: 0,
            next_request: 0,
            entropy: rng.fork(0xE270 + index as u64),
        }
    }

    pub fn running(&self) -> bool {
        self.power == Power::Running
    }

    /// Starts the daemon process over the machine's store: `Node::open` over
    /// `MemStore::reopen`, with the same owner credential. The endpoint
    /// identity is derived from the secret the node keeps in its store, as
    /// the transport derives it.
    pub fn start(&mut self, now_ms: u64) {
        assert!(self.node.is_none(), "{} is already running", self.name);
        self.boot += 1;
        let entropy = SimEntropy(self.entropy.fork(u64::from(self.boot)));
        let node = Node::open(
            self.store.reopen(),
            entropy,
            self.owner.digest(),
            "sim".into(),
            now_ms,
        )
        .unwrap_or_else(|error| panic!("{} cannot open its store: {error}", self.name));
        let endpoint = EndpointId(Keypair::from_seed(node.endpoint_secret()).public().0);
        if self.boot > 1 {
            assert_eq!(endpoint, self.endpoint, "{} changed identity", self.name);
        }
        self.endpoint = endpoint;
        self.node = Some(node);
        self.power = Power::Running;
        self.next_accepted = 0;
    }

    /// Ends the daemon process. Everything it held in memory is gone.
    pub fn stop(&mut self) {
        self.node = None;
        self.power = Power::Stopped;
    }

    /// One CLI command: a connection, one request, its answer, and the end
    /// of the connection. A request that parks is a simulator bug, because
    /// the scenarios never send `wait`.
    pub fn call(&mut self, who: Who, request: Request, now_ms: u64) -> Result<Response, ApiError> {
        let (credential, session) = match who {
            Who::Owner => (self.owner, None),
            Who::Agent => (self.agent, None),
            Who::Session(n) => (self.agent, Some(self.sessions[n])),
        };
        let node = self.node.as_mut().filter(|_| self.power == Power::Running);
        let Some(node) = node else {
            return Err(ApiError::new(
                ErrorCode::Unavailable,
                "sim: the daemon is not running",
            ));
        };
        self.next_conn += 1;
        self.next_request += 1;
        let conn = ConnId(self.next_conn);
        let hello = ClientHello {
            api_version: API_VERSION,
            credential,
            session,
        };
        if let ServerHello::Refused { error, .. } = node.connect(conn, &hello, now_ms) {
            return Err(error);
        }
        let frame = RequestFrame {
            id: self.next_request,
            idempotency: None,
            on_behalf: None,
            request: request.clone(),
        };
        let step = node.request(conn, frame, now_ms);
        node.take_changed();
        node.disconnect(conn);
        match step {
            Step::Reply(reply) => {
                assert_eq!(reply.id, self.next_request, "the answer names its request");
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

    /// Everything the owner and the principal can read, as text: status,
    /// and for every goal its status, board, findings and pending work. Which
    /// peers are connected, which computers the restore guard has heard
    /// from, whether the data was restored and how many goals the copy
    /// lost are left out, because only those may differ after a restart:
    /// the first ordinary start with nothing held forgets the restore. So is
    /// a goal waiting for the owner to continue it, which follows from whom
    /// the guard heard; the holds themselves are compared.
    /// `revisions: false` also leaves out the revision of pending work.
    /// `None` before the principal is enrolled.
    pub fn visible(&mut self, now_ms: u64, revisions: bool) -> Option<String> {
        self.principal?;
        let Ok(Response::Status(mut status)) = self.call(Who::Owner, Request::Status, now_ms)
        else {
            return None;
        };
        let unheard = |views: &mut Vec<locust_proto::api::GuardView>| {
            for view in views {
                view.heard.clear();
                view.waiting.clear();
            }
        };
        for summary in &mut status.goals {
            unheard(&mut summary.guard);
            summary.restored = None;
        }
        status
            .waiting
            .retain(|item| !matches!(item.kind, locust_proto::api::WaitingKind::CatchingUp { .. }));
        status.lost_goals = 0;
        let mut text = format!("{status:?}");
        for goal in status.goals.iter().map(|summary| summary.goal) {
            let reads = [
                Request::GoalStatus { goal },
                Request::Board { goal },
                Request::Contributions { goal, task: None },
                Request::Pending { goal },
            ];
            for request in reads {
                let mut answer = self.call(Who::Agent, request, now_ms);
                if let Ok(Response::GoalStatus(status)) = &mut answer {
                    status.peers.clear();
                    unheard(&mut status.guard);
                    status.restored = None;
                }
                if let Ok(Response::Pending(work)) = &mut answer
                    && !revisions
                {
                    work.revision = 0;
                }
                text.push_str(&format!("\n{answer:?}"));
            }
        }
        Some(text)
    }

    /// Whether a goal still holds its restore record, which the next
    /// ordinary start with nothing held deletes.
    pub fn restore_remembered(&self) -> bool {
        self.node.as_ref().is_some_and(|node| {
            node.goals
                .values()
                .any(|entry| entry.local.restored.is_some())
        })
    }

    /// How many events the store holds, over every goal.
    pub fn event_count(&self) -> usize {
        let goals = self.store.goals().expect("a memory store reads");
        goals
            .iter()
            .map(|goal| self.store.log(goal, 0, usize::MAX).expect("log").len())
            .sum()
    }
}
