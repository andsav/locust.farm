//! Dependency-driven validation with scope-local exact proof contexts. Evaluation
//! is pure: no signing, durable writes or delivery happens here.
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use locust_proto::api::Rule;
use locust_proto::event::{
    Body, Context, DecisionAction, DecisionPurpose, EffectAction, Event, ReviewVerdict, Scope,
    ScopeKey, TaskBinding, TaskId,
};
use locust_proto::id::{EventId, PublicKey};
use locust_proto::organization::{CompletionRule, Selector, StartRule};

use super::DefinitionLookup;
use super::chain::Chain;
use super::commitments::{self, Proof};
use super::history::History;
use super::rules::{self, Resolved, invalid};
use super::standing::{Dependency, Evaluation, Halt, RuleRefusal, Standing, Waiting};

type CacheKey = (EventId, Option<EventId>);
type LatestReviews = BTreeMap<(EventId, Option<String>), BTreeMap<PublicKey, EventId>>;
type DecisionPredecessor = (PublicKey, ScopeKey, Option<EventId>);

pub(super) struct Verifier<'a, D: DefinitionLookup + ?Sized> {
    pub history: &'a History,
    pub chain: &'a Chain,
    pub definitions: &'a D,
    memo: RefCell<BTreeMap<CacheKey, Standing>>,
    visiting: RefCell<BTreeSet<CacheKey>>,
    proofs: RefCell<BTreeMap<EventId, Result<Rc<Proof>, Standing>>>,
    resolved: RefCell<BTreeMap<Context, Result<Resolved, Standing>>>,
    pub checkpoint_lineages: RefCell<BTreeMap<EventId, Result<BTreeSet<EventId>, Standing>>>,
    pub missing: RefCell<BTreeSet<Dependency>>,
    pub scope_halts: RefCell<BTreeMap<ScopeKey, Halt>>,
    pub rule_refusals: RefCell<BTreeMap<EventId, RuleRefusal>>,
    witnesses: BTreeMap<EventId, Vec<EventId>>,
    latest_reviews: RefCell<LatestReviews>,
    #[cfg(test)]
    pub(super) latest_review_scans: std::cell::Cell<usize>,
    #[cfg(test)]
    pub(super) rule_resolutions: std::cell::Cell<usize>,
    // Candidates only: authorization and conflicts are still checked each fold.
    decision_successors: BTreeMap<DecisionPredecessor, Vec<EventId>>,
    closure_index: RefCell<commitments::Index>,
}

