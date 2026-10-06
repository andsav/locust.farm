//! Revocation is durable local state, not a replicated goal halt.
use super::lifecycle::setup;
use super::*;
use locust_proto::store::Store;

#[test]
fn revoked_host_cannot_resume_governance_through_enrollment_but_old_store_can() {
    let (mut d, host, owner, agent, goal) = setup();
    let (member, _) = super::authorization::join_local(&mut d, agent, goal, 2);
    let Response::Status(status) = d.ok(owner, Request::Status) else {
        panic!()
    };
    let name = status
        .agents
        .iter()
        .find(|view| view.agent == host)
        .unwrap()
        .name
        .clone();
    let backup = snapshot(&d.store);
    let before = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    d.ok(owner, Request::AgentRevoke { agent: host });
    for restart in [false, true] {
        if restart {
            d.restart();
        }
        let owner = d.owner();
        for request in [
            Request::MemberRemove { goal, member },
            Request::GoalInvite {
                goal,
                expires_ms: 1_000_000,
            },
        ] {
            let error = d.call(owner, request).unwrap_err();
            assert_eq!(error.code, ErrorCode::Denied);
            assert_eq!(
                error.message,
                "the host agent is disconnected; nothing can sign for this goal"
            );
        }
        assert_eq!(
            code(d.call(
                owner,
                Request::AgentEnroll {
                    name: name.clone(),
                    credential: credential(1).digest(),
                }
            )),
            ErrorCode::Conflict,
        );
        assert_eq!(
            code(d.on_behalf(
                owner,
                member,
                Request::GoalInvite {
                    goal,
                    expires_ms: 1_000_000,
                }
            )),
            ErrorCode::Invalid,
        );
        assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before);
        assert!(d.node.goals[&goal].halted().is_none());
    }
    // Restoring this old local state restores its unrevoked key. No goal
    // records were signed in between, so this control introduces no fork.
    d.store = backup;
    d.restart();
    let owner = d.owner();
    d.ok(owner, Request::MemberRemove { goal, member });
    assert!(!d.node.goals[&goal].is_member(&member));
}
