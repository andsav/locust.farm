//! Public content requests and invitation issuance.
use super::lifecycle::{event, setup};
use super::*;
use locust_proto::api::{BlobState, Membership};
use locust_proto::engine::{PeerEngine, PeerInput};
use locust_proto::event::{Context, Doc, ReviewVerdict, Scope, TaskBinding, TaskId};
use locust_proto::id::EventId;
use locust_proto::id::{BlobHash, EndpointId};
use locust_proto::invite::Invitation;
use locust_proto::store::{Space, Store};

#[test]
fn stale_document_revision_remains_inspectable_after_accept_withdraw_and_restart() {
    let (mut daemon, _, _, administrator, goal) = setup();
    let (_, member) = super::authorization::join_local(&mut daemon, administrator, goal, 2);
    let first = event(daemon.ok(
        administrator,
        Request::DocRevise {
            goal,
            doc: Doc::Plan,
            base: None,
            text: "accepted plan".into(),
        },
    ));
    let competing = event(daemon.ok(
        member,
        Request::DocRevise {
            goal,
            doc: Doc::Plan,
            base: None,
            text: "competing plan".into(),
        },
    ));
    daemon.ok(
        administrator,
        Request::ReviewRecord {
            goal,
            subject: first,
            verdict: ReviewVerdict::Approve,
            text: "reviewed".into(),
        },
    );
    daemon.ok(
        administrator,
        Request::ScopeSelect {
            goal,
            subject: first,
            expected: None,
        },
    );
    assert_eq!(
        code(daemon.call(
            administrator,
            Request::ScopeSelect {
                goal,
                subject: competing,
                expected: None
            }
        )),
        ErrorCode::Conflict
    );
    let competing_wire = daemon.store.event(&competing).unwrap().unwrap();
    let hash = competing_wire.header().payload.unwrap().hash;
    let sealed = daemon.store.blob(&hash).unwrap().unwrap();
    daemon.ok(member, Request::BlobWithdraw { goal, hash });
    daemon.ok(member, Request::GoalLeave { goal });
    daemon.restart();
    let administrator = daemon.connect(credential(1), None);
    let member = daemon.connect(credential(2), None);
    assert_eq!(
        daemon.store.event(&competing).unwrap(),
        Some(competing_wire)
    );
    assert_eq!(daemon.store.blob(&hash).unwrap(), Some(sealed));
    let Response::Event(detail) = daemon.ok(
        administrator,
        Request::Event {
            goal,
            event: competing,
        },
    ) else {
        panic!("expected retained competing revision");
    };
    assert_eq!(detail.text, None);
    let Response::Doc(view) = daemon.ok(
        administrator,
        Request::DocRead {
            goal,
            doc: Doc::Plan,
        },
    ) else {
        panic!("expected accepted document");
    };
    assert_eq!(view.selected, Some(first));
    assert_eq!(view.text.as_deref(), Some("accepted plan"));
    assert_eq!(
        code(daemon.call(
            member,
            Request::DocRevise {
                goal,
                doc: Doc::Plan,
                base: Some(first),
                text: "left member".into(),
            }
        )),
        ErrorCode::Denied
    );
    assert_eq!(
        code(daemon.call(
            administrator,
            Request::ScopeSelect {
                goal,
                subject: competing,
                expected: None
            }
        )),
        ErrorCode::Conflict
    );
}

#[test]
fn content_put_get_scope_withdrawal_and_reput_survive_restart() {
    let (mut daemon, _, _, agent, goal) = setup();
    let bytes = b"private local output".to_vec();
    let Response::BlobStored { hash } = daemon.ok(
        agent,
        Request::BlobPut {
            goal,
            bytes: bytes.clone(),
        },
    ) else {
        panic!()
    };
    assert_eq!(
        daemon.ok(agent, Request::BlobGet { goal, hash }),
        Response::Blob {
            bytes: bytes.clone()
        }
    );
    assert_ne!(daemon.store.blob(&hash).unwrap().unwrap(), bytes);
    let Response::GoalCreated { goal: other } = daemon.ok(
        agent,
        Request::GoalCreate {
            title: "Other".into(),
            blueprint_json: None,
            roles: Default::default(),
            inputs: Default::default(),
        },
    ) else {
        panic!()
    };
    assert_eq!(
        code(daemon.call(agent, Request::BlobGet { goal: other, hash })),
        ErrorCode::NotFound
    );
    let Response::BlobStates(states) = daemon.ok(
        agent,
        Request::BlobStat {
            goal: other,
            hashes: vec![hash],
        },
    ) else {
        panic!()
    };
    assert_eq!(states[0].state, BlobState::Unknown);
    daemon.ok(agent, Request::BlobWithdraw { goal, hash });
    assert_eq!(
        code(daemon.call(agent, Request::BlobGet { goal, hash })),
        ErrorCode::Unavailable
    );
    daemon.restart();
    let agent = daemon.connect(credential(1), None);
    assert_eq!(
        code(daemon.call(agent, Request::BlobGet { goal, hash })),
        ErrorCode::Unavailable
    );
    let Response::BlobStates(states) = daemon.ok(
        agent,
        Request::BlobStat {
            goal,
            hashes: vec![hash],
        },
    ) else {
        panic!()
    };
    assert_eq!(states[0].state, BlobState::Unavailable);
    assert_eq!(
        daemon.ok(
            agent,
            Request::BlobPut {
                goal,
                bytes: bytes.clone()
            }
        ),
        Response::BlobStored { hash }
    );
    assert_eq!(
        daemon.ok(agent, Request::BlobGet { goal, hash }),
        Response::Blob { bytes }
    );
}

