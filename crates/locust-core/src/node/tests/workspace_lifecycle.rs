//! Signed workspace publication and durable operation receipts through Engine.
//! Memory stores and synthetic identities do not establish real-host transport.

use super::*;

use locust_proto::api::{
    WorkspaceAuthority, WorkspaceCandidate, WorkspaceContent, WorkspaceOperation,
    WorkspaceOperationKind, WorkspaceOperationState, WorkspaceView,
};
use locust_proto::event::{
    Body, Context, DecisionAction, Event, Header, Scope, WorkspaceCheckpoint,
};
use locust_proto::id::{BlobHash, EventId, GoalId, WorkspaceOperationId};
use locust_proto::manifest::{Entry as ManifestEntry, Manifest};
use locust_proto::organization::{Authority, CompletionRule, Formation, Selector, WorkspacePolicy};
use locust_proto::store::Store;

pub(super) fn setup() -> (Daemon, PublicKey, ConnId, ConnId, GoalId) {
    let (mut daemon, principal, owner, agent, goal) = lifecycle::setup();
    let formation = Formation {
        workspace: Some(WorkspacePolicy {
            integrator: Authority::Participant {
                key: principal.to_string(),
            },
            completion: CompletionRule::Declaration {
                by: Selector::Members,
            },
        }),
        ..Formation::default()
    };
    let expected = daemon.node.goals[&goal].state().current_rules.unwrap();
    let rules = lifecycle::event(daemon.ok(
        owner,
        Request::RulesBind {
            no_role: false,
            goal,
            expected,
            formation_json: serde_json::to_string(&formation).unwrap(),

            inputs: Default::default(),
        },
    ));
    daemon.ok(
        owner,
        Request::WorkspaceEpochSet {
            goal,
            expected_epoch: None,
            rules,
            checkpoint: WorkspaceCheckpoint::Unseeded,
        },
    );
    (daemon, principal, owner, agent, goal)
}

fn view(daemon: &mut Daemon, agent: ConnId, goal: GoalId) -> WorkspaceView {
    let Response::Workspace(view) = daemon.ok(agent, Request::WorkspaceHead { goal }) else {
        panic!()
    };
    view
}

fn content(daemon: &mut Daemon, agent: ConnId, goal: GoalId, bytes: Vec<u8>) -> BlobHash {
    let Response::BlobStored { hash } = daemon.ok(agent, Request::BlobPut { goal, bytes }) else {
        panic!()
    };
    hash
}

fn get_content(daemon: &mut Daemon, agent: ConnId, goal: GoalId, hash: BlobHash) -> Vec<u8> {
    let Response::Blob { bytes } = daemon.ok(agent, Request::BlobGet { goal, hash }) else {
        panic!()
    };
    bytes
}

fn tree(daemon: &mut Daemon, agent: ConnId, goal: GoalId, bytes: &[u8]) -> BlobHash {
    let file = content(daemon, agent, goal, bytes.to_vec());
    let manifest = Manifest {
        entries: vec![ManifestEntry {
            path: "tree.txt".into(),
            executable: false,
            size: bytes.len() as u64,
            content: file,
        }],
    };
    content(daemon, agent, goal, manifest.encode().unwrap())
}

fn operation(tag: u8, kind: WorkspaceOperationKind) -> WorkspaceOperation {
    WorkspaceOperation {
        id: WorkspaceOperationId([tag; 16]),
        checkout: None,
        idempotency_key: IdempotencyKey([tag; 16]),
        kind,
        state: WorkspaceOperationState::Prepared,
    }
}

