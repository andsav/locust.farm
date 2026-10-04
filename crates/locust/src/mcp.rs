//! Production stdio MCP bridge to the authenticated local daemon API.
//!
//! Implements the negotiated 2025-11-25, 2025-06-18 and 2025-03-26 lifecycle:
//! https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle
//! https://modelcontextprotocol.io/specification/2025-11-25/server/tools
//! https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/cancellation
//! Each call has its own socket. Cancelling a request shuts down that socket;
//! it never means an already committed mutation was rolled back. EOF cancels
//! and joins every outstanding worker before this bridge returns.
//! The local launched client controls concurrent calls: there is no invented
//! task cap. One encoded output frame is retained while stdout drains; input
//! and completed calls backpressure behind it. Outstanding calls can retain
//! results until they drain, so this is not an untrusted-ingress resource limit.
mod schema;
mod stdio;

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::net::Shutdown;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use locust_proto::api::{
    Caller, Credential, ErrorCode, OPERATIONS, Request, Response, SessionSecret,
};
use locust_proto::client::Client;
use locust_proto::id::IdempotencyKey;
use locust_proto::local;
use serde_json::{Map, Value, json};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::sync::watch;
use tokio::task::JoinSet;

use crate::{connection, failure::Failure};

const VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];

pub(crate) struct Config {
    pub home: PathBuf,
    pub credential: PathBuf,
    pub session: Option<PathBuf>,
    pub lifecycle_receipt: Option<PathBuf>,
}

struct Authentication {
    socket: PathBuf,
    credential: Credential,
    session: Option<SessionSecret>,
}
impl Config {
    fn prepare(self) -> Result<(Arc<Authentication>, Option<File>), Failure> {
        for path in [&self.home, &self.credential]
            .into_iter()
            .chain(self.session.iter())
        {
            if !path.is_absolute() {
                return Err(Failure::usage(
                    "MCP home, credential and session paths must be absolute",
                ));
            }
        }
        let metadata = fs::metadata(&self.home)
            .map_err(|error| Failure::invalid(format!("MCP home: {error}")))?;
        if !metadata.is_dir() || metadata.permissions().mode() & 0o7777 != local::HOME_MODE {
            return Err(Failure::invalid(
                "MCP home must be a directory with mode 0700",
            ));
        }
        let authentication = Authentication {
            socket: local::socket_path(&self.home)?,
            credential: Credential(connection::read_secret(&self.credential)?),
            session: self
                .session
                .as_ref()
                .map(|path| connection::read_secret(path).map(SessionSecret))
                .transpose()?,
        };
        if self.lifecycle_receipt.is_some() && authentication.session.is_none() {
            return Err(Failure::usage(
                "MCP lifecycle receipts require an execution session",
            ));
        }
        let receipt = self
            .lifecycle_receipt
            .map(|path| {
                if !path.is_absolute() {
                    return Err(Failure::usage(
                        "MCP lifecycle receipt path must be absolute",
                    ));
                }
                // Do not create or follow an attacker-replaced receipt path.
                let fd = rustix::fs::open(
                    &path,
                    rustix::fs::OFlags::WRONLY
                        | rustix::fs::OFlags::APPEND
                        | rustix::fs::OFlags::NOFOLLOW,
                    rustix::fs::Mode::empty(),
                )
                .map_err(|error| Failure::invalid(format!("MCP lifecycle receipt: {error}")))?;
                let file = File::from(fd);
                let metadata = file
                    .metadata()
                    .map_err(|error| Failure::invalid(format!("MCP lifecycle receipt: {error}")))?;
                if !metadata.is_file()
                    || metadata.permissions().mode() & 0o7777 != local::SECRET_FILE_MODE
                {
                    return Err(Failure::invalid(
                        "MCP lifecycle receipt must be an existing regular file with mode 0600",
                    ));
                }
                Ok(file)
            })
            .transpose()?;
        Ok((Arc::new(authentication), receipt))
    }
}

