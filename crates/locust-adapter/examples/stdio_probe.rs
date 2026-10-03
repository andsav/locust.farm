//! MCP fixture for isolated real-client qualification; not Locust's daemon bridge.
//!
//! Run with `--io-timeout-ms N --events-file ABS`. The bridge environment must
//! supply absolute `LOCUST_HOME`, `LOCUST_SESSION` and `LOCUST_CREDENTIAL` paths.
//! Each proof file contains exactly 32 fixture bytes with no group/other access.
//! Tools connect to `$LOCUST_HOME/daemon.sock`, send one `locust-probe-v1` JSON
//! line with an operation and the two hex proofs, and expect a matching success
//! and sequence. Only the trusted harness sets those files and socket. No real
//! daemon, account or credential belongs in this fixture.
//!
//! The read tool observes fixture state; the write tool mutates only that state;
//! the wait tool holds until the harness replies or the caller-selected I/O
//! timeout expires. Calls are sequential: interrupt/restart checks concern the
//! client process, not concurrent MCP cancellation-notification handling.
//! Socket connection establishment is blocking, so the harness also owns a
//! process deadline. The private JSONL event file contains receipt metadata only,
//! never proof values, paths, model arguments or socket response text.
//!
//! Protocol reference: <https://modelcontextprotocol.io/specification/2025-11-25>.

#![forbid(unsafe_code)]

#[cfg(unix)]
mod unix {
    use std::ffi::OsString;
    use std::fs::{File, OpenOptions};
    use std::io::{self, BufRead, BufReader, Read, Write};
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use std::os::unix::net::UnixStream;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::Duration;

    use serde_json::{Value, json};