fn capture(
    daemon: &mut Daemon,
    agent: ConnId,
    goal: GoalId,
    tag: u8,
    manifest: BlobHash,
) -> WorkspaceOperation {
    let current = view(daemon, agent, goal);
    let operation = operation(
        tag,
        WorkspaceOperationKind::Capture {
            candidate: WorkspaceCandidate {
                context: Context {
                    scope: Scope::Workspace,
                    round: current.epoch.unwrap(),
                },
                parent: current.head.map(|head| head.revision),
                result_manifest: manifest,
                sources: Vec::new(),
                captured_paths: vec!["tree.txt".into()],
                replacement: false,
            },
        },
    );
    let Caller::Agent(principal) = daemon.node.conns[&agent].caller else {
        panic!()
    };
    let owner = daemon.owner();
    daemon
        .on_behalf(
            owner,
            principal,
            Request::WorkspaceOperationPrepare {
                goal,
                operation: operation.clone(),
            },
        )
        .unwrap();
    operation
}

fn recorded(response: Response) -> (WorkspaceOperation, EventId) {
    let Response::WorkspaceOperation(operation) = response else {
        panic!("{response:?}")
    };
    let WorkspaceOperationState::Recorded { event } = operation.state else {
        panic!("{operation:?}")
    };
    (operation, event)
}

fn publish(
    daemon: &mut Daemon,
    agent: ConnId,
    goal: GoalId,
    operation: &WorkspaceOperation,
) -> EventId {
    recorded(daemon.ok(
        agent,
        Request::WorkspacePublish {
            goal,
            operation: operation.id,
        },
    ))
    .1
}

fn declare(daemon: &mut Daemon, agent: ConnId, goal: GoalId, subject: EventId) {
    daemon.ok(agent, Request::CompletionDeclare { goal, subject });
}

fn prepare_integration(
    daemon: &mut Daemon,
    agent: ConnId,
    goal: GoalId,
    tag: u8,
    proposal: EventId,
) -> WorkspaceOperation {
    let current = view(daemon, agent, goal);
    let operation = operation(
        tag,
        WorkspaceOperationKind::Integrate {
            expected_epoch: current.epoch.unwrap(),
            expected_head: current.head.map(|head| head.revision),
            proposal,
        },
    );
    daemon.ok(
        agent,
        Request::WorkspaceOperationPrepare {
            goal,
            operation: operation.clone(),
        },
    );
    operation
}

pub(super) fn accepted_tree(
    daemon: &mut Daemon,
    agent: ConnId,
    goal: GoalId,
    tag: u8,
) -> (EventId, BlobHash) {
    let manifest = tree(daemon, agent, goal, format!("snapshot {tag}").as_bytes());
    let capture = capture(daemon, agent, goal, tag, manifest);
    let proposal = publish(daemon, agent, goal, &capture);
    declare(daemon, agent, goal, proposal);
    let integrate = prepare_integration(daemon, agent, goal, tag + 100, proposal);
    let revision = recorded(daemon.ok(
        agent,
        Request::WorkspaceIntegrate {
            goal,
            operation: integrate.id,
        },
    ))
    .1;
    (revision, manifest)
}

#[test]
fn publication_declaration_and_integration_are_distinct_durable_steps() {
    let (mut daemon, _, _, agent, goal) = setup();
    let (initial, _) = accepted_tree(&mut daemon, agent, goal, 20);
    let manifest = tree(&mut daemon, agent, goal, b"seed files");
    let captured = capture(&mut daemon, agent, goal, 1, manifest);
    let proposal = publish(&mut daemon, agent, goal, &captured);
    let before = view(&mut daemon, agent, goal);
    assert_eq!(before.authority, WorkspaceAuthority::Ready);
    assert_eq!(before.head.unwrap().revision, initial);
    let Response::WorkspaceProposal(candidate) =
        daemon.ok(agent, Request::WorkspaceProposal { goal, proposal })
    else {
        panic!()
    };
    assert!(!candidate.approved);
    assert_eq!(candidate.result_manifest, manifest);
    assert_eq!(
        candidate.content,
        WorkspaceContent::Complete {
            files: 1,
            bytes: 10
        }
    );
    let integration = prepare_integration(&mut daemon, agent, goal, 101, proposal);
    assert_eq!(
        code(daemon.call(
            agent,
            Request::WorkspaceIntegrate {
                goal,
                operation: integration.id
            }
        )),
        ErrorCode::Conflict
    );
    declare(&mut daemon, agent, goal, proposal);
    assert_eq!(
        view(&mut daemon, agent, goal).head.unwrap().revision,
        initial
    );
    let (receipt, revision) = recorded(daemon.ok(
        agent,
        Request::WorkspaceIntegrate {
            goal,
            operation: integration.id,
        },
    ));
    let accepted = view(&mut daemon, agent, goal);
    assert_eq!(accepted.head.as_ref().unwrap().revision, revision);
    assert_eq!(accepted.head.as_ref().unwrap().result_manifest, manifest);
    assert_eq!(
        accepted.content,
        Some(WorkspaceContent::Complete {
            files: 1,
            bytes: 10
        })
    );
    daemon.restart();
    let agent = daemon.connect(credential(1), None);
    assert_eq!(view(&mut daemon, agent, goal), accepted);
    assert_eq!(
        daemon.ok(
            agent,
            Request::WorkspaceIntegrate {
                goal,
                operation: integration.id
            }
        ),
        Response::WorkspaceOperation(receipt)
    );
}

