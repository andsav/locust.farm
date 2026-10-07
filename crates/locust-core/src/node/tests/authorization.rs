//! Principal isolation and invitation/lifecycle regressions through the API.
use super::lifecycle::{event, finding, setup};
use super::*;
use locust_proto::api::{BlobState, EventView, Membership};
use locust_proto::crypto::Keypair;
use locust_proto::event::Body;
use locust_proto::id::{EventId, GoalId};
use locust_proto::invite::{Invitation, InviteSecret};
use locust_proto::store::Store;

/// A copy of the governance key the daemon holds for `goal`.
pub(super) fn governance_key(daemon: &Daemon, goal: GoalId) -> Keypair {
    Keypair::from_seed(
        daemon.node.goals[&goal]
            .local
            .governance
            .as_ref()
            .expect("this daemon hosts the goal")
            .seed(),
    )
}

/// The signer of a stored record.
pub(super) fn author_of(daemon: &Daemon, id: &EventId) -> PublicKey {
    daemon.store.event(id).unwrap().unwrap().header().author
}

/// Every record of the goal as the owner lists it.
fn events(daemon: &mut Daemon, owner: ConnId, goal: GoalId) -> Vec<EventView> {
    let Response::Events(events) = daemon.ok(
        owner,
        Request::Events {
            goal,
            after: None,
            limit: 1_000,
        },
    ) else {
        panic!()
    };
    events
}

