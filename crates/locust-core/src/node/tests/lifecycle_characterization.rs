//! Revocation is durable local state, not a replicated goal halt.
use super::lifecycle::setup;
use super::*;
use locust_proto::store::Store;

#[test]
fn revoked_host_cannot_resume_governance_through_grants_or_enrollment_but_old_store_can() {
    let (mut d, host, owner, agent, goal) = setup();
    let (member, _) = super::authorization::join_local(&mut d, agent, goal, 2);
    let backup = snapshot(&d.store);
    let before = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    d.ok(owner, Request::AgentRevoke { agent: host });
    for restart in [false, true] {
        if restart {
            d.restart();
        }
        let owner = d.owner();
        assert!(
            d.on_behalf(owner, host, Request::MemberRemove { goal, member })
                .is_err()
        );
        assert!(
            d.call(
                owner,
                Request::AgentGrant {
                    agent: host,
                    grants: Grants { manage_goals: true }
                }
            )
            .is_err()
        );
        assert_eq!(
            code(d.call(
                owner,
                Request::AgentEnroll {
                    name: "administrator".into(),
                    grants: Grants { manage_goals: true },
                    credential: credential(1).digest()
                }
            )),
            ErrorCode::Conflict
        );
        d.ok(
            owner,
            Request::GoalGrant {
                goal,
                agent: host,
                grants: locust_proto::api::GoalGrants {
                    administer: true,
                    ..Default::default()
                },
            },
        );
        assert!(
            d.on_behalf(
                owner,
                host,
                Request::GoalInvite {
                    goal,
                    expires_ms: None
                }
            )
            .is_err()
        );
        let error = d
            .on_behalf(
                owner,
                member,
                Request::GoalInvite {
                    goal,
                    expires_ms: None,
                },
            )
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Denied);
        assert_eq!(
            error.message,
            "only the goal's administrator makes this request"
        );
        assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before);
        assert!(d.node.goals[&goal].halted().is_none());
    }
    // Restoring this old local state restores its unrevoked key. No goal
    // records were signed in between, so this control introduces no fork.
    d.store = backup;
    d.restart();
    let agent = d.connect(credential(1), None);
    d.ok(agent, Request::MemberRemove { goal, member });
    assert!(!d.node.goals[&goal].is_member(&member));
}
