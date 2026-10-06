//! Bundled examples of the offline authoring contract.
use super::{
    Authority, CompletionRule, DecisionRules, EvidenceKind, Formation, Prerequisite, Preset, Role,
    Selector, Stage, StartRule, TaskType,
};

fn role(name: &str) -> Selector {
    Selector::Role { name: name.into() }
}
fn authority(name: &str) -> Authority {
    Authority::Role { name: name.into() }
}
fn declare_role(formation: &mut Formation, name: &str, description: &str) {
    formation.roles.insert(
        name.into(),
        Role {
            description: description.into(),
        },
    );
}
fn preset(name: &str, description: &str, formation: Formation) -> Preset {
    Preset {
        name: name.into(),
        description: description.into(),
        formation,
    }
}

/// Examples ship within the binary's contract dependency; no checkout is needed.
pub fn presets() -> Vec<Preset> {
    let open = Formation::default();

    let mut directed = Formation::default();
    declare_role(
        &mut directed,
        "lead",
        "Hands out tasks, picks the result to use and can close a task. One member.",
    );
    directed.work.starts = vec![StartRule::Offered {
        by: role("lead"),
        to: Selector::Members,
    }];
    declare_role(
        &mut directed,
        "reviewer",
        "Approves results, including its own.",
    );
    directed.decisions = DecisionRules {
        completion: CompletionRule::Reviews {
            by: role("reviewer"),
            count: 1,
            exclude_author: false,
        },
        selection: Some(authority("lead")),
        finish: Some(authority("lead")),
    };

    let mut peer_review = Formation::default();
    peer_review.decisions.completion = peer_completion();

    let mut independent = Formation::default();
    declare_role(
        &mut independent,
        "lead",
        "Picks the result to use. One member.",
    );
    independent.decisions.selection = Some(authority("lead"));

    let mut panel = Formation::default();
    declare_role(
        &mut panel,
        "reviewer",
        "Approves results from other members.",
    );
    panel.decisions.completion = CompletionRule::Reviews {
        by: role("reviewer"),
        count: 2,
        exclude_author: true,
    };

    let mut pipeline = Formation::default();
    pipeline.task_types.insert(
        "draft".into(),
        TaskType {
            work: None,
            decisions: Some(DecisionRules {
                completion: peer_completion(),
                selection: None,
                finish: None,
            }),
        },
    );
    pipeline.flow.insert(
        "draft".into(),
        Stage {
            recipients: Selector::Members,
            task_type: Some("draft".into()),
            requires: Vec::new(),
        },
    );
    pipeline.flow.insert(
        "ship".into(),
        Stage {
            recipients: Selector::Members,
            task_type: None,
            requires: vec![Prerequisite {
                stage: "draft".into(),
                evidence: EvidenceKind::Completion,
            }],
        },
    );

    vec![
        preset(
            "open",
            "Members start work on their own and say when their own result is done.",
            open,
        ),
        preset(
            "peer-review",
            "Members start work on their own; a result counts once another member approves it, and a goal's only member needs no approval.",
            peer_review,
        ),
        preset(
            "pipeline",
            "A draft stage needs one approval by another member, or none while the goal has one member; a ship stage opens when a draft counts.",
            pipeline,
        ),
        preset(
            "independent-attempts",
            "Members try the same task on their own; the lead picks the result to use.",
            independent,
        ),
        preset(
            "review-panel",
            "A result counts once two reviewers who did not write it approve it.",
            panel,
        ),
        preset(
            "directed",
            "A lead hands out tasks, picks the result to use and closes tasks; a reviewer approves results.",
            directed,
        ),
    ]
}

fn peer_completion() -> CompletionRule {
    CompletionRule::Any {
        rules: vec![
            CompletionRule::Reviews {
                by: Selector::Members,
                count: 1,
                exclude_author: true,
            },
            CompletionRule::Contribution {
                by: Selector::OnlyMember,
            },
        ],
    }
}
