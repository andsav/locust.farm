//! The read model of a goal: what its judged events add up to as of the
//! latest applied decision. Plain data; the node renders its views from it.
//!
//! Every list is in canonical application order, so two daemons holding the
//! same events hold equal states, field for field.

use std::collections::{BTreeMap, BTreeSet};

use locust_proto::event::{CancelOutcome, Doc};
use locust_proto::id::{BlobHash, EndpointId, EventId, PublicKey};

use super::ids::IdMap;

/// Where a task stands. The cases are those of the local API's task state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    /// Proposed, not assigned.
    Proposed,
    /// Assigned; the assignee has not answered.
    Assigned,
    /// The assignee took the assignment, or reported progress on it.
    Taken,
    /// The assignee declined.
    Declined,
    /// The attempt failed.
    Failed,
    /// A result awaits the coordinator.
    Submitted,
    /// The coordinator accepted a result.
    Accepted,
    /// The coordinator rejected the latest result.
    Rejected,
    /// Cancellation requested; the executor has not answered.
    CancelRequested,
    /// The executor answered a cancellation.
    Cancelled(CancelOutcome),
}

/// One task, identified by the event that proposed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    /// The `TaskProposed` event.
    pub id: EventId,
    /// Who proposed it.
    pub proposer: PublicKey,
    /// Manifest of the snapshot the work starts from.
    pub input: Option<BlobHash>,
    /// Tasks this one was proposed as depending on; not checked here.
    pub depends_on: Vec<EventId>,
    /// Authored absolute deadline in Unix milliseconds.
    pub deadline_ms: Option<u64>,
    /// Authored attempt budget; absent means unlimited.
    pub max_attempts: Option<u32>,
    /// Where the task stands.
    pub state: TaskState,
    /// Attempt number of the current assignment; zero before the first.
    pub attempt: u32,
    /// The current assignment.
    pub assignment: Option<EventId>,
    /// Who the current assignment names.
    pub assignee: Option<PublicKey>,
    /// The latest result of the current assignment.
    pub result: Option<EventId>,
    /// The unanswered cancellation request of the current assignment.
    pub cancel: Option<EventId>,
    /// Every assignment of the task, oldest first.
    pub assignments: Vec<EventId>,
    /// The accepted result.
    pub accepted: Option<EventId>,
    /// The workspace head the acceptance carried.
    pub accepted_head: Option<BlobHash>,
}

/// What an assignee recorded on an assignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordKind {
    /// `AssignmentAccepted`.
    Accepted,
    /// `AssignmentDeclined`.
    Declined,
    /// `Progress`.
    Progress,
    /// `ResultSubmitted`.
    Submitted,
    /// `AttemptFailed`.
    Failed,
    /// `CancelAcknowledged`.
    CancelAcknowledged(CancelOutcome),
}

/// One event an assignee recorded on an assignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    /// The assignee's event.
    pub event: EventId,
    /// What it reports.
    pub kind: RecordKind,
}

/// A cancellation of one assignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cancellation {
    /// The `CancelRequested` decision.
    pub request: EventId,
    /// The assignee's latest answer.
    pub outcome: Option<CancelOutcome>,
}

/// One assignment, identified by the `TaskAssigned` decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    /// The `TaskAssigned` decision.
    pub id: EventId,
    /// The task it is for.
    pub task: EventId,
    /// The member who is to do the work.
    pub assignee: PublicKey,
    /// Its attempt number, from one.
    pub attempt: u32,
    /// Permanently fenced by a removal, including after readmission.
    pub revoked: bool,
    /// What the assignee recorded on it, in the assignee's order. Records
    /// stay here when the assignment is superseded or cancelled.
    pub records: Vec<Record>,
    /// Its latest submitted result.
    pub result: Option<EventId>,
    /// Its cancellation, if one was requested.
    pub cancel: Option<Cancellation>,
}

/// What the coordinator decided about a result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Accepted by this decision, with the head it made the accepted one.
    Accepted {
        /// The `ResultAccepted` decision.
        decision: EventId,
        /// The manifest that became the accepted head.
        head: Option<BlobHash>,
    },
    /// Rejected by this decision.
    Rejected {
        /// The `ResultRejected` decision.
        decision: EventId,
    },
}

/// One submitted result, identified by the `ResultSubmitted` event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submission {
    /// The `ResultSubmitted` event.
    pub id: EventId,
    /// The assignment it answers.
    pub assignment: EventId,
    /// The task that assignment is for.
    pub task: EventId,
    /// Who submitted it.
    pub author: PublicKey,
    /// Manifest the work was done against.
    pub base: Option<BlobHash>,
    /// Patch against `base`.
    pub patch: Option<BlobHash>,
    /// Further output objects.
    pub artifacts: Vec<BlobHash>,
    /// The coordinator's latest decision about it.
    pub verdict: Option<Verdict>,
}

/// One accepted workspace head.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcceptedHead {
    /// The manifest.
    pub head: BlobHash,
    /// The `ResultAccepted` decision that named it.
    pub decision: EventId,
    /// The accepted result.
    pub result: EventId,
    /// The task that result is for.
    pub task: EventId,
}

/// One proposed revision of a shared document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revision {
    /// The `Revision` event.
    pub id: EventId,
    /// Who proposed it.
    pub author: PublicKey,
    /// The accepted revision it was written against.
    pub base: Option<EventId>,
}

