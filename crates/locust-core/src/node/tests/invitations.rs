//! Review confirmation, durable inventory and invitation lifecycle boundaries.

use super::lifecycle::setup;
use super::*;
use locust_proto::api::{InvitationState, InvitationSummary, Membership};
use locust_proto::id::{EndpointId, GoalId};
use locust_proto::invite::{Invitation, JoinRequest, Ticket};
use locust_proto::store::{Space, Store};
use locust_proto::sync::Refusal;

fn issue(daemon: &mut Daemon, _agent: ConnId, goal: GoalId, expires_ms: Option<u64>) -> Ticket {
    let owner = daemon.owner();
    let Response::Invited { ticket } = daemon.ok(
        owner,
        Request::GoalInvite {
            goal,
            expires_ms: expires_ms.unwrap_or(604_801_000),
        },
    ) else {
        panic!()
    };
    ticket
}

fn inventory(daemon: &mut Daemon, owner: ConnId, goal: GoalId) -> Vec<InvitationSummary> {
    let Response::Invitations { invitations } = daemon.ok(owner, Request::GoalInvitations { goal })
    else {
        panic!()
    };
    invitations
}

fn reviewed(ticket: Ticket, agent: PublicKey) -> Request {
    Request::GoalJoin {
        agent,
        ticket,
        level: locust_proto::api::Level::Auto,
    }
}

#[test]
fn inspect_is_signed_capability_free_and_does_not_join_or_change_state() {
    let (mut daemon, governance, owner, agent, goal) = setup();
    let ticket = issue(&mut daemon, agent, goal, None);
    let before = daemon.store.scan(Space::Goal, &[]).unwrap();
    let Response::InvitationInspected { preview } = daemon.ok(
        owner,
        Request::InvitationInspect {
            ticket: ticket.clone(),
        },
    ) else {
        panic!()
    };
    assert_eq!(preview.goal, goal);
    assert_eq!(preview.goal_title.as_deref(), Some("A test goal"));
    assert_eq!(preview.governance, governance);
    assert!(preview.signature_verified);
    assert_eq!(before, daemon.store.scan(Space::Goal, &[]).unwrap());
    let encoded = serde_json::to_string(&preview).unwrap();
    assert!(!encoded.contains(ticket.as_str()));
    assert!(!encoded.contains("locust-invite-"));
    let mut altered = Invitation::from_ticket(ticket.as_str()).unwrap();
    altered.goal_title = Some("Deceptive title".into());
    assert_eq!(
        code(daemon.call(
            owner,
            Request::InvitationInspect {
                ticket: altered.to_ticket().unwrap()
            }
        )),
        ErrorCode::Denied
    );
}

#[test]
fn person_joins_with_existing_principal_at_the_selected_level() {
    let (mut daemon, _, owner, agent, goal) = setup();
    let member = daemon.enroll("person-selected", 2);
    let ticket = issue(&mut daemon, agent, goal, None);
    let request = reviewed(ticket, member);
    assert_eq!(code(daemon.call(agent, request.clone())), ErrorCode::Denied);
    let result = daemon.ok(owner, request.clone());
    assert!(matches!(
        result,
        Response::Joined {
            membership: Membership::Member,
            ..
        }
    ));
    assert_eq!(daemon.ok(owner, request.clone()), result);
    assert_eq!(
        daemon.node.goals[&goal].local.level(&member),
        locust_proto::api::Level::Auto
    );
    assert!(
        !daemon.node.goals[&goal]
            .local
            .checkouts
            .keys()
            .any(|(principal, _)| *principal == member)
    );
    daemon.restart();
    let owner = daemon.owner();
    assert_eq!(daemon.ok(owner, request), result);
    assert_eq!(
        daemon.node.goals[&goal].local.level(&member),
        locust_proto::api::Level::Auto
    );
}

#[test]
fn joining_again_keeps_the_level_the_person_chose() {
    let (mut daemon, _, owner, agent, goal) = setup();
    let member = daemon.enroll("person-lowered", 2);
    let ticket = issue(&mut daemon, agent, goal, None);
    daemon.ok(owner, reviewed(ticket.clone(), member));
    daemon.ok(
        owner,
        Request::LevelSet {
            goal,
            agent: member,
            level: locust_proto::api::Level::Read,
        },
    );
    // The repeated join asks for auto, the default, and changes nothing.
    assert!(matches!(
        daemon.ok(owner, reviewed(ticket, member)),
        Response::Joined {
            membership: Membership::Member,
            level: locust_proto::api::Level::Read,
            ..
        }
    ));
    assert_eq!(
        daemon.node.goals[&goal].local.level(&member),
        locust_proto::api::Level::Read
    );
}

