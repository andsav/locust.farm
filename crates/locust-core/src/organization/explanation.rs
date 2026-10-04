use locust_proto::organization::*;

use super::{Explanation, validation};

fn selector(value: &Selector) -> String {
    match value {
        Selector::Members => "goal members".into(),
        Selector::Role { name } => format!("members bound to role {name:?}"),
        Selector::Participant { key } => format!("participant {key}"),
        Selector::TaskCreator => "the task creator".into(),
        Selector::ContributionAuthor => "the contribution author".into(),
        Selector::Any { selectors } => selectors
            .iter()
            .map(selector)
            .collect::<Vec<_>>()
            .join(" or "),
        Selector::Nobody => "nobody".into(),
    }
}
fn authority(value: &Authority) -> String {
    match value {
        Authority::Role { name } => format!("the single member bound to role {name:?}"),
        Authority::Participant { key } => format!("participant {key}"),
    }
}
fn completion(value: &CompletionRule) -> String {
    match value {
        CompletionRule::Contribution { by } => format!("publication by {}", selector(by)),
        CompletionRule::Declaration { by } => {
            format!("a completion declaration by {}", selector(by))
        }
        CompletionRule::Reviews {
            by,
            count,
            exclude_author,
        } => format!(
            "{count} distinct approving reviewer(s) from {}, {}",
            selector(by),
            if *exclude_author {
                "excluding the contribution author"
            } else {
                "including the author if eligible"
            }
        ),
        CompletionRule::Check { name, by } => format!(
            "check {name:?} attested by {} on the exact contribution",
            selector(by)
        ),
        CompletionRule::All { rules } => format!(
            "all of [{}]",
            rules.iter().map(completion).collect::<Vec<_>>().join("; ")
        ),
        CompletionRule::Any { rules } => format!(
            "any of [{}]",
            rules.iter().map(completion).collect::<Vec<_>>().join("; ")
        ),
    }
}
fn work(value: &WorkRules, prefix: &str, lines: &mut Vec<String>) {
    lines.push(format!(
        "{prefix}: {} may propose tasks; {} may publish contributions without a task.",
        selector(&value.propose),
        selector(&value.publish)
    ));
    if value.starts.is_empty() {
        lines.push(format!("{prefix}: no new task attempts are permitted."));
    }
    for start in &value.starts {
        lines.push(match start {
            StartRule::Independent { by } => format!("{prefix}: {} may start independent attempts; concurrent attempts may coexist.", selector(by)),
            StartRule::Offered { by, to } => format!("{prefix}: {} may offer work to {}; the recipient must acknowledge before starting.", selector(by), selector(to)),
        });
    }
}
fn decisions(value: &DecisionRules, prefix: &str, lines: &mut Vec<String>) {
    lines.push(format!(
        "{prefix}: completion requires {} on the exact candidate.",
        completion(&value.completion)
    ));
    lines.push(match &value.selection {
        Some(value) => format!(
            "{prefix}: {} may select one qualifying output; approval alone does not select it.",
            authority(value)
        ),
        None => {
            format!("{prefix}: qualifying contributions coexist; no single output is selected.")
        }
    });
    lines.push(match &value.finish {
        Some(value) => format!(
            "{prefix}: {} may declare the scope finished.",
            authority(value)
        ),
        None => format!(
            "{prefix}: no one may declare it finished; an empty task list is not completion."
        ),
    });
}
pub(super) fn explain(value: &Blueprint) -> Explanation {
    let mut summary = vec![
        "The goal administrator manages membership and rules separately from work permissions."
            .into(),
    ];
    work(&value.work, "Default rules", &mut summary);
    decisions(&value.decisions, "Default rules", &mut summary);
    for (name, task_type) in &value.task_types {
        let prefix = format!("Task type {name:?}");
        work(
            task_type.work.as_ref().unwrap_or(&value.work),
            &prefix,
            &mut summary,
        );
        decisions(
            task_type.decisions.as_ref().unwrap_or(&value.decisions),
            &prefix,
            &mut summary,
        );
    }
    for (name, stage) in &value.flow {
        summary.push(format!("Stage {name:?}: {} runs this stage: it creates the configured task and durably delivers ready work to {}.", authority(&stage.runner), selector(&stage.recipients)));
        let needs = if stage.requires.is_empty() {
            "no upstream evidence".into()
        } else {
            stage
                .requires
                .iter()
                .map(|requirement| {
                    format!(
                        "{} from {:?}",
                        match requirement.evidence {
                            EvidenceKind::Publication => "publication",
                            EvidenceKind::Review => "eligible review",
                            EvidenceKind::Completion => "completion",
                            EvidenceKind::Selection => "selection",
                        },
                        requirement.stage
                    )
                })
                .collect::<Vec<_>>()
                .join(" and ")
        };
        summary.push(format!(
            "Stage {name:?} requires {needs} and uses {}.",
            stage.task_type.as_ref().map_or_else(
                || "default rules".into(),
                |name| format!("task type {name:?}")
            )
        ));
    }
    let references = validation::references(value);
    Explanation { summary, required_roles: references.roles.into_iter().collect(), authority_roles: references.authorities.into_iter().collect(),
        required_inputs: value.context.inputs.iter().filter(|(_, input)| input.required).map(|(name, _)| name.clone()).collect(),
        contextual_checks: vec![
            "Bind referenced role slots to authenticated eligible members; decision-authority roles require exactly one member.".into(),
            "Supply required inputs and verify child rules stay within delegated parent authority.".into(),
            "Verify membership, pinned rule context, exact evidence and distinct reviewer eligibility for each action.".into(),
            "Check authority availability and local execution, filesystem, spending and sharing permissions separately.".into(),
            "This offline inspection does not publish a definition, create a goal, deliver work or launch a process.".into(),
        ] }
}
