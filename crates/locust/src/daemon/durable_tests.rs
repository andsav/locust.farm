//! The production assembly over real SQLite, Unix sockets and local Iroh.
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use locust_net::{Endpoint, EndpointConfig, IpTransport, Lookup, RelayConfig, TransportBudget};
use locust_proto::api::{
    Credential, ErrorCode, Grants, Request, Response, SessionSecret, TaskState,
};
use locust_proto::client::{Client, ClientError};
use locust_proto::id::{EventId, GoalId, IdempotencyKey, PublicKey};
use locust_proto::invite::Invitation;
use locust_proto::local;

use super::run_networked_with;
use crate::failure::Failure;
use crate::testdir::short_dir;

type LocalClient = Client<UnixStream>;

pub(super) async fn local_endpoint(secret_key: [u8; 32]) -> Result<Endpoint, Failure> {
    Endpoint::bind(EndpointConfig {
        secret_key,
        relays: RelayConfig::Disabled,
        lookup: Lookup::DISABLED,
        ip_transport: IpTransport::Bind("127.0.0.1:0".parse().unwrap()),
        port_mapping: false,
        budget: TransportBudget::default(),
    })
    .await
    .map_err(|error| Failure::unavailable(error.to_string()))
}

pub(super) struct Running {
    pub(super) home: PathBuf,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<JoinHandle<Result<(), Failure>>>,
}
impl Running {
    pub(super) fn start(home: &Path) -> Self {
        let (shutdown, stopped) = tokio::sync::oneshot::channel();
        let (ready, waiting) = mpsc::channel();
        let state = home.to_path_buf();
        let thread = thread::spawn(move || {
            run_networked_with(&state, local_endpoint, move |_| {
                ready.send(()).unwrap();
                Ok(async move {
                    let _ = stopped.await;
                })
            })
        });
        waiting
            .recv_timeout(Duration::from_secs(15))
            .expect("production daemon did not start");
        Self {
            home: home.to_path_buf(),
            shutdown: Some(shutdown),
            thread: Some(thread),
        }
    }
    pub(super) fn client(
        &self,
        credential: Credential,
        session: Option<SessionSecret>,
    ) -> LocalClient {
        let stream = UnixStream::connect(local::socket_path(&self.home).unwrap()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        Client::open(stream, credential, session).unwrap()
    }
    pub(super) fn owner(&self) -> LocalClient {
        let credential =
            Credential(crate::secret::read(&local::owner_credential_path(&self.home)).unwrap());
        self.client(credential, None)
    }
    pub(super) fn enroll(&self, tag: u8) -> PublicKey {
        let response = self
            .owner()
            .call(Request::AgentEnroll {
                name: format!("agent-{tag}"),
                grants: Grants { manage_goals: true },
                credential: Credential([tag; 32]).digest(),
            })
            .unwrap();
        let Response::AgentEnrolled { agent } = response else {
            panic!("{response:?}")
        };
        agent
    }
    fn stop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap().unwrap();
        }
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        self.stop();
    }
}

