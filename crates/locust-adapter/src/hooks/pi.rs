//! Pi's adapter row. Pi runs hooks through a generated extension, the shim in
//! `pi-shim.ts`, which spawns the bound launcher and relays its one line.
//! Native contracts are pinned to earendil-works/pi 2db5e359bf84c1c0be51d2c5c5c5c7cf27072c2b.
use super::{ConfigShape, EnvelopeKind, HookAdapter, NativeEvent, ResponseEncoding, Unattended};
use crate::config::Client;

const EVENTS: &[NativeEvent] = &[
    super::event("session_start", "start"),
    super::event("session_compact", "start"),
    super::event("agent_before_settle", "stop"),
    super::event("tool_result", "tool"),
];
pub(super) const ADAPTER: HookAdapter = HookAdapter {
    client: Client::Pi,
    harness: "pi",
    relative_config_path: ".pi/agent/extensions/locust.ts",
    config: ConfigShape::OwnedSource {
        template: include_str!("pi-shim.ts"),
    },
    events: EVENTS,
    // A nested call (parentToolCallId) is a tool of this same session, not a
    // separate agent; Pi names no subagent in its events.
    subagent_fields: &[],
    // The shim passes Pi's run mode. Print and JSON runs have no person.
    unattended: Some(Unattended {
        field: "mode",
        values: &["print", "json"],
    }),
    native_tool_prefix: "mcp__locust__",
    native_tool_use_id: true,
    response: ResponseEncoding::NestedMcp,
    envelope: EnvelopeKind::Line,
    command_timeout_seconds: 30,
    stop_timeout_seconds: 300,
    trust_review: false,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::{
        Event, HookError, HookRegistration, Outcome, Parsed, envelope, install, nested_projection,
        parse_configuration, parse_input, reapply, registration, remove, render_configuration,
        retain_present,
    };
    use serde_json::{Value, json};
    use std::path::Path;

    fn owned(source: Option<&str>) -> HookRegistration {
        HookRegistration::OwnedSource {
            source: source.map(str::to_owned),
        }
    }

    #[test]
    fn source_roundtrip_distinguishes_absence_empty_and_valid_extension() {
        for bytes in [
            None,
            Some(b"".as_slice()),
            Some(b"export default () => {};\n".as_slice()),
        ] {
            let document = parse_configuration(Client::Pi, bytes).unwrap();
            assert_eq!(
                render_configuration(Client::Pi, &document)
                    .unwrap()
                    .as_deref(),
                bytes
            );
        }
        assert!(parse_configuration(Client::Pi, Some(&[0xff])).is_err());
        for bad in [
            json!({}),
            json!({"source":1}),
            json!({"source":null,"other":true}),
            json!([]),
        ] {
            assert!(render_configuration(Client::Pi, &bad).is_err());
        }
    }

    #[test]
    fn source_ownership_refuses_existing_or_modified_files_and_preserves_hand_removal() {
        let original = parse_configuration(Client::Pi, None).unwrap();
        let mine = owned(Some("owned source"));
        let installed = install(&original, &mine).unwrap();
        for current in [
            parse_configuration(Client::Pi, Some(b"")).unwrap(),
            json!({"source":"unrelated source"}),
            installed.clone(),
        ] {
            assert_eq!(install(&current, &mine), Err(HookError::EntryConflict));
        }
        assert_eq!(remove(&installed, &mine, &original).unwrap(), original);
        assert_eq!(render_configuration(Client::Pi, &original).unwrap(), None);
        assert_eq!(retain_present(&original, &mine).unwrap(), owned(None));
        assert_eq!(remove(&original, &mine, &original).unwrap(), original);
        let changed = json!({"source":"user edits"});
        assert_eq!(
            retain_present(&changed, &mine),
            Err(HookError::EntryConflict)
        );
        assert_eq!(
            remove(&changed, &mine, &original),
            Err(HookError::EntryConflict)
        );
        assert_eq!(retain_present(&changed, &owned(None)).unwrap(), owned(None));
        assert_eq!(remove(&changed, &owned(None), &original).unwrap(), changed);
    }

    #[test]
    fn reapply_replaces_the_owned_shim_and_keeps_a_deleted_one_deleted() {
        let old = owned(Some("old shim"));
        let new = owned(Some("new shim"));
        let installed = json!({"source":"old shim"});
        let updated = reapply(&installed, &old, &[], &new).unwrap();
        assert_eq!(updated.document, json!({"source":"new shim"}));
        assert_eq!(updated.registration, new);
        let deleted = json!({"source":null});
        for recorded in [&old, &owned(None)] {
            let kept = reapply(&deleted, recorded, &[], &new).unwrap();
            assert_eq!(kept.document, deleted);
            assert_eq!(kept.registration, owned(None));
        }
        assert_eq!(
            reapply(&json!({"source":"edited"}), &old, &[], &new),
            Err(HookError::EntryConflict)
        );
    }

    fn rendered(path: &Path) -> String {
        let HookRegistration::OwnedSource {
            source: Some(source),
        } = registration(Client::Pi, path).unwrap()
        else {
            panic!("source registration")
        };
        source
    }

    #[test]
    fn generator_embeds_only_literal_paths_metadata_and_no_line_of_its_own() {
        let path = Path::new("/tmp/lh.fixture/cli ' ${HOME} $& `touch no` ");
        let source = rendered(path);
        assert!(source.contains(&format!(
            "const launcher = {};",
            serde_json::to_string(path.to_str().unwrap()).unwrap()
        )));
        assert!(source.contains("const harness = \"pi\";"));
        assert!(source.contains("const commandTimeoutMs = 30000;"));
        assert!(source.contains("const stopTimeoutMs = 300000;"));
        assert!(!source.contains("__LOCUST_"));
        assert!(source.contains("shell: false"));
        assert!(!source.contains(crate::hooks::core::FAILURE_LINE));
        assert!(!source.contains("Locust:"));
        assert!(registration(Client::Pi, Path::new("relative")).is_err());
        assert!(registration(Client::Pi, Path::new("/tmp/new\nline")).is_err());
    }

    #[test]
    fn template_marker_text_in_literal_launcher_paths_is_not_rewritten() {
        let path = Path::new(
            "/tmp/lh.fixture/__LOCUST_COMMAND_TIMEOUT_MS__/__LOCUST_HARNESS_JSON__/__LOCUST_LAUNCHER_JSON__",
        );
        let source = rendered(path);
        assert!(source.contains(&format!(
            "const launcher = {};",
            serde_json::to_string(path.to_str().unwrap()).unwrap()
        )));
        assert!(source.contains("const commandTimeoutMs = 30000;"));
        assert!(source.contains("const harness = \"pi\";"));
    }

    fn native() -> Value {
        json!({"session_id":"chat","hook_event_name":"tool_result","mode":"tui","tool_name":"mcp__locust__locust_wait","tool_use_id":"script/1","parent_tool_call_id":"script","tool_input":{"goal":"11".repeat(32),"seen":0,"timeout_ms":0},"tool_response":{"isError":false,"details":{"server":"locust","tool":"locust_wait"},"structuredContent":{"isError":false,"content":[{"type":"text","text":"model output must not establish a call"}],"structuredContent":{"ok":true,"result":{"waited":"no_event"}}}}})
    }

    #[test]
    fn nested_mcp_projection_retains_call_identity_and_only_unwraps_matching_success() {
        let value = native();
        let projected = nested_projection(&ADAPTER, &value).unwrap();
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
            assert!(nested_projection(&ADAPTER, &changed).is_none());
        }
    }

    #[test]
    fn native_event_and_output_goldens_include_compaction_mode_and_nested_tool_calls() {
        for (name, expected) in [
            ("session_start", "start"),
            ("session_compact", "start"),
            ("agent_before_settle", "stop"),
        ] {
            for (mode, unattended) in [
                ("tui", false),
                ("rpc", false),
                ("print", true),
                ("json", true),
            ] {
                let value = json!({"hook_event_name":name,"session_id":"native-chat","mode":mode});
                let Parsed::Input(input) = parse_input(&ADAPTER, expected, &value) else {
                    panic!("native {name} input");
                };
                if expected == "stop" {
                    assert_eq!(input.event, Event::Stop { unattended });
                } else {
                    assert_eq!(input.event, Event::Start);
                }
                let outcome = Outcome {
                    line: Some("Locust: fixed context line".into()),
                    keep_going: expected == "stop",
                    wait: false,
                };
                assert_eq!(
                    envelope(&ADAPTER, expected, name, &outcome).unwrap(),
                    Some(
                        json!({"line":"Locust: fixed context line","keep_going":expected=="stop"})
                    )
                );
                assert_eq!(
                    envelope(&ADAPTER, expected, name, &Outcome::default()).unwrap(),
                    None
                );
            }
        }
        let Parsed::Input(input) = parse_input(&ADAPTER, "tool", &native()) else {
            panic!("native nested Locust call");
        };
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
        let Parsed::Input(input) = parse_input(&ADAPTER, "tool", &failed) else {
            panic!("failed native call is still a tool event");
        };
        assert!(matches!(input.event, Event::Tool { own_call: None }));
    }
}
