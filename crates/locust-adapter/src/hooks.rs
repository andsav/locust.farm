//! Pure native hook adapters. Callers supply input and configuration documents;
//! this module never reads profiles, transcripts, credentials or files.
//!
//! An adapter is data: a row of [`ADAPTERS`] names its native events, config
//! file and shape, payload encodings, envelope and time limits. The code here
//! serves every row through those facts and never names a harness, so a new
//! harness adds a row, a shim if it needs one, and goldens.
//!
//! Native contracts: <https://learn.chatgpt.com/docs/hooks> and
//! <https://code.claude.com/docs/en/hooks>. These adapters cover root lifecycle
//! hooks. A subagent's callback is not this chat's: the shared execution
//! session's claims cannot establish a subagent's ownership, so it is ignored
//! without a word. Codex 0.153.4 serializes agent_id on subagent tool hooks
//! (the common-field docs omit it):
//! <https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/hooks/src/events/post_tool_use.rs>.
//! Droid's standalone event map and envelopes follow
//! <https://docs.factory.com/harness/hooks>. Root callbacks retain the exact
//! session_id; child callback routing remains unverified. Droid supplies no
//! tool invocation ID, so repeated identical successful logical effects may
//! coalesce. Typed recorded events and claims distinguish authored effects.

pub mod core;
mod pi;
pub use core::{Event, Outcome, OwnAction, OwnCall};

use std::fmt;
use std::path::Path;

use locust_proto::api::{OPERATIONS, Request, Response};
use locust_proto::id::{GoalId, IdempotencyKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config::Client;

/// The MCP server name setup registers in every client.
const SERVER: &str = "locust";

#[derive(Clone, Copy, Debug)]
pub struct HookAdapter {
    pub client: Client,
    pub harness: &'static str,
    /// Relative to the explicitly selected profile home.
    pub relative_config_path: &'static str,
    pub config: ConfigShape,
    pub events: &'static [NativeEvent],
    /// Payload fields whose presence marks a subagent's callback.
    pub subagent_fields: &'static [&'static str],
    /// Where the payload can say that no person types in this chat.
    pub unattended: Option<Unattended>,
    pub native_tool_prefix: &'static str,
    pub native_tool_use_id: bool,
    pub response: ResponseEncoding,
    pub envelope: EnvelopeKind,
    /// The native limit for start and tool commands, which never wait.
    pub command_timeout_seconds: u32,
    /// The native limit for the stop command, the only one that may wait.
    pub stop_timeout_seconds: u32,
    /// The harness asks its person to review new hooks before running them.
    pub trust_review: bool,
}

/// How a native config file holds Locust's hook entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigShape {
    /// A JSON settings object whose `hooks` key maps native events to groups.
    HooksKey,
    /// A JSON file that is itself the map of native events to groups.
    EventMap,
    /// A source file generated whole from this template, owned by Locust.
    OwnedSource { template: &'static str },
}

/// How a native tool callback carries the tool's result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseEncoding {
    /// The MCP `CallToolResult` object.
    Mcp,
    /// The MCP result object, or the Locust JSON envelope as a string.
    McpOrText,
    /// A native result with `details.server`/`details.tool` whose
    /// `structuredContent` is the whole MCP result.
    NestedMcp,
}

/// The JSON a harness reads the one line, or a block, from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeKind {
    /// `decision: block` with `reason`; `systemMessage` for a stop line;
    /// `hookSpecificOutput.additionalContext` otherwise.
    Hook,
    /// `{"line": ..., "keep_going": ...}` for a shim.
    Line,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeEvent {
    pub native_name: &'static str,
    pub event: &'static str,
    /// A `tool` event for a call that succeeded, whose result can show this
    /// chat's own Locust call.
    pub success: bool,
}

/// A payload field whose listed values say no person types in the chat.
#[derive(Clone, Copy, Debug)]
pub struct Unattended {
    pub field: &'static str,
    pub values: &'static [&'static str],
}

const fn event(native_name: &'static str, event: &'static str) -> NativeEvent {
    NativeEvent {
        native_name,
        event,
        success: true,
    }
}

const EVENTS: &[NativeEvent] = &[
    event("SessionStart", "start"),
    event("Stop", "stop"),
    event("PostToolUse", "tool"),
];
const CLAUDE_EVENTS: &[NativeEvent] = &[
    event("SessionStart", "start"),
    event("Stop", "stop"),
    event("PostToolUse", "tool"),
    NativeEvent {
        native_name: "PostToolUseFailure",
        event: "tool",
        success: false,
    },
];
pub const ADAPTERS: &[HookAdapter] = &[
    pi::ADAPTER,
    HookAdapter {
        client: Client::Codex,
        harness: "codex",
        relative_config_path: ".codex/hooks.json",
        config: ConfigShape::HooksKey,
        events: EVENTS,
        subagent_fields: &["agent_id", "agent_type"],
        unattended: None,
        native_tool_prefix: "mcp__locust__",
        native_tool_use_id: true,
        response: ResponseEncoding::Mcp,
        envelope: EnvelopeKind::Hook,
        command_timeout_seconds: 30,
        stop_timeout_seconds: 300,
        trust_review: true,
    },
    HookAdapter {
        client: Client::ClaudeCode,
        harness: "claude",
        relative_config_path: ".claude/settings.json",
        config: ConfigShape::HooksKey,
        events: CLAUDE_EVENTS,
        subagent_fields: &["agent_id"],
        unattended: None,
        native_tool_prefix: "mcp__locust__",
        native_tool_use_id: true,
        response: ResponseEncoding::McpOrText,
        envelope: EnvelopeKind::Hook,
        command_timeout_seconds: 30,
        stop_timeout_seconds: 300,
        trust_review: false,
    },
    HookAdapter {
        client: Client::FactoryDroid,
        harness: "droid",
        relative_config_path: ".factory/hooks.json",
        config: ConfigShape::EventMap,
        events: EVENTS,
        subagent_fields: &["agent_id"],
        unattended: None,
        native_tool_prefix: "locust___",
        native_tool_use_id: false,
        response: ResponseEncoding::McpOrText,
        envelope: EnvelopeKind::Hook,
        command_timeout_seconds: 30,
        stop_timeout_seconds: 300,
        trust_review: false,
    },
];

pub fn adapter(client: Client) -> Option<&'static HookAdapter> {
    ADAPTERS.iter().find(|adapter| adapter.client == client)
}

/// A normalized document is the adapter interface; setup does not own native
/// serialization. JSON shapes normalize absent files to empty documents.
pub fn parse_configuration(client: Client, bytes: Option<&[u8]>) -> Result<Value, HookError> {
    let adapter = adapter(client).ok_or(HookError::UnsupportedClient)?;
    if let ConfigShape::OwnedSource { .. } = adapter.config {
        let source = bytes
            .map(std::str::from_utf8)
            .transpose()
            .map_err(|_| HookError::InvalidConfiguration)?;
        return Ok(json!({ "source": source }));
    }
    let value = match bytes {
        None => json!({}),
        Some(bytes) => {
            serde_json::from_slice(bytes).map_err(|_| HookError::InvalidConfiguration)?
        }
    };
    if !value.is_object() {
        return Err(HookError::InvalidConfiguration);
    }
    if adapter.config == ConfigShape::EventMap {
        if value.get("hooks").is_some() {
            return Err(HookError::InvalidConfiguration);
        }
        Ok(json!({ "hooks": value }))
    } else {
        Ok(value)
    }
}

