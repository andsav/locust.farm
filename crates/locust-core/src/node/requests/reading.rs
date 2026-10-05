//! Reading a goal: the board, one task, one event, pending work, the feed
//! and waiting for a change.

use locust_proto::api::{ApiError, ErrorCode, MAX_FEED_PAGE, Response, ResponseFrame, WaitOutcome};
use locust_proto::engine::{ConnId, Entropy, Parked, Step};
use locust_proto::event::{Scope, TaskId};
use locust_proto::id::{EventId, GoalId};
use locust_proto::store::Store;

use super::{Plan, answer};
use crate::node::Node;
use crate::node::access::not_found;
use crate::node::callers::{Actor, ParkedWait};

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn board(&self, actor: &Actor, goal: GoalId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let reader = actor.principal.as_ref();
        let mut tasks = entry.state().tasks.clone();
        for selection in entry.state().selections.values() {
            if let Scope::Task(id) = selection.context.scope
                && !tasks.contains_key(&id)
                && let Some(task) = entry.goal.selected_task(id)
            {
                tasks.insert(id, task);
            }
        }
        answer(Response::Board(
            tasks
                .values()
                .map(|task| self.task_view(entry, task, reader))
                .collect(),
        ))
    }

    pub(super) fn task_show(&self, actor: &Actor, goal: GoalId, task: TaskId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let selected = entry.goal.selected_task(task);
        let found = entry
            .state()
            .tasks
            .get(&task)
            .or(selected.as_ref())
            .ok_or_else(|| not_found("no such task"))?;
        answer(Response::Task(self.task_detail(
            entry,
            found,
            actor.principal.as_ref(),
        )))
    }

    pub(super) fn contributions(&self, actor: &Actor, goal: GoalId, task: Option<TaskId>) -> Plan {
        let entry = self.readable(actor, &goal)?;
        // A selection pins its exact proof branch in that scope. Keep the
        // selected view readable even when a later fork makes the same event
        // pending in the ordinary contribution projection.
        let mut subjects = entry
            .state()
            .contributions
            .iter()
            .map(|(id, contribution)| (*id, (contribution, false)))
            .collect::<std::collections::BTreeMap<_, _>>();
        for selection in entry.state().selections.values() {
            if let crate::goal::state::SelectedSubject::Contribution(contribution) =
                &selection.subject
            {
                subjects.insert(contribution.id, (contribution, true));
            }
        }
        let contributions = subjects
            .values()
            .filter(|(contribution, _)| {
                task.is_none_or(|task| contribution.context.scope == Scope::Task(task))
            })
            .map(
                |(contribution, selected)| locust_proto::api::ContributionView {
                    contribution: contribution.id,
                    sources: contribution.sources.clone(),
                    author: contribution.author,
                    context: contribution.context,
                    attempt: contribution.attempt,
                    approved: contribution.approved,
                    selected: *selected,
                    evidence: contribution.evidence.iter().copied().collect(),
                    artifacts: contribution.artifacts.clone(),
                    text: entry
                        .goal
                        .event(&contribution.id)
                        .and_then(|event| entry.text(&self.store, event, actor.principal.as_ref())),
                },
            )
            .collect();
        answer(Response::Contributions(contributions))
    }

    pub(super) fn contribution_inspect(
        &self,
        actor: &Actor,
        goal: GoalId,
        contribution: EventId,
    ) -> Plan {
        use locust_proto::api::{ContributionInspection, ContributionSource};
        use locust_proto::event::Body;
        let entry = self.readable(actor, &goal)?;
        let found = entry
            .goal
            .event(&contribution)
            .ok_or_else(|| not_found("no such contribution"))?;
        let Body::ContributionPublished {
            context,
            attempt,
            sources,
            ..
        } = &found.header().body
        else {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "event is not a contribution",
            ));
        };
        let reference = |event| ContributionSource {
            event,
            detail: entry
                .goal
                .event(&event)
                .map(|found| self.event_detail(entry, found, actor.principal.as_ref())),
        };
        answer(Response::ContributionInspected(Box::new(
            ContributionInspection {
                contribution: self.event_detail(entry, found, actor.principal.as_ref()),
                declared_sources: sources.iter().copied().map(reference).collect(),
                attempt: attempt.map(reference),
                task_round: matches!(context.scope, Scope::Task(_))
                    .then(|| reference(context.round)),
            },
        )))
    }

    pub(super) fn event_show(&self, actor: &Actor, goal: GoalId, event: EventId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let found = entry
            .goal
            .event(&event)
            .ok_or_else(|| not_found("no such event"))?;
        answer(Response::Event(Box::new(self.event_detail(
            entry,
            found,
            actor.principal.as_ref(),
        ))))
    }

    pub(super) fn pending(&self, actor: &Actor, goal: GoalId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        answer(Response::Pending(self.pending_work(entry, actor)))
    }

    /// `events`: an observational feed read. None starts at the beginning;
    /// a continuation never acknowledges content for any session.
    pub(super) fn events(
        &self,
        actor: &Actor,
        goal: GoalId,
        after: Option<u64>,
        limit: u32,
    ) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let from = after.unwrap_or(0);
        let page = limit.min(MAX_FEED_PAGE) as usize;
        let views = entry
            .feed
            .after(from, page)
            .filter_map(|(position, id)| {
                entry.goal.event(id).map(|event| {
                    let mut view = entry.event_view(event);
                    view.position = Some(position);
                    view
                })
            })
            .collect();
        answer(Response::Events(views))
    }

    /// How a wait that saw no change ends when its time is up.
    fn quiet(&self, goal: &GoalId) -> WaitOutcome {
        if self.reachable(goal) {
            WaitOutcome::NoEvent
        } else {
            WaitOutcome::Disconnected
        }
    }

    /// `wait`: answers as soon as the goal's revision is past `seen`;
    /// otherwise parks until it is or the timeout passes. The revision never
    /// falls, so a `seen` ahead of it is no revision this daemon returned.
    /// It is refused: taken as a change, it would answer every call at once.
    pub(super) fn wait(
        &mut self,
        conn: ConnId,
        request_id: u64,
        actor: Actor,
        goal: GoalId,
        seen: u64,
        timeout_ms: u32,
    ) -> Result<Step, ApiError> {
        let entry = self.readable(&actor, &goal)?;
        let revision = entry.revision();
        if seen > revision {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                format!(
                    "seen {seen} is ahead of the goal's revision {revision}; \
                     pass a revision this daemon returned"
                ),
            ));
        }
        let outcome = if revision != seen {
            WaitOutcome::Work(Box::new(self.pending_work(entry, &actor)))
        } else if timeout_ms == 0 {
            self.quiet(&goal)
        } else {
            if let Some(state) = self.conns.get_mut(&conn) {
                state.parked = Some(ParkedWait {
                    request_id,
                    goal,
                    seen,
                    actor,
                });
            }
            return Ok(Step::Park(Parked {
                request_id,
                goal,
                timeout_ms,
            }));
        };
        Ok(Step::Reply(ResponseFrame {
            id: request_id,
            result: Ok(Response::Waited(outcome)),
        }))
    }

    /// Revisits a parked wait: answers when the goal's revision moved or the
    /// time is up, and parks it again otherwise.
    pub(in crate::node) fn resume_wait(
        &mut self,
        conn: ConnId,
        parked: &Parked,
        timed_out: bool,
        _now_ms: u64,
    ) -> Step {
        let reply = |result| {
            Step::Reply(ResponseFrame {
                id: parked.request_id,
                result,
            })
        };
        let waiting = self
            .conns
            .get(&conn)
            .and_then(|state| state.parked)
            .filter(|waiting| waiting.request_id == parked.request_id);
        let Some(waiting) = waiting else {
            return reply(Err(ApiError::new(
                ErrorCode::Invalid,
                "the connection has no such parked wait",
            )));
        };
        let access = if self.failed {
            Err(ApiError::new(
                ErrorCode::Internal,
                "storage failed earlier; restart the daemon",
            ))
        } else if waiting
            .actor
            .principal
            .is_some_and(|principal| self.principals.active(&principal).is_none())
        {
            Err(ApiError::new(
                ErrorCode::Denied,
                "the credential was revoked",
            ))
        } else {
            self.readable(&waiting.actor, &waiting.goal)
        };
        let outcome = match access {
            Err(error) => Err(error),
            Ok(entry) if entry.revision() != waiting.seen => Ok(WaitOutcome::Work(Box::new(
                self.pending_work(entry, &waiting.actor),
            ))),
            Ok(_) if timed_out => Ok(self.quiet(&waiting.goal)),
            Ok(_) => return Step::Park(*parked),
        };
        if let Some(state) = self.conns.get_mut(&conn) {
            state.parked = None;
        }
        reply(outcome.map(Response::Waited))
    }
}
