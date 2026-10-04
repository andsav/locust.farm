use super::*;
use crate::config::BridgePaths;
use locust_proto::id::EventId;
use serde_json::json;
use std::cell::RefCell;

fn spec(client: Client, workspace: PathBuf) -> LaunchSpec {
    LaunchSpec {
        client,
        version: "fixture-1".into(),
        executable: "/bin/echo".into(),
        workspace,
        profile: "/tmp/managed-profile".into(),
        bridge: StdioServer {
            executable: "/bin/echo".into(),
            arguments: vec!["mcp".into()],
            paths: BridgePaths {
                home: "/tmp/daemon".into(),
                credential: "/tmp/agent.credential".into(),
                session: "/tmp/session.secret".into(),
            },
        },
        launch_id: "local-launch-1".into(),
        binding: Binding {
            instance: InstanceId([1; 16]),
            principal: PublicKey([2; 32]),
            goal: Some(GoalId([3; 32])),
            attempt: Some(AttemptBinding {
                task: locust_proto::event::TaskId::Authored(EventId([4; 32])),
                attempt: EventId([5; 32]),
            }),
            claim: None,
        },
        mode: Mode::New,
        arguments: vec![],
        global_arguments: vec![],
        environment: BTreeMap::new(),
        prompt: "PRIVATE PROMPT".into(),
        pi_session: Some("/tmp/native.jsonl".into()),
        lifecycle_receipt: "/tmp/launch-tools-ready.jsonl".into(),
    }
}

#[test]
fn all_clients_prepare_new_resume_without_permission_bypass() {
    let dir = tempfile::tempdir().unwrap();
    for client in [
        Client::Codex,
        Client::ClaudeCode,
        Client::FactoryDroid,
        Client::Pi,
    ] {
        let plan = prepare(
            spec(client, dir.path().into()),
            &[],
            &json!({"unrelated":true}),
        )
        .unwrap();
        let args = plan
            .arguments
            .iter()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>();
        assert!(!args.iter().any(|a| a.contains("skip-permissions")
            || a.contains("danger")
            || a.contains("trust")));
        assert_eq!(args.last().unwrap(), "PRIVATE PROMPT");
        if let Some(config) = plan.configuration {
            assert_eq!(config.document["unrelated"], true);
        }
        let mut resume = spec(client, dir.path().into());
        resume.mode = Mode::Resume("native-id".into());
        let plan = prepare(resume, &[], &json!({})).unwrap();
        if client != Client::Pi {
            assert!(plan.arguments.contains(&"native-id".into()));
        }
    }
}

#[test]
fn launch_intent_is_durable_before_spawn_and_failed_intent_prevents_side_effect() {
    let dir = tempfile::tempdir().unwrap();
    let mut candidate = spec(Client::Codex, dir.path().into());
    candidate.executable = "/definitely/no/binary".into();
    let plan = prepare(candidate, &[], &json!({})).unwrap();
    let states = RefCell::new(Vec::new());
    assert!(
        launch(plan, 100, |record| {
            states.borrow_mut().push(record.state);
            Ok(())
        })
        .is_err()
    );
    assert_eq!(
        *states.borrow(),
        vec![SessionState::Launching, SessionState::Exited]
    );
    let plan = prepare(spec(Client::Codex, dir.path().into()), &[], &json!({})).unwrap();
    assert!(launch(plan, 100, |_| Err(Error::report_failed())).is_err());
}