#[test]
fn missing_content_read_records_want_only_for_nonviewers() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    let hash = BlobHash([91; 32]);
    daemon.ok(
        agent,
        Request::TaskOpen {
            goal,
            text: "needs input".into(),
            task_type: None,
            parent: None,
            inputs: std::collections::BTreeMap::from([("workspace".into(), hash)]),
        },
    );
    daemon.ok(
        owner,
        Request::ViewerEnroll {
            agent: principal,
            credential: credential(9).digest(),
        },
    );
    let viewer = daemon.connect(credential(9), None);
    let before = daemon.store.scan(Space::Blob, &[]).unwrap();
    assert_eq!(
        code(daemon.call(viewer, Request::BlobGet { goal, hash })),
        ErrorCode::Unavailable
    );
    assert_eq!(daemon.store.scan(Space::Blob, &[]).unwrap(), before);
    assert_eq!(
        code(daemon.call(agent, Request::BlobGet { goal, hash })),
        ErrorCode::Unavailable
    );
    assert_eq!(
        daemon.store.scan(Space::Blob, &[]).unwrap().len(),
        before.len() + 1
    );
    let Response::BlobStates(states) = daemon.ok(
        agent,
        Request::BlobStat {
            goal,
            hashes: vec![hash],
        },
    ) else {
        panic!()
    };
    assert_eq!(states[0].state, BlobState::Requested);
    let after = daemon.store.scan(Space::Blob, &[]).unwrap();
    assert_eq!(
        code(daemon.call(agent, Request::BlobGet { goal, hash })),
        ErrorCode::Unavailable
    );
    assert_eq!(daemon.store.scan(Space::Blob, &[]).unwrap(), after);
    assert_eq!(
        code(daemon.call(
            agent,
            Request::BlobGet {
                goal,
                hash: BlobHash([92; 32])
            }
        )),
        ErrorCode::NotFound
    );
}

#[test]
fn withdrawn_shared_payload_disappears_from_all_text_views() {
    let (mut daemon, _, _, agent, goal) = setup();
    let text = "A test goal".to_owned();
    let task = event(daemon.ok(
        agent,
        Request::TaskOpen {
            goal,
            text: text.clone(),
            task_type: None,
            parent: None,
            inputs: Default::default(),
        },
    ));
    daemon.ok(
        agent,
        Request::ContributionPublish {
            goal,
            task: None,
            attempt: None,
            generation: None,
            summary: text.clone(),
            base: None,
            patch: None,
            artifacts: vec![],
        },
    );
    let revision = event(daemon.ok(
        agent,
        Request::DocRevise {
            goal,
            doc: Doc::Plan,
            base: None,
            text: text.clone(),
        },
    ));
    daemon.ok(
        agent,
        Request::ReviewRecord {
            goal,
            subject: revision,
            verdict: ReviewVerdict::Approve,
            text: "reviewed".into(),
        },
    );
    daemon.ok(
        agent,
        Request::ScopeSelect {
            goal,
            subject: revision,
            expected: None,
        },
    );
    let payload = daemon
        .store
        .event(&task)
        .unwrap()
        .unwrap()
        .header()
        .payload
        .unwrap();
    daemon.ok(
        agent,
        Request::BlobWithdraw {
            goal,
            hash: payload.hash,
        },
    );
    let Response::Task(detail) = daemon.ok(
        agent,
        Request::Task {
            goal,
            task: TaskId::Authored(task),
        },
    ) else {
        panic!()
    };
    assert_eq!(detail.text, None);
    assert_eq!(detail.view.title, None);
    let Response::Event(detail) = daemon.ok(agent, Request::Event { goal, event: task }) else {
        panic!()
    };
    assert_eq!(detail.text, None);
    let Response::Contributions(notes) =
        daemon.ok(agent, Request::Contributions { goal, task: None })
    else {
        panic!()
    };
    assert_eq!(notes[0].text, None);
    let Response::Doc(doc) = daemon.ok(
        agent,
        Request::DocRead {
            goal,
            doc: Doc::Plan,
        },
    ) else {
        panic!()
    };
    assert_eq!(doc.text, None);
    let Response::GoalStatus(status) = daemon.ok(agent, Request::GoalStatus { goal }) else {
        panic!()
    };
    assert_eq!(status.title, None);
    daemon.ok(
        agent,
        Request::BlobPut {
            goal,
            bytes: text.into_bytes(),
        },
    );
    let Response::Task(detail) = daemon.ok(
        agent,
        Request::Task {
            goal,
            task: TaskId::Authored(task),
        },
    ) else {
        panic!()
    };
    assert_eq!(detail.text.as_deref(), Some("A test goal"));
}