#[test]
fn stale_integration_cas_refuses_without_new_event_or_receipt() {
    let (mut daemon, _, _, agent, goal) = setup();
    let (_, _) = accepted_tree(&mut daemon, agent, goal, 1);
    let manifest = tree(&mut daemon, agent, goal, b"stale proposal");
    let capture = capture(&mut daemon, agent, goal, 2, manifest);
    let proposal = publish(&mut daemon, agent, goal, &capture);
    declare(&mut daemon, agent, goal, proposal);
    let stale = prepare_integration(&mut daemon, agent, goal, 102, proposal);
    let (new_head, _) = accepted_tree(&mut daemon, agent, goal, 3);
    let before = daemon.store.log(&goal, 0, usize::MAX).unwrap();
    assert_eq!(
        code(daemon.call(
            agent,
            Request::WorkspaceIntegrate {
                goal,
                operation: stale.id
            }
        )),
        ErrorCode::Conflict
    );
    assert_eq!(daemon.store.log(&goal, 0, usize::MAX).unwrap(), before);
    assert_eq!(
        view(&mut daemon, agent, goal).head.unwrap().revision,
        new_head
    );
    assert_eq!(
        daemon.ok(
            agent,
            Request::WorkspaceOperationShow {
                goal,
                operation: stale.id
            }
        ),
        Response::WorkspaceOperation(stale)
    );
}

#[test]
fn publication_lost_reply_retries_the_frozen_candidate_after_restart_and_new_capture_bytes() {
    let (mut daemon, _, _, agent, goal) = setup();
    let original = tree(&mut daemon, agent, goal, b"frozen candidate");
    let captured = capture(&mut daemon, agent, goal, 1, original);
    let mut frame = daemon.frame(Request::WorkspacePublish {
        goal,
        operation: captured.id,
    });
    frame.idempotency = Some(captured.idempotency_key);
    let (receipt, proposal) = recorded(daemon.send(agent, frame.clone()).unwrap());
    // Treat the response as lost by resolving only the persisted request on reopen.
    daemon.restart();
    let agent = daemon.connect(credential(1), None);
    let changed = tree(
        &mut daemon,
        agent,
        goal,
        b"different live bytes after lost reply",
    );
    assert_ne!(changed, original);
    let before = daemon.store.log(&goal, 0, usize::MAX).unwrap();
    assert_eq!(
        daemon.send(agent, frame).unwrap(),
        Response::WorkspaceOperation(receipt.clone())
    );
    assert_eq!(
        daemon.ok(
            agent,
            Request::WorkspacePublish {
                goal,
                operation: captured.id
            }
        ),
        Response::WorkspaceOperation(receipt.clone())
    );
    assert_eq!(
        daemon.ok(
            agent,
            Request::WorkspaceOperationShow {
                goal,
                operation: captured.id
            }
        ),
        Response::WorkspaceOperation(receipt)
    );
    assert_eq!(daemon.store.log(&goal, 0, usize::MAX).unwrap(), before);
    let Response::WorkspaceProposal(proposal) =
        daemon.ok(agent, Request::WorkspaceProposal { goal, proposal })
    else {
        panic!()
    };
    assert_eq!(proposal.result_manifest, original);
    let mut replacement = captured;
    let WorkspaceOperationKind::Capture { candidate } = &mut replacement.kind else {
        unreachable!()
    };
    candidate.result_manifest = changed;
    assert_eq!(
        code(daemon.call(
            agent,
            Request::WorkspaceOperationPrepare {
                goal,
                operation: replacement
            }
        )),
        ErrorCode::Conflict
    );
}

