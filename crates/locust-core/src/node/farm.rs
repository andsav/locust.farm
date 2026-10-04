//! Owner-approved public projection and durable serialized publisher outbox.
use super::callers::Actor;
use super::commit::Tx;
use super::entry::Entry;
use super::requests::{Plan, Planned, answer};
use super::{Node, records};
use crate::goal::{DefinitionLookup, Standing};
use locust_proto::api::{ApiError, Caller, ErrorCode, Request, Response};
use locust_proto::crypto::{self, Keypair};
use locust_proto::engine::Entropy;
use locust_proto::event::{
    AttemptStatus, Body, Context, DecisionAction, DecisionPurpose, Scope, ScopeKey,
};
use locust_proto::farm::*;
use locust_proto::id::{EventId, GoalId, PublicKey};
use locust_proto::store::{LocalWrite, Space, Store};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const FARM: u8 = b'F';
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct FarmLocal {
    seed: [u8; 32],
    salt: [u8; 32],
    pub base_url: String,
    publication: EventId,
    desired: Option<FarmVisibility>,
    policy: DisclosurePolicy,
    ids: BTreeMap<String, u32>,
    refs: BTreeMap<String, String>,
    observed: BTreeMap<u64, u64>,
    change_ids: BTreeMap<u64, u64>,
    observed_feed: u64,
    next_sequence: u64,
    pending: Option<SignedFarmRequest>,
    receipt: Option<FarmReceipt>,
    acknowledged_at_ms: Option<u64>,
    last_snapshot: Option<String>,
    next_send_ms: u64,
    retry_ms: u64,
    suspended: bool,
    deleted: bool,
    control: Option<FarmOperation>,
    last_error: Option<String>,
}
impl std::fmt::Debug for FarmLocal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FarmLocal")
            .field("farm_id", &self.id())
            .finish_non_exhaustive()
    }
}
impl FarmLocal {
    fn id(&self) -> FarmId {
        FarmId::from_key(Keypair::from_seed(self.seed).public())
    }
    fn number(&mut self, kind: &str, key: String) -> u32 {
        let key = format!("{kind}:{key}");
        if let Some(n) = self.ids.get(&key) {
            return *n;
        }
        let n = self
            .ids
            .keys()
            .filter(|x| x.starts_with(&format!("{kind}:")))
            .count() as u32
            + 1;
        self.ids.insert(key, n);
        n
    }
    fn reference(&mut self, key: String) -> String {
        if let Some(r) = self.refs.get(&key) {
            return r.clone();
        }
        let mut bytes = self.salt.to_vec();
        bytes.extend(key.as_bytes());
        let digest = crypto::content_hash(&bytes).to_string();
        let mut len = 4;
        while self.refs.values().any(|r| r == &digest[..len]) {
            len += 1;
        }
        let reference = digest[..len].to_owned();
        self.refs.insert(key, reference.clone());
        reference
    }
}
pub(super) fn write(goal: GoalId, state: &FarmLocal) -> LocalWrite {
    records::put(Space::Goal, records::key(FARM, &[&goal.0]), state)
}
pub(super) fn is_record(key: &[u8]) -> bool {
    key.first() == Some(&FARM)
}
fn invalid(message: &str) -> ApiError {
    ApiError::new(ErrorCode::Invalid, message)
}
fn require_owner(actor: &Actor) -> Result<(), ApiError> {
    if actor.caller != Caller::Owner || actor.principal.is_some() {
        Err(ApiError::new(
            ErrorCode::Denied,
            "farm publication requires the local owner",
        ))
    } else {
        Ok(())
    }
}