#[test]
fn native_and_authenticated_tools_are_independent_and_delivery_survives() {
    let dir = tempfile::tempdir().unwrap();
    let plan = prepare(spec(Client::Codex, dir.path().into()), &[], &json!({})).unwrap();
    let mut owned = launch(plan, 100, |record| {
        assert!(!String::from_utf8_lossy(&record.detail).contains("PRIVATE"));
        Ok(())
    })
    .unwrap();
    assert_eq!(owned.record.state, SessionState::Started);
    owned.set_delivery(json!({"retained":true})).unwrap();
    assert!(!owned.observe_native(&json!({"text":"ready"})).unwrap());
    assert!(!owned.observe_tools(&json!({"schema":1,"event":"tools_ready","instance":InstanceId([9;16]).to_string(),"pid":123})).unwrap());
    assert!(owned.observe_tools(&json!({"schema":1,"event":"tools_ready","instance":InstanceId([1;16]).to_string(),"pid":123})).unwrap());
    assert_eq!(owned.record.state, SessionState::Started);
    assert!(
        owned
            .observe_native(&json!({"type":"thread.started","thread_id":"native"}))
            .unwrap()
    );
    assert_eq!(owned.record.state, SessionState::Ready);
    assert!(owned.record.capabilities.tools);
    assert!(!owned.record.capabilities.confinement);
    assert_eq!(
        decode(owned.record()).unwrap().delivery,
        json!({"retained":true})
    );
    owned.child.wait().unwrap();
    assert!(owned.poll().unwrap());
    assert_eq!(owned.record.state, SessionState::Exited);
}

#[test]
fn interruption_never_retries_or_uses_historical_pid() {
    let dir = tempfile::tempdir().unwrap();
    let plan = prepare(spec(Client::Codex, dir.path().into()), &[], &json!({})).unwrap();
    let mut owned = launch(plan, 100, |record| {
        if record.state == SessionState::Started {
            Err(Error::report_failed())
        } else {
            Ok(())
        }
    })
    .unwrap();
    assert_eq!(owned.record.state, SessionState::Unknown);
    let recovered = recover(owned.record()).unwrap();
    assert_eq!(recovered.state, SessionState::Unknown);
    assert_eq!(
        decode(&recovered).unwrap().process.unwrap().pid,
        owned.child.id()
    );
    owned.child.wait().unwrap();
}

#[test]
fn resumed_native_identity_mismatch_requires_resolution() {
    let dir = tempfile::tempdir().unwrap();
    let mut candidate = spec(Client::ClaudeCode, dir.path().into());
    candidate.mode = Mode::Resume("expected".into());
    let mut owned = launch(
        prepare(candidate, &[], &json!({})).unwrap(),
        100,
        |_| Ok(()),
    )
    .unwrap();
    assert!(
        owned
            .observe_native(&json!({"type":"system","subtype":"init","session_id":"different"}))
            .is_err()
    );
    assert_eq!(owned.record.state, SessionState::Unknown);
    owned.child.wait().unwrap();
}

#[test]
fn all_native_formats_and_blocked_state_require_structured_evidence() {
    let dir = tempfile::tempdir().unwrap();
    for (client, event) in [
        (
            Client::Codex,
            json!({"type":"thread.started","thread_id":"native"}),
        ),
        (
            Client::ClaudeCode,
            json!({"type":"system","subtype":"init","session_id":"native"}),
        ),
        (
            Client::FactoryDroid,
            json!({"type":"system","subtype":"init","session_id":"native"}),
        ),
        (Client::Pi, json!({"type":"session","id":"native"})),
    ] {
        let mut owned = launch(
            prepare(spec(client, dir.path().into()), &[], &json!({})).unwrap(),
            100,
            |_| Ok(()),
        )
        .unwrap();
        assert!(owned.observe_native(&event).unwrap());
        owned.blocked().unwrap();
        owned.observe_tools(&json!({"schema":1,"event":"tools_ready","instance":InstanceId([1;16]).to_string(),"pid":123})).unwrap();
        assert_eq!(owned.record.state, SessionState::Blocked);
        owned.child.wait().unwrap();
    }
}

