//! What this daemon alone knows about a goal: `Space::Goal`.
//!
//! Records are keyed by a tag, the goal and, where they are about one
//! principal or one assignment, that key or identifier:
//!
//! | tag | rest of key | value |
//! |---|---|---|
//! | `r` | goal | revision counter |
//! | `t` | goal | title, cached from the genesis payload |
//! | `l` | goal, principal | local level |
//! | `w` | goal, principal | workspace binding |
//! | `j` | goal, principal | join in progress |
//! | `a` | goal, task, principal | wanted or allowed task |
//! | `o` | goal, event | signed at the owner's direct request |
//! | `m` | goal, principal | the principal takes or took part; whether it left |
//! | `K` | goal | the seed of the goal's governance key, held by its host |

use std::collections::{BTreeMap, BTreeSet};

use locust_proto::api::{Checkout, Level, WorkspaceOperation};
use locust_proto::crypto::Keypair;
use locust_proto::event::TaskId;
use locust_proto::id::{
    CheckoutId, EffectId, EndpointId, EventId, GoalId, PublicKey, WorkspaceOperationId,
};
use locust_proto::invite::InviteSecret;
use locust_proto::store::{LocalWrite, Space, StoreError};
use serde::{Deserialize, Serialize};

use super::records;

const REVISION: u8 = b'r';
const TITLE: u8 = b't';
const LEVEL: u8 = b'l';
const CHECKOUT: u8 = b'W';
const WORKSPACE_OPERATION: u8 = b'O';
const JOIN: u8 = b'j';
const ALLOWANCE: u8 = b'a';
const BY_OWNER: u8 = b'o';
const PART: u8 = b'm';
const GOVERNANCE: u8 = b'K';

/// A redeemed invitation whose admission has not arrived, or was refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct JoinRecord {
    /// The governance the ticket named, checked against the genesis record
    /// once it arrives.
    pub governance: PublicKey,
    pub host_name: String,
    pub name: String,
    /// The inviting daemon.
    pub endpoint: EndpointId,
    pub hints: Vec<String>,
    pub secret: InviteSecret,
    pub publication: Option<locust_proto::api::InvitationPublication>,
    /// True once the inviter refused the ticket.
    pub refused: bool,
}

/// One local request to take a task, or an allowance for its exact round.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Allowance {
    Wanted { since_ms: u64 },
    Allowed { round: EventId },
}

/// One goal's local records, as loaded.
#[derive(Debug, Default)]
pub(super) struct Local {
    pub goal_sync: BTreeMap<EndpointId, u64>,
    pub farm: Option<super::farm::FarmLocal>,
    /// Zero until the first commit about the goal; a stored revision is
    /// never zero.
    pub revision: u64,
    pub title: Option<String>,
    pub levels: BTreeMap<PublicKey, Level>,
    pub checkouts: BTreeMap<(PublicKey, CheckoutId), Checkout>,
    pub workspace_operations: BTreeMap<(PublicKey, WorkspaceOperationId), WorkspaceOperation>,
    pub joins: BTreeMap<PublicKey, JoinRecord>,
    pub allowances: BTreeMap<(TaskId, PublicKey), Allowance>,
    pub by_owner: BTreeSet<EventId>,
    /// Local principals that are or were members here; true once the
    /// principal asked to leave.
    pub part: BTreeMap<PublicKey, bool>,
    /// The goal's governance key, on the computer that hosts the goal.
    pub governance: Option<Keypair>,
}

fn key(tag: u8, goal: &GoalId, rest: &[u8]) -> Vec<u8> {
    records::key(tag, &[&goal.0, rest])
}

pub(super) fn revision_write(goal: &GoalId, revision: u64) -> LocalWrite {
    records::put(Space::Goal, key(REVISION, goal, &[]), &revision)
}

pub(super) fn title_write(goal: &GoalId, title: &str) -> LocalWrite {
    records::put(Space::Goal, key(TITLE, goal, &[]), title)
}

pub(super) fn level_write(goal: &GoalId, principal: &PublicKey, level: &Level) -> LocalWrite {
    records::put(Space::Goal, key(LEVEL, goal, &principal.0), level)
}

pub(super) fn level_delete(goal: &GoalId, principal: &PublicKey) -> LocalWrite {
    records::delete(Space::Goal, key(LEVEL, goal, &principal.0))
}

pub(super) fn checkout_write(
    goal: &GoalId,
    principal: &PublicKey,
    checkout: &Checkout,
) -> LocalWrite {
    records::put(
        Space::Goal,
        records::key(CHECKOUT, &[&goal.0, &principal.0, &checkout.id.0]),
        checkout,
    )
}

pub(super) fn workspace_operation_write(
    goal: &GoalId,
    principal: &PublicKey,
    operation: &WorkspaceOperation,
) -> LocalWrite {
    records::put(
        Space::Goal,
        records::key(
            WORKSPACE_OPERATION,
            &[&goal.0, &principal.0, &operation.id.0],
        ),
        operation,
    )
}

pub(super) fn join_write(goal: &GoalId, principal: &PublicKey, join: &JoinRecord) -> LocalWrite {
    records::put(Space::Goal, key(JOIN, goal, &principal.0), join)
}

pub(super) fn join_delete(goal: &GoalId, principal: &PublicKey) -> LocalWrite {
    records::delete(Space::Goal, key(JOIN, goal, &principal.0))
}

pub(super) fn task_bytes(task: &TaskId) -> [u8; 33] {
    let mut bytes = [0; 33];
    match task {
        TaskId::Authored(event) => bytes[1..].copy_from_slice(&event.0),
        TaskId::Derived(effect) => {
            bytes[0] = 1;
            bytes[1..].copy_from_slice(&effect.0);
        }
    }
    bytes
}

