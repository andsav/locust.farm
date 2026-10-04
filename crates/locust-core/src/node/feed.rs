//! A goal's event feed: `Space::Cursor`.
//!
//! The feed numbers standing changes from 1, including retractions and
//! missing-proof transitions. An event may therefore appear more than once. That order depends on arrival, so it
//! cannot be recomputed from the log and is stored: one record per position,
//! written in the commit that judges the event. Reading this feed changes
//! nothing; context acknowledgments live separately in `context`.

use std::collections::HashMap;

use locust_proto::id::{EventId, GoalId};
use locust_proto::store::{LocalWrite, Space, StoreError};

use super::records;

const ENTRY: u8 = b'f';

/// The feed of one goal, in memory: position `n` is `order[n - 1]`.
#[derive(Debug, Default)]
pub(super) struct Feed {
    order: Vec<EventId>,
    positions: HashMap<EventId, u64>,
}

impl Feed {
    /// The write that appends `event` at `position`.
    pub fn entry_write(goal: &GoalId, position: u64, event: &EventId) -> LocalWrite {
        records::put(
            Space::Cursor,
            records::key(ENTRY, &[&goal.0, &position.to_be_bytes()]),
            event,
        )
    }

    /// Applies one committed feed entry. Entries are written and loaded in
    /// ascending position, so each one extends the feed by one.
    pub fn absorb(&mut self, key: &[u8], value: &[u8]) -> Result<(), StoreError> {
        let position =
            u64::from_be_bytes(records::part(key, 1 + GoalId::LEN).ok_or_else(records::bad_key)?);
        if position != self.len() + 1 {
            return Err(StoreError::Corrupted("a goal's feed has a gap".into()));
        }
        let event: EventId = records::read(value)?;
        self.order.push(event);
        self.positions.insert(event, position);
        Ok(())
    }

    /// The number of entries, which is also the last position.
    pub fn len(&self) -> u64 {
        self.order.len() as u64
    }

    /// The latest feed position of `event`, if it has been observed.
    pub fn position(&self, event: &EventId) -> Option<u64> {
        self.positions.get(event).copied()
    }

    /// At most `limit` entries after position `after`, with their positions.
    pub fn after(&self, after: u64, limit: usize) -> impl Iterator<Item = (u64, &EventId)> {
        let start =
            usize::try_from(after).map_or(self.order.len(), |after| after.min(self.order.len()));
        self.order[start..]
            .iter()
            .take(limit)
            .zip(after.saturating_add(1)..)
            .map(|(event, position)| (position, event))
    }
}

/// True if `key` is a feed entry rather than a context acknowledgment.
pub(super) fn is_entry(key: &[u8]) -> bool {
    key.first() == Some(&ENTRY)
}

/// The goal a `Space::Cursor` key is about.
pub(super) fn goal_of(key: &[u8]) -> Result<GoalId, StoreError> {
    records::part(key, 1)
        .map(GoalId)
        .ok_or_else(records::bad_key)
}
