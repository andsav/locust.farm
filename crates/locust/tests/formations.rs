//! Offline authoring contracts exercised through the installed binary surface.
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_locust"));
    command
        .env_remove("HOME")
        .env_remove("LOCUST_HOME")
        .env_remove("LOCUST_CREDENTIAL")
        .env_remove("LOCUST_SESSION");
    command
}
fn output(arguments: &[&str], input: Option<&str>) -> Output {
    let mut command = cli();
    command.args(arguments);
    if let Some(input) = input {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    } else {
        command.output().unwrap()
    }
}
fn value(output: &Output, status: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(status),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn discovery_is_offline_and_schema_is_raw_json() {
    let schema = value(&output(&["formation", "schema"], None), 0);
    assert_eq!(schema["type"], "object");
    let envelope = value(&output(&["--json", "formation", "schema"], None), 0);
    assert_eq!(envelope["result"], schema);
    let help = output(&["formation", "--help"], None);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("offline"));
}
#[test]
fn inspection_does_not_create_state_or_read_credentials() {
    let scratch = tempfile::tempdir().unwrap();
    let home = scratch.path().join("absent");
    let result = cli()
        .env("HOME", scratch.path())
        .env("LOCUST_HOME", &home)
        .env("LOCUST_CREDENTIAL", "/unavailable/credential")
        .env("LOCUST_SESSION", "/unavailable/session")
        .args(["--json", "formation", "schema"])
        .output()
        .unwrap();
    assert_eq!(value(&result, 0)["ok"], true);
    assert!(!home.exists());
    assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
}
#[test]
fn invalid_json_and_unknown_templates_have_stable_exit_codes() {
    let malformed = value(
        &output(&["--json", "formation", "validate", "-"], Some("{")),
        6,
    );
    assert_eq!(malformed["ok"], false);
    assert!(
        !malformed["result"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let missing = value(
        &output(&["--json", "formation", "example", "does-not-exist"], None),
        5,
    );
    assert_eq!(missing["error"]["code"], "not_found");
    let unsupported = json!({"schema_version": 999});
    let unsupported = value(
        &output(
            &["--json", "formation", "validate", "-"],
            Some(&unsupported.to_string()),
        ),
        10,
    );
    assert_eq!(unsupported["ok"], false);
    assert!(
        !unsupported["result"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn bundled_examples_round_trip_through_stdin_and_disk() {
    let examples = value(&output(&["--json", "formation", "examples"], None), 0);
    let examples = examples["result"].as_array().unwrap();
    assert_eq!(examples.len(), 6);
    for example in examples {
        let name = example["name"].as_str().unwrap();
        assert!(example.get("formation").is_none());
        let raw = output(&["formation", "example", name], None);
        let formation = value(&raw, 0);
        let source = String::from_utf8(raw.stdout).unwrap();
        let validation = value(
            &output(&["--json", "formation", "validate", "-"], Some(&source)),
            0,
        );
        assert_eq!(validation["ok"], true);
        assert_eq!(validation["result"]["valid"], true);
        let explanation = value(
            &output(&["--json", "formation", "explain", "-"], Some(&source)),
            0,
        );
        assert!(explanation["result"]["explanation"]["summary"].is_array());
        let normalized = output(&["formation", "normalize", "-"], Some(&source));
        let normalized_value = value(&normalized, 0);
        let normalized_source = String::from_utf8(normalized.stdout).unwrap();
        let again = value(
            &output(
                &["--json", "formation", "validate", "-"],
                Some(&normalized_source),
            ),
            0,
        );
        assert_eq!(
            validation["result"]["semantic_hash"],
            again["result"]["semantic_hash"]
        );
        let envelope = value(&output(&["--json", "formation", "example", name], None), 0);
        assert_eq!(envelope["result"], formation);
        let scratch = tempfile::tempdir().unwrap();
        let path = scratch.path().join("formation.json");
        std::fs::write(&path, &normalized_source).unwrap();
        let disk = value(
            &output(
                &["--json", "formation", "normalize", path.to_str().unwrap()],
                None,
            ),
            0,
        );
        assert_eq!(disk["result"], normalized_value);
    }
}
#[test]
fn invented_fields_are_rejected_with_structured_diagnostics() {
    let source = json!({"schema_version":1,"invented":true}).to_string();
    let result = value(
        &output(&["--json", "formation", "validate", "-"], Some(&source)),
        6,
    );
    let diagnostic = &result["result"]["diagnostics"][0];
    for field in ["code", "phase", "path", "message", "correction"] {
        assert!(
            diagnostic[field].is_string(),
            "missing {field}: {diagnostic}"
        );
    }
}

#[test]
fn installed_contract_matches_cli_schema_examples_and_operations() {
    let result = value(&output(&["--json", "formation", "contract"], None), 0);
    let contract = &result["result"];
    assert_eq!(contract["schema_version"], 1);
    assert_eq!(contract["scope"], "offline_authoring");
    assert_eq!(
        contract["schema"],
        value(&output(&["formation", "schema"], None), 0)
    );
    for example in contract["examples"].as_array().unwrap() {
        let name = example["name"].as_str().unwrap();
        assert_eq!(
            example["formation"],
            value(&output(&["formation", "example", name], None), 0)
        );
    }
    let help = value(&output(&["--json", "formation", "--help"], None), 0);
    let help = help["result"]["help"].as_str().unwrap();
    for operation in contract["operations"].as_array().unwrap() {
        let name = operation["name"].as_str().unwrap();
        assert!(
            help.lines()
                .any(|line| line.split_whitespace().next() == Some(name)),
            "catalog command missing in help: {name}"
        );
        assert!(
            output(&["formation", name, "--help"], None)
                .status
                .success()
        );
    }
    for boundary in ["offline", "contextual", "runtime"] {
        assert!(contract["verification"][boundary].is_string());
    }
}

#[test]
fn runtime_contract_discovers_requests_responses_events_and_cli_without_state() {
    let scratch = tempfile::tempdir().unwrap();
    let home = scratch.path().join("absent");
    let result = cli()
        .env("LOCUST_HOME", &home)
        .env("LOCUST_CREDENTIAL", "/unavailable/credential")
        .args(["--json", "contract"])
        .output()
        .unwrap();
    let contract = value(&result, 0)["result"].clone();
    assert_eq!(contract["api_version"], locust_proto::API_VERSION);
    assert!(!home.exists());
    let operations = contract["operations"].as_array().unwrap();
    assert!(
        operations
            .iter()
            .any(|operation| operation["mcp_tool"] == "locust_formation_draft_update")
    );
    assert!(contract["response_schema"]["$defs"]["Draft"].is_object());
    assert!(contract["event_schema"]["$defs"]["Effect"].is_object());
    assert!(
        contract["cli"]["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|command| command["name"] == "attempt")
    );
    assert!(
        !operations
            .iter()
            .any(|operation| operation["name"] == "task.assign")
    );
}

#[test]
fn semantic_diff_uses_normalized_definitions_and_retains_invalid_side_diagnostics() {
    let directory = tempfile::tempdir().unwrap();
    let before = directory.path().join("before.json");
    let after = directory.path().join("after.json");
    std::fs::write(&before, r#"{"schema_version":1}"#).unwrap();
    let normalized = value(&output(&["formation", "example", "open"], None), 0);
    std::fs::write(&after, serde_json::to_string_pretty(&normalized).unwrap()).unwrap();
    let arguments = [
        "--json",
        "formation",
        "diff",
        before.to_str().unwrap(),
        after.to_str().unwrap(),
    ];
    let same = value(&output(&arguments, None), 0);
    assert_eq!(same["result"]["equivalent"], true);
    assert_eq!(same["result"]["changes"], json!([]));
    assert_eq!(
        same["result"]["before"]["semantic_hash"],
        same["result"]["after"]["semantic_hash"]
    );
    std::fs::write(
        &after,
        r#"{"schema_version":1,"context":{"guidance":"different"}}"#,
    )
    .unwrap();
    let changed = value(&output(&arguments, None), 0);
    assert_eq!(changed["result"]["equivalent"], false);
    assert_eq!(changed["result"]["changes"][0]["path"], "/context/guidance");
    assert_eq!(changed["result"]["changes"][0]["after"], "different");
    std::fs::write(&after, r#"{"schema_version":2}"#).unwrap();
    let unsupported = value(&output(&arguments, None), 10);
    assert_eq!(unsupported["ok"], false);
    assert_eq!(unsupported["result"]["equivalent"], Value::Null);
    assert_eq!(unsupported["result"]["before"]["diagnostics"], json!([]));
    assert_eq!(
        unsupported["result"]["after"]["diagnostics"][0]["code"],
        "unsupported_version"
    );
    std::fs::write(&after, "{").unwrap();
    let invalid = value(&output(&arguments, None), 6);
    assert_eq!(invalid["result"]["changes"], json!([]));
    assert!(
        !invalid["result"]["after"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
