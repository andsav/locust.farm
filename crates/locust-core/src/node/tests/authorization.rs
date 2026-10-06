//! Principal isolation and invitation/lifecycle regressions through the API.
use super::lifecycle::{event, setup};
use super::*;
use locust_proto::api::{BlobState, Membership};
use locust_proto::id::GoalId;
use locust_proto::invite::{Invitation, InviteSecret};
use locust_proto::store::Store;

fn invite(daemon: &mut Daemon, _agent: ConnId, goal: GoalId) -> locust_proto::invite::Ticket {
    let owner = daemon.owner();
    let Response::Invited { ticket } = daemon.ok(
        owner,
        Request::GoalInvite {
            goal,
            expires_ms: 604_801_000,
        },
    ) else {
        panic!()
    };
    ticket
}

fn creation(agent: PublicKey) -> Request {
    Request::GoalCreate {
        agent,
        title: "Owner's goal".into(),
        formation_json: None,
        roles: Default::default(),
        inputs: Default::default(),
    }
}

#[test]
fn goal_create_is_the_owners_act_and_names_the_host_agent() {
    let (mut d, host, owner, agent, _) = setup();
    let before = d.node.goals.len();
    assert_eq!(code(d.call(agent, creation(host))), ErrorCode::Denied);
    assert_eq!(
        code(d.on_behalf(owner, host, creation(host))),
        ErrorCode::Invalid
    );
    assert_eq!(
        code(d.call(owner, creation(PublicKey([255; 32])))),
        ErrorCode::NotFound
    );
    let revoked = d.enroll("revoked", 8);
    d.ok(owner, Request::AgentRevoke { agent: revoked });
    assert_eq!(code(d.call(owner, creation(revoked))), ErrorCode::NotFound);
    let Response::AuthorEnrolled { author } = d.ok(
        owner,
        Request::AuthorEnroll {
            name: "author".into(),
            credential: credential(9).digest(),
        },
    ) else {
        panic!()
    };
    assert_eq!(code(d.call(owner, creation(author))), ErrorCode::Denied);
    assert_eq!(d.node.goals.len(), before);
    let Response::GoalCreated { goal } = d.ok(owner, creation(host)) else {
        panic!()
    };
    assert_eq!(d.node.goals[&goal].state().governance, Some(host));
    assert_eq!(
        d.node.goals[&goal].local.level(&host),
        locust_proto::api::Level::Auto
    );
    let first = d.store.log(&goal, 0, 1).unwrap().remove(0).1;
    assert_eq!(first.header().author, host);
}

#[test]
fn join_and_leave_are_the_owners_acts_for_a_named_agent() {
    let (mut d, host, owner, host_conn, goal) = setup();
    let member = d.enroll("joiner", 10);
    let member_conn = d.connect(credential(10), None);
    let ticket = invite(&mut d, host_conn, goal);
    let request = Request::GoalJoin {
        agent: member,
        ticket,
        level: locust_proto::api::Level::Auto,
    };
    assert_eq!(
        code(d.call(member_conn, request.clone())),
        ErrorCode::Denied
    );
    assert!(!d.node.goals[&goal].is_member(&member));
    d.ok(owner, request);
    assert!(d.node.goals[&goal].is_member(&member));
    assert_eq!(
        code(d.call(
            member_conn,
            Request::GoalLeave {
                goal,
                agent: member
            }
        )),
        ErrorCode::Denied
    );
    assert_eq!(
        code(d.call(owner, Request::GoalLeave { goal, agent: host })),
        ErrorCode::Conflict
    );
    assert_eq!(
        code(d.call(owner, Request::MemberRemove { goal, member: host })),
        ErrorCode::Conflict
    );
    let response = d.ok(
        owner,
        Request::GoalLeave {
            goal,
            agent: member,
        },
    );
    let event = event(response);
    assert_eq!(
        d.store.event(&event).unwrap().unwrap().header().author,
        member
    );
    assert_eq!(
        d.node.goals[&goal].membership(&member),
        Some(Membership::Left)
    );
}