/// `None` removes an adapter-owned file; an empty source file is not absence.
pub fn render_configuration(
    client: Client,
    document: &Value,
) -> Result<Option<Vec<u8>>, HookError> {
    let adapter = adapter(client).ok_or(HookError::UnsupportedClient)?;
    if let ConfigShape::OwnedSource { .. } = adapter.config {
        return Ok(source(document)?.map(|source| source.as_bytes().to_vec()));
    }
    if !document.is_object() {
        return Err(HookError::InvalidConfiguration);
    }
    let empty = json!({});
    let native = if adapter.config == ConfigShape::EventMap {
        if document
            .as_object()
            .is_some_and(|object| object.keys().any(|key| key != "hooks"))
        {
            return Err(HookError::InvalidConfiguration);
        }
        document.get("hooks").unwrap_or(&empty)
    } else {
        document
    };
    if !native.is_object() {
        return Err(HookError::InvalidConfiguration);
    }
    let mut bytes =
        serde_json::to_vec_pretty(native).map_err(|_| HookError::InvalidConfiguration)?;
    bytes.push(b'\n');
    Ok(Some(bytes))
}

fn source(document: &Value) -> Result<Option<&str>, HookError> {
    let map = document
        .as_object()
        .ok_or(HookError::InvalidConfiguration)?;
    if map.len() != 1 || !map.contains_key("source") {
        return Err(HookError::InvalidConfiguration);
    }
    match &map["source"] {
        Value::Null => Ok(None),
        Value::String(source) => Ok(Some(source)),
        _ => Err(HookError::InvalidConfiguration),
    }
}

/// Only the native session ID identifies a chat. Titles, transcripts and
/// working directories never do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatIdentity {
    pub session_id: String,
}

#[derive(Clone, Debug)]
pub struct NativeInput {
    pub chat: ChatIdentity,
    pub event: Event,
    pub native_event_name: String,
}

/// What a native callback is, as far as Locust may act on it.
#[derive(Clone, Debug)]
pub enum Parsed {
    /// Not a root chat Locust can attribute, such as a subagent's callback:
    /// print nothing, read nothing, write nothing.
    Ignored,
    /// The chat is known; the rest of the callback is not.
    Invalid {
        chat: ChatIdentity,
        native_event_name: String,
    },
    Input(NativeInput),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookError {
    UnsupportedClient,
    InvalidOutput,
    InvalidConfiguration,
    EntryConflict,
    InvalidExecutable,
}
impl fmt::Display for HookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedClient => "this client has no Locust hook adapter",
            Self::InvalidOutput => "invalid Locust hook output",
            Self::InvalidConfiguration => "hook configuration must contain objects and event arrays",
            Self::EntryConflict => "Locust hook entry is occupied or modified",
            Self::InvalidExecutable => "hook executable must be an absolute UTF-8 path without controls or client expansion",
        })
    }
}
impl std::error::Error for HookError {}

/// Parse native event facts only. Unknown fields, including titles and transcript
/// paths, are ignored. They never become core state or model-visible output.
pub fn parse_input(adapter: &HookAdapter, expected_event: &str, value: &Value) -> Parsed {
    if adapter
        .subagent_fields
        .iter()
        .any(|field| value.get(*field).is_some_and(|value| !value.is_null()))
    {
        return Parsed::Ignored;
    }
    let Ok(session_id) = required_string(value, "session_id") else {
        return Parsed::Ignored;
    };
    let chat = ChatIdentity {
        session_id: session_id.to_owned(),
    };
    match parse_event(adapter, expected_event, value) {
        Some(event) => Parsed::Input(NativeInput {
            chat,
            event,
            native_event_name: value["hook_event_name"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        }),
        None => Parsed::Invalid {
            chat,
            native_event_name: native_event_name(adapter, expected_event, value).to_owned(),
        },
    }
}

/// The native event a reply names: the callback's own when it is one of this
/// event's names, otherwise the first.
pub fn native_event_name<'a>(
    adapter: &'a HookAdapter,
    expected_event: &str,
    value: &Value,
) -> &'a str {
    let names = adapter
        .events
        .iter()
        .filter(|event| event.event == expected_event);
    let given = value.get("hook_event_name").and_then(Value::as_str);
    names
        .clone()
        .find(|event| Some(event.native_name) == given)
        .or_else(|| names.clone().next())
        .map_or("", |event| event.native_name)
}

fn parse_event(adapter: &HookAdapter, expected_event: &str, value: &Value) -> Option<Event> {
    let native_name = required_string(value, "hook_event_name").ok()?;
    let mapping = adapter
        .events
        .iter()
        .find(|event| event.native_name == native_name && event.event == expected_event)?;
    match mapping.event {
        "start" => Some(Event::Start),
        "stop" => Some(Event::Stop {
            unattended: adapter.unattended.is_some_and(|signal| {
                value
                    .get(signal.field)
                    .and_then(Value::as_str)
                    .is_some_and(|given| signal.values.contains(&given))
            }),
        }),
        "tool" => {
            required_string(value, "tool_name").ok()?;
            if adapter.native_tool_use_id {
                required_string(value, "tool_use_id").ok()?;
            }
            Some(Event::Tool {
                own_call: if mapping.success {
                    own_call(adapter, value).map(Box::new)
                } else {
                    None
                },
            })
        }
        _ => None,
    }
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, ()> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or(())
}

fn own_call(adapter: &HookAdapter, value: &Value) -> Option<OwnCall> {
    let projected;
    let value = if adapter.response == ResponseEncoding::NestedMcp {
        projected = nested_projection(adapter, value)?;
        &projected
    } else {
        value
    };
    // No bare tool-name or vendor/plugin prefix aliases: only the configured
    // server name and the operation registry can establish a Locust call.
    let tool = value["tool_name"]
        .as_str()?
        .strip_prefix(adapter.native_tool_prefix)?;
    let operation = OPERATIONS
        .iter()
        .find(|operation| operation.tool && operation.tool_name() == tool)?;
    let mut args = value["tool_input"].as_object()?.clone();
    if let Some(key) = args.remove("idempotency_key") {
        serde_json::from_value::<Option<IdempotencyKey>>(key).ok()?;
    }
    let result = successful_result(adapter.response, &value["tool_response"])?;
    let (goal, action, effect) = if operation.name == "context.acknowledge" {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Acknowledge {
            goal: GoalId,
            receipt: String,
        }
        let args: Acknowledge = serde_json::from_value(Value::Object(args)).ok()?;
        let reference = args.receipt.strip_prefix("ctx:")?;
        if reference.len() != 64
            || !reference
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return None;
        }
        let Response::ContextAcknowledged(ack) = serde_json::from_value(result).ok()? else {
            return None;
        };
        if ack.goal != args.goal {
            return None;
        }
        let effect =
            json!([{"goal":args.goal,"receipt":args.receipt},Response::ContextAcknowledged(ack)]);
        (Some(args.goal), OwnAction::Other, effect)
    } else {
        let request_value = if args.is_empty()
            && serde_json::from_value::<Request>(json!(operation.name)).is_ok()
        {
            json!(operation.name)
        } else {
            json!({operation.name: args})
        };
        let request: Request = serde_json::from_value(request_value).ok()?;
        request.check().ok()?;
        // The bridge exposes context receipts as local references. Discard that
        // transport-only field before checking the otherwise typed response.
        let mut result = result;
        if operation.name == "context.read" && result["context"]["receipt"].is_string() {
            result["context"]["receipt"] = Value::Null;
        }
        let response: Response = serde_json::from_value(result).ok()?;
        if !request.is_answered_by(&response) {
            return None;
        }
        let effect = match &response {
            Response::Recorded { event } => json!({"recorded":event}),
            Response::Claimed(claim) => json!({"claimed":claim}),
            _ => json!([&request, &response]),
        };
        let claim = match response {
            Response::Claimed(claim) => Some(claim),
            _ => None,
        };
        if let Some(claim) = &claim {
            if request.goal() != Some(claim.goal) {
                return None;
            }
            match &request {
                Request::AttemptStart {
                    task: Some(task), ..
                } if *task != claim.task => return None,
                Request::AttemptTakeover { attempt, .. } if *attempt != claim.attempt => {
                    return None;
                }
                _ => {}
            }
        }
        let action = match &request {
            Request::AttemptReport {
                attempt,
                generation,
                status,
                ..
            } => OwnAction::Report {
                attempt: *attempt,
                generation: *generation,
                status: *status,
            },
            Request::CancelAcknowledge {
                cancel,
                generation,
                outcome,
                ..
            } => OwnAction::CancelAcknowledged {
                cancel: *cancel,
                generation: *generation,
                outcome: *outcome,
                target: None,
            },
            _ => claim.map_or(OwnAction::Other, OwnAction::Claimed),
        };
        (request.goal(), action, effect)
    };
    Some(OwnCall {
        invocation_id: if adapter.native_tool_use_id {
            value["tool_use_id"].as_str()?.to_owned()
        } else {
            let bytes = serde_json::to_vec(&json!([operation.name, goal, effect])).ok()?;
            format!(
                "logical-effect:{}",
                locust_proto::crypto::content_hash(&bytes)
            )
        },
        operation: operation.name.to_owned(),
        goal,
        action,
    })
}

