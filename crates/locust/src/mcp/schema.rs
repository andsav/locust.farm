//! MCP schemas are extracted from the typed request contract.
use locust_proto::api::{OPERATIONS, Operation, operation_schema};
use serde_json::{Value, json};
use std::sync::OnceLock;
pub(super) fn tools() -> &'static [Value] {
    static TOOLS: OnceLock<Vec<Value>> = OnceLock::new();
    TOOLS.get_or_init(|| {
        OPERATIONS
            .iter()
            .filter(|operation| operation.tool)
            .map(tool)
            .collect()
    })
}
fn tool(operation: &Operation) -> Value {
    let mut input =
        operation_schema(operation.name).expect("every registry operation has a request schema");
    if !operation.read_only {
        input["properties"]["idempotency_key"] = json!({"type":["string","null"],"pattern":"^[0-9a-fA-F]{32}$","description":"Optional caller-owned retry key; reuse only for identical requests."});
    }
    // These are additive records or acknowledgments. The hints describe effects;
    // daemon grants and the client's approval policy still govern every write.
    let additive = matches!(
        operation.name,
        "goal.create"
            | "task.open"
            | "work.offer"
            | "contribution.publish"
            | "completion.declare"
            | "review.record"
            | "check.attest"
            | "doc.revise"
            | "delivery.acknowledge"
            | "context.acknowledge"
            | "blueprint.draft.create"
            | "blueprint.publish"
    );
    let destructive = !operation.read_only && !additive;
    let idempotent = operation.read_only || operation.name == "context.acknowledge";
    json!({"name":operation.tool_name(),"description":operation.summary,"inputSchema":input,
        "annotations":{"readOnlyHint":operation.read_only,"destructiveHint":destructive,"idempotentHint":idempotent,"openWorldHint":true}})
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "explicit schema cost measurement"]
    fn tool_schema_cost_report() {
        use std::{hint::black_box, time::Instant};

        let iterations = std::env::var("LOCUST_SCHEMA_BENCH_ITERATIONS")
            .map(|value| value.parse::<usize>().expect("positive iteration count"))
            .unwrap_or(100);
        assert!(iterations > 0);
        let cold_start = Instant::now();
        let first = tools();
        let cold_ns = cold_start.elapsed().as_nanos();
        let encoded = serde_json::to_vec(&first).unwrap();
        let mut construction_ns = Vec::with_capacity(iterations);
        let mut serialization_ns = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            black_box(tools());
            construction_ns.push(start.elapsed().as_nanos());
            let start = Instant::now();
            black_box(serde_json::to_vec(&first).unwrap());
            serialization_ns.push(start.elapsed().as_nanos());
        }
        construction_ns.sort_unstable();
        serialization_ns.sort_unstable();
        println!(
            "{}",
            json!({
                "iterations": iterations,
                "tool_count": first.len(),
                "compact_bytes": encoded.len(),
                "cold_construction_ns": cold_ns,
                "warm_construction_median_ns": construction_ns[iterations / 2],
                "serialization_median_ns": serialization_ns[iterations / 2],
            })
        );
    }

    #[test]
    fn every_tool_retains_its_generated_request_fields() {
        let tools = tools();
        for operation in OPERATIONS.iter().filter(|operation| operation.tool) {
            let tool = tools
                .iter()
                .find(|tool| tool["name"] == operation.tool_name())
                .unwrap();
            let mut input = tool["inputSchema"].clone();
            input["properties"]
                .as_object_mut()
                .unwrap()
                .remove("idempotency_key");
            assert_eq!(input, operation_schema(operation.name).unwrap());
        }
    }

    #[test]
    fn context_tools_expose_observation_and_explicit_idempotent_acknowledgment() {
        let tools = tools();
        let read = tools
            .iter()
            .find(|tool| tool["name"] == "locust_context_read")
            .unwrap();
        let ack = tools
            .iter()
            .find(|tool| tool["name"] == "locust_context_acknowledge")
            .unwrap();
        assert_eq!(read["annotations"]["readOnlyHint"], true);
        assert_eq!(read["annotations"]["destructiveHint"], false);
        assert!(
            read["inputSchema"]["properties"]
                .get("idempotency_key")
                .is_none()
        );
        for property in [
            "goal",
            "task",
            "after",
            "limit",
            "preview_chars",
            "unread_only",
        ] {
            assert!(
                read["inputSchema"]["properties"].get(property).is_some(),
                "{property}"
            );
        }
        assert_eq!(ack["annotations"]["readOnlyHint"], false);
        assert_eq!(ack["annotations"]["destructiveHint"], false);
        assert_eq!(ack["annotations"]["idempotentHint"], true);
        assert!(ack["inputSchema"]["properties"].get("receipt").is_some());
        assert!(
            ack["inputSchema"]["properties"]
                .get("idempotency_key")
                .is_some()
        );
    }

    #[test]
    fn observations_have_no_retry_key_and_invitations_stay_outside_model_tools() {
        let tools = tools();
        for operation in OPERATIONS
            .iter()
            .filter(|operation| operation.tool && operation.read_only)
        {
            let tool = tools
                .iter()
                .find(|tool| tool["name"] == operation.tool_name())
                .unwrap();
            assert!(
                tool["inputSchema"]["properties"]
                    .get("idempotency_key")
                    .is_none(),
                "{}",
                operation.name
            );
        }
        for name in [
            "locust_goal_invite",
            "locust_goal_join",
            "locust_invitation_inspect",
            "locust_invitation_join",
            "locust_invitation_list",
            "locust_invitation_revoke",
            "locust_member_add_local",
        ] {
            assert!(tools.iter().all(|tool| tool["name"] != name), "{name}");
        }
    }

    #[test]
    fn additive_evidence_is_distinguished_from_replacement_and_control() {
        let tools = tools();
        for name in [
            "task.open",
            "work.offer",
            "contribution.publish",
            "completion.declare",
            "review.record",
            "check.attest",
            "doc.revise",
            "delivery.acknowledge",
        ] {
            let operation = OPERATIONS
                .iter()
                .find(|operation| operation.name == name)
                .unwrap();
            let tool = tools
                .iter()
                .find(|tool| tool["name"] == operation.tool_name())
                .unwrap();
            assert_eq!(tool["annotations"]["destructiveHint"], false, "{name}");
            assert_eq!(tool["annotations"]["readOnlyHint"], false, "{name}");
            assert_eq!(tool["annotations"]["idempotentHint"], false, "{name}");
        }
        for name in [
            "member.remove",
            "rules.bind",
            "workspace.set",
            "task.revise",
            "attempt.takeover",
            "attempt.cancel",
            "scope.select",
            "scope.close",
            "blob.withdraw",
            "blueprint.draft.update",
        ] {
            let operation = OPERATIONS
                .iter()
                .find(|operation| operation.name == name)
                .unwrap();
            let tool = tools
                .iter()
                .find(|tool| tool["name"] == operation.tool_name())
                .unwrap();
            assert_eq!(tool["annotations"]["destructiveHint"], true, "{name}");
        }
    }
}
