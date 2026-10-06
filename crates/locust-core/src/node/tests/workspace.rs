//! Durable checkout transitions and the daemon-supplied file capability.
use super::*;
use locust_proto::api::{
    Checkout, DirectoryIdentity, SessionCapabilities, SessionRecord, SessionState,
    WorkspaceOperation, WorkspaceOperationKind, WorkspaceOperationState, WorkspaceRecovery,
};
use locust_proto::id::{BlobHash, CheckoutId, EventId, WorkspaceOperationId};
use std::sync::{Arc, Mutex};

struct Materializer(Arc<Mutex<Vec<CheckoutId>>>);

impl crate::node::CheckoutFiles for Materializer {
    fn destination(&self, id: CheckoutId) -> Result<String, ApiError> {
        Ok(format!("/daemon/checkouts/{id}"))
    }

    fn materialize(
        &self,
        id: CheckoutId,
        manifest: &locust_proto::manifest::Manifest,
        fetch: &mut dyn FnMut(BlobHash) -> Result<Vec<u8>, ApiError>,
    ) -> Result<(String, DirectoryIdentity), ApiError> {
        for entry in &manifest.entries {
            let bytes = fetch(entry.content)?;
            assert_eq!(bytes.len() as u64, entry.size);
            assert_eq!(bytes, b"snapshot 30");
        }
        self.0.lock().unwrap().push(id);
        Ok((
            format!("/daemon/checkouts/{id}"),
            DirectoryIdentity {
                device: 7,
                inode: u64::from(id.0[0]),
            },
        ))
    }
}

#[test]
fn agent_checkout_rejects_an_owner_connected_ancestor_before_materialization() {
    let (mut d, principal, owner, agent, goal) = workspace_lifecycle::setup();
    let (revision, manifest) = workspace_lifecycle::accepted_tree(&mut d, agent, goal, 30);
    let made = Arc::new(Mutex::new(Vec::new()));
    d.node
        .set_checkout_files(Box::new(Materializer(made.clone())));
    let mut existing = checkout(87);
    existing.root = "/daemon/checkouts".into();
    existing.base_revision = revision;
    existing.base_manifest = manifest;
    d.ok(
        owner,
        Request::WorkspaceConnect {
            goal,
            agent: principal,
            checkout: existing,
        },
    );
    assert_eq!(
        code(d.call(
            agent,
            Request::CheckoutRegister {
                goal,
                checkout: CheckoutId([88; 16]),
                revision: None,
                task: None,
                attempt: None,
            }
        )),
        ErrorCode::Conflict
    );
    assert!(made.lock().unwrap().is_empty());
}

