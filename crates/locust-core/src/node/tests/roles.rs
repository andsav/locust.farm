//! Role and name changes through the daemon's authenticated request seam.
use super::lifecycle::{event, finding};
use super::*;
use locust_proto::api::{GoalStatus, Level, Membership};
use locust_proto::event::{Body, ReviewVerdict};
use locust_proto::id::{EventId, GoalId};
use locust_proto::invite::Invitation;
use locust_proto::organization::{Authority, CompletionRule, Formation, Selector};
use locust_proto::store::Store;

fn formation(name: &str) -> Formation {
    locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == name)
        .unwrap()
        .formation
}
fn setup(preset: Option<&str>) -> (Daemon, PublicKey, ConnId, ConnId, GoalId) {
    let (mut daemon, host, owner, agent, _) = lifecycle::setup();
    let Response::GoalCreated { goal } = daemon.ok(
        owner,
        Request::GoalCreate {
            agent: host,
            name: "Harbor".into(),
            title: "Named goal".into(),
            formation_json: preset.map(|preset| serde_json::to_string(&formation(preset)).unwrap()),
            inputs: Default::default(),
        },
    ) else {
        panic!()
    };
    (daemon, host, owner, agent, goal)
}
fn status(daemon: &mut Daemon, owner: ConnId, goal: GoalId) -> GoalStatus {
    let Response::GoalStatus(status) = daemon.ok(owner, Request::GoalStatus { goal }) else {
        panic!()
    };
    status
}
fn join(
    daemon: &mut Daemon,
    owner: ConnId,
    goal: GoalId,
    tag: u8,
    name: &str,
    role: Option<&str>,
) -> (PublicKey, ConnId) {
    let member = daemon.enroll(&format!("local-{tag}"), tag);
    let Response::Invited { ticket } = daemon.ok(
        owner,
        Request::GoalInvite {
            goal,
            expires_ms: 1_000_000,
            role: role.map(str::to_owned),
        },
    ) else {
        panic!()
    };
    let invitation = Invitation::from_ticket(ticket.as_str()).unwrap();
    assert_eq!(invitation.host_name, "Harbor");
    assert_eq!(invitation.role.as_deref(), role);
    assert!(matches!(daemon.ok(owner, Request::GoalJoin {
        agent: member, name: name.into(), ticket, level: Level::Auto,
    }), Response::Joined { membership: Membership::Member, host_name, .. } if host_name == "Harbor"));
    (member, daemon.connect(credential(tag), Some(session(tag))))
}
fn change(
    daemon: &mut Daemon,
    owner: ConnId,
    goal: GoalId,
    member: PublicKey,
    role: &str,
    give: bool,
) -> Response {
    let expected = daemon.node.goals[&goal].state().roles[role].clone();
    let request = if give {
        Request::RoleGive {
            goal,
            role: role.into(),
            member,
            expected,
        }
    } else {
        Request::RoleTake {
            goal,
            role: role.into(),
            member,
            expected,
        }
    };
    daemon.ok(owner, request)
}
fn bind(
    daemon: &mut Daemon,
    owner: ConnId,
    goal: GoalId,
    formation: Formation,
) -> Result<Response, ApiError> {
    let expected = daemon.node.goals[&goal].state().current_rules.unwrap();
    daemon.call(
        owner,
        Request::RulesBind {
            goal,
            expected,
            formation_json: serde_json::to_string(&formation).unwrap(),
            inputs: Default::default(),
        },
    )
}
fn records(daemon: &Daemon, goal: GoalId) -> usize {
    daemon.store.log(&goal, 0, usize::MAX).unwrap().len()
}
fn approved(daemon: &Daemon, goal: GoalId, subject: EventId) -> bool {
    daemon.node.goals[&goal].state().contributions[&subject].approved
}

