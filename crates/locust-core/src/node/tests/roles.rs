//! Role and name changes through the daemon's authenticated request seam.
use super::lifecycle::{event, finding, offered, progress};
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
            no_role: false,
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
fn rules_may_keep_an_unused_deciding_role_but_cannot_turn_an_unused_group_into_a_decider() {
    let (mut d, host, owner, _, goal) = setup(Some("directed"));
    let mut unused = formation("peer-review");
    unused.roles = formation("directed").roles;
    bind(&mut d, owner, goal, unused).unwrap();
    let view = status(&mut d, owner, goal);
    assert_eq!(view.roles["lead"], [host]);
    assert!(view.deciding.contains("lead"));
    let mut later = formation("peer-review");
    later.roles.insert("judge".into(), Default::default());
    bind(&mut d, owner, goal, later.clone()).unwrap();
    later.decisions.selection = Some(Authority::Role {
        name: "judge".into(),
    });
    let before = records(&d, goal);
    assert_eq!(code(bind(&mut d, owner, goal, later)), ErrorCode::Conflict);
    assert_eq!(records(&d, goal), before);
}

#[test]
fn oversized_role_give_and_take_report_a_plain_limit_and_sign_nothing() {
    use crate::node::authoring::{Place, sign_at};
    use crate::node::commit::Tx;
    use locust_proto::crypto::Keypair;
    let (mut d, host, _, _, goal) = setup(Some("review-panel"));
    let entry = &d.node.goals[&goal];
    let signer = entry.local.governance.as_ref().unwrap();
    let endpoint = entry.state().members[&host].endpoint;
    let head = entry.state().head.unwrap();
    let mut previous = d.store.event(&head).unwrap().unwrap();
    let mut tx = Tx::none();
    let count = locust_proto::limits::MAX_HEADER_BYTES / 32 + 8;
    let mut last = host;
    for index in 0..=count {
        let mut seed = [0x71; 32];
        seed[..8].copy_from_slice(&(index as u64).to_le_bytes());
        let member = Keypair::from_seed(seed).public();
        previous = sign_at(
            goal,
            signer,
            Place {
                seq: previous.header().seq + 1,
                prev: Some(previous.id()),
                anchor: Some(previous.id()),
                epoch: 0,
            },
            Body::MemberAdmitted {
                member,
                endpoint,
                name: format!("Member {index}"),
                role: (index < count).then(|| "reviewer".into()),
            },
            None,
            100 + index as u64,
            &mut tx,
        )
        .unwrap();
        last = member;
    }
    d.store.commit(&tx.commit).unwrap();
    d.restart();
    let owner = d.owner();
    let expected = d.node.goals[&goal].state().roles["reviewer"].clone();
    let before = records(&d, goal);
    for request in [
        Request::RoleGive {
            goal,
            role: "reviewer".into(),
            member: last,
            expected: expected.clone(),
        },
        Request::RoleTake {
            goal,
            role: "reviewer".into(),
            member: host,
            expected: expected.clone(),
        },
    ] {
        let error = d.call(owner, request).unwrap_err();
        assert_eq!(error.code, ErrorCode::LimitExceeded);
        assert!(error.message.contains("reviewer"));
        assert!(error.message.contains("too many holders"));
        assert!(
            error
                .message
                .contains("Remove a member from the goal or use another role.")
        );
        assert!(!error.message.contains("header"));
        assert_eq!(records(&d, goal), before);
    }
    let expected_rules = d.node.goals[&goal].state().current_rules.unwrap();
    let error = bind(&mut d, owner, goal, formation("review-panel")).unwrap_err();
    assert_eq!(error.code, ErrorCode::LimitExceeded);
    assert!(error.message.contains("every member a \"reviewer\""));
    assert!(error.message.contains("--no-role"), "{}", error.message);
    assert!(!error.message.contains("another role"), "{}", error.message);
    assert_eq!(records(&d, goal), before);
    assert_eq!(
        d.node.goals[&goal].state().current_rules,
        Some(expected_rules)
    );
    d.ok(
        owner,
        Request::RulesBind {
            goal,
            expected: expected_rules,
            formation_json: serde_json::to_string(&formation("review-panel")).unwrap(),
            inputs: Default::default(),
            no_role: true,
        },
    );
    assert_eq!(records(&d, goal), before + 1);
}

