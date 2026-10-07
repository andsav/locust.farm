//! Pure organization evaluation. Equal authenticated evidence and definitions
//! produce equal projections regardless of transport or insertion order.
mod chain;
mod closure;
mod commitments;
mod delegation;
mod flow;
mod fold;
mod history;
mod ids;
mod projection;
mod rules;
mod screen;
pub mod standing;
pub mod state;
mod workspace;

use std::collections::{BTreeMap, BTreeSet};

use locust_proto::api::{Ability, Level, Rule};
use locust_proto::event::{AuthorPoint, Body, Context, Event, Scope, TaskId};
use locust_proto::id::{DefinitionHash, EventId, GoalId, PublicKey};
use locust_proto::organization::{Formation, Selector, StartRule};
use locust_proto::store::{Store, StoreError};
use locust_proto::sync::{AuthorFrontier, Frontier};

pub use rules::{EffectiveRules, asks_for_review};
pub use standing::{
    Changes, Dependency, DesiredEffect, Evaluation, Exclusion, Halt, Next, RuleRefusal, Standing,
    Waiting,
};
pub use state::*;

pub trait DefinitionLookup {
    fn definition(&self, hash: &DefinitionHash) -> Option<&Formation>;
}

impl DefinitionLookup for BTreeMap<DefinitionHash, Formation> {
    fn definition(&self, hash: &DefinitionHash) -> Option<&Formation> {
        self.get(hash)
    }
}

fn valid_definition(hash: &DefinitionHash, definition: &Formation) -> bool {
    let Ok(source) = serde_json::to_string(definition) else {
        return false;
    };
    let inspected = crate::organization::inspect(&source);
    inspected.valid
        && inspected.normalized.as_ref() == Some(definition)
        && inspected.semantic_hash.as_deref() == Some(hash.to_string().as_str())
}

/// A goal-wide finding is a published contribution outside any attempt.
pub fn is_goal_finding(event: &Event) -> bool {
    matches!(
        &event.header().body,
        Body::ContributionPublished {
            context,
            attempt: None,
            ..
        } if context.scope == Scope::Goal
    )
}

#[derive(Clone)]
pub struct Goal {
    id: GoalId,
    history: history::History,
    chain: chain::Chain,
    evaluation: Evaluation,
    closure_index: commitments::Index,
    #[cfg(test)]
    refolds: usize,
}