/// Every covered work author, including removed members, must match the policy.
fn eligible(
    entry: &Entry,
    policy: &DisclosurePolicy,
) -> Result<BTreeMap<PublicKey, PublicProfile>, String> {
    let state = entry.state();
    if entry.goal.evaluation().admin_halt.is_some()
        || !entry.goal.evaluation().scope_halts.is_empty()
    {
        return Err("publication authority is disputed".into());
    }
    let Some((_, set)) = &state.publication else {
        return Err("publication policy has not arrived".into());
    };
    if set.policy.digest() != policy.digest() {
        return Err("publication policy has changed".into());
    }
    let mut required: BTreeSet<_> = state
        .members
        .values()
        .filter(|m| m.is_active())
        .map(|m| m.principal)
        .collect();
    for author in entry.goal.authors() {
        for point in entry.goal.points(author) {
            let event = entry.goal.event(&point.id).unwrap();
            if entry.goal.standing(&event.id()) == Some(Standing::Effective)
                && !matches!(
                    event.header().body,
                    Body::Genesis(_)
                        | Body::PublicationSet(_)
                        | Body::PublicationConsent(_)
                        | Body::MemberAdmitted { .. }
                        | Body::MemberRemoved { .. }
                        | Body::RulesBound { .. }
                )
            {
                required.insert(*author);
            }
        }
    }
    let mut profiles = BTreeMap::new();
    for principal in required {
        // A later pending/disputed/revoking consent must not fall back to an older acceptance.
        let latest = entry
            .goal
            .points(&principal)
            .iter()
            .filter_map(|p| entry.goal.event(&p.id))
            .filter(|e| matches!(e.header().body, Body::PublicationConsent(_)))
            .max_by_key(|e| e.header().seq);
        let Some(event) = latest else {
            return Err("a covered participant has not approved publication".into());
        };
        if entry.goal.standing(&event.id()) != Some(Standing::Effective) {
            return Err("publication consent is unavailable or disputed".into());
        }
        let Body::PublicationConsent(consent) = &event.header().body else {
            unreachable!()
        };
        let consent_set =
            entry
                .goal
                .event(&consent.publication)
                .and_then(|e| match &e.header().body {
                    Body::PublicationSet(set) => Some(set),
                    _ => None,
                });
        if !consent.accept
            || consent.policy_digest != policy.digest()
            || consent_set.is_none_or(|prior| {
                prior.farm_id != set.farm_id || prior.upload_key != set.upload_key
            })
        {
            return Err("a covered participant declined or has outdated consent".into());
        }
        profiles.insert(
            principal,
            consent
                .profile
                .clone()
                .ok_or("approved public profile is unavailable")?,
        );
    }
    Ok(profiles)
}
fn status(entry: &Entry, local: &FarmLocal) -> FarmStatus {
    let mut copy = local.clone();
    let eligibility = project(entry, &mut copy);
    FarmStatus {
        goal: entry.id(),
        farm_id: local.id(),
        desired: local.desired,
        eligible: local.desired.is_some() && eligibility.is_ok(),
        reason: eligibility.err(),
        pending: local
            .pending
            .as_ref()
            .map(|r| r.operation)
            .or(local.control),
        receipt: local.receipt.clone(),
        last_error: local.last_error.clone(),
    }
}
fn preview(entry: &Entry, local: &FarmLocal) -> FarmPreview {
    let mut copy = local.clone();
    FarmPreview {
        snapshot: project(entry, &mut copy).ok(),
        policy: Some(local.policy.clone()),
        status: Some(status(entry, local)),
    }
}

