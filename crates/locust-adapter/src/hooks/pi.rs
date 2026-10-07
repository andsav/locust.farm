//! Pi's source extension and normalized native payload. No daemon or profile I/O.
//! Native contracts are pinned to earendil-works/pi 2db5e359bf84c1c0be51d2c5c5c5c7cf27072c2b.
use super::{HookAdapter, HookError, HookRegistration, NativeEvent, core};
use crate::config::Client;
use serde_json::{Value, json};
use std::path::Path;

pub(super) const EVENTS: &[NativeEvent] = &[
    NativeEvent {
        native_name: "session_start",
        event: "start",
    },
    NativeEvent {
        native_name: "session_compact",
        event: "start",
    },
    NativeEvent {
        native_name: "agent_before_settle",
        event: "stop",
    },
    NativeEvent {
        native_name: "tool_result",
        event: "tool",
    },
];
pub(super) const ADAPTER: HookAdapter = HookAdapter {
    client: Client::Pi,
    harness: "pi",
    relative_config_path: ".pi/agent/extensions/locust.ts",
    events: EVENTS,
    native_tool_prefix: "mcp__locust__",
    native_tool_use_id: true,
    stop_timeout_seconds: 300,
    command_timeout_seconds: 300,
    wait_limit_ms: 270_000,
};

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

pub(super) fn parse_configuration(bytes: Option<&[u8]>) -> Result<Value, HookError> {
    let source = bytes
        .map(std::str::from_utf8)
        .transpose()
        .map_err(|_| HookError::InvalidConfiguration)?;
    Ok(json!({"source": source}))
}

pub(super) fn render_configuration(document: &Value) -> Result<Option<Vec<u8>>, HookError> {
    Ok(source(document)?.map(|source| source.as_bytes().to_vec()))
}

pub(super) fn registration(
    spec: &HookAdapter,
    launcher: &Path,
) -> Result<HookRegistration, HookError> {
    let path = launcher
        .to_str()
        .filter(|path| launcher.is_absolute() && !path.chars().any(char::is_control))
        .ok_or(HookError::InvalidExecutable)?;
    let failure = core::failure().line.expect("core failure line");
    // Insert the path last so marker-like text in quoted path data cannot
    // be scanned again as a template placeholder.
    let rendered = include_str!("pi-shim.ts")
        .replace(
            "__LOCUST_HOOK_TIMEOUT_MS__",
            &(u64::from(spec.command_timeout_seconds) * 1000).to_string(),
        )
        .replace(
            "__LOCUST_FAILURE_LINE_JSON__",
            &serde_json::to_string(&failure).map_err(|_| HookError::InvalidConfiguration)?,
        )
        .replace(
            "__LOCUST_LAUNCHER_JSON__",
            &serde_json::to_string(path).map_err(|_| HookError::InvalidExecutable)?,
        );
    Ok(HookRegistration::OwnedSource {
        source: Some(rendered),
    })
}

pub(super) fn install(current: &Value, expected: Option<&str>) -> Result<Value, HookError> {
    if expected.is_none() {
        source(current)?;
        return Ok(current.clone());
    }
    if source(current)?.is_some() {
        return Err(HookError::EntryConflict);
    }
    Ok(json!({"source": expected}))
}

pub(super) fn retain_present(
    current: &Value,
    expected: Option<&str>,
) -> Result<HookRegistration, HookError> {
    let present = source(current)?;
    if expected.is_some() && present.is_some() && present != expected {
        return Err(HookError::EntryConflict);
    }
    Ok(HookRegistration::OwnedSource {
        source: if expected.is_some() {
            present.map(str::to_owned)
        } else {
            None
        },
    })
}

pub(super) fn remove(
    current: &Value,
    expected: Option<&str>,
    original: &Value,
) -> Result<Value, HookError> {
    let present = source(current)?;
    source(original)?;
    if expected.is_none() || present.is_none() {
        return Ok(current.clone());
    }
    if present != expected {
        return Err(HookError::EntryConflict);
    }
    Ok(original.clone())
}

