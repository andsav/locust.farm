//! Capability-free invitation review and issuer inventory.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::id::{EndpointId, GoalId, PublicKey};

/// The sharing boundary signed into every invitation. A topic or role is
/// never a confidential subset of a goal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InvitationSharing {
    WholeGoal,
}

/// Authenticated ticket facts, without the bearer capability. Signature
/// verification attributes the presentation to a key, not a human identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InvitationPreview {
    pub goal: GoalId,
    pub goal_title: Option<String>,
    pub administrator: PublicKey,
    pub endpoint: EndpointId,
    pub hints: Vec<String>,
    pub expires_ms: Option<u64>,
    pub expired: bool,
    pub sharing: InvitationSharing,
    /// Digest of this complete signed invitation; confirmation must echo it.
    pub review: String,
    pub signature_verified: bool,
    pub title_provenance: String,
    pub identity_provenance: String,
    /// Inspection is offline; it does not promise admission or contact an issuer.
    pub admission_status: String,
    pub sharing_facts: Vec<String>,
}

impl InvitationPreview {
    pub fn sharing_facts() -> Vec<String> {
        [
            "Admission allows reading shared goal content, including available history; topics and roles are not private channels.",
            "Material you publish to this goal becomes readable to its members under the goal's membership rules.",
            "Joining does not automatically share local files, private chats or credentials.",
            "Membership grants no local execution, provider spending or workspace access; those require separate local authorization.",
            "Revocation stops an unused invitation. Removing a member cannot retract copies or keys already received.",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InvitationState {
    Pending,
    Expired,
    Revoked,
    Redeemed,
}

/// Issuer-local inventory. The identifier is a one-way capability digest;
/// none of these fields can redeem an invitation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InvitationSummary {
    pub invitation: String,
    pub goal: GoalId,
    pub goal_title: Option<String>,
    pub administrator: PublicKey,
    pub created_ms: u64,
    pub expires_ms: Option<u64>,
    pub state: InvitationState,
    pub revoked_ms: Option<u64>,
    pub redeemed_ms: Option<u64>,
    pub redeemed_by: Option<PublicKey>,
    pub redeemed_endpoint: Option<EndpointId>,
}
