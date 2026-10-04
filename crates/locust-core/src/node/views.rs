//! What reads answer with: the API's views, rendered from a goal's state and
//! this daemon's records about it.

use std::collections::BTreeSet;

use locust_proto::api::{
    self, AssignmentRef, BlobState, BlobStatus, CancelItem, EventDetail, EventView, GoalSummary,
    Membership, PendingWork, ReviewItem, TaskDetail, TaskView,
};
use locust_proto::engine::Entropy;
use locust_proto::event::Event;
use locust_proto::id::{BlobHash, EventId, PublicKey};
use locust_proto::store::Store;

use super::Node;
use super::callers::Actor;
use super::entry::Entry;
use crate::goal::{Standing, Task, TaskState};

/// The API's name for a task state.
pub(super) fn task_state(state: TaskState) -> api::TaskState {
    match state {
        TaskState::Proposed => api::TaskState::Proposed,
        TaskState::Assigned => api::TaskState::Assigned,
        TaskState::Taken => api::TaskState::Taken,
        TaskState::Declined => api::TaskState::Declined,
        TaskState::Failed => api::TaskState::Failed,
        TaskState::Submitted => api::TaskState::Submitted,
        TaskState::Accepted => api::TaskState::Accepted,
        TaskState::Rejected => api::TaskState::Rejected,
        TaskState::CancelRequested => api::TaskState::CancelRequested,
        TaskState::Cancelled(outcome) => api::TaskState::Cancelled(outcome),
    }
}

/// True while the current assignment of a task in this state still has work
/// or an answer outstanding from its assignee.
pub(super) fn unfinished(state: TaskState) -> bool {
    matches!(
        state,
        TaskState::Assigned | TaskState::Taken | TaskState::CancelRequested
    )
}

impl Entry {
    /// How a local principal stands in the goal; `None` when it never took
    /// part, in which case the goal does not exist for it.
    pub fn membership(&self, principal: &PublicKey) -> Option<Membership> {
        if self.local.joins.contains_key(principal) {
            return Some(Membership::Joining);
        }
        Some(match self.local.part.get(principal)? {
            true => Membership::Left,
            false if self.is_member(principal) => Membership::Member,
            false => Membership::Removed,
        })
    }

    pub fn halted(&self) -> Option<api::Halt> {
        self.goal.halt().map(|_| api::Halt::AuthorityConflict)
    }

    /// True if `principal` may claim `assignment` without asking the owner.
    pub fn may_claim(&self, principal: &PublicKey, assignment: &EventId) -> bool {
        self.local.grants(principal).execute || self.local.authorized.contains_key(assignment)
    }

    /// One event as the feed lists it.
    pub fn event_view(&self, event: &Event) -> EventView {
        let id = event.id();
        let header = event.header();
        EventView {
            position: self.feed.position(&id),
            event: id,
            author: header.author,
            kind: header.body.kind().into(),
            at_ms: header.at_ms,
            standing: match self.goal.standing(&id) {
                Some(Standing::Effective) => api::Standing::Effective,
                Some(Standing::Excluded(_)) => api::Standing::Excluded,
                Some(Standing::Pending(_)) | None => api::Standing::Pending,
            },
        }
    }
}

impl<S: Store, E: Entropy> Node<S, E> {
    /// The goal's title: the cached one, or the genesis text once its
    /// payload and key are held.
    pub(super) fn title(&self, entry: &Entry) -> Option<String> {
        if let Some(payload) = entry
            .state()
            .genesis
            .and_then(|id| entry.goal.event(&id))
            .and_then(|event| event.header().payload)
            && super::requests::content::blob_record(&self.store, &entry.id(), &payload.hash)
                .ok()
                .flatten()
                .is_some_and(|record| record.withdrawn)
        {
            return None;
        }
        entry.local.title.clone().or_else(|| {
            let genesis = entry.goal.event(&entry.state().genesis?)?;
            entry.text(&self.store, genesis)
        })
    }

    /// One entry per goal and local principal in it: only `principal`'s own
    /// when one is named.
    pub(super) fn goal_summaries(&self, principal: Option<PublicKey>) -> Vec<GoalSummary> {
        let mut summaries = Vec::new();
        for (goal, entry) in &self.goals {
            let local = &entry.local;
            let members: BTreeSet<_> = local.joins.keys().chain(local.part.keys()).collect();
            for member in members
                .into_iter()
                .filter(|member| principal.is_none_or(|own| own == **member))
            {
                let Some(membership) = entry.membership(member) else {
                    continue;
                };
                summaries.push(GoalSummary {
                    goal: *goal,
                    title: self.title(entry),
                    member: *member,
                    membership,
                    halted: entry.halted(),
                });
            }
        }
        summaries
    }

