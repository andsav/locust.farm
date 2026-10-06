use locust_proto::id::PublicKey;
use locust_proto::organization::*;
use serde::Serialize;

fn set<T: Serialize + PartialEq>(values: &mut Vec<T>) {
    values.sort_by_cached_key(|value| {
        locust_proto::codec::encode(value).expect("encodable definition")
    });
    values.dedup();
}
fn selector(value: &mut Selector) {
    match value {
        Selector::Participant { key } => {
            if let Ok(parsed) = key.parse::<PublicKey>() {
                *key = parsed.to_string();
            }
        }
        Selector::Any { selectors } => {
            for value in selectors.iter_mut() {
                selector(value);
            }
            let mut flattened = Vec::new();
            for value in std::mem::take(selectors) {
                if let Selector::Any { selectors } = value {
                    flattened.extend(selectors);
                } else {
                    flattened.push(value);
                }
            }
            set(&mut flattened);
            if flattened.len() == 1 {
                *value = flattened.pop().expect("one selector");
            } else {
                *selectors = flattened;
            }
        }
        _ => {}
    }
}
fn authority(value: &mut Authority) {
    if let Authority::Participant { key } = value
        && let Ok(parsed) = key.parse::<PublicKey>()
    {
        *key = parsed.to_string();
    }
}
fn work(value: &mut WorkRules) {
    selector(&mut value.propose);
    selector(&mut value.publish);
    for start in &mut value.starts {
        match start {
            StartRule::Independent { by } => selector(by),
            StartRule::Offered { by, to } => {
                selector(by);
                selector(to);
            }
        }
    }
    set(&mut value.starts);
}
fn completion(value: &mut CompletionRule) {
    match value {
        CompletionRule::Contribution { by }
        | CompletionRule::Declaration { by }
        | CompletionRule::Reviews { by, .. }
        | CompletionRule::Check { by, .. } => selector(by),
        CompletionRule::All { rules } | CompletionRule::Any { rules } => {
            for rule in rules.iter_mut() {
                completion(rule);
            }
            set(rules);
            if rules.len() == 1 {
                *value = rules.pop().expect("one criterion");
            }
        }
    }
}
fn decisions(value: &mut DecisionRules) {
    completion(&mut value.completion);
    if let Some(value) = &mut value.selection {
        authority(value);
    }
    if let Some(value) = &mut value.finish {
        authority(value);
    }
}
pub(super) fn normalize(value: &mut Formation) {
    work(&mut value.work);
    decisions(&mut value.decisions);
    if let Some(workspace) = &mut value.workspace {
        authority(&mut workspace.integrator);
        completion(&mut workspace.completion);
    }
    for task_type in value.task_types.values_mut() {
        let rules = task_type.work.get_or_insert_with(|| value.work.clone());
        work(rules);
        let rules = task_type
            .decisions
            .get_or_insert_with(|| value.decisions.clone());
        decisions(rules);
    }
    for stage in value.flow.values_mut() {
        selector(&mut stage.recipients);
        set(&mut stage.requires);
    }
}
