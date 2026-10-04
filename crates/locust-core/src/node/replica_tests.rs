//! Production Nodes over reopenable Stores, driven through their public seams.
use crate::node::Node;
use locust_proto::api::{
    ClientHello, Credential, Grants, Request, RequestFrame, Response, ServerHello,
};
use locust_proto::engine::{
    ConnId, Engine, Entropy, ExchangeId, PeerEngine, PeerInput, PeerOutput, Step,
};
use locust_proto::event::{Body, Context, Scope};
use locust_proto::id::{EndpointId, GoalId, PublicKey};
use locust_proto::store::{MemStore, Store};
use std::collections::{BTreeMap, HashMap, VecDeque};

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
struct Peer {
    node: Node<MemStore, Random>,
    store: MemStore,
    principal: PublicKey,
    endpoint: EndpointId,
    online: bool,
}
const OWNER: Credential = Credential([99; 32]);
fn connect(node: &mut Node<MemStore, Random>) {
    assert!(matches!(
        node.connect(
            ConnId(1),
            &ClientHello {
                api_version: locust_proto::API_VERSION,
                credential: OWNER,
                session: None
            },
            0
        ),
        ServerHello::Welcome { .. }
    ));
}
fn request(
    node: &mut Node<MemStore, Random>,
    principal: Option<PublicKey>,
    request: Request,
) -> Response {
    match node.request(
        ConnId(1),
        RequestFrame {
            id: 1,
            idempotency: None,
            on_behalf: principal,
            request,
        },
        10,
    ) {
        Step::Reply(reply) => reply.result.expect("public operation succeeds"),
        Step::Park(_) => panic!("unexpected wait"),
    }
}
impl Peer {
    fn new(seed: u8) -> Self {
        let store = MemStore::new();
        let mut node = Node::open(
            store.reopen(),
            Random(u64::from(seed) * 10000),
            OWNER.digest(),
            "test".into(),
            0,
        )
        .unwrap();
        connect(&mut node);
        let endpoint = EndpointId([seed; 32]);
        node.peer(
            PeerInput::Endpoint {
                endpoint,
                hints: vec![],
            },
            locust_proto::engine::PeerTime {
                unix_ms: 0,
                elapsed_ms: 0,
            },
            &mut vec![],
        );
        let Response::AgentEnrolled { agent } = request(
            &mut node,
            None,
            Request::AgentEnroll {
                name: format!("peer{seed}"),
                grants: Grants { manage_goals: true },
                credential: Credential([seed; 32]).digest(),
            },
        ) else {
            panic!("enroll");
        };
        Self {
            node,
            store,
            principal: agent,
            endpoint,
            online: true,
        }
    }
    fn call(&mut self, operation: Request) -> Response {
        request(&mut self.node, Some(self.principal), operation)
    }
    fn restart(&mut self) {
        self.node = Node::open(
            self.store.reopen(),
            Random(900000),
            OWNER.digest(),
            "test".into(),
            0,
        )
        .unwrap();
        connect(&mut self.node);
    }
}

fn reconcile(peers: &mut [Peer], now: u64) -> Vec<locust_proto::sync::SyncMessage> {
    let dialers: Vec<_> = (0..peers.len()).collect();
    reconcile_from(peers, now, &dialers)
}

fn reconcile_from(
    peers: &mut [Peer],
    now: u64,
    dialers: &[usize],
) -> Vec<locust_proto::sync::SyncMessage> {
    let mut sent = Vec::new();
    let mut queue: VecDeque<_> = dialers.iter().map(|i| (*i, PeerInput::Poll)).collect();
    let mut routes = HashMap::new();
    let mut accepted = 0;
    let mut steps = 0;
    while let Some((i, input)) = queue.pop_front() {
        steps += 1;
        assert!(steps < 100000, "network does not quiesce");
        if !peers[i].online {
            continue;
        }
        let mut outputs = vec![];
        peers[i].node.peer(
            input,
            locust_proto::engine::PeerTime {
                unix_ms: now,
                elapsed_ms: now,
            },
            &mut outputs,
        );
        for output in outputs {
            match output {
                PeerOutput::Open {
                    exchange, endpoint, ..
                } => {
                    if let Some(j) = peers
                        .iter()
                        .position(|peer| peer.online && peer.endpoint == endpoint)
                    {
                        let incoming = ExchangeId::Accepted(accepted);
                        accepted += 1;
                        routes.insert((i, exchange), (j, incoming));
                        routes.insert((j, incoming), (i, exchange));
                        queue.push_back((
                            j,
                            PeerInput::Accepted {
                                exchange: incoming,
                                remote: peers[i].endpoint,
                            },
                        ));
                        queue.push_back((i, PeerInput::Opened(exchange)));
                    } else {
                        queue.push_back((i, PeerInput::OpenFailed(exchange)));
                    }
                }
                PeerOutput::Send { exchange, frame } => {
                    sent.push(frame.clone());
                    let (j, incoming) = routes[&(i, exchange)];
                    queue.push_back((
                        j,
                        PeerInput::Frame {
                            exchange: incoming,
                            frame,
                        },
                    ));
                    queue.push_back((i, PeerInput::Writable(exchange)));
                }
                PeerOutput::Admit(_) | PeerOutput::Evidence(_) => {}
                PeerOutput::Finish(exchange) => queue.push_back((i, PeerInput::Finished(exchange))),
            }
        }
    }
    sent
}

