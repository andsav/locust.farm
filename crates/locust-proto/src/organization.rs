//! Declarative organization definitions for offline authoring.
//!
//! These types do not grant local permissions or enable a goal runtime. They
//! describe the shared contract which the core authoring validator checks.

use std::collections::BTreeMap;

use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};

/// The only organization definition format understood by this implementation.
pub const SCHEMA_VERSION: u32 = 2;

/// An agreement about work organization, independent of local execution.
/// Membership and organization-rule administration belong to one separately
/// authenticated goal host. Roles never grant that authority themselves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Formation {
    #[schemars(range(min = 2, max = 2))]
    pub schema_version: u32,
    #[serde(default)]
    pub roles: BTreeMap<String, Role>,
    #[serde(default)]
    pub context: Context,
    #[serde(default)]
    pub work: WorkRules,
    #[serde(default)]
    pub decisions: DecisionRules,
    /// Explicit policy for the goal's shared file tree. Absence disables writes.
    #[serde(default)]
    pub workspace: Option<WorkspacePolicy>,
    /// Explicitly delegated alternatives to the default task rules.
    #[serde(default)]
    pub task_types: BTreeMap<String, TaskType>,
    /// Optional named stages and their prerequisite evidence.
    #[serde(default)]
    pub flow: BTreeMap<String, Stage>,
}

impl Default for Formation {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            roles: BTreeMap::new(),
            context: Context::default(),
            work: WorkRules::default(),
            decisions: DecisionRules::default(),
            workspace: None,
            task_types: BTreeMap::new(),
            flow: BTreeMap::new(),
        }
    }
}

/// A named role. The host's agent holds it initially; the host gives or takes it from members.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Role {
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Context {
    /// Advisory instructions, versioned with the agreement. Never permissions.
    #[serde(default)]
    pub guidance: String,
    #[serde(default)]
    pub inputs: BTreeMap<String, Input>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub kind: InputKind,
    #[serde(default = "yes")]
    pub required: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    Text,
    Artifact,
}

/// Eligibility within the pinned instance context, not a local tool grant.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Selector {
    #[default]
    Members,
    Role {
        name: String,
    },
    Participant {
        key: String,
    },
    TaskCreator,
    ContributionAuthor,
    Any {
        selectors: Vec<Selector>,
    },
    Nobody,
    OnlyMember,
}

/// One scope-specific authority. A role used here must bind exactly one member.
/// This is separate from membership administration and from local execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Authority {
    Role { name: String },
    Participant { key: String },
}

/// One named integration authority and exact-candidate completion policy.
/// Eligibility here does not grant the host permission to sign a decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspacePolicy {
    pub integrator: Authority,
    #[serde(default)]
    pub completion: CompletionRule,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkRules {
    #[serde(default)]
    pub propose: Selector,
    /// Publishing unattached findings does not require a task or attempt.
    #[serde(default)]
    pub publish: Selector,
    /// An empty list allows contributions but no new task attempts.
    #[serde(default = "independent_starts")]
    pub starts: Vec<StartRule>,
}

fn independent_starts() -> Vec<StartRule> {
    vec![StartRule::Independent {
        by: Selector::Members,
    }]
}

impl Default for WorkRules {
    fn default() -> Self {
        Self {
            propose: Selector::Members,
            publish: Selector::Members,
            starts: independent_starts(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StartRule {
    Independent {
        by: Selector,
    },
    /// An offer needs the recipient's acknowledgment; it does not launch work.
    Offered {
        by: Selector,
        to: Selector,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecisionRules {
    #[serde(default)]
    pub completion: CompletionRule,
    /// Approval alone never chooses one winner among qualifying contributions.
    #[serde(default)]
    pub selection: Option<Authority>,
    /// An optional single decider who may declare the scope finished; an empty board is not finished.
    #[serde(default)]
    pub finish: Option<Authority>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CompletionRule {
    Contribution {
        by: Selector,
    },
    Declaration {
        by: Selector,
    },
    Reviews {
        by: Selector,
        #[schemars(range(min = 1))]
        count: u32,
        #[serde(default = "yes")]
        exclude_author: bool,
    },
    /// A named attestor reports a check on the exact candidate. A label does
    /// not prove the check ran independently or that a local command is allowed.
    Check {
        name: String,
        by: Selector,
    },
    All {
        rules: Vec<CompletionRule>,
    },
    Any {
        rules: Vec<CompletionRule>,
    },
}

impl Default for CompletionRule {
    fn default() -> Self {
        Self::Declaration {
            by: Selector::ContributionAuthor,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaskType {
    /// Omitted groups inherit the goal definition's full corresponding group.
    #[serde(default)]
    pub work: Option<WorkRules>,
    #[serde(default)]
    pub decisions: Option<DecisionRules>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    /// Eligible recipients of durable ready-work delivery, resolved at the materialization anchor.
    #[serde(default)]
    pub recipients: Selector,
    #[serde(default)]
    pub task_type: Option<String>,
    #[serde(default)]
    pub requires: Vec<Prerequisite>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Prerequisite {
    pub stage: String,
    pub evidence: EvidenceKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Publication,
    Review,
    Completion,
    Selection,
}

/// Exported from the same types used by deserialization and validation.
pub fn schema() -> serde_json::Value {
    serde_json::to_value(schema_for!(Formation)).expect("JSON Schema is JSON serializable")
}

/// The semantic identity of a normalized current-format definition. This is
/// distinct from the hash of encrypted bytes used to share it within a goal.
pub fn semantic_hash(normalized: &Formation) -> String {
    let bytes = crate::codec::encode(normalized).expect("formation is canonically encodable");
    let mut hash = blake3::Hasher::new_derive_key("locust organization definition v1");
    hash.update(&bytes);
    hash.finalize().to_hex().to_string()
}

/// Display metadata is outside the semantic definition.
#[derive(Clone, Debug, Serialize)]
pub struct Preset {
    pub name: String,
    pub description: String,
    pub formation: Formation,
}

mod presets;
pub use presets::presets;

mod contract;
pub use contract::{OPERATIONS, Operation, contract};

pub mod catalog;

/// The shared validation rule for declared, signed and invited roles.
pub fn is_role_name(name: &str) -> bool {
    !name.trim().is_empty() && !name.chars().any(char::is_control)
}
