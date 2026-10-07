//! A test-only stand-in for exercising the local shell.
//!
//! It answers the hello, `status`, `agent.enroll` and `daemon.stop`
//! truthfully and every other operation with `unavailable`, so the daemon
//! command, the socket and the CLI can be exercised end to end. Principals
//! it enrolls live in memory only and are gone after a restart. Delete this
//! module is compiled only for tests; production always uses Node and SQLite.

use std::collections::BTreeMap;

use locust_proto::API_VERSION;
use locust_proto::api::{
    AgentView, ApiError, Caller, ClientHello, DaemonStatus, ErrorCode, Request, RequestFrame,
    Response, ResponseFrame, ServerHello,
};
use locust_proto::crypto::Keypair;
use locust_proto::engine::{ConnId, Engine, Entropy, Parked, Step};
use locust_proto::id::{GoalId, PublicKey};
use locust_proto::limits::MAX_BLOB_BYTES;

use super::EngineInit;

struct Principal {
    name: String,
    /// Digest of the credential it was enrolled under.
    credential: [u8; 32],
    key: PublicKey,
    author_only: bool,
}

pub(crate) struct MinimalEngine {
    daemon_version: String,
    /// Digest of the owner's credential.
    owner: [u8; 32],
    entropy: Box<dyn Entropy>,
    principals: Vec<Principal>,
    callers: BTreeMap<ConnId, Caller>,
    stop: bool,
}

impl MinimalEngine {
    pub(crate) fn new(init: EngineInit) -> Self {
        Self {
            daemon_version: init.daemon_version,
            owner: init.owner_digest,
            entropy: init.entropy,
            principals: Vec::new(),
            callers: BTreeMap::new(),
            stop: false,
        }
    }

    fn answer(&mut self, caller: Caller, frame: RequestFrame) -> Result<Response, ApiError> {
        frame.request.check()?;
        let owner_only = |what: &str| {
            if caller != Caller::Owner {
                Err(ApiError::new(
                    ErrorCode::Denied,
                    format!("{what} is for the owner"),
                ))
            } else if frame.on_behalf.is_some() {
                Err(ApiError::new(
                    ErrorCode::Invalid,
                    format!("{what} cannot be made on behalf of a principal"),
                ))
            } else {
                Ok(())
            }
        };
        match frame.request {
            Request::Status => Ok(Response::Status(self.status(caller))),
            Request::Shutdown => {
                owner_only("daemon.stop")?;
                self.stop = true;
                Ok(Response::Done)
            }
            Request::AgentEnroll { name, credential } => {
                owner_only("agent.enroll")?;
                self.enroll(name, false, credential)
                    .map(|agent| Response::AgentEnrolled { agent })
            }
            other => Err(ApiError::new(
                ErrorCode::Unavailable,
                format!(
                    "{}: the state machine is not linked into this build",
                    other.name()
                ),
            )),
        }
    }

    fn status(&self, caller: Caller) -> DaemonStatus {
        let visible = |principal: &&Principal| match caller {
            Caller::Owner => true,
            Caller::Agent(key) | Caller::Author(key) => principal.key == key,
        };
        DaemonStatus {
            daemon_version: self.daemon_version.clone(),
            endpoint: None,
            waiting: Vec::new(),
            agents: self
                .principals
                .iter()
                .filter(visible)
                .map(|principal| AgentView {
                    agent: principal.key,
                    name: principal.name.clone(),
                    author_only: principal.author_only,
                    revoked: false,
                })
                .collect(),
            goals: Vec::new(),
        }
    }

    fn enroll(
        &mut self,
        name: String,
        author_only: bool,
        credential: [u8; 32],
    ) -> Result<PublicKey, ApiError> {
        if let Some(existing) = self.principals.iter().find(|p| p.name == name) {
            return if existing.credential == credential {
                Ok(existing.key)
            } else {
                Err(ApiError::new(
                    ErrorCode::Conflict,
                    format!("the name {name} is taken by another credential"),
                ))
            };
        }
        let in_use =
            credential == self.owner || self.principals.iter().any(|p| p.credential == credential);
        if in_use {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "the credential is already enrolled",
            ));
        }
        let mut seed = [0u8; 32];
        self.entropy.fill(&mut seed);
        let key = Keypair::from_seed(seed).public();
        self.principals.push(Principal {
            name,
            credential,
            key,
            author_only,
        });
        Ok(key)
    }
}