#[test]
fn role_give_appends_for_many_seat_roles_and_replaces_for_authorities() {
    let (mut d, host, owner, _, goal) = setup(Some("directed"));
    let (member, _) = join(&mut d, owner, goal, 2, "Maple", None);
    change(&mut d, owner, goal, member, "reviewer", true);
    let mut expected = vec![host, member];
    expected.sort();
    assert_eq!(status(&mut d, owner, goal).roles["reviewer"], expected);
    change(&mut d, owner, goal, member, "lead", true);
    assert_eq!(status(&mut d, owner, goal).roles["lead"], vec![member]);
    let stale = Request::RoleGive {
        goal,
        role: "lead".into(),
        member: host,
        expected: vec![host],
    };
    assert_eq!(code(d.call(owner, stale)), ErrorCode::Conflict);
    assert_eq!(
        code(d.call(
            owner,
            Request::RoleGive {
                goal,
                role: "missing".into(),
                member,
                expected: vec![]
            }
        )),
        ErrorCode::Invalid
    );
    let mut unsorted = expected;
    unsorted.reverse();
    assert_eq!(
        code(d.call(
            owner,
            Request::RoleTake {
                goal,
                role: "reviewer".into(),
                member,
                expected: unsorted
            }
        )),
        ErrorCode::Invalid
    );
}

#[test]
fn role_take_of_the_last_holder_falls_back_to_the_host_agent_and_the_host_seat_cannot_be_taken() {
    let (mut d, host, owner, _, goal) = setup(Some("directed"));
    let (member, _) = join(&mut d, owner, goal, 2, "Maple", None);
    change(&mut d, owner, goal, member, "lead", true);
    change(&mut d, owner, goal, member, "lead", false);
    assert_eq!(status(&mut d, owner, goal).roles["lead"], vec![host]);
    assert_eq!(
        code(d.call(
            owner,
            Request::RoleTake {
                goal,
                role: "lead".into(),
                member: host,
                expected: vec![host]
            }
        )),
        ErrorCode::Conflict
    );
    change(&mut d, owner, goal, member, "reviewer", true);
    change(&mut d, owner, goal, host, "reviewer", false);
    change(&mut d, owner, goal, member, "reviewer", false);
    assert_eq!(status(&mut d, owner, goal).roles["reviewer"], vec![host]);
}

#[test]
fn taking_a_group_role_from_the_hosts_agent_alone_signs_nothing() {
    let (mut d, host, owner, _, goal) = setup(Some("directed"));
    let before = records(&d, goal);
    assert_eq!(
        change(&mut d, owner, goal, host, "reviewer", false),
        Response::Done
    );
    assert_eq!(records(&d, goal), before);
}

#[test]
fn role_give_and_take_work_on_a_role_only_earlier_rules_declare() {
    let (mut d, host, owner, _, goal) = setup(Some("directed"));
    let (member, _) = join(&mut d, owner, goal, 2, "Maple", None);
    bind(&mut d, owner, goal, formation("peer-review")).unwrap();
    change(&mut d, owner, goal, member, "lead", true);
    assert_eq!(status(&mut d, owner, goal).roles["lead"], vec![member]);
    change(&mut d, owner, goal, member, "lead", false);
    let status = status(&mut d, owner, goal);
    assert_eq!(status.roles["lead"], vec![host]);
    assert!(status.deciding.contains("lead"));
    d.restart();
    assert_eq!(d.node.goals[&goal].state().roles["lead"], vec![host]);
}

#[test]
fn rules_bind_is_refused_when_a_role_would_change_kind() {
    let (mut d, _, owner, _, goal) = setup(Some("directed"));
    let mut group = formation("peer-review");
    group.roles = formation("directed").roles;
    group.decisions.completion = CompletionRule::Reviews {
        by: Selector::Role {
            name: "lead".into(),
        },
        count: 1,
        exclude_author: false,
    };
    let before = records(&d, goal);
    assert_eq!(code(bind(&mut d, owner, goal, group)), ErrorCode::Conflict);
    let mut deciding = formation("directed");
    deciding.decisions.selection = Some(Authority::Role {
        name: "reviewer".into(),
    });
    assert_eq!(
        code(bind(&mut d, owner, goal, deciding)),
        ErrorCode::Conflict
    );
    assert_eq!(records(&d, goal), before);
}

