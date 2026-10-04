use super::*;
use locust_proto::api::{ApiError, ClientHello, RequestFrame, ResponseFrame, ServerHello};
use locust_proto::{API_VERSION, codec};
use std::os::unix::net::UnixListener;
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

fn auth(socket: PathBuf) -> Arc<Authentication> {
    Arc::new(Authentication {
        socket,
        credential: Credential([1; 32]),
        session: Some(SessionSecret([2; 32])),
    })
}
fn fake(
    socket: &std::path::Path,
    owner: bool,
    block: bool,
) -> (std::thread::JoinHandle<()>, std::sync::mpsc::Receiver<()>) {
    fake_response(
        socket,
        owner,
        block,
        Err(ApiError {
            code: ErrorCode::Denied,
            message: "grant required".into(),
            details_json: None,
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
    assert_eq!(
        answer["result"]["structuredContent"]["error"]["code"],
        "denied"
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
            request: Request::Status,
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
        drop(writer);
        bridge.await.unwrap().unwrap();
    }
}

#[test]
fn pre_cancelled_worker_and_nested_unknown_arguments_are_rejected() {
    let cancellation = Cancellation::new();
    cancellation.cancel();
    let (stream, _) = UnixStream::pair().unwrap();
    assert!(cancellation.install(&stream).is_err());
    assert!(matches!(
        parse_call(
            &json!({"name":"locust_workspace_set","arguments":{"goal":"01".repeat(32),"binding":{"shell":"rm"}}})
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
    let status = Response::Status(locust_proto::api::DaemonStatus {
        daemon_version: "test".into(),
        endpoint: None,
        agents: vec![],
        goals: vec![],
    });
    let (daemon, _) = fake_response(&socket, false, false, Ok(status));
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
    assert_eq!(
        receive(&mut reader).await["result"]["tools"],
        json!(schema::tools())
    );
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
                request: Request::Status,
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
async fn private_author_uses_catalog_without_an_execution_session() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("author-socket");
    let listener = UnixListener::bind(&socket).unwrap();
    let daemon = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let hello: ClientHello =
            codec::decode(&codec::read_frame(&mut stream, 4096).unwrap().unwrap()).unwrap();
        assert_eq!(hello.session, None);
        let welcome = ServerHello::Welcome {
            api_version: API_VERSION,
            daemon_version: "test".into(),
            caller: Caller::Author(locust_proto::id::PublicKey([4; 32])),
            max_blob_bytes: 1024,
        };
        codec::write_frame(&mut stream, &codec::encode(&welcome).unwrap()).unwrap();
        let frame: RequestFrame =
            codec::decode(&codec::read_frame(&mut stream, 4096).unwrap().unwrap()).unwrap();
        assert_eq!(frame.request, Request::BlueprintDrafts);
        assert_eq!(frame.on_behalf, None);
        codec::write_frame(
            &mut stream,
            &codec::encode(&ResponseFrame {
                id: frame.id,
                result: Ok(Response::BlueprintDrafts(vec![])),
            })
            .unwrap(),
        )
        .unwrap();
    });
    let (input, mut writer) = tokio::io::duplex(8192);
    let (output, reader) = tokio::io::duplex(8192);
    let mut reader = BufReader::new(reader);
    let authentication = Arc::new(Authentication {
        socket,
        credential: Credential([1; 32]),
        session: None,
    });
    let bridge = tokio::spawn(serve(input, output, authentication, None));
    initialize(&mut writer, &mut reader, VERSIONS[0]).await;
    send(&mut writer,json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"locust_blueprint_drafts"}})).await;
    let answer = receive(&mut reader).await;
    assert_eq!(answer["result"]["structuredContent"]["ok"], true);
    drop(writer);
    bridge.await.unwrap().unwrap();
    daemon.join().unwrap();
}
