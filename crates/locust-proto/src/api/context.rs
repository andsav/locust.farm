//! Coherent shared context and explicit, session-bound acknowledgment.
//!
//! Text in these views is attributed participant content, not instructions from
//! the daemon. Reading never consumes it. A receipt proves which complete event
//! texts the daemon returned; acknowledging it records the session's deliberate
//! acknowledgment, not proof that a model understood or used the content.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    CancelItem, Claim, DeliveryItem, EventDetail, GoalStatus, Halt, PendingWork, ReviewItem,
    TaskDetail, WorkItem,
};
use crate::event::{Context, Doc, TaskId};
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
    pub view: ContextViewMode,
    pub preview_chars: Option<u32>,
    pub limit: u32,
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
    /// Present only on the first page. Continuations deliver items and receipts
    /// from the same revision without repeating the scope snapshot.
    pub summary: Option<ContextSummary>,
    pub items: Vec<ContextItem>,
    pub next: Option<ContextCursor>,
    /// None for owner/viewer observation without an acting execution session,
    /// or when no complete, available items were delivered on this page.
    pub receipt: Option<ContextReceipt>,
}

/// Explicit scope snapshot detail. Event text completeness is controlled only
/// by the separately requested preview, never by this summary choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContextViewMode {
    Compact,
    Full,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContextSummary {
    Compact(Box<ContextBrief>),
    Full(Box<ContextSnapshot>),
}

/// Small current-scope checkpoint. Detailed rules, task text, membership, peer
/// status and obligations remain available through explicit full/detail reads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContextBrief {
    pub goal: GoalId,
    pub title: Option<String>,
    pub administrator: PublicKey,
    pub governance_head: Option<EventId>,
    pub current_rules: Option<EventId>,
    pub halted: Option<Halt>,
    pub context: Option<Context>,
    pub documents: Vec<ContextDocumentSelection>,
    pub pending: PendingCounts,
    pub context_news: Option<ContextNews>,
}

/// Document selections without repeating the complete revision history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContextDocumentSelection {
    pub doc: Doc,
    pub selected: Option<EventId>,
}

/// Complete scope metadata on an explicitly requested full first page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContextSnapshot {
    pub status: GoalStatus,
    pub task: Option<TaskDetail>,
    /// The requested scope's pinned rules, including a task's variation.
    pub effective_rules_json: String,
    /// Named inputs of the requested goal or task scope.
    pub inputs: BTreeMap<String, BlobHash>,
    pub documents: Vec<ContextDocument>,
    /// Goal-wide obligations from the same observation as all other fields.
    pub pending: PendingWork,
}

/// Counts describe all obligations, even when a page filters one category.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PendingCounts {
    pub to_authorize: u64,
    pub to_start: u64,
    pub claimed: u64,
    pub held_elsewhere: u64,
    pub to_acknowledge: u64,
    pub to_review: u64,
    pub deliveries: u64,
}

impl From<&PendingWork> for PendingCounts {
    fn from(work: &PendingWork) -> Self {
        Self {
            to_authorize: work.to_authorize.len() as u64,
            to_start: work.to_start.len() as u64,
            claimed: work.claimed.len() as u64,
            held_elsewhere: work.held_elsewhere.len() as u64,
            to_acknowledge: work.to_acknowledge.len() as u64,
            to_review: work.to_review.len() as u64,
            deliveries: work.deliveries.len() as u64,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PendingKind {
    ToAuthorize,
    ToStart,
    Claimed,
    HeldElsewhere,
    ToAcknowledge,
    ToReview,
    Deliveries,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PendingItem {
    ToAuthorize(WorkItem),
    ToStart(WorkItem),
    Claimed(Claim),
    HeldElsewhere(Claim),
    ToAcknowledge(CancelItem),
    ToReview(ReviewItem),
    Deliveries(DeliveryItem),
}

/// Continuation for one complete obligation query and execution session.
/// Changes to goal content or local work authority require restarting the read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PendingCursor {
    pub goal: GoalId,
    pub revision: u64,
    pub reader: Option<PublicKey>,
    pub session: Option<InstanceId>,
    pub kind: Option<PendingKind>,
    pub limit: u32,
    pub offset: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PendingPage {
    pub revision: u64,
    pub counts: PendingCounts,
    pub context_news: Option<ContextNews>,
    pub items: Vec<PendingItem>,
    pub next: Option<PendingCursor>,
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