fn found(peers: &mut [Peer]) -> GoalId {
    let Response::GoalCreated { goal } = peers[0].call(Request::GoalCreate {
        title: "shared durable goal".into(),
        blueprint_json: Some(r#"{"schema_version":1,"context":{"inputs":{"snapshot":{"kind":"artifact","required":false}}}}"#.into()),
        roles: BTreeMap::new(),
        inputs: BTreeMap::new(),
    }) else {
        panic!("create");
    };
    for index in 1..peers.len() {
        let Response::Invited { ticket } = peers[0].call(Request::GoalInvite {
            goal,
            expires_ms: None,
        }) else {
            panic!("invite");
        };
        peers[index].call(Request::GoalJoin { ticket });
    }
    reconcile(peers, 1000);
    reconcile(peers, 2000);
    goal
}

#[test]
fn actual_nodes_join_converge_read_sealed_content_and_reopen() {
    let mut peers = [Peer::new(1), Peer::new(2), Peer::new(3)];
    let goal = found(&mut peers);
    for peer in &mut peers {
        let Response::GoalStatus(status) = peer.call(Request::GoalStatus { goal }) else {
            panic!("status");
        };
        assert_eq!(status.members.len(), 3);
        assert_eq!(status.title.as_deref(), Some("shared durable goal"));
    }
    peers[0].online = false;
    peers[1].call(Request::ContributionPublish {
        goal,
        task: None,
        attempt: None,
        generation: None,
        base: None,
        patch: None,
        artifacts: vec![],
        summary: "offline from administrator".into(),
    });
    reconcile(&mut peers, 35000);
    let Response::Contributions(notes) = peers[2].call(Request::Contributions { goal, task: None })
    else {
        panic!("notes");
    };
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].text.as_deref(), Some("offline from administrator"));
    peers[0].restart();
    peers[0].online = true;
    reconcile(&mut peers, 100000);
    reconcile(&mut peers, 140000);
    let logs: Vec<_> = peers
        .iter()
        .map(|peer| {
            peer.store
                .log(&goal, 0, usize::MAX)
                .unwrap()
                .into_iter()
                .map(|(_, event)| event.id())
                .collect::<std::collections::BTreeSet<_>>()
        })
        .collect();
    assert!(logs.iter().all(|log| log == &logs[0]));
    peers[2].restart();
    let Response::Contributions(notes) = peers[2].call(Request::Contributions { goal, task: None })
    else {
        panic!("notes");
    };
    assert_eq!(notes[0].text.as_deref(), Some("offline from administrator"));
}

