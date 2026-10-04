use std::collections::{BTreeMap, BTreeSet};

use locust_proto::id::PublicKey;
use locust_proto::organization::*;

use super::{Diagnostic, References, escape};

struct Validator<'a> {
    blueprint: &'a Blueprint,
    diagnostics: Vec<Diagnostic>,
    references: References,
}

impl Validator<'_> {
    fn error(&mut self, code: &str, path: &str, message: impl Into<String>, correction: &str) {
        self.diagnostics.push(Diagnostic::error(
            code,
            "definition",
            path,
            message,
            correction,
        ));
    }
    fn name(&mut self, name: &str, path: &str) {
        if name.trim().is_empty() || name.chars().any(char::is_control) {
            self.error(
                "invalid_name",
                path,
                "Names must contain visible text and no control characters",
                "Choose a stable descriptive name.",
            );
        }
    }
    fn role(&mut self, name: &str, path: &str) {
        self.references.roles.insert(name.into());
        if !self.blueprint.roles.contains_key(name) {
            self.error(
                "unknown_role",
                path,
                format!("Role {name:?} is not declared"),
                "Declare the role in roles or reference an existing role.",
            );
        }
    }
    fn key(&mut self, key: &str, path: &str) {
        if key.parse::<PublicKey>().is_err() {
            self.error("invalid_participant", path, "Participant identity must be a 32-byte public key encoded as 64 hex characters", "Use the authenticated participant public key, or a declared role slot for a reusable template.");
        }
    }
    fn selector(&mut self, selector: &Selector, path: &str, task: bool, contribution: bool) {
        match selector {
            Selector::Role { name } => self.role(name, &format!("{path}/name")),
            Selector::Participant { key } => self.key(key, &format!("{path}/key")),
            Selector::TaskCreator if !task => self.error(
                "selector_scope",
                path,
                "There is no task creator in this scope",
                "Use members, a role or a participant in goal-level rules.",
            ),
            Selector::ContributionAuthor if !contribution => self.error(
                "selector_scope",
                path,
                "There is no contribution author in this scope",
                "Use contribution_author only in a contribution completion criterion.",
            ),
            Selector::Any { selectors } => {
                if selectors.is_empty() {
                    self.error("empty_selector", path, "An any selector needs at least one alternative", "Add an eligible selector, or use nobody to deliberately disable a work action.");
                }
                for (index, selector) in selectors.iter().enumerate() {
                    self.selector(
                        selector,
                        &format!("{path}/selectors/{index}"),
                        task,
                        contribution,
                    );
                }
            }
            _ => {}
        }
    }
    fn authority(&mut self, authority: &Authority, path: &str) {
        match authority {
            Authority::Role { name } => {
                self.role(name, &format!("{path}/name"));
                self.references.authorities.insert(name.into());
            }
            Authority::Participant { key } => self.key(key, &format!("{path}/key")),
        }
    }
    fn work(&mut self, work: &WorkRules, path: &str) {
        self.selector(&work.propose, &format!("{path}/propose"), false, false);
        self.selector(&work.publish, &format!("{path}/publish"), false, false);
        for (index, start) in work.starts.iter().enumerate() {
            let path = format!("{path}/starts/{index}");
            match start {
                StartRule::Independent { by } => {
                    self.selector(by, &format!("{path}/by"), true, false)
                }
                StartRule::Offered { by, to } => {
                    self.selector(by, &format!("{path}/by"), true, false);
                    self.selector(to, &format!("{path}/to"), true, false);
                }
            }
        }
    }
    fn completion(&mut self, rule: &CompletionRule, path: &str) {
        match rule {
            CompletionRule::Contribution { by }
            | CompletionRule::Declaration { by }
            | CompletionRule::Check { by, .. }
            | CompletionRule::Reviews { by, .. } => {
                self.selector(by, &format!("{path}/by"), true, true);
                if fixed_members(by).is_some_and(|members| members.is_empty()) {
                    self.error(
                        "impossible_completion",
                        &format!("{path}/by"),
                        "This criterion has no possible eligible signer",
                        "Choose an eligible participant selector.",
                    );
                }
                if let CompletionRule::Check { name, .. } = rule {
                    self.name(name, &format!("{path}/name"));
                }
                if let CompletionRule::Reviews {
                    count,
                    exclude_author,
                    ..
                } = rule
                {
                    if *count == 0 {
                        self.error(
                            "invalid_threshold",
                            &format!("{path}/count"),
                            "Review thresholds must be positive",
                            "Require one or more distinct eligible reviewers.",
                        );
                    }
                    if let Some(mut members) = fixed_members(by) {
                        if *exclude_author {
                            members.remove("contribution_author");
                        }
                        if u64::from(*count) > members.len() as u64 {
                            self.error("impossible_threshold", &format!("{path}/count"), "The requested distinct-reviewer threshold exceeds the explicitly possible identities", "Add eligible identities, reduce the threshold, or use a role whose membership is checked when binding the template.");
                        }
                    }
                }
            }
            CompletionRule::All { rules } | CompletionRule::Any { rules } => {
                if rules.is_empty() {
                    self.error(
                        "empty_criteria",
                        path,
                        "Completion groups need at least one criterion",
                        "Add an explicit completion criterion.",
                    );
                }
                for (index, rule) in rules.iter().enumerate() {
                    self.completion(rule, &format!("{path}/rules/{index}"));
                }
            }
        }
    }
    fn decisions(&mut self, decisions: &DecisionRules, path: &str) {
        self.completion(&decisions.completion, &format!("{path}/completion"));
        if let Some(authority) = &decisions.selection {
            self.authority(authority, &format!("{path}/selection"));
        }
        if let Some(authority) = &decisions.closure {
            self.authority(authority, &format!("{path}/closure"));
        }
    }
    fn run(&mut self) {
        for name in self.blueprint.roles.keys() {
            self.name(name, &format!("/roles/{}", escape(name)));
        }
        for name in self.blueprint.context.inputs.keys() {
            self.name(name, &format!("/context/inputs/{}", escape(name)));
        }
        self.work(&self.blueprint.work, "/work");
        self.decisions(&self.blueprint.decisions, "/decisions");
        for (name, variation) in &self.blueprint.variations {
            let path = format!("/variations/{}", escape(name));
            self.name(name, &path);
            if let Some(work) = &variation.work {
                self.work(work, &format!("{path}/work"));
            }
            if let Some(decisions) = &variation.decisions {
                self.decisions(decisions, &format!("{path}/decisions"));
            }
        }
        let mut dependencies = BTreeMap::new();
        for (name, stage) in &self.blueprint.flow {
            let path = format!("/flow/{}", escape(name));
            self.name(name, &path);
            self.authority(&stage.materializer, &format!("{path}/materializer"));
            self.selector(
                &stage.recipients,
                &format!("{path}/recipients"),
                false,
                false,
            );
            if let Some(variation) = &stage.variation
                && !self.blueprint.variations.contains_key(variation)
            {
                self.error(
                    "unknown_variation",
                    &format!("{path}/variation"),
                    format!("Variation {variation:?} is not declared"),
                    "Declare the variation or remove the reference to inherit the default rules.",
                );
            }
            let mut required = BTreeSet::new();
            for (index, requirement) in stage.requires.iter().enumerate() {
                let requirement_path = format!("{path}/requires/{index}");
                if let Some(upstream) = self.blueprint.flow.get(&requirement.stage) {
                    required.insert(requirement.stage.clone());
                    let decisions = upstream
                        .variation
                        .as_ref()
                        .and_then(|name| self.blueprint.variations.get(name))
                        .and_then(|variation| variation.decisions.as_ref())
                        .unwrap_or(&self.blueprint.decisions);
                    if requirement.evidence == EvidenceKind::Selection
                        && decisions.selection.is_none()
                    {
                        self.error("unavailable_evidence", &format!("{requirement_path}/evidence"), "The upstream stage has no selection authority", "Require completion/publication/review evidence, or explicitly configure upstream selection.");
                    }
                } else {
                    self.error(
                        "unknown_stage",
                        &format!("{requirement_path}/stage"),
                        format!("Stage {:?} is not declared", requirement.stage),
                        "Reference a declared stage.",
                    );
                }
            }
            dependencies.insert(name.clone(), required);
        }
        // Iterative graph elimination avoids adding a traversal-depth cap.
        while !dependencies.is_empty() {
            let ready: BTreeSet<_> = dependencies
                .iter()
                .filter(|(_, dependencies)| dependencies.is_empty())
                .map(|(name, _)| name.clone())
                .collect();
            if ready.is_empty() {
                self.error("flow_cycle", "/flow", format!("Flow has a dependency cycle involving: {}", dependencies.keys().cloned().collect::<Vec<_>>().join(", ")), "Remove a cyclic prerequisite; retries and new rounds are explicit runtime actions.");
                break;
            }
            dependencies.retain(|name, _| !ready.contains(name));
            for requirements in dependencies.values_mut() {
                requirements.retain(|name| !ready.contains(name));
            }
        }
    }
}

/// Upper bound only: dynamic selectors cannot be proven unsatisfiable offline.
fn fixed_members(selector: &Selector) -> Option<BTreeSet<String>> {
    match selector {
        Selector::Nobody => Some(BTreeSet::new()),
        Selector::Participant { key } => Some(BTreeSet::from([key.to_lowercase()])),
        Selector::TaskCreator => Some(BTreeSet::from(["task_creator".into()])),
        Selector::ContributionAuthor => Some(BTreeSet::from(["contribution_author".into()])),
        Selector::Any { selectors } => {
            let mut members = BTreeSet::new();
            for selector in selectors {
                members.extend(fixed_members(selector)?);
            }
            Some(members)
        }
        Selector::Members | Selector::Role { .. } => None,
    }
}

pub(super) fn validate(blueprint: &Blueprint) -> Vec<Diagnostic> {
    let mut validator = Validator {
        blueprint,
        diagnostics: Vec::new(),
        references: References::default(),
    };
    validator.run();
    validator.diagnostics
}

pub(super) fn references(blueprint: &Blueprint) -> References {
    let mut validator = Validator {
        blueprint,
        diagnostics: Vec::new(),
        references: References::default(),
    };
    validator.run();
    validator.references
}
