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
        .args(["task", "claim", "--goal", "abc"])
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
            Request::NoteAdd {
                goal,
                about: None,
                supersedes: None,
                text: "first\nsecond\n".into()
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
        "note",
        "add",
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
    let assignment = locust_proto::id::EventId([4; 32]);
    let handle = server(home.path(), 2, move |frame| {
        assert_eq!(frame.request, Request::TaskClaim { goal, assignment });
        Ok(Response::Claimed(locust_proto::api::Claim {
            goal,
            assignment,
            task: locust_proto::id::EventId([5; 32]),
            instance: SessionSecret([7; 32]).instance(),
            generation: 1,
        }))
    });
    let fields = [
        "--owner",
        "task",
        "claim",
        "--goal",
        &goal.to_string(),
        "--assignment",
        &assignment.to_string(),
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
