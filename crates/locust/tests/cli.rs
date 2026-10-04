//! Binary CLI contracts exercised against a typed local Unix server.
use locust_proto::API_VERSION;
use locust_proto::api::{
    ApiError, Caller, ClientHello, Credential, DaemonStatus, ErrorCode, GoalSummary, Grants,
    Membership, Request, RequestFrame, Response, ResponseFrame, ServerHello, SessionSecret,
    WaitOutcome,
};
use locust_proto::codec;
use locust_proto::id::{GoalId, IdempotencyKey, PublicKey};
use locust_proto::limits::{MAX_HELLO_FRAME_BYTES, MAX_LOCAL_FRAME_BYTES};
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::process::{Command, Output, Stdio};
use std::thread;

fn scratch() -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix("lc-cli-")
        .tempdir_in("/tmp")
        .unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    dir
}
fn write_secret(path: &std::path::Path, bytes: &[u8; 32]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
fn cli(home: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_locust"));
    command
        .env_remove("LOCUST_HOME")
        .env_remove("LOCUST_CREDENTIAL")
        .env_remove("LOCUST_SESSION");
    command.arg("--home").arg(home).arg("--json");
    command
}
fn envelope(output: &Output, status: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(status),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).lines().count(), 1);
    serde_json::from_slice(&output.stdout).unwrap()
}
fn status(goals: Vec<GoalSummary>) -> Response {
    Response::Status(DaemonStatus {
        daemon_version: "stub".into(),
        endpoint: None,
        agents: vec![],
        goals,
    })
}
fn server(
    home: &std::path::Path,
    count: usize,
    mut respond: impl FnMut(RequestFrame) -> Result<Response, ApiError> + Send + 'static,
) -> thread::JoinHandle<Vec<ClientHello>> {
    let listener = UnixListener::bind(home.join("daemon.sock")).unwrap();
    thread::spawn(move || {
        let mut hellos = Vec::new();
        for _ in 0..count {
            let (mut stream, _) = listener.accept().unwrap();
            let Some(frame) = codec::read_frame(&mut stream, MAX_HELLO_FRAME_BYTES).unwrap() else {
                continue;
            };
            let hello = ClientHello::decode(&frame).unwrap();
            hellos.push(hello);
            let mut encoded = Vec::new();
            codec::encode_frame(
                &ServerHello::Welcome {
                    api_version: API_VERSION,
                    daemon_version: "stub".into(),
                    caller: Caller::Owner,
                    max_blob_bytes: 1024,
                },
                &mut encoded,
            )
            .unwrap();
            stream.write_all(&encoded).unwrap();
            while let Some(frame) = codec::read_frame(&mut stream, MAX_LOCAL_FRAME_BYTES).unwrap() {
                let frame: RequestFrame = codec::decode(&frame).unwrap();
                let id = frame.id;
                let result = respond(frame);
                encoded.clear();
                codec::encode_frame(&ResponseFrame { id, result }, &mut encoded).unwrap();
                stream.write_all(&encoded).unwrap();
            }
        }
        hellos
    })
}
#[test]
fn no_credential_never_falls_back_to_owner_and_usage_errors_are_json() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let output = cli(home.path()).arg("status").output().unwrap();
    assert_eq!(envelope(&output, 2)["error"]["code"], "invalid");
    let output = cli(home.path())
        .args(["--as", "worker", "status"])
        .output()
        .unwrap();
    assert_eq!(envelope(&output, 2)["error"]["code"], "invalid");
    let output = cli(home.path())
        .args(["attempt", "start", "--goal", "abc"])
        .output()
        .unwrap();
    assert_eq!(envelope(&output, 2)["error"]["code"], "invalid");
    let output = cli(home.path())
        .args(["call", "status", "[]"])
        .output()
        .unwrap();
    assert_eq!(envelope(&output, 2)["error"]["code"], "invalid");
}
#[test]
fn absent_daemon_is_unavailable_and_names_the_socket() {
    let home = scratch();
    let output = cli(home.path())
        .args(["--owner", "status"])
        .output()
        .unwrap();
    let body = envelope(&output, 8);
    assert_eq!(body["error"]["code"], "unavailable");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains(&home.path().join("daemon.sock").display().to_string())
    );
}
#[test]
fn explicit_session_creation_is_private_stable_and_needs_no_daemon() {
    let home = scratch();
    let path = home.path().join("sessions/worker.secret");
    let output = cli(home.path())
        .args(["session", "create"])
        .arg(&path)
        .output()
        .unwrap();
    let body = envelope(&output, 0);
    let secret = fs::read(&path).unwrap();
    assert_eq!(secret.len(), 32);
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
        0o600
    );
    let expected = SessionSecret(secret.as_slice().try_into().unwrap()).instance();
    assert_eq!(body["result"]["instance"], expected.to_string());
    let output = cli(home.path())
        .args(["session", "create"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(envelope(&output, 0), body);
    assert_eq!(fs::read(path).unwrap(), secret);
}
#[test]
fn enrollment_persists_credential_before_request_and_reuses_it_on_retry() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let worker = home.path().join("agents/worker.credential");
    let observed = worker.clone();
    let mut first = None;
    let handle = server(home.path(), 2, move |frame| {
        let Request::AgentEnroll {
            name,
            grants,
            credential,
        } = frame.request
        else {
            panic!("unexpected request")
        };
        assert_eq!(name, "worker");
        assert!(grants.manage_goals);
        let bytes = fs::read(&observed).unwrap();
        assert_eq!(
            fs::metadata(&observed).unwrap().permissions().mode() & 0o7777,
            0o600
        );
        assert_eq!(
            Credential(bytes.as_slice().try_into().unwrap()).digest(),
            credential
        );
        if let Some(first) = first {
            assert_eq!(first, credential);
        } else {
            first = Some(credential);
        }
        Ok(Response::AgentEnrolled {
            agent: PublicKey([2; 32]),
        })
    });
    for _ in 0..2 {
        let output = cli(home.path())
            .args(["--owner", "agent", "enroll", "worker", "--manage-goals"])
            .output()
            .unwrap();
        let body = envelope(&output, 0);
        assert_eq!(
            body["result"]["agent_enrolled"]["credential_path"],
            worker.display().to_string()
        );
        assert_eq!(
            body["result"]["agent_enrolled"]["agent"],
            PublicKey([2; 32]).to_string()
        );
    }
    assert_eq!(handle.join().unwrap().len(), 2);
}
#[test]
fn stdin_text_and_idempotency_are_forwarded_without_changing_text() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([3; 32]);
    let key = IdempotencyKey([4; 16]);
    let handle = server(home.path(), 1, move |frame| {
        assert_eq!(frame.idempotency, Some(key));
        assert_eq!(frame.on_behalf, Some(PublicKey([5; 32])));
        assert_eq!(
            frame.request,
            Request::ContributionPublish {
                goal,
                task: None,
                attempt: None,
                generation: None,
                summary: "first\nsecond\n".into(),
                base: None,
                patch: None,
                sources: Vec::new(),
                artifacts: vec![]
            }
        );
        Ok(Response::Recorded {
            event: locust_proto::id::EventId([6; 32]),
        })
    });
    let mut command = cli(home.path());
    command.args([
        "--owner",
        "--as",
        &PublicKey([5; 32]).to_string(),
        "--idempotency-key",
        &key.to_string(),
        "contribution",
        "publish",
        "--goal",
        &goal.to_string(),
        "-",
    ]);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"first\nsecond\n")
        .unwrap();
    envelope(&child.wait_with_output().unwrap(), 0);
    handle.join().unwrap();
}
#[test]
fn goal_prefixes_are_resolved_uniquely_and_duplicate_memberships_are_one_goal() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([0xab; 32]);
    let summary = GoalSummary {
        goal,
        title: None,
        member: PublicKey([2; 32]),
        membership: Membership::Member,
        halted: None,
    };
    let handle = server(home.path(), 1, move |frame| match frame.request {
        Request::Status => Ok(status(vec![summary.clone(), summary.clone()])),
        Request::Board { goal: actual } => {
            assert_eq!(actual, goal);
            Ok(Response::Board(vec![]))
        }
        _ => panic!("unexpected request"),
    });
    let output = cli(home.path())
        .args(["--owner", "board", "--goal", "ABABABAB"])
        .output()
        .unwrap();
    assert_eq!(envelope(&output, 0)["result"], json!({"board": []}));
    handle.join().unwrap();
}
#[test]
fn ambiguous_goal_prefix_is_invalid_and_does_not_send_the_operation() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let mut other = [0xab; 32];
    other[31] = 0xcd;
    let handle = server(home.path(), 1, move |frame| {
        assert_eq!(frame.request, Request::Status);
        Ok(status(
            [GoalId([0xab; 32]), GoalId(other)]
                .into_iter()
                .map(|goal| GoalSummary {
                    goal,
                    title: None,
                    member: PublicKey([2; 32]),
                    membership: Membership::Member,
                    halted: None,
                })
                .collect(),
        ))
    });
    let output = cli(home.path())
        .args(["--owner", "board", "--goal", "abababab"])
        .output()
        .unwrap();
    assert_eq!(envelope(&output, 6)["error"]["code"], "invalid");
    handle.join().unwrap();
}
#[test]
fn named_principal_is_resolved_via_status_before_impersonation() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let mut n = 0;
    let handle = server(home.path(), 1, move |frame| {
        n += 1;
        assert_eq!(frame.request, Request::Status);
        if n == 1 {
            assert!(frame.on_behalf.is_none());
            Ok(Response::Status(DaemonStatus {
                daemon_version: "stub".into(),
                endpoint: None,
                agents: vec![locust_proto::api::AgentView {
                    agent: PublicKey([2; 32]),
                    name: "worker".into(),
                    grants: Grants::default(),
                    revoked: false,
                }],
                goals: vec![],
            }))
        } else {
            assert_eq!(frame.on_behalf, Some(PublicKey([2; 32])));
            Ok(status(vec![]))
        }
    });
    envelope(
        &cli(home.path())
            .args(["--owner", "--as", "worker", "status"])
            .output()
            .unwrap(),
        0,
    );
    handle.join().unwrap();
}
#[test]
fn generic_call_and_wait_use_stable_error_and_timeout_statuses() {
    for (answer, exit, expected) in [
        (
            Err(ApiError::new(ErrorCode::ClaimHeld, "held")),
            7,
            "claim_held",
        ),
        (
            Err(ApiError::new(
                ErrorCode::AuthorizationRequired,
                "grant needed",
            )),
            4,
            "authorization_required",
        ),
        (Ok(Response::Waited(WaitOutcome::NoEvent)), 20, ""),
        (Ok(Response::Waited(WaitOutcome::Disconnected)), 21, ""),
    ] {
        let home = scratch();
        write_secret(&home.path().join("owner.credential"), &[1; 32]);
        let handle = server(home.path(), 1, move |frame| {
            assert!(matches!(frame.request, Request::Wait { .. }));
            answer.clone()
        });
        let fields = json!({"goal": GoalId([3; 32]), "seen": 1, "timeout_ms": 0}).to_string();
        let output = cli(home.path())
            .args(["--owner", "call", "wait", &fields])
            .output()
            .unwrap();
        let body = envelope(&output, exit);
        if !expected.is_empty() {
            assert_eq!(body["error"]["code"], expected);
        }
        handle.join().unwrap();
    }
}
#[test]
fn unsupported_hello_names_both_api_versions_and_uses_exit_ten() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let listener = UnixListener::bind(home.path().join("daemon.sock")).unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        codec::read_frame(&mut stream, MAX_HELLO_FRAME_BYTES)
            .unwrap()
            .unwrap();
        let mut bytes = Vec::new();
        codec::encode_frame(
            &ServerHello::Refused {
                error: ApiError::new(ErrorCode::UnsupportedVersion, "versions differ"),
                api_version: API_VERSION + 1,
                daemon_version: "stub".into(),
            },
            &mut bytes,
        )
        .unwrap();
        stream.write_all(&bytes).unwrap();
    });
    let output = cli(home.path())
        .args(["--owner", "status"])
        .output()
        .unwrap();
    let body = envelope(&output, 10);
    assert_eq!(body["error"]["code"], "unsupported_version");
    let text = body["error"]["message"].as_str().unwrap();
    assert!(text.contains(&format!("API {}", API_VERSION + 1)));
    assert!(text.contains(&format!("client API {API_VERSION}")));
    handle.join().unwrap();
}
#[test]
fn doctor_reports_ordered_failures_and_exit_one_without_creating_state() {
    let home = scratch();
    let absent = home.path().join("absent");
    let output = cli(&absent).args(["--owner", "doctor"]).output().unwrap();
    let body = envelope(&output, 1);
    assert_eq!(body["ok"], false);
    let names: Vec<_> = body["result"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|check| check["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "state_directory",
            "socket_path",
            "daemon_lock",
            "socket_connection",
            "hello",
            "status",
            "credential_file"
        ]
    );
    assert!(!absent.exists());
}
#[test]
fn malformed_and_insecure_credentials_are_refused_without_hello() {
    for (bytes, mode) in [(vec![1; 31], 0o600), (vec![1; 32], 0o644)] {
        let home = scratch();
        let path = home.path().join("owner.credential");
        fs::write(&path, bytes.clone()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        let handle = server(home.path(), 1, |_| panic!("must not receive a request"));
        let output = cli(home.path())
            .args(["--owner", "status"])
            .output()
            .unwrap();
        assert_eq!(envelope(&output, 6)["error"]["code"], "invalid");
        assert!(handle.join().unwrap().is_empty());
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}

#[test]
fn claims_require_an_explicit_session_and_present_exact_file_bytes() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let session = home.path().join("worker.secret");
    write_secret(&session, &[7; 32]);
    let goal = GoalId([3; 32]);
    let attempt = locust_proto::id::EventId([4; 32]);
    let task = locust_proto::event::TaskId::Authored(locust_proto::id::EventId([5; 32]));
    let handle = server(home.path(), 2, move |frame| {
        assert_eq!(
            frame.request,
            Request::AttemptStart {
                goal,
                task,
                offer: None
            }
        );
        Ok(Response::Claimed(locust_proto::api::Claim {
            goal,
            attempt,
            task,
            instance: SessionSecret([7; 32]).instance(),
            generation: 1,
        }))
    });
    let fields = [
        "--owner",
        "attempt",
        "start",
        "--goal",
        &goal.to_string(),
        "--task",
        &task.to_string(),
    ];
    assert_eq!(
        envelope(&cli(home.path()).args(fields).output().unwrap(), 6)["error"]["code"],
        "invalid"
    );
    let output = cli(home.path())
        .arg("--session")
        .arg(&session)
        .args(fields)
        .output()
        .unwrap();
    assert_eq!(envelope(&output, 0)["result"]["claimed"]["generation"], 1);
    let hellos = handle.join().unwrap();
    assert_eq!(hellos.len(), 2);
    assert!(hellos[0].session.is_none());
    assert_eq!(hellos[1].session, Some(SessionSecret([7; 32])));
}
#[test]
fn doctor_succeeds_for_a_locked_private_home_and_valid_session() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let session = home.path().join("worker.secret");
    write_secret(&session, &[7; 32]);
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(home.path().join("daemon.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    writeln!(&lock, "{}", std::process::id()).unwrap();
    let handle = server(home.path(), 1, |frame| {
        assert_eq!(frame.request, Request::Status);
        Ok(status(vec![]))
    });
    let output = cli(home.path())
        .args(["--owner", "--session"])
        .arg(&session)
        .arg("doctor")
        .output()
        .unwrap();
    let body = envelope(&output, 0);
    assert_eq!(body["ok"], true);
    assert_eq!(body["result"]["checks"].as_array().unwrap().len(), 8);
    assert!(
        body["result"]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|check| check["ok"] == true)
    );
    handle.join().unwrap();
}

#[test]
fn version_is_registered_and_json_works_on_either_side() {
    for args in [["--json", "--version"], ["--version", "--json"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_locust"))
            .args(args)
            .output()
            .unwrap();
        let body = envelope(&output, 0);
        let version = body["result"]["version"].as_str().unwrap();
        assert!(version.starts_with("locust "));
        assert!(version.contains(&format!(
            "api {API_VERSION} protocol {}",
            locust_proto::PROTOCOL_VERSION
        )));
    }
    let output = Command::new(env!("CARGO_BIN_EXE_locust"))
        .args(["--json", "--help"])
        .output()
        .unwrap();
    let body = envelope(&output, 0);
    let help = body["result"]["help"].as_str().unwrap();
    assert!(help.contains("--version"));
    assert!(help.contains("Explicit credential file"));
    assert!(help.contains("Local participant daemon and client"));
}

#[test]
fn explicit_path_errors_name_the_option_and_missing_auth_lists_choices() {
    let home = scratch();
    for (args, expected) in [
        (
            vec!["--home", "relative", "status"],
            "--home must be an absolute path",
        ),
        (
            vec!["--credential", "relative", "status"],
            "--credential must be an absolute path",
        ),
        (
            vec!["--owner", "--session", "relative", "status"],
            "--session must be an absolute path",
        ),
        (
            vec!["session", "create", "relative"],
            "session create PATH must be an absolute path",
        ),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_locust"));
        command
            .env_remove("LOCUST_CREDENTIAL")
            .env_remove("LOCUST_SESSION")
            .env("LOCUST_HOME", home.path())
            .arg("--json")
            .args(args);
        let output = command.output().unwrap();
        assert_eq!(envelope(&output, 2)["error"]["message"], expected);
    }
    let output = cli(home.path()).arg("status").output().unwrap();
    let body = envelope(&output, 2);
    let message = body["error"]["message"].as_str().unwrap();
    for choice in ["--credential", "LOCUST_CREDENTIAL", "--owner"] {
        assert!(message.contains(choice));
    }
}

#[test]
fn named_grant_explicitly_sets_or_revokes_goal_management() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let agent = PublicKey([2; 32]);
    let mut expected = [true, false].into_iter();
    let handle = server(home.path(), 2, move |frame| {
        assert_eq!(
            frame.request,
            Request::AgentGrant {
                agent,
                grants: Grants {
                    manage_goals: expected.next().unwrap()
                }
            }
        );
        Ok(Response::Done)
    });
    for value in ["true", "false"] {
        let grants = format!(r#"{{"manage_goals":{value}}}"#);
        envelope(
            &cli(home.path())
                .args([
                    "--owner",
                    "agent",
                    "grant",
                    "--agent",
                    &agent.to_string(),
                    "--grants",
                    &grants,
                ])
                .output()
                .unwrap(),
            0,
        );
    }
    handle.join().unwrap();
}

#[test]
fn invitation_can_be_read_from_stdin_with_only_line_endings_removed() {
    use locust_proto::id::EndpointId;
    use locust_proto::invite::{Invitation, InviteSecret};
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let invitation = Invitation::signed(
        GoalId([3; 32]),
        Some("Stdin invitation".into()),
        EndpointId([5; 32]),
        vec![],
        InviteSecret([6; 32]),
        None,
        &locust_proto::crypto::Keypair::from_seed([4; 32]),
    )
    .unwrap();
    let ticket = invitation.to_ticket().unwrap();
    let expected = ticket.clone();
    let handle = server(home.path(), 1, move |frame| {
        assert_eq!(
            frame.request,
            Request::GoalJoin {
                ticket: expected.clone()
            }
        );
        Ok(Response::Joined {
            goal: invitation.goal,
            administrator: invitation.administrator,
            membership: Membership::Joining,
        })
    });
    let mut child = cli(home.path())
        .args(["--owner", "goal", "join", "--ticket", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    write!(child.stdin.take().unwrap(), "{}\r\n", ticket.as_str()).unwrap();
    assert_eq!(
        envelope(&child.wait_with_output().unwrap(), 0)["result"]["joined"]["membership"],
        "joining"
    );
    handle.join().unwrap();
}

#[test]
fn human_status_names_membership_and_halt_with_stable_tags() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let handle = server(home.path(), 1, |_| {
        Ok(status(vec![
            GoalSummary {
                goal: GoalId([3; 32]),
                member: PublicKey([4; 32]),
                title: Some("a goal".into()),
                membership: Membership::Refused,
                halted: Some(locust_proto::api::Halt::AuthorityConflict),
            },
            GoalSummary {
                goal: GoalId([5; 32]),
                member: PublicKey([4; 32]),
                title: None,
                membership: Membership::Joining,
                halted: None,
            },
        ]))
    });
    let output = Command::new(env!("CARGO_BIN_EXE_locust"))
        .env_remove("LOCUST_CREDENTIAL")
        .env_remove("LOCUST_SESSION")
        .arg("--home")
        .arg(home.path())
        .args(["--owner", "status"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains(&format!("Goal a goal ({})", GoalId([3; 32]))));
    assert!(text.contains(&format!("{} · refused", PublicKey([4; 32]))));
    assert!(text.contains("Blocked: conflicting authority history"));
    assert!(text.contains("decisions cannot advance"));
    assert!(text.contains("fresh invitation, inspect it, and join again"));
    assert!(text.contains(&format!("Title unavailable ({})", GoalId([5; 32]))));
    assert!(text.contains("Admission has not arrived. Check connectivity to the issuer"));
    handle.join().unwrap();
}

#[test]
fn corrupt_database_startup_preserves_corrupted_code_and_cleans_socket() {
    let home = scratch();
    fs::write(home.path().join("locust.db"), b"not a SQLite database").unwrap();
    let output = cli(home.path()).args(["daemon", "run"]).output().unwrap();
    assert_eq!(envelope(&output, 11)["error"]["code"], "corrupted");
    assert!(!home.path().join("daemon.sock").exists());
}

#[test]
fn held_daemon_lock_is_unavailable() {
    let home = scratch();
    let lock = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(home.path().join("daemon.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    let output = cli(home.path()).args(["daemon", "run"]).output().unwrap();
    assert_eq!(envelope(&output, 8)["error"]["code"], "unavailable");
}

#[test]
fn closed_stderr_does_not_abort_daemon_startup_or_shutdown() {
    let home = scratch();
    let mut child = cli(home.path())
        .args(["daemon", "run"])
        .env("LOCUST_RELAY", "none")
        .env("LOCUST_LOOKUP", "none")
        .env("LOCUST_BIND", "127.0.0.1:0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Close the reader before the daemon's first listening log line.
    drop(child.stderr.take());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let ready = loop {
        if child.try_wait().unwrap().is_some() {
            break false;
        }
        if home.path().join("daemon.sock").exists() {
            break true;
        }
        if std::time::Instant::now() >= deadline {
            break false;
        }
        thread::sleep(std::time::Duration::from_millis(10));
    };
    if !ready {
        let _ = child.kill();
        let output = child.wait_with_output().unwrap();
        panic!("daemon did not start with closed stderr: {:?}", output);
    }
    let stop = cli(home.path())
        .args(["--owner", "daemon", "stop"])
        .output()
        .unwrap();
    if !stop.status.success() {
        let _ = child.kill();
    }
    envelope(&stop, 0);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() && std::time::Instant::now() < deadline {
        thread::sleep(std::time::Duration::from_millis(10));
    }
    if child.try_wait().unwrap().is_none() {
        let _ = child.kill();
    }
    let output = child.wait_with_output().unwrap();
    envelope(&output, 0);
    assert!(!home.path().join("daemon.sock").exists());
}

#[test]
fn human_goal_and_task_titles_resolve_to_exact_authorized_write() {
    use locust_proto::api::TaskView;
    use locust_proto::event::{Context, Scope, TaskId};
    use locust_proto::id::EventId;
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([0x31; 32]);
    let task = TaskId::Authored(EventId([0x41; 32]));
    let agent = PublicKey([0x51; 32]);
    let handle = server(home.path(), 1, move |frame| match frame.request {
        Request::Status => Ok(status(vec![GoalSummary {
            goal,
            title: Some("Demo".into()),
            member: agent,
            membership: Membership::Member,
            halted: None,
        }])),
        Request::Board { goal: g } => {
            assert_eq!(g, goal);
            Ok(Response::Board(vec![TaskView {
                task,
                context: Context {
                    scope: Scope::Task(task),
                    round: EventId([0x41; 32]),
                },
                creator: agent,
                title: Some("Fix greeting".into()),
                attempts: vec![],
                contributions: vec![],
                completed: false,
                selected: None,
                closed: false,
            }]))
        }
        Request::WorkOffer {
            goal: g,
            task: t,
            recipient,
        } => {
            assert_eq!((g, t, recipient), (goal, task, agent));
            Ok(Response::Recorded {
                event: EventId([0x61; 32]),
            })
        }
        other => panic!("unexpected request {other:?}"),
    });
    let output = cli(home.path())
        .args([
            "--owner",
            "work",
            "offer",
            "--goal",
            "Demo",
            "--task",
            "Fix greeting",
            "--recipient",
            &agent.to_string(),
        ])
        .output()
        .unwrap();
    envelope(&output, 0);
    handle.join().unwrap();
}

#[test]
fn duplicate_goal_titles_refuse_writes_instead_of_guessing() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let handle = server(home.path(), 1, move |frame| {
        assert!(matches!(frame.request, Request::Status));
        Ok(status(
            [1, 2]
                .into_iter()
                .map(|n| GoalSummary {
                    goal: GoalId([n; 32]),
                    title: Some("Demo".into()),
                    member: PublicKey([3; 32]),
                    membership: Membership::Member,
                    halted: None,
                })
                .collect(),
        ))
    });
    let output = cli(home.path())
        .args(["--owner", "goal", "leave", "--goal", "Demo"])
        .output()
        .unwrap();
    assert!(
        envelope(&output, 6)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("ambiguous")
    );
    handle.join().unwrap();
}

#[test]
fn review_subject_prefix_checks_later_feed_pages_before_writing() {
    use locust_proto::api::{EventView, Standing};
    use locust_proto::id::EventId;
    for ambiguous in [false, true] {
        let home = scratch();
        write_secret(&home.path().join("owner.credential"), &[1; 32]);
        let goal = GoalId([1; 32]);
        let first = EventId([0xab; 32]);
        let mut other = [0xab; 32];
        other[31] = 2;
        let handle = server(home.path(), 1, move |frame| match frame.request {
            Request::Events { goal: g, after, .. } => {
                assert_eq!(g, goal);
                let candidate = match after {
                    None => Some((1, first)),
                    Some(1) => Some((
                        2,
                        if ambiguous {
                            EventId(other)
                        } else {
                            EventId([2; 32])
                        },
                    )),
                    Some(2) => None,
                    other => panic!("bad cursor {other:?}"),
                };
                Ok(Response::Events(
                    candidate
                        .into_iter()
                        .map(|(position, event)| EventView {
                            position: Some(position),
                            event,
                            author: PublicKey([3; 32]),
                            kind: "contribution_published".into(),
                            at_ms: 0,
                            standing: Standing::Effective,
                        })
                        .collect(),
                ))
            }
            Request::ReviewRecord {
                goal: g, subject, ..
            } => {
                assert!(!ambiguous, "ambiguous selector wrote a review");
                assert_eq!((g, subject), (goal, first));
                Ok(Response::Recorded {
                    event: EventId([4; 32]),
                })
            }
            other => panic!("unexpected {other:?}"),
        });
        let output = cli(home.path())
            .args([
                "--owner",
                "review",
                "record",
                "--goal",
                &goal.to_string(),
                "--subject",
                "ABABABAB",
                "--verdict",
                "approve",
                "Reviewed exact changes",
            ])
            .output()
            .unwrap();
        let result = envelope(&output, if ambiguous { 6 } else { 0 });
        if ambiguous {
            assert!(
                result["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("ambiguous")
            );
        }
        handle.join().unwrap();
    }
}

#[test]
fn generic_calls_keep_full_typed_identity_contract_and_reject_human_selectors_offline() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    for (operation, fields) in [
        ("goal.leave", json!({"goal":"Demo"})),
        (
            "task.show",
            json!({"goal":GoalId([1;32]),"task":"task:abababab"}),
        ),
        (
            "event.show",
            json!({"goal":GoalId([1;32]),"event":"abababab"}),
        ),
    ] {
        let output = cli(home.path())
            .args(["--owner", "call", operation, &fields.to_string()])
            .output()
            .unwrap();
        let result = envelope(&output, 2);
        assert_eq!(result["error"]["code"], "invalid");
        assert!(
            !result["error"]["message"]
                .as_str()
                .unwrap()
                .contains("socket")
        );
    }
}
