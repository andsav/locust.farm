//! Shared replay authority followed by this person's local level.
//!
//! A principal sees only the goals it takes or took part in; any other goal
//! does not exist for it (`NotFound`). A request that is not the caller's to
//! make is `Denied`. The owner acting on a principal's behalf skips only
//! the local level, never the shared rule.

use locust_proto::api::{Act, ApiError, ErrorCode, Level, Membership, Refused, Voice, Why, render};
use locust_proto::engine::Entropy;
use locust_proto::event::{Body, DecisionAction, Event, Scope, TaskId};
use locust_proto::id::{GoalId, PublicKey};
use locust_proto::store::Store;

use super::Node;
use super::callers::Actor;
use super::commit::Trial;
use super::entry::Entry;
use crate::goal::Standing;

pub(super) fn not_found(message: &'static str) -> ApiError {
    ApiError::new(ErrorCode::NotFound, message)
}

pub(super) fn denied(message: &'static str) -> ApiError {
    ApiError::new(ErrorCode::Denied, message)
}

pub(super) fn conflict(message: &'static str) -> ApiError {
    ApiError::new(ErrorCode::Conflict, message)
}

/// A candidate already signed in memory, or a local content/claim action.
pub(super) enum Attempted<'a> {
    Sign {
        event: &'a Event,
        preceding: &'a [Event],
    },
    Store,
    Withdraw,
    Resume {
        task: TaskId,
    },
}

fn act(body: &Body) -> Act {
    match body {
        Body::TaskOpened { .. } => Act::OpenTask,
        Body::WorkOffered { .. } => Act::HandOut,
        Body::AttemptStarted { .. } => Act::TakeTask,
        Body::ContributionPublished { .. } | Body::DocumentRevised { .. } => Act::Post,
        Body::WorkspaceProposed { .. } => Act::ProposeFiles,
        Body::CompletionDeclared { .. } => Act::DeclareDone,
        Body::ReviewRecorded { .. } => Act::Approve,
        Body::CheckAttested { .. } => Act::Attest,
        Body::ScopeDecided {
            context, action, ..
        } => match action {
            DecisionAction::Select { .. } if context.scope == Scope::Workspace => Act::MergeFiles,
            DecisionAction::Select { .. } => Act::Pick,
            DecisionAction::Close => Act::Close,
            DecisionAction::Reopen => Act::Reopen,
        },
        Body::CancelRequested { .. } => Act::Cancel,
        Body::TaskRevised { .. } => Act::Revise,
        Body::RulesBound { .. } => Act::ChangeRules,
        Body::LeaveRequested { .. } => Act::Leave,
        Body::PublicationSet(_) => Act::Publish,
        _ => Act::PersonCommand,
    }
}

const NO_GOAL: &str = "no such goal";

impl<S: Store, E: Entropy> Node<S, E> {
    /// The goal as `actor` may read it. The owner asking directly reads
    /// every goal; a principal only those it takes or took part in.
    pub(super) fn readable(&self, actor: &Actor, goal: &GoalId) -> Result<&Entry, ApiError> {
        let entry = self.goals.get(goal).ok_or_else(|| not_found(NO_GOAL))?;
        match actor.principal {
            Some(principal) if entry.membership(&principal).is_none() => Err(not_found(NO_GOAL)),
            Some(principal)
                if entry.membership(&principal) == Some(Membership::Refused)
                    && entry.goal.read_epoch(&principal).is_none() =>
            {
                Err(denied(
                    "the invitation was refused; join with a fresh invitation",
                ))
            }
            Some(principal) if entry.goal.read_epoch(&principal).is_none() => Err(ApiError::new(
                ErrorCode::Unavailable,
                "canonical admission has not arrived for this principal",
            )),
            _ => Ok(entry),
        }
    }

