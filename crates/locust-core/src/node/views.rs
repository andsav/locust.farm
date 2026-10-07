//! What reads answer with: the API's views, rendered from a goal's state and
//! this daemon's records about it.

use std::collections::{BTreeMap, BTreeSet};

use locust_proto::api::{
    self, ApiError, Attempting, BlobState, BlobStatus, CancelItem, ContextNews, DeliveryItem,
    EventDetail, EventView, GoalSummary, Level, Membership, PendingWork, ReviewItem, TaskDetail,
    TaskView, Verdict, WaitingForYou, WaitingKind, WorkItem, allow_command, level_command, short,
};
use locust_proto::engine::Entropy;
use locust_proto::event::{
    AttemptStatus, Body, Context, DecisionAction, DecisionPurpose, Event, ReviewVerdict, Scope,
    ScopeKey, TaskId,
};
use locust_proto::id::{BlobHash, GoalId, PublicKey};
use locust_proto::organization::CompletionRule;
use locust_proto::store::{Space, Store};

use super::Node;
use super::callers::Actor;
use super::entry::Entry;
use super::records;
use super::requests::invitations::InviteRecord;
use crate::goal::{Standing, Task};

impl Entry {
    /// How a local principal stands in the goal; `None` when it never took
    /// part, in which case the goal does not exist for it.
    pub fn membership(&self, principal: &PublicKey) -> Option<Membership> {
        if let Some(join) = self.local.joins.get(principal) {
            return Some(if join.refused {
                Membership::Refused
            } else {
                Membership::Joining
            });
        }
        self.goal.read_epoch(principal)?;
        Some(match self.local.part.get(principal)? {
            true => Membership::Left,
            false if self.is_member(principal) => Membership::Member,
            false => Membership::Removed,
        })
    }

    pub fn halted(&self) -> Option<api::Halt> {
        self.goal
            .evaluation()
            .host_halt
            .as_ref()
            .map(|_| api::Halt::AuthorityConflict)
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
            by_owner: self.local.by_owner.contains(&id),
            by_host: self.state().governance == Some(header.author),
            standing: match self.goal.standing(&id) {
                Some(Standing::Effective) => api::Standing::Effective,
                Some(Standing::Excluded(_)) => api::Standing::Excluded,
                Some(Standing::Disputed) => api::Standing::Disputed,
                Some(Standing::Pending(_)) | None => api::Standing::Pending,
            },
        }
    }
}