impl<'a, D: DefinitionLookup + ?Sized> Verifier<'a, D> {
    pub fn new(
        history: &'a History,
        chain: &'a Chain,
        definitions: &'a D,
        mut closure_index: commitments::Index,
    ) -> Self {
        closure_index.refresh(history);
        let mut witnesses = BTreeMap::<EventId, Vec<EventId>>::new();
        let mut decision_successors = BTreeMap::<DecisionPredecessor, Vec<EventId>>::new();
        for event in &history.events {
            if let Body::CompletionDeclared { subject, .. }
            | Body::ReviewRecorded { subject, .. }
            | Body::CheckAttested { subject, .. } = event.header().body
            {
                witnesses.entry(subject).or_default().push(event.id());
            }
            if let Body::ScopeDecided {
                context,
                previous,
                action,
                ..
            } = &event.header().body
            {
                let key = ScopeKey {
                    context: *context,
                    purpose: action.purpose(),
                };
                decision_successors
                    .entry((event.header().author, key, *previous))
                    .or_default()
                    .push(event.id());
            }
        }
        Self {
            witnesses,
            latest_reviews: RefCell::new(BTreeMap::new()),
            #[cfg(test)]
            latest_review_scans: std::cell::Cell::new(0),
            #[cfg(test)]
            rule_resolutions: std::cell::Cell::new(0),
            decision_successors,
            closure_index: RefCell::new(closure_index),
            history,
            chain,
            definitions,
            memo: RefCell::new(BTreeMap::new()),
            visiting: RefCell::new(BTreeSet::new()),
            proofs: RefCell::new(BTreeMap::new()),
            resolved: RefCell::new(BTreeMap::new()),
            checkpoint_lineages: RefCell::new(BTreeMap::new()),
            missing: RefCell::new(chain.missing.clone()),
            scope_halts: RefCell::new(BTreeMap::new()),
            rule_refusals: RefCell::new(BTreeMap::new()),
        }
    }
    fn rule_invalid(
        &self,
        event: EventId,
        rule: Rule,
        qualifies: Selector,
        except_author: bool,
        reason: &'static str,
    ) -> Standing {
        self.rule_refusals.borrow_mut().insert(
            event,
            RuleRefusal {
                rule,
                qualifies,
                except_author,
            },
        );
        invalid(reason)
    }
    pub fn resolve(&self, context: Context, anchor: EventId) -> Result<Resolved, Standing> {
        let snapshot = self
            .chain
            .snapshot(&anchor)
            .ok_or(Standing::Pending(Waiting::Anchor))?;
        let cached = self.resolved.borrow().get(&context).cloned();
        let result = cached.unwrap_or_else(|| {
            #[cfg(test)]
            self.rule_resolutions.set(self.rule_resolutions.get() + 1);
            let result = rules::resolve(self.history, self.definitions, context);
            self.resolved.borrow_mut().insert(context, result.clone());
            result
        });
        result.map(|mut resolved| {
            resolved.effective.roles = snapshot.roles.clone();
            resolved.effective.only_member =
                (snapshot.members.len() == 1).then(|| *snapshot.members.keys().next().unwrap());
            resolved
        })
    }
    pub fn event(&self, id: EventId) -> Result<&Event, Standing> {
        self.history.get(&id).ok_or_else(|| {
            self.missing.borrow_mut().insert(Dependency::Event(id));
            Standing::Pending(Waiting::Reference)
        })
    }
    pub fn require(&self, id: EventId, proof: Option<EventId>) -> Result<(), Standing> {
        match self.status(id, proof) {
            Standing::Effective => Ok(()),
            other => Err(other),
        }
    }
    pub fn proof(&self, id: EventId) -> Result<Rc<Proof>, Standing> {
        if let Some(result) = self.proofs.borrow().get(&id).cloned() {
            return result;
        }
        let event = self.event(id)?;
        let result = commitments::build(
            self.history,
            self.chain,
            &mut self.closure_index.borrow_mut(),
            event,
            &mut self.missing.borrow_mut(),
        )
        .map(Rc::new);
        self.proofs.borrow_mut().insert(id, result.clone());
        result
    }
    pub fn status(&self, id: EventId, proof: Option<EventId>) -> Standing {
        let key = (id, proof);
        if let Some(status) = self.memo.borrow().get(&key).copied() {
            return status;
        }
        if !self.visiting.borrow_mut().insert(key) {
            return invalid("cyclic semantic dependency");
        }
        let status = match self.check(id, proof) {
            Ok(()) => Standing::Effective,
            Err(status) => status,
        };
        self.visiting.borrow_mut().remove(&key);
        self.memo.borrow_mut().insert(key, status);
        status
    }
    fn check(&self, id: EventId, proof: Option<EventId>) -> Result<(), Standing> {
        let event = self.event(id)?;
        let h = event.header();
        // A scoped decision always validates its own authority and its own exact
        // proof. Another scope may not select a branch of this authority for it.
        let checkpoint = proof.filter(|owner| self.checkpoint_contains(*owner, id));
        if matches!(h.body, Body::ScopeDecided { .. }) && proof.is_some() && checkpoint.is_none() {
            return self.require(id, None);
        }
        if let Some(owner) = checkpoint {
            self.checkpoint_signer(event, owner)?;
        }
        let exact = proof.map(|id| self.proof(id)).transpose()?;
        // A later workspace decision may retain its own signer's exact earlier
        // checkpoint branch. This scope-local continuation does not authorize
        // that signer in other scopes or a sibling outside checkpoint ancestry.
        let continuation = if proof.is_none()
            && let Body::ScopeDecided {
                context:
                    Context {
                        scope: Scope::Workspace,
                        round,
                    },
                ..
            } = h.body
            && self.workspace_boundary(round).ok().flatten().is_some()
            && self.checkpoint_signer(event, round).is_ok()
        {
            let mut retained = self.proof(round)?.retained.clone();
            retained.insert(self.history.slot(&id).expect("event is held"));
            Some(retained)
        } else {
            None
        };
        let pins = continuation
            .as_ref()
            .or_else(|| exact.as_ref().map(|proof| &proof.retained));
        let base = self
            .chain
            .authorize(self.history, event, pins, &mut self.missing.borrow_mut());
        if base != Standing::Effective {
            // Same-slot authority equivocation is still a scope conflict even
            // though neither branch belongs to the ordinary usable prefix.
            if base == Standing::Pending(Waiting::ForkProof)
                && matches!(h.body, Body::ScopeDecided { .. })
                && self.decision(event, None) == Err(Standing::Disputed)
            {
                return Err(Standing::Disputed);
            }
            return Err(base);
        }
        if h.body.is_governance()
            && !matches!(
                h.body,
                Body::TaskRevised { .. } | Body::WorkspaceEpoch { .. }
            )
        {
            return Ok(());
        }
        if matches!(h.body, Body::ScopeDecided { .. }) {
            return self.decision(event, checkpoint);
        }
        if matches!(h.body, Body::WorkspaceEpoch { .. }) {
            return self.workspace_epoch(event);
        }
        for dependency in h.body.dependencies() {
            if matches!(h.body, Body::WorkspaceProposed { parent: Some(parent), .. } if dependency == parent)
            {
                continue;
            }
            self.require(dependency, proof)?;
        }
        match &h.body {
            Body::PublicationConsent(consent) => {
                let publication = self.event(consent.publication)?;
                let Body::PublicationSet(set) = &publication.header().body else {
                    return Err(invalid("consent does not name publication policy"));
                };
                if consent.policy_digest != set.policy.digest()
                    || (consent.accept
                        && consent
                            .profile
                            .as_ref()
                            .is_none_or(|p| p.validate().is_err()))
                    || (!consent.accept && consent.profile.is_some())
                {
                    return Err(invalid("invalid publication consent or public profile"));
                }
            }
            Body::TaskOpened { binding } => {
                if binding.stage.is_some() {
                    return Err(invalid("only a configured effect may create a stage task"));
                }
                let resolved = self.task_binding(binding, h.author, proof, h.anchor.unwrap())?;
                if let Some(
                    parent_context @ Context {
                        scope: Scope::Task(_),
                        ..
                    },
                ) = binding.parent
                {
                    self.active_context(parent_context, event, proof)?;
                    let parent = self.resolve(parent_context, h.anchor.unwrap())?;
                    if !rules::matches(
                        &parent.effective.work.propose,
                        h.author,
                        &parent.effective,
                        None,
                    ) {
                        return Err(self.rule_invalid(
                            id,
                            Rule::Propose,
                            parent.effective.work.propose.clone(),
                            false,
                            "principal may not propose under the parent task",
                        ));
                    }
                }
                if !matches!(
                    binding.parent,
                    Some(Context {
                        scope: Scope::Task(_),
                        ..
                    })
                ) && self
                    .chain
                    .snapshot(&h.anchor.unwrap())
                    .and_then(|snapshot| snapshot.rules)
                    != Some(binding.rules)
                {
                    return Err(invalid(
                        "new task does not use defaults at its governance anchor",
                    ));
                }
                if !rules::matches(
                    &resolved.effective.work.propose,
                    h.author,
                    &resolved.effective,
                    None,
                ) {
                    return Err(self.rule_invalid(
                        id,
                        Rule::Propose,
                        resolved.effective.work.propose.clone(),
                        false,
                        "principal may not propose this task",
                    ));
                }
            }
            Body::TaskRevised {
                task,
                expected_round,
                binding,
            } => {
                let current = self.current_round(*task, h.anchor.unwrap(), proof)?;
                if current != *expected_round {
                    return Err(invalid("task round compare-and-swap failed"));
                }
                let creator = rules::task_creator(self.history, *task, *expected_round)?;
                let resolved = self.task_binding(binding, creator, proof, h.anchor.unwrap())?;
                if Some(creator) == self.chain.state.governance
                    && !crate::organization::stage_task_creator_paths(
                        &resolved.effective.work,
                        &resolved.effective.decisions,
                        "/work",
                        "/decisions",
                    )
                    .is_empty()
                {
                    return Err(invalid(
                        "a stage's task is opened by the host's computer; name members, a role or a participant instead of task_creator",
                    ));
                }
            }
            Body::WorkOffered { context, recipient } => {
                self.active_context(*context, event, proof)?;
                let resolved = self.resolve(*context, h.anchor.unwrap())?;
                if !matches!(context.scope, Scope::Task(_)) {
                    return Err(invalid("work offers require a task"));
                }
                if !self.member_at(*recipient, h.anchor.unwrap()) {
                    return Err(invalid("work offer recipient is not a member"));
                }
                let offer_rules: Vec<_> = resolved
                    .effective
                    .work
                    .starts
                    .iter()
                    .filter_map(|rule| match rule {
                        StartRule::Offered { by, to }
                            if rules::matches(to, *recipient, &resolved.effective, None) =>
                        {
                            Some(by.clone())
                        }
                        _ => None,
                    })
                    .collect();
                if !offer_rules
                    .iter()
                    .any(|by| rules::matches(by, h.author, &resolved.effective, None))
                {
                    return Err(self.rule_invalid(
                        id,
                        Rule::Offer,
                        Selector::Any {
                            selectors: offer_rules,
                        },
                        false,
                        "work offer is not authorized by the pinned rules",
                    ));
                }
            }
            Body::AttemptStarted {
                context,
                offer,
                closure,
            } => {
                self.active_context(*context, event, proof)?;
                self.open_at_observed_closure(event, *context, *closure, proof)?;
                if !matches!(context.scope, Scope::Task(_)) {
                    return Err(invalid("attempt requires a task"));
                }
                let resolved = self.resolve(*context, h.anchor.unwrap())?;
                if let Some(offer) = offer {
                    let (offered, recipient) = self.offer(*offer)?;
                    if offered != *context || recipient != h.author {
                        return Err(invalid("attempt does not accept this recipient's offer"));
                    }
                    self.no_prior_offer_answer(*offer, event, proof)?;
                } else {
                    let start_rules: Vec<_> = resolved
                        .effective
                        .work
                        .starts
                        .iter()
                        .filter_map(|rule| match rule {
                            StartRule::Independent { by } => Some(by.clone()),
                            _ => None,
                        })
                        .collect();
                    if !start_rules
                        .iter()
                        .any(|by| rules::matches(by, h.author, &resolved.effective, None))
                    {
                        return Err(self.rule_invalid(
                            id,
                            Rule::Start,
                            Selector::Any {
                                selectors: start_rules,
                            },
                            false,
                            "independent start is not authorized",
                        ));
                    }
                }
            }
            Body::AttemptReported { attempt, .. } => {
                let target = self.event(*attempt)?;
                if !matches!(target.header().body, Body::AttemptStarted { .. })
                    || target.header().author != h.author
                {
                    return Err(invalid("only the attempt author reports its status"));
                }
            }
            Body::WorkDeclined { offer } => {
                let (_, recipient) = self.offer(*offer)?;
                if recipient != h.author {
                    return Err(invalid("only the offer recipient may decline"));
                }
                self.no_prior_offer_answer(*offer, event, proof)?;
            }
            Body::CancelRequested { attempt } => {
                let target = self.event(*attempt)?;
                let Body::AttemptStarted { offer, .. } = &target.header().body else {
                    return Err(invalid("cancellation target is not an attempt"));
                };
                let offerer = offer
                    .and_then(|id| self.history.get(&id))
                    .map(|event| event.header().author);
                if target.header().author != h.author && offerer != Some(h.author) {
                    let mut selectors = vec![Selector::Participant {
                        key: target.header().author.to_string(),
                    }];
                    if let Some(offerer) = offerer {
                        selectors.push(Selector::Participant {
                            key: offerer.to_string(),
                        });
                    }
                    return Err(self.rule_invalid(
                        id,
                        Rule::Cancel,
                        Selector::Any { selectors },
                        false,
                        "only the worker or its offerer may request cancellation",
                    ));
                }
            }
            Body::CancelAcknowledged { cancel, .. } => {
                let Body::CancelRequested { attempt } = &self.event(*cancel)?.header().body else {
                    return Err(invalid("acknowledgment target is not a cancellation"));
                };
                if self.event(*attempt)?.header().author != h.author {
                    return Err(invalid("only the worker acknowledges cancellation"));
                }
            }
            Body::ContributionPublished {
                context, attempt, ..
            } => {
                self.active_context(*context, event, proof)?;
                if matches!(context.scope, Scope::Document(_) | Scope::Workspace) {
                    return Err(invalid(
                        "documents and workspace candidates use their typed events",
                    ));
                }
                let resolved = self.resolve(*context, h.anchor.unwrap())?;
                if !rules::matches(
                    &resolved.effective.work.publish,
                    h.author,
                    &resolved.effective,
                    Some(h.author),
                ) {
                    return Err(self.rule_invalid(
                        id,
                        Rule::Publish,
                        resolved.effective.work.publish.clone(),
                        false,
                        "principal may not publish under this rule",
                    ));
                }
                if let Some(attempt) = attempt {
                    let target = self.event(*attempt)?;
                    if target.header().author != h.author
                        || !matches!(target.header().body,Body::AttemptStarted{context:attempt_context,..} if attempt_context==*context)
                    {
                        return Err(invalid(
                            "contribution does not belong to this author's attempt and round",
                        ));
                    }
                }
            }
            Body::WorkspaceProposed {
                context,
                parent,
                sources,
                ..
            } => {
                if context.scope != Scope::Workspace {
                    return Err(invalid("workspace proposal requires workspace scope"));
                }
                self.active_context(*context, event, proof)?;
                let resolved = self.resolve(*context, h.anchor.unwrap())?;
                if resolved.effective.decisions.selection.is_none() {
                    return Err(invalid("workspace writes are disabled in this epoch"));
                }
                if !rules::matches(
                    &resolved.effective.work.publish,
                    h.author,
                    &resolved.effective,
                    Some(h.author),
                ) {
                    return Err(self.rule_invalid(
                        id,
                        Rule::Publish,
                        resolved.effective.work.publish.clone(),
                        false,
                        "principal may not publish workspace proposals",
                    ));
                }
                self.workspace_parent(*context, *parent, proof)?;
                let mut distinct = BTreeSet::new();
                if sources.iter().any(|source| !distinct.insert(*source)) {
                    return Err(invalid("workspace source proposals must be distinct"));
                }
                self.source_authors(id)?;
            }
            Body::CompletionDeclared { context, subject } => {
                let author = self.subject(*subject, *context, h.anchor.unwrap())?;
                let resolved = self.resolve(*context, h.anchor.unwrap())?;
                if !rules::may_declare(
                    &resolved.effective.decisions.completion,
                    h.author,
                    &resolved.effective,
                    author,
                ) {
                    let (qualifies, except_author) = rules::completion_qualifies(
                        &resolved.effective.decisions.completion,
                        Rule::Declare,
                        None,
                    );
                    return Err(self.rule_invalid(
                        id,
                        Rule::Declare,
                        qualifies,
                        except_author,
                        "principal may not declare this candidate complete",
                    ));
                }
            }
            Body::ReviewRecorded {
                context, subject, ..
            } => {
                let author = self.subject(*subject, *context, h.anchor.unwrap())?;
                let resolved = self.resolve(*context, h.anchor.unwrap())?;
                if rules::asks_for_review(&resolved.effective.decisions.completion)
                    && !rules::may_review_with_authors(
                        &resolved.effective.decisions.completion,
                        h.author,
                        &resolved.effective,
                        author,
                        &self.source_authors(*subject)?,
                    )
                {
                    let (qualifies, except_author) = rules::completion_qualifies(
                        &resolved.effective.decisions.completion,
                        Rule::Review,
                        None,
                    );
                    return Err(self.rule_invalid(
                        id,
                        Rule::Review,
                        qualifies,
                        except_author,
                        "reviewer is not eligible for this exact candidate",
                    ));
                }
            }
            Body::CheckAttested {
                context,
                subject,
                name,
                ..
            } => {
                let author = self.subject(*subject, *context, h.anchor.unwrap())?;
                let resolved = self.resolve(*context, h.anchor.unwrap())?;
                if !rules::may_attest(
                    &resolved.effective.decisions.completion,
                    name,
                    h.author,
                    &resolved.effective,
                    author,
                ) {
                    let (qualifies, except_author) = rules::completion_qualifies(
                        &resolved.effective.decisions.completion,
                        Rule::Attest,
                        Some(name),
                    );
                    return Err(self.rule_invalid(
                        id,
                        Rule::Attest,
                        qualifies,
                        except_author,
                        "attestor or check name is not authorized",
                    ));
                }
            }
            Body::DocumentRevised { context, doc, base } => {
                if context.scope != Scope::Document(*doc) {
                    return Err(invalid("document context names another document"));
                }
                self.active_context(*context, event, proof)?;
                let resolved = self.resolve(*context, h.anchor.unwrap())?;
                if !rules::matches(
                    &resolved.effective.work.publish,
                    h.author,
                    &resolved.effective,
                    Some(h.author),
                ) {
                    return Err(self.rule_invalid(
                        id,
                        Rule::Publish,
                        resolved.effective.work.publish.clone(),
                        false,
                        "principal may not publish document revisions",
                    ));
                }
                if let Some(base) = base
                    && !matches!(self.event(*base)?.header().body,Body::DocumentRevised{doc:prior,..} if prior==*doc)
                {
                    return Err(invalid("document base belongs to another document"));
                }
            }
            Body::EffectMaterialized { effect } => {
                self.validate_effect(event, effect, proof)?;
            }
            Body::DeliveryAcknowledged { effect } => {
                let mut found = false;
                let mut entitled = false;
                for materialized in &self.history.events {
                    if let Body::EffectMaterialized { effect: body } = &materialized.header().body
                        && body.id(h.goal) == *effect
                        && self.status(materialized.id(), proof) == Standing::Effective
                    {
                        found = true;
                        entitled |= self.effect_recipients(body).contains(&h.author);
                    }
                }
                if !found {
                    return Err(Standing::Pending(Waiting::Reference));
                }
                if !entitled {
                    return Err(invalid("delivery acknowledgment signer is not a recipient"));
                }
            }
            Body::LeaveRequested { admission } => {
                if self
                    .chain
                    .tenure_at(&h.author, h.anchor.unwrap())
                    .map(|tenure| tenure.admission)
                    != Some(*admission)
                {
                    return Err(invalid(
                        "leave request does not name the author's admission",
                    ));
                }
            }
            Body::Genesis(_)
            | Body::PublicationSet(_)
            | Body::MemberAdmitted { .. }
            | Body::MemberRemoved { .. }
            | Body::RoleHolders { .. }
            | Body::RulesBound { .. }
            | Body::WorkspaceEpoch { .. }
            | Body::ScopeDecided { .. } => unreachable!(),
        }
        Ok(())
    }
    pub fn member_at(&self, principal: PublicKey, anchor: EventId) -> bool {
        self.chain
            .snapshot(&anchor)
            .is_some_and(|snapshot| snapshot.members.contains_key(&principal))
    }
    fn task_binding(
        &self,
        binding: &TaskBinding,
        creator: PublicKey,
        proof: Option<EventId>,
        anchor: EventId,
    ) -> Result<Resolved, Standing> {
        self.require(binding.rules, proof)?;
        let snapshot = self
            .chain
            .snapshot(&anchor)
            .ok_or(Standing::Pending(Waiting::Anchor))?;
        let mut resolved = rules::resolve_binding(
            self.history,
            self.definitions,
            binding.rules,
            Some(binding.clone()),
            Some(creator),
        )?;
        resolved.effective.roles = snapshot.roles.clone();
        resolved.effective.only_member =
            (snapshot.members.len() == 1).then(|| *snapshot.members.keys().next().unwrap());
        if let Some(parent) = binding.parent {
            self.require(parent.round, proof)?;
            let parent_context = parent;
            let parent = self.resolve(parent_context, anchor)?;
            if parent.effective.rules != binding.rules {
                return Err(invalid("child task cannot replace the parent definition"));
            }
            if matches!(parent_context.scope, Scope::Task(_))
                && !super::delegation::narrows(&resolved.effective, &parent.effective)
            {
                return Err(invalid(
                    "child task type does not prove narrower parent authority and completion",
                ));
            }
        }
        let definition = self
            .definitions
            .definition(&resolved.effective.definition)
            .expect("resolved definition exists");
        if binding
            .inputs
            .keys()
            .any(|name| !definition.context.inputs.contains_key(name))
            || definition
                .context
                .inputs
                .iter()
                .any(|(name, input)| input.required && !binding.inputs.contains_key(name))
        {
            return Err(invalid(
                "task inputs do not match required definition inputs",
            ));
        }
        Ok(resolved)
    }
    pub fn subject(
        &self,
        id: EventId,
        context: Context,
        anchor: EventId,
    ) -> Result<PublicKey, Standing> {
        let event = self.event(id)?;
        if !matches!(event.header().body,Body::ContributionPublished{context:subject,..}|Body::DocumentRevised{context:subject,..}|Body::WorkspaceProposed{context:subject,..} if subject==context)
        {
            return Err(invalid(
                "evidence subject is not a contribution in this exact round",
            ));
        }
        if self.chain.position(&anchor)
            < event
                .header()
                .anchor
                .and_then(|anchor| self.chain.position(&anchor))
        {
            return Err(invalid("evidence is anchored before its subject"));
        }
        Ok(event.header().author)
    }
    pub fn offer(&self, id: EventId) -> Result<(Context, PublicKey), Standing> {
        match &self.event(id)?.header().body {
            Body::WorkOffered { context, recipient } => Ok((*context, *recipient)),
            Body::EffectMaterialized { effect } => match effect.action {
                EffectAction::Offer { context, recipient } => Ok((context, recipient)),
                _ => Err(invalid("effect is not a work offer")),
            },
            _ => Err(invalid("reference is not a work offer")),
        }
    }
    fn no_prior_offer_answer(
        &self,
        offer: EventId,
        event: &Event,
        proof: Option<EventId>,
    ) -> Result<(), Standing> {
        for prior in &self.history.events {
            if prior.header().author != event.header().author
                || prior.header().seq >= event.header().seq
            {
                continue;
            }
            if matches!(prior.header().body,Body::WorkDeclined{offer:id}|Body::AttemptStarted{offer:Some(id),..} if id==offer)
                && self.status(prior.id(), proof) == Standing::Effective
            {
                return Err(invalid("offer was already answered by this recipient"));
            }
        }
        Ok(())
    }
    pub fn current_round(
        &self,
        task: TaskId,
        anchor: EventId,
        proof: Option<EventId>,
    ) -> Result<EventId, Standing> {
        let initial=match task{
            TaskId::Authored(id)=>{self.require(id,proof)?;if !matches!(self.event(id)?.header().body,Body::TaskOpened{..}){return Err(invalid("task identifier has the wrong kind"));}id}
            TaskId::Derived(id)=>{
                self.history.events.iter().filter(|event|matches!(&event.header().body,Body::EffectMaterialized{effect} if effect.id(event.header().goal)==id&&matches!(effect.action,EffectAction::OpenTask{..}))).filter(|event|self.status(event.id(),proof)==Standing::Effective).min_by_key(|event|(event.header().seq,event.id())).map(Event::id).ok_or(Standing::Pending(Waiting::Reference))?
            }
        };
        let mut current = initial;
        let position = self
            .chain
            .position(&anchor)
            .ok_or(Standing::Pending(Waiting::Anchor))?;
        for id in self.chain.order.iter().take(position + 1) {
            let event = self.history.get(id).expect("chain event exists");
            if matches!(event.header().body,Body::TaskRevised{task:target,expected_round,..} if target==task&&expected_round==current)
                && self.status(*id, proof) == Standing::Effective
            {
                current = *id;
            }
        }
        Ok(current)
    }
    fn active_context(
        &self,
        context: Context,
        event: &Event,
        proof: Option<EventId>,
    ) -> Result<(), Standing> {
        let anchor = event.header().anchor.unwrap();
        match context.scope {
            Scope::Workspace => {
                if self
                    .chain
                    .snapshot(&anchor)
                    .and_then(|snapshot| snapshot.workspace_epoch)
                    != Some(context.round)
                {
                    return Err(invalid(
                        "workspace context is fenced at its governance anchor",
                    ));
                }
            }
            Scope::Task(task) => {
                if self.current_round(task, anchor, proof)? != context.round {
                    return Err(invalid(
                        "work names a superseded task round at its governance anchor",
                    ));
                }
            }
            Scope::Goal | Scope::Document(_) => {
                if self
                    .chain
                    .snapshot(&anchor)
                    .and_then(|snapshot| snapshot.rules)
                    != Some(context.round)
                {
                    return Err(invalid(
                        "work does not name rules current at its governance anchor",
                    ));
                }
            }
        }
        Ok(())
    }
    fn decision(&self, event: &Event, checkpoint: Option<EventId>) -> Result<(), Standing> {
        let Body::ScopeDecided {
            context,
            previous,
            action,
            evidence,
        } = &event.header().body
        else {
            unreachable!()
        };
        let key = ScopeKey {
            context: *context,
            purpose: action.purpose(),
        };
        let resolved = self.resolve(*context, event.header().anchor.unwrap())?;
        if context.scope == Scope::Workspace {
            self.active_context(*context, event, checkpoint)?;
            if !matches!(action, DecisionAction::Select { .. }) {
                return Err(invalid(
                    "workspace epochs manage enablement; closure decisions are invalid",
                ));
            }
        }
        let nominated = match action.purpose() {
            DecisionPurpose::Selection => resolved.effective.decisions.selection.as_ref(),
            DecisionPurpose::Closure => resolved.effective.decisions.finish.as_ref(),
        };
        let authority =
            nominated.and_then(|authority| rules::authority(authority, &resolved.effective));
        if authority != Some(event.header().author) {
            let rule = if context.scope == Scope::Workspace {
                Rule::Integrate
            } else if action.purpose() == DecisionPurpose::Selection {
                Rule::Select
            } else {
                Rule::Finish
            };
            let qualifies = nominated.map(rules::qualifies).unwrap_or(Selector::Nobody);
            return Err(self.rule_invalid(
                event.id(),
                rule,
                qualifies,
                false,
                "principal is not the named scope authority",
            ));
        }
        let successors: Vec<_> = self
            .decision_successors
            .get(&(event.header().author, key, *previous))
            .into_iter()
            .flatten()
            .copied()
            .filter(|id| {
                let candidate = self.history.get(id).expect("indexed decision exists");
                matches!(
                    self.chain.authorize(
                        self.history,
                        candidate,
                        None,
                        &mut self.missing.borrow_mut()
                    ),
                    Standing::Effective | Standing::Pending(Waiting::ForkProof)
                )
            })
            .collect();
        if checkpoint.is_none() && successors.len() > 1 {
            let mut events = successors;
            events.sort();
            self.scope_halts.borrow_mut().insert(
                key,
                Halt::Successors {
                    previous: *previous,
                    events,
                },
            );
            return Err(Standing::Disputed);
        }
        if let Some(previous) = previous {
            let prior = self.event(*previous)?;
            if !matches!(&prior.header().body,Body::ScopeDecided{context:other,action:prior_action,..} if *other==*context&&prior_action.purpose()==action.purpose())
            {
                return Err(invalid("decision predecessor belongs to another stream"));
            }
            self.require(*previous, checkpoint)?;
        }
        let proof = self.proof(event.id()).inspect_err(|standing| {
            if *standing == Standing::Disputed {
                self.scope_halts
                    .borrow_mut()
                    .insert(key, Halt::IncompatibleProof { event: event.id() });
            }
        })?;
        let proof_owner = checkpoint.or(Some(event.id()));
        self.require(context.round, proof_owner)?;
        for root in evidence {
            self.require(*root, proof_owner)?;
        }
        match action {
            DecisionAction::Select { subject } => {
                self.require(*subject, proof_owner)?;
                self.subject(*subject, *context, event.header().anchor.unwrap())?;
                if context.scope == Scope::Workspace {
                    let Body::WorkspaceProposed { parent, .. } =
                        self.event(*subject)?.header().body
                    else {
                        unreachable!()
                    };
                    if parent != previous.or(self.workspace_boundary(context.round)?) {
                        return Err(invalid(
                            "workspace selection parent does not match predecessor or epoch checkpoint",
                        ));
                    }
                }
                if let Scope::Document(doc) = context.scope
                    && !matches!(self.event(*subject)?.header().body,Body::DocumentRevised{doc:target,..} if target==doc)
                {
                    return Err(invalid("selection subject belongs to another document"));
                }
                if self
                    .approval(*subject, proof_owner, Some(&proof.roots))?
                    .is_none()
                {
                    return Err(Standing::Pending(Waiting::Evidence));
                }
            }
            DecisionAction::Close | DecisionAction::Reopen => {}
        }
        Ok(())
    }
    pub fn approval(
        &self,
        subject: EventId,
        proof: Option<EventId>,
        allowed: Option<&BTreeSet<EventId>>,
    ) -> Result<Option<BTreeSet<EventId>>, Standing> {
        self.require(subject, proof)?;
        let event = self.event(subject)?;
        let (Body::ContributionPublished { context, .. }
        | Body::DocumentRevised { context, .. }
        | Body::WorkspaceProposed { context, .. }) = event.header().body
        else {
            return Err(invalid("completion subject is not a contribution"));
        };
        if matches!(&event.header().body, Body::WorkspaceProposed { parent: None, sources, .. } if sources.is_empty())
            && Some(event.header().author) == self.history.host
        {
            return Ok(Some(BTreeSet::from([subject])));
        }
        let resolved = self.resolve(context, event.header().anchor.unwrap())?;
        self.predicate(
            &resolved.effective.decisions.completion,
            subject,
            context,
            &resolved,
            proof,
            allowed,
        )
    }
    /// Latest effective review, or attestation of a named check, in this member's log.
    pub fn latest_review(
        &self,
        subject: EventId,
        member: PublicKey,
        check: Option<&str>,
    ) -> Option<EventId> {
        let key = (subject, check.map(str::to_owned));
        if !self.latest_reviews.borrow().contains_key(&key) {
            #[cfg(test)]
            self.latest_review_scans
                .set(self.latest_review_scans.get() + 1);
            let mut latest = BTreeMap::new();
            for event in self
                .witnesses
                .get(&subject)
                .into_iter()
                .flatten()
                .filter_map(|id| self.history.get(id))
            {
                let matches = match (&event.header().body, check) {
                    (Body::ReviewRecorded { .. }, None) => true,
                    (Body::CheckAttested { name, .. }, Some(check)) => name == check,
                    _ => false,
                };
                if matches && self.status(event.id(), None) == Standing::Effective {
                    let candidate = (event.header().seq, event.id());
                    latest
                        .entry(event.header().author)
                        .and_modify(|held| *held = candidate.max(*held))
                        .or_insert(candidate);
                }
            }
            self.latest_reviews.borrow_mut().insert(
                key.clone(),
                latest
                    .into_iter()
                    .map(|(author, (_, id))| (author, id))
                    .collect(),
            );
        }
        self.latest_reviews
            .borrow()
            .get(&key)
            .and_then(|latest| latest.get(&member))
            .copied()
    }
    fn predicate(
        &self,
        rule: &CompletionRule,
        subject: EventId,
        context: Context,
        resolved: &Resolved,
        proof: Option<EventId>,
        allowed: Option<&BTreeSet<EventId>>,
    ) -> Result<Option<BTreeSet<EventId>>, Standing> {
        let author = self.event(subject)?.header().author;
        let authors = self.source_authors(subject)?;
        let mut found = BTreeSet::from([subject]);
        match rule {
            CompletionRule::Contribution { by } => {
                return Ok(
                    rules::matches(by, author, &resolved.effective, Some(author)).then_some(found),
                );
            }
            CompletionRule::All { rules } => {
                for rule in rules {
                    let Some(evidence) =
                        self.predicate(rule, subject, context, resolved, proof, allowed)?
                    else {
                        return Ok(None);
                    };
                    found.extend(evidence);
                }
                return Ok(Some(found));
            }
            CompletionRule::Any { rules } => {
                for rule in rules {
                    if let Some(evidence) =
                        self.predicate(rule, subject, context, resolved, proof, allowed)?
                    {
                        return Ok(Some(evidence));
                    }
                }
                return Ok(None);
            }
            _ => {}
        }
        let mut matches = BTreeMap::<PublicKey, EventId>::new();
        for id in self.witnesses.get(&subject).into_iter().flatten() {
            let candidate = self.history.get(id).expect("indexed witness exists");
            if allowed.is_some_and(|ids| !ids.contains(&candidate.id())) {
                continue;
            }
            let principal = candidate.header().author;
            if allowed.is_none() {
                let latest = match rule {
                    CompletionRule::Reviews { .. } => self.latest_review(subject, principal, None),
                    CompletionRule::Check { name, .. } => {
                        self.latest_review(subject, principal, Some(name))
                    }
                    _ => Some(candidate.id()),
                };
                if latest != Some(candidate.id()) {
                    continue;
                }
            }
            // A record that is not effective is no evidence, and the host
            // record it names may not be held, so its rules are never read.
            if self.status(candidate.id(), proof) != Standing::Effective {
                continue;
            }
            let resolved = self.resolve(context, candidate.header().anchor.unwrap())?;
            let eligible = match (rule, &candidate.header().body) {
                (
                    CompletionRule::Declaration { by },
                    Body::CompletionDeclared {
                        context: other,
                        subject: id,
                    },
                ) => {
                    *other == context
                        && *id == subject
                        && rules::matches(by, principal, &resolved.effective, Some(author))
                }
                (
                    CompletionRule::Reviews {
                        by, exclude_author, ..
                    },
                    Body::ReviewRecorded {
                        context: other,
                        subject: id,
                        verdict: ReviewVerdict::Approve,
                    },
                ) => {
                    *other == context
                        && *id == subject
                        && (!exclude_author || !authors.contains(&principal))
                        && rules::matches(by, principal, &resolved.effective, Some(author))
                }
                (
                    CompletionRule::Check { name, by },
                    Body::CheckAttested {
                        context: other,
                        subject: id,
                        name: check,
                        passed: true,
                    },
                ) => {
                    *other == context
                        && *id == subject
                        && check == name
                        && rules::matches(by, principal, &resolved.effective, Some(author))
                }
                _ => false,
            };
            if eligible {
                matches
                    .entry(principal)
                    .and_modify(|id| *id = (*id).min(candidate.id()))
                    .or_insert(candidate.id());
            }
        }
        let count = if let CompletionRule::Reviews { count, .. } = rule {
            *count as usize
        } else {
            1
        };
        if matches.len() < count {
            return Ok(None);
        }
        found.extend(matches.into_values().take(count));
        Ok(Some(found))
    }
}