impl Engine for MinimalEngine {
    fn connect(&mut self, conn: ConnId, hello: &ClientHello, _now_ms: u64) -> ServerHello {
        let refuse = |code, message: &str| ServerHello::Refused {
            error: ApiError::new(code, message),
            api_version: API_VERSION,
            daemon_version: self.daemon_version.clone(),
        };
        if hello.api_version != API_VERSION {
            return refuse(
                ErrorCode::UnsupportedVersion,
                "the client speaks another API version",
            );
        }
        let digest = hello.credential.digest();
        let caller = if digest == self.owner {
            Caller::Owner
        } else if let Some(principal) = self.principals.iter().find(|p| p.credential == digest) {
            Caller::Agent(principal.key)
        } else {
            return refuse(ErrorCode::Denied, "the credential is not known");
        };
        self.callers.insert(conn, caller);
        ServerHello::Welcome {
            api_version: API_VERSION,
            daemon_version: self.daemon_version.clone(),
            caller,
            max_blob_bytes: MAX_BLOB_BYTES as u64,
        }
    }

    fn request(&mut self, conn: ConnId, frame: RequestFrame, _now_ms: u64) -> Step {
        let id = frame.id;
        let result = match self.callers.get(&conn) {
            Some(caller) => self.answer(*caller, frame),
            None => Err(ApiError::new(
                ErrorCode::Internal,
                "request on a connection that was not welcomed",
            )),
        };
        Step::Reply(ResponseFrame { id, result })
    }

    fn resume(&mut self, _conn: ConnId, parked: &Parked, _timed_out: bool, _now_ms: u64) -> Step {
        // This engine never parks a request.
        Step::Reply(ResponseFrame {
            id: parked.request_id,
            result: Err(ApiError::new(ErrorCode::Internal, "no request was parked")),
        })
    }

    fn take_changed(&mut self) -> Vec<GoalId> {
        Vec::new()
    }

    fn disconnect(&mut self, conn: ConnId) {
        self.callers.remove(&conn);
    }

    fn stop_requested(&self) -> bool {
        self.stop
    }
}

#[cfg(test)]
mod tests {
    use locust_proto::api::{Credential, Level};

    use super::*;

    const OWNER: Credential = Credential([1; 32]);
    const WORKER: Credential = Credential([2; 32]);

    /// Counts up, so every key it seeds differs.
    struct Counter(u8);

    impl Entropy for Counter {
        fn fill(&mut self, bytes: &mut [u8]) {
            self.0 += 1;
            bytes.fill(self.0);
        }
    }

    fn engine() -> MinimalEngine {
        MinimalEngine::new(EngineInit {
            home: "/h".into(),
            owner_digest: OWNER.digest(),
            daemon_version: "0.1.0 (test)".to_string(),
            entropy: Box::new(Counter(0)),
        })
    }

    fn hello(credential: Credential) -> ClientHello {
        ClientHello {
            api_version: API_VERSION,
            credential,
            session: None,
        }
    }

    fn caller(engine: &mut MinimalEngine, conn: u64, credential: Credential) -> Option<Caller> {
        match engine.connect(ConnId(conn), &hello(credential), 0) {
            ServerHello::Welcome { caller, .. } => Some(caller),
            ServerHello::Refused { error, .. } => {
                assert_eq!(error.code, ErrorCode::Denied);
                None
            }
        }
    }

    fn ask(engine: &mut MinimalEngine, conn: u64, request: Request) -> Result<Response, ApiError> {
        let frame = RequestFrame {
            id: 9,
            idempotency: None,
            on_behalf: None,
            request,
        };
        match engine.request(ConnId(conn), frame, 0) {
            Step::Reply(ResponseFrame { id: 9, result }) => result,
            other => panic!("unexpected step {other:?}"),
        }
    }

    fn enroll(name: &str, credential: Credential) -> Request {
        Request::AgentEnroll {
            name: name.to_string(),
            credential: credential.digest(),
        }
    }

    #[test]
    fn the_owner_is_welcomed_and_an_unknown_credential_is_refused() {
        let mut engine = engine();
        assert_eq!(caller(&mut engine, 1, OWNER), Some(Caller::Owner));
        assert_eq!(caller(&mut engine, 2, WORKER), None);
        let ServerHello::Welcome {
            api_version,
            daemon_version,
            max_blob_bytes,
            ..
        } = engine.connect(ConnId(3), &hello(OWNER), 0)
        else {
            panic!("the owner was refused");
        };
        assert_eq!(api_version, API_VERSION);
        assert_eq!(daemon_version, "0.1.0 (test)");
        assert_eq!(max_blob_bytes, MAX_BLOB_BYTES as u64);
    }

