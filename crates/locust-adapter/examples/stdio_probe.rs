//! Stand-in MCP server for isolated client qualification, not the Locust bridge.
//!
//! Run `stdio_probe --socket PATH --io-timeout-ms N`. The harness chooses the
//! socket and positive I/O timeout; the tool accepts no arguments.
//! Point it at a fixture socket, not `daemon.sock`: each call connects, writes
//! `locust-probe-v0 ping\n`, and expects exactly `locust-probe-v0 pong\n`.
//! A matching reply proves this MCP process completed that socket roundtrip;
//! it proves neither daemon authentication nor client approval behavior.
//!
//! Uses newline-delimited JSON-RPC with initialization, initialized notification,
//! ping, tools/list and tools/call. Requests run sequentially. EOF exits. This
//! fixture does not implement wait, cancellation, launch or session recovery.
//! Socket I/O failures are tool errors; stdout contains protocol messages only.
//!
//! Protocol reference: <https://modelcontextprotocol.io/specification/2025-11-25>.

#![forbid(unsafe_code)]

#[cfg(unix)]
mod unix {
    use std::ffi::OsString;
    use std::io::{self, BufRead, Read, Write};
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;
    use std::time::Duration;

    use serde_json::{Value, json};

    const TOOL_NAME: &str = "locust_probe_socket";
    const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];
    const SOCKET_REQUEST: &[u8] = b"locust-probe-v0 ping\n";
    const SOCKET_RESPONSE: &[u8] = b"locust-probe-v0 pong\n";
    const USAGE: &str = "usage: stdio_probe --socket PATH --io-timeout-ms N";

    struct Config {
        socket: PathBuf,
        io_timeout: Duration,
    }

    impl Config {
        fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, &'static str> {
            let mut args = args.into_iter();
            let mut socket = None;
            let mut timeout = None;
            while let Some(arg) = args.next() {
                if arg == "--socket" && socket.is_none() {
                    let path = args.next().ok_or(USAGE)?;
                    if path.is_empty() {
                        return Err(USAGE);
                    }
                    socket = Some(PathBuf::from(path));
                } else if arg == "--io-timeout-ms" && timeout.is_none() {
                    let milliseconds = args
                        .next()
                        .and_then(|value| value.to_str().and_then(|text| text.parse::<u64>().ok()))
                        .filter(|value| *value > 0)
                        .ok_or(USAGE)?;
                    timeout = Some(Duration::from_millis(milliseconds));
                } else {
                    return Err(USAGE);
                }
            }
            Ok(Self {
                socket: socket.ok_or(USAGE)?,
                io_timeout: timeout.ok_or(USAGE)?,
            })
        }
    }

    #[derive(Default, PartialEq)]
    enum Phase {
        #[default]
        New,
        AwaitingInitialized,
        Ready,
    }

    struct Server {
        config: Config,
        phase: Phase,
    }

    impl Server {
        fn new(config: Config) -> Self {
            Self {
                config,
                phase: Phase::New,
            }
        }

        fn handle(&mut self, line: &str) -> Option<Value> {
            let message: Value = match serde_json::from_str(line) {
                Ok(message) => message,
                Err(_) => return Some(error(Value::Null, -32700, "Parse error")),
            };
            let Some(object) = message.as_object() else {
                return Some(error(Value::Null, -32600, "Invalid request"));
            };
            let id = object.get("id");
            let valid_id = id.filter(|id| id.is_string() || id.is_i64() || id.is_u64());
            let method = object.get("method").and_then(Value::as_str);
            if object.get("jsonrpc") != Some(&json!("2.0"))
                || method.is_none()
                || id.is_some() && valid_id.is_none()
            {
                return Some(error(
                    valid_id.cloned().unwrap_or(Value::Null),
                    -32600,
                    "Invalid request",
                ));
            }
            let method = method.expect("method was checked");
            let params = object.get("params");
            let Some(id) = valid_id.cloned() else {
                // Notifications, including unknown ones, never receive replies
                // and never execute tools. The initialized notification alone
                // advances the handshake.
                if method == "notifications/initialized"
                    && self.phase == Phase::AwaitingInitialized
                    && params.is_none_or(Value::is_object)
                {
                    self.phase = Phase::Ready;
                }
                return None;
            };
            if params.is_some_and(|value| !value.is_object()) {
                return Some(error(id, -32602, "Parameters must be an object"));
            }
            if method == "ping" {
                return Some(success(id, json!({})));
            }
            if method == "initialize" {
                return Some(self.initialize(id, params));
            }
            if self.phase != Phase::Ready {
                return Some(error(id, -32000, "Initialization is not complete"));
            }
            Some(match method {
                "tools/list" => {
                    if params.is_some_and(|value| value.get("cursor").is_some()) {
                        error(id, -32602, "This fixture has no pagination cursor")
                    } else {
                        success(id, tools())
                    }
                }
                "tools/call" => self.call(id, params),
                _ => error(id, -32601, "Method not found"),
            })
        }

        fn initialize(&mut self, id: Value, params: Option<&Value>) -> Value {
            if self.phase != Phase::New {
                return error(id, -32600, "Already initialized");
            }
            let Some(params) = params else {
                return error(id, -32602, "Missing initialization parameters");
            };
            let Some(version) = params.get("protocolVersion").and_then(Value::as_str) else {
                return error(id, -32602, "Missing protocol version");
            };
            if !params.get("capabilities").is_some_and(Value::is_object)
                || !params["clientInfo"]["name"].is_string()
                || !params["clientInfo"]["version"].is_string()
            {
                return error(id, -32602, "Missing client information or capabilities");
            }
            let version = if PROTOCOL_VERSIONS.contains(&version) {
                version
            } else {
                PROTOCOL_VERSIONS[0]
            };
            self.phase = Phase::AwaitingInitialized;
            success(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": {
                        "name": "locust-stdio-probe",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            )
        }

        fn call(&self, id: Value, params: Option<&Value>) -> Value {
            let Some(params) = params else {
                return error(id, -32602, "Missing tool parameters");
            };
            if params.get("name").and_then(Value::as_str) != Some(TOOL_NAME) {
                return error(id, -32602, "Unknown tool");
            }
            if params.get("arguments").is_some_and(|arguments| {
                !arguments
                    .as_object()
                    .is_some_and(|object| object.is_empty())
            }) {
                return error(id, -32602, "This tool accepts no arguments");
            }
            let (is_error, text) = match self.roundtrip() {
                Ok(()) => (false, "Fixture socket roundtrip succeeded.".to_owned()),
                Err(message) => (true, message),
            };
            success(
                id,
                json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": is_error
                }),
            )
        }

        fn roundtrip(&self) -> Result<(), String> {
            let mut socket = UnixStream::connect(&self.config.socket)
                .map_err(|error| socket_error("connect", error))?;
            socket
                .set_read_timeout(Some(self.config.io_timeout))
                .and_then(|()| socket.set_write_timeout(Some(self.config.io_timeout)))
                .map_err(|error| socket_error("timeout configuration", error))?;
            socket
                .write_all(SOCKET_REQUEST)
                .map_err(|error| socket_error("write", error))?;
            let mut response = [0; SOCKET_RESPONSE.len()];
            socket
                .read_exact(&mut response)
                .map_err(|error| socket_error("read", error))?;
            if response != SOCKET_RESPONSE {
                return Err("Fixture socket returned an unexpected response.".to_owned());
            }
            Ok(())
        }
    }

    fn socket_error(stage: &str, error: io::Error) -> String {
        // Do not expose the configured path or arbitrary peer response bytes.
        format!("Fixture socket {stage} failed ({:?}).", error.kind())
    }

    fn success(id: Value, result: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "result": result })
    }

    fn error(id: Value, code: i32, message: &str) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
    }

    fn tools() -> Value {
        json!({ "tools": [{
            "name": TOOL_NAME,
            "description": "Perform a harmless roundtrip to the fixture socket selected by the local qualification harness.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
            "annotations": {
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false
            }
        }] })
    }

    fn serve(input: impl BufRead, mut output: impl Write, mut server: Server) -> io::Result<()> {
        for line in input.lines() {
            if let Some(response) = server.handle(&line?) {
                serde_json::to_writer(&mut output, &response)?;
                output.write_all(b"\n")?;
                output.flush()?;
            }
        }
        Ok(())
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config::parse(std::env::args_os().skip(1))?;
        serve(io::stdin().lock(), io::stdout().lock(), Server::new(config))?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixListener;
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::thread;

        struct TempSocket {
            directory: PathBuf,
        }

        impl TempSocket {
            fn new() -> Self {
                static COUNTER: AtomicU64 = AtomicU64::new(0);
                loop {
                    // A short path also fits macOS's Unix socket path limit.
                    let directory = PathBuf::from(format!(
                        "/tmp/locust-probe-{}-{}",
                        std::process::id(),
                        COUNTER.fetch_add(1, Ordering::Relaxed)
                    ));
                    match fs::create_dir(&directory) {
                        Ok(()) => {
                            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
                                .unwrap();
                            return Self { directory };
                        }
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                        Err(error) => panic!("create test directory: {error}"),
                    }
                }
            }

            fn path(&self) -> PathBuf {
                self.directory.join("fixture.sock")
            }

            fn server(&self) -> Server {
                Server::new(Config {
                    socket: self.path(),
                    io_timeout: Duration::from_secs(1),
                })
            }
        }

        impl Drop for TempSocket {
            fn drop(&mut self) {
                let _ = fs::remove_file(self.path());
                let _ = fs::remove_dir(&self.directory);
            }
        }

        fn request(server: &mut Server, value: Value) -> Value {
            server.handle(&value.to_string()).unwrap()
        }

        fn initialize(server: &mut Server, version: &str) -> Value {
            request(
                server,
                json!({ "jsonrpc": "2.0", "id": "init", "method": "initialize", "params": {
                    "protocolVersion": version,
                    "capabilities": {},
                    "clientInfo": { "name": "test", "version": "1" }
                } }),
            )
        }

        fn ready(server: &mut Server) {
            initialize(server, PROTOCOL_VERSIONS[0]);
            assert!(
                server
                    .handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                    .is_none()
            );
        }

        fn call(server: &mut Server) -> Value {
            request(
                server,
                json!({ "jsonrpc": "2.0", "id": 19, "method": "tools/call", "params": {
                    "name": TOOL_NAME, "arguments": {}
                } }),
            )
        }

        #[test]
        fn handshake_negotiates_versions_and_preserves_string_ids() {
            let fixture = TempSocket::new();
            for version in PROTOCOL_VERSIONS.iter().copied().chain(["future-version"]) {
                let mut server = fixture.server();
                let response = initialize(&mut server, version);
                assert_eq!(response["id"], "init");
                assert_eq!(
                    response["result"]["protocolVersion"],
                    if PROTOCOL_VERSIONS.contains(&version) {
                        version
                    } else {
                        PROTOCOL_VERSIONS[0]
                    }
                );
                assert_eq!(
                    response["result"]["capabilities"]["tools"]["listChanged"],
                    false
                );
            }
        }

        #[test]
        fn notifications_have_no_response_or_tool_side_effect() {
            let fixture = TempSocket::new();
            let mut server = fixture.server();
            for method in ["notifications/initialized", "tools/call", "unknown"] {
                assert!(
                    server
                        .handle(&json!({ "jsonrpc": "2.0", "method": method }).to_string())
                        .is_none()
                );
            }
            assert!(server.phase == Phase::New);
            initialize(&mut server, PROTOCOL_VERSIONS[0]);
            let response = call(&mut server);
            assert_eq!(response["id"], 19);
            assert_eq!(response["error"]["code"], -32000);
        }

        #[test]
        fn request_errors_preserve_valid_ids_without_echoing_input() {
            let fixture = TempSocket::new();
            let mut server = fixture.server();
            ready(&mut server);
            let cases = [
                (
                    json!({ "jsonrpc": "2.0", "id": "unknown", "method": "secret-method" }),
                    json!("unknown"),
                    -32601,
                ),
                (
                    json!({ "jsonrpc": "1.0", "id": 7, "method": "ping" }),
                    json!(7),
                    -32600,
                ),
                (
                    json!({ "jsonrpc": "2.0", "id": true, "method": "ping" }),
                    Value::Null,
                    -32600,
                ),
                (
                    json!({ "jsonrpc": "2.0", "id": 8, "method": "tools/call", "params": [] }),
                    json!(8),
                    -32602,
                ),
                (
                    json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": { "name": "secret-tool" } }),
                    json!(9),
                    -32602,
                ),
            ];
            for (message, id, code) in cases {
                let response = request(&mut server, message);
                assert_eq!(response["id"], id);
                assert_eq!(response["error"]["code"], code);
                assert!(!response.to_string().contains("secret"));
            }
            let response = server.handle("{").unwrap();
            assert_eq!(response["id"], Value::Null);
            assert_eq!(response["error"]["code"], -32700);
        }

        #[test]
        fn tool_listing_has_no_model_selected_socket_and_rejects_arguments() {
            let fixture = TempSocket::new();
            let mut server = fixture.server();
            ready(&mut server);
            let response = request(
                &mut server,
                json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
            );
            let tools = response["result"]["tools"].as_array().unwrap();
            assert_eq!(tools.len(), 1);
            assert_eq!(tools[0]["name"], TOOL_NAME);
            assert_eq!(tools[0]["annotations"]["readOnlyHint"], true);
            assert_eq!(tools[0]["inputSchema"]["additionalProperties"], false);
            assert_eq!(tools[0]["inputSchema"]["properties"], json!({}));
            let response = request(
                &mut server,
                json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
                    "name": TOOL_NAME, "arguments": { "socket": "/secret/socket" }
                } }),
            );
            assert_eq!(response["error"]["code"], -32602);
            assert!(!response.to_string().contains("/secret/socket"));
        }

        #[test]
        fn real_unix_socket_roundtrip_succeeds() {
            let fixture = TempSocket::new();
            let listener = UnixListener::bind(fixture.path()).unwrap();
            let peer = thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                let mut request = [0; SOCKET_REQUEST.len()];
                socket.read_exact(&mut request).unwrap();
                assert_eq!(request, SOCKET_REQUEST);
                socket.write_all(SOCKET_RESPONSE).unwrap();
            });
            let mut server = fixture.server();
            ready(&mut server);
            let response = call(&mut server);
            assert_eq!(response["id"], 19);
            assert_eq!(response["result"]["isError"], false);
            peer.join().unwrap();
        }

        #[test]
        fn refused_socket_is_a_tool_error_without_a_path() {
            let fixture = TempSocket::new();
            let listener = UnixListener::bind(fixture.path()).unwrap();
            drop(listener);
            let mut server = fixture.server();
            ready(&mut server);
            let response = call(&mut server);
            assert_eq!(response["id"], 19);
            assert_eq!(response["result"]["isError"], true);
            assert!(
                response["result"]["content"][0]["text"]
                    .as_str()
                    .unwrap()
                    .contains("connect failed")
            );
            assert!(
                !response
                    .to_string()
                    .contains(&fixture.path().display().to_string())
            );
        }

        #[test]
        fn wrong_socket_response_is_not_reported_as_success_or_echoed() {
            let fixture = TempSocket::new();
            let listener = UnixListener::bind(fixture.path()).unwrap();
            let peer = thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                let mut request = [0; SOCKET_REQUEST.len()];
                socket.read_exact(&mut request).unwrap();
                socket.write_all(&[b'x'; SOCKET_RESPONSE.len()]).unwrap();
            });
            let mut server = fixture.server();
            ready(&mut server);
            let response = call(&mut server);
            assert_eq!(response["result"]["isError"], true);
            assert!(!response.to_string().contains("xxxx"));
            peer.join().unwrap();
        }

        #[test]
        fn stdio_is_one_json_response_per_line_and_eof_exits() {
            let fixture = TempSocket::new();
            let input = concat!(
                "{\"jsonrpc\":\"2.0\",\"id\":41,\"method\":\"ping\"}\n",
                "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/cancelled\"}\n",
                "{bad json}\n"
            );
            let mut output = Vec::new();
            serve(input.as_bytes(), &mut output, fixture.server()).unwrap();
            let output = String::from_utf8(output).unwrap();
            let responses: Vec<Value> = output
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            assert_eq!(responses.len(), 2);
            assert_eq!(responses[0], success(json!(41), json!({})));
            assert_eq!(responses[1]["error"]["code"], -32700);
            assert!(output.ends_with('\n'));
        }

        #[test]
        fn trusted_cli_requires_one_socket_and_a_positive_timeout() {
            let parse = |args: &[&str]| Config::parse(args.iter().map(OsString::from));
            let config = parse(&["--io-timeout-ms", "25", "--socket", "/tmp/a"]).unwrap();
            assert_eq!(config.socket, PathBuf::from("/tmp/a"));
            assert_eq!(config.io_timeout, Duration::from_millis(25));
            for args in [
                vec![],
                vec!["--socket"],
                vec!["--socket", "/tmp/a"],
                vec!["--socket", ""],
                vec!["--socket", "/tmp/a", "--socket", "/tmp/b"],
                vec!["--socket", "/tmp/a", "--io-timeout-ms", "0"],
                vec!["--socket", "/tmp/a", "--io-timeout-ms", "-1"],
            ] {
                assert!(parse(&args).is_err());
            }
        }
    }
}

fn main() -> std::process::ExitCode {
    #[cfg(unix)]
    match unix::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("stdio_probe: {error}");
            std::process::ExitCode::FAILURE
        }
    }
    #[cfg(not(unix))]
    {
        eprintln!("stdio_probe: Unix sockets are required");
        std::process::ExitCode::FAILURE
    }
}