    /// Whether the daemon can serve one content object of a goal.
    pub(super) fn blob_state(&self, entry: &Entry, hash: &BlobHash) -> BlobState {
        self.content_state(entry, hash)
            .unwrap_or(BlobState::Unavailable)
    }

    /// One task as the board lists it for `reader`.
    pub(super) fn task_view(
        &self,
        entry: &Entry,
        task: &Task,
        reader: Option<&PublicKey>,
    ) -> TaskView {
        let state = entry.state();
        let title = entry
            .goal
            .event(&task.id)
            .and_then(|event| entry.text(&self.store, event))
            .map(|text| text.lines().next().unwrap_or_default().to_owned());
        let integrated = reader
            .and_then(|reader| entry.local.workspace.get(reader)?.integrated)
            .and_then(|head| state.head_position(&head));
        let accepted_at = task.accepted_head.and_then(|_| {
            state
                .accepted_heads
                .iter()
                .rposition(|accepted| accepted.task == task.id)
        });
        TaskView {
            task: task.id,
            state: task_state(task.state),
            title,
            proposer: task.proposer,
            assignee: task.assignee,
            assignment: task.assignment,
            attempt: task.attempt,
            result: task.result,
            applied: matches!((integrated, accepted_at), (Some(mine), Some(theirs)) if mine >= theirs),
        }
    }

    pub(super) fn task_detail(
        &self,
        entry: &Entry,
        task: &Task,
        reader: Option<&PublicKey>,
    ) -> TaskDetail {
        TaskDetail {
            view: self.task_view(entry, task, reader),
            text: entry
                .goal
                .event(&task.id)
                .and_then(|event| entry.text(&self.store, event)),
            input: task.input,
            depends_on: task.depends_on.clone(),
            deadline_ms: task.deadline_ms,
            max_attempts: task.max_attempts,
            cancel: task.cancel,
        }
    }

    /// One event in full.
    pub(super) fn event_detail(&self, entry: &Entry, event: &Event) -> EventDetail {
        let header = event.header();
        EventDetail {
            view: entry.event_view(event),
            anchor: header.anchor,
            body: header.body.clone(),
            payload: header.payload,
            text: entry.text(&self.store, event),
            task: entry.goal.task_of(&event.id()),
            content: header
                .blobs()
                .into_iter()
                .map(|hash| BlobStatus {
                    hash,
                    state: self.blob_state(entry, &hash),
                })
                .collect(),
        }
    }

    /// What needs the caller in one goal now.
    pub(super) fn pending_work(&self, entry: &Entry, actor: &Actor) -> PendingWork {
        let state = entry.state();
        let goal = entry.id();
        let mut work = PendingWork {
            revision: entry.revision(),
            ..PendingWork::default()
        };
        let taking_part = |principal: &PublicKey| {
            entry.membership(principal) == Some(Membership::Member)
                && self.principals.active(principal).is_some()
        };
        for task in &state.tasks {
            let (Some(assignment), Some(assignee)) = (task.assignment, task.assignee) else {
                continue;
            };
            if state
                .assignment(&assignment)
                .is_some_and(|assignment| assignment.revoked)
            {
                continue;
            }
            let item = AssignmentRef {
                task: task.id,
                assignment,
            };
            let claim = entry.claims.get(&assignment);
            let unclaimed = claim.is_none() && task.state == TaskState::Assigned;
            let Some(principal) = actor.principal else {
                // The owner asking directly: what waits for an authorization.
                if unclaimed && taking_part(&assignee) && !entry.may_claim(&assignee, &assignment) {
                    work.to_authorize.push(item);
                }
                continue;
            };
            if !taking_part(&principal) {
                continue;
            }
            if principal == assignee {
                if unclaimed {
                    if entry.may_claim(&assignee, &assignment) {
                        work.to_claim.push(item);
                    } else {
                        work.to_authorize.push(item);
                    }
                }
                let mine = claim.filter(|claim| Some(claim.instance) == actor.session);
                if let Some(claim) = claim.filter(|_| unfinished(task.state)) {
                    let view = claim.view(goal, assignment);
                    if mine.is_some() {
                        work.claimed.push(view);
                    } else {
                        work.held_elsewhere.push(view);
                    }
                }
                if let Some(cancel) = task.cancel
                    && (claim.is_none() || mine.is_some())
                {
                    work.to_acknowledge.push(CancelItem {
                        task: task.id,
                        assignment,
                        cancel,
                        generation: mine.map(|claim| claim.generation),
                    });
                }
            }
            if state.coordinator == Some(principal)
                && task.state == TaskState::Submitted
                && let Some(result) = task.result
            {
                work.to_review.push(ReviewItem {
                    task: task.id,
                    result,
                });
            }
        }
        work
    }
}