#[test]
fn host_operations_work_at_read_and_sign_as_the_host_agent() {
    use locust_proto::event::{TaskId, WorkspaceCheckpoint};
    let (mut d, host, owner, agent, goal) = setup();
    let (member, _) = join_local(&mut d, agent, goal, 12);
    let task = TaskId::Authored(event(
        d.on_behalf(
            owner,
            host,
            Request::TaskOpen {
                goal,
                text: "Host revision".into(),
                task_type: None,
                inputs: Default::default(),
                parent: None,
            },
        )
        .unwrap(),
    ));
    d.ok(
        owner,
        Request::LevelSet {
            goal,
            agent: host,
            level: locust_proto::api::Level::Read,
        },
    );
    let expected = d.node.goals[&goal].state().current_rules.unwrap();
    let formation = locust_proto::organization::Formation {
        workspace: Some(locust_proto::organization::WorkspacePolicy {
            integrator: locust_proto::organization::Authority::Participant {
                key: host.to_string(),
            },
            completion: locust_proto::organization::CompletionRule::Declaration {
                by: locust_proto::organization::Selector::Members,
            },
        }),
        ..Default::default()
    };
    let rules = event(d.ok(
        owner,
        Request::RulesBind {
            goal,
            expected,
            formation_json: serde_json::to_string(&formation).unwrap(),
            roles: Default::default(),
            inputs: Default::default(),
        },
    ));
    let round = d.node.goals[&goal].state().tasks[&task].current_round;
    let revised = event(d.ok(
        owner,
        Request::TaskRevise {
            goal,
            task,
            expected_round: round,
            task_type: None,
        },
    ));
    let removed = event(d.ok(owner, Request::MemberRemove { goal, member }));
    let epoch = event(d.ok(
        owner,
        Request::WorkspaceEpochSet {
            goal,
            expected_epoch: None,
            rules,
            checkpoint: WorkspaceCheckpoint::Unseeded,
        },
    ));
    let Response::Invited { ticket } = d.ok(
        owner,
        Request::GoalInvite {
            goal,
            expires_ms: 2_000,
        },
    ) else {
        panic!()
    };
    assert_eq!(
        Invitation::from_ticket(ticket.as_str()).unwrap().governance,
        host
    );
    for id in [rules, revised, removed, epoch] {
        assert_eq!(d.store.event(&id).unwrap().unwrap().header().author, host);
    }
}