    const TOOL_NAME: &str = "locust_probe_socket";
    const WRITE_TOOL: &str = "locust_probe_write";
    const WAIT_TOOL: &str = "locust_probe_wait";
    const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];
    const FIXTURE: &str = "locust-probe-v1";
    const USAGE: &str = "usage: stdio_probe --io-timeout-ms N --events-file ABS";

    struct Arguments {
        events: PathBuf,
        io_timeout: Duration,
    }

    impl Arguments {
        fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, &'static str> {
            let mut args = args.into_iter();
            let mut events = None;
            let mut timeout = None;
            while let Some(arg) = args.next() {
                if arg == "--events-file" && events.is_none() {
                    let path = PathBuf::from(args.next().ok_or(USAGE)?);
                    if !path.is_absolute() {
                        return Err(USAGE);
                    }
                    events = Some(path);
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
                events: events.ok_or(USAGE)?,
                io_timeout: timeout.ok_or(USAGE)?,
            })
        }
    }

    struct Config {
        socket: PathBuf,
        session: String,
        credential: String,
        io_timeout: Duration,
        events: Option<Mutex<File>>,
    }

    impl Config {
        fn from_environment(
            arguments: Arguments,
            env: impl Fn(&str) -> Option<OsString>,
        ) -> Result<Self, &'static str> {
            let path = |key| {
                let path = PathBuf::from(env(key).ok_or("missing bridge environment")?);
                if !path.is_absolute() {
                    return Err("bridge environment paths must be absolute");
                }
                Ok(path)
            };
            let home = path("LOCUST_HOME")?;
            let metadata = std::fs::metadata(&home).map_err(|_| "cannot inspect fixture home")?;
            if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
                return Err("fixture home must be a private directory");
            }
            let session = read_proof(&path("LOCUST_SESSION")?)?;
            let credential = read_proof(&path("LOCUST_CREDENTIAL")?)?;
            let events = OpenOptions::new()
                .create(true)
                .append(true)
                .mode(0o600)
                .open(arguments.events)
                .map_err(|_| "cannot open fixture event file")?;
            if events
                .metadata()
                .map_err(|_| "cannot inspect fixture event file")?
                .permissions()
                .mode()
                & 0o077
                != 0
            {
                return Err("fixture event file must be private");
            }
            let config = Self {
                socket: home.join("daemon.sock"),
                session,
                credential,
                io_timeout: arguments.io_timeout,
                events: Some(Mutex::new(events)),
            };
            config.record(json!({"event":"environment", "home":true,
                "session":true, "credential":true, "pid":std::process::id()}))?;
            Ok(config)
        }

        fn record(&self, event: Value) -> Result<(), &'static str> {
            let Some(events) = &self.events else {
                return Ok(());
            };
            let mut file = events
                .lock()
                .map_err(|_| "fixture event file lock failed")?;
            serde_json::to_writer(&mut *file, &event).map_err(|_| "fixture event write failed")?;
            file.write_all(b"\n")
                .and_then(|()| file.flush())
                .map_err(|_| "fixture event flush failed")
        }
    }

    fn read_proof(path: &Path) -> Result<String, &'static str> {
        let mut file = File::open(path).map_err(|_| "cannot open fixture proof file")?;
        let metadata = file
            .metadata()
            .map_err(|_| "cannot inspect fixture proof file")?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o077 != 0 {
            return Err("fixture proof file must be a private regular file");
        }
        let mut proof = [0; 32];
        file.read_exact(&mut proof)
            .map_err(|_| "fixture proof must contain exactly 32 bytes")?;
        if file
            .read(&mut [0])
            .map_err(|_| "cannot read fixture proof file")?
            != 0
        {
            return Err("fixture proof must contain exactly 32 bytes");
        }
        Ok(proof.iter().map(|byte| format!("{byte:02x}")).collect())
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
            // A peer response is not a request. This fixture sends no requests,
            // so it cannot match a response and must not answer one.
            if object.get("jsonrpc") == Some(&json!("2.0"))
                && !object.contains_key("method")
                && id.is_some()
                && (object.contains_key("result") || object.contains_key("error"))
            {
                return None;
            }
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
                        match self.config.record(
                            json!({"event":"tools/list", "phase":"complete", "success":true}),
                        ) {
                            Ok(()) => success(id, tools()),
                            Err(message) => error(id, -32603, message),
                        }
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
            if let Err(message) = self
                .config
                .record(json!({"event":"initialize", "phase":"complete", "success":true}))
            {
                return error(id, -32603, message);
            }
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
            let name = params.get("name").and_then(Value::as_str);
            let operation = match name {
                Some(TOOL_NAME) => "read",
                Some(WRITE_TOOL) => "write",
                Some(WAIT_TOOL) => "wait",
                _ => return error(id, -32602, "Unknown tool"),
            };
            if params.get("arguments").is_some_and(|arguments| {
                !arguments
                    .as_object()
                    .is_some_and(|object| object.is_empty())
            }) {
                return error(id, -32602, "This tool accepts no arguments");
            }
            if let Err(message) = self
                .config
                .record(json!({"event":"tools/call", "phase":"start", "tool":name}))
            {
                return error(id, -32603, message);
            }
            let result = self.roundtrip(operation);
            if let Err(message) = self.config.record(json!({"event":"tools/call", "phase":"complete", "tool":name, "success":result.is_ok()})) {
                return error(id, -32603, message);
            }
            let (is_error, text) = match result {
                Ok(sequence) => (
                    false,
                    format!("Fixture {operation} succeeded; sequence {sequence}."),
                ),
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

        fn roundtrip(&self, operation: &str) -> Result<u64, String> {
            let mut socket = UnixStream::connect(&self.config.socket)
                .map_err(|error| socket_error("connect", error))?;
            socket
                .set_read_timeout(Some(self.config.io_timeout))
                .and_then(|()| socket.set_write_timeout(Some(self.config.io_timeout)))
                .map_err(|error| socket_error("timeout configuration", error))?;
            let request = json!({"fixture":FIXTURE, "operation":operation,
                "session":self.config.session, "credential":self.config.credential});
            serde_json::to_writer(&mut socket, &request)
                .map_err(|_| "Fixture socket request failed.".to_owned())?;
            socket
                .write_all(b"\n")
                .map_err(|error| socket_error("write", error))?;
            let mut response = String::new();
            BufReader::new(socket)
                .read_line(&mut response)
                .map_err(|error| socket_error("read", error))?;
            let value: Value = serde_json::from_str(&response)
                .map_err(|_| "Fixture socket returned an unexpected response.".to_owned())?;
            if value["fixture"] != FIXTURE || value["ok"] != true {
                return Err("Fixture socket refused the request.".to_owned());
            }
            value["sequence"]
                .as_u64()
                .ok_or_else(|| "Fixture socket returned an invalid sequence.".to_owned())
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
        let tools: Vec<_> = [
            (TOOL_NAME, "Read the isolated qualification fixture state.", true, true),
            (WRITE_TOOL, "Increment the isolated qualification fixture counter; changes fixture state only.", false, false),
            (WAIT_TOOL, "Wait for the qualification harness to release a fixture response.", true, true),
        ].into_iter().map(|(name, description, read_only, idempotent)| json!({
            "name": name,
            "description": description,
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
            "annotations": {
                "readOnlyHint": read_only,
                "destructiveHint": false,
                "idempotentHint": idempotent,
                "openWorldHint": false
            }
        })).collect();
        json!({"tools":tools})
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
        let arguments = Arguments::parse(std::env::args_os().skip(1))?;
        let config = Config::from_environment(arguments, |name| std::env::var_os(name))?;
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
                self.directory.join("daemon.sock")
            }

            fn server(&self) -> Server {
                Server::new(Config {
                    socket: self.path(),
                    io_timeout: Duration::from_secs(1),
                    session: "11".repeat(32),
                    credential: "22".repeat(32),
                    events: None,
                })
            }
        }

        impl Drop for TempSocket {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.directory);
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
            assert_eq!(tools.len(), 3);
            assert_eq!(tools[1]["name"], WRITE_TOOL);
            assert_eq!(tools[1]["annotations"]["readOnlyHint"], false);
            assert_eq!(tools[1]["annotations"]["idempotentHint"], false);
            assert_eq!(tools[2]["name"], WAIT_TOOL);
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
                let mut line = String::new();
                BufReader::new(&mut socket).read_line(&mut line).unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                assert_eq!(
                    request,
                    json!({"fixture":FIXTURE, "operation":"read",
                    "session":"11".repeat(32), "credential":"22".repeat(32)})
                );
                writeln!(
                    socket,
                    "{}",
                    json!({"fixture":FIXTURE,"ok":true,"sequence":0})
                )
                .unwrap();
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
                let mut line = String::new();
                BufReader::new(&mut socket).read_line(&mut line).unwrap();
                socket.write_all(b"xxxx\n").unwrap();
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

        fn private_file(path: &Path, bytes: &[u8]) {
            OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(path)
                .unwrap()
                .write_all(bytes)
                .unwrap();
        }

        fn environment_config(fixture: &TempSocket) -> Result<Config, &'static str> {
            Config::from_environment(
                Arguments {
                    events: fixture.directory.join("events.jsonl"),
                    io_timeout: Duration::from_millis(25),
                },
                |key| {
                    Some(match key {
                        "LOCUST_HOME" => fixture.directory.clone().into_os_string(),
                        "LOCUST_SESSION" => fixture.directory.join("session").into_os_string(),
                        "LOCUST_CREDENTIAL" => {
                            fixture.directory.join("credential").into_os_string()
                        }
                        _ => unreachable!(),
                    })
                },
            )
        }

        #[test]
        fn environment_paths_and_proof_files_are_required_and_private() {
            let fixture = TempSocket::new();
            assert!(environment_config(&fixture).is_err());
            let session = fixture.directory.join("session");
            let credential = fixture.directory.join("credential");
            private_file(&session, &[17; 32]);
            private_file(&credential, &[34; 32]);
            let config = environment_config(&fixture).unwrap();
            assert_eq!(config.socket, fixture.path());
            assert_eq!(config.session, "11".repeat(32));
            assert_eq!(config.credential, "22".repeat(32));
            fs::set_permissions(&session, fs::Permissions::from_mode(0o644)).unwrap();
            assert_eq!(
                environment_config(&fixture).err(),
                Some("fixture proof file must be a private regular file")
            );
            fs::set_permissions(&session, fs::Permissions::from_mode(0o600)).unwrap();
            for length in [31, 33] {
                fs::write(&session, vec![17; length]).unwrap();
                assert_eq!(
                    environment_config(&fixture).err(),
                    Some("fixture proof must contain exactly 32 bytes")
                );
            }
            assert!(
                Config::from_environment(
                    Arguments {
                        events: fixture.directory.join("events.jsonl"),
                        io_timeout: Duration::from_millis(25),
                    },
                    |_| None
                )
                .is_err()
            );
            assert!(
                Config::from_environment(
                    Arguments {
                        events: fixture.directory.join("events.jsonl"),
                        io_timeout: Duration::from_millis(25),
                    },
                    |_| Some("relative".into())
                )
                .is_err()
            );
        }

        #[test]
        fn read_write_wait_roundtrips_and_receipts_do_not_expose_proofs() {
            let fixture = TempSocket::new();
            private_file(&fixture.directory.join("session"), &[17; 32]);
            private_file(&fixture.directory.join("credential"), &[34; 32]);
            let mut server = Server::new(environment_config(&fixture).unwrap());
            let listener = UnixListener::bind(fixture.path()).unwrap();
            let peer = thread::spawn(move || {
                for (operation, sequence) in [("read", 0), ("write", 1), ("wait", 1)] {
                    let (mut socket, _) = listener.accept().unwrap();
                    let mut line = String::new();
                    BufReader::new(&mut socket).read_line(&mut line).unwrap();
                    let request: Value = serde_json::from_str(&line).unwrap();
                    assert_eq!(request["operation"], operation);
                    assert_eq!(request["session"], "11".repeat(32));
                    assert_eq!(request["credential"], "22".repeat(32));
                    writeln!(
                        socket,
                        "{}",
                        json!({"fixture":FIXTURE,"ok":true,"sequence":sequence})
                    )
                    .unwrap();
                }
            });
            ready(&mut server);
            for tool in [TOOL_NAME, WRITE_TOOL, WAIT_TOOL] {
                let response = request(
                    &mut server,
                    json!({"jsonrpc":"2.0","id":1,
                    "method":"tools/call", "params":{"name":tool,"arguments":{}}}),
                );
                assert_eq!(response["result"]["isError"], false);
                assert!(!response.to_string().contains(&"11".repeat(32)));
                assert!(!response.to_string().contains(&"22".repeat(32)));
            }
            peer.join().unwrap();
            let raw = fs::read_to_string(fixture.directory.join("events.jsonl")).unwrap();
            assert!(!raw.contains(&fixture.directory.display().to_string()));
            assert!(!raw.contains(&"11".repeat(32)));
            assert!(!raw.contains(&"22".repeat(32)));
            let events: Vec<Value> = raw
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            assert_eq!(events[0]["event"], "environment");
            assert!(events[0]["pid"].as_u64().unwrap() > 0);
            let completed: Vec<_> = events
                .iter()
                .filter(|event| {
                    event["event"] == "tools/call"
                        && event["phase"] == "complete"
                        && event["success"] == true
                })
                .collect();
            assert_eq!(completed.len(), 3);
        }

        #[test]
        fn held_wait_honors_the_operator_io_timeout() {
            let fixture = TempSocket::new();
            let listener = UnixListener::bind(fixture.path()).unwrap();
            let (release, released) = std::sync::mpsc::channel();
            let peer = thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(&mut socket).read_line(&mut line).unwrap();
                let value: Value = serde_json::from_str(&line).unwrap();
                assert_eq!(value["operation"], "wait");
                released.recv().unwrap();
            });
            let mut server = fixture.server();
            server.config.io_timeout = Duration::from_millis(25);
            ready(&mut server);
            let response = request(
                &mut server,
                json!({"jsonrpc":"2.0","id":1,
                "method":"tools/call", "params":{"name":WAIT_TOOL,"arguments":{}}}),
            );
            release.send(()).unwrap();
            peer.join().unwrap();
            assert_eq!(response["result"]["isError"], true);
            assert!(
                response["result"]["content"][0]["text"]
                    .as_str()
                    .unwrap()
                    .contains("read failed")
            );
        }

        #[test]
        fn json_rpc_responses_are_never_answered() {
            let fixture = TempSocket::new();
            let mut server = fixture.server();
            for value in [
                json!({"jsonrpc":"2.0","id":1,"result":{}}),
                json!({"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"untrusted"}}),
            ] {
                assert!(server.handle(&value.to_string()).is_none());
            }
        }

        #[test]
        fn trusted_cli_requires_private_events_and_positive_timeout() {
            let parse = |args: &[&str]| Arguments::parse(args.iter().map(OsString::from));
            let config = parse(&["--io-timeout-ms", "25", "--events-file", "/tmp/a"]).unwrap();
            assert_eq!(config.events, PathBuf::from("/tmp/a"));
            assert_eq!(config.io_timeout, Duration::from_millis(25));
            for args in [
                vec![],
                vec!["--events-file"],
                vec!["--events-file", "/tmp/a"],
                vec!["--events-file", "relative", "--io-timeout-ms", "1"],
                vec![
                    "--events-file",
                    "/tmp/a",
                    "--events-file",
                    "/tmp/b",
                    "--io-timeout-ms",
                    "1",
                ],
                vec!["--events-file", "/tmp/a", "--io-timeout-ms", "0"],
                vec!["--events-file", "/tmp/a", "--io-timeout-ms", "-1"],
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
