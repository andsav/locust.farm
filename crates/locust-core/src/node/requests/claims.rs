//! Tasks as their assignee acts on them: claiming, taking over, declining,
//! reporting, submitting, failing and answering a cancellation.
//!
//! A claim binds an assignment to the session that took it and carries a
//! generation. Claim-bound writes name the generation the session was given;
//! a takeover raises it, so the earlier holder's delayed writes are refused
//! as `Superseded` instead of landing.

use locust_proto::api::{ApiError, ErrorCode, Response};
use locust_proto::engine::Entropy;
use locust_proto::event::{Body, CancelOutcome};
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey};
use locust_proto::store::Store;

use super::tasks::recorded;
use super::{Plan, Planned, answer};
use crate::goal::{Assignment, Task, TaskState};
use crate::node::Node;
use crate::node::access::{authorization_required, conflict, denied, not_found};
use crate::node::callers::Actor;
use crate::node::commit::Tx;
use crate::node::entry::Entry;
use crate::node::sessions::{ClaimRecord, claim_write};

fn superseded(message: &'static str) -> ApiError {
    ApiError::new(ErrorCode::Superseded, message)
}

/// The assignment `id` of `principal`, with its task, while it is the task's
/// current one.
fn current<'a>(
    entry: &'a Entry,
    principal: &PublicKey,
    id: &EventId,
) -> Result<(&'a Assignment, &'a Task), ApiError> {
    let state = entry.state();
    let assignment = state
        .assignment(id)
        .ok_or_else(|| not_found("no such assignment"))?;
    if assignment.assignee != *principal {
        return Err(denied("the assignment is for another principal"));
    }
    let task = state
        .task(&assignment.task)
        .ok_or_else(|| not_found("no such task"))?;
    if assignment.revoked || task.assignment != Some(*id) || task.accepted.is_some() {
        return Err(superseded("the assignment is no longer current"));
    }
    Ok((assignment, task))
}

impl<S: Store, E: Entropy> Node<S, E> {
    /// `task.claim`: the assignee's session takes an assignment, or recovers
    /// the claim it already holds.
    pub(super) fn task_claim(
        &self,
        actor: &Actor,
        goal: GoalId,
        assignment: EventId,
        now_ms: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        let instance = actor.session()?;
        let (found, task) = current(entry, &principal, &assignment)?;
        if let Some(claim) = entry.claims.get(&assignment) {
            return if claim.instance == instance {
                answer(Response::Claimed(claim.view(goal, assignment)))
            } else {
                Err(ApiError::new(
                    ErrorCode::ClaimHeld,
                    "another session holds the claim",
                ))
            };
        }
        if task.state != TaskState::Assigned {
            return Err(conflict("the assignment is not open to be claimed"));
        }
        if !actor.owner_act && !entry.may_claim(&principal, &assignment) {
            return Err(authorization_required(
                "no grant lets the principal take this assignment; the owner authorizes it",
            ));
        }
        let claim = ClaimRecord {
            task: found.task,
            instance,
            generation: 1,
            principal,
        };
        let mut tx = Tx::none();
        tx.commit
            .local
            .extend(self.sessions.bind(&instance, &principal)?);
        tx.local(claim_write(&goal, &assignment, &claim));
        let body = Body::AssignmentAccepted { assignment };
        self.author(entry, &principal, body, None, now_ms, &mut tx)?;
        Ok(Planned {
            response: Response::Claimed(claim.view(goal, assignment)),
            tx,
        })
    }

