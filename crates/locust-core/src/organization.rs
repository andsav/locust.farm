//! Pure organization authoring. Contextual authorization remains a runtime check.

use std::collections::BTreeSet;

use locust_proto::organization::{Formation, SCHEMA_VERSION, semantic_hash};
use serde::Serialize;
use serde_json::Value;

mod explanation;
mod roles;
pub use roles::{RoleDuty, is_authority_role, role_duties};
mod normalize;
mod strict_json;
mod validation;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: String,
    pub phase: String,
    /// RFC 6901 JSON Pointer; an empty string names the whole document.
    pub path: String,
    pub message: String,
    pub correction: String,
    pub related_paths: Vec<String>,
}

impl Diagnostic {
    fn error(
        code: &str,
        phase: &str,
        path: &str,
        message: impl Into<String>,
        correction: &str,
    ) -> Self {
        Self {
            code: code.into(),
            severity: "error".into(),
            phase: phase.into(),
            path: path.into(),
            message: message.into(),
            correction: correction.into(),
            related_paths: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Explanation {
    pub summary: Vec<String>,
    pub required_roles: Vec<String>,
    pub required_inputs: Vec<String>,
    pub authority_roles: Vec<String>,
    pub contextual_checks: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Inspection {
    pub valid: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub normalized: Option<Formation>,
    pub semantic_hash: Option<String>,
    pub explanation: Option<Explanation>,
}

impl Inspection {
    fn failed(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            valid: false,
            diagnostics,
            normalized: None,
            semantic_hash: None,
            explanation: None,
        }
    }
}

/// Validate a reusable template, without I/O, principals or local execution.
/// Binding slots may remain unbound. A successful inspection grants no rights.
pub fn inspect(source: &str) -> Inspection {
    let value = match strict_json::parse(source) {
        Ok(value) => value,
        Err(diagnostic) => return Inspection::failed(vec![*diagnostic]),
    };
    if let Some(version) = value.get("schema_version").and_then(Value::as_u64)
        && version != u64::from(SCHEMA_VERSION)
    {
        return Inspection::failed(vec![Diagnostic::error(
            "unsupported_version",
            "load",
            "/schema_version",
            format!("schema version {version} is unsupported; this build accepts {SCHEMA_VERSION}"),
            "Use the contract exported by this build. Unsupported documents are not converted.",
        )]);
    }
    let mut formation: Formation = match serde_path_to_error::deserialize(value) {
        Ok(formation) => formation,
        Err(error) => {
            let path = error
                .path()
                .iter()
                .filter_map(|segment| match segment {
                    serde_path_to_error::Segment::Seq { index } => Some(index.to_string()),
                    serde_path_to_error::Segment::Map { key } => Some(escape(key)),
                    _ => None,
                })
                .map(|segment| format!("/{segment}"))
                .collect::<String>();
            return Inspection::failed(vec![Diagnostic::error(
                "invalid_structure",
                "definition",
                &path,
                error.inner().to_string(),
                "Use formation schema or a bundled example for supported fields and values.",
            )]);
        }
    };
    let diagnostics = validation::validate(&formation);
    if !diagnostics.is_empty() {
        return Inspection::failed(diagnostics);
    }
    normalize::normalize(&mut formation);
    Inspection {
        valid: true,
        diagnostics,
        semantic_hash: Some(semantic_hash(&formation)),
        explanation: Some(explanation::explain(&formation)),
        normalized: Some(formation),
    }
}

fn escape(text: &str) -> String {
    text.replace('~', "~0").replace('/', "~1")
}

#[derive(Default)]
struct References {
    roles: BTreeSet<String>,
    authorities: BTreeSet<String>,
}

#[cfg(test)]
mod tests;

pub mod catalog;

pub mod diff;