#[test]
fn durable_multichunk_resume_verifies_hash_and_withdrawal_stops_serving() {
    use crate::node::requests::content::{BlobRecord, blob_write};
    use crate::sync::{Host, Staged};
    use locust_proto::limits::BLOB_CHUNK_BYTES;
    use locust_proto::store::Commit;
    use locust_proto::sync::SyncMessage;
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let plain = vec![0x6a; 2 * BLOB_CHUNK_BYTES + 5];
    let Response::BlobStored { hash } = peers[0].call(Request::BlobPut {
        goal,
        bytes: plain.clone(),
    }) else {
        panic!("put");
    };
    let sealed = peers[0].store.blob(&hash).unwrap().unwrap();
    peers[1]
        .store
        .commit(&Commit {
            local: vec![blob_write(
                &goal,
                &hash,
                &BlobRecord {
                    wanted: true,
                    withdrawn: false,
                },
            )],
            ..Commit::default()
        })
        .unwrap();
    let replica = Host::replica(&mut peers[1].node, &goal).unwrap();
    assert_eq!(
        replica.stage(&hash, 0, sealed.len() as u64, &vec![0; sealed.len()]),
        Staged::Rejected
    );
    assert_eq!(peers[1].store.staged_len(&hash).unwrap(), 0);
    let replica = Host::replica(&mut peers[1].node, &goal).unwrap();
    assert_eq!(
        replica.stage(&hash, 0, sealed.len() as u64, &sealed[..BLOB_CHUNK_BYTES]),
        Staged::More(BLOB_CHUNK_BYTES as u64)
    );
    peers[1].restart();
    let sent = reconcile(&mut peers, 35000);
    assert!(sent.iter().any(|frame| matches!(frame, SyncMessage::BlobRequest { hash: asked, offset } if *asked == hash && *offset == BLOB_CHUNK_BYTES as u64)));
    assert_eq!(peers[1].store.staged_len(&hash).unwrap(), 0);
    let Response::Blob { bytes } = peers[1].call(Request::BlobGet { goal, hash }) else {
        panic!("get");
    };
    assert_eq!(bytes, plain);
    peers[0].call(Request::BlobWithdraw { goal, hash });
    let replica = Host::replica(&mut peers[0].node, &goal).unwrap();
    assert!(replica.blob_len(&hash).is_none());
    assert!(replica.blob_range(&hash, 0, 10).is_none());
    peers[0].restart();
    assert!(
        Host::replica(&mut peers[0].node, &goal)
            .unwrap()
            .blob_len(&hash)
            .is_none()
    );
    // A valid epoch key is durable; replacing it with random bytes is refused.
    let replica = Host::replica(&mut peers[1].node, &goal).unwrap();
    assert!(!replica.offer_key(0, locust_proto::crypto::ContentKey([0; 32])));
    assert!(replica.key(0).is_some());
}

#[test]
fn invitations_bind_once_to_authenticated_member_and_survive_restart() {
    use crate::sync::Host;
    use locust_proto::invite::{Invitation, JoinRequest};
    use locust_proto::sync::Refusal;
    let mut peers = [Peer::new(1), Peer::new(2), Peer::new(3)];
    let Response::GoalCreated { goal } = peers[0].call(Request::GoalCreate {
        title: "join checks".into(),
        blueprint_json: None,
        roles: BTreeMap::new(),
        inputs: BTreeMap::new(),
    }) else {
        panic!("create");
    };
    let Response::Invited { ticket } = peers[0].call(Request::GoalInvite {
        goal,
        expires_ms: Some(100),
    }) else {
        panic!("invite");
    };
    let invite = Invitation::from_ticket(ticket.as_str()).unwrap();
    let join = JoinRequest::sign(
        goal,
        peers[1].endpoint,
        invite.secret,
        peers[1].node.signer(&peers[1].principal).unwrap(),
    );
    assert_eq!(
        Host::join(&mut peers[0].node, &EndpointId([7; 32]), &join, 20),
        Err(Refusal::InvitationRefused)
    );
    let endpoint = peers[1].endpoint;
    assert_eq!(Host::join(&mut peers[0].node, &endpoint, &join, 20), Ok(()));
    let count = peers[0].store.log(&goal, 0, usize::MAX).unwrap().len();
    peers[0].restart();
    assert_eq!(
        Host::join(&mut peers[0].node, &endpoint, &join, 500),
        Ok(())
    );
    assert_eq!(
        peers[0].store.log(&goal, 0, usize::MAX).unwrap().len(),
        count
    );
    let other = JoinRequest::sign(
        goal,
        peers[2].endpoint,
        invite.secret,
        peers[2].node.signer(&peers[2].principal).unwrap(),
    );
    let endpoint = peers[2].endpoint;
    assert_eq!(
        Host::join(&mut peers[0].node, &endpoint, &other, 30),
        Err(Refusal::InvitationRefused)
    );
    let Response::Invited { ticket } = peers[0].call(Request::GoalInvite {
        goal,
        expires_ms: Some(100),
    }) else {
        panic!("invite");
    };
    let invite = Invitation::from_ticket(ticket.as_str()).unwrap();
    let expired = JoinRequest::sign(
        goal,
        endpoint,
        invite.secret,
        peers[2].node.signer(&peers[2].principal).unwrap(),
    );
    assert_eq!(
        Host::join(&mut peers[0].node, &endpoint, &expired, 100),
        Err(Refusal::InvitationRefused)
    );
}

