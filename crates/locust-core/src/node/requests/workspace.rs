//! Durable local workspace metadata. File observations come from the CLI;
//! new agent checkouts use the daemon shell's validated filesystem capability.

use locust_proto::api::{
    ApiError, Checkout, ErrorCode, Response, WorkspaceAuthority, WorkspaceContent,
    WorkspaceFileView, WorkspaceOperation, WorkspaceOperationKind, WorkspaceOperationState,
    WorkspaceProposalStatus, WorkspaceProposalView, WorkspaceRevisionView, WorkspaceTreeView,
    WorkspaceView,
};
use locust_proto::engine::Entropy;
use locust_proto::event::{
    Body, Context, DecisionAction, DecisionPurpose, Scope, ScopeKey, WorkspaceCheckpoint,
};
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey, WorkspaceOperationId};
use locust_proto::store::Store;

use super::{Plan, Planned, answer};
use crate::goal::Standing;
use crate::node::Node;
use crate::node::access::{conflict, not_found};
use crate::node::callers::Actor;
use crate::node::commit::Tx;
use crate::node::content_graph::{FileState, ManifestState};
use crate::node::entry::Entry;
use crate::node::local;

/// The folders one agent may hold in one goal. Each is a full copy of the
/// shared files, and no request removes one.
const MAX_AGENT_CHECKOUTS: usize = 64;

fn revision_view(entry: &Entry, id: EventId) -> Result<WorkspaceRevisionView, ApiError> {
    let event = entry
        .goal
        .event(&id)
        .ok_or_else(|| not_found("workspace revision has not arrived"))?;
    let Body::ScopeDecided {
        context,
        action: DecisionAction::Select { subject },
        ..
    } = event.header().body
    else {
        return Err(ApiError::new(
            ErrorCode::Invalid,
            "workspace revision must be a selection",
        ));
    };
    let proposal = entry
        .goal
        .event(&subject)
        .ok_or_else(|| not_found("workspace proposal has not arrived"))?;
    let Body::WorkspaceProposed {
        context: proposal_context,
        parent,
        result_manifest,
        ..
    } = proposal.header().body
    else {
        return Err(ApiError::new(
            ErrorCode::Invalid,
            "selection subject is not a workspace proposal",
        ));
    };
    if context.scope != Scope::Workspace || proposal_context != context {
        return Err(ApiError::new(
            ErrorCode::Invalid,
            "workspace revision belongs to another context",
        ));
    }
    Ok(WorkspaceRevisionView {
        revision: id,
        context,
        proposal: subject,
        parent,
        result_manifest,
        in_lineage: entry.state().workspace_lineage.contains(&id),
    })
}

fn canonical_absolute(path: &std::path::Path) -> bool {
    path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
        && !path.as_os_str().is_empty()
}