    /// The goal and the principal `actor` acts as in it, which must be a
    /// current member that has not left.
    pub(super) fn member(
        &self,
        actor: &Actor,
        goal: &GoalId,
    ) -> Result<(&Entry, PublicKey), ApiError> {
        let entry = self.readable(actor, goal)?;
        let principal = actor.principal()?;
        match entry.membership(&principal) {
            Some(Membership::Member) => Ok((entry, principal)),
            Some(Membership::Joining) => Err(ApiError::new(
                ErrorCode::Unavailable,
                "the host's admission has not arrived yet",
            )),
            _ => Err(denied("the principal is not a current member of the goal")),
        }
    }

    /// Whether this daemon holds the goal's governance key: the one its
    /// first record names.
    pub(super) fn hosts(&self, entry: &Entry) -> bool {
        entry
            .local
            .governance
            .as_ref()
            .is_some_and(|key| entry.state().governance == Some(key.public()))
    }

    /// The hosted goal and its governance key. No agent is involved: the
    /// key signs whether or not the host's agent is connected.
    pub(super) fn host(
        &self,
        actor: &Actor,
        goal: &GoalId,
    ) -> Result<(&Entry, PublicKey), ApiError> {
        let entry = self.readable(actor, goal)?;
        let governance = entry.state().governance.ok_or_else(|| not_found(NO_GOAL))?;
        if !self.hosts(entry) {
            return Err(denied(
                "this goal is hosted on another computer; its host decides",
            ));
        }
        Ok((entry, governance))
    }

    pub(super) fn refused_error(
        &self,
        entry: &Entry,
        agent: PublicKey,
        attempted: &Attempted<'_>,
        why: Why,
    ) -> ApiError {
        let agent_name = self
            .principals
            .get(&agent)
            .map(|p| p.record.name.clone())
            .unwrap_or_else(|| agent.to_string().chars().take(8).collect());
        let (act, task) = match attempted {
            Attempted::Sign { event, .. } => (
                act(&event.header().body),
                match &event.header().body {
                    Body::AttemptStarted { context, .. } => match context.scope {
                        Scope::Task(task) => Some(task),
                        _ => None,
                    },
                    _ => None,
                },
            ),
            Attempted::Resume { task } => (Act::Resume, Some(*task)),
            Attempted::Store => (Act::Post, None),
            Attempted::Withdraw => (Act::Withdraw, None),
        };
        let task_title = task
            .and_then(|task| entry.state().tasks.get(&task))
            .and_then(|task| entry.goal.event(&task.created))
            .and_then(|event| entry.text(&self.store, event, None));
        let code = match &why {
            Why::YourSetting { .. } => ErrorCode::LevelRequired,
            Why::Rules { .. } => ErrorCode::NotEligible,
            Why::State { .. } => ErrorCode::Conflict,
            Why::OnlyYou { .. } => ErrorCode::Denied,
        };
        let refused = Refused {
            agent,
            agent_name,
            member_name: entry
                .state()
                .members
                .get(&agent)
                .map(|member| member.name.clone()),
            goal: Some(entry.id()),
            goal_title: entry.local.title.clone(),
            act,
            task,
            task_title,
            why,
        };
        // The agent's voice quotes nothing another member wrote; the titles
        // and names travel only in the details.
        ApiError::new(code, render(&refused, Voice::Agent))
            .with_details(serde_json::to_value(refused).expect("refusal serializes"))
    }

    /// Replay the exact signed candidate before the local level, using the
    /// same projection that will later be committed. Nothing refused is stored.
    pub(super) fn allowed(
        &self,
        actor: &Actor,
        entry: &Entry,
        principal: PublicKey,
        attempted: Attempted<'_>,
    ) -> Result<(), ApiError> {
        self.allowed_keeping(actor, entry, principal, attempted)
            .map(|_| ())
    }

