//! One rule/selector interpretation shared by replay and caller opportunities.
use std::collections::{BTreeMap, BTreeSet};

use locust_proto::event::{Body, Context, EffectAction, RulesBinding, Scope, TaskBinding, TaskId};
use locust_proto::id::{DefinitionHash, EventId, PublicKey};
use locust_proto::organization::{Authority, CompletionRule, DecisionRules, Selector, WorkRules};

use super::DefinitionLookup;
use super::history::History;
use super::standing::{Exclusion, Standing, Waiting};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct EffectiveRules {
    pub work: WorkRules,
    pub decisions: DecisionRules,
    pub roles: BTreeMap<String, Vec<PublicKey>>,
    pub creator: Option<PublicKey>,
    pub rules: EventId,
    pub definition: DefinitionHash,
}

#[derive(Clone)]
pub(super) struct Resolved {
    pub effective: EffectiveRules,
    pub binding: RulesBinding,
    pub task: Option<TaskBinding>,
}

pub(super) fn resolve<D: DefinitionLookup + ?Sized>(
    history: &History,
    definitions: &D,
    context: Context,
) -> Result<Resolved, Standing> {
    let event = history
        .get(&context.round)
        .ok_or(Standing::Pending(Waiting::Reference))?;
    let (rules, task, creator) = match context.scope {
        Scope::Goal | Scope::Document(_) => {
            if !matches!(event.header().body, Body::RulesBound { .. }) {
                return Err(invalid("context round is not a rules binding"));
            }
            (context.round, None, None)
        }
        Scope::Workspace => {
            let Body::WorkspaceEpoch { rules, .. } = event.header().body else {
                return Err(invalid("workspace context round is not an epoch"));
            };
            (rules, None, None)
        }
        Scope::Task(task) => match &event.header().body {
            Body::TaskOpened { binding } if task == TaskId::Authored(event.id()) => (
                binding.rules,
                Some(binding.clone()),
                Some(event.header().author),
            ),
            Body::TaskRevised {
                task: target,
                expected_round,
                binding,
            } if *target == task => {
                let creator = task_creator(history, task, *expected_round)?;
                (binding.rules, Some(binding.clone()), Some(creator))
            }
            Body::EffectMaterialized { effect }
                if task == TaskId::Derived(effect.id(event.header().goal)) =>
            {
                let EffectAction::OpenTask { binding, .. } = &effect.action else {
                    return Err(invalid("context effect does not create a task"));
                };
                (
                    binding.rules,
                    Some(binding.clone()),
                    Some(event.header().author),
                )
            }
            _ => return Err(invalid("context round belongs to another task")),
        },
    };
    let mut resolved = resolve_binding(history, definitions, rules, task, creator)?;
    if context.scope == Scope::Workspace {
        let definition = definitions
            .definition(&resolved.effective.definition)
            .expect("resolved definition exists");
        resolved.effective.decisions = definition.workspace.as_ref().map_or_else(
            || DecisionRules {
                completion: CompletionRule::Contribution {
                    by: Selector::Nobody,
                },
                selection: None,
                finish: None,
            },
            |policy| DecisionRules {
                completion: policy.completion.clone(),
                selection: Some(policy.integrator.clone()),
                finish: None,
            },
        );
    }
    Ok(resolved)
}