pub(crate) fn run(config: Config) -> Result<(), Failure> {
    let (authentication, receipt) = config.prepare()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Failure::internal(format!("MCP runtime: {error}")))?;
    runtime.block_on(async {
        let input = stdio::Pipe::new(io::stdin()).map_err(io_failure)?;
        let output = stdio::Pipe::new(io::stdout()).map_err(io_failure)?;
        serve(input, output, authentication, receipt).await
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RequestId(String);
impl RequestId {
    fn parse(value: &Value) -> Option<Self> {
        (value.is_string() || value.as_i64().is_some() || value.as_u64().is_some())
            .then(|| Self(value.to_string()))
    }
}

struct Cancellation {
    cancelled: watch::Sender<bool>,
    socket: Mutex<Option<UnixStream>>,
}
impl Cancellation {
    fn new() -> Self {
        let (cancelled, _) = watch::channel(false);
        Self {
            cancelled,
            socket: Mutex::new(None),
        }
    }
    fn cancel(&self) {
        self.cancelled.send_replace(true);
        if let Some(socket) = self.socket.lock().expect("cancellation mutex").as_ref() {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }
    fn is_cancelled(&self) -> bool {
        *self.cancelled.borrow()
    }
    fn install(&self, stream: &UnixStream) -> Result<(), Failure> {
        let mut socket = self.socket.lock().expect("cancellation mutex");
        if self.is_cancelled() {
            return Err(Failure::unavailable("MCP request was cancelled"));
        }
        *socket = Some(stream.try_clone().map_err(io_failure)?);
        Ok(())
    }
}

struct Call {
    request: Request,
    idempotency: Option<IdempotencyKey>,
}
#[derive(Clone, Copy)]
enum Kind {
    Tool,
    List,
}
struct Pending {
    id: Value,
    kind: Kind,
    cancellation: Arc<Cancellation>,
}
struct Outgoing {
    bytes: Vec<u8>,
    written: usize,
    receipt: bool,
}
fn enqueue(output: &mut Option<Outgoing>, message: Value, receipt: bool) {
    let mut bytes = serde_json::to_vec(&message).expect("JSON value serializes");
    bytes.push(b'\n');
    assert!(output.is_none(), "output backpressure admits one response");
    *output = Some(Outgoing {
        bytes,
        written: 0,
        receipt,
    });
}
fn response(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}
fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}
fn io_failure(error: impl std::fmt::Display) -> Failure {
    Failure::unavailable(format!("MCP I/O: {error}"))
}

async fn invoke(
    auth: Arc<Authentication>,
    call: Call,
    cancellation: Arc<Cancellation>,
) -> Result<Response, Failure> {
    let mut cancelled = cancellation.cancelled.subscribe();
    if *cancelled.borrow() {
        return Err(Failure::unavailable("MCP request was cancelled"));
    }
    let stream = tokio::select! {
        biased;
        _ = cancelled.changed() => return Err(Failure::unavailable("MCP request was cancelled")),
        stream = tokio::net::UnixStream::connect(&auth.socket) => stream.map_err(|error| Failure::unavailable(format!("daemon connection: {error}")))?,
    };
    let stream = stream.into_std().map_err(io_failure)?;
    stream.set_nonblocking(false).map_err(io_failure)?;
    cancellation.install(&stream)?;
    tokio::task::spawn_blocking(move || {
        let result = (|| {
            // The blocking pool can queue work. A cancelled queued worker must
            // never start the authentication handshake or daemon operation.
            if cancellation.is_cancelled() {
                return Err(Failure::unavailable("MCP request was cancelled"));
            }
            let mut client = Client::open(stream, auth.credential, auth.session)
                .map_err(|error| connection::client_error(error, &auth.socket))?;
            if client.caller() == Caller::Owner {
                return Err(Failure::new(ErrorCode::Denied, "MCP requires an enrolled agent or author credential; owner authority is not exposed to models"));
            }
            if cancellation.is_cancelled() { return Err(Failure::unavailable("MCP request was cancelled")); }
            client.call_with(call.request, call.idempotency, None)
                .map_err(|error| connection::client_error(error, &auth.socket))
        })();
        cancellation.socket.lock().expect("cancellation mutex").take();
        result
    }).await.map_err(|_| Failure::internal("MCP daemon call worker failed"))?
}

#[derive(Default, PartialEq)]
enum Phase {
    #[default]
    New,
    Initializing,
    Ready,
}

async fn serve<R: AsyncRead + Unpin, W: stdio::Output>(
    input: R,
    mut output: W,
    authentication: Arc<Authentication>,
    mut receipt: Option<File>,
) -> Result<(), Failure> {
    let mut input = BufReader::new(input);
    let mut line_buffer = Vec::new();
    let mut phase = Phase::New;
    let mut version = VERSIONS[0];
    let mut pending: HashMap<RequestId, Pending> = HashMap::new();
    let mut calls = JoinSet::new();
    let mut outgoing: Option<Outgoing> = None;
    let closed = output.closed();
    tokio::pin!(closed);
    let outcome = loop {
        tokio::select! {
            closure = &mut closed => {
                break Err(match closure {
                    Ok(()) => Failure::unavailable("MCP stdout closed"),
                    Err(error) => io_failure(error),
                });
            }
            line = read_line(&mut input, &mut line_buffer), if outgoing.is_none() => {
                let line = match line {
                    Ok(Some(line)) => line,
                    Ok(None) => break Ok(()),
                    Err(error) => break Err(io_failure(error)),
                };
                let message: Value = match serde_json::from_str(&line) {
                    Ok(message) => message,
                    Err(_) => { enqueue(&mut outgoing, error(Value::Null, -32700, "Parse error"), false); continue; }
                };
                let Some(object) = message.as_object() else {
                    enqueue(&mut outgoing, error(Value::Null, -32600, "Invalid request"), false); continue;
                };
                let id = object.get("id").cloned();
                let method = object.get("method").and_then(Value::as_str);
                if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
                    || method.is_none() || object.contains_key("result") || object.contains_key("error")
                    || id.as_ref().is_some_and(|id| RequestId::parse(id).is_none()) {
                    // Responses are not requests, and the bridge sends no server requests.
                    if object.contains_key("result") || object.contains_key("error") { continue; }
                    enqueue(&mut outgoing, error(id.filter(|id| RequestId::parse(id).is_some()).unwrap_or(Value::Null), -32600, "Invalid request"), false);
                    continue;
                }
                let method = method.unwrap();
                let params = object.get("params").cloned().unwrap_or_else(|| json!({}));
                let Some(id) = id else {
                    match method {
                        "notifications/initialized" if phase == Phase::Initializing && params.is_object() => phase = Phase::Ready,
                        "notifications/cancelled" => {
                            if let Some(key) = params.get("requestId").and_then(RequestId::parse)
                                && let Some(call) = pending.get(&key) { call.cancellation.cancel(); }
                        }
                        _ => {}
                    }
                    continue;
                };
                let key = RequestId::parse(&id).expect("validated request id");
                if pending.contains_key(&key) {
                    enqueue(&mut outgoing, error(id, -32600, "Request id is already in use"), false); continue;
                }
                if !params.is_object() {
                    enqueue(&mut outgoing, error(id, -32602, "Params must be an object"), false); continue;
                }
                if method == "ping" {
                    enqueue(&mut outgoing, response(id, json!({})), false); continue;
                }
                if method == "initialize" {
                    if phase != Phase::New {
                        enqueue(&mut outgoing, error(id, -32600, "Already initialized"), false); continue;
                    }
                    let Some(requested) = params["protocolVersion"].as_str().filter(|_| {
                        params["capabilities"].is_object() && params["clientInfo"]["name"].is_string() && params["clientInfo"]["version"].is_string()
                    }) else {
                        enqueue(&mut outgoing, error(id, -32602, "Initialize requires protocolVersion, capabilities and clientInfo name/version"), false); continue;
                    };
                    version = VERSIONS.iter().copied().find(|supported| *supported == requested).unwrap_or(VERSIONS[0]);
                    phase = Phase::Initializing;
                    enqueue(&mut outgoing, response(id, json!({
                        "protocolVersion": version, "capabilities": {"tools": {"listChanged": false}},
                        "serverInfo": {"name": "locust", "version": env!("CARGO_PKG_VERSION")},
                        "instructions": "Start each work step with context.read for the goal or task. Read attributed findings, inputs, progress and review reasons; cite useful event IDs and publish new findings with contribution.publish. Follow context pagination, then acknowledge only complete content you actually read with the returned session receipt. pending.context_news reports unread and unavailable content. Inspect the goal rules, allowed actions and pending work. Shared eligibility is separate from local execution authorization. Independent work begins with an attempt; publishing a contribution does not select it or apply files. Durable deliveries remain pending until acknowledged. Cancellation of an MCP call does not undo committed work. Retry uncertain writes with the same idempotency_key."
                    })), false);
                    continue;
                }
                if phase != Phase::Ready {
                    enqueue(&mut outgoing, error(id, -32002, "Initialize and send notifications/initialized before using tools"), false); continue;
                }
                let (kind, call) = match method {
                    "tools/list" if params.get("cursor").is_none() => (Kind::List, Call { request: Request::Status, idempotency: None }),
                    "tools/list" => { enqueue(&mut outgoing, error(id, -32602, "Tool list is not paginated; omit cursor"), false); continue; }
                    "tools/call" => match parse_call(&params) {
                        Ok(call) => (Kind::Tool, call),
                        Err(CallError::Protocol(message)) => { enqueue(&mut outgoing, error(id, -32602, message), false); continue; }
                        Err(CallError::Arguments(failure)) => { enqueue(&mut outgoing, response(id, tool_result(Err(failure), version)), false); continue; }
                    },
                    _ => { enqueue(&mut outgoing, error(id, -32601, "Method not found"), false); continue; }
                };
                let cancellation = Arc::new(Cancellation::new());
                pending.insert(key.clone(), Pending { id, kind, cancellation: cancellation.clone() });
                let auth = authentication.clone();
                calls.spawn(async move { (key, invoke(auth, call, cancellation).await) });
            }
            completed = calls.join_next(), if !calls.is_empty() && outgoing.is_none() => {
                let Some(Ok((key, result))) = completed else { break Err(Failure::internal("MCP call task failed")); };
                let Some(call) = pending.remove(&key) else { continue; };
                if call.cancellation.is_cancelled() { continue; }
                match call.kind {
                    Kind::Tool => enqueue(&mut outgoing, response(call.id, tool_result(result, version)), false),
                    Kind::List => match result {
                        Ok(_) => enqueue(&mut outgoing, response(call.id, json!({"tools": schema::tools()})), true),
                        Err(failure) => {
                            let mut message = error(call.id, -32000, "Cannot authenticate MCP tools with the daemon");
                            message["error"]["data"] = failure_value(failure);
                            enqueue(&mut outgoing, message, false);
                        }
                    }
                }
            }
            written = async {
                let frame = outgoing.as_ref().expect("nonempty output");
                output.write(&frame.bytes[frame.written..]).await
            }, if outgoing.is_some() => {
                match written {
                    Ok(0) => break Err(Failure::unavailable("MCP stdout closed")),
                    Err(error) => break Err(io_failure(error)),
                    Ok(count) => {
                        let frame = outgoing.as_mut().unwrap();
                        frame.written += count;
                        if frame.written == frame.bytes.len() {
                            let frame = outgoing.take().unwrap();
                            if frame.receipt && let Some(file) = receipt.as_mut() {
                                let record = json!({"schema": 1, "event": "tools_ready", "instance": authentication.session.expect("receipt requires session").instance().to_string(), "pid": std::process::id()});
                                if let Err(error) = serde_json::to_writer(&mut *file, &record).map_err(io::Error::other)
                                    .and_then(|()| file.write_all(b"\n")).and_then(|()| file.flush()) {
                                    break Err(io_failure(error));
                                }
                            }
                        }
                    }
                }
            }
        }
    };
    for call in pending.values() {
        call.cancellation.cancel();
    }
    while calls.join_next().await.is_some() {}
    outcome
}

// Retain partial input across select cancellation. The existing local API frame
// ceiling also bounds the JSON-RPC envelope before allocation; oversized input
// terminates the transport rather than silently truncating a request.
async fn read_line<R: tokio::io::AsyncBufRead + Unpin>(
    input: &mut R,
    buffer: &mut Vec<u8>,
) -> io::Result<Option<String>> {
    loop {
        let bytes = input.fill_buf().await?;
        if bytes.is_empty() {
            if buffer.is_empty() {
                return Ok(None);
            }
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "MCP frame lacks newline",
            ));
        }
        let count = bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |at| at + 1);
        if buffer.len() + count > locust_proto::limits::MAX_LOCAL_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "MCP frame exceeds local API frame limit",
            ));
        }
        let complete = bytes[count - 1] == b'\n';
        buffer.extend_from_slice(&bytes[..count]);
        input.consume(count);
        if complete {
            return String::from_utf8(std::mem::take(buffer))
                .map(Some)
                .map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "MCP frame must be UTF-8")
                });
        }
    }
}

