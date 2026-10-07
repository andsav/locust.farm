//! Current lifecycle behavior, using the farm tests' real engine fixture.
use super::*;
use crate::node::tests::{authorization::join_local, lifecycle::event};
use crate::sync::Host;
use locust_proto::{
    event::{Context, DecisionPurpose, Scope, ScopeKey},
    store::Store,
};

#[test]
fn departed_member_cannot_sign_consent_withdrawal_even_for_owner_after_restart() {
    let (mut d, host, owner, agent, goal) = setup();
    let (member, _conn) = join_local(&mut d, agent, goal, 2);
    d.ok(owner, on(goal));
    d.ok(owner, consent(goal, host, true));
    d.ok(owner, consent(goal, member, true));
    let first = d.node.farm_poll(2000).remove(0);
    assert_eq!(first.request.operation, FarmOperation::Upload);
    acknowledge(&mut d, &first, 2000);
    d.ok(
        owner,
        Request::GoalLeave {
            goal,
            agent: member,
        },
    );
    assert!(d.node.goals[&goal].is_member(&member));
    assert!(d.node.goals[&goal].goal.next(&member).is_some());
    let before = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    assert_eq!(
        code(d.call(owner, consent(goal, member, false))),
        ErrorCode::Denied
    );
    assert_eq!(
        code(d.on_behalf(owner, member, finding(goal, "departed"))),
        ErrorCode::Denied
    );
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before);
    d.restart();
    let owner = d.owner();
    assert_eq!(
        code(d.call(owner, consent(goal, member, false))),
        ErrorCode::Denied
    );
    let Response::FarmPreview(preview) = d.ok(owner, Request::FarmShow { goal }) else {
        panic!()
    };
    assert!(preview.status.unwrap().eligible);
    assert_eq!(preview.snapshot.unwrap().agents.len(), 2);
    assert!(
        d.node.goals[&goal].state().publication_consents[&member]
            .1
            .accept
    );
}

#[test]
fn later_consent_with_missing_predecessor_suspends_but_replayed_old_consent_does_not() {
    let (mut d, host, owner, agent, goal) = setup();
    let (member, _) = join_local(&mut d, agent, goal, 2);
    d.ok(owner, on(goal));
    d.ok(owner, consent(goal, host, true));
    d.ok(owner, consent(goal, member, true));
    let first = d.node.farm_poll(2000).remove(0);
    assert_eq!(first.request.operation, FarmOperation::Upload);
    acknowledge(&mut d, &first, 2000);
    let old = d.node.goals[&goal].state().publication_consents[&member].0;
    let old = d.store.event(&old).unwrap().unwrap();
    // Marks kept: the data directory is put back, its marks directory survives.
    let copy = snapshot(&d.store);
    d.ok(owner, consent(goal, member, true));
    let preceding = d.node.goals[&goal].state().publication_consents[&member].0;
    d.ok(owner, consent(goal, member, true));
    let later = d.node.goals[&goal].state().publication_consents[&member].0;
    let preceding = d.store.event(&preceding).unwrap().unwrap();
    let later = d.store.event(&later).unwrap().unwrap();
    assert_eq!(later.header().prev, Some(preceding.id()));
    d.store = copy;
    d.restart();
    d.node
        .replica(&goal)
        .unwrap()
        .receive(vec![later.to_wire()])
        .unwrap();
    assert_eq!(
        d.node.goals[&goal].goal.standing(&later.id()),
        Some(crate::goal::Standing::Pending(
            crate::goal::Waiting::Predecessor
        ))
    );
    assert!(d.node.goals[&goal].halted().is_none());
    let suspend = d.node.farm_poll(4000).remove(0);
    assert_eq!(suspend.request.operation, FarmOperation::Suspend);
    acknowledge(&mut d, &suspend, 4000);
    d.node
        .replica(&goal)
        .unwrap()
        .receive(vec![preceding.to_wire()])
        .unwrap();
    assert!(
        d.node.goals[&goal]
            .goal
            .standing(&later.id())
            .unwrap()
            .is_effective()
    );
    let resume = d.node.farm_poll(6000).remove(0);
    assert_eq!(resume.request.operation, FarmOperation::Upload);
    acknowledge(&mut d, &resume, 6000);
    d.node
        .replica(&goal)
        .unwrap()
        .receive(vec![old.to_wire()])
        .unwrap();
    let owner = d.owner();
    let Response::FarmPreview(preview) = d.ok(owner, Request::FarmShow { goal }) else {
        panic!()
    };
    assert!(preview.status.unwrap().eligible);
    assert!(d.node.farm_poll(7000).is_empty());
}

