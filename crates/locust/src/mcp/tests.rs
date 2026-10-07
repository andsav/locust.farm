use super::*;
use locust_proto::api::Response;
use locust_proto::api::{ApiError, ClientHello, RequestFrame, ResponseFrame, ServerHello};
use locust_proto::{API_VERSION, codec};
use std::os::unix::net::UnixListener;
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

fn auth(socket: PathBuf) -> Arc<Authentication> {
    Arc::new(Authentication {
        home: socket.parent().unwrap().to_owned(),
        socket,
        credential: Credential([1; 32]),
        session: Some(SessionSecret([2; 32])),
    })
}
/// Refusal 2 of mockup P5-2: the agent's level is ask and the task needs auto.
fn refused() -> locust_proto::api::Refused {
    use locust_proto::api::{Act, Level, Refused, Why};
    use locust_proto::id::{EventId, GoalId, PublicKey};
    Refused {
        agent: PublicKey([3; 32]),
        agent_name: "codex-maple-1a2b3c4d".into(),
        member_name: Some("Maple".into()),
        goal: Some(GoalId([1; 32])),
        goal_title: Some("Static site search".into()),
        act: Act::TakeTask,
        task: Some(locust_proto::event::TaskId::Authored(EventId([4; 32]))),
        task_title: Some("Fix the parser".into()),
        why: Why::YourSetting {
            level: Level::Ask,
            needs: Level::Auto,
        },
    }
}
fn fake(
    socket: &std::path::Path,
    owner: bool,
    block: bool,
) -> (std::thread::JoinHandle<()>, std::sync::mpsc::Receiver<()>) {
    let refused = refused();
    fake_response(
        socket,
        owner,
        block,
        Err(ApiError {
            code: ErrorCode::LevelRequired,
            message: locust_proto::api::render(&refused, locust_proto::api::Voice::Agent),
            details_json: Some(serde_json::to_string(&refused).unwrap()),
        }),
    )
}
fn fake_response(
    socket: &std::path::Path,
    owner: bool,
    block: bool,
    result: Result<Response, ApiError>,
) -> (std::thread::JoinHandle<()>, std::sync::mpsc::Receiver<()>) {
    let listener = UnixListener::bind(socket).unwrap();
    let (ready, received) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let hello: ClientHello =
            codec::decode(&codec::read_frame(&mut stream, 4096).unwrap().unwrap()).unwrap();
        assert_eq!(hello.credential, Credential([1; 32]));
        assert_eq!(hello.session, Some(SessionSecret([2; 32])));
        let welcome = ServerHello::Welcome {
            api_version: API_VERSION,
            daemon_version: "test".into(),
            caller: if owner {
                Caller::Owner
            } else {
                Caller::Agent(locust_proto::id::PublicKey([3; 32]))
            },
            max_blob_bytes: 1024,
        };
        codec::write_frame(&mut stream, &codec::encode(&welcome).unwrap()).unwrap();
        if owner {
            assert!(codec::read_frame(&mut stream, 4096).unwrap().is_none());
            return;
        }
        let frame: RequestFrame =
            codec::decode(&codec::read_frame(&mut stream, 4096).unwrap().unwrap()).unwrap();
        let _ = ready.send(());
        if block {
            assert!(codec::read_frame(&mut stream, 4096).unwrap().is_none());
            return;
        }
        let response = ResponseFrame {
            id: frame.id,
            result,
        };
        codec::write_frame(&mut stream, &codec::encode(&response).unwrap()).unwrap();
    });
    (thread, received)
}
/// Accepts one bridge connection and welcomes it as `caller`.
fn welcome(listener: &UnixListener, caller: Caller, session: Option<SessionSecret>) -> UnixStream {
    let (mut stream, _) = listener.accept().unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let hello: ClientHello =
        codec::decode(&codec::read_frame(&mut stream, 4096).unwrap().unwrap()).unwrap();
    assert_eq!(hello.credential, Credential([1; 32]));
    assert_eq!(hello.session, session);
    let welcome = ServerHello::Welcome {
        api_version: API_VERSION,
        daemon_version: "test".into(),
        caller,
        max_blob_bytes: 1024,
    };
    codec::write_frame(&mut stream, &codec::encode(&welcome).unwrap()).unwrap();
    stream
}
fn names(answer: &Value) -> Vec<&str> {
    answer["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect()
}
async fn send(writer: &mut (impl AsyncWrite + Unpin), value: Value) {
    writer
        .write_all(format!("{value}\n").as_bytes())
        .await
        .unwrap();
}
async fn receive(reader: &mut (impl tokio::io::AsyncBufRead + Unpin)) -> Value {
    let mut line = String::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        reader.read_line(&mut line),
    )
    .await
    .unwrap()
    .unwrap();
    serde_json::from_str(&line).unwrap()
}
async fn initialize(
    writer: &mut (impl AsyncWrite + Unpin),
    reader: &mut (impl tokio::io::AsyncBufRead + Unpin),
    version: &str,
) {
    send(writer,json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":version,"capabilities":{},"clientInfo":{"name":"test","version":"1"}}})).await;
    assert_eq!(receive(reader).await["result"]["protocolVersion"], version);
    send(
        writer,
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    )
    .await;
}
#[tokio::test]
async fn authenticated_daemon_errors_are_structured_tool_errors() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("s");
    let (daemon, _) = fake(&socket, false, false);
    let (input, mut writer) = tokio::io::duplex(8192);
    let (output, reader) = tokio::io::duplex(8192);
    let mut reader = BufReader::new(reader);
    let bridge = tokio::spawn(serve(input, output, auth(socket), None));
    initialize(&mut writer, &mut reader, VERSIONS[0]).await;
    send(
        &mut writer,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"locust_status"}}),
    )
    .await;
    let answer = receive(&mut reader).await;
    assert_eq!(answer["result"]["isError"], true);
    let error = &answer["result"]["structuredContent"]["error"];
    assert_eq!(error["code"], "level_required");
    // Mockup P5-3: the message names the agent by its local name and says
    // "this task"; the titles and the name in the goal are only in details.
    let message = error["message"].as_str().unwrap();
    assert_eq!(
        message,
        "codex-maple-1a2b3c4d can't take this task in this goal: codex-maple-1a2b3c4d's level here is ask (set by codex-maple-1a2b3c4d's owner). codex-maple-1a2b3c4d's owner can allow this task or set codex-maple-1a2b3c4d to auto."
    );
    assert!(!message.contains("Fix the parser"));
    assert!(!message.contains("Maple "));
    assert_eq!(error["details"]["why"]["side"], "your_setting");
    assert_eq!(error["details"]["task_title"], "Fix the parser");
    assert_eq!(error["details"]["member_name"], "Maple");
    assert_eq!(
        answer["result"]["content"][0]["text"],
        serde_json::to_string(&answer["result"]["structuredContent"]).unwrap()
    );
    drop(writer);
    bridge.await.unwrap().unwrap();
    daemon.join().unwrap();
}
#[tokio::test]
async fn owner_credentials_never_dispatch_a_model_operation() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("s");
    let (daemon, _) = fake(&socket, true, false);
    let result = invoke(
        auth(socket),
        Call {
            request: ClientRequest::Native(Box::new(Request::Status)),
            idempotency: None,
        },
        Arc::new(Cancellation::new()),
    )
    .await;
    assert_eq!(result.unwrap_err().code, ErrorCode::Denied);
    daemon.join().unwrap();
}
#[tokio::test]
async fn cancellation_and_eof_join_a_blocked_socket_worker() {
    for explicit in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("s");
        let (daemon, ready) = fake(&socket, false, true);
        let (input, mut writer) = tokio::io::duplex(8192);
        let (output, reader) = tokio::io::duplex(8192);
        let mut reader = BufReader::new(reader);
        let bridge = tokio::spawn(serve(input, output, auth(socket), None));
        initialize(&mut writer, &mut reader, VERSIONS[0]).await;
        send(
            &mut writer,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"locust_wait","arguments":{"goal":"01".repeat(32),"seen":0,"timeout_ms":u32::MAX}}}),
        )
        .await;
        send(&mut writer,json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"locust_wait","arguments":{"goal":"01".repeat(32),"seen":0,"timeout_ms":u32::MAX}}})).await;
        // Ping ordering proves the call was admitted; the server waits for EOF.
        send(&mut writer, json!({"jsonrpc":"2.0","id":3,"method":"ping"})).await;
        assert_eq!(receive(&mut reader).await["id"], 3);
        tokio::task::spawn_blocking(move || {
            ready
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
        })
        .await
        .unwrap();
        if explicit {
            send(&mut writer,json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":2}})).await;
        }
        drop(writer);
        tokio::time::timeout(std::time::Duration::from_secs(5), bridge)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        daemon.join().unwrap();
    }
}
#[test]
fn malformed_or_forbidden_tool_calls_are_protocol_errors() {
    for value in [
        json!({"name":"locust_daemon_stop"}),
        json!({"name":"locust_blob_get"}),
        json!({"name":"locust_status","arguments":[]}),
    ] {
        assert!(matches!(parse_call(&value), Err(CallError::Protocol(_))));
    }
    assert!(matches!(
        parse_call(&json!({"name":"locust_status","arguments":{"unexpected":1}})),
        Err(CallError::Arguments(_))
    ));
}

