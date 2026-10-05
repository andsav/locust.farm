//! Configured, deterministic logical effects. Materialization and delivery are
//! separate durable runtime operations, never callbacks from the pure fold.
use std::collections::{BTreeMap, BTreeSet};

use locust_proto::event::{
    Body, Context, DecisionAction, Effect, EffectAction, Event, ReviewVerdict, Scope, TaskBinding,
    TaskId, Trigger,
};
use locust_proto::id::{EffectId, EventId, PublicKey};
use locust_proto::organization::{EvidenceKind, StartRule};

use super::DefinitionLookup;
use super::fold::Verifier;
use super::rules::{self, invalid};
use super::standing::{DesiredEffect, Standing, Waiting};

impl<D: DefinitionLookup + ?Sized> Verifier<'_, D> {
    pub(super) fn effect_recipients(&self, effect: &Effect) -> BTreeSet<PublicKey> {
        match &effect.action {
            EffectAction::OpenTask { recipients, .. } => recipients.iter().copied().collect(),
            EffectAction::Offer { recipient, .. }
            | EffectAction::RequestReview { recipient, .. } => BTreeSet::from([*recipient]),
        }
    }
    fn stage_template(&self, rules: EventId, name: &str) -> Result<(PublicKey, Effect), Standing> {
        let context = Context {
            scope: Scope::Goal,
            round: rules,
        };
        let resolved = self.resolve(context)?;
        let definition = self
            .definitions
            .definition(&resolved.effective.definition)
            .expect("resolved definition exists");
        let stage = definition
            .flow
            .get(name)
            .ok_or(invalid("unknown configured flow stage"))?;
        // The goal's administrator runs every stage.
        let runner = self
            .history
            .administrator
            .ok_or(invalid("goal has no administrator"))?;
        let binding = TaskBinding {
            rules,
            task_type: stage.task_type.clone(),
            inputs: resolved.binding.inputs.clone(),
            parent: Some(context),
            stage: Some(name.to_owned()),
        };
        let effective = rules::resolve_binding(
            self.history,
            self.definitions,
            rules,
            Some(binding.clone()),
            Some(runner),
        )?
        .effective;
        let snapshot = self
            .chain
            .snapshot(&rules)
            .ok_or(Standing::Pending(Waiting::Anchor))?;
        let recipients = rules::selected(
            &stage.recipients,
            snapshot.members.keys().copied(),
            &effective,
            None,
        )
        .into_iter()
        .collect();
        Ok((
            runner,
            Effect {
                context,
                transition: format!("stage:{name}"),
                trigger: Trigger::Stage {
                    rules,
                    stage: name.to_owned(),
                },
                target_slot: name.to_owned(),
                action: EffectAction::OpenTask {
                    binding,
                    recipients,
                },
                evidence: Vec::new(),
            },
        ))
    }
    fn stage_instance(
        &self,
        rules: EventId,
        name: &str,
        proof: Option<EventId>,
    ) -> Result<(EffectId, EventId), Standing> {
        let (_, template) = self.stage_template(rules, name)?;
        let goal = self.event(rules)?.header().goal;
        let id = template.id(goal);
        let event=self.history.events.iter().filter(|event|matches!(&event.header().body,Body::EffectMaterialized{effect} if effect.id(goal)==id)).filter(|event|self.status(event.id(),proof)==Standing::Effective).min_by_key(|event|(event.header().seq,event.id())).ok_or(Standing::Pending(Waiting::Reference))?;
        Ok((id, event.id()))
    }
    fn stage_ready(
        &self,
        rules: EventId,
        name: &str,
        proof: Option<EventId>,
        allowed: Option<&BTreeSet<EventId>>,
        anchor: EventId,
    ) -> Result<Option<BTreeSet<EventId>>, Standing> {
        let resolved = self.resolve(Context {
            scope: Scope::Goal,
            round: rules,
        })?;
        let definition = self
            .definitions
            .definition(&resolved.effective.definition)
            .expect("resolved definition exists");
        let stage = definition
            .flow
            .get(name)
            .ok_or(invalid("unknown configured stage"))?;
        let mut evidence = BTreeSet::new();
        for requirement in &stage.requires {
            let (task, _) = match self.stage_instance(rules, &requirement.stage, proof) {
                Ok(value) => value,
                Err(Standing::Pending(_)) => return Ok(None),
                Err(other) => return Err(other),
            };
            // Resolve the applicable round at the effect governance anchor, so a
            // revised upstream stage is satisfied by its new round rather
            // than the original materialization.
            let task_id = TaskId::Derived(task);
            let round = match self.current_round(task_id, anchor, proof) {
                Ok(round) => round,
                Err(Standing::Pending(_)) => return Ok(None),
                Err(other) => return Err(other),
            };
            let context = Context {
                scope: Scope::Task(task_id),
                round,
            };
            let mut candidates: Vec<_> = self
                .history
                .events
                .iter()
                .filter(|event| allowed.is_none_or(|ids| ids.contains(&event.id())))
                .collect();
            candidates.sort_by_key(|event| event.id());
            let mut found = None;
            for event in candidates {
                let h = event.header();
                let matching = match (&requirement.evidence, &h.body) {
                    (
                        EvidenceKind::Publication,
                        Body::ContributionPublished { context: other, .. },
                    ) => *other == context,
                    (
                        EvidenceKind::Review,
                        Body::ReviewRecorded {
                            context: other,
                            verdict: ReviewVerdict::Approve,
                            ..
                        },
                    ) => *other == context,
                    (
                        EvidenceKind::Completion,
                        Body::ContributionPublished { context: other, .. },
                    ) => *other == context,
                    (
                        EvidenceKind::Selection,
                        Body::ScopeDecided {
                            context: other,
                            action: DecisionAction::Select { .. },
                            ..
                        },
                    ) => *other == context,
                    _ => false,
                };
                if !matching || self.status(event.id(), proof) != Standing::Effective {
                    continue;
                }
                let witness = if requirement.evidence == EvidenceKind::Completion {
                    let Some(witness) = self.approval(event.id(), proof, allowed)? else {
                        continue;
                    };
                    witness
                } else {
                    BTreeSet::from([event.id()])
                };
                found = Some(witness);
                break;
            }
            let Some(witness) = found else {
                return Ok(None);
            };
            evidence.extend(witness);
        }
        Ok(Some(evidence))
    }
    fn review_templates(&self, subject: EventId) -> Result<Vec<(PublicKey, Effect)>, Standing> {
        let event = self.event(subject)?;
        let (Body::ContributionPublished { context, .. } | Body::DocumentRevised { context, .. }) =
            event.header().body
        else {
            return Err(invalid("review trigger is not a contribution"));
        };
        let resolved = self.resolve(context)?;
        let runner =
            if let Some(stage) = resolved.task.as_ref().and_then(|task| task.stage.as_ref()) {
                let definition = self
                    .definitions
                    .definition(&resolved.effective.definition)
                    .expect("resolved definition exists");
                if !definition.flow.contains_key(stage) {
                    return Err(invalid("task names an unknown stage"));
                }
                self.history
                    .administrator
                    .ok_or(invalid("goal has no administrator"))?
            } else {
                event.header().author
            };
        let snapshot = self
            .chain
            .snapshot(&event.header().anchor.unwrap())
            .ok_or(Standing::Pending(Waiting::Anchor))?;
        Ok(snapshot
            .members
            .keys()
            .filter(|principal| {
                rules::may_review(
                    &resolved.effective.decisions.completion,
                    **principal,
                    &resolved.effective,
                    event.header().author,
                )
            })
            .map(|recipient| {
                (
                    runner,
                    Effect {
                        context,
                        transition: "review".into(),
                        trigger: Trigger::Contribution(subject),
                        target_slot: recipient.to_string(),
                        action: EffectAction::RequestReview {
                            context,
                            subject,
                            recipient: *recipient,
                        },
                        evidence: vec![subject],
                    },
                )
            })
            .collect())
    }
    fn offer_templates(
        &self,
        rules: EventId,
        name: &str,
        proof: Option<EventId>,
    ) -> Result<Vec<(PublicKey, Effect)>, Standing> {
        let (runner, template) = self.stage_template(rules, name)?;
        let EffectAction::OpenTask { recipients, .. } = template.action else {
            unreachable!()
        };
        let (task, round) = self.stage_instance(rules, name, proof)?;
        let context = Context {
            scope: Scope::Task(TaskId::Derived(task)),
            round,
        };
        let resolved = self.resolve(context)?;
        Ok(recipients.into_iter().filter(|recipient|resolved.effective.work.starts.iter().any(|rule|matches!(rule,StartRule::Offered{by,to} if rules::matches(by,runner,&resolved.effective,None)&&rules::matches(to,*recipient,&resolved.effective,None)))).map(|recipient|{
            (runner,Effect{context,transition:format!("stage:{name}:offer"),trigger:Trigger::Stage{rules,stage:name.to_owned()},target_slot:recipient.to_string(),action:EffectAction::Offer{context,recipient},evidence:vec![round]})
        }).collect())
    }
    pub(super) fn validate_effect(
        &self,
        event: &Event,
        effect: &Effect,
        proof: Option<EventId>,
    ) -> Result<(), Standing> {
        let (runner, expected) = match &effect.action {
            EffectAction::OpenTask { binding, .. } => {
                let Trigger::Stage { rules, stage } = &effect.trigger else {
                    return Err(invalid(
                        "task materialization requires a configured stage trigger",
                    ));
                };
                if binding.rules != *rules {
                    return Err(invalid("stage binding uses different rules"));
                }
                let (runner, expected) = self.stage_template(*rules, stage)?;
                let allowed = effect.evidence.iter().copied().collect();
                if self
                    .stage_ready(
                        *rules,
                        stage,
                        proof,
                        Some(&allowed),
                        event.header().anchor.unwrap(),
                    )?
                    .is_none()
                {
                    return Err(Standing::Pending(Waiting::Evidence));
                }
                (runner, expected)
            }
            EffectAction::RequestReview { subject, .. } => self
                .review_templates(*subject)?
                .into_iter()
                .find(|(_, candidate)| candidate.action == effect.action)
                .ok_or(invalid("review delivery recipient is not configured"))?,
            EffectAction::Offer { .. } => {
                let Trigger::Stage { rules, stage } = &effect.trigger else {
                    return Err(invalid("automatic offer requires a stage trigger"));
                };
                self.offer_templates(*rules, stage, proof)?
                    .into_iter()
                    .find(|(_, candidate)| candidate.action == effect.action)
                    .ok_or(invalid("automatic offer is not authorized"))?
            }
        };
        if runner != event.header().author {
            return Err(invalid("effect signer is not its configured runner"));
        }
        if expected.context != effect.context
            || expected.transition != effect.transition
            || expected.trigger != effect.trigger
            || expected.target_slot != effect.target_slot
            || expected.action != effect.action
        {
            return Err(invalid("effect differs from its configured action"));
        }
        Ok(())
    }
    pub(super) fn desired_effects(&self) -> BTreeMap<EffectId, DesiredEffect> {
        let mut desired = BTreeMap::new();
        let Some(goal) = self.history.events.first().map(|event| event.header().goal) else {
            return desired;
        };
        let mut insert = |runner: PublicKey, effect: Effect| {
            let id = effect.id(goal);
            if self.history.events.iter().any(|event|matches!(&event.header().body,Body::EffectMaterialized{effect:held} if held.id(goal)==id)&&self.status(event.id(),None)==Standing::Effective){return;}
            desired.insert(
                id,
                DesiredEffect {
                    id,
                    runner,
                    recipients: self.effect_recipients(&effect),
                    effect,
                },
            );
        };
        for event in &self.history.events {
            if self.status(event.id(), None) != Standing::Effective {
                continue;
            }
            match &event.header().body {
                Body::RulesBound { binding, .. } => {
                    let Some(definition) =
                        self.definitions.definition(&binding.definition.semantic)
                    else {
                        continue;
                    };
                    for name in definition.flow.keys() {
                        if let Ok(Some(witness)) = self.stage_ready(
                            event.id(),
                            name,
                            None,
                            None,
                            self.chain.state.head.unwrap(),
                        ) && let Ok((runner, mut effect)) = self.stage_template(event.id(), name)
                        {
                            effect.evidence = witness.into_iter().collect();
                            insert(runner, effect);
                        }
                        if let Ok(offers) = self.offer_templates(event.id(), name, None) {
                            for (runner, effect) in offers {
                                insert(runner, effect);
                            }
                        }
                    }
                }
                Body::ContributionPublished { .. } | Body::DocumentRevised { .. } => {
                    if let Ok(reviews) = self.review_templates(event.id()) {
                        for (runner, effect) in reviews {
                            insert(runner, effect);
                        }
                    }
                }
                _ => {}
            }
        }
        desired
    }
}
