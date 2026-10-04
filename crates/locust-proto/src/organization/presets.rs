//! Bundled examples of the offline authoring contract.
use super::{
    Authority, Blueprint, CompletionRule, DecisionRules, EvidenceKind, Prerequisite, Preset, Role,
    Selector, Stage, StartRule, TaskVariation,
};

fn role(name: &str) -> Selector {
    Selector::Role { name: name.into() }
}
fn authority(name: &str) -> Authority {
    Authority::Role { name: name.into() }
}
fn declare_role(blueprint: &mut Blueprint, name: &str, description: &str) {
    blueprint.roles.insert(
        name.into(),
        Role {
            description: description.into(),
        },
    );
}
fn preset(name: &str, description: &str, blueprint: Blueprint) -> Preset {
    Preset {
        name: name.into(),
        description: description.into(),
        blueprint,
    }
}

/// Examples ship within the binary's contract dependency; no checkout is needed.
pub fn presets() -> Vec<Preset> {
    let open = Blueprint::default();

    let mut coordinator = Blueprint::default();
    declare_role(
        &mut coordinator,
        "coordinator",
        "One member offers work and decides completion, selection and closure.",
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
        closure: Some(authority("coordinator")),
    };

    let mut peer_review = Blueprint::default();
    peer_review.decisions.completion = CompletionRule::Reviews {
        by: Selector::Members,
        count: 1,
        exclude_author: true,
    };

    let mut independent = Blueprint::default();
    declare_role(
        &mut independent,
        "chooser",
        "One member selects among independently completed contributions.",
    );
    independent.decisions.selection = Some(authority("chooser"));

    let mut panel = Blueprint::default();
    declare_role(
        &mut panel,
        "reviewer",
        "Members eligible to review contributions by other authors.",
    );
    panel.decisions.completion = CompletionRule::Reviews {
        by: role("reviewer"),
        count: 2,
        exclude_author: true,
    };

    let mut pipeline = Blueprint::default();
    pipeline.variations.insert(
        "reviewed".into(),
        TaskVariation {
            work: None,
            decisions: Some(DecisionRules {
                completion: CompletionRule::Reviews {
                    by: Selector::Members,
                    count: 1,
                    exclude_author: true,
                },
                selection: None,
                closure: None,
            }),
        },
    );
    declare_role(
        &mut pipeline,
        "materializer",
        "One member's daemon advances the configured stages and durably delivers ready work.",
    );
    pipeline.flow.insert(
        "draft".into(),
        Stage {
            materializer: authority("materializer"),
            recipients: Selector::Members,
            variation: None,
            requires: Vec::new(),
        },
    );
    pipeline.flow.insert(
        "review".into(),
        Stage {
            materializer: authority("materializer"),
            recipients: Selector::Members,
            variation: Some("reviewed".into()),
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
            "Authors complete independent attempts; a bound chooser selects contributions.",
            independent,
        ),
        preset(
            "review-panel",
            "Completion requires two distinct reviews from the reviewer role, excluding the author.",
            panel,
        ),
        preset(
            "pipeline",
            "A review stage requires completed draft evidence and uses a named peer-review variation.",
            pipeline,
        ),
    ]
}
