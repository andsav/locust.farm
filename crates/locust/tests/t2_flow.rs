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
        Self::prepared(|_| {})
    }
    fn prepared(prepare: impl FnOnce(&mut Node<MemStore, Counting>)) -> Self {
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
        prepare(&mut node);
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
    fn approved_cli(&self, authority: &[&str], args: &[&str]) -> Value {
        let mut planned_args = args.to_vec();
        planned_args.push("--plan");
        let plan = self.cli(authority, &planned_args);
        assert_eq!(plan["action"], "review_required");
        assert_eq!(plan["changed"], false);
        let mut confirmed_args = args.to_vec();
        confirmed_args.extend(["--confirm", plan["plan_id"].as_str().unwrap()]);
        self.cli(authority, &confirmed_args)
    }

    fn human(&self, args: &[&str]) -> String {
        let out = self.command().args(args).output().unwrap();
        assert!(
            out.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }
    fn undo(&self, output: &str) -> String {
        let command = output
            .lines()
            .find_map(|line| line.strip_prefix("Undo: locust "))
            .unwrap();
        self.human(&command.split_whitespace().collect::<Vec<_>>())
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
fn workspace_event(operation: &Value) -> &str {
    operation["state"]["recorded"]["event"].as_str().unwrap()
}

#[test]
fn mcp_task_reports_and_cli_workspace_updates_use_distinct_signed_selections() {
    let p = Participant::new();
    let enrolled = p.cli(&["--owner"], &["agent", "enroll", "builder"]);
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
        .formation;
    let formation = serde_json::to_string(&definition).unwrap();
    let created = p.approved_cli(
        &["--owner", "--agent", agent],
        &[
            "goal",
            "create",
            "--title",
            "T2 real core",
            "--formation-json",
            &formation,
        ],
    );
    let goal = created["goal_created"]["goal"].as_str().unwrap();
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("code.txt"), "before\n").unwrap();
    let seed = p.approved_cli(
        &["--owner"],
        &[
            "workspace",
            "init",
            "--goal",
            goal,
            "--root",
            root.path().to_str().unwrap(),
            "--path",
            "code.txt",
            "--publish",
        ],
    );
    assert!(!root.path().join(".git").exists());
    let seed_proposal = workspace_event(&seed["operation"]);
    let base = seed["candidate"]["result_manifest"].as_str().unwrap();
    let mut mcp = Mcp::new(&p, credential, session);
    mcp.tool(
        "locust_completion_declare",
        json!({"goal":goal,"subject":seed_proposal}),
    );
    let seed_integration = p.cli(
        &authority,
        &[
            "workspace",
            "integrate",
            "--goal",
            goal,
            "--proposal",
            seed_proposal,
            "--expected-empty",
        ],
    );
    let seed_revision = workspace_event(&seed_integration["workspace_operation"]);
    let opened = mcp.tool(
        "locust_task_open",
        json!({"goal":goal,"text":"change code.txt","task_type":null,"inputs":{},"parent":null}),
    );
    let task = format!("task:{}", opened["recorded"]["event"].as_str().unwrap());
    let claim = mcp.tool("locust_attempt_start", json!({"goal":goal}));
    assert_eq!(claim["claimed"]["task"], task);
    let attempt = claim["claimed"]["attempt"].as_str().unwrap();
    let generation = claim["claimed"]["generation"].as_u64().unwrap();
    let destination = p.home.path().join("work");
    let destination = destination.to_str().unwrap();
    let checkout = p.approved_cli(
        &["--owner", "--agent", agent],
        &[
            "workspace",
            "connect",
            "--goal",
            goal,
            "--folder",
            destination,
            "--task",
            &task,
            "--attempt",
            attempt,
        ],
    );
    let checkout_id = checkout["checkout"]["id"].as_str().unwrap();
    assert_eq!(checkout["checkout"]["base_revision"], seed_revision);
    p.cli(
        &authority,
        &[
            "workspace",
            "bind",
            "--goal",
            goal,
            "--checkout",
            checkout_id,
        ],
    );
    let accepted_directory = p.home.path().join("accepted");
    let accepted_checkout = p.approved_cli(
        &["--owner", "--agent", agent],
        &[
            "workspace",
            "connect",
            "--goal",
            goal,
            "--folder",
            accepted_directory.to_str().unwrap(),
        ],
    );
    let accepted_checkout_id = accepted_checkout["checkout"]["id"].as_str().unwrap();
    fs::write(Path::new(destination).join("code.txt"), "after\n").unwrap();
    fs::write(Path::new(destination).join("private.txt"), "private work\n").unwrap();
    fs::write(accepted_directory.join("unrelated.txt"), "local work\n").unwrap();
    let captured = p.cli(
        &authority,
        &[
            "workspace",
            "propose",
            "--goal",
            goal,
            "--checkout",
            checkout_id,
        ],
    );
    let manifest = captured["candidate"]["result_manifest"].as_str().unwrap();
    assert_ne!(manifest, base);
    assert_eq!(captured["candidate"]["captured_paths"], json!(["code.txt"]));
    let published = p.cli(
        &authority,
        &[
            "workspace",
            "publish",
            "--goal",
            goal,
            "--operation",
            captured["operation"]["id"].as_str().unwrap(),
        ],
    );
    let proposal = workspace_event(&published["workspace_operation"]);
    let reviewed = p.cli(
        &authority,
        &[
            "workspace",
            "review",
            "--goal",
            goal,
            "--proposal",
            proposal,
        ],
    );
    assert!(
        reviewed["changes"][0]["unified_diff"]
            .as_str()
            .unwrap()
            .contains("+after")
    );
    let submitted = mcp.tool(
        "locust_contribution_publish",
        json!({
            "goal":goal,"attempt":attempt,"generation":generation,
            "summary":"Verified code change; workspace candidate is separately published.",
            "sources":[proposal],"artifacts":[manifest],
        }),
    );
    let result = submitted["recorded"]["event"].as_str().unwrap();
    mcp.tool(
        "locust_completion_declare",
        json!({"goal":goal,"subject":result}),
    );
    mcp.tool(
        "locust_scope_select",
        json!({"goal":goal,"subject":result,"expected":null}),
    );
    let contributions = mcp.tool("locust_contributions", json!({"goal":goal,"task":task}));
    assert_eq!(contributions["contributions"][0]["contribution"], result);
    assert_eq!(contributions["contributions"][0]["selected"], true);
    let board = mcp.tool("locust_board", json!({"goal":goal}));
    assert_eq!(board["board"][0]["completed"], true);
    assert_eq!(board["board"][0]["selected"], result);
    let head = mcp.tool("locust_workspace_head", json!({"goal":goal}));
    assert_eq!(head["workspace"]["head"]["revision"], seed_revision);
    assert_eq!(
        fs::read_to_string(accepted_directory.join("code.txt")).unwrap(),
        "before\n"
    );
    mcp.tool(
        "locust_completion_declare",
        json!({"goal":goal,"subject":proposal}),
    );
    let integration = p.cli(
        &authority,
        &[
            "workspace",
            "integrate",
            "--goal",
            goal,
            "--proposal",
            proposal,
            "--expected-head",
            seed_revision,
        ],
    );
    let revision = workspace_event(&integration["workspace_operation"]);
    assert_ne!(revision, result);
    let head = mcp.tool("locust_workspace_head", json!({"goal":goal}));
    assert_eq!(head["workspace"]["head"]["revision"], revision);
    assert_eq!(head["workspace"]["head"]["result_manifest"], manifest);
    assert_eq!(
        fs::read_to_string(accepted_directory.join("code.txt")).unwrap(),
        "before\n"
    );
    let updated = p.cli(
        &authority,
        &[
            "workspace",
            "update",
            "--goal",
            goal,
            "--checkout",
            accepted_checkout_id,
        ],
    );
    assert_eq!(updated["target_in_lineage_at_completion"], true);
    assert_eq!(
        fs::read_to_string(accepted_directory.join("code.txt")).unwrap(),
        "after\n"
    );
    assert_eq!(
        fs::read_to_string(accepted_directory.join("unrelated.txt")).unwrap(),
        "local work\n"
    );
    assert_eq!(
        fs::read_to_string(Path::new(destination).join("private.txt")).unwrap(),
        "private work\n"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("code.txt")).unwrap(),
        "before\n"
    );
    assert!(!accepted_directory.join(".git").exists());
    let status = p.cli(
        &authority,
        &[
            "workspace",
            "status",
            "--goal",
            goal,
            "--checkout",
            accepted_checkout_id,
        ],
    );
    assert_eq!(status["checkout"]["base_revision"], revision);
    assert_eq!(status["checkout"]["base_manifest"], manifest);
    assert_eq!(status["untracked_paths"], json!(["unrelated.txt"]));
    let board = mcp.tool("locust_board", json!({"goal":goal}));
    assert_eq!(board["board"][0]["selected"], result);
}

#[test]
fn human_levels_allowances_and_mcp_shared_findings_form_one_workflow() {
    let participant = Participant::new();
    let enrollment = participant.cli(&["--owner"], &["agent", "enroll", "reader"]);
    let principal = enrollment["agent_enrolled"]["agent"].as_str().unwrap();
    let credential = enrollment["agent_enrolled"]["credential_path"]
        .as_str()
        .unwrap();
    let first_session_path = participant.home.path().join("first.session");
    let first_session = first_session_path.to_str().unwrap();
    participant.cli(&[], &["session", "create", first_session]);
    let created = participant.approved_cli(
        &["--owner", "--agent", principal],
        &["goal", "create", "--title", "Shared decisions"],
    );
    let goal = created["goal_created"]["goal"].as_str().unwrap();
    let human = participant.human(&[
        "--owner", "--agent", "reader", "level", "--goal", goal, "ask",
    ]);
    assert!(human.contains("reader in \"Shared decisions\": ask."));
    participant.undo(&human);
    let status = participant.cli(&["--owner"], &["goal", "status", "--goal", goal]);
    assert_eq!(status["goal_status"]["abilities"][0]["level"], "auto");
    let setting = participant.cli(
        &["--owner"],
        &["level", "--goal", goal, "--agent", "reader", "ask"],
    );
    assert_eq!(setting["level"], "ask");
    assert_eq!(setting["changed"], true);
    let unchanged = participant.human(&[
        "--owner", "--agent", "reader", "level", "--goal", goal, "ask",
    ]);
    assert!(!unchanged.contains("Undo:"));

    let authority = ["--credential", credential, "--session", first_session];
    let pending_before = participant.cli(&authority, &["pending", "--goal", goal]);
    let page = participant.cli(
        &authority,
        &["pending", "page", "--goal", goal, "--limit", "1"],
    );
    for category in [
        "ask_first",
        "to_start",
        "claimed",
        "held_elsewhere",
        "to_review",
        "to_acknowledge",
        "deliveries",
    ] {
        assert_eq!(
            page["pending_page"]["counts"][category].as_u64().unwrap(),
            pending_before["pending"][category]
                .as_array()
                .unwrap()
                .len() as u64
        );
    }
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
    assert!(!watched_human.contains("--owner --agent"));

    let mut mcp = Mcp::new(&participant, credential, first_session);
    let status = mcp.tool("locust_goal_status", json!({"goal":goal}));
    assert_eq!(status["goal_status"]["abilities"][0]["agent"], principal);
    assert_eq!(status["goal_status"]["abilities"][0]["level"], "ask");
    let opened = mcp.tool(
        "locust_task_open",
        json!({"goal":goal,"text":"Check the source","task_type":null,"inputs":{},"parent":null}),
    );
    let task = format!("task:{}", opened["recorded"]["event"].as_str().unwrap());
    let refused = mcp.request(
        "tools/call",
        json!({"name":"locust_attempt_start","arguments":{"goal":goal,"task":task,"offer":null}}),
    );
    assert_eq!(refused["isError"], true);
    assert_eq!(
        refused["structuredContent"]["error"]["code"],
        "level_required"
    );
    let allowed = participant.human(&[
        "--owner", "--agent", "reader", "allow", "--goal", goal, "--task", &task,
    ]);
    assert!(allowed.contains(
        "reader may take \"Check the source\" in \"Shared decisions\" until the host revises it."
    ));
    let revoked = participant.undo(&allowed);
    assert!(revoked.contains("A running attempt is not stopped."));
    let restored = participant.undo(&revoked);
    assert!(restored.contains("until the host revises it."));
    let claim = mcp.tool(
        "locust_attempt_start",
        json!({"goal":goal,"task":task,"offer":null}),
    );
    assert!(claim["claimed"]["attempt"].is_string());
    let revoked = participant.human(&[
        "--owner", "--agent", "reader", "allow", "--goal", goal, "--task", &task, "--revoke",
    ]);
    assert!(revoked.contains("Undo:"));
    let pending = mcp.tool("locust_pending", json!({"goal":goal}));
    assert_eq!(pending["pending"]["claimed"].as_array().unwrap().len(), 1);
    let unchanged = participant.human(&[
        "--owner", "--agent", "reader", "allow", "--goal", goal, "--task", &task, "--revoke",
    ]);
    assert!(!unchanged.contains("Undo:"));
    let finding = mcp.tool("locust_contribution_publish", json!({"goal":goal,"attempt":null,"generation":null,"summary":"Use exact source hashes; the previous cache is stale.","artifacts":[],"sources":[]}));
    let event = finding["recorded"]["event"].as_str().unwrap();
    let query = json!({"goal":goal,"view":"compact","task":null,"after":null,"limit":2,"preview_chars":null,"unread_only":true});
    let cli_page = participant.cli(
        &authority,
        &[
            "context",
            "read",
            "--goal",
            goal,
            "--view",
            "compact",
            "--limit",
            "2",
            "--unread-only",
            "true",
        ],
    );
    let cli_receipt = cli_page["context"]["receipt"].as_str().unwrap();
    assert!(cli_receipt.starts_with("ctx:"));
    assert_eq!(cli_receipt.len(), 68);
    // CLI and MCP use the same private reference namespace for this session.
    mcp.tool(
        "locust_context_acknowledge",
        json!({"goal":goal,"receipt":cli_receipt}),
    );
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
            assert!(brief["receipt"].as_str().unwrap().starts_with("ctx:"));
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
    // A fresh CLI process resolves an MCP-created reference after bridge exit.
    participant.cli(
        &authority,
        &[
            "context",
            "acknowledge",
            "--goal",
            goal,
            "--receipt",
            last_receipt.as_str().unwrap(),
        ],
    );
    let mut reopened = Mcp::new(&participant, credential, first_session);
    reopened.tool(
        "locust_context_acknowledge",
        json!({"goal":goal,"receipt":last_receipt}),
    );
    drop(reopened);
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
    // Another session's reference is unknown to this one, like a mistyped one.
    let unknown = other.request("tools/call", json!({"name":"locust_context_acknowledge","arguments":{"goal":goal,"receipt":last_receipt}}));
    assert_eq!(unknown["isError"], true);
    assert_eq!(unknown["structuredContent"]["error"]["code"], "not_found");
}

#[test]
fn reviewed_local_membership_uses_names_and_defaults_to_auto_without_tickets() {
    let participant = Participant::new();
    let alice = participant.cli(&["--owner"], &["agent", "enroll", "alice"]);
    let bob = participant.cli(&["--owner"], &["agent", "enroll", "bob"]);
    let bob_key = bob["agent_enrolled"]["agent"].as_str().unwrap();
    let created = participant.approved_cli(
        &["--owner", "--agent", "alice"],
        &[
            "goal",
            "create",
            "--title",
            "Demo work",
            "--formation",
            "peer-review",
        ],
    );
    let goal = created["goal_created"]["goal"].as_str().unwrap();
    let invites = || participant.cli(&["--owner"], &["invitation", "list", "--goal", "Demo work"]);
    assert_eq!(
        invites()["invitations"]["invitations"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let plan = participant.cli(
        &["--owner"],
        &[
            "goal",
            "add",
            "--goal",
            "Demo work",
            "--agent",
            "bob",
            "--plan",
        ],
    );
    assert_eq!(plan["action"], "review_required");
    assert_eq!(plan["plan"]["agent"], bob_key);
    assert_eq!(plan["plan"]["already_member"], false);
    assert_eq!(
        invites()["invitations"]["invitations"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    // --json is never an interactive approval, even without explicit --plan.
    let unapproved = participant.cli(
        &["--owner"],
        &["goal", "add", "--goal", "Demo work", "--agent", "bob"],
    );
    assert_eq!(unapproved["changed"], false);
    assert_eq!(
        invites()["invitations"]["invitations"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    let joined = participant.cli(
        &["--owner"],
        &[
            "goal",
            "add",
            "--goal",
            "Demo work",
            "--agent",
            "bob",
            "--confirm",
            plan["plan_id"].as_str().unwrap(),
        ],
    );
    assert_eq!(joined["membership"], "member");
    assert_eq!(joined["changed"], true);
    assert!(!joined.to_string().contains("ticket"));
    let inventory = invites();
    assert_eq!(
        inventory["invitations"]["invitations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        inventory["invitations"]["invitations"][0]["state"],
        "redeemed"
    );
    assert_eq!(joined["level"], "auto");
    let status = participant.cli(&["--owner"], &["goal", "status", "--goal", "Demo work"]);
    let abilities = status["goal_status"]["abilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|view| view["agent"] == bob_key)
        .unwrap();
    assert_eq!(abilities["level"], "auto");
    assert_eq!(abilities["membership"], "member");
    let owner_status = participant.cli(&["--owner"], &["status"]);
    assert!(
        owner_status["status"]["agents"]
            .as_array()
            .unwrap()
            .iter()
            .all(|agent| agent["author_only"] == false)
    );
    let repeated = participant
        .command()
        .args([
            "--owner",
            "--json",
            "goal",
            "add",
            "--goal",
            "Demo work",
            "--agent",
            "bob",
            "--confirm",
            plan["plan_id"].as_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(repeated.status.success());
    let repeated: Value = serde_json::from_slice(&repeated.stdout).unwrap();
    assert_eq!(repeated["result"]["changed"], false);
    assert_eq!(repeated["result"]["name"], "bob");
    assert_eq!(inventory, invites());
    let goal_status = participant.cli(&["--owner"], &["goal", "status", "--goal", goal]);
    assert!(
        goal_status["goal_status"]["members"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["member"] == bob_key)
    );
    // The global selector cannot give a credential access to an owner's command.
    let denied = participant
        .command()
        .args([
            "--credential",
            bob["agent_enrolled"]["credential_path"].as_str().unwrap(),
            "--json",
            "goal",
            "add",
            "--goal",
            "Demo work",
            "--agent",
            "alice",
            "--plan",
        ])
        .output()
        .unwrap();
    assert!(!denied.status.success());
    assert_eq!(inventory, invites());
    assert_ne!(
        alice["agent_enrolled"]["agent"],
        bob["agent_enrolled"]["agent"]
    );
}

#[test]
fn confirmed_adds_carry_the_counting_role_and_repeats_keep_the_signed_name() {
    let p = Participant::new();
    for name in ["harbor", "maple", "juniper", "cedar"] {
        p.cli(&["--owner"], &["agent", "enroll", name]);
    }
    let created = p.approved_cli(
        &["--owner", "--agent", "harbor"],
        &[
            "goal",
            "create",
            "--title",
            "Panel",
            "--formation",
            "review-panel",
        ],
    );
    let goal = created["goal_created"]["goal"].as_str().unwrap();
    for (agent, name, no_role) in [
        ("maple", "Maple", false),
        ("juniper", "Juniper", false),
        ("cedar", "Cedar", true),
    ] {
        let mut args = vec![
            "goal", "add", "--goal", goal, "--agent", agent, "--name", name,
        ];
        if no_role {
            args.push("--no-role");
        }
        assert_eq!(p.approved_cli(&["--owner"], &args)["changed"], true);
    }
    let before = p.cli(&["--owner"], &["goal", "status", "--goal", goal]);
    let members = before["goal_status"]["members"].as_array().unwrap();
    let key = |name: &str| {
        members
            .iter()
            .find(|member| member["name"] == name)
            .unwrap()["member"]
            .as_str()
            .unwrap()
    };
    let reviewers = before["goal_status"]["roles"]["reviewer"]
        .as_array()
        .unwrap();
    assert_eq!(reviewers.len(), 3);
    assert!(reviewers.contains(&json!(key("Maple"))));
    assert!(reviewers.contains(&json!(key("Juniper"))));
    assert!(!reviewers.contains(&json!(key("Cedar"))));
    let add = p.human(&[
        "--owner", "goal", "add", "--goal", goal, "--agent", "cedar", "--name", "Oak",
    ]);
    assert!(
        add.contains("cedar is already in \"Panel\" as Cedar · auto."),
        "{add}"
    );
    assert!(add.contains("A name cannot change."));
    assert!(add.contains("Give the role: locust --owner role give"));
    assert!(!add.contains("Proceed?"));
    let invited = p.approved_cli(&["--owner"], &["goal", "invite", "--goal", goal]);
    let ticket_file = p.home.path().join("repeat.ticket");
    fs::write(&ticket_file, invited["invited"]["ticket"].as_str().unwrap()).unwrap();
    fs::set_permissions(&ticket_file, fs::Permissions::from_mode(0o600)).unwrap();
    let joined = p.human(&[
        "--owner",
        "goal",
        "join",
        "--ticket-file",
        ticket_file.to_str().unwrap(),
        "--agent",
        "cedar",
        "--name",
        "Oak",
    ]);
    assert!(
        joined.contains("cedar is already in \"Panel\" as Cedar · auto."),
        "{joined}"
    );
    assert!(joined.contains("A name cannot change."));
    assert!(
        joined.contains("Give the role: locust --owner role give"),
        "{joined}"
    );
    assert!(!joined.contains("Proceed?"));
    let after = p.cli(&["--owner"], &["goal", "status", "--goal", goal]);
    assert_eq!(
        after["goal_status"]["members"],
        before["goal_status"]["members"]
    );
    assert_eq!(
        after["goal_status"]["roles"],
        before["goal_status"]["roles"]
    );
}

#[test]
fn a_real_member_daemon_reports_that_the_host_is_on_another_computer() {
    use locust_core::sync::{Host, Staged};
    use locust_proto::api::{Level, Request, Response, ServerHello};
    use locust_proto::id::EndpointId;
    use locust_proto::store::Store;
    fn call(node: &mut Node<MemStore, Counting>, request: Request) -> Response {
        let conn = ConnId(9000);
        assert!(matches!(
            node.connect(
                conn,
                &ClientHello {
                    api_version: locust_proto::API_VERSION,
                    credential: Credential([1; 32]),
                    session: None
                },
                1000
            ),
            ServerHello::Welcome { .. }
        ));
        let Step::Reply(reply) = node.request(
            conn,
            RequestFrame {
                id: 1,
                idempotency: None,
                on_behalf: None,
                request,
            },
            1000,
        ) else {
            panic!()
        };
        node.disconnect(conn);
        reply.result.unwrap()
    }
    let mut selected_goal = None;
    let mut role_ticket = None;
    let participant = Participant::prepared(|member| {
        let store = MemStore::new();
        let mut host = Node::open(
            store.reopen(),
            Counting(1000),
            Credential([1; 32]).digest(),
            "host".into(),
            0,
        )
        .unwrap();
        host.peer(
            PeerInput::Endpoint {
                endpoint: EndpointId([22; 32]),
                hints: vec![],
            },
            locust_proto::engine::PeerTime {
                unix_ms: 0,
                elapsed_ms: 0,
            },
            &mut vec![],
        );
        let Response::AgentEnrolled { agent } = call(
            &mut host,
            Request::AgentEnroll {
                name: "maple".into(),
                credential: Credential([2; 32]).digest(),
            },
        ) else {
            panic!()
        };
        let Response::GoalCreated { goal } = call(
            &mut host,
            Request::GoalCreate {
                name: "Maple".into(),
                agent,
                title: "Remote host".into(),
                formation_json: Some(
                    serde_json::to_string(
                        &locust_proto::organization::presets()
                            .into_iter()
                            .find(|preset| preset.name == "review-panel")
                            .unwrap()
                            .formation,
                    )
                    .unwrap(),
                ),
                inputs: Default::default(),
            },
        ) else {
            panic!()
        };
        let Response::GoalStatus(status) = call(&mut host, Request::GoalStatus { goal }) else {
            panic!()
        };
        assert!(status.hosted_here);
        let Response::Invited { ticket } = call(
            &mut host,
            Request::GoalInvite {
                goal,
                role: None,
                expires_ms: 604_801_000,
            },
        ) else {
            panic!()
        };
        let Response::AgentEnrolled { agent } = call(
            member,
            Request::AgentEnroll {
                name: "juniper".into(),
                credential: Credential([3; 32]).digest(),
            },
        ) else {
            panic!()
        };
        call(
            member,
            Request::GoalJoin {
                name: "Juniper".into(),
                agent,
                ticket,
                level: Level::Auto,
            },
        );
        let joining = Host::joins(member).pop().unwrap();
        Host::join(&mut host, &EndpointId([21; 32]), &joining.request, 1000).unwrap();
        let events: Vec<_> = store
            .log(&goal, 0, usize::MAX)
            .unwrap()
            .into_iter()
            .map(|(_, event)| event)
            .collect();
        let key = Host::replica(&mut host, &goal).unwrap().key(0).unwrap();
        Host::replica(member, &goal)
            .unwrap()
            .receive(events.iter().map(|event| event.to_wire()).collect())
            .unwrap();
        for event in &events {
            for hash in event.header().blobs() {
                let bytes = store.blob(&hash).unwrap().unwrap();
                assert_eq!(
                    Host::replica(member, &goal).unwrap().stage(
                        &hash,
                        0,
                        bytes.len() as u64,
                        &bytes
                    ),
                    Staged::Complete
                );
            }
        }
        assert!(Host::replica(member, &goal).unwrap().offer_key(0, key));
        let Response::GoalStatus(status) = call(member, Request::GoalStatus { goal }) else {
            panic!()
        };
        assert!(!status.hosted_here);
        assert_eq!(status.members.len(), 2);
        // A later invitation carrying the role the rules count on; the
        // ticket's expiry is read against the real clock by the command line.
        let Response::Invited { ticket } = call(
            &mut host,
            Request::GoalInvite {
                goal,
                role: Some("reviewer".into()),
                expires_ms: u64::MAX / 2,
            },
        ) else {
            panic!()
        };
        role_ticket = Some(ticket);
        selected_goal = Some(goal);
    });
    let goal = selected_goal.unwrap().to_string();
    let text = participant.human(&["--owner", "goal", "status", "--goal", &goal]);
    assert!(text.contains("Host: on another computer · Maple"), "{text}");
    assert!(!text.contains("Host: you"), "{text}");
    let ticket_file = participant.home.path().join("role.ticket");
    fs::write(&ticket_file, role_ticket.unwrap().as_str()).unwrap();
    fs::set_permissions(&ticket_file, fs::Permissions::from_mode(0o600)).unwrap();
    let joined = participant.human(&[
        "--owner",
        "goal",
        "join",
        "--ticket-file",
        ticket_file.to_str().unwrap(),
        "--agent",
        "juniper",
        "--name",
        "Oak",
    ]);
    assert_eq!(
        joined.trim_end(),
        "juniper is already in \"Remote host\" as Juniper · auto. A name cannot change. It does not hold reviewer; the host, Maple's owner, gives roles."
    );
}

#[test]
fn an_agents_cli_names_its_owner_and_refuses_workspace_init_without_a_false_disconnect() {
    let participant = Participant::new();
    participant.cli(&["--owner"], &["agent", "enroll", "maple"]);
    let enrollment = participant.cli(&["--owner"], &["agent", "enroll", "juniper"]);
    let credential = enrollment["agent_enrolled"]["credential_path"]
        .as_str()
        .unwrap();
    let created = participant.approved_cli(
        &["--owner", "--agent", "maple"],
        &["goal", "create", "--title", "Hosted here"],
    );
    let goal = created["goal_created"]["goal"].as_str().unwrap();
    participant.approved_cli(
        &["--owner", "--agent", "juniper"],
        &["goal", "add", "--goal", goal],
    );
    let text = participant.human(&["--credential", credential, "goal", "status", "--goal", goal]);
    assert!(text.contains("Host: juniper's owner"), "{text}");
    let output = participant
        .command()
        .args([
            "--credential",
            credential,
            "--json",
            "workspace",
            "init",
            "--goal",
            goal,
            "--empty",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["error"]["code"], "denied");
    assert_eq!(
        result["error"]["message"],
        "workspace init is your owner's command; use locust --owner workspace init"
    );
    let status = participant.cli(&["--owner"], &["status"]);
    assert!(
        status["status"]["agents"]
            .as_array()
            .unwrap()
            .iter()
            .all(|agent| agent["revoked"] == false)
    );
}

#[test]
fn binding_reviewer_rules_assigns_current_members_and_no_role_keeps_the_choice_explicit() {
    for no_role in [false, true] {
        let p = Participant::new();
        let mut keys = Vec::new();
        for name in ["harbor", "maple", "juniper"] {
            keys.push(
                p.cli(&["--owner"], &["agent", "enroll", name])["agent_enrolled"]["agent"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            );
        }
        let created = p.approved_cli(
            &["--owner", "--agent", "harbor"],
            &[
                "goal",
                "create",
                "--title",
                "Changing rules",
                "--formation",
                "peer-review",
            ],
        );
        let goal = created["goal_created"]["goal"].as_str().unwrap();
        for name in ["maple", "juniper"] {
            p.approved_cli(
                &["--owner"],
                &["goal", "add", "--goal", goal, "--agent", name],
            );
        }
        let mut args = vec![
            "--owner",
            "rules",
            "bind",
            "--goal",
            goal,
            "--formation",
            "review-panel",
        ];
        if no_role {
            args.push("--no-role");
        }
        let mut plan_args = args.clone();
        plan_args.push("--plan");
        let plan = p.human(&plan_args);
        let expected = if no_role {
            "2 more reviewers are needed."
        } else {
            "Everyone in the goal becomes a reviewer."
        };
        assert!(plan.contains(expected), "{plan}");
        if no_role {
            assert_eq!(
                plan.matches("Give the role: locust --owner role give")
                    .count(),
                2
            );
        }
        let id = plan
            .lines()
            .find_map(|line| line.strip_prefix("Plan id: "))
            .unwrap();
        args.extend(["--confirm", id]);
        let result = p.human(&args);
        assert!(result.contains(expected), "{result}");
        let status = p.cli(&["--owner"], &["goal", "status", "--goal", goal]);
        let holders = status["goal_status"]["roles"]["reviewer"]
            .as_array()
            .unwrap();
        let mut expected_holders = if no_role {
            vec![keys[0].clone()]
        } else {
            keys.clone()
        };
        expected_holders.sort();
        assert_eq!(
            *holders,
            expected_holders
                .into_iter()
                .map(Value::String)
                .collect::<Vec<_>>()
        );
        if !no_role {
            let undo = result
                .lines()
                .find_map(|line| line.strip_prefix("Undo for one member: locust "))
                .unwrap();
            p.human(&undo.split_whitespace().collect::<Vec<_>>());
            let status = p.cli(&["--owner"], &["goal", "status", "--goal", goal]);
            assert_eq!(
                status["goal_status"]["roles"]["reviewer"]
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
        }
    }

    // A rule that lets either of two roles approve counts the holders of
    // both; before anyone holds them, the plan and the status both say so.
    let p = Participant::new();
    for name in ["harbor", "maple", "juniper"] {
        p.cli(&["--owner"], &["agent", "enroll", name]);
    }
    let created = p.approved_cli(
        &["--owner", "--agent", "harbor"],
        &[
            "goal",
            "create",
            "--title",
            "Either role",
            "--formation",
            "peer-review",
        ],
    );
    let goal = created["goal_created"]["goal"].as_str().unwrap();
    for name in ["maple", "juniper"] {
        p.approved_cli(
            &["--owner"],
            &["goal", "add", "--goal", goal, "--agent", name],
        );
    }
    let mut formation = locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == "review-panel")
        .unwrap()
        .formation;
    formation
        .roles
        .insert("senior".into(), locust_proto::organization::Role::default());
    formation.decisions.completion = locust_proto::organization::CompletionRule::Reviews {
        by: locust_proto::organization::Selector::Any {
            selectors: vec![
                locust_proto::organization::Selector::Role {
                    name: "reviewer".into(),
                },
                locust_proto::organization::Selector::Role {
                    name: "senior".into(),
                },
            ],
        },
        count: 2,
        exclude_author: true,
    };
    let formation_json = serde_json::to_string(&formation).unwrap();
    let plan = p.human(&[
        "--owner",
        "rules",
        "bind",
        "--goal",
        goal,
        "--formation-json",
        &formation_json,
        "--no-role",
        "--plan",
    ]);
    assert!(plan.contains("2 more reviewers are needed."), "{plan}");
    let id = plan
        .lines()
        .find_map(|line| line.strip_prefix("Plan id: "))
        .unwrap();
    p.human(&[
        "--owner",
        "rules",
        "bind",
        "--goal",
        goal,
        "--formation-json",
        &formation_json,
        "--no-role",
        "--confirm",
        id,
    ]);
    let status = p.human(&["--owner", "goal", "status", "--goal", goal]);
    assert!(status.contains("2 more reviewers are needed."), "{status}");
    assert_eq!(
        status
            .matches("Give the role: locust --owner role give")
            .count(),
        2,
        "{status}"
    );

    // Where the earlier rules let one reviewer approve alone, the bind gives
    // the role to no one and the plan says so before asking.
    let p = Participant::new();
    for name in ["harbor", "maple"] {
        p.cli(&["--owner"], &["agent", "enroll", name]);
    }
    let created = p.approved_cli(
        &["--owner", "--agent", "harbor"],
        &[
            "goal",
            "create",
            "--title",
            "Was directed",
            "--formation",
            "directed",
        ],
    );
    let goal = created["goal_created"]["goal"].as_str().unwrap();
    p.approved_cli(
        &["--owner"],
        &["goal", "add", "--goal", goal, "--agent", "maple"],
    );
    let plan = p.human(&[
        "--owner",
        "rules",
        "bind",
        "--goal",
        goal,
        "--formation",
        "review-panel",
        "--plan",
    ]);
    assert!(
        plan.contains(
            "Under the goal's earlier rules, which open tasks still follow, one reviewer acts alone, so this change gives the role to no one.\n2 more reviewers are needed. The role's holders stay as they are.\nGive the role: locust --owner role give"
        ),
        "{plan}"
    );
    assert!(!plan.contains("Everyone in the goal becomes"), "{plan}");
    let id = plan
        .lines()
        .find_map(|line| line.strip_prefix("Plan id: "))
        .unwrap();
    p.human(&[
        "--owner",
        "rules",
        "bind",
        "--goal",
        goal,
        "--formation",
        "review-panel",
        "--confirm",
        id,
    ]);
    let status = p.cli(&["--owner"], &["goal", "status", "--goal", goal]);
    assert_eq!(
        status["goal_status"]["roles"]["reviewer"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn a_refused_task_reaches_the_person_as_a_waiting_line_they_can_run() {
    let participant = Participant::new();
    let enrollment = participant.cli(&["--owner"], &["agent", "enroll", "reader"]);
    let principal = enrollment["agent_enrolled"]["agent"].as_str().unwrap();
    let credential = enrollment["agent_enrolled"]["credential_path"]
        .as_str()
        .unwrap();
    let session_path = participant.home.path().join("one.session");
    let session = session_path.to_str().unwrap();
    participant.cli(&[], &["session", "create", session]);
    let created = participant.approved_cli(
        &["--owner", "--agent", principal],
        &["goal", "create", "--title", "Shared decisions"],
    );
    let goal = created["goal_created"]["goal"].as_str().unwrap();
    participant.cli(
        &["--owner"],
        &["level", "--goal", goal, "--agent", "reader", "ask"],
    );
    let mut mcp = Mcp::new(&participant, credential, session);
    let opened = mcp.tool(
        "locust_task_open",
        json!({"goal":goal,"text":"Check the source","task_type":null,"inputs":{},"parent":null}),
    );
    let task = format!("task:{}", opened["recorded"]["event"].as_str().unwrap());
    let refused = mcp.request(
        "tools/call",
        json!({"name":"locust_attempt_start","arguments":{"goal":goal,"task":task,"offer":null}}),
    );
    assert_eq!(refused["isError"], true);
    let error = &refused["structuredContent"]["error"];
    assert_eq!(error["code"], "level_required");
    // The tool's message names the agent by its local name and says "this
    // task"; the title is only in details.
    let message = error["message"].as_str().unwrap();
    assert!(
        message.starts_with("reader can't take this task in this goal: reader's level here is ask"),
        "{message}"
    );
    assert!(!message.contains("Check the source"), "{message}");
    assert_eq!(error["details"]["task_title"], "Check the source");
    assert_eq!(error["details"]["why"]["side"], "your_setting");
    let same = participant
        .command()
        .args(["--credential", credential, "--session", session])
        .args(["attempt", "start", "--goal", goal, "--task", &task])
        .output()
        .unwrap();
    assert_eq!(same.status.code(), Some(4));
    assert_eq!(
        String::from_utf8(same.stderr).unwrap().trim_end(),
        format!("locust: level_required: {message}")
    );

    let status = participant.human(&["--owner", "status"]);
    assert!(status.starts_with("Waiting for you\n"), "{status}");
    assert!(
        status.contains("  reader wants to take \"Check the source\" in \"Shared decisions\"\n    locust --owner allow --goal "),
        "{status}"
    );
    let line = status
        .lines()
        .find_map(|line| line.strip_prefix("    locust "))
        .unwrap();
    assert!(line.contains(&format!("--goal {}", &goal[..8])), "{line}");
    assert!(line.contains(&format!("--task {}", &task[..13])), "{line}");
    assert!(line.ends_with("--agent reader"), "{line}");
    let allowed = participant.human(&line.split_whitespace().collect::<Vec<_>>());
    assert!(
        allowed.contains("reader may take \"Check the source\" in \"Shared decisions\""),
        "{allowed}"
    );
    assert!(
        allowed.contains("\nUndo: locust --owner allow --goal "),
        "{allowed}"
    );
    let claim = mcp.tool(
        "locust_attempt_start",
        json!({"goal":goal,"task":task,"offer":null}),
    );
    assert!(claim["claimed"]["attempt"].is_string());
    let status = participant.human(&["--owner", "status"]);
    assert!(
        status.starts_with("Nothing is waiting for you.\n"),
        "{status}"
    );
    assert!(
        status.contains("Shared decisions (") && status.contains(") · host: you\n  reader · member · ask\n      posts, reviews; waits for your yes before each task"),
        "{status}"
    );
    // The agent's own status is worded about its owner.
    let agent_status =
        participant.human(&["--credential", credential, "--session", session, "status"]);
    assert!(
        agent_status.starts_with("Nothing is waiting for reader's owner.\n"),
        "{agent_status}"
    );
    assert!(
        agent_status.contains("host: reader's owner\n"),
        "{agent_status}"
    );
    assert!(
        agent_status.contains("posts, reviews; waits for reader's owner before each task"),
        "{agent_status}"
    );
}

#[test]
fn pending_in_text_mode_names_the_attempting_member_and_the_tasks_title() {
    let participant = Participant::new();
    let alice = participant.cli(&["--owner"], &["agent", "enroll", "alice"]);
    participant.cli(&["--owner"], &["agent", "enroll", "bob"]);
    let credential = alice["agent_enrolled"]["credential_path"].as_str().unwrap();
    let session_path = participant.home.path().join("alice.session");
    let session = session_path.to_str().unwrap();
    participant.cli(&[], &["session", "create", session]);
    let authority = ["--credential", credential, "--session", session];
    let created = participant.approved_cli(
        &["--owner", "--agent", "alice"],
        &[
            "goal",
            "create",
            "--title",
            "Demo work",
            "--formation",
            "peer-review",
        ],
    );
    let goal = created["goal_created"]["goal"].as_str().unwrap();
    participant.approved_cli(
        &["--owner"],
        &["goal", "add", "--goal", goal, "--agent", "bob"],
    );
    let opened = participant.cli(
        &authority,
        &["task", "open", "--goal", goal, "Fix the parser"],
    );
    let task = format!("task:{}", opened["recorded"]["event"].as_str().unwrap());
    let claim = participant.cli(
        &authority,
        &["attempt", "start", "--goal", goal, "--task", &task],
    );
    // Bob's owner reads names and titles, not keys and bare identifiers.
    let pending = participant.human(&["--owner", "--agent", "bob", "pending", "--goal", goal]);
    assert!(
        pending.contains(&format!(
            "Ready to start: Fix the parser ({})\n  Attempting: alice (",
            &task[..13]
        )),
        "{pending}"
    );
    participant.cli(
        &authority,
        &[
            "contribution",
            "publish",
            "--goal",
            goal,
            "--attempt",
            claim["claimed"]["attempt"].as_str().unwrap(),
            "--generation",
            &claim["claimed"]["generation"].to_string(),
            "A first result",
        ],
    );
    let pending = participant.human(&["--owner", "--agent", "bob", "pending", "--goal", goal]);
    assert!(pending.contains("Attempting: alice ("), "{pending}");
    assert!(pending.contains(" · 0 of 1 approvals"), "{pending}");
    // The agent's own wait, answered at once, reads the same names.
    let waited = participant.human(&[
        "--owner",
        "--agent",
        "bob",
        "watch",
        "--goal",
        goal,
        "--timeout-ms",
        "0",
    ]);
    assert!(waited.contains("Fix the parser"), "{waited}");
}
