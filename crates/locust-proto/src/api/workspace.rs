//! Local checkout and operation records. Paths are inert CLI-supplied references;
//! these records never authorize the daemon to inspect or mutate host files.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::event::{Context, TaskId};
use crate::id::{
    BlobHash, CheckoutId, EventId, IdempotencyKey, InstanceId, PublicKey, WorkspaceOperationId,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Checkout {
    pub id: CheckoutId,
    pub root: String,
    pub root_identity: DirectoryIdentity,
    pub base_revision: EventId,
    pub base_manifest: BlobHash,
    /// Explicitly bound caller session; absence does not prove the files are idle.
    pub session: Option<InstanceId>,
    pub task: Option<TaskId>,
    pub attempt: Option<EventId>,
    pub active_operation: Option<WorkspaceOperationId>,
}

/// A frozen publication. Publication consumes these exact sealed object IDs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceCandidate {
    pub context: Context,
    pub parent: Option<EventId>,
    pub result_manifest: BlobHash,
    pub sources: Vec<EventId>,
    pub captured_paths: Vec<String>,
    pub replacement: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DirectoryIdentity {
    pub device: u64,
    pub inode: u64,
}

/// Exact CLI recovery identity registered before any checkout mutation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceRecovery {
    pub root: String,
    pub root_identity: DirectoryIdentity,
    pub recovery_directory: String,
    pub recovery_identity: DirectoryIdentity,
    pub plan_digest: BlobHash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceOperationKind {
    Capture {
        candidate: WorkspaceCandidate,
    },
    Integrate {
        expected_epoch: EventId,
        expected_head: Option<EventId>,
        proposal: EventId,
    },
    Update {
        expected_revision: EventId,
        target_revision: EventId,
        target_manifest: BlobHash,
        recovery: WorkspaceRecovery,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkspaceOperationState {
    Prepared,
    Recorded {
        event: EventId,
    },
    /// The files and binding completed; this records authority at that instant.
    Completed {
        target_in_lineage_at_completion: bool,
    },
}

/// Kept outside managed trees in the daemon's durable local store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceOperation {
    pub id: WorkspaceOperationId,
    pub checkout: Option<CheckoutId>,
    pub idempotency_key: IdempotencyKey,
    pub kind: WorkspaceOperationKind,
    pub state: WorkspaceOperationState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceAuthority {
    Uninitialized,
    Pending,
    Ready,
    Disputed,
    Invalid,
}

/// Content usability is independent of signed acceptance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceContent {
    ManifestMissing {
        manifest: BlobHash,
    },
    KeyMissing {
        hash: BlobHash,
        key_epoch: u32,
    },
    InvalidManifest {
        manifest: BlobHash,
        reason: String,
    },
    FilesMissing {
        manifest: BlobHash,
        missing: Vec<BlobHash>,
    },
    InvalidFile {
        hash: BlobHash,
        reason: String,
    },
    Withdrawn {
        hash: BlobHash,
    },
    Complete {
        files: u64,
        bytes: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceView {
    pub epoch: Option<EventId>,
    pub checkpoint: Option<EventId>,
    pub head: Option<WorkspaceRevisionView>,
    pub enabled: bool,
    pub authority: WorkspaceAuthority,
    pub content: Option<WorkspaceContent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceRevisionView {
    pub revision: EventId,
    pub context: Context,
    pub proposal: EventId,
    pub parent: Option<EventId>,
    pub result_manifest: BlobHash,
    /// True only for the currently retained accepted lineage, including checkpoints.
    pub in_lineage: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceProposalView {
    pub proposal: EventId,
    pub context: Context,
    pub author: PublicKey,
    pub parent: Option<EventId>,
    pub result_manifest: BlobHash,
    pub sources: Vec<EventId>,
    pub source_authors: Vec<PublicKey>,
    pub standing: super::Standing,
    /// Eligible as a typed source under the current epoch checkpoint proof.
    pub usable_as_source: bool,
    pub integrated_as: Vec<EventId>,
    pub status: WorkspaceProposalStatus,
    pub approved: bool,
    pub evidence: Vec<EventId>,
    pub stale: bool,
    pub content: WorkspaceContent,
}

/// Small shared authority and local binding observation for pending/context reads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceStatus {
    pub epoch: EventId,
    pub head: Option<EventId>,
    pub ready: bool,
    pub enabled: bool,
    pub checkout: Option<Checkout>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceProposalStatus {
    PendingAuthority,
    Disputed,
    ContentUnavailable,
    AwaitingEvidence,
    Stale,
    Ready,
    Integrated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceTreeView {
    pub revision: WorkspaceRevisionView,
    pub manifest: crate::manifest::Manifest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceFileView {
    pub revision: EventId,
    pub path: String,
    pub executable: bool,
    #[serde(with = "crate::codec::bytes")]
    #[schemars(with = "Vec<u8>")]
    pub bytes: Vec<u8>,
}