#[test]
fn integration_lost_reply_returns_its_original_revision_after_restart_and_head_movement() {
    let (mut daemon, _, _, agent, goal) = setup();
    let manifest = tree(&mut daemon, agent, goal, b"first result");
    let capture = capture(&mut daemon, agent, goal, 1, manifest);
    let proposal = publish(&mut daemon, agent, goal, &capture);
    declare(&mut daemon, agent, goal, proposal);
    let integration = prepare_integration(&mut daemon, agent, goal, 101, proposal);
    let mut frame = daemon.frame(Request::WorkspaceIntegrate {
        goal,
        operation: integration.id,
    });
    frame.idempotency = Some(integration.idempotency_key);
    let (receipt, revision) = recorded(daemon.send(agent, frame.clone()).unwrap());
    daemon.restart();
    let agent = daemon.connect(credential(1), None);
    let (new_head, _) = accepted_tree(&mut daemon, agent, goal, 2);
    assert_ne!(new_head, revision);
    let before = daemon.store.log(&goal, 0, usize::MAX).unwrap();
    assert_eq!(
        daemon.send(agent, frame).unwrap(),
        Response::WorkspaceOperation(receipt.clone())
    );
    assert_eq!(
        daemon.ok(
            agent,
            Request::WorkspaceIntegrate {
                goal,
                operation: integration.id
            }
        ),
        Response::WorkspaceOperation(receipt)
    );
    assert_eq!(daemon.store.log(&goal, 0, usize::MAX).unwrap(), before);
    assert_eq!(
        view(&mut daemon, agent, goal).head.unwrap().revision,
        new_head
    );
}

fn receive_signed(daemon: &mut Daemon, principal: PublicKey, goal: GoalId, body: Body) -> EventId {
    use crate::sync::Host;
    let next = daemon.node.goals[&goal].goal.next(&principal).unwrap();
    let event = Event::sign(
        Header {
            version: locust_proto::PROTOCOL_VERSION,
            goal,
            author: principal,
            seq: next.seq,
            prev: next.prev,
            anchor: Some(next.anchor),
            parents: Vec::new(),
            at_ms: 1000,
            payload: None,
            body,
        },
        daemon.node.signer(&principal).unwrap(),
    )
    .unwrap();
    assert_eq!(
        Host::replica(&mut daemon.node, &goal)
            .unwrap()
            .receive(vec![event.to_wire()]),
        Ok(1)
    );
    event.id()
}