    /// `task.takeover`: the assignee's session replaces another session's
    /// claim and fences it by raising the generation.
    pub(super) fn task_takeover(&self, actor: &Actor, goal: GoalId, assignment: EventId) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        let instance = actor.session()?;
        current(entry, &principal, &assignment)?;
        let Some(claim) = entry.claims.get(&assignment) else {
            return Err(conflict("nobody holds a claim; use task.claim"));
        };
        if claim.instance == instance {
            return answer(Response::Claimed(claim.view(goal, assignment)));
        }
        let granted = entry.local.grants(&principal).takeover
            || entry
                .local
                .authorized
                .get(&assignment)
                .is_some_and(|authorization| authorization.takeover);
        if !actor.owner_act && !granted {
            return Err(authorization_required(
                "no grant lets the principal take this claim over; the owner authorizes it",
            ));
        }
        let taken = ClaimRecord {
            instance,
            generation: claim
                .generation
                .checked_add(1)
                .ok_or_else(|| conflict("the claim generation is exhausted"))?,
            ..*claim
        };
        let mut tx = Tx::none();
        tx.commit
            .local
            .extend(self.sessions.bind(&instance, &principal)?);
        tx.local(claim_write(&goal, &assignment, &taken))
            .touch(goal);
        Ok(Planned {
            response: Response::Claimed(taken.view(goal, assignment)),
            tx,
        })
    }

    /// `task.decline`: only before the assignment is claimed.
    pub(super) fn task_decline(
        &self,
        actor: &Actor,
        goal: GoalId,
        assignment: EventId,
        now_ms: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        let (_, task) = current(entry, &principal, &assignment)?;
        if entry.claims.contains_key(&assignment) || task.state != TaskState::Assigned {
            return Err(conflict("the assignment was already taken"));
        }
        let body = Body::AssignmentDeclined { assignment };
        let mut tx = Tx::none();
        let event = self.author(entry, &principal, body, None, now_ms, &mut tx)?;
        recorded(event, tx)
    }

    /// A claim-bound write: the connection's session holds the claim at
    /// `generation`, and the attempt is still running.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn claim_bound(
        &self,
        actor: &Actor,
        goal: GoalId,
        assignment: EventId,
        generation: u32,
        body: Body,
        text: &str,
        now_ms: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        let instance = actor.session()?;
        let (found, task) = current(entry, &principal, &assignment)?;
        let holds = entry
            .claims
            .get(&assignment)
            .is_some_and(|claim| claim.instance == instance && claim.generation == generation);
        if !holds {
            return Err(superseded(
                "the session does not hold the claim at that generation",
            ));
        }
        if found.cancel.is_some() {
            return Err(conflict(
                "cancellation was requested; answer it with cancel.acknowledge",
            ));
        }
        if task.state != TaskState::Taken {
            return Err(conflict("the attempt has already ended"));
        }
        let mut tx = Tx::none();
        let event = self.author(entry, &principal, body, Some(text), now_ms, &mut tx)?;
        recorded(event, tx)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn task_submit(
        &self,
        actor: &Actor,
        goal: GoalId,
        assignment: EventId,
        generation: u32,
        summary: String,
        base: Option<BlobHash>,
        patch: Option<BlobHash>,
        artifacts: Vec<BlobHash>,
        now_ms: u64,
    ) -> Plan {
        let body = Body::ResultSubmitted {
            assignment,
            base,
            patch,
            artifacts,
        };
        self.claim_bound(actor, goal, assignment, generation, body, &summary, now_ms)
    }

    /// `cancel.acknowledge`: while a claim exists only its holder answers,
    /// naming its generation; otherwise any connection of the assignee.
    pub(super) fn cancel_acknowledge(
        &self,
        actor: &Actor,
        goal: GoalId,
        cancel: EventId,
        generation: Option<u32>,
        outcome: CancelOutcome,
        now_ms: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        let found = entry
            .state()
            .cancelled(&cancel)
            .ok_or_else(|| not_found("no such cancellation request"))?;
        if found.assignee != principal {
            return Err(denied("the cancelled assignment is for another principal"));
        }
        let allowed = match entry.claims.get(&found.id) {
            Some(claim) => {
                Some(claim.instance) == actor.session && generation == Some(claim.generation)
            }
            None => generation.is_none(),
        };
        if !allowed {
            return Err(superseded(
                "the cancellation is the claim holder's to answer, at its current generation",
            ));
        }
        if found
            .cancel
            .is_some_and(|request| request.outcome.is_some())
        {
            return Err(conflict("the cancellation was already answered"));
        }
        let body = Body::CancelAcknowledged { cancel, outcome };
        let mut tx = Tx::none();
        let event = self.author(entry, &principal, body, None, now_ms, &mut tx)?;
        recorded(event, tx)
    }
}
