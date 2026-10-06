//! Farm authorization, privacy and durable delivery through the actual engine.
use super::lifecycle::{authorize, finding, offered, setup};
use super::*;
use locust_proto::api::{SessionCapabilities, SessionRecord, SessionState};
use locust_proto::event::AttemptStatus;
use locust_proto::farm::*;
use locust_proto::id::GoalId;
fn on(goal: GoalId) -> Request {
    Request::FarmOn {
        goal,
        base_url: "http://127.0.0.1:3001".into(),
        listed: true,
        title: Some("Approved public title".into()),
        formation: "Collaborative build".into(),
        stage_labels: Default::default(),
        role_labels: Default::default(),
        recent_changes: 50,
    }
}
fn consent(goal: GoalId, agent: PublicKey, accept: bool) -> Request {
    Request::FarmConsent {
        goal,
        agent,
        accept,
        name: accept.then(|| "Approved principal".into()),
        group_label: accept.then(|| "Machine A".into()),
    }
}
fn acknowledge(daemon: &mut Daemon, upload: &FarmUpload, now: u64) {
    daemon
        .node
        .farm_complete(
            FarmUploadResult {
                goal: upload.goal,
                farm_id: upload.request.farm_id.clone(),
                sequence: upload.request.sequence,
                outcome: Ok(FarmReceipt {
                    farm_id: upload.request.farm_id.clone(),
                    sequence: upload.request.sequence,
                    request_digest: upload.request.request_digest(),
                    stream_version: upload.request.sequence,
                    received_at_ms: u64::MAX / 2,
                }),
            },
            now,
        )
        .unwrap();
}
fn activate(daemon: &mut Daemon, owner: ConnId, goal: GoalId, principal: PublicKey) -> FarmUpload {
    daemon.ok(owner, on(goal));
    daemon.ok(owner, consent(goal, principal, true));
    let uploads = daemon.node.farm_poll(2000);
    assert_eq!(uploads.len(), 1);
    assert_eq!(uploads[0].request.operation, FarmOperation::Upload);
    uploads.into_iter().next().unwrap()
}
#[test]
fn only_owner_can_publish_or_approve_profile() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    for request in [
        on(goal),
        consent(goal, principal, true),
        Request::FarmOff { goal },
        Request::FarmShow { goal },
        Request::FarmStatus,
    ] {
        assert_eq!(code(daemon.call(agent, request)), ErrorCode::Denied);
    }
    assert_eq!(
        code(daemon.on_behalf(owner, principal, on(goal))),
        ErrorCode::Invalid
    );
    daemon.ok(owner, on(goal));
    assert_eq!(
        daemon.node.goals[&goal].state().publication_consents.len(),
        0
    );
    let uploads = daemon.node.farm_poll(2000);
    assert_eq!(uploads.len(), 1);
    assert_eq!(uploads[0].request.operation, FarmOperation::Suspend);
    let Response::FarmPreview(preview) = daemon.ok(owner, Request::FarmShow { goal }) else {
        panic!()
    };
    assert!(preview.snapshot.is_none());
    assert!(!preview.status.unwrap().eligible);
}
#[test]
fn snapshot_excludes_private_data_and_restart_retries_exact_request() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    daemon.ok(
        agent,
        finding(goal, "PRIVATE_RESULT_CANARY /private/workspace"),
    );
    daemon.ok(
        agent,
        Request::SessionReport {
            record: SessionRecord {
                harness: Harness::Codex,
                client: "PRIVATE_CLIENT_CANARY".into(),
                state: SessionState::Started,
                client_session: Some("PRIVATE_SESSION_CANARY".into()),
                capabilities: SessionCapabilities::default(),
                detail: br#"{"private":"PRIVATE_DETAIL_CANARY"}"#.to_vec(),
            },
        },
    );
    let first = activate(&mut daemon, owner, goal, principal);
    first.request.verify().unwrap();
    let body: FarmUploadBody = serde_json::from_str(&first.request.body).unwrap();
    assert_eq!(body.snapshot.agents[0].harness, Harness::Codex);
    assert_eq!(body.snapshot.agents[0].name, "Approved principal");
    assert!(
        body.snapshot
            .changes
            .iter()
            .all(|c| c.observed_at_ms.is_none() || c.kind == FarmChangeKind::Publication)
    );
    for private in [
        "PRIVATE_RESULT_CANARY",
        "PRIVATE_CLIENT_CANARY",
        "PRIVATE_SESSION_CANARY",
        "PRIVATE_DETAIL_CANARY",
        "governance",
        &principal.to_string(),
        &goal.to_string(),
    ] {
        assert!(
            !first.request.body.contains(private),
            "public JSON exposed {private}"
        );
    }
    daemon.restart();
    let retry = daemon.node.farm_poll(4000);
    assert_eq!(retry.len(), 1);
    assert_eq!(retry[0].request, first.request);
    acknowledge(&mut daemon, &retry[0], 4000);
    assert!(daemon.node.farm_poll(7000).is_empty());
    // Receipt clock is intentionally far in the future: local acknowledgment drives cadence.
    let heartbeat = daemon.node.farm_poll(34_001);
    assert_eq!(heartbeat.len(), 1);
    assert_eq!(heartbeat[0].request.operation, FarmOperation::CheckIn);
    let _ = daemon.node.farm_poll(1_000_000);
    daemon.restart();
    let again = daemon.node.farm_poll(100);
    assert_eq!(again[0].request, heartbeat[0].request);
}
#[test]
fn pending_resume_is_fenced_on_revocation_and_off_survives_restart() {
    let (mut daemon, principal, owner, _, goal) = setup();
    let first = activate(&mut daemon, owner, goal, principal);
    acknowledge(&mut daemon, &first, 2000);
    daemon.ok(owner, consent(goal, principal, false));
    let suspended = daemon.node.farm_poll(4000).remove(0);
    assert_eq!(suspended.request.operation, FarmOperation::Suspend);
    acknowledge(&mut daemon, &suspended, 4000);
    daemon.ok(owner, consent(goal, principal, true));
    let resume = daemon.node.farm_poll(6000).remove(0);
    assert_eq!(resume.request.operation, FarmOperation::Upload);
    daemon.ok(owner, consent(goal, principal, false));
    let revoked = daemon.node.farm_poll(6500).remove(0);
    assert_eq!(revoked.request.operation, FarmOperation::Suspend);
    assert!(revoked.request.sequence > resume.request.sequence);
    acknowledge(&mut daemon, &resume, 7000);
    assert_eq!(
        daemon
            .node
            .farm_poll(7500)
            .into_iter()
            .next()
            .map(|u| u.request.farm_id)
            .unwrap_or(revoked.request.farm_id.clone()),
        revoked.request.farm_id
    );
    daemon.ok(owner, Request::FarmOff { goal });
    assert_eq!(code(daemon.call(owner, on(goal))), ErrorCode::Conflict);
    let delete = daemon.node.farm_poll(8000).remove(0);
    assert_eq!(delete.request.operation, FarmOperation::Delete);
    daemon.restart();
    let retry = daemon.node.farm_poll(10_000).remove(0);
    assert_eq!(retry.request, delete.request);
    acknowledge(&mut daemon, &retry, 10_000);
    let owner = daemon.owner();
    daemon.ok(owner, Request::FarmOff { goal });
    assert!(daemon.node.farm_poll(12_000).is_empty());
    daemon.ok(owner, on(goal));
    let Response::FarmPreview(preview) = daemon.ok(owner, Request::FarmShow { goal }) else {
        panic!()
    };
    assert_ne!(
        preview.status.as_ref().unwrap().farm_id,
        delete.request.farm_id
    );
    assert!(!preview.status.unwrap().eligible);
    assert!(preview.snapshot.is_none());
}
#[test]
fn joining_member_and_removed_work_author_require_consent() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    let first = activate(&mut daemon, owner, goal, principal);
    acknowledge(&mut daemon, &first, 2000);
    let (member, member_conn) = super::authorization::join_local(&mut daemon, agent, goal, 2);
    let suspended = daemon.node.farm_poll(4000).remove(0);
    assert_eq!(suspended.request.operation, FarmOperation::Suspend);
    acknowledge(&mut daemon, &suspended, 4000);
    daemon.ok(owner, consent(goal, member, true));
    daemon.ok(member_conn, finding(goal, "PRIVATE_REMOTE_FINDING"));
    let resumed = daemon.node.farm_poll(6000).remove(0);
    acknowledge(&mut daemon, &resumed, 6000);
    daemon.ok(owner, consent(goal, member, false));
    daemon.ok(owner, Request::MemberRemove { goal, member });
    let Response::FarmPreview(preview) = daemon.ok(owner, Request::FarmShow { goal }) else {
        panic!()
    };
    assert!(!preview.status.unwrap().eligible);
    assert!(preview.snapshot.is_none());
}
#[test]
fn multiple_attempts_are_reported_and_history_observation_is_unknown() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    let (task, offer) = offered(&mut daemon, agent, goal, principal);
    authorize(&mut daemon, owner, goal, task, principal);
    let Response::Claimed(claim) = daemon.ok(
        agent,
        Request::AttemptStart {
            goal,
            task,
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    let first = claim.attempt;
    daemon.ok(
        agent,
        Request::AttemptReport {
            goal,
            attempt: first,
            generation: 1,
            status: AttemptStatus::Failed,
            text: "PRIVATE_ERROR_CANARY".into(),
        },
    );
    let (other_task, other_offer) = offered(&mut daemon, agent, goal, principal);
    authorize(&mut daemon, owner, goal, other_task, principal);
    let _ = daemon.ok(
        agent,
        Request::AttemptStart {
            goal,
            task: other_task,
            offer: Some(other_offer),
        },
    );
    let upload = activate(&mut daemon, owner, goal, principal);
    let body: FarmUploadBody = serde_json::from_str(&upload.request.body).unwrap();
    assert_eq!(body.snapshot.attempts.len(), 2);
    assert_eq!(body.snapshot.agents.len(), 1);
    assert!(
        body.snapshot
            .attempts
            .iter()
            .all(|a| a.agent == body.snapshot.agents[0].id && a.observed_at_ms.is_none())
    );
    assert!(
        body.snapshot
            .attempts
            .iter()
            .any(|a| a.state == FarmAttemptState::Failed)
    );
    assert!(body.snapshot.tasks.iter().all(|t| t.stage.is_none()));
    assert!(!upload.request.body.contains("PRIVATE_ERROR_CANARY"));
}

#[test]
fn invitation_discloses_signed_policy_without_implicitly_consenting() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    activate(&mut daemon, owner, goal, principal);
    let Response::Invited { ticket } = daemon.ok(
        owner,
        Request::GoalInvite {
            goal,
            expires_ms: 604_801_000,
        },
    ) else {
        panic!()
    };
    let Response::InvitationInspected { preview } = daemon.ok(
        owner,
        Request::InvitationInspect {
            ticket: ticket.clone(),
        },
    ) else {
        panic!()
    };
    assert_eq!(
        preview
            .publication
            .as_ref()
            .unwrap()
            .publication
            .policy
            .title
            .as_deref(),
        Some("Approved public title")
    );
    let (member, _) = super::authorization::join_local(&mut daemon, agent, goal, 2);
    assert!(
        !daemon.node.goals[&goal]
            .state()
            .publication_consents
            .contains_key(&member)
    );
    let Response::FarmPreview(preview) = daemon.ok(owner, Request::FarmShow { goal }) else {
        panic!()
    };
    assert!(preview.snapshot.is_none());
}

