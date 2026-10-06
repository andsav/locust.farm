//! Local level and task allowance behavior through the public request seam.

use super::authorization::join_local;
use super::lifecycle::{event, offered, setup};
use super::*;
use locust_proto::api::{Level, Why};
use locust_proto::event::Scope;
use locust_proto::store::Store;

fn set_level(
    d: &mut Daemon,
    owner: ConnId,
    goal: locust_proto::id::GoalId,
    agent: PublicKey,
    level: Level,
) {
    let Response::Abilities(abilities) = d.ok(owner, Request::LevelSet { goal, agent, level })
    else {
        panic!()
    };
    assert_eq!(abilities.level, level);
}

#[test]
fn a_refused_start_is_not_stored_and_the_task_can_be_allowed_for_its_round() {
    let (mut d, principal, owner, agent, goal) = setup();
    let (task, offer) = offered(&mut d, agent, goal, principal);
    set_level(&mut d, owner, goal, principal, Level::Read);
    let before = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    let error = d
        .call(
            agent,
            Request::AttemptStart {
                goal,
                task,
                offer: Some(offer),
            },
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::LevelRequired);
    let refused: locust_proto::api::Refused =
        serde_json::from_str(error.details_json.as_deref().unwrap()).unwrap();
    assert_eq!(refused.task, Some(task));
    assert!(matches!(
        refused.why,
        Why::YourSetting {
            level: Level::Read,
            needs: Level::Auto
        }
    ));
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before);
    assert!(matches!(
        d.node.goals[&goal].local.allowances.get(&(task, principal)),
        Some(crate::node::local::Allowance::Wanted { .. })
    ));

    let Response::Abilities(allowed) = d.ok(
        owner,
        Request::TaskAllow {
            goal,
            agent: principal,
            task,
        },
    ) else {
        panic!()
    };
    assert_eq!(allowed.allowed_tasks, vec![task]);
    assert!(allowed.wanted_tasks.is_empty());
    set_level(&mut d, owner, goal, principal, Level::Ask);
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
    d.ok(
        owner,
        Request::TaskDisallow {
            goal,
            agent: principal,
            task,
        },
    );
    assert!(d.node.goals[&goal].claims.contains_key(&claim.attempt));
    d.restart();
    assert!(d.node.goals[&goal].claims.contains_key(&claim.attempt));
    assert_eq!(d.node.goals[&goal].local.level(&principal), Level::Ask);
    assert!(
        !d.node.goals[&goal]
            .local
            .allowances
            .contains_key(&(task, principal))
    );
}

#[test]
fn replay_refusal_precedes_local_level_and_owner_act_persists() {
    let (mut d, principal, owner, agent, goal) = setup();
    let (task, _) = offered(&mut d, agent, goal, principal);
    set_level(&mut d, owner, goal, principal, Level::Read);
    let before = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    let error = d
        .call(
            agent,
            Request::AttemptStart {
                goal,
                task,
                offer: None,
            },
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::NotEligible);
    let refused: locust_proto::api::Refused =
        serde_json::from_str(error.details_json.as_deref().unwrap()).unwrap();
    assert!(matches!(refused.why, Why::Rules { .. }));
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before);
    assert!(
        !d.node.goals[&goal]
            .local
            .allowances
            .contains_key(&(task, principal))
    );

    let id = event(
        d.on_behalf(
            owner,
            principal,
            Request::TaskOpen {
                goal,
                text: "Owned task".into(),
                task_type: None,
                inputs: Default::default(),
                parent: None,
            },
        )
        .unwrap(),
    );
    assert!(d.node.goals[&goal].local.by_owner.contains(&id));
    d.restart();
    assert!(d.node.goals[&goal].local.by_owner.contains(&id));
}