fn overlaps(a: &std::path::Path, b: &std::path::Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn workspace_recovery_parent(
        &self,
        actor: &Actor,
        goal: GoalId,
        parent: String,
    ) -> Plan {
        self.readable(actor, &goal)?;
        let parent = std::path::Path::new(&parent);
        if !canonical_absolute(parent)
            || self.goals.values().any(|entry| {
                entry
                    .local
                    .checkouts
                    .values()
                    .any(|checkout| parent.starts_with(&checkout.root))
            })
        {
            return Err(conflict("recovery parent is inside a managed checkout"));
        }
        answer(Response::Done)
    }

    pub(super) fn workspace_epoch_set(
        &self,
        actor: &Actor,
        goal: GoalId,
        expected_epoch: Option<EventId>,
        rules: EventId,
        checkpoint: WorkspaceCheckpoint,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.host(actor, &goal)?;
        let current = entry.state().workspace.as_ref();
        if current.map(|workspace| workspace.epoch) != expected_epoch {
            return Err(conflict("workspace epoch changed").with_details(
                serde_json::json!({"epoch":current.map(|workspace|workspace.epoch)}),
            ));
        }
        if matches!(checkpoint, WorkspaceCheckpoint::Unseeded)
            && current.is_some_and(|workspace| workspace.head.is_some())
        {
            return Err(conflict(
                "an observed seeded workspace requires a revision checkpoint or explicit restoration",
            ));
        }
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::WorkspaceEpoch {
                expected_epoch,
                rules,
                checkpoint,
            },
            None,
            now,
            &mut tx,
        )?;
        super::tasks::recorded(event, tx)
    }

    pub(in crate::node) fn workspace_content(
        &self,
        entry: &Entry,
        manifest: BlobHash,
        reader: Option<&PublicKey>,
    ) -> Result<WorkspaceContent, ApiError> {
        let tree = match self.workspace_manifest(entry, manifest, reader)? {
            ManifestState::Missing => return Ok(WorkspaceContent::ManifestMissing { manifest }),
            ManifestState::Withdrawn => return Ok(WorkspaceContent::Withdrawn { hash: manifest }),
            ManifestState::KeyMissing { epoch } => {
                return Ok(WorkspaceContent::KeyMissing {
                    hash: manifest,
                    key_epoch: epoch,
                });
            }
            ManifestState::Invalid { reason } => {
                return Ok(WorkspaceContent::InvalidManifest { manifest, reason });
            }
            ManifestState::Ready { manifest, .. } => manifest,
        };
        let mut missing = std::collections::BTreeSet::new();
        let mut total = 0u64;
        for file in &tree.entries {
            let size = match self.workspace_file(entry, file.content, reader)? {
                FileState::Missing => {
                    missing.insert(file.content);
                    continue;
                }
                FileState::Withdrawn => {
                    return Ok(WorkspaceContent::Withdrawn { hash: file.content });
                }
                FileState::KeyMissing { epoch } => {
                    return Ok(WorkspaceContent::KeyMissing {
                        hash: file.content,
                        key_epoch: epoch,
                    });
                }
                FileState::Invalid { reason } => {
                    return Ok(WorkspaceContent::InvalidFile {
                        hash: file.content,
                        reason,
                    });
                }
                FileState::Ready { size } => size,
            };
            if size != file.size {
                return Ok(WorkspaceContent::InvalidFile {
                    hash: file.content,
                    reason: "file size does not match manifest".into(),
                });
            }
            total = total.checked_add(file.size).ok_or_else(|| {
                ApiError::new(ErrorCode::Invalid, "workspace byte total overflows")
            })?;
        }
        Ok(if missing.is_empty() {
            WorkspaceContent::Complete {
                files: tree.entries.len() as u64,
                bytes: total,
            }
        } else {
            WorkspaceContent::FilesMissing {
                manifest,
                missing: missing.into_iter().collect(),
            }
        })
    }

    pub(in crate::node) fn workspace_view(
        &self,
        entry: &Entry,
        actor: &Actor,
    ) -> Result<WorkspaceView, ApiError> {
        let mut view = Self::workspace_authority(entry)?;
        view.content = view
            .head
            .as_ref()
            .map(|head| {
                self.workspace_content(entry, head.result_manifest, actor.principal.as_ref())
            })
            .transpose()?;
        Ok(view)
    }

    fn workspace_authority(entry: &Entry) -> Result<WorkspaceView, ApiError> {
        let Some(workspace) = &entry.state().workspace else {
            return Ok(WorkspaceView {
                epoch: None,
                checkpoint: None,
                head: None,
                enabled: false,
                authority: WorkspaceAuthority::Uninitialized,
                content: None,
            });
        };
        let key = ScopeKey {
            context: Context {
                scope: Scope::Workspace,
                round: workspace.epoch,
            },
            purpose: DecisionPurpose::Selection,
        };
        let authority = if entry.goal.evaluation().scope_halts.contains_key(&key) {
            WorkspaceAuthority::Disputed
        } else {
            match entry.goal.standing(&workspace.epoch) {
                Some(Standing::Effective) => WorkspaceAuthority::Ready,
                Some(Standing::Excluded(_)) => WorkspaceAuthority::Invalid,
                Some(Standing::Disputed) => WorkspaceAuthority::Disputed,
                _ => WorkspaceAuthority::Pending,
            }
        };
        let head = workspace
            .head
            .map(|head| revision_view(entry, head))
            .transpose()?;
        Ok(WorkspaceView {
            epoch: Some(workspace.epoch),
            checkpoint: workspace.checkpoint,
            head,
            enabled: workspace.enabled,
            authority,
            content: None,
        })
    }

    pub(super) fn workspace_head(&self, actor: &Actor, goal: GoalId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        answer(Response::Workspace(self.workspace_view(entry, actor)?))
    }

    fn readable_workspace_tree(
        &self,
        actor: &Actor,
        goal: GoalId,
        requested: Option<EventId>,
    ) -> Result<WorkspaceTreeView, ApiError> {
        let entry = self.readable(actor, &goal)?;
        let workspace = Self::workspace_authority(entry)?;
        if workspace.authority != WorkspaceAuthority::Ready {
            return Err(conflict("workspace authority is not ready"));
        }
        let id = requested
            .or_else(|| workspace.head.map(|head| head.revision))
            .ok_or_else(|| not_found("workspace has no accepted tree"))?;
        let revision = revision_view(entry, id)?;
        if !revision.in_lineage {
            return Err(conflict(
                "revision is outside the retained workspace lineage",
            ));
        }
        if !matches!(
            self.workspace_content(entry, revision.result_manifest, actor.principal.as_ref())?,
            WorkspaceContent::Complete { .. }
        ) {
            return Err(ApiError::new(
                ErrorCode::Unavailable,
                "accepted workspace content is not usable",
            ));
        }
        let ManifestState::Ready { manifest, .. } =
            self.workspace_manifest(entry, revision.result_manifest, actor.principal.as_ref())?
        else {
            unreachable!("content verified above")
        };
        Ok(WorkspaceTreeView {
            revision,
            manifest: (*manifest).clone(),
        })
    }

    pub(super) fn workspace_tree(
        &self,
        actor: &Actor,
        goal: GoalId,
        revision: Option<EventId>,
    ) -> Plan {
        answer(Response::WorkspaceTree(
            self.readable_workspace_tree(actor, goal, revision)?,
        ))
    }

    pub(super) fn workspace_read(
        &self,
        actor: &Actor,
        goal: GoalId,
        revision: Option<EventId>,
        path: String,
    ) -> Plan {
        if !locust_proto::manifest::is_safe_path(&path) {
            return Err(ApiError::new(ErrorCode::Invalid, "unsafe workspace path"));
        }
        let tree = self.readable_workspace_tree(actor, goal, revision)?;
        let file = tree
            .manifest
            .entries
            .iter()
            .find(|entry| entry.path == path)
            .ok_or_else(|| not_found("path is not in this workspace tree"))?;
        let Response::Blob { bytes } = self.blob_get(actor, goal, file.content)?.response else {
            unreachable!()
        };
        answer(Response::WorkspaceFile(WorkspaceFileView {
            revision: tree.revision.revision,
            path,
            executable: file.executable,
            bytes,
        }))
    }

    pub(super) fn workspace_revision(&self, actor: &Actor, goal: GoalId, id: EventId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        answer(Response::WorkspaceRevision(revision_view(entry, id)?))
    }

    fn proposal_view(
        &self,
        entry: &Entry,
        actor: &Actor,
        id: EventId,
    ) -> Result<WorkspaceProposalView, ApiError> {
        let event = entry
            .goal
            .event(&id)
            .ok_or_else(|| not_found("no such workspace proposal"))?;
        let Body::WorkspaceProposed {
            context,
            parent,
            result_manifest,
            sources,
        } = &event.header().body
        else {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "event is not a workspace proposal",
            ));
        };
        let projected = entry.state().workspace_proposals.get(&id);
        let workspace = entry.state().workspace.as_ref();
        let content = self.workspace_content(entry, *result_manifest, actor.principal.as_ref())?;
        let integrated_as: Vec<_> = entry
            .state()
            .workspace_revisions
            .values()
            .filter(|revision| revision.proposal == id)
            .map(|revision| revision.id)
            .collect();
        let stale = workspace
            .is_none_or(|workspace| workspace.epoch != context.round || workspace.head != *parent);
        let status = if !integrated_as.is_empty() {
            WorkspaceProposalStatus::Integrated
        } else if entry.goal.standing(&id) == Some(Standing::Disputed) {
            WorkspaceProposalStatus::Disputed
        } else if projected.is_none() {
            WorkspaceProposalStatus::PendingAuthority
        } else if !matches!(content, WorkspaceContent::Complete { .. }) {
            WorkspaceProposalStatus::ContentUnavailable
        } else if stale {
            WorkspaceProposalStatus::Stale
        } else if !projected.is_some_and(|proposal| proposal.approved) {
            WorkspaceProposalStatus::AwaitingEvidence
        } else {
            WorkspaceProposalStatus::Ready
        };
        Ok(WorkspaceProposalView {
            proposal: id,
            context: *context,
            author: event.header().author,
            parent: *parent,
            result_manifest: *result_manifest,
            sources: sources.clone(),
            source_authors: projected
                .map(|proposal| proposal.source_authors.iter().copied().collect())
                .unwrap_or_default(),
            usable_as_source: entry.state().workspace_sources.contains(&id),
            integrated_as,
            status,
            standing: entry.event_view(event).standing,
            approved: projected.is_some_and(|proposal| proposal.approved),
            evidence: projected
                .map(|proposal| proposal.evidence.iter().copied().collect())
                .unwrap_or_default(),
            stale,
            content,
        })
    }

    pub(super) fn workspace_proposal(&self, actor: &Actor, goal: GoalId, id: EventId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        answer(Response::WorkspaceProposal(
            self.proposal_view(entry, actor, id)?,
        ))
    }

    pub(super) fn workspace_proposals(&self, actor: &Actor, goal: GoalId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let proposals = entry
            .goal
            .authors()
            .flat_map(|author| entry.goal.points(author))
            .filter_map(|point| entry.goal.event(&point.id))
            .filter(|event| matches!(event.header().body, Body::WorkspaceProposed { .. }))
            .map(|event| self.proposal_view(entry, actor, event.id()))
            .collect::<Result<Vec<_>, _>>()?;
        answer(Response::WorkspaceProposals(proposals))
    }

    pub(super) fn workspace_submit(
        &self,
        actor: &Actor,
        goal: GoalId,
        id: WorkspaceOperationId,
        integrate: bool,
        now: u64,
    ) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let principal = actor.principal()?;
        let mut operation = entry
            .local
            .workspace_operations
            .get(&(principal, id))
            .ok_or_else(|| not_found("no such prepared workspace operation"))?
            .clone();
        if integrate != matches!(operation.kind, WorkspaceOperationKind::Integrate { .. })
            || (!integrate && !matches!(operation.kind, WorkspaceOperationKind::Capture { .. }))
        {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "operation kind does not match submission",
            ));
        }
        if matches!(operation.state, WorkspaceOperationState::Recorded { .. }) {
            return answer(Response::WorkspaceOperation(operation));
        }
        self.member(actor, &goal)?;
        if operation.state != WorkspaceOperationState::Prepared {
            return Err(conflict("operation is not ready to submit"));
        }
        let workspace = entry
            .state()
            .workspace
            .as_ref()
            .ok_or_else(|| conflict("workspace is not initialized"))?;
        let context = Context {
            scope: Scope::Workspace,
            round: workspace.epoch,
        };
        let key = ScopeKey {
            context,
            purpose: DecisionPurpose::Selection,
        };
        if !workspace.ready
            || !workspace.enabled
            || entry.goal.evaluation().scope_halts.contains_key(&key)
        {
            return Err(conflict(
                "workspace authority is pending, disabled or disputed",
            ));
        }
        let body = match &operation.kind {
            WorkspaceOperationKind::Capture { candidate } => {
                if candidate.context != context {
                    return Err(conflict("captured workspace epoch is superseded"));
                }
                if !matches!(
                    self.workspace_content(entry, candidate.result_manifest, Some(&principal))?,
                    WorkspaceContent::Complete { .. }
                ) {
                    return Err(ApiError::new(
                        ErrorCode::Unavailable,
                        "captured candidate is not fully available and validated",
                    ));
                }
                Body::WorkspaceProposed {
                    context,
                    parent: candidate.parent,
                    result_manifest: candidate.result_manifest,
                    sources: candidate.sources.clone(),
                }
            }
            WorkspaceOperationKind::Integrate {
                expected_epoch,
                expected_head,
                proposal,
            } => {
                if workspace.epoch != *expected_epoch || workspace.head != *expected_head {
                    return Err(conflict("workspace epoch or head changed").with_details(
                        serde_json::json!({"epoch":workspace.epoch,"head":workspace.head}),
                    ));
                }
                let proposal = entry
                    .state()
                    .workspace_proposals
                    .get(proposal)
                    .ok_or_else(|| not_found("no effective workspace proposal"))?;
                if proposal.context != context || proposal.parent != workspace.head {
                    return Err(conflict(
                        "proposal requires composition against the current head",
                    ));
                }
                if !proposal.approved {
                    return Err(conflict(
                        "exact workspace candidate lacks required evidence",
                    ));
                }
                if !matches!(
                    self.workspace_content(entry, proposal.result_manifest, Some(&principal))?,
                    WorkspaceContent::Complete { .. }
                ) {
                    return Err(ApiError::new(
                        ErrorCode::Unavailable,
                        "candidate content is not fully available and validated",
                    ));
                }
                let previous = entry
                    .state()
                    .decisions
                    .get(&key)
                    .and_then(|decisions| decisions.last())
                    .map(|decision| decision.id);
                Body::ScopeDecided {
                    context,
                    previous,
                    action: DecisionAction::Select {
                        subject: proposal.id,
                    },
                    evidence: proposal.evidence.iter().copied().collect(),
                }
            }
            WorkspaceOperationKind::Update { .. } => unreachable!(),
        };
        let mut tx = Tx::none();
        let event = self.sign_for(actor, entry, &principal, body, None, now, &mut tx)?;
        operation.state = WorkspaceOperationState::Recorded { event };
        tx.local(local::workspace_operation_write(
            &goal, &principal, &operation,
        ));
        Ok(Planned {
            response: Response::WorkspaceOperation(operation),
            tx,
        })
    }

    fn accepted_workspace_tree(
        &self,
        entry: &Entry,
        revision: EventId,
        manifest: BlobHash,
        principal: &PublicKey,
    ) -> Result<(), ApiError> {
        let revision = revision_view(entry, revision)?;
        if !revision.in_lineage || revision.result_manifest != manifest {
            return Err(conflict(
                "revision and manifest must identify the retained accepted workspace tree",
            ));
        }
        if !matches!(
            self.workspace_content(entry, manifest, Some(principal))?,
            WorkspaceContent::Complete { .. }
        ) {
            return Err(ApiError::new(
                ErrorCode::Unavailable,
                "workspace tree is not fully available and validated",
            ));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)] // Mirrors the typed request fields.
    pub(super) fn checkout_register(
        &self,
        actor: &Actor,
        goal: GoalId,
        id: locust_proto::id::CheckoutId,
        revision: Option<EventId>,
        task: Option<locust_proto::event::TaskId>,
        attempt: Option<EventId>,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        if let Some(prior) = entry.local.checkouts.get(&(principal, id)) {
            return if revision.is_none_or(|revision| revision == prior.base_revision)
                && prior.task == task
                && prior.attempt == attempt
            {
                answer(Response::Checkout(prior.clone()))
            } else {
                Err(conflict(
                    "checkout identifier already names another binding",
                ))
            };
        }
        Self::validate_checkout_context(entry, principal, task, attempt)?;
        if entry
            .local
            .checkouts
            .keys()
            .filter(|(owner, _)| *owner == principal)
            .count()
            >= MAX_AGENT_CHECKOUTS
        {
            return Err(ApiError::new(
                ErrorCode::LimitExceeded,
                "this agent already holds the most folders one agent may register in a goal",
            ));
        }
        let tree = self.readable_workspace_tree(actor, goal, revision)?;
        // Whatever can refuse the binding is checked before a file is written.
        self.accepted_workspace_tree(
            entry,
            tree.revision.revision,
            tree.revision.result_manifest,
            &principal,
        )?;
        let files = self.checkout_files.as_ref().ok_or_else(|| {
            ApiError::new(
                ErrorCode::Unavailable,
                "private checkout materializer is unavailable",
            )
        })?;
        let destination = files.destination(id)?;
        if !canonical_absolute(std::path::Path::new(&destination)) {
            return Err(ApiError::new(
                ErrorCode::Unavailable,
                "private checkout materializer did not provide an absolute destination",
            ));
        }
        if self.checkout_path_conflicts(&destination) {
            return Err(conflict(
                "this directory already belongs to a managed checkout",
            ));
        }
        let mut fetch = |hash| {
            let Response::Blob { bytes } = self.blob_get(actor, goal, hash)?.response else {
                unreachable!()
            };
            Ok(bytes)
        };
        let (root, root_identity) = files.materialize(id, &tree.manifest, &mut fetch)?;
        let checkout = Checkout {
            id,
            root,
            root_identity,
            base_revision: tree.revision.revision,
            base_manifest: tree.revision.result_manifest,
            session: None,
            task,
            attempt,
            active_operation: None,
        };
        self.connect_checkout(actor, goal, checkout)
    }

    pub(super) fn workspace_connect(
        &self,
        actor: &Actor,
        goal: GoalId,
        agent: PublicKey,
        checkout: Checkout,
    ) -> Plan {
        let local_actor = self.local_agent(actor, agent)?;
        self.connect_checkout(&local_actor, goal, checkout)
    }

    fn validate_checkout_context(
        entry: &Entry,
        principal: PublicKey,
        task: Option<locust_proto::event::TaskId>,
        attempt: Option<EventId>,
    ) -> Result<(), ApiError> {
        if task.is_some_and(|task| !entry.state().tasks.contains_key(&task)) {
            return Err(not_found("checkout task is not in this goal"));
        }
        if let Some(attempt) = attempt {
            let attempt = entry
                .state()
                .attempts
                .get(&attempt)
                .ok_or_else(|| not_found("checkout attempt is not in this goal"))?;
            if attempt.author != principal
                || task.is_none_or(|task| {
                    attempt.context.scope != locust_proto::event::Scope::Task(task)
                })
            {
                return Err(conflict(
                    "checkout attempt must belong to this principal and task",
                ));
            }
        }
        Ok(())
    }

    fn connect_checkout(&self, actor: &Actor, goal: GoalId, checkout: Checkout) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        if let Some(prior) = entry.local.checkouts.get(&(principal, checkout.id)) {
            return if prior == &checkout {
                answer(Response::Checkout(prior.clone()))
            } else {
                Err(conflict(
                    "checkout identifier already names another binding",
                ))
            };
        }
        if checkout.session.is_some()
            || checkout.active_operation.is_some()
            || !canonical_absolute(std::path::Path::new(&checkout.root))
        {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "new checkout requires an absolute root and no active operation",
            ));
        }
        if self.checkout_path_conflicts(&checkout.root)
            || self.goals.values().any(|entry| {
                entry
                    .local
                    .checkouts
                    .values()
                    .any(|prior| prior.root_identity == checkout.root_identity)
            })
        {
            return Err(conflict(
                "this directory already belongs to a managed checkout",
            ));
        }
        self.accepted_workspace_tree(
            entry,
            checkout.base_revision,
            checkout.base_manifest,
            &principal,
        )?;
        Self::validate_checkout_context(entry, principal, checkout.task, checkout.attempt)?;
        let mut tx = Tx::none();
        tx.local(local::checkout_write(&goal, &principal, &checkout))
            .touch(goal);
        Ok(Planned {
            response: Response::Checkout(checkout),
            tx,
        })
    }

    fn checkout_path_conflicts(&self, root: &str) -> bool {
        let root = std::path::Path::new(root);
        self.goals.values().any(|entry| {
            entry
                .local
                .checkouts
                .values()
                .any(|prior| overlaps(std::path::Path::new(&prior.root), root))
                || entry.local.workspace_operations.values().any(|operation| {
                    matches!(&operation.kind, WorkspaceOperationKind::Update { recovery, .. }
                    if overlaps(std::path::Path::new(&recovery.recovery_directory), root))
                })
        })
    }

    pub(in crate::node) fn bound_checkout(&self, entry: &Entry, actor: &Actor) -> Option<Checkout> {
        let (principal, session) = actor.principal.zip(actor.session)?;
        entry
            .local
            .checkouts
            .iter()
            .find(|((owner, _), checkout)| *owner == principal && checkout.session == Some(session))
            .map(|(_, checkout)| checkout.clone())
    }

    fn checkout_session_available(
        &self,
        actor: &Actor,
        checkout: &Checkout,
    ) -> Result<(), ApiError> {
        if let Some(previous) = checkout
            .session
            .filter(|previous| Some(*previous) != actor.session)
        {
            let attached = self
                .conns
                .values()
                .any(|connection| connection.session == Some(previous));
            let exited = self.sessions.get(&previous).is_some_and(|entry| {
                entry.record.as_ref().is_some_and(|(record, _)| {
                    record.state == locust_proto::api::SessionState::Exited
                })
            });
            if attached || !exited {
                return Err(conflict(
                    "checkout belongs to another active or unverified session",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn checkout_bind_session(
        &self,
        actor: &Actor,
        goal: GoalId,
        id: locust_proto::id::CheckoutId,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        let session = actor.session()?;
        let mut checkout = entry
            .local
            .checkouts
            .get(&(principal, id))
            .ok_or_else(|| not_found("no such checkout for this principal"))?
            .clone();
        if checkout.active_operation.is_some() {
            return Err(conflict("checkout has an unresolved file operation"));
        }
        self.checkout_session_available(actor, &checkout)?;
        let mut tx = Tx::none();
        for ((owner, other_id), other) in &entry.local.checkouts {
            if *owner == principal && *other_id != id && other.session == Some(session) {
                if other.active_operation.is_some() {
                    return Err(conflict(
                        "session checkout has an unresolved file operation",
                    ));
                }
                let mut other = other.clone();
                other.session = None;
                tx.local(local::checkout_write(&goal, &principal, &other));
            }
        }
        checkout.session = Some(session);
        tx.local(local::checkout_write(&goal, &principal, &checkout))
            .touch(goal);
        Ok(Planned {
            response: Response::Checkout(checkout),
            tx,
        })
    }

    pub(super) fn checkouts(&self, actor: &Actor, goal: GoalId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let checkouts = entry
            .local
            .checkouts
            .iter()
            .filter(|((principal, _), _)| actor.principal.is_none_or(|reader| reader == *principal))
            .map(|(_, checkout)| checkout.clone())
            .collect();
        answer(Response::Checkouts(checkouts))
    }

    pub(super) fn workspace_operation_prepare(
        &self,
        actor: &Actor,
        goal: GoalId,
        operation: WorkspaceOperation,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        if let Some(prior) = entry
            .local
            .workspace_operations
            .get(&(principal, operation.id))
        {
            let mut prepared = prior.clone();
            prepared.state = WorkspaceOperationState::Prepared;
            return if prepared == operation {
                answer(Response::WorkspaceOperation(prior.clone()))
            } else {
                Err(conflict(
                    "operation identifier already names another request",
                ))
            };
        }
        if operation.state != WorkspaceOperationState::Prepared {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "new operation must be prepared",
            ));
        }
        if entry
            .local
            .workspace_operations
            .iter()
            .any(|((owner, _), prior)| {
                *owner == principal && prior.idempotency_key == operation.idempotency_key
            })
        {
            return Err(conflict(
                "idempotency key already belongs to another workspace operation",
            ));
        }
        let checkout = operation
            .checkout
            .map(|id| {
                entry
                    .local
                    .checkouts
                    .get(&(principal, id))
                    .ok_or_else(|| not_found("no such checkout for this principal"))
            })
            .transpose()?;
        let mut tx = Tx::none();
        if let WorkspaceOperationKind::Update {
            expected_revision,
            target_revision,
            target_manifest,
            recovery,
        } = &operation.kind
        {
            let checkout = checkout.ok_or_else(|| {
                ApiError::new(ErrorCode::Invalid, "update requires an explicit checkout")
            })?;
            self.checkout_session_available(actor, checkout)?;
            if checkout.base_revision != *expected_revision || checkout.active_operation.is_some() {
                return Err(conflict(
                    "checkout base changed or recovery is already active",
                ));
            }
            if recovery.root != checkout.root || recovery.root_identity != checkout.root_identity {
                return Err(conflict(
                    "recovery descriptor does not identify the bound directory",
                ));
            }
            self.accepted_workspace_tree(entry, *target_revision, *target_manifest, &principal)?;
            let recovery_path = std::path::Path::new(&recovery.recovery_directory);
            if !canonical_absolute(recovery_path) || recovery_path.parent() != std::path::Path::new(&checkout.root).parent()
                || recovery.recovery_identity.device != recovery.root_identity.device || recovery.recovery_identity == recovery.root_identity
                || self.goals.values().any(|entry| entry.local.checkouts.values().any(|bound| overlaps(recovery_path, std::path::Path::new(&bound.root)))
                    || entry.local.workspace_operations.values().any(|prior| matches!(&prior.kind,
                        WorkspaceOperationKind::Update { recovery: prior, .. } if recovery.recovery_identity == prior.recovery_identity
                            || overlaps(recovery_path, std::path::Path::new(&prior.recovery_directory))))) {
                return Err(ApiError::new(ErrorCode::Invalid, "recovery must be on the same device and outside every managed tree"));
            }
            let mut checkout = checkout.clone();
            checkout.active_operation = Some(operation.id);
            tx.local(local::checkout_write(&goal, &principal, &checkout));
        } else if checkout.is_some_and(|checkout| checkout.active_operation.is_some()) {
            return Err(conflict("checkout has an unresolved file operation"));
        }
        if let WorkspaceOperationKind::Capture { candidate } = &operation.kind {
            if checkout.is_none() && candidate.sources.is_empty() && !actor.owner_act {
                return Err(crate::node::access::denied(
                    "sharing files from this computer needs a folder your owner connected to this goal",
                ));
            }
            if !candidate.replacement
                && checkout.is_some_and(|checkout| candidate.parent != Some(checkout.base_revision))
            {
                return Err(conflict(
                    "frozen candidate parent differs from its checkout base",
                ));
            }
        }
        tx.local(local::workspace_operation_write(
            &goal, &principal, &operation,
        ))
        .touch(goal);
        Ok(Planned {
            response: Response::WorkspaceOperation(operation),
            tx,
        })
    }

    pub(super) fn workspace_operation_show(
        &self,
        actor: &Actor,
        goal: GoalId,
        operation: WorkspaceOperationId,
    ) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let principal = actor.principal()?;
        let operation = entry
            .local
            .workspace_operations
            .get(&(principal, operation))
            .ok_or_else(|| not_found("no such workspace operation for this principal"))?;
        answer(Response::WorkspaceOperation(operation.clone()))
    }

    pub(super) fn workspace_operations(&self, actor: &Actor, goal: GoalId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let principal = actor.principal()?;
        answer(Response::WorkspaceOperations(
            entry
                .local
                .workspace_operations
                .iter()
                .filter(|((owner, _), _)| *owner == principal)
                .map(|(_, operation)| operation.clone())
                .collect(),
        ))
    }

    pub(super) fn workspace_operation_complete(
        &self,
        actor: &Actor,
        goal: GoalId,
        operation: WorkspaceOperationId,
    ) -> Plan {
        // A removed member can finish recovery of its already-registered local files.
        let entry = self.readable(actor, &goal)?;
        let principal = actor.principal()?;
        let mut operation = entry
            .local
            .workspace_operations
            .get(&(principal, operation))
            .ok_or_else(|| not_found("no such workspace operation for this principal"))?
            .clone();
        if matches!(operation.state, WorkspaceOperationState::Completed { .. }) {
            return answer(Response::WorkspaceOperation(operation));
        }
        let WorkspaceOperationKind::Update {
            expected_revision,
            target_revision,
            target_manifest,
            ..
        } = operation.kind
        else {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "only a file update is finalized this way",
            ));
        };
        let checkout_id = operation
            .checkout
            .ok_or_else(|| conflict("update has no checkout"))?;
        let mut checkout = entry
            .local
            .checkouts
            .get(&(principal, checkout_id))
            .ok_or_else(|| not_found("no such checkout"))?
            .clone();
        if checkout.active_operation != Some(operation.id)
            || checkout.base_revision != expected_revision
        {
            return Err(conflict(
                "checkout no longer matches the prepared transition",
            ));
        }
        checkout.base_revision = target_revision;
        checkout.base_manifest = target_manifest;
        checkout.active_operation = None;
        operation.state = WorkspaceOperationState::Completed {
            target_in_lineage_at_completion: entry
                .state()
                .workspace_lineage
                .contains(&target_revision),
        };
        let mut tx = Tx::none();
        tx.local(local::checkout_write(&goal, &principal, &checkout))
            .local(local::workspace_operation_write(
                &goal, &principal, &operation,
            ))
            .touch(goal);
        Ok(Planned {
            response: Response::WorkspaceOperation(operation),
            tx,
        })
    }
}
