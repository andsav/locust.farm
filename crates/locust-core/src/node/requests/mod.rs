//! Local requests: resolution of the caller, idempotency, and dispatch to the
//! module that owns each family of operations.

mod claims;
pub(super) mod content;
mod daemon;
mod goals;
pub(super) mod invitations;
mod notes;
mod reading;
mod sessions;
mod tasks;

use locust_proto::api::{ApiError, ErrorCode, Request, RequestFrame, Response, ResponseFrame};
use locust_proto::engine::{ConnId, Entropy, Step};
use locust_proto::event::Body;
use locust_proto::store::Store;

use super::Node;
use super::callers::{self, Actor};
use super::commit::{Tx, request_digest};

/// A request planned against the node as it is: the answer, and what must be
/// durable before the answer is released.
pub(super) struct Planned {
    pub response: Response,
    pub tx: Tx,
}

pub(super) type Plan = Result<Planned, ApiError>;

/// An answer that changes nothing.
pub(super) fn answer(response: Response) -> Plan {
    Ok(Planned {
        response,
        tx: Tx::none(),
    })
}

impl<S: Store, E: Entropy> Node<S, E> {
    /// Handles one request frame from a welcomed connection.
    pub(super) fn handle(&mut self, conn: ConnId, frame: RequestFrame, now_ms: u64) -> Step {
        let id = frame.id;
        self.respond(conn, frame, now_ms).unwrap_or_else(|error| {
            Step::Reply(ResponseFrame {
                id,
                result: Err(error),
            })
        })
    }

    fn respond(
        &mut self,
        conn: ConnId,
        frame: RequestFrame,
        now_ms: u64,
    ) -> Result<Step, ApiError> {
        if self.failed {
            return Err(ApiError::new(
                ErrorCode::Internal,
                "storage failed earlier; restart the daemon",
            ));
        }
        let Some(state) = self.conns.get(&conn) else {
            return Err(ApiError::new(
                ErrorCode::Denied,
                "the connection has not been welcomed",
            ));
        };
        frame.request.check()?;
        let actor = callers::resolve(&self.principals, state.caller, state.session, &frame)?;
        let id = frame.id;
        if let Request::Wait {
            goal,
            seen,
            timeout_ms,
        } = frame.request
        {
            return self.wait(conn, id, actor, goal, seen, timeout_ms);
        }
        let reply = |response| {
            Ok(Step::Reply(ResponseFrame {
                id,
                result: Ok(response),
            }))
        };

        // A read changes nothing, so running it again is its own idempotency;
        // only requests that write are recorded under their key.
        let key = frame.idempotency.filter(|_| {
            !frame.request.is_read_only()
                && !actor.is_viewer()
                && !matches!(frame.request, Request::Shutdown)
        });
        let digest = key.map(|_| request_digest(frame.on_behalf, &frame.request));
        if let (Some(key), Some(digest)) = (&key, &digest)
            && let Some(response) = self.replayed(actor.caller, key, digest)?
        {
            return reply(response);
        }

        let blob_get = match &frame.request {
            Request::BlobGet { goal, hash } => Some((*goal, *hash)),
            _ => None,
        };
        let Planned { response, mut tx } = match self.plan(&actor, frame.request, now_ms) {
            Ok(plan) => plan,
            Err(error) => {
                if error.code == ErrorCode::Unavailable
                    && let Some((goal, hash)) = blob_get
                {
                    self.note_blob_want(&actor, goal, hash)?;
                }
                return Err(error);
            }
        };
        if let (Some(key), Some(digest)) = (&key, digest) {
            tx.local(Self::remember(actor.caller, key, digest, &response));
        }
        self.land(tx)?;
        reply(response)
    }