#[test]
fn an_invitation_role_is_given_on_admission_and_an_authority_role_is_refused_at_issue() {
    let (mut d, _, owner, agent, goal) = setup(Some("directed"));
    for role in ["lead", "absent"] {
        assert_eq!(
            code(d.call(
                owner,
                Request::GoalInvite {
                    goal,
                    role: Some(role.into()),
                    expires_ms: 1_000_000
                }
            )),
            ErrorCode::Invalid
        );
    }
    let before = records(&d, goal);
    let (member, _) = join(&mut d, owner, goal, 2, "Maple", Some("reviewer"));
    let records = d.store.log(&goal, before as u64, usize::MAX).unwrap();
    assert!(
        records
            .iter()
            .all(|(_, event)| !matches!(event.header().body, Body::RoleHolders { .. }))
    );
    let member_record = &d.node.goals[&goal].state().members[&member];
    assert!(
        matches!(&d.store.event(&member_record.admission).unwrap().unwrap().header().body,
        Body::MemberAdmitted { name, role, .. } if name == "Maple" && role.as_deref() == Some("reviewer"))
    );
    let expected = d.node.goals[&goal].state().roles["reviewer"].clone();
    assert_eq!(
        code(d.call(
            agent,
            Request::RoleTake {
                goal,
                role: "reviewer".into(),
                member,
                expected
            }
        )),
        ErrorCode::Denied
    );
}

#[test]
fn an_invitation_role_only_earlier_rules_declare_is_given_at_admission() {
    let (mut d, _, owner, _, goal) = setup(Some("directed"));
    bind(&mut d, owner, goal, formation("peer-review")).unwrap();
    let (member, _) = join(&mut d, owner, goal, 2, "Maple", Some("reviewer"));
    assert!(status(&mut d, owner, goal).roles["reviewer"].contains(&member));
}

#[test]
fn member_names_are_signed_at_admission_and_the_host_name_rides_the_ticket() {
    let (mut d, host, owner, _, goal) = setup(None);
    let (member, _) = join(&mut d, owner, goal, 2, "Maple", None);
    let state = status(&mut d, owner, goal);
    assert_eq!(state.host_name.as_deref(), Some("Harbor"));
    assert_eq!(
        state
            .members
            .iter()
            .find(|view| view.member == host)
            .unwrap()
            .name,
        "Harbor"
    );
    assert_eq!(
        state
            .members
            .iter()
            .find(|view| view.member == member)
            .unwrap()
            .name,
        "Maple"
    );
    assert_eq!(d.node.goals[&goal].local.level(&member), Level::Auto);
    d.restart();
    let owner = d.owner();
    assert_eq!(status(&mut d, owner, goal).members, state.members);
}

#[test]
fn review_panel_starts_with_the_host_as_sole_reviewer_and_counts_nothing_until_two_more() {
    let (mut d, host, owner, agent, goal) = setup(Some("review-panel"));
    assert_eq!(status(&mut d, owner, goal).roles["reviewer"], vec![host]);
    let subject = event(d.ok(agent, finding(goal, "a result")));
    assert!(!approved(&d, goal, subject));
    let (_, second) = join(&mut d, owner, goal, 2, "Maple", Some("reviewer"));
    d.ok(
        second,
        Request::ReviewRecord {
            goal,
            subject,
            verdict: ReviewVerdict::Approve,
            text: "yes".into(),
        },
    );
    assert!(!approved(&d, goal, subject));
    let (_, third) = join(&mut d, owner, goal, 3, "Juniper", Some("reviewer"));
    d.ok(
        third,
        Request::ReviewRecord {
            goal,
            subject,
            verdict: ReviewVerdict::Approve,
            text: "yes".into(),
        },
    );
    assert!(approved(&d, goal, subject));
}

