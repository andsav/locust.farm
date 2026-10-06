//! The read level fences new local work without hiding existing attempts.

use super::authorization::join_local;
use super::lifecycle::{event, finding, offered, setup};
use super::*;
use locust_proto::api::{Act, Level, Why};
use locust_proto::event::{AttemptStatus, CancelOutcome, Scope, TaskId};

fn set_read(d: &mut Daemon, owner: ConnId, goal: locust_proto::id::GoalId, agent: PublicKey) {
    let Response::Abilities(abilities) = d.ok(
        owner,
        Request::LevelSet {
            goal,
            agent,
            level: Level::Read,
        },
    ) else {
        panic!()
    };
    assert_eq!(abilities.level, Level::Read);
}

#[test]
fn read_agent_can_report_and_complete_only_after_its_result_was_posted() {
    let (mut d, principal, owner, agent, goal) = setup();
    let (task, offer) = offered(&mut d, agent, goal, principal);
    d.ok(
        owner,
        Request::TaskAllow {
            goal,
            agent: principal,
            task,
        },
    );
    let Response::Claimed(claim) = d.ok(
        agent,
        Request::AttemptStart {
            goal,
            task,
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    set_read(&mut d, owner, goal, principal);
    d.ok(
        agent,
        Request::AttemptReport {
            goal,
            attempt: claim.attempt,
            generation: claim.generation,
            status: AttemptStatus::Progress,
            text: "Still working".into(),
        },
    );
    assert_eq!(
        code(d.call(
            agent,
            Request::AttemptReport {
                goal,
                attempt: claim.attempt,
                generation: claim.generation,
                status: AttemptStatus::Completed,
                text: "Done".into()
            }
        )),
        ErrorCode::Conflict
    );
    let refusal = d
        .call(
            agent,
            Request::ContributionPublish {
                goal,
                attempt: Some(claim.attempt),
                generation: Some(claim.generation),
                summary: "A result".into(),
                sources: vec![],
                artifacts: vec![],
            },
        )
        .unwrap_err();
    assert_eq!(refusal.code, ErrorCode::LevelRequired);
    assert!(matches!(
        refusal.refused().unwrap().why,
        Why::YourSetting {
            level: Level::Read,
            needs: Level::Ask
        }
    ));
    d.ok(
        owner,
        Request::LevelSet {
            goal,
            agent: principal,
            level: Level::Ask,
        },
    );
    event(d.ok(
        agent,
        Request::ContributionPublish {
            goal,
            attempt: Some(claim.attempt),
            generation: Some(claim.generation),
            summary: "A result".into(),
            sources: vec![],
            artifacts: vec![],
        },
    ));
    set_read(&mut d, owner, goal, principal);
    d.ok(
        agent,
        Request::AttemptReport {
            goal,
            attempt: claim.attempt,
            generation: claim.generation,
            status: AttemptStatus::Completed,
            text: "Result posted".into(),
        },
    );
}

#[test]
fn read_agent_can_decline_and_acknowledge_a_prior_cancellation() {
    let (mut d, principal, owner, agent, goal) = setup();
    let (_task, offer) = offered(&mut d, agent, goal, principal);
    set_read(&mut d, owner, goal, principal);
    event(d.ok(agent, Request::WorkDecline { goal, offer }));

    let (mut d, principal, owner, agent, goal) = setup();
    let (task, offer) = offered(&mut d, agent, goal, principal);
    d.ok(
        owner,
        Request::TaskAllow {
            goal,
            agent: principal,
            task,
        },
    );
    let Response::Claimed(claim) = d.ok(
        agent,
        Request::AttemptStart {
            goal,
            task,
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    let cancel = event(d.ok(
        agent,
        Request::AttemptCancel {
            goal,
            attempt: claim.attempt,
        },
    ));
    set_read(&mut d, owner, goal, principal);
    event(d.ok(
        agent,
        Request::CancelAcknowledge {
            goal,
            cancel,
            generation: Some(claim.generation),
            outcome: CancelOutcome::Stopped,
        },
    ));
}

#[test]
fn read_blocks_post_store_and_withdraw_but_keeps_content_readable() {
    let (mut d, principal, owner, agent, goal) = setup();
    set_read(&mut d, owner, goal, principal);
    for request in [
        finding(goal, "A new finding"),
        Request::BlobPut {
            goal,
            bytes: b"sample".to_vec(),
        },
    ] {
        let error = d.call(agent, request).unwrap_err();
        assert_eq!(error.code, ErrorCode::LevelRequired);
        assert!(matches!(
            error.refused().unwrap().why,
            Why::YourSetting {
                level: Level::Read,
                needs: Level::Ask
            }
        ));
    }
    d.ok(
        owner,
        Request::LevelSet {
            goal,
            agent: principal,
            level: Level::Ask,
        },
    );
    let Response::BlobStored { hash } = d.ok(
        agent,
        Request::BlobPut {
            goal,
            bytes: b"sample".to_vec(),
        },
    ) else {
        panic!()
    };
    set_read(&mut d, owner, goal, principal);
    assert!(matches!(
        d.ok(agent, Request::BlobGet { goal, hash }),
        Response::Blob { .. }
    ));
    let error = d
        .call(agent, Request::BlobWithdraw { goal, hash })
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::LevelRequired);
    assert_eq!(error.refused().unwrap().act, Act::Withdraw);
}

#[test]
fn task_result_requires_the_authors_attempt_and_claiming_session() {
    let (mut d, principal, owner, agent, goal) = setup();
    let (task, offer) = offered(&mut d, agent, goal, principal);
    d.ok(
        owner,
        Request::TaskAllow {
            goal,
            agent: principal,
            task,
        },
    );
    let Response::Claimed(claim) = d.ok(
        agent,
        Request::AttemptStart {
            goal,
            task,
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    let other_session = d.connect(credential(1), Some(session(2)));
    assert_eq!(
        code(d.call(
            other_session,
            Request::ContributionPublish {
                goal,
                attempt: Some(claim.attempt),
                generation: Some(claim.generation),
                summary: "Another session".into(),
                sources: vec![],
                artifacts: vec![]
            }
        )),
        ErrorCode::Superseded
    );
    let (_other, other_agent) = join_local(&mut d, agent, goal, 32);
    assert_eq!(
        code(d.call(
            other_agent,
            Request::ContributionPublish {
                goal,
                attempt: Some(claim.attempt),
                generation: Some(claim.generation),
                summary: "Another member".into(),
                sources: vec![],
                artifacts: vec![]
            }
        )),
        ErrorCode::Denied
    );
    let result = event(d.ok(
        agent,
        Request::ContributionPublish {
            goal,
            attempt: Some(claim.attempt),
            generation: Some(claim.generation),
            summary: "Own result".into(),
            sources: vec![],
            artifacts: vec![],
        },
    ));
    let Response::Event(detail) = d.ok(
        agent,
        Request::Event {
            goal,
            event: result,
        },
    ) else {
        panic!()
    };
    assert_eq!(detail.task, Some(task));
}

#[test]
fn hidden_closed_allowance_is_revoked_and_does_not_return_on_reopen() {
    let (mut d, principal, owner, agent, goal) = setup();
    let (task, offer) = offered(&mut d, agent, goal, principal);
    d.ok(
        owner,
        Request::TaskAllow {
            goal,
            agent: principal,
            task,
        },
    );
    let closed = event(d.ok(
        agent,
        Request::ScopeClose {
            goal,
            scope: Scope::Task(task),
            expected: None,
        },
    ));
    let Response::GoalStatus(status) = d.ok(owner, Request::GoalStatus { goal }) else {
        panic!()
    };
    assert!(
        status
            .abilities
            .iter()
            .find(|item| item.agent == principal)
            .unwrap()
            .allowed_tasks
            .is_empty()
    );
    let Response::TaskDisallowed {
        changed,
        was_allowed,
        ..
    } = d.ok(
        owner,
        Request::TaskDisallow {
            goal,
            agent: principal,
            task,
        },
    )
    else {
        panic!()
    };
    assert!(
        changed && was_allowed,
        "stored closed allowance was removed despite being hidden"
    );
    let Response::TaskDisallowed { changed, .. } = d.ok(
        owner,
        Request::TaskDisallow {
            goal,
            agent: principal,
            task,
        },
    ) else {
        panic!()
    };
    assert!(!changed, "a repeat revocation changes nothing");
    d.ok(
        agent,
        Request::ScopeReopen {
            goal,
            scope: Scope::Task(task),
            expected: Some(closed),
        },
    );
    assert_eq!(
        code(d.call(
            agent,
            Request::AttemptStart {
                goal,
                task,
                offer: Some(offer)
            }
        )),
        ErrorCode::LevelRequired
    );
}

#[test]
fn owner_at_read_can_post_for_agent_but_goal_rules_still_apply_and_mark_the_event() {
    let (mut d, principal, owner, agent, goal) = setup();
    set_read(&mut d, owner, goal, principal);
    assert_eq!(
        code(d.call(
            agent,
            Request::TaskOpen {
                goal,
                text: "Agent task".into(),
                task_type: None,
                inputs: Default::default(),
                parent: None
            }
        )),
        ErrorCode::LevelRequired
    );
    let opened = event(
        d.on_behalf(
            owner,
            principal,
            Request::TaskOpen {
                goal,
                text: "Owner task".into(),
                task_type: None,
                inputs: Default::default(),
                parent: None,
            },
        )
        .unwrap(),
    );
    let Response::Event(detail) = d.ok(
        agent,
        Request::Event {
            goal,
            event: opened,
        },
    ) else {
        panic!()
    };
    assert!(detail.view.by_owner);
    d.restart();
    let owner = d.owner();
    let agent = d.connect(credential(1), Some(session(1)));
    let Response::Event(detail) = d.ok(
        agent,
        Request::Event {
            goal,
            event: opened,
        },
    ) else {
        panic!()
    };
    assert!(detail.view.by_owner);

    let Response::GoalCreated { goal: open_goal } = d.ok(
        owner,
        Request::GoalCreate {
            agent: principal,
            title: "Open rules".into(),
            formation_json: None,
            roles: Default::default(),
            inputs: Default::default(),
        },
    ) else {
        panic!()
    };
    set_read(&mut d, owner, open_goal, principal);
    let open_task = event(
        d.on_behalf(
            owner,
            principal,
            Request::TaskOpen {
                goal: open_goal,
                text: "Task under open rules".into(),
                task_type: None,
                inputs: Default::default(),
                parent: None,
            },
        )
        .unwrap(),
    );
    let error = d
        .on_behalf(
            owner,
            principal,
            Request::ScopeClose {
                goal: open_goal,
                scope: Scope::Task(TaskId::Authored(open_task)),
                expected: None,
            },
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::NotEligible);
    assert!(matches!(error.refused().unwrap().why, Why::Rules { .. }));
}