/// The `locust_` names in `text`: what a model would try to call.
fn tool_names(text: &str) -> Vec<&str> {
    text.match_indices("locust_")
        .map(|(start, _)| {
            let name = &text[start..];
            let end = name
                .find(|c: char| !(c.is_ascii_lowercase() || c == '_'))
                .unwrap_or(name.len());
            &name[..end]
        })
        .collect()
}
fn strings(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::String(text) => found.push(text.clone()),
        Value::Array(values) => values.iter().for_each(|value| strings(value, found)),
        Value::Object(fields) => fields.values().for_each(|value| strings(value, found)),
        _ => {}
    }
}
#[test]
fn strings_written_for_a_model_name_listed_tools_and_no_operation() {
    let tools = schema::tools(Caller::Agent(locust_proto::id::PublicKey([3; 32])));
    let listed: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    for name in [
        "locust_context_read",
        "locust_pending_page",
        "locust_contribution_publish",
    ] {
        assert!(tool_names(INSTRUCTIONS).contains(&name), "{name}");
    }

    // Everything the bridge itself writes: the instructions, the listing,
    // and its own argument and receipt-reference errors, plus the packaged
    // skill a model reads beside them. Daemon answers are not written here.
    let mut written = vec![
        INSTRUCTIONS.to_owned(),
        include_str!("../../../../skills/locust/SKILL.md").to_owned(),
    ];
    for tool in &tools {
        strings(tool, &mut written);
    }
    for name in &listed {
        match parse_call(&json!({"name": name, "arguments": {"no_such_argument": 1}})) {
            Err(CallError::Arguments(failure)) => written.push(failure.message),
            _ => panic!("{name} accepted an unknown argument"),
        }
    }
    let home = tempfile::tempdir().unwrap();
    fs::set_permissions(home.path(), fs::Permissions::from_mode(0o700)).unwrap();
    for session in [None, Some(SessionSecret([2; 32]))] {
        let cache = Cache::new(home.path(), Credential([1; 32]), session, Surface::Mcp);
        for reference in ["ctx:typo".to_owned(), format!("ctx:{}", "0".repeat(64))] {
            written.push(cache.load(&reference).unwrap_err().message);
        }
    }

    for text in &written {
        for name in tool_names(text) {
            assert!(listed.contains(&name), "{name} is not a tool: {text}");
        }
        for operation in OPERATIONS.iter().filter(|op| op.name.contains('.')) {
            assert!(
                !text.contains(operation.name),
                "operation name {} in: {text}",
                operation.name
            );
        }
    }
    // Every operation an agent or author can call says what it does, not
    // just its name: the summary is the tool description and the command's
    // help.
    for operation in OPERATIONS.iter().filter(|op| {
        matches!(
            op.audience,
            locust_proto::api::Audience::Agent | locust_proto::api::Audience::Author
        )
    }) {
        let bare = operation.name.replace(['.', '_'], " ");
        assert!(
            !operation.summary.eq_ignore_ascii_case(&bare)
                && !operation.summary.eq_ignore_ascii_case(operation.name),
            "{} has only its name as its summary",
            operation.name
        );
    }
    // The old words are gone from the instructions and every description.
    let descriptions: Vec<String> = std::iter::once(INSTRUCTIONS.to_owned())
        .chain(
            tools
                .iter()
                .map(|tool| tool["description"].as_str().unwrap().to_owned()),
        )
        .collect();
    for text in &descriptions {
        let lower = text.to_lowercase();
        for word in [
            "grant",
            "authoriz",
            "administrator",
            "participant",
            "principal",
            "viewer",
        ] {
            assert!(!lower.contains(word), "{word} in: {text}");
        }
    }
    let description = |name: &str| {
        tools
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap_or_else(|| panic!("{name} is listed"))["description"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let attended = "Tasks to start come least-attended first, each with the members already attempting it; results to review carry the approvals so far and the number needed.";
    assert!(description("locust_pending").ends_with(attended));
    assert!(description("locust_wait").ends_with(attended));
    let review = description("locust_review_record");
    assert!(review.contains("A member's latest review of a result is the one that counts."));
    assert!(review.contains(
        "A pick, a plan text or a file change already recorded on an earlier approval is not undone."
    ));
}

#[tokio::test]
async fn broken_output_cancels_and_joins_an_outstanding_call() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("s");
    let (daemon, ready) = fake(&socket, false, true);
    let (input, mut writer) = tokio::io::duplex(8192);
    let (output, reader) = tokio::io::duplex(8192);
    let mut reader = BufReader::new(reader);
    let bridge = tokio::spawn(serve(input, output, auth(socket), None));
    initialize(&mut writer, &mut reader, VERSIONS[0]).await;
    send(
        &mut writer,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"locust_status"}}),
    )
    .await;
    tokio::task::spawn_blocking(move || {
        ready
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
    })
    .await
    .unwrap();
    drop(reader);
    send(&mut writer, json!({"jsonrpc":"2.0","id":3,"method":"ping"})).await;
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), bridge)
        .await
        .unwrap()
        .unwrap();
    assert!(result.is_err());
    daemon.join().unwrap();
}

