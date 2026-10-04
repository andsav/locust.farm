//! Task bindings, work offers, and scoped decisions. Eligibility is evaluated
//! by the same goal evaluator used for replicated events.
use super::{Plan, Planned};
use crate::node::access::{conflict, not_found};
use crate::node::callers::Actor;
use crate::node::commit::Tx;
use crate::node::entry::Entry;
use crate::node::{Node, local};
use locust_proto::api::{ApiError, Response};
use locust_proto::engine::Entropy;
use locust_proto::event::{
    Body, Context, DecisionAction, ReviewVerdict, Scope, TaskBinding, TaskId,
};
use locust_proto::id::{BlobHash, EffectId, EventId, GoalId, PublicKey};
use locust_proto::store::Store;
use std::collections::BTreeMap;

pub(super) fn recorded(event: EventId, tx: Tx) -> Plan {
    Ok(Planned {
        response: Response::Recorded { event },
        tx,
    })
}

pub(super) fn task_context(entry: &Entry, task: TaskId) -> Result<Context, ApiError> {
    let task = entry
        .state()
        .tasks
        .get(&task)
        .ok_or_else(|| not_found("no such task"))?;
    Ok(Context {
        scope: Scope::Task(task.id),
        round: task.current_round,
    })
}
pub(super) fn subject_context(entry: &Entry, subject: EventId) -> Result<Context, ApiError> {
    entry
        .state()
        .contributions
        .get(&subject)
        .map(|c| c.context)
        .or_else(|| entry.state().revisions.get(&subject).map(|r| r.context))
        .ok_or_else(|| not_found("no such contribution or document revision"))
}