#[test]
fn an_agent_registers_its_own_new_checkout_without_the_person_connecting_a_folder() {
    let (mut d, principal, owner, agent, goal) = workspace_lifecycle::setup();
    let (revision, manifest) = workspace_lifecycle::accepted_tree(&mut d, agent, goal, 30);
    let made = Arc::new(Mutex::new(Vec::new()));
    d.node
        .set_checkout_files(Box::new(Materializer(made.clone())));
    let id = CheckoutId([88; 16]);
    let request = Request::CheckoutRegister {
        goal,
        checkout: id,
        revision: None,
        task: None,
        attempt: None,
    };
    let Response::Checkout(bound) = d.ok(agent, request.clone()) else {
        panic!()
    };
    assert_eq!(bound.id, id);
    assert_eq!(bound.base_revision, revision);
    assert_eq!(bound.base_manifest, manifest);
    assert!(bound.root.starts_with("/daemon/checkouts/"));
    assert_eq!(*made.lock().unwrap(), [id]);
    assert_eq!(
        d.ok(agent, request.clone()),
        Response::Checkout(bound.clone())
    );
    assert_eq!(*made.lock().unwrap(), [id]);

    // Choosing a filesystem path is a different, owner-only operation.
    assert_eq!(
        code(d.call(
            agent,
            Request::WorkspaceConnect {
                goal,
                agent: principal,
                checkout: checkout(89),
            }
        )),
        ErrorCode::Denied
    );
    assert_eq!(*made.lock().unwrap(), [id]);

    // Invalid bindings are rejected before the shell is asked to copy files.
    let mut invalid = request;
    if let Request::CheckoutRegister { checkout, task, .. } = &mut invalid {
        *checkout = CheckoutId([90; 16]);
        *task = Some(locust_proto::event::TaskId::Authored(EventId([99; 32])));
    }
    assert_eq!(code(d.call(agent, invalid)), ErrorCode::NotFound);
    assert_eq!(*made.lock().unwrap(), [id]);

    // The registered folder is immediately usable for a first proposal.
    let epoch = d.node.goals[&goal]
        .state()
        .workspace
        .as_ref()
        .unwrap()
        .epoch;
    let operation = WorkspaceOperation {
        id: WorkspaceOperationId([91; 16]),
        checkout: Some(id),
        idempotency_key: IdempotencyKey([91; 16]),
        kind: WorkspaceOperationKind::Capture {
            candidate: locust_proto::api::WorkspaceCandidate {
                context: locust_proto::event::Context {
                    scope: locust_proto::event::Scope::Workspace,
                    round: epoch,
                },
                parent: Some(revision),
                result_manifest: manifest,
                sources: Vec::new(),
                captured_paths: vec!["tree.txt".into()],
                replacement: false,
            },
        },
        state: WorkspaceOperationState::Prepared,
    };
    d.ok(
        agent,
        Request::WorkspaceOperationPrepare {
            goal,
            operation: operation.clone(),
        },
    );
    let response = d.ok(
        agent,
        Request::WorkspacePublish {
            goal,
            operation: operation.id,
        },
    );
    assert!(matches!(
        response,
        Response::WorkspaceOperation(WorkspaceOperation {
            state: WorkspaceOperationState::Recorded { .. },
            ..
        })
    ));

    // An owner-selected folder still uses the named local agent.
    let mut connected = checkout(92);
    connected.base_revision = revision;
    connected.base_manifest = manifest;
    d.ok(
        owner,
        Request::WorkspaceConnect {
            goal,
            agent: principal,
            checkout: connected,
        },
    );
    d.restart();
    let agent = d.connect(credential(1), None);
    let Response::Checkouts(saved) = d.ok(agent, Request::Checkouts { goal }) else {
        panic!()
    };
    assert!(saved.contains(&bound));
}

#[test]
fn capture_requires_the_agents_own_checkout_except_composition_and_owner_capture() {
    use locust_proto::api::WorkspaceCandidate;
    use locust_proto::event::{Context, Scope};

    let (mut d, principal, owner, agent, goal) = workspace_lifecycle::setup();
    let (base, manifest) = workspace_lifecycle::accepted_tree(&mut d, agent, goal, 30);
    let mut bound = checkout(50);
    bound.base_revision = base;
    bound.base_manifest = manifest;
    d.ok(
        owner,
        Request::WorkspaceConnect {
            goal,
            agent: principal,
            checkout: bound.clone(),
        },
    );
    let (other, other_conn) = super::authorization::join_local(&mut d, agent, goal, 2);
    let mut other_bound = bound.clone();
    other_bound.id = CheckoutId([51; 16]);
    other_bound.root = "/work/other-agent".into();
    other_bound.root_identity.inode = 51;
    d.ok(
        owner,
        Request::WorkspaceConnect {
            goal,
            agent: other,
            checkout: other_bound.clone(),
        },
    );
    let (head, head_manifest) = workspace_lifecycle::accepted_tree(&mut d, agent, goal, 31);
    let state = d.node.goals[&goal].state();
    let epoch = state.workspace.as_ref().unwrap().epoch;
    let proposal = state.workspace_revisions[&head].proposal;
    let capture = |tag: u8, checkout, replacement, sources| WorkspaceOperation {
        id: WorkspaceOperationId([tag; 16]),
        checkout,
        idempotency_key: IdempotencyKey([tag; 16]),
        state: WorkspaceOperationState::Prepared,
        kind: WorkspaceOperationKind::Capture {
            candidate: WorkspaceCandidate {
                context: Context {
                    scope: Scope::Workspace,
                    round: epoch,
                },
                parent: Some(head),
                result_manifest: head_manifest,
                sources,
                captured_paths: vec!["tree.txt".into()],
                replacement,
            },
        },
    };
    for replacement in [false, true] {
        let before = d.node.goals[&goal].local.workspace_operations.len();
        let error = d
            .call(
                agent,
                Request::WorkspaceOperationPrepare {
                    goal,
                    operation: capture(60, None, replacement, Vec::new()),
                },
            )
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Denied);
        assert_eq!(
            error.message,
            "sharing files from this computer needs a folder your owner connected to this goal"
        );
        assert_eq!(d.node.goals[&goal].local.workspace_operations.len(), before);
    }
    assert_eq!(
        code(d.call(
            agent,
            Request::WorkspaceOperationPrepare {
                goal,
                operation: capture(61, Some(other_bound.id), true, Vec::new()),
            }
        )),
        ErrorCode::NotFound
    );
    assert_eq!(
        code(d.call(
            other_conn,
            Request::WorkspaceOperationPrepare {
                goal,
                operation: capture(62, Some(bound.id), true, Vec::new()),
            }
        )),
        ErrorCode::NotFound
    );
    assert_eq!(
        code(d.call(
            agent,
            Request::WorkspaceOperationPrepare {
                goal,
                operation: capture(63, Some(bound.id), false, Vec::new()),
            }
        )),
        ErrorCode::Conflict
    );
    d.ok(
        agent,
        Request::WorkspaceOperationPrepare {
            goal,
            operation: capture(64, Some(bound.id), true, Vec::new()),
        },
    );
    d.ok(
        agent,
        Request::WorkspaceOperationPrepare {
            goal,
            operation: capture(65, None, false, vec![proposal]),
        },
    );
    d.on_behalf(
        owner,
        principal,
        Request::WorkspaceOperationPrepare {
            goal,
            operation: capture(66, None, false, Vec::new()),
        },
    )
    .unwrap();
}