#[test]
fn excluded_consent_arriving_after_removal_suspends_even_when_it_accepts() {
    for accept in [false, true] {
        let (mut d, host, owner, agent, goal) = setup();
        let (member, conn) = join_local(&mut d, agent, goal, 2);
        d.ok(conn, finding(goal, "covered author even after removal"));
        d.ok(owner, on(goal));
        d.ok(owner, consent(goal, host, true));
        d.ok(owner, consent(goal, member, true));
        let first = d.node.farm_poll(2000).remove(0);
        assert_eq!(first.request.operation, FarmOperation::Upload);
        acknowledge(&mut d, &first, 2000);
        // Marks kept: the data directory is put back, its marks directory survives.
        let copy = snapshot(&d.store);
        d.ok(owner, consent(goal, member, accept));
        let late = d.node.goals[&goal].state().publication_consents[&member].0;
        let late = d.store.event(&late).unwrap().unwrap();
        // The host removes based on the prefix it held before this consent.
        d.store = copy;
        d.restart();
        let owner = d.owner();
        d.ok(owner, Request::MemberRemove { goal, member });
        let Response::FarmPreview(preview) = d.ok(owner, Request::FarmShow { goal }) else {
            panic!()
        };
        assert!(preview.status.unwrap().eligible);
        d.node
            .replica(&goal)
            .unwrap()
            .receive(vec![late.to_wire()])
            .unwrap();
        assert_eq!(
            d.node.goals[&goal].goal.standing(&late.id()),
            Some(crate::goal::Standing::Excluded(
                crate::goal::Exclusion::PastRemoval
            ))
        );
        assert!(
            d.node.goals[&goal].state().publication_consents[&member]
                .1
                .accept
        );
        let suspend = d.node.farm_poll(4000).remove(0);
        assert_eq!(suspend.request.operation, FarmOperation::Suspend);
        d.restart();
        assert_eq!(d.node.farm_poll(6000).remove(0).request, suspend.request);
    }
}

#[test]
fn goal_close_is_shared_and_publicly_ended_but_does_not_stop_work_or_admission() {
    let (mut d, host, owner, agent, goal) = setup();
    let (member, _) = join_local(&mut d, agent, goal, 2);
    let close = Request::ScopeClose {
        goal,
        scope: Scope::Goal,
        expected: None,
    };
    let error = d.on_behalf(owner, member, close.clone()).unwrap_err();
    assert_eq!(error.code, ErrorCode::NotEligible);
    assert!(matches!(
        error.refused().unwrap().why,
        locust_proto::api::Why::Rules {
            rule: locust_proto::api::Rule::Finish,
            ..
        }
    ));
    d.ok(owner, on(goal));
    d.ok(owner, consent(goal, host, true));
    d.ok(owner, consent(goal, member, true));
    let first = d.node.farm_poll(2000).remove(0);
    assert_eq!(first.request.operation, FarmOperation::Upload);
    acknowledge(&mut d, &first, 2000);
    let id = event(d.ok(agent, close));
    let entry = &d.node.goals[&goal];
    assert!(entry.goal.standing(&id).unwrap().is_effective());
    let key = ScopeKey {
        context: Context {
            scope: Scope::Goal,
            round: entry.state().current_rules.unwrap(),
        },
        purpose: DecisionPurpose::Closure,
    };
    assert_eq!(entry.state().decisions[&key].last().unwrap().id, id);
    let Response::FarmPreview(preview) = d.ok(owner, Request::FarmShow { goal }) else {
        panic!()
    };
    assert_eq!(preview.snapshot.unwrap().goal_state, FarmGoalState::Ended);
    d.ok(agent, finding(goal, "after goal close"));
    d.ok(
        agent,
        Request::TaskOpen {
            goal,
            text: "new task after goal close".into(),
            task_type: None,
            inputs: Default::default(),
            parent: None,
        },
    );
    let (newcomer, _) = join_local(&mut d, agent, goal, 3);
    d.ok(owner, consent(goal, newcomer, true));
    d.ok(
        agent,
        Request::ScopeReopen {
            goal,
            scope: Scope::Goal,
            expected: Some(id),
        },
    );
    let Response::FarmPreview(preview) = d.ok(owner, Request::FarmShow { goal }) else {
        panic!()
    };
    assert_eq!(preview.snapshot.unwrap().goal_state, FarmGoalState::Open);
    assert!(d.node.goals[&goal].halted().is_none());
}