#[test]
fn invitation_issuer_stores_digest_and_repeated_local_join_is_read_only() {
    let (mut daemon, principal, _, agent, goal) = setup();
    let Response::Invited { ticket } = daemon.ok(
        agent,
        Request::GoalInvite {
            goal,
            expires_ms: Some(5000),
        },
    ) else {
        panic!()
    };
    let invitation = Invitation::from_ticket(ticket.as_str()).unwrap();
    assert_eq!(invitation.goal, goal);
    assert_eq!(invitation.administrator, principal);
    let rows = daemon.store.scan(Space::Invite, &[]).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, invitation.secret.digest());
    assert!(
        !rows[0]
            .1
            .windows(32)
            .any(|part| part == invitation.secret.0)
    );
    let count = daemon.store.log(&goal, 0, 256).unwrap().len();
    let Response::Joined { membership, .. } = daemon.ok(agent, Request::GoalJoin { ticket }) else {
        panic!()
    };
    assert_eq!(membership, Membership::Member);
    assert_eq!(daemon.store.log(&goal, 0, 256).unwrap().len(), count);
}

#[test]
fn pending_join_cannot_relabel_the_administrator() {
    let (mut issuer, administrator, _, agent, goal) = setup();
    let Response::Invited { ticket } = issuer.ok(
        agent,
        Request::GoalInvite {
            goal,
            expires_ms: None,
        },
    ) else {
        panic!()
    };
    let mut joining = Daemon::new(22);
    joining.node.peer(
        PeerInput::Endpoint {
            endpoint: EndpointId([22; 32]),
            hints: vec![],
        },
        locust_proto::engine::PeerTime {
            unix_ms: 0,
            elapsed_ms: 0,
        },
        &mut Vec::new(),
    );
    joining.enroll("joining", 2, true);
    let actor = joining.connect(credential(2), None);
    joining.ok(
        actor,
        Request::GoalJoin {
            ticket: ticket.clone(),
        },
    );
    let mut altered = Invitation::from_ticket(ticket.as_str()).unwrap();
    altered.administrator = PublicKey([99; 32]);
    assert_eq!(
        code(joining.call(
            actor,
            Request::GoalJoin {
                ticket: altered.to_ticket().unwrap(),
            }
        )),
        ErrorCode::Conflict
    );
    assert_eq!(
        joining.ok(actor, Request::GoalJoin { ticket }),
        Response::Joined {
            goal,
            administrator,
            membership: Membership::Joining,
        }
    );
}

#[test]
fn sealed_bytes_without_their_key_are_not_reported_readable() {
    let (mut daemon, _, _, agent, goal) = setup();
    let Response::BlobStored { hash } = daemon.ok(
        agent,
        Request::BlobPut {
            goal,
            bytes: b"awaiting its epoch key".to_vec(),
        },
    ) else {
        panic!()
    };
    // A replica may receive the encrypted object before its epoch key.
    daemon.node.goals.get_mut(&goal).unwrap().keys.clear();
    assert_eq!(
        code(daemon.call(agent, Request::BlobGet { goal, hash })),
        ErrorCode::Unavailable
    );
    let Response::BlobStates(states) = daemon.ok(
        agent,
        Request::BlobStat {
            goal,
            hashes: vec![hash],
        },
    ) else {
        panic!()
    };
    assert_ne!(states[0].state, BlobState::Held);
}

