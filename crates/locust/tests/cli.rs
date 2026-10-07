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
        waiting: vec![],
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
        waiting: vec![],
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
    let handle = server(home.path(), 14, move |frame| {
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
                waiting: vec![],
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
                    name: "agent".into(),
                    host_name: None,
                    invitations_open: 0,
                    invitations_expire_ms: None,
                    halted: None,
                    abilities: current(level, allowed),
                }],
            })),
            Request::GoalStatus { goal: selected } => {
                assert_eq!(selected, goal);
                Ok(Response::GoalStatus(GoalStatus {
                    host_name: Some("Host".into()),
                    roles: Default::default(),
                    deciding: Default::default(),
                    acting_alone: Default::default(),

                    goal,
                    title: Some("Goal title".into()),
                    governance: PublicKey([9; 32]),
                    hosted_here: true,
                    host: Some(agent),
                    governance_head: None,
                    current_rules: None,
                    scope_halts: vec![],
                    members: vec![MemberView {
                        name: "Member".into(),

                        member: agent,
                        endpoint: EndpointId([3; 32]),
                        local: true,
                        admitted: 0,
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
    // At read the allowance is stored but settles nothing; the level line is
    // the way on.
    let at_read = run(&[
        "--owner",
        "allow",
        "--goal",
        &goal.to_string(),
        "--task",
        &task.to_string(),
    ]);
    assert!(
        at_read.contains(
            "\"Task title\" is allowed for worker, but at read it only reads. It takes the task once it is set to ask:\n  locust --owner level --goal abababab --agent worker ask\n"
        ),
        "{at_read}"
    );
    assert!(!at_read.contains("may take"), "{at_read}");
    run_undo(&at_read);
    run(&["--owner", "level", "--goal", &goal.to_string(), "ask"]);
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
    assert!(
        changed.contains("Undo: locust --owner allow --goal abababab --task task:cdcdcdcd --agent worker --revoke"),
        "{changed}"
    );
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
        name: "agent".into(),
        host_name: None,
        invitations_open: 0,
        invitations_expire_ms: None,
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
                    name: "agent".into(),
                    host_name: None,
                    invitations_open: 0,
                    invitations_expire_ms: None,
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
                waiting: vec![],
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
    let mut created = false;
    let handle = server(home.path(), 3, move |frame| {
        if matches!(frame.request, Request::Status) {
            return Ok(if created {
                status(vec![])
            } else {
                status_agents(vec![AgentView {
                    agent,
                    name: "Maple".into(),
                    author_only: false,
                    revoked: false,
                }])
            });
        }
        assert_eq!(frame.on_behalf, None);
        let Request::GoalCreate {
            agent: selected,
            title,
            name,
            ..
        } = frame.request
        else {
            panic!("expected goal.create");
        };
        created = true;
        assert_eq!(selected, agent);
        assert_eq!(title, "Owner's goal");
        assert_eq!(name, "Maple");
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
                    "goal":goal,"title":"Demo","governance":PublicKey([9;32]),"hosted_here":true,"host_name":"Host","roles":{},"deciding":[],"acting_alone":[],"host":PublicKey([2;32]),
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
        // The "Stop admission" line cuts the goal among the daemon's goals.
        if matches!(frame.request, Request::Status) {
            return Ok(status_agents(vec![]));
        }
        assert_eq!(frame.on_behalf, None);
        let Request::GoalInvite {
            goal: selected,
            expires_ms,
            ..
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
                    "governance":PublicKey([9;32]),"hosted_here":true,"host_name":"Host","roles":{},"deciding":[],"acting_alone":[],"host":PublicKey([2;32]),"governance_head":null,"current_rules":null,
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
                    "goal":goal,"title":"Demo","governance":PublicKey([9;32]),"hosted_here":true,"host_name":"Host","roles":{},"deciding":[],"acting_alone":[],"host":PublicKey([2;32]),
                    "governance_head":null,"current_rules":null,"scope_halts":[],
                    "members":[{"member":worker,"name":"Member","endpoint":EndpointId([3;32]),"local":true,"admitted":0}],
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
        host_name: Some("Host".into()),
        roles: Default::default(),
        deciding: Default::default(),
        acting_alone: Default::default(),

        goal,
        title: Some("Demo".into()),
        governance: PublicKey([9; 32]),
        hosted_here,
        host: Some(host_agent),
        governance_head: None,
        current_rules: None,
        scope_halts: vec![],
        members: vec![MemberView {
            name: "Member".into(),

            member: host_agent,
            endpoint: EndpointId([3; 32]),
            local: hosted_here,
            admitted: 0,
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
    let handle = server(home.path(), 5, move |frame| match frame.request {
        Request::Status => Ok(Response::Status(DaemonStatus {
            daemon_version: "stub".into(),
            endpoint: None,
            waiting: vec![],
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
                name: "agent".into(),
                host_name: None,
                invitations_open: 0,
                invitations_expire_ms: None,
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
    assert_eq!(
        human(&["--owner", "agent", "reconnect", "--agent", "worker"]),
        "worker is not disconnected. Nothing changed.\n"
    );
    assert_eq!(reconnects.load(Ordering::SeqCst), 2);
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
                    "goal":goal,"title":"Demo","governance":PublicKey([9;32]),"hosted_here":true,"host_name":"Host","roles":{},"deciding":[],"acting_alone":[],"host":PublicKey([2;32]),
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
                expires_ms: 42,
                role: None,
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
                    "goal":goal,"title":"Demo","governance":PublicKey([9;32]),"hosted_here":true,"host_name":"Host","roles":{},"deciding":[],"acting_alone":[],"host":PublicKey([2;32]),
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
        // The "Invite again" line cuts the goal among the daemon's goals.
        Request::Status => Ok(status_agents(vec![])),
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
        "Host".into(),
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
                name: "Member".into(),

                agent: PublicKey([2; 32]),
                ticket: expected.clone(),
                level: locust_proto::api::Level::Auto,
            }
        );
        Ok(Response::Joined {
            host_name: "Host".into(),
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
                "--name",
                "Member",
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
fn a_repeated_join_says_an_admitted_name_stays() {
    use locust_proto::id::EndpointId;
    use locust_proto::invite::{Invitation, InviteSecret};
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([3; 32]);
    let agent = PublicKey([2; 32]);
    let invitation = Invitation::signed(
        goal,
        Some("Harbor work".into()),
        EndpointId([5; 32]),
        vec![],
        InviteSecret([6; 32]),
        None,
        "Host".into(),
        None,
        &locust_proto::crypto::Keypair::from_seed([4; 32]),
    )
    .unwrap();
    let ticket = invitation.to_ticket().unwrap();
    let ticket_file = home.path().join("ticket");
    fs::write(&ticket_file, ticket.as_str()).unwrap();
    fs::set_permissions(&ticket_file, fs::Permissions::from_mode(0o600)).unwrap();
    for (waiting, expected) in [
        (
            false,
            "Joining \"Harbor work\" as Member (auto). Admission comes from the host's computer; locust --owner status shows it.",
        ),
        (
            true,
            "Joining \"Harbor work\" as Member (auto), unless the host's computer already admitted it under the earlier name; an admitted name stays. Admission comes from the host's computer; locust --owner status shows it.",
        ),
    ] {
        let _ = fs::remove_file(home.path().join("daemon.sock"));
        let handle = server(home.path(), 2, move |frame| match frame.request {
            Request::Status => Ok(status(if waiting {
                vec![GoalSummary {
                    goal,
                    title: Some("Harbor work".into()),
                    member: agent,
                    membership: Membership::Joining,
                    name: "Earlier".into(),
                    host_name: Some("Host".into()),
                    invitations_open: 0,
                    invitations_expire_ms: None,
                    halted: None,
                    abilities: Abilities {
                        name: "Earlier".into(),
                        host: None,
                        hosted_here: false,
                        ..abilities(goal, agent)
                    },
                }]
            } else {
                vec![]
            })),
            Request::GoalJoin { .. } => Ok(Response::Joined {
                host_name: "Host".into(),
                goal,
                governance: invitation.governance,
                membership: Membership::Joining,
                level: locust_proto::api::Level::Auto,
            }),
            request => panic!("unexpected {request:?}"),
        });
        let run = |extra: &[&str]| {
            plain()
                .arg("--home")
                .arg(home.path())
                .args([
                    "--owner",
                    "--agent",
                    &agent.to_string(),
                    "goal",
                    "join",
                    "--name",
                    "Member",
                    "--ticket-file",
                    ticket_file.to_str().unwrap(),
                ])
                .args(extra)
                .output()
                .unwrap()
        };
        let plan = run(&["--plan"]);
        assert!(
            plan.status.success(),
            "{}",
            String::from_utf8_lossy(&plan.stderr)
        );
        let id = String::from_utf8(plan.stdout)
            .unwrap()
            .lines()
            .find_map(|line| line.strip_prefix("Plan id: "))
            .unwrap()
            .to_string();
        let joined = run(&["--confirm", &id]);
        assert!(
            joined.status.success(),
            "{}",
            String::from_utf8_lossy(&joined.stderr)
        );
        assert_eq!(
            String::from_utf8(joined.stdout).unwrap().trim_end(),
            expected
        );
        handle.join().unwrap();
    }
}

#[test]
fn human_status_names_membership_and_halt_with_stable_tags() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let handle = server(home.path(), 1, |_| {
        let Response::Status(mut view) = status(vec![
            GoalSummary {
                goal: GoalId([3; 32]),
                member: PublicKey([4; 32]),
                title: Some("a goal".into()),
                membership: Membership::Refused,
                name: "worker".into(),
                host_name: None,
                invitations_open: 0,
                invitations_expire_ms: None,
                halted: Some(locust_proto::api::Halt::AuthorityConflict),
                abilities: Abilities {
                    name: "worker".into(),
                    ..abilities(GoalId([3; 32]), PublicKey([4; 32]))
                },
            },
            GoalSummary {
                goal: GoalId([5; 32]),
                member: PublicKey([4; 32]),
                title: None,
                membership: Membership::Joining,
                name: "worker".into(),
                host_name: Some("Harbor".into()),
                invitations_open: 0,
                invitations_expire_ms: None,
                halted: None,
                abilities: Abilities {
                    name: "worker".into(),
                    host: None,
                    hosted_here: false,
                    ..abilities(GoalId([5; 32]), PublicKey([4; 32]))
                },
            },
        ]) else {
            unreachable!("status fixture")
        };
        view.agents = vec![
            AgentView {
                agent: PublicKey([4; 32]),
                name: "worker".into(),
                author_only: false,
                revoked: false,
            },
            AgentView {
                agent: PublicKey([6; 32]),
                name: "idle".into(),
                author_only: false,
                revoked: false,
            },
        ];
        Ok(Response::Status(view))
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
    assert!(text.starts_with("Nothing is waiting for you.\n"), "{text}");
    // The halted goal and the joining agent show under their goals only.
    assert!(
        text.contains(
            "a goal (03030303) · host: you · halted\n  Blocked: conflicting authority history."
        ),
        "{text}"
    );
    assert_eq!(text.matches("decisions cannot advance").count(), 1);
    assert!(
        text.contains("  worker · refused\n      The invitation was refused. Ask the goal host for a fresh invitation, inspect it, and join again."),
        "{text}"
    );
    assert!(
        text.contains("Title unavailable (05050505) · host: on another computer · the ticket names Harbor; not confirmed until admission arrives"),
        "{text}"
    );
    assert_eq!(
        text.matches("Admission has not arrived. It comes from the host's computer when that computer is on; nothing here waits for you.").count(),
        1,
        "{text}"
    );
    assert!(!text.contains("Waiting for you"), "{text}");
    assert!(
        text.contains("\nidle is connected and in no goal.\nDaemon stub"),
        "{text}"
    );
    handle.join().unwrap();
}

#[test]
fn a_role_refusal_names_the_role_and_the_goals_roles_for_the_person() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let host = PublicKey([2; 32]);
    let handle = server(home.path(), 1, move |frame| match frame.request {
        Request::Status => Ok(status(vec![GoalSummary {
            goal,
            title: Some("Demo".into()),
            member: host,
            membership: Membership::Member,
            name: "agent".into(),
            host_name: None,
            invitations_open: 0,
            invitations_expire_ms: None,
            halted: None,
            abilities: abilities(goal, host),
        }])),
        Request::GoalStatus { .. } => {
            Ok(Response::GoalStatus(hosted_goal_status(goal, host, true)))
        }
        Request::RoleGive { .. } => Err(ApiError {
            code: ErrorCode::NotFound,
            message: "this goal has no such role".into(),
            details_json: Some(
                json!({"role": "reviewers\u{202e}", "roles": ["lead", "reviewer"]}).to_string(),
            ),
        }),
        request => panic!("unexpected {request:?}"),
    });
    let output = plain()
        .arg("--home")
        .arg(home.path())
        .args([
            "--owner",
            "role",
            "give",
            "--goal",
            &goal.to_string(),
            "--member",
            &host.to_string(),
            "reviewers",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(5));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        stderr.trim_end(),
        "locust: not_found: this goal has no such role Role: reviewers\\u{202e} Roles here: lead, reviewer"
    );
    handle.join().unwrap();

    // A bind refused because a role would change kind names the role once,
    // and the JSON message is the daemon's own.
    let message = "this role is a group in this goal; these rules make it pick or close. Use another role name.";
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let handle = server(home.path(), 4, move |frame| match frame.request {
        Request::Status => Ok(status(vec![])),
        Request::GoalStatus { .. } => {
            let mut view = hosted_goal_status(goal, host, true);
            view.current_rules = Some(locust_proto::id::EventId([5; 32]));
            Ok(Response::GoalStatus(view))
        }
        Request::RulesBind { .. } => Err(ApiError {
            code: ErrorCode::Conflict,
            message: message.into(),
            details_json: Some(json!({"role": "reviewer"}).to_string()),
        }),
        request => panic!("unexpected {request:?}"),
    });
    for json in [false, true] {
        let run = |extra: &[&str]| {
            let mut command = plain();
            command.arg("--home").arg(home.path());
            if json {
                command.arg("--json");
            }
            command
                .args([
                    "--owner",
                    "rules",
                    "bind",
                    "--goal",
                    &goal.to_string(),
                    "--formation",
                    "directed",
                ])
                .args(extra)
                .output()
                .unwrap()
        };
        let plan = run(&["--plan"]);
        let id = if json {
            envelope(&plan, 0)["result"]["plan_id"]
                .as_str()
                .unwrap()
                .to_string()
        } else {
            String::from_utf8(plan.stdout)
                .unwrap()
                .lines()
                .find_map(|line| line.strip_prefix("Plan id: "))
                .unwrap()
                .to_string()
        };
        let refused = run(&["--confirm", &id]);
        if json {
            assert_eq!(envelope(&refused, 7)["error"]["message"], message);
        } else {
            assert_eq!(refused.status.code(), Some(7));
            assert_eq!(
                String::from_utf8(refused.stderr).unwrap().trim_end(),
                format!("locust: conflict: {message} Role: reviewer")
            );
        }
    }
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
        // Names and tasks for the view, read before the first print.
        Request::GoalStatus { goal } => Ok(Response::GoalStatus(hosted_goal_status(
            goal,
            PublicKey([2; 32]),
            true,
        ))),
        Request::Board { .. } => Ok(Response::Board(vec![])),
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
fn watch_names_a_task_and_a_member_that_appeared_while_it_waited() {
    use locust_proto::api::{Attempting, MemberView, PendingWork, TaskView, WaitOutcome, WorkItem};
    use locust_proto::event::{Context, Scope, TaskId};
    use locust_proto::id::{EndpointId, EventId};
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([3; 32]);
    let host = PublicKey([2; 32]);
    let juniper = PublicKey([7; 32]);
    let task = TaskId::Authored(EventId([4; 32]));
    let mut status_reads = 0;
    let mut board_reads = 0;
    let handle = server(home.path(), 1, move |frame| match frame.request {
        Request::Status => Ok(status(vec![])),
        Request::Pending { .. } => Ok(Response::Pending(Default::default())),
        Request::GoalStatus { .. } => {
            status_reads += 1;
            let mut view = hosted_goal_status(goal, host, true);
            if status_reads > 1 {
                view.members.push(MemberView {
                    name: "Juniper".into(),
                    member: juniper,
                    endpoint: EndpointId([3; 32]),
                    local: false,
                    admitted: 1,
                });
            }
            Ok(Response::GoalStatus(view))
        }
        Request::Board { .. } => {
            board_reads += 1;
            Ok(Response::Board(if board_reads > 1 {
                vec![TaskView {
                    task,
                    context: Context {
                        scope: Scope::Task(task),
                        round: EventId([4; 32]),
                    },
                    creator: host,
                    by_host: false,
                    title: Some("Fix the parser".into()),
                    attempts: vec![],
                    contributions: vec![],
                    completed: false,
                    selected: None,
                    closed: false,
                }]
            } else {
                vec![]
            }))
        }
        Request::Wait { .. } => Ok(Response::Waited(WaitOutcome::Work(Box::new(PendingWork {
            revision: 2,
            to_start: vec![WorkItem {
                task,
                offer: None,
                attempting: vec![Attempting {
                    member: juniper,
                    status: None,
                }],
                results: 0,
            }],
            ..Default::default()
        })))),
        other => panic!("unexpected {other:?}"),
    });
    let output = plain()
        .arg("--home")
        .arg(home.path())
        .args(["--owner", "watch", "--goal", &goal.to_string()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains(
            "Ready to start: Fix the parser (task:04040404)\n  Attempting: Juniper (07070707)"
        ),
        "{text}"
    );
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
            name: "agent".into(),
            host_name: None,
            invitations_open: 0,
            invitations_expire_ms: None,
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
                    name: "agent".into(),
                    host_name: None,
                    invitations_open: 0,
                    invitations_expire_ms: None,
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

#[test]
fn role_give_sends_the_holders_it_read_and_a_change_in_between_is_conflict() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let host = PublicKey([2; 32]);
    let member = PublicKey([3; 32]);
    let handle = server(home.path(), 1, move |frame| match frame.request {
        Request::GoalStatus { .. } => {
            let mut view = hosted_goal_status(goal, host, true);
            view.roles.insert("reviewer".into(), vec![host]);
            view.members.push(locust_proto::api::MemberView {
                member,
                name: "Maple".into(),
                endpoint: locust_proto::id::EndpointId([3; 32]),
                local: false,
                admitted: 0,
            });
            Ok(Response::GoalStatus(view))
        }
        Request::RoleGive {
            goal: selected,
            role,
            member: selected_member,
            expected,
        } => {
            assert_eq!(
                (selected, role.as_str(), selected_member, expected),
                (goal, "reviewer", member, vec![host])
            );
            Err(ApiError::new(
                ErrorCode::Conflict,
                "the holders of this role changed; look again and repeat",
            ))
        }
        request => panic!("unexpected {request:?}"),
    });
    let output = cli(home.path())
        .args([
            "--owner",
            "role",
            "give",
            "--goal",
            &goal.to_string(),
            "--member",
            &member.to_string(),
            "reviewer",
        ])
        .output()
        .unwrap();
    assert_eq!(envelope(&output, 7)["error"]["code"], "conflict");
    handle.join().unwrap();
}

#[test]
fn role_undo_lines_put_the_holders_back_and_name_the_member_by_key() {
    use std::sync::{Arc, Mutex};
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let host = PublicKey([2; 32]);
    let member = PublicKey([3; 32]);
    let role = "review team's";
    let holders = Arc::new(Mutex::new(vec![host]));
    let server_holders = holders.clone();
    let handle = server(home.path(), 6, move |frame| match frame.request {
        Request::Status => Ok(status(vec![GoalSummary {
            goal,
            title: Some("Demo".into()),
            member: host,
            membership: Membership::Member,
            name: "agent".into(),
            host_name: None,
            invitations_open: 0,
            invitations_expire_ms: None,
            halted: None,
            abilities: abilities(goal, host),
        }])),
        Request::GoalStatus { .. } => {
            let mut view = hosted_goal_status(goal, host, true);
            view.roles
                .insert(role.into(), server_holders.lock().unwrap().clone());
            view.members[0].name = "Harbor".into();
            view.members.push(locust_proto::api::MemberView {
                member,
                name: "Maple ; touch forbidden".into(),
                endpoint: locust_proto::id::EndpointId([3; 32]),
                local: false,
                admitted: 0,
            });
            Ok(Response::GoalStatus(view))
        }
        Request::RoleGive {
            member, expected, ..
        } => {
            let mut held = server_holders.lock().unwrap();
            assert_eq!(*held, expected);
            held.push(member);
            held.sort();
            held.dedup();
            Ok(Response::Recorded {
                event: locust_proto::id::EventId([5; 32]),
            })
        }
        Request::RoleTake {
            member, expected, ..
        } => {
            let mut held = server_holders.lock().unwrap();
            assert_eq!(*held, expected);
            if *held == vec![host] && member == host {
                return Ok(Response::Done);
            }
            held.retain(|key| *key != member);
            if held.is_empty() {
                held.push(host);
            }
            Ok(Response::Recorded {
                event: locust_proto::id::EventId([6; 32]),
            })
        }
        request => panic!("unexpected {request:?}"),
    });
    let run = |verb: &str, who: PublicKey| {
        let output = plain()
            .arg("--home")
            .arg(home.path())
            .args([
                "--owner",
                "role",
                verb,
                "--goal",
                &goal.to_string(),
                "--member",
                &who.to_string(),
                role,
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let give = run("give", member);
    assert!(
        give.contains(
            "Undo: locust --owner role take --goal 04040404 --member 03030303 'review team'\\''s'"
        ),
        "{give}"
    );
    assert!(
        !give
            .lines()
            .find(|line| line.starts_with("Undo:"))
            .unwrap()
            .contains("Maple")
    );
    let take_host = run("take", host);
    assert!(take_host.contains("Undo: locust --owner role give --goal 04040404 --member 02020202"));
    let take_last = run("take", member);
    assert!(!take_last.contains("Undo:"));
    assert!(take_last.contains("Give it back:"));
    assert!(take_last.contains("The host's agent holds"));
    let no_change = run("take", host);
    assert!(no_change.contains("stays with the host's agent"));
    assert!(!no_change.contains("Undo:"));
    let binary_dir = std::path::Path::new(env!("CARGO_BIN_EXE_locust"))
        .parent()
        .unwrap();
    let shell_path = format!(
        "{}:{}",
        binary_dir.display(),
        std::env::var("PATH").unwrap()
    );
    for line in take_last.lines().filter(|line| line.contains(": locust ")) {
        let (_, command) = line.split_once(": ").unwrap();
        let output = Command::new("sh")
            .args(["-c", command])
            .env("PATH", &shell_path)
            .env("LOCUST_HOME", home.path())
            .env_remove("LOCUST_CREDENTIAL")
            .env_remove("LOCUST_SESSION")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(*holders.lock().unwrap(), vec![member]);
    handle.join().unwrap();
    assert_eq!(
        plain()
            .args([
                "--owner",
                "role",
                "give",
                "--goal",
                &goal.to_string(),
                "--member",
                &member.to_string(),
                role,
                "--plan"
            ])
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn a_role_named_like_a_flag_prints_a_runnable_line_and_hidden_characters_print_none() {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let host = PublicKey([2; 32]);
    let member = PublicKey([3; 32]);
    let hidden = "rev\u{202e}iewer";
    let holders: Arc<Mutex<BTreeMap<String, Vec<PublicKey>>>> = Arc::new(Mutex::new(
        [
            ("--help".to_owned(), vec![host]),
            (hidden.to_owned(), vec![host]),
        ]
        .into(),
    ));
    let server_holders = holders.clone();
    let handle = server(home.path(), 3, move |frame| match frame.request {
        Request::Status => Ok(status(vec![GoalSummary {
            goal,
            title: Some("Demo".into()),
            member: host,
            membership: Membership::Member,
            name: "agent".into(),
            host_name: None,
            invitations_open: 0,
            invitations_expire_ms: None,
            halted: None,
            abilities: abilities(goal, host),
        }])),
        Request::GoalStatus { .. } => {
            let mut view = hosted_goal_status(goal, host, true);
            view.roles = server_holders.lock().unwrap().clone();
            view.members.push(locust_proto::api::MemberView {
                member,
                name: "Maple".into(),
                endpoint: locust_proto::id::EndpointId([3; 32]),
                local: false,
                admitted: 1,
            });
            Ok(Response::GoalStatus(view))
        }
        Request::RoleGive {
            role,
            member,
            expected,
            ..
        } => {
            let mut held = server_holders.lock().unwrap();
            assert_eq!(held[&role], expected);
            held.get_mut(&role).unwrap().push(member);
            Ok(Response::Recorded {
                event: locust_proto::id::EventId([5; 32]),
            })
        }
        Request::RoleTake {
            role,
            member,
            expected,
            ..
        } => {
            let mut held = server_holders.lock().unwrap();
            assert_eq!(held[&role], expected);
            held.get_mut(&role).unwrap().retain(|key| *key != member);
            Ok(Response::Recorded {
                event: locust_proto::id::EventId([6; 32]),
            })
        }
        request => panic!("unexpected {request:?}"),
    });
    let run = |role: &str| {
        let output = plain()
            .arg("--home")
            .arg(home.path())
            .args([
                "--owner",
                "role",
                "give",
                "--goal",
                &goal.to_string(),
                "--member",
                &member.to_string(),
                "--",
                role,
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    // A leading dash goes after `--`, so clap fills the role and shows no help.
    let give = run("--help");
    assert!(
        give.contains("Undo: locust --owner role take --goal 04040404 --member 03030303 -- --help"),
        "{give}"
    );
    let undo = give
        .lines()
        .find_map(|line| line.strip_prefix("Undo: "))
        .unwrap();
    let binary_dir = std::path::Path::new(env!("CARGO_BIN_EXE_locust"))
        .parent()
        .unwrap();
    let output = Command::new("sh")
        .args(["-c", undo])
        .env(
            "PATH",
            format!(
                "{}:{}",
                binary_dir.display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .env("LOCUST_HOME", home.path())
        .env_remove("LOCUST_CREDENTIAL")
        .env_remove("LOCUST_SESSION")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{undo}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(holders.lock().unwrap()["--help"], vec![host]);
    // Hidden characters never reach the terminal inside a command.
    let give = run(hidden);
    assert!(!give.contains('\u{202e}'), "{give}");
    assert!(!give.contains("Undo: locust"), "{give}");
    assert!(
        give.contains("Undo with role take --goal 04040404 --member 03030303 and the role's name; it holds hidden characters, so no line is printed for it."),
        "{give}"
    );
    handle.join().unwrap();
}

#[test]
fn a_plan_shows_the_name_and_defaults_to_the_enrolled_one() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let agent = PublicKey([2; 32]);
    let handle = server(home.path(), 2, move |frame| {
        assert_eq!(frame.request, Request::Status);
        Ok(status_agents(vec![AgentView {
            agent,
            name: "codex-maple-12345678".into(),
            author_only: false,
            revoked: false,
        }]))
    });
    for (extra, expected) in [
        (vec![], "codex-maple-12345678"),
        (vec!["--name", "Maple"], "Maple"),
    ] {
        let output = cli(home.path())
            .args([
                "--owner",
                "--agent",
                &agent.to_string(),
                "goal",
                "create",
                "--title",
                "Names",
                "--plan",
            ])
            .args(extra)
            .output()
            .unwrap();
        let envelope = envelope(&output, 0);
        assert_eq!(envelope["result"]["plan"]["name"], expected);
        assert_eq!(envelope["result"]["plan"]["formation"], "peer-review");
        assert!(envelope["result"]["plan"].get("roles").is_none());
    }
    handle.join().unwrap();
}

fn formation_event(_goal: GoalId, event: locust_proto::id::EventId, bytes: &[u8]) -> Response {
    use locust_proto::api::{EventDetail, EventView, Standing};
    use locust_proto::event::{Body, DefinitionRef, PayloadRef, RulesBinding};
    let hash = locust_proto::crypto::content_hash(bytes);
    let formation = serde_json::from_slice(bytes).unwrap();
    Response::Event(Box::new(EventDetail {
        view: EventView {
            event,
            position: Some(1),
            author: PublicKey([9; 32]),
            kind: "rules_bound".into(),
            at_ms: 0,
            standing: Standing::Effective,
            by_host: true,
            by_owner: false,
        },
        anchor: None,
        body: Body::RulesBound {
            expected: None,
            binding: RulesBinding {
                definition: DefinitionRef {
                    semantic: locust_proto::organization::semantic_hash(&formation)
                        .parse()
                        .unwrap(),
                    object: PayloadRef {
                        hash,
                        len: bytes.len() as u32,
                        key_epoch: 0,
                    },
                },
                inputs: Default::default(),
            },
        },
        payload: None,
        text: None,
        task: None,
        content: vec![],
    }))
}

#[test]
fn add_and_invite_carry_the_role_the_rules_count_on() {
    use locust_proto::id::EventId;
    for (formation, expected) in [
        ("review-panel", Some("reviewer")),
        ("peer-review", None),
        ("directed", None),
    ] {
        let home = scratch();
        write_secret(&home.path().join("owner.credential"), &[1; 32]);
        let goal = GoalId([4; 32]);
        let host = PublicKey([2; 32]);
        let worker = PublicKey([3; 32]);
        let rules = EventId([5; 32]);
        let formation = locust_proto::organization::presets()
            .into_iter()
            .find(|preset| preset.name == formation)
            .unwrap()
            .formation;
        let bytes = serde_json::to_vec(&formation).unwrap();
        let mut invites = 0;
        let handle = server(home.path(), 6, move |frame| match frame.request {
            Request::Status => Ok(status_agents(vec![AgentView {
                agent: worker,
                name: "worker".into(),
                author_only: false,
                revoked: false,
            }])),
            Request::GoalStatus { .. } => {
                let mut view = hosted_goal_status(goal, host, true);
                view.current_rules = Some(rules);
                Ok(Response::GoalStatus(view))
            }
            Request::GoalInvitations { .. } => Ok(Response::Invitations {
                invitations: vec![],
            }),
            Request::Event { .. } => Ok(formation_event(goal, rules, &bytes)),
            Request::BlobGet { .. } => Ok(Response::Blob {
                bytes: bytes.clone(),
            }),
            Request::GoalInvite { role, .. } => {
                assert_eq!(role.as_deref(), if invites == 0 { expected } else { None });
                invites += 1;
                Ok(Response::Invited {
                    ticket: locust_proto::invite::Ticket("fixture-ticket".into()),
                })
            }
            other => panic!("a plan wrote {other:?}"),
        });
        for command in ["add", "invite"] {
            for override_none in [false, true] {
                let mut command_line = cli(home.path());
                command_line.args([
                    "--owner",
                    "goal",
                    command,
                    "--goal",
                    &goal.to_string(),
                    "--plan",
                ]);
                if command == "add" {
                    command_line.args(["--agent", &worker.to_string()]);
                }
                if override_none {
                    command_line.arg("--no-role");
                }
                let output = command_line.output().unwrap();
                let envelope = envelope(&output, 0);
                let plan = envelope["result"]["plan"].clone();
                if command == "invite" {
                    let mut confirmed = cli(home.path());
                    confirmed.args([
                        "--owner",
                        "goal",
                        "invite",
                        "--goal",
                        &goal.to_string(),
                        "--confirm",
                        envelope["result"]["plan_id"].as_str().unwrap(),
                    ]);
                    if override_none {
                        confirmed.arg("--no-role");
                    }
                    let output = confirmed.output().unwrap();
                    assert!(
                        output.status.success(),
                        "{}",
                        String::from_utf8_lossy(&output.stdout)
                    );
                }
                assert_eq!(
                    plan["role"].as_str(),
                    if override_none { None } else { expected }
                );
            }
        }
        handle.join().unwrap();
    }
}

#[test]
fn rules_bind_moves_the_shared_files_to_the_new_rules_and_says_so() {
    use locust_proto::id::EventId;
    use locust_proto::organization::{
        Authority, CompletionRule, Formation, Selector, WorkspacePolicy,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let host = PublicKey([2; 32]);
    let rules = EventId([5; 32]);
    let epoch = EventId([6; 32]);
    let one_change = Arc::new(AtomicBool::new(false));
    let server_one_change = one_change.clone();
    let tree_on = Arc::new(AtomicBool::new(true));
    let server_tree_on = tree_on.clone();
    let old = Formation {
        workspace: Some(WorkspacePolicy {
            integrator: Authority::Participant {
                key: host.to_string(),
            },
            completion: CompletionRule::Contribution {
                by: Selector::Members,
            },
        }),
        ..Formation::default()
    };
    let bytes = serde_json::to_vec(&old).unwrap();
    let handle = server(home.path(), 6, move |frame| match frame.request {
        Request::GoalStatus { .. } => {
            let mut view = hosted_goal_status(goal, host, true);
            view.current_rules = Some(rules);
            if server_tree_on.load(Ordering::SeqCst) {
                view.workspace = Some(locust_proto::api::WorkspaceView {
                    epoch: Some(epoch),
                    checkpoint: None,
                    head: None,
                    enabled: true,
                    authority: locust_proto::api::WorkspaceAuthority::Ready,
                    content: None,
                });
            }
            Ok(Response::GoalStatus(view))
        }
        // The plan's printed commands cut the goal among the daemon's goals.
        Request::Status => Ok(status_agents(vec![])),
        Request::Event { .. } => Ok(formation_event(goal, rules, &bytes)),
        Request::BlobGet { .. } => Ok(Response::Blob {
            bytes: bytes.clone(),
        }),
        Request::WorkspaceProposals { .. } => {
            use locust_proto::api::{
                Standing, WorkspaceContent, WorkspaceProposalStatus, WorkspaceProposalView,
            };
            use locust_proto::event::{Context, Scope};
            let proposal = |tag| WorkspaceProposalView {
                proposal: EventId([tag; 32]),
                context: Context {
                    scope: Scope::Workspace,
                    round: epoch,
                },
                author: host,
                parent: Some(EventId([9; 32])),
                result_manifest: locust_proto::id::BlobHash([8; 32]),
                sources: vec![],
                source_authors: vec![],
                standing: Standing::Effective,
                usable_as_source: true,
                integrated_as: vec![],
                status: WorkspaceProposalStatus::AwaitingEvidence,
                approved: false,
                evidence: vec![],
                stale: false,
                content: WorkspaceContent::Complete { files: 1, bytes: 1 },
            };
            let mut proposals = vec![proposal(10), proposal(11)];
            if server_one_change.load(Ordering::SeqCst) {
                proposals.pop();
            }
            for (tag, standing) in [
                (12, Standing::Excluded),
                (13, Standing::Pending),
                (14, Standing::Disputed),
            ] {
                let mut item = proposal(tag);
                item.standing = standing;
                proposals.push(item);
            }
            let mut stale = proposal(15);
            stale.stale = true;
            proposals.push(stale);
            let mut integrated = proposal(16);
            integrated.integrated_as = vec![EventId([20; 32])];
            proposals.push(integrated);
            let mut old = proposal(17);
            old.context.round = EventId([21; 32]);
            proposals.push(old);
            let mut seed = proposal(18);
            seed.parent = None;
            proposals.push(seed);
            Ok(Response::WorkspaceProposals(proposals))
        }
        Request::RulesBind {
            expected,
            formation_json,
            ..
        } => {
            assert_eq!(expected, rules);
            let formation: Formation = serde_json::from_str(&formation_json).unwrap();
            if server_tree_on.load(Ordering::SeqCst) {
                let policy = formation.workspace.as_ref().unwrap();
                assert_eq!(
                    policy.integrator,
                    Authority::Participant {
                        key: host.to_string()
                    }
                );
                assert_eq!(policy.completion, formation.decisions.completion);
            } else {
                assert!(formation.workspace.is_none());
            }
            Ok(Response::Recorded {
                event: EventId([7; 32]),
            })
        }
        request => panic!("unexpected {request:?}"),
    });
    let run = |extra: &[&str]| {
        plain()
            .arg("--home")
            .arg(home.path())
            .args([
                "--owner",
                "rules",
                "bind",
                "--goal",
                &goal.to_string(),
                "--formation",
                "peer-review",
            ])
            .args(extra)
            .output()
            .unwrap()
    };
    let plan = run(&["--plan"]);
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    let text = String::from_utf8(plan.stdout).unwrap();
    assert!(
        text.contains("Shared files will follow these rules too."),
        "{text}"
    );
    assert!(
        text.contains("This replaces the rule you gave the shared files."),
        "{text}"
    );
    assert!(
        text.contains(
            "2 file changes that have not landed must be proposed again by their authors."
        ),
        "{text}"
    );
    assert!(text.contains("Share them again with locust --owner workspace init --goal 04040404 and the same seed options."), "{text}");
    let id = text
        .lines()
        .find_map(|line| line.strip_prefix("Plan id: "))
        .unwrap();
    let committed = run(&["--confirm", id]);
    assert!(
        committed.status.success(),
        "{}",
        String::from_utf8_lossy(&committed.stderr)
    );
    let committed = String::from_utf8(committed.stdout).unwrap();
    assert!(committed.contains("The shared files follow them too."));
    assert!(
        committed.contains(
            "2 file changes that have not landed must be proposed again by their authors."
        )
    );
    assert!(committed.contains("Share them again with locust --owner workspace init --goal"));
    let invalid = plain().arg("--home").arg(home.path()).args([
        "--owner", "rules", "bind", "--goal", &goal.to_string(), "--formation-json",
        r#"{"schema_version":2,"decisions":{"completion":{"kind":"declaration","by":{"kind":"task_creator"}}}}"#,
        "--plan",
    ]).output().unwrap();
    assert_eq!(invalid.status.code(), Some(6));
    assert!(String::from_utf8(invalid.stderr).unwrap().contains("These rules cannot apply to the shared files. Give the files a rule in the formation's workspace part."));
    one_change.store(true, Ordering::SeqCst);
    let singular = run(&["--plan"]);
    assert!(singular.status.success());
    let singular = String::from_utf8(singular.stdout).unwrap();
    assert!(
        singular
            .contains("1 file change that has not landed must be proposed again by its author.")
    );
    tree_on.store(false, Ordering::SeqCst);
    let plan = run(&["--plan"]);
    let text = String::from_utf8(plan.stdout).unwrap();
    assert!(!text.contains("Shared files will"), "{text}");
    let id = text
        .lines()
        .find_map(|line| line.strip_prefix("Plan id: "))
        .unwrap();
    assert!(run(&["--confirm", id]).status.success());
    handle.join().unwrap();
}

#[test]
fn invalid_stage_rules_are_explained_before_create_or_bind_has_a_plan() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let handle = server(home.path(), 2, |frame| {
        panic!(
            "invalid formation should need no daemon request: {:?}",
            frame.request
        )
    });
    let source = r#"{"schema_version":2,"flow":{"draft":{}},"decisions":{"completion":{"kind":"declaration","by":{"kind":"task_creator"}}}}"#;
    for args in [
        vec!["goal", "create", "--title", "Invalid stage"],
        vec!["rules", "bind", "--goal", &goal.to_string()],
    ] {
        let output = cli(home.path())
            .arg("--owner")
            .args(args)
            .args(["--formation-json", source, "--plan"])
            .output()
            .unwrap();
        let result = envelope(&output, 6);
        let message = result["error"]["message"].as_str().unwrap();
        assert!(message.contains("selector_scope"), "{message}");
        assert!(message.contains("task_creator"), "{message}");
        assert!(
            message.contains("Name members, a role or a participant"),
            "{message}"
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("review_required"));
    }
    handle.join().unwrap();
}

#[test]
fn disconnected_members_are_offered_reconnect_in_status_and_owner_commands() {
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let agent = PublicKey([2; 32]);
    let handle = server(home.path(), 4, move |frame| match frame.request {
        Request::Status => Ok(Response::Status(DaemonStatus {
            daemon_version: "stub".into(),
            endpoint: None,
            waiting: vec![],
            agents: vec![AgentView {
                agent,
                name: "worker".into(),
                revoked: true,
                author_only: false,
            }],
            goals: vec![GoalSummary {
                goal,
                title: Some("Demo".into()),
                member: agent,
                membership: Membership::Member,
                name: "agent".into(),
                host_name: None,
                invitations_open: 0,
                invitations_expire_ms: None,
                halted: None,
                abilities: Abilities {
                    name: "worker".into(),
                    ..abilities(goal, agent)
                },
            }],
        })),
        other => panic!("disconnected agent must not act: {other:?}"),
    });
    for selector in [vec![], vec!["--agent", "worker"]] {
        let output = cli(home.path())
            .arg("--owner")
            .args(selector)
            .args(["level", "--goal", &goal.to_string(), "ask"])
            .output()
            .unwrap();
        let result = envelope(&output, 2);
        let message = result["error"]["message"].as_str().unwrap();
        assert!(message.contains("worker is disconnected"), "{message}");
        assert!(
            message.contains("agent reconnect --agent worker"),
            "{message}"
        );
        assert!(!message.contains(&agent.to_string()), "{message}");
        assert!(!message.contains("none of your agents"), "{message}");
    }
    let output = cli(home.path())
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
        .unwrap();
    assert!(
        envelope(&output, 2)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("agent reconnect")
    );
    let output = plain()
        .arg("--home")
        .arg(home.path())
        .args(["--owner", "status"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.contains("\n  agent (worker) · disconnected\n      worker is disconnected."),
        "{text}"
    );
    assert!(!text.contains("\nworker is disconnected."), "{text}");
    assert!(text.contains("agent reconnect --agent worker"), "{text}");
    assert!(!text.contains(&agent.to_string()), "{text}");
    handle.join().unwrap();
}

#[test]
fn a_leads_undo_restores_its_previous_holder_and_role_duties_are_explained() {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    let home = scratch();
    write_secret(&home.path().join("owner.credential"), &[1; 32]);
    let goal = GoalId([4; 32]);
    let host = PublicKey([2; 32]);
    let member = PublicKey([3; 32]);
    let previous = PublicKey([4; 32]);
    let rules = locust_proto::id::EventId([5; 32]);
    let mut formation = locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == "review-panel")
        .unwrap()
        .formation;
    formation.roles.insert("unused".into(), Default::default());
    let bytes = serde_json::to_vec(&formation).unwrap();
    let holders = Arc::new(Mutex::new(BTreeMap::from([
        ("lead".to_owned(), vec![previous]),
        ("reviewer".to_owned(), vec![host]),
        ("unused".to_owned(), vec![host]),
    ])));
    let server_holders = holders.clone();
    let handle = server(home.path(), 4, move |frame| match frame.request {
        Request::Status => Ok(status(vec![GoalSummary {
            goal,
            title: Some("Demo".into()),
            member: host,
            membership: Membership::Member,
            name: "Harbor".into(),
            host_name: Some("Harbor".into()),
            invitations_open: 0,
            invitations_expire_ms: None,
            halted: None,
            abilities: abilities(goal, host),
        }])),
        Request::GoalStatus { .. } => {
            let mut view = hosted_goal_status(goal, host, true);
            view.current_rules = Some(rules);
            view.roles = server_holders.lock().unwrap().clone();
            view.deciding = ["lead".into()].into();
            for (key, name) in [(member, "Maple"), (previous, "Juniper")] {
                view.members.push(locust_proto::api::MemberView {
                    member: key,
                    name: name.into(),
                    endpoint: locust_proto::id::EndpointId([3; 32]),
                    local: false,
                    admitted: 0,
                });
            }
            Ok(Response::GoalStatus(view))
        }
        Request::Event { .. } => Ok(formation_event(goal, rules, &bytes)),
        Request::BlobGet { .. } => Ok(Response::Blob {
            bytes: bytes.clone(),
        }),
        Request::RoleGive {
            role,
            member,
            expected,
            ..
        } => {
            let mut holders = server_holders.lock().unwrap();
            let held = holders.get_mut(&role).unwrap();
            assert_eq!(*held, expected);
            if role == "lead" {
                held.clear();
            }
            held.push(member);
            held.sort();
            held.dedup();
            Ok(Response::Recorded {
                event: locust_proto::id::EventId([6; 32]),
            })
        }
        request => panic!("unexpected {request:?}"),
    });
    let run = |args: &[&str]| {
        let out = plain()
            .arg("--home")
            .arg(home.path())
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    let give = |role: &str| {
        run(&[
            "--owner",
            "role",
            "give",
            "--goal",
            &goal.to_string(),
            "--member",
            &member.to_string(),
            role,
        ])
    };
    let lead = give("lead");
    assert!(
        lead.contains(
            "lead is not in the current rules. It still applies to work under earlier rules."
        ),
        "{lead}"
    );
    let undo = lead
        .lines()
        .find_map(|line| line.strip_prefix("Undo: "))
        .unwrap();
    assert_eq!(
        undo,
        "locust --owner role give --goal 04040404 --member 04040404 lead"
    );
    run(&undo
        .strip_prefix("locust ")
        .unwrap()
        .split_whitespace()
        .collect::<Vec<_>>());
    assert_eq!(holders.lock().unwrap()["lead"], [previous]);
    let reviewer = give("reviewer");
    assert!(
        reviewer.contains("A reviewer here: approves results."),
        "{reviewer}"
    );
    let unused = give("unused");
    assert!(
        unused.contains("No rule in the current rules names unused, so it changes nothing yet."),
        "{unused}"
    );
    handle.join().unwrap();
}