#[tokio::test]
async fn lifecycle_negotiates_versions_and_requires_initialized() {
    for version in VERSIONS {
        let (input, mut writer) = tokio::io::duplex(8192);
        let (output, reader) = tokio::io::duplex(8192);
        let mut reader = BufReader::new(reader);
        let bridge = tokio::spawn(serve(input, output, auth(PathBuf::from("/missing")), None));
        send(
            &mut writer,
            json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}),
        )
        .await;
        assert_eq!(receive(&mut reader).await["error"]["code"], -32002);
        initialize(&mut writer, &mut reader, version).await;
        send(&mut writer, json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"locust_blob_put"}})).await;
        assert_eq!(receive(&mut reader).await["error"]["code"], -32602);
        // No daemon answers at this socket, so no caller is known to list for.
        send(
            &mut writer,
            json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}),
        )
        .await;
        let answer = receive(&mut reader).await;
        assert_eq!(answer["error"]["code"], -32000);
        assert_eq!(answer["error"]["data"]["code"], "unavailable");
        drop(writer);
        bridge.await.unwrap().unwrap();
    }
}

#[test]
fn pre_cancelled_worker_and_nested_invalid_arguments_are_rejected() {
    let cancellation = Cancellation::new();
    cancellation.cancel();
    let (stream, _) = UnixStream::pair().unwrap();
    assert!(cancellation.install(&stream).is_err());
    assert!(matches!(
        parse_call(
            &json!({"name":"locust_task_open","arguments":{"goal":"01".repeat(32),"text":"task","inputs":{"source":{"shell":"rm"}}}})
        ),
        Err(CallError::Arguments(_))
    ));
    assert!(
        tool_result(Err(Failure::invalid("invalid")), "2025-03-26")
            .get("structuredContent")
            .is_none()
    );
}