pub(super) fn task_from_bytes(bytes: [u8; 33]) -> Result<TaskId, StoreError> {
    let id: [u8; 32] = bytes[1..].try_into().expect("exact task key width");
    match bytes[0] {
        0 => Ok(TaskId::Authored(EventId(id))),
        1 => Ok(TaskId::Derived(EffectId(id))),
        _ => Err(records::bad_key()),
    }
}

pub(super) fn allowance_write(
    goal: &GoalId,
    task: &TaskId,
    principal: &PublicKey,
    allowance: &Allowance,
) -> LocalWrite {
    records::put(
        Space::Goal,
        records::key(ALLOWANCE, &[&goal.0, &task_bytes(task), &principal.0]),
        allowance,
    )
}

pub(super) fn allowance_delete(goal: &GoalId, task: &TaskId, principal: &PublicKey) -> LocalWrite {
    records::delete(
        Space::Goal,
        records::key(ALLOWANCE, &[&goal.0, &task_bytes(task), &principal.0]),
    )
}

pub(super) fn by_owner_write(goal: &GoalId, event: &EventId) -> LocalWrite {
    records::put(
        Space::Goal,
        records::key(BY_OWNER, &[&goal.0, &event.0]),
        &(),
    )
}

/// Records that `principal` is or was a member here, and whether it left.
pub(super) fn part_write(goal: &GoalId, principal: &PublicKey, left: bool) -> LocalWrite {
    records::put(Space::Goal, key(PART, goal, &principal.0), &left)
}

/// Stores the seed of the goal's governance key. Written once, by
/// `goal_create`, in the commit that founds the goal.
pub(super) fn governance_write(goal: &GoalId, seed: &[u8; 32]) -> LocalWrite {
    records::put(Space::Goal, key(GOVERNANCE, goal, &[]), seed)
}

/// The goal a `Space::Goal` key is about.
pub(super) fn goal_of(key: &[u8]) -> Result<GoalId, StoreError> {
    records::part(key, 1)
        .map(GoalId)
        .ok_or_else(records::bad_key)
}

impl Local {
    /// Applies one committed write of `Space::Goal` that is about this goal.
    pub fn absorb(&mut self, key: &[u8], value: Option<&[u8]>) -> Result<(), StoreError> {
        const REST: usize = 1 + GoalId::LEN;
        let tag = *key.first().ok_or_else(records::bad_key)?;
        let subject = || records::part::<32>(key, REST).ok_or_else(records::bad_key);
        match (tag, value) {
            (b'Y', Some(value)) => {
                self.goal_sync
                    .insert(EndpointId(subject()?), records::read(value)?);
            }
            (_, Some(value)) if super::farm::is_record(key) => {
                self.farm = Some(records::read(value)?)
            }
            (REVISION, Some(value)) => self.revision = records::read(value)?,
            (TITLE, Some(value)) => self.title = Some(records::read(value)?),
            (LEVEL, Some(value)) => {
                self.levels
                    .insert(PublicKey(subject()?), records::read(value)?);
            }
            (LEVEL, None) => {
                self.levels.remove(&PublicKey(subject()?));
            }
            (CHECKOUT, Some(value)) => {
                let id = CheckoutId(records::part(key, REST + 32).ok_or_else(records::bad_key)?);
                self.checkouts
                    .insert((PublicKey(subject()?), id), records::read(value)?);
            }
            (WORKSPACE_OPERATION, Some(value)) => {
                let id = WorkspaceOperationId(
                    records::part(key, REST + 32).ok_or_else(records::bad_key)?,
                );
                self.workspace_operations
                    .insert((PublicKey(subject()?), id), records::read(value)?);
            }
            (JOIN, Some(value)) => {
                self.joins
                    .insert(PublicKey(subject()?), records::read(value)?);
            }
            (JOIN, None) => {
                self.joins.remove(&PublicKey(subject()?));
            }
            (ALLOWANCE, value) => {
                let task = task_from_bytes(records::part(key, REST).ok_or_else(records::bad_key)?)?;
                let principal =
                    PublicKey(records::part(key, REST + 33).ok_or_else(records::bad_key)?);
                if let Some(value) = value {
                    self.allowances
                        .insert((task, principal), records::read(value)?);
                } else {
                    self.allowances.remove(&(task, principal));
                }
            }
            (BY_OWNER, Some(_)) => {
                self.by_owner.insert(EventId(subject()?));
            }
            (BY_OWNER, None) => {
                self.by_owner.remove(&EventId(subject()?));
            }
            (PART, Some(value)) => {
                self.part
                    .insert(PublicKey(subject()?), records::read(value)?);
            }
            (GOVERNANCE, Some(value)) => {
                let seed: [u8; 32] = records::read(value)?;
                self.governance = Some(Keypair::from_seed(seed));
            }
            (REVISION | TITLE | PART | GOVERNANCE, None) => {}
            _ => return Err(records::bad_key()),
        }
        Ok(())
    }

    /// Missing local record defaults an admitted agent to auto.
    pub fn level(&self, principal: &PublicKey) -> Level {
        self.levels.get(principal).copied().unwrap_or(Level::Auto)
    }

    /// A current-round allowance lowers the start/resume threshold to ask.
    pub fn start_level(&self, task: TaskId, round: EventId, principal: PublicKey) -> Level {
        if self.allowances.get(&(task, principal)) == Some(&Allowance::Allowed { round }) {
            Level::Ask
        } else {
            Level::Auto
        }
    }
}
