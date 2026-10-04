//! MCP schemas are extracted from the typed request contract.
use locust_proto::api::{OPERATIONS, Operation, operation_schema};
use serde_json::{Value, json};
pub(super) fn tools() -> Vec<Value> {
    OPERATIONS
        .iter()
        .filter(|operation| operation.tool)
        .map(tool)
        .collect()
}
fn tool(operation: &Operation) -> Value {
    let mut input =
        operation_schema(operation.name).expect("every registry operation has a request schema");
    input["properties"]["idempotency_key"] = json!({"type":["string","null"],"pattern":"^[0-9a-fA-F]{32}$","description":"Optional caller-owned retry key; reuse only for identical requests."});
    json!({"name":operation.tool_name(),"description":operation.summary,"inputSchema":input,
        "annotations":{"readOnlyHint":operation.read_only,"destructiveHint":!operation.read_only,"idempotentHint":operation.read_only,"openWorldHint":true}})
}
#[cfg(test)]
mod tests {
    use super::*;
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
}