#[test]
fn unknown_or_revoked_agents_cannot_join() {
    let (mut daemon, _, owner, agent, goal) = setup();
    let member = daemon.enroll("recipient", 2);
    let second = issue(&mut daemon, agent, goal, None);
    assert_eq!(
        code(daemon.call(owner, reviewed(second.clone(), PublicKey([99; 32])))),
        ErrorCode::NotFound
    );
    daemon.ok(owner, Request::AgentRevoke { agent: member });
    assert_eq!(
        code(daemon.call(owner, reviewed(second, member))),
        ErrorCode::NotFound
    );
}

#[test]
fn revoke_is_durable_idempotent_and_inventory_never_returns_capability_bytes() {
    let (mut daemon, _, owner, agent, goal) = setup();
    let member = daemon.enroll("recipient", 2);
    let ticket = issue(&mut daemon, agent, goal, None);
    let issued = inventory(&mut daemon, owner, goal);
    assert_eq!(issued.len(), 1);
    assert_eq!(issued[0].state, InvitationState::Pending);
    let serialized = serde_json::to_string(&issued).unwrap();
    let secret = Invitation::from_ticket(ticket.as_str())
        .unwrap()
        .secret
        .0
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert!(!serialized.contains(&secret));
    assert!(!serialized.contains("locust-invite-"));
    let revoke = Request::InvitationRevoke {
        goal,
        invitation: Some(issued[0].invitation.clone()),
    };
    let result = daemon.ok(owner, revoke.clone());
    assert_eq!(daemon.ok(owner, revoke.clone()), result);
    assert_eq!(
        code(daemon.call(owner, reviewed(ticket.clone(), member))),
        ErrorCode::Denied
    );
    daemon.restart();
    let owner = daemon.owner();
    assert_eq!(daemon.ok(owner, revoke), result);
    assert_eq!(
        inventory(&mut daemon, owner, goal)[0].state,
        InvitationState::Revoked
    );
    assert_eq!(
        code(daemon.call(owner, reviewed(ticket, member))),
        ErrorCode::Denied
    );
    assert!(!daemon.node.goals[&goal].is_member(&member));
}

#[test]
fn host_can_revoke_all_pending_invitations_without_disturbing_redeemed_membership() {
    let (mut daemon, _, owner, agent, goal) = setup();
    let member = daemon.enroll("redeemed-member", 2);
    let redeemed = issue(&mut daemon, agent, goal, None);
    daemon.ok(owner, reviewed(redeemed, member));
    let pending_a = issue(&mut daemon, agent, goal, None);
    let pending_b = issue(&mut daemon, agent, goal, None);
    let revoke_all = Request::InvitationRevoke {
        goal,
        invitation: None,
    };
    assert_eq!(
        daemon.ok(owner, revoke_all.clone()),
        Response::InvitationsRevoked { count: 2 }
    );
    assert_eq!(
        daemon.ok(owner, revoke_all.clone()),
        Response::InvitationsRevoked { count: 0 }
    );
    daemon.restart();
    let owner = daemon.owner();
    assert_eq!(
        daemon.ok(owner, revoke_all),
        Response::InvitationsRevoked { count: 0 }
    );
    let inventory = inventory(&mut daemon, owner, goal);
    assert_eq!(
        inventory
            .iter()
            .filter(|item| item.state == InvitationState::Revoked)
            .count(),
        2
    );
    assert_eq!(
        inventory
            .iter()
            .filter(|item| item.state == InvitationState::Redeemed)
            .count(),
        1
    );
    assert!(daemon.node.goals[&goal].is_member(&member));
    let late = daemon.enroll("late-member", 3);
    for ticket in [pending_a, pending_b] {
        assert_eq!(
            code(daemon.call(owner, reviewed(ticket, late))),
            ErrorCode::Denied
        );
    }
}

#[test]
fn invitation_issuance_requires_an_explicit_expiry_and_owner_authority() {
    let (mut daemon, _, owner, agent, goal) = setup();
    assert!(
        serde_json::from_value::<Request>(serde_json::json!({
            "goal.invite": { "goal": goal }
        }))
        .is_err()
    );
    let request = Request::GoalInvite {
        goal,
        expires_ms: 604_801_000,
    };
    assert_eq!(code(daemon.call(agent, request.clone())), ErrorCode::Denied);
    assert!(matches!(
        daemon.ok(owner, request),
        Response::Invited { .. }
    ));
    assert_eq!(
        code(daemon.call(agent, Request::GoalInvitations { goal })),
        ErrorCode::Denied
    );
    assert_eq!(
        code(daemon.call(
            agent,
            Request::InvitationRevoke {
                goal,
                invitation: None
            }
        )),
        ErrorCode::Denied
    );
}