fn recorded(response: Response) -> EventId {
    let Response::Recorded { event } = response else {
        panic!("{response:?}")
    };
    event
}
fn goal(client: &mut LocalClient) -> GoalId {
    let Response::GoalCreated { goal } = client
        .call(Request::GoalCreate {
            title: "Durable lifecycle".into(),
        })
        .unwrap()
    else {
        panic!()
    };
    goal
}
fn propose(client: &mut LocalClient, goal: GoalId, text: String) -> EventId {
    recorded(
        client
            .call(Request::TaskPropose {
                goal,
                text,
                input: None,
                depends_on: vec![],
                deadline_ms: None,
                max_attempts: None,
            })
            .unwrap(),
    )
}
fn eventually<T>(mut read: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(value) = read() {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "real daemons did not converge before the test deadline"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn unix_sqlite_claims_and_idempotent_events_survive_restart_and_finish() {
    let dir = short_dir();
    let mut running = Running::start(dir.path());
    let principal = running.enroll(1);
    let mut agent = running.client(Credential([1; 32]), Some(SessionSecret([1; 32])));
    let mut owner = running.owner();
    let goal = goal(&mut agent);
    let task = propose(&mut agent, goal, "Complete after a restart".into());
    let assignment = recorded(
        agent
            .call(Request::TaskAssign {
                goal,
                task,
                assignee: principal,
            })
            .unwrap(),
    );
    owner
        .call(Request::TaskAuthorize {
            goal,
            assignment,
            takeover: true,
        })
        .unwrap();
    let Response::Claimed(claim) = agent.call(Request::TaskClaim { goal, assignment }).unwrap()
    else {
        panic!()
    };
    assert_eq!(claim.generation, 1);
    let note_request = Request::NoteAdd {
        goal,
        about: Some(task),
        supersedes: None,
        text: "Retried safely".into(),
    };
    let note = agent
        .call_with(note_request.clone(), Some(IdempotencyKey([1; 16])), None)
        .unwrap();
    let status = owner.call(Request::Status).unwrap();
    drop(agent);
    drop(owner);
    running.stop();
    assert!(!local::socket_path(dir.path()).unwrap().exists());
    let running = Running::start(dir.path());
    let mut owner = running.owner();
    assert_eq!(owner.call(Request::Status).unwrap(), status);
    let mut agent = running.client(Credential([1; 32]), Some(SessionSecret([1; 32])));
    assert_eq!(
        agent
            .call_with(note_request, Some(IdempotencyKey([1; 16])), None)
            .unwrap(),
        note
    );
    assert_eq!(
        agent.call(Request::TaskClaim { goal, assignment }).unwrap(),
        Response::Claimed(claim)
    );
    let result = recorded(
        agent
            .call(Request::TaskSubmit {
                goal,
                assignment,
                generation: 1,
                summary: "Restarted work is complete".into(),
                base: None,
                patch: None,
                artifacts: vec![],
            })
            .unwrap(),
    );
    let Response::Event(detail) = agent
        .call(Request::Event {
            goal,
            event: result,
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(detail.text.as_deref(), Some("Restarted work is complete"));
    agent
        .call(Request::ResultAccept {
            goal,
            result,
            head: None,
        })
        .unwrap();
    let Response::Board(board) = agent.call(Request::Board { goal }).unwrap() else {
        panic!()
    };
    assert_eq!(board[0].state, TaskState::Accepted);
}

#[test]
fn two_real_daemons_join_claim_sync_large_payload_and_accept() {
    let first = short_dir();
    let second = short_dir();
    let coordinator = Running::start(first.path());
    let worker = Running::start(second.path());
    coordinator.enroll(1);
    let worker_key = worker.enroll(2);
    let mut c = coordinator.client(Credential([1; 32]), None);
    let mut w = worker.client(Credential([2; 32]), Some(SessionSecret([2; 32])));
    let goal = goal(&mut c);
    let Response::Invited { ticket } = c
        .call(Request::GoalInvite {
            goal,
            expires_ms: None,
        })
        .unwrap()
    else {
        panic!()
    };
    w.call(Request::GoalJoin { ticket }).unwrap();
    eventually(|| match w.call(Request::GoalStatus { goal }) {
        Ok(Response::GoalStatus(status))
            if status
                .members
                .iter()
                .any(|member| member.member == worker_key) =>
        {
            Some(())
        }
        _ => None,
    });
    let text = "large encrypted task details ".repeat(400);
    let task = propose(&mut c, goal, text.clone());
    let assignment = recorded(
        c.call(Request::TaskAssign {
            goal,
            task,
            assignee: worker_key,
        })
        .unwrap(),
    );
    eventually(|| match w.call(Request::Task { goal, task }) {
        Ok(Response::Task(detail))
            if detail.text.as_ref() == Some(&text)
                && detail.view.assignment == Some(assignment) =>
        {
            Some(())
        }
        _ => None,
    });
    let mut owner = worker.owner();
    owner
        .call(Request::TaskAuthorize {
            goal,
            assignment,
            takeover: false,
        })
        .unwrap();
    w.call(Request::TaskClaim { goal, assignment }).unwrap();
    w.call(Request::TaskProgress {
        goal,
        assignment,
        generation: 1,
        text: "running remotely".into(),
    })
    .unwrap();
    let summary = "large encrypted result evidence ".repeat(350);
    let result = recorded(
        w.call(Request::TaskSubmit {
            goal,
            assignment,
            generation: 1,
            summary: summary.clone(),
            base: None,
            patch: None,
            artifacts: vec![],
        })
        .unwrap(),
    );
    eventually(|| {
        match c.call(Request::Event {
            goal,
            event: result,
        }) {
            Ok(Response::Event(detail)) if detail.text.as_ref() == Some(&summary) => Some(()),
            _ => None,
        }
    });
    let Response::Pending(pending) = c.call(Request::Pending { goal }).unwrap() else {
        panic!()
    };
    assert_eq!(pending.to_review[0].result, result);
    c.call(Request::ResultAccept {
        goal,
        result,
        head: None,
    })
    .unwrap();
    eventually(|| match w.call(Request::Board { goal }) {
        Ok(Response::Board(board)) if board[0].state == TaskState::Accepted => Some(()),
        _ => None,
    });
    assert!(
        matches!(w.call(Request::TaskProgress {goal,assignment,generation:1,text:"late".into()}),Err(ClientError::Api(error)) if error.code == ErrorCode::Superseded)
    );
    let Response::GoalStatus(status) = c.call(Request::GoalStatus { goal }).unwrap() else {
        panic!()
    };
    assert!(
        status
            .peers
            .iter()
            .any(|peer| peer.connected && peer.last_sync_ms.is_some())
    );
}

#[test]
fn peer_decode_and_prefix_failures_deliver_refusals_before_close() {
    use locust_net::FrameLimits;
    use locust_proto::sync::{Refusal, SyncMessage};
    let dir = short_dir();
    let running = Running::start(dir.path());
    running.enroll(1);
    let mut agent = running.client(Credential([1; 32]), None);
    let goal = goal(&mut agent);
    let Response::Invited { ticket } = agent
        .call(Request::GoalInvite {
            goal,
            expires_ms: None,
        })
        .unwrap()
    else {
        panic!()
    };
    let invitation = Invitation::from_ticket(ticket.as_str()).unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let remote = local_endpoint([91; 32]).await.unwrap();
            for (frame, expected) in [
                (
                    SyncMessage::Hello {
                        version: locust_proto::PROTOCOL_VERSION + 1,
                        goal,
                    },
                    Refusal::UnsupportedVersion,
                ),
                (
                    SyncMessage::BlobChunk {
                        hash: locust_proto::id::BlobHash([3; 32]),
                        offset: 0,
                        total: 8192,
                        bytes: vec![0; 8192],
                    },
                    Refusal::LimitExceeded,
                ),
                (
                    SyncMessage::Hello {
                        version: locust_proto::PROTOCOL_VERSION,
                        goal,
                    },
                    Refusal::NotAMember,
                ),
            ] {
                let connection = remote
                    .connect(invitation.endpoint, &invitation.hints)
                    .await
                    .unwrap();
                let mut link = connection.open_link(FrameLimits::peer()).await.unwrap();
                link.send(&frame).await.unwrap();
                if expected == Refusal::NotAMember {
                    link.send(&SyncMessage::Frontier(locust_proto::sync::Frontier {
                        authors: Vec::new(),
                    }))
                    .await
                    .unwrap();
                }
                let reply = tokio::time::timeout(Duration::from_secs(5), link.recv())
                    .await
                    .unwrap_or_else(|_| panic!("timed out waiting for {expected:?}"))
                    .unwrap()
                    .unwrap();
                assert_eq!(reply, SyncMessage::Refused(expected));
                // Rejected pre-admission connections release their transport
                // budget instead of remaining available for more streams.
                tokio::time::timeout(Duration::from_secs(5), connection.closed())
                    .await
                    .unwrap();
            }
            remote.close().await;
        });
    // Refused unauthenticated peers are closed; local service remains healthy.
    agent.call(Request::Status).unwrap();
}