/// Pi puts the entire MCP CallToolResult inside native structuredContent. Check
/// both native error status and exact server/tool metadata before unwrapping it
/// for the shared typed Locust request/response extractor.
pub(super) fn own_call_projection(value: &Value) -> Option<Value> {
    let response = &value["tool_response"];
    if response["isError"].as_bool()? {
        return None;
    }
    let details = response.get("details")?.as_object()?;
    if details.get("server")?.as_str()? != "locust" {
        return None;
    }
    let tool = details.get("tool")?.as_str()?;
    if value["tool_name"].as_str()? != format!("mcp__locust__{tool}") {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_roundtrip_distinguishes_absence_empty_and_valid_extension() {
        for bytes in [
            None,
            Some(b"".as_slice()),
            Some(b"export default () => {};\n".as_slice()),
        ] {
            let document = parse_configuration(bytes).unwrap();
            assert_eq!(render_configuration(&document).unwrap().as_deref(), bytes);
        }
        assert!(parse_configuration(Some(&[0xff])).is_err());
        for bad in [
            json!({}),
            json!({"source":1}),
            json!({"source":null,"other":true}),
            json!([]),
        ] {
            assert!(render_configuration(&bad).is_err());
        }
    }

    #[test]
    fn source_ownership_refuses_existing_or_modified_files_and_preserves_hand_removal() {
        let original = parse_configuration(None).unwrap();
        let installed = install(&original, Some("owned source")).unwrap();
        for current in [
            parse_configuration(Some(b"")).unwrap(),
            json!({"source":"unrelated source"}),
            installed.clone(),
        ] {
            assert_eq!(
                install(&current, Some("owned source")),
                Err(HookError::EntryConflict)
            );
        }
        assert_eq!(
            remove(&installed, Some("owned source"), &original).unwrap(),
            original
        );
        assert_eq!(render_configuration(&original).unwrap(), None);
        assert_eq!(
            retain_present(&original, Some("owned source")).unwrap(),
            HookRegistration::OwnedSource { source: None }
        );
        assert_eq!(
            remove(&original, Some("owned source"), &original).unwrap(),
            original
        );
        let changed = json!({"source":"user edits"});
        assert_eq!(
            retain_present(&changed, Some("owned source")),
            Err(HookError::EntryConflict)
        );
        assert_eq!(
            remove(&changed, Some("owned source"), &original),
            Err(HookError::EntryConflict)
        );
        assert_eq!(
            retain_present(&changed, None).unwrap(),
            HookRegistration::OwnedSource { source: None }
        );
        assert_eq!(remove(&changed, None, &original).unwrap(), changed);
    }

    #[test]
    fn generator_embeds_only_literal_paths_metadata_and_the_core_failure_line() {
        let path = Path::new("/tmp/lh.fixture/cli ' ${HOME} $& `touch no` ");
        let HookRegistration::OwnedSource {
            source: Some(source),
        } = registration(&ADAPTER, path).unwrap()
        else {
            panic!("source registration")
        };
        assert!(source.contains(&format!(
            "const launcher = {};",
            serde_json::to_string(path.to_str().unwrap()).unwrap()
        )));
        assert!(source.contains("const hookTimeoutMs = 300000;"));
        assert!(source.contains(&serde_json::to_string(&core::failure().line.unwrap()).unwrap()));
        assert!(!source.contains("__LOCUST_"));
        assert!(source.contains("shell: false"));
        assert!(registration(&ADAPTER, Path::new("relative")).is_err());
        assert!(registration(&ADAPTER, Path::new("/tmp/new\nline")).is_err());
    }

    #[test]
    fn template_marker_text_in_literal_launcher_paths_is_not_rewritten() {
        let path = Path::new(
            "/tmp/lh.fixture/__LOCUST_HOOK_TIMEOUT_MS__/__LOCUST_FAILURE_LINE_JSON__/__LOCUST_LAUNCHER_JSON__",
        );
        let HookRegistration::OwnedSource {
            source: Some(source),
        } = registration(&ADAPTER, path).unwrap()
        else {
            panic!("source registration");
        };
        assert!(source.contains(&format!(
            "const launcher = {};",
            serde_json::to_string(path.to_str().unwrap()).unwrap()
        )));
        assert!(source.contains("const hookTimeoutMs = 300000;"));
        assert!(source.contains(&format!(
            "const failureLine = {};",
            serde_json::to_string(&core::failure().line.unwrap()).unwrap()
        )));
    }

    fn native() -> Value {
        json!({"session_id":"chat","hook_event_name":"tool_result","tool_name":"mcp__locust__locust_wait","tool_use_id":"script/1","parent_tool_call_id":"script","tool_input":{"goal":"11".repeat(32),"seen":0,"timeout_ms":0},"tool_response":{"isError":false,"details":{"server":"locust","tool":"locust_wait"},"structuredContent":{"isError":false,"content":[{"type":"text","text":"model output must not establish a call"}],"structuredContent":{"ok":true,"result":{"waited":"no_event"}}}}})
    }

    #[test]
    fn nested_mcp_projection_retains_call_identity_and_only_unwraps_matching_success() {
        let value = native();
        let projected = own_call_projection(&value).unwrap();
        assert_eq!(projected["tool_use_id"], "script/1");
        assert_eq!(projected["parent_tool_call_id"], "script");
        assert_eq!(
            projected["tool_response"],
            value["tool_response"]["structuredContent"]
        );
        for mutation in 0..5 {
            let mut changed = value.clone();
            match mutation {
                0 => changed["tool_response"]["isError"] = json!(true),
                1 => changed["tool_response"]["structuredContent"]["isError"] = json!(true),
                2 => changed["tool_response"]["details"]["server"] = json!("other"),
                3 => changed["tool_response"]["details"]["tool"] = json!("locust_status"),
                _ => {
                    changed["tool_response"]["structuredContent"] =
                        json!({"ok":true,"result":{"waited":"no_event"}})
                }
            }
            assert!(own_call_projection(&changed).is_none());
        }
    }

    #[test]
    fn native_event_and_output_goldens_include_compaction_and_nested_tool_calls() {
        use crate::hooks::{Event, Outcome, envelope, parse_input};
        for (name, expected, compacted) in [
            ("session_start", "start", false),
            ("session_compact", "start", true),
            ("agent_before_settle", "stop", false),
        ] {
            let value = json!({"hook_event_name":name,"session_id":"native-chat","source":if compacted {"compact"} else {"startup"}});
            let input = parse_input(Client::Pi, expected, &value).unwrap();
            assert_eq!(input.compacted, compacted);
            let outcome = Outcome {
                line: Some("Locust: fixed context line".into()),
                keep_going: expected == "stop",
                wait: false,
            };
            assert_eq!(
                envelope(&input, &outcome).unwrap(),
                Some(json!({"line":"Locust: fixed context line","keep_going":expected=="stop"}))
            );
            assert_eq!(envelope(&input, &Outcome::default()).unwrap(), None);
        }
        let input = parse_input(Client::Pi, "tool", &native()).unwrap();
        let Event::Tool {
            own_call: Some(call),
        } = input.event
        else {
            panic!("native nested Locust call");
        };
        assert_eq!(call.invocation_id, "script/1");
        assert_eq!(call.operation, "wait");
        let mut failed = native();
        failed["tool_response"]["isError"] = json!(true);
        assert!(matches!(
            parse_input(Client::Pi, "tool", &failed).unwrap().event,
            Event::Tool { own_call: None }
        ));
    }
}