fn successful_result(encoding: ResponseEncoding, response: &Value) -> Option<Value> {
    // Native successful MCP callbacks may project the server's text envelope.
    // It is still checked as the exact typed Locust result below.
    let envelope = if encoding == ResponseEncoding::McpOrText && response.is_string() {
        serde_json::from_str(response.as_str()?).ok()?
    } else if response["isError"].as_bool()? {
        return None;
    } else if let Some(structured) = response.get("structuredContent") {
        structured.clone()
    } else {
        let content = response["content"].as_array()?;
        if content.len() != 1 || content[0]["type"] != "text" {
            return None;
        }
        serde_json::from_str(content[0]["text"].as_str()?).ok()?
    };
    if envelope["ok"] != true {
        return None;
    }
    envelope.get("result").cloned()
}

/// A nested native result holds the whole MCP result in structured content.
/// Check both error layers and the exact server and tool before unwrapping it
/// for the shared typed Locust request/response extractor.
fn nested_projection(adapter: &HookAdapter, value: &Value) -> Option<Value> {
    let response = &value["tool_response"];
    if response["isError"].as_bool()? {
        return None;
    }
    let details = response.get("details")?.as_object()?;
    if details.get("server")?.as_str()? != SERVER {
        return None;
    }
    let tool = details.get("tool")?.as_str()?;
    if value["tool_name"].as_str()? != format!("{}{tool}", adapter.native_tool_prefix) {
        return None;
    }
    let mut mcp = response.get("structuredContent")?.as_object()?.clone();
    if !mcp.get("content").is_some_and(Value::is_array) {
        return None;
    }
    match mcp.get("isError") {
        None | Some(Value::Bool(false)) => {}
        _ => return None,
    }
    mcp.insert("isError".into(), Value::Bool(false));
    let mut projected = value.clone();
    projected["tool_response"] = Value::Object(mcp);
    Some(projected)
}