#[test]
fn rules_bind_gives_the_counting_role_in_one_commit_unless_no_role_is_set() {
    use locust_proto::event::WorkspaceCheckpoint;
    use locust_proto::organization::WorkspacePolicy;
    for no_role in [false, true] {
        for with_workspace in [false, true] {
            let (mut d, host, owner, _, goal) = setup(Some("peer-review"));
            let (maple, _) = join(&mut d, owner, goal, 2, "Maple", None);
            let (juniper, _) = join(&mut d, owner, goal, 3, "Juniper", None);
            let workspace = with_workspace.then(|| WorkspacePolicy {
                integrator: Authority::Participant {
                    key: host.to_string(),
                },
                completion: CompletionRule::default(),
            });
            if with_workspace {
                let mut initial = formation("peer-review");
                initial.workspace = workspace.clone();
                let Response::Recorded { event: rules } =
                    bind(&mut d, owner, goal, initial).unwrap()
                else {
                    panic!()
                };
                d.ok(
                    owner,
                    Request::WorkspaceEpochSet {
                        goal,
                        expected_epoch: None,
                        rules,
                        checkpoint: WorkspaceCheckpoint::Unseeded,
                    },
                );
            }
            let mut next = formation("review-panel");
            next.workspace = workspace;
            let expected = d.node.goals[&goal].state().current_rules.unwrap();
            let before = records(&d, goal);
            let revision = d.node.goals[&goal].local.revision;
            let Response::Recorded { event: rules } = d.ok(
                owner,
                Request::RulesBind {
                    goal,
                    expected,
                    formation_json: serde_json::to_string(&next).unwrap(),
                    inputs: Default::default(),
                    no_role,
                },
            ) else {
                panic!()
            };
            assert_eq!(d.node.goals[&goal].local.revision, revision + 1);
            let appended = d.store.log(&goal, before as u64, usize::MAX).unwrap();
            assert_eq!(
                appended.len(),
                1 + usize::from(with_workspace) + usize::from(!no_role)
            );
            assert_eq!(appended[0].1.id(), rules);
            for pair in appended.windows(2) {
                assert_eq!(pair[1].1.header().prev, Some(pair[0].1.id()));
                assert_eq!(pair[1].1.header().anchor, Some(pair[0].1.id()));
            }
            if with_workspace {
                assert!(
                    matches!(appended[1].1.header().body, Body::WorkspaceEpoch { rules: actual, .. } if actual == rules)
                );
            }
            let mut expected_holders = if no_role {
                vec![host]
            } else {
                vec![host, maple, juniper]
            };
            expected_holders.sort();
            assert_eq!(
                d.node.goals[&goal].state().roles["reviewer"],
                expected_holders
            );
            if !no_role {
                assert!(matches!(&appended.last().unwrap().1.header().body,
                Body::RoleHolders { role, holders } if role == "reviewer" && *holders == expected_holders));
            }
            d.restart();
            assert_eq!(
                d.node.goals[&goal].state().roles["reviewer"],
                expected_holders
            );
        }
    }
}