/// Accept an authentic peer event while its content is already held locally.
fn receive_payload(
    daemon: &mut Daemon,
    goal: locust_proto::id::GoalId,
    author: PublicKey,
    payload: locust_proto::event::PayloadRef,
    mut body: locust_proto::event::Body,
) -> locust_proto::id::EventId {
    use crate::sync::Host;
    use locust_proto::event::{Event, Header};
    let entry = &daemon.node.goals[&goal];
    match &mut body {
        locust_proto::event::Body::TaskOpened { binding } => {
            binding.rules = entry.state().current_rules.unwrap()
        }
        locust_proto::event::Body::ContributionPublished { context, .. } => {
            context.round = entry.state().current_rules.unwrap()
        }
        _ => (),
    }
    let next = entry.goal.next(&author).unwrap();
    let event = Event::sign(
        Header {
            version: locust_proto::PROTOCOL_VERSION,
            goal,
            author,
            seq: next.seq,
            prev: next.prev,
            anchor: Some(next.anchor),
            parents: vec![],
            at_ms: 1000,
            payload: Some(payload),
            body,
        },
        daemon.node.signer(&author).unwrap(),
    )
    .unwrap();
    assert_eq!(
        Host::replica(&mut daemon.node, &goal)
            .unwrap()
            .receive(vec![event.to_wire()]),
        Ok(1)
    );
    event.id()
}

#[test]
fn held_content_requires_matching_references_and_each_text_view_checks_its_own_reference() {
    use locust_proto::event::{Body, PayloadRef};
    let (mut daemon, principal, _, agent, goal) = setup();
    let text = b"Authentic bytes with a dishonest length".to_vec();
    let Response::BlobStored { hash } = daemon.ok(
        agent,
        Request::BlobPut {
            goal,
            bytes: text.clone(),
        },
    ) else {
        panic!()
    };
    let payload = PayloadRef {
        hash,
        len: daemon.store.blob_len(&hash).unwrap().unwrap() as u32,
        key_epoch: 0,
    };
    let task = receive_payload(
        &mut daemon,
        goal,
        principal,
        PayloadRef {
            len: payload.len + 1,
            ..payload
        },
        Body::TaskOpened {
            binding: TaskBinding {
                rules: EventId([0; 32]),
                task_type: None,
                inputs: Default::default(),
                parent: None,
                stage: None,
            },
        },
    );
    assert_eq!(
        code(daemon.call(agent, Request::BlobGet { goal, hash })),
        ErrorCode::Unavailable
    );
    let Response::BlobStates(states) = daemon.ok(
        agent,
        Request::BlobStat {
            goal,
            hashes: vec![hash],
        },
    ) else {
        panic!()
    };
    assert_ne!(states[0].state, BlobState::Held);
    let note = receive_payload(
        &mut daemon,
        goal,
        principal,
        payload,
        Body::ContributionPublished {
            context: Context {
                scope: Scope::Goal,
                round: EventId([0; 32]),
            },
            attempt: None,
            base: None,
            patch: None,
            artifacts: vec![],
        },
    );
    // Another matching reference admits the object, but not the task's text.
    for restart in [false, true] {
        let reader = if restart {
            daemon.restart();
            daemon.connect(credential(1), None)
        } else {
            agent
        };
        assert_eq!(
            daemon.ok(reader, Request::BlobGet { goal, hash }),
            Response::Blob {
                bytes: text.clone()
            }
        );
        let Response::BlobStates(states) = daemon.ok(
            reader,
            Request::BlobStat {
                goal,
                hashes: vec![hash],
            },
        ) else {
            panic!()
        };
        assert_eq!(states[0].state, BlobState::Held);
        let Response::Task(detail) = daemon.ok(
            reader,
            Request::Task {
                goal,
                task: TaskId::Authored(task),
            },
        ) else {
            panic!()
        };
        assert_eq!(detail.text, None);
        assert_eq!(detail.view.title, None);
        let Response::Event(detail) = daemon.ok(reader, Request::Event { goal, event: task })
        else {
            panic!()
        };
        assert_eq!(detail.text, None);
        let Response::Event(detail) = daemon.ok(reader, Request::Event { goal, event: note })
        else {
            panic!()
        };
        assert_eq!(detail.text.as_deref(), std::str::from_utf8(&text).ok());
    }
}

