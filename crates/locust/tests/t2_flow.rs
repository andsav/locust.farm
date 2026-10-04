//! Production MCP and CLI against a real Node/MemStore over local OS pipes.
//! Network replication has separate Node/Driver tests; no client or relay runs.
use locust_core::node::Node;
use locust_proto::api::{ClientHello, Credential, RequestFrame};
use locust_proto::codec;
use locust_proto::engine::{ConnId, Engine, Entropy, PeerEngine, PeerInput, Step};
use locust_proto::limits::{MAX_HELLO_FRAME_BYTES, MAX_LOCAL_FRAME_BYTES};
use locust_proto::store::MemStore;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;

struct Counting(u64);
impl Entropy for Counting {
    fn fill(&mut self, bytes: &mut [u8]) {
        self.0 += 1;
        let digest = locust_proto::crypto::content_hash(&self.0.to_le_bytes());
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = digest.0[i % 32] ^ (i / 32) as u8;
        }
    }
}
struct Participant {
    home: tempfile::TempDir,
    stopped: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Participant {
    fn new() -> Self {
        let home = tempfile::Builder::new()
            .prefix("lc-t2-")
            .tempdir_in("/tmp")
            .unwrap();
        fs::set_permissions(home.path(), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(home.path().join("owner.credential"), [1u8; 32]).unwrap();
        fs::set_permissions(
            home.path().join("owner.credential"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let mut node = Node::open(
            MemStore::new(),
            Counting(0),
            Credential([1; 32]).digest(),
            "t2-production-core".into(),
            0,
        )
        .unwrap();
        node.peer(
            PeerInput::Endpoint {
                endpoint: locust_proto::id::EndpointId([21; 32]),
                hints: vec![],
            },
            locust_proto::engine::PeerTime {
                unix_ms: 0,
                elapsed_ms: 0,
            },
            &mut vec![],
        );
        let listener = UnixListener::bind(home.path().join("daemon.sock")).unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let done = stopped.clone();
        let thread = thread::spawn(move || {
            let mut serial = 0;
            loop {
                let (mut stream, _) = listener.accept().unwrap();
                if done.load(Ordering::SeqCst) {
                    break;
                }
                serial += 1;
                let conn = ConnId(serial);
                let hello = codec::read_frame(&mut stream, MAX_HELLO_FRAME_BYTES)
                    .unwrap()
                    .unwrap();
                let hello = ClientHello::decode(&hello).unwrap();
                let answer = node.connect(conn, &hello, 1000);
                let mut bytes = Vec::new();
                codec::encode_frame(&answer, &mut bytes).unwrap();
                stream.write_all(&bytes).unwrap();
                while let Some(frame) =
                    codec::read_frame(&mut stream, MAX_LOCAL_FRAME_BYTES).unwrap()
                {
                    let frame: RequestFrame = codec::decode(&frame).unwrap();
                    let Step::Reply(response) = node.request(conn, frame, 1000) else {
                        panic!("workflow does not park a wait")
                    };
                    bytes.clear();
                    codec::encode_frame(&response, &mut bytes).unwrap();
                    stream.write_all(&bytes).unwrap();
                }
                node.disconnect(conn);
            }
        });
        Self {
            home,
            stopped,
            thread: Some(thread),
        }
    }
    fn command(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_locust"));
        c.env_remove("LOCUST_HOME")
            .env_remove("LOCUST_CREDENTIAL")
            .env_remove("LOCUST_SESSION")
            .arg("--home")
            .arg(self.home.path());
        c
    }
    fn cli(&self, authority: &[&str], args: &[&str]) -> Value {
        let out = self
            .command()
            .args(authority)
            .arg("--json")
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["result"].clone()
    }
}
impl Drop for Participant {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        let _ = UnixStream::connect(self.home.path().join("daemon.sock"));
        self.thread.take().unwrap().join().unwrap();
    }
}
struct Mcp {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    next: u64,
}
impl Mcp {
    fn new(p: &Participant, credential: &str, session: &str) -> Self {
        let mut child = p
            .command()
            .args(["--credential", credential, "--session", session, "mcp"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = Some(child.stdin.take().unwrap());
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut m = Self {
            child,
            input,
            output,
            next: 0,
        };
        assert_eq!(m.request("initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t2-component","version":"1"}}))["protocolVersion"],"2025-11-25");
        writeln!(
            m.input.as_mut().unwrap(),
            "{}",
            json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        )
        .unwrap();
        let tools = m.request("tools/list", json!({}));
        assert!(
            tools["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["name"] == "locust_attempt_start")
        );
        m
    }
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        writeln!(
            self.input.as_mut().unwrap(),
            "{}",
            json!({"jsonrpc":"2.0","id":self.next,"method":method,"params":params})
        )
        .unwrap();
        let mut line = String::new();
        assert!(self.output.read_line(&mut line).unwrap() > 0);
        let answer: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(answer["id"], self.next);
        assert!(answer.get("error").is_none(), "{answer}");
        answer["result"].clone()
    }
    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        let r = self.request("tools/call", json!({"name":name,"arguments":arguments}));
        assert_eq!(r["isError"], false, "{r}");
        r["structuredContent"]["result"].clone()
    }
}
impl Drop for Mcp {
    fn drop(&mut self) {
        self.input.take();
        if !std::thread::panicking() {
            assert!(self.child.wait().unwrap().success());
        } else {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

#[test]
fn mcp_attempt_and_cli_contribution_selection_apply_use_real_authority_and_sealed_content() {
    let p = Participant::new();
    let enrolled = p.cli(
        &["--owner"],
        &["agent", "enroll", "builder", "--manage-goals"],
    );
    let agent = enrolled["agent_enrolled"]["agent"].as_str().unwrap();
    let credential = enrolled["agent_enrolled"]["credential_path"]
        .as_str()
        .unwrap();
    let session = p.home.path().join("run.session");
    let session = session.to_str().unwrap();
    p.cli(&[], &["session", "create", session]);
    let authority = ["--credential", credential, "--session", session];
    let definition = locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == "independent-attempts")
        .unwrap()
        .blueprint;
    let fields = json!({"title":"T2 real core", "blueprint_json":serde_json::to_string(&definition).unwrap(), "roles":{"chooser":[agent]}, "inputs":{}}).to_string();
    let created = p.cli(&authority, &["call", "goal.create", &fields]);
    let goal = created["goal_created"]["goal"].as_str().unwrap();
    let root = tempfile::tempdir().unwrap();
    git(root.path(), &["init", "-q"]);
    git(root.path(), &["config", "user.name", "T2 fixture"]);
    git(root.path(), &["config", "user.email", "t2@example.invalid"]);
    fs::write(root.path().join("code.txt"), "before\n").unwrap();
    git(root.path(), &["add", "code.txt"]);
    git(root.path(), &["commit", "-qm", "base"]);
    let commit = git(root.path(), &["rev-parse", "HEAD"]);
    fs::write(root.path().join("unrelated.txt"), "local work\n").unwrap();
    let export = p.cli(
        &authority,
        &[
            "workspace",
            "export",
            "--goal",
            goal,
            "--root",
            root.path().to_str().unwrap(),
            "--commit",
            &commit,
        ],
    );
    let base = export["manifest"].as_str().unwrap();
    let mut mcp = Mcp::new(&p, credential, session);
    let grants = json!({"goal":goal,"agent":agent,"grants":{"contribute":true,"execute":true,"review":false,"select":true,"flow":false,"administer":false,"takeover":false}}).to_string();
    p.cli(&["--owner"], &["call", "goal.grant", &grants]);
    let opened = mcp.tool(
        "locust_task_open",
        json!({"goal":goal,"text":"change code.txt","variation":null,"inputs":{},"parent":null}),
    );
    let task = format!("task:{}", opened["recorded"]["event"].as_str().unwrap());
    let authorize = json!({"goal":goal,"task":task,"agent":agent,"takeover":false}).to_string();
    p.cli(&["--owner"], &["call", "task.authorize", &authorize]);
    let claim = mcp.tool(
        "locust_attempt_start",
        json!({"goal":goal,"task":task,"offer":null}),
    );
    let attempt = claim["claimed"]["attempt"].as_str().unwrap();
    let generation = claim["claimed"]["generation"].as_u64().unwrap().to_string();
    let destination = p.home.path().join("work");
    let destination = destination.to_str().unwrap();
    p.cli(
        &authority,
        &[
            "workspace",
            "materialize",
            "--goal",
            goal,
            "--manifest",
            base,
            "--destination",
            destination,
        ],
    );
    fs::write(Path::new(destination).join("code.txt"), "after\n").unwrap();
    let patch = p.cli(
        &authority,
        &[
            "patch",
            "create",
            "--goal",
            goal,
            "--root",
            destination,
            "--base",
            base,
            "--path",
            "code.txt",
        ],
    );
    let hash = patch["contribution_id"].as_str().unwrap();
    let head = patch["contribution"]["head"].as_str().unwrap();
    assert_ne!(head, base);
    let reviewed = p.cli(
        &authority,
        &["patch", "review", "--goal", goal, "--patch", hash],
    );
    assert!(
        reviewed["changes"][0]["unified_diff"]
            .as_str()
            .unwrap()
            .contains("+after")
    );
    let submitted = p.cli(
        &authority,
        &[
            "patch",
            "submit",
            "--goal",
            goal,
            "--patch",
            hash,
            "--attempt",
            attempt,
            "--generation",
            &generation,
            "verified change",
        ],
    );
    let result = submitted["recorded"]["event"].as_str().unwrap();
    mcp.tool(
        "locust_completion_declare",
        json!({"goal":goal,"subject":result}),
    );
    p.cli(
        &authority,
        &[
            "patch",
            "select",
            "--goal",
            goal,
            "--patch",
            hash,
            "--subject",
            result,
        ],
    );
    assert_eq!(
        fs::read_to_string(root.path().join("code.txt")).unwrap(),
        "before\n"
    );
    let status = mcp.tool("locust_goal_status", json!({"goal":goal}));
    assert!(status["goal_status"].get("head").is_none());
    assert_eq!(status["goal_status"]["workspace"]["integrated"], base);
    let contributions = mcp.tool("locust_contributions", json!({"goal":goal,"task":task}));
    assert_eq!(contributions["contributions"][0]["contribution"], result);
    assert_eq!(contributions["contributions"][0]["selected"], true);
    p.cli(
        &authority,
        &[
            "patch",
            "apply",
            "--subject",
            result,
            "--goal",
            goal,
            "--patch",
            hash,
            "--root",
            root.path().to_str().unwrap(),
            "--expected-base",
            base,
            "--expected-git-head",
            &commit,
        ],
    );
    assert_eq!(
        fs::read_to_string(root.path().join("code.txt")).unwrap(),
        "after\n"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("unrelated.txt")).unwrap(),
        "local work\n"
    );
    assert_eq!(git(root.path(), &["rev-parse", "HEAD"]), commit);
    let status = mcp.tool("locust_goal_status", json!({"goal":goal}));
    assert_eq!(status["goal_status"]["workspace"]["integrated"], head);
    let board = mcp.tool("locust_board", json!({"goal":goal}));
    assert_eq!(board["board"][0]["completed"], true);
    assert_eq!(board["board"][0]["selected"], result);
}

#[test]
fn human_permission_controls_and_mcp_shared_findings_form_one_workflow() {
    let participant = Participant::new();
    let enrollment = participant.cli(
        &["--owner"],
        &["agent", "enroll", "reader", "--manage-goals"],
    );
    let principal = enrollment["agent_enrolled"]["agent"].as_str().unwrap();
    let credential = enrollment["agent_enrolled"]["credential_path"]
        .as_str()
        .unwrap();
    let first_session_path = participant.home.path().join("first.session");
    let first_session = first_session_path.to_str().unwrap();
    participant.cli(&[], &["session", "create", first_session]);
    let created = participant.cli(
        &["--credential", credential],
        &["goal", "create", "--title", "Shared decisions"],
    );
    let goal = created["goal_created"]["goal"].as_str().unwrap();
    let permission_args = [
        "permission",
        "allow",
        "--goal",
        goal,
        "--agent",
        "reader",
        "contribute",
        "review",
    ];
    let grants = participant.cli(&["--owner"], &permission_args);
    assert_eq!(grants["permissions"]["grants"]["contribute"], true);
    assert_eq!(grants["permissions"]["grants"]["review"], true);
    assert_eq!(grants["permissions"]["grants"]["execute"], false);
    let human = participant
        .command()
        .args([
            "--owner",
            "permission",
            "inspect",
            "--goal",
            goal,
            "--agent",
            "reader",
        ])
        .output()
        .unwrap();
    assert!(
        human.status.success(),
        "{}",
        String::from_utf8_lossy(&human.stderr)
    );
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains(&format!("Permissions for reader ({principal})")));
    assert!(human.contains("not allowed"));
    assert!(!human.trim_start().starts_with('{'));
    let revoked = participant.cli(
        &["--owner"],
        &[
            "permission",
            "revoke",
            "--goal",
            goal,
            "--agent",
            "reader",
            "review",
        ],
    );
    assert_eq!(revoked["permissions"]["grants"]["review"], false);
    assert_eq!(revoked["permissions"]["grants"]["contribute"], true);