#[test]
fn join_reconciliation_waits_for_exact_advertised_publication_proof() {
    use super::super::{commit::Tx, local};
    use locust_proto::api::InvitationPublication;
    use locust_proto::id::EventId;
    use locust_proto::invite::InviteSecret;
    let (mut daemon, principal, owner, _, goal) = setup();
    activate(&mut daemon, owner, goal, principal);
    let (event, publication) = daemon.node.goals[&goal]
        .state()
        .publication
        .clone()
        .unwrap();
    let endpoint = daemon.node.goals[&goal].state().members[&principal].endpoint;
    let mut join = local::JoinRecord {
        governance: principal,
        endpoint,
        hints: vec![],
        secret: InviteSecret([71; 32]),
        publication: Some(InvitationPublication {
            event: EventId([99; 32]),
            publication: publication.clone(),
        }),
        refused: false,
    };
    let mut tx = Tx::none();
    tx.local(local::join_write(&goal, &principal, &join));
    daemon.node.land(tx).unwrap();
    let mut tx = Tx::none();
    tx.touch(goal);
    daemon.node.land(tx).unwrap();
    assert!(
        daemon.node.goals[&goal]
            .local
            .joins
            .contains_key(&principal)
    );
    join.publication = Some(InvitationPublication { event, publication });
    let mut tx = Tx::none();
    tx.local(local::join_write(&goal, &principal, &join));
    daemon.node.land(tx).unwrap();
    let mut tx = Tx::none();
    tx.touch(goal);
    daemon.node.land(tx).unwrap();
    assert!(
        !daemon.node.goals[&goal]
            .local
            .joins
            .contains_key(&principal)
    );
}
#[test]
fn unadmitted_work_does_not_leak_through_public_change_counts_or_times() {
    use super::super::commit::Tx;
    use locust_proto::event::{Body, Context, Scope};
    use locust_proto::testkit::Author;
    let (mut daemon, principal, owner, _, goal) = setup();
    let initial = activate(&mut daemon, owner, goal, principal);
    acknowledge(&mut daemon, &initial, 2000);
    let Response::FarmPreview(before) = daemon.ok(owner, Request::FarmShow { goal }) else {
        panic!()
    };
    let entry = &daemon.node.goals[&goal];
    let event = Author::new(77).event(
        goal,
        entry.state().head,
        Body::ContributionPublished {
            context: Context {
                scope: Scope::Goal,
                round: entry.state().current_rules.unwrap(),
            },
            attempt: None,
            sources: vec![],
            artifacts: vec![],
        },
    );
    let mut tx = Tx::none();
    tx.commit.events.push(event);
    daemon.node.land(tx).unwrap();
    assert!(daemon.node.farm_poll(4000).is_empty());
    let Response::FarmPreview(after) = daemon.ok(owner, Request::FarmShow { goal }) else {
        panic!()
    };
    assert_eq!(before.snapshot, after.snapshot);
}