impl Goal {
    pub fn new(id: GoalId) -> Self {
        Self {
            id,
            history: history::History::default(),
            chain: chain::Chain::default(),
            evaluation: Evaluation::default(),
            closure_index: commitments::Index::default(),
            #[cfg(test)]
            refolds: 0,
        }
    }
    pub fn load<S: Store, D: DefinitionLookup + ?Sized>(
        store: &S,
        id: GoalId,
        definitions: &D,
    ) -> Result<Self, StoreError> {
        let mut goal = Self::new(id);
        let mut cursor = 0;
        loop {
            let page = store.log(&id, cursor, 256)?;
            if page.is_empty() {
                break;
            }
            for (position, event) in page {
                goal.history.insert(&event);
                cursor = position;
            }
        }
        goal.refresh(definitions);
        Ok(goal)
    }
    pub fn apply<D: DefinitionLookup + ?Sized>(
        &mut self,
        events: &[Event],
        definitions: &D,
    ) -> Changes {
        let mut inserted = false;
        for event in events {
            if event.header().goal == self.id {
                inserted |= self.history.insert(event).is_some();
            }
        }
        let mut changes = self.refresh(definitions);
        changes.changed |= inserted;
        changes
    }
    pub fn refresh<D: DefinitionLookup + ?Sized>(&mut self, definitions: &D) -> Changes {
        #[cfg(test)]
        {
            self.refolds += 1;
        }
        let chain = chain::Chain::build(&self.history, definitions);
        let closure_index = if chain.order == self.chain.order {
            std::mem::take(&mut self.closure_index)
        } else {
            commitments::Index::default()
        };
        let (evaluation, closure_index) =
            fold::evaluate(&self.history, &chain, definitions, closure_index);
        self.closure_index = closure_index;
        let judged = evaluation
            .standings
            .iter()
            .filter(|(id, status)| self.evaluation.standings.get(id) != Some(status))
            .map(|(id, _)| *id)
            .collect();
        let changed = evaluation != self.evaluation;
        self.chain = chain;
        self.evaluation = evaluation;
        Changes {
            judged,
            changed,
            refolded: true,
        }
    }
    pub fn id(&self) -> GoalId {
        self.id
    }
    #[cfg(test)]
    pub(crate) fn refold_count(&self) -> usize {
        self.refolds
    }
    /// Number of cutoff-ancestry traversals performed in the last fold. One
    /// per removed tenure regardless of how many retained events authorize.
    #[cfg(test)]
    pub(crate) fn cutoff_traversals(&self) -> usize {
        self.chain.cutoff_traversals.get()
    }
    pub fn len(&self) -> usize {
        self.history.events.len()
    }
    pub fn is_empty(&self) -> bool {
        self.history.events.is_empty()
    }
    pub fn evaluation(&self) -> &Evaluation {
        &self.evaluation
    }
    pub fn rule_refusal(&self, event: &EventId) -> Option<&RuleRefusal> {
        self.evaluation.rule_refusals.get(event)
    }
    pub fn state(&self) -> &State {
        &self.evaluation.state
    }
    pub fn selection(&self, key: &locust_proto::event::ScopeKey) -> Option<&ScopedSelection> {
        self.state().selections.get(key)
    }
    /// Read-only accepted task history. Current means the latest selected round
    /// linked by explicit revisions; unrelated branches yield no combined task.
    pub fn selected_task(&self, id: TaskId) -> Option<Task> {
        let rounds: BTreeMap<_, _> = self
            .state()
            .selections
            .values()
            .filter(|selection| selection.context.scope == Scope::Task(id))
            .filter_map(|selection| selection.task.clone())
            .map(|round| (round.context.round, round))
            .collect();
        let mut candidates = Vec::new();
        for candidate in rounds.keys().copied() {
            let mut ancestry = BTreeSet::new();
            let mut current = candidate;
            loop {
                if !ancestry.insert(current) {
                    break;
                }
                let event = self.event(&current)?;
                if let Body::TaskRevised { expected_round, .. } = event.header().body {
                    current = expected_round;
                } else {
                    break;
                }
            }
            if rounds.keys().all(|round| ancestry.contains(round)) {
                candidates.push((candidate, current));
            }
        }
        let [(current_round, created)] = candidates.as_slice() else {
            return None;
        };
        let creator = self.event(created)?.header().author;
        Some(Task {
            id,
            creator,
            created: *created,
            current_round: *current_round,
            rounds,
        })
    }
    /// Rules exposed for one accepted view, without granting ordinary execution.
    pub fn selected_rules<D: DefinitionLookup + ?Sized>(
        &self,
        context: Context,
        definitions: &D,
    ) -> Option<EffectiveRules> {
        self.selection(&locust_proto::event::ScopeKey {
            context,
            purpose: locust_proto::event::DecisionPurpose::Selection,
        })?;
        rules::resolve(&self.history, definitions, context)
            .ok()
            .map(|mut resolved| {
                resolved.effective.roles = self.state().roles.clone();
                resolved.effective.only_member = self.only_member();
                resolved.effective
            })
    }
    pub fn event(&self, id: &EventId) -> Option<&Event> {
        self.history.get(id)
    }
    pub fn holds(&self, id: &EventId) -> bool {
        self.history.get(id).is_some()
    }
    pub fn standing(&self, id: &EventId) -> Option<Standing> {
        self.evaluation.standings.get(id).copied()
    }
    pub fn genesis(&self) -> Option<EventId> {
        self.chain.order.first().copied()
    }
    /// Where a host-chain event sits in chain order.
    pub fn position(&self, id: &EventId) -> Option<usize> {
        self.chain.position(id)
    }
    pub fn authors(&self) -> impl Iterator<Item = &PublicKey> {
        self.history.logs.keys()
    }
    pub fn points(&self, author: &PublicKey) -> &[AuthorPoint] {
        self.history.log(author).map_or(&[], |log| &log.points)
    }
    pub fn usable(&self, author: &PublicKey) -> u64 {
        self.history.log(author).map_or(0, |log| log.usable as u64)
    }
    pub fn fork_point(&self, author: &PublicKey) -> Option<u64> {
        self.history.log(author).and_then(|log| log.fork)
    }
    /// Whether `point` is in the usable prefix of `author`'s log.
    pub fn holds_usable(&self, author: &PublicKey, point: AuthorPoint) -> bool {
        self.history
            .log(author)
            .is_some_and(|log| log.contains_usable(point))
    }
    pub fn frontier(&self) -> Frontier {
        self.history.frontier()
    }
    pub fn frontier_of(&self, author: &PublicKey) -> AuthorFrontier {
        self.history.frontier_of(author)
    }
    pub fn extends(&self, theirs: &AuthorFrontier) -> bool {
        self.history.extends(theirs)
    }
    pub fn screen(&self, events: Vec<Event>) -> Vec<Event> {
        screen::screen(self.id, &self.history, events)
    }
    pub fn next(&self, author: &PublicKey) -> Option<Next> {
        let anchor = self.state().head?;
        let log = self.history.log(author);
        if log.is_some_and(|log| log.fork.is_some() || log.waiting() != 0)
            || (self.state().governance.as_ref() == Some(author)
                && self.evaluation.host_halt.is_some())
        {
            return None;
        }
        Some(Next {
            seq: self.usable(author),
            prev: log.and_then(|log| log.tip()),
            anchor,
            epoch: self.state().epoch,
        })
    }
    pub fn epoch_of(&self, id: &EventId) -> Option<u32> {
        let event = self.event(id)?;
        let anchor = if event.header().body.is_governance() {
            *id
        } else {
            event.header().anchor?
        };
        Some(self.chain.snapshot(&anchor)?.epoch)
    }
    pub fn read_epoch(&self, member: &PublicKey) -> Option<u32> {
        self.state()
            .members
            .get(member)
            .map(|member| member.read_epoch)
    }
    pub fn current_context(&self, scope: Scope) -> Option<Context> {
        let round = match scope {
            Scope::Task(task) => self.state().tasks.get(&task)?.current_round,
            Scope::Goal | Scope::Document(_) => self.state().current_rules?,
            Scope::Workspace => self.state().workspace.as_ref()?.epoch,
        };
        Some(Context { scope, round })
    }
    pub fn task_of(&self, id: &EventId) -> Option<TaskId> {
        let event = self.event(id)?;
        match &event.header().body {
            Body::TaskOpened { .. } => Some(TaskId::Authored(*id)),
            Body::TaskRevised { task, .. } => Some(*task),
            Body::EffectMaterialized { effect }
                if matches!(
                    effect.action,
                    locust_proto::event::EffectAction::OpenTask { .. }
                ) =>
            {
                Some(TaskId::Derived(effect.id(self.id)))
            }
            Body::AttemptReported { attempt, .. } | Body::CancelRequested { attempt } => {
                self.task_of(attempt)
            }
            Body::WorkDeclined { offer } => self.task_of(offer),
            Body::CancelAcknowledged { cancel, .. } => self.task_of(cancel),
            _ => event
                .header()
                .body
                .context()
                .and_then(|context| match context.scope {
                    Scope::Task(task) => Some(task),
                    _ => None,
                }),
        }
    }
    fn only_member(&self) -> Option<PublicKey> {
        let mut members = self
            .state()
            .members
            .values()
            .filter(|member| member.is_active());
        let first = members.next()?.principal;
        members.next().is_none().then_some(first)
    }
    /// Each member's latest effective review of each of `subjects`, from one
    /// verifier and one memo for the whole call.
    pub fn latest_reviews<D: DefinitionLookup + ?Sized>(
        &self,
        subjects: &[EventId],
        definitions: &D,
    ) -> BTreeMap<EventId, BTreeMap<PublicKey, EventId>> {
        if subjects.is_empty() {
            return BTreeMap::new();
        }
        let verifier = fold::Verifier::new(
            &self.history,
            &self.chain,
            definitions,
            self.closure_index.clone(),
        );
        subjects
            .iter()
            .map(|subject| {
                let reviews = self
                    .authors()
                    .filter_map(|member| {
                        verifier
                            .latest_review(*subject, *member, None)
                            .map(|id| (*member, id))
                    })
                    .collect();
                (*subject, reviews)
            })
            .collect()
    }
    pub fn effective_rules<D: DefinitionLookup + ?Sized>(
        &self,
        context: Context,
        definitions: &D,
    ) -> Option<EffectiveRules> {
        if self.standing(&context.round) != Some(Standing::Effective) {
            return None;
        }
        rules::resolve(&self.history, definitions, context)
            .ok()
            .map(|mut resolved| {
                resolved.effective.roles = self.state().roles.clone();
                resolved.effective.only_member = self.only_member();
                resolved.effective
            })
    }
    pub fn eligible<D: DefinitionLookup + ?Sized>(
        &self,
        context: Context,
        selector: &Selector,
        subject_author: Option<PublicKey>,
        definitions: &D,
    ) -> BTreeSet<PublicKey> {
        let Some(rules) = self.effective_rules(context, definitions) else {
            return BTreeSet::new();
        };
        rules::selected(
            selector,
            self.state()
                .members
                .values()
                .filter(|member| member.is_active())
                .map(|member| member.principal),
            &rules,
            subject_author,
        )
    }
    pub fn can_start<D: DefinitionLookup + ?Sized>(
        &self,
        context: Context,
        principal: PublicKey,
        offer: Option<EventId>,
        definitions: &D,
    ) -> bool {
        if !self.state().is_member(&principal) || !self.task_available(context) {
            return false;
        }
        let Some(rules) = self.effective_rules(context, definitions) else {
            return false;
        };
        match offer {
            None=>rules.work.starts.iter().any(|rule|matches!(rule,StartRule::Independent{by} if rules::matches(by,principal,&rules,None))),
            Some(id)=>self.state().offers.get(&id).is_some_and(|offer|offer.context==context&&offer.recipient==principal&&offer.attempts.is_empty()&&offer.declined.is_empty()),
        }
    }
    /// Whether nobody is running an attempt in the current task round.
    /// Hooks use this local-evidence predicate; automatic task selection must
    /// reuse it. This principal's attempts count even when their claim is absent.
    pub fn unattended(&self, task: TaskId) -> bool {
        self.state().tasks.get(&task).is_some_and(|task| {
            let round = &task.rounds[&task.current_round];
            !round.attempts.iter().any(|id| {
                self.state().attempts.get(id).is_some_and(|attempt| {
                    matches!(
                        attempt.status,
                        None | Some(locust_proto::event::AttemptStatus::Progress)
                    )
                })
            })
        })
    }
    /// Current local state permits another offer or attempt on this round.
    /// Replay still decides whether the particular signed event is eligible.
    pub fn task_available(&self, context: Context) -> bool {
        self.state()
            .task_round(context)
            .is_some_and(|round| !round.closed && !round.completed && round.selected.is_none())
    }
    pub fn can_review<D: DefinitionLookup + ?Sized>(
        &self,
        subject: EventId,
        principal: PublicKey,
        definitions: &D,
    ) -> bool {
        let candidate = self
            .state()
            .contributions
            .get(&subject)
            .map(|candidate| (candidate.context, candidate.author))
            .or_else(|| {
                self.state()
                    .revisions
                    .get(&subject)
                    .map(|candidate| (candidate.context, candidate.author))
            })
            .or_else(|| {
                self.state()
                    .workspace_proposals
                    .get(&subject)
                    .map(|candidate| (candidate.context, candidate.author))
            });
        let Some((context, author)) = candidate else {
            return false;
        };
        let Some(rules) = self.effective_rules(context, definitions) else {
            return false;
        };
        let authors = self
            .state()
            .workspace_proposals
            .get(&subject)
            .map(|candidate| candidate.source_authors.clone())
            .unwrap_or_else(|| BTreeSet::from([author]));
        self.state().is_member(&principal)
            && rules::may_review_with_authors(
                &rules.decisions.completion,
                principal,
                &rules,
                author,
                &authors,
            )
    }