#[tokio::test]
async fn partial_frames_survive_select_cancellation_and_missing_newlines_fail() {
    let (input, mut writer) = tokio::io::duplex(128);
    let mut input = BufReader::new(input);
    let mut buffer = Vec::new();
    writer.write_all(b"{\"jsonrpc\":").await.unwrap();
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(10),
            read_line(&mut input, &mut buffer)
        )
        .await
        .is_err()
    );
    writer.write_all(b"\"2.0\"}\n").await.unwrap();
    assert_eq!(
        read_line(&mut input, &mut buffer).await.unwrap().unwrap(),
        "{\"jsonrpc\":\"2.0\"}\n"
    );
    writer.write_all(b"{}").await.unwrap();
    drop(writer);
    assert_eq!(
        read_line(&mut input, &mut buffer).await.unwrap_err().kind(),
        io::ErrorKind::UnexpectedEof
    );
}

#[tokio::test]
async fn nonblocking_fd_reads_writes_and_restores_flags() {
    use tokio::io::AsyncReadExt;
    let (a, mut b) = UnixStream::pair().unwrap();
    let flags = rustix::fs::fcntl_getfl(&a).unwrap();
    let mut pipe = stdio::Pipe::new(&a).unwrap();
    assert!(
        rustix::fs::fcntl_getfl(&a)
            .unwrap()
            .contains(rustix::fs::OFlags::NONBLOCK)
    );
    std::io::Write::write_all(&mut b, b"test").unwrap();
    let mut bytes = [0; 4];
    pipe.read_exact(&mut bytes).await.unwrap();
    assert_eq!(&bytes, b"test");
    pipe.write_all(b"reply").await.unwrap();
    let mut answer = [0; 5];
    std::io::Read::read_exact(&mut b, &mut answer).unwrap();
    assert_eq!(&answer, b"reply");
    drop(pipe);
    assert_eq!(
        rustix::fs::fcntl_getfl(&a)
            .unwrap()
            .contains(rustix::fs::OFlags::NONBLOCK),
        flags.contains(rustix::fs::OFlags::NONBLOCK)
    );
}