pub(super) fn resolve_binding<D: DefinitionLookup + ?Sized>(
    history: &History,
    definitions: &D,
    rules: EventId,
    task: Option<TaskBinding>,
    creator: Option<PublicKey>,
) -> Result<Resolved, Standing> {
    let event = history
        .get(&rules)
        .ok_or(Standing::Pending(Waiting::Reference))?;
    let Body::RulesBound { binding, .. } = &event.header().body else {
        return Err(invalid("rules reference has the wrong event kind"));
    };
    let definition = definitions
        .definition(&binding.definition.semantic)
        .ok_or(Standing::Pending(Waiting::Definition))?;
    if !super::valid_definition(&binding.definition.semantic, definition) {
        return Err(Standing::Excluded(Exclusion::InvalidDefinition));
    }
    let inherited = task
        .as_ref()
        .and_then(|task| task.parent)
        .filter(|context| matches!(context.scope, Scope::Task(_)))
        .map(|context| resolve(history, definitions, context))
        .transpose()?;
    let (mut work, mut decisions) = if let Some(parent) = &inherited {
        super::delegation::inherit(&parent.effective)
    } else {
        (definition.work.clone(), definition.decisions.clone())
    };
    if let Some(name) = task.as_ref().and_then(|task| task.task_type.as_ref()) {
        let task_type = definition
            .task_types
            .get(name)
            .ok_or(invalid("task type is not delegated by the definition"))?;
        if let Some(value) = &task_type.work {
            work = value.clone();
        }
        if let Some(value) = &task_type.decisions {
            decisions = value.clone();
        }
    }
    Ok(Resolved {
        effective: EffectiveRules {
            work,
            decisions,
            roles: binding.roles.clone(),
            creator,
            rules,
            definition: binding.definition.semantic,
        },
        binding: binding.clone(),
        task,
    })
}

/// The author of the record that opened `task`, reached from `round` through
/// the rounds it replaced. A revision is effective only while it replaces the
/// current round, so an effective round leads back to the effective opening
/// record; a held copy of that record under another signature is never on the
/// path.
pub(super) fn task_creator(
    history: &History,
    task: TaskId,
    mut round: EventId,
) -> Result<PublicKey, Standing> {
    loop {
        let event = history
            .get(&round)
            .ok_or(Standing::Pending(Waiting::Reference))?;
        let h = event.header();
        match &h.body {
            Body::TaskRevised {
                task: target,
                expected_round,
                ..
            } if *target == task => round = *expected_round,
            Body::TaskOpened { .. } if task == TaskId::Authored(round) => return Ok(h.author),
            Body::EffectMaterialized { effect }
                if task == TaskId::Derived(effect.id(h.goal))
                    && matches!(effect.action, EffectAction::OpenTask { .. }) =>
            {
                return Ok(h.author);
            }
            _ => return Err(invalid("task round descends from another task")),
        }
    }
}

pub(super) fn matches(
    selector: &Selector,
    principal: PublicKey,
    rules: &EffectiveRules,
    subject: Option<PublicKey>,
) -> bool {
    match selector {
        Selector::Members => true,
        Selector::Nobody => false,
        Selector::Role { name } => rules
            .roles
            .get(name)
            .is_some_and(|keys| keys.contains(&principal)),
        Selector::Participant { key } => key.parse::<PublicKey>().ok() == Some(principal),
        Selector::TaskCreator => rules.creator == Some(principal),
        Selector::ContributionAuthor => subject == Some(principal),
        Selector::Any { selectors } => selectors
            .iter()
            .any(|selector| matches(selector, principal, rules, subject)),
    }
}

pub(super) fn selected(
    selector: &Selector,
    members: impl Iterator<Item = PublicKey>,
    rules: &EffectiveRules,
    subject: Option<PublicKey>,
) -> BTreeSet<PublicKey> {
    members
        .filter(|principal| matches(selector, *principal, rules, subject))
        .collect()
}

pub(super) fn authority(authority: &Authority, rules: &EffectiveRules) -> Option<PublicKey> {
    match authority {
        Authority::Participant { key } => key.parse().ok(),
        Authority::Role { name } => {
            let keys = rules.roles.get(name)?;
            if keys.len() == 1 { Some(keys[0]) } else { None }
        }
    }
}

/// Show the authority named by a decision without reinterpreting it.
pub(super) fn qualifies(authority: &Authority) -> Selector {
    match authority {
        Authority::Participant { key } => Selector::Participant { key: key.clone() },
        Authority::Role { name } => Selector::Role { name: name.clone() },
    }
}