#[test]
fn a_second_agent_of_the_same_person_is_a_second_member() {
    let (mut d, _, owner, agent, goal) = setup(None);
    let first = event(d.ok(agent, finding(goal, "alone")));
    assert!(approved(&d, goal, first));
    assert!(status(&mut d, owner, goal).roles.is_empty());
    let (member, second) = join(&mut d, owner, goal, 2, "Maple", None);
    assert!(approved(&d, goal, first));
    let subject = event(d.ok(agent, finding(goal, "with company")));
    assert!(!approved(&d, goal, subject));
    d.ok(
        second,
        Request::ReviewRecord {
            goal,
            subject,
            verdict: ReviewVerdict::Approve,
            text: "yes".into(),
        },
    );
    assert!(approved(&d, goal, subject));
    d.ok(
        owner,
        Request::GoalLeave {
            goal,
            agent: member,
        },
    );
    assert!(d.node.goals[&goal].is_member(&member));
    let pending = event(d.ok(agent, finding(goal, "still two members")));
    assert!(!approved(&d, goal, pending));
}

#[test]
fn a_rules_refusal_names_the_member_and_the_host() {
    let (mut d, _, owner, _, goal) = setup(Some("review-panel"));
    let (_, member) = join(&mut d, owner, goal, 2, "Maple", None);
    let subject = event(d.ok(member, finding(goal, "result")));
    let error = d
        .call(
            member,
            Request::ReviewRecord {
                goal,
                subject,
                verdict: ReviewVerdict::Approve,
                text: "no role".into(),
            },
        )
        .unwrap_err();
    let refused = error.refused().unwrap();
    assert_eq!(refused.member_name.as_deref(), Some("Maple"));
    assert!(
        matches!(&refused.why, locust_proto::api::Why::Rules { host_name: Some(name), .. } if name == "Harbor")
    );
    assert!(!error.message.contains("Maple"));
}

#[test]
fn earlier_missing_definitions_block_role_and_rule_changes_but_status_still_reads() {
    let (mut d, host, owner, _, goal) = setup(Some("directed"));
    d.node.goals.get_mut(&goal).unwrap().definitions = Default::default();
    assert!(status(&mut d, owner, goal).deciding.is_empty());
    assert_eq!(
        code(d.call(
            owner,
            Request::RoleTake {
                goal,
                role: "reviewer".into(),
                member: host,
                expected: vec![host]
            }
        )),
        ErrorCode::Unavailable
    );
    assert_eq!(
        code(d.call(
            owner,
            Request::GoalInvite {
                goal,
                role: None,
                expires_ms: 1_000_000
            }
        )),
        ErrorCode::Unavailable
    );
    assert_eq!(
        code(bind(&mut d, owner, goal, formation("peer-review"))),
        ErrorCode::Unavailable
    );
}

#[test]
fn a_result_whose_approver_rejects_is_listed_for_review_again() {
    let (mut d, _, owner, agent, goal) = setup(None);
    let (_, reviewer) = join(&mut d, owner, goal, 2, "Maple", None);
    let (_, third) = join(&mut d, owner, goal, 3, "Juniper", None);
    let subject = event(d.ok(agent, finding(goal, "peer result")));
    d.ok(
        reviewer,
        Request::ReviewRecord {
            goal,
            subject,
            verdict: ReviewVerdict::Approve,
            text: "yes".into(),
        },
    );
    let Response::Pending(pending) = d.ok(third, Request::Pending { goal }) else {
        panic!()
    };
    assert!(pending.to_review.iter().all(|item| item.subject != subject));
    let rejected = event(d.ok(
        reviewer,
        Request::ReviewRecord {
            goal,
            subject,
            verdict: ReviewVerdict::Reject,
            text: "changed my review".into(),
        },
    ));
    let Response::Pending(pending) = d.ok(third, Request::Pending { goal }) else {
        panic!()
    };
    let item = pending
        .to_review
        .iter()
        .find(|item| item.subject == subject)
        .unwrap();
    assert_eq!(item.approvals, 0);
    assert_eq!(item.verdicts.len(), 1);
    assert_eq!(item.verdicts[0].event, rejected);
    assert!(!item.verdicts[0].approve);
    assert!(!item.verdicts[0].opinion);
    d.restart();
    let third = d.connect(credential(3), Some(session(3)));
    let Response::Pending(restarted) = d.ok(third, Request::Pending { goal }) else {
        panic!()
    };
    assert_eq!(restarted.to_review, pending.to_review);
}

