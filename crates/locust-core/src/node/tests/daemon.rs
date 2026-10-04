//! Opening, credentials, enrollment, status and stopping.

use locust_proto::API_VERSION;
use locust_proto::api::{
    Caller, ClientHello, Credential, ErrorCode, Grants, Request, Response, ServerHello,
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
    let agent = daemon.enroll("mira", 1, false);
    let (_, answer) = daemon.hello(credential(1), Some(session(1)));
    assert_eq!(caller_of(&answer), Some(Caller::Agent(agent)));
}

#[test]
fn enrolling_again_with_the_same_name_and_credential_returns_the_same_principal() {
    let mut daemon = Daemon::new(1);
    let first = daemon.enroll("mira", 1, false);
    let second = daemon.enroll("mira", 1, false);
    assert_eq!(first, second);
}

#[test]
fn a_taken_name_with_another_credential_is_a_conflict() {
    let mut daemon = Daemon::new(1);
    daemon.enroll("mira", 1, false);
    let owner = daemon.owner();
    let refused = daemon.call(
        owner,
        Request::AgentEnroll {
            name: "mira".into(),
            grants: Grants::default(),
            credential: credential(2).digest(),
        },
    );
    assert_eq!(code(refused), ErrorCode::Conflict);
}

#[test]
fn a_credential_that_already_names_something_is_a_conflict() {
    let mut daemon = Daemon::new(1);
    daemon.enroll("mira", 1, false);
    let owner = daemon.owner();
    for taken in [credential(1), OWNER] {
        let refused = daemon.call(
            owner,
            Request::AgentEnroll {
                name: "noor".into(),
                grants: Grants::default(),
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
            grants: Grants::default(),
            credential: credential(1).digest(),
        },
    );
    assert_eq!(code(refused), ErrorCode::Invalid);
}

#[test]
fn an_agent_may_not_make_owner_only_requests() {
    let mut daemon = Daemon::new(1);
    daemon.enroll("mira", 1, false);
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
    let mira = daemon.enroll("mira", 1, false);
    let agent = daemon.connect(credential(1), None);
    let refused = daemon.on_behalf(agent, mira, Request::Status);
    assert_eq!(code(refused), ErrorCode::Denied);
}

#[test]
fn the_owner_naming_on_behalf_on_an_owner_only_request_is_invalid() {
    let mut daemon = Daemon::new(1);
    let mira = daemon.enroll("mira", 1, false);
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
    let mira = daemon.enroll("mira", 1, true);
    let noor = daemon.enroll("noor", 2, false);

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
    assert!(status.agents[0].grants.manage_goals);
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
    let mira = daemon.enroll("mira", 1, false);
    let secret = daemon.node.endpoint_secret();
    assert_ne!(secret, [0; 32]);

    daemon.restart();
    assert_eq!(daemon.node.endpoint_secret(), secret);
    let (_, answer) = daemon.hello(credential(1), None);
    assert_eq!(caller_of(&answer), Some(Caller::Agent(mira)));
    // The same enrollment still answers with the same principal.
    assert_eq!(daemon.enroll("mira", 1, false), mira);
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
