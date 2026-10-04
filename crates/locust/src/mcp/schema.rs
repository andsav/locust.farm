//! MCP inputs mirror the typed local API. Only OPERATIONS.tool entries are
//! exposed; files, commands, credentials and session management are not tools.
use locust_proto::api::{MAX_STAT_HASHES, OPERATIONS, Operation};
use locust_proto::limits::{MAX_ARTIFACTS, MAX_DEPENDENCIES};
use serde_json::{Map, Value, json};

pub(super) fn tools() -> Vec<Value> {
    OPERATIONS.iter().filter(|op| op.tool).map(tool).collect()
}

fn tool(op: &Operation) -> Value {
    let mut input = input(op.name);
    input["properties"]["idempotency_key"] = json!({
        "anyOf": [hex(16), {"type": "null"}],
        "description": "Optional caller-chosen retry key (32 hex characters). Reuse it only for exactly the same operation and arguments; a lost or cancelled reply does not prove a write did not commit."
    });
    json!({
        "name": op.tool_name(),
        "description": description(op),
        "inputSchema": input,
        "annotations": {
            "readOnlyHint": op.read_only,
            "destructiveHint": !op.read_only,
            "idempotentHint": op.read_only,
            "openWorldHint": true
        }
    })
}

fn description(op: &Operation) -> String {
    let detail = match op.name {
        "workspace.set" => " Records metadata only; does not read, write, export or apply files.",
        "task.claim" => {
            " Requires local execution authorization. Returns this session's claim generation; retain it for progress and submission."
        }
        "task.takeover" => {
            " Requires explicit local takeover authority and fences the earlier session."
        }
        "task.submit" => {
            " Submission records evidence; it does not accept or integrate the result. Use immutable object identifiers supplied by trusted workspace commands."
        }
        "result.accept" => {
            " Acceptance advances goal authority; it does not apply files to a checkout."
        }
        "wait" => {
            " Pass the last observed revision as seen; zero returns current state. Cancellation closes only this wait, without cancelling a task or undoing a committed operation."
        }
        "events" => {
            " The daemon returns at most 256 events per page; follow the returned cursor to continue."
        }
        "goal.invite" => {
            " expires_ms is an absolute Unix-millisecond timestamp. The returned invitation is a secret to share only with the intended participant."
        }
        _ => "",
    };
    format!("{}{detail}", op.summary)
}

fn hex(bytes: usize) -> Value {
    json!({"type": "string", "pattern": format!("^[0-9a-fA-F]{{{}}}$", bytes * 2)})
}
fn text() -> Value {
    json!({"type": "string"})
}
fn number(maximum: u64) -> Value {
    let mut schema = json!({"type": "integer", "minimum": 0});
    // Model providers may parse schema numbers as IEEE-754 doubles and reject
    // u64::MAX. Omit that annotation rather than inventing a smaller API limit;
    // the typed request still validates the full unsigned integer range.
    if maximum < (1_u64 << 53) {
        schema["maximum"] = json!(maximum);
    }
    schema
}
fn array(maximum: usize) -> Value {
    json!({"type": "array", "items": hex(32), "maxItems": maximum})
}
fn object(required: &[(&str, Value)], optional: &[(&str, Value)]) -> Value {
    let mut properties = Map::new();
    for (name, schema) in required {
        properties.insert((*name).into(), schema.clone());
    }
    for (name, schema) in optional {
        properties.insert((*name).into(), json!({"anyOf": [schema, {"type": "null"}]}));
    }
    json!({
        "type": "object", "properties": properties,
        "required": required.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        "additionalProperties": false
    })
}

