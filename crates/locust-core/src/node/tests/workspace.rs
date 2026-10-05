//! Durable checkout transition tests. Filesystem execution remains in the CLI.
use super::*;
use locust_proto::api::{
    Checkout, DirectoryIdentity, SessionCapabilities, SessionRecord, SessionState,
    WorkspaceOperation, WorkspaceOperationKind, WorkspaceOperationState, WorkspaceRecovery,
};
use locust_proto::id::{BlobHash, CheckoutId, EventId, WorkspaceOperationId};

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
    let (mut d, _, _, agent, goal) = workspace_lifecycle::setup();
    let mut checkout = checkout(1);
    (checkout.base_revision, checkout.base_manifest) =
        workspace_lifecycle::accepted_tree(&mut d, agent, goal, 30);
    d.ok(
        agent,
        Request::CheckoutRegister {
            goal,
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
    let (mut d, _, _, agent, goal) = workspace_lifecycle::setup();
    let mut first = checkout(1);
    (first.base_revision, first.base_manifest) =
        workspace_lifecycle::accepted_tree(&mut d, agent, goal, 30);
    d.ok(
        agent,
        Request::CheckoutRegister {
            goal,
            checkout: first.clone(),
        },
    );
    let mut alias = checkout(2);
    alias.root_identity = first.root_identity.clone();
    assert_eq!(
        code(d.call(
            agent,
            Request::CheckoutRegister {
                goal,
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
    let (mut d, _, _, agent, goal) = workspace_lifecycle::setup();
    assert_eq!(
        code(d.call(
            agent,
            Request::CheckoutRegister {
                goal,
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
            agent,
            Request::CheckoutRegister {
                goal,
                checkout: mismatched
            }
        )),
        ErrorCode::Conflict
    );
    d.ok(
        agent,
        Request::CheckoutRegister {
            goal,
            checkout: bound.clone(),
        },
    );
    let mut nested = bound.clone();
    nested.id = CheckoutId([2; 16]);
    nested.root_identity.inode = 2;
    nested.root.push_str("/nested");
    assert_eq!(
        code(d.call(
            agent,
            Request::CheckoutRegister {
                goal,
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
            agent,
            Request::CheckoutRegister {
                goal,
                checkout: journal_root
            }
        )),
        ErrorCode::Conflict
    );
}

#[test]
fn explicit_session_binding_is_local_unique_durable_and_visible_in_context() {
    use locust_proto::api::{ContextSummary, ContextViewMode};
    let (mut d, _, _, agent, goal) = workspace_lifecycle::setup();
    let mut first = checkout(1);
    (first.base_revision, first.base_manifest) =
        workspace_lifecycle::accepted_tree(&mut d, agent, goal, 30);
    let mut second = first.clone();
    second.id = CheckoutId([2; 16]);
    second.root = "/work/second".into();
    second.root_identity.inode = 2;
    d.ok(
        agent,
        Request::CheckoutRegister {
            goal,
            checkout: first.clone(),
        },
    );
    d.ok(
        agent,
        Request::CheckoutRegister {
            goal,
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
    let (mut d, _, _, agent, goal) = workspace_lifecycle::setup();
    let mut bound = checkout(1);
    (bound.base_revision, bound.base_manifest) =
        workspace_lifecycle::accepted_tree(&mut d, agent, goal, 30);
    d.ok(
        agent,
        Request::CheckoutRegister {
            goal,
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