#[test]
fn removal_distributes_a_verified_new_epoch_only_to_remaining_members() {
    use crate::sync::Host;
    let mut peers = [Peer::new(1), Peer::new(2), Peer::new(3)];
    let goal = found(&mut peers);
    let removed = peers[2].principal;
    peers[0].call(Request::MemberRemove {
        goal,
        member: removed,
    });
    peers[0].call(Request::ContributionPublish {
        goal,
        task: None,
        attempt: None,
        generation: None,
        base: None,
        patch: None,
        artifacts: vec![],
        summary: "new epoch".into(),
    });
    reconcile(&mut peers, 35000);
    reconcile(&mut peers, 70000);
    assert!(
        Host::replica(&mut peers[1].node, &goal)
            .unwrap()
            .key(1)
            .is_some()
    );
    assert!(
        Host::replica(&mut peers[2].node, &goal)
            .unwrap()
            .key(1)
            .is_none()
    );
    let Response::Contributions(notes) = peers[1].call(Request::Contributions { goal, task: None })
    else {
        panic!("notes");
    };
    assert_eq!(notes[0].text.as_deref(), Some("new epoch"));
    peers[1].restart();
    assert!(
        Host::replica(&mut peers[1].node, &goal)
            .unwrap()
            .key(1)
            .is_some()
    );
}

#[test]
fn refused_join_does_not_poison_another_local_principals_invitation() {
    let mut peers = [Peer::new(1), Peer::new(2)];
    let Response::AgentEnrolled { agent: second } = request(
        &mut peers[1].node,
        None,
        Request::AgentEnroll {
            name: "second".into(),
            grants: Grants { manage_goals: true },
            credential: Credential([5; 32]).digest(),
        },
    ) else {
        panic!("enroll");
    };
    let first = peers[1].principal.min(second);
    let second = peers[1].principal.max(second);
    let Response::GoalCreated { goal } = peers[0].call(Request::GoalCreate {
        title: "independent invitations".into(),
        blueprint_json: None,
        roles: BTreeMap::new(),
        inputs: BTreeMap::new(),
    }) else {
        panic!("create");
    };
    let Response::Invited { ticket: expired } = peers[0].call(Request::GoalInvite {
        goal,
        expires_ms: Some(100),
    }) else {
        panic!("invite");
    };
    let Response::Invited { ticket: valid } = peers[0].call(Request::GoalInvite {
        goal,
        expires_ms: None,
    }) else {
        panic!("invite");
    };
    request(
        &mut peers[1].node,
        Some(first),
        Request::GoalJoin { ticket: expired },
    );
    request(
        &mut peers[1].node,
        Some(second),
        Request::GoalJoin { ticket: valid },
    );
    reconcile(&mut peers, 1000);
    let joins = &peers[1].node.goals[&goal].local.joins;
    assert!(joins[&first].refused);
    assert!(!joins[&second].refused);
    reconcile(&mut peers, 35000);
    assert!(peers[1].node.goals[&goal].is_member(&second));
    assert!(!peers[1].node.goals[&goal].is_member(&first));
}

#[test]
fn peer_connection_status_is_ephemeral_but_last_sync_is_durable() {
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let remote = peers[1].endpoint;
    assert_eq!(peers[0].node.peer_view(&remote).last_sync_ms, Some(2000));
    assert!(!peers[0].node.peer_view(&remote).connected);
    peers[0].node.peer(
        PeerInput::Connection {
            endpoint: remote,
            connected: true,
        },
        locust_proto::engine::PeerTime {
            unix_ms: 3000,
            elapsed_ms: 3000,
        },
        &mut vec![],
    );
    assert!(peers[0].node.peer_view(&remote).connected);
    assert!(peers[0].node.reachable(&goal));
    peers[0].node.peer(
        PeerInput::Connection {
            endpoint: remote,
            connected: false,
        },
        locust_proto::engine::PeerTime {
            unix_ms: 3001,
            elapsed_ms: 3001,
        },
        &mut vec![],
    );
    assert!(!peers[0].node.peer_view(&remote).connected);
    assert!(!peers[0].node.reachable(&goal));
    peers[0].restart();
    assert_eq!(peers[0].node.peer_view(&remote).last_sync_ms, Some(2000));
    assert!(!peers[0].node.peer_view(&remote).connected);
}

#[test]
fn pending_join_has_exactly_one_status_entry_per_local_principal() {
    let mut peers = [Peer::new(1), Peer::new(2)];
    let Response::GoalCreated { goal } = peers[0].call(Request::GoalCreate {
        title: "status cardinality".into(),
        blueprint_json: None,
        roles: BTreeMap::new(),
        inputs: BTreeMap::new(),
    }) else {
        panic!("create");
    };
    let Response::Invited { ticket } = peers[0].call(Request::GoalInvite {
        goal,
        expires_ms: None,
    }) else {
        panic!("invite");
    };
    peers[1].call(Request::GoalJoin { ticket });
    let Response::Status(status) = peers[1].call(Request::Status) else {
        panic!("status");
    };
    assert_eq!(status.goals.len(), 1);
    assert_eq!(status.goals[0].goal, goal);
}

