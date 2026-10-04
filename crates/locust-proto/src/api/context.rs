//! Coherent shared context and explicit, session-bound acknowledgment.
//!
//! Text in these views is attributed participant content, not instructions from
//! the daemon. Reading never consumes it. A receipt proves which complete event
//! texts the daemon returned; acknowledging it records the session's deliberate
//! acknowledgment, not proof that a model understood or used the content.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{EventDetail, GoalStatus, PendingWork, TaskDetail};
use crate::event::{Doc, TaskId};
use crate::id::{BlobHash, EventId, GoalId, InstanceId, PublicKey, Signature};

/// A continuation for exactly one observed revision and query. If the goal
/// changes, restart the read; pages from different revisions are never joined.
/// Session acknowledgments do not change this revision. The offset addresses
/// relevant events before unread filtering, so acknowledging each page is safe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContextCursor {
    pub goal: GoalId,
    pub revision: u64,
    pub reader: Option<PublicKey>,
    pub session: Option<InstanceId>,
    pub task: Option<TaskId>,
    pub unread_only: bool,
    pub offset: u64,
}

/// One immutable event version delivered with its entire text.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub struct ContextSeen {
    pub event: EventId,
    /// Binds the event's current standing, local feed position and payload availability.
    pub version: BlobHash,
}

/// A daemon-signed receipt bound to the hello session. Only complete, available
/// event texts are listed. Missing or previewed text cannot be acknowledged.
/// Receipts survive daemon restart and remain valid after unrelated changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContextReceipt {
    pub goal: GoalId,
    pub principal: PublicKey,
    pub session: InstanceId,
    pub revision: u64,
    pub entries: Vec<ContextSeen>,
    pub signature: Signature,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContextItem {
    pub event: EventDetail,
    /// False for missing text or an explicitly requested preview. Read again
    /// with preview_chars omitted for full text; the same event remains unread.
    pub text_complete: bool,
    /// None for an observer without a session. Never principal-wide.
    pub acknowledged: Option<bool>,
}

/// Current document selection and candidates; the attributed text and reviews
/// are in the paginated items, including in task-specific context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContextDocument {
    pub doc: Doc,
    pub selected: Option<EventId>,
    pub revisions: Vec<EventId>,
}

/// News for this execution session at a normal pending-work checkpoint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContextNews {
    pub unacknowledged: u64,
    /// Included in unacknowledged; text is not currently readable here.
    pub unavailable: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContextView {
    pub revision: u64,
    pub status: GoalStatus,
    pub task: Option<TaskDetail>,
    /// The requested scope's pinned rules, including a task's variation.
    pub effective_rules_json: String,
    /// Named inputs of the requested goal or task scope.
    pub inputs: BTreeMap<String, BlobHash>,
    pub documents: Vec<ContextDocument>,
    /// Goal-wide obligations from the same observation as all other fields.
    pub pending: PendingWork,
    pub items: Vec<ContextItem>,
    pub next: Option<ContextCursor>,
    /// None for owner/viewer observation without an acting execution session,
    /// or when no complete, available items were delivered on this page.
    pub receipt: Option<ContextReceipt>,
}

/// Durable local acknowledgment of these exact event versions. Repeating a
/// receipt returns the same acknowledgment and cannot consume later versions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContextAcknowledgment {
    pub goal: GoalId,
    pub principal: PublicKey,
    pub session: InstanceId,
    pub entries: Vec<ContextSeen>,
}