fn project(entry: &Entry, local: &mut FarmLocal) -> Result<FarmSnapshot, String> {
    let profiles = eligible(entry, &local.policy)?;
    if entry
        .state()
        .publication
        .as_ref()
        .is_none_or(|(_, set)| set.farm_id != local.id() || set.visibility != local.desired)
    {
        return Err("local farm identity differs from signed policy".into());
    }
    let state = entry.state();
    let mut snapshot = FarmSnapshot {
        version: FARM_VERSION,
        farm_id: local.id(),
        title: local.policy.title.clone(),
        formation: local.policy.formation.clone(),
        goal_state: FarmGoalState::Open,
        observed_at_ms: local.observed.values().max().copied(),
        agents: vec![],
        groups: vec![],
        stages: vec![],
        tasks: vec![],
        attempts: vec![],
        candidates: vec![],
        changes: vec![],
        omitted_changes: 0,
    };
    let mut agent_ids = BTreeMap::new();
    let mut groups = BTreeMap::<u32, Option<String>>::new();
    let mut members: Vec<_> = profiles.iter().collect();
    members.sort_by_key(|(key, _)| {
        state
            .members
            .get(key)
            .map(|m| entry.goal.event(&m.admission).map(|e| e.header().seq))
    });
    for (key, profile) in members {
        let member = state
            .members
            .get(key)
            .ok_or("covered author membership proof is unavailable")?;
        let agent = local.number("agent", key.to_string());
        let group = local.number("group", member.endpoint.to_string());
        agent_ids.insert(*key, agent);
        let group_label = groups
            .entry(group)
            .or_insert_with(|| profile.group_label.clone());
        if group_label != &profile.group_label {
            *group_label = None;
        }
        let roles = state
            .current_rules
            .and_then(|id| state.rules.get(&id))
            .map(|r| {
                r.binding
                    .roles
                    .iter()
                    .filter(|(_, keys)| keys.contains(key))
                    .filter_map(|(role, _)| local.policy.role_labels.get(role).cloned())
                    .collect()
            })
            .unwrap_or_default();
        snapshot.agents.push(FarmAgent {
            id: agent,
            group,
            name: profile.name.clone(),
            harness: profile.harness,
            roles,
        });
    }
    snapshot.groups = groups
        .into_iter()
        .map(|(id, label)| FarmGroup {
            id,
            label,
            last_sync_at_ms: state
                .members
                .values()
                .find(|m| local.ids.get(&format!("group:{}", m.endpoint)) == Some(&id))
                .and_then(|m| entry.local.goal_sync.get(&m.endpoint).copied()),
        })
        .collect();
    let mut stage_ids = BTreeMap::new();
    let mut rules_ids: BTreeSet<_> = state
        .tasks
        .values()
        .flat_map(|t| t.rounds.values().map(|r| r.binding.rules))
        .collect();
    rules_ids.extend(state.current_rules);
    for rules_id in rules_ids {
        let Some(bound) = state.rules.get(&rules_id) else {
            continue;
        };
        let Some(definition) = entry
            .definitions
            .definition(&bound.binding.definition.semantic)
        else {
            return Err("formation proof is unavailable".into());
        };
        for name in definition.flow.keys() {
            let id = local.number("stage", format!("{rules_id}:{name}"));
            stage_ids.insert((rules_id, name.clone()), id);
        }
        for (name, stage) in &definition.flow {
            let id = stage_ids[&(rules_id, name.clone())];
            let label = local
                .policy
                .stage_labels
                .get(name)
                .cloned()
                .unwrap_or_else(|| format!("Stage {id}"));
            let prerequisites = stage
                .requires
                .iter()
                .filter_map(|r| stage_ids.get(&(rules_id, r.stage.clone())).copied())
                .collect();
            snapshot.stages.push(FarmStage {
                id,
                label,
                prerequisites,
            });
        }
    }
    let mut task_ids = BTreeMap::new();
    let mut round_ids = BTreeMap::new();
    for (key, task) in &state.tasks {
        let id = local.number("task", key.to_string());
        task_ids.insert(*key, id);
        let mut rounds: Vec<_> = task.rounds.keys().copied().collect();
        rounds.sort_by_key(|id| {
            (
                *id != task.created,
                entry.goal.event(id).map(|e| e.header().seq),
            )
        });
        for round in rounds {
            let n = local.number(&format!("round:{id}"), round.to_string());
            round_ids.insert((*key, round), n);
        }
        let round = &task.rounds[&task.current_round];
        let selected_candidate = round
            .selected
            .map(|id| local.number("candidate", id.to_string()));
        let reported = !round.attempts.is_empty();
        let task_state = if round.closed {
            FarmTaskState::Closed
        } else if round.completed {
            FarmTaskState::Completed
        } else if !round.contributions.is_empty() {
            FarmTaskState::AwaitingEvidence
        } else if reported {
            FarmTaskState::Reported
        } else {
            FarmTaskState::Open
        };
        snapshot.tasks.push(FarmTask {
            id,
            reference: local.reference(key.to_string()),
            stage: round
                .binding
                .stage
                .as_ref()
                .and_then(|s| stage_ids.get(&(round.binding.rules, s.clone())).copied()),
            round: round_ids[&(*key, task.current_round)],
            state: task_state,
            completed: round.completed,
            closed: round.closed,
            selected_candidate,
        });
    }
    for (key, attempt) in &state.attempts {
        let Scope::Task(task) = attempt.context.scope else {
            continue;
        };
        let Some(task_id) = task_ids.get(&task) else {
            continue;
        };
        let Some(agent) = agent_ids.get(&attempt.author) else {
            return Err("attempt author lacks consent".into());
        };
        let Some(round) = round_ids.get(&(task, attempt.context.round)) else {
            continue;
        };
        let last = attempt.reports.last().unwrap_or(key);
        let observed_at_ms = entry
            .feed
            .position(last)
            .and_then(|pos| local.observed.get(&pos).copied());
        snapshot.attempts.push(FarmAttempt {
            id: local.number("attempt", key.to_string()),
            agent: *agent,
            task: *task_id,
            round: *round,
            state: match attempt.status {
                None => FarmAttemptState::Started,
                Some(AttemptStatus::Progress) => FarmAttemptState::Progress,
                Some(AttemptStatus::Completed) => FarmAttemptState::Completed,
                Some(AttemptStatus::Failed) => FarmAttemptState::Failed,
                Some(AttemptStatus::Abandoned) => FarmAttemptState::Abandoned,
                Some(AttemptStatus::Uncertain) => FarmAttemptState::Uncertain,
            },
            observed_at_ms,
        });
    }
    for (key, candidate) in &state.contributions {
        let Some(agent) = agent_ids.get(&candidate.author) else {
            return Err("contribution author lacks consent".into());
        };
        let task = match candidate.context.scope {
            Scope::Task(task) => task_ids.get(&task).copied(),
            _ => None,
        };
        let round = match candidate.context.scope {
            Scope::Task(task) => round_ids
                .get(&(task, candidate.context.round))
                .copied()
                .ok_or("candidate round proof is unavailable")?,
            _ => local.number("goal_round", candidate.context.round.to_string()),
        };
        let selected = match candidate.context.scope {
            Scope::Task(task) => state
                .tasks
                .get(&task)
                .and_then(|t| t.rounds.get(&t.current_round))
                .is_some_and(|r| r.context == candidate.context && r.selected == Some(*key)),
            _ => false,
        };
        let requirement = entry
            .goal
            .effective_rules(candidate.context, &entry.definitions)
            .map(|r| requirement(&r.decisions.completion))
            .unwrap_or_else(|| "Completion rule unavailable".into());
        snapshot.candidates.push(FarmCandidate {
            id: local.number("candidate", key.to_string()),
            agent: *agent,
            task,
            round,
            completed: candidate.approved,
            selected,
            evidence_count: candidate.evidence.len().saturating_sub(1) as u32,
            requirement,
        });
    }
    if let Some(round) = state.current_rules {
        let key = ScopeKey {
            context: Context {
                scope: Scope::Goal,
                round,
            },
            purpose: DecisionPurpose::Closure,
        };
        if entry.goal.evaluation().scope_halts.contains_key(&key) {
            snapshot.goal_state = FarmGoalState::Disputed;
        } else if state
            .decisions
            .get(&key)
            .and_then(|d| d.last())
            .is_some_and(|d| matches!(d.action, DecisionAction::Close))
        {
            snapshot.goal_state = FarmGoalState::Ended;
        }
    }
    let recent = local.policy.recent_changes as usize;
    let visible_feed: Vec<_> = entry
        .feed
        .after(0, usize::MAX)
        .filter(|(_, id)| {
            entry
                .goal
                .event(id)
                .is_some_and(|event| agent_ids.contains_key(&event.header().author))
        })
        .collect();
    for &(position, _) in &visible_feed {
        if !local.change_ids.contains_key(&position) {
            let id = (local.change_ids.len() as u64)
                .checked_add(1)
                .ok_or("public change counter exhausted")?;
            local.change_ids.insert(position, id);
        }
    }
    snapshot.observed_at_ms = visible_feed
        .iter()
        .filter_map(|(position, _)| local.observed.get(position).copied())
        .max();
    let omitted = visible_feed.len().saturating_sub(recent);
    for &(position, id) in visible_feed.iter().skip(omitted) {
        let Some(event) = entry.goal.event(id) else {
            continue;
        };
        let body = &event.header().body;
        let (kind, text) = if entry.goal.standing(id) != Some(Standing::Effective) {
            (FarmChangeKind::Retraction, "Evidence standing changed")
        } else {
            match body {
                Body::PublicationSet(_) | Body::PublicationConsent(_) => {
                    (FarmChangeKind::Publication, "Publication approval changed")
                }
                Body::MemberAdmitted { .. } | Body::MemberRemoved { .. } => {
                    (FarmChangeKind::Membership, "Membership changed")
                }
                Body::TaskOpened { .. }
                | Body::TaskRevised { .. }
                | Body::EffectMaterialized { .. } => {
                    (FarmChangeKind::Task, "Work configuration changed")
                }
                Body::AttemptStarted { .. } | Body::AttemptReported { .. } => {
                    (FarmChangeKind::Attempt, "Attempt reported")
                }
                Body::ContributionPublished { .. } => {
                    (FarmChangeKind::Contribution, "Contribution published")
                }
                Body::ReviewRecorded { .. }
                | Body::CheckAttested { .. }
                | Body::CompletionDeclared { .. } => {
                    (FarmChangeKind::Evidence, "Completion evidence reported")
                }
                Body::ScopeDecided { .. } => (FarmChangeKind::Closure, "Scope decision reported"),
                _ => (FarmChangeKind::Task, "Collaboration record changed"),
            }
        };
        let task = body.context().and_then(|c| match c.scope {
            Scope::Task(t) => task_ids.get(&t).copied(),
            _ => None,
        });
        snapshot.changes.push(FarmChange {
            id: local.change_ids[&position],
            kind,
            agent: agent_ids.get(&event.header().author).copied(),
            task,
            observed_at_ms: local.observed.get(&position).copied(),
            text: text.into(),
        });
    }
    snapshot.omitted_changes = omitted as u64;
    snapshot.validate()?;
    Ok(snapshot)
}
fn requirement(rule: &locust_proto::organization::CompletionRule) -> String {
    use locust_proto::organization::CompletionRule::*;
    match rule {
        Contribution { .. } => "Eligible contribution".into(),
        Declaration { .. } => "Authorized declaration".into(),
        Reviews { count, .. } => format!("{count} eligible distinct reviews"),
        Check { .. } => "Authorized check".into(),
        All { .. } => "All configured requirements".into(),
        Any { .. } => "Any configured requirement".into(),
    }
}

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn farm_request(&self, actor: &Actor, request: Request, now: u64) -> Plan {
        require_owner(actor)?;
        if matches!(request, Request::FarmStatus) {
            return answer(Response::Farms(
                self.goals
                    .values()
                    .filter_map(|entry| entry.local.farm.as_ref().map(|local| status(entry, local)))
                    .collect(),
            ));
        }
        let goal = request
            .goal()
            .ok_or_else(|| invalid("farm request requires a goal"))?;
        let entry = self
            .goals
            .get(&goal)
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "goal not found"))?;
        if matches!(request, Request::FarmShow { .. }) {
            return answer(Response::FarmPreview(
                entry
                    .local
                    .farm
                    .as_ref()
                    .map(|l| preview(entry, l))
                    .unwrap_or(FarmPreview {
                        snapshot: None,
                        policy: entry
                            .state()
                            .publication
                            .as_ref()
                            .map(|(_, set)| set.policy.clone()),
                        status: None,
                    }),
            ));
        }
        let mut tx = Tx::none();
        if let Request::FarmConsent {
            agent,
            accept,
            name,
            group_label,
            ..
        } = request
        {
            self.signer(&agent)?;
            let Some((publication, set)) = &entry.state().publication else {
                return Err(invalid("publication policy has not arrived"));
            };
            let profile = if accept {
                let name = name.ok_or_else(|| {
                    invalid("accepting publication requires an explicit public name")
                })?;
                let profile = PublicProfile {
                    name,
                    group_label,
                    harness: self.public_harness(agent),
                };
                profile.validate().map_err(|e| invalid(&e))?;
                Some(profile)
            } else {
                None
            };
            self.author(
                entry,
                &agent,
                Body::PublicationConsent(PublicationConsent {
                    publication: *publication,
                    policy_digest: set.policy.digest(),
                    accept,
                    profile,
                }),
                None,
                now,
                &mut tx,
            )?;
            return Ok(Planned {
                response: Response::FarmPreview(FarmPreview {
                    snapshot: None,
                    policy: Some(set.policy.clone()),
                    status: entry.local.farm.as_ref().map(|l| status(entry, l)),
                }),
                tx,
            });
        }
        let administrator = entry
            .state()
            .administrator
            .ok_or_else(|| invalid("administrator proof unavailable"))?;
        self.signer(&administrator)?;
        let mut local = match request {
            Request::FarmOn {
                base_url,
                listed,
                title,
                formation,
                stage_labels,
                role_labels,
                recent_changes,
                ..
            } => {
                let base_url = service_origin(&base_url)
                    .map_err(|error| invalid(&error))?
                    .to_string()
                    .trim_end_matches('/')
                    .to_owned();
                if let Some(existing) = &entry.local.farm {
                    if existing.desired.is_none() && !existing.deleted {
                        return Err(ApiError::new(
                            ErrorCode::Conflict,
                            "farm deletion is pending; wait for its receipt",
                        ));
                    }
                    if !existing.deleted && existing.base_url != base_url.trim_end_matches('/') {
                        return Err(ApiError::new(
                            ErrorCode::Conflict,
                            "delete the existing farm before changing service",
                        ));
                    }
                }
                let policy = DisclosurePolicy {
                    version: FARM_VERSION,
                    title,
                    formation,
                    stage_labels,
                    role_labels,
                    recent_changes,
                };
                policy.validate().map_err(|e| invalid(&e))?;
                let visibility = if listed {
                    FarmVisibility::Listed
                } else {
                    FarmVisibility::Link
                };
                let mut local = entry
                    .local
                    .farm
                    .as_ref()
                    .filter(|l| !l.deleted && l.desired.is_some())
                    .cloned()
                    .unwrap_or_else(|| FarmLocal {
                        seed: self.random(),
                        salt: self.random(),
                        base_url: base_url.clone(),
                        publication: EventId([0; 32]),
                        desired: None,
                        policy: policy.clone(),
                        ids: BTreeMap::new(),
                        refs: BTreeMap::new(),
                        observed: BTreeMap::new(),
                        change_ids: BTreeMap::new(),
                        observed_feed: entry.feed.len(),
                        next_sequence: 1,
                        pending: None,
                        receipt: None,
                        acknowledged_at_ms: None,
                        last_snapshot: None,
                        next_send_ms: 0,
                        retry_ms: 1000,
                        suspended: false,
                        deleted: false,
                        control: None,
                        last_error: None,
                    });
                local.base_url = base_url.trim_end_matches('/').into();
                local.policy = policy;
                local.desired = Some(visibility);
                local.deleted = false;
                local.next_send_ms = 0;
                local.last_snapshot = None;
                local
            }
            Request::FarmOff { .. } => {
                let mut local = entry
                    .local
                    .farm
                    .clone()
                    .ok_or_else(|| invalid("no farm configured"))?;
                if local.deleted || local.desired.is_none() {
                    return answer(Response::FarmPreview(preview(entry, &local)));
                }
                local.desired = None;
                local.control = Some(FarmOperation::Delete);
                local.pending = None;
                local.next_send_ms = 0;
                local
            }
            _ => return Err(invalid("unsupported farm request")),
        };
        local.publication = self.author(
            entry,
            &administrator,
            Body::PublicationSet(PublicationSet {
                farm_id: local.id(),
                upload_key: Keypair::from_seed(local.seed).public(),
                visibility: local.desired,
                policy: local.policy.clone(),
            }),
            None,
            now,
            &mut tx,
        )?;
        tx.local(write(goal, &local)).touch(goal);
        let response = Response::FarmPreview(FarmPreview {
            snapshot: None,
            policy: Some(local.policy.clone()),
            status: Some(status(entry, &local)),
        });
        Ok(Planned { response, tx })
    }
    fn public_harness(&self, agent: PublicKey) -> Harness {
        let bindings: BTreeSet<_> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.principal == agent)
            .filter_map(|(_, s)| s.record.as_ref())
            .map(|(r, _)| r.harness)
            .collect();
        if bindings.len() > 1 {
            Harness::Multiple
        } else {
            bindings.into_iter().next().unwrap_or(Harness::Unknown)
        }
    }
    pub(super) fn farm_poll_local(&mut self, now: u64) -> Vec<FarmUpload> {
        if self.failed {
            return vec![];
        }
        let goals: Vec<_> = self.goals.keys().copied().collect();
        let mut uploads = vec![];
        for goal in goals {
            let entry = &self.goals[&goal];
            let Some(mut local) = entry.local.farm.clone() else {
                continue;
            };
            if local.deleted {
                continue;
            }
            if local.next_send_ms.saturating_sub(now) > 60_000 {
                local.next_send_ms = now;
            }

            for (position, _) in entry.feed.after(local.observed_feed, usize::MAX) {
                local.observed.insert(position, now);
            }
            local.observed_feed = entry.feed.len();
            let eligibility =
                eligible(entry, &local.policy).and_then(|_| project(entry, &mut local).map(|_| ()));
            if local.desired.is_some()
                && eligibility.is_err()
                && (!local.suspended
                    || local.pending.as_ref().is_some_and(|r| {
                        matches!(r.operation, FarmOperation::Upload | FarmOperation::CheckIn)
                    }))
                && local.control != Some(FarmOperation::Suspend)
            {
                local.control = Some(FarmOperation::Suspend);
                local.pending = None;
                local.next_send_ms = 0;
            }
            if local.desired.is_some() && eligibility.is_ok() && local.suspended {
                local.last_snapshot = None;
            }
            let mut send = None;
            if now >= local.next_send_ms {
                if local.pending.is_none() {
                    let operation_body = if let Some(control) = local.control {
                        Some((control, "{}".into()))
                    } else if local.desired.is_some() && eligibility.is_ok() {
                        match project(entry, &mut local) {
                            Ok(snapshot) => {
                                let body = serde_json::to_string(&FarmUploadBody {
                                    visibility: local.desired.unwrap(),
                                    snapshot: snapshot.clone(),
                                })
                                .expect("snapshot JSON");
                                let digest = crypto::content_hash(body.as_bytes()).to_string();
                                if local.last_snapshot.as_ref() != Some(&digest) {
                                    local.last_snapshot = Some(digest);
                                    Some((FarmOperation::Upload, body))
                                } else if snapshot.goal_state == FarmGoalState::Open
                                    && local.acknowledged_at_ms.is_none_or(|ack| {
                                        now < ack || now.saturating_sub(ack) >= 30_000
                                    })
                                {
                                    Some((FarmOperation::CheckIn, "{}".into()))
                                } else {
                                    None
                                }
                            }
                            Err(error) => {
                                local.last_error = Some(error);
                                None
                            }
                        }
                    } else {
                        None
                    };
                    if let Some((operation, body)) = operation_body {
                        if let Some(next) = local.next_sequence.checked_add(1) {
                            local.pending = Some(SignedFarmRequest::sign(
                                &Keypair::from_seed(local.seed),
                                operation,
                                local.next_sequence,
                                body,
                            ));
                            local.next_sequence = next;
                        } else {
                            local.last_error = Some("publisher sequence exhausted".into());
                        }
                    }
                }
                if let Some(request) = &local.pending {
                    send = Some(FarmUpload {
                        goal,
                        base_url: local.base_url.clone(),
                        request: request.clone(),
                    });
                    local.next_send_ms = now.saturating_add(local.retry_ms);
                }
            }
            if self.goals[&goal].local.farm.as_ref() == Some(&local) {
                continue;
            }
            let mut tx = Tx::none();
            tx.local(write(goal, &local));
            if self.land_once(tx).is_err() {
                self.failed = true;
                return vec![];
            }
            if let Some(send) = send {
                uploads.push(send);
            }
        }
        uploads
    }
    pub(super) fn farm_complete_local(
        &mut self,
        result: FarmUploadResult,
        now: u64,
    ) -> Result<(), ApiError> {
        let Some(entry) = self.goals.get(&result.goal) else {
            return Ok(());
        };
        let Some(mut local) = entry.local.farm.clone() else {
            return Ok(());
        };
        let Some(pending) = local.pending.as_ref() else {
            return Ok(());
        };
        if result.farm_id != pending.farm_id || result.sequence != pending.sequence {
            return Ok(());
        }
        match result.outcome {
            Ok(receipt) => {
                if receipt.farm_id != pending.farm_id
                    || receipt.sequence != pending.sequence
                    || receipt.request_digest != pending.request_digest()
                {
                    return Err(invalid("farm receipt does not match pending request"));
                }
                match pending.operation {
                    FarmOperation::Delete => {
                        local.deleted = true;
                        local.control = None;
                    }
                    FarmOperation::Suspend => {
                        local.suspended = true;
                        local.control = None;
                        local.last_snapshot = None;
                    }
                    FarmOperation::Upload => local.suspended = false,
                    FarmOperation::CheckIn => {}
                }
                local.receipt = Some(receipt);
                local.acknowledged_at_ms = Some(now);
                local.pending = None;
                local.last_error = None;
                local.retry_ms = 1000;
                local.next_send_ms = now.saturating_add(2000);
            }
            Err(error) => {
                local.last_error = Some(error);
                local.retry_ms = (local.retry_ms.saturating_mul(2)).min(60_000);
                local.next_send_ms = now.saturating_add(local.retry_ms);
            }
        }
        let mut tx = Tx::none();
        tx.local(write(result.goal, &local));
        self.land_once(tx)
    }
}
