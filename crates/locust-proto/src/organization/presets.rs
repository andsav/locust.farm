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

    let mut coordinator = Formation::default();
    declare_role(
        &mut coordinator,
        "coordinator",
        "Hands out work, accepts results, picks the final answer and says when the goal is finished. One person.",
    );
    coordinator.work.starts = vec![StartRule::Offered {
        by: role("coordinator"),
        to: Selector::Members,
    }];
    coordinator.decisions = DecisionRules {
        completion: CompletionRule::Reviews {
            by: role("coordinator"),
            count: 1,
            exclude_author: false,
        },
        selection: Some(authority("coordinator")),
        finish: Some(authority("coordinator")),
    };

    let mut peer_review = Formation::default();
    peer_review.decisions.completion = CompletionRule::Reviews {
        by: Selector::Members,
        count: 1,
        exclude_author: true,
    };

    let mut independent = Formation::default();
    declare_role(
        &mut independent,
        "judge",
        "Picks which finished attempt to use. One person.",
    );
    independent.decisions.selection = Some(authority("judge"));

    let mut panel = Formation::default();
    declare_role(&mut panel, "reviewer", "Reviews work done by other people.");
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
                completion: CompletionRule::Reviews {
                    by: Selector::Members,
                    count: 1,
                    exclude_author: true,
                },
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
            "Members contribute and independently start work; authors declare completion.",
            open,
        ),
        preset(
            "coordinator",
            "A coordinator offers work, reviews completion, selects contributions and closes the goal.",
            coordinator,
        ),
        preset(
            "peer-review",
            "Members independently start work; completion requires one review by another member.",
            peer_review,
        ),
        preset(
            "independent-attempts",
            "Authors complete independent attempts; a bound judge selects contributions.",
            independent,
        ),
        preset(
            "review-panel",
            "Completion requires two distinct reviews from the reviewer role, excluding the author.",
            panel,
        ),
        preset(
            "pipeline",
            "A draft stage needs one review by another member; a ship stage starts when the draft is complete.",
            pipeline,
        ),
    ]
}
