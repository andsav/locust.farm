use super::*;
use locust_proto::organization::presets;
use serde_json::json;

fn checked(value: Value) -> Inspection {
    inspect(&value.to_string())
}

#[test]
fn minimal_open_template_is_valid_and_has_no_authority_binding() {
    let result = inspect(r#"{"schema_version":2}"#);
    assert!(result.valid, "{:?}", result.diagnostics);
    assert!(result.explanation.unwrap().authority_roles.is_empty());
    assert_eq!(result.normalized, Some(Formation::default()));
}

#[test]
fn workspace_policy_requires_explicit_valid_integrator_and_completion() {
    let result = checked(json!({
        "schema_version":2,
        "roles":{"integrator":{}, "reviewer":{}},
        "workspace":{
            "integrator":{"kind":"role","name":"integrator"},
            "completion":{"kind":"reviews","by":{"kind":"role","name":"reviewer"},"count":1,"exclude_author":true}
        }
    }));
    assert!(result.valid, "{:?}", result.diagnostics);
    let explanation = result.explanation.unwrap();
    assert_eq!(explanation.authority_roles, ["integrator"]);
    assert!(
        explanation
            .summary
            .iter()
            .any(|line| line.contains("composition-source"))
    );
    assert!(
        !checked(json!({"schema_version":2, "workspace":{
            "integrator":{"kind":"role","name":"missing"}
        }}))
        .valid
    );
    assert!(
        !checked(json!({"schema_version":2, "workspace":{
            "integrator":{"kind":"participant","key":"invalid"}
        }}))
        .valid
    );
}

#[test]
fn all_presets_are_valid_reusable_templates_and_normalization_is_idempotent() {
    for preset in presets() {
        let result = checked(serde_json::to_value(preset.formation).unwrap());
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
    let duplicate = inspect(r#"{"schema_version":2,"roles":{"a/b~c":{},"a/b~c":{}}}"#);
    assert!(!duplicate.valid);
    assert_eq!(duplicate.diagnostics[0].code, "duplicate_key");
    assert_eq!(duplicate.diagnostics[0].path, "/roles/a~1b~0c");
    assert!(!inspect(r#"{"schema_version":2} {}"#).valid);
}

#[test]
fn unsupported_version_and_unknown_fields_do_not_produce_a_normalized_document() {
    for source in [
        r#"{"schema_version":1,"future":true}"#,
        r#"{"schema_version":2,"future":true}"#,
        r#"{}"#,
    ] {
        let result = inspect(source);
        assert!(!result.valid);
        assert!(result.normalized.is_none());
        assert!(result.semantic_hash.is_none());
    }
    assert_eq!(
        inspect(r#"{"schema_version":1}"#).diagnostics[0].code,
        "unsupported_version"
    );
}

#[test]
fn effective_defaults_and_set_order_have_identical_identity() {
    assert_eq!(
        inspect(r#"{"schema_version":2}"#).semantic_hash,
        checked(serde_json::to_value(Formation::default()).unwrap()).semantic_hash
    );
    let key = "ab".repeat(32);
    let first = checked(
        json!({"schema_version":2,"decisions":{"completion":{"kind":"any","rules":[
            {"kind":"declaration","by":{"kind":"participant","key":key}},
            {"kind":"contribution","by":{"kind":"members"}}
        ]}}}),
    );
    let second = checked(json!({"decisions":{"completion":{"rules":[
        {"by":{"kind":"members"},"kind":"contribution"},
        {"by":{"key":key.to_uppercase(),"kind":"participant"},"kind":"declaration"},
        {"by":{"kind":"members"},"kind":"contribution"}
    ],"kind":"any"}},"schema_version":2}));
    assert!(first.valid && second.valid);
    assert_eq!(first.semantic_hash, second.semantic_hash);
    assert_ne!(
        first.semantic_hash,
        checked(json!({"schema_version":2,"context":{"guidance":"Changed instructions"}}))
            .semantic_hash
    );
}

#[test]
fn role_key_scope_threshold_and_cycles_report_actionable_locations() {
    let cases = [
        (
            json!({"schema_version":2,"work":{"propose":{"kind":"role","name":"missing"}}}),
            "unknown_role",
            "/work/propose/name",
        ),
        (
            json!({"schema_version":2,"work":{"propose":{"kind":"task_creator"}}}),
            "selector_scope",
            "/work/propose",
        ),
        (
            json!({"schema_version":2,"work":{"publish":{"kind":"participant","key":"wrong"}}}),
            "invalid_participant",
            "/work/publish/key",
        ),
        (
            json!({"schema_version":2,"decisions":{"completion":{"kind":"reviews","by":{"kind":"contribution_author"},"count":1}}}),
            "impossible_threshold",
            "/decisions/completion/count",
        ),
        (
            json!({"schema_version":2,"flow":{"a":{"requires":[{"stage":"b","evidence":"completion"}]},"b":{"requires":[{"stage":"a","evidence":"completion"}]}}}),
            "flow_cycle",
            "/flow",
        ),
        (
            json!({"schema_version":2,"flow":{"a":{},"b":{"requires":[{"stage":"a","evidence":"selection"}]}}}),
            "unavailable_evidence",
            "/flow/b/requires/0/evidence",
        ),
    ];
    for (source, code, path) in cases {
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

/// A stage's task is opened by the host's computer, which is no member, so
/// `task_creator` where only a member can act is met by nobody.
#[test]
fn a_stage_rule_that_names_the_task_creator_where_a_member_must_act_is_refused() {
    let creator = json!({"kind":"task_creator"});
    let wrapped = json!({"kind":"any","selectors":[{"kind":"nobody"},{"kind":"task_creator"}]});
    let mut refused = Vec::new();
    for by in [&creator, &wrapped] {
        refused.extend([
            (json!({"starts":[{"kind":"independent","by":by}]}), json!({})),
            (
                json!({"starts":[{"kind":"offered","by":{"kind":"members"},"to":by}]}),
                json!({}),
            ),
            (json!({}), json!({"completion":{"kind":"declaration","by":by}})),
            (
                json!({}),
                json!({"completion":{"kind":"reviews","by":by,"count":1,"exclude_author":false}}),
            ),
            (
                json!({}),
                json!({"completion":{"kind":"check","name":"build","by":by}}),
            ),
            (
                json!({}),
                json!({"completion":{"kind":"contribution","by":by}}),
            ),
            (
                json!({}),
                json!({"completion":{"kind":"all","rules":[{"kind":"declaration","by":{"kind":"members"}},{"kind":"declaration","by":by}]}}),
            ),
        ]);
    }
    for (work, decisions) in &refused {
        // The rules are the stage's task type's, or the formation's own.
        let typed = json!({"schema_version":2,
            "task_types":{"staged":{"work":work,"decisions":decisions}},
            "flow":{"research":{"task_type":"staged"}}});
        let own = json!({"schema_version":2,"work":work,"decisions":decisions,
            "flow":{"research":{}}});
        for source in [typed, own] {
            let result = checked(source.clone());
            assert!(!result.valid, "{source}");
            assert!(
                result.diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == "selector_scope"
                        && diagnostic.path == "/flow/research"
                        && diagnostic.message.contains("host's computer")
                }),
                "{source}: {:?}",
                result.diagnostics
            );
        }
        // Outside a stage the same rules are the task creator's to meet.
        let plain = json!({"schema_version":2,"work":work,"decisions":decisions});
        assert!(checked(plain.clone()).valid, "{plain}");
    }
    let offered_by_creator = json!({"schema_version":2,
        "task_types":{"staged":{"work":{"starts":[{"kind":"offered","by":creator,"to":{"kind":"members"}}]}}},
        "flow":{"research":{"task_type":"staged"}}});
    assert!(checked(offered_by_creator).valid);
    for preset in presets() {
        assert!(checked(serde_json::to_value(preset.formation).unwrap()).valid);
    }
}

#[test]
fn repeated_identity_does_not_make_an_impossible_review_threshold_possible() {
    let key = "ab".repeat(32);
    let result = checked(
        json!({"schema_version":2,"decisions":{"completion":{"kind":"reviews","count":2,"by":{"kind":"any","selectors":[{"kind":"participant","key":key},{"kind":"participant","key":key.to_uppercase()}]}}}}),
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
        json!({"schema_version":2,"roles":{"reviewer":{}},"context":{"inputs":{"spec":{"kind":"artifact"},"notes":{"kind":"text","required":false}}},"decisions":{"completion":{"kind":"reviews","by":{"kind":"role","name":"reviewer"},"count":2}}}),
    );
    assert!(result.valid);
    let explanation = result.explanation.unwrap();
    assert_eq!(explanation.required_roles, ["reviewer"]);
    assert_eq!(explanation.required_inputs, ["spec"]);
    assert!(explanation.authority_roles.is_empty());
    assert!(!explanation.contextual_checks.is_empty());
}
