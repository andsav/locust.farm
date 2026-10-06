//! Binary CLI contracts exercised against a typed local Unix server.
use locust_proto::API_VERSION;
use locust_proto::api::{
    Abilities, AgentView, ApiError, Caller, ClientHello, Credential, DaemonStatus, ErrorCode,
    GoalSummary, Level, Membership, Request, RequestFrame, Response, ResponseFrame, ServerHello,
    SessionSecret, WaitOutcome,
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
fn plain() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_locust"));
    command
        .env_remove("LOCUST_HOME")
        .env_remove("LOCUST_CREDENTIAL")
        .env_remove("LOCUST_SESSION");
    command
}
fn cli(home: &std::path::Path) -> Command {
    let mut command = plain();
    command.arg("--home").arg(home).arg("--json");
    command
}
/// Run `command` after the reader of its standard output, or of its standard
/// error, has gone, so its first write there meets a broken pipe. Returns the
/// exit status and everything the other stream received.
fn closed_reader(command: &mut Command, error_stream: bool) -> (Option<i32>, String) {
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    if error_stream {
        command.stdout(Stdio::piped()).stderr(writer);
    } else {
        command.stdout(writer).stderr(Stdio::piped());
    }
    let output = command.stdin(Stdio::null()).output().unwrap();
    let other = if error_stream {
        output.stdout
    } else {
        output.stderr
    };
    (output.status.code(), String::from_utf8(other).unwrap())
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
fn abilities(goal: GoalId, agent: PublicKey) -> Abilities {
    Abilities {
        goal,
        agent,
        name: agent.to_string(),
        membership: Some(Membership::Member),
        level: Level::Auto,
        host: Some(agent),
        hosted_here: true,
        roles: vec![],
        rules: vec![],
        allowed_tasks: vec![],
        wanted_tasks: vec![],
        claims: vec![],
    }
}
fn status_agents(agents: Vec<AgentView>) -> Response {
    Response::Status(DaemonStatus {
        daemon_version: "stub".into(),
        endpoint: None,
        agents,
        goals: vec![],
    })
}

fn task_fixture(
    agent: PublicKey,
    task: locust_proto::event::TaskId,
) -> locust_proto::api::TaskDetail {
    use locust_proto::event::{Context, Scope};
    use locust_proto::id::EventId;
    let round = match task {
        locust_proto::event::TaskId::Authored(id) => id,
        _ => EventId([0; 32]),
    };
    locust_proto::api::TaskDetail {
        view: locust_proto::api::TaskView {
            task,
            context: Context {
                scope: Scope::Task(task),
                round,
            },
            creator: agent,
            by_host: false,
            title: Some("Task title".into()),
            attempts: vec![],
            contributions: vec![],
            completed: false,
            selected: None,
            closed: false,
        },
        text: None,
        inputs: Default::default(),
        parent: None,
        task_type: None,
        effective_rules_json: "{}".into(),
    }
}

#[test]
fn level_and_allow_apply_in_one_run_and_print_an_undo_that_names_the_agent() {
    use locust_proto::api::{GoalStatus, MemberView};
    use locust_proto::event::TaskId;
    use locust_proto::id::{EndpointId, EventId};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([0xab; 32]);
    let agent = PublicKey([2; 32]);
    let task = TaskId::Authored(EventId([0xcd; 32]));
    let detail = task_fixture(agent, task);
    let closed = Arc::new(AtomicBool::new(false));
    let closed_on_server = Arc::clone(&closed);
    let mut level = Level::Read;
    let mut allowed = false;
    let handle = server(home.path(), 11, move |frame| {
        // One typed connection is opened per command; the closure retains the local record.
        let current = |level, allowed| {
            let mut a = abilities(goal, agent);
            a.name = "worker".into();
            a.level = level;
            if allowed && !closed_on_server.load(Ordering::SeqCst) {
                a.allowed_tasks.push(task);
            }
            a
        };
        match frame.request {
            Request::Status => Ok(Response::Status(DaemonStatus {
                daemon_version: "stub".into(),
                endpoint: None,
                agents: vec![AgentView {
                    agent,
                    name: "worker".into(),
                    author_only: false,
                    revoked: false,
                }],
                goals: vec![GoalSummary {
                    goal,
                    title: Some("Goal title".into()),
                    member: agent,
                    membership: Membership::Member,
                    halted: None,
                    abilities: current(level, allowed),
                }],
            })),
            Request::GoalStatus { goal: selected } => {
                assert_eq!(selected, goal);
                Ok(Response::GoalStatus(GoalStatus {
                    goal,
                    title: Some("Goal title".into()),
                    governance: PublicKey([9; 32]),
                    hosted_here: true,
                    host: Some(agent),
                    governance_head: None,
                    current_rules: None,
                    scope_halts: vec![],
                    members: vec![MemberView {
                        member: agent,
                        endpoint: EndpointId([3; 32]),
                        local: true,
                    }],
                    halted: None,
                    workspace: None,
                    abilities: vec![current(level, allowed)],
                    stalled: vec![],
                    peers: vec![],
                }))
            }
            Request::Board { goal: selected } => {
                assert_eq!(selected, goal);
                Ok(Response::Board(vec![detail.view.clone()]))
            }
            Request::Task {
                goal: selected,
                task: selected_task,
            } => {
                assert_eq!((selected, selected_task), (goal, task));
                let mut detail = detail.clone();
                detail.view.closed = closed_on_server.load(Ordering::SeqCst);
                Ok(Response::Task(detail))
            }
            Request::LevelSet {
                goal: selected,
                agent: selected_agent,
                level: new_level,
            } => {
                assert_eq!((selected, selected_agent), (goal, agent));
                level = new_level;
                Ok(Response::Abilities(current(level, allowed)))
            }
            Request::TaskAllow {
                goal: selected,
                agent: selected_agent,
                task: selected_task,
            } => {
                assert_eq!(
                    (selected, selected_agent, selected_task),
                    (goal, agent, task)
                );
                allowed = true;
                Ok(Response::Abilities(current(level, allowed)))
            }
            Request::TaskDisallow {
                goal: selected,
                agent: selected_agent,
                task: selected_task,
            } => {
                assert_eq!(
                    (selected, selected_agent, selected_task),
                    (goal, agent, task)
                );
                let changed = allowed;
                allowed = false;
                Ok(Response::TaskDisallowed {
                    abilities: current(level, allowed),
                    changed,
                    was_allowed: changed,
                })
            }
            other => panic!("unexpected {other:?}"),
        }
    });
    let run = |parts: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_locust"))
            .arg("--home")
            .arg(home.path())
            .args(parts)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let run_undo = |printed: &str| {
        let line = printed
            .lines()
            .find_map(|line| line.strip_prefix("Undo: "))
            .expect("undo line");
        assert!(line.contains("--agent worker"));
        assert!(line.contains("--goal abababab"));
        let binary_dir = std::path::Path::new(env!("CARGO_BIN_EXE_locust"))
            .parent()
            .unwrap();
        let path = format!(
            "{}:{}",
            binary_dir.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let output = Command::new("sh")
            .arg("-c")
            .arg(line)
            .env("PATH", path)
            .env("LOCUST_HOME", home.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    let changed = run(&["--owner", "level", "--goal", &goal.to_string(), "auto"]);
    assert!(changed.contains("worker in \"Goal title\": auto."));
    run_undo(&changed);
    let unchanged = run(&["--owner", "level", "--goal", &goal.to_string(), "read"]);
    assert!(!unchanged.contains("Undo:"));
    let changed = run(&[
        "--owner",
        "allow",
        "--goal",
        &goal.to_string(),
        "--task",
        &task.to_string(),
    ]);
    assert!(
        changed.contains(
            "worker may take \"Task title\" in \"Goal title\" until the host revises it."
        )
    );
    assert!(changed.contains("--task task:cdcdcdcd --revoke"));
    run_undo(&changed);
    let unchanged = run(&[
        "--owner",
        "allow",
        "--revoke",
        "--goal",
        &goal.to_string(),
        "--task",
        &task.to_string(),
    ]);
    assert!(!unchanged.contains("Undo:"));
    run(&[
        "--owner",
        "allow",
        "--goal",
        &goal.to_string(),
        "--task",
        &task.to_string(),
    ]);
    closed.store(true, Ordering::SeqCst);
    let revoked = cli(home.path())
        .args([
            "--owner",
            "allow",
            "--revoke",
            "--goal",
            &goal.to_string(),
            "--task",
            &task.to_string(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        envelope(&revoked, 0)["result"]["changed"],
        true,
        "a hidden stored allowance must still be revoked"
    );
    closed.store(false, Ordering::SeqCst);
    let unchanged = run(&[
        "--owner",
        "allow",
        "--revoke",
        "--goal",
        &goal.to_string(),
        "--task",
        &task.to_string(),
    ]);
    assert!(!unchanged.contains("Undo:"));
    run(&[
        "--owner",
        "allow",
        "--goal",
        &goal.to_string(),
        "--task",
        &task.to_string(),
    ]);
    closed.store(true, Ordering::SeqCst);
    let hidden_revocation = run(&[
        "--owner",
        "allow",
        "--revoke",
        "--goal",
        &goal.to_string(),
        "--task",
        &task.to_string(),
    ]);
    assert!(
        !hidden_revocation.contains("Undo:"),
        "restoring allow while closed would be refused"
    );
    handle.join().unwrap();
}
fn server(
    home: &std::path::Path,
    count: usize,
    respond: impl FnMut(RequestFrame) -> Result<Response, ApiError> + Send + 'static,
) -> thread::JoinHandle<Vec<ClientHello>> {
    server_as(home, count, Caller::Owner, respond)
}
fn server_as(
    home: &std::path::Path,
    count: usize,
    caller: Caller,
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
                    caller,
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
fn package_keygen_persists_bare_relative_paths_without_overwriting() {
    let directory = scratch();
    let run = || {
        plain()
            .current_dir(directory.path())
            .args([
                "--json",
                "package",
                "keygen",
                "--secret-key",
                "signing.key",
                "--public-key",
                "trust.pub",
            ])
            .output()
            .unwrap()
    };
    assert_eq!(envelope(&run(), 0)["result"]["created"], true);
    let secret = fs::read(directory.path().join("signing.key")).unwrap();
    let public = fs::read(directory.path().join("trust.pub")).unwrap();
    let seed: [u8; 32] = secret.as_slice().try_into().unwrap();
    let key = ed25519_dalek::SigningKey::from_bytes(&seed);
    assert!(public == key.verifying_key().to_bytes());
    for (name, mode) in [("signing.key", 0o600), ("trust.pub", 0o644)] {
        assert_eq!(
            fs::metadata(directory.path().join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            mode
        );
    }
    assert_eq!(envelope(&run(), 7)["error"]["code"], "conflict");
    assert!(fs::read(directory.path().join("signing.key")).unwrap() == secret);
    assert!(fs::read(directory.path().join("trust.pub")).unwrap() == public);
}

#[test]
fn package_withdrawal_signing_persists_a_bare_relative_signature() {
    let directory = scratch();
    let key = ed25519_dalek::SigningKey::from_bytes(&[23; 32]);
    write_secret(&directory.path().join("signing.key"), &key.to_bytes());
    let registry =
        br#"{"format":"locust-withdrawals-v1","sequence":1,"withdrawn_manifest_sha256":[]}"#;
    fs::write(directory.path().join("withdrawals.json"), registry).unwrap();
    let run = || {
        plain()
            .current_dir(directory.path())
            .args([
                "--json",
                "package",
                "sign-withdrawals",
                "--registry",
                "withdrawals.json",
                "--secret-key",
                "signing.key",
            ])
            .output()
            .unwrap()
    };
    assert_eq!(envelope(&run(), 0)["result"]["signed"], true);
    let path = directory.path().join("withdrawals.json.sig");
    let bytes = fs::read(&path).unwrap();
    let signature = ed25519_dalek::Signature::from_slice(&bytes).unwrap();
    key.verifying_key()
        .verify_strict(registry, &signature)
        .unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
        0o644
    );
    assert_eq!(envelope(&run(), 6)["error"]["code"], "invalid");
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn no_credential_never_falls_back_to_owner_and_usage_errors_are_json() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let output = cli(home.path()).arg("status").output().unwrap();
    assert_eq!(envelope(&output, 2)["error"]["code"], "invalid");
    let output = cli(home.path())
        .args(["--agent", "worker", "status"])
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
        let Request::AgentEnroll { name, credential } = frame.request else {
            panic!("unexpected request")
        };
        assert_eq!(name, "worker");
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
            .args(["--owner", "agent", "enroll", "worker"])
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
fn owner_agent_selector_cannot_create_an_enrollment_credential() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let handle = server(home.path(), 1, |_| {
        panic!("invalid enrollment sent a request")
    });
    let output = cli(home.path())
        .args([
            "--owner",
            "--agent",
            &PublicKey([2; 32]).to_string(),
            "agent",
            "enroll",
            "worker",
        ])
        .output()
        .unwrap();
    assert!(
        envelope(&output, 2)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("drop --agent")
    );
    assert!(!home.path().join("agents/worker.credential").exists());
    assert_eq!(handle.join().unwrap().len(), 1);
}

#[test]
fn owner_session_drop_sends_no_implicit_agent() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let instance = SessionSecret([7; 32]).instance();
    let handle = server(home.path(), 1, move |frame| {
        assert_eq!(frame.on_behalf, None);
        assert_eq!(frame.request, Request::SessionDrop { instance });
        Ok(Response::Done)
    });
    let output = cli(home.path())
        .args([
            "--owner",
            "session",
            "drop",
            "--instance",
            &instance.to_string(),
        ])
        .output()
        .unwrap();
    assert_eq!(envelope(&output, 0)["result"], json!(Response::Done));
    handle.join().unwrap();
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
                attempt: None,
                generation: None,
                summary: "first\nsecond\n".into(),
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
        "--agent",
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
        abilities: abilities(goal, PublicKey([2; 32])),
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
                    abilities: abilities(goal, PublicKey([2; 32])),
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
                    author_only: false,
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
            .args(["--owner", "--agent", "worker", "status"])
            .output()
            .unwrap(),
        0,
    );
    handle.join().unwrap();
}

#[test]
fn goal_create_uses_owner_authority_and_names_the_selected_host_agent() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let agent = PublicKey([2; 32]);
    let handle = server(home.path(), 3, move |frame| {
        if matches!(frame.request, Request::Status) {
            return Ok(status(vec![]));
        }
        assert_eq!(frame.on_behalf, None);
        let Request::GoalCreate {
            agent: selected,
            title,
            ..
        } = frame.request
        else {
            panic!("expected goal.create");
        };
        assert_eq!(selected, agent);
        assert_eq!(title, "Owner's goal");
        Ok(Response::GoalCreated {
            goal: GoalId([3; 32]),
        })
    });
    let plan = cli(home.path())
        .args([
            "--owner",
            "--agent",
            &agent.to_string(),
            "goal",
            "create",
            "--title",
            "Owner's goal",
            "--plan",
        ])
        .output()
        .unwrap();
    let plan = envelope(&plan, 0);
    assert_eq!(plan["result"]["action"], "review_required");
    let plan_id = plan["result"]["plan_id"].as_str().unwrap();
    let result = cli(home.path())
        .args([
            "--owner",
            "--agent",
            &agent.to_string(),
            "goal",
            "create",
            "--title",
            "Owner's goal",
            "--confirm",
            plan_id,
        ])
        .output()
        .unwrap();
    assert_eq!(
        envelope(&result, 0)["result"]["goal_created"]["goal"],
        GoalId([3; 32]).to_string()
    );
    let missing_agent = cli(home.path())
        .args(["--owner", "goal", "create", "--title", "No agent"])
        .output()
        .unwrap();
    assert!(
        envelope(&missing_agent, 2)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("connect an agent first")
    );
    handle.join().unwrap();
}

#[test]
fn owner_infers_only_one_active_non_author_agent_for_a_write() {
    for (agents, expected) in [
        (
            vec![AgentView {
                agent: PublicKey([2; 32]),
                name: "worker".into(),
                author_only: false,
                revoked: false,
            }],
            Some(PublicKey([2; 32])),
        ),
        (
            vec![AgentView {
                agent: PublicKey([2; 32]),
                name: "writer".into(),
                author_only: true,
                revoked: false,
            }],
            None,
        ),
        (
            vec![
                AgentView {
                    agent: PublicKey([2; 32]),
                    name: "one".into(),
                    author_only: false,
                    revoked: false,
                },
                AgentView {
                    agent: PublicKey([3; 32]),
                    name: "two".into(),
                    author_only: false,
                    revoked: false,
                },
            ],
            None,
        ),
    ] {
        let home = scratch();
        write_secret(&home.path().join("owner.credential"), &[1; 32]);
        let handle = server(home.path(), 1, move |frame| {
            assert_eq!(frame.request, Request::Status);
            Ok(status_agents(agents.clone()))
        });
        let output = cli(home.path())
            .args(["--owner", "goal", "create", "--title", "Demo", "--plan"])
            .output()
            .unwrap();
        match expected {
            Some(agent) => assert_eq!(
                envelope(&output, 0)["result"]["plan"]["agent"],
                agent.to_string()
            ),
            None => assert_eq!(envelope(&output, 2)["error"]["code"], "invalid"),
        }
        handle.join().unwrap();
    }
}

#[test]
fn goal_invite_defaults_to_seven_days_and_never_impersonates_the_host_agent() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let start = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let handle = server(home.path(), 2, move |frame| {
        if let Request::GoalStatus { goal: selected } = frame.request {
            assert_eq!(selected, goal);
            return Ok(Response::GoalStatus(
                serde_json::from_value(json!({
                    "goal":goal,"title":"Demo","governance":PublicKey([9;32]),"hosted_here":true,"host":PublicKey([2;32]),
                    "governance_head":null,"current_rules":null,"scope_halts":[],
                    "members":[],"halted":null,"abilities":[],"stalled":[],"peers":[]
                }))
                .unwrap(),
            ));
        }
        if matches!(frame.request, Request::GoalInvitations { .. }) {
            return Ok(Response::Invitations {
                invitations: vec![],
            });
        }
        assert_eq!(frame.on_behalf, None);
        let Request::GoalInvite {
            goal: selected,
            expires_ms,
        } = frame.request
        else {
            panic!("expected goal.invite");
        };
        assert_eq!(selected, goal);
        assert!(expires_ms >= start + 7 * 24 * 60 * 60 * 1000);
        assert!(expires_ms <= start + 7 * 24 * 60 * 60 * 1000 + 30_000);
        Ok(Response::Invited {
            ticket: locust_proto::invite::Ticket("test-ticket".into()),
        })
    });
    let plan = cli(home.path())
        .args([
            "--owner",
            "goal",
            "invite",
            "--goal",
            &goal.to_string(),
            "--plan",
        ])
        .output()
        .unwrap();
    let plan = envelope(&plan, 0);
    assert_eq!(plan["result"]["action"], "review_required");
    let result = cli(home.path())
        .args([
            "--owner",
            "goal",
            "invite",
            "--goal",
            &goal.to_string(),
            "--confirm",
            plan["result"]["plan_id"].as_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        envelope(&result, 0)["result"]["invited"]["ticket"],
        "test-ticket"
    );
    handle.join().unwrap();
    let with_as = cli(home.path())
        .args([
            "--owner",
            "--agent",
            &PublicKey([2; 32]).to_string(),
            "goal",
            "invite",
            "--goal",
            &goal.to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        envelope(&with_as, 2)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("drop --agent")
    );
}

#[test]
fn changed_goal_title_refuses_an_invitation_confirm_without_issuing_a_ticket() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let mut reads = 0;
    let handle = server(home.path(), 2, move |frame| match frame.request {
        Request::GoalStatus { goal: selected } => {
            assert_eq!(selected, goal);
            reads += 1;
            Ok(Response::GoalStatus(
                serde_json::from_value(json!({
                    "goal":goal,"title":if reads < 3 {"Demo"} else {"Renamed"},
                    "governance":PublicKey([9;32]),"hosted_here":true,"host":PublicKey([2;32]),"governance_head":null,"current_rules":null,
                    "scope_halts":[],"members":[],"halted":null,
                    "abilities":[],"stalled":[],"peers":[]
                }))
                .unwrap(),
            ))
        }
        Request::GoalInvitations { goal: selected } => {
            assert_eq!(selected, goal);
            Ok(Response::Invitations {
                invitations: vec![],
            })
        }
        other => panic!("stale confirmation sent {other:?}"),
    });
    let plan = cli(home.path())
        .args([
            "--owner",
            "goal",
            "invite",
            "--goal",
            &goal.to_string(),
            "--plan",
        ])
        .output()
        .unwrap();
    let plan = envelope(&plan, 0);
    let confirm = cli(home.path())
        .args([
            "--owner",
            "goal",
            "invite",
            "--goal",
            &goal.to_string(),
            "--confirm",
            plan["result"]["plan_id"].as_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        envelope(&confirm, 7)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("plan changed")
    );
    handle.join().unwrap();
}

#[test]
fn nonowner_member_remove_reaches_daemon_authorization() {
    let home = scratch();
    let credential = home.path().join("agent.credential");
    write_secret(&credential, &[2; 32]);
    let goal = GoalId([4; 32]);
    let member = PublicKey([5; 32]);
    let handle = server_as(
        home.path(),
        1,
        Caller::Agent(PublicKey([2; 32])),
        move |frame| {
            assert_eq!(frame.on_behalf, None);
            assert_eq!(frame.request, Request::MemberRemove { goal, member });
            Err(ApiError::new(
                ErrorCode::Denied,
                "only the host can remove members",
            ))
        },
    );
    let output = cli(home.path())
        .arg("--credential")
        .arg(&credential)
        .args([
            "member",
            "remove",
            "--goal",
            &goal.to_string(),
            "--member",
            &member.to_string(),
        ])
        .output()
        .unwrap();
    assert_eq!(envelope(&output, 3)["error"]["code"], "denied");
    handle.join().unwrap();
}

#[test]
fn member_selector_resolves_local_name_and_visible_key_prefix() {
    use locust_proto::id::EndpointId;
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let worker = PublicKey([0xab; 32]);
    let handle = server(home.path(), 2, move |frame| match frame.request {
        Request::GoalStatus { goal: selected } => {
            assert_eq!(selected, goal);
            Ok(Response::GoalStatus(
                serde_json::from_value(json!({
                    "goal":goal,"title":"Demo","governance":PublicKey([9;32]),"hosted_here":true,"host":PublicKey([2;32]),
                    "governance_head":null,"current_rules":null,"scope_halts":[],
                    "members":[{"member":worker,"endpoint":EndpointId([3;32]),"local":true}],
                    "halted":null,"abilities":[],"stalled":[],"peers":[]
                }))
                .unwrap(),
            ))
        }
        Request::Status => Ok(status_agents(vec![AgentView {
            agent: worker,
            name: "worker".into(),
            author_only: false,
            revoked: false,
        }])),
        other => panic!("plan unexpectedly wrote {other:?}"),
    });
    for selector in ["worker", "ABABABAB"] {
        let output = cli(home.path())
            .args([
                "--owner",
                "member",
                "remove",
                "--goal",
                &goal.to_string(),
                "--member",
                selector,
                "--plan",
            ])
            .output()
            .unwrap();
        assert_eq!(
            envelope(&output, 0)["result"]["plan"]["member"],
            worker.to_string()
        );
    }
    handle.join().unwrap();
}

/// A goal whose host's agent is `host_agent` and whose other local agent is
/// `worker`; the host's agent is disconnected, which stops no host command.
fn hosted_goal_status(
    goal: GoalId,
    host_agent: PublicKey,
    hosted_here: bool,
) -> locust_proto::api::GoalStatus {
    use locust_proto::api::{GoalStatus, MemberView};
    use locust_proto::id::EndpointId;
    GoalStatus {
        goal,
        title: Some("Demo".into()),
        governance: PublicKey([9; 32]),
        hosted_here,
        host: Some(host_agent),
        governance_head: None,
        current_rules: None,
        scope_halts: vec![],
        members: vec![MemberView {
            member: host_agent,
            endpoint: EndpointId([3; 32]),
            local: hosted_here,
        }],
        halted: None,
        workspace: None,
        abilities: vec![],
        stalled: vec![],
        peers: vec![],
    }
}

#[test]
fn goal_add_needs_this_computer_to_host_the_goal() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let host_agent = PublicKey([2; 32]);
    let worker = PublicKey([3; 32]);
    let hosted_here = Arc::new(AtomicBool::new(true));
    let hosted_on_server = Arc::clone(&hosted_here);
    let handle = server(home.path(), 2, move |frame| match frame.request {
        Request::Status => Ok(status_agents(vec![
            AgentView {
                agent: host_agent,
                name: "maple".into(),
                author_only: false,
                revoked: true,
            },
            AgentView {
                agent: worker,
                name: "worker".into(),
                author_only: false,
                revoked: false,
            },
        ])),
        Request::GoalStatus { goal: selected } => {
            assert_eq!(selected, goal);
            Ok(Response::GoalStatus(hosted_goal_status(
                goal,
                host_agent,
                hosted_on_server.load(Ordering::SeqCst),
            )))
        }
        other => panic!("goal add wrote {other:?}"),
    });
    let run = || {
        cli(home.path())
            .args([
                "--owner",
                "--agent",
                "worker",
                "goal",
                "add",
                "--goal",
                &goal.to_string(),
                "--plan",
            ])
            .output()
            .unwrap()
    };
    // The host's agent is disconnected; the goal's own key still admits.
    let shown = envelope(&run(), 0);
    assert_eq!(shown["result"]["action"], "review_required");
    assert_eq!(shown["result"]["plan"]["agent"], worker.to_string());
    hosted_here.store(false, Ordering::SeqCst);
    let refused = envelope(&run(), 3);
    assert_eq!(refused["error"]["code"], "denied");
    assert_eq!(
        refused["error"]["message"],
        "this goal is hosted on another computer; request an invitation from its host"
    );
    assert!(refused.get("result").is_none(), "{refused}");
    handle.join().unwrap();
}

#[test]
fn agent_revoke_applies_at_once_and_prints_the_command_that_undoes_it() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let agent = PublicKey([2; 32]);
    let revoked = Arc::new(AtomicBool::new(false));
    let revokes = Arc::new(AtomicUsize::new(0));
    let reconnects = Arc::new(AtomicUsize::new(0));
    let (revoked_on_server, revokes_on_server, reconnects_on_server) = (
        Arc::clone(&revoked),
        Arc::clone(&revokes),
        Arc::clone(&reconnects),
    );
    let handle = server(home.path(), 4, move |frame| match frame.request {
        Request::Status => Ok(Response::Status(DaemonStatus {
            daemon_version: "stub".into(),
            endpoint: None,
            agents: vec![AgentView {
                agent,
                name: "worker".into(),
                author_only: false,
                revoked: revoked_on_server.load(Ordering::SeqCst),
            }],
            goals: vec![GoalSummary {
                goal,
                title: Some("Demo".into()),
                member: agent,
                membership: Membership::Member,
                halted: None,
                abilities: abilities(goal, agent),
            }],
        })),
        Request::GoalStatus { goal: selected } => {
            assert_eq!(selected, goal);
            Ok(Response::GoalStatus(hosted_goal_status(goal, agent, true)))
        }
        Request::AgentRevoke { agent: selected } => {
            assert_eq!(selected, agent);
            revokes_on_server.fetch_add(1, Ordering::SeqCst);
            revoked_on_server.store(true, Ordering::SeqCst);
            Ok(Response::Done)
        }
        Request::AgentReconnect { agent: selected } => {
            assert_eq!(selected, agent);
            reconnects_on_server.fetch_add(1, Ordering::SeqCst);
            revoked_on_server.store(false, Ordering::SeqCst);
            Ok(Response::Done)
        }
        other => panic!("unexpected {other:?}"),
    });
    // Nothing to plan or confirm: the flags do not exist and no connection is made.
    for flag in [&["--plan"][..], &["--confirm", "x"][..]] {
        let output = plain()
            .arg("--home")
            .arg(home.path())
            .args(["--owner", "agent", "revoke", "--agent", "worker"])
            .args(flag)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{flag:?}");
    }
    let output = cli(home.path())
        .args(["--owner", "agent", "revoke", "--agent", "worker"])
        .output()
        .unwrap();
    let result = envelope(&output, 0);
    assert_eq!(
        result["result"],
        json!({
            "agent": agent,
            "name": "worker",
            "connected": false,
            "changed": true,
            "hosted_goals": [{"goal": goal, "title": "Demo"}],
        })
    );
    assert_eq!(revokes.load(Ordering::SeqCst), 1);
    let human = |parts: &[&str]| {
        let output = plain()
            .arg("--home")
            .arg(home.path())
            .args(parts)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let reconnected = human(&["--owner", "agent", "reconnect", "--agent", "worker"]);
    assert!(
        reconnected.starts_with("worker is connected again.\n"),
        "{reconnected}"
    );
    assert_eq!(
        reconnected.lines().last(),
        Some("Undo: locust --owner agent revoke --agent worker")
    );
    assert_eq!(reconnects.load(Ordering::SeqCst), 1);
    let disconnected = human(&["--owner", "agent", "revoke", "--agent", "worker"]);
    assert!(
        disconnected.starts_with("worker is disconnected. The name stays taken.\n"),
        "{disconnected}"
    );
    let undo = disconnected.lines().last().unwrap();
    assert_eq!(undo, "Undo: locust --owner agent reconnect --agent worker");
    assert_eq!(revokes.load(Ordering::SeqCst), 2);
    let printed: Vec<&str> = undo
        .strip_prefix("Undo: locust ")
        .unwrap()
        .split(' ')
        .collect();
    let output = plain()
        .env("LOCUST_HOME", home.path())
        .args(&printed)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(reconnects.load(Ordering::SeqCst), 2);
    assert_eq!(revokes.load(Ordering::SeqCst), 2);
    assert!(!revoked.load(Ordering::SeqCst));
    handle.join().unwrap();
}

#[test]
fn member_remove_naming_the_hosts_agent_refuses_before_any_plan() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let host_agent = PublicKey([2; 32]);
    let handle = server(home.path(), 2, move |frame| match frame.request {
        Request::GoalStatus { goal: selected } => {
            assert_eq!(selected, goal);
            Ok(Response::GoalStatus(hosted_goal_status(
                goal, host_agent, true,
            )))
        }
        other => panic!("member remove wrote {other:?}"),
    });
    for flags in [&[][..], &["--plan"][..]] {
        let output = cli(home.path())
            .args([
                "--owner",
                "member",
                "remove",
                "--goal",
                &goal.to_string(),
                "--member",
                &host_agent.to_string(),
            ])
            .args(flags)
            .output()
            .unwrap();
        let refused = envelope(&output, 7);
        assert_eq!(refused["error"]["code"], "conflict");
        assert_eq!(
            refused["error"]["message"],
            "the host's agent cannot be removed from its own goal"
        );
        assert!(refused.get("result").is_none(), "{refused}");
    }
    handle.join().unwrap();
}

#[test]
fn goal_leave_naming_the_hosts_agent_refuses_before_any_plan() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let host_agent = PublicKey([2; 32]);
    let handle = server(home.path(), 2, move |frame| match frame.request {
        Request::GoalStatus { goal: selected } => {
            assert_eq!(selected, goal);
            Ok(Response::GoalStatus(hosted_goal_status(
                goal, host_agent, true,
            )))
        }
        other => panic!("goal leave wrote {other:?}"),
    });
    for flags in [&[][..], &["--plan"][..]] {
        let output = cli(home.path())
            .args([
                "--owner",
                "goal",
                "leave",
                "--goal",
                &goal.to_string(),
                "--agent",
                &host_agent.to_string(),
            ])
            .args(flags)
            .output()
            .unwrap();
        let refused = envelope(&output, 7);
        assert_eq!(refused["error"]["code"], "conflict");
        assert_eq!(
            refused["error"]["message"],
            "the host's agent cannot leave its own goal"
        );
        assert!(refused.get("result").is_none(), "{refused}");
    }
    handle.join().unwrap();
}

#[test]
fn subtask_revision_plan_names_parent_rules() {
    use locust_proto::api::{TaskDetail, TaskView};
    use locust_proto::event::{Context, Scope, TaskId};
    use locust_proto::id::EventId;
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let task = TaskId::Authored(EventId([5; 32]));
    let parent = TaskId::Authored(EventId([6; 32]));
    let handle = server(home.path(), 1, move |frame| match frame.request {
        Request::GoalStatus { goal: selected } => {
            assert_eq!(selected, goal);
            Ok(Response::GoalStatus(
                serde_json::from_value(json!({
                    "goal":goal,"title":"Demo","governance":PublicKey([9;32]),"hosted_here":true,"host":PublicKey([2;32]),
                    "governance_head":null,"current_rules":null,"scope_halts":[],
                    "members":[],"halted":null,"abilities":[],"stalled":[],"peers":[]
                }))
                .unwrap(),
            ))
        }
        Request::Task {
            goal: selected,
            task: selected_task,
        } => {
            assert_eq!((selected, selected_task), (goal, task));
            Ok(Response::Task(TaskDetail {
                view: TaskView {
                    task,
                    context: Context {
                        scope: Scope::Task(task),
                        round: EventId([7; 32]),
                    },
                    creator: PublicKey([2; 32]),
                    by_host: false,
                    title: Some("Child task".into()),
                    attempts: vec![],
                    contributions: vec![],
                    completed: false,
                    selected: None,
                    closed: false,
                },
                text: None,
                inputs: Default::default(),
                parent: Some(parent),
                task_type: None,
                effective_rules_json: "{}".into(),
            }))
        }
        other => panic!("plan unexpectedly wrote {other:?}"),
    });
    let output = plain()
        .arg("--home")
        .arg(home.path())
        .args([
            "--owner",
            "task",
            "revise",
            "--goal",
            &goal.to_string(),
            "--task",
            &task.to_string(),
            "--plan",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let human = String::from_utf8(output.stdout).unwrap();
    assert!(human.contains("under its parent task's rules"), "{human}");
    assert!(!human.contains("under the current rules"), "{human}");
    handle.join().unwrap();
}

#[test]
fn raw_call_invite_sends_exact_request_without_plan_reads() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let handle = server(home.path(), 1, move |frame| {
        assert_eq!(frame.on_behalf, None);
        assert_eq!(
            frame.request,
            Request::GoalInvite {
                goal,
                expires_ms: 42
            }
        );
        Ok(Response::Invited {
            ticket: locust_proto::invite::Ticket("raw-ticket".into()),
        })
    });
    let output = cli(home.path())
        .args([
            "--owner",
            "call",
            "goal.invite",
            &json!({"goal":goal,"expires_ms":42}).to_string(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        envelope(&output, 0)["result"]["invited"]["ticket"],
        "raw-ticket"
    );
    handle.join().unwrap();
}

#[test]
fn invitation_revoke_all_runs_immediately_and_reports_count() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let handle = server(home.path(), 1, move |frame| match frame.request {
        Request::GoalStatus { goal: selected } => {
            assert_eq!(selected, goal);
            Ok(Response::GoalStatus(
                serde_json::from_value(json!({
                    "goal":goal,"title":"Demo","governance":PublicKey([9;32]),"hosted_here":true,"host":PublicKey([2;32]),
                    "governance_head":null,"current_rules":null,"scope_halts":[],
                    "members":[],"halted":null,"abilities":[],"stalled":[],"peers":[]
                }))
                .unwrap(),
            ))
        }
        Request::GoalInvitations { goal: selected } => {
            assert_eq!(selected, goal);
            Ok(Response::Invitations {
                invitations: vec![],
            })
        }
        Request::InvitationRevoke {
            goal: selected,
            invitation,
        } => {
            assert_eq!((selected, invitation), (goal, None));
            Ok(Response::InvitationsRevoked { count: 2 })
        }
        other => panic!("unexpected planning request {other:?}"),
    });
    let output = cli(home.path())
        .args([
            "--owner",
            "invitation",
            "revoke",
            "--goal",
            &goal.to_string(),
            "--all",
        ])
        .output()
        .unwrap();
    assert_eq!(
        envelope(&output, 0)["result"]["invitations_revoked"]["count"],
        2
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
            Err(ApiError::new(ErrorCode::LevelRequired, "level needed")),
            4,
            "level_required",
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
        if expected.is_empty() {
            // A quiet wait is a result, not a failure, whatever its status.
            assert_eq!(body["ok"], true);
        } else {
            assert_eq!(body["error"]["code"], expected);
        }
        handle.join().unwrap();
    }
}
#[test]
fn wait_help_names_the_statuses_of_a_quiet_wait() {
    let output = plain().args(["wait", "--help"]).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let help = String::from_utf8(output.stdout).unwrap();
    for status in [
        "20 when nothing changed before the timeout",
        "21 when nothing changed and no peer of the goal is reachable",
    ] {
        assert!(help.contains(status), "{help}");
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
        "--agent",
        &PublicKey([2; 32]).to_string(),
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
fn unknown_context_receipt_is_not_found_without_operating_system_text() {
    let home = scratch();
    let credential = home.path().join("worker.credential");
    let session = home.path().join("worker.secret");
    write_secret(&credential, &[1; 32]);
    write_secret(&session, &[7; 32]);
    // No daemon runs: the reference is resolved before any connection.
    let acknowledge = |receipt: &str| {
        cli(home.path())
            .arg("--credential")
            .arg(&credential)
            .arg("--session")
            .arg(&session)
            .args(["context", "acknowledge", "--goal", &"03".repeat(32)])
            .args(["--receipt", receipt])
            .output()
            .unwrap()
    };
    let unknown = envelope(&acknowledge(&format!("ctx:{}", "0".repeat(64))), 5);
    assert_eq!(unknown["error"]["code"], "not_found");
    let message = unknown["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("Read context again with locust context read"),
        "{message}"
    );
    assert!(!message.contains("os error"), "{message}");
    let malformed = envelope(&acknowledge("ctx:typo"), 6);
    assert_eq!(malformed["error"]["code"], "invalid");
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
    let handle = server(home.path(), 2, move |frame| {
        if matches!(frame.request, Request::Status) {
            return Ok(status(vec![]));
        }
        assert_eq!(
            frame.request,
            Request::GoalJoin {
                agent: PublicKey([2; 32]),
                ticket: expected.clone(),
                level: locust_proto::api::Level::Auto,
            }
        );
        Ok(Response::Joined {
            goal: invitation.goal,
            governance: invitation.governance,
            membership: Membership::Joining,
            level: locust_proto::api::Level::Auto,
        })
    });
    let run = |extra: &[&str]| {
        let mut command = cli(home.path());
        command
            .args([
                "--owner",
                "--agent",
                &PublicKey([2; 32]).to_string(),
                "goal",
                "join",
                "--ticket",
                "-",
            ])
            .args(extra)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        write!(child.stdin.take().unwrap(), "{}\r\n", ticket.as_str()).unwrap();
        child.wait_with_output().unwrap()
    };
    let plan = run(&[]);
    let plan = envelope(&plan, 0);
    assert_eq!(plan["result"]["action"], "review_required");
    let child = run(&["--confirm", plan["result"]["plan_id"].as_str().unwrap()]);
    assert_eq!(
        envelope(&child, 0)["result"]["joined"]["membership"],
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
                abilities: abilities(GoalId([3; 32]), PublicKey([4; 32])),
            },
            GoalSummary {
                goal: GoalId([5; 32]),
                member: PublicKey([4; 32]),
                title: None,
                membership: Membership::Joining,
                halted: None,
                abilities: abilities(GoalId([5; 32]), PublicKey([4; 32])),
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
fn a_closed_reader_ends_the_command_quietly_with_its_own_status() {
    let home = scratch();
    let home = home.path().to_str().unwrap();
    // Help, the human rendering, the JSON envelope and a failure's envelope.
    for (args, own) in [
        (&["--help"][..], 0),
        (&["--version"], 0),
        (&["--json", "--help"], 0),
        (&["contract"], 0),
        (&["--json", "contract"], 0),
        (&["--json", "--home", home, "status"], 2),
    ] {
        let (status, stderr) = closed_reader(plain().args(args), false);
        assert_eq!((status, stderr.as_str()), (Some(own), ""), "{args:?}");
    }
    // A failure in text and the help of `mcp` are written to standard error.
    for (args, own) in [
        (&["--home", home, "status"][..], 2),
        (&["mcp", "--help"], 0),
    ] {
        let (status, stdout) = closed_reader(plain().args(args), true);
        assert_eq!((status, stdout.as_str()), (Some(own), ""), "{args:?}");
    }
}
#[test]
fn a_committed_write_and_a_quiet_wait_keep_their_status_without_a_reader() {
    let goal = GoalId([3; 32]);
    let publish = [
        "contribution",
        "publish",
        "--goal",
        &goal.to_string(),
        "A finding",
    ]
    .map(str::to_owned);
    let wait = [
        "call".to_owned(),
        "wait".to_owned(),
        json!({"goal": goal, "seen": 1, "timeout_ms": 0}).to_string(),
    ];
    for (args, answer, own) in [
        (
            &publish[..],
            Response::Recorded {
                event: locust_proto::id::EventId([6; 32]),
            },
            0,
        ),
        (&wait[..], Response::Waited(WaitOutcome::NoEvent), 20),
    ] {
        let home = scratch();
        write_secret(&home.path().join("owner.credential"), &[1; 32]);
        let handle = server(home.path(), 1, move |frame| {
            assert!(matches!(
                frame.request,
                Request::ContributionPublish { .. } | Request::Wait { .. }
            ));
            Ok(answer.clone())
        });
        let principal = PublicKey([5; 32]).to_string();
        let (status, stderr) = closed_reader(
            cli(home.path())
                .args(["--owner", "--agent", &principal])
                .args(args),
            false,
        );
        assert_eq!((status, stderr.as_str()), (Some(own), ""), "{args:?}");
        // The daemon received and answered the request all the same.
        assert_eq!(handle.join().unwrap().len(), 1);
    }
}
#[test]
fn watch_stops_observing_when_nobody_can_read_it() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let handle = server(home.path(), 1, |frame| match frame.request {
        Request::Status => Ok(status(vec![])),
        Request::Pending { .. } => Ok(Response::Pending(Default::default())),
        other => panic!("watch sent {other:?} with nobody reading"),
    });
    let (status, stderr) = closed_reader(
        plain().arg("--home").arg(home.path()).args([
            "--owner",
            "watch",
            "--goal",
            &GoalId([3; 32]).to_string(),
        ]),
        false,
    );
    assert_eq!((status, stderr.as_str()), (Some(0), ""));
    handle.join().unwrap();
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
            abilities: abilities(goal, agent),
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
                by_host: false,
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
            "--agent",
            &agent.to_string(),
            "work",
            "offer",
            "--goal",
            "Demo",
            "--task",
            "Fix greeting",
            "--member",
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
                    abilities: abilities(GoalId([n; 32]), PublicKey([3; 32])),
                })
                .collect(),
        ))
    });
    let output = cli(home.path())
        .args([
            "--owner",
            "--agent",
            &PublicKey([3; 32]).to_string(),
            "goal",
            "leave",
            "--goal",
            "Demo",
        ])
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
                            by_owner: false,
                            by_host: false,
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
                "--agent",
                &PublicKey([3; 32]).to_string(),
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