enum CallError {
    Protocol(&'static str),
    Arguments(Failure),
}
fn parse_call(params: &Value) -> Result<Call, CallError> {
    let object = params
        .as_object()
        .ok_or(CallError::Protocol("Tool params must be an object"))?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "name" | "arguments" | "_meta"))
    {
        return Err(CallError::Protocol("Unsupported tools/call parameter"));
    }
    let name = params["name"]
        .as_str()
        .ok_or(CallError::Protocol("Tool name must be a string"))?;
    let operation = OPERATIONS
        .iter()
        .find(|op| op.tool && op.tool_name() == name)
        .ok_or(CallError::Protocol("Unknown tool"))?;
    let mut arguments: Map<String, Value> = match object.get("arguments") {
        None => Map::new(),
        Some(Value::Object(arguments)) => arguments.clone(),
        _ => return Err(CallError::Protocol("Tool arguments must be an object")),
    };
    let idempotency = arguments
        .remove("idempotency_key")
        .map(serde_json::from_value::<Option<IdempotencyKey>>)
        .transpose()
        .map_err(|_| {
            CallError::Arguments(Failure::invalid(
                "idempotency_key must be 32 hexadecimal characters or null",
            ))
        })?
        .flatten();
    let value = if arguments.is_empty()
        && serde_json::from_value::<Request>(json!(operation.name)).is_ok()
    {
        json!(operation.name)
    } else {
        json!({operation.name: arguments})
    };
    let request: Request = serde_json::from_value(value).map_err(|error| {
        CallError::Arguments(Failure::invalid(format!(
            "Invalid arguments for {}: {error}",
            operation.name
        )))
    })?;
    request
        .check()
        .map_err(|error| CallError::Arguments(error.into()))?;
    Ok(Call {
        request,
        idempotency,
    })
}
fn failure_value(failure: Failure) -> Value {
    json!({"code": failure.code.as_str(), "message": failure.message,"details":failure.details_json.as_deref().and_then(|text|serde_json::from_str::<Value>(text).ok())})
}
fn tool_result(result: Result<Response, Failure>, version: &str) -> Value {
    let (value, failed) = match result {
        Ok(response) => (json!({"ok": true, "result": response}), false),
        Err(failure) => (json!({"ok": false, "error": failure_value(failure)}), true),
    };
    let mut result =
        json!({"content": [{"type": "text", "text": value.to_string()}], "isError": failed});
    if version != "2025-03-26" {
        result["structuredContent"] = value;
    }
    result
}

#[cfg(test)]
mod tests;