impl<S: Store, E: Entropy> Node<S, E> {
    /// The goal's title: the cached one, or the genesis text once its
    /// payload and key are held.
    pub(super) fn title(&self, entry: &Entry, reader: Option<&PublicKey>) -> Option<String> {
        if !entry.may_read_epoch(reader, 0) {
            return None;
        }
        if let Some(payload) = entry
            .goal
            .genesis()
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
            let genesis = entry.goal.event(&entry.goal.genesis()?)?;
            entry.text(&self.store, genesis, reader)
        })
    }

    /// One entry per goal and local agent in it: only `principal`'s own when
    /// one is named. Open invitations are counted in one scan, for the owner
    /// and for the goals this daemon hosts.
    pub(super) fn goal_summaries(
        &self,
        principal: Option<PublicKey>,
        now_ms: u64,
    ) -> Result<Vec<GoalSummary>, ApiError> {
        let mut invitations: BTreeMap<GoalId, (u32, Option<u64>)> = BTreeMap::new();
        if principal.is_none() {
            for (_, bytes) in self.store.scan(Space::Invite, &[])? {
                let record: InviteRecord = records::read(&bytes)?;
                let open = record.redeemed.is_none()
                    && record.revoked_ms.is_none()
                    && record.expires_ms.is_none_or(|expires| expires > now_ms);
                if !open {
                    continue;
                }
                let open = invitations.entry(record.goal).or_default();
                open.0 += 1;
                open.1 = open.1.max(record.expires_ms);
            }
        }
        let mut summaries = Vec::new();
        for (goal, entry) in &self.goals {
            let local = &entry.local;
            let members: BTreeSet<_> = local.joins.keys().chain(local.part.keys()).collect();
            let (invitations_open, invitations_expire_ms) = if self.hosts(entry) {
                invitations.get(goal).copied().unwrap_or_default()
            } else {
                (0, None)
            };
            for member in members
                .into_iter()
                .filter(|member| principal.is_none_or(|own| own == **member))
            {
                let Some(membership) = entry.membership(member) else {
                    continue;
                };
                let abilities = self.abilities(entry, *member);
                summaries.push(GoalSummary {
                    goal: *goal,
                    title: self.title(entry, principal.as_ref()),
                    member: *member,
                    // The name the person chose: signed into the admission,
                    // or asked for in the join until the host admits it.
                    name: entry
                        .state()
                        .members
                        .get(member)
                        .map(|found| found.name.clone())
                        .or_else(|| local.joins.get(member).map(|join| join.name.clone()))
                        .unwrap_or_else(|| abilities.name.clone()),
                    membership,
                    host_name: Self::host_name(entry),
                    invitations_open,
                    invitations_expire_ms,
                    halted: self.halt(entry, Some(member)),
                    guard: self.guard_views(entry, [*member]),
                    restored: local.restored.map(|restored| restored.revoked),
                    abilities,
                });
            }
        }
        Ok(summaries)
    }

    /// What a command of the person settles: one entry per task an agent
    /// wanted while its level is below auto, oldest first. At ask the command
    /// allows the task; at read an allowance would not help, so it sets the
    /// agent to ask. A joining or disconnected agent, a halted goal, a task
    /// the agent already holds an attempt on and a task the rules no longer
    /// let it start give no entry: no command of the person settles those.
    pub(super) fn waiting_for(&self, summaries: &[GoalSummary]) -> Vec<WaitingForYou> {
        let goals: Vec<String> = self.goals.keys().map(ToString::to_string).collect();
        let mut waiting = Vec::new();
        for summary in summaries {
            if summary.membership != Membership::Member
                || summary.halted.is_some()
                || summary.abilities.level >= Level::Auto
                || self.principals.active(&summary.member).is_none()
            {
                continue;
            }
            let Some(entry) = self.goals.get(&summary.goal) else {
                continue;
            };
            let tasks: Vec<String> = entry
                .state()
                .tasks
                .keys()
                .map(ToString::to_string)
                .collect();
            let goal = short(&summary.goal.to_string(), &goals);
            for wanted in &summary.abilities.wanted_tasks {
                let holds_attempt = summary.abilities.claims.iter().any(|claim| {
                    entry
                        .state()
                        .attempts
                        .get(&claim.attempt)
                        .is_some_and(|attempt| attempt.context.scope == Scope::Task(wanted.task))
                });
                if holds_attempt || !self.may_start(entry, summary.member, wanted.task) {
                    continue;
                }
                let task = short(&wanted.task.to_string(), &tasks);
                let (kind, command) = if summary.abilities.level == Level::Read {
                    (
                        WaitingKind::SetAsk {
                            task: wanted.task,
                            task_title: wanted.title.clone(),
                        },
                        level_command(&goal, &summary.abilities.name, "ask"),
                    )
                } else {
                    (
                        WaitingKind::AllowTask {
                            task: wanted.task,
                            task_title: wanted.title.clone(),
                        },
                        allow_command(&goal, &task, &summary.abilities.name, false),
                    )
                };
                waiting.push((
                    wanted.since_ms,
                    WaitingForYou {
                        goal: summary.goal,
                        title: summary.title.clone(),
                        agent: Some(summary.member),
                        agent_name: Some(summary.name.clone()),
                        kind,
                        command,
                    },
                ));
            }
        }
        waiting.sort_by_key(|(since, _)| *since);
        waiting.into_iter().map(|(_, entry)| entry).collect()
    }

    /// Whether the rules let `member` start the current round of `task`, by
    /// itself or through an offer addressed to it on that round.
    fn may_start(&self, entry: &Entry, member: PublicKey, task: TaskId) -> bool {
        let Some(found) = entry.state().tasks.get(&task) else {
            return false;
        };
        let round = &found.rounds[&found.current_round];
        std::iter::once(None)
            .chain(
                entry
                    .state()
                    .offers
                    .values()
                    .filter(|offer| offer.context == round.context && offer.recipient == member)
                    .map(|offer| Some(offer.id)),
            )
            .any(|offer| {
                entry
                    .goal
                    .can_start(round.context, member, offer, &entry.definitions)
            })
    }

    /// Whether the daemon can serve one content object of a goal.
    pub(super) fn blob_state(
        &self,
        entry: &Entry,
        hash: &BlobHash,
        reader: Option<&PublicKey>,
    ) -> BlobState {
        self.read_content_state(entry, hash, reader)
            .unwrap_or(BlobState::Unavailable)
    }

    /// One task as the board lists it for `reader`.
    pub(super) fn task_view(
        &self,
        entry: &Entry,
        task: &Task,
        reader: Option<&PublicKey>,
    ) -> TaskView {
        let round = &task.rounds[&task.current_round];
        let title = entry.task_title(&self.store, &task.id, reader);
        TaskView {
            task: task.id,
            context: round.context,
            creator: task.creator,
            by_host: entry.state().governance == Some(task.creator),
            title,
            attempts: round.attempts.iter().copied().collect(),
            contributions: round.contributions.iter().copied().collect(),
            completed: round.completed,
            selected: round.selected,
            closed: round.closed,
        }
    }

    pub(super) fn task_detail(
        &self,
        entry: &Entry,
        task: &Task,
        reader: Option<&PublicKey>,
    ) -> TaskDetail {
        let round = &task.rounds[&task.current_round];
        TaskDetail {
            view: self.task_view(entry, task, reader),
            text: entry
                .goal
                .event(&task.created)
                .and_then(|event| entry.text(&self.store, event, reader)),
            inputs: round.binding.inputs.clone(),
            parent: round.binding.parent.and_then(|context| {
                if let Scope::Task(task) = context.scope {
                    Some(task)
                } else {
                    None
                }
            }),
            task_type: round.binding.task_type.clone(),
            effective_rules_json: serde_json::to_string(
                &entry
                    .goal
                    .effective_rules(round.context, &entry.definitions)
                    .or_else(|| entry.goal.selected_rules(round.context, &entry.definitions)),
            )
            .expect("effective rules encode"),
        }
    }

    /// One event in full.
    pub(super) fn event_detail(
        &self,
        entry: &Entry,
        event: &Event,
        reader: Option<&PublicKey>,
    ) -> EventDetail {
        let header = event.header();
        EventDetail {
            view: entry.event_view(event),
            anchor: header.anchor,
            body: header.body.clone(),
            payload: header.payload,
            text: entry.text(&self.store, event, reader),
            task: entry.goal.task_of(&event.id()),
            content: header
                .blobs()
                .into_iter()
                .map(|hash| BlobStatus {
                    hash,
                    state: self.blob_state(entry, &hash, reader),
                })
                .collect(),
        }
    }

    /// Durable evidence determines pending work after every retry and restart.
    pub(super) fn pending_work(&self, entry: &Entry, actor: &Actor) -> PendingWork {
        self.pending_work_with_news(entry, actor, self.context_news(entry, actor))
    }

    pub(super) fn pending_work_with_news(
        &self,
        entry: &Entry,
        actor: &Actor,
        context_news: Option<ContextNews>,
    ) -> PendingWork {
        let mut work = PendingWork {
            workspace: entry.state().workspace.as_ref().map(|workspace| {
                Box::new(locust_proto::api::WorkspaceStatus {
                    epoch: workspace.epoch,
                    head: workspace.head,
                    ready: workspace.ready,
                    enabled: workspace.enabled,
                    checkout: self.bound_checkout(entry, actor),
                })
            }),
            revision: entry.revision(),
            context_news,
            ..PendingWork::default()
        };
        let candidates: Vec<_> = match actor.principal {
            Some(principal) => vec![principal],
            None => entry.local.part.keys().copied().collect(),
        };
        for principal in candidates {
            if entry.membership(&principal) != Some(Membership::Member)
                || self.principals.active(&principal).is_none()
            {
                continue;
            }
            for task in entry.state().tasks.values() {
                let round = &task.rounds[&task.current_round];
                let offers = std::iter::once(None).chain(
                    entry
                        .state()
                        .offers
                        .values()
                        .filter(|offer| {
                            offer.context == round.context && offer.recipient == principal
                        })
                        .map(|offer| Some(offer.id)),
                );
                for offer in offers {
                    // A start recovers this session's existing claim for this
                    // offer. Other sessions may still make independent attempts.
                    let already_running = round.attempts.iter().any(|id| {
                        entry.state().attempts.get(id).is_some_and(|attempt| {
                            attempt.author == principal
                                && attempt.offer == offer
                                && matches!(attempt.status, None | Some(AttemptStatus::Progress))
                                && entry
                                    .claims
                                    .get(id)
                                    .is_some_and(|claim| Some(claim.instance) == actor.session)
                        })
                    });
                    if already_running {
                        continue;
                    }

                    if !entry
                        .goal
                        .can_start(round.context, principal, offer, &entry.definitions)
                    {
                        continue;
                    }
                    let item = WorkItem {
                        task: task.id,
                        unattended: entry.goal.unattended(task.id),
                        offer,
                        attempting: round
                            .attempts
                            .iter()
                            .filter_map(|id| entry.state().attempts.get(id))
                            .filter(|attempt| {
                                attempt.author != principal
                                    && matches!(
                                        attempt.status,
                                        None | Some(AttemptStatus::Progress)
                                    )
                            })
                            .map(|attempt| Attempting {
                                member: attempt.author,
                                status: attempt.status,
                            })
                            .collect(),
                        results: round.contributions.len() as u32,
                    };
                    let level = entry.local.level(&principal);
                    if level
                        >= entry
                            .local
                            .start_level(task.id, task.current_round, principal)
                    {
                        if actor.principal.is_some() {
                            work.to_start.push(item);
                        }
                    } else if level == Level::Ask {
                        // At read an allowance would not help; only a level
                        // change does, and that is not a per-task ask.
                        work.ask_first.push(item);
                    }
                }
            }
            if actor.principal.is_none() {
                continue;
            }
            for (id, attempt) in &entry.state().attempts {
                if attempt.author != principal {
                    continue;
                }
                let Scope::Task(task) = attempt.context.scope else {
                    continue;
                };
                if entry.goal.current_context(attempt.context.scope) != Some(attempt.context) {
                    continue;
                }
                // An ended attempt holds no claim and owes no answer. Listing a
                // request without its claim would leave the session with an
                // obligation that names no current generation.
                if !matches!(attempt.status, None | Some(AttemptStatus::Progress)) {
                    continue;
                }
                let claim = entry.claims.get(id);
                let mine = claim.filter(|claim| Some(claim.instance) == actor.session);
                if let Some(claim) = claim {
                    let view = claim.view(entry.id(), *id);
                    if mine.is_some() {
                        work.claimed.push(view);
                    } else {
                        work.held_elsewhere.push(view);
                    }
                }
                if claim.is_none() || mine.is_some() {
                    for cancel in &attempt.cancellations {
                        if entry
                            .state()
                            .cancellations
                            .get(cancel)
                            .is_some_and(|cancel| cancel.acknowledgments.is_empty())
                        {
                            work.to_acknowledge.push(CancelItem {
                                task,
                                attempt: *id,
                                cancel: *cancel,
                                generation: mine.map(|claim| claim.generation),
                            });
                        }
                    }
                }
            }
            // The caller's own reviews and attestations decide what is left for
            // it to judge. Other members' verdicts are read only for those
            // candidates, so an idle poll does not walk the whole goal.
            let mut own_reviewed = BTreeSet::new();
            let mut attested = BTreeSet::new();
            for point in entry.goal.points(&principal) {
                let Some(event) = entry.goal.event(&point.id) else {
                    continue;
                };
                if entry.goal.standing(&event.id()) != Some(Standing::Effective) {
                    continue;
                }
                match &event.header().body {
                    Body::ReviewRecorded { subject, .. } => {
                        own_reviewed.insert(*subject);
                    }
                    Body::CheckAttested { subject, .. } => {
                        attested.insert(*subject);
                    }
                    _ => {}
                }
            }
            let reviewable = entry
                .state()
                .contributions
                .values()
                .map(|c| (c.id, c.context, c.approved))
                .chain(
                    entry
                        .state()
                        .revisions
                        .values()
                        .map(|r| (r.id, r.context, r.approved)),
                );
            let reviewable = reviewable.chain(
                entry
                    .state()
                    .workspace_proposals
                    .values()
                    .filter(|proposal| {
                        entry.state().workspace.as_ref().is_some_and(|workspace| {
                            workspace.ready
                                && workspace.epoch == proposal.context.round
                                && workspace.head == proposal.parent
                        })
                    })
                    .map(|proposal| (proposal.id, proposal.context, proposal.approved)),
            );
            let candidates: Vec<_> = reviewable
                .filter(|(subject, context, approved)| {
                    // Nothing asks for a review on a task round that is over:
                    // the engine signs no request there, so the view lists none.
                    if *approved || !wants_review(entry, *context) {
                        return false;
                    }
                    let can_review = !own_reviewed.contains(subject)
                        && entry
                            .goal
                            .can_review(*subject, principal, &entry.definitions);
                    let can_attest = !attested.contains(subject)
                        && entry
                            .goal
                            .can_attest(*subject, principal, &entry.definitions);
                    let selected = entry
                        .state()
                        .selections
                        .values()
                        .any(|selection| selection.subject.id() == *subject);
                    !selected && (can_review || can_attest)
                })
                .collect();
            let subjects: Vec<_> = candidates.iter().map(|(subject, ..)| *subject).collect();
            let mut latest = entry.goal.latest_reviews(&subjects, &entry.definitions);
            for (subject, context, _) in candidates {
                let rules = entry.goal.effective_rules(context, &entry.definitions);
                let needed = rules
                    .as_ref()
                    .and_then(|rules| first_review_count(&rules.decisions.completion))
                    .unwrap_or(0);
                let opinion = rules.as_ref().is_some_and(|rules| {
                    !crate::goal::asks_for_review(&rules.decisions.completion)
                });
                let verdicts: Vec<_> = latest
                    .remove(&subject)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|(member, id)| {
                        let event = entry.goal.event(&id)?;
                        let Body::ReviewRecorded { verdict, .. } = &event.header().body else {
                            return None;
                        };
                        Some(Verdict {
                            member,
                            approve: *verdict == ReviewVerdict::Approve,
                            event: id,
                            opinion,
                        })
                    })
                    .collect();
                let approvals = verdicts
                    .iter()
                    .filter(|verdict| verdict.approve && !verdict.opinion)
                    .count() as u32;
                work.to_review.push(ReviewItem {
                    subject,
                    context,
                    needed,
                    approvals,
                    verdicts,
                });
            }
            for ((effect, recipient), delivery) in &entry.deliveries {
                if *recipient != principal || !delivery.received {
                    continue;
                }
                work.deliveries.push(DeliveryItem {
                    effect: *effect,
                    context: delivery.context,
                    acknowledged: entry
                        .state()
                        .effects
                        .get(effect)
                        .is_some_and(|effect| effect.acknowledged.contains(&principal)),
                    received: delivery.received,
                    available: delivery.available,
                    action: delivery.action.clone(),
                });
            }
        }
        work.to_start
            .sort_by_key(|item| (item.attempting.len(), item.results, item.task));
        work
    }
}

/// Whether a result in `context` can still be asked about. A task result
/// needs its round to be the current one with nothing picked or closed; a
/// finding, document revision or file proposal only needs its scope open,
/// since the engine keeps asking for their review after new rules are bound.
fn wants_review(entry: &Entry, context: Context) -> bool {
    match context.scope {
        Scope::Task(_) => {
            entry.goal.current_context(context.scope) == Some(context)
                && entry
                    .state()
                    .task_round(context)
                    .is_some_and(|round| !round.closed && round.selected.is_none())
        }
        Scope::Goal | Scope::Document(_) | Scope::Workspace => {
            let closed = entry
                .state()
                .decisions
                .get(&ScopeKey {
                    context,
                    purpose: DecisionPurpose::Closure,
                })
                .and_then(|decisions| decisions.last())
                .is_some_and(|decision| decision.action == DecisionAction::Close);
            !closed
        }
    }
}

fn first_review_count(rule: &CompletionRule) -> Option<u32> {
    match rule {
        CompletionRule::Reviews { count, .. } => Some(*count),
        CompletionRule::All { rules } | CompletionRule::Any { rules } => {
            rules.iter().find_map(first_review_count)
        }
        _ => None,
    }
}