#[tokio::test]
async fn authenticated_list_advertises_only_registry_tools() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("s");
    let agent = Caller::Agent(locust_proto::id::PublicKey([3; 32]));
    let listener = UnixListener::bind(&socket).unwrap();
    let daemon = std::thread::spawn(move || {
        let mut stream = welcome(&listener, agent, Some(SessionSecret([2; 32])));
        assert!(
            codec::read_frame(&mut stream, 4096).unwrap().is_none(),
            "the hello authenticates a listing; no operation follows it"
        );
    });
    let (input, mut writer) = tokio::io::duplex(8192);
    let (output, reader) = tokio::io::duplex(8192);
    let mut reader = BufReader::new(reader);
    let bridge = tokio::spawn(serve(input, output, auth(socket), None));
    initialize(&mut writer, &mut reader, VERSIONS[0]).await;
    send(
        &mut writer,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    )
    .await;
    let answer = receive(&mut reader).await;
    assert_eq!(answer["result"]["tools"], json!(schema::tools(agent)));
    let registry: Vec<String> = OPERATIONS
        .iter()
        .filter(|operation| operation.tool)
        .map(|operation| operation.tool_name())
        .collect();
    assert_eq!(names(&answer), registry);
    drop(writer);
    bridge.await.unwrap().unwrap();
    daemon.join().unwrap();
}

#[tokio::test]
async fn unknown_receipt_reference_is_not_found_and_names_the_read_tool() {
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let socket = dir.path().join("s");
    let listener = UnixListener::bind(&socket).unwrap();
    let daemon = std::thread::spawn(move || {
        let agent = Caller::Agent(locust_proto::id::PublicKey([3; 32]));
        let mut stream = welcome(&listener, agent, Some(SessionSecret([2; 32])));
        assert!(
            codec::read_frame(&mut stream, 4096).unwrap().is_none(),
            "an unresolved reference acknowledges nothing"
        );
    });
    let (input, mut writer) = tokio::io::duplex(8192);
    let (output, reader) = tokio::io::duplex(8192);
    let mut reader = BufReader::new(reader);
    let bridge = tokio::spawn(serve(input, output, auth(socket), None));
    initialize(&mut writer, &mut reader, VERSIONS[0]).await;
    send(&mut writer,json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"locust_context_acknowledge","arguments":{"goal":"01".repeat(32),"receipt":format!("ctx:{}", "0".repeat(64))}}})).await;
    let answer = receive(&mut reader).await;
    assert_eq!(answer["result"]["isError"], true);
    let error = &answer["result"]["structuredContent"]["error"];
    assert_eq!(error["code"], "not_found");
    let message = error["message"].as_str().unwrap();
    assert!(
        message.contains("Read context again with locust_context_read"),
        "{message}"
    );
    assert!(!message.contains("os error"), "{message}");
    drop(writer);
    bridge.await.unwrap().unwrap();
    daemon.join().unwrap();
}

#[tokio::test]
async fn owner_credential_is_refused_a_tool_list() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("s");
    let (daemon, _) = fake(&socket, true, false);
    let (input, mut writer) = tokio::io::duplex(8192);
    let (output, reader) = tokio::io::duplex(8192);
    let mut reader = BufReader::new(reader);
    let bridge = tokio::spawn(serve(input, output, auth(socket), None));
    initialize(&mut writer, &mut reader, VERSIONS[0]).await;
    send(
        &mut writer,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    )
    .await;
    let answer = receive(&mut reader).await;
    assert!(answer.get("result").is_none(), "{answer}");
    assert_eq!(answer["error"]["code"], -32000);
    assert_eq!(answer["error"]["data"]["code"], "denied");
    drop(writer);
    bridge.await.unwrap().unwrap();
    daemon.join().unwrap();
}

