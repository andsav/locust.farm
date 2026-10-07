//! Opening, credentials, enrollment, status and stopping.

use locust_proto::API_VERSION;
use locust_proto::api::{
    Caller, ClientHello, Credential, ErrorCode, Request, Response, ServerHello,
};
use locust_proto::engine::{ConnId, Engine, PeerEngine};

use super::{Daemon, OWNER, caller_of, code, credential, session};

#[test]
fn the_owner_credential_is_welcomed_as_the_owner() {
    let mut daemon = Daemon::new(1);
    let (_, answer) = daemon.hello(OWNER, None);
    assert_eq!(caller_of(&answer), Some(Caller::Owner));
}

#[test]
fn an_unknown_credential_is_refused_as_denied() {
    let mut daemon = Daemon::new(1);
    let (_, answer) = daemon.hello(credential(9), None);
    let ServerHello::Refused { error, .. } = answer else {
        panic!("welcomed an unknown credential");
    };
    assert_eq!(error.code, ErrorCode::Denied);
}

#[test]
fn another_api_version_is_refused_as_unsupported() {
    let mut daemon = Daemon::new(1);
    let hello = ClientHello {
        api_version: API_VERSION + 1,
        credential: OWNER,
        session: None,
    };
    let ServerHello::Refused { error, .. } = daemon.node.connect(ConnId(1), &hello, 0) else {
        panic!("welcomed another version");
    };
    assert_eq!(error.code, ErrorCode::UnsupportedVersion);
}

#[test]
fn an_enrolled_agent_is_welcomed_as_its_principal() {
    let mut daemon = Daemon::new(1);
    let agent = daemon.enroll("mira", 1);
    let (_, answer) = daemon.hello(credential(1), Some(session(1)));
    assert_eq!(caller_of(&answer), Some(Caller::Agent(agent)));
}

#[test]
fn enrolling_again_with_the_same_name_and_credential_returns_the_same_principal() {
    let mut daemon = Daemon::new(1);
    let first = daemon.enroll("mira", 1);
    let second = daemon.enroll("mira", 1);
    assert_eq!(first, second);
}

#[test]
fn a_taken_name_with_another_credential_is_a_conflict() {
    let mut daemon = Daemon::new(1);
    daemon.enroll("mira", 1);
    let owner = daemon.owner();
    let refused = daemon.call(
        owner,
        Request::AgentEnroll {
            name: "mira".into(),
            credential: credential(2).digest(),
        },
    );
    assert_eq!(code(refused), ErrorCode::Conflict);
}

#[test]
fn a_credential_that_already_names_something_is_a_conflict() {
    let mut daemon = Daemon::new(1);
    daemon.enroll("mira", 1);
    let owner = daemon.owner();
    for taken in [credential(1), OWNER] {
        let refused = daemon.call(
            owner,
            Request::AgentEnroll {
                name: "noor".into(),
                credential: taken.digest(),
            },
        );
        assert_eq!(code(refused), ErrorCode::Conflict);
    }
}

#[test]
fn an_invalid_name_is_refused_before_anything_else() {
    let mut daemon = Daemon::new(1);
    let owner = daemon.owner();
    let refused = daemon.call(
        owner,
        Request::AgentEnroll {
            name: "Mira".into(),
            credential: credential(1).digest(),
        },
    );
    assert_eq!(code(refused), ErrorCode::Invalid);
}

#[test]
fn an_agent_may_not_make_owner_only_requests() {
    let mut daemon = Daemon::new(1);
    daemon.enroll("mira", 1);
    let agent = daemon.connect(credential(1), None);
    assert_eq!(
        code(daemon.call(agent, Request::Shutdown)),
        ErrorCode::Denied
    );
    assert!(!daemon.node.stop_requested());
}

#[test]
fn an_agent_naming_on_behalf_is_denied() {
    let mut daemon = Daemon::new(1);
    let mira = daemon.enroll("mira", 1);
    let agent = daemon.connect(credential(1), None);
    let refused = daemon.on_behalf(agent, mira, Request::Status);
    assert_eq!(code(refused), ErrorCode::Denied);
}

#[test]
fn the_owner_naming_on_behalf_on_an_owner_only_request_is_invalid() {
    let mut daemon = Daemon::new(1);
    let mira = daemon.enroll("mira", 1);
    let owner = daemon.owner();
    let refused = daemon.on_behalf(owner, mira, Request::Shutdown);
    assert_eq!(code(refused), ErrorCode::Invalid);
}

