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

use locust_proto::event::{AuthorPoint, Body, Context, Event, Scope, TaskId};
use locust_proto::id::{DefinitionHash, EventId, GoalId, PublicKey};
use locust_proto::organization::{Formation, Selector, StartRule};
use locust_proto::store::{Store, StoreError};
use locust_proto::sync::{AuthorFrontier, Frontier};

pub use rules::EffectiveRules;
pub use standing::{
    Changes, Dependency, DesiredEffect, Evaluation, Exclusion, Halt, Next, Standing, Waiting,
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
    pub fn len(&self) -> usize {
        self.history.events.len()
    }
    pub fn is_empty(&self) -> bool {
        self.history.events.is_empty()
    }
    pub fn evaluation(&self) -> &Evaluation {
        &self.evaluation
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
            .map(|resolved| resolved.effective)
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
            || (self.state().administrator.as_ref() == Some(author)
                && self.evaluation.admin_halt.is_some())
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
            .map(|resolved| resolved.effective)
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
        if !self.state().is_member(&principal)
            || self
                .state()
                .task_round(context)
                .is_none_or(|round| round.closed || round.completed || round.selected.is_some())
        {
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
}

#[cfg(test)]
mod tests;
