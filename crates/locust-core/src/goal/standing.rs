//! How a held event stands, why a goal is halted, and what `apply` reports.

use locust_proto::id::EventId;

/// How one held event stands in the goal. A function of the set of events
/// held, never of the order they arrived in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// Not judged yet: something it needs is not held.
    Pending(Waiting),
    /// Judged and kept as evidence; it grants nothing.
    Excluded(Exclusion),
    /// Its transition was applied.
    Effective,
}

impl Standing {
    /// True while the event has not been judged.
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending(_))
    }

    /// True if the event's transition was applied.
    pub fn is_effective(&self) -> bool {
        matches!(self, Self::Effective)
    }
}

/// What a pending event waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waiting {
    /// An earlier position of its author's log is missing, its `prev` names
    /// an event that is not the one held before it, or an earlier event of
    /// its author is itself waiting for its anchor.
    Predecessor,
    /// Its anchor is not a decision of the applied chain yet.
    Anchor,
    /// It is a decision naming a contribution that is not held, or is held
    /// and waits for a predecessor.
    Reference,
}

/// Why a judged event grants nothing. The names are stable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exclusion {
    /// At or past its author's fork point.
    Forked,
    /// Its author was not a member at its anchor.
    NotAMember,
    /// Beyond its author's removal cutoff.
    PastRemoval,
    /// Its anchor is earlier in the chain than an anchor its author used
    /// before it.
    AnchorRegressed,
    /// A decision body not signed by the coordinator.
    NotCoordinator,
    /// Its payload names another key epoch than the event's.
    BadEpoch,
    /// The transition is not allowed in the state it applies to.
    Precondition(&'static str),
    /// A coordinator event at or past the halt.
    AfterHalt,
}

impl Exclusion {
    /// Stable name for logs and command output.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Forked => "forked",
            Self::NotAMember => "not_a_member",
            Self::PastRemoval => "past_removal",
            Self::AnchorRegressed => "anchor_regressed",
            Self::NotCoordinator => "not_coordinator",
            Self::BadEpoch => "bad_epoch",
            Self::Precondition(_) => "precondition",
            Self::AfterHalt => "after_halt",
        }
    }

    /// The short reason a precondition failed; absent for the other cases.
    pub fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Precondition(reason) => Some(reason),
            _ => None,
        }
    }
}

/// Why a goal's decision chain cannot advance, with the evidence. Permanent
/// for the held set: more events never remove it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Halt {
    /// The coordinator signed different events at one position of its log.
    Fork {
        /// The coordinator's lowest forked position.
        seq: u64,
        /// Every held event at that position, ascending by identifier.
        events: Vec<EventId>,
    },
    /// A decision names an anchor other than the decision before it.
    BrokenChain {
        /// The offending decision.
        event: EventId,
    },
}

/// What one [`Goal::apply`](super::Goal::apply) changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// Events that had no standing or a pending one before the call and are
    /// judged now, in the order they were applied.
    pub judged: Vec<EventId>,
    /// True if at least one event was newly held, so the history, and
    /// possibly the state or a standing, differs from before the call.
    pub changed: bool,
    /// True if the call recomputed the state from scratch instead of
    /// extending it. Diagnostic only.
    pub refolded: bool,
}

/// What the next event signed by one author must carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Next {
    /// The author's next log position.
    pub seq: u64,
    /// The author's event before it; absent exactly when `seq` is zero.
    pub prev: Option<EventId>,
    /// The latest applied decision. A decision names it as the decision it
    /// succeeds; a contribution names it as its anchor.
    pub anchor: EventId,
    /// The key epoch at `anchor`, which a payload must be sealed under. A
    /// `MemberRemoved` decision is itself in the next epoch, `epoch + 1`.
    pub epoch: u32,
}
