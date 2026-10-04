//! Execution sessions and the claims they hold.
//!
//! `Space::Session` holds one entry per session instance: the principal the
//! session is bound to and, once its adapter reported one, its record.
//! `Space::Claim` holds one claim per (goal, assignment). A session belongs
//! to the principal that first used it; using it as another is `Denied`.

use std::collections::BTreeMap;

use locust_proto::api::{ApiError, Claim, ErrorCode, SessionRecord};
use locust_proto::id::{EventId, GoalId, InstanceId, PublicKey};
use locust_proto::store::{LocalWrite, Space, StoreError};
use serde::{Deserialize, Serialize};

use super::records;

const SESSION: u8 = b's';
const CLAIM: u8 = b'c';

/// What the store keeps about one session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct SessionEntry {
    /// The principal the session is bound to.
    pub principal: PublicKey,
    /// What the session's adapter last reported, and when.
    pub record: Option<(SessionRecord, u64)>,
}

/// Every session that was ever used here and not dropped.
#[derive(Debug, Default)]
pub(super) struct Sessions {
    by_instance: BTreeMap<InstanceId, SessionEntry>,
}

impl Sessions {
    pub fn write(instance: &InstanceId, entry: &SessionEntry) -> LocalWrite {
        records::put(Space::Session, records::key(SESSION, &[&instance.0]), entry)
    }

    /// Applies one committed write of `Space::Session`.
    pub fn absorb(&mut self, key: &[u8], value: Option<&[u8]>) -> Result<(), StoreError> {
        let instance = InstanceId(records::part(key, 1).ok_or_else(records::bad_key)?);
        match value {
            Some(value) => {
                self.by_instance.insert(instance, records::read(value)?);
            }
            None => {
                self.by_instance.remove(&instance);
            }
        }
        Ok(())
    }

    pub fn get(&self, instance: &InstanceId) -> Option<&SessionEntry> {
        self.by_instance.get(instance)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&InstanceId, &SessionEntry)> {
        self.by_instance.iter()
    }

    /// Checks that `instance` may act as `principal`, and returns the write
    /// that binds it when this is its first use.
    pub fn bind(
        &self,
        instance: &InstanceId,
        principal: &PublicKey,
    ) -> Result<Option<LocalWrite>, ApiError> {
        match self.by_instance.get(instance) {
            Some(entry) if entry.principal == *principal => Ok(None),
            Some(_) => Err(ApiError::new(
                ErrorCode::Denied,
                "the session belongs to another principal",
            )),
            None => Ok(Some(Self::write(
                instance,
                &SessionEntry {
                    principal: *principal,
                    record: None,
                },
            ))),
        }
    }
}

/// One session's hold on an assignment, as stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ClaimRecord {
    pub task: EventId,
    pub instance: InstanceId,
    pub generation: u32,
    pub principal: PublicKey,
}

impl ClaimRecord {
    pub fn view(&self, goal: GoalId, assignment: EventId) -> Claim {
        Claim {
            goal,
            task: self.task,
            assignment,
            instance: self.instance,
            generation: self.generation,
        }
    }
}

pub(super) fn claim_write(goal: &GoalId, assignment: &EventId, claim: &ClaimRecord) -> LocalWrite {
    records::put(
        Space::Claim,
        records::key(CLAIM, &[&goal.0, &assignment.0]),
        claim,
    )
}

/// The goal and assignment a `Space::Claim` key is about.
pub(super) fn claim_subject(key: &[u8]) -> Result<(GoalId, EventId), StoreError> {
    let goal = records::part(key, 1).ok_or_else(records::bad_key)?;
    let assignment = records::part(key, 1 + GoalId::LEN).ok_or_else(records::bad_key)?;
    Ok((GoalId(goal), EventId(assignment)))
}