#[test]
fn replica_can_select_invalid_content_but_head_reports_it_and_replacement_repairs_it() {
    let (mut daemon, principal, _, agent, goal) = setup();
    let manifest = content(
        &mut daemon,
        agent,
        goal,
        b"authenticated malformed manifest".to_vec(),
    );
    let current = view(&mut daemon, agent, goal);
    let context = Context {
        scope: Scope::Workspace,
        round: current.epoch.unwrap(),
    };
    // Bypass the normal authoring helper using authentic replicated signed events.
    let proposal = receive_signed(
        &mut daemon,
        principal,
        goal,
        Body::WorkspaceProposed {
            context,
            parent: None,
            result_manifest: manifest,
            sources: Vec::new(),
        },
    );
    declare(&mut daemon, agent, goal, proposal);
    let evidence = daemon.node.goals[&goal].state().workspace_proposals[&proposal]
        .evidence
        .iter()
        .copied()
        .collect();
    let revision = receive_signed(
        &mut daemon,
        principal,
        goal,
        Body::ScopeDecided {
            context,
            previous: None,
            action: DecisionAction::Select { subject: proposal },
            evidence,
        },
    );
    let accepted = view(&mut daemon, agent, goal);
    assert_eq!(accepted.authority, WorkspaceAuthority::Ready);
    assert_eq!(accepted.head.as_ref().unwrap().revision, revision);
    assert!(
        matches!(accepted.content, Some(WorkspaceContent::InvalidManifest { manifest: id, .. }) if id == manifest)
    );
    daemon.restart();
    let agent = daemon.connect(credential(1), None);
    assert_eq!(view(&mut daemon, agent, goal), accepted);
    let (repaired, repaired_manifest) = accepted_tree(&mut daemon, agent, goal, 2);
    let after = view(&mut daemon, agent, goal);
    assert_eq!(after.head.as_ref().unwrap().revision, repaired);
    assert_eq!(after.head.as_ref().unwrap().parent, Some(revision));
    assert_eq!(after.head.unwrap().result_manifest, repaired_manifest);
    assert!(matches!(
        after.content,
        Some(WorkspaceContent::Complete { .. })
    ));
    assert!(
        daemon.node.goals[&goal]
            .state()
            .workspace_lineage
            .contains(&revision)
    );
}