#[test]
fn an_opinion_is_recorded_under_open_and_marked_in_verdicts() {
    let (mut d, _, owner, agent, goal) = setup(Some("open"));
    let (_, reviewer) = join(&mut d, owner, goal, 2, "Maple", None);
    let (_, attestor) = join(&mut d, owner, goal, 3, "Juniper", None);
    let mut checked = formation("open");
    checked.decisions.completion = CompletionRule::Check {
        name: "tests".into(),
        by: Selector::Members,
    };
    bind(&mut d, owner, goal, checked).unwrap();
    let subject = event(d.ok(agent, finding(goal, "check needed")));
    d.ok(
        reviewer,
        Request::ReviewRecord {
            goal,
            subject,
            verdict: ReviewVerdict::Approve,
            text: "an opinion".into(),
        },
    );
    assert!(!approved(&d, goal, subject));
    let Response::Pending(pending) = d.ok(attestor, Request::Pending { goal }) else {
        panic!()
    };
    let item = pending
        .to_review
        .iter()
        .find(|item| item.subject == subject)
        .unwrap();
    assert_eq!(item.approvals, 0);
    assert!(item.verdicts[0].opinion);
    d.ok(
        attestor,
        Request::CheckAttest {
            goal,
            subject,
            name: "tests".into(),
            passed: true,
            text: "passed".into(),
        },
    );
    assert!(approved(&d, goal, subject));
}

#[test]
fn a_picked_result_gets_no_review_request_after_its_approver_rejects() {
    let (mut d, _, owner, agent, goal) = setup(Some("directed"));
    let subject = event(d.ok(agent, finding(goal, "picked result")));
    d.ok(
        agent,
        Request::ReviewRecord {
            goal,
            subject,
            verdict: ReviewVerdict::Approve,
            text: "yes".into(),
        },
    );
    d.ok(
        agent,
        Request::ScopeSelect {
            goal,
            subject,
            expected: None,
        },
    );
    d.ok(
        agent,
        Request::ReviewRecord {
            goal,
            subject,
            verdict: ReviewVerdict::Reject,
            text: "later no".into(),
        },
    );
    let (_, reviewer) = join(&mut d, owner, goal, 2, "Maple", Some("reviewer"));
    let Response::Pending(pending) = d.ok(reviewer, Request::Pending { goal }) else {
        panic!()
    };
    assert!(pending.to_review.iter().all(|item| item.subject != subject));
}

#[test]
fn role_commands_refuse_an_owner_on_a_copy_that_does_not_host_the_goal() {
    let (mut d, host, owner, _, goal) = setup(Some("directed"));
    // This is a held signed copy with no governance key, as on a member's daemon.
    d.node.goals.get_mut(&goal).unwrap().local.governance = None;
    let before = records(&d, goal);
    for request in [
        Request::RoleGive {
            goal,
            role: "lead".into(),
            member: host,
            expected: vec![host],
        },
        Request::RoleTake {
            goal,
            role: "reviewer".into(),
            member: host,
            expected: vec![host],
        },
    ] {
        assert_eq!(code(d.call(owner, request)), ErrorCode::Denied);
    }
    assert_eq!(records(&d, goal), before);
}

#[test]
fn a_departed_author_is_not_asked_to_send_review_requests_to_new_members() {
    for removed in [false, true] {
        let (mut d, _, owner, _, goal) = setup(None);
        let (author, agent) = join(&mut d, owner, goal, 2, "Juniper", None);
        let subject = event(d.ok(agent, finding(goal, "a result before departure")));
        assert!(status(&mut d, owner, goal).stalled.is_empty());
        d.ok(
            owner,
            if removed {
                Request::MemberRemove {
                    goal,
                    member: author,
                }
            } else {
                Request::GoalLeave {
                    goal,
                    agent: author,
                }
            },
        );
        let (_, cedar) = join(&mut d, owner, goal, 3, "Cedar", None);
        assert!(status(&mut d, owner, goal).stalled.is_empty());
        let Response::Pending(pending) = d.ok(cedar, Request::Pending { goal }) else {
            panic!()
        };
        assert!(pending.to_review.iter().any(|item| item.subject == subject));
    }
}