/// A shared document: what is accepted and every revision proposed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    /// The accepted revision.
    pub accepted: Option<EventId>,
    /// Every effective revision, in application order.
    pub revisions: Vec<Revision>,
}

impl Document {
    /// Revisions proposed against the accepted one and not decided yet.
    pub fn open(&self) -> impl Iterator<Item = &Revision> {
        self.revisions
            .iter()
            .filter(|revision| revision.base == self.accepted)
    }
}

/// One note or finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Note {
    /// The `Note` event.
    pub id: EventId,
    /// Who wrote it.
    pub author: PublicKey,
    /// The event it is about. Not checked: it may name anything, held or not.
    pub about: Option<EventId>,
    /// The earlier note it corrects, as authored.
    pub supersedes: Option<EventId>,
    /// The author's clock. Diagnostic only.
    pub at_ms: u64,
    /// The later effective note by the same author that corrects this one.
    pub superseded_by: Option<EventId>,
}

/// What an event identifier denotes in the state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Entry {
    Task(u32),
    Assignment(u32),
    /// A cancellation request, by the assignment it cancels.
    Cancel(u32),
    Result(u32),
    Note(u32),
    Revision(Doc, u32),
}

/// A goal as of its latest applied decision.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// The key named by the genesis record; absent until a genesis is held.
    pub coordinator: Option<PublicKey>,
    /// The applied genesis event.
    pub genesis: Option<EventId>,
    /// The latest applied decision.
    pub head: Option<EventId>,
    /// The key epoch at `head`.
    pub epoch: u32,
    /// The first decision of every epoch, by epoch: the genesis, then each
    /// `MemberRemoved` decision. Its payload proves that epoch's key.
    pub epochs: Vec<EventId>,
    /// Current members and the endpoint each is bound to.
    pub members: BTreeMap<PublicKey, EndpointId>,
    /// The members each endpoint speaks for.
    pub endpoints: BTreeMap<EndpointId, BTreeSet<PublicKey>>,
    /// Tasks in board order: the order their proposals were applied.
    pub tasks: Vec<Task>,
    /// Every assignment, in decision order.
    pub assignments: Vec<Assignment>,
    /// Every submitted result, in application order.
    pub results: Vec<Submission>,
    /// Accepted workspace heads, oldest first; the last is the current one.
    pub accepted_heads: Vec<AcceptedHead>,
    /// The shared documents.
    pub documents: BTreeMap<Doc, Document>,
    /// Notes in application order.
    pub notes: Vec<Note>,
    /// The open leave request of each author that has one.
    pub leaves: BTreeMap<PublicKey, EventId>,
    /// What each event identifier denotes above.
    pub(super) index: IdMap<Entry>,
}

impl State {
    /// True if `key` is a current member.
    pub fn is_member(&self, key: &PublicKey) -> bool {
        self.members.contains_key(key)
    }

    /// The members `endpoint` speaks for; empty for an unknown endpoint.
    pub fn speaks_for(&self, endpoint: &EndpointId) -> impl Iterator<Item = &PublicKey> {
        self.endpoints.get(endpoint).into_iter().flatten()
    }

    /// The task proposed by `id`.
    pub fn task(&self, id: &EventId) -> Option<&Task> {
        match self.index.get(id)? {
            Entry::Task(at) => self.tasks.get(*at as usize),
            _ => None,
        }
    }

    /// The assignment made by `id`.
    pub fn assignment(&self, id: &EventId) -> Option<&Assignment> {
        match self.index.get(id)? {
            Entry::Assignment(at) => self.assignments.get(*at as usize),
            _ => None,
        }
    }

    /// The assignment that the cancellation request `id` cancels.
    pub fn cancelled(&self, id: &EventId) -> Option<&Assignment> {
        match self.index.get(id)? {
            Entry::Cancel(at) => self.assignments.get(*at as usize),
            _ => None,
        }
    }

    /// The result submitted by `id`.
    pub fn result(&self, id: &EventId) -> Option<&Submission> {
        match self.index.get(id)? {
            Entry::Result(at) => self.results.get(*at as usize),
            _ => None,
        }
    }

    /// The note `id`.
    pub fn note(&self, id: &EventId) -> Option<&Note> {
        match self.index.get(id)? {
            Entry::Note(at) => self.notes.get(*at as usize),
            _ => None,
        }
    }

    /// The revision proposed by `id`, with the document it revises.
    pub fn revision(&self, id: &EventId) -> Option<(Doc, &Revision)> {
        match self.index.get(id)? {
            Entry::Revision(doc, at) => {
                let revision = self.documents.get(doc)?.revisions.get(*at as usize)?;
                Some((*doc, revision))
            }
            _ => None,
        }
    }

    /// The shared document `doc`; absent while nothing was proposed for it.
    pub fn document(&self, doc: Doc) -> Option<&Document> {
        self.documents.get(&doc)
    }

    /// The accepted workspace head.
    pub fn accepted_head(&self) -> Option<BlobHash> {
        self.accepted_heads.last().map(|accepted| accepted.head)
    }

    /// Where `head` stands in the list of accepted heads, counting from the
    /// oldest; the latest place when it was accepted more than once. A task's
    /// accepted head is applied locally when the head the workspace records
    /// stands at or after it.
    pub fn head_position(&self, head: &BlobHash) -> Option<usize> {
        self.accepted_heads
            .iter()
            .rposition(|accepted| accepted.head == *head)
    }
}
