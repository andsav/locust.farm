//! Local session fencing is independent of replicated work eligibility.
use super::tasks::{recorded, task_context};
use super::{Plan, Planned, answer};
use crate::node::Node;
use crate::node::access::{authorization_required, conflict, denied, not_found};
use crate::node::callers::Actor;
use crate::node::commit::Tx;
use crate::node::entry::Entry;
use crate::node::sessions::{ClaimRecord, claim_write};
use locust_proto::api::{ApiError, ErrorCode, Response};
use locust_proto::engine::Entropy;
use locust_proto::event::{
    AttemptStatus, Body, CancelOutcome, DecisionPurpose, Scope, ScopeKey, TaskId,
};
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey};
use locust_proto::store::Store;

fn superseded(message: &'static str) -> ApiError {
    ApiError::new(ErrorCode::Superseded, message)
}

pub(super) fn active_attempt(
    entry: &Entry,
    principal: PublicKey,
    attempt: EventId,
) -> Result<&crate::goal::state::Attempt, ApiError> {
    let found = entry
        .state()
        .attempts
        .get(&attempt)
        .ok_or_else(|| not_found("no such attempt"))?;
    if found.author != principal {
        return Err(denied("the attempt belongs to another principal"));
    }
    let Scope::Task(task) = found.context.scope else {
        return Err(conflict("the attempt has no task"));
    };
    if task_context(entry, task)? != found.context {
        return Err(superseded("the attempt belongs to an earlier task round"));
    }
    if !matches!(found.status, None | Some(AttemptStatus::Progress)) {
        return Err(conflict("the attempt has ended"));
    }
    Ok(found)
}

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn attempt_start(
        &self,
        actor: &Actor,
        goal: GoalId,
        task: TaskId,
        offer: Option<EventId>,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        let instance = actor.session()?;
        let context = task_context(entry, task)?;
        // Retrying on the same session recovers its durable claim. Independent
        // attempts by other sessions remain independent shared facts.
        for (attempt, claim) in &entry.claims {
            if claim.instance == instance
                && claim.principal == principal
                && let Some(found) = entry.state().attempts.get(attempt)
                && found.context == context
                && found.offer == offer
                && matches!(found.status, None | Some(AttemptStatus::Progress))
            {
                return answer(Response::Claimed(claim.view(goal, *attempt)));
            }
        }
        if !actor.owner_act && !entry.may_start(&principal, context) {
            return Err(authorization_required(
                "the owner must authorize local execution",
            ));
        }
        if !entry
            .goal
            .can_start(context, principal, offer, &entry.definitions)
        {
            return Err(denied("the pinned rules do not permit this attempt"));
        }
        let mut tx = Tx::none();
        tx.commit
            .local
            .extend(self.sessions.bind(&instance, &principal)?);
        let closure = entry
            .state()
            .decisions
            .get(&ScopeKey {
                context,
                purpose: DecisionPurpose::Closure,
            })
            .and_then(|decisions| decisions.last())
            .map(|decision| decision.id);
        let attempt = self.author(
            entry,
            &principal,
            Body::AttemptStarted {
                context,
                offer,
                closure,
            },
            None,
            now,
            &mut tx,
        )?;
        let claim = ClaimRecord {
            task,
            instance,
            generation: 1,
            principal,
        };
        tx.local(claim_write(&goal, &attempt, &claim));
        Ok(Planned {
            response: Response::Claimed(claim.view(goal, attempt)),
            tx,
        })
    }
    pub(super) fn attempt_takeover(&self, actor: &Actor, goal: GoalId, attempt: EventId) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        let instance = actor.session()?;
        let found = active_attempt(entry, principal, attempt)?;
        let claim = entry
            .claims
            .get(&attempt)
            .ok_or_else(|| conflict("no local session holds this attempt"))?;
        if claim.instance == instance {
            return answer(Response::Claimed(claim.view(goal, attempt)));
        }
        let granted = entry.local.grants(&principal).takeover
            || entry
                .local
                .authorized
                .get(&(found.context.round, principal))
                .is_some_and(|authorization| authorization.takeover);
        if !actor.owner_act && !granted {
            return Err(authorization_required("the owner must authorize takeover"));
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
        tx.local(claim_write(&goal, &attempt, &taken)).touch(goal);
        Ok(Planned {
            response: Response::Claimed(taken.view(goal, attempt)),
            tx,
        })
    }
    fn check_claim(
        &self,
        entry: &Entry,
        actor: &Actor,
        principal: PublicKey,
        attempt: EventId,
        generation: u32,
    ) -> Result<(), ApiError> {
        let found = active_attempt(entry, principal, attempt)?;
        if !entry.claims.get(&attempt).is_some_and(|claim| {
            Some(claim.instance) == actor.session
                && claim.principal == principal
                && claim.generation == generation
        }) {
            return Err(superseded(
                "the session does not hold this claim generation",
            ));
        }
        if !found.cancellations.is_empty() {
            return Err(conflict(
                "answer the cancellation before reporting more work",
            ));
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)] // Mirrors the typed API request fields.
    pub(super) fn attempt_report(
        &self,
        actor: &Actor,
        goal: GoalId,
        attempt: EventId,
        generation: u32,
        status: AttemptStatus,
        text: String,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        self.check_claim(entry, actor, principal, attempt, generation)?;
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::AttemptReported { attempt, status },
            Some(&text),
            now,
            &mut tx,
        )?;
        recorded(event, tx)
    }
    #[allow(clippy::too_many_arguments)] // Mirrors the typed API request fields.
    pub(super) fn contribution_publish(
        &self,
        actor: &Actor,
        goal: GoalId,
        task: Option<TaskId>,
        attempt: Option<EventId>,
        generation: Option<u32>,
        summary: String,
        sources: Vec<EventId>,
        base: Option<BlobHash>,
        patch: Option<BlobHash>,
        artifacts: Vec<BlobHash>,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        self.require_grant(actor, entry, entry.local.grants(&principal).contribute)?;
        let context = if let Some(task) = task {
            task_context(entry, task)?
        } else {
            entry
                .goal
                .current_context(Scope::Goal)
                .ok_or_else(|| conflict("no current rules binding"))?
        };
        match (attempt, generation) {
            (Some(attempt), Some(generation)) => {
                self.check_claim(entry, actor, principal, attempt, generation)?;
                if entry.state().attempts[&attempt].context != context {
                    return Err(conflict("the attempt concerns a different task round"));
                }
            }
            (None, None) => (),
            _ => {
                return Err(ApiError::new(
                    ErrorCode::Invalid,
                    "attempt and generation must be supplied together",
                ));
            }
        }
        // Sources are signed attribution, not authority or causal parents.
        // Local authors can cite any held event here, including historical or
        // excluded evidence; another replica need not hold it for this result
        // to remain eligible under its ordinary completion rule.
        for source in &sources {
            if entry.goal.event(source).is_none() {
                return Err(not_found("a declared source is not held in this goal"));
            }
        }
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::ContributionPublished {
                context,
                attempt,
                sources,
                base,
                patch,
                artifacts,
            },
            Some(&summary),
            now,
            &mut tx,
        )?;
        recorded(event, tx)
    }
    pub(super) fn cancel_acknowledge(
        &self,
        actor: &Actor,
        goal: GoalId,
        cancel: EventId,
        generation: Option<u32>,
        outcome: CancelOutcome,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        let found = entry
            .state()
            .cancellations
            .get(&cancel)
            .ok_or_else(|| not_found("no such cancellation request"))?;
        let attempt = entry
            .state()
            .attempts
            .get(&found.attempt)
            .ok_or_else(|| not_found("no such attempt"))?;
        if attempt.author != principal {
            return Err(denied("the attempt belongs to another principal"));
        }
        let allowed = match entry.claims.get(&found.attempt) {
            Some(claim) => {
                Some(claim.instance) == actor.session && generation == Some(claim.generation)
            }
            None => generation.is_none(),
        };
        if !allowed {
            return Err(superseded(
                "only the current claim holder can answer this cancellation",
            ));
        }
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::CancelAcknowledged { cancel, outcome },
            None,
            now,
            &mut tx,
        )?;
        recorded(event, tx)
    }
}
