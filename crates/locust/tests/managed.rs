//! Production CLI + MCP bridge against a typed isolated daemon fixture.
//! Native clients here are synthetic executables, not client qualification.
use locust_proto::{
    API_VERSION,
    api::*,
    codec,
    id::{EventId, GoalId, PublicKey},
    limits::{MAX_HELLO_FRAME_BYTES, MAX_LOCAL_FRAME_BYTES},
};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};
const KEY: PublicKey = PublicKey([2; 32]);
const GOAL: GoalId = GoalId([3; 32]);
const TASK: EventId = EventId([4; 32]);
const ASSIGNMENT: EventId = EventId([5; 32]);
const SESSION: SessionSecret = SessionSecret([6; 32]);
#[derive(Default)]
struct State {
    record: Option<SessionRecord>,
    reports: Vec<SessionRecord>,
    requests: Vec<Request>,
    reject_report: bool,
    reject_started: bool,
    claims: Vec<Claim>,
    wrong_assignee: bool,
    race_pending: bool,
    race_observed: bool,
}
impl State {
    fn answer(&mut self, request: Request) -> Result<Response, ApiError> {
        self.requests.push(request.clone());
        match request {
            Request::Status => Ok(Response::Status(DaemonStatus {
                daemon_version: "fixture".into(),
                endpoint: None,
                agents: vec![AgentView {
                    agent: KEY,
                    name: "worker".into(),
                    grants: Grants::default(),
                    revoked: false,
                }],
                goals: vec![GoalSummary {
                    goal: GOAL,
                    title: None,
                    member: KEY,
                    membership: Membership::Member,
                    halted: None,
                }],
            })),
            Request::Session { instance } => {
                assert!(instance.is_none());
                self.record
                    .clone()
                    .map(|record| {
                        Response::Session(SessionView {
                            instance: SESSION.instance(),
                            principal: KEY,
                            record,
                            updated_ms: 1,
                            attached: true,
                            claims: self.claims.clone(),
                        })
                    })
                    .ok_or(ApiError::new(ErrorCode::NotFound, "no record"))
            }
            Request::SessionReport { record } => {
                if self.reject_started && record.state == SessionState::Started {
                    self.reject_started = false;
                    return Err(ApiError::new(
                        ErrorCode::Unavailable,
                        "injected started report failure",
                    ));
                }
                if self.reject_report {
                    return Err(ApiError::new(
                        ErrorCode::Unavailable,
                        "injected report failure",
                    ));
                }
                self.reports.push(record.clone());
                self.record = Some(record);
                Ok(Response::Done)
            }
            Request::Pending { goal } => {
                assert_eq!(goal, GOAL);
                if self.race_pending
                    && self
                        .record
                        .as_ref()
                        .is_some_and(|r| r.client_session.is_some())
                {
                    self.race_pending = false;
                    self.race_observed = true;
                    self.claims = vec![Claim {
                        goal: GOAL,
                        task: TASK,
                        assignment: ASSIGNMENT,
                        instance: SESSION.instance(),
                        generation: 1,
                    }];
                }
                Ok(Response::Pending(PendingWork {
                    revision: self.reports.len() as u64 + 1,
                    claimed: self.claims.clone(),
                    to_acknowledge: vec![CancelItem {
                        task: TASK,
                        assignment: ASSIGNMENT,
                        cancel: EventId([7; 32]),
                        generation: self.claims.first().map(|c| c.generation),
                    }],
                    ..Default::default()
                }))
            }
            Request::Board { goal } => {
                assert_eq!(goal, GOAL);
                Ok(Response::Board(vec![TaskView {
                    task: TASK,
                    state: TaskState::Assigned,
                    title: None,
                    proposer: KEY,
                    assignee: Some(if self.wrong_assignee {
                        PublicKey([9; 32])
                    } else {
                        KEY
                    }),
                    assignment: Some(ASSIGNMENT),
                    attempt: 1,
                    result: None,
                    applied: false,
                }]))
            }
            other => panic!("unexpected managed request {other:?}"),
        }
    }
}
struct Fixture {
    dir: tempfile::TempDir,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    server: Option<thread::JoinHandle<()>>,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("lc-managed-")
            .tempdir_in("/tmp")
            .unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        for (name, bytes) in [("agent", [1; 32]), ("session", SESSION.0)] {
            fs::write(dir.path().join(name), bytes).unwrap();
            fs::set_permissions(dir.path().join(name), fs::Permissions::from_mode(0o600)).unwrap();
        }
        let state = Arc::new(Mutex::new(State::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let listener = UnixListener::bind(dir.path().join("daemon.sock")).unwrap();
        let state2 = state.clone();
        let stop2 = stop.clone();
        let server = thread::spawn(move || {
            let mut connections = Vec::new();
            while !stop2.load(Ordering::SeqCst) {
                let (mut stream, _) = listener.accept().unwrap();
                if stop2.load(Ordering::SeqCst) {
                    break;
                }
                let state = state2.clone();
                connections.push(thread::spawn(move || {
                    let frame = codec::read_frame(&mut stream, MAX_HELLO_FRAME_BYTES)
                        .unwrap()
                        .unwrap();
                    let hello = ClientHello::decode(&frame).unwrap();
                    assert_eq!(hello.credential, Credential([1; 32]));
                    assert_eq!(hello.session, Some(SESSION));
                    let mut out = Vec::new();
                    codec::encode_frame(
                        &ServerHello::Welcome {
                            api_version: API_VERSION,
                            daemon_version: "fixture".into(),
                            caller: Caller::Agent(KEY),
                            max_blob_bytes: locust_proto::limits::MAX_BLOB_BYTES as u64,
                        },
                        &mut out,
                    )
                    .unwrap();
                    stream.write_all(&out).unwrap();
                    while let Ok(Some(frame)) =
                        codec::read_frame(&mut stream, MAX_LOCAL_FRAME_BYTES)
                    {
                        let frame: RequestFrame = codec::decode(&frame).unwrap();
                        assert_eq!(frame.on_behalf, None);
                        let result = state.lock().unwrap().answer(frame.request);
                        out.clear();
                        codec::encode_frame(
                            &ResponseFrame {
                                id: frame.id,
                                result,
                            },
                            &mut out,
                        )
                        .unwrap();
                        if stream.write_all(&out).is_err() {
                            break;
                        }
                    }
                }));
            }
            for connection in connections {
                connection.join().unwrap();
            }
        });
        Self {
            dir,
            state,
            stop,
            server: Some(server),
        }
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_locust"));
        command
            .args(["--json", "--home"])
            .arg(self.dir.path())
            .arg("--credential")
            .arg(self.dir.path().join("agent"))
            .arg("--session")
            .arg(self.dir.path().join("session"));
        command
    }
    fn setup(&self, body: &str) -> (PathBuf, PathBuf, PathBuf) {
        let executable = self.dir.path().join("client.py");
        fs::write(&executable, format!("#!/usr/bin/env python3\n{body}\n")).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        let workspace = self.dir.path().join("workspace");
        let profile = self.dir.path().join("profile");
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(profile.join(".factory")).unwrap();
        fs::write(
            profile.join(".factory/mcp.json"),
            b"{\"unrelated\":true,\"mcpServers\":{\"other\":{\"command\":\"other\"}}}\n",
        )
        .unwrap();
        (executable, workspace, profile)
    }
    fn launch(&self, paths: &(PathBuf, PathBuf, PathBuf)) -> Command {
        let mut command = self.command();
        command
            .args(["client", "run", "--client", "factory-droid", "--executable"])
            .arg(&paths.0)
            .arg("--workspace")
            .arg(&paths.1)
            .arg("--profile")
            .arg(&paths.2)
            .args([
                "--client-version",
                "synthetic-fixture",
                "--prompt",
                "PRIVATE PROMPT",
                "--goal",
            ])
            .arg(GOAL.to_string())
            .arg("--assignment")
            .arg(ASSIGNMENT.to_string());
        command
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = UnixStream::connect(self.dir.path().join("daemon.sock"));
        self.server.take().unwrap().join().unwrap();
    }
}
fn result(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "not one JSON envelope: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}
const READY_CLIENT: &str = r#"
import json,os,subprocess,sys,time
assert not any(k.startswith('LOCUST_') for k in os.environ)
profile=os.environ['HOME']
config=json.load(open(profile+'/.factory/mcp.json'))
assert config['unrelated'] is True and 'other' in config['mcpServers']
server=config['mcpServers']['locust']
print(json.dumps({'type':'system','subtype':'init','session_id':'native-fixture'}),flush=True)
env=dict(os.environ);env.update(server['env'])
bridge=subprocess.Popen([server['command'],*server['args']],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env)
for i,method in [(1,'initialize'),(2,'tools/list')]:
    bridge.stdin.write((json.dumps({'jsonrpc':'2.0','id':i,'method':method,'params':{'protocolVersion':'2025-03-26','capabilities':{},'clientInfo':{'name':'synthetic-fixture','version':'1'}} if method=='initialize' else {}})+'\n').encode());bridge.stdin.flush()
    response=json.loads(bridge.stdout.readline());assert response['id']==i and 'error' not in response
    if method=='initialize':
        bridge.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized"}\n');bridge.stdin.flush()
print(json.dumps({'type':'fixture_receipt_written'}),flush=True)
time.sleep(.2)
bridge.stdin.close();assert bridge.wait(timeout=5)==0
"#;
#[test]
fn launch_readiness_pending_and_exact_resume_preserve_profile_and_cancellation() {
    let fixture = Fixture::new();
    let paths = fixture.setup(READY_CLIENT);
    let config = paths.2.join(".factory/mcp.json");
    let baseline = fs::read(&config).unwrap();
    let output = fixture.launch(&paths).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let value = result(&output);
    assert_eq!(value["result"]["native_output"], "stderr");
    assert_eq!(value["result"]["session"]["state"], "exited");
    assert_eq!(fs::read(&config).unwrap(), baseline);
    let state = fixture.state.lock().unwrap();
    let states: Vec<_> = state.reports.iter().map(|r| r.state).collect();
    assert_eq!(states.first(), Some(&SessionState::Launching));
    assert!(states.contains(&SessionState::Started));
    assert!(
        states.contains(&SessionState::Ready),
        "states={states:?} stderr={} detail={:?}",
        String::from_utf8_lossy(&output.stderr),
        state
            .record
            .as_ref()
            .map(|record| locust_adapter::managed::decode(record).unwrap())
    );
    assert_eq!(states.last(), Some(&SessionState::Exited));
    let metadata = locust_adapter::managed::decode(state.record.as_ref().unwrap()).unwrap();
    assert_eq!(
        metadata.binding.assignment.as_ref().unwrap().assignment,
        ASSIGNMENT
    );
    assert!(
        !String::from_utf8_lossy(&state.record.as_ref().unwrap().detail).contains("PRIVATE PROMPT")
    );
    assert!(
        state.reports.len() < 15,
        "receipt revision feedback loop: {}",
        state.reports.len()
    );
    drop(state);
    let before = fixture.state.lock().unwrap().reports.len();
    let pending = fixture
        .command()
        .args(["client", "pending"])
        .output()
        .unwrap();
    assert!(pending.status.success());
    assert_eq!(
        result(&pending)["result"]["cancellation_acknowledged"],
        false
    );
    assert_eq!(fixture.state.lock().unwrap().reports.len(), before);
    let resumed = fixture
        .launch(&paths)
        .args(["--resume", "native-fixture"])
        .output()
        .unwrap();
    assert!(
        resumed.status.success(),
        "{}",
        String::from_utf8_lossy(&resumed.stdout)
    );
    assert_eq!(fs::read(config).unwrap(), baseline);
    assert!(
        !fixture
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .any(|r| matches!(
                r,
                Request::TaskClaim { .. }
                    | Request::CancelAcknowledge { .. }
                    | Request::TaskSubmit { .. }
            ))
    );
}
#[test]
fn report_failure_prevents_launch_and_restores_profile() {
    let fixture = Fixture::new();
    let paths = fixture
        .setup("import pathlib,os\npathlib.Path(os.environ['HOME']+'/spawned').write_text('BAD')");
    let baseline = fs::read(paths.2.join(".factory/mcp.json")).unwrap();
    fixture.state.lock().unwrap().reject_report = true;
    let output = fixture.launch(&paths).output().unwrap();
    assert!(!output.status.success());
    assert_eq!(result(&output)["error"]["code"], "unavailable");
    assert!(!paths.2.join("spawned").exists());
    assert_eq!(
        fs::read(paths.2.join(".factory/mcp.json")).unwrap(),
        baseline
    );
}
#[test]
fn uncertain_launch_refuses_duplicates_and_recovery_never_spawns() {
    let fixture = Fixture::new();
    let paths = fixture.setup(READY_CLIENT);
    let output = fixture.launch(&paths).output().unwrap();
    assert!(output.status.success());
    fixture.state.lock().unwrap().record.as_mut().unwrap().state = SessionState::Started;
    let output = fixture.launch(&paths).output().unwrap();
    assert_eq!(result(&output)["error"]["code"], "conflict");
    let output = fixture
        .command()
        .args(["client", "recover"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value = result(&output);
    assert_eq!(value["result"]["session"]["state"], "unknown");
    assert_eq!(value["result"]["spawned"], false);
    assert_eq!(value["result"]["signaled"], false);
    let output = fixture
        .launch(&paths)
        .args(["--resume", "native-fixture"])
        .output()
        .unwrap();
    assert_eq!(result(&output)["error"]["code"], "conflict");
}
#[test]
fn wrong_assignee_is_fenced_before_spawn_or_profile_mutation() {
    let fixture = Fixture::new();
    let paths = fixture.setup(READY_CLIENT);
    let baseline = fs::read(paths.2.join(".factory/mcp.json")).unwrap();
    fixture.state.lock().unwrap().wrong_assignee = true;
    let output = fixture.launch(&paths).output().unwrap();
    assert_eq!(result(&output)["error"]["code"], "denied");
    assert!(fixture.state.lock().unwrap().reports.is_empty());
    assert_eq!(
        fs::read(paths.2.join(".factory/mcp.json")).unwrap(),
        baseline
    );
}
#[test]
fn concurrent_profile_edit_survives_and_private_backup_is_retained() {
    let fixture = Fixture::new();
    let paths=fixture.setup("import json,os\np=os.environ['HOME']+'/.factory/mcp.json'\nv=json.load(open(p));v['concurrent']=42\nopen(p,'w').write(json.dumps(v))");
    let output = fixture.launch(&paths).output().unwrap();
    assert_eq!(result(&output)["error"]["code"], "conflict");
    let value: Value =
        serde_json::from_slice(&fs::read(paths.2.join(".factory/mcp.json")).unwrap()).unwrap();
    assert_eq!(value["concurrent"], 42);
    let backup = fs::read_dir(paths.2.join(".factory"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "original"))
        .unwrap();
    assert_eq!(
        fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(fs::read_to_string(backup).unwrap().contains("unrelated"));
}

#[test]
fn edited_original_backup_is_never_restored_silently() {
    let fixture = Fixture::new();
    let paths=fixture.setup("import pathlib,os\np=pathlib.Path(os.environ['HOME'])/'.factory'\nnext(p.glob('*.original')).write_text('CHANGED BACKUP')");
    let output = fixture.launch(&paths).output().unwrap();
    assert_eq!(result(&output)["error"]["code"], "conflict");
    let config: Value =
        serde_json::from_slice(&fs::read(paths.2.join(".factory/mcp.json")).unwrap()).unwrap();
    assert!(config["mcpServers"]["locust"].is_object());
    assert!(fs::read_dir(paths.2.join(".factory")).unwrap().any(|e| {
        e.unwrap()
            .path()
            .extension()
            .is_some_and(|extension| extension == "original")
    }));
}

#[test]
fn failed_post_spawn_report_drains_child_and_retains_api_error() {
    let fixture = Fixture::new();
    let paths = fixture
        .setup("import pathlib,os\npathlib.Path(os.environ['HOME']+'/spawned').write_text('YES')");
    fixture.state.lock().unwrap().reject_started = true;
    let output = fixture.launch(&paths).output().unwrap();
    assert_eq!(result(&output)["error"]["code"], "unavailable");
    assert!(paths.2.join("spawned").exists());
    assert_eq!(
        fixture.state.lock().unwrap().record.as_ref().unwrap().state,
        SessionState::Exited
    );
    assert!(
        !fs::read_to_string(paths.2.join(".factory/mcp.json"))
            .unwrap()
            .contains("\"locust\"")
    );
}

#[test]
fn session_pending_claim_race_retries_without_failing_completed_client() {
    let fixture = Fixture::new();
    // Exit immediately after initialization. The first native observation is
    // reported before the next (now closed-stream) iteration injects the race.
    let paths = fixture.setup("import json\nprint(json.dumps({'type':'system','subtype':'init','session_id':'native-fixture'}),flush=True)");
    fixture.state.lock().unwrap().race_pending = true;
    let output = fixture.launch(&paths).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let state = fixture.state.lock().unwrap();
    assert!(state.race_observed);
    let record = state.record.as_ref().unwrap();
    assert_eq!(record.state, SessionState::Exited);
    let metadata = locust_adapter::managed::decode(record).unwrap();
    assert_eq!(metadata.binding.claim, state.claims.first().copied());
    let receipt = locust_adapter::delivery::DeliveryReceipt::decode(&metadata.delivery)
        .unwrap()
        .unwrap();
    assert_eq!(receipt.binding.claim, metadata.binding.claim);
}

#[test]
fn native_exit_drains_buffered_identity_without_waiting_for_descendant_pipe() {
    use std::sync::mpsc;
    use std::time::Duration;
    let fixture = Fixture::new();
    let paths = fixture.setup(
        r#"import json,os,pathlib,signal,time
p=pathlib.Path(os.environ['HOME'])
fifo=p/'descendant-control'
os.mkfifo(fifo,0o600)
pid=os.fork()
if pid==0:
    def unexpected(number,frame):
        (p/'descendant-signaled').write_text(str(number))
    signal.signal(signal.SIGINT,unexpected)
    signal.signal(signal.SIGTERM,unexpected)
    (p/'descendant-pid').write_text(str(os.getpid()))
    with open(fifo) as control:
        control.readline()
    os._exit(0)
while not (p/'descendant-pid').exists():
    time.sleep(0.001)
print(json.dumps({'type':'system','subtype':'init','session_id':'native-fixture'}),flush=True)
os.write(2,b'buffered partial stderr')
os._exit(0)"#,
    );
    let baseline = fs::read(paths.2.join(".factory/mcp.json")).unwrap();
    let mut command = fixture.launch(&paths);
    let (sender, done) = mpsc::channel();
    let runner = thread::spawn(move || {
        sender.send(command.output().unwrap()).unwrap();
    });
    let output = done.recv_timeout(Duration::from_secs(5));
    // Always release the fixture descendant so a failed regression remains bounded.
    let pid_path = paths.2.join("descendant-pid");
    let descendant_alive = pid_path.exists()
        && Command::new("/bin/kill")
            .args(["-0", fs::read_to_string(&pid_path).unwrap().trim()])
            .status()
            .unwrap()
            .success();
    let signaled = paths.2.join("descendant-signaled").exists();
    let mut control = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(paths.2.join("descendant-control"))
        .unwrap();
    control.write_all(b"finish\n").unwrap();
    drop(control);
    runner.join().unwrap();
    let output = output.expect("launcher must return while descendant retains native pipes");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(descendant_alive);
    assert!(!signaled);
    assert!(String::from_utf8_lossy(&output.stderr).contains("buffered partial stderr"));
    let state = fixture.state.lock().unwrap();
    let record = state.record.as_ref().unwrap();
    assert_eq!(record.state, SessionState::Exited);
    assert_eq!(record.client_session.as_deref(), Some("native-fixture"));
    let metadata = locust_adapter::managed::decode(record).unwrap();
    assert!(metadata.initialized);
    assert!(metadata.process.unwrap().exited);
    assert_eq!(
        fs::read(paths.2.join(".factory/mcp.json")).unwrap(),
        baseline
    );
}