    let authority = ["--credential", credential, "--session", first_session];
    let pending_before = participant.cli(&authority, &["pending", "--goal", goal]);
    let watched = participant.cli(&authority, &["watch", "--goal", goal, "--timeout-ms", "0"]);
    assert_eq!(watched["initial"], pending_before["pending"]);
    assert!(matches!(
        watched["result"]["waited"].as_str(),
        Some("no_event" | "disconnected")
    ));
    let pending_after = participant.cli(&authority, &["pending", "--goal", goal]);
    assert_eq!(pending_after, pending_before);
    let watched_human = participant
        .command()
        .args(authority)
        .args(["watch", "--goal", goal, "--timeout-ms", "0"])
        .output()
        .unwrap();
    assert!(
        watched_human.status.success(),
        "{}",
        String::from_utf8_lossy(&watched_human.stderr)
    );
    let watched_human = String::from_utf8(watched_human.stdout).unwrap();
    assert!(watched_human.contains("Observed revision"));
    assert!(watched_human.contains("Observing for up to 0 ms"));
    assert!(watched_human.contains("Observation only: no work or context was acknowledged."));
    assert!(!watched_human.contains("--owner --as"));

    let mut mcp = Mcp::new(&participant, credential, first_session);
    let finding = mcp.tool("locust_contribution_publish", json!({"goal":goal,"task":null,"attempt":null,"generation":null,"summary":"Use exact source hashes; the previous cache is stale.","base":null,"patch":null,"artifacts":[]}));
    let event = finding["recorded"]["event"].as_str().unwrap();
    let query = json!({"goal":goal,"task":null,"after":null,"limit":2,"preview_chars":null,"unread_only":true});
    let mut after = Value::Null;
    let mut read_finding = false;
    let mut last_receipt = Value::Null;
    loop {
        let mut arguments = query.clone();
        arguments["after"] = after;
        let brief = mcp.tool("locust_context_read", arguments);
        let brief = &brief["context"];
        for item in brief["items"].as_array().unwrap() {
            if item["event"]["view"]["event"] == event {
                assert_eq!(item["event"]["view"]["author"], principal);
                assert_eq!(
                    item["event"]["text"],
                    "Use exact source hashes; the previous cache is stale."
                );
                assert_eq!(item["text_complete"], true);
                read_finding = true;
            }
        }
        if !brief["receipt"].is_null() {
            last_receipt = brief["receipt"].clone();
            mcp.tool(
                "locust_context_acknowledge",
                json!({"goal":goal,"receipt":last_receipt}),
            );
        }
        after = brief["next"].clone();
        if after.is_null() {
            break;
        }
    }
    assert!(read_finding);
    let pending = mcp.tool("locust_pending", json!({"goal":goal}));
    assert_eq!(pending["pending"]["context_news"]["unacknowledged"], 0);
    drop(mcp);
    let second_session_path = participant.home.path().join("second.session");
    let second_session = second_session_path.to_str().unwrap();
    participant.cli(&[], &["session", "create", second_session]);
    let mut other = Mcp::new(&participant, credential, second_session);
    let pending = other.tool("locust_pending", json!({"goal":goal}));
    assert!(
        pending["pending"]["context_news"]["unacknowledged"]
            .as_u64()
            .unwrap()
            > 0
    );
    let denied = other.request("tools/call", json!({"name":"locust_context_acknowledge","arguments":{"goal":goal,"receipt":last_receipt}}));
    assert_eq!(denied["isError"], true);
    assert_eq!(denied["structuredContent"]["error"]["code"], "denied");
}