#[test]
fn allowance_survives_close_reopen_but_not_a_revision() {
    let (mut d, principal, owner, agent, goal) = setup();
    let (task, _) = offered(&mut d, agent, goal, principal);
    set_level(&mut d, owner, goal, principal, Level::Ask);
    d.ok(
        owner,
        Request::TaskAllow {
            goal,
            agent: principal,
            task,
        },
    );
    let round = d.node.goals[&goal].state().tasks[&task].current_round;
    let closed = event(d.ok(
        agent,
        Request::ScopeClose {
            goal,
            scope: Scope::Task(task),
            expected: None,
        },
    ));
    let error = d
        .call(
            owner,
            Request::TaskAllow {
                goal,
                agent: principal,
                task,
            },
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
    let refused: locust_proto::api::Refused =
        serde_json::from_str(error.details_json.as_deref().unwrap()).unwrap();
    assert!(matches!(refused.why, Why::State { .. }));
    d.ok(
        agent,
        Request::ScopeReopen {
            goal,
            scope: Scope::Task(task),
            expected: Some(closed),
        },
    );
    assert_eq!(
        d.node.goals[&goal].state().tasks[&task].current_round,
        round
    );
    assert_eq!(
        d.node.goals[&goal]
            .local
            .start_level(task, round, principal),
        Level::Ask
    );
    let new_round = event(d.ok(
        owner,
        Request::TaskRevise {
            goal,
            task,
            expected_round: round,
            task_type: None,
        },
    ));
    assert_ne!(round, new_round);
    assert_eq!(
        d.node.goals[&goal]
            .local
            .start_level(task, new_round, principal),
        Level::Auto
    );
}

#[test]
fn removal_clears_the_level_and_allowance_before_readmission() {
    let (mut d, principal, owner, agent, goal) = setup();
    let (member, _) = join_local(&mut d, agent, goal, 31);
    let task = locust_proto::event::TaskId::Authored(event(d.ok(
        agent,
        Request::TaskOpen {
            goal,
            text: "Shared task".into(),
            task_type: None,
            inputs: Default::default(),
            parent: None,
        },
    )));
    d.ok(
        owner,
        Request::TaskAllow {
            goal,
            agent: member,
            task,
        },
    );
    assert_eq!(d.node.goals[&goal].local.level(&member), Level::Ask);
    assert!(
        d.node.goals[&goal]
            .local
            .allowances
            .contains_key(&(task, member))
    );
    d.ok(owner, Request::MemberRemove { goal, member });
    assert!(!d.node.goals[&goal].local.levels.contains_key(&member));
    assert!(
        !d.node.goals[&goal]
            .local
            .allowances
            .contains_key(&(task, member))
    );
    d.restart();
    assert!(!d.node.goals[&goal].local.levels.contains_key(&member));
    let owner = d.owner();
    let Response::Invited { ticket } = d.ok(
        owner,
        Request::GoalInvite {
            goal,
            expires_ms: 604_801_000,
        },
    ) else {
        panic!()
    };
    d.ok(
        owner,
        Request::GoalJoin {
            agent: member,
            ticket,
            level: Level::Auto,
        },
    );
    assert_eq!(d.node.goals[&goal].local.level(&member), Level::Auto);
    assert!(
        !d.node.goals[&goal]
            .local
            .allowances
            .contains_key(&(task, member))
    );
    assert_eq!(d.node.goals[&goal].local.level(&principal), Level::Ask);
}

#[test]
fn status_and_abilities_roundtrip_on_the_real_api_codec() {
    let (mut d, principal, owner, agent, goal) = setup();
    let status = d.ok(agent, Request::GoalStatus { goal });
    let bytes = locust_proto::codec::encode(&status).unwrap();
    assert_eq!(
        locust_proto::codec::decode::<Response>(&bytes).unwrap(),
        status
    );
    let abilities = d.ok(
        owner,
        Request::LevelSet {
            goal,
            agent: principal,
            level: Level::Ask,
        },
    );
    let bytes = locust_proto::codec::encode(&abilities).unwrap();
    assert_eq!(
        locust_proto::codec::decode::<Response>(&bytes).unwrap(),
        abilities
    );
}

#[test]
fn owner_only_commands_report_why_and_do_not_use_operation_names_in_messages() {
    let (mut d, principal, _, agent, goal) = setup();
    for (request, expected_host, expected_act) in [
        (
            Request::LevelSet {
                goal,
                agent: principal,
                level: Level::Read,
            },
            false,
            locust_proto::api::Act::PersonCommand,
        ),
        (
            Request::GoalInvite {
                goal,
                expires_ms: 604_801_000,
            },
            true,
            locust_proto::api::Act::Invite,
        ),
    ] {
        let error = d.call(agent, request).unwrap_err();
        assert_eq!(error.code, ErrorCode::Denied);
        names_no_operation(&error);
        let refused = error.refused().unwrap();
        assert_eq!(refused.agent, principal);
        assert_eq!(refused.act, expected_act);
        assert_eq!(refused.goal, Some(goal));
        assert!(matches!(refused.why, Why::OnlyYou { host, .. } if host == expected_host));
    }
}

#[test]
fn joining_agent_can_choose_a_level_before_admission() {
    let (mut d, _, owner, _, goal) = setup();
    let joiner = d.enroll("joiner", 42);
    let Response::Invited { ticket } = d.ok(
        owner,
        Request::GoalInvite {
            goal,
            expires_ms: 604_801_000,
        },
    ) else {
        panic!()
    };
    let invitation = locust_proto::invite::Invitation::from_ticket(ticket.as_str()).unwrap();
    let join = crate::node::local::JoinRecord {
        governance: invitation.governance,
        endpoint: invitation.endpoint,
        hints: Vec::new(),
        secret: invitation.secret,
        publication: None,
        refused: false,
    };
    let mut tx = crate::node::commit::Tx::none();
    tx.local(crate::node::local::join_write(&goal, &joiner, &join))
        .local(crate::node::local::part_write(&goal, &joiner, false));
    d.node.land(tx).unwrap();
    assert_eq!(
        d.node.goals[&goal].membership(&joiner),
        Some(locust_proto::api::Membership::Joining)
    );
    assert_eq!(d.node.goals[&goal].local.level(&joiner), Level::Auto);
    set_level(&mut d, owner, goal, joiner, Level::Ask);
    d.restart();
    assert_eq!(
        d.node.goals[&goal].membership(&joiner),
        Some(locust_proto::api::Membership::Joining)
    );
    assert_eq!(d.node.goals[&goal].local.level(&joiner), Level::Ask);
}

#[test]
fn abilities_include_named_checks_and_own_declaration_opportunities() {
    use locust_proto::organization::{CompletionRule, Selector};
    let (mut d, principal, owner, agent, goal) = setup();
    let mut formation = locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == "open")
        .unwrap()
        .formation;
    formation.decisions.completion = CompletionRule::All {
        rules: vec![
            CompletionRule::Declaration {
                by: Selector::ContributionAuthor,
            },
            CompletionRule::Check {
                name: "build".into(),
                by: Selector::Members,
            },
        ],
    };
    let expected = d.node.goals[&goal].state().current_rules.unwrap();
    d.ok(
        owner,
        Request::RulesBind {
            goal,
            expected,
            formation_json: serde_json::to_string(&formation).unwrap(),
            roles: Default::default(),
            inputs: Default::default(),
        },
    );
    let abilities =
        d.node.goals[&goal]
            .goal
            .abilities(principal, Level::Ask, &d.node.goals[&goal].definitions);
    assert!(
        abilities
            .iter()
            .any(|row| row.rule == locust_proto::api::Rule::Declare && row.eligible)
    );
    assert!(
        abilities
            .iter()
            .any(|row| row.rule == locust_proto::api::Rule::Attest && row.eligible)
    );
    let subject = event(d.ok(
        agent,
        Request::ContributionPublish {
            goal,
            attempt: None,
            generation: None,
            summary: "Result".into(),
            sources: vec![],
            artifacts: vec![],
        },
    ));
    assert!(d.node.goals[&goal].goal.can_attest(
        subject,
        principal,
        &d.node.goals[&goal].definitions
    ));
}

#[test]
fn disallow_reports_hidden_allowance_and_wanted_record_truthfully() {
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
    let Response::TaskDisallowed {
        abilities,
        changed,
        was_allowed,
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
    assert!(changed && was_allowed);
    assert!(abilities.allowed_tasks.is_empty());
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
    assert!(!changed && !was_allowed);
    d.ok(
        agent,
        Request::ScopeReopen {
            goal,
            scope: Scope::Task(task),
            expected: Some(closed),
        },
    );
    set_level(&mut d, owner, goal, principal, Level::Read);
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
    assert!(changed && !was_allowed);
}