#[test]
fn binding_counted_reviews_over_rules_where_one_reviewer_acts_alone_gives_the_role_to_no_one() {
    let (mut d, host, owner, lead, goal) = setup(Some("directed"));
    let (maple, maple_conn) = join(&mut d, owner, goal, 2, "Maple", None);
    let (task, offer) = offered(&mut d, lead, goal, maple);
    let Response::Claimed(claim) = d.ok(
        maple_conn,
        Request::AttemptStart {
            goal,
            task: Some(task),
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    assert!(
        status(&mut d, owner, goal)
            .acting_alone
            .contains("reviewer")
    );
    let before = records(&d, goal);
    bind(&mut d, owner, goal, formation("review-panel")).unwrap();
    // One record: the binding alone, no role list.
    assert_eq!(records(&d, goal), before + 1);
    assert_eq!(d.node.goals[&goal].state().roles["reviewer"], vec![host]);
    // The open task keeps the directed rules, under which one reviewer
    // approves alone, even its own result; Maple holds no role there.
    d.ok(maple_conn, progress(goal, claim.attempt, 1));
    let result = event(d.ok(
        maple_conn,
        Request::ContributionPublish {
            goal,
            attempt: Some(claim.attempt),
            generation: Some(1),
            summary: "done".into(),
            sources: Vec::new(),
            artifacts: vec![],
        },
    ));
    let _ = d.call(
        maple_conn,
        Request::ReviewRecord {
            goal,
            subject: result,
            verdict: ReviewVerdict::Approve,
            text: "mine".into(),
        },
    );
    assert!(!approved(&d, goal, result));
    d.ok(
        lead,
        Request::ReviewRecord {
            goal,
            subject: result,
            verdict: ReviewVerdict::Approve,
            text: "reviewed".into(),
        },
    );
    assert!(approved(&d, goal, result));
    // Without such earlier rules the bind gives the role as before.
    let (mut d, host, owner, _, goal) = setup(Some("peer-review"));
    let (maple, _) = join(&mut d, owner, goal, 2, "Maple", None);
    assert!(status(&mut d, owner, goal).acting_alone.is_empty());
    bind(&mut d, owner, goal, formation("review-panel")).unwrap();
    let mut holders = vec![host, maple];
    holders.sort();
    assert_eq!(d.node.goals[&goal].state().roles["reviewer"], holders);
}

#[test]
fn an_invitation_role_is_given_on_admission_and_an_authority_role_is_refused_at_issue() {
    let (mut d, _, owner, agent, goal) = setup(Some("directed"));
    for role in ["lead", "absent"] {
        let error = d
            .call(
                owner,
                Request::GoalInvite {
                    goal,
                    role: Some(role.into()),
                    expires_ms: 1_000_000,
                },
            )
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Invalid);
        let details: serde_json::Value =
            serde_json::from_str(error.details_json.as_deref().unwrap()).unwrap();
        assert_eq!(details["role"], role);
        if role == "absent" {
            // The goal's roles travel with the refusal so the person can
            // pick the right one without a second command.
            assert_eq!(details["roles"], serde_json::json!(["lead", "reviewer"]));
        }
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
    // The daemon hosts the goal, and Maple wrote the result it tried to review.
    assert!(matches!(
        &refused.why,
        locust_proto::api::Why::Rules {
            host_name: Some(name),
            hosted_here: true,
            except_author: true,
            author: true,
            ..
        } if name == "Harbor"
    ));
    assert!(!error.message.contains("Maple"));
    assert!(
        error
            .message
            .ends_with("Nothing to change; pick other work."),
        "{}",
        error.message
    );
    // A reviewer who did hold the role is refused for its authorship alone.
    let (_, reviewer) = join(&mut d, owner, goal, 3, "Cedar", Some("reviewer"));
    let own = event(d.ok(reviewer, finding(goal, "own result")));
    let error = d
        .call(
            reviewer,
            Request::ReviewRecord {
                goal,
                subject: own,
                verdict: ReviewVerdict::Approve,
                text: "mine".into(),
            },
        )
        .unwrap_err();
    assert!(
        error.message.contains("is the author") && !error.message.contains("does not hold"),
        "{}",
        error.message
    );
}

#[test]
fn an_offer_no_rule_covers_is_refused_without_a_blank() {
    let (mut d, _, owner, agent, goal) = setup(Some("open"));
    let (member, _) = join(&mut d, owner, goal, 2, "Maple", None);
    let task = locust_proto::event::TaskId::Authored(event(d.ok(
        agent,
        Request::TaskOpen {
            goal,
            text: "Fix the parser".into(),
            task_type: None,
            inputs: Default::default(),
            parent: None,
        },
    )));
    let error = d
        .call(
            agent,
            Request::WorkOffer {
                goal,
                task,
                recipient: member,
            },
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::NotEligible);
    assert_eq!(
        error.message,
        "host can't hand out this task in this goal: these rules let nobody hand a task to that member (the goal's rules). Nothing to change; pick other work."
    );
    let refused = error.refused().unwrap();
    assert!(matches!(
        &refused.why,
        locust_proto::api::Why::Rules {
            rule: locust_proto::api::Rule::Offer,
            qualifies: Selector::Any { selectors },
            ..
        } if selectors.is_empty()
    ));
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
