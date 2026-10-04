//! Binary workspace contracts against an isolated typed Unix API fixture.
//! This tests CLI request/filesystem behavior, not daemon or peer replication.
use locust_proto::API_VERSION;
use locust_proto::api::*;
use locust_proto::codec;
use locust_proto::crypto::content_hash;
use locust_proto::event::{Body, Context, Scope, TaskId};
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey};
use locust_proto::limits::{MAX_HELLO_FRAME_BYTES, MAX_LOCAL_FRAME_BYTES};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::Command;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;

const GOAL: GoalId = GoalId([0x31; 32]);
const PRINCIPAL: PublicKey = PublicKey([0x42; 32]);
const TASK: TaskId = TaskId::Authored(EventId([0x52; 32]));
const ATTEMPT: EventId = EventId([0x53; 32]);
const CONTEXT: Context = Context {
    scope: Scope::Task(TASK),
    round: EventId([0x52; 32]),
};
const RESULT: EventId = EventId([0x64; 32]);

#[derive(Default)]
struct State {
    objects: HashMap<BlobHash, Vec<u8>>,
    binding: WorkspaceBinding,
    selected_patch: Option<BlobHash>,
    submitted: Option<Body>,
    requests: Vec<RequestFrame>,
    reject_workspace_set: bool,
}
impl State {
    fn respond(&mut self, frame: RequestFrame) -> Result<Response, ApiError> {
        self.requests.push(frame.clone());
        if !matches!(frame.request, Request::Status) {
            assert_eq!(frame.on_behalf, Some(PRINCIPAL));
        }
        match frame.request {
            Request::Status => Ok(Response::Status(DaemonStatus {
                daemon_version: "fixture".into(),
                endpoint: None,
                agents: vec![AgentView {
                    agent: PRINCIPAL,
                    name: "worker".into(),
                    grants: Grants::default(),
                    revoked: false,
                }],
                goals: vec![GoalSummary {
                    goal: GOAL,
                    title: Some("workspace".into()),
                    member: PRINCIPAL,
                    membership: Membership::Member,
                    halted: None,
                }],
            })),
            Request::GoalStatus { goal } => {
                assert_eq!(goal, GOAL);
                Ok(Response::GoalStatus(GoalStatus {
                    goal,
                    title: Some("workspace".into()),
                    administrator: PRINCIPAL,
                    governance_head: None,
                    current_rules: None,
                    scope_halts: vec![],
                    members: vec![],
                    halted: None,
                    workspace: Some(self.binding.clone()),
                    grants: GoalGrants::default(),
                    peers: vec![],
                }))
            }
            Request::BlobPut { goal, bytes } => {
                assert_eq!(goal, GOAL);
                let hash = content_hash(&bytes);
                self.objects.insert(hash, bytes);
                Ok(Response::BlobStored { hash })
            }
            Request::BlobGet { goal, hash } => {
                assert_eq!(goal, GOAL);
                self.objects
                    .get(&hash)
                    .cloned()
                    .map(|bytes| Response::Blob { bytes })
                    .ok_or_else(|| {
                        ApiError::new(
                            ErrorCode::Unavailable,
                            "object is unavailable from isolated fixture",
                        )
                    })
            }
            Request::WorkspaceSet { goal, binding } => {
                assert_eq!(goal, GOAL);
                if self.reject_workspace_set {
                    return Err(ApiError::new(
                        ErrorCode::Unavailable,
                        "binding storage unavailable in isolated fixture",
                    ));
                }
                self.binding = binding;
                Ok(Response::Done)
            }
            Request::ContributionPublish {
                goal,
                task,
                attempt,
                generation,
                base,
                patch,
                artifacts,
                sources,
                ..
            } => {
                assert_eq!(goal, GOAL);
                assert_eq!(task, Some(TASK));
                assert_eq!(attempt, Some(ATTEMPT));
                assert_eq!(generation, Some(7));
                let contribution = locust_proto::contribution::Contribution::decode(
                    self.objects.get(&patch.unwrap()).unwrap(),
                )
                .unwrap();
                assert_eq!(base, Some(contribution.base));
                assert_eq!(artifacts, [contribution.head]);
                self.submitted = Some(Body::ContributionPublished {
                    context: CONTEXT,
                    attempt,
                    base,
                    patch,
                    sources,
                    artifacts,
                });
                Ok(Response::Recorded { event: RESULT })
            }
            Request::Events { goal, after, .. } => {
                assert_eq!(goal, GOAL);
                Ok(Response::Events(if after.is_none() {
                    vec![EventView {
                        position: Some(1),
                        event: RESULT,
                        author: PRINCIPAL,
                        kind: "contribution_published".into(),
                        at_ms: 1,
                        standing: Standing::Effective,
                    }]
                } else {
                    vec![]
                }))
            }
            Request::Event { goal, event } => {
                assert_eq!(goal, GOAL);
                assert_eq!(event, RESULT);
                Ok(Response::Event(Box::new(EventDetail {
                    view: EventView {
                        position: Some(1),
                        event,
                        author: PRINCIPAL,
                        kind: "contribution_published".into(),
                        at_ms: 1,
                        standing: Standing::Effective,
                    },
                    anchor: None,
                    body: self.submitted.clone().unwrap(),
                    payload: None,
                    text: Some("result".into()),
                    task: None,
                    content: vec![],
                })))
            }
            Request::Pending { goal } => {
                assert_eq!(goal, GOAL);
                Ok(Response::Pending(PendingWork {
                    claimed: vec![Claim {
                        goal,
                        task: TASK,
                        attempt: ATTEMPT,
                        instance: SessionSecret([1; 32]).instance(),
                        generation: 7,
                    }],
                    ..Default::default()
                }))
            }
            Request::Contributions { goal, task } => {
                assert_eq!(goal, GOAL);
                assert_eq!(task, None);
                Ok(Response::Contributions(
                    self.submitted
                        .as_ref()
                        .map(|body| {
                            let Body::ContributionPublished {
                                base,
                                patch,
                                artifacts,
                                sources,
                                ..
                            } = body
                            else {
                                unreachable!()
                            };
                            ContributionView {
                                contribution: RESULT,
                                author: PRINCIPAL,
                                context: CONTEXT,
                                attempt: Some(ATTEMPT),
                                approved: true,
                                selected: self.selected_patch == *patch,
                                evidence: vec![],
                                base: *base,
                                patch: *patch,
                                sources: sources.clone(),
                                artifacts: artifacts.clone(),
                                text: Some("result".into()),
                            }
                        })
                        .into_iter()
                        .collect(),
                ))
            }
            Request::ScopeSelect {
                goal,
                subject,
                expected,
            } => {
                assert_eq!(goal, GOAL);
                assert_eq!(subject, RESULT);
                assert_eq!(expected, None);
                let Some(Body::ContributionPublished { patch, .. }) = self.submitted.as_ref()
                else {
                    unreachable!()
                };
                self.selected_patch = *patch;
                Ok(Response::Recorded {
                    event: EventId([0x75; 32]),
                })
            }
            other => panic!("unexpected request: {other:?}"),
        }
    }
}
struct Fixture {
    home: tempfile::TempDir,
    state: Arc<Mutex<State>>,
    stopped: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Fixture {
    fn new() -> Self {
        let home = tempfile::Builder::new()
            .prefix("lc-ws-")
            .tempdir_in("/tmp")
            .unwrap();
        fs::set_permissions(home.path(), fs::Permissions::from_mode(0o700)).unwrap();
        for name in ["owner.credential", "session"] {
            fs::write(home.path().join(name), [1; 32]).unwrap();
            fs::set_permissions(home.path().join(name), fs::Permissions::from_mode(0o600)).unwrap();
        }
        let listener = UnixListener::bind(home.path().join("daemon.sock")).unwrap();
        let state = Arc::new(Mutex::new(State::default()));
        let stopped = Arc::new(AtomicBool::new(false));
        let server_state = state.clone();
        let server_stopped = stopped.clone();
        let handle = thread::spawn(move || {
            while !server_stopped.load(Ordering::SeqCst) {
                let (mut stream, _) = listener.accept().unwrap();
                if server_stopped.load(Ordering::SeqCst) {
                    break;
                }
                let frame = codec::read_frame(&mut stream, MAX_HELLO_FRAME_BYTES)
                    .unwrap()
                    .unwrap();
                let hello = ClientHello::decode(&frame).unwrap();
                assert_eq!(hello.credential, Credential([1; 32]));
                let mut out = Vec::new();
                codec::encode_frame(
                    &ServerHello::Welcome {
                        api_version: API_VERSION,
                        daemon_version: "fixture".into(),
                        caller: Caller::Owner,
                        max_blob_bytes: locust_proto::limits::MAX_BLOB_BYTES as u64,
                    },
                    &mut out,
                )
                .unwrap();
                stream.write_all(&out).unwrap();
                while let Some(frame) =
                    codec::read_frame(&mut stream, MAX_LOCAL_FRAME_BYTES).unwrap()
                {
                    let request: RequestFrame = codec::decode(&frame).unwrap();
                    let id = request.id;
                    let result = server_state.lock().unwrap().respond(request);
                    out.clear();
                    codec::encode_frame(&ResponseFrame { id, result }, &mut out).unwrap();
                    stream.write_all(&out).unwrap();
                }
            }
        });
        Self {
            home,
            state,
            stopped,
            handle: Some(handle),
        }
    }
    fn cli(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_locust"));
        cmd.env_remove("LOCUST_HOME")
            .env_remove("LOCUST_CREDENTIAL")
            .env_remove("LOCUST_SESSION");
        cmd.arg("--home")
            .arg(self.home.path())
            .args(["--json", "--owner", "--as", "worker"]);
        cmd
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        let _ = UnixStream::connect(self.home.path().join("daemon.sock"));
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}
fn output(mut command: Command, code: i32) -> Value {
    let out = command.output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(code),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty());
    serde_json::from_slice(&out.stdout).unwrap()
}
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(root)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .args([
            "-c",
            "user.name=Workspace Test",
            "-c",
            "user.email=test@locust.invalid",
            "-c",
            "commit.gpgSign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}

#[test]
fn snapshot_patch_selection_and_integration_are_explicit_and_bound() {
    let fixture = Fixture::new();
    let files = tempfile::tempdir().unwrap();
    let repo = files.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    fs::write(repo.join("file"), b"base\n").unwrap();
    git(&repo, &["add", "file"]);
    git(&repo, &["commit", "-qm", "base"]);
    let commit = git(&repo, &["rev-parse", "HEAD"]);
    fs::write(repo.join("file"), b"uncommitted private\n").unwrap();
    let mut cmd = fixture.cli();
    cmd.args([
        "workspace",
        "export",
        "--goal",
        "31313131",
        "--commit",
        &commit,
        "--root",
    ])
    .arg(&repo);
    let exported = output(cmd, 0);
    let base = exported["result"]["manifest"].as_str().unwrap().to_string();
    let destination = files.path().join("destination");
    let mut cmd = fixture.cli();
    cmd.args([
        "workspace",
        "materialize",
        "--goal",
        "31313131",
        "--manifest",
        &base,
        "--destination",
    ])
    .arg(&destination);
    let materialized = output(cmd, 0);
    assert_eq!(materialized["result"]["integrated"], true);
    assert_eq!(fs::read(destination.join("file")).unwrap(), b"base\n");
    fs::write(destination.join("file"), b"result\n").unwrap();
    let stored = fixture.state.lock().unwrap().objects.len();
    let mut denied = fixture.cli();
    denied
        .args([
            "patch", "create", "--goal", "31313131", "--base", &base, "--root",
        ])
        .arg(files.path())
        .args(["--path", "unbound"]);
    assert_eq!(output(denied, 3)["error"]["code"], "denied");
    assert_eq!(fixture.state.lock().unwrap().objects.len(), stored);
    let mut cmd = fixture.cli();
    cmd.args([
        "patch", "create", "--goal", "31313131", "--base", &base, "--root",
    ])
    .arg(&destination)
    .args(["--path", "file"]);
    let created = output(cmd, 0);
    let patch = created["result"]["contribution_id"]
        .as_str()
        .unwrap()
        .to_string();
    let head = created["result"]["contribution"]["head"]
        .as_str()
        .unwrap()
        .to_string();
    let mut cmd = fixture.cli();
    cmd.args(["patch", "review", "--goal", "31313131", "--patch", &patch]);
    let reviewed = output(cmd, 0);
    assert!(
        reviewed["result"]["changes"][0]["unified_diff"]
            .as_str()
            .unwrap()
            .contains("+result\n")
    );
    let mut no_session = fixture.cli();
    no_session.args([
        "patch",
        "submit",
        "--goal",
        "31313131",
        "--patch",
        &patch,
        "--attempt",
        &ATTEMPT.to_string(),
        "--generation",
        "7",
        "finished",
    ]);
    assert_eq!(output(no_session, 2)["error"]["code"], "invalid");
    assert!(fixture.state.lock().unwrap().submitted.is_none());
    let mut cmd = fixture.cli();
    cmd.arg("--session")
        .arg(fixture.home.path().join("session"))
        .args([
            "patch",
            "submit",
            "--goal",
            "31313131",
            "--patch",
            &patch,
            "--attempt",
            &ATTEMPT.to_string(),
            "--generation",
            "7",
            "--source",
            &EventId([0x61; 32]).to_string(),
            "--source",
            &EventId([0x62; 32]).to_string(),
            "finished",
        ]);
    output(cmd, 0);
    assert!(fixture.state.lock().unwrap().selected_patch.is_none());
    let mut cmd = fixture.cli();
    cmd.args([
        "patch",
        "review",
        "--goal",
        "workspace",
        "--subject",
        "64646464",
    ]);
    let reviewed_subject = output(cmd, 0);
    assert_eq!(reviewed_subject["result"], reviewed["result"]);
    assert!(fixture.state.lock().unwrap().selected_patch.is_none());
    let submitted = fixture.state.lock().unwrap().submitted.clone().unwrap();
    let Body::ContributionPublished { sources, .. } = &submitted else {
        panic!()
    };
    assert_eq!(sources, &[EventId([0x61; 32]), EventId([0x62; 32])]);
    fixture.state.lock().unwrap().submitted = Some(Body::AttemptReported {
        attempt: ATTEMPT,
        status: locust_proto::event::AttemptStatus::Completed,
    });
    let mut wrong_kind = fixture.cli();
    wrong_kind.args([
        "patch",
        "review",
        "--goal",
        "workspace",
        "--subject",
        "64646464",
    ]);
    assert!(
        output(wrong_kind, 6)["error"]["message"]
            .as_str()
            .unwrap()
            .contains("signed patch and base")
    );
    fixture.state.lock().unwrap().submitted = Some(submitted.clone());

    if let Some(Body::ContributionPublished { base, .. }) =
        &mut fixture.state.lock().unwrap().submitted
    {
        *base = Some(BlobHash([0x99; 32]));
    }
    let mut review = fixture.cli();
    review.args([
        "patch",
        "review",
        "--goal",
        "workspace",
        "--subject",
        "64646464",
    ]);
    assert_eq!(output(review, 7)["error"]["code"], "conflict");
    let mut cmd = fixture.cli();
    cmd.args([
        "patch",
        "select",
        "--goal",
        "31313131",
        "--subject",
        "64646464",
    ]);
    assert_eq!(output(cmd, 7)["error"]["code"], "conflict");
    assert!(fixture.state.lock().unwrap().selected_patch.is_none());
    fixture.state.lock().unwrap().submitted = Some(submitted);
    let mut cmd = fixture.cli();
    cmd.args([
        "patch",
        "select",
        "--goal",
        "31313131",
        "--subject",
        "64646464",
    ]);
    output(cmd, 0);
    assert_eq!(
        fixture
            .state
            .lock()
            .unwrap()
            .selected_patch
            .unwrap()
            .to_string(),
        patch
    );
    assert_eq!(
        fixture
            .state
            .lock()
            .unwrap()
            .binding
            .integrated
            .unwrap()
            .to_string(),
        base
    );
    fs::write(destination.join("file"), b"base\n").unwrap();
    fs::write(destination.join("unrelated"), b"WIP").unwrap();
    let selected = fixture.state.lock().unwrap().selected_patch;
    fixture.state.lock().unwrap().selected_patch = Some(BlobHash([0x99; 32]));
    let mut cmd = fixture.cli();
    cmd.args([
        "patch",
        "apply",
        "--subject",
        "64646464",
        "--goal",
        "31313131",
        "--root",
    ])
    .arg(&destination);
    assert_eq!(output(cmd, 7)["error"]["code"], "conflict");
    assert_eq!(
        fixture
            .state
            .lock()
            .unwrap()
            .binding
            .integrated
            .unwrap()
            .to_string(),
        base
    );
    assert_eq!(fs::read(destination.join("file")).unwrap(), b"base\n");
    fixture.state.lock().unwrap().selected_patch = selected;
    let saved_submission = fixture.state.lock().unwrap().submitted.clone();
    if let Some(Body::ContributionPublished { base, .. }) =
        &mut fixture.state.lock().unwrap().submitted
    {
        *base = Some(BlobHash([0x99; 32]));
    }
    let mut cmd = fixture.cli();
    cmd.args([
        "patch",
        "apply",
        "--subject",
        "64646464",
        "--goal",
        "31313131",
        "--root",
    ])
    .arg(&destination);
    assert_eq!(output(cmd, 6)["error"]["code"], "invalid");
    fixture.state.lock().unwrap().submitted = saved_submission;
    assert_eq!(fs::read(destination.join("file")).unwrap(), b"base\n");
    let mut cmd = fixture.cli();
    cmd.args([
        "patch",
        "apply",
        "--subject",
        "64646464",
        "--goal",
        "31313131",
        "--root",
    ])
    .arg(&destination);
    output(cmd, 0);
    assert_eq!(fs::read(destination.join("file")).unwrap(), b"result\n");
    assert_eq!(fs::read(destination.join("unrelated")).unwrap(), b"WIP");
    assert_eq!(
        fs::read(repo.join("file")).unwrap(),
        b"uncommitted private\n"
    );
    assert_eq!(
        fixture
            .state
            .lock()
            .unwrap()
            .binding
            .integrated
            .unwrap()
            .to_string(),
        head
    );
    assert!(
        fixture
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .any(|f| matches!(f.request, Request::Status) && f.on_behalf == Some(PRINCIPAL))
    );
}

#[test]
fn unavailable_blob_preserves_api_error_code_and_message() {
    let fixture = Fixture::new();
    let unknown = BlobHash([0xab; 32]).to_string();
    let mut cmd = fixture.cli();
    cmd.args(["patch", "review", "--goal", "31313131", "--patch", &unknown]);
    let error = output(cmd, 8);
    assert_eq!(error["error"]["code"], "unavailable");
    assert_eq!(
        error["error"]["message"],
        "object is unavailable from isolated fixture"
    );
}

#[test]
fn applied_files_survive_binding_failure_and_exact_retry_recovers_integration() {
    use locust_proto::contribution::{Change, Contribution};
    use locust_proto::manifest::{Entry, Manifest};
    let fixture = Fixture::new();
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("changed"), b"before\n").unwrap();
    fs::write(root.path().join("unrelated"), b"local WIP").unwrap();
    let head = {
        let mut state = fixture.state.lock().unwrap();
        let mut put = |bytes: &[u8]| {
            let hash = content_hash(bytes);
            state.objects.insert(hash, bytes.to_vec());
            hash
        };
        let before = Entry {
            path: "changed".into(),
            size: 7,
            executable: false,
            content: put(b"before\n"),
        };
        let after = Entry {
            path: "changed".into(),
            size: 6,
            executable: false,
            content: put(b"after\n"),
        };
        let base = put(&Manifest {
            entries: vec![before.clone()],
        }
        .encode()
        .unwrap());
        let head = put(&Manifest {
            entries: vec![after.clone()],
        }
        .encode()
        .unwrap());
        let patch = put(&Contribution {
            version: 1,
            base,
            head,
            changes: vec![Change {
                path: "changed".into(),
                before: Some(before),
                after: Some(after),
            }],
        }
        .encode()
        .unwrap());
        state.selected_patch = Some(patch);
        state.submitted = Some(Body::ContributionPublished {
            context: CONTEXT,
            attempt: Some(ATTEMPT),
            base: Some(base),
            patch: Some(patch),
            sources: Vec::new(),
            artifacts: vec![head],
        });
        state.binding.destination = Some(root.path().to_str().unwrap().into());
        state.reject_workspace_set = true;
        head
    };
    let apply = || {
        let mut cmd = fixture.cli();
        cmd.args([
            "patch",
            "apply",
            "--subject",
            &RESULT.to_string(),
            "--goal",
            "31313131",
            "--root",
        ])
        .arg(root.path());
        cmd
    };
    let failure = output(apply(), 8);
    assert_eq!(failure["error"]["code"], "unavailable");
    let message = failure["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("files applied; integration binding was not recorded"),
        "{message}"
    );
    assert!(
        message.contains("binding storage unavailable in isolated fixture"),
        "{message}"
    );
    assert_eq!(fs::read(root.path().join("changed")).unwrap(), b"after\n");
    assert_eq!(
        fs::read(root.path().join("unrelated")).unwrap(),
        b"local WIP"
    );
    assert!(fixture.state.lock().unwrap().binding.integrated.is_none());
    fs::write(
        root.path().join("unrelated"),
        b"newer local WIP after failed binding",
    )
    .unwrap();
    fixture.state.lock().unwrap().reject_workspace_set = false;
    let retry = output(apply(), 0);
    assert_eq!(retry["result"]["applied"], serde_json::json!([]));
    assert_eq!(
        retry["result"]["already_applied"],
        serde_json::json!(["changed"])
    );
    assert_eq!(fixture.state.lock().unwrap().binding.integrated, Some(head));
    assert_eq!(fs::read(root.path().join("changed")).unwrap(), b"after\n");
    assert_eq!(
        fs::read(root.path().join("unrelated")).unwrap(),
        b"newer local WIP after failed binding"
    );
}
