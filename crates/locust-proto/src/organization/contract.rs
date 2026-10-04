//! Installed discovery for the current offline authoring surface.
use super::{SCHEMA_VERSION, presets, schema};
use serde::Serialize;

/// Stable discovery metadata; request field schemas come from the typed schema.
#[derive(Clone, Debug, Serialize)]
pub struct Operation {
    pub name: &'static str,
    pub input: &'static str,
    pub output: &'static str,
    pub summary: &'static str,
}

pub const OPERATIONS: &[Operation] = &[
    Operation {
        name: "contract",
        input: "none",
        output: "contract",
        summary: "Discover installed schema, operations, examples and verification boundaries",
    },
    Operation {
        name: "schema",
        input: "none",
        output: "json_schema",
        summary: "Print the authoring JSON Schema (raw JSON unless --json)",
    },
    Operation {
        name: "examples",
        input: "none",
        output: "example_catalog",
        summary: "List bundled authoring examples",
    },
    Operation {
        name: "example",
        input: "example_name",
        output: "blueprint",
        summary: "Print a bundled example (raw JSON unless --json)",
    },
    Operation {
        name: "validate",
        input: "json_path_or_stdin",
        output: "inspection",
        summary: "Validate a JSON document offline with structured diagnostics",
    },
    Operation {
        name: "explain",
        input: "json_path_or_stdin",
        output: "inspection",
        summary: "Explain effective rules and required bindings offline",
    },
    Operation {
        name: "normalize",
        input: "json_path_or_stdin",
        output: "blueprint_or_invalid_inspection",
        summary: "Print normalized semantic JSON (raw JSON unless --json)",
    },
];

/// Uses the same schema and examples consumed by validation and CLI discovery.
pub fn contract() -> serde_json::Value {
    serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "scope": "offline_authoring",
        "verification": {
            "offline": "Parsing, structural and semantic definition inspection",
            "contextual": "Member bindings, input values and current authority require instance context and are not verified offline",
            "runtime": "This authoring surface does not enable organization execution or verify delivery and flow transitions"
        },
        "operations": OPERATIONS,
        "schema": schema(),
        "examples": presets(),
        "source": "bundled_current_contract"
    })
}
