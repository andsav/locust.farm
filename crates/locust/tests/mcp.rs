//! Actual binary stdio-pipe contracts, independent of installed model clients.
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn command(home: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_locust"));
    command
        .env_remove("LOCUST_HOME")
        .env_remove("LOCUST_CREDENTIAL")
        .env_remove("LOCUST_SESSION")
        .arg("--home")
        .arg(home);
    command
}
fn scratch() -> tempfile::TempDir {
    let home = tempfile::Builder::new()
        .prefix("lc-mcp-")
        .tempdir_in("/tmp")
        .unwrap();
    fs::set_permissions(home.path(), fs::Permissions::from_mode(0o700)).unwrap();
    for file in ["credential", "session"] {
        fs::write(home.path().join(file), [1u8; 32]).unwrap();
        fs::set_permissions(home.path().join(file), fs::Permissions::from_mode(0o600)).unwrap();
    }
    home
}
#[test]
fn actual_stdio_pipes_negotiate_ping_and_exit_cleanly_on_eof() {
    let home = scratch();
    let mut child = Process(
        command(home.path())
            .arg("--credential")
            .arg(home.path().join("credential"))
            .arg("--session")
            .arg(home.path().join("session"))
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut stdin = child.0.stdin.take().unwrap();
    let stdout = child.0.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            sender
                .send(serde_json::from_str::<Value>(&line.unwrap()).unwrap())
                .unwrap();
        }
    });
    writeln!(stdin,"{}",json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"future-version","capabilities":{},"clientInfo":{"name":"binary-test","version":"1"}}})).unwrap();
    let initialized = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
    writeln!(
        stdin,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    writeln!(stdin, "{}", json!({"jsonrpc":"2.0","id":2,"method":"ping"})).unwrap();
    assert_eq!(
        receiver.recv_timeout(Duration::from_secs(5)).unwrap(),
        json!({"jsonrpc":"2.0","id":2,"result":{}})
    );
    drop(stdin);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "MCP did not exit after stdin EOF"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    reader.join().unwrap();
    let stderr = child.0.stderr.take().unwrap();
    assert_eq!(BufReader::new(stderr).lines().count(), 0);
}
#[test]
fn invalid_mcp_options_and_receipt_without_session_never_write_cli_json_to_stdout() {
    let home = scratch();
    for args in [
        vec!["mcp", "--json"],
        vec!["mcp", "--owner"],
        vec!["mcp", "--idempotency-key", "01"],
        vec!["mcp", "--as", "worker"],
    ] {
        let output = command(home.path()).args(args).output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    let output = command(home.path())
        .arg("--credential")
        .arg(home.path().join("credential"))
        .arg("mcp")
        .arg("--lifecycle-receipt")
        .arg(home.path().join("session"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("execution session"));
}

#[test]
fn ordinary_cli_values_named_mcp_do_not_select_transport_output() {
    let home = scratch();
    for args in [
        vec!["--json", "--credential", "mcp", "status"],
        vec![
            "--json",
            "contribution",
            "publish",
            "--goal",
            "invalid",
            "mcp",
        ],
        vec!["--json", "call", "status", "mcp"],
    ] {
        let output = command(home.path()).args(args).output().unwrap();
        assert!(!output.status.success());
        assert!(output.stderr.is_empty());
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["ok"], false);
    }
}

#[test]
fn closing_stdout_during_an_idle_wait_cancels_and_joins_without_more_input() {
    use locust_proto::api::{
        Caller, ClientHello, Credential, Request, RequestFrame, ServerHello, SessionSecret,
    };
    use locust_proto::{API_VERSION, codec};
    use std::os::unix::net::UnixListener;
    let home = scratch();
    let listener = UnixListener::bind(home.path().join("daemon.sock")).unwrap();
    let (waiting, wait_started) = std::sync::mpsc::channel();
    let daemon = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let hello: ClientHello =
            codec::decode(&codec::read_frame(&mut stream, 4096).unwrap().unwrap()).unwrap();
        assert_eq!(hello.credential, Credential([1; 32]));
        assert_eq!(hello.session, Some(SessionSecret([1; 32])));
        let welcome = ServerHello::Welcome {
            api_version: API_VERSION,
            daemon_version: "pipe-test".into(),
            caller: Caller::Agent(locust_proto::id::PublicKey([3; 32])),
            max_blob_bytes: 1024,
        };
        codec::write_frame(&mut stream, &codec::encode(&welcome).unwrap()).unwrap();
        let frame: RequestFrame =
            codec::decode(&codec::read_frame(&mut stream, 4096).unwrap().unwrap()).unwrap();
        assert!(matches!(
            frame.request,
            Request::Wait {
                timeout_ms: u32::MAX,
                ..
            }
        ));
        waiting.send(()).unwrap();
        assert!(
            codec::read_frame(&mut stream, 4096).unwrap().is_none(),
            "stdout closure did not cancel daemon wait"
        );
    });
    let mut child = Process(
        command(home.path())
            .arg("--credential")
            .arg(home.path().join("credential"))
            .arg("--session")
            .arg(home.path().join("session"))
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut stdin = child.0.stdin.take().unwrap();
    let stdout = child.0.stdout.take().unwrap();
    let (initialized, receive_initialized) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut stdout = BufReader::new(stdout);
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        initialized
            .send((serde_json::from_str::<Value>(&line).unwrap(), stdout))
            .unwrap();
    });
    writeln!(stdin,"{}",json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"binary-test","version":"1"}}})).unwrap();
    let (answer, stdout) = receive_initialized
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    assert_eq!(answer["result"]["protocolVersion"], "2025-11-25");
    reader.join().unwrap();
    writeln!(
        stdin,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    writeln!(stdin,"{}",json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"locust_wait","arguments":{"goal":"01".repeat(32),"seen":0,"timeout_ms":u32::MAX}}})).unwrap();
    wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
    // Keep stdin open. No cancellation, ping, EOF or other input follows.
    drop(stdout);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert_eq!(status.code(), Some(8));
            break;
        }
        assert!(
            Instant::now() < deadline,
            "MCP did not tear down its idle wait after stdout closure"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    daemon.join().unwrap();
    drop(stdin);
}