#[test]
fn encoded_two_daemon_workspace_edit_retains_files_and_converges_after_both_restart() {
    let mut net = delivery::Network::new();
    let integrator = net.nodes[0].enroll("integrator", 1);
    let worker = net.nodes[1].enroll("worker", 2);
    let left = net.nodes[0].connect(credential(1), None);
    let right = net.nodes[1].connect(credential(2), None);
    let owner = net.nodes[0].owner();
    let formation = Formation {
        workspace: Some(WorkspacePolicy {
            integrator: Authority::Participant {
                key: integrator.to_string(),
            },
            completion: CompletionRule::Declaration {
                by: Selector::Members,
            },
        }),
        ..Formation::default()
    };
    let Response::GoalCreated { goal } = net.nodes[0].ok(
        owner,
        Request::GoalCreate {
            name: "host".into(),
            agent: integrator,
            title: "Shared workspace transport".into(),
            formation_json: Some(serde_json::to_string(&formation).unwrap()),

            inputs: Default::default(),
        },
    ) else {
        panic!()
    };
    net.nodes[0].ok(
        owner,
        Request::LevelSet {
            goal,
            agent: integrator,
            level: locust_proto::api::Level::Ask,
        },
    );
    let Response::Invited { ticket } = net.nodes[0].ok(
        owner,
        Request::GoalInvite {
            role: None,
            goal,
            expires_ms: 604_801_000,
        },
    ) else {
        panic!()
    };
    let joining_owner = net.nodes[1].owner();
    net.nodes[1].ok(
        joining_owner,
        Request::GoalJoin {
            name: "member".into(),
            agent: worker,
            ticket,
            level: locust_proto::api::Level::Auto,
        },
    );
    net.poll(1);
    assert!(net.nodes[0].node.goals[&goal].is_member(&worker));
    let owner = net.nodes[1].owner();
    net.nodes[1].ok(
        owner,
        Request::LevelSet {
            goal,
            agent: worker,
            level: locust_proto::api::Level::Ask,
        },
    );
    let rules = net.nodes[0].node.goals[&goal]
        .state()
        .current_rules
        .unwrap();
    let host_owner = net.nodes[0].owner();
    net.nodes[0].ok(
        host_owner,
        Request::WorkspaceEpochSet {
            goal,
            expected_epoch: None,
            rules,
            checkpoint: WorkspaceCheckpoint::Unseeded,
        },
    );
    let (seed, seed_manifest) = accepted_tree(&mut net.nodes[0], left, goal, 1);
    net.poll(1);
    assert_eq!(
        view(&mut net.nodes[0], left, goal),
        view(&mut net.nodes[1], right, goal)
    );
    let base =
        Manifest::decode(&get_content(&mut net.nodes[1], right, goal, seed_manifest)).unwrap();
    let retained = base.entries[0].clone();
    // Blobs are fetched in hash order, one exchange's cursor never going back.
    // The child must sort before the manifest that names it, so the exchange
    // that discovers the manifest has already passed the child by. Sealing is
    // deterministic, so the file's last byte is varied until that holds.
    let (notes_bytes, notes, manifest, result_manifest) = (b'0'..=b'9')
        .chain(b'a'..=b'z')
        .find_map(|last| {
            let notes_bytes = [&b"remote note"[..], &[last]].concat();
            let notes = content(&mut net.nodes[1], right, goal, notes_bytes.clone());
            let mut manifest = base.clone();
            manifest.entries.push(ManifestEntry {
                path: "notes.txt".into(),
                executable: true,
                size: 12,
                content: notes,
            });
            manifest.entries.sort_by(|a, b| a.path.cmp(&b.path));
            let result_manifest =
                content(&mut net.nodes[1], right, goal, manifest.encode().unwrap());
            (notes < result_manifest).then_some((notes_bytes, notes, manifest, result_manifest))
        })
        .expect("some child sorts before its manifest");
    let current = view(&mut net.nodes[1], right, goal);
    let checkout = locust_proto::api::Checkout {
        id: locust_proto::id::CheckoutId([33; 16]),
        root: "/work/remote-checkout".into(),
        root_identity: locust_proto::api::DirectoryIdentity {
            device: 1,
            inode: 33,
        },
        base_revision: seed,
        base_manifest: seed_manifest,
        session: None,
        task: None,
        attempt: None,
        active_operation: None,
    };
    net.nodes[1].ok(
        owner,
        Request::WorkspaceConnect {
            goal,
            agent: worker,
            checkout: checkout.clone(),
        },
    );
    let mut capture = operation(
        1,
        WorkspaceOperationKind::Capture {
            candidate: WorkspaceCandidate {
                context: Context {
                    scope: Scope::Workspace,
                    round: current.epoch.unwrap(),
                },
                parent: Some(seed),
                result_manifest,
                sources: Vec::new(),
                captured_paths: vec!["notes.txt".into()],
                replacement: false,
            },
        },
    );
    capture.checkout = Some(checkout.id);
    net.nodes[1].ok(
        right,
        Request::WorkspaceOperationPrepare {
            goal,
            operation: capture.clone(),
        },
    );
    let proposal = publish(&mut net.nodes[1], right, goal, &capture);
    declare(&mut net.nodes[1], right, goal, proposal);
    assert_eq!(
        view(&mut net.nodes[0], left, goal).head.unwrap().revision,
        seed
    );
    net.poll(1);
    let candidate = net.nodes[0].ok(left, Request::WorkspaceProposal { goal, proposal });
    assert!(
        matches!(&candidate, Response::WorkspaceProposal(candidate)
        if candidate.approved
            && matches!(&candidate.content, WorkspaceContent::FilesMissing { missing, .. } if missing == &vec![notes])),
        "{candidate:?}"
    );
    let integration = prepare_integration(&mut net.nodes[0], left, goal, 102, proposal);
    assert_eq!(
        code(net.nodes[0].call(
            left,
            Request::WorkspaceIntegrate {
                goal,
                operation: integration.id
            }
        )),
        ErrorCode::Unavailable
    );
    // A second anti-entropy exchange fetches the child without interpreting
    // its absence as authority.
    net.poll(crate::sync::ANTI_ENTROPY_MS + 1);
    let candidate = net.nodes[0].ok(left, Request::WorkspaceProposal { goal, proposal });
    assert!(
        matches!(&candidate, Response::WorkspaceProposal(candidate)
        if matches!(candidate.content, WorkspaceContent::Complete { files: 2, bytes: 22 })),
        "{candidate:?}"
    );
    let revision = recorded(net.nodes[0].ok(
        left,
        Request::WorkspaceIntegrate {
            goal,
            operation: integration.id,
        },
    ))
    .1;
    net.poll(1);
    let accepted = view(&mut net.nodes[0], left, goal);
    assert_eq!(accepted.authority, WorkspaceAuthority::Ready);
    assert_eq!(accepted.head.as_ref().unwrap().revision, revision);
    assert_eq!(accepted.head.as_ref().unwrap().parent, Some(seed));
    assert_eq!(
        accepted.head.as_ref().unwrap().result_manifest,
        result_manifest
    );
    assert_eq!(
        accepted.content,
        Some(WorkspaceContent::Complete {
            files: 2,
            bytes: 22,
        })
    );
    assert_eq!(view(&mut net.nodes[1], right, goal), accepted);
    for (index, agent) in [(0, left), (1, right)] {
        assert_eq!(
            get_content(&mut net.nodes[index], agent, goal, retained.content),
            b"snapshot 1"
        );
        assert_eq!(
            get_content(&mut net.nodes[index], agent, goal, notes),
            notes_bytes
        );
        assert_eq!(
            Manifest::decode(&get_content(
                &mut net.nodes[index],
                agent,
                goal,
                result_manifest
            ))
            .unwrap(),
            manifest
        );
    }
    let host_owner = net.nodes[0].owner();
    let new_rules = bind_peer_files(&mut net.nodes[0], host_owner, integrator, goal);
    net.poll(1);
    let carried = view(&mut net.nodes[0], left, goal);
    assert_eq!(carried.head, accepted.head);
    assert_ne!(carried.epoch, accepted.epoch);
    assert_eq!(view(&mut net.nodes[1], right, goal), carried);
    assert_eq!(
        net.nodes[1].node.goals[&goal].state().current_rules,
        Some(new_rules)
    );
    let accepted = carried;
    net.restart();
    let left = net.nodes[0].connect(credential(1), None);
    let right = net.nodes[1].connect(credential(2), None);
    net.poll(1);
    for (index, agent) in [(0, left), (1, right)] {
        assert_eq!(view(&mut net.nodes[index], agent, goal), accepted);
        assert_eq!(
            get_content(&mut net.nodes[index], agent, goal, retained.content),
            b"snapshot 1"
        );
        assert_eq!(
            get_content(&mut net.nodes[index], agent, goal, notes),
            notes_bytes
        );
    }
}