#[test]
fn the_owner_naming_an_unknown_principal_is_not_found() {
    let mut daemon = Daemon::new(1);
    let owner = daemon.owner();
    let stranger = locust_proto::testkit::keypair(7).public();
    let refused = daemon.on_behalf(owner, stranger, Request::Status);
    assert_eq!(code(refused), ErrorCode::NotFound);
}

#[test]
fn status_shows_the_owner_every_principal_and_an_agent_only_itself() {
    let mut daemon = Daemon::new(1);
    let mira = daemon.enroll("mira", 1);
    let noor = daemon.enroll("noor", 2);

    let owner = daemon.owner();
    let Response::Status(status) = daemon.ok(owner, Request::Status) else {
        unreachable!()
    };
    assert_eq!(status.daemon_version, "test");
    assert_eq!(status.endpoint, None);
    let mut keys: Vec<_> = status.agents.iter().map(|agent| agent.agent).collect();
    keys.sort();
    let mut expected = vec![mira, noor];
    expected.sort();
    assert_eq!(keys, expected);

    let agent = daemon.connect(credential(1), None);
    let Response::Status(status) = daemon.ok(agent, Request::Status) else {
        unreachable!()
    };
    assert_eq!(status.agents.len(), 1);
    assert_eq!(status.agents[0].agent, mira);
    assert_eq!(status.agents[0].name, "mira");
    assert!(!status.agents[0].author_only);
    assert!(status.waiting.is_empty());
    assert!(status.goals.is_empty());

    // A hosted goal with one open invitation: the owner sees its count and
    // expiry, the agent none, and both see the agent's name in the goal.
    let (mut d, principal, owner, agent, goal) = super::lifecycle::setup();
    d.ok(
        owner,
        Request::GoalInvite {
            role: None,
            goal,
            expires_ms: 604_801_000,
        },
    );
    let Response::Status(status) = d.ok(owner, Request::Status) else {
        unreachable!()
    };
    assert_eq!(status.goals.len(), 1);
    assert_eq!(status.goals[0].member, principal);
    assert_eq!(status.goals[0].name, "host");
    assert_eq!(status.goals[0].host_name.as_deref(), Some("host"));
    assert_eq!(status.goals[0].invitations_open, 1);
    assert_eq!(status.goals[0].invitations_expire_ms, Some(604_801_000));
    assert!(status.waiting.is_empty());
    let Response::Status(status) = d.ok(agent, Request::Status) else {
        unreachable!()
    };
    assert_eq!(status.goals.len(), 1);
    assert_eq!(status.goals[0].invitations_open, 0);
    assert_eq!(status.goals[0].invitations_expire_ms, None);
    d.ok(
        owner,
        Request::InvitationRevoke {
            goal,
            invitation: None,
        },
    );
    let Response::Status(status) = d.ok(owner, Request::Status) else {
        unreachable!()
    };
    assert_eq!(status.goals[0].invitations_open, 0);

    // With two local agents in the goal, each sees only its own entry and
    // its own waiting line; the owner sees both entries.
    let (task, offer) = super::lifecycle::offered(&mut d, agent, goal, principal);
    assert_eq!(
        d.call(
            agent,
            Request::AttemptStart {
                goal,
                task,
                offer: Some(offer),
            },
        )
        .unwrap_err()
        .code,
        ErrorCode::LevelRequired
    );
    let (second, second_conn) = super::authorization::join_local(&mut d, agent, goal, 2);
    let Response::Status(status) = d.ok(second_conn, Request::Status) else {
        unreachable!()
    };
    assert_eq!(status.goals.len(), 1);
    assert_eq!(status.goals[0].member, second);
    assert!(status.waiting.is_empty());
    let Response::Status(status) = d.ok(agent, Request::Status) else {
        unreachable!()
    };
    assert_eq!(status.goals.len(), 1);
    assert_eq!(status.goals[0].member, principal);
    assert_eq!(status.waiting.len(), 1);
    let Response::Status(status) = d.ok(owner, Request::Status) else {
        unreachable!()
    };
    assert_eq!(status.goals.len(), 2);
    assert_eq!(status.waiting.len(), 1);
}

