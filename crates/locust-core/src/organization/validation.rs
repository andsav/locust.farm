use std::collections::{BTreeMap, BTreeSet};

use locust_proto::id::PublicKey;
use locust_proto::organization::*;

use super::{Diagnostic, References, escape};

struct Validator<'a> {
    formation: &'a Formation,
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
        if !is_role_name(name) {
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
        if !self.formation.roles.contains_key(name) {
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
    fn completion(&mut self, rule: &CompletionRule, path: &str, task: bool) {
        match rule {
            CompletionRule::Contribution { by }
            | CompletionRule::Declaration { by }
            | CompletionRule::Check { by, .. }
            | CompletionRule::Reviews { by, .. } => {
                self.selector(by, &format!("{path}/by"), task, true);
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
                    self.completion(rule, &format!("{path}/rules/{index}"), task);
                }
            }
        }
    }
    fn decisions(&mut self, decisions: &DecisionRules, path: &str) {
        self.completion(&decisions.completion, &format!("{path}/completion"), true);
        if let Some(authority) = &decisions.selection {
            self.authority(authority, &format!("{path}/selection"));
        }
        if let Some(authority) = &decisions.finish {
            self.authority(authority, &format!("{path}/finish"));
        }
    }
    /// A stage's task is opened by the host's computer under the goal's
    /// signing key, which is no member, so a rule that lets only
    /// `task_creator` act in it can be met by nobody. The `by` of an offered
    /// start is not checked: the host's computer makes those offers itself.
    fn stage_task_creator(&mut self, stage_name: &str, stage: &Stage, path: &str) {
        let task_type = stage
            .task_type
            .as_ref()
            .and_then(|name| self.formation.task_types.get(name).map(|t| (name, t)));
        let (work, work_path) = match task_type {
            Some((name, task_type)) if task_type.work.is_some() => (
                task_type.work.as_ref().unwrap(),
                format!("/task_types/{}/work", escape(name)),
            ),
            _ => (&self.formation.work, "/work".to_owned()),
        };
        let (decisions, decisions_path) = match task_type {
            Some((name, task_type)) if task_type.decisions.is_some() => (
                task_type.decisions.as_ref().unwrap(),
                format!("/task_types/{}/decisions", escape(name)),
            ),
            _ => (&self.formation.decisions, "/decisions".to_owned()),
        };
        let mut found = Vec::new();
        for (index, start) in work.starts.iter().enumerate() {
            match start {
                StartRule::Independent { by } if names_task_creator(by) => {
                    found.push(format!("{work_path}/starts/{index}/by"));
                }
                StartRule::Offered { to, .. } if names_task_creator(to) => {
                    found.push(format!("{work_path}/starts/{index}/to"));
                }
                _ => {}
            }
        }
        completion_task_creator(
            &decisions.completion,
            &format!("{decisions_path}/completion"),
            &mut found,
        );
        for rule in found {
            self.error(
                "selector_scope",
                path,
                format!(
                    "Stage {stage_name:?} opens its task from the host's computer, which is no member, so task_creator at {rule} can be met by nobody"
                ),
                "Name members, a role or a participant in the rules this stage's task uses.",
            );
        }
    }
    fn run(&mut self) {
        for name in self.formation.roles.keys() {
            self.name(name, &format!("/roles/{}", escape(name)));
        }
        for name in self.formation.context.inputs.keys() {
            self.name(name, &format!("/context/inputs/{}", escape(name)));
        }
        self.work(&self.formation.work, "/work");
        self.decisions(&self.formation.decisions, "/decisions");
        if let Some(workspace) = &self.formation.workspace {
            self.authority(&workspace.integrator, "/workspace/integrator");
            self.completion(&workspace.completion, "/workspace/completion", false);
        }
        for (name, task_type) in &self.formation.task_types {
            let path = format!("/task_types/{}", escape(name));
            self.name(name, &path);
            if let Some(work) = &task_type.work {
                self.work(work, &format!("{path}/work"));
            }
            if let Some(decisions) = &task_type.decisions {
                self.decisions(decisions, &format!("{path}/decisions"));
            }
        }
        let mut dependencies = BTreeMap::new();
        for (name, stage) in &self.formation.flow {
            let path = format!("/flow/{}", escape(name));
            self.name(name, &path);
            self.selector(
                &stage.recipients,
                &format!("{path}/recipients"),
                false,
                false,
            );
            if let Some(task_type) = &stage.task_type
                && !self.formation.task_types.contains_key(task_type)
            {
                self.error(
                    "unknown_task_type",
                    &format!("{path}/task_type"),
                    format!("Task type {task_type:?} is not declared"),
                    "Declare the task type or remove the reference to inherit the default rules.",
                );
            }
            self.stage_task_creator(name, stage, &path);
            let mut required = BTreeSet::new();
            for (index, requirement) in stage.requires.iter().enumerate() {
                let requirement_path = format!("{path}/requires/{index}");
                if let Some(upstream) = self.formation.flow.get(&requirement.stage) {
                    required.insert(requirement.stage.clone());
                    let decisions = upstream
                        .task_type
                        .as_ref()
                        .and_then(|name| self.formation.task_types.get(name))
                        .and_then(|task_type| task_type.decisions.as_ref())
                        .unwrap_or(&self.formation.decisions);
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

fn names_task_creator(selector: &Selector) -> bool {
    match selector {
        Selector::TaskCreator => true,
        Selector::Any { selectors } => selectors.iter().any(names_task_creator),
        _ => false,
    }
}

fn completion_task_creator(rule: &CompletionRule, path: &str, found: &mut Vec<String>) {
    match rule {
        CompletionRule::Contribution { by }
        | CompletionRule::Declaration { by }
        | CompletionRule::Check { by, .. }
        | CompletionRule::Reviews { by, .. } => {
            if names_task_creator(by) {
                found.push(format!("{path}/by"));
            }
        }
        CompletionRule::All { rules } | CompletionRule::Any { rules } => {
            for (index, rule) in rules.iter().enumerate() {
                completion_task_creator(rule, &format!("{path}/rules/{index}"), found);
            }
        }
    }
}

/// Upper bound only: dynamic selectors cannot be proven unsatisfiable offline.
fn fixed_members(selector: &Selector) -> Option<BTreeSet<String>> {
    match selector {
        Selector::Nobody => Some(BTreeSet::new()),
        Selector::OnlyMember => Some(BTreeSet::from(["only_member".into()])),
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

pub(super) fn validate(formation: &Formation) -> Vec<Diagnostic> {
    let mut validator = Validator {
        formation,
        diagnostics: Vec::new(),
        references: References::default(),
    };
    validator.run();
    validator.diagnostics
}

pub(super) fn references(formation: &Formation) -> References {
    let mut validator = Validator {
        formation,
        diagnostics: Vec::new(),
        references: References::default(),
    };
    validator.run();
    validator.references
}