fn invite(daemon: &mut Daemon, _agent: ConnId, goal: GoalId) -> locust_proto::invite::Ticket {
    let owner = daemon.owner();
    let Response::Invited { ticket } = daemon.ok(
        owner,
        Request::GoalInvite {
            role: None,
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
        name: "host".into(),
        agent,
        title: "Owner's goal".into(),
        formation_json: Some("{\"schema_version\":2}".into()),

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
    let entry = &d.node.goals[&goal];
    let governance = entry.state().governance.unwrap();
    assert_eq!(entry.state().host, Some(host));
    assert_eq!(entry.local.level(&host), locust_proto::api::Level::Auto);
    assert!(entry.is_member(&host));
    // The goal's key is stored with the goal and is no agent and no member.
    assert_ne!(governance, host);
    assert_eq!(governance_key(&d, goal).public(), governance);
    assert!(d.node.principals.get(&governance).is_none());
    assert_eq!(code(d.node.signer(&governance)), ErrorCode::NotFound);
    assert!(entry.membership(&governance).is_none());
    let records = d.store.log(&goal, 0, 4).unwrap();
    assert_eq!(records.len(), 3);
    let kinds: Vec<_> = records
        .iter()
        .map(|(_, record)| record.header().body.kind())
        .collect();
    assert_eq!(kinds, ["genesis", "member_admitted", "rules_bound"]);
    for (_, record) in &records {
        assert_eq!(record.header().author, governance);
    }
    let Body::Genesis(genesis) = &records[0].1.header().body else {
        panic!()
    };
    assert_eq!((genesis.governance, genesis.host), (governance, host));
    assert!(matches!(
        records[1].1.header().body,
        Body::MemberAdmitted { member, .. } if member == host
    ));
    let Response::GoalStatus(status) = d.ok(owner, Request::GoalStatus { goal }) else {
        panic!()
    };
    assert_eq!(status.governance, governance);
    assert!(status.hosted_here);
    assert_eq!(status.host, Some(host));
    assert_eq!(status.members.len(), 1);
    assert_eq!(status.members[0].member, host);
    let Response::Status(daemon) = d.ok(owner, Request::Status) else {
        panic!()
    };
    assert!(daemon.agents.iter().all(|view| view.agent != governance));
}

#[test]
fn join_and_leave_are_the_owners_acts_for_a_named_agent() {
    let (mut d, host, owner, host_conn, goal) = setup();
    let member = d.enroll("joiner", 10);
    let member_conn = d.connect(credential(10), None);
    let ticket = invite(&mut d, host_conn, goal);
    let request = Request::GoalJoin {
        name: "member".into(),
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
fn host_operations_need_no_grant_and_sign_with_the_governance_key() {
    check_host_operations(false);
}

#[test]
fn disconnecting_the_hosts_agent_stops_no_host_command() {
    check_host_operations(true);
}

fn check_host_operations(disconnected: bool) {
    use locust_proto::event::{TaskId, WorkspaceCheckpoint};
    let (mut d, host, owner, agent, goal) = setup();
    let governance = governance_key(&d, goal).public();
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
    if disconnected {
        d.ok(owner, Request::AgentRevoke { agent: host });
    }
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
            no_role: false,
            goal,
            expected,
            formation_json: serde_json::to_string(&formation).unwrap(),

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
            role: None,
            goal,
            expires_ms: 2_000,
        },
    ) else {
        panic!()
    };
    assert_eq!(
        Invitation::from_ticket(ticket.as_str()).unwrap().governance,
        governance
    );
    d.ok(
        owner,
        Request::InvitationRevoke {
            goal,
            invitation: None,
        },
    );
    d.ok(
        owner,
        Request::FarmOn {
            goal,
            base_url: "https://example.test".into(),
            listed: false,
            title: None,
            formation: "open".into(),
            stage_labels: Default::default(),
            role_labels: Default::default(),
            recent_changes: 5,
        },
    );
    let on = d.node.goals[&goal].state().publication.as_ref().unwrap().0;
    d.ok(owner, Request::FarmOff { goal });
    let off = d.node.goals[&goal].state().publication.as_ref().unwrap().0;
    assert_eq!(author_of(&d, &on), governance);
    assert_eq!(author_of(&d, &off), governance);
    // The host's agent kept its read level: no grant was needed, because
    // none of these records is the agent's.
    assert_eq!(
        d.node.goals[&goal].local.level(&host),
        locust_proto::api::Level::Read
    );
    let signed: Vec<_> = [rules, revised, removed, epoch]
        .into_iter()
        .map(|id| author_of(&d, &id))
        .collect();
    assert_eq!(signed, vec![governance; 4]);
    let TaskId::Authored(opened) = task else {
        panic!()
    };
    assert_eq!(author_of(&d, &opened), host);
    let views = events(&mut d, owner, goal);
    let by_host = |id: EventId| views.iter().find(|view| view.event == id).unwrap().by_host;
    assert!([rules, revised, removed, epoch].into_iter().all(by_host));
    assert!(!by_host(opened));
    let Response::Task(detail) = d.ok(owner, Request::Task { goal, task }) else {
        panic!()
    };
    assert!(!detail.view.by_host);
    assert_eq!(detail.view.creator, host);
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
                name: "member".into(),
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
                role: None,
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
                name: "member".into(),
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
        host_name: "host".into(),
        name: "member".into(),
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
            name: "member".into(),
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
fn no_credential_and_no_on_behalf_reaches_the_governance_key() {
    use locust_proto::api::{Audience, OPERATIONS};
    use locust_proto::event::{TaskId, WorkspaceCheckpoint};
    use std::collections::BTreeSet;
    let (mut d, host, owner, agent, goal) = setup();
    let governance = governance_key(&d, goal).public();
    let rules = d.node.goals[&goal].state().current_rules.unwrap();
    let host_requests = vec![
        Request::FarmOn {
            goal,
            base_url: "http://127.0.0.1:3001".into(),
            listed: true,
            title: None,
            formation: "Collaborative build".into(),
            stage_labels: Default::default(),
            role_labels: Default::default(),
            recent_changes: 50,
        },
        Request::FarmOff { goal },
        Request::GoalInvite {
            role: None,
            goal,
            expires_ms: 2_000,
        },
        Request::MemberRemove { goal, member: host },
        Request::RoleGive {
            goal,
            role: "reviewer".into(),
            member: host,
            expected: vec![host],
        },
        Request::RoleTake {
            goal,
            role: "reviewer".into(),
            member: host,
            expected: vec![host],
        },
        Request::RulesBind {
            no_role: false,
            goal,
            expected: rules,
            formation_json: "{\"schema_version\":2}".into(),

            inputs: Default::default(),
        },
        Request::WorkspaceEpochSet {
            goal,
            expected_epoch: None,
            rules,
            checkpoint: WorkspaceCheckpoint::Unseeded,
        },
        Request::TaskRevise {
            goal,
            task: TaskId::Authored(rules),
            expected_round: rules,
            task_type: None,
        },
        Request::GoalInvitations { goal },
        Request::InvitationRevoke {
            goal,
            invitation: None,
        },
    ];
    // Every host operation is covered, so a new one fails here until it is.
    let covered: BTreeSet<_> = host_requests.iter().map(|request| request.name()).collect();
    let host_only: BTreeSet<_> = OPERATIONS
        .iter()
        .filter(|operation| operation.audience == Audience::Host)
        .map(|operation| operation.name)
        .collect();
    assert_eq!(covered, host_only);
    let before = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    for request in host_requests {
        let name = request.name();
        assert_eq!(
            code(d.call(agent, request.clone())),
            ErrorCode::Denied,
            "{name}"
        );
        let mut frame = d.frame(request.clone());
        frame.on_behalf = Some(governance);
        assert_eq!(code(d.send(agent, frame)), ErrorCode::Denied, "{name}");
        assert_eq!(
            code(d.on_behalf(owner, governance, request)),
            ErrorCode::Invalid,
            "{name}"
        );
    }
    // The governance key is no principal: nothing acts on its behalf.
    for request in [
        Request::GoalStatus { goal },
        Request::Board { goal },
        finding(goal, "not the key's work"),
        Request::TaskOpen {
            goal,
            text: "not the key's task".into(),
            task_type: None,
            inputs: Default::default(),
            parent: None,
        },
    ] {
        let name = request.name();
        assert_eq!(
            code(d.on_behalf(owner, governance, request)),
            ErrorCode::NotFound,
            "{name}"
        );
    }
    // The owner's commands that name an agent find none under that key.
    for request in [
        Request::GoalLeave {
            goal,
            agent: governance,
        },
        Request::LevelSet {
            goal,
            agent: governance,
            level: locust_proto::api::Level::Auto,
        },
        Request::AgentRevoke { agent: governance },
        Request::AgentReconnect { agent: governance },
    ] {
        let name = request.name();
        assert_eq!(code(d.call(owner, request)), ErrorCode::NotFound, "{name}");
    }
    assert_eq!(code(d.node.signer(&governance)), ErrorCode::NotFound);
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before);
}

#[test]
fn the_hosts_agent_cannot_leave_or_be_removed_and_can_be_disconnected() {
    let (mut d, host, owner, agent, goal) = setup();
    let (member, _) = join_local(&mut d, agent, goal, 2);
    let before = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    assert_eq!(
        code(d.call(owner, Request::GoalLeave { goal, agent: host })),
        ErrorCode::Conflict
    );
    assert_eq!(
        code(d.call(owner, Request::MemberRemove { goal, member: host })),
        ErrorCode::Conflict
    );
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before);
    assert_eq!(
        d.node.goals[&goal].membership(&host),
        Some(Membership::Member)
    );
    assert_eq!(
        d.ok(owner, Request::AgentRevoke { agent: host }),
        Response::Done
    );
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before);
    assert!(d.node.goals[&goal].halted().is_none());
    assert_eq!(
        d.node.goals[&goal].membership(&host),
        Some(Membership::Member)
    );
    assert_eq!(
        code(d.call(agent, finding(goal, "after"))),
        ErrorCode::Denied
    );
    assert_eq!(
        code(d.on_behalf(owner, host, finding(goal, "after"))),
        ErrorCode::NotFound
    );
    let Response::Status(status) = d.ok(owner, Request::Status) else {
        panic!()
    };
    let view = status
        .agents
        .iter()
        .find(|view| view.agent == host)
        .unwrap();
    assert!(view.revoked);
    // Membership and the goal's records are untouched; the other member
    // still works and its removal is still the host's to decide.
    let removed = event(d.ok(owner, Request::MemberRemove { goal, member }));
    assert_eq!(author_of(&d, &removed), governance_key(&d, goal).public());
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before + 1);
}