#[test]
fn held_content_epoch_must_match_the_signed_reference_even_with_a_decryption_key() {
    use locust_proto::event::{Body, PayloadRef};
    use locust_proto::seal;
    use locust_proto::store::{Blob, Commit};
    let (mut daemon, principal, _, agent, goal) = setup();
    let key = daemon.node.goals[&goal].keys[&0];
    let blob = Blob::new(seal::seal(&goal, 1, &key, b"Wrong sealed epoch").unwrap());
    let payload = PayloadRef {
        hash: blob.hash(),
        len: blob.bytes().len() as u32,
        key_epoch: 0,
    };
    // Isolate the signed metadata check from key availability/authentication.
    daemon
        .node
        .goals
        .get_mut(&goal)
        .unwrap()
        .keys
        .insert(1, key);
    daemon
        .store
        .commit(&Commit {
            blobs: vec![blob],
            ..Commit::default()
        })
        .unwrap();
    let note = receive_payload(
        &mut daemon,
        goal,
        principal,
        payload,
        Body::ContributionPublished {
            context: Context {
                scope: Scope::Goal,
                round: EventId([0; 32]),
            },
            attempt: None,
            base: None,
            patch: None,
            artifacts: vec![],
        },
    );
    assert_eq!(
        code(daemon.call(
            agent,
            Request::BlobGet {
                goal,
                hash: payload.hash
            }
        )),
        ErrorCode::Unavailable
    );
    let Response::BlobStates(states) = daemon.ok(
        agent,
        Request::BlobStat {
            goal,
            hashes: vec![payload.hash],
        },
    ) else {
        panic!()
    };
    assert_ne!(states[0].state, BlobState::Held);
    let Response::Event(detail) = daemon.ok(agent, Request::Event { goal, event: note }) else {
        panic!()
    };
    assert_eq!(detail.text, None);
    let Response::Contributions(notes) =
        daemon.ok(agent, Request::Contributions { goal, task: None })
    else {
        panic!()
    };
    assert_eq!(notes[0].text, None);
}

#[test]
fn selected_contribution_stays_readable_in_its_scope_after_author_fork() {
    use locust_proto::event::{Event, ReviewVerdict, TaskId};
    let (mut daemon, _, _, agent, goal) = super::lifecycle::setup();
    let (member, member_conn) = super::authorization::join_local(&mut daemon, agent, goal, 2);
    let task = TaskId::Authored(super::lifecycle::event(daemon.ok(
        member_conn,
        Request::TaskOpen {
            goal,
            text: "selected output".into(),
            task_type: None,
            inputs: Default::default(),
            parent: None,
        },
    )));
    let subject = super::lifecycle::event(daemon.ok(
        member_conn,
        Request::ContributionPublish {
            goal,
            task: Some(task),
            attempt: None,
            generation: None,
            summary: "retained exact result".into(),
            base: None,
            patch: None,
            artifacts: vec![],
        },
    ));
    daemon.ok(
        agent,
        Request::ReviewRecord {
            goal,
            subject,
            verdict: ReviewVerdict::Approve,
            text: "reviewed".into(),
        },
    );
    daemon.ok(
        agent,
        Request::ScopeSelect {
            goal,
            subject,
            expected: None,
        },
    );
    let TaskId::Authored(opened) = task else {
        panic!()
    };
    let mut header = daemon.node.goals[&goal]
        .goal
        .event(&opened)
        .unwrap()
        .header()
        .clone();
    header.at_ms += 1;
    let fork = Event::sign(header, daemon.node.signer(&member).unwrap()).unwrap();
    let mut tx = crate::node::commit::Tx::none();
    tx.commit.events.push(fork);
    daemon.node.land(tx).unwrap();
    for reopened in [false, true] {
        let reader = if reopened {
            daemon.restart();
            daemon.connect(credential(1), None)
        } else {
            agent
        };
        assert!(matches!(
            daemon.node.goals[&goal].goal.standing(&subject),
            Some(crate::goal::Standing::Pending(_))
        ));
        let Response::Contributions(contributions) = daemon.ok(
            reader,
            Request::Contributions {
                goal,
                task: Some(task),
            },
        ) else {
            panic!()
        };
        assert_eq!(contributions.len(), 1);
        assert!(contributions[0].selected && contributions[0].approved);
        assert_eq!(contributions[0].contribution, subject);
        let Response::Task(detail) = daemon.ok(reader, Request::Task { goal, task }) else {
            panic!()
        };
        assert_eq!(detail.view.selected, Some(subject));
        assert_eq!(detail.view.creator, member);
        assert_ne!(detail.effective_rules_json, "null");
        let Response::Board(board) = daemon.ok(reader, Request::Board { goal }) else {
            panic!()
        };
        assert_eq!(board.len(), 1);
        assert_eq!(board[0].selected, Some(subject));
        assert_eq!(
            contributions[0].text.as_deref(),
            Some("retained exact result")
        );
    }
}
