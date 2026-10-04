//! Reading a goal: the board, one task, one event, pending work, the feed
//! and waiting for a change.

use locust_proto::api::{ApiError, ErrorCode, MAX_FEED_PAGE, Response, ResponseFrame, WaitOutcome};
use locust_proto::engine::{ConnId, Entropy, Parked, Step};
use locust_proto::id::{EventId, GoalId};
use locust_proto::store::{Space, Store};

use super::{Plan, Planned, answer};
use crate::node::access::not_found;
use crate::node::callers::{Actor, ParkedWait};
use crate::node::commit::Tx;
use crate::node::{Node, feed, records};

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn board(&self, actor: &Actor, goal: GoalId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let reader = actor.principal.as_ref();
        answer(Response::Board(
            entry
                .state()
                .tasks
                .iter()
                .map(|task| self.task_view(entry, task, reader))
                .collect(),
        ))
    }

    pub(super) fn task_show(&self, actor: &Actor, goal: GoalId, task: EventId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let found = entry
            .state()
            .task(&task)
            .ok_or_else(|| not_found("no such task"))?;
        answer(Response::Task(self.task_detail(
            entry,
            found,
            actor.principal.as_ref(),
        )))
    }

    pub(super) fn event_show(&self, actor: &Actor, goal: GoalId, event: EventId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let found = entry
            .goal
            .event(&event)
            .ok_or_else(|| not_found("no such event"))?;
        answer(Response::Event(self.event_detail(
            entry,
            found,
            actor.principal.as_ref(),
        )))
    }

    pub(super) fn pending(&self, actor: &Actor, goal: GoalId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        answer(Response::Pending(self.pending_work(entry, actor)))
    }

    /// `events`: the feed after a position. `after: Some(p)` stores `p` as
    /// the reader's cursor; `None` resumes from the stored one. A viewer
    /// stores nothing and has no cursor.
    pub(super) fn events(
        &self,
        actor: &Actor,
        goal: GoalId,
        after: Option<u64>,
        limit: u32,
    ) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let reader = actor.principal.as_ref();
        let mut tx = Tx::none();
        let from = match after {
            Some(position) => {
                if !actor.is_viewer() {
                    tx.local(feed::cursor_write(&goal, reader, position));
                }
                position
            }
            None if actor.is_viewer() => 0,
            None => match self
                .store
                .get(Space::Cursor, &feed::cursor_key(&goal, reader))?
            {
                Some(stored) => records::read(&stored)?,
                None => 0,
            },
        };
        let page = limit.min(MAX_FEED_PAGE) as usize;
        let views = entry
            .feed
            .after(from, page)
            .filter_map(|(_, id)| entry.goal.event(id))
            .map(|event| entry.event_view(event))
            .collect();
        Ok(Planned {
            response: Response::Events(views),
            tx,
        })
    }

    /// How a wait that saw no change ends when its time is up.
    fn quiet(&self, goal: &GoalId) -> WaitOutcome {
        if self.reachable(goal) {
            WaitOutcome::NoEvent
        } else {
            WaitOutcome::Disconnected
        }
    }

    /// `wait`: answers as soon as the goal's revision differs from `seen`;
    /// otherwise parks until it does or the timeout passes.
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
        let outcome = if entry.revision() != seen {
            WaitOutcome::Work(self.pending_work(entry, &actor))
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
            Ok(entry) if entry.revision() != waiting.seen => {
                Ok(WaitOutcome::Work(self.pending_work(entry, &waiting.actor)))
            }
            Ok(_) if timed_out => Ok(self.quiet(&waiting.goal)),
            Ok(_) => return Step::Park(*parked),
        };
        if let Some(state) = self.conns.get_mut(&conn) {
            state.parked = None;
        }
        reply(outcome.map(Response::Waited))
    }
}