#[test]
fn daemon_claim_evidence_and_failed_receipt_update_preserve_valid_record() {
    let dir = tempfile::tempdir().unwrap();
    let mut owned = launch(
        prepare(spec(Client::Codex, dir.path().into()), &[], &json!({})).unwrap(),
        100,
        |_| Ok(()),
    )
    .unwrap();
    let original = owned.record().clone();
    assert!(
        owned
            .set_delivery(
                json!({"too_large":"x".repeat(locust_proto::limits::MAX_SESSION_DETAIL_BYTES + 1)})
            )
            .is_err()
    );
    assert_eq!(&original, owned.record());
    assert_eq!(owned.metadata.delivery, Value::Null);
    let claim = Claim {
        goal: GoalId([3; 32]),
        task: locust_proto::event::TaskId::Authored(EventId([4; 32])),
        attempt: EventId([5; 32]),
        instance: InstanceId([1; 16]),
        generation: 2,
    };
    let mut view = SessionView {
        instance: InstanceId([1; 16]),
        principal: PublicKey([2; 32]),
        record: original,
        updated_ms: 100,
        attached: true,
        claims: vec![claim],
    };
    owned.observe_session(&view).unwrap();
    assert_eq!(owned.metadata.binding.claim, Some(claim));
    view.claims.clear();
    owned.observe_session(&view).unwrap();
    assert_eq!(owned.metadata.binding.claim, None);
    view.instance = InstanceId([9; 16]);
    assert!(owned.observe_session(&view).is_err());
    owned.child.wait().unwrap();
}

#[cfg(unix)]
#[test]
fn owned_exit_records_signal_separately_from_exit_code() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("owned-client");
    std::fs::write(&script, "#!/bin/sh\nexec /bin/sleep 10\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut candidate = spec(Client::Codex, dir.path().into());
    candidate.executable = script;
    let mut owned = launch(
        prepare(candidate, &[], &json!({})).unwrap(),
        100,
        |_| Ok(()),
    )
    .unwrap();
    owned.child_mut().kill().unwrap();
    owned.child_mut().wait().unwrap();
    assert!(owned.poll().unwrap());
    let process = owned.metadata().process.as_ref().unwrap();
    assert_eq!(process.exit_code, None);
    assert_eq!(process.termination_signal, Some(9));
    assert_eq!(
        decode(owned.record())
            .unwrap()
            .process
            .unwrap()
            .termination_signal,
        Some(9)
    );
}

#[test]
fn native_continue_resume_and_fork_aliases_cannot_bypass_launch_mode() {
    let dir = tempfile::tempdir().unwrap();
    for (client, args) in [
        (
            Client::ClaudeCode,
            vec!["-c", "-r", "--continue", "--fork-session"],
        ),
        (Client::Pi, vec!["-c", "-r", "--continue"]),
        (Client::Codex, vec!["resume", "--fork"]),
    ] {
        for argument in args {
            let mut candidate = spec(client, dir.path().into());
            candidate.global_arguments.push(argument.into());
            assert!(prepare(candidate, &[], &json!({})).is_err());
        }
    }
    let mut codex = spec(Client::Codex, dir.path().into());
    codex.global_arguments = vec!["-c".into(), "model=\"participant-choice\"".into()];
    assert!(prepare(codex, &[], &json!({})).is_ok());
}

#[test]
fn manual_resume_capability_requires_exact_successful_resume_readiness() {
    let dir = tempfile::tempdir().unwrap();
    for mode in [
        Mode::New,
        Mode::Resume("native".into()),
        Mode::Resume("different".into()),
    ] {
        let mut candidate = spec(Client::Codex, dir.path().into());
        candidate.mode = mode.clone();
        let mut owned = launch(
            prepare(candidate, &[], &json!({})).unwrap(),
            100,
            |_| Ok(()),
        )
        .unwrap();
        assert!(!owned.record().capabilities.manual_resume);
        owned.observe_tools(&json!({"schema":1,"event":"tools_ready","instance":InstanceId([1;16]).to_string(),"pid":123})).unwrap();
        assert!(!owned.record().capabilities.manual_resume);
        let result = owned.observe_native(&json!({"type":"thread.started","thread_id":"native"}));
        match mode {
            Mode::Resume(ref expected) if expected == "native" => {
                result.unwrap();
                assert!(owned.record().capabilities.manual_resume);
            }
            Mode::Resume(_) => {
                assert!(result.is_err());
                assert!(!owned.record().capabilities.manual_resume);
            }
            Mode::New => {
                result.unwrap();
                assert!(!owned.record().capabilities.manual_resume);
            }
        }
        owned.child_mut().wait().unwrap();
    }
}
