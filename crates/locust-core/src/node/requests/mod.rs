//! Local requests: resolution of the caller, idempotency, and dispatch to the
//! module that owns each family of operations.

mod catalog;
mod claims;
pub(super) mod content;
mod daemon;
mod documents;
mod goals;
pub(super) mod invitations;
mod levels;
mod reading;
mod sessions;
mod tasks;
mod workspace;

use locust_proto::api::{ApiError, ErrorCode, Request, RequestFrame, Response, ResponseFrame};
use locust_proto::engine::{ConnId, Entropy, Step};
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
        // Receipt replay must never bypass its execution-session boundary.
        if let Request::ContextAcknowledge { receipt, .. } = &frame.request
            && (Some(receipt.principal) != actor.principal
                || Some(receipt.session) != actor.session)
        {
            return Err(ApiError::new(
                ErrorCode::Denied,
                "context receipt belongs to another principal or session",
            ));
        }
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
            !frame.request.is_read_only() && !matches!(frame.request, Request::Shutdown)
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
                // A note that cannot be written must not replace the refusal
                // itself.
                if error.code == ErrorCode::LevelRequired
                    && let Some(refused) = error.refused()
                    && let (Some(goal), Some(task)) = (refused.goal, refused.task)
                {
                    let _ = self.note_task_want(goal, task, refused.agent, now_ms);
                }
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
            request @ (Request::FarmOn { .. }
            | Request::FarmOff { .. }
            | Request::FarmShow { .. }
            | Request::FarmStatus
            | Request::FarmConsent { .. }) => self.farm_request(actor, request, now),
            Request::Context {
                goal,
                view,
                task,
                after,
                limit,
                preview_chars,
                unread_only,
            } => self.context_read(
                actor,
                goal,
                task,
                after,
                limit,
                preview_chars,
                unread_only,
                view,
            ),
            Request::ContextAcknowledge { goal, receipt } => {
                self.context_acknowledge(actor, goal, receipt)
            }
            Request::InvitationInspect { ticket } => self.invitation_inspect(ticket, now),
            Request::GoalInvitations { goal } => self.goal_invitations(actor, goal, now),
            Request::InvitationRevoke { goal, invitation } => {
                self.invitation_revoke(actor, goal, invitation, now)
            }
            Request::LevelSet { goal, agent, level } => self.level_set(actor, goal, agent, level),
            Request::TaskAllow { goal, agent, task } => self.task_allow(actor, goal, agent, task),
            Request::TaskDisallow { goal, agent, task } => {
                self.task_disallow(actor, goal, agent, task)
            }
            Request::AgentRevoke { agent } => self.agent_revoke(agent),
            Request::AgentReconnect { agent } => self.agent_reconnect(agent),
            Request::AuthorEnroll { name, credential } => self.author_enroll(name, credential),
            Request::SessionReport { record } => self.session_report(actor, record, now),
            Request::Session { instance } => self.session_show(actor, instance),
            Request::Sessions => self.sessions_list(actor),
            Request::SessionDrop { instance } => self.session_drop(actor, instance),
            Request::GoalLeave { goal, agent } => self.goal_leave(actor, goal, agent, now),
            Request::GoalContinue { goal } => self.goal_continue(goal),
            Request::MemberRemove { goal, member } => self.member_remove(actor, goal, member, now),
            Request::GoalJoin {
                agent,
                name,
                ticket,
                level,
            } => self.goal_join(actor, agent, name, ticket, level, now),
            Request::GoalInvite {
                goal,
                expires_ms,
                role,
            } => self.goal_invite(actor, goal, expires_ms, role, now),
            Request::RoleGive {
                goal,
                role,
                member,
                expected,
            } => self.role_change(actor, goal, role, member, expected, true, now),
            Request::RoleTake {
                goal,
                role,
                member,
                expected,
            } => self.role_change(actor, goal, role, member, expected, false, now),
            Request::BlobPut { goal, bytes } => self.blob_put(actor, goal, bytes),
            Request::BlobGet { goal, hash } => self.blob_get(actor, goal, hash),
            Request::BlobStat { goal, hashes } => self.blob_stat(actor, goal, hashes),
            Request::BlobWithdraw { goal, hash } => self.blob_withdraw(actor, goal, hash),
            Request::Status => self.status(actor, now),
            Request::Shutdown => self.shutdown(),
            Request::AgentEnroll { name, credential } => self.agent_enroll(name, credential),
            Request::GoalCreate {
                agent,
                title,
                formation_json,
                name,
                inputs,
            } => self.goal_create(
                actor,
                goals::GoalCreateInput {
                    agent,
                    title,
                    formation_json,
                    name,
                    inputs,
                },
                now,
            ),
            Request::GoalStatus { goal } => self.goal_status(actor, goal),
            Request::RulesBind {
                goal,
                expected,
                formation_json,
                inputs,
                no_role,
            } => self.rules_bind(actor, goal, expected, formation_json, inputs, no_role, now),
            Request::WorkspaceEpochSet {
                goal,
                expected_epoch,
                rules,
                checkpoint,
            } => self.workspace_epoch_set(actor, goal, expected_epoch, rules, checkpoint, now),
            Request::WorkspaceTree { goal, revision } => self.workspace_tree(actor, goal, revision),
            Request::WorkspaceRead {
                goal,
                revision,
                path,
            } => self.workspace_read(actor, goal, revision, path),
            Request::WorkspaceHead { goal } => self.workspace_head(actor, goal),
            Request::WorkspaceProposal { goal, proposal } => {
                self.workspace_proposal(actor, goal, proposal)
            }
            Request::WorkspaceProposals { goal } => self.workspace_proposals(actor, goal),
            Request::WorkspaceRevision { goal, revision } => {
                self.workspace_revision(actor, goal, revision)
            }
            Request::WorkspacePublish { goal, operation } => {
                self.workspace_submit(actor, goal, operation, false, now)
            }
            Request::WorkspaceIntegrate { goal, operation } => {
                self.workspace_submit(actor, goal, operation, true, now)
            }
            Request::WorkspaceRecoveryParentCheck { goal, parent } => {
                self.workspace_recovery_parent(actor, goal, parent)
            }
            Request::CheckoutRegister {
                goal,
                checkout,
                revision,
                task,
                attempt,
            } => self.checkout_register(actor, goal, checkout, revision, task, attempt),
            Request::WorkspaceConnect {
                goal,
                agent,
                checkout,
            } => self.workspace_connect(actor, goal, agent, checkout),
            Request::CheckoutBindSession { goal, checkout } => {
                self.checkout_bind_session(actor, goal, checkout)
            }
            Request::Checkouts { goal } => self.checkouts(actor, goal),
            Request::WorkspaceOperationPrepare { goal, operation } => {
                self.workspace_operation_prepare(actor, goal, operation)
            }
            Request::WorkspaceOperationShow { goal, operation } => {
                self.workspace_operation_show(actor, goal, operation)
            }
            Request::WorkspaceOperations { goal } => self.workspace_operations(actor, goal),
            Request::WorkspaceOperationComplete { goal, operation } => {
                self.workspace_operation_complete(actor, goal, operation)
            }
            Request::Board { goal } => self.board(actor, goal),
            Request::Task { goal, task } => self.task_show(actor, goal, task),
            Request::Event { goal, event } => self.event_show(actor, goal, event),
            Request::TaskOpen {
                goal,
                text,
                task_type,
                inputs,
                parent,
            } => self.task_open(actor, goal, text, task_type, inputs, parent, now),
            Request::TaskRevise {
                goal,
                task,
                expected_round,
                task_type,
            } => self.task_revise(actor, goal, task, expected_round, task_type, now),
            Request::WorkOffer {
                goal,
                task,
                recipient,
            } => self.work_offer(actor, goal, task, recipient, now),
            Request::AttemptStart { goal, task, offer } => {
                self.attempt_start(actor, goal, task, offer, now)
            }
            Request::AttemptTakeover { goal, attempt } => {
                self.attempt_takeover(actor, goal, attempt)
            }
            Request::WorkDecline { goal, offer } => self.work_decline(actor, goal, offer, now),
            Request::AttemptCancel { goal, attempt } => {
                self.attempt_cancel(actor, goal, attempt, now)
            }
            Request::AttemptReport {
                goal,
                attempt,
                generation,
                status,
                text,
            } => self.attempt_report(actor, goal, attempt, generation, status, text, now),
            Request::ContributionPublish {
                goal,
                attempt,
                generation,
                summary,
                sources,
                artifacts,
            } => self.contribution_publish(
                actor, goal, attempt, generation, summary, sources, artifacts, now,
            ),
            Request::Contributions { goal, task } => self.contributions(actor, goal, task),
            Request::ContributionInspect { goal, contribution } => {
                self.contribution_inspect(actor, goal, contribution)
            }
            Request::CompletionDeclare { goal, subject } => {
                self.completion_declare(actor, goal, subject, now)
            }
            Request::ReviewRecord {
                goal,
                subject,
                verdict,
                text,
            } => self.review_record(actor, goal, subject, verdict, text, now),
            Request::CheckAttest {
                goal,
                subject,
                name,
                passed,
                text,
            } => self.check_attest(actor, goal, subject, name, passed, text, now),
            Request::ScopeSelect {
                goal,
                subject,
                expected,
            } => self.scope_select(actor, goal, subject, expected, now),
            Request::ScopeClose {
                goal,
                scope,
                expected,
            } => self.scope_close(actor, goal, scope, expected, false, now),
            Request::ScopeReopen {
                goal,
                scope,
                expected,
            } => self.scope_close(actor, goal, scope, expected, true, now),
            Request::DeliveryAcknowledge { goal, effect } => {
                self.delivery_acknowledge(actor, goal, effect, now)
            }
            Request::CancelAcknowledge {
                goal,
                cancel,
                generation,
                outcome,
            } => self.cancel_acknowledge(actor, goal, cancel, generation, outcome, now),
            Request::Pending { goal } => self.pending(actor, goal),
            Request::PendingPage {
                goal,
                kind,
                after,
                limit,
            } => self.pending_page(actor, goal, kind, after, limit),
            Request::Events { goal, after, limit } => self.events(actor, goal, after, limit),
            Request::DocRead { goal, doc } => self.doc_read(actor, goal, doc),
            Request::DocRevise {
                goal,
                doc,
                base,
                text,
            } => self.doc_revise(actor, goal, doc, base, text, now),
            Request::Wait { .. } => unreachable!("wait is dispatched before planning"),
            request @ (Request::FormationDraftCreate { .. }
            | Request::FormationDraftUpdate { .. }
            | Request::FormationDraft { .. }
            | Request::FormationDrafts
            | Request::FormationPublish { .. }
            | Request::FormationPublication { .. }
            | Request::FormationPublications
            | Request::FormationPresentation { .. }
            | Request::FormationPresentationUpdate { .. }
            | Request::FormationValidate { .. }
            | Request::FormationExplain { .. }) => self.catalog(actor, request),
        }
    }
}
