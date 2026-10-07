//! The production assembly over real SQLite, Unix sockets and local Iroh.
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use locust_net::{Endpoint, EndpointConfig, IpTransport, Lookup, RelayConfig, TransportBudget};
use locust_proto::api::{
    ApiError, ClientHello, Credential, ErrorCode, GoalStatus, GuardReason, GuardView, Halt, Level,
    Request, RequestFrame, Response, ServerHello, SessionSecret,
};
use locust_proto::client::{Client, ClientError};
use locust_proto::engine::{
    ConnId, Engine, ExchangeId, Parked, PeerEngine, PeerInput, PeerOutput, PeerTime, Step,
};
use locust_proto::event::{AttemptStatus, Header, ReviewVerdict, TaskId};
use locust_proto::farm::{FarmUpload, FarmUploadResult};
use locust_proto::id::{EndpointId, EventId, GoalId, IdempotencyKey, PublicKey};
use locust_proto::invite::Invitation;
use locust_proto::local;
use locust_proto::store::{FileId, Store};
use locust_store::SqliteStore;

use super::{ProductionNode, run_networked_with};
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
        Self::start_with_marks(home, &local::marks_dir(home))
    }
    /// A daemon on `home` whose marks directory is `marks`. Given the marks
    /// of the directory `home` was copied from, it is that daemon with its
    /// data directory put back from the copy. A copied marks file reads as
    /// lost, so the marks are handed over, not copied.
    pub(super) fn start_with_marks(home: &Path, marks: &Path) -> Self {
        Self::start_observed(home, marks, std::convert::identity)
    }
    /// The same daemon, with its node handed to `observe` on the engine thread.
    pub(super) fn start_observed<E, O>(home: &Path, marks: &Path, observe: O) -> Self
    where
        E: Engine + PeerEngine + 'static,
        O: FnOnce(ProductionNode) -> E + Send + 'static,
    {
        let (shutdown, stopped) = tokio::sync::oneshot::channel();
        let (ready, waiting) = mpsc::channel();
        let state = home.to_path_buf();
        let marks = marks.to_path_buf();
        let thread = thread::spawn(move || {
            run_networked_with(&state, &marks, observe, local_endpoint, move |_| {
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
                credential: Credential([tag; 32]).digest(),
            })
            .unwrap();
        let Response::AgentEnrolled { agent } = response else {
            panic!("{response:?}")
        };
        agent
    }
    pub(super) fn stop(&mut self) {
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

pub(super) fn recorded(response: Response) -> EventId {
    let Response::Recorded { event } = response else {
        panic!("{response:?}")
    };
    event
}
fn goal(client: &mut LocalClient, agent: PublicKey) -> GoalId {
    let Response::GoalCreated { goal } = client
        .call(Request::GoalCreate {
            name: "Host".into(),

            agent,
            title: "Durable lifecycle".into(),
            formation_json: Some(
                serde_json::to_string(
                    &locust_proto::organization::presets()
                        .into_iter()
                        .find(|p| p.name == "directed")
                        .unwrap()
                        .formation,
                )
                .unwrap(),
            ),
            inputs: Default::default(),
        })
        .unwrap()
    else {
        panic!()
    };
    goal
}
fn propose(client: &mut LocalClient, goal: GoalId, text: String) -> TaskId {
    TaskId::Authored(recorded(
        client
            .call(Request::TaskOpen {
                goal,
                text,
                task_type: None,
                inputs: Default::default(),
                parent: None,
            })
            .unwrap(),
    ))
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

/// Preserve the latest observation and the invitation phase on a watchdog
/// failure. These reads contain membership and peer state, never a ticket.
#[track_caller]
pub(super) fn eventually_observed<R: std::fmt::Debug, T>(
    phase: &str,
    mut read: impl FnMut() -> R,
    mut ready: impl FnMut(&R) -> Option<T>,
) -> T {
    let start = Instant::now();
    let deadline = start + Duration::from_secs(20);
    loop {
        let observed = read();
        if let Some(value) = ready(&observed) {
            return value;
        }
        assert!(
            Instant::now() < deadline,
            "{phase} did not converge after {:?}; latest observation: {observed:?}",
            start.elapsed()
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
    let goal = goal(&mut owner, principal);
    let task = propose(&mut agent, goal, "Complete after a restart".into());
    let assignment = recorded(
        agent
            .call(Request::WorkOffer {
                goal,
                task,
                recipient: principal,
            })
            .unwrap(),
    );
    owner
        .call(Request::TaskAllow {
            goal,
            task,
            agent: principal,
        })
        .unwrap();
    let Response::Claimed(claim) = agent
        .call(Request::AttemptStart {
            goal,
            task,
            offer: Some(assignment),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(claim.generation, 1);
    let note_request = Request::ContributionPublish {
        goal,
        attempt: None,
        generation: None,
        sources: Vec::new(),
        artifacts: vec![],
        summary: "Retried safely".into(),
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
        agent
            .call(Request::AttemptStart {
                goal,
                task,
                offer: Some(assignment)
            })
            .unwrap(),
        Response::Claimed(claim)
    );
    let result = recorded(
        agent
            .call(Request::ContributionPublish {
                goal,
                attempt: Some(claim.attempt),
                generation: Some(1),
                summary: "Restarted work is complete".into(),
                sources: Vec::new(),
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
        .call(Request::ReviewRecord {
            goal,
            subject: result,
            verdict: ReviewVerdict::Approve,
            text: String::new(),
        })
        .unwrap();
    let Response::Board(board) = agent.call(Request::Board { goal }).unwrap() else {
        panic!()
    };
    assert!(board[0].completed);
    agent
        .call(Request::ScopeSelect {
            goal,
            subject: result,
            expected: None,
        })
        .unwrap();
}

#[test]
fn reviewed_invitation_joins_two_real_daemons_at_read_level() {
    use locust_proto::api::{ErrorCode, InvitationState, Membership};
    let first = short_dir();
    let second = short_dir();
    let issuer = Running::start(first.path());
    let mut recipient = Running::start(second.path());
    let host_agent = issuer.enroll(1);
    let principal = recipient.enroll(2);
    let mut host_owner = issuer.owner();
    let mut owner = recipient.owner();
    let goal = goal(&mut host_owner, host_agent);
    let Response::Invited { ticket: revoked } = host_owner
        .call(Request::GoalInvite {
            role: None,

            goal,
            expires_ms: u64::MAX,
        })
        .unwrap()
    else {
        panic!()
    };
    let Response::Invitations { invitations } = issuer
        .owner()
        .call(Request::GoalInvitations { goal })
        .unwrap()
    else {
        panic!()
    };
    issuer
        .owner()
        .call(Request::InvitationRevoke {
            goal,
            invitation: Some(invitations[0].invitation.clone()),
        })
        .unwrap();
    let refused_join = Request::GoalJoin {
        name: "Member".into(),

        agent: principal,
        ticket: revoked,
        level: Level::Read,
    };
    assert!(matches!(
        owner.call(refused_join.clone()).unwrap(),
        Response::Joined {
            membership: Membership::Joining,
            ..
        }
    ));
    eventually_observed(
        "revoked invitation refusal",
        || owner.call(Request::Status),
        |observed| match observed {
            Ok(Response::Status(status))
                if status
                    .goals
                    .iter()
                    .any(|entry| entry.goal == goal && entry.membership == Membership::Refused) =>
            {
                Some(())
            }
            _ => None,
        },
    );
    assert!(
        matches!(owner.call(refused_join), Err(locust_proto::client::ClientError::Api(error)) if error.code == ErrorCode::Denied)
    );
    assert!(
        matches!(recipient.client(Credential([2; 32]), None).call(Request::GoalStatus { goal }), Err(locust_proto::client::ClientError::Api(error)) if error.code == ErrorCode::Denied)
    );
    let Response::Invited { ticket } = host_owner
        .call(Request::GoalInvite {
            role: None,

            goal,
            expires_ms: u64::MAX,
        })
        .unwrap()
    else {
        panic!()
    };
    let Response::InvitationInspected { preview } = owner
        .call(Request::InvitationInspect {
            ticket: ticket.clone(),
        })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(preview.goal_title.as_deref(), Some("Durable lifecycle"));
    let join = Request::GoalJoin {
        name: "Member".into(),

        agent: principal,
        ticket,
        level: Level::Read,
    };
    assert!(matches!(
        owner.call(join.clone()).unwrap(),
        Response::Joined {
            membership: Membership::Joining,
            ..
        }
    ));
    let mut member = recipient.client(Credential([2; 32]), None);
    eventually_observed(
        "replacement invitation admission",
        || member.call(Request::GoalStatus { goal }),
        |observed| match observed {
            Ok(Response::GoalStatus(status))
                if status.members.iter().any(|entry| entry.member == principal) =>
            {
                assert_eq!(status.abilities[0].level, Level::Read);
                let workspace = status.workspace.as_ref().unwrap();
                assert!(!workspace.enabled);
                assert!(workspace.head.is_none());
                assert_eq!(
                    workspace.authority,
                    locust_proto::api::WorkspaceAuthority::Uninitialized
                );
                Some(())
            }
            _ => None,
        },
    );
    assert!(matches!(
        owner.call(join.clone()).unwrap(),
        Response::Joined {
            membership: Membership::Member,
            ..
        }
    ));
    let Response::Invitations { invitations } = issuer
        .owner()
        .call(Request::GoalInvitations { goal })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(invitations.len(), 2);
    assert!(
        invitations
            .iter()
            .any(|invitation| invitation.state == InvitationState::Revoked)
    );
    let redeemed = invitations
        .iter()
        .find(|invitation| invitation.state == InvitationState::Redeemed)
        .unwrap();
    assert_eq!(redeemed.redeemed_by, Some(principal));
    drop(member);
    drop(owner);
    recipient.stop();
    let recipient = Running::start(second.path());
    assert!(matches!(
        recipient.owner().call(join).unwrap(),
        Response::Joined {
            membership: Membership::Member,
            ..
        }
    ));
    let Response::GoalStatus(status) = recipient
        .client(Credential([2; 32]), None)
        .call(Request::GoalStatus { goal })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(status.abilities[0].level, Level::Read);
    let workspace = status.workspace.as_ref().unwrap();
    assert!(!workspace.enabled);
    assert!(workspace.head.is_none());
    assert_eq!(
        workspace.authority,
        locust_proto::api::WorkspaceAuthority::Uninitialized
    );
}

#[test]
fn two_real_daemons_join_claim_sync_large_payload_and_accept() {
    let first = short_dir();
    let second = short_dir();
    let coordinator = Running::start(first.path());
    let worker = Running::start(second.path());
    let coordinator_key = coordinator.enroll(1);
    let worker_key = worker.enroll(2);
    let mut c = coordinator.client(Credential([1; 32]), None);
    let mut w = worker.client(Credential([2; 32]), Some(SessionSecret([2; 32])));
    let goal = goal(&mut coordinator.owner(), coordinator_key);
    let Response::Invited { ticket } = coordinator
        .owner()
        .call(Request::GoalInvite {
            role: None,

            goal,
            expires_ms: u64::MAX,
        })
        .unwrap()
    else {
        panic!()
    };
    worker
        .owner()
        .call(Request::GoalJoin {
            name: "Member".into(),

            agent: worker_key,
            ticket,
            level: Level::Auto,
        })
        .unwrap();
    // Admitted, and heard from the host's computer since: until then the
    // worker's key is held, and exchanges that bring records do not count.
    eventually(|| match w.call(Request::GoalStatus { goal }) {
        Ok(Response::GoalStatus(status))
            if status
                .members
                .iter()
                .any(|member| member.member == worker_key)
                && status.guard.is_empty() =>
        {
            Some(())
        }
        _ => None,
    });
    let text = "large encrypted task details ".repeat(400);
    let task = propose(&mut c, goal, text.clone());
    let assignment = recorded(
        c.call(Request::WorkOffer {
            goal,
            task,
            recipient: worker_key,
        })
        .unwrap(),
    );
    eventually(|| match w.call(Request::Task { goal, task }) {
        Ok(Response::Task(detail))
            if detail.text.as_ref() == Some(&text) && detail.view.task == task =>
        {
            Some(())
        }
        _ => None,
    });
    let mut owner = worker.owner();
    owner
        .call(Request::TaskAllow {
            goal,
            task,
            agent: worker_key,
        })
        .unwrap();
    // Task text and the later work offer replicate independently. Seeing the
    // task does not yet prove that this exact offered attempt is eligible.
    eventually_observed(
        "worker offer readiness",
        || w.call(Request::Pending { goal }),
        |observed| match observed {
            Ok(Response::Pending(work))
                if work
                    .to_start
                    .iter()
                    .any(|item| item.task == task && item.offer == Some(assignment)) =>
            {
                Some(())
            }
            _ => None,
        },
    );
    let Response::Claimed(claim) = w
        .call(Request::AttemptStart {
            goal,
            task,
            offer: Some(assignment),
        })
        .unwrap()
    else {
        panic!()
    };
    w.call(Request::AttemptReport {
        goal,
        attempt: claim.attempt,
        status: AttemptStatus::Progress,
        generation: 1,
        text: "running remotely".into(),
    })
    .unwrap();
    let summary = "large encrypted result evidence ".repeat(350);
    let result = recorded(
        w.call(Request::ContributionPublish {
            goal,
            attempt: Some(claim.attempt),
            generation: Some(1),
            summary: summary.clone(),
            sources: Vec::new(),
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
    assert_eq!(pending.to_review[0].subject, result);
    c.call(Request::ReviewRecord {
        goal,
        subject: result,
        verdict: ReviewVerdict::Approve,
        text: String::new(),
    })
    .unwrap();
    eventually(|| match w.call(Request::Board { goal }) {
        Ok(Response::Board(board)) if board[0].completed => Some(()),
        _ => None,
    });
    c.call(Request::ScopeSelect {
        goal,
        subject: result,
        expected: None,
    })
    .unwrap();
    eventually(|| match w.call(Request::Board { goal }) {
        Ok(Response::Board(board)) if board[0].selected == Some(result) => Some(()),
        _ => None,
    });
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
    let host_agent = running.enroll(1);
    let mut agent = running.client(Credential([1; 32]), None);
    let goal = goal(&mut running.owner(), host_agent);
    let Response::Invited { ticket } = running
        .owner()
        .call(Request::GoalInvite {
            role: None,

            goal,
            expires_ms: u64::MAX,
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

#[test]
fn refused_inbound_exchange_does_not_cut_this_daemons_own_join() {
    use locust_net::FrameLimits;
    use locust_proto::invite::InviteSecret;
    use locust_proto::sync::{Frontier, Refusal, SyncMessage};
    let dir = short_dir();
    let running = Running::start(dir.path());
    let principal = running.enroll(1);
    let mut agent = running.client(Credential([1; 32]), None);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let inviter = local_endpoint([93; 32]).await.unwrap();
            let goal = GoalId([94; 32]);
            let ticket = Invitation::signed(
                goal,
                Some("Join recovery".into()),
                inviter.id(),
                inviter.hints(),
                InviteSecret([96; 32]),
                None,
                "Host".into(),
                None,
                &locust_proto::crypto::Keypair::from_seed([95; 32]),
            )
            .unwrap()
            .to_ticket()
            .unwrap();
            running
                .owner()
                .call(Request::GoalJoin {
                    name: "Member".into(),

                    agent: principal,
                    ticket,
                    level: Level::Read,
                })
                .unwrap();
            let within = |seconds| Duration::from_secs(seconds);
            let connection = tokio::time::timeout(within(10), async {
                inviter.accept().await.unwrap().accept().await.unwrap()
            })
            .await
            .expect("the joining daemon did not dial its inviter");
            let mut join = connection.accept_link(FrameLimits::peer()).await.unwrap();
            let hello = SyncMessage::Hello {
                version: locust_proto::PROTOCOL_VERSION,
                goal,
            };
            assert_eq!(join.recv().await.unwrap(), Some(hello.clone()));
            assert!(matches!(
                join.recv().await.unwrap(),
                Some(SyncMessage::Join(_))
            ));
            // The inviter dials the joiner on the same connection before the
            // joiner holds the goal, and is refused.
            let mut link = connection.open_link(FrameLimits::peer()).await.unwrap();
            link.send(&hello).await.unwrap();
            link.send(&SyncMessage::Frontier(Frontier {
                authors: Vec::new(),
            }))
            .await
            .unwrap();
            let reply = tokio::time::timeout(within(5), link.recv())
                .await
                .expect("timed out waiting for the refusal")
                .unwrap();
            assert_eq!(reply, Some(SyncMessage::Refused(Refusal::NotAMember)));
            assert!(
                tokio::time::timeout(within(2), connection.closed())
                    .await
                    .is_err(),
                "the refused exchange closed the connection under the join in flight"
            );
            // Once the join itself ends, the unadmitted connection closes.
            join.send(&SyncMessage::Refused(Refusal::NotAMember))
                .await
                .unwrap();
            tokio::time::timeout(within(5), connection.closed())
                .await
                .expect("the unadmitted connection outlived its last exchange");
            inviter.close().await;
        });
    agent.call(Request::Status).unwrap();
}

#[test]
fn newer_connection_replaces_older_ones_from_the_same_endpoint() {
    let dir = short_dir();
    let running = Running::start(dir.path());
    let host_agent = running.enroll(1);
    let mut agent = running.client(Credential([1; 32]), None);
    let goal = goal(&mut running.owner(), host_agent);
    let Response::Invited { ticket } = running
        .owner()
        .call(Request::GoalInvite {
            role: None,

            goal,
            expires_ms: u64::MAX,
        })
        .unwrap()
    else {
        panic!()
    };
    let daemon = Invitation::from_ticket(ticket.as_str()).unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            // One identity three times: a peer and the processes that
            // replace it after a crash, while its old sockets stay silent.
            let mut peers = Vec::new();
            let mut connections = Vec::new();
            for _ in 0..2 {
                let peer = local_endpoint([97; 32]).await.unwrap();
                let connection = peer.connect(daemon.endpoint, &daemon.hints).await.unwrap();
                connections.push(connection);
                peers.push(peer);
            }
            // Two connections made together are simultaneous dials: both stay.
            for connection in &connections {
                assert!(
                    tokio::time::timeout(Duration::from_millis(500), connection.closed())
                        .await
                        .is_err()
                );
            }
            tokio::time::sleep(super::network::REPLACED_AFTER).await;
            let restarted = local_endpoint([97; 32]).await.unwrap();
            let newest = restarted
                .connect(daemon.endpoint, &daemon.hints)
                .await
                .unwrap();
            for connection in &connections {
                tokio::time::timeout(Duration::from_secs(5), connection.closed())
                    .await
                    .expect("an older connection outlived the one that replaced it");
            }
            assert!(
                tokio::time::timeout(Duration::from_millis(500), newest.closed())
                    .await
                    .is_err(),
                "the newest connection was not kept"
            );
            restarted.close().await;
            for peer in peers {
                peer.close().await;
            }
        });
    agent.call(Request::Status).unwrap();
}

/// Copy a stopped daemon's directory, including its identity and SQLite files.
/// Sockets are runtime endpoints, not backup data.
pub(super) fn copy_stopped_home(from: &Path, to: &Path) {
    use std::os::unix::fs::FileTypeExt;
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        let destination = to.join(entry.file_name());
        if kind.is_dir() {
            copy_stopped_home(&entry.path(), &destination);
        } else if kind.is_file() {
            std::fs::copy(entry.path(), destination).unwrap();
        } else {
            assert!(
                kind.is_socket(),
                "unexpected backup entry: {:?}",
                entry.path()
            );
        }
    }
}

/// The goal's events held by a stopped daemon's store.
fn held_events(home: &Path, goal: GoalId) -> Vec<locust_proto::event::Event> {
    use locust_proto::store::Store;
    locust_store::SqliteStore::open(home, &local::marks_dir(home))
        .unwrap()
        .log(&goal, 0, usize::MAX)
        .unwrap()
        .into_iter()
        .map(|(_, event)| event)
        .collect()
}

/// Replay events durably into a stopped daemon's store. Peer ingestion and
/// its order are tested with Network.
fn replay(home: &Path, events: Vec<locust_proto::event::Event>) {
    use locust_proto::store::{Commit, Store};
    locust_store::SqliteStore::open(home, &local::marks_dir(home))
        .unwrap()
        .commit(&Commit {
            events,
            ..Default::default()
        })
        .unwrap();
}

/// The current rules, rebound unchanged: a record only the goal's own key
/// signs, on the governance log rather than any member's.
fn rebind_rules(owner: &mut LocalClient, goal: GoalId) -> Result<EventId, ClientError> {
    let status = goal_status(owner, goal);
    owner
        .call(Request::RulesBind {
            no_role: false,
            goal,
            expected: status.current_rules.unwrap(),
            formation_json: serde_json::to_string(
                &locust_proto::organization::presets()
                    .into_iter()
                    .find(|p| p.name == "directed")
                    .unwrap()
                    .formation,
            )
            .unwrap(),
            inputs: Default::default(),
        })
        .map(recorded)
}

fn task_open(client: &mut LocalClient, goal: GoalId, text: &str) -> Result<EventId, ClientError> {
    client
        .call(Request::TaskOpen {
            goal,
            text: text.into(),
            task_type: None,
            inputs: Default::default(),
            parent: None,
        })
        .map(recorded)
}

fn goal_status(client: &mut LocalClient, goal: GoalId) -> GoalStatus {
    let Response::GoalStatus(status) = client.call(Request::GoalStatus { goal }).unwrap() else {
        panic!()
    };
    status
}

#[track_caller]
fn assert_refused<T: std::fmt::Debug>(result: Result<T, ClientError>, code: ErrorCode) {
    assert!(
        matches!(&result, Err(ClientError::Api(error)) if error.code == code),
        "expected {code:?}: {result:?}"
    );
}

fn continue_goal(owner: &mut LocalClient, goal: GoalId) -> u32 {
    let Response::Continued { keys } = owner.call(Request::GoalContinue { goal }).unwrap() else {
        panic!()
    };
    keys
}

/// The header of an event a stopped daemon's store holds.
fn header(home: &Path, marks: &Path, event: EventId) -> Header {
    SqliteStore::open(home, marks)
        .unwrap()
        .event(&event)
        .unwrap()
        .unwrap()
        .header()
        .clone()
}

/// A goal one daemon hosts for its agent, with another computer's agent
/// admitted on both.
struct Shared {
    goal: GoalId,
    /// Agent 1, on the host's computer.
    host_agent: PublicKey,
    /// Agent 2, on the member's computer.
    member_agent: PublicKey,
}

fn shared_goal(host: &Running, member: &Running) -> Shared {
    let host_agent = host.enroll(1);
    let member_agent = member.enroll(2);
    let goal = goal(&mut host.owner(), host_agent);
    let Response::Invited { ticket } = host
        .owner()
        .call(Request::GoalInvite {
            role: None,
            goal,
            expires_ms: u64::MAX,
        })
        .unwrap()
    else {
        panic!()
    };
    member
        .owner()
        .call(Request::GoalJoin {
            name: "Member".into(),
            agent: member_agent,
            ticket,
            level: Level::Auto,
        })
        .unwrap();
    let mut agent = member.client(Credential([2; 32]), None);
    // Admitted, and no longer held for having just been admitted.
    eventually_observed(
        "the member's admission",
        || agent.call(Request::GoalStatus { goal }),
        |observed| match observed {
            Ok(Response::GoalStatus(status))
                if status.members.iter().any(|m| m.member == member_agent)
                    && status.guard.is_empty() =>
            {
                Some(())
            }
            _ => None,
        },
    );
    Shared {
        goal,
        host_agent,
        member_agent,
    }
}

/// The computer `key` is bound to in `status`.
fn endpoint_of(status: &GoalStatus, key: PublicKey) -> EndpointId {
    status
        .members
        .iter()
        .find(|member| member.member == key)
        .unwrap()
        .endpoint
}

/// The production node, opening no exchange until `open` is set: a daemon
/// whose peers are out of reach. They cannot call it either, since every
/// start binds a new port that no peer has been told.
struct Gated {
    node: ProductionNode,
    open: Arc<AtomicBool>,
}

impl Engine for Gated {
    fn connect(&mut self, conn: ConnId, hello: &ClientHello, now_ms: u64) -> ServerHello {
        self.node.connect(conn, hello, now_ms)
    }
    fn request(&mut self, conn: ConnId, frame: RequestFrame, now_ms: u64) -> Step {
        self.node.request(conn, frame, now_ms)
    }
    fn resume(&mut self, conn: ConnId, parked: &Parked, timed_out: bool, now_ms: u64) -> Step {
        self.node.resume(conn, parked, timed_out, now_ms)
    }
    fn take_changed(&mut self) -> Vec<GoalId> {
        Engine::take_changed(&mut self.node)
    }
    fn disconnect(&mut self, conn: ConnId) {
        self.node.disconnect(conn);
    }
    fn stop_requested(&self) -> bool {
        self.node.stop_requested()
    }
    fn failure(&self) -> Option<ApiError> {
        self.node.failure()
    }
    fn farm_poll(&mut self, now_ms: u64) -> Vec<FarmUpload> {
        self.node.farm_poll(now_ms)
    }
    fn farm_complete(&mut self, result: FarmUploadResult, now_ms: u64) -> Result<(), ApiError> {
        self.node.farm_complete(result, now_ms)
    }
}

impl PeerEngine for Gated {
    fn endpoint_secret(&self) -> [u8; 32] {
        self.node.endpoint_secret()
    }
    fn peer_readable(&self, exchange: ExchangeId) -> bool {
        self.node.peer_readable(exchange)
    }
    fn peer(&mut self, input: PeerInput, time: PeerTime, out: &mut Vec<PeerOutput>) {
        if matches!(input, PeerInput::Poll) && !self.open.load(Ordering::Acquire) {
            return;
        }
        self.node.peer(input, time, out);
    }
}

/// Which of the host's records the older copy lacks.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lost {
    /// A task the host's agent opened.
    Agent,
    /// A rules record the goal's own key signed.
    Governance,
}

/// The host's data directory is put back from a copy older than one of its
/// records, beside the marks it kept. The key that signed the record signs
/// nothing in the goal until the record is held again, and then signs at
/// the next position. The member's computer, which holds the record, stays
/// out of reach, so no hearing ends the hold: a goal never shared would be
/// given up at once.
fn an_older_directory_is_held_until_its_later_events_return(lost: Lost) {
    let (original, member_home, backup) = (short_dir(), short_dir(), short_dir());
    let mut host = Running::start(original.path());
    let mut member = Running::start(member_home.path());
    let shared = shared_goal(&host, &member);
    let goal = shared.goal;
    member.stop();
    host.stop();
    copy_stopped_home(original.path(), backup.path());

    let mut host = Running::start(original.path());
    let mut owner = host.owner();
    let mut agent = host.client(Credential([1; 32]), None);
    let member_endpoint = endpoint_of(&goal_status(&mut owner, goal), shared.member_agent);
    let old_id = match lost {
        Lost::Agent => task_open(&mut agent, goal, "After the copy").unwrap(),
        Lost::Governance => rebind_rules(&mut owner, goal).unwrap(),
    };
    drop((owner, agent));
    host.stop();
    let marks = local::marks_dir(original.path());
    let old = header(original.path(), &marks, old_id);
    let held = held_events(original.path(), goal);

    let mut restored = Running::start_with_marks(backup.path(), &marks);
    let mut owner = restored.owner();
    let mut agent = restored.client(Credential([1; 32]), None);
    let status = goal_status(&mut owner, goal);
    assert_eq!(status.restored, Some(0), "{status:?}");
    assert_eq!(
        status.guard,
        vec![GuardView {
            key: old.author,
            by_host: lost == Lost::Governance,
            reason: GuardReason::Behind {
                held: old.seq,
                signed: old.seq + 1,
            },
            heard: vec![],
            waiting: vec![member_endpoint],
        }]
    );
    match lost {
        Lost::Agent => {
            assert_eq!(status.halted, None, "{status:?}");
            assert_refused(task_open(&mut agent, goal, "Held"), ErrorCode::ReadOnly);
            // Every other key signs at once.
            rebind_rules(&mut owner, goal).unwrap();
        }
        Lost::Governance => {
            assert_eq!(status.halted, Some(Halt::SignerRecovery), "{status:?}");
            assert_refused(rebind_rules(&mut owner, goal), ErrorCode::ReadOnly);
            // On the host's computer every agent is held with the goal's key.
            assert_refused(task_open(&mut agent, goal, "Held"), ErrorCode::ReadOnly);
        }
    }
    drop((owner, agent));
    restored.stop();

    replay(backup.path(), held);
    let mut restored = Running::start_with_marks(backup.path(), &marks);
    let mut owner = restored.owner();
    let mut agent = restored.client(Credential([1; 32]), None);
    let status = goal_status(&mut owner, goal);
    assert!(status.guard.is_empty(), "{status:?}");
    assert_eq!(status.halted, None, "{status:?}");
    assert_eq!(status.restored, None, "{status:?}");
    let new_id = match lost {
        Lost::Agent => task_open(&mut agent, goal, "After the events returned").unwrap(),
        Lost::Governance => rebind_rules(&mut owner, goal).unwrap(),
    };
    drop((owner, agent));
    restored.stop();
    let new = header(backup.path(), &marks, new_id);
    assert_eq!(new.author, old.author);
    assert_eq!(new.seq, old.seq + 1);
    assert_eq!(new.prev, Some(old_id));
}

#[test]
fn an_older_directory_is_held_until_its_later_events_return_for_an_agent_record() {
    an_older_directory_is_held_until_its_later_events_return(Lost::Agent);
}

#[test]
fn an_older_directory_is_held_until_its_later_events_return_for_a_governance_record() {
    an_older_directory_is_held_until_its_later_events_return(Lost::Governance);
}

#[test]
fn a_lost_marks_directory_is_an_ordinary_start_and_a_copy_of_both_is_not() {
    let (home, copy) = (short_dir(), short_dir());
    let marks = local::marks_dir(home.path());
    let mut running = Running::start(home.path());
    let principal = running.enroll(1);
    let goal = goal(&mut running.owner(), principal);
    let before = task_open(
        &mut running.client(Credential([1; 32]), None),
        goal,
        "Before the marks are lost",
    )
    .unwrap();
    running.stop();
    std::fs::remove_dir_all(&marks).unwrap();

    // The database is the file last used: an ordinary start.
    let mut running = Running::start(home.path());
    let status = goal_status(&mut running.owner(), goal);
    assert!(status.guard.is_empty(), "{status:?}");
    assert_eq!(status.halted, None, "{status:?}");
    assert_eq!(status.restored, None, "{status:?}");
    let governance = status.governance;
    running.stop();
    // That start wrote the marks again from the store, before anything signed.
    {
        let found = SqliteStore::open(home.path(), &marks)
            .unwrap()
            .marks()
            .unwrap();
        let kept = found.kept.expect("the marks are written again");
        let mark = |key| {
            kept.iter()
                .find(|mark| mark.goal == goal && mark.key == key)
        };
        assert_eq!(mark(principal).unwrap().point.id, before);
        assert!(mark(governance).is_some(), "{kept:?}");
    }
    let mut running = Running::start(home.path());
    let after = task_open(
        &mut running.client(Credential([1; 32]), None),
        goal,
        "After the marks are lost",
    )
    .unwrap();
    running.stop();
    assert_eq!(
        header(home.path(), &marks, after).seq,
        header(home.path(), &marks, before).seq + 1
    );

    // Both directories copied, as a whole computer is restored: the copied
    // marks are lost and the database is another file, so the copy is of
    // unknown age. Where this daemon hosts the goal it waits for the person,
    // with no other computer to hear from.
    copy_stopped_home(home.path(), copy.path());
    copy_stopped_home(&marks, &local::marks_dir(copy.path()));
    let mut restored = Running::start(copy.path());
    let mut owner = restored.owner();
    let mut agent = restored.client(Credential([1; 32]), None);
    let status = goal_status(&mut owner, goal);
    assert_eq!(status.restored, Some(0), "{status:?}");
    assert_eq!(status.halted, Some(Halt::SignerRecovery), "{status:?}");
    let unheard = |key, by_host| GuardView {
        key,
        by_host,
        reason: GuardReason::Unheard,
        heard: vec![],
        waiting: vec![],
    };
    let mut expected = vec![unheard(governance, true), unheard(principal, false)];
    expected.sort_by_key(|view| view.key);
    assert_eq!(status.guard, expected);
    assert_refused(task_open(&mut agent, goal, "Held"), ErrorCode::ReadOnly);
    assert_refused(rebind_rules(&mut owner, goal), ErrorCode::ReadOnly);
    // An agent cannot end the hold.
    assert_refused(
        agent.call(Request::GoalContinue { goal }),
        ErrorCode::Denied,
    );
    assert_eq!(continue_goal(&mut owner, goal), 2);
    let status = goal_status(&mut owner, goal);
    assert!(status.guard.is_empty(), "{status:?}");
    assert_eq!(status.halted, None, "{status:?}");
    let next = task_open(&mut agent, goal, "After continuing").unwrap();
    drop((owner, agent));
    restored.stop();
    let copy_marks = local::marks_dir(copy.path());
    assert_eq!(
        header(copy.path(), &copy_marks, next).prev,
        Some(after),
        "the copy held every record, so continuing reused no position"
    );
}

/// Writes a stopped daemon's database files from `from` over those of `to`
/// in place, as a restore that rewrites a file's bytes does: the database
/// keeps its inode and creation time, so it is still the file last used.
fn overwrite_database_in_place(from: &Path, to: &Path) {
    use std::io::Write;
    for suffix in ["", "-wal", "-shm"] {
        let path = |home: &Path| {
            let mut path = local::database_path(home).into_os_string();
            path.push(suffix);
            PathBuf::from(path)
        };
        let (source, target) = (path(from), path(to));
        if source.exists() {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&target)
                .unwrap();
            file.write_all(&std::fs::read(&source).unwrap()).unwrap();
            file.sync_all().unwrap();
        } else if target.exists() {
            std::fs::remove_file(&target).unwrap();
        }
    }
}

/// The database file is overwritten in place by an older copy, beside the
/// marks it kept. One file holds every goal, so every goal is treated as
/// restored, not only the one whose mark is ahead.
#[test]
fn a_database_overwritten_in_place_is_found_by_its_marks() {
    use locust_proto::api::InvitationState;
    let (home, member_home, backup) = (short_dir(), short_dir(), short_dir());
    let mut host = Running::start(home.path());
    let mut member = Running::start(member_home.path());
    let shared = shared_goal(&host, &member);
    member.stop();
    let mut owner = host.owner();
    let other = goal(&mut owner, shared.host_agent);
    let Response::Invited { .. } = owner
        .call(Request::GoalInvite {
            role: None,
            goal: other,
            expires_ms: u64::MAX,
        })
        .unwrap()
    else {
        panic!()
    };
    drop(owner);
    host.stop();
    copy_stopped_home(home.path(), backup.path());

    let mut host = Running::start(home.path());
    let lost = task_open(
        &mut host.client(Credential([1; 32]), None),
        shared.goal,
        "After the copy",
    )
    .unwrap();
    host.stop();
    let database = local::database_path(home.path());
    let file = FileId::of(&database).unwrap();
    overwrite_database_in_place(backup.path(), home.path());
    assert_eq!(FileId::of(&database).unwrap(), file);
    assert_eq!(
        SqliteStore::open(home.path(), &local::marks_dir(home.path()))
            .unwrap()
            .event(&lost)
            .unwrap(),
        None
    );

    let mut host = Running::start(home.path());
    let mut owner = host.owner();
    let mut agent = host.client(Credential([1; 32]), None);
    // The goal whose mark is ahead: its agent is behind.
    let status = goal_status(&mut owner, shared.goal);
    assert_eq!(status.restored, Some(0), "{status:?}");
    assert!(
        matches!(
            status.guard.as_slice(),
            [GuardView { key, by_host: false, reason: GuardReason::Behind { .. }, .. }]
                if *key == shared.host_agent
        ),
        "{status:?}"
    );
    assert_refused(
        task_open(&mut agent, shared.goal, "Held"),
        ErrorCode::ReadOnly,
    );
    // The other goal has no mark ahead and is restored all the same: its
    // pending invitation is revoked, and none of its keys is held.
    let status = goal_status(&mut owner, other);
    assert_eq!(status.restored, Some(1), "{status:?}");
    assert!(status.guard.is_empty(), "{status:?}");
    assert_eq!(status.halted, None, "{status:?}");
    let Response::Invitations { invitations } = owner
        .call(Request::GoalInvitations { goal: other })
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(
        invitations
            .iter()
            .map(|invitation| invitation.state)
            .collect::<Vec<_>>(),
        vec![InvitationState::Revoked]
    );
    task_open(&mut agent, other, "Signed at once").unwrap();
    rebind_rules(&mut owner, other).unwrap();
    drop((owner, agent));
    host.stop();
}

/// A restore found while its marks were kept is remembered as the goal's
/// `RESTORED` record. Losing the marks before the next ordinary start must
/// not make that goal ordinary: its keys become unheard instead.
#[test]
fn lost_marks_while_a_restore_is_caught_up_make_the_goal_unheard() {
    on_the_hosts_computer_the_goal_waits_for_the_person();
    on_a_members_computer_the_hold_ends_when_the_hosts_computer_is_heard();
}

fn on_the_hosts_computer_the_goal_waits_for_the_person() {
    let (original, member_home, backup) = (short_dir(), short_dir(), short_dir());
    let mut host = Running::start(original.path());
    let mut member = Running::start(member_home.path());
    let shared = shared_goal(&host, &member);
    let goal = shared.goal;
    member.stop();
    host.stop();
    copy_stopped_home(original.path(), backup.path());
    let mut host = Running::start(original.path());
    let lost = task_open(
        &mut host.client(Credential([1; 32]), None),
        goal,
        "After the copy",
    )
    .unwrap();
    host.stop();
    let marks = local::marks_dir(original.path());
    let held = held_events(original.path(), goal);

    let mut restored = Running::start_with_marks(backup.path(), &marks);
    let status = goal_status(&mut restored.owner(), goal);
    assert_eq!(status.restored, Some(0), "{status:?}");
    assert!(
        matches!(
            status.guard.as_slice(),
            [GuardView {
                reason: GuardReason::Behind { .. },
                ..
            }]
        ),
        "{status:?}"
    );
    restored.stop();
    std::fs::remove_dir_all(&marks).unwrap();

    let mut restored = Running::start_with_marks(backup.path(), &marks);
    let mut owner = restored.owner();
    let mut agent = restored.client(Credential([1; 32]), None);
    let status = goal_status(&mut owner, goal);
    assert_eq!(status.restored, Some(0), "{status:?}");
    assert_eq!(status.halted, Some(Halt::SignerRecovery), "{status:?}");
    assert_eq!(status.guard.len(), 2, "{status:?}");
    assert!(
        status
            .guard
            .iter()
            .all(|view| view.reason == GuardReason::Unheard),
        "{status:?}"
    );
    assert_refused(task_open(&mut agent, goal, "Held"), ErrorCode::ReadOnly);
    drop((owner, agent));
    restored.stop();

    // Every record returns, and the hold lasts: on the host's computer only
    // the person ends it.
    replay(backup.path(), held);
    let mut restored = Running::start_with_marks(backup.path(), &marks);
    let mut owner = restored.owner();
    let mut agent = restored.client(Credential([1; 32]), None);
    let status = goal_status(&mut owner, goal);
    assert!(
        !status.guard.is_empty()
            && status
                .guard
                .iter()
                .all(|view| view.reason == GuardReason::Unheard),
        "{status:?}"
    );
    assert_refused(task_open(&mut agent, goal, "Held"), ErrorCode::ReadOnly);
    assert_eq!(continue_goal(&mut owner, goal), 2);
    let next = task_open(&mut agent, goal, "After continuing").unwrap();
    drop((owner, agent));
    restored.stop();
    let next = header(backup.path(), &marks, next);
    assert_eq!(next.prev, Some(lost));
}

/// Opens a task as a member's agent once its daemon can: content arrives
/// after the records that name it, and a candidate that cannot be applied
/// yet is refused and signs nothing.
fn member_task_open(agent: &mut LocalClient, goal: GoalId, text: &str) -> EventId {
    eventually_observed(
        "a task the member's agent may open",
        || task_open(agent, goal, text),
        |opened| opened.as_ref().ok().copied(),
    )
}

fn on_a_members_computer_the_hold_ends_when_the_hosts_computer_is_heard() {
    let (host_home, original, copy) = (short_dir(), short_dir(), short_dir());
    let host = Running::start(host_home.path());
    let mut member = Running::start(original.path());
    let shared = shared_goal(&host, &member);
    let goal = shared.goal;
    member.stop();
    copy_stopped_home(original.path(), copy.path());
    let mut member = Running::start(original.path());
    let lost = member_task_open(
        &mut member.client(Credential([2; 32]), None),
        goal,
        "After the copy",
    );
    let mut host_agent = host.client(Credential([1; 32]), None);
    eventually(|| {
        matches!(
            host_agent.call(Request::Event { goal, event: lost }),
            Ok(Response::Event(_))
        )
        .then_some(())
    });
    member.stop();
    let marks = local::marks_dir(original.path());

    // Beside the marks it kept, the copy is behind until the host's computer
    // returns the record. The goal's `RESTORED` record outlives the hold.
    let mut restored = Running::start_with_marks(copy.path(), &marks);
    let mut agent = restored.client(Credential([2; 32]), None);
    eventually_observed(
        "the copy catching up",
        || goal_status(&mut agent, goal),
        |status| status.guard.is_empty().then_some(()),
    );
    let status = goal_status(&mut agent, goal);
    assert_eq!(status.restored, Some(0), "{status:?}");
    let host_endpoint = endpoint_of(&status, shared.host_agent);
    drop(agent);
    restored.stop();
    std::fs::remove_dir_all(&marks).unwrap();

    let open = Arc::new(AtomicBool::new(false));
    let gate = open.clone();
    let mut restored =
        Running::start_observed(copy.path(), &marks, move |node| Gated { node, open: gate });
    let mut agent = restored.client(Credential([2; 32]), None);
    let status = goal_status(&mut agent, goal);
    assert_eq!(
        status.guard,
        vec![GuardView {
            key: shared.member_agent,
            by_host: false,
            reason: GuardReason::Unheard,
            heard: vec![],
            waiting: vec![host_endpoint],
        }]
    );
    assert_refused(task_open(&mut agent, goal, "Held"), ErrorCode::ReadOnly);
    open.store(true, Ordering::Release);
    eventually_observed(
        "the host's computer heard",
        || goal_status(&mut agent, goal),
        |status| status.guard.is_empty().then_some(()),
    );
    let next = member_task_open(&mut agent, goal, "After hearing the host");
    drop((agent, host_agent));
    restored.stop();
    let next = header(copy.path(), &marks, next);
    assert_eq!(next.prev, Some(lost));
}