fn checkout(tag: u8) -> Checkout {
    Checkout {
        id: CheckoutId([tag; 16]),
        root: format!("/work/checkout-{tag}"),
        root_identity: DirectoryIdentity {
            device: 1,
            inode: u64::from(tag),
        },
        base_revision: EventId([30; 32]),
        base_manifest: BlobHash([31; 32]),
        session: None,
        task: None,
        attempt: None,
        active_operation: None,
    }
}

fn update(checkout: &Checkout, tag: u8) -> WorkspaceOperation {
    WorkspaceOperation {
        id: WorkspaceOperationId([tag; 16]),
        checkout: Some(checkout.id),
        idempotency_key: IdempotencyKey([tag; 16]),
        state: WorkspaceOperationState::Prepared,
        kind: WorkspaceOperationKind::Update {
            expected_revision: checkout.base_revision,
            target_revision: checkout.base_revision,
            target_manifest: checkout.base_manifest,
            recovery: WorkspaceRecovery {
                root: checkout.root.clone(),
                root_identity: checkout.root_identity.clone(),
                recovery_directory: format!("/work/.locust-workspace-{tag}"),
                recovery_identity: DirectoryIdentity {
                    device: 1,
                    inode: u64::from(tag) + 100,
                },
                plan_digest: BlobHash([tag; 32]),
            },
        },
    }
}

