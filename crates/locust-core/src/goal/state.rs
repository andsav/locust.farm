//! Current organization projections. Historical signed events remain available
//! independently of whether their evidence presently grants authority.

use std::collections::{BTreeMap, BTreeSet};

use locust_proto::event::{
    AttemptStatus, CancelOutcome, Context, DecisionAction, Doc, Effect, RulesBinding, ScopeKey,
    TaskBinding, TaskId,
};
use locust_proto::id::{BlobHash, EffectId, EndpointId, EventId, PublicKey};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub principal: PublicKey,
    pub endpoint: EndpointId,
    pub admission: EventId,
    pub removed: Option<EventId>,
    /// Highest content-key epoch this identity may read; removal does not erase old keys.
    pub read_epoch: u32,
}

impl Member {
    pub fn is_active(&self) -> bool {
        self.removed.is_none()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundRules {
    pub id: EventId,
    pub binding: RulesBinding,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub id: TaskId,
    pub creator: PublicKey,
    pub created: EventId,
    pub current_round: EventId,
    pub rounds: BTreeMap<EventId, TaskRound>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskRound {
    pub context: Context,
    pub binding: TaskBinding,
    pub contributions: BTreeSet<EventId>,
    pub attempts: BTreeSet<EventId>,
    pub completed: bool,
    pub selected: Option<EventId>,
    pub closed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub id: EventId,
    pub author: PublicKey,
    pub context: Context,
    pub recipient: PublicKey,
    pub attempts: BTreeSet<EventId>,
    pub declined: BTreeSet<EventId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attempt {
    pub id: EventId,
    pub author: PublicKey,
    pub context: Context,
    pub offer: Option<EventId>,
    /// Reports follow this author's usable log, not arrival time.
    pub status: Option<AttemptStatus>,
    pub reports: Vec<EventId>,
    pub cancellations: BTreeSet<EventId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cancellation {
    pub id: EventId,
    pub author: PublicKey,
    pub attempt: EventId,
    pub acknowledgments: BTreeMap<EventId, CancelOutcome>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contribution {
    pub id: EventId,
    pub sources: Vec<EventId>,
    pub author: PublicKey,
    pub context: Context,
    pub attempt: Option<EventId>,
    pub base: Option<BlobHash>,
    pub patch: Option<BlobHash>,
    pub artifacts: Vec<BlobHash>,
    pub approved: bool,
    /// Exact positive evidence sufficient under the pinned completion rule.
    pub evidence: BTreeSet<EventId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Revision {
    pub id: EventId,
    pub author: PublicKey,
    pub context: Context,
    pub doc: Doc,
    pub base: Option<EventId>,
    pub approved: bool,
    pub evidence: BTreeSet<EventId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    pub revisions: BTreeSet<EventId>,
    pub selected: Option<EventId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub id: EventId,
    pub author: PublicKey,
    pub key: ScopeKey,
    pub previous: Option<EventId>,
    pub action: DecisionAction,
    pub evidence: BTreeSet<EventId>,
}

/// An accepted view is valid only in this exact decision scope. It does not
/// promote its subject or author ancestry into the ordinary effective projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopedSelection {
    pub decision: EventId,
    pub context: Context,
    pub subject: SelectedSubject,
    pub task: Option<TaskRound>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectedSubject {
    Contribution(Contribution),
    Revision(Revision),
}

impl SelectedSubject {
    pub fn id(&self) -> EventId {
        match self {
            Self::Contribution(subject) => subject.id,
            Self::Revision(subject) => subject.id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterializedEffect {
    pub id: EffectId,
    /// Multiple equivalent signatures still name one logical action.
    pub events: BTreeSet<EventId>,
    pub materializer: PublicKey,
    pub effect: Effect,
    pub recipients: BTreeSet<PublicKey>,
    pub acknowledged: BTreeSet<PublicKey>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub administrator: Option<PublicKey>,
    /// Verified governance head, never an accepted workspace head.
    pub head: Option<EventId>,
    pub epoch: u32,
    pub members: BTreeMap<PublicKey, Member>,
    pub current_rules: Option<EventId>,
    pub rules: BTreeMap<EventId, BoundRules>,
    pub tasks: BTreeMap<TaskId, Task>,
    pub offers: BTreeMap<EventId, Offer>,
    pub attempts: BTreeMap<EventId, Attempt>,
    pub cancellations: BTreeMap<EventId, Cancellation>,
    pub contributions: BTreeMap<EventId, Contribution>,
    pub revisions: BTreeMap<EventId, Revision>,
    pub documents: BTreeMap<Doc, Document>,
    pub decisions: BTreeMap<ScopeKey, Vec<Decision>>,
    pub effects: BTreeMap<EffectId, MaterializedEffect>,
    pub selections: BTreeMap<ScopeKey, ScopedSelection>,
    pub leave_requests: BTreeSet<EventId>,
}

impl State {
    pub fn is_member(&self, principal: &PublicKey) -> bool {
        self.members.get(principal).is_some_and(Member::is_active)
    }

    pub fn task_round(&self, context: Context) -> Option<&TaskRound> {
        let locust_proto::event::Scope::Task(task) = context.scope else {
            return None;
        };
        self.tasks.get(&task)?.rounds.get(&context.round)
    }
}