#[test]
fn signed_payload_epoch_must_match_the_sealed_object() {
    use crate::sync::{Host, Staged};
    use locust_proto::event::{Event, Header, PayloadRef};
    use locust_proto::seal;
    use locust_proto::store::Blob;
    let mut peers = [Peer::new(1), Peer::new(2), Peer::new(3)];
    let goal = found(&mut peers);
    let removed = peers[2].principal;
    peers[0].call(Request::MemberRemove {
        goal,
        member: removed,
    });
    reconcile(&mut peers, 35000);
    let author = peers[0].principal;
    let source = &peers[0].node;
    let entry = &source.goals[&goal];
    let next = entry.goal.next(&author).unwrap();
    assert_eq!(next.epoch, 1);
    let sealed = seal::seal(&goal, 0, &entry.keys[&0], b"stale encryption").unwrap();
    let blob = Blob::new(sealed);
    let hash = blob.hash();
    let event = Event::sign(
        Header {
            version: locust_proto::PROTOCOL_VERSION,
            goal,
            author,
            seq: next.seq,
            prev: next.prev,
            anchor: Some(next.anchor),
            parents: vec![],
            at_ms: 36000,
            payload: Some(PayloadRef {
                hash,
                len: blob.bytes().len() as u32,
                key_epoch: 1,
            }),
            body: standalone(Context {
                scope: Scope::Goal,
                round: entry.state().current_rules.unwrap(),
            }),
        },
        source.signer(&author).unwrap(),
    )
    .unwrap();
    let replica = Host::replica(&mut peers[1].node, &goal).unwrap();
    assert_eq!(replica.receive(vec![event.to_wire()]), Ok(1));
    // A prefix may arrive in pieces across daemon restarts. It still has to
    // match the signed reference before the complete object becomes held.
    assert_eq!(
        replica.stage(&hash, 0, blob.bytes().len() as u64, &blob.bytes()[..2]),
        Staged::More(2)
    );
    peers[1].restart();
    let replica = Host::replica(&mut peers[1].node, &goal).unwrap();
    assert_eq!(
        replica.stage(&hash, 2, blob.bytes().len() as u64, &blob.bytes()[2..]),
        Staged::Rejected
    );
    assert!(peers[1].store.blob_len(&hash).unwrap().is_none());
    peers[1].restart();
    assert_eq!(peers[1].store.staged_len(&hash).unwrap(), 0);
}

fn reference_event(
    peer: &Peer,
    goal: GoalId,
    payload: Option<locust_proto::event::PayloadRef>,
    mut body: locust_proto::event::Body,
) -> locust_proto::event::Event {
    use locust_proto::event::{Event, Header};
    let rules = peer.node.goals[&goal].state().current_rules.unwrap();
    match &mut body {
        Body::ContributionPublished { context, .. }
            if context.round == locust_proto::id::EventId([0; 32]) =>
        {
            context.round = rules
        }
        Body::TaskOpened { binding } if binding.rules == locust_proto::id::EventId([0; 32]) => {
            binding.rules = rules
        }
        _ => {}
    }
    let next = peer.node.goals[&goal].goal.next(&peer.principal).unwrap();
    Event::sign(
        Header {
            version: locust_proto::PROTOCOL_VERSION,
            goal,
            author: peer.principal,
            seq: next.seq,
            prev: next.prev,
            anchor: Some(next.anchor),
            parents: vec![],
            at_ms: 36000,
            payload,
            body,
        },
        peer.node.signer(&peer.principal).unwrap(),
    )
    .unwrap()
}

#[test]
fn matching_reference_allows_short_chunks_despite_a_conflicting_reference() {
    use crate::sync::{Host, Staged};
    use locust_proto::event::PayloadRef;
    use locust_proto::{seal, store::Blob};
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let blob = Blob::new(
        seal::seal(
            &goal,
            0,
            &peers[0].node.goals[&goal].keys[&0],
            b"valid bytes",
        )
        .unwrap(),
    );
    let hash = blob.hash();
    let total = blob.bytes().len() as u64;
    for len in [total as u32 + 1, total as u32] {
        let event = reference_event(
            &peers[0],
            goal,
            Some(PayloadRef {
                hash,
                len,
                key_epoch: 0,
            }),
            standalone(Context {
                scope: Scope::Goal,
                round: locust_proto::id::EventId([0; 32]),
            }),
        );
        for peer in &mut peers {
            assert_eq!(
                Host::replica(&mut peer.node, &goal)
                    .unwrap()
                    .receive(vec![event.to_wire()]),
                Ok(1)
            );
        }
        if len != total as u32 {
            assert_eq!(
                Host::replica(&mut peers[1].node, &goal).unwrap().stage(
                    &hash,
                    0,
                    total,
                    blob.bytes()
                ),
                Staged::Rejected
            );
            assert!(peers[1].store.blob_len(&hash).unwrap().is_none());
        }
    }
    for (offset, byte) in blob.bytes().iter().enumerate() {
        let expected = if offset + 1 == blob.bytes().len() {
            Staged::Complete
        } else {
            Staged::More(offset as u64 + 1)
        };
        assert_eq!(
            Host::replica(&mut peers[1].node, &goal).unwrap().stage(
                &hash,
                offset as u64,
                total,
                &[*byte]
            ),
            expected
        );
        if offset == 1 {
            peers[1].restart();
        }
    }
    peers[1].restart();
    assert_eq!(
        Host::replica(&mut peers[1].node, &goal)
            .unwrap()
            .blob_len(&hash),
        Some(total)
    );
    assert_eq!(
        peers[1].store.blob(&hash).unwrap().as_deref(),
        Some(blob.bytes())
    );
}

