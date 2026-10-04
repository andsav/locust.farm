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
    let schema = value(&output(&["blueprint", "schema"], None), 0);
    assert_eq!(schema["type"], "object");
    let envelope = value(&output(&["--json", "blueprint", "schema"], None), 0);
    assert_eq!(envelope["result"], schema);
    let help = output(&["blueprint", "--help"], None);
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
        .args(["--json", "blueprint", "schema"])
        .output()
        .unwrap();
    assert_eq!(value(&result, 0)["ok"], true);
    assert!(!home.exists());
    assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
}
#[test]
fn invalid_json_and_unknown_templates_have_stable_exit_codes() {
    let malformed = value(
        &output(&["--json", "blueprint", "validate", "-"], Some("{")),
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
        &output(&["--json", "blueprint", "example", "does-not-exist"], None),
        5,
    );
    assert_eq!(missing["error"]["code"], "not_found");
    let unsupported = json!({"schema_version": 999});
    let unsupported = value(
        &output(
            &["--json", "blueprint", "validate", "-"],
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
    let examples = value(&output(&["--json", "blueprint", "examples"], None), 0);
    let examples = examples["result"].as_array().unwrap();
    assert_eq!(examples.len(), 6);
    for example in examples {
        let name = example["name"].as_str().unwrap();
        assert!(example.get("blueprint").is_none());
        let raw = output(&["blueprint", "example", name], None);
        let blueprint = value(&raw, 0);
        let source = String::from_utf8(raw.stdout).unwrap();
        let validation = value(
            &output(&["--json", "blueprint", "validate", "-"], Some(&source)),
            0,
        );
        assert_eq!(validation["ok"], true);
        assert_eq!(validation["result"]["valid"], true);
        let explanation = value(
            &output(&["--json", "blueprint", "explain", "-"], Some(&source)),
            0,
        );
        assert!(explanation["result"]["explanation"]["summary"].is_array());
        let normalized = output(&["blueprint", "normalize", "-"], Some(&source));
        let normalized_value = value(&normalized, 0);
        let normalized_source = String::from_utf8(normalized.stdout).unwrap();
        let again = value(
            &output(
                &["--json", "blueprint", "validate", "-"],
                Some(&normalized_source),
            ),
            0,
        );
        assert_eq!(
            validation["result"]["semantic_hash"],
            again["result"]["semantic_hash"]
        );
        let envelope = value(&output(&["--json", "blueprint", "example", name], None), 0);
        assert_eq!(envelope["result"], blueprint);
        let scratch = tempfile::tempdir().unwrap();
        let path = scratch.path().join("blueprint.json");
        std::fs::write(&path, &normalized_source).unwrap();
        let disk = value(
            &output(
                &["--json", "blueprint", "normalize", path.to_str().unwrap()],
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
        &output(&["--json", "blueprint", "validate", "-"], Some(&source)),
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
    let result = value(&output(&["--json", "blueprint", "contract"], None), 0);
    let contract = &result["result"];
    assert_eq!(contract["schema_version"], 1);
    assert_eq!(contract["scope"], "offline_authoring");
    assert_eq!(
        contract["schema"],
        value(&output(&["blueprint", "schema"], None), 0)
    );
    for example in contract["examples"].as_array().unwrap() {
        let name = example["name"].as_str().unwrap();
        assert_eq!(
            example["blueprint"],
            value(&output(&["blueprint", "example", name], None), 0)
        );
    }
    let help = value(&output(&["--json", "blueprint", "--help"], None), 0);
    let help = help["result"]["help"].as_str().unwrap();
    for operation in contract["operations"].as_array().unwrap() {
        let name = operation["name"].as_str().unwrap();
        assert!(
            help.lines()
                .any(|line| line.split_whitespace().next() == Some(name)),
            "catalog command missing in help: {name}"
        );
        assert!(
            output(&["blueprint", name, "--help"], None)
                .status
                .success()
        );
    }
    for boundary in ["offline", "contextual", "runtime"] {
        assert!(contract["verification"][boundary].is_string());
    }
}
