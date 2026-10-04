//! Tasks as their proposer, the coordinator and the owner act on them:
//! proposing, assigning, cancelling, authorizing, and deciding on results.

use locust_proto::api::{ApiError, ErrorCode, Response};
use locust_proto::engine::Entropy;
use locust_proto::event::Body;
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey};
use locust_proto::store::Store;

use super::{Plan, Planned};
use crate::goal::Submission;
use crate::node::Node;
use crate::node::access::{conflict, not_found};
use crate::node::callers::Actor;
use crate::node::commit::Tx;
use crate::node::entry::Entry;
use crate::node::local::{self, Authorization};

/// The answer of a request that signed `event`.
pub(super) fn recorded(event: EventId, tx: Tx) -> Plan {
    Ok(Planned {
        response: Response::Recorded { event },
        tx,
    })
}

/// A result the coordinator may still decide on: the latest one of its
/// task's current assignment, not cancelled and not yet accepted.
fn undecided<'a>(entry: &'a Entry, result: &EventId) -> Result<&'a Submission, ApiError> {
    let state = entry.state();
    let submission = state
        .result(result)
        .ok_or_else(|| not_found("no such result"))?;
    let task = state
        .task(&submission.task)
        .ok_or_else(|| not_found("no such task"))?;
    let assignment = state
        .assignment(&submission.assignment)
        .ok_or_else(|| not_found("no such assignment"))?;
    if submission.verdict.is_some() {
        return Err(conflict("the result was already decided"));
    }
    if assignment.revoked {
        return Err(conflict("the assignment was revoked"));
    }
    if task.accepted.is_some() {
        return Err(conflict("the task already has an accepted result"));
    }
    if task.assignment != Some(assignment.id) {
        return Err(conflict("the result's assignment was superseded"));
    }
    if assignment.result != Some(*result) {
        return Err(conflict("a later result replaced this one"));
    }
    if assignment.cancel.is_some() {
        return Err(conflict("the result's assignment was cancelled"));
    }
    Ok(submission)
}

impl<S: Store, E: Entropy> Node<S, E> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn task_propose(
        &self,
        actor: &Actor,
        goal: GoalId,
        text: String,
        input: Option<BlobHash>,
        depends_on: Vec<EventId>,
        deadline_ms: Option<u64>,
        max_attempts: Option<u32>,
        now_ms: u64,
    ) -> Plan {
        let (entry, proposer) = self.member(actor, &goal)?;
        let body = Body::TaskProposed {
            input,
            depends_on,
            deadline_ms,
            max_attempts,
        };
        let mut tx = Tx::none();
        let event = self.author(entry, &proposer, body, Some(&text), now_ms, &mut tx)?;
        recorded(event, tx)
    }

    pub(super) fn task_assign(
        &self,
        actor: &Actor,
        goal: GoalId,
        task: EventId,
        assignee: PublicKey,
        now_ms: u64,
    ) -> Plan {
        let (entry, coordinator) = self.coordinator(actor, &goal)?;
        let found = entry
            .state()
            .task(&task)
            .ok_or_else(|| not_found("no such task"))?;
        if !entry.is_member(&assignee) {
            return Err(conflict("the assignee is not a member of the goal"));
        }
        if found.accepted.is_some() {
            return Err(conflict("the task already has an accepted result"));
        }
        let attempt = found
            .attempt
            .checked_add(1)
            .ok_or_else(|| conflict("the attempt number is exhausted"))?;
        if found.max_attempts.is_some_and(|budget| attempt > budget) {
            return Err(conflict("the task's attempt budget is used up"));
        }
        let body = Body::TaskAssigned {
            task,
            assignee,
            attempt,
        };
        let mut tx = Tx::none();
        let event = self.author(entry, &coordinator, body, None, now_ms, &mut tx)?;
        recorded(event, tx)
    }

    pub(super) fn task_cancel(
        &self,
        actor: &Actor,
        goal: GoalId,
        assignment: EventId,
        now_ms: u64,
    ) -> Plan {
        let (entry, coordinator) = self.coordinator(actor, &goal)?;
        let state = entry.state();
        let found = state
            .assignment(&assignment)
            .ok_or_else(|| not_found("no such assignment"))?;
        if found.cancel.is_some() {
            return Err(conflict("cancellation was already requested"));
        }
        if state
            .task(&found.task)
            .is_some_and(|task| task.accepted.is_some())
        {
            return Err(conflict("the task already has an accepted result"));
        }
        let body = Body::CancelRequested { assignment };
        let mut tx = Tx::none();
        let event = self.author(entry, &coordinator, body, None, now_ms, &mut tx)?;
        recorded(event, tx)
    }

    /// `task.authorize`: the owner lets the assignee claim one assignment
    /// that no grant covers.
    pub(super) fn task_authorize(
        &self,
        actor: &Actor,
        goal: GoalId,
        assignment: EventId,
        takeover: bool,
    ) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let found = entry
            .state()
            .assignment(&assignment)
            .ok_or_else(|| not_found("no such assignment"))?;
        if !self.principals.holds(&found.assignee) {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "the assignment is not for a principal of this daemon",
            ));
        }
        let earlier = entry.local.authorized.get(&assignment);
        let authorization = Authorization {
            takeover: takeover || earlier.is_some_and(|earlier| earlier.takeover),
        };
        let mut tx = Tx::none();
        if earlier != Some(&authorization) {
            tx.local(local::authorization_write(
                &goal,
                &assignment,
                &authorization,
            ))
            .touch(goal);
        }
        Ok(Planned {
            response: Response::Done,
            tx,
        })
    }

    pub(super) fn result_accept(
        &self,
        actor: &Actor,
        goal: GoalId,
        result: EventId,
        head: Option<BlobHash>,
        now_ms: u64,
    ) -> Plan {
        let (entry, coordinator) = self.coordinator(actor, &goal)?;
        let submission = undecided(entry, &result)?;
        let accepted = entry.state().accepted_head();
        if head.is_some() && accepted.is_some() && submission.base != accepted {
            return Err(conflict(
                "the result was not made against the accepted workspace head",
            ));
        }
        let body = Body::ResultAccepted { result, head };
        let mut tx = Tx::none();
        let event = self.author(entry, &coordinator, body, None, now_ms, &mut tx)?;
        recorded(event, tx)
    }

    pub(super) fn result_reject(
        &self,
        actor: &Actor,
        goal: GoalId,
        result: EventId,
        reason: String,
        now_ms: u64,
    ) -> Plan {
        let (entry, coordinator) = self.coordinator(actor, &goal)?;
        undecided(entry, &result)?;
        let body = Body::ResultRejected { result };
        let mut tx = Tx::none();
        let event = self.author(entry, &coordinator, body, Some(&reason), now_ms, &mut tx)?;
        recorded(event, tx)
    }
}