fn bind_peer_files(daemon: &mut Daemon, owner: ConnId, host: PublicKey, goal: GoalId) -> EventId {
    let mut formation = locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == "peer-review")
        .unwrap()
        .formation;
    formation.workspace = Some(WorkspacePolicy {
        integrator: Authority::Participant {
            key: host.to_string(),
        },
        completion: formation.decisions.completion.clone(),
    });
    let expected = daemon.node.goals[&goal].state().current_rules.unwrap();
    lifecycle::event(daemon.ok(
        owner,
        Request::RulesBind {
            no_role: false,
            goal,
            expected,
            formation_json: serde_json::to_string(&formation).unwrap(),
            inputs: Default::default(),
        },
    ))
}

#[test]
fn the_host_accepts_its_first_files_with_no_approval_and_its_next_change_waits() {
    let (mut d, host, owner, agent, goal) = setup();
    let (_, reviewer) = super::authorization::join_local(&mut d, agent, goal, 2);
    bind_peer_files(&mut d, owner, host, goal);
    let manifest = tree(&mut d, agent, goal, b"first files");
    let operation = capture(&mut d, agent, goal, 1, manifest);
    let proposal = publish(&mut d, agent, goal, &operation);
    assert!(d.node.goals[&goal].state().workspace_proposals[&proposal].approved);
    let operation = prepare_integration(&mut d, agent, goal, 101, proposal);
    d.ok(
        agent,
        Request::WorkspaceIntegrate {
            goal,
            operation: operation.id,
        },
    );
    let next_manifest = tree(&mut d, agent, goal, b"later files");
    let operation = capture(&mut d, agent, goal, 2, next_manifest);
    let proposal = publish(&mut d, agent, goal, &operation);
    assert!(!d.node.goals[&goal].state().workspace_proposals[&proposal].approved);
    let operation = prepare_integration(&mut d, agent, goal, 102, proposal);
    assert_eq!(
        code(d.call(
            agent,
            Request::WorkspaceIntegrate {
                goal,
                operation: operation.id
            }
        )),
        ErrorCode::Conflict
    );
    d.ok(
        reviewer,
        Request::ReviewRecord {
            goal,
            subject: proposal,
            verdict: locust_proto::event::ReviewVerdict::Approve,
            text: "checked".into(),
        },
    );
    d.ok(
        agent,
        Request::WorkspaceIntegrate {
            goal,
            operation: operation.id,
        },
    );
    assert_eq!(
        view(&mut d, agent, goal).head.unwrap().result_manifest,
        next_manifest
    );
}

