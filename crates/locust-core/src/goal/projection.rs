//! Build read models only from verified facts. Ordering here presents history;
//! it never elects a winner among independent contributions.
use std::collections::{BTreeMap, BTreeSet};

use locust_proto::event::{
    Body, Context, DecisionAction, EffectAction, Scope, ScopeKey, TaskBinding, TaskId,
};
use locust_proto::id::{EventId, PublicKey};

use super::DefinitionLookup;
use super::fold::Verifier;
use super::standing::{Evaluation, Standing};
use super::state::*;

fn add_task(
    state: &mut State,
    id: TaskId,
    created: EventId,
    creator: PublicKey,
    binding: &TaskBinding,
) {
    state.tasks.entry(id).or_insert_with(|| Task {
        id,
        creator,
        created,
        current_round: created,
        rounds: BTreeMap::from([(
            created,
            TaskRound {
                context: Context {
                    scope: Scope::Task(id),
                    round: created,
                },
                binding: binding.clone(),
                contributions: BTreeSet::new(),
                attempts: BTreeSet::new(),
                completed: false,
                selected: None,
                closed: false,
            },
        )]),
    });
}

pub(super) fn project<D: DefinitionLookup + ?Sized>(v: &Verifier<'_, D>, out: &mut Evaluation) {
    let mut events: Vec<_> = v
        .history
        .events
        .iter()
        .filter(|event| out.standings.get(&event.id()) == Some(&Standing::Effective))
        .collect();
    events.sort_by_key(|event| {
        (
            event
                .header()
                .anchor
                .and_then(|id| v.chain.position(&id))
                .unwrap_or(0),
            event.header().author,
            event.header().seq,
            event.id(),
        )
    });
    for event in &events {
        match &event.header().body {
            Body::TaskOpened { binding } => add_task(
                &mut out.state,
                TaskId::Authored(event.id()),
                event.id(),
                event.header().author,
                binding,
            ),
            Body::EffectMaterialized { effect } => {
                let id = effect.id(event.header().goal);
                let record = out
                    .state
                    .effects
                    .entry(id)
                    .or_insert_with(|| MaterializedEffect {
                        id,
                        events: BTreeSet::new(),
                        materializer: event.header().author,
                        effect: effect.clone(),
                        recipients: v.effect_recipients(effect),
                        acknowledged: BTreeSet::new(),
                    });
                record.events.insert(event.id());
                if let EffectAction::OpenTask { binding, .. } = &effect.action {
                    add_task(
                        &mut out.state,
                        TaskId::Derived(id),
                        event.id(),
                        event.header().author,
                        binding,
                    );
                }
            }
            _ => {}
        }
    }
    for id in &v.chain.order {
        if out.standings.get(id) != Some(&Standing::Effective) {
            continue;
        }
        let event = v.history.get(id).expect("chain event exists");
        if let Body::TaskRevised { task, binding, .. } = &event.header().body
            && let Some(task) = out.state.tasks.get_mut(task)
        {
            task.current_round = *id;
            task.rounds.insert(
                *id,
                TaskRound {
                    context: Context {
                        scope: Scope::Task(task.id),
                        round: *id,
                    },
                    binding: binding.clone(),
                    contributions: BTreeSet::new(),
                    attempts: BTreeSet::new(),
                    completed: false,
                    selected: None,
                    closed: false,
                },
            );
        }
    }
    for event in &events {
        let h = event.header();
        let id = event.id();
        match &h.body {
            Body::WorkOffered { context, recipient } => {
                out.state.offers.insert(
                    id,
                    Offer {
                        id,
                        author: h.author,
                        context: *context,
                        recipient: *recipient,
                        attempts: BTreeSet::new(),
                        declined: BTreeSet::new(),
                    },
                );
            }
            Body::EffectMaterialized { effect } => {
                if let EffectAction::Offer { context, recipient } = effect.action {
                    out.state.offers.entry(id).or_insert(Offer {
                        id,
                        author: h.author,
                        context,
                        recipient,
                        attempts: BTreeSet::new(),
                        declined: BTreeSet::new(),
                    });
                }
            }
            Body::AttemptStarted { context, offer } => {
                out.state.attempts.insert(
                    id,
                    Attempt {
                        id,
                        author: h.author,
                        context: *context,
                        offer: *offer,
                        status: None,
                        reports: Vec::new(),
                        cancellations: BTreeSet::new(),
                    },
                );
                if let Scope::Task(task) = context.scope
                    && let Some(round) = out
                        .state
                        .tasks
                        .get_mut(&task)
                        .and_then(|task| task.rounds.get_mut(&context.round))
                {
                    round.attempts.insert(id);
                }
            }
            Body::CancelRequested { attempt } => {
                out.state.cancellations.insert(
                    id,
                    Cancellation {
                        id,
                        author: h.author,
                        attempt: *attempt,
                        acknowledgments: BTreeMap::new(),
                    },
                );
            }
            Body::ContributionPublished {
                context,
                attempt,
                base,
                patch,
                artifacts,
            } => {
                let witness = v.approval(id, None, None).ok().flatten();
                let approved = witness.is_some();
                out.state.contributions.insert(
                    id,
                    Contribution {
                        id,
                        author: h.author,
                        context: *context,
                        attempt: *attempt,
                        base: *base,
                        patch: *patch,
                        artifacts: artifacts.clone(),
                        approved,
                        evidence: witness.unwrap_or_default(),
                    },
                );
                if let Scope::Task(task) = context.scope
                    && let Some(round) = out
                        .state
                        .tasks
                        .get_mut(&task)
                        .and_then(|task| task.rounds.get_mut(&context.round))
                {
                    round.contributions.insert(id);
                    round.completed |= approved;
                }
            }
            Body::DocumentRevised { context, doc, base } => {
                let witness = v.approval(id, None, None).ok().flatten();
                out.state.revisions.insert(
                    id,
                    Revision {
                        id,
                        author: h.author,
                        context: *context,
                        doc: *doc,
                        base: *base,
                        approved: witness.is_some(),
                        evidence: witness.unwrap_or_default(),
                    },
                );
                out.state
                    .documents
                    .entry(*doc)
                    .or_default()
                    .revisions
                    .insert(id);
            }
            Body::ScopeDecided {
                context,
                previous,
                action,
                evidence,
            } => {
                let key = ScopeKey {
                    context: *context,
                    purpose: action.purpose(),
                };
                out.state.decisions.entry(key).or_default().push(Decision {
                    id,
                    author: h.author,
                    key,
                    previous: *previous,
                    action: action.clone(),
                    evidence: evidence.iter().copied().collect(),
                });
            }
            Body::LeaveRequested { .. } => {
                out.state.leave_requests.insert(id);
            }
            _ => {}
        }
    }
    for event in &events {
        let id = event.id();
        match &event.header().body {
            Body::AttemptStarted {
                offer: Some(offer), ..
            } => {
                if let Some(offer) = out.state.offers.get_mut(offer) {
                    offer.attempts.insert(id);
                }
            }
            Body::AttemptReported { attempt, status } => {
                if let Some(attempt) = out.state.attempts.get_mut(attempt) {
                    attempt.status = Some(*status);
                    attempt.reports.push(id);
                }
            }
            Body::WorkDeclined { offer } => {
                if let Some(offer) = out.state.offers.get_mut(offer) {
                    offer.declined.insert(id);
                }
            }
            Body::CancelRequested { attempt } => {
                if let Some(attempt) = out.state.attempts.get_mut(attempt) {
                    attempt.cancellations.insert(id);
                }
            }
            Body::CancelAcknowledged { cancel, outcome } => {
                if let Some(cancel) = out.state.cancellations.get_mut(cancel) {
                    cancel.acknowledgments.insert(id, *outcome);
                }
            }
            Body::DeliveryAcknowledged { effect } => {
                if let Some(effect) = out.state.effects.get_mut(effect) {
                    effect.acknowledged.insert(event.header().author);
                }
            }
            _ => {}
        }
    }
    for (key, decisions) in &mut out.state.decisions {
        decisions.sort_by_key(|decision| {
            v.history
                .get(&decision.id)
                .expect("decision exists")
                .header()
                .seq
        });
        let Some(last) = decisions.last() else {
            continue;
        };
        if let DecisionAction::Select { subject } = last.action {
            let proof = v.proof(last.id).expect("effective decision has a proof");
            let evidence = v
                .approval(subject, Some(last.id), Some(&proof.roots))
                .expect("effective decision has valid subject")
                .expect("effective selection has positive evidence");
            let event = v.history.get(&subject).expect("selected subject retained");
            let h = event.header();
            let selected = match &h.body {
                Body::ContributionPublished {
                    context,
                    attempt,
                    base,
                    patch,
                    artifacts,
                } => SelectedSubject::Contribution(Contribution {
                    id: subject,
                    author: h.author,
                    context: *context,
                    attempt: *attempt,
                    base: *base,
                    patch: *patch,
                    artifacts: artifacts.clone(),
                    approved: true,
                    evidence,
                }),
                Body::DocumentRevised { context, doc, base } => {
                    SelectedSubject::Revision(Revision {
                        id: subject,
                        author: h.author,
                        context: *context,
                        doc: *doc,
                        base: *base,
                        approved: true,
                        evidence,
                    })
                }
                _ => unreachable!("selection requires a contribution or revision"),
            };
            let task = v
                .resolve(key.context)
                .expect("effective context resolves")
                .task
                .map(|binding| TaskRound {
                    context: key.context,
                    binding,
                    contributions: BTreeSet::from([subject]),
                    attempts: BTreeSet::new(),
                    completed: true,
                    selected: Some(subject),
                    closed: false,
                });
            out.state.selections.insert(
                *key,
                ScopedSelection {
                    decision: last.id,
                    context: key.context,
                    subject: selected,
                    task,
                },
            );
        }
        match (&key.context.scope, &last.action) {
            (Scope::Task(task), action) => {
                if let Some(round) = out
                    .state
                    .tasks
                    .get_mut(task)
                    .and_then(|task| task.rounds.get_mut(&key.context.round))
                {
                    match action {
                        DecisionAction::Select { subject } => {
                            round.selected = Some(*subject);
                            round.completed = true;
                        }
                        DecisionAction::Close => round.closed = true,
                        DecisionAction::Reopen => round.closed = false,
                    }
                }
            }
            (Scope::Document(doc), DecisionAction::Select { subject }) => {
                out.state.documents.entry(*doc).or_default().selected = Some(*subject);
            }
            _ => {}
        }
    }
}
