//! What reads answer with: the API's views, rendered from a goal's state and
//! this daemon's records about it.

use std::collections::{BTreeMap, BTreeSet};

use locust_proto::api::{
    self, Attempting, BlobState, BlobStatus, CancelItem, ContextNews, DeliveryItem, EventDetail,
    EventView, GoalSummary, Membership, PendingWork, ReviewItem, TaskDetail, TaskView, Verdict,
    WorkItem,
};
use locust_proto::engine::Entropy;
use locust_proto::event::{AttemptStatus, Body, Event, ReviewVerdict, Scope};
use locust_proto::id::{BlobHash, EventId, PublicKey};
use locust_proto::organization::CompletionRule;
use locust_proto::store::Store;

use super::Node;
use super::callers::Actor;
use super::entry::Entry;
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

    /// One entry per goal and local principal in it: only `principal`'s own
    /// when one is named.
    pub(super) fn goal_summaries(&self, principal: Option<PublicKey>) -> Vec<GoalSummary> {
        let mut summaries = Vec::new();
        for (goal, entry) in &self.goals {
            let local = &entry.local;
            let members: BTreeSet<_> = local.joins.keys().chain(local.part.keys()).collect();
            for member in members
                .into_iter()
                .filter(|member| principal.is_none_or(|own| own == **member))
            {
                let Some(membership) = entry.membership(member) else {
                    continue;
                };
                summaries.push(GoalSummary {
                    goal: *goal,
                    title: self.title(entry, principal.as_ref()),
                    member: *member,
                    membership,
                    halted: entry.halted(),
                    abilities: self.abilities(entry, *member),
                });
            }
        }
        summaries
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
        let title = entry
            .goal
            .event(&task.created)
            .and_then(|event| entry.text(&self.store, event, reader))
            .map(|text| text.lines().next().unwrap_or_default().to_owned());
        TaskView {
            task: task.id,
            context: round.context,
            creator: task.creator,
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
                    if entry.local.level(&principal)
                        >= entry
                            .local
                            .start_level(task.id, task.current_round, principal)
                    {
                        if actor.principal.is_some() {
                            work.to_start.push(item);
                        }
                    } else {
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
                .filter(|(subject, _, approved)| {
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
                    !approved && !selected && (can_review || can_attest)
                })
                .collect();
            // Keep each member's latest verdict, and (until the later replay
            // change) every member with positive evidence, matching the current
            // fold.
            let wanted: BTreeSet<EventId> =
                candidates.iter().map(|(subject, _, _)| *subject).collect();
            let mut latest: BTreeMap<EventId, BTreeMap<PublicKey, (u64, Verdict)>> =
                BTreeMap::new();
            let mut approvals: BTreeMap<EventId, BTreeSet<PublicKey>> = BTreeMap::new();
            if !wanted.is_empty() {
                for author in entry.goal.authors() {
                    for point in entry.goal.points(author) {
                        let Some(event) = entry.goal.event(&point.id) else {
                            continue;
                        };
                        let Body::ReviewRecorded {
                            subject, verdict, ..
                        } = &event.header().body
                        else {
                            continue;
                        };
                        if !wanted.contains(subject)
                            || entry.goal.standing(&event.id()) != Some(Standing::Effective)
                        {
                            continue;
                        }
                        let approve = *verdict == ReviewVerdict::Approve;
                        let by_member = latest.entry(*subject).or_default();
                        if by_member
                            .get(author)
                            .is_none_or(|(seq, _)| *seq < point.seq)
                        {
                            by_member.insert(
                                *author,
                                (
                                    point.seq,
                                    Verdict {
                                        member: *author,
                                        approve,
                                        event: event.id(),
                                    },
                                ),
                            );
                        }
                        if approve && entry.goal.can_review(*subject, *author, &entry.definitions) {
                            approvals.entry(*subject).or_default().insert(*author);
                        }
                    }
                }
            }
            for (subject, context, _) in candidates {
                let needed = entry
                    .goal
                    .effective_rules(context, &entry.definitions)
                    .and_then(|rules| first_review_count(&rules.decisions.completion))
                    .unwrap_or(0);
                work.to_review.push(ReviewItem {
                    subject,
                    context,
                    needed,
                    approvals: approvals
                        .get(&subject)
                        .map_or(0, |members| members.len() as u32),
                    verdicts: latest
                        .get(&subject)
                        .map(|members| {
                            members
                                .values()
                                .map(|(_, verdict)| verdict.clone())
                                .collect()
                        })
                        .unwrap_or_default(),
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

fn first_review_count(rule: &CompletionRule) -> Option<u32> {
    match rule {
        CompletionRule::Reviews { count, .. } => Some(*count),
        CompletionRule::All { rules } | CompletionRule::Any { rules } => {
            rules.iter().find_map(first_review_count)
        }
        _ => None,
    }
}
