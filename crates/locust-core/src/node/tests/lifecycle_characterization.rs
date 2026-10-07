//! Revocation is durable local state, not a replicated goal halt.
use super::authorization::{author_of, governance_key, join_local};
use super::lifecycle::{event, finding, setup};
use super::*;
use locust_proto::invite::Invitation;
use locust_proto::store::Store;

#[test]
fn revoking_the_hosts_agent_stops_that_agent_and_not_governance() {
    let (mut d, host, owner, agent, goal) = setup();
    let governance = governance_key(&d, goal).public();
    let (first, _) = join_local(&mut d, agent, goal, 2);
    let (second, _) = join_local(&mut d, agent, goal, 3);
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
    // Marks kept: the data directory is put back, its marks directory survives.
    let backup = snapshot(&d.store);
    let before = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    d.ok(owner, Request::AgentRevoke { agent: host });
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before);
    assert_eq!(
        code(d.call(agent, finding(goal, "revoked"))),
        ErrorCode::Denied
    );
    let mut records = before;
    for (restart, member) in [(false, first), (true, second)] {
        if restart {
            d.restart();
            let (_, answer) = d.hello(credential(1), Some(session(1)));
            assert!(matches!(answer, ServerHello::Refused { .. }));
        }
        let owner = d.owner();
        // The agent is stopped: its credential, its name and its key.
        assert_eq!(
            code(d.on_behalf(owner, host, finding(goal, "revoked"))),
            ErrorCode::NotFound
        );
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
                    role: None,
                    goal,
                    expires_ms: 1_000_000,
                }
            )),
            ErrorCode::Invalid,
        );
        // Governance is not: the host's commands sign with the goal's key.
        let Response::Invited { ticket } = d.ok(
            owner,
            Request::GoalInvite {
                role: None,
                goal,
                expires_ms: 1_000_000,
            },
        ) else {
            panic!()
        };
        assert_eq!(
            Invitation::from_ticket(ticket.as_str()).unwrap().governance,
            governance
        );
        let removed = event(d.ok(owner, Request::MemberRemove { goal, member }));
        assert_eq!(author_of(&d, &removed), governance);
        assert!(!d.node.goals[&goal].is_member(&member));
        records += 1;
        assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), records);
        assert!(d.node.goals[&goal].halted().is_none());
        assert!(d.node.goals[&goal].is_member(&host));
    }
    // Connected again, the agent works under its old key; neither the
    // revoke nor the reconnect left a goal record.
    let owner = d.owner();
    d.ok(owner, Request::AgentReconnect { agent: host });
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), records);
    let agent = d.connect(credential(1), Some(session(1)));
    let posted = event(d.ok(agent, finding(goal, "connected again")));
    assert_eq!(author_of(&d, &posted), host);
    // The store as it was before the revoke still has the agent connected.
    d.store = backup;
    d.restart();
    let agent = d.connect(credential(1), Some(session(1)));
    let owner = d.owner();
    let Response::Status(status) = d.ok(owner, Request::Status) else {
        panic!()
    };
    let view = status
        .agents
        .iter()
        .find(|view| view.agent == host)
        .unwrap();
    assert!(!view.revoked);
    assert_eq!(view.name, name);
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before);
    assert!(d.node.goals[&goal].is_member(&first));
    let posted = event(d.ok(agent, finding(goal, "never revoked here")));
    assert_eq!(author_of(&d, &posted), host);
}