#[test]
fn update_registration_survives_restart_and_completion_atomically_advances_binding() {
    let (mut d, principal, owner, agent, goal) = workspace_lifecycle::setup();
    let mut checkout = checkout(1);
    (checkout.base_revision, checkout.base_manifest) =
        workspace_lifecycle::accepted_tree(&mut d, agent, goal, 30);
    d.ok(
        owner,
        Request::WorkspaceConnect {
            goal,
            agent: principal,
            checkout: checkout.clone(),
        },
    );
    let mut operation = update(&checkout, 1);
    let (target_revision, target_manifest) =
        workspace_lifecycle::accepted_tree(&mut d, agent, goal, 40);
    if let WorkspaceOperationKind::Update {
        target_revision: revision,
        target_manifest: manifest,
        ..
    } = &mut operation.kind
    {
        *revision = target_revision;
        *manifest = target_manifest;
    }
    let prepared = d.ok(
        agent,
        Request::WorkspaceOperationPrepare {
            goal,
            operation: operation.clone(),
        },
    );
    d.restart();
    let agent = d.connect(credential(1), None);
    assert_eq!(
        d.ok(
            agent,
            Request::WorkspaceOperationShow {
                goal,
                operation: operation.id
            }
        ),
        prepared
    );
    let conflicting = update(&checkout, 2);
    assert_eq!(
        code(d.call(
            agent,
            Request::WorkspaceOperationPrepare {
                goal,
                operation: conflicting
            }
        )),
        ErrorCode::Conflict
    );
    let completed = d.ok(
        agent,
        Request::WorkspaceOperationComplete {
            goal,
            operation: operation.id,
        },
    );
    d.restart();
    let agent = d.connect(credential(1), None);
    assert_eq!(
        d.ok(
            agent,
            Request::WorkspaceOperationComplete {
                goal,
                operation: operation.id
            }
        ),
        completed
    );
    assert_eq!(
        d.ok(
            agent,
            Request::WorkspaceOperationPrepare { goal, operation }
        ),
        completed
    );
    let Response::Checkouts(checkouts) = d.ok(agent, Request::Checkouts { goal }) else {
        panic!()
    };
    assert_eq!(checkouts.len(), 1);
    assert_eq!(checkouts[0].base_revision, target_revision);
    assert_eq!(checkouts[0].base_manifest, target_manifest);
    assert_eq!(checkouts[0].active_operation, None);
}

#[test]
fn checkout_identity_and_recovery_boundaries_are_exclusive() {
    let (mut d, principal, owner, agent, goal) = workspace_lifecycle::setup();
    let mut first = checkout(1);
    (first.base_revision, first.base_manifest) =
        workspace_lifecycle::accepted_tree(&mut d, agent, goal, 30);
    d.ok(
        owner,
        Request::WorkspaceConnect {
            goal,
            agent: principal,
            checkout: first.clone(),
        },
    );
    let mut alias = checkout(2);
    alias.root_identity = first.root_identity.clone();
    assert_eq!(
        code(d.call(
            owner,
            Request::WorkspaceConnect {
                goal,
                agent: principal,
                checkout: alias
            }
        )),
        ErrorCode::Conflict
    );
    let mut operation = update(&first, 1);
    if let WorkspaceOperationKind::Update { recovery, .. } = &mut operation.kind {
        recovery.recovery_directory = format!("{}/journal", first.root);
    }
    assert_eq!(
        code(d.call(
            agent,
            Request::WorkspaceOperationPrepare { goal, operation }
        )),
        ErrorCode::Invalid
    );
    let mut operation = update(&first, 2);
    if let WorkspaceOperationKind::Update { recovery, .. } = &mut operation.kind {
        recovery.recovery_identity.device = 2;
    }
    assert_eq!(
        code(d.call(
            agent,
            Request::WorkspaceOperationPrepare { goal, operation }
        )),
        ErrorCode::Invalid
    );
    let Response::Checkouts(checkouts) = d.ok(agent, Request::Checkouts { goal }) else {
        panic!()
    };
    assert_eq!(checkouts, [first]);
}

#[test]
fn checkout_rejects_unaccepted_or_mismatched_trees_and_overlapping_paths() {
    let (mut d, principal, owner, agent, goal) = workspace_lifecycle::setup();
    assert_eq!(
        code(d.call(
            owner,
            Request::WorkspaceConnect {
                goal,
                agent: principal,
                checkout: checkout(1)
            }
        )),
        ErrorCode::NotFound
    );
    let mut bound = checkout(1);
    (bound.base_revision, bound.base_manifest) =
        workspace_lifecycle::accepted_tree(&mut d, agent, goal, 30);
    let mut mismatched = bound.clone();
    mismatched.base_manifest = BlobHash([99; 32]);
    assert_eq!(
        code(d.call(
            owner,
            Request::WorkspaceConnect {
                goal,
                agent: principal,
                checkout: mismatched
            }
        )),
        ErrorCode::Conflict
    );
    d.ok(
        owner,
        Request::WorkspaceConnect {
            goal,
            agent: principal,
            checkout: bound.clone(),
        },
    );
    let mut nested = bound.clone();
    nested.id = CheckoutId([2; 16]);
    nested.root_identity.inode = 2;
    nested.root.push_str("/nested");
    assert_eq!(
        code(d.call(
            owner,
            Request::WorkspaceConnect {
                goal,
                agent: principal,
                checkout: nested
            }
        )),
        ErrorCode::Conflict
    );
    let mut invalid_target = update(&bound, 4);
    if let WorkspaceOperationKind::Update {
        target_manifest, ..
    } = &mut invalid_target.kind
    {
        *target_manifest = BlobHash([98; 32]);
    }
    assert_eq!(
        code(d.call(
            agent,
            Request::WorkspaceOperationPrepare {
                goal,
                operation: invalid_target
            }
        )),
        ErrorCode::Conflict
    );
    let operation = update(&bound, 5);
    let WorkspaceOperationKind::Update { ref recovery, .. } = operation.kind else {
        panic!()
    };
    let mut journal_root = bound.clone();
    journal_root.id = CheckoutId([3; 16]);
    journal_root.root = recovery.recovery_directory.clone();
    journal_root.root_identity = recovery.recovery_identity.clone();
    d.ok(
        agent,
        Request::WorkspaceOperationPrepare { goal, operation },
    );
    assert_eq!(
        code(d.call(
            owner,
            Request::WorkspaceConnect {
                goal,
                agent: principal,
                checkout: journal_root
            }
        )),
        ErrorCode::Conflict
    );
}

