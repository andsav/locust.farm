//! Owner permission controls preserve independent categories and observation.

use super::lifecycle::{authorize, event, offered, setup};
use super::*;
use locust_proto::api::{GoalPermission, GoalPermissions};
use locust_proto::id::GoalId;
use locust_proto::store::Store;

fn inspect(daemon: &mut Daemon, owner: ConnId, goal: GoalId, agent: PublicKey) -> GoalPermissions {
    let Response::Permissions(view) = daemon.ok(owner, Request::Permissions { goal, agent }) else {
        panic!()
    };
    view
}

#[test]
fn permission_changes_preserve_unrelated_categories_and_survive_restart() {
    let (mut daemon, principal, owner, _, goal) = setup();
    let original = inspect(&mut daemon, owner, goal, principal);
    assert!(original.grants.review);
    let Response::Permissions(allowed) = daemon.ok(
        owner,
        Request::PermissionAllow {
            goal,
            agent: principal,
            permissions: vec![GoalPermission::Execute, GoalPermission::Takeover],
        },
    ) else {
        panic!()
    };
    assert!(allowed.grants.execute && allowed.grants.takeover);
    assert_eq!(allowed.grants.review, original.grants.review);
    daemon.ok(
        owner,
        Request::PermissionRevoke {
            goal,
            agent: principal,
            permissions: vec![GoalPermission::Review],
        },
    );
    daemon.restart();
    let owner = daemon.owner();
    let view = inspect(&mut daemon, owner, goal, principal);
    assert!(!view.grants.review);
    assert!(view.grants.execute && view.grants.takeover);
    assert_eq!(view.name, "host");
}

#[test]
fn task_exceptions_stay_visible_until_explicitly_revoked_and_do_not_cancel_claims() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    let (task, offer) = offered(&mut daemon, agent, goal, principal);
    authorize(&mut daemon, owner, goal, task, principal);
    daemon.ok(
        owner,
        Request::PermissionRevoke {
            goal,
            agent: principal,
            permissions: vec![GoalPermission::Execute, GoalPermission::Takeover],
        },
    );
    let view = inspect(&mut daemon, owner, goal, principal);
    assert!(!view.grants.execute);
    assert_eq!(view.task_authorizations.len(), 1);
    assert_eq!(view.task_authorizations[0].task, Some(task));
    assert!(view.task_authorizations[0].current && view.task_authorizations[0].takeover);
    let Response::Claimed(claim) = daemon.ok(
        agent,
        Request::AttemptStart {
            goal,
            task,
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    daemon.ok(
        owner,
        Request::PermissionTaskRevoke {
            goal,
            agent: principal,
            task,
        },
    );
    assert!(
        inspect(&mut daemon, owner, goal, principal)
            .task_authorizations
            .is_empty()
    );
    let Response::Pending(pending) = daemon.ok(agent, Request::Pending { goal }) else {
        panic!()
    };
    assert_eq!(pending.claimed, vec![claim]);
    daemon.restart();
    let owner = daemon.owner();
    assert!(
        inspect(&mut daemon, owner, goal, principal)
            .task_authorizations
            .is_empty()
    );
}

#[test]
fn task_revocation_restores_authorization_requirement() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    let (task, offer) = offered(&mut daemon, agent, goal, principal);
    authorize(&mut daemon, owner, goal, task, principal);
    daemon.ok(
        owner,
        Request::PermissionTaskRevoke {
            goal,
            agent: principal,
            task,
        },
    );
    assert_eq!(
        code(daemon.call(
            agent,
            Request::AttemptStart {
                goal,
                task,
                offer: Some(offer)
            }
        )),
        ErrorCode::AuthorizationRequired
    );
}

#[test]
fn allowing_task_execution_preserves_an_existing_task_takeover_grant() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    let (task, _) = offered(&mut daemon, agent, goal, principal);
    for takeover in [true, false] {
        let Response::Permissions(view) = daemon.ok(
            owner,
            Request::PermissionTaskAllow {
                goal,
                agent: principal,
                task,
                takeover,
            },
        ) else {
            panic!()
        };
        assert_eq!(view.task_authorizations.len(), 1);
        assert!(view.task_authorizations[0].takeover);
        assert!(!view.grants.execute && !view.grants.takeover);
    }
    daemon.restart();
    let owner = daemon.owner();
    assert!(inspect(&mut daemon, owner, goal, principal).task_authorizations[0].takeover);
}

#[test]
fn own_permissions_are_visible_but_changes_and_other_principals_remain_owner_only() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    for conn in [agent] {
        let own = inspect(&mut daemon, conn, goal, principal);
        assert_eq!(own, inspect(&mut daemon, owner, goal, principal));
        assert_eq!(
            code(daemon.call(
                conn,
                Request::Permissions {
                    goal,
                    agent: PublicKey([99; 32])
                }
            )),
            ErrorCode::Denied
        );
        for request in [
            Request::PermissionAllow {
                goal,
                agent: principal,
                permissions: vec![GoalPermission::Execute],
            },
            Request::PermissionRevoke {
                goal,
                agent: principal,
                permissions: vec![GoalPermission::Review],
            },
            Request::Inbox,
        ] {
            assert_eq!(code(daemon.call(conn, request)), ErrorCode::Denied);
        }
    }
}

#[test]
fn owner_inbox_identifies_person_and_task_without_consuming_or_authorizing() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    let (task, _) = offered(&mut daemon, agent, goal, principal);
    let before = daemon.ok(agent, Request::Pending { goal });
    let Response::Inbox(inbox) = daemon.ok(owner, Request::Inbox) else {
        panic!()
    };
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].agent, principal);
    assert_eq!(inbox[0].name, "host");
    assert_eq!(inbox[0].tasks[0].task, task);
    assert!(
        inbox[0]
            .pending
            .to_authorize
            .iter()
            .any(|item| item.task == task)
    );
    assert!(!inbox[0].grants.execute);
    assert_eq!(daemon.ok(agent, Request::Pending { goal }), before);
    assert!(
        inspect(&mut daemon, owner, goal, principal)
            .task_authorizations
            .is_empty()
    );
}

/// Task revisions are the host's decision, without a local grant.
#[test]
fn task_revise_is_a_host_act_without_a_grant() {
    let (mut d, principal, owner, agent, goal) = setup();
    let (task, _) = offered(&mut d, agent, goal, principal);
    let round = d.node.goals[&goal].state().tasks[&task].current_round;
    let before = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    assert_eq!(
        code(d.call(
            agent,
            Request::TaskRevise {
                goal,
                task,
                expected_round: round,
                task_type: None
            }
        )),
        ErrorCode::Denied
    );
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before);
    event(d.ok(
        owner,
        Request::TaskRevise {
            goal,
            task,
            expected_round: round,
            task_type: None,
        },
    ));
}