#[test]
fn a_disconnected_agent_is_connected_again_with_its_name_and_key() {
    use locust_proto::store::Space;
    let (mut d, host, owner, agent, goal) = setup();
    let (member, member_conn) = join_local(&mut d, agent, goal, 2);
    let unknown = PublicKey([77; 32]);
    assert_eq!(
        code(d.call(agent, Request::AgentReconnect { agent: member })),
        ErrorCode::Denied
    );
    assert_eq!(
        code(d.call(owner, Request::AgentReconnect { agent: unknown })),
        ErrorCode::NotFound
    );
    // Reconnecting a connected agent writes nothing.
    let agents = d.store.scan(Space::Agent, &[]).unwrap();
    let revision = d.node.goals[&goal].revision();
    assert_eq!(
        d.ok(owner, Request::AgentReconnect { agent: member }),
        Response::Done
    );
    assert_eq!(d.store.scan(Space::Agent, &[]).unwrap(), agents);
    assert_eq!(d.node.goals[&goal].revision(), revision);
    let records = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    d.ok(owner, Request::AgentRevoke { agent: member });
    assert_eq!(
        code(d.call(member_conn, finding(goal, "while disconnected"))),
        ErrorCode::Denied
    );
    d.restart();
    let (_, refused) = d.hello(credential(2), None);
    assert!(matches!(refused, ServerHello::Refused { .. }));
    let owner = d.owner();
    assert_eq!(
        code(d.on_behalf(owner, member, finding(goal, "while disconnected"))),
        ErrorCode::NotFound
    );
    assert_eq!(
        d.ok(owner, Request::AgentReconnect { agent: member }),
        Response::Done
    );
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), records);
    let Response::Status(status) = d.ok(owner, Request::Status) else {
        panic!()
    };
    let view = status
        .agents
        .iter()
        .find(|view| view.agent == member)
        .unwrap();
    assert_eq!(view.name, "member-2");
    assert!(!view.revoked);
    let member_conn = d.connect(credential(2), None);
    let posted = event(d.ok(member_conn, finding(goal, "connected again")));
    assert_eq!(author_of(&d, &posted), member);
    // The host's agent is disconnected and connected again the same way;
    // in between the host's commands work and the goal's records are
    // untouched.
    let agent = d.connect(credential(1), Some(session(1)));
    let records = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    d.ok(owner, Request::AgentRevoke { agent: host });
    assert_eq!(
        code(d.call(agent, finding(goal, "host"))),
        ErrorCode::Denied
    );
    let ticket = invite(&mut d, agent, goal);
    assert_eq!(
        Invitation::from_ticket(ticket.as_str()).unwrap().governance,
        governance_key(&d, goal).public()
    );
    assert_eq!(
        d.ok(owner, Request::AgentReconnect { agent: host }),
        Response::Done
    );
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), records);
    let posted = event(d.ok(agent, finding(goal, "host connected again")));
    assert_eq!(author_of(&d, &posted), host);
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
    let governance = governance_key(&daemon, goal);
    let mut invitation = Invitation::from_ticket(ticket.as_str()).unwrap();
    assert_eq!(invitation.governance, governance.public());
    invitation.endpoint = EndpointId([88; 32]);
    invitation.sign(&governance).unwrap();
    let principal = daemon.enroll("joiner", 4);
    assert_eq!(
        code(daemon.call(
            owner,
            Request::GoalJoin {
                name: "member".into(),
                agent: principal,
                ticket: invitation.to_ticket().unwrap(),
                level: locust_proto::api::Level::Auto,
            }
        )),
        ErrorCode::Conflict
    );
    invitation.goal = GoalId([77; 32]);
    invitation.sign(&governance).unwrap();
    daemon.ok(
        owner,
        Request::GoalJoin {
            name: "member".into(),
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
                name: "member".into(),
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
    use locust_proto::event::Event;
    use locust_proto::id::EndpointId;
    use locust_proto::invite::JoinRequest;
    let (mut daemon, host, _, agent, goal) = setup();
    let ticket = invite(&mut daemon, agent, goal);
    let invitation = Invitation::from_ticket(ticket.as_str()).unwrap();
    let remote = EndpointId([44; 32]);
    let worker = Keypair::from_seed([44; 32]);
    let request = JoinRequest::sign(goal, remote, "member".into(), invitation.secret, &worker);
    daemon.node.join(&remote, &request, 1000).unwrap();
    let events = daemon.store.log(&goal, 0, 20).unwrap();
    let own = &events[1].1;
    assert!(matches!(
        own.header().body,
        Body::MemberAdmitted { member, .. } if member == host
    ));
    let mut header = own.header().clone();
    header.at_ms += 44;
    // Only the governance key can fork its own log; the host's agent's key
    // signs nothing the governance key signed.
    assert_eq!(
        Event::sign(header.clone(), daemon.node.signer(&host).unwrap()).unwrap_err(),
        locust_proto::event::EventError::AuthorMismatch
    );
    let fork = Event::sign(header, &governance_key(&daemon, goal)).unwrap();
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

#[test]
fn reconnect_receives_waiting_deliveries_without_signing_another_step() {
    let (mut d, host, owner, agent, goal) = setup();
    let (member, _) = join_local(&mut d, agent, goal, 2);
    d.ok(owner, Request::AgentRevoke { agent: member });
    let expected = d.node.goals[&goal].state().current_rules.unwrap();
    d.ok(owner, super::delivery::pipeline_request(goal, expected));
    let effect = *d.node.goals[&goal].state().effects.keys().next().unwrap();
    assert!(!d.node.goals[&goal].deliveries[&(effect, member)].received);
    assert!(d.node.goals[&goal].deliveries[&(effect, host)].received);
    let records = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    d.ok(owner, Request::AgentReconnect { agent: member });
    assert!(d.node.goals[&goal].deliveries[&(effect, member)].received);
    assert!(d.node.goals[&goal].deliveries[&(effect, member)].delivered);
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), records);
    d.restart();
    assert!(d.node.goals[&goal].deliveries[&(effect, member)].received);
}

