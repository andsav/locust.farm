//! What this daemon alone knows about a goal: `Space::Goal`.
//!
//! Records are keyed by a tag, the goal and, where they are about one
//! principal or one assignment, that key or identifier:
//!
//! | tag | rest of key | value |
//! |---|---|---|
//! | `r` | goal | revision counter |
//! | `t` | goal | title, cached from the genesis payload |
//! | `g` | goal, principal | standing grants |
//! | `w` | goal, principal | workspace binding |
//! | `j` | goal, principal | join in progress |
//! | `a` | goal, assignment | the owner's authorization of one assignment |
//! | `m` | goal, principal | the principal takes or took part; whether it left |

use std::collections::BTreeMap;

use locust_proto::api::{Checkout, GoalGrants, WorkspaceOperation};
use locust_proto::id::{CheckoutId, EndpointId, EventId, GoalId, PublicKey, WorkspaceOperationId};
use locust_proto::invite::InviteSecret;
use locust_proto::store::{LocalWrite, Space, StoreError};
use serde::{Deserialize, Serialize};

use super::records;

const REVISION: u8 = b'r';
const TITLE: u8 = b't';
const GRANTS: u8 = b'g';
const CHECKOUT: u8 = b'W';
const WORKSPACE_OPERATION: u8 = b'O';
const JOIN: u8 = b'j';
const AUTHORIZATION: u8 = b'a';
const PART: u8 = b'm';

/// A redeemed invitation whose admission has not arrived, or was refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct JoinRecord {
    /// The governance the ticket named, checked against the genesis record
    /// once it arrives.
    pub governance: PublicKey,
    /// The inviting daemon.
    pub endpoint: EndpointId,
    pub hints: Vec<String>,
    pub secret: InviteSecret,
    pub publication: Option<locust_proto::api::InvitationPublication>,
    /// True once the inviter refused the ticket.
    pub refused: bool,
}

/// The owner's authorization of one assignment that no grant covers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Authorization {
    /// Also lets a session of the assignee take the claim over.
    pub takeover: bool,
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
    pub grants: BTreeMap<PublicKey, GoalGrants>,
    pub checkouts: BTreeMap<(PublicKey, CheckoutId), Checkout>,
    pub workspace_operations: BTreeMap<(PublicKey, WorkspaceOperationId), WorkspaceOperation>,
    pub joins: BTreeMap<PublicKey, JoinRecord>,
    pub authorized: BTreeMap<(EventId, PublicKey), Authorization>,
    /// Local principals that are or were members here; true once the
    /// principal asked to leave.
    pub part: BTreeMap<PublicKey, bool>,
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

pub(super) fn grants_write(
    goal: &GoalId,
    principal: &PublicKey,
    grants: &GoalGrants,
) -> LocalWrite {
    records::put(Space::Goal, key(GRANTS, goal, &principal.0), grants)
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

pub(super) fn authorization_write(
    goal: &GoalId,
    round: &EventId,
    principal: &PublicKey,
    authorization: &Authorization,
) -> LocalWrite {
    records::put(
        Space::Goal,
        records::key(AUTHORIZATION, &[&goal.0, &round.0, &principal.0]),
        authorization,
    )
}

pub(super) fn authorization_delete(
    goal: &GoalId,
    round: &EventId,
    principal: &PublicKey,
) -> LocalWrite {
    records::delete(
        Space::Goal,
        records::key(AUTHORIZATION, &[&goal.0, &round.0, &principal.0]),
    )
}

/// Records that `principal` is or was a member here, and whether it left.
pub(super) fn part_write(goal: &GoalId, principal: &PublicKey, left: bool) -> LocalWrite {
    records::put(Space::Goal, key(PART, goal, &principal.0), &left)
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
            (GRANTS, Some(value)) => {
                self.grants
                    .insert(PublicKey(subject()?), records::read(value)?);
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
            (AUTHORIZATION, Some(value)) => {
                self.authorized.insert(
                    (
                        EventId(subject()?),
                        PublicKey(records::part(key, REST + 32).ok_or_else(records::bad_key)?),
                    ),
                    records::read(value)?,
                );
            }
            (AUTHORIZATION, None) => {
                self.authorized.remove(&(
                    EventId(subject()?),
                    PublicKey(records::part(key, REST + 32).ok_or_else(records::bad_key)?),
                ));
            }
            (PART, Some(value)) => {
                self.part
                    .insert(PublicKey(subject()?), records::read(value)?);
            }
            (REVISION | TITLE | GRANTS | PART, None) => {}
            _ => return Err(records::bad_key()),
        }
        Ok(())
    }

    /// The standing grants of `principal`; none unless the owner set them.
    pub fn grants(&self, principal: &PublicKey) -> GoalGrants {
        self.grants.get(principal).copied().unwrap_or_default()
    }
}