#[test]
fn status_lists_what_waits_for_the_owner_with_a_ready_command() {
    use locust_proto::api::{Level, WaitingKind};
    let (mut d, principal, owner, agent, goal) = super::lifecycle::setup();
    let (task, offer) = super::lifecycle::offered(&mut d, agent, goal, principal);
    let set_level = |d: &mut Daemon, level| {
        d.ok(
            owner,
            Request::LevelSet {
                goal,
                agent: principal,
                level,
            },
        );
    };
    let start = |d: &mut Daemon| {
        d.call(
            agent,
            Request::AttemptStart {
                goal,
                task,
                offer: Some(offer),
            },
        )
    };
    let waiting = |d: &mut Daemon, conn| {
        let Response::Status(status) = d.ok(conn, Request::Status) else {
            unreachable!()
        };
        status.waiting
    };
    set_level(&mut d, Level::Ask);
    assert_eq!(start(&mut d).unwrap_err().code, ErrorCode::LevelRequired);

    let entries = waiting(&mut d, owner);
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.goal, goal);
    assert_eq!(entry.title.as_deref(), Some("A test goal"));
    assert_eq!(entry.agent, Some(principal));
    assert_eq!(entry.agent_name.as_deref(), Some("host"));
    // The title is the task's first line, not its whole text.
    assert_eq!(
        entry.kind,
        WaitingKind::AllowTask {
            task,
            task_title: Some("Read and implement".into()),
        }
    );
    let goal_prefix = &goal.to_string()[..8];
    let task_prefix = &task.to_string()[..13];
    assert_eq!(
        entry.command,
        format!("locust --owner allow --goal {goal_prefix} --task {task_prefix} --agent host")
    );
    // The agent gets its own entry too.
    assert_eq!(waiting(&mut d, agent), entries);

    // Allowing settles it; the start then goes through.
    d.ok(
        owner,
        Request::TaskAllow {
            goal,
            agent: principal,
            task,
        },
    );
    assert!(waiting(&mut d, owner).is_empty());
    assert!(matches!(start(&mut d), Ok(Response::Claimed(_))));

    // So does a disallow, a finished task and the level auto.
    let (other, other_offer) = super::lifecycle::offered(&mut d, agent, goal, principal);
    let start_other = |d: &mut Daemon| {
        d.call(
            agent,
            Request::AttemptStart {
                goal,
                task: other,
                offer: Some(other_offer),
            },
        )
    };
    assert_eq!(
        start_other(&mut d).unwrap_err().code,
        ErrorCode::LevelRequired
    );
    assert_eq!(waiting(&mut d, owner).len(), 1);
    d.ok(
        owner,
        Request::TaskDisallow {
            goal,
            agent: principal,
            task: other,
        },
    );
    assert!(waiting(&mut d, owner).is_empty());
    assert_eq!(
        start_other(&mut d).unwrap_err().code,
        ErrorCode::LevelRequired
    );
    assert_eq!(waiting(&mut d, owner).len(), 1);
    set_level(&mut d, Level::Auto);
    assert!(waiting(&mut d, owner).is_empty());
    set_level(&mut d, Level::Ask);
    assert_eq!(waiting(&mut d, owner).len(), 1);
    d.ok(
        agent,
        Request::ScopeClose {
            goal,
            scope: locust_proto::event::Scope::Task(other),
            expected: None,
        },
    );
    assert!(waiting(&mut d, owner).is_empty());

    // A halted goal and a joining agent give no entry: each shows under its
    // goal instead.
    assert_eq!(start_other(&mut d).unwrap_err().code, ErrorCode::Conflict);
    let (third, third_offer) = super::lifecycle::offered(&mut d, agent, goal, principal);
    assert_eq!(
        d.call(
            agent,
            Request::AttemptStart {
                goal,
                task: third,
                offer: Some(third_offer),
            },
        )
        .unwrap_err()
        .code,
        ErrorCode::LevelRequired
    );
    let mut summaries = d.node.goal_summaries(None, 1_000).unwrap();
    assert_eq!(d.node.waiting_for(&summaries).len(), 1);
    summaries[0].halted = Some(locust_proto::api::Halt::AuthorityConflict);
    assert!(d.node.waiting_for(&summaries).is_empty());
    summaries[0].halted = None;
    summaries[0].membership = locust_proto::api::Membership::Joining;
    assert!(d.node.waiting_for(&summaries).is_empty());

    // At read an allowance would not help, so the entry asks for the level.
    set_level(&mut d, Level::Read);
    let entries = waiting(&mut d, owner);
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].kind,
        WaitingKind::SetAsk {
            task: third,
            task_title: Some("Read and implement".into()),
        }
    );
    assert_eq!(
        entries[0].command,
        format!("locust --owner level --goal {goal_prefix} --agent host ask")
    );
    set_level(&mut d, Level::Ask);
    assert!(matches!(
        waiting(&mut d, owner)[0].kind,
        WaitingKind::AllowTask { task, .. } if task == third
    ));

    // A disconnected agent gives no entry until it is connected again.
    d.ok(owner, Request::AgentRevoke { agent: principal });
    assert!(waiting(&mut d, owner).is_empty());
    d.ok(owner, Request::AgentReconnect { agent: principal });
    assert_eq!(waiting(&mut d, owner).len(), 1);

    // Nor does a task the agent already holds an attempt on, even though the
    // want stays recorded while its round is open.
    set_level(&mut d, Level::Auto);
    assert!(matches!(
        d.call(
            agent,
            Request::AttemptStart {
                goal,
                task: third,
                offer: Some(third_offer),
            },
        ),
        Ok(Response::Claimed(_))
    ));
    set_level(&mut d, Level::Ask);
    let Response::GoalStatus(status) = d.ok(owner, Request::GoalStatus { goal }) else {
        unreachable!()
    };
    assert_eq!(status.abilities[0].wanted_tasks.len(), 1);
    assert!(waiting(&mut d, owner).is_empty());

    // Nor a task the rules no longer let it start: here, a declined offer.
    let (fourth, fourth_offer) = super::lifecycle::offered(&mut d, agent, goal, principal);
    assert_eq!(
        d.call(
            agent,
            Request::AttemptStart {
                goal,
                task: fourth,
                offer: Some(fourth_offer),
            },
        )
        .unwrap_err()
        .code,
        ErrorCode::LevelRequired
    );
    assert_eq!(waiting(&mut d, owner).len(), 1);
    d.ok(
        agent,
        Request::WorkDecline {
            goal,
            offer: fourth_offer,
        },
    );
    assert!(waiting(&mut d, owner).is_empty());
}