#[test]
fn redeemed_invitation_is_not_revoked_and_retries_recover_after_expiry() {
    let (mut daemon, _, owner, agent, goal) = setup();
    let member = daemon.enroll("recipient", 2);
    let ticket = issue(&mut daemon, agent, goal, Some(1_100));
    let join = reviewed(ticket.clone(), member);
    daemon.ok(owner, join.clone());
    let summary = inventory(&mut daemon, owner, goal).remove(0);
    assert_eq!(summary.state, InvitationState::Redeemed);
    assert_eq!(summary.redeemed_by, Some(member));
    assert_eq!(summary.redeemed_ms, Some(1_000));
    let error = daemon
        .call(
            owner,
            Request::InvitationRevoke {
                goal,
                invitation: Some(summary.invitation),
            },
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
    assert!(error.message.contains("member remove"));
    let frame = daemon.frame(join);
    assert!(matches!(
        daemon.step(owner, frame, 2_000),
        Step::Reply(locust_proto::api::ResponseFrame {
            result: Ok(Response::Joined {
                membership: Membership::Member,
                ..
            }),
            ..
        })
    ));
    let invitation = Invitation::from_ticket(ticket.as_str()).unwrap();
    let endpoint = EndpointId([21; 32]);
    let same = JoinRequest::sign(
        goal,
        endpoint,
        invitation.secret,
        daemon.node.signer(&member).unwrap(),
    );
    assert!(daemon.node.plan_join(&endpoint, &same, 2_000).is_ok());
    let other = daemon.enroll("other", 3);
    let different = JoinRequest::sign(
        goal,
        endpoint,
        invitation.secret,
        daemon.node.signer(&other).unwrap(),
    );
    assert!(matches!(
        daemon.node.plan_join(&endpoint, &different, 2_000),
        Err(Refusal::InvitationRefused)
    ));
}

#[test]
fn expired_preview_is_readable_but_join_is_refused_without_side_effects() {
    let (mut daemon, _, owner, agent, goal) = setup();
    let member = daemon.enroll("recipient", 2);
    let ticket = issue(&mut daemon, agent, goal, Some(1_100));
    let frame = daemon.frame(Request::InvitationInspect {
        ticket: ticket.clone(),
    });
    let Step::Reply(reply) = daemon.step(owner, frame, 2_000) else {
        panic!()
    };
    let Response::InvitationInspected { preview } = reply.result.unwrap() else {
        panic!()
    };
    assert!(preview.expired);
    let frame = daemon.frame(Request::GoalJoin {
        agent: member,
        ticket,
        level: locust_proto::api::Level::Auto,
    });
    let Step::Reply(reply) = daemon.step(owner, frame, 2_000) else {
        panic!()
    };
    let error = reply.result.unwrap_err();
    assert_eq!(error.code, ErrorCode::Denied);
    assert!(error.message.contains("fresh invitation"));
    assert!(!daemon.node.goals[&goal].is_member(&member));
    assert!(daemon.node.goals[&goal].local.joins.is_empty());
    let frame = daemon.frame(Request::GoalInvitations { goal });
    let Step::Reply(reply) = daemon.step(owner, frame, 2_000) else {
        panic!()
    };
    let Response::Invitations { invitations } = reply.result.unwrap() else {
        panic!()
    };
    assert_eq!(invitations[0].state, InvitationState::Expired);
    let fresh = issue(&mut daemon, agent, goal, None);
    assert!(matches!(
        daemon.ok(owner, reviewed(fresh, member)),
        Response::Joined {
            membership: Membership::Member,
            ..
        }
    ));
}

#[test]
fn admission_stops_when_the_host_agent_is_revoked() {
    let (mut daemon, governance, owner, agent, goal) = setup();
    let member = daemon.enroll("joining", 2);
    let ticket = issue(&mut daemon, agent, goal, None);
    daemon.ok(owner, Request::AgentRevoke { agent: governance });
    assert_eq!(
        code(daemon.call(owner, reviewed(ticket, member))),
        ErrorCode::Denied
    );
    assert!(!daemon.node.goals[&goal].is_member(&member));
    assert_eq!(
        inventory(&mut daemon, owner, goal)[0].state,
        InvitationState::Pending
    );
}