impl<S: Store, E: Entropy> Node<S, E> {
    #[allow(clippy::too_many_arguments)] // Mirrors the typed API request fields.
    pub(super) fn task_open(
        &self,
        actor: &Actor,
        goal: GoalId,
        text: String,
        task_type: Option<String>,
        inputs: BTreeMap<String, BlobHash>,
        parent: Option<TaskId>,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        self.require_grant(actor, entry, entry.local.grants(&principal).contribute)?;
        let parent = parent.map(|task| task_context(entry, task)).transpose()?;
        let rules = if let Some(context) = parent {
            entry
                .state()
                .task_round(context)
                .ok_or_else(|| not_found("no such parent round"))?
                .binding
                .rules
        } else {
            entry
                .state()
                .current_rules
                .ok_or_else(|| conflict("no current rules binding"))?
        };
        let binding = TaskBinding {
            rules,
            task_type,
            inputs,
            parent,
            stage: None,
        };
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::TaskOpened { binding },
            Some(&text),
            now,
            &mut tx,
        )?;
        recorded(event, tx)
    }
    pub(super) fn task_revise(
        &self,
        actor: &Actor,
        goal: GoalId,
        task: TaskId,
        expected_round: EventId,
        task_type: Option<String>,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        self.require_grant(actor, entry, entry.local.grants(&principal).contribute)?;
        let context = task_context(entry, task)?;
        if context.round != expected_round {
            return Err(conflict("the task round changed"));
        }
        let mut binding = entry
            .state()
            .task_round(context)
            .expect("task context")
            .binding
            .clone();
        if binding.parent.is_none() {
            binding.rules = entry
                .state()
                .current_rules
                .ok_or_else(|| conflict("no current rules binding"))?;
        }
        binding.task_type = task_type;
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::TaskRevised {
                task,
                expected_round,
                binding,
            },
            None,
            now,
            &mut tx,
        )?;
        recorded(event, tx)
    }
    pub(super) fn work_offer(
        &self,
        actor: &Actor,
        goal: GoalId,
        task: TaskId,
        recipient: PublicKey,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        self.require_grant(actor, entry, entry.local.grants(&principal).flow)?;
        let context = task_context(entry, task)?;
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::WorkOffered { context, recipient },
            None,
            now,
            &mut tx,
        )?;
        recorded(event, tx)
    }
    pub(super) fn task_authorize(
        &self,
        actor: &Actor,
        goal: GoalId,
        task: TaskId,
        agent: PublicKey,
        takeover: bool,
    ) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let context = task_context(entry, task)?;
        if !entry.is_member(&agent) || self.principals.active(&agent).is_none() {
            return Err(not_found("no active local member has that key"));
        }
        let mut tx = Tx::none();
        tx.local(local::authorization_write(
            &goal,
            &context.round,
            &agent,
            &local::Authorization { takeover },
        ))
        .touch(goal);
        Ok(Planned {
            response: Response::Done,
            tx,
        })
    }
    pub(super) fn work_decline(
        &self,
        actor: &Actor,
        goal: GoalId,
        offer: EventId,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::WorkDeclined { offer },
            None,
            now,
            &mut tx,
        )?;
        recorded(event, tx)
    }
    pub(super) fn attempt_cancel(
        &self,
        actor: &Actor,
        goal: GoalId,
        attempt: EventId,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        self.require_grant(actor, entry, entry.local.grants(&principal).flow)?;
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::CancelRequested { attempt },
            None,
            now,
            &mut tx,
        )?;
        recorded(event, tx)
    }
    pub(super) fn completion_declare(
        &self,
        actor: &Actor,
        goal: GoalId,
        subject: EventId,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        self.require_grant(actor, entry, entry.local.grants(&principal).contribute)?;
        let context = subject_context(entry, subject)?;
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::CompletionDeclared { context, subject },
            None,
            now,
            &mut tx,
        )?;
        recorded(event, tx)
    }
    pub(super) fn review_record(
        &self,
        actor: &Actor,
        goal: GoalId,
        subject: EventId,
        verdict: ReviewVerdict,
        text: String,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        self.require_grant(actor, entry, entry.local.grants(&principal).review)?;
        let context = subject_context(entry, subject)?;
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::ReviewRecorded {
                context,
                subject,
                verdict,
            },
            Some(&text),
            now,
            &mut tx,
        )?;
        recorded(event, tx)
    }
    #[allow(clippy::too_many_arguments)] // Mirrors the typed API request fields.
    pub(super) fn check_attest(
        &self,
        actor: &Actor,
        goal: GoalId,
        subject: EventId,
        name: String,
        passed: bool,
        text: String,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        self.require_grant(actor, entry, entry.local.grants(&principal).review)?;
        let context = subject_context(entry, subject)?;
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::CheckAttested {
                context,
                subject,
                name,
                passed,
            },
            Some(&text),
            now,
            &mut tx,
        )?;
        recorded(event, tx)
    }
    pub(super) fn scope_select(
        &self,
        actor: &Actor,
        goal: GoalId,
        subject: EventId,
        expected: Option<EventId>,
        now: u64,
    ) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let context = subject_context(entry, subject)?;
        let mut evidence = vec![subject];
        if let Some(contribution) = entry.state().contributions.get(&subject) {
            evidence.extend(contribution.evidence.iter().copied());
        }
        if let Some(revision) = entry.state().revisions.get(&subject) {
            evidence.extend(revision.evidence.iter().copied());
        }
        self.scope_decide(
            actor,
            goal,
            context,
            expected,
            DecisionAction::Select { subject },
            evidence,
            now,
        )
    }
    pub(super) fn scope_close(
        &self,
        actor: &Actor,
        goal: GoalId,
        scope: Scope,
        expected: Option<EventId>,
        reopen: bool,
        now: u64,
    ) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let context = entry
            .goal
            .current_context(scope)
            .ok_or_else(|| not_found("no such current scope"))?;
        let evidence = entry
            .state()
            .contributions
            .values()
            .filter(|contribution| contribution.context == context && contribution.approved)
            .flat_map(|contribution| {
                std::iter::once(contribution.id).chain(contribution.evidence.iter().copied())
            })
            .collect();
        self.scope_decide(
            actor,
            goal,
            context,
            expected,
            if reopen {
                DecisionAction::Reopen
            } else {
                DecisionAction::Close
            },
            evidence,
            now,
        )
    }
    #[allow(clippy::too_many_arguments)] // Mirrors the typed API request fields.
    fn scope_decide(
        &self,
        actor: &Actor,
        goal: GoalId,
        context: Context,
        previous: Option<EventId>,
        action: DecisionAction,
        mut evidence: Vec<EventId>,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        self.require_grant(actor, entry, entry.local.grants(&principal).select)?;
        evidence.sort();
        evidence.dedup();
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::ScopeDecided {
                context,
                previous,
                action,
                evidence,
            },
            None,
            now,
            &mut tx,
        )?;
        recorded(event, tx)
    }
    pub(super) fn delivery_acknowledge(
        &self,
        actor: &Actor,
        goal: GoalId,
        effect: EffectId,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::DeliveryAcknowledged { effect },
            None,
            now,
            &mut tx,
        )?;
        recorded(event, tx)
    }
}
