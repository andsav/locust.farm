//! Actual binary stdio-pipe contracts, independent of installed model clients.
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
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