#[test]
fn reconnect_signs_the_step_that_waited_for_the_agent() {
    use crate::node::commit::Tx;
    use locust_proto::api::Stall;
    use locust_proto::event::Scope;
    let (mut d, _, owner, agent, goal) = setup();
    let (member, _) = join_local(&mut d, agent, goal, 2);
    let entry = &d.node.goals[&goal];
    let context = entry.goal.current_context(Scope::Goal).unwrap();
    let mut tx = Tx::none();
    d.node
        .author_alone(
            entry,
            &member,
            Body::ContributionPublished {
                context,
                attempt: None,
                sources: vec![],
                artifacts: vec![],
            },
            &mut tx,
        )
        .unwrap();
    d.ok(owner, Request::AgentRevoke { agent: member });
    d.node.land(tx).unwrap();
    let entry = &d.node.goals[&goal];
    let waiting = d.node.stalled(entry);
    assert!(!waiting.is_empty());
    assert!(
        waiting
            .iter()
            .all(|step| step.runner == member && step.reason == Stall::RunnerRevoked)
    );
    let before = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    d.ok(owner, Request::AgentReconnect { agent: member });
    assert_eq!(
        d.store.log(&goal, 0, usize::MAX).unwrap().len(),
        before + waiting.len()
    );
    for step in waiting {
        let effect = &d.node.goals[&goal].state().effects[&step.effect];
        assert_eq!(effect.runner, member);
        assert!(effect.events.iter().all(|id| author_of(&d, id) == member));
    }
    assert!(d.node.stalled(&d.node.goals[&goal]).is_empty());
}

