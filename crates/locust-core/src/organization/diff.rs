//! Offline comparison of the same normalized definitions used by inspection.
use super::{Diagnostic, inspect};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Serialize)]
pub struct Document {
    pub semantic_hash: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Debug, Serialize)]
pub struct Change {
    /// RFC 6901 pointer in the normalized semantic document.
    pub path: String,
    pub kind: &'static str,
    pub before: Option<Value>,
    pub after: Option<Value>,
}
#[derive(Debug, Serialize)]
pub struct Diff {
    pub valid: bool,
    /// Invalid inputs cannot establish semantic equivalence.
    pub equivalent: Option<bool>,
    pub before: Document,
    pub after: Document,
    pub changes: Vec<Change>,
    pub verification: &'static str,
}

/// No member, local authority, delivery or runtime-readiness claims are made.
pub fn compare(before: &str, after: &str) -> Diff {
    let before = inspect(before);
    let after = inspect(after);
    let valid = before.valid && after.valid;
    let mut changes = Vec::new();
    if valid {
        let left = serde_json::to_value(before.normalized.as_ref().unwrap()).unwrap();
        let right = serde_json::to_value(after.normalized.as_ref().unwrap()).unwrap();
        compare_values("", Some(&left), Some(&right), &mut changes);
    }
    Diff {
        valid,
        equivalent: valid.then(|| before.semantic_hash == after.semantic_hash),
        before: Document {
            semantic_hash: before.semantic_hash,
            diagnostics: before.diagnostics,
        },
        after: Document {
            semantic_hash: after.semantic_hash,
            diagnostics: after.diagnostics,
        },
        changes,
        verification: "Normalized offline definition comparison; instance bindings, authority and runtime readiness are unverified",
    }
}
fn pointer(path: &str, key: &str) -> String {
    format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"))
}
fn compare_values(
    path: &str,
    before: Option<&Value>,
    after: Option<&Value>,
    changes: &mut Vec<Change>,
) {
    if before == after {
        return;
    }
    match (before, after) {
        (Some(Value::Object(left)), Some(Value::Object(right))) => {
            for key in left.keys().chain(right.keys()).collect::<BTreeSet<_>>() {
                compare_values(&pointer(path, key), left.get(key), right.get(key), changes);
            }
        }
        (Some(Value::Array(left)), Some(Value::Array(right))) => {
            for index in 0..left.len().max(right.len()) {
                compare_values(
                    &pointer(path, &index.to_string()),
                    left.get(index),
                    right.get(index),
                    changes,
                );
            }
        }
        _ => changes.push(Change {
            path: path.into(),
            kind: match (before, after) {
                (None, _) => "added",
                (_, None) => "removed",
                _ => "changed",
            },
            before: before.cloned(),
            after: after.cloned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalized_defaults_and_whitespace_are_equivalent() {
        let full =
            serde_json::to_string(&locust_proto::organization::Formation::default()).unwrap();
        let diff = compare("{\"schema_version\":2}", &full);
        assert_eq!(diff.equivalent, Some(true));
        assert!(diff.changes.is_empty());
        assert_eq!(diff.before.semantic_hash, diff.after.semantic_hash);
    }
    #[test]
    fn exact_changes_escape_pointer_keys_and_preserve_values() {
        let diff = compare(
            r#"{"schema_version":2,"roles":{"a/b~c":{"description":"old"}}}"#,
            r#"{"schema_version":2,"roles":{"a/b~c":{"description":"new"}}}"#,
        );
        assert_eq!(diff.equivalent, Some(false));
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(diff.changes[0].path, "/roles/a~1b~0c/description");
        assert_eq!(diff.changes[0].before, Some(Value::String("old".into())));
        assert_eq!(diff.changes[0].after, Some(Value::String("new".into())));
    }
    #[test]
    fn invalid_input_retains_side_diagnostics_without_equivalence_claim() {
        let diff = compare(r#"{"schema_version":2}"#, r#"{"schema_version":1}"#);
        assert!(!diff.valid);
        assert_eq!(diff.equivalent, None);
        assert!(diff.before.diagnostics.is_empty());
        assert!(
            diff.after
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "unsupported_version")
        );
        assert!(diff.changes.is_empty());
    }
}