fn input(name: &str) -> Value {
    let goal = ("goal", hex(32));
    let assignment = ("assignment", hex(32));
    let generation = ("generation", number(u32::MAX.into()));
    let doc = (
        "doc",
        json!({"type": "string", "enum": ["plan", "summary"]}),
    );
    match name {
        "status" => object(&[], &[]),
        "goal.create" => object(&[("title", text())], &[]),
        "goal.join" => object(&[("ticket", text())], &[]),
        "goal.invite" => object(&[goal], &[("expires_ms", number(u64::MAX))]),
        "goal.leave" | "goal.status" | "board" | "pending" => object(&[goal], &[]),
        "member.remove" => object(&[goal, ("member", hex(32))], &[]),
        "workspace.set" => object(
            &[
                goal,
                (
                    "binding",
                    object(
                        &[],
                        &[
                            ("export_root", text()),
                            ("source_commit", text()),
                            ("exported", hex(32)),
                            ("destination", text()),
                            ("integrated", hex(32)),
                        ],
                    ),
                ),
            ],
            &[],
        ),
        "task.show" => object(&[goal, ("task", hex(32))], &[]),
        "event.show" => object(&[goal, ("event", hex(32))], &[]),
        "task.propose" => object(
            &[
                goal,
                ("text", text()),
                ("depends_on", array(MAX_DEPENDENCIES)),
            ],
            &[
                ("input", hex(32)),
                ("deadline_ms", number(u64::MAX)),
                ("max_attempts", number(u32::MAX.into())),
            ],
        ),
        "task.assign" => object(&[goal, ("task", hex(32)), ("assignee", hex(32))], &[]),
        "task.cancel" | "task.claim" | "task.takeover" | "task.decline" => {
            object(&[goal, assignment], &[])
        }
        "task.progress" => object(&[goal, assignment, generation, ("text", text())], &[]),
        "task.submit" => object(
            &[
                goal,
                assignment,
                generation,
                ("summary", text()),
                ("artifacts", array(MAX_ARTIFACTS)),
            ],
            &[("base", hex(32)), ("patch", hex(32))],
        ),
        "task.fail" => object(&[goal, assignment, generation, ("reason", text())], &[]),
        "cancel.acknowledge" => object(
            &[
                goal,
                ("cancel", hex(32)),
                (
                    "outcome",
                    json!({"type": "string", "enum": ["stopped", "completed", "uncertain"]}),
                ),
            ],
            &[generation],
        ),
        "result.accept" => object(&[goal, ("result", hex(32))], &[("head", hex(32))]),
        "result.reject" => object(&[goal, ("result", hex(32)), ("reason", text())], &[]),
        "wait" => object(
            &[
                goal,
                ("seen", number(u64::MAX)),
                ("timeout_ms", number(u32::MAX.into())),
            ],
            &[],
        ),
        "events" => object(
            &[goal, ("limit", number(u32::MAX.into()))],
            &[("after", number(u64::MAX))],
        ),
        "note.add" => object(
            &[goal, ("text", text())],
            &[("about", hex(32)), ("supersedes", hex(32))],
        ),
        "notes" => object(&[goal], &[("about", hex(32))]),
        "doc.read" => object(&[goal, doc], &[]),
        "doc.revise" => object(&[goal, doc, ("text", text())], &[("base", hex(32))]),
        "doc.accept" => object(&[goal, ("revision", hex(32))], &[]),
        "blob.stat" => object(&[goal, ("hashes", array(MAX_STAT_HASHES))], &[]),
        "blob.withdraw" => object(&[goal, ("hash", hex(32))], &[]),
        _ => unreachable!("OPERATIONS.tool requires an MCP input schema: {name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_proto::api::{Audience, Request};

    #[test]
    fn provider_schemas_avoid_unsafe_numbers_without_lowering_api_limits() {
        fn check(value: &Value) {
            match value {
                Value::Number(number) => assert!(number.as_u64().unwrap() < 1_u64 << 53),
                Value::Array(values) => values.iter().for_each(check),
                Value::Object(fields) => fields.values().for_each(check),
                _ => {}
            }
        }
        tools().iter().for_each(check);
        let goal = "01".repeat(32);
        for (operation, field, fields) in [
            (
                "goal.invite",
                "expires_ms",
                json!({"goal": goal, "expires_ms": u64::MAX}),
            ),
            (
                "task.propose",
                "deadline_ms",
                json!({"goal": goal, "text": "test", "depends_on": [], "deadline_ms": u64::MAX}),
            ),
            (
                "wait",
                "seen",
                json!({"goal": goal, "seen": u64::MAX, "timeout_ms": 1}),
            ),
            (
                "events",
                "after",
                json!({"goal": goal, "after": u64::MAX, "limit": 1}),
            ),
        ] {
            let schema = input(operation);
            let property = &schema["properties"][field];
            let property = if property.get("anyOf").is_some() {
                &property["anyOf"][0]
            } else {
                property
            };
            assert!(property.get("maximum").is_none());
            let request: Request = serde_json::from_value(json!({operation: fields})).unwrap();
            assert_eq!(request.name(), operation);
        }
        assert_eq!(
            input("wait")["properties"]["timeout_ms"]["maximum"],
            u32::MAX
        );
    }

    fn sample(schema: &Value) -> Value {
        if schema.get("anyOf").is_some() {
            return Value::Null;
        }
        if let Some(values) = schema.get("enum") {
            return values[0].clone();
        }
        match schema["type"].as_str().unwrap() {
            "object" => schema["properties"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(key, value)| (key.clone(), sample(value)))
                .collect(),
            "string" if schema.get("pattern").is_some() => json!("01".repeat(32)),
            "string" => json!("sample"),
            "array" => json!([]),
            "integer" => json!(0),
            other => panic!("unhandled schema {other}"),
        }
    }

    #[test]
    fn every_exposed_operation_has_a_typed_schema_matching_the_local_request() {
        let list = tools();
        assert_eq!(list.len(), OPERATIONS.iter().filter(|op| op.tool).count());
        for op in OPERATIONS.iter().filter(|op| op.tool) {
            assert_ne!(op.audience, Audience::Owner);
            let tool = list
                .iter()
                .find(|tool| tool["name"] == op.tool_name())
                .unwrap();
            assert_eq!(tool["annotations"]["readOnlyHint"], op.read_only);
            let mut fields = sample(&tool["inputSchema"]);
            fields.as_object_mut().unwrap().remove("idempotency_key");
            let encoded = if op.name == "status" {
                json!("status")
            } else {
                json!({op.name: fields})
            };
            let request: Request = serde_json::from_value(encoded)
                .unwrap_or_else(|error| panic!("{}: {error}", op.name));
            assert_eq!(request.name(), op.name);
            assert!(request.operation().tool);
        }
        for hidden in [
            "locust_daemon_stop",
            "locust_agent_enroll",
            "locust_blob_get",
            "locust_blob_put",
            "locust_session_report",
            "locust_task_authorize",
        ] {
            assert!(!list.iter().any(|tool| tool["name"] == hidden));
        }
    }
}