#[test]
fn bare_object_reference_refuses_an_epoch_after_its_event() {
    use crate::sync::{Host, Staged};
    use locust_proto::event::Body;
    use locust_proto::{seal, store::Blob};
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let blob = Blob::new(
        seal::seal(
            &goal,
            1,
            &peers[0].node.goals[&goal].keys[&0],
            b"future epoch",
        )
        .unwrap(),
    );
    let hash = blob.hash();
    let event = reference_event(
        &peers[0],
        goal,
        None,
        Body::TaskOpened {
            binding: locust_proto::event::TaskBinding {
                rules: locust_proto::id::EventId([0; 32]),
                task_type: None,
                inputs: BTreeMap::from([("snapshot".into(), hash)]),
                parent: None,
                stage: None,
            },
        },
    );
    let replica = Host::replica(&mut peers[1].node, &goal).unwrap();
    assert_eq!(replica.receive(vec![event.to_wire()]), Ok(1));
    assert_eq!(
        replica.stage(&hash, 0, blob.bytes().len() as u64, blob.bytes()),
        Staged::Rejected
    );
    assert!(peers[1].store.blob_len(&hash).unwrap().is_none());
}

#[test]
fn inflated_advertisement_is_rejected_and_corrupted_oversized_stage_recovers_from_zero() {
    use crate::sync::{Host, Staged};
    use locust_proto::event::PayloadRef;
    use locust_proto::sync::SyncMessage;
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let Response::BlobStored { hash } = peers[0].call(Request::BlobPut {
        goal,
        bytes: b"small".to_vec(),
    }) else {
        panic!("put")
    };
    let sealed = peers[0].store.blob(&hash).unwrap().unwrap();
    let event = reference_event(
        &peers[0],
        goal,
        Some(PayloadRef {
            hash,
            len: sealed.len() as u32,
            key_epoch: 0,
        }),
        standalone(Context {
            scope: Scope::Goal,
            round: locust_proto::id::EventId([0; 32]),
        }),
    );
    for peer in &mut peers {
        Host::replica(&mut peer.node, &goal)
            .unwrap()
            .receive(vec![event.to_wire()])
            .unwrap();
    }
    let replica = Host::replica(&mut peers[1].node, &goal).unwrap();
    assert_eq!(
        replica.stage(&hash, 0, 1_000_000, &sealed),
        Staged::Rejected
    );
    assert_eq!(peers[1].store.staged_len(&hash).unwrap(), 0);
    // Simulate an oversized corrupted partial object, then reopen.
    peers[1]
        .store
        .stage_blob(&hash, 0, &vec![9; 100_000])
        .unwrap();
    peers[1].restart();
    let frames = reconcile(&mut peers, 35_000);
    for offset in [100_000, 0] {
        assert!(frames.iter().any(|frame| matches!(frame, SyncMessage::BlobRequest { hash: asked, offset: at } if *asked == hash && *at == offset)));
    }
    assert_eq!(peers[1].store.blob(&hash).unwrap(), Some(sealed));
    assert_eq!(peers[1].store.staged_len(&hash).unwrap(), 0);
}

