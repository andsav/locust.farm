//! Immutable generated schemas and the reference closure for each operation.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde_json::{Map, Value, json};

use super::Request;

pub(super) fn request() -> &'static Value {
    static REQUEST: OnceLock<Value> = OnceLock::new();
    REQUEST.get_or_init(|| {
        serde_json::to_value(schemars::schema_for!(Request)).expect("request schema serializes")
    })
}

pub(super) fn operation(name: &str) -> Option<&'static Value> {
    static OPERATIONS: OnceLock<BTreeMap<String, Value>> = OnceLock::new();
    OPERATIONS
        .get_or_init(|| {
            let schema = request();
            let branches = schema
                .get("oneOf")
                .or_else(|| schema.get("anyOf"))
                .and_then(Value::as_array)
                .expect("request enum has schema branches");
            let definitions = schema.get("$defs").and_then(Value::as_object);
            let mut operations = BTreeMap::new();
            for branch in branches {
                let names = branch.get("const").into_iter().chain(
                    branch
                        .get("enum")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten(),
                );
                for name in names.filter_map(Value::as_str) {
                    operations.insert(
                        name.to_owned(),
                        json!({"type":"object","properties":{},"additionalProperties":false}),
                    );
                }
                if let Some(properties) = branch.get("properties").and_then(Value::as_object) {
                    for (name, input) in properties {
                        operations.insert(name.clone(), standalone(input.clone(), definitions));
                    }
                }
            }
            operations
        })
        .get(name)
}

fn standalone(mut input: Value, definitions: Option<&Map<String, Value>>) -> Value {
    if let Some(definitions) = definitions {
        let retained = reachable_definitions(&input, definitions);
        if !retained.is_empty() {
            input["$defs"] = Value::Object(retained);
        }
    }
    input
}

fn reachable_definitions(input: &Value, definitions: &Map<String, Value>) -> Map<String, Value> {
    let mut reached = BTreeSet::new();
    let mut pending = vec![input];
    while let Some(value) = pending.pop() {
        match value {
            Value::Array(values) => pending.extend(values),
            Value::Object(object) => {
                for keyword in ["$ref", "$dynamicRef", "$recursiveRef"] {
                    let Some(reference) = object.get(keyword).and_then(Value::as_str) else {
                        continue;
                    };
                    // Schemars emits root definition pointers. Other reference
                    // forms (anchors, URI fragments or external resources) keep
                    // the complete definitions, preserving their resolution.
                    let Some(path) = reference.strip_prefix("#/$defs/") else {
                        return definitions.clone();
                    };
                    if path.contains('%') {
                        return definitions.clone();
                    }
                    let name = path
                        .split('/')
                        .next()
                        .unwrap()
                        .replace("~1", "/")
                        .replace("~0", "~");
                    if reached.insert(name.clone())
                        && let Some(definition) = definitions.get(&name)
                    {
                        pending.push(definition);
                    }
                }
                pending.extend(object.values());
            }
            _ => {}
        }
    }
    reached
        .into_iter()
        .filter_map(|name| definitions.get(&name).map(|value| (name, value.clone())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{OPERATIONS, operation_schema, request_schema};

    fn assert_reference_targets_equal(value: &Value, original: &Value, reduced: &Value) {
        match value {
            Value::Object(object) => {
                if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                    let pointer = reference.strip_prefix('#').unwrap();
                    let target = original
                        .pointer(pointer)
                        .expect("generated reference resolves");
                    assert_eq!(Some(target), reduced.pointer(pointer), "{reference}");
                }
                for value in object.values() {
                    assert_reference_targets_equal(value, original, reduced);
                }
            }
            Value::Array(values) => {
                for value in values {
                    assert_reference_targets_equal(value, original, reduced);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn generated_operations_preserve_fields_and_every_reference_target() {
        let full = serde_json::to_value(schemars::schema_for!(Request)).unwrap();
        assert_eq!(request_schema(), full);
        for operation in OPERATIONS {
            let reduced = operation_schema(operation.name).unwrap();
            let Some(original) = full["oneOf"]
                .as_array()
                .unwrap()
                .iter()
                .find_map(|branch| branch["properties"].get(operation.name))
            else {
                // Unit variants expose an empty MCP argument object.
                assert_eq!(
                    reduced,
                    json!({"type":"object","properties":{},"additionalProperties":false})
                );
                continue;
            };
            let mut fields = reduced.clone();
            fields.as_object_mut().unwrap().remove("$defs");
            assert_eq!(&fields, original, "{}", operation.name);
            assert_reference_targets_equal(&reduced, &full, &reduced);
        }
        assert!(operation_schema("unknown.operation").is_none());
    }

    #[test]
    fn transitive_and_cyclic_references_keep_exact_definitions_and_pointer_escapes() {
        let full = json!({
            "type":"object", "properties":{"root":{"$ref":"#/$defs/a~1b~0c"}},
            "$defs":{
                "a/b~c":{"anyOf":[{"$ref":"#/$defs/B"},{"$ref":"#/$defs/Nested/properties/value"}]},
                "B":{"anyOf":[{"$ref":"#/$defs/a~1b~0c"},{"type":"null"}]},
                "Nested":{"type":"object","properties":{"value":{"$ref":"#/$defs/Tail"}}},
                "Tail":{"type":"integer","minimum":1},
                "Unused":{"type":"string"}
            }
        });
        let mut input = full.clone();
        input.as_object_mut().unwrap().remove("$defs");
        let reduced = standalone(input, full["$defs"].as_object());
        let mut expected = full.clone();
        expected["$defs"].as_object_mut().unwrap().remove("Unused");
        assert_eq!(reduced, expected);
        assert_reference_targets_equal(&reduced, &full, &reduced);
    }

    #[test]
    fn unreferenced_definitions_are_omitted_and_other_reference_forms_are_preserved() {
        let definitions = json!({
            "A":{"type":"string","$anchor":"name"},
            "B":{"type":"integer"}
        });
        let input = json!({"type":"object","properties":{"name":{"type":"string"}}});
        assert_eq!(standalone(input.clone(), definitions.as_object()), input);
        for (keyword, reference) in [
            ("$ref", "#name"),
            ("$ref", "#/$defs/%41"),
            ("$ref", "https://example.invalid/schema#/$defs/A"),
            ("$dynamicRef", "#name"),
            ("$recursiveRef", "#"),
        ] {
            let input = json!({"type":"object","properties":{"name":{keyword:reference}}});
            let reduced = standalone(input, definitions.as_object());
            assert_eq!(reduced["$defs"], definitions);
        }
    }

    #[test]
    fn owned_schema_edits_cannot_mutate_cached_definitions() {
        let mut full = request_schema();
        full["$defs"] = json!({});
        assert_ne!(full, request_schema());
        let mut operation = operation_schema("context.acknowledge").unwrap();
        operation["$defs"] = json!({});
        assert_ne!(Some(operation), operation_schema("context.acknowledge"));
    }
}