pub(super) fn completion_qualifies(
    rule: &CompletionRule,
    kind: locust_proto::api::Rule,
    name: Option<&str>,
) -> (Selector, bool) {
    use locust_proto::api::Rule;
    match rule {
        CompletionRule::Declaration { by } if kind == Rule::Declare => (by.clone(), false),
        CompletionRule::Reviews {
            by, exclude_author, ..
        } if kind == Rule::Review => (by.clone(), *exclude_author),
        CompletionRule::Check { name: check, by }
            if kind == Rule::Attest && name.is_none_or(|name| name == check) =>
        {
            (by.clone(), false)
        }
        CompletionRule::All { rules } | CompletionRule::Any { rules } => {
            let selectors: Vec<_> = rules
                .iter()
                .map(|rule| completion_qualifies(rule, kind, name))
                .filter(|(selector, _)| *selector != Selector::Nobody)
                .collect();
            let except_author = selectors.iter().any(|(_, except)| *except);
            let selectors: Vec<_> = selectors
                .into_iter()
                .map(|(selector, _)| selector)
                .collect();
            if selectors.is_empty() {
                (Selector::Nobody, false)
            } else {
                (Selector::Any { selectors }, except_author)
            }
        }
        _ => (Selector::Nobody, false),
    }
}

pub(super) fn may_review(
    rule: &CompletionRule,
    principal: PublicKey,
    rules: &EffectiveRules,
    subject: PublicKey,
) -> bool {
    may_review_with_authors(rule, principal, rules, subject, &BTreeSet::from([subject]))
}

pub(super) fn may_review_with_authors(
    rule: &CompletionRule,
    principal: PublicKey,
    rules: &EffectiveRules,
    subject: PublicKey,
    authors: &BTreeSet<PublicKey>,
) -> bool {
    match rule {
        CompletionRule::Reviews {
            by, exclude_author, ..
        } => {
            (!exclude_author || !authors.contains(&principal))
                && matches(by, principal, rules, Some(subject))
        }
        CompletionRule::All { rules: items } | CompletionRule::Any { rules: items } => items
            .iter()
            .any(|rule| may_review_with_authors(rule, principal, rules, subject, authors)),
        _ => false,
    }
}

pub(super) fn may_declare(
    rule: &CompletionRule,
    principal: PublicKey,
    rules: &EffectiveRules,
    subject: PublicKey,
) -> bool {
    match rule {
        CompletionRule::Declaration { by } => matches(by, principal, rules, Some(subject)),
        CompletionRule::All { rules: items } | CompletionRule::Any { rules: items } => items
            .iter()
            .any(|rule| may_declare(rule, principal, rules, subject)),
        _ => false,
    }
}

pub(super) fn may_attest(
    rule: &CompletionRule,
    name: &str,
    principal: PublicKey,
    rules: &EffectiveRules,
    subject: PublicKey,
) -> bool {
    match rule {
        CompletionRule::Check { name: expected, by } => {
            name == expected && matches(by, principal, rules, Some(subject))
        }
        CompletionRule::All { rules: items } | CompletionRule::Any { rules: items } => items
            .iter()
            .any(|rule| may_attest(rule, name, principal, rules, subject)),
        _ => false,
    }
}

pub(super) fn may_attest_any(
    rule: &CompletionRule,
    principal: PublicKey,
    rules: &EffectiveRules,
    subject: PublicKey,
) -> bool {
    match rule {
        CompletionRule::Check { name, .. } => may_attest(rule, name, principal, rules, subject),
        CompletionRule::All { rules: items } | CompletionRule::Any { rules: items } => items
            .iter()
            .any(|item| may_attest_any(item, principal, rules, subject)),
        _ => false,
    }
}

pub(super) fn invalid(reason: &'static str) -> Standing {
    Standing::Excluded(Exclusion::Precondition(reason))
}