#[test]
fn unavailable_peer_and_incomplete_conflicting_retry_preserve_shared_staging() {
    use crate::sync::{Host, Staged};
    use locust_proto::event::PayloadRef;
    use locust_proto::sync::SyncMessage;
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    let Response::BlobStored { hash } = peers[0].call(Request::BlobPut {
        goal,
        bytes: vec![7; 200],
    }) else {
        panic!("put")
    };
    let sealed = peers[0].store.blob(&hash).unwrap().unwrap();
    let event = reference_event(
        &peers[0],
        goal,
        Some(PayloadRef {
            hash,
            len: sealed.len() as u32,
            key_epoch: 0,
        }),
        standalone(Context {
            scope: Scope::Goal,
            round: locust_proto::id::EventId([0; 32]),
        }),
    );
    for peer in &mut peers {
        Host::replica(&mut peer.node, &goal)
            .unwrap()
            .receive(vec![event.to_wire()])
            .unwrap();
    }
    let replica = Host::replica(&mut peers[1].node, &goal).unwrap();
    assert_eq!(
        replica.stage(&hash, 0, sealed.len() as u64, &sealed[..100]),
        Staged::More(100)
    );
    assert_eq!(
        replica.stage(&hash, 0, sealed.len() as u64, &[0; 2]),
        Staged::Rejected
    );
    peers[0].call(Request::BlobWithdraw { goal, hash });
    let frames = reconcile(&mut peers, 35_000);
    assert!(
        frames
            .iter()
            .filter(|frame| matches!(frame, SyncMessage::BlobUnavailable(given) if *given == hash))
            .count()
            >= 2
    );
    assert_eq!(
        peers[1].store.staged_range(&hash, 0, 100).unwrap(),
        Some(sealed[..100].to_vec())
    );
}

#[test]
fn wanted_cursor_is_sorted_and_updates_after_commits_completion_and_reopen() {
    use crate::sync::Host;
    let mut peers = [Peer::new(1), Peer::new(2)];
    let goal = found(&mut peers);
    for n in 0..100 {
        peers[0].call(Request::ContributionPublish {
            goal,
            task: None,
            attempt: None,
            generation: None,
            base: None,
            patch: None,
            artifacts: vec![],
            summary: format!("missing {n}"),
        });
    }
    let wires = peers[0]
        .store
        .log(&goal, 0, usize::MAX)
        .unwrap()
        .into_iter()
        .map(|(_, event)| event.to_wire())
        .collect();
    Host::replica(&mut peers[1].node, &goal)
        .unwrap()
        .receive(wires)
        .unwrap();
    let wanted = Host::replica(&mut peers[1].node, &goal)
        .unwrap()
        .wanted_blobs(usize::MAX);
    assert_eq!(wanted.len(), 100);
    assert!(wanted.windows(2).all(|pair| pair[0].0 < pair[1].0));
    peers[1].restart();
    assert_eq!(
        Host::replica(&mut peers[1].node, &goal)
            .unwrap()
            .wanted_blobs(usize::MAX),
        wanted
    );
    reconcile(&mut peers, 35_000);
    assert!(
        Host::replica(&mut peers[1].node, &goal)
            .unwrap()
            .wanted_blobs(usize::MAX)
            .is_empty()
    );
    peers[1].restart();
    assert!(
        Host::replica(&mut peers[1].node, &goal)
            .unwrap()
            .wanted_blobs(usize::MAX)
            .is_empty()
    );
}

#[test]
fn administrator_halt_reaches_a_historical_contact_without_history_or_key_admission() {
    use crate::sync::Host;
    use locust_proto::event::{Body, Event};
    use locust_proto::sync::SyncMessage;
    let mut peers = [Peer::new(1), Peer::new(2), Peer::new(3)];
    let goal = found(&mut peers);
    let admission = peers[0]
        .store
        .log(&goal, 0, usize::MAX)
        .unwrap()
        .into_iter()
        .map(|(_, event)| event)
        .find(|event| matches!(event.header().body, Body::MemberAdmitted { .. }))
        .unwrap();
    let mut header = admission.header().clone();
    header.at_ms += 1;
    let fork = Event::sign(header, peers[0].node.signer(&peers[0].principal).unwrap()).unwrap();
    for peer in &mut peers[..2] {
        Host::replica(&mut peer.node, &goal)
            .unwrap()
            .receive(vec![fork.to_wire()])
            .unwrap();
    }
    let frames = reconcile_from(&mut peers, 35_000, &[0, 1]);
    assert!(
        frames
            .iter()
            .any(|frame| matches!(frame, SyncMessage::HaltProof(_)))
    );
    assert!(
        frames.iter().all(|frame| matches!(
            frame,
            SyncMessage::Hello { .. }
                | SyncMessage::HaltProof(_)
                | SyncMessage::Done
                | SyncMessage::Refused(_)
        )),
        "proof exchange disclosed ordinary data: {frames:?}"
    );
    for peer in &mut peers {
        assert!(
            peer.node.goals[&goal]
                .goal
                .evaluation()
                .admin_halt
                .is_some()
        );
        peer.restart();
        assert!(
            peer.node.goals[&goal]
                .goal
                .evaluation()
                .admin_halt
                .is_some()
        );
    }
    let before = peers[1].node.goals[&goal].revision();
    let remote = peers[0].endpoint;
    Host::receive_halt_proof(
        &mut peers[1].node,
        &goal,
        &remote,
        [admission.to_wire(), fork.to_wire()],
    )
    .unwrap();
    assert_eq!(
        peers[1].node.goals[&goal].revision(),
        before,
        "duplicate proof must not wake waits or schedule another broadcast"
    );
    assert!(Host::take_changed(&mut peers[1].node).is_empty());
}

