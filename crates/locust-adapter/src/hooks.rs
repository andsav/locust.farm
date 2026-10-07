//! Pure native hook adapters. Callers supply input and configuration documents;
//! this module never reads profiles, transcripts, credentials or files.
//!
//! Native contracts: <https://learn.chatgpt.com/docs/hooks> and
//! <https://code.claude.com/docs/en/hooks>. These adapters cover root lifecycle
//! hooks. Identifiable subagent callbacks are refused: the shared execution
//! session's claims cannot establish a subagent's ownership. Codex 0.153.4
//! serializes agent_id on subagent tool hooks (the common-field docs omit it):
//! <https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/hooks/src/events/post_tool_use.rs>.

pub mod core;
pub use core::{Event, Outcome, OwnCall};

use std::fmt;
use std::path::Path;

use locust_proto::api::{OPERATIONS, Request, Response};
use locust_proto::event::AttemptStatus;
use locust_proto::id::{GoalId, IdempotencyKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config::Client;

#[derive(Clone, Copy, Debug)]
pub struct HookAdapter {
    pub client: Client,
    pub harness: &'static str,
    /// Relative to the explicitly selected profile home.
    pub relative_config_path: &'static str,
    pub events: &'static [NativeEvent],
    /// The stop command gets 300 seconds; the runtime leaves 30 for transport.
    pub stop_timeout_seconds: u32,
    pub wait_limit_ms: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct NativeEvent {
    pub native_name: &'static str,
    pub event: &'static str,
}

const EVENTS: &[NativeEvent] = &[
    NativeEvent {
        native_name: "SessionStart",
        event: "start",
    },
    NativeEvent {
        native_name: "Stop",
        event: "stop",
    },
    NativeEvent {
        native_name: "PostToolUse",
        event: "tool",
    },
];
const CLAUDE_EVENTS: &[NativeEvent] = &[
    NativeEvent {
        native_name: "SessionStart",
        event: "start",
    },
    NativeEvent {
        native_name: "Stop",
        event: "stop",
    },
    NativeEvent {
        native_name: "PostToolUse",
        event: "tool",
    },
    NativeEvent {
        native_name: "PostToolUseFailure",
        event: "tool",
    },
];
pub const ADAPTERS: &[HookAdapter] = &[
    HookAdapter {
        client: Client::Codex,
        harness: "codex",
        relative_config_path: ".codex/hooks.json",
        events: EVENTS,
        stop_timeout_seconds: 300,
        wait_limit_ms: 270_000,
    },
    HookAdapter {
        client: Client::ClaudeCode,
        harness: "claude",
        relative_config_path: ".claude/settings.json",
        events: CLAUDE_EVENTS,
        stop_timeout_seconds: 300,
        wait_limit_ms: 270_000,
    },
];

pub fn adapter(client: Client) -> Option<&'static HookAdapter> {
    ADAPTERS.iter().find(|adapter| adapter.client == client)
}

/// A normalized document is the adapter interface; setup does not own native
/// serialization. Current adapters both use JSON, and an absent file is empty.
pub fn parse_configuration(client: Client, bytes: Option<&[u8]>) -> Result<Value, HookError> {
    adapter(client).ok_or(HookError::UnsupportedClient)?;
    let value = match bytes {
        None => json!({}),
        Some(bytes) => {
            serde_json::from_slice(bytes).map_err(|_| HookError::InvalidConfiguration)?
        }
    };
    if !value.is_object() {
        return Err(HookError::InvalidConfiguration);
    }
    Ok(value)
}

pub fn render_configuration(client: Client, document: &Value) -> Result<Vec<u8>, HookError> {
    adapter(client).ok_or(HookError::UnsupportedClient)?;
    if !document.is_object() {
        return Err(HookError::InvalidConfiguration);
    }
    let mut bytes =
        serde_json::to_vec_pretty(document).map_err(|_| HookError::InvalidConfiguration)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatIdentity {
    pub session_id: String,
    pub agent_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NativeInput {
    pub client: Client,
    pub chat: ChatIdentity,
    pub event: Event,
    pub native_event_name: String,
    pub compacted: bool,
    pub stop_hook_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookError {
    UnsupportedClient,
    InvalidInput,
    InvalidOutput,
    InvalidConfiguration,
    EntryConflict,
    InvalidExecutable,
}
impl fmt::Display for HookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedClient => "this client has no Locust hook adapter",
            Self::InvalidInput => "invalid native hook input",
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
pub fn parse_input(
    client: Client,
    expected_event: &str,
    value: &Value,
) -> Result<NativeInput, HookError> {
    let adapter = adapter(client).ok_or(HookError::UnsupportedClient)?;
    let native_name = required_string(value, "hook_event_name")?;
    let mapping = adapter
        .events
        .iter()
        .find(|event| event.native_name == native_name && event.event == expected_event)
        .ok_or(HookError::InvalidInput)?;
    let session_id = required_string(value, "session_id")?.to_owned();
    let agent_id = optional_string(value, "agent_id")?;
    if agent_id.is_some()
        || (client == Client::Codex && optional_string(value, "agent_type")?.is_some())
    {
        return Err(HookError::InvalidInput);
    }
    let stop_hook_active = match value.get("stop_hook_active") {
        None => false,
        Some(Value::Bool(active)) => *active,
        _ => return Err(HookError::InvalidInput),
    };
    let event = match mapping.event {
        "start" => Event::Start,
        "stop" => Event::Stop,
        "tool" => {
            required_string(value, "tool_name")?;
            required_string(value, "tool_use_id")?;
            Event::Tool {
                own_call: if native_name == "PostToolUse" {
                    own_call(client, value).map(Box::new)
                } else {
                    None
                },
            }
        }
        _ => unreachable!("adapter table contains generic events"),
    };
    Ok(NativeInput {
        client,
        chat: ChatIdentity {
            session_id,
            agent_id,
        },
        event,
        native_event_name: native_name.to_owned(),
        compacted: value.get("source").and_then(Value::as_str) == Some("compact"),
        stop_hook_active,
    })
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, HookError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or(HookError::InvalidInput)
}
fn optional_string(value: &Value, field: &str) -> Result<Option<String>, HookError> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if !text.is_empty() => Ok(Some(text.clone())),
        _ => Err(HookError::InvalidInput),
    }
}

fn own_call(client: Client, value: &Value) -> Option<OwnCall> {
    // No bare tool-name or vendor/plugin prefix aliases: only the configured
    // server name and the operation registry can establish a Locust call.
    let tool = value["tool_name"].as_str()?.strip_prefix("mcp__locust__")?;
    let operation = OPERATIONS
        .iter()
        .find(|operation| operation.tool && operation.tool_name() == tool)?;
    let mut args = value["tool_input"].as_object()?.clone();
    if let Some(key) = args.remove("idempotency_key") {
        serde_json::from_value::<Option<IdempotencyKey>>(key).ok()?;
    }
    let result = successful_result(client, &value["tool_response"])?;
    let (goal, claim, finished_attempt) = if operation.name == "context.acknowledge" {
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
        (Some(args.goal), None, None)
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
        let claim = match response {
            Response::Claimed(claim) => Some(claim),
            _ => None,
        };
        if let Some(claim) = &claim {
            if request.goal() != Some(claim.goal) {
                return None;
            }
            match &request {
                Request::AttemptStart { task, .. } if *task != claim.task => return None,
                Request::AttemptTakeover { attempt, .. } if *attempt != claim.attempt => {
                    return None;
                }
                _ => {}
            }
        }
        let finished = match &request {
            Request::AttemptReport {
                attempt,
                status: AttemptStatus::Completed | AttemptStatus::Failed | AttemptStatus::Abandoned,
                ..
            } => Some(*attempt),
            _ => None,
        };
        (request.goal(), claim, finished)
    };
    Some(OwnCall {
        invocation_id: value["tool_use_id"].as_str()?.to_owned(),
        operation: operation.name.to_owned(),
        goal,
        claim,
        finished_attempt,
    })
}

fn successful_result(client: Client, response: &Value) -> Option<Value> {
    // Native successful MCP callbacks may project the server's text envelope.
    // It is still checked as the exact typed Locust result below.
    let envelope = if client == Client::ClaudeCode && response.is_string() {
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

/// Wrapping never adds instructions or native response text. Stop informational
/// lines use systemMessage: additionalContext itself continues some harnesses.
pub fn envelope(input: &NativeInput, outcome: &Outcome) -> Result<Option<Value>, HookError> {
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
    if outcome.keep_going {
        if !matches!(input.event, Event::Stop) {
            return Err(HookError::InvalidOutput);
        }
        Ok(Some(json!({"decision":"block","reason":line})))
    } else if matches!(input.event, Event::Stop) {
        Ok(Some(json!({"systemMessage":line})))
    } else {
        Ok(Some(
            json!({"hookSpecificOutput":{"hookEventName":input.native_event_name,"additionalContext":line}}),
        ))
    }
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
}

impl HookRegistration {
    pub fn entries(&self) -> &[HookEntry] {
        let Self::JsonGroups { entries } = self;
        entries
    }
}

pub fn registration(client: Client, executable: &Path) -> Result<HookRegistration, HookError> {
    let adapter = adapter(client).ok_or(HookError::UnsupportedClient)?;
    let path = executable
        .to_str()
        .filter(|path| {
            executable.is_absolute() && !path.chars().any(char::is_control) && !path.contains("${")
        })
        .ok_or(HookError::InvalidExecutable)?;
    let quoted = format!("'{}'", path.replace('\'', "'\\''"));
    Ok(HookRegistration::JsonGroups { entries: adapter.events.iter().map(|event| {
        let timeout = if event.event == "stop" { adapter.stop_timeout_seconds } else { 600 };
        HookEntry { native_event: event.native_name.to_owned(), group: json!({"hooks":[{"type":"command","command":format!("{quoted} hook {} --harness {}", event.event, adapter.harness),"timeout":timeout,"statusMessage":"Locust"}]}) }
    }).collect() })
}

/// Install only fresh own groups; existing Locust groups are conflicts. Reapply
/// should first remove the retained exact groups, then install their replacement.
pub fn install(current: &Value, registration: &HookRegistration) -> Result<Value, HookError> {
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

/// Keep entries that still exist exactly, preserving intentional hand-removal.
pub fn retain_present(
    current: &Value,
    registration: &HookRegistration,
) -> Result<HookRegistration, HookError> {
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

    fn native(event: &str) -> Value {
        json!({"session_id":"chat-1","hook_event_name":event,"source":"compact","stop_hook_active":false,"tool_name":"mcp__locust__locust_wait","tool_use_id":"call-1","tool_input":{"goal":"11".repeat(32),"seen":0,"timeout_ms":0},"tool_response":{"isError":false,"structuredContent":{"ok":true,"result":{"waited":"no_event"}}}})
    }

    fn native_for(client: Client, event: &str) -> Value {
        let mut value = native(event);
        if client == Client::ClaudeCode {
            value["tool_response"] = json!(value["tool_response"]["structuredContent"].to_string());
        }
        value
    }

    #[test]
    fn config_and_envelope_goldens_for_both_vendors() {
        for spec in ADAPTERS {
            let registration =
                registration(spec.client, Path::new("/tmp/lh.fixture/launch.sh")).unwrap();
            assert_eq!(
                registration.entries()[0].group,
                json!({"hooks":[{"type":"command","command":format!("'/tmp/lh.fixture/launch.sh' hook start --harness {}",spec.harness),"timeout":600,"statusMessage":"Locust"}]})
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
            let start = parse_input(
                spec.client,
                "start",
                &native_for(spec.client, "SessionStart"),
            )
            .unwrap();
            assert!(start.compacted);
            assert_eq!(
                envelope(
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
            let stop = parse_input(spec.client, "stop", &native_for(spec.client, "Stop")).unwrap();
            assert_eq!(
                envelope(
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
                envelope(
                    &stop,
                    &Outcome {
                        line: Some("Locust context was NOT injected".into()),
                        ..Outcome::default()
                    }
                )
                .unwrap(),
                Some(json!({"systemMessage":"Locust context was NOT injected"}))
            );
            assert_eq!(envelope(&stop, &Outcome::default()).unwrap(), None);
        }
    }

    #[test]
    fn setup_uses_native_configuration_parser_and_renderer() {
        for spec in ADAPTERS {
            assert_eq!(parse_configuration(spec.client, None).unwrap(), json!({}));
            let document = json!({"hooks":{},"unrelated":7});
            let bytes = render_configuration(spec.client, &document).unwrap();
            assert_eq!(
                parse_configuration(spec.client, Some(&bytes)).unwrap(),
                document
            );
            assert!(parse_configuration(spec.client, Some(b"[]")).is_err());
            assert!(render_configuration(spec.client, &json!([])).is_err());
        }
        assert_eq!(
            parse_configuration(Client::Pi, None),
            Err(HookError::UnsupportedClient)
        );
    }

    #[test]
    fn own_groups_round_trip_preserving_original_and_unrelated_edits() {
        for spec in ADAPTERS {
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
    fn subagent_identity_is_refused_and_transcripts_titles_never_enter_core() {
        for spec in ADAPTERS {
            let mut value = native_for(spec.client, "PostToolUse");
            value["agent_id"] = json!("child-1");
            value["transcript_path"] = json!("/forbidden/profile/transcript");
            value["last_assistant_message"] = json!("private arbitrary title");
            assert!(parse_input(spec.client, "tool", &value).is_err());
            value.as_object_mut().unwrap().remove("agent_id");
            let parsed = parse_input(spec.client, "tool", &value).unwrap();
            assert_eq!(
                parsed.chat,
                ChatIdentity {
                    session_id: "chat-1".into(),
                    agent_id: None
                }
            );
            let Event::Tool {
                own_call: Some(call),
            } = parsed.event
            else {
                panic!("verified call expected");
            };
            assert_eq!(call.operation, "wait");
            assert_eq!(call.invocation_id, "call-1");
            assert!(call.claim.is_none());
            assert_eq!(call.goal, Some(GoalId([0x11; 32])));
        }
        assert!(parse_input(Client::Codex, "stop", &native("PostToolUse")).is_err());
        let mut malformed_child = native("PostToolUse");
        malformed_child["agent_type"] = json!("default");
        assert!(parse_input(Client::Codex, "tool", &malformed_child).is_err());
        assert!(adapter(Client::FactoryDroid).is_none());
        assert!(adapter(Client::Pi).is_none());
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
                    parse_input(spec.client, "tool", &value).unwrap().event,
                    Event::Tool { own_call: None }
                ));
            }
        }
        assert!(matches!(
            parse_input(Client::ClaudeCode, "tool", &native("PostToolUseFailure"))
                .unwrap()
                .event,
            Event::Tool { own_call: None }
        ));
        assert!(parse_input(Client::Codex, "tool", &native("PostToolUseFailure")).is_err());
    }

    #[test]
    fn native_successful_tool_string_requires_exact_typed_locust_envelope() {
        let mut input = native_for(Client::ClaudeCode, "PostToolUse");
        assert!(matches!(
            parse_input(Client::ClaudeCode, "tool", &input)
                .unwrap()
                .event,
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
                parse_input(Client::ClaudeCode, "tool", &input)
                    .unwrap()
                    .event,
                Event::Tool { own_call: None }
            ));
        }
    }

    #[test]
    fn mcp_text_envelope_is_typed_and_no_opaque_text_is_accepted() {
        let mut value = native("PostToolUse");
        value["tool_response"] = json!({"isError":false,"content":[{"type":"text","text":json!({"ok":true,"result":{"waited":"no_event"}}).to_string()}]});
        assert!(matches!(
            parse_input(Client::Codex, "tool", &value).unwrap().event,
            Event::Tool { own_call: Some(_) }
        ));
        value["tool_response"]["content"][0]["text"] = json!("success");
        assert!(matches!(
            parse_input(Client::Codex, "tool", &value).unwrap().event,
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
        } = parse_input(Client::Codex, "tool", &value).unwrap().event
        else {
            panic!("claim expected");
        };
        assert_eq!(call.claim, Some(claim));
        value["tool_input"]["task"] = json!("77".repeat(32));
        assert!(matches!(
            parse_input(Client::Codex, "tool", &value).unwrap().event,
            Event::Tool { own_call: None }
        ));
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
            };
            let tool =
                parse_input(spec.client, "tool", &native_for(spec.client, "PostToolUse")).unwrap();
            let used = decide(tool.event, &snapshot, &mut marks);
            let stop = parse_input(spec.client, "stop", &native_for(spec.client, "Stop")).unwrap();
            let first = decide(stop.event.clone(), &snapshot, &mut marks);
            assert!(first.keep_going);
            assert!(envelope(&stop, &first).unwrap().is_some());
            let ignored = decide(stop.event, &snapshot, &mut marks);
            assert!(!ignored.keep_going);
            snapshot.goals[0].pending.to_start.clear();
            snapshot.goals[0].pending.to_acknowledge.push(CancelItem {
                task,
                attempt: EventId([3; 32]),
                cancel: EventId([5; 32]),
                generation: Some(1),
            });
            let cancelled = decide(
                parse_input(spec.client, "stop", &native_for(spec.client, "Stop"))
                    .unwrap()
                    .event,
                &snapshot,
                &mut marks,
            );
            assert!(cancelled.keep_going);
            snapshot.goals[0].pending.to_acknowledge.clear();
            snapshot.goals[0].pending.claimed.push(Claim {
                goal,
                task,
                attempt: EventId([3; 32]),
                instance: snapshot.instance,
                generation: 1,
            });
            let compacted = parse_input(
                spec.client,
                "start",
                &native_for(spec.client, "SessionStart"),
            )
            .unwrap();
            let start = decide(compacted.event, &snapshot, &mut marks);
            assert!(start.line.as_ref().unwrap().contains("1 held attempts"));
            results.push((used, first, ignored, cancelled, start));
        }
        assert_eq!(results[0], results[1]);
    }

    #[test]
    fn output_cannot_smuggle_a_second_line_or_non_ascii_text() {
        let stop = parse_input(Client::Codex, "stop", &native("Stop")).unwrap();
        for line in [
            "line\nsecond".to_owned(),
            "é".to_owned(),
            "x".repeat(512),
            String::new(),
        ] {
            assert_eq!(
                envelope(
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
        } = parse_input(Client::Codex, "tool", &value).unwrap().event
        else {
            panic!("successful acknowledgment expected");
        };
        assert_eq!(call.operation, "context.acknowledge");
        assert_eq!(call.goal, Some(goal));
        value["tool_input"]["receipt"] = json!("opaque receipt");
        assert!(matches!(
            parse_input(Client::Codex, "tool", &value).unwrap().event,
            Event::Tool { own_call: None }
        ));
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
