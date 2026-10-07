//! The production assembly over real SQLite, Unix sockets and local Iroh.
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use locust_net::{Endpoint, EndpointConfig, IpTransport, Lookup, RelayConfig, TransportBudget};
use locust_proto::api::{Caller, Credential, Level, Request, Response, SessionSecret};
use locust_proto::client::Client;
use locust_proto::engine::{Engine, PeerEngine};
use locust_proto::event::{AttemptStatus, ReviewVerdict, TaskId};
use locust_proto::id::{EventId, GoalId, IdempotencyKey, PublicKey};
use locust_proto::invite::Invitation;
use locust_proto::local;

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
        Self::start_observed(home, std::convert::identity)
    }
    /// The same daemon, with its node handed to `observe` on the engine thread.
    pub(super) fn start_observed<E, O>(home: &Path, observe: O) -> Self
    where
        E: Engine + PeerEngine + 'static,
        O: FnOnce(ProductionNode) -> E + Send + 'static,
    {
        let (shutdown, stopped) = tokio::sync::oneshot::channel();
        let (ready, waiting) = mpsc::channel();
        let state = home.to_path_buf();
        let thread = thread::spawn(move || {
            run_networked_with(
                &state,
                &local::marks_dir(&state),
                observe,
                local_endpoint,
                move |_| {
                    ready.send(()).unwrap();
                    Ok(async move {
                        let _ = stopped.await;
                    })
                },
            )
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
fn rebind_rules(owner: &mut LocalClient, goal: GoalId, _agent: PublicKey) -> EventId {
    let Response::GoalStatus(status) = owner.call(Request::GoalStatus { goal }).unwrap() else {
        panic!()
    };
    recorded(
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
            .unwrap(),
    )
}

#[test]
fn sqlite_older_directory_forks_only_the_hosts_agent_when_its_own_work_was_lost() {
    use locust_proto::store::Store;
    use locust_store::SqliteStore;
    for recover_first in [false, true] {
        let original = short_dir();
        let backup = short_dir();
        let mut running = Running::start(original.path());
        let principal = running.enroll(1);
        let agent = running.client(Credential([1; 32]), None);
        let goal = goal(&mut running.owner(), principal);
        drop(agent);
        running.stop();
        copy_stopped_home(original.path(), backup.path());

        let mut running = Running::start(original.path());
        let mut agent = running.client(Credential([1; 32]), None);
        let TaskId::Authored(old_id) = propose(&mut agent, goal, "After backup".into()) else {
            panic!()
        };
        drop(agent);
        running.stop();
        let held = held_events(original.path(), goal);
        let old = held.iter().find(|e| e.id() == old_id).unwrap();
        if recover_first {
            replay(backup.path(), held.clone());
        }
        let mut restored = Running::start(backup.path());
        let mut agent = restored.client(Credential([1; 32]), None);
        assert_eq!(agent.caller(), Caller::Agent(principal));
        let TaskId::Authored(new_id) = propose(&mut agent, goal, "After restoring backup".into())
        else {
            panic!()
        };
        drop(agent);
        restored.stop();
        {
            let store = SqliteStore::open(backup.path(), &local::marks_dir(backup.path())).unwrap();
            let new = store.event(&new_id).unwrap().unwrap();
            assert_ne!(new_id, old_id);
            assert_eq!(
                new.header().seq,
                old.header().seq + u64::from(recover_first)
            );
            assert_eq!(
                new.header().prev,
                if recover_first {
                    Some(old_id)
                } else {
                    old.header().prev
                }
            );
        }
        replay(backup.path(), held);
        let mut restored = Running::start(backup.path());
        let mut agent = restored.client(Credential([1; 32]), None);
        let Response::GoalStatus(status) = agent.call(Request::GoalStatus { goal }).unwrap() else {
            panic!()
        };
        // The agent's own log forked, not the goal's: governance goes on.
        assert_eq!(status.halted, None, "{status:?}");
        let Response::Invited { .. } = restored
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
        if recover_first {
            propose(
                &mut agent,
                goal,
                "Still extends after another restart".into(),
            );
        } else {
            let result = agent.call(Request::TaskOpen {
                goal,
                text: "Known fork".into(),
                task_type: None,
                inputs: Default::default(),
                parent: None,
            });
            assert!(
                matches!(&result, Err(locust_proto::client::ClientError::Api(error))
                if error.code == locust_proto::api::ErrorCode::Unavailable),
                "{result:?}"
            );
        }
        drop(agent);
        restored.stop();
    }
}

#[test]
fn sqlite_older_directory_halts_governance_when_a_governance_record_was_lost() {
    use locust_proto::store::Store;
    use locust_store::SqliteStore;
    let original = short_dir();
    let backup = short_dir();
    let mut running = Running::start(original.path());
    let principal = running.enroll(1);
    let goal = goal(&mut running.owner(), principal);
    running.stop();
    copy_stopped_home(original.path(), backup.path());

    let mut running = Running::start(original.path());
    let old_id = rebind_rules(&mut running.owner(), goal, principal);
    running.stop();
    let held = held_events(original.path(), goal);
    let old = held.iter().find(|e| e.id() == old_id).unwrap();

    let mut restored = Running::start(backup.path());
    let new_id = rebind_rules(&mut restored.owner(), goal, principal);
    restored.stop();
    {
        let store = SqliteStore::open(backup.path(), &local::marks_dir(backup.path())).unwrap();
        let new = store.event(&new_id).unwrap().unwrap();
        assert_ne!(new_id, old_id);
        assert_eq!(new.header().author, old.header().author);
        assert_eq!(new.header().seq, old.header().seq);
        assert_eq!(new.header().prev, old.header().prev);
    }
    replay(backup.path(), held);
    let mut restored = Running::start(backup.path());
    let mut agent = restored.client(Credential([1; 32]), None);
    let Response::GoalStatus(status) = agent.call(Request::GoalStatus { goal }).unwrap() else {
        panic!()
    };
    assert!(status.halted.is_some(), "{status:?}");
    drop(agent);
    restored.stop();
}