/// One initialized `locust mcp` process over its real pipes.
struct Bridge {
    _child: Process,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next: u64,
}
impl Bridge {
    fn start(home: &Path, credential: &Path, session: Option<&Path>) -> Self {
        let mut command = command(home);
        command.arg("--credential").arg(credential);
        if let Some(session) = session {
            command.arg("--session").arg(session);
        }
        let mut child = Process(
            command
                .arg("mcp")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let mut bridge = Self {
            stdin: child.0.stdin.take().unwrap(),
            stdout: BufReader::new(child.0.stdout.take().unwrap()),
            _child: child,
            next: 0,
        };
        let initialized = bridge.ask(
            "initialize",
            json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"binary-test","version":"1"}}),
        );
        assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
        writeln!(
            bridge.stdin,
            "{}",
            json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        )
        .unwrap();
        bridge
    }
    fn ask(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        writeln!(
            self.stdin,
            "{}",
            json!({"jsonrpc":"2.0","id":self.next,"method":method,"params":params})
        )
        .unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
    /// The names `tools/list` answers with.
    fn tools(&mut self) -> Vec<String> {
        let answer = self.ask("tools/list", json!({}));
        answer["result"]["tools"]
            .as_array()
            .unwrap_or_else(|| panic!("{answer}"))
            .iter()
            .map(|tool| tool["name"].as_str().unwrap().to_owned())
            .collect()
    }
    /// The structured result of calling `name` without arguments.
    fn call(&mut self, name: &str) -> Value {
        self.ask("tools/call", json!({"name": name}))["result"]["structuredContent"].clone()
    }
}