#[test]
fn explicit_session_binding_is_local_unique_durable_and_visible_in_context() {
    use locust_proto::api::{ContextSummary, ContextViewMode};
    let (mut d, principal, owner, agent, goal) = workspace_lifecycle::setup();
    let mut first = checkout(1);
    (first.base_revision, first.base_manifest) =
        workspace_lifecycle::accepted_tree(&mut d, agent, goal, 30);
    let mut second = first.clone();
    second.id = CheckoutId([2; 16]);
    second.root = "/work/second".into();
    second.root_identity.inode = 2;
    d.ok(
        owner,
        Request::WorkspaceConnect {
            goal,
            agent: principal,
            checkout: first.clone(),
        },
    );
    d.ok(
        owner,
        Request::WorkspaceConnect {
            goal,
            agent: principal,
            checkout: second.clone(),
        },
    );
    let no_session = d.connect(credential(1), None);
    assert_eq!(
        code(d.call(
            no_session,
            Request::CheckoutBindSession {
                goal,
                checkout: first.id
            }
        )),
        ErrorCode::Invalid
    );
    let secret = session(44);
    let caller = d.connect(credential(1), Some(secret));
    d.ok(
        caller,
        Request::CheckoutBindSession {
            goal,
            checkout: first.id,
        },
    );
    d.ok(
        caller,
        Request::CheckoutBindSession {
            goal,
            checkout: second.id,
        },
    );
    d.restart();
    let caller = d.connect(credential(1), Some(secret));
    let Response::Checkouts(checkouts) = d.ok(caller, Request::Checkouts { goal }) else {
        panic!()
    };
    assert_eq!(
        checkouts
            .iter()
            .filter(|checkout| checkout.session.is_some())
            .count(),
        1
    );
    let Response::Pending(pending) = d.ok(caller, Request::Pending { goal }) else {
        panic!()
    };
    assert_eq!(pending.workspace.unwrap().checkout.unwrap().id, second.id);
    let Response::Context(view) = d.ok(
        caller,
        Request::Context {
            goal,
            task: None,
            after: None,
            limit: 100,
            preview_chars: None,
            unread_only: false,
            view: ContextViewMode::Compact,
        },
    ) else {
        panic!()
    };
    let Some(ContextSummary::Compact(brief)) = view.summary else {
        panic!()
    };
    assert_eq!(brief.checkout.unwrap().id, second.id);
    assert_eq!(brief.workspace.head.unwrap().revision, first.base_revision);
}