pub(super) fn evaluate<D: DefinitionLookup + ?Sized>(
    history: &History,
    chain: &Chain,
    definitions: &D,
    closure_index: commitments::Index,
) -> (Evaluation, commitments::Index) {
    let verifier = Verifier::new(history, chain, definitions, closure_index);
    let mut evaluation = Evaluation {
        state: chain.state.clone(),
        host_halt: chain.halt.clone(),
        ..Evaluation::default()
    };
    for event in &history.events {
        evaluation
            .standings
            .insert(event.id(), verifier.status(event.id(), None));
    }
    super::projection::project(&verifier, &mut evaluation);
    evaluation.desired_effects = verifier.desired_effects(&evaluation.state);
    evaluation.rule_refusals = verifier.rule_refusals.into_inner();
    evaluation.scope_halts = verifier.scope_halts.into_inner();
    evaluation.missing = verifier.missing.into_inner();
    evaluation.retained.extend(chain.order.iter().copied());
    let mut retained = commitments::EventSet::new(history.events.len());
    for (id, result) in verifier.proofs.into_inner() {
        if evaluation
            .standings
            .get(&id)
            .is_some_and(|standing| matches!(standing, Standing::Effective | Standing::Pending(_)))
            && let Ok(proof) = result
        {
            retained.extend(&proof.retained);
        }
    }
    evaluation
        .retained
        .extend(retained.slots().map(|slot| history.events[slot].id()));
    (evaluation, verifier.closure_index.into_inner())
}