#[path = "content_graph_tests.rs"]
mod content_graph;

#[test]
fn joining_fetches_founding_text_and_key_before_bulk_history_content() {
    use locust_proto::event::Body;
    use locust_proto::sync::SyncMessage;
    let mut peers = [Peer::new(1), Peer::new(2)];
    let Response::GoalCreated { goal } = peers[0].call(Request::GoalCreate {
        title: "early readable title".into(),
        blueprint_json: None,
        roles: BTreeMap::new(),
        inputs: BTreeMap::new(),
    }) else {
        panic!("create")
    };
    for index in 0..40 {
        peers[0].call(Request::ContributionPublish {
            goal,
            task: None,
            attempt: None,
            generation: None,
            base: None,
            patch: None,
            artifacts: vec![],
            summary: format!("history {index}"),
        });
    }
    let genesis = peers[0]
        .store
        .log(&goal, 0, usize::MAX)
        .unwrap()
        .into_iter()
        .find_map(|(_, event)| {
            matches!(event.header().body, Body::Genesis(_))
                .then_some(event.header().payload.unwrap().hash)
        })
        .unwrap();
    let Response::Invited { ticket } = peers[0].call(Request::GoalInvite {
        goal,
        expires_ms: None,
    }) else {
        panic!("invite")
    };
    peers[1].call(Request::GoalJoin { ticket });
    let sent = reconcile_from(&mut peers, 1000, &[1]);
    let requests: Vec<_> = sent
        .iter()
        .filter(|frame| {
            matches!(
                frame,
                SyncMessage::BlobRequest { .. } | SyncMessage::KeyRequest { .. }
            )
        })
        .collect();
    assert!(matches!(requests[0], SyncMessage::BlobRequest { hash, .. } if *hash == genesis));
    assert!(matches!(requests[1], SyncMessage::KeyRequest { epoch: 0 }));
    assert!(requests.len() > 40);
    let Response::GoalStatus(status) = peers[1].call(Request::GoalStatus { goal }) else {
        panic!("status")
    };
    assert_eq!(status.title.as_deref(), Some("early readable title"));
}

#[test]
fn an_offline_removed_endpoint_is_refused_after_restart_without_learning_new_history() {
    use crate::sync::Host;
    use locust_proto::sync::{Refusal, SyncMessage};
    let mut peers = [Peer::new(1), Peer::new(2), Peer::new(3)];
    let goal = found(&mut peers);
    let removed = peers[2].principal;
    let held_before = peers[2].store.log(&goal, 0, usize::MAX).unwrap().len();
    peers[2].online = false;
    peers[0].call(Request::MemberRemove {
        goal,
        member: removed,
    });
    peers[0].call(Request::ContributionPublish {
        goal,
        task: None,
        attempt: None,
        generation: None,
        base: None,
        patch: None,
        artifacts: vec![],
        summary: "only current members may read this".into(),
    });
    reconcile(&mut peers, 35_000);
    for peer in &mut peers[..2] {
        assert!(!peer.node.goals[&goal].is_member(&removed));
        assert!(
            Host::replica(&mut peer.node, &goal)
                .unwrap()
                .key(1)
                .is_some()
        );
    }
    peers[2].restart();
    peers[2].online = true;
    let frames = reconcile_from(&mut peers, 70_000, &[2]);
    assert!(
        frames
            .iter()
            .any(|frame| matches!(frame, SyncMessage::Refused(Refusal::NotAMember)))
    );
    assert_eq!(
        peers[2].store.log(&goal, 0, usize::MAX).unwrap().len(),
        held_before
    );
    assert!(
        Host::replica(&mut peers[2].node, &goal)
            .unwrap()
            .key(1)
            .is_none()
    );
    let Response::Contributions(notes) = peers[2].call(Request::Contributions { goal, task: None })
    else {
        panic!("notes")
    };
    assert!(notes.is_empty());
    // Status describes held state. A transport refusal is not a signed
    // removal decision and cannot rewrite the offline replica's projection.
    assert!(peers[2].node.goals[&goal].is_member(&removed));
}

fn standalone(context: Context) -> Body {
    Body::ContributionPublished {
        context,
        attempt: None,
        base: None,
        patch: None,
        artifacts: vec![],
    }
}