#[tokio::test]
async fn cancellation_notification_ends_wait_while_bridge_remains_available() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("s");
    let (daemon, ready) = fake(&socket, false, true);
    let (input, mut writer) = tokio::io::duplex(8192);
    let (output, reader) = tokio::io::duplex(8192);
    let mut reader = BufReader::new(reader);
    let bridge = tokio::spawn(serve(input, output, auth(socket), None));
    initialize(&mut writer, &mut reader, VERSIONS[0]).await;
    send(&mut writer,json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"locust_wait","arguments":{"goal":"01".repeat(32),"seen":0,"timeout_ms":u32::MAX}}})).await;
    tokio::task::spawn_blocking(move || {
        ready
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
    })
    .await
    .unwrap();
    send(
        &mut writer,
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":2}}),
    )
    .await;
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(move || daemon.join().unwrap()),
    )
    .await
    .unwrap()
    .unwrap();
    send(&mut writer, json!({"jsonrpc":"2.0","id":3,"method":"ping"})).await;
    assert_eq!(
        receive(&mut reader).await["id"],
        3,
        "cancelled wait must not emit a response"
    );
    drop(writer);
    bridge.await.unwrap().unwrap();
}

#[test]
fn a_cancelled_queued_blocking_worker_never_starts_a_handshake() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("s");
    let listener = UnixListener::bind(&socket).unwrap();
    let daemon = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        assert!(
            codec::read_frame(&mut stream, 4096).unwrap().is_none(),
            "queued cancelled call sent a handshake"
        );
    });
    runtime.block_on(async {
        let (started, ready) = std::sync::mpsc::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        let holder = tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            blocked
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
        });
        ready
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let cancellation = Arc::new(Cancellation::new());
        let call = tokio::spawn(invoke(
            auth(socket),
            Call {
                request: ClientRequest::Native(Box::new(Request::Status)),
                idempotency: None,
            },
            cancellation.clone(),
        ));
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while cancellation.socket.lock().unwrap().is_none() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        cancellation.cancel();
        release.send(()).unwrap();
        holder.await.unwrap();
        assert_eq!(
            call.await.unwrap().unwrap_err().code,
            ErrorCode::Unavailable
        );
    });
    daemon.join().unwrap();
}

#[tokio::test]
async fn private_author_lists_then_uses_its_catalog_without_an_execution_session() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("author-socket");
    let listener = UnixListener::bind(&socket).unwrap();
    let daemon = std::thread::spawn(move || {
        let author = Caller::Author(locust_proto::id::PublicKey([4; 32]));
        // An author may not call `status` or any other goal operation, so a
        // listing that asked one would be denied before any tool was known.
        let mut stream = welcome(&listener, author, None);
        assert!(
            codec::read_frame(&mut stream, 4096).unwrap().is_none(),
            "the hello authenticates a listing; no operation follows it"
        );
        let mut stream = welcome(&listener, author, None);
        let frame: RequestFrame =
            codec::decode(&codec::read_frame(&mut stream, 4096).unwrap().unwrap()).unwrap();
        assert_eq!(frame.request, Request::FormationDrafts);
        assert_eq!(frame.on_behalf, None);
        codec::write_frame(
            &mut stream,
            &codec::encode(&ResponseFrame {
                id: frame.id,
                result: Ok(Response::FormationDrafts(vec![])),
            })
            .unwrap(),
        )
        .unwrap();
    });
    let (input, mut writer) = tokio::io::duplex(8192);
    let (output, reader) = tokio::io::duplex(8192);
    let mut reader = BufReader::new(reader);
    let authentication = Arc::new(Authentication {
        home: socket.parent().unwrap().to_owned(),
        socket,
        credential: Credential([1; 32]),
        session: None,
    });
    let bridge = tokio::spawn(serve(input, output, authentication, None));
    initialize(&mut writer, &mut reader, VERSIONS[0]).await;
    send(
        &mut writer,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    )
    .await;
    let listed = receive(&mut reader).await;
    let listed = names(&listed);
    assert!(listed.contains(&"locust_formation_drafts"), "{listed:?}");
    assert!(
        listed
            .iter()
            .all(|name| name.starts_with("locust_formation_")),
        "{listed:?}"
    );
    send(&mut writer,json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"locust_formation_drafts"}})).await;
    let answer = receive(&mut reader).await;
    assert_eq!(answer["result"]["structuredContent"]["ok"], true);
    drop(writer);
    bridge.await.unwrap().unwrap();
    daemon.join().unwrap();
}
