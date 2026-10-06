//! Explicit authority, missing-proof and scope-conflict outcomes.
use std::collections::{BTreeMap, BTreeSet};

use locust_proto::event::{Effect, ScopeKey};
use locust_proto::id::{BlobHash, DefinitionHash, EffectId, EventId, PublicKey};

use super::state::State;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dependency {
    Event(EventId),
    Definition(DefinitionHash),
    Content(BlobHash),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waiting {
    Predecessor,
    Anchor,
    Reference,
    Definition,
    ForkProof,
    Evidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exclusion {
    NotAMember,
    PastRemoval,
    AnchorRegressed,
    NotHost,
    BadEpoch,
    InvalidDefinition,
    Precondition(&'static str),
    AfterHalt,
}

impl Exclusion {
    pub fn name(&self) -> &'static str {
        match self {
            Self::NotAMember => "not_a_member",
            Self::PastRemoval => "past_removal",
            Self::AnchorRegressed => "anchor_regressed",
            Self::NotHost => "not_host",
            Self::BadEpoch => "bad_epoch",
            Self::InvalidDefinition => "invalid_definition",
            Self::Precondition(_) => "precondition",
            Self::AfterHalt => "after_halt",
        }
    }
    pub fn reason(&self) -> Option<&'static str> {
        if let Self::Precondition(reason) = self {
            Some(reason)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Halt {
    Fork {
        seq: u64,
        events: Vec<EventId>,
    },
    BrokenChain {
        event: EventId,
    },
    Successors {
        previous: Option<EventId>,
        events: Vec<EventId>,
    },
    IncompatibleProof {
        event: EventId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    Pending(Waiting),
    Excluded(Exclusion),
    Disputed,
    Effective,
}

impl Standing {
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending(_))
    }
    pub fn is_effective(&self) -> bool {
        matches!(self, Self::Effective)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesiredEffect {
    pub id: EffectId,
    pub runner: PublicKey,
    pub recipients: BTreeSet<PublicKey>,
    pub effect: Effect,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Evaluation {
    pub state: State,
    pub standings: BTreeMap<EventId, Standing>,
    pub host_halt: Option<Halt>,
    pub scope_halts: BTreeMap<ScopeKey, Halt>,
    pub missing: BTreeSet<Dependency>,
    pub retained: BTreeSet<EventId>,
    pub desired_effects: BTreeMap<EffectId, DesiredEffect>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// Every changed standing, including retractions from effective to pending.
    pub judged: Vec<EventId>,
    pub changed: bool,
    pub refolded: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Next {
    pub seq: u64,
    pub prev: Option<EventId>,
    pub anchor: EventId,
    pub epoch: u32,
}
