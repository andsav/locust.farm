//! What one goal's held events mean: history, standing and state. See the crate documentation.
//!
//! The property every rule here serves: the state of a goal is a function of
//! the set of events held, never of the order they arrived in.
//!
//! - `history`: the held events, per-author logs with running digests, usable
//!   prefixes and fork points.
//! - [`standing`]: how one event stands, and why a goal is halted.
//! - [`state`]: the read model, as of the latest applied decision.

mod append;
mod chain;
mod fold;
mod history;
mod ids;
mod screen;
pub mod standing;
pub mod state;
mod transition;

use locust_proto::event::{AuthorPoint, Event};
use locust_proto::id::{EventId, GoalId, PublicKey};
use locust_proto::store::{Store, StoreError};
use locust_proto::sync::{AuthorFrontier, Frontier};

pub use standing::{Changes, Exclusion, Halt, Next, Standing, Waiting};
pub use state::{
    AcceptedHead, Assignment, Cancellation, Document, Note, Record, RecordKind, Revision, State,
    Submission, Task, TaskState, Verdict,
};

/// Largest number of different events retained at one position of one
/// author's log. Two are enough to prove a fork.
pub use screen::MAX_FORK_VARIANTS;

/// Largest number of events of one author retained past its usable prefix.
pub use screen::MAX_WAITING_PER_AUTHOR;

/// One goal: every event held for it and what they add up to.
pub struct Goal {
    id: GoalId,
    history: history::History,
    folded: fold::Folded,
}

impl Goal {
    /// A goal with nothing held yet, as while a join is in progress.
    pub fn new(id: GoalId) -> Self {
        Self {
            id,
            history: history::History::default(),
            folded: fold::Folded::default(),
        }
    }

    /// Rebuilds a goal by replaying everything `store` holds for it.
    pub fn load<S: Store>(store: &S, id: GoalId) -> Result<Self, StoreError> {
        let mut goal = Self::new(id);
        let mut cursor = 0;
        loop {
            let page = store.log(&id, cursor, 256)?;
            if page.is_empty() {
                break;
            }
            for (position, event) in page {
                goal.history.insert(&event);
                cursor = position;
            }
        }
        goal.folded = fold::fold(&goal.history).0;
        Ok(goal)
    }

    /// The goal's identifier.
    pub fn id(&self) -> GoalId {
        self.id
    }

    /// The number of events held.
    pub fn len(&self) -> usize {
        self.history.events.len()
    }

    /// True while nothing is held.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The subset of `events` to retain, in an order that keeps each author's
    /// log ascending. Call it before the store commit; what it drops will be
    /// offered again by a later reconciliation if it becomes admissible.
    pub fn screen(&self, events: Vec<Event>) -> Vec<Event> {
        let mut events = screen::screen(self.id, &self.history, events);
        events.sort_by_key(|event| (event.header().author, event.header().seq, event.id()));
        events
    }

    /// Applies events the store has durably committed. Events already held,
    /// and events of another goal, are ignored. Returns what changed.
    pub fn apply(&mut self, events: &[Event]) -> Changes {
        let before = self.folded.standings.clone();
        let mut changes = Changes::default();
        let mut judged = Vec::new();
        for event in events {
            if event.header().goal != self.id {
                continue;
            }
            let Some((slot, placed)) = self.history.insert(event) else {
                continue;
            };
            changes.changed = true;
            if !changes.refolded && !self.folded.append(&self.history, slot, placed, &mut judged) {
                changes.refolded = true;
            }
        }
        if changes.refolded {
            (self.folded, judged) = fold::fold(&self.history);
        }
        changes.judged = judged
            .into_iter()
            .filter(|&slot| before.get(slot as usize).is_none_or(Standing::is_pending))
            .map(|slot| self.history.events[slot as usize].id())
            .collect();
        changes
    }

    /// The held event with this identifier.
    pub fn event(&self, id: &EventId) -> Option<&Event> {
        self.history.get(id)
    }

    /// Whether the event with this identifier is held.
    pub fn holds(&self, id: &EventId) -> bool {
        self.history.slot(id).is_some()
    }

    /// How a held event stands.
    pub fn standing(&self, id: &EventId) -> Option<Standing> {
        Some(self.folded.standings[self.history.slot(id)? as usize])
    }

