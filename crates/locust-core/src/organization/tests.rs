use super::*;
use locust_proto::organization::presets;
use serde_json::json;

fn checked(value: Value) -> Inspection {
    inspect(&value.to_string())
}

#[test]
fn minimal_open_template_is_valid_and_has_no_authority_binding() {
    let result = inspect(r#"{"schema_version":1}"#);
    assert!(result.valid, "{:?}", result.diagnostics);
    assert!(result.explanation.unwrap().authority_roles.is_empty());
    assert_eq!(result.normalized, Some(Blueprint::default()));
}

#[test]
fn all_presets_are_valid_reusable_templates_and_normalization_is_idempotent() {
    for preset in presets() {
        let result = checked(serde_json::to_value(preset.blueprint).unwrap());
        assert!(result.valid, "{}: {:?}", preset.name, result.diagnostics);
        let again = checked(serde_json::to_value(result.normalized.as_ref().unwrap()).unwrap());
        assert!(again.valid);
        assert_eq!(result.semantic_hash, again.semantic_hash);
        assert_eq!(result.normalized, again.normalized);
        if preset.name == "coordinator" {
            assert_eq!(
                result.explanation.as_ref().unwrap().authority_roles,
                ["coordinator"]
            );
        }
    }
}

#[test]
fn duplicate_keys_and_trailing_documents_are_never_silently_accepted() {
    let duplicate = inspect(r#"{"schema_version":1,"roles":{"a/b~c":{},"a/b~c":{}}}"#);
    assert!(!duplicate.valid);
    assert_eq!(duplicate.diagnostics[0].code, "duplicate_key");
    assert_eq!(duplicate.diagnostics[0].path, "/roles/a~1b~0c");
    assert!(!inspect(r#"{"schema_version":1} {}"#).valid);
}

#[test]
fn unsupported_version_and_unknown_fields_do_not_produce_a_normalized_document() {
    for source in [
        r#"{"schema_version":2,"future":true}"#,
        r#"{"schema_version":1,"future":true}"#,
        r#"{}"#,
    ] {
        let result = inspect(source);
        assert!(!result.valid);
        assert!(result.normalized.is_none());
        assert!(result.semantic_hash.is_none());
    }
    assert_eq!(
        inspect(r#"{"schema_version":2}"#).diagnostics[0].code,
        "unsupported_version"
    );
}

#[test]
fn effective_defaults_and_set_order_have_identical_identity() {
    assert_eq!(
        inspect(r#"{"schema_version":1}"#).semantic_hash,
        checked(serde_json::to_value(Blueprint::default()).unwrap()).semantic_hash
    );
    let key = "ab".repeat(32);
    let first = checked(
        json!({"schema_version":1,"decisions":{"completion":{"kind":"any","rules":[
            {"kind":"declaration","by":{"kind":"participant","key":key}},
            {"kind":"contribution","by":{"kind":"members"}}
        ]}}}),
    );
    let second = checked(json!({"decisions":{"completion":{"rules":[
        {"by":{"kind":"members"},"kind":"contribution"},
        {"by":{"key":key.to_uppercase(),"kind":"participant"},"kind":"declaration"},
        {"by":{"kind":"members"},"kind":"contribution"}
    ],"kind":"any"}},"schema_version":1}));
    assert!(first.valid && second.valid);
    assert_eq!(first.semantic_hash, second.semantic_hash);
    assert_ne!(
        first.semantic_hash,
        checked(json!({"schema_version":1,"context":{"guidance":"Changed instructions"}}))
            .semantic_hash
    );
}

#[test]
fn role_key_scope_threshold_and_cycles_report_actionable_locations() {
    let cases = [
        (
            json!({"schema_version":1,"work":{"propose":{"kind":"role","name":"missing"}}}),
            "unknown_role",
            "/work/propose/name",
        ),
        (
            json!({"schema_version":1,"work":{"propose":{"kind":"task_creator"}}}),
            "selector_scope",
            "/work/propose",
        ),
        (
            json!({"schema_version":1,"work":{"publish":{"kind":"participant","key":"wrong"}}}),
            "invalid_participant",
            "/work/publish/key",
        ),
        (
            json!({"schema_version":1,"decisions":{"completion":{"kind":"reviews","by":{"kind":"contribution_author"},"count":1}}}),
            "impossible_threshold",
            "/decisions/completion/count",
        ),
        (
            json!({"schema_version":1,"flow":{"a":{"requires":[{"stage":"b","evidence":"completion"}]},"b":{"requires":[{"stage":"a","evidence":"completion"}]}}}),
            "flow_cycle",
            "/flow",
        ),
        (
            json!({"schema_version":1,"flow":{"a":{},"b":{"requires":[{"stage":"a","evidence":"selection"}]}}}),
            "unavailable_evidence",
            "/flow/b/requires/0/evidence",
        ),
    ];
    for (mut source, code, path) in cases {
        if let Some(flow) = source.get_mut("flow").and_then(Value::as_object_mut) {
            for stage in flow.values_mut() {
                stage["runner"] = json!({"kind":"participant","key":"ab".repeat(32)});
            }
        }
        let result = checked(source);
        assert!(!result.valid);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code && diagnostic.path == path),
            "{:?}",
            result.diagnostics
        );
    }
}

#[test]
fn repeated_identity_does_not_make_an_impossible_review_threshold_possible() {
    let key = "ab".repeat(32);
    let result = checked(
        json!({"schema_version":1,"decisions":{"completion":{"kind":"reviews","count":2,"by":{"kind":"any","selectors":[{"kind":"participant","key":key},{"kind":"participant","key":key.to_uppercase()}]}}}}),
    );
    assert!(!result.valid);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "impossible_threshold")
    );
}

#[test]
fn referenced_role_and_input_slots_are_reported_without_demanding_live_bindings() {
    let result = checked(
        json!({"schema_version":1,"roles":{"reviewer":{}},"context":{"inputs":{"spec":{"kind":"artifact"},"notes":{"kind":"text","required":false}}},"decisions":{"completion":{"kind":"reviews","by":{"kind":"role","name":"reviewer"},"count":2}}}),
    );
    assert!(result.valid);
    let explanation = result.explanation.unwrap();
    assert_eq!(explanation.required_roles, ["reviewer"]);
    assert_eq!(explanation.required_inputs, ["spec"]);
    assert!(explanation.authority_roles.is_empty());
    assert!(!explanation.contextual_checks.is_empty());
}