#[test]
fn default_formation_has_no_goal_finish_decider() {
    let (mut d, host, owner, _agent, _) = setup();
    let Response::GoalCreated { goal } = d.ok(
        owner,
        Request::GoalCreate {
            name: "host".into(),
            agent: host,
            title: "Default".into(),
            formation_json: Some("{\"schema_version\":2}".into()),

            inputs: Default::default(),
        },
    ) else {
        panic!()
    };
    let owner = d.owner();
    let governance = d.node.goals[&goal].state().governance.unwrap();
    assert_ne!(governance, host);
    let before = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    let close = Request::ScopeClose {
        goal,
        scope: Scope::Goal,
        expected: None,
    };
    // The governance key is no principal, so nothing acts on its behalf.
    assert_eq!(
        code(d.on_behalf(owner, governance, close.clone())),
        ErrorCode::NotFound
    );
    let error = d.on_behalf(owner, host, close).unwrap_err();
    assert_eq!(error.code, ErrorCode::NotEligible);
    assert!(matches!(
        error.refused().unwrap().why,
        locust_proto::api::Why::Rules {
            rule: locust_proto::api::Rule::Finish,
            ..
        }
    ));
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before);
    assert!(
        d.node.goals[&goal]
            .goal
            .effective_rules(
                d.node.goals[&goal]
                    .goal
                    .current_context(Scope::Goal)
                    .unwrap(),
                &d.node.goals[&goal].definitions
            )
            .unwrap()
            .decisions
            .finish
            .is_none()
    );
}

#[test]
fn goal_close_follows_finish_role_instead_of_host_identity() {
    let (mut d, _, owner, agent, goal) = setup();
    let (member, _) = join_local(&mut d, agent, goal, 2);
    let formation = locust_proto::organization::presets()
        .into_iter()
        .find(|p| p.name == "directed")
        .unwrap()
        .formation;
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
    let expected = d.node.goals[&goal].state().roles["lead"].clone();
    d.ok(
        owner,
        Request::RoleGive {
            goal,
            role: "lead".into(),
            member,
            expected,
        },
    );
    let close = Request::ScopeClose {
        goal,
        scope: Scope::Goal,
        expected: None,
    };
    let before = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    let error = d.call(agent, close.clone()).unwrap_err();
    assert_eq!(error.code, ErrorCode::NotEligible);
    assert!(matches!(
        error.refused().unwrap().why,
        locust_proto::api::Why::Rules {
            rule: locust_proto::api::Rule::Finish,
            ..
        }
    ));
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before);
    let id = event(d.on_behalf(owner, member, close).unwrap());
    assert!(
        d.node.goals[&goal]
            .goal
            .standing(&id)
            .unwrap()
            .is_effective()
    );
    assert_eq!(d.store.event(&id).unwrap().unwrap().header().author, member);
}