#[test]
fn rules_bind_signs_the_binding_and_the_epoch_in_one_commit() {
    let (mut d, host, owner, agent, goal) = setup();
    let (head, manifest) = accepted_tree(&mut d, agent, goal, 1);
    let (_, reviewer) = super::authorization::join_local(&mut d, agent, goal, 2);
    let old_epoch = d.node.goals[&goal]
        .state()
        .workspace
        .as_ref()
        .unwrap()
        .epoch;
    let new_manifest = tree(&mut d, agent, goal, b"not yet landed");
    let operation = capture(&mut d, agent, goal, 2, new_manifest);
    let old_proposal = publish(&mut d, agent, goal, &operation);
    declare(&mut d, agent, goal, old_proposal);
    let old_integration = prepare_integration(&mut d, agent, goal, 102, old_proposal);
    let before = d.node.goals[&goal].local.revision;
    let rules = bind_peer_files(&mut d, owner, host, goal);
    // One externally visible revision contains both adjacent governance records.
    assert_eq!(d.node.goals[&goal].local.revision, before + 1);
    let workspace = d.node.goals[&goal].state().workspace.as_ref().unwrap();
    let epoch = workspace.epoch;
    assert_ne!(epoch, old_epoch);
    assert_eq!(workspace.head, Some(head));
    let binding = d.store.event(&rules).unwrap().unwrap();
    let epoch_event = d.store.event(&epoch).unwrap().unwrap();
    assert_eq!(epoch_event.header().prev, Some(rules));
    assert_eq!(epoch_event.header().seq, binding.header().seq + 1);
    assert!(matches!(epoch_event.header().body, Body::WorkspaceEpoch {
        rules: actual, checkpoint: WorkspaceCheckpoint::Revision(revision), ..
    } if actual == rules && revision == head));
    assert_eq!(
        view(&mut d, agent, goal).head.unwrap().result_manifest,
        manifest
    );
    assert_eq!(
        code(d.call(
            agent,
            Request::WorkspaceIntegrate {
                goal,
                operation: old_integration.id
            }
        )),
        ErrorCode::Conflict
    );
    let operation = capture(&mut d, agent, goal, 3, new_manifest);
    let proposal = publish(&mut d, agent, goal, &operation);
    assert!(!d.node.goals[&goal].state().workspace_proposals[&proposal].approved);
    d.ok(
        reviewer,
        Request::ReviewRecord {
            goal,
            subject: proposal,
            verdict: locust_proto::event::ReviewVerdict::Approve,
            text: "new rule met".into(),
        },
    );
    assert!(d.node.goals[&goal].state().workspace_proposals[&proposal].approved);
    d.restart();
    assert_eq!(
        d.node.goals[&goal].state().workspace.as_ref().unwrap().head,
        Some(head)
    );
    assert_eq!(
        d.node.goals[&goal]
            .state()
            .workspace
            .as_ref()
            .unwrap()
            .epoch,
        epoch
    );

    let (mut d, host, owner, _, goal) = lifecycle::setup();
    let before = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    bind_peer_files(&mut d, owner, host, goal);
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before + 1);
    assert!(d.node.goals[&goal].state().workspace.is_none());
}