#[test]
fn stopping_is_answered_and_then_reported_to_the_shell() {
    let mut daemon = Daemon::new(1);
    let owner = daemon.owner();
    assert!(!daemon.node.stop_requested());
    assert_eq!(daemon.ok(owner, Request::Shutdown), Response::Done);
    assert!(daemon.node.stop_requested());
}

#[test]
fn a_request_on_an_unwelcomed_connection_is_denied() {
    let mut daemon = Daemon::new(1);
    let refused = daemon.call(ConnId(77), Request::Status);
    assert_eq!(code(refused), ErrorCode::Denied);
}

#[test]
fn a_restart_keeps_the_endpoint_secret_and_the_principals() {
    let mut daemon = Daemon::new(1);
    let mira = daemon.enroll("mira", 1);
    let secret = daemon.node.endpoint_secret();
    assert_ne!(secret, [0; 32]);

    daemon.restart();
    assert_eq!(daemon.node.endpoint_secret(), secret);
    let (_, answer) = daemon.hello(credential(1), None);
    assert_eq!(caller_of(&answer), Some(Caller::Agent(mira)));
    // The same enrollment still answers with the same principal.
    assert_eq!(daemon.enroll("mira", 1), mira);
}

#[test]
fn a_changed_owner_credential_replaces_the_old_one_at_start() {
    let daemon = Daemon::new(1);
    let renewed = Credential([0xB0; 32]);
    let mut node = super::Node::open(
        daemon.store.reopen(),
        super::Counting::new(2),
        renewed.digest(),
        "test".into(),
        0,
    )
    .unwrap();
    let hello = |credential| ClientHello {
        api_version: API_VERSION,
        credential,
        session: None,
    };
    let welcomed = node.connect(ConnId(1), &hello(renewed), 0);
    assert_eq!(caller_of(&welcomed), Some(Caller::Owner));
    let refused = node.connect(ConnId(2), &hello(OWNER), 0);
    assert_eq!(caller_of(&refused), None);
}