#[test]
fn actual_parallel_flow_projects_approved_dag_and_revised_task_rounds() {
    use locust_proto::event::TaskId;
    use locust_proto::organization::{
        CompletionRule, EvidenceKind, Formation, Prerequisite, Selector, Stage,
    };
    use std::collections::{BTreeMap, BTreeSet};

    fn stage_task(daemon: &Daemon, goal: GoalId, name: &str) -> Option<TaskId> {
        daemon.node.goals[&goal]
            .state()
            .tasks
            .values()
            .find(|task| task.rounds[&task.current_round].binding.stage.as_deref() == Some(name))
            .map(|task| task.id)
    }
    fn publish_stage(daemon: &mut Daemon, agent: ConnId, goal: GoalId, task: TaskId) {
        let owner = daemon.owner();
        let principal = daemon.node.conns[&agent].caller;
        let locust_proto::api::Caller::Agent(principal) = principal else {
            panic!()
        };
        daemon.ok(
            owner,
            Request::TaskAllow {
                goal,
                agent: principal,
                task,
            },
        );
        let Response::Claimed(claim) = daemon.ok(
            agent,
            Request::AttemptStart {
                goal,
                task,
                offer: None,
            },
        ) else {
            panic!()
        };
        daemon.ok(
            agent,
            Request::ContributionPublish {
                goal,
                attempt: Some(claim.attempt),
                generation: Some(claim.generation),
                summary: "PRIVATE_STAGE_RESULT_CANARY".into(),
                sources: vec![],
                artifacts: vec![],
            },
        );
    }
    let (mut daemon, principal, owner, agent, goal) = setup();
    let mut formation = Formation::default();
    formation.context.guidance = "PRIVATE_FORMATION_GUIDANCE_CANARY".into();
    formation.decisions.completion = CompletionRule::Contribution {
        by: Selector::Members,
    };
    let contract = "private-contract-canary";
    let frontend = "private-frontend-canary";
    let backend = "private-backend-canary";
    let integration = "private-integration-canary";
    formation.flow = BTreeMap::from([
        (
            contract.into(),
            Stage {
                recipients: Selector::Members,
                task_type: None,
                requires: vec![],
            },
        ),
        (
            frontend.into(),
            Stage {
                recipients: Selector::Members,
                task_type: None,
                requires: vec![Prerequisite {
                    stage: contract.into(),
                    evidence: EvidenceKind::Completion,
                }],
            },
        ),
        (
            backend.into(),
            Stage {
                recipients: Selector::Members,
                task_type: None,
                requires: vec![Prerequisite {
                    stage: contract.into(),
                    evidence: EvidenceKind::Completion,
                }],
            },
        ),
        (
            integration.into(),
            Stage {
                recipients: Selector::Members,
                task_type: None,
                requires: vec![
                    Prerequisite {
                        stage: frontend.into(),
                        evidence: EvidenceKind::Completion,
                    },
                    Prerequisite {
                        stage: backend.into(),
                        evidence: EvidenceKind::Completion,
                    },
                ],
            },
        ),
    ]);
    let expected = daemon.node.goals[&goal].state().current_rules.unwrap();
    daemon.ok(
        owner,
        Request::RulesBind {
            goal,
            expected,
            formation_json: serde_json::to_string(&formation).unwrap(),
            roles: Default::default(),
            inputs: Default::default(),
        },
    );
    let contract_task =
        stage_task(&daemon, goal, contract).expect("flow materializes initial contract");
    assert!(stage_task(&daemon, goal, frontend).is_none());
    publish_stage(&mut daemon, agent, goal, contract_task);
    let frontend_task =
        stage_task(&daemon, goal, frontend).expect("contract completion materializes frontend");
    let backend_task = stage_task(&daemon, goal, backend)
        .expect("contract completion materializes parallel backend");
    publish_stage(&mut daemon, agent, goal, frontend_task);
    assert!(
        stage_task(&daemon, goal, integration).is_none(),
        "one completed branch is insufficient"
    );
    publish_stage(&mut daemon, agent, goal, backend_task);
    let integration_task =
        stage_task(&daemon, goal, integration).expect("both prerequisites materialize integration");

    let mut publication = on(goal);
    let Request::FarmOn { stage_labels, .. } = &mut publication else {
        unreachable!()
    };
    *stage_labels = BTreeMap::from([
        (contract.into(), "Contract".into()),
        (frontend.into(), "Frontend".into()),
        (backend.into(), "Backend".into()),
        (integration.into(), "Integration".into()),
    ]);
    daemon.ok(owner, publication);
    daemon.ok(owner, consent(goal, principal, true));
    let upload = daemon.node.farm_poll(2000).remove(0);
    let body: FarmUploadBody = serde_json::from_str(&upload.request.body).unwrap();
    body.snapshot.validate().unwrap();
    let stage_id = |label: &str| {
        body.snapshot
            .stages
            .iter()
            .find(|s| s.label == label)
            .unwrap()
            .id
    };
    let contract_id = stage_id("Contract");
    let frontend_id = stage_id("Frontend");
    let backend_id = stage_id("Backend");
    let integration_id = stage_id("Integration");
    assert!(
        body.snapshot
            .stages
            .iter()
            .find(|s| s.id == contract_id)
            .unwrap()
            .prerequisites
            .is_empty()
    );
    for id in [frontend_id, backend_id] {
        assert_eq!(
            body.snapshot
                .stages
                .iter()
                .find(|s| s.id == id)
                .unwrap()
                .prerequisites,
            vec![contract_id]
        );
    }
    assert_eq!(
        body.snapshot
            .stages
            .iter()
            .find(|s| s.id == integration_id)
            .unwrap()
            .prerequisites
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([frontend_id, backend_id])
    );
    assert_eq!(
        body.snapshot
            .tasks
            .iter()
            .filter_map(|t| t.stage)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([contract_id, frontend_id, backend_id, integration_id])
    );
    assert_eq!(body.snapshot.tasks.len(), 4);
    let initial_contract = body
        .snapshot
        .tasks
        .iter()
        .find(|t| t.stage == Some(contract_id))
        .unwrap();
    assert!(initial_contract.completed);
    assert_eq!(
        body.snapshot
            .tasks
            .iter()
            .find(|t| t.stage == Some(integration_id))
            .unwrap()
            .state,
        FarmTaskState::Open
    );
    for private in [
        contract,
        frontend,
        backend,
        integration,
        "PRIVATE_FORMATION_GUIDANCE_CANARY",
        "PRIVATE_STAGE_RESULT_CANARY",
        &integration_task.to_string(),
    ] {
        assert!(
            !upload.request.body.contains(private),
            "public projection leaked {private}"
        );
    }

    // Revising a configured task keeps its public identity/stage, advances its
    // round, and does not transfer acceptance from the old exact candidate.
    let expected_round = daemon.node.goals[&goal].state().tasks[&contract_task].current_round;
    daemon.ok(
        owner,
        Request::TaskRevise {
            goal,
            task: contract_task,
            expected_round,
            task_type: None,
        },
    );
    let Response::FarmPreview(revised) = daemon.ok(owner, Request::FarmShow { goal }) else {
        panic!()
    };
    let snapshot = revised
        .snapshot
        .expect("revised configured task remains publishable");
    let current_contract = snapshot
        .tasks
        .iter()
        .find(|t| t.id == initial_contract.id)
        .unwrap();
    assert_eq!(current_contract.reference, initial_contract.reference);
    assert_eq!(current_contract.stage, initial_contract.stage);
    assert_eq!(current_contract.round, initial_contract.round + 1);
    assert!(!current_contract.completed);
    assert_eq!(current_contract.state, FarmTaskState::Open);
    assert!(
        snapshot
            .candidates
            .iter()
            .filter(|c| c.task == Some(current_contract.id))
            .all(|c| c.round < current_contract.round && !c.selected)
    );
}

