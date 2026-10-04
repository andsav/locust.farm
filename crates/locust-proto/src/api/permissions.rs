//! Local owner permission controls and non-consuming attention views.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{GoalGrants, Halt, Membership, PendingWork, TaskView};
use crate::event::TaskId;
use crate::id::{EventId, GoalId, PublicKey};

/// Independent local permission categories. Membership and organization roles
/// remain separate conditions for an operation to be effective.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GoalPermission {
    Administer,
    Contribute,
    Execute,
    Review,
    Select,
    Flow,
    Takeover,
}

impl GoalPermission {
    pub fn set(self, grants: &mut GoalGrants, allowed: bool) {
        *match self {
            Self::Administer => &mut grants.administer,
            Self::Contribute => &mut grants.contribute,
            Self::Execute => &mut grants.execute,
            Self::Review => &mut grants.review,
            Self::Select => &mut grants.select,
            Self::Flow => &mut grants.flow,
            Self::Takeover => &mut grants.takeover,
        } = allowed;
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GoalPermissions {
    pub goal: GoalId,
    pub agent: PublicKey,
    /// Local display label; the public key identifies the principal.
    pub name: String,
    pub revoked: bool,
    pub membership: Option<Membership>,
    pub grants: GoalGrants,
    pub task_authorizations: Vec<TaskAuthorization>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TaskAuthorization {
    pub task: Option<TaskId>,
    pub round: EventId,
    pub current: bool,
    pub takeover: bool,
}

/// An owner's view of a local principal's obligations. Reading does not claim,
/// acknowledge, consume a context cursor, or authorize anything.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AttentionEntry {
    pub goal: GoalId,
    pub title: Option<String>,
    pub agent: PublicKey,
    pub name: String,
    pub halted: Option<Halt>,
    pub grants: GoalGrants,
    pub pending: PendingWork,
    pub tasks: Vec<TaskView>,
}