#[test]
fn session_rebinding_cannot_bypass_active_or_unknown_checkout_ownership() {
    let (mut d, principal, owner, agent, goal) = workspace_lifecycle::setup();
    let mut bound = checkout(1);
    (bound.base_revision, bound.base_manifest) =
        workspace_lifecycle::accepted_tree(&mut d, agent, goal, 30);
    d.ok(
        owner,
        Request::WorkspaceConnect {
            goal,
            agent: principal,
            checkout: bound.clone(),
        },
    );
    let first = d.connect(credential(1), Some(session(44)));
    let second = d.connect(credential(1), Some(session(45)));
    let bind = Request::CheckoutBindSession {
        goal,
        checkout: bound.id,
    };
    let prepare = Request::WorkspaceOperationPrepare {
        goal,
        operation: update(&bound, 46),
    };
    d.ok(first, bind.clone());
    assert_eq!(code(d.call(second, bind.clone())), ErrorCode::Conflict);
    assert_eq!(code(d.call(second, prepare.clone())), ErrorCode::Conflict);
    let sessionless = d.connect(credential(1), None);
    assert_eq!(
        code(d.call(sessionless, prepare.clone())),
        ErrorCode::Conflict
    );
    d.node.disconnect(first);
    assert_eq!(code(d.call(second, bind.clone())), ErrorCode::Conflict);
    assert_eq!(code(d.call(second, prepare.clone())), ErrorCode::Conflict);
    d.restart();
    let first = d.connect(credential(1), Some(session(44)));
    let second = d.connect(credential(1), Some(session(45)));
    d.ok(
        first,
        Request::SessionReport {
            record: SessionRecord {
                harness: locust_proto::farm::Harness::Unknown,
                client: "test".into(),
                state: SessionState::Exited,
                client_session: None,
                capabilities: SessionCapabilities::default(),
                detail: vec![],
            },
        },
    );
    assert_eq!(code(d.call(second, bind.clone())), ErrorCode::Conflict);
    assert_eq!(code(d.call(second, prepare.clone())), ErrorCode::Conflict);
    d.node.disconnect(first);
    let prepared = d.ok(second, prepare.clone());
    d.ok(
        second,
        Request::WorkspaceOperationComplete {
            goal,
            operation: WorkspaceOperationId([46; 16]),
        },
    );
    d.ok(second, bind);
    // A saved exact operation remains discoverable after ownership moves.
    let former = d.connect(credential(1), Some(session(44)));
    let Response::WorkspaceOperation(saved) = d.ok(former, prepare) else {
        panic!()
    };
    let Response::WorkspaceOperation(prepared) = prepared else {
        panic!()
    };
    assert_eq!(saved.id, prepared.id);
    assert!(matches!(
        saved.state,
        WorkspaceOperationState::Completed { .. }
    ));
    let Response::Checkouts(checkouts) = d.ok(second, Request::Checkouts { goal }) else {
        panic!()
    };
    assert_eq!(checkouts[0].session, Some(session(45).instance()));
}

#[test]
fn only_the_person_posts_a_change_with_no_parent() {
    use locust_proto::api::WorkspaceCandidate;
    use locust_proto::event::{Context, Scope};
    let (mut d, principal, owner, agent, goal) = workspace_lifecycle::setup();
    let (base, manifest) = workspace_lifecycle::accepted_tree(&mut d, agent, goal, 40);
    let mut bound = checkout(55);
    bound.base_revision = base;
    bound.base_manifest = manifest;
    d.ok(
        owner,
        Request::WorkspaceConnect {
            goal,
            agent: principal,
            checkout: bound.clone(),
        },
    );
    let epoch = d.node.goals[&goal]
        .state()
        .workspace
        .as_ref()
        .unwrap()
        .epoch;
    for (tag, checkout) in [(80, None), (81, Some(bound.id))] {
        let operation = WorkspaceOperation {
            id: WorkspaceOperationId([tag; 16]),
            checkout,
            idempotency_key: IdempotencyKey([tag; 16]),
            state: WorkspaceOperationState::Prepared,
            kind: WorkspaceOperationKind::Capture {
                candidate: WorkspaceCandidate {
                    context: Context {
                        scope: Scope::Workspace,
                        round: epoch,
                    },
                    parent: None,
                    result_manifest: manifest,
                    sources: vec![],
                    captured_paths: vec!["tree.txt".into()],
                    replacement: true,
                },
            },
        };
        let request = Request::WorkspaceOperationPrepare { goal, operation };
        let error = d.call(agent, request.clone()).unwrap_err();
        assert_eq!(error.code, ErrorCode::Denied);
        assert_eq!(error.message, "only the host shares a goal's first files");
        d.on_behalf(owner, principal, request).unwrap();
    }
}