/// The listing must agree with a real daemon: every kind of credential the
/// bridge accepts sees tools before calling one, and a tool left out of its
/// list is one the daemon denies it.
#[test]
fn each_kind_of_credential_lists_the_tools_a_real_daemon_lets_it_call() {
    use locust_proto::api::{Credential, OPERATIONS};
    let home = scratch();
    let _daemon = Process(
        command(home.path())
            .args(["daemon", "run"])
            .env("LOCUST_RELAY", "none")
            .env("LOCUST_LOOKUP", "none")
            .env("LOCUST_BIND", "127.0.0.1:0")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let owner = |arguments: &[&str]| -> Option<Value> {
        let output = command(home.path())
            .args(["--owner", "--json"])
            .args(arguments)
            .output()
            .unwrap();
        let envelope: Value = serde_json::from_slice(&output.stdout).ok()?;
        (envelope["ok"] == true).then(|| envelope["result"].clone())
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    while owner(&["status"]).is_none() {
        assert!(Instant::now() < deadline, "daemon did not start");
        std::thread::sleep(Duration::from_millis(10));
    }
    let agent = owner(&["agent", "enroll", "worker"]).unwrap()["agent_enrolled"].clone();
    let author = owner(&["author", "enroll", "scribe"]).unwrap()["author_enrolled"].clone();
    let viewer = home.path().join("viewer.credential");
    fs::write(&viewer, [7u8; 32]).unwrap();
    fs::set_permissions(&viewer, fs::Permissions::from_mode(0o600)).unwrap();
    let enrollment =
        json!({"agent": agent["agent"], "credential": Credential([7; 32]).digest()}).to_string();
    owner(&["call", "viewer.enroll", &enrollment]).unwrap();

    // The author lists before it calls anything, and has no session.
    let mut bridge = Bridge::start(
        home.path(),
        Path::new(author["credential_path"].as_str().unwrap()),
        None,
    );
    let listed = bridge.tools();
    assert!(listed.contains(&"locust_formation_drafts".to_owned()));
    assert!(
        listed
            .iter()
            .all(|name| name.starts_with("locust_formation_")),
        "{listed:?}"
    );
    assert_eq!(bridge.call("locust_formation_drafts")["ok"], true);
    assert_eq!(bridge.call("locust_status")["error"]["code"], "denied");

    let mut bridge = Bridge::start(
        home.path(),
        Path::new(agent["credential_path"].as_str().unwrap()),
        Some(&home.path().join("session")),
    );
    let registry: Vec<String> = OPERATIONS
        .iter()
        .filter(|operation| operation.tool)
        .map(|operation| operation.tool_name())
        .collect();
    assert_eq!(bridge.tools(), registry);
    assert_eq!(bridge.call("locust_status")["ok"], true);
    assert_eq!(bridge.call("locust_formation_drafts")["ok"], true);

    let mut bridge = Bridge::start(home.path(), &viewer, None);
    let listed = bridge.tools();
    assert!(listed.contains(&"locust_status".to_owned()));
    for operation in OPERATIONS.iter().filter(|operation| operation.tool) {
        if listed.contains(&operation.tool_name()) {
            assert!(operation.read_only, "{}", operation.name);
        }
    }
    assert_eq!(bridge.call("locust_status")["ok"], true);
    assert!(!listed.contains(&"locust_formation_drafts".to_owned()));
    assert_eq!(
        bridge.call("locust_formation_drafts")["error"]["code"],
        "denied"
    );

    let mut bridge = Bridge::start(home.path(), &home.path().join("owner.credential"), None);
    let answer = bridge.ask("tools/list", json!({}));
    assert!(answer.get("result").is_none(), "{answer}");
    assert_eq!(answer["error"]["code"], -32000);
    assert_eq!(answer["error"]["data"]["code"], "denied");
    assert_eq!(bridge.call("locust_status")["error"]["code"], "denied");
}