    /// Plans one request without changing anything.
    fn plan(&self, actor: &Actor, request: Request, now: u64) -> Plan {
        match request {
            Request::AgentGrant { agent, grants } => self.agent_grant(agent, grants),
            Request::AgentRevoke { agent } => self.agent_revoke(agent),
            Request::ViewerEnroll { agent, credential } => self.viewer_enroll(agent, credential),
            Request::SessionReport { record } => self.session_report(actor, record, now),
            Request::Session { instance } => self.session_show(actor, instance),
            Request::Sessions => self.sessions_list(actor),
            Request::SessionDrop { instance } => self.session_drop(actor, instance),
            Request::GoalLeave { goal } => self.goal_leave(actor, goal, now),
            Request::MemberRemove { goal, member } => self.member_remove(actor, goal, member, now),
            Request::GoalJoin { ticket } => self.goal_join(actor, ticket, now),
            Request::GoalInvite { goal, expires_ms } => {
                self.goal_invite(actor, goal, expires_ms, now)
            }
            Request::BlobPut { goal, bytes } => self.blob_put(actor, goal, bytes),
            Request::BlobGet { goal, hash } => self.blob_get(actor, goal, hash),
            Request::BlobStat { goal, hashes } => self.blob_stat(actor, goal, hashes),
            Request::BlobWithdraw { goal, hash } => self.blob_withdraw(actor, goal, hash),
            Request::Status => self.status(actor),
            Request::Shutdown => self.shutdown(),
            Request::AgentEnroll {
                name,
                grants,
                credential,
            } => self.agent_enroll(name, grants, credential),
            Request::GoalCreate { title } => self.goal_create(actor, title, now),
            Request::GoalGrant {
                goal,
                agent,
                grants,
            } => self.goal_grant(actor, goal, agent, grants),
            Request::GoalStatus { goal } => self.goal_status(actor, goal),
            Request::WorkspaceSet { goal, binding } => self.workspace_set(actor, goal, binding),
            Request::Board { goal } => self.board(actor, goal),
            Request::Task { goal, task } => self.task_show(actor, goal, task),
            Request::Event { goal, event } => self.event_show(actor, goal, event),
            Request::TaskPropose {
                goal,
                text,
                input,
                depends_on,
                deadline_ms,
                max_attempts,
            } => self.task_propose(
                actor,
                goal,
                text,
                input,
                depends_on,
                deadline_ms,
                max_attempts,
                now,
            ),
            Request::TaskAssign {
                goal,
                task,
                assignee,
            } => self.task_assign(actor, goal, task, assignee, now),
            Request::TaskCancel { goal, assignment } => {
                self.task_cancel(actor, goal, assignment, now)
            }
            Request::TaskAuthorize {
                goal,
                assignment,
                takeover,
            } => self.task_authorize(actor, goal, assignment, takeover),
            Request::TaskClaim { goal, assignment } => {
                self.task_claim(actor, goal, assignment, now)
            }
            Request::TaskTakeover { goal, assignment } => {
                self.task_takeover(actor, goal, assignment)
            }
            Request::TaskDecline { goal, assignment } => {
                self.task_decline(actor, goal, assignment, now)
            }
            Request::TaskProgress {
                goal,
                assignment,
                generation,
                text,
            } => {
                let body = Body::Progress { assignment };
                self.claim_bound(actor, goal, assignment, generation, body, &text, now)
            }
            Request::TaskSubmit {
                goal,
                assignment,
                generation,
                summary,
                base,
                patch,
                artifacts,
            } => self.task_submit(
                actor, goal, assignment, generation, summary, base, patch, artifacts, now,
            ),
            Request::TaskFail {
                goal,
                assignment,
                generation,
                reason,
            } => {
                let body = Body::AttemptFailed { assignment };
                self.claim_bound(actor, goal, assignment, generation, body, &reason, now)
            }
            Request::CancelAcknowledge {
                goal,
                cancel,
                generation,
                outcome,
            } => self.cancel_acknowledge(actor, goal, cancel, generation, outcome, now),
            Request::ResultAccept { goal, result, head } => {
                self.result_accept(actor, goal, result, head, now)
            }
            Request::ResultReject {
                goal,
                result,
                reason,
            } => self.result_reject(actor, goal, result, reason, now),
            Request::Pending { goal } => self.pending(actor, goal),
            Request::Events { goal, after, limit } => self.events(actor, goal, after, limit),
            Request::NoteAdd {
                goal,
                about,
                supersedes,
                text,
            } => self.note_add(actor, goal, about, supersedes, text, now),
            Request::Notes { goal, about } => self.notes(actor, goal, about),
            Request::DocRead { goal, doc } => self.doc_read(actor, goal, doc),
            Request::DocRevise {
                goal,
                doc,
                base,
                text,
            } => self.doc_revise(actor, goal, doc, base, text, now),
            Request::DocAccept { goal, revision } => self.doc_accept(actor, goal, revision, now),
            Request::Wait { .. } => unreachable!("wait is dispatched before planning"),
        }
    }
}