pub(super) fn join_local(
    daemon: &mut Daemon,
    agent: ConnId,
    goal: GoalId,
    tag: u8,
) -> (PublicKey, ConnId) {
    let member = daemon.enroll(&format!("member-{tag}"), tag);
    let conn = daemon.connect(credential(tag), None);
    let ticket = invite(daemon, agent, goal);
    let owner = daemon.owner();
    assert!(matches!(
        daemon.ok(
            owner,
            Request::GoalJoin {
                agent: member,
                ticket,
                level: locust_proto::api::Level::Auto,
            }
        ),
        Response::Joined {
            membership: Membership::Member,
            ..
        }
    ));
    daemon.ok(
        owner,
        Request::LevelSet {
            goal,
            agent: member,
            level: locust_proto::api::Level::Ask,
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
    let owner = daemon.owner();
    assert!(matches!(
        daemon.ok(conn, Request::GoalStatus { goal }),
        Response::GoalStatus(_)
    ));
    assert_eq!(
        code(daemon.call(
            owner,
            Request::GoalInvite {
                goal,
                expires_ms: 1000
            }
        )),
        ErrorCode::Conflict
    );
}

#[test]
fn fabricated_join_intent_never_grants_read_access_on_a_shared_daemon() {
    let (mut daemon, _, owner, agent, goal) = setup();
    let ticket = invite(&mut daemon, agent, goal);
    let mut forged = Invitation::from_ticket(ticket.as_str()).unwrap();
    forged.secret = InviteSecret([99; 32]);
    let intruder = daemon.enroll("intruder", 3);
    let conn = daemon.connect(credential(3), None);
    assert_eq!(
        code(daemon.call(
            owner,
            Request::GoalJoin {
                agent: intruder,
                ticket: forged.to_ticket().unwrap(),
                level: locust_proto::api::Level::Auto,
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
        governance: forged.governance,
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
fn removed_principal_cannot_read_new_epoch_but_readmission_restores_history() {
    let (mut daemon, _, owner, agent, goal) = setup();
    let (member, conn) = join_local(&mut daemon, agent, goal, 2);
    let old = event(daemon.ok(
        agent,
        Request::ContributionPublish {
            goal,
            attempt: None,
            generation: None,
            summary: "old".into(),
            sources: Vec::new(),
            artifacts: vec![],
        },
    ));
    daemon.ok(owner, Request::MemberRemove { goal, member });
    let new = event(daemon.ok(
        agent,
        Request::ContributionPublish {
            goal,
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
    {
        let reader = conn;
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
    let owner = daemon.owner();
    daemon.ok(
        owner,
        Request::GoalJoin {
            agent: member,
            ticket,
            level: locust_proto::api::Level::Auto,
        },
    );
    assert_eq!(
        daemon.ok(conn, Request::BlobGet { goal, hash }),
        Response::Blob {
            bytes: b"new".to_vec()
        }
    );
}

#[test]
fn admission_stops_when_the_host_agent_is_revoked() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    let ticket = invite(&mut daemon, agent, goal);
    let joiner = daemon.enroll("joining", 2);
    daemon.ok(owner, Request::AgentRevoke { agent: principal });
    assert_eq!(
        code(daemon.call(
            owner,
            Request::GoalJoin {
                agent: joiner,
                ticket,
                level: locust_proto::api::Level::Auto,
            }
        )),
        ErrorCode::Denied
    );
    assert_eq!(daemon.node.goals[&goal].state().members.len(), 1);
}

#[test]
fn enrollment_conflicts_on_revocation_and_shutdown_retries_stop_again() {
    let mut daemon = Daemon::new(1);
    let principal = daemon.enroll("one", 1);
    let owner = daemon.owner();
    daemon.ok(owner, Request::AgentRevoke { agent: principal });
    assert_eq!(
        code(daemon.call(
            owner,
            Request::AgentEnroll {
                name: "one".into(),
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
        .sign(daemon.node.signer(&invitation.governance).unwrap())
        .unwrap();
    let principal = daemon.enroll("joiner", 4);
    assert_eq!(
        code(daemon.call(
            owner,
            Request::GoalJoin {
                agent: principal,
                ticket: invitation.to_ticket().unwrap(),
                level: locust_proto::api::Level::Auto,
            }
        )),
        ErrorCode::Conflict
    );
    invitation.goal = GoalId([77; 32]);
    invitation
        .sign(daemon.node.signer(&invitation.governance).unwrap())
        .unwrap();
    daemon.ok(
        owner,
        Request::GoalJoin {
            agent: principal,
            ticket: invitation.to_ticket().unwrap(),
            level: locust_proto::api::Level::Auto,
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
    let (mut daemon, _, owner, agent, goal) = setup();
    let (principal, _member) = join_local(&mut daemon, agent, goal, 2);
    let spare = invite(&mut daemon, agent, goal);
    daemon.ok(
        owner,
        Request::GoalLeave {
            goal,
            agent: principal,
        },
    );
    assert_eq!(
        code(daemon.call(
            owner,
            Request::GoalJoin {
                agent: principal,
                ticket: spare,
                level: locust_proto::api::Level::Auto,
            }
        )),
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
    let (mut daemon, governance, _, agent, goal) = setup();
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
    let fork = Event::sign(header, daemon.node.signer(&governance).unwrap()).unwrap();
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
            .host_halt
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