    /// As [`Self::allowed`], and hands back the replayed copy of the goal so
    /// that landing the same events need not replay them a second time.
    pub(super) fn allowed_keeping(
        &self,
        actor: &Actor,
        entry: &Entry,
        principal: PublicKey,
        attempted: Attempted<'_>,
    ) -> Result<Option<Trial>, ApiError> {
        let mut replayed = None;
        if let Attempted::Sign { event, preceding } = &attempted {
            if let Body::AttemptStarted { context, .. } | Body::WorkOffered { context, .. } =
                &event.header().body
                && matches!(context.scope, Scope::Task(_))
                && entry.state().task_round(*context).is_some()
                && !entry.goal.task_available(*context)
            {
                return Err(self.refused_error(
                    entry,
                    principal,
                    &attempted,
                    Why::State {
                        reason: "this task is closed, finished or picked".into(),
                    },
                ));
            }
            let mut trial = entry.goal.clone();
            let mut events = preceding.to_vec();
            events.push((*event).clone());
            let changes = trial.apply(&events, &entry.definitions);
            match trial.standing(&event.id()) {
                Some(Standing::Effective) => {}
                Some(Standing::Excluded(exclusion)) => {
                    if let Some(rule) = trial.rule_refusal(&event.id()) {
                        return Err(self.refused_error(
                            entry,
                            principal,
                            &attempted,
                            Why::Rules {
                                rule: rule.rule,
                                qualifies: rule.qualifies.clone(),
                                except_author: rule.except_author,
                                host: entry.state().host.unwrap_or(principal),
                                host_name: Self::host_name(entry),
                            },
                        ));
                    }
                    return Err(self.refused_error(
                        entry,
                        principal,
                        &attempted,
                        Why::State {
                            reason: exclusion
                                .reason()
                                .unwrap_or_else(|| exclusion.name())
                                .into(),
                        },
                    ));
                }
                _ => {
                    return Err(self.refused_error(
                        entry,
                        principal,
                        &attempted,
                        Why::State {
                            reason: "the candidate cannot be applied yet".into(),
                        },
                    ));
                }
            }
            replayed = Some(Trial {
                goal: entry.id(),
                revision: entry.local.revision,
                events: events
                    .iter()
                    .filter(|event| event.header().goal == entry.id())
                    .map(|event| event.id())
                    .collect(),
                replayed: trial,
                changes,
            });
        }
        if actor.owner_act {
            return Ok(replayed);
        }
        let needed = match &attempted {
            Attempted::Sign { event, .. } => match &event.header().body {
                Body::AttemptStarted { context, .. } => match context.scope {
                    Scope::Task(task) => entry.local.start_level(task, context.round, principal),
                    _ => Level::Auto,
                },
                Body::TaskOpened { .. }
                | Body::WorkOffered { .. }
                | Body::ContributionPublished { .. }
                | Body::DocumentRevised { .. }
                | Body::WorkspaceProposed { .. }
                | Body::CompletionDeclared { .. }
                | Body::ReviewRecorded { .. }
                | Body::CheckAttested { .. }
                | Body::ScopeDecided { .. }
                | Body::CancelRequested { .. } => Level::Ask,
                _ => Level::Read,
            },
            Attempted::Resume { task } => entry
                .state()
                .tasks
                .get(task)
                .map(|task_state| {
                    entry
                        .local
                        .start_level(*task, task_state.current_round, principal)
                })
                .unwrap_or(Level::Auto),
            Attempted::Store | Attempted::Withdraw => Level::Ask,
        };
        let level = entry.local.level(&principal);
        if level < needed {
            Err(self.refused_error(
                entry,
                principal,
                &attempted,
                Why::YourSetting {
                    level,
                    needs: needed,
                },
            ))
        } else {
            Ok(replayed)
        }
    }

    /// Resolve an owner-named agent for a person's goal request.
    pub(super) fn local_agent(&self, actor: &Actor, agent: PublicKey) -> Result<Actor, ApiError> {
        let principal = self.principals.active(&agent).ok_or_else(|| {
            ApiError::new(
                ErrorCode::NotFound,
                self.principals.inactive_message(&agent),
            )
        })?;
        if principal.record.author_only {
            return Err(denied("an authoring principal cannot act in goals"));
        }
        Ok(Actor {
            principal: Some(agent),
            owner_act: true,
            ..*actor
        })
    }
}