/// Wrapping never adds instructions or native response text. Stop informational
/// lines use systemMessage: additionalContext itself continues some harnesses.
pub fn envelope(
    adapter: &HookAdapter,
    event: &str,
    native_event_name: &str,
    outcome: &Outcome,
) -> Result<Option<Value>, HookError> {
    let Some(line) = &outcome.line else {
        return if outcome.keep_going {
            Err(HookError::InvalidOutput)
        } else {
            Ok(None)
        };
    };
    if line.is_empty()
        || line.len() >= 512
        || !line.bytes().all(|byte| (b' '..=b'~').contains(&byte))
    {
        return Err(HookError::InvalidOutput);
    }
    if outcome.keep_going && event != "stop" {
        return Err(HookError::InvalidOutput);
    }
    Ok(Some(match adapter.envelope {
        EnvelopeKind::Line => json!({"line": line, "keep_going": outcome.keep_going}),
        EnvelopeKind::Hook if outcome.keep_going => json!({"decision": "block", "reason": line}),
        EnvelopeKind::Hook if event == "stop" => json!({"systemMessage": line}),
        EnvelopeKind::Hook => json!({
            "hookSpecificOutput": {"hookEventName": native_event_name, "additionalContext": line}
        }),
    }))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookEntry {
    pub native_event: String,
    pub group: Value,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HookRegistration {
    JsonGroups { entries: Vec<HookEntry> },
    OwnedSource { source: Option<String> },
}

impl HookRegistration {
    pub fn entries(&self) -> &[HookEntry] {
        match self {
            Self::JsonGroups { entries } => entries,
            Self::OwnedSource { .. } => &[],
        }
    }
}

pub fn registration(client: Client, executable: &Path) -> Result<HookRegistration, HookError> {
    let adapter = adapter(client).ok_or(HookError::UnsupportedClient)?;
    let path = executable
        .to_str()
        .filter(|path| executable.is_absolute() && !path.chars().any(char::is_control))
        .ok_or(HookError::InvalidExecutable)?;
    if let ConfigShape::OwnedSource { template } = adapter.config {
        // The path is inserted last, as a JSON string literal, so marker-like
        // text in it is never scanned again as a template placeholder.
        let rendered = template
            .replace(
                "__LOCUST_COMMAND_TIMEOUT_MS__",
                &(u64::from(adapter.command_timeout_seconds) * 1000).to_string(),
            )
            .replace(
                "__LOCUST_STOP_TIMEOUT_MS__",
                &(u64::from(adapter.stop_timeout_seconds) * 1000).to_string(),
            )
            .replace(
                "__LOCUST_HARNESS_JSON__",
                &serde_json::to_string(adapter.harness)
                    .map_err(|_| HookError::InvalidConfiguration)?,
            )
            .replace(
                "__LOCUST_LAUNCHER_JSON__",
                &serde_json::to_string(path).map_err(|_| HookError::InvalidExecutable)?,
            );
        return Ok(HookRegistration::OwnedSource {
            source: Some(rendered),
        });
    }
    // A command string is expanded by the harness; refuse its expansion syntax.
    if path.contains("${") {
        return Err(HookError::InvalidExecutable);
    }
    let quoted = format!("'{}'", path.replace('\'', "'\\''"));
    Ok(HookRegistration::JsonGroups {
        entries: adapter
            .events
            .iter()
            .map(|event| {
                let timeout = if event.event == "stop" {
                    adapter.stop_timeout_seconds
                } else {
                    adapter.command_timeout_seconds
                };
                HookEntry {
                    native_event: event.native_name.to_owned(),
                    group: json!({"hooks":[{"type":"command","command":format!("{quoted} hook {} --harness {}", event.event, adapter.harness),"timeout":timeout,"statusMessage":"Locust"}]}),
                }
            })
            .collect(),
    })
}

/// Install only fresh own groups; existing Locust groups are conflicts. An
/// installed registration is updated with [`reapply`].
pub fn install(current: &Value, registration: &HookRegistration) -> Result<Value, HookError> {
    if let HookRegistration::OwnedSource { source: expected } = registration {
        if expected.is_none() {
            source(current)?;
            return Ok(current.clone());
        }
        if source(current)?.is_some() {
            return Err(HookError::EntryConflict);
        }
        return Ok(json!({ "source": expected }));
    }
    let mut document = current.clone();
    let hooks = document
        .as_object_mut()
        .ok_or(HookError::InvalidConfiguration)?
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or(HookError::InvalidConfiguration)?;
    for entry in registration.entries() {
        let groups = hooks
            .entry(&entry.native_event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or(HookError::InvalidConfiguration)?;
        if groups.iter().any(|group| owns_marker(group, registration)) {
            return Err(HookError::EntryConflict);
        }
        groups.push(entry.group.clone());
    }
    Ok(document)
}

/// An installed registration brought up to this release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reapplied {
    pub document: Value,
    pub registration: HookRegistration,
    /// Native events whose entry the person removed; they stay out.
    pub declined: Vec<String>,
}

/// Replace an installed registration with this release's, entry by entry and
/// in place. An entry the person removed stays out, now and in later
/// releases; an event new in this release is added while any owned entry
/// remains.
pub fn reapply(
    current: &Value,
    owned: &HookRegistration,
    declined: &[String],
    fresh: &HookRegistration,
) -> Result<Reapplied, HookError> {
    let present = retain_present(current, owned)?;
    let unchanged = |present: HookRegistration, declined: Vec<String>| Reapplied {
        document: current.clone(),
        registration: present,
        declined,
    };
    match (&present, fresh) {
        (HookRegistration::OwnedSource { source: kept }, HookRegistration::OwnedSource { .. }) => {
            if kept.is_none() {
                return Ok(unchanged(present, declined.to_vec()));
            }
            source(current)?;
            let HookRegistration::OwnedSource { source } = fresh else {
                unreachable!("matched above");
            };
            Ok(Reapplied {
                document: json!({ "source": source }),
                registration: fresh.clone(),
                declined: declined.to_vec(),
            })
        }
        (
            HookRegistration::JsonGroups { entries: kept },
            HookRegistration::JsonGroups { entries: new },
        ) => {
            let had = |name: &str, entries: &[HookEntry]| {
                entries.iter().any(|entry| entry.native_event == name)
            };
            let mut declined = declined.to_vec();
            for entry in owned.entries() {
                if !had(&entry.native_event, kept) && !declined.contains(&entry.native_event) {
                    declined.push(entry.native_event.clone());
                }
            }
            let wanted: Vec<_> = new
                .iter()
                .filter(|entry| !kept.is_empty() && !declined.contains(&entry.native_event))
                .cloned()
                .collect();
            if wanted == *kept {
                return Ok(unchanged(present, declined));
            }
            let mut document = current.clone();
            let hooks = document
                .as_object_mut()
                .ok_or(HookError::InvalidConfiguration)?
                .entry("hooks")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or(HookError::InvalidConfiguration)?;
            for entry in kept {
                let groups = hooks
                    .get_mut(&entry.native_event)
                    .and_then(Value::as_array_mut)
                    .ok_or(HookError::InvalidConfiguration)?;
                let position = groups
                    .iter()
                    .position(|group| *group == entry.group)
                    .ok_or(HookError::EntryConflict)?;
                match wanted
                    .iter()
                    .find(|update| update.native_event == entry.native_event)
                {
                    Some(update) => groups[position] = update.group.clone(),
                    None => {
                        groups.remove(position);
                    }
                }
            }
            for entry in wanted
                .iter()
                .filter(|entry| !had(&entry.native_event, kept))
            {
                hooks
                    .entry(&entry.native_event)
                    .or_insert_with(|| json!([]))
                    .as_array_mut()
                    .ok_or(HookError::InvalidConfiguration)?
                    .push(entry.group.clone());
            }
            Ok(Reapplied {
                document,
                registration: HookRegistration::JsonGroups { entries: wanted },
                declined,
            })
        }
        _ => Err(HookError::InvalidConfiguration),
    }
}

/// Keep entries that still exist exactly, preserving intentional hand-removal.
pub fn retain_present(
    current: &Value,
    registration: &HookRegistration,
) -> Result<HookRegistration, HookError> {
    if let HookRegistration::OwnedSource { source: expected } = registration {
        let present = source(current)?;
        if expected.is_some() && present.is_some() && present != expected.as_deref() {
            return Err(HookError::EntryConflict);
        }
        return Ok(HookRegistration::OwnedSource {
            source: if expected.is_some() {
                present.map(str::to_owned)
            } else {
                None
            },
        });
    }
    let root = current.as_object().ok_or(HookError::InvalidConfiguration)?;
    let Some(hooks) = root.get("hooks") else {
        return Ok(HookRegistration::JsonGroups { entries: vec![] });
    };
    let hooks = hooks.as_object().ok_or(HookError::InvalidConfiguration)?;
    for (native_event, groups) in hooks {
        if let Some(groups) = groups.as_array()
            && groups.iter().any(|group| {
                owns_marker(group, registration)
                    && !registration
                        .entries()
                        .iter()
                        .any(|entry| entry.native_event == *native_event && entry.group == *group)
            })
        {
            return Err(HookError::EntryConflict);
        }
    }
    let mut entries = Vec::new();
    for entry in registration.entries() {
        let Some(groups) = hooks.get(&entry.native_event) else {
            continue;
        };
        let groups = groups.as_array().ok_or(HookError::InvalidConfiguration)?;
        let matching: Vec<_> = groups
            .iter()
            .filter(|group| owns_marker(group, registration))
            .collect();
        if matching.iter().any(|group| **group != entry.group) || matching.len() > 1 {
            return Err(HookError::EntryConflict);
        }
        if !matching.is_empty() {
            entries.push(entry.clone());
        }
    }
    Ok(HookRegistration::JsonGroups { entries })
}

pub fn installed(current: &Value, registration: &HookRegistration) -> Result<bool, HookError> {
    Ok(retain_present(current, registration)? == *registration)
}

/// Remove only exact recorded groups; clean empty containers only when they
/// were absent in the original document. Caller owns locking and byte restoration.
pub fn remove(
    current: &Value,
    registration: &HookRegistration,
    original: &Value,
) -> Result<Value, HookError> {
    if let HookRegistration::OwnedSource { source: expected } = registration {
        let present = source(current)?;
        source(original)?;
        if expected.is_none() || present.is_none() {
            return Ok(current.clone());
        }
        if present != expected.as_deref() {
            return Err(HookError::EntryConflict);
        }
        return Ok(original.clone());
    }
    let present = retain_present(current, registration)?;
    let mut document = current.clone();
    let root = document
        .as_object_mut()
        .ok_or(HookError::InvalidConfiguration)?;
    let Some(hooks) = root.get_mut("hooks") else {
        return Ok(document);
    };
    let hooks = hooks
        .as_object_mut()
        .ok_or(HookError::InvalidConfiguration)?;
    for entry in present.entries() {
        let groups = hooks
            .get_mut(&entry.native_event)
            .and_then(Value::as_array_mut)
            .ok_or(HookError::InvalidConfiguration)?;
        groups.retain(|group| *group != entry.group);
    }
    for entry in registration.entries() {
        if hooks
            .get(&entry.native_event)
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
            && original
                .get("hooks")
                .and_then(|hooks| hooks.get(&entry.native_event))
                .is_none()
        {
            hooks.remove(&entry.native_event);
        }
    }
    if hooks.is_empty() && original.get("hooks").is_none() {
        root.remove("hooks");
    }
    Ok(document)
}

fn owns_marker(group: &Value, registration: &HookRegistration) -> bool {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|handlers| {
            handlers.iter().any(|handler| {
                handler["statusMessage"] == "Locust"
                    || registration.entries().iter().any(|entry| {
                        handler["command"].is_string()
                            && handler["command"] == entry.group["hooks"][0]["command"]
                    })
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_proto::api::{CancelItem, Claim, ContextAcknowledgment, PendingWork, WorkItem};
    use locust_proto::event::TaskId;
    use locust_proto::id::{EventId, InstanceId, PublicKey};

    fn parse(client: Client, event: &str, value: &Value) -> NativeInput {
        match parse_input(adapter(client).unwrap(), event, value) {
            Parsed::Input(input) => input,
            other => panic!("{other:?}"),
        }
    }

    fn kind(event: &Event) -> &'static str {
        match event {
            Event::Start => "start",
            Event::Stop { .. } => "stop",
            Event::Tool { .. } => "tool",
        }
    }

    fn wrap(
        client: Client,
        input: &NativeInput,
        outcome: &Outcome,
    ) -> Result<Option<Value>, HookError> {
        envelope(
            adapter(client).unwrap(),
            kind(&input.event),
            &input.native_event_name,
            outcome,
        )
    }

    fn native(event: &str) -> Value {
        json!({"session_id":"chat-1","hook_event_name":event,"source":"compact","stop_hook_active":false,"tool_name":"mcp__locust__locust_wait","tool_use_id":"call-1","tool_input":{"goal":"11".repeat(32),"seen":0,"timeout_ms":0},"tool_response":{"isError":false,"structuredContent":{"ok":true,"result":{"waited":"no_event"}}}})
    }

    fn native_for(client: Client, event: &str) -> Value {
        let mut value = native(event);
        if client == Client::Pi {
            value["hook_event_name"] = json!(match event {
                "SessionStart" => "session_start",
                "Stop" => "agent_before_settle",
                "PostToolUse" => "tool_result",
                _ => event,
            });
            value["tool_response"] = response_for(
                client,
                "mcp__locust__locust_wait",
                value["tool_response"]["structuredContent"].clone(),
            );
        }
        if client == Client::FactoryDroid {
            value["tool_name"] = json!("locust___locust_wait");
            value.as_object_mut().unwrap().remove("tool_use_id");
        }
        if matches!(client, Client::ClaudeCode | Client::FactoryDroid) {
            value["tool_response"] = json!(value["tool_response"]["structuredContent"].to_string());
        }
        value
    }

    fn response_for(client: Client, tool: &str, envelope: Value) -> Value {
        if matches!(client, Client::ClaudeCode | Client::FactoryDroid) {
            return json!(envelope.to_string());
        }
        let mcp = json!({"isError":false,"content":[{"type":"text","text":envelope.to_string()}],"structuredContent":envelope});
        if client == Client::Pi {
            json!({"isError":false,"details":{"server":"locust","tool":tool.strip_prefix("mcp__locust__").unwrap()},"structuredContent":mcp})
        } else {
            mcp
        }
    }

    #[test]
    fn config_and_envelope_goldens_for_all_adapters() {
        for spec in ADAPTERS.iter().filter(|spec| spec.client != Client::Pi) {
            let registration =
                registration(spec.client, Path::new("/tmp/lh.fixture/launch.sh")).unwrap();
            assert_eq!(
                registration.entries()[0].group,
                json!({"hooks":[{"type":"command","command":format!("'/tmp/lh.fixture/launch.sh' hook start --harness {}",spec.harness),"timeout":spec.command_timeout_seconds,"statusMessage":"Locust"}]})
            );
            assert_eq!(
                registration.entries()[1].group,
                json!({"hooks":[{"type":"command","command":format!("'/tmp/lh.fixture/launch.sh' hook stop --harness {}",spec.harness),"timeout":300,"statusMessage":"Locust"}]})
            );
            assert_eq!(registration.entries()[2].native_event, "PostToolUse");
            assert_eq!(
                registration.entries().len(),
                if spec.client == Client::ClaudeCode {
                    4
                } else {
                    3
                }
            );
            let start = parse(
                spec.client,
                "start",
                &native_for(spec.client, "SessionStart"),
            );
            assert_eq!(
                wrap(
                    spec.client,
                    &start,
                    &Outcome {
                        line: Some("Locust: held 1; locust_status".into()),
                        ..Outcome::default()
                    }
                )
                .unwrap(),
                Some(
                    json!({"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"Locust: held 1; locust_status"}})
                )
            );
            let stop = parse(spec.client, "stop", &native_for(spec.client, "Stop"));
            assert_eq!(
                wrap(
                    spec.client,
                    &stop,
                    &Outcome {
                        line: Some("Locust: tasks 1; locust_wait".into()),
                        keep_going: true,
                        wait: false
                    }
                )
                .unwrap(),
                Some(json!({"decision":"block","reason":"Locust: tasks 1; locust_wait"}))
            );
            assert_eq!(
                wrap(
                    spec.client,
                    &stop,
                    &Outcome {
                        line: Some("Locust context was NOT injected".into()),
                        ..Outcome::default()
                    }
                )
                .unwrap(),
                Some(json!({"systemMessage":"Locust context was NOT injected"}))
            );
            assert_eq!(wrap(spec.client, &stop, &Outcome::default()).unwrap(), None);
        }
    }

    #[test]
    fn setup_uses_native_configuration_parser_and_renderer() {
        for spec in ADAPTERS.iter().filter(|spec| spec.client != Client::Pi) {
            assert_eq!(
                parse_configuration(spec.client, None).unwrap(),
                if spec.client == Client::FactoryDroid {
                    json!({"hooks":{}})
                } else {
                    json!({})
                }
            );
            let document = if spec.client == Client::FactoryDroid {
                json!({"hooks":{"Notification":[]}})
            } else {
                json!({"hooks":{},"unrelated":7})
            };
            let bytes = render_configuration(spec.client, &document)
                .unwrap()
                .unwrap();
            assert_eq!(
                parse_configuration(spec.client, Some(&bytes)).unwrap(),
                document
            );
            assert!(parse_configuration(spec.client, Some(b"[]")).is_err());
            assert!(render_configuration(spec.client, &json!([])).is_err());
        }
        assert_eq!(
            parse_configuration(Client::KimiCode, None),
            Err(HookError::UnsupportedClient)
        );
    }

    #[test]
    fn own_groups_round_trip_preserving_original_and_unrelated_edits() {
        for spec in ADAPTERS.iter().filter(|spec| spec.client != Client::Pi) {
            let registration =
                registration(spec.client, Path::new("/tmp/lh.fixture/launch.sh")).unwrap();
            for original in [
                json!({}),
                json!({"hooks":{}}),
                json!({"hooks":{"Stop":[]}}),
                json!({"permissions":{"allow":["Read"]},"hooks":{"Stop":[{"hooks":[{"type":"command","command":"other"}]}]}}),
            ] {
                let installed_document = install(&original, &registration).unwrap();
                assert!(installed(&installed_document, &registration).unwrap());
                assert_eq!(
                    remove(&installed_document, &registration, &original).unwrap(),
                    original
                );
                let mut changed = installed_document;
                changed["unrelated"] = json!(17);
                let removed = remove(&changed, &registration, &original).unwrap();
                let mut expected = original;
                expected["unrelated"] = json!(17);
                assert_eq!(removed, expected);
            }
        }
    }

    #[test]
    fn hand_removed_entries_stay_removed_and_modifications_conflict() {
        let registration =
            registration(Client::Codex, Path::new("/tmp/lh.fixture/launch.sh")).unwrap();
        let original = json!({});
        let mut current = install(&original, &registration).unwrap();
        current["hooks"].as_object_mut().unwrap().remove("Stop");
        let retained = retain_present(&current, &registration).unwrap();
        assert_eq!(retained.entries().len(), 2);
        assert!(!installed(&current, &registration).unwrap());
        let clean = remove(&current, &retained, &original).unwrap();
        assert!(
            install(&clean, &retained).unwrap()["hooks"]
                .get("Stop")
                .is_none()
        );
        current["hooks"]["SessionStart"][0]["hooks"][0]["command"] = json!("changed");
        assert_eq!(
            retain_present(&current, &registration),
            Err(HookError::EntryConflict)
        );
        assert_eq!(
            remove(&current, &registration, &original),
            Err(HookError::EntryConflict)
        );
        assert_eq!(
            install(&current, &registration),
            Err(HookError::EntryConflict)
        );
    }

    #[test]
    fn reapply_updates_in_place_keeps_declined_events_out_and_adds_new_ones() {
        let fresh =
            registration(Client::ClaudeCode, Path::new("/tmp/lh.fixture/launch.sh")).unwrap();
        let mut old = fresh.clone();
        let HookRegistration::JsonGroups { entries } = &mut old else {
            panic!("JSON hooks");
        };
        entries.retain(|entry| entry.native_event != "PostToolUseFailure");
        for entry in entries.iter_mut() {
            entry.group["hooks"][0]["timeout"] = json!(600);
        }
        let other = json!({"hooks":[{"type":"command","command":"after-locust"}]});
        let mut current = install(&json!({}), &old).unwrap();
        current["hooks"]["Stop"]
            .as_array_mut()
            .unwrap()
            .push(other.clone());
        current["hooks"]
            .as_object_mut()
            .unwrap()
            .remove("SessionStart");
        let updated = reapply(&current, &old, &[], &fresh).unwrap();
        assert_eq!(updated.declined, vec!["SessionStart".to_owned()]);
        assert_eq!(updated.document["hooks"]["Stop"][1], other);
        assert_eq!(
            updated.document["hooks"]["Stop"][0]["hooks"][0]["timeout"],
            300
        );
        assert!(updated.document["hooks"].get("SessionStart").is_none());
        assert!(
            updated.document["hooks"]
                .get("PostToolUseFailure")
                .is_some()
        );
        assert!(installed(&updated.document, &updated.registration).unwrap());
        // The next release still leaves the declined event out.
        let again = reapply(
            &updated.document,
            &updated.registration,
            &updated.declined,
            &fresh,
        )
        .unwrap();
        assert_eq!(again.document, updated.document);
        assert_eq!(again.registration, updated.registration);
        // Every owned entry removed: nothing comes back, new events included.
        let bare = json!({"hooks":{"Stop":[other]}});
        let none = reapply(&bare, &old, &[], &fresh).unwrap();
        assert_eq!(none.document, bare);
        assert!(none.registration.entries().is_empty());
    }

    #[test]
    fn subagent_callbacks_are_ignored_and_transcripts_titles_never_enter_core() {
        for spec in ADAPTERS {
            let mut value = native_for(spec.client, "PostToolUse");
            value["transcript_path"] = json!("/forbidden/profile/transcript");
            value["last_assistant_message"] = json!("private arbitrary title");
            for field in spec.subagent_fields {
                let mut child = value.clone();
                child[*field] = json!("child-1");
                assert!(matches!(parse_input(spec, "tool", &child), Parsed::Ignored));
                // A malformed child payload is still a child's: nothing to say.
                child["hook_event_name"] = json!("Unknown");
                assert!(matches!(parse_input(spec, "tool", &child), Parsed::Ignored));
            }
            let parsed = parse(spec.client, "tool", &value);
            assert_eq!(
                parsed.chat,
                ChatIdentity {
                    session_id: "chat-1".into(),
                }
            );
            let Event::Tool {
                own_call: Some(call),
            } = parsed.event
            else {
                panic!("verified call expected");
            };
            assert_eq!(call.operation, "wait");
            if spec.native_tool_use_id {
                assert_eq!(call.invocation_id, "call-1");
            } else {
                assert!(call.invocation_id.starts_with("logical-effect:"));
            }
            assert_eq!(call.action, OwnAction::Other);
            assert_eq!(call.goal, Some(GoalId([0x11; 32])));
        }
        let codex = adapter(Client::Codex).unwrap();
        assert!(matches!(
            parse_input(codex, "stop", &native("PostToolUse")),
            Parsed::Invalid { chat, native_event_name }
                if chat.session_id == "chat-1" && native_event_name == "Stop"
        ));
        let mut nameless = native("PostToolUse");
        nameless.as_object_mut().unwrap().remove("session_id");
        assert!(matches!(
            parse_input(codex, "tool", &nameless),
            Parsed::Ignored
        ));
        assert!(matches!(
            parse_input(codex, "tool", &json!("not an object")),
            Parsed::Ignored
        ));
        // Claude's main thread may name its agent type; only agent_id marks a child.
        let mut main_thread = native("PostToolUse");
        main_thread["agent_type"] = json!("reviewer");
        assert!(matches!(
            parse_input(adapter(Client::ClaudeCode).unwrap(), "tool", &main_thread),
            Parsed::Input(_)
        ));
        assert_eq!(
            adapter(Client::FactoryDroid).unwrap().relative_config_path,
            ".factory/hooks.json"
        );
        assert_eq!(
            adapter(Client::Pi).unwrap().relative_config_path,
            ".pi/agent/extensions/locust.ts"
        );
        assert!(adapter(Client::KimiCode).is_none());
    }

    #[test]
    fn failed_lookalike_and_mismatched_results_never_observe_own_success() {
        let fixtures = [
            ("tool_name", json!("locust_wait")),
            ("tool_name", json!("mcp__other__locust_wait")),
            ("tool_name", json!("mcp__locust__locust_shutdown")),
            (
                "tool_response",
                json!({"isError":true,"structuredContent":{"ok":true,"result":{"waited":"no_event"}}}),
            ),
            (
                "tool_response",
                json!({"isError":false,"structuredContent":{"ok":false,"result":{"waited":"no_event"}}}),
            ),
            (
                "tool_response",
                json!({"isError":false,"structuredContent":{"ok":true,"result":"done"}}),
            ),
            (
                "tool_input",
                json!({"goal":"11".repeat(32),"seen":0,"timeout_ms":0,"extra":"bad"}),
            ),
        ];
        for spec in ADAPTERS {
            for (field, replacement) in &fixtures {
                let mut value = native_for(spec.client, "PostToolUse");
                value[*field] = replacement.clone();
                assert!(matches!(
                    parse(spec.client, "tool", &value).event,
                    Event::Tool { own_call: None }
                ));
            }
        }
        assert!(matches!(
            parse(Client::ClaudeCode, "tool", &native("PostToolUseFailure")).event,
            Event::Tool { own_call: None }
        ));
        assert!(matches!(
            parse_input(
                adapter(Client::Codex).unwrap(),
                "tool",
                &native("PostToolUseFailure")
            ),
            Parsed::Invalid { .. }
        ));
    }

    #[test]
    fn native_successful_tool_string_requires_exact_typed_locust_envelope() {
        let mut input = native_for(Client::ClaudeCode, "PostToolUse");
        assert!(matches!(
            parse(Client::ClaudeCode, "tool", &input).event,
            Event::Tool { own_call: Some(_) }
        ));
        for response in [
            "success".to_owned(),
            json!({"ok":false,"result":{"waited":"no_event"}}).to_string(),
            json!({"ok":true,"result":"done"}).to_string(),
            json!({"ok":true,"result":{"recorded":{"event":"11".repeat(32)}}}).to_string(),
        ] {
            input["tool_response"] = json!(response);
            assert!(matches!(
                parse(Client::ClaudeCode, "tool", &input).event,
                Event::Tool { own_call: None }
            ));
        }
    }

    #[test]
    fn mcp_text_envelope_is_typed_and_no_opaque_text_is_accepted() {
        let mut value = native("PostToolUse");
        value["tool_response"] = json!({"isError":false,"content":[{"type":"text","text":json!({"ok":true,"result":{"waited":"no_event"}}).to_string()}]});
        assert!(matches!(
            parse(Client::Codex, "tool", &value).event,
            Event::Tool { own_call: Some(_) }
        ));
        value["tool_response"]["content"][0]["text"] = json!("success");
        assert!(matches!(
            parse(Client::Codex, "tool", &value).event,
            Event::Tool { own_call: None }
        ));
    }

    #[test]
    fn typed_claim_facts_match_exact_requested_task() {
        let claim = Claim {
            goal: GoalId([0x11; 32]),
            task: TaskId::Authored(EventId([2; 32])),
            attempt: EventId([3; 32]),
            instance: InstanceId([4; 16]),
            generation: 1,
        };
        let mut value = native("PostToolUse");
        value["tool_name"] = json!("mcp__locust__locust_attempt_start");
        value["tool_input"] = json!({"goal":claim.goal,"task":claim.task,"offer":null});
        value["tool_response"] = json!({"isError":false,"structuredContent":{"ok":true,"result":Response::Claimed(claim)}});
        let Event::Tool {
            own_call: Some(call),
        } = parse(Client::Codex, "tool", &value).event
        else {
            panic!("claim expected");
        };
        assert_eq!(call.action, OwnAction::Claimed(claim));
        value["tool_input"]["task"] = json!("77".repeat(32));
        assert!(matches!(
            parse(Client::Codex, "tool", &value).event,
            Event::Tool { own_call: None }
        ));
    }

    #[test]
    fn no_task_start_observes_the_task_in_its_claimed_answer() {
        let claim = Claim {
            goal: GoalId([0x11; 32]),
            task: TaskId::Authored(EventId([2; 32])),
            attempt: EventId([3; 32]),
            instance: InstanceId([4; 16]),
            generation: 1,
        };
        for spec in ADAPTERS {
            let mut value = native_for(spec.client, "PostToolUse");
            value["tool_name"] = json!(format!("{}locust_attempt_start", spec.native_tool_prefix));
            value["tool_input"] = json!({"goal":claim.goal});
            value["tool_response"] = response_for(
                spec.client,
                value["tool_name"].as_str().unwrap(),
                json!({"ok":true,"result":Response::Claimed(claim)}),
            );
            let Event::Tool {
                own_call: Some(call),
            } = parse(spec.client, "tool", &value).event
            else {
                panic!("the no-task start's claim must be observed");
            };
            assert_eq!(call.operation, "attempt.start");
            assert_eq!(call.goal, Some(claim.goal));
            assert_eq!(call.action, OwnAction::Claimed(claim));
            value["tool_input"]["goal"] = json!(GoalId([9; 32]));
            assert!(matches!(
                parse(spec.client, "tool", &value).event,
                Event::Tool { own_call: None }
            ));
        }
    }

    #[test]
    fn no_task_start_observes_pending_without_inventing_a_claim() {
        let goal = GoalId([0x11; 32]);
        for spec in ADAPTERS {
            let mut value = native_for(spec.client, "PostToolUse");
            value["tool_name"] = json!(format!("{}locust_attempt_start", spec.native_tool_prefix));
            value["tool_input"] = json!({"goal":goal});
            value["tool_response"] = response_for(
                spec.client,
                value["tool_name"].as_str().unwrap(),
                json!({"ok":true,"result":Response::Pending(PendingWork::default())}),
            );
            let Event::Tool {
                own_call: Some(call),
            } = parse(spec.client, "tool", &value).event
            else {
                panic!("the pending answer must be observed");
            };
            assert_eq!(call.operation, "attempt.start");
            assert_eq!(call.goal, Some(goal));
            assert_eq!(call.action, OwnAction::Other);
            value["tool_input"]["task"] = json!(TaskId::Authored(EventId([2; 32])));
            assert!(matches!(
                parse(spec.client, "tool", &value).event,
                Event::Tool { own_call: None }
            ));
        }
    }

    #[test]
    fn fake_harness_conformance_has_same_generic_outcomes() {
        use super::core::{GoalWork, Marks, Snapshot, decide};
        let goal = GoalId([0x11; 32]);
        let task = TaskId::Authored(EventId([2; 32]));
        let mut results = Vec::new();
        for spec in ADAPTERS {
            let mut marks = Marks::default();
            let mut snapshot = Snapshot {
                instance: InstanceId([4; 16]),
                goals: vec![GoalWork {
                    goal,
                    pending: PendingWork {
                        to_start: vec![WorkItem {
                            task,
                            offer: None,
                            attempting: vec![],
                            results: 0,
                            unattended: true,
                        }],
                        ..PendingWork::default()
                    },
                }],
                released: Default::default(),
            };
            let tool = parse(spec.client, "tool", &native_for(spec.client, "PostToolUse"));
            let used = decide(tool.event, &snapshot, &mut marks);
            let stop = parse(spec.client, "stop", &native_for(spec.client, "Stop"));
            let first = decide(stop.event.clone(), &snapshot, &mut marks);
            assert!(first.keep_going);
            assert!(wrap(spec.client, &stop, &first).unwrap().is_some());
            let ignored = decide(stop.event, &snapshot, &mut marks);
            assert!(!ignored.keep_going);
            snapshot.goals[0].pending.to_start.clear();
            snapshot.goals[0].pending.claimed.push(Claim {
                goal,
                task,
                attempt: EventId([3; 32]),
                instance: snapshot.instance,
                generation: 1,
            });
            snapshot.goals[0].pending.to_acknowledge.push(CancelItem {
                task,
                attempt: EventId([3; 32]),
                cancel: EventId([5; 32]),
                generation: Some(1),
            });
            let cancelled = decide(
                parse(spec.client, "stop", &native_for(spec.client, "Stop")).event,
                &snapshot,
                &mut marks,
            );
            assert!(cancelled.keep_going);
            snapshot.goals[0].pending.to_acknowledge.clear();
            let compacted = parse(
                spec.client,
                "start",
                &native_for(spec.client, "SessionStart"),
            );
            let start = decide(compacted.event, &snapshot, &mut marks);
            assert!(start.line.as_ref().unwrap().contains("1 held attempts"));
            results.push((used, first, ignored, cancelled, start));
        }
        for outcome in results.iter().skip(1) {
            assert_eq!(results[0], *outcome);
        }
    }

    #[test]
    fn output_cannot_smuggle_a_second_line_or_non_ascii_text() {
        let stop = parse(Client::Codex, "stop", &native("Stop"));
        for line in [
            "line\nsecond".to_owned(),
            "é".to_owned(),
            "x".repeat(512),
            String::new(),
        ] {
            assert_eq!(
                wrap(
                    Client::Codex,
                    &stop,
                    &Outcome {
                        line: Some(line),
                        ..Outcome::default()
                    }
                ),
                Err(HookError::InvalidOutput)
            );
        }
    }

    #[test]
    fn acknowledgment_local_reference_is_validated_without_reading_cache() {
        let goal = GoalId([0x11; 32]);
        let mut value = native("PostToolUse");
        value["tool_name"] = json!("mcp__locust__locust_context_acknowledge");
        value["tool_input"] = json!({"goal":goal,"receipt":format!("ctx:{}","aa".repeat(32))});
        value["tool_response"] = json!({"isError":false,"structuredContent":{"ok":true,"result":Response::ContextAcknowledged(ContextAcknowledgment {goal,principal:PublicKey([3;32]),session:InstanceId([4;16]),entries:vec![]})}});
        let Event::Tool {
            own_call: Some(call),
        } = parse(Client::Codex, "tool", &value).event
        else {
            panic!("successful acknowledgment expected");
        };
        assert_eq!(call.operation, "context.acknowledge");
        assert_eq!(call.goal, Some(goal));
        value["tool_input"]["receipt"] = json!("opaque receipt");
        assert!(matches!(
            parse(Client::Codex, "tool", &value).event,
            Event::Tool { own_call: None }
        ));
    }

    #[test]
    fn native_report_and_cancel_actions_preserve_exact_status_outcome_and_generation() {
        use locust_proto::event::{AttemptStatus, CancelOutcome};
        let attempt = EventId([3; 32]);
        let cancel = EventId([4; 32]);
        for spec in ADAPTERS {
            for status in [
                AttemptStatus::Progress,
                AttemptStatus::Completed,
                AttemptStatus::Failed,
                AttemptStatus::Abandoned,
                AttemptStatus::Uncertain,
            ] {
                let mut input = native_for(spec.client, "PostToolUse");
                input["tool_name"] =
                    json!(format!("{}locust_attempt_report", spec.native_tool_prefix));
                input["tool_input"] = json!({"goal":GoalId([0x11;32]),"attempt":attempt,"generation":7,"status":status,"text":"TITLE\nDo not retain this text"});
                let response =
                    json!({"ok":true,"result":Response::Recorded {event:EventId([5;32])}});
                input["tool_response"] =
                    response_for(spec.client, input["tool_name"].as_str().unwrap(), response);
                let Event::Tool {
                    own_call: Some(call),
                } = parse(spec.client, "tool", &input).event
                else {
                    panic!("validated report expected");
                };
                assert_eq!(
                    call.action,
                    OwnAction::Report {
                        attempt,
                        generation: 7,
                        status
                    }
                );
                assert!(!format!("{call:?}").contains("TITLE"));
            }
            for outcome in [
                CancelOutcome::Stopped,
                CancelOutcome::Completed,
                CancelOutcome::Uncertain,
            ] {
                for generation in [None, Some(7)] {
                    let mut input = native_for(spec.client, "PostToolUse");
                    input["tool_name"] = json!(format!(
                        "{}locust_cancel_acknowledge",
                        spec.native_tool_prefix
                    ));
                    input["tool_input"] = json!({"goal":GoalId([0x11;32]),"cancel":cancel,"generation":generation,"outcome":outcome});
                    let response =
                        json!({"ok":true,"result":Response::Recorded {event:EventId([5;32])}});
                    input["tool_response"] =
                        response_for(spec.client, input["tool_name"].as_str().unwrap(), response);
                    let Event::Tool {
                        own_call: Some(call),
                    } = parse(spec.client, "tool", &input).event
                    else {
                        panic!("validated acknowledgment expected");
                    };
                    assert_eq!(
                        call.action,
                        OwnAction::CancelAcknowledged {
                            cancel,
                            generation,
                            outcome,
                            target: None
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn droid_configuration_and_envelope_goldens_are_native_unwrapped_json() {
        let client = Client::FactoryDroid;
        let registration = registration(client, Path::new("/tmp/lh.fixture/locust-cli")).unwrap();
        let current = parse_configuration(client, None).unwrap();
        let installed = install(&current, &registration).unwrap();
        let bytes = render_configuration(client, &installed).unwrap().unwrap();
        let native: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            native,
            json!({
                "SessionStart":[{"hooks":[{"type":"command","command":"'/tmp/lh.fixture/locust-cli' hook start --harness droid","timeout":30,"statusMessage":"Locust"}]}],
                "Stop":[{"hooks":[{"type":"command","command":"'/tmp/lh.fixture/locust-cli' hook stop --harness droid","timeout":300,"statusMessage":"Locust"}]}],
                "PostToolUse":[{"hooks":[{"type":"command","command":"'/tmp/lh.fixture/locust-cli' hook tool --harness droid","timeout":30,"statusMessage":"Locust"}]}],
            })
        );
        assert!(native.get("hooks").is_none());
        assert_eq!(
            parse_configuration(client, Some(&bytes)).unwrap(),
            installed
        );
        assert_eq!(
            render_configuration(client, &json!({})).unwrap(),
            Some(b"{}\n".to_vec())
        );
        assert!(parse_configuration(client, Some(b"{\"hooks\":{}}")).is_err());
        let input = parse(client, "tool", &native_for(client, "PostToolUse"));
        assert_eq!(input.chat.session_id, "chat-1");
        assert_eq!(
            wrap(
                client,
                &input,
                &Outcome {
                    line: Some("Locust: claim lost. Use locust_pending.".into()),
                    ..Outcome::default()
                }
            )
            .unwrap(),
            Some(
                json!({"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"Locust: claim lost. Use locust_pending."}})
            )
        );
    }

    #[test]
    fn droid_logical_effect_identity_deduplicates_replays_and_distinguishes_typed_effects() {
        let client = Client::FactoryDroid;
        let mut report = native_for(client, "PostToolUse");
        report["tool_name"] = json!("locust___locust_attempt_report");
        report["tool_input"] = json!({"goal":GoalId([0x11;32]),"attempt":EventId([3;32]),"generation":1,"status":"progress","text":"original prose"});
        let recorded =
            |event| json!(json!({"ok":true,"result":Response::Recorded {event}}).to_string());
        report["tool_response"] = recorded(EventId([5; 32]));
        let identity = |value: &Value| {
            let Event::Tool {
                own_call: Some(call),
            } = parse(client, "tool", value).event
            else {
                panic!("validated Droid effect expected");
            };
            call.invocation_id
        };
        let first = identity(&report);
        assert_eq!(first, identity(&report));
        report["tool_input"]["text"] = json!("different prose, same typed authored event");
        report["tool_use_id"] = json!("untrusted-native-id");
        report["message_id"] = json!("another-native-message");
        assert_eq!(first, identity(&report));
        report["tool_response"] = recorded(EventId([6; 32]));
        assert_ne!(first, identity(&report));
        let mut claimed = native_for(client, "PostToolUse");
        claimed["tool_name"] = json!("locust___locust_attempt_start");
        let mut claim = Claim {
            goal: GoalId([0x11; 32]),
            task: TaskId::Authored(EventId([3; 32])),
            attempt: EventId([4; 32]),
            instance: InstanceId([5; 16]),
            generation: 1,
        };
        claimed["tool_input"] = json!({"goal":claim.goal,"task":claim.task,"offer":null});
        claimed["tool_response"] =
            json!(json!({"ok":true,"result":Response::Claimed(claim)}).to_string());
        let claim_id = identity(&claimed);
        claim.generation = 2;
        claimed["tool_response"] =
            json!(json!({"ok":true,"result":Response::Claimed(claim)}).to_string());
        assert_ne!(claim_id, identity(&claimed));
        let mut acknowledgment = native_for(client, "PostToolUse");
        acknowledgment["tool_name"] = json!("locust___locust_context_acknowledge");
        acknowledgment["tool_input"] =
            json!({"goal":claim.goal,"receipt":format!("ctx:{}","aa".repeat(32))});
        acknowledgment["tool_response"]=json!(json!({"ok":true,"result":Response::ContextAcknowledged(ContextAcknowledgment {goal:claim.goal,principal:PublicKey([6;32]),session:claim.instance,entries:vec![]})}).to_string());
        let receipt_id = identity(&acknowledgment);
        acknowledgment["tool_input"]["receipt"] = json!(format!("ctx:{}", "bb".repeat(32)));
        assert_ne!(receipt_id, identity(&acknowledgment));
    }

    #[test]
    fn executable_shell_quoting_and_expansion_rejection() {
        let registration = registration(
            Client::ClaudeCode,
            Path::new("/tmp/lh.fixture/a'b $(touch X)`command`/launch.sh"),
        )
        .unwrap();
        assert_eq!(
            registration.entries()[0].group["hooks"][0]["command"],
            "'/tmp/lh.fixture/a'\\''b $(touch X)`command`/launch.sh' hook start --harness claude"
        );
        for path in ["relative", "/tmp/${HOME}/launch", "/tmp/line\nlaunch"] {
            assert_eq!(
                super::registration(Client::ClaudeCode, Path::new(path)),
                Err(HookError::InvalidExecutable)
            );
        }
    }
}