#[test]
fn duplicate_upstream_stage_prerequisites_are_deduplicated_in_snapshot() {
    use locust_proto::event::TaskId;
    use locust_proto::organization::{
        CompletionRule, EvidenceKind, Formation, Prerequisite, Selector, Stage,
    };
    use std::collections::{BTreeMap, BTreeSet};

    fn stage_task(daemon: &Daemon, goal: GoalId, name: &str) -> Option<TaskId> {
        daemon.node.goals[&goal]
            .state()
            .tasks
            .values()
            .find(|task| task.rounds[&task.current_round].binding.stage.as_deref() == Some(name))
            .map(|task| task.id)
    }
    fn publish_stage(daemon: &mut Daemon, agent: ConnId, goal: GoalId, task: TaskId) {
        let owner = daemon.owner();
        let principal = daemon.node.conns[&agent].caller;
        let locust_proto::api::Caller::Agent(principal) = principal else {
            panic!()
        };
        daemon.ok(
            owner,
            Request::TaskAllow {
                goal,
                agent: principal,
                task,
            },
        );
        let Response::Claimed(claim) = daemon.ok(
            agent,
            Request::AttemptStart {
                goal,
                task,
                offer: None,
            },
        ) else {
            panic!()
        };
        daemon.ok(
            agent,
            Request::ContributionPublish {
                goal,
                attempt: Some(claim.attempt),
                generation: Some(claim.generation),
                summary: "stage result".into(),
                sources: vec![],
                artifacts: vec![],
            },
        );
    }

    let (mut daemon, principal, owner, agent, goal) = setup();
    let mut formation = Formation::default();
    formation.decisions.completion = CompletionRule::Contribution {
        by: Selector::Members,
    };
    let upstream = "upstream";
    let downstream = "downstream";
    formation.flow = BTreeMap::from([
        (
            upstream.into(),
            Stage {
                recipients: Selector::Members,
                task_type: None,
                requires: vec![],
            },
        ),
        (
            downstream.into(),
            Stage {
                recipients: Selector::Members,
                task_type: None,
                requires: vec![
                    Prerequisite {
                        stage: upstream.into(),
                        evidence: EvidenceKind::Completion,
                    },
                    Prerequisite {
                        stage: upstream.into(),
                        evidence: EvidenceKind::Publication,
                    },
                ],
            },
        ),
    ]);
    let expected = daemon.node.goals[&goal].state().current_rules.unwrap();
    daemon.ok(
        owner,
        Request::RulesBind {
            goal,
            expected,
            formation_json: serde_json::to_string(&formation).unwrap(),
            roles: Default::default(),
            inputs: Default::default(),
        },
    );
    let upstream_task =
        stage_task(&daemon, goal, upstream).expect("flow materializes upstream stage");
    publish_stage(&mut daemon, agent, goal, upstream_task);
    let downstream_task =
        stage_task(&daemon, goal, downstream).expect("both evidence kinds materialize downstream");
    let mut publication = on(goal);
    let Request::FarmOn { stage_labels, .. } = &mut publication else {
        unreachable!()
    };
    *stage_labels = BTreeMap::from([
        (upstream.into(), "Upstream".into()),
        (downstream.into(), "Downstream".into()),
    ]);
    daemon.ok(owner, publication);
    daemon.ok(owner, consent(goal, principal, true));
    let upload = daemon.node.farm_poll(2000).remove(0);
    let body: FarmUploadBody = serde_json::from_str(&upload.request.body).unwrap();
    body.snapshot.validate().unwrap();
    let upstream_id = body
        .snapshot
        .stages
        .iter()
        .find(|s| s.label == "Upstream")
        .unwrap()
        .id;
    let downstream_stage = body
        .snapshot
        .stages
        .iter()
        .find(|s| s.label == "Downstream")
        .unwrap();
    // The same upstream stage required for two evidence kinds projects one
    // edge, not two duplicate prerequisite IDs.
    assert_eq!(downstream_stage.prerequisites, vec![upstream_id]);
    assert_eq!(
        downstream_stage
            .prerequisites
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([upstream_id]),
    );
    let _ = downstream_task;
}

#[path = "farm_characterization.rs"]
mod lifecycle_characterization;