#[test]
fn revising_a_stage_keeps_its_type_and_refuses_unusable_creator_rules() {
    use locust_proto::event::TaskId;
    use locust_proto::organization::{
        CompletionRule, DecisionRules, Formation, Selector, Stage, TaskType,
    };
    let (mut d, _, owner, _, goal) = setup();
    let mut formation = Formation::default();
    formation.decisions.completion = CompletionRule::Declaration {
        by: Selector::TaskCreator,
    };
    formation.task_types.insert(
        "safe".into(),
        TaskType {
            work: None,
            decisions: Some(DecisionRules {
                completion: CompletionRule::Declaration {
                    by: Selector::Members,
                },
                ..Default::default()
            }),
        },
    );
    formation.task_types.insert(
        "unusable".into(),
        TaskType {
            work: None,
            decisions: None,
        },
    );
    formation.flow.insert(
        "draft".into(),
        Stage {
            task_type: Some("safe".into()),
            requires: vec![],
            recipients: Selector::Members,
        },
    );
    let expected = d.node.goals[&goal].state().current_rules.unwrap();
    d.ok(
        owner,
        Request::RulesBind {
            no_role: false,
            goal,
            expected,
            formation_json: serde_json::to_string(&formation).unwrap(),
            inputs: Default::default(),
        },
    );
    let task = *d.node.goals[&goal].state().tasks.keys().next().unwrap();
    assert!(matches!(task, TaskId::Derived(_)));
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
    let entry = &d.node.goals[&goal];
    assert_eq!(
        entry.state().tasks[&task].rounds[&revised]
            .binding
            .task_type
            .as_deref(),
        Some("safe")
    );
    let records = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    let refused = d
        .call(
            owner,
            Request::TaskRevise {
                goal,
                task,
                expected_round: revised,
                task_type: Some("unusable".into()),
            },
        )
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::Conflict);
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), records);
    assert_eq!(
        d.node.goals[&goal].state().tasks[&task].current_round,
        revised
    );
}