    #[test]
    fn an_enrolled_principal_is_welcomed_and_sees_only_itself() {
        let mut engine = engine();
        caller(&mut engine, 1, OWNER);
        let Ok(Response::AgentEnrolled { agent }) = ask(&mut engine, 1, enroll("worker", WORKER))
        else {
            panic!("enrollment failed");
        };
        ask(&mut engine, 1, enroll("other", Credential([3; 32]))).unwrap();

        assert_eq!(caller(&mut engine, 2, WORKER), Some(Caller::Agent(agent)));
        let Ok(Response::Status(own)) = ask(&mut engine, 2, Request::Status) else {
            panic!("status failed");
        };
        assert_eq!(own.agents.len(), 1);
        assert_eq!(
            (own.agents[0].agent, own.agents[0].name.as_str()),
            (agent, "worker")
        );
        assert!(!own.agents[0].author_only);
        assert_eq!((own.endpoint, own.goals.len()), (None, 0));

        let Ok(Response::Status(all)) = ask(&mut engine, 1, Request::Status) else {
            panic!("status failed");
        };
        assert_eq!(all.agents.len(), 2);
        assert_ne!(all.agents[0].agent, all.agents[1].agent);
    }

    #[test]
    fn enrolling_again_returns_the_same_principal_and_a_taken_name_conflicts() {
        let mut engine = engine();
        caller(&mut engine, 1, OWNER);
        let first = ask(&mut engine, 1, enroll("worker", WORKER)).unwrap();
        assert_eq!(
            ask(&mut engine, 1, enroll("worker", WORKER)).unwrap(),
            first
        );
        let taken = ask(&mut engine, 1, enroll("worker", Credential([3; 32]))).unwrap_err();
        assert_eq!(taken.code, ErrorCode::Conflict);
        let reused = ask(&mut engine, 1, enroll("second", WORKER)).unwrap_err();
        assert_eq!(reused.code, ErrorCode::Conflict);
        let owner = ask(&mut engine, 1, enroll("third", OWNER)).unwrap_err();
        assert_eq!(owner.code, ErrorCode::Conflict);
        let bad_name = ask(&mut engine, 1, enroll("Worker", Credential([4; 32]))).unwrap_err();
        assert_eq!(bad_name.code, ErrorCode::Invalid);
    }

    #[test]
    fn owner_requests_are_denied_to_a_principal() {
        let mut engine = engine();
        caller(&mut engine, 1, OWNER);
        ask(&mut engine, 1, enroll("worker", WORKER)).unwrap();
        caller(&mut engine, 2, WORKER);
        let denied = ask(&mut engine, 2, enroll("other", Credential([3; 32]))).unwrap_err();
        assert_eq!(denied.code, ErrorCode::Denied);
        let denied = ask(&mut engine, 2, Request::Shutdown).unwrap_err();
        assert_eq!(denied.code, ErrorCode::Denied);
        assert!(!engine.stop_requested());

        assert_eq!(ask(&mut engine, 1, Request::Shutdown), Ok(Response::Done));
        assert!(engine.stop_requested());
    }

    #[test]
    fn every_other_operation_says_the_state_machine_is_not_linked() {
        let mut engine = engine();
        caller(&mut engine, 1, OWNER);
        let goal = GoalId([7; 32]);
        for request in [
            Request::GoalCreate {
                name: "Host".into(),

                agent: PublicKey([1; 32]),
                title: "Ship it".to_string(),
                formation_json: None,
                inputs: Default::default(),
            },
            Request::Board { goal },
            Request::Wait {
                goal,
                seen: 0,
                timeout_ms: 1000,
            },
            Request::LevelSet {
                goal,
                agent: PublicKey([2; 32]),
                level: Level::Read,
            },
        ] {
            let name = request.name();
            let error = ask(&mut engine, 1, request).unwrap_err();
            assert_eq!(error.code, ErrorCode::Unavailable);
            assert_eq!(
                error.message,
                format!("{name}: the state machine is not linked into this build")
            );
        }
        assert!(engine.take_changed().is_empty());
    }

    #[test]
    fn a_disconnected_connection_is_forgotten() {
        let mut engine = engine();
        caller(&mut engine, 1, OWNER);
        engine.disconnect(ConnId(1));
        let error = ask(&mut engine, 1, Request::Status).unwrap_err();
        assert_eq!(error.code, ErrorCode::Internal);
    }
}