    /// The read model as of the latest applied decision.
    pub fn state(&self) -> &State {
        &self.folded.state
    }

    /// Why the goal's decisions cannot advance, with the evidence.
    pub fn halt(&self) -> Option<&Halt> {
        self.folded.chain.halt.as_ref()
    }

    /// Every author with at least one held event, ascending.
    pub fn authors(&self) -> impl Iterator<Item = &PublicKey> {
        self.history.logs.keys()
    }

    /// The held points of `author`, ascending by (position, identifier).
    pub fn points(&self, author: &PublicKey) -> &[AuthorPoint] {
        self.history.log(author).map_or(&[], |log| &log.points)
    }

    /// The number of leading positions of `author`'s log that form its usable
    /// prefix: each held once, each naming the one before it.
    pub fn usable(&self, author: &PublicKey) -> u64 {
        self.history.log(author).map_or(0, |log| log.usable as u64)
    }

    /// The lowest position of `author`'s log holding two different events.
    pub fn fork_point(&self, author: &PublicKey) -> Option<u64> {
        self.history.log(author).and_then(|log| log.fork)
    }

    /// What this goal holds, an entry per author.
    pub fn frontier(&self) -> Frontier {
        self.history.frontier()
    }

    /// The frontier entry for `author`; the empty entry when nothing is held.
    pub fn frontier_of(&self, author: &PublicKey) -> AuthorFrontier {
        self.history.frontier_of(author)
    }

    /// The reconciliation rule of [`AuthorFrontier::is_prefix_of`], answered
    /// by lookup: true when the peer that sent `theirs` holds exactly this
    /// goal's prefix of that author's log.
    pub fn extends(&self, theirs: &AuthorFrontier) -> bool {
        self.history.extends(theirs)
    }

    /// What the next event signed by `author` must carry. `None` while the
    /// goal has no applied decision, while `author`'s log here is forked or
    /// has events past a missing position (signing would fork it), and for
    /// the coordinator while a decision of its chain waits to be applied.
    pub fn next(&self, author: &PublicKey) -> Option<Next> {
        let anchor = self.state().head?;
        let log = self.history.log(author);
        if log.is_some_and(|log| log.fork.is_some() || log.waiting() != 0)
            || (self.history.coordinator.as_ref() == Some(author)
                && (self.folded.stalled() || self.halt().is_some()))
        {
            return None;
        }
        Some(Next {
            seq: self.usable(author),
            prev: log.and_then(|log| log.tip()),
            anchor,
            epoch: self.state().epoch,
        })
    }

    /// The key epoch of a held event: the epoch at its anchor, or at its own
    /// place for a decision of the chain. `None` while its anchor is not a
    /// decision of the chain.
    pub fn epoch_of(&self, id: &EventId) -> Option<u32> {
        let event = self.event(id)?;
        let position = self.folded.chain.position(id).or_else(|| {
            event
                .header()
                .anchor
                .and_then(|anchor| self.folded.chain.position(&anchor))
        })?;
        Some(self.folded.chain.epoch_at(position))
    }

    /// The task a held event concerns, resolved through its assignment,
    /// result or cancellation.
    pub fn task_of(&self, id: &EventId) -> Option<EventId> {
        use locust_proto::event::Body;
        let assignment_task = |assignment: &EventId| match &self.event(assignment)?.header().body {
            Body::TaskAssigned { task, .. } => Some(*task),
            _ => None,
        };
        match &self.event(id)?.header().body {
            Body::TaskProposed { .. } => Some(*id),
            Body::TaskAssigned { task, .. } => Some(*task),
            Body::AssignmentAccepted { assignment }
            | Body::AssignmentDeclined { assignment }
            | Body::Progress { assignment }
            | Body::AttemptFailed { assignment }
            | Body::ResultSubmitted { assignment, .. }
            | Body::CancelRequested { assignment } => assignment_task(assignment),
            Body::ResultAccepted { result, .. } | Body::ResultRejected { result } => {
                match &self.event(result)?.header().body {
                    Body::ResultSubmitted { assignment, .. } => assignment_task(assignment),
                    _ => None,
                }
            }
            Body::CancelAcknowledged { cancel, .. } => match &self.event(cancel)?.header().body {
                Body::CancelRequested { assignment } => assignment_task(assignment),
                _ => None,
            },
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
