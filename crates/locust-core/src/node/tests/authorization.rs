//! Principal isolation and invitation/lifecycle regressions through the API.
use super::lifecycle::{event, setup};
use super::*;
use locust_proto::api::{BlobState, GoalGrants, Membership};
use locust_proto::id::GoalId;
use locust_proto::invite::{Invitation, InviteSecret};
use locust_proto::store::Store;

fn invite(daemon: &mut Daemon, agent: ConnId, goal: GoalId) -> locust_proto::invite::Ticket {
    let Response::Invited { ticket } = daemon.ok(
        agent,
        Request::GoalInvite {
            goal,
            expires_ms: None,
        },
    ) else {
        panic!()
    };
    ticket
}

pub(super) fn join_local(
    daemon: &mut Daemon,
    agent: ConnId,
    goal: GoalId,
    tag: u8,
) -> (PublicKey, ConnId) {
    let member = daemon.enroll(&format!("member-{tag}"), tag, true);
    let conn = daemon.connect(credential(tag), None);
    let ticket = invite(daemon, agent, goal);
    assert!(matches!(
        daemon.ok(conn, Request::GoalJoin { ticket }),
        Response::Joined {
            membership: Membership::Member,
            ..
        }
    ));
    let owner = daemon.owner();
    daemon.ok(
        owner,
        Request::GoalGrant {
            goal,
            agent: member,
            grants: GoalGrants {
                contribute: true,
                ..Default::default()
            },
        },
    );
    (member, conn)
}

#[test]
fn local_join_is_atomic_and_expired_or_invalid_tickets_do_not_admit() {
    let (mut daemon, _, _, agent, goal) = setup();
    let (member, _conn) = join_local(&mut daemon, agent, goal, 2);
    assert!(daemon.node.goals[&goal].is_member(&member));
    daemon.restart();
    let conn = daemon.connect(credential(2), None);
    assert!(matches!(
        daemon.ok(conn, Request::GoalStatus { goal }),
        Response::GoalStatus(_)
    ));
    assert_eq!(
        code(daemon.call(
            conn,
            Request::GoalInvite {
                goal,
                expires_ms: Some(1000)
            }
        )),
        ErrorCode::Conflict
    );
}

#[test]
fn fabricated_join_intent_never_grants_read_access_on_a_shared_daemon() {
    let (mut daemon, _, _, agent, goal) = setup();
    let ticket = invite(&mut daemon, agent, goal);
    let mut forged = Invitation::from_ticket(ticket.as_str()).unwrap();
    forged.secret = InviteSecret([99; 32]);
    let intruder = daemon.enroll("intruder", 3, true);
    let conn = daemon.connect(credential(3), None);
    assert_eq!(
        code(daemon.call(
            conn,
            Request::GoalJoin {
                ticket: forged.to_ticket().unwrap()
            }
        )),
        ErrorCode::Denied
    );
    assert_eq!(
        code(daemon.call(conn, Request::GoalStatus { goal })),
        ErrorCode::NotFound
    );
    assert_eq!(
        code(daemon.call(conn, Request::Board { goal })),
        ErrorCode::NotFound
    );
    assert!(daemon.node.goals[&goal].membership(&intruder).is_none());
    // Remote join intent and refused intent both preserve no plaintext authority.
    let mut join = crate::node::local::JoinRecord {
        publication: None,
        administrator: forged.administrator,
        endpoint: forged.endpoint,
        hints: vec![],
        secret: forged.secret,
        refused: false,
    };
    let mut tx = crate::node::commit::Tx::none();
    tx.local(crate::node::local::join_write(&goal, &intruder, &join))
        .local(crate::node::local::part_write(&goal, &intruder, false));
    daemon.node.land(tx).unwrap();
    for refused in [false, true] {
        if refused {
            join.refused = true;
            let mut tx = crate::node::commit::Tx::none();
            tx.local(crate::node::local::join_write(&goal, &intruder, &join));
            daemon.node.land(tx).unwrap();
        }
        assert_eq!(
            code(daemon.call(conn, Request::Board { goal })),
            if refused {
                ErrorCode::Denied
            } else {
                ErrorCode::Unavailable
            }
        );
        let Response::Status(status) = daemon.ok(conn, Request::Status) else {
            panic!()
        };
        assert_eq!(status.goals[0].title, None);
        assert_eq!(
            status.goals[0].membership,
            if refused {
                Membership::Refused
            } else {
                Membership::Joining
            }
        );
    }
    daemon.restart();
    let conn = daemon.connect(credential(3), None);
    assert_eq!(
        code(daemon.call(conn, Request::Board { goal })),
        ErrorCode::Denied
    );
}