    pub fn can_attest<D: DefinitionLookup + ?Sized>(
        &self,
        subject: EventId,
        principal: PublicKey,
        definitions: &D,
    ) -> bool {
        let candidate = self
            .state()
            .contributions
            .get(&subject)
            .map(|c| (c.context, c.author))
            .or_else(|| {
                self.state()
                    .revisions
                    .get(&subject)
                    .map(|c| (c.context, c.author))
            })
            .or_else(|| {
                self.state()
                    .workspace_proposals
                    .get(&subject)
                    .map(|c| (c.context, c.author))
            });
        let Some((context, author)) = candidate else {
            return false;
        };
        let Some(rules) = self.effective_rules(context, definitions) else {
            return false;
        };
        self.state().is_member(&principal)
            && rules::may_attest_any(&rules.decisions.completion, principal, &rules, author)
    }

    /// Informational goal-scope opportunities. Replay remains the authority
    /// for any concrete event and may reject an opportunity for state reasons.
    pub fn abilities<D: DefinitionLookup + ?Sized>(
        &self,
        author: PublicKey,
        level: Level,
        definitions: &D,
    ) -> Vec<Ability> {
        let Some(context) = self.current_context(Scope::Goal) else {
            return Vec::new();
        };
        let Some(effective) = self.effective_rules(context, definitions) else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        let mut add = |rule: Rule, qualifies: Selector, except_author: bool, eligible: bool| {
            let needs = if rule == Rule::Start {
                Level::Auto
            } else {
                Level::Ask
            };
            rows.push(Ability {
                rule,
                qualifies,
                except_author,
                eligible,
                needs,
                allowed: eligible && level >= needs,
            });
        };
        let propose = effective.work.propose.clone();
        add(
            Rule::Propose,
            propose.clone(),
            false,
            rules::matches(&propose, author, &effective, None),
        );
        let publish = effective.work.publish.clone();
        add(
            Rule::Publish,
            publish.clone(),
            false,
            rules::matches(&publish, author, &effective, Some(author)),
        );
        for start in &effective.work.starts {
            match start {
                StartRule::Independent { by } => add(
                    Rule::Start,
                    by.clone(),
                    false,
                    rules::matches(by, author, &effective, None),
                ),
                StartRule::Offered { by, .. } => add(
                    Rule::Offer,
                    by.clone(),
                    false,
                    rules::matches(by, author, &effective, None),
                ),
            }
        }
        for rule in [Rule::Declare, Rule::Review, Rule::Attest] {
            let (qualifies, except_author) =
                rules::completion_qualifies(&effective.decisions.completion, rule, None);
            if qualifies != Selector::Nobody {
                // No concrete result is selected yet. Declaration and named
                // checks can concern this author's own future result; reviews
                // excluding that author require a different candidate.
                let subject = (rule != Rule::Review || !except_author).then_some(author);
                let eligible = rules::matches(&qualifies, author, &effective, subject);
                add(rule, qualifies, except_author, eligible);
            }
        }
        if let Some(authority) = &effective.decisions.selection {
            add(
                Rule::Select,
                rules::qualifies(authority),
                false,
                rules::authority(authority, &effective) == Some(author),
            );
        }
        if let Some(authority) = &effective.decisions.finish {
            add(
                Rule::Finish,
                rules::qualifies(authority),
                false,
                rules::authority(authority, &effective) == Some(author),
            );
        }
        if let Some(workspace) = self
            .current_context(Scope::Workspace)
            .and_then(|context| self.effective_rules(context, definitions))
            && let Some(authority) = &workspace.decisions.selection
        {
            add(
                Rule::Integrate,
                rules::qualifies(authority),
                false,
                rules::authority(authority, &workspace) == Some(author),
            );
        }
        rows
    }
}

#[cfg(test)]
mod tests;
