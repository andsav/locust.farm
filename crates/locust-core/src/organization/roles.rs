//! Role duties are derived from the rule slots that actually name a role.
use locust_proto::organization::{
    Authority, CompletionRule, DecisionRules, Formation, Selector, StartRule, WorkRules,
};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RoleDuty {
    Propose,
    Start,
    Offer,
    Receive,
    Publish,
    Declare,
    Review,
    Attest,
    Pick,
    Close,
}

pub fn is_authority_role(formation: &Formation, role: &str) -> bool {
    super::validation::references(formation)
        .authorities
        .contains(role)
}

pub fn role_duties(formation: &Formation, role: &str) -> BTreeSet<RoleDuty> {
    fn names(selector: &Selector, role: &str) -> bool {
        match selector {
            Selector::Role { name } => name == role,
            Selector::Any { selectors } => selectors.iter().any(|s| names(s, role)),
            _ => false,
        }
    }
    fn add(set: &mut BTreeSet<RoleDuty>, selector: &Selector, role: &str, duty: RoleDuty) {
        if names(selector, role) {
            set.insert(duty);
        }
    }
    fn work(set: &mut BTreeSet<RoleDuty>, rules: &WorkRules, role: &str) {
        add(set, &rules.propose, role, RoleDuty::Propose);
        add(set, &rules.publish, role, RoleDuty::Publish);
        for start in &rules.starts {
            match start {
                StartRule::Independent { by } => add(set, by, role, RoleDuty::Start),
                StartRule::Offered { by, to } => {
                    add(set, by, role, RoleDuty::Offer);
                    add(set, to, role, RoleDuty::Receive);
                }
            }
        }
    }
    fn completion(set: &mut BTreeSet<RoleDuty>, rule: &CompletionRule, role: &str) {
        match rule {
            CompletionRule::Contribution { by } => add(set, by, role, RoleDuty::Publish),
            CompletionRule::Declaration { by } => add(set, by, role, RoleDuty::Declare),
            CompletionRule::Reviews { by, .. } => add(set, by, role, RoleDuty::Review),
            CompletionRule::Check { by, .. } => add(set, by, role, RoleDuty::Attest),
            CompletionRule::All { rules } | CompletionRule::Any { rules } => {
                for rule in rules {
                    completion(set, rule, role);
                }
            }
        }
    }
    fn authority(set: &mut BTreeSet<RoleDuty>, value: &Authority, role: &str, duty: RoleDuty) {
        if matches!(value, Authority::Role { name } if name == role) {
            set.insert(duty);
        }
    }
    fn decisions(set: &mut BTreeSet<RoleDuty>, rules: &DecisionRules, role: &str) {
        completion(set, &rules.completion, role);
        if let Some(value) = &rules.selection {
            authority(set, value, role, RoleDuty::Pick);
        }
        if let Some(value) = &rules.finish {
            authority(set, value, role, RoleDuty::Close);
        }
    }
    let mut duties = BTreeSet::new();
    work(&mut duties, &formation.work, role);
    decisions(&mut duties, &formation.decisions, role);
    for task in formation.task_types.values() {
        if let Some(value) = &task.work {
            work(&mut duties, value, role);
        }
        if let Some(value) = &task.decisions {
            decisions(&mut duties, value, role);
        }
    }
    for stage in formation.flow.values() {
        add(&mut duties, &stage.recipients, role, RoleDuty::Receive);
    }
    if let Some(value) = &formation.workspace {
        completion(&mut duties, &value.completion, role);
        authority(&mut duties, &value.integrator, role, RoleDuty::Pick);
    }
    duties
}