#[test]
fn removed_principal_and_viewer_cannot_read_new_epoch_but_readmission_restores_history() {
    let (mut daemon, _, owner, agent, goal) = setup();
    let (member, conn) = join_local(&mut daemon, agent, goal, 2);
    daemon.ok(
        owner,
        Request::ViewerEnroll {
            agent: member,
            credential: credential(9).digest(),
        },
    );
    let viewer = daemon.connect(credential(9), None);
    let old = event(daemon.ok(
        agent,
        Request::ContributionPublish {
            goal,
            task: None,
            attempt: None,
            generation: None,
            summary: "old".into(),
            sources: Vec::new(),
            artifacts: vec![],
        },
    ));
    daemon.ok(agent, Request::MemberRemove { goal, member });
    let new = event(daemon.ok(
        agent,
        Request::ContributionPublish {
            goal,
            task: None,
            attempt: None,
            generation: None,
            summary: "new".into(),
            sources: Vec::new(),
            artifacts: vec![],
        },
    ));
    let hash = daemon
        .store
        .event(&new)
        .unwrap()
        .unwrap()
        .header()
        .payload
        .unwrap()
        .hash;
    for reader in [conn, viewer] {
        let Response::Event(detail) = daemon.ok(reader, Request::Event { goal, event: old }) else {
            panic!()
        };
        assert_eq!(detail.text.as_deref(), Some("old"));
        let Response::Event(detail) = daemon.ok(reader, Request::Event { goal, event: new }) else {
            panic!()
        };
        assert_eq!(detail.text, None);
        assert_eq!(detail.content[0].state, BlobState::Unavailable);
        assert_eq!(
            code(daemon.call(reader, Request::BlobGet { goal, hash })),
            ErrorCode::Denied
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
        assert_eq!(states[0].state, BlobState::Unavailable);
    }
    daemon.restart();
    let conn = daemon.connect(credential(2), None);
    assert_eq!(
        code(daemon.call(conn, Request::BlobGet { goal, hash })),
        ErrorCode::Denied
    );
    let agent = daemon.connect(credential(1), None);
    let ticket = invite(&mut daemon, agent, goal);
    daemon.ok(conn, Request::GoalJoin { ticket });
    assert_eq!(
        daemon.ok(conn, Request::BlobGet { goal, hash }),
        Response::Blob {
            bytes: b"new".to_vec()
        }
    );
}

#[test]
fn invite_redemption_rechecks_issuer_grants_and_revocation() {
    for revoke in [false, true] {
        let (mut daemon, principal, owner, agent, goal) = setup();
        let ticket = invite(&mut daemon, agent, goal);
        daemon.enroll("joining", 2, true);
        let joining = daemon.connect(credential(2), None);
        if revoke {
            daemon.ok(owner, Request::AgentRevoke { agent: principal });
        } else {
            daemon.ok(
                owner,
                Request::GoalGrant {
                    goal,
                    agent: principal,
                    grants: GoalGrants::default(),
                },
            );
        }
        assert_eq!(
            code(daemon.call(joining, Request::GoalJoin { ticket })),
            ErrorCode::Denied
        );
        assert_eq!(daemon.node.goals[&goal].state().members.len(), 1);
    }
}

#[test]
fn enrollment_conflicts_on_revocation_or_changed_grants_and_shutdown_retries_stop_again() {
    let mut daemon = Daemon::new(1);
    let principal = daemon.enroll("one", 1, false);
    let owner = daemon.owner();
    assert_eq!(
        code(daemon.call(
            owner,
            Request::AgentEnroll {
                name: "one".into(),
                grants: Grants { manage_goals: true },
                credential: credential(1).digest()
            }
        )),
        ErrorCode::Conflict
    );
    daemon.ok(owner, Request::AgentRevoke { agent: principal });
    assert_eq!(
        code(daemon.call(
            owner,
            Request::AgentEnroll {
                name: "one".into(),
                grants: Grants {
                    manage_goals: false
                },
                credential: credential(1).digest()
            }
        )),
        ErrorCode::Conflict
    );
    daemon.keyed(owner, 3, Request::Shutdown).unwrap();
    assert!(daemon.node.stop_requested());
    daemon.restart();
    let owner = daemon.owner();
    assert!(!daemon.node.stop_requested());
    daemon.keyed(owner, 3, Request::Shutdown).unwrap();
    assert!(daemon.node.stop_requested());
}

#[test]
fn held_ticket_endpoint_is_checked_and_revoked_principals_stop_joining() {
    use crate::sync::Host;
    use locust_proto::id::EndpointId;
    let (mut daemon, _, owner, agent, goal) = setup();
    let ticket = invite(&mut daemon, agent, goal);
    let mut invitation = Invitation::from_ticket(ticket.as_str()).unwrap();
    invitation.endpoint = EndpointId([88; 32]);
    invitation
        .sign(daemon.node.signer(&invitation.administrator).unwrap())
        .unwrap();
    let principal = daemon.enroll("joiner", 4, true);
    let conn = daemon.connect(credential(4), None);
    assert_eq!(
        code(daemon.call(
            conn,
            Request::GoalJoin {
                ticket: invitation.to_ticket().unwrap()
            }
        )),
        ErrorCode::Conflict
    );
    invitation.goal = GoalId([77; 32]);
    invitation
        .sign(daemon.node.signer(&invitation.administrator).unwrap())
        .unwrap();
    daemon.ok(
        conn,
        Request::GoalJoin {
            ticket: invitation.to_ticket().unwrap(),
        },
    );
    assert_eq!(daemon.node.joins().len(), 1);
    daemon.ok(owner, Request::AgentRevoke { agent: principal });
    assert!(daemon.node.joins().is_empty());
    daemon.restart();
    assert!(daemon.node.joins().is_empty());
}

#[test]
fn leaving_member_cannot_clear_local_departure_with_a_spare_ticket() {
    let (mut daemon, _, _, agent, goal) = setup();
    let (principal, member) = join_local(&mut daemon, agent, goal, 2);
    let spare = invite(&mut daemon, agent, goal);
    daemon.ok(member, Request::GoalLeave { goal });
    assert_eq!(
        code(daemon.call(member, Request::GoalJoin { ticket: spare })),
        ErrorCode::Conflict
    );
    assert_eq!(
        daemon.node.goals[&goal].membership(&principal),
        Some(Membership::Left)
    );
    daemon.ok(
        agent,
        Request::ContributionPublish {
            goal,
            task: None,
            attempt: None,
            generation: None,
            summary: "still left".into(),
            sources: Vec::new(),
            artifacts: vec![],
        },
    );
    assert_eq!(
        daemon.node.goals[&goal].membership(&principal),
        Some(Membership::Left)
    );
}

#[test]
fn halt_proofs_reach_historical_contacts_without_restoring_membership() {
    use crate::sync::Host;
    use locust_proto::crypto::Keypair;
    use locust_proto::event::Event;
    use locust_proto::id::EndpointId;
    use locust_proto::invite::JoinRequest;
    let (mut daemon, administrator, _, agent, goal) = setup();
    let ticket = invite(&mut daemon, agent, goal);
    let invitation = Invitation::from_ticket(ticket.as_str()).unwrap();
    let remote = EndpointId([44; 32]);
    let worker = Keypair::from_seed([44; 32]);
    let request = JoinRequest::sign(goal, remote, invitation.secret, &worker);
    daemon.node.join(&remote, &request, 1000).unwrap();
    let events = daemon.store.log(&goal, 0, 20).unwrap();
    let own = &events[1].1;
    let mut header = own.header().clone();
    header.at_ms += 44;
    let fork = Event::sign(header, daemon.node.signer(&administrator).unwrap()).unwrap();
    let proof = [own.to_wire(), fork.to_wire()];
    assert!(
        daemon
            .node
            .receive_halt_proof(&goal, &EndpointId([99; 32]), proof.clone())
            .is_err()
    );
    daemon
        .node
        .receive_halt_proof(&goal, &remote, proof.clone())
        .unwrap();
    assert!(!daemon.node.goals[&goal].is_member(&worker.public()));
    assert!(!daemon.node.speaks_for_member(&goal, &remote));
    assert!(
        daemon
            .node
            .halt_proofs()
            .iter()
            .any(|(id, endpoint, _)| *id == goal && *endpoint == remote)
    );
    assert!(
        daemon
            .node
            .receive_halt_proof(&goal, &remote, [own.to_wire(), own.to_wire()])
            .is_err()
    );
    daemon.restart();
    assert!(
        daemon.node.goals[&goal]
            .goal
            .evaluation()
            .admin_halt
            .as_ref()
            .is_some()
    );
    assert!(
        daemon
            .node
            .halt_proofs()
            .iter()
            .any(|(_, endpoint, _)| *endpoint == remote)
    );
}
