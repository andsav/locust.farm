//! The local daemon API: what the CLI, the stdio MCP bridge and client
//! adapters ask of the daemon over its Unix socket.
//!
//! # Connection
//!
//! A connection opens with one [`ClientHello`] frame answered by one
//! [`ServerHello`]. After that the client sends [`RequestFrame`]s and receives
//! exactly one [`ResponseFrame`] per request, in order, matched by `id`.
//! Frames use [`crate::codec`]; a hello frame is read with
//! [`MAX_HELLO_FRAME_BYTES`](crate::limits::MAX_HELLO_FRAME_BYTES) and every
//! later frame with
//! [`MAX_LOCAL_FRAME_BYTES`](crate::limits::MAX_LOCAL_FRAME_BYTES). A frame
//! that does not decode has no `id` to answer under, so the daemon closes the
//! connection. A request that blocks, such as `wait`, occupies its connection
//! until it is answered; a client that wants to keep working meanwhile opens
//! a second connection with the same credential and session.
//! [`crate::client::Client`] is the client side.
//!
//! # Who is asking
//!
//! The credential in the hello fixes the [`Caller`] for the whole connection:
//! the owner, one enrolled principal, or one authoring principal. Every
//! goal-scoped request names its goal. There is no current or default goal,
//! and a missing credential never selects a wider scope. A principal sees
//! only the goals it is part of; to it, any other goal does not exist
//! ([`ErrorCode::NotFound`]).
//!
//! The owner authors no events. Directly it may make the owner-only requests,
//! every read-only request and `session.drop`. Any other request acts as a
//! principal, so the owner names one in [`RequestFrame::on_behalf`]; that
//! direct act skips the local level, but shared goal rules still apply.
//!
//! # Sessions and claims
//!
//! A session is one execution session of a coding client. Its launcher or
//! bridge generates a [`SessionSecret`], keeps it in a file the model never
//! reads, and presents it in the hello; the connection is that session for
//! its whole life. No request carries a session or instance field, so a
//! second session of the same principal cannot act as the first by naming
//! its public handle. A session belongs to one principal: the first request
//! that uses it (a session record or a claim) binds it, and using it as
//! another principal is [`ErrorCode::Denied`].
//!
//! A [`Claim`] binds an attempt to the session that took it and carries a
//! generation. Claim-bound writes name the generation the session was given.
//! A takeover raises it, which fences the earlier holder: its delayed writes
//! fail with [`ErrorCode::Superseded`] instead of landing.
//!
//! # Levels
//!
//! Each local agent has one [`Level`] in a goal. Shared rules are checked
//! before the level; [`Refused`] names which side declined an action.
//!
//! # Pending work and waiting
//!
//! What needs a caller is computed from task state and local records each
//! time it is asked ([`Request::Pending`]); it never depends on a
//! notification having been delivered. Every goal has a change counter, its
//! revision, raised in the same commit as any change to the goal's events or
//! to a local record that feeds pending work. It is the daemon's own count
//! and never falls, a restart included. [`Request::Wait`] answers as soon as
//! the revision is past the one the caller has seen, so a change between two
//! calls is never missed; a `seen` ahead of the revision is refused as
//! [`ErrorCode::Invalid`].
//!
//! # Text forms
//!
//! Human-readable output is these same types rendered as JSON. Identifiers
//! appear as hex, enum values by their stable snake_case names, and a request
//! as `{"<operation name>": {fields}}` (the bare name when it has no fields).
//! Credentials, session secrets and invitation secrets render as
//! `<redacted>` and are never read from text. The one capability JSON does
//! show is an invitation [`Ticket`], because showing it is how it is handed
//! over.

pub mod context;
pub mod guard;
pub mod invitations;
pub mod level;
mod schema;
pub use context::*;
pub use guard::*;
pub use invitations::*;
pub use level::*;

use crate::organization::catalog::{Draft, Presentation, Publication};
use schemars::{JsonSchema, schema_for};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use serde::{Deserialize, Serialize};

use crate::API_VERSION;
use crate::codec::{self, CodecError};
use crate::crypto::{self, domain};
use crate::event::{
    AttemptStatus, Body, CancelOutcome, Context, Doc, EventError, PayloadRef, ReviewVerdict, Scope,
    TaskId, WorkspaceCheckpoint,
};
use crate::id::{
    BlobHash, EffectId, EndpointId, EventId, GoalId, IdempotencyKey, InstanceId, PublicKey,
    WorkspaceOperationId,
};
use crate::invite::{InviteError, Ticket};
use crate::limits::{MAX_ARTIFACTS, MAX_SESSION_DETAIL_BYTES};
use crate::manifest::ManifestError;
use crate::store::StoreError;

mod workspace;
pub use workspace::*;

/// Longest name of an enrolled principal, in bytes.
pub const MAX_AGENT_NAME_BYTES: usize = 32;

/// Longest short string a local record stores verbatim: a session's client
/// name or client session identifier, a workspace path or commit name.
pub const MAX_LOCAL_TEXT_BYTES: usize = 4096;

/// Most hashes one `blob.stat` may ask about.
pub const MAX_STAT_HASHES: usize = 256;

/// Most events one `events` answer carries. A larger `limit` is treated as
/// this value.
pub const MAX_FEED_PAGE: u32 = 256;

/// Most items one `context.read` page carries. A larger `limit` is treated as
/// this value, and the cursor continues from there. Items take about half a
/// local frame; the first page's summary is not counted.
pub const MAX_CONTEXT_PAGE: u32 = 32;
const _: () = assert!(
    MAX_CONTEXT_PAGE as usize
        * (crate::limits::MAX_PAYLOAD_BYTES + crate::limits::MAX_HEADER_BYTES)
        < crate::limits::MAX_LOCAL_FRAME_BYTES
);

/// True if `name` may name an enrolled principal: 1 to
/// [`MAX_AGENT_NAME_BYTES`] characters from `a-z`, `0-9` and `-`. The name is
/// also a file name (`agents/<name>.credential`), which is why nothing else
/// is admitted.
pub fn is_agent_name(name: &str) -> bool {
    (1..=MAX_AGENT_NAME_BYTES).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// A local bearer secret that selects the caller: the owner, one enrolled
/// principal, or one authoring principal. It scopes what a client may ask for and
/// attributes its requests; it is not protection against another process
/// that can read the same user's files.
///
/// Whoever generates a credential writes it to its file first (see
/// [`crate::local`]); a principal's is generated by the client
/// that enrolls it. The daemon stores only [`Credential::digest`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Credential(pub [u8; 32]);

impl Credential {
    /// What the daemon stores and looks up in place of the secret, and what
    /// [`Request::AgentEnroll`] carry.
    pub fn digest(&self) -> [u8; 32] {
        crypto::domain_hash(domain::LOCAL_CREDENTIAL, &self.0)
    }
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Credential(..)")
    }
}

impl Serialize for Credential {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str("<redacted>")
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Credential {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            Err(serde::de::Error::custom(
                "credentials are never read from text",
            ))
        } else {
            <[u8; 32]>::deserialize(deserializer).map(Self)
        }
    }
}

/// Proof that a connection is one particular execution session. The launcher
/// or bridge generates it, keeps it in a file and presents it in the hello;
/// the model never sees it, and no request repeats it.
///
/// Holding the secret is what recovers a claim after a lost response or a
/// restart. The public handle, [`SessionSecret::instance`], proves nothing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SessionSecret(pub [u8; 32]);

impl SessionSecret {
    /// The session's public handle: the first 16 bytes of the
    /// domain-separated digest of the secret. Views name sessions by it.
    pub fn instance(&self) -> InstanceId {
        let digest = crypto::domain_hash(domain::SESSION_INSTANCE, &self.0);
        let mut handle = [0u8; InstanceId::LEN];
        handle.copy_from_slice(&digest[..InstanceId::LEN]);
        InstanceId(handle)
    }
}

impl fmt::Debug for SessionSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionSecret(..)")
    }
}

impl Serialize for SessionSecret {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str("<redacted>")
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for SessionSecret {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            Err(serde::de::Error::custom(
                "session secrets are never read from text",
            ))
        } else {
            <[u8; 32]>::deserialize(deserializer).map(Self)
        }
    }
}

/// The first frame of a connection. `api_version` is encoded first and stays
/// first in every future layout, so a daemon can tell an unsupported client
/// from a damaged frame; see [`ClientHello::decode`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientHello {
    /// [`API_VERSION`] of the client.
    pub api_version: u16,
    /// Selects the caller for the whole connection.
    pub credential: Credential,
    /// The execution session this connection is, if it is one. Without it the
    /// connection can read and coordinate but cannot hold a claim. An authoring credential does not carry one.
    pub session: Option<SessionSecret>,
}

impl ClientHello {
    /// The API version a hello frame opens with, read without decoding the
    /// rest. `None` if the frame does not even start with a version.
    pub fn api_version_of(frame: &[u8]) -> Option<u16> {
        postcard::take_from_bytes::<u16>(frame)
            .ok()
            .map(|(version, _)| version)
    }

    /// Daemon side: reads a hello frame. A frame that opens with another API
    /// version is refused as [`ErrorCode::UnsupportedVersion`] whatever
    /// follows, because its layout is not known here; a frame of this version
    /// that does not decode is [`ErrorCode::Invalid`].
    pub fn decode(frame: &[u8]) -> Result<Self, ApiError> {
        match Self::api_version_of(frame) {
            Some(API_VERSION) => codec::decode(frame)
                .map_err(|_| ApiError::new(ErrorCode::Invalid, "hello frame is malformed")),
            Some(version) => Err(ApiError::new(
                ErrorCode::UnsupportedVersion,
                format!("client speaks API version {version}; this daemon speaks {API_VERSION}"),
            )),
            None => Err(ApiError::new(
                ErrorCode::Invalid,
                "hello frame is malformed",
            )),
        }
    }
}

/// Who a connection acts as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Caller {
    /// The local participant: enrolls principals, sets their levels and may
    /// act on their behalf. Authors no events itself.
    Owner,
    /// An enrolled principal acting within the goal rules and its local level.
    Agent(PublicKey),
    /// Authoring-only credential: no goal access, sessions or on-behalf authority.
    Author(PublicKey),
}

/// The daemon's answer to a hello. It must fit
/// [`MAX_HELLO_FRAME_BYTES`](crate::limits::MAX_HELLO_FRAME_BYTES).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerHello {
    /// The connection is open and requests may follow.
    Welcome {
        /// The API version of this connection; equals the client's.
        api_version: u16,
        /// The daemon's own version, for people.
        daemon_version: String,
        /// Who the credential resolved to.
        caller: Caller,
        /// Largest content object this daemon stores, in bytes, counted as
        /// stored: sealing adds a few bytes to what a client sends. Operators
        /// may set it below [`MAX_BLOB_BYTES`](crate::limits::MAX_BLOB_BYTES).
        max_blob_bytes: u64,
    },
    /// The connection is refused and closed: `Denied` for an unknown or
    /// revoked credential, `UnsupportedVersion`, or `Invalid`, which includes
    /// an invalid execution-session binding. The layout of
    /// this variant never changes, so that a client of any version can read
    /// why it was refused and which version the daemon speaks.
    Refused {
        /// Why.
        error: ApiError,
        /// The API version the daemon speaks.
        api_version: u16,
        /// The daemon's own version, for people.
        daemon_version: String,
    },
}

/// One request as it crosses the socket.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestFrame {
    /// Chosen by the client; echoed in the response.
    pub id: u64,
    /// With a key, repeating a request that succeeded returns its first
    /// result instead of running it again. A request that failed left nothing
    /// behind and runs again. Keys are scoped to the caller; reusing one for
    /// a different request fails with [`ErrorCode::IdempotencyMismatch`].
    pub idempotency: Option<IdempotencyKey>,
    /// Owner only: perform the request as this enrolled principal. The
    /// owner's direct act skips the local level, not shared rules.
    /// From any other caller this is `Denied`; an unknown or
    /// revoked principal is `NotFound`; on an owner-only request it is
    /// `Invalid`.
    pub on_behalf: Option<PublicKey>,
    /// The operation and its arguments.
    pub request: Request,
}

/// The answer to the request frame with the same `id`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseFrame {
    /// The `id` of the request this answers.
    pub id: u64,
    /// The answer of the kind the request gets, or why it was refused.
    pub result: Result<Response, ApiError>,
}

/// Current greenfield API. Shared formation rules and local levels are separate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub enum Request {
    #[serde(rename = "farm.on")]
    FarmOn {
        goal: GoalId,
        base_url: String,
        listed: bool,
        title: Option<String>,
        formation: String,
        stage_labels: BTreeMap<String, String>,
        role_labels: BTreeMap<String, String>,
        recent_changes: u32,
    },
    #[serde(rename = "farm.off")]
    FarmOff { goal: GoalId },
    #[serde(rename = "farm.show")]
    FarmShow { goal: GoalId },
    #[serde(rename = "farm.status")]
    FarmStatus,
    #[serde(rename = "farm.consent")]
    FarmConsent {
        goal: GoalId,
        agent: PublicKey,
        accept: bool,
        name: Option<String>,
        group_label: Option<String>,
    },

    #[serde(rename = "status")]
    Status,
    #[serde(rename = "daemon.stop")]
    Shutdown,
    #[serde(rename = "agent.enroll")]
    AgentEnroll { name: String, credential: [u8; 32] },
    #[serde(rename = "author.enroll")]
    AuthorEnroll { name: String, credential: [u8; 32] },
    #[serde(rename = "agent.revoke")]
    AgentRevoke { agent: PublicKey },
    #[serde(rename = "agent.reconnect")]
    AgentReconnect { agent: PublicKey },
    #[serde(rename = "session.report")]
    SessionReport { record: SessionRecord },
    #[serde(rename = "session.show")]
    Session { instance: Option<InstanceId> },
    #[serde(rename = "sessions")]
    Sessions,
    #[serde(rename = "session.drop")]
    SessionDrop { instance: InstanceId },
    #[serde(rename = "goal.create")]
    GoalCreate {
        agent: PublicKey,
        name: String,
        title: String,
        formation_json: Option<String>,
        inputs: BTreeMap<String, BlobHash>,
    },
    #[serde(rename = "goal.join")]
    GoalJoin {
        agent: PublicKey,
        name: String,
        ticket: Ticket,
        level: Level,
    },
    #[serde(rename = "goal.invite")]
    GoalInvite {
        goal: GoalId,
        expires_ms: u64,
        role: Option<String>,
    },
    #[serde(rename = "goal.leave")]
    GoalLeave { goal: GoalId, agent: PublicKey },
    #[serde(rename = "goal.continue")]
    GoalContinue { goal: GoalId },
    #[serde(rename = "level.set")]
    LevelSet {
        goal: GoalId,
        agent: PublicKey,
        level: Level,
    },
    #[serde(rename = "task.allow")]
    TaskAllow {
        goal: GoalId,
        agent: PublicKey,
        task: TaskId,
    },
    #[serde(rename = "task.disallow")]
    TaskDisallow {
        goal: GoalId,
        agent: PublicKey,
        task: TaskId,
    },
    #[serde(rename = "goal.status")]
    GoalStatus { goal: GoalId },
    #[serde(rename = "member.remove")]
    MemberRemove { goal: GoalId, member: PublicKey },
    #[serde(rename = "role.give")]
    RoleGive {
        goal: GoalId,
        role: String,
        member: PublicKey,
        expected: Vec<PublicKey>,
    },
    #[serde(rename = "role.take")]
    RoleTake {
        goal: GoalId,
        role: String,
        member: PublicKey,
        expected: Vec<PublicKey>,
    },
    #[serde(rename = "rules.bind")]
    RulesBind {
        goal: GoalId,
        expected: EventId,
        formation_json: String,
        inputs: BTreeMap<String, BlobHash>,
        /// Keep role holders unchanged instead of giving the counting role to every member.
        #[serde(default)]
        no_role: bool,
    },
    #[serde(rename = "checkout.register")]
    CheckoutRegister {
        goal: GoalId,
        checkout: crate::id::CheckoutId,
        revision: Option<EventId>,
        task: Option<TaskId>,
        attempt: Option<EventId>,
    },
    #[serde(rename = "workspace.connect")]
    WorkspaceConnect {
        goal: GoalId,
        agent: PublicKey,
        checkout: Checkout,
    },
    #[serde(rename = "checkout.bind_session")]
    CheckoutBindSession {
        goal: GoalId,
        checkout: crate::id::CheckoutId,
    },
    #[serde(rename = "checkouts")]
    Checkouts { goal: GoalId },
    #[serde(rename = "workspace.recovery.parent")]
    WorkspaceRecoveryParentCheck { goal: GoalId, parent: String },
    #[serde(rename = "workspace.operation.prepare")]
    WorkspaceOperationPrepare {
        goal: GoalId,
        operation: WorkspaceOperation,
    },
    #[serde(rename = "workspace.operation.show")]
    WorkspaceOperationShow {
        goal: GoalId,
        operation: WorkspaceOperationId,
    },
    #[serde(rename = "workspace.operations")]
    WorkspaceOperations { goal: GoalId },
    #[serde(rename = "workspace.operation.complete")]
    WorkspaceOperationComplete {
        goal: GoalId,
        operation: WorkspaceOperationId,
    },
    #[serde(rename = "workspace.epoch")]
    WorkspaceEpochSet {
        goal: GoalId,
        expected_epoch: Option<EventId>,
        rules: EventId,
        checkpoint: WorkspaceCheckpoint,
    },
    #[serde(rename = "workspace.head")]
    WorkspaceHead { goal: GoalId },
    #[serde(rename = "workspace.tree")]
    WorkspaceTree {
        goal: GoalId,
        revision: Option<EventId>,
    },
    #[serde(rename = "workspace.read")]
    WorkspaceRead {
        goal: GoalId,
        revision: Option<EventId>,
        path: String,
    },
    #[serde(rename = "workspace.proposals")]
    WorkspaceProposals { goal: GoalId },
    #[serde(rename = "workspace.proposal")]
    WorkspaceProposal { goal: GoalId, proposal: EventId },
    #[serde(rename = "workspace.revision")]
    WorkspaceRevision { goal: GoalId, revision: EventId },
    #[serde(rename = "workspace.publish")]
    WorkspacePublish {
        goal: GoalId,
        operation: WorkspaceOperationId,
    },
    #[serde(rename = "workspace.integrate")]
    WorkspaceIntegrate {
        goal: GoalId,
        operation: WorkspaceOperationId,
    },
    #[serde(rename = "board")]
    Board { goal: GoalId },
    #[serde(rename = "task.show")]
    Task { goal: GoalId, task: TaskId },
    #[serde(rename = "event.show")]
    Event { goal: GoalId, event: EventId },
    #[serde(rename = "task.open")]
    TaskOpen {
        goal: GoalId,
        text: String,
        task_type: Option<String>,
        inputs: BTreeMap<String, BlobHash>,
        parent: Option<TaskId>,
    },
    #[serde(rename = "task.revise")]
    TaskRevise {
        goal: GoalId,
        task: TaskId,
        expected_round: EventId,
        task_type: Option<String>,
    },
    #[serde(rename = "work.offer")]
    WorkOffer {
        goal: GoalId,
        task: TaskId,
        recipient: PublicKey,
    },
    #[serde(rename = "attempt.start")]
    AttemptStart {
        goal: GoalId,
        task: TaskId,
        offer: Option<EventId>,
    },
    #[serde(rename = "attempt.takeover")]
    AttemptTakeover { goal: GoalId, attempt: EventId },
    #[serde(rename = "work.decline")]
    WorkDecline { goal: GoalId, offer: EventId },
    #[serde(rename = "attempt.cancel")]
    AttemptCancel { goal: GoalId, attempt: EventId },
    #[serde(rename = "attempt.report")]
    AttemptReport {
        goal: GoalId,
        attempt: EventId,
        generation: u32,
        status: AttemptStatus,
        text: String,
    },
    #[serde(rename = "contribution.publish")]
    ContributionPublish {
        goal: GoalId,
        attempt: Option<EventId>,
        generation: Option<u32>,
        summary: String,
        /// Known events in this goal that the author declares as sources.
        /// This does not attest that a model read, understood or used them.
        #[serde(default)]
        sources: Vec<EventId>,
        artifacts: Vec<BlobHash>,
    },
    #[serde(rename = "contributions")]
    Contributions { goal: GoalId, task: Option<TaskId> },
    #[serde(rename = "contribution.inspect")]
    ContributionInspect { goal: GoalId, contribution: EventId },
    #[serde(rename = "completion.declare")]
    CompletionDeclare { goal: GoalId, subject: EventId },
    #[serde(rename = "review.record")]
    ReviewRecord {
        goal: GoalId,
        subject: EventId,
        verdict: ReviewVerdict,
        text: String,
    },
    #[serde(rename = "check.attest")]
    CheckAttest {
        goal: GoalId,
        subject: EventId,
        name: String,
        passed: bool,
        text: String,
    },
    #[serde(rename = "scope.select")]
    ScopeSelect {
        goal: GoalId,
        subject: EventId,
        expected: Option<EventId>,
    },
    #[serde(rename = "scope.close")]
    ScopeClose {
        goal: GoalId,
        scope: Scope,
        expected: Option<EventId>,
    },
    #[serde(rename = "scope.reopen")]
    ScopeReopen {
        goal: GoalId,
        scope: Scope,
        expected: Option<EventId>,
    },
    #[serde(rename = "delivery.acknowledge")]
    DeliveryAcknowledge { goal: GoalId, effect: EffectId },
    #[serde(rename = "cancel.acknowledge")]
    CancelAcknowledge {
        goal: GoalId,
        cancel: EventId,
        generation: Option<u32>,
        outcome: CancelOutcome,
    },
    #[serde(rename = "pending")]
    Pending { goal: GoalId },
    #[serde(rename = "pending.page")]
    PendingPage {
        goal: GoalId,
        kind: Option<PendingKind>,
        after: Option<PendingCursor>,
        limit: u32,
    },
    #[serde(rename = "wait")]
    Wait {
        goal: GoalId,
        seen: u64,
        timeout_ms: u32,
    },
    #[serde(rename = "events")]
    Events {
        goal: GoalId,
        after: Option<u64>,
        limit: u32,
    },
    #[serde(rename = "doc.read")]
    DocRead { goal: GoalId, doc: Doc },
    #[serde(rename = "doc.revise")]
    DocRevise {
        goal: GoalId,
        doc: Doc,
        base: Option<EventId>,
        text: String,
    },
    #[serde(rename = "blob.put")]
    BlobPut {
        goal: GoalId,
        #[serde(with = "codec::bytes")]
        #[schemars(with = "Vec<u8>")]
        bytes: Vec<u8>,
    },
    #[serde(rename = "blob.get")]
    BlobGet { goal: GoalId, hash: BlobHash },
    #[serde(rename = "blob.stat")]
    BlobStat { goal: GoalId, hashes: Vec<BlobHash> },
    #[serde(rename = "blob.withdraw")]
    BlobWithdraw { goal: GoalId, hash: BlobHash },
    #[serde(rename = "formation.draft.create")]
    FormationDraftCreate {
        id: String,
        expected_revision: u64,
        source: String,
    },
    #[serde(rename = "formation.draft.update")]
    FormationDraftUpdate {
        id: String,
        expected_revision: u64,
        source: String,
    },
    #[serde(rename = "formation.draft.show")]
    FormationDraft { id: String },
    #[serde(rename = "formation.drafts")]
    FormationDrafts,
    #[serde(rename = "formation.publish")]
    FormationPublish {
        draft: String,
        id: String,
        expected_revision: u64,
        expected_source_hash: String,
    },
    #[serde(rename = "formation.show")]
    FormationPublication { id: String },
    #[serde(rename = "formation.list")]
    FormationPublications,
    #[serde(rename = "formation.presentation.show")]
    FormationPresentation { id: String },
    #[serde(rename = "formation.presentation.update")]
    FormationPresentationUpdate {
        id: String,
        expected_revision: u64,
        data_json: String,
    },
    #[serde(rename = "formation.validate")]
    FormationValidate { source: String },
    #[serde(rename = "formation.explain")]
    FormationExplain { source: String },

    #[serde(rename = "context.read")]
    Context {
        goal: GoalId,
        view: ContextViewMode,
        task: Option<TaskId>,
        after: Option<ContextCursor>,
        /// Most items on this page, at least 1.
        /// A value above 32 is treated as 32; follow next for the rest.
        limit: u32,
        preview_chars: Option<u32>,
        unread_only: bool,
    },
    #[serde(rename = "context.acknowledge")]
    ContextAcknowledge {
        goal: GoalId,
        receipt: ContextReceipt,
    },
    #[serde(rename = "invitation.inspect")]
    InvitationInspect { ticket: Ticket },
    #[serde(rename = "invitation.list")]
    GoalInvitations { goal: GoalId },
    #[serde(rename = "invitation.revoke")]
    InvitationRevoke {
        goal: GoalId,
        invitation: Option<String>,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Audience {
    /// The owner credential only, never on behalf of a principal.
    Owner,
    /// Any enrolled principal within its local level, or the owner (on behalf of a
    /// principal unless the request is read-only).
    Agent,
    /// The owner credential on a goal this daemon hosts.
    Host,
    /// An agent or private authoring credential within its own catalog.
    Author,
}

/// The stable facts about one operation, for command tables, MCP tool lists,
/// logs and errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Operation {
    /// Stable name: the command name, the JSON tag of the request and the
    /// name in logs and errors.
    pub name: &'static str,
    /// True if the operation authors no event and changes nothing another
    /// caller can observe. Bridges advertise these as read-only so clients
    /// need not prompt for them. Reading events or context never acknowledges
    /// content; context acknowledgment is a separate session-bound write.
    pub read_only: bool,
    /// True if the request names a goal; see [`Request::goal`].
    pub goal_scoped: bool,
    /// Who may make the request.
    pub audience: Audience,
    /// True if the stdio bridge offers the operation to the model as a tool.
    /// False for owner-only operations, for operations that carry raw bytes,
    /// and for session records, which belong to the launcher and adapter.
    pub tool: bool,
    /// One line for a command or tool listing.
    pub summary: &'static str,
}

impl Operation {
    /// The MCP tool name: `locust_` plus the operation name with dots
    /// replaced by underscores.
    pub fn tool_name(&self) -> String {
        format!("locust_{}", self.name.replace('.', "_"))
    }
}

/// Builds [`OPERATIONS`] and [`Request::operation`] from one list, so the
/// table and the enum cannot drift: the generated `match` has no wildcard,
/// which means a new `Request` variant does not compile until it has a row.
/// Rows are `Variant => (name, read-only, goal-scoped, audience, tool,
/// summary)` in the enum's declaration order.
macro_rules! operations {
    ($(
        $variant:ident $({ $($fields:tt)* })? =>
            ($name:literal, $read_only:expr, $goal_scoped:expr, $audience:ident, $tool:expr, $summary:literal),
    )*) => {
        /// One entry per [`Request`] variant, in declaration order.
        pub const OPERATIONS: &[Operation] = &[$(
            Operation {
                name: $name,
                read_only: $read_only,
                goal_scoped: $goal_scoped,
                audience: Audience::$audience,
                tool: $tool,
                summary: $summary,
            },
        )*];

        /// Positions in [`OPERATIONS`], named after the variants.
        enum Slot {
            $($variant,)*
        }

        impl Request {
            /// This request's entry in [`OPERATIONS`].
            pub fn operation(&self) -> &'static Operation {
                let slot = match self {
                    $(Self::$variant $({ $($fields)* })? => Slot::$variant,)*
                };
                &OPERATIONS[slot as usize]
            }
        }
    };
}

operations! {
    FarmOn { .. } => ("farm.on", false, true, Host, false, "Enable owner-approved farm publication"),
    FarmOff { .. } => ("farm.off", false, true, Host, false, "Disable and delete farm publication"),
    FarmShow { .. } => ("farm.show", true, true, Owner, false, "Preview public farm data and consent"),
    FarmStatus => ("farm.status", true, false, Owner, false, "Show farm publication status"),
    FarmConsent { .. } => ("farm.consent", false, true, Owner, false, "Approve a local public profile"),
    Status => ("status", true, false, Agent, true, "Shows what waits for the person, each with the command that settles it, then every goal with each agent's name, roles, level and standing. An agent reads its own entries, worded about its owner."),
    Shutdown => ("daemon.stop", false, false, Owner, false, "daemon stop"),
    AgentEnroll { .. } => ("agent.enroll", false, false, Owner, false, "agent enroll"),
    AuthorEnroll { .. } => ("author.enroll", false, false, Owner, false, "author enroll"),
    AgentRevoke { .. } => ("agent.revoke", false, false, Owner, false, "agent revoke"),
    AgentReconnect { .. } => ("agent.reconnect", false, false, Owner, false, "agent reconnect"),
    SessionReport { .. } => ("session.report", false, false, Agent, false, "Records this session's report on this computer, replacing the last one: client and version, session state, resume identifier, capabilities and adapter detail. The launcher or adapter sends it; agent work never needs it."),
    Session { .. } => ("session.show", true, false, Agent, false, "Shows one of the caller's sessions on this computer, the calling one when none is named: its last report and when it was written, whether a connection holds it open, and its claims on unfinished attempts."),
    Sessions => ("sessions", true, false, Agent, false, "Lists the sessions on this computer, only the caller's own for an agent, each with its last report and when it was written, whether a connection holds it open, and its claims on unfinished attempts."),
    SessionDrop { .. } => ("session.drop", false, false, Agent, false, "Removes a session's record from this computer once it holds no unfinished attempt. Only the session itself or the owner may drop it, and the session stays bound to its agent."),
    GoalCreate { .. } => ("goal.create", false, false, Owner, false, "goal create"),
    GoalJoin { .. } => ("goal.join", false, false, Owner, false, "goal join"),
    GoalInvite { .. } => ("goal.invite", false, true, Host, false, "goal invite"),
    GoalLeave { .. } => ("goal.leave", false, true, Owner, false, "goal leave"),
    GoalContinue { .. } => ("goal.continue", false, true, Owner, false, "goal continue"),
    LevelSet { .. } => ("level.set", false, true, Owner, false, "Set this agent's local level in a goal"),
    TaskAllow { .. } => ("task.allow", false, true, Owner, false, "Allow this agent to take one task in its current round"),
    TaskDisallow { .. } => ("task.disallow", false, true, Owner, false, "Remove this agent's task allowance or request"),
    GoalStatus { .. } => ("goal.status", true, true, Agent, true, "Shows one goal: its host, members, roles, rules, shared files and each local agent's standing in it."),
    MemberRemove { .. } => ("member.remove", false, true, Host, false, "member remove"),
    RoleGive { .. } => ("role.give", false, true, Host, false, "role give"),
    RoleTake { .. } => ("role.take", false, true, Host, false, "role take"),
    RulesBind { .. } => ("rules.bind", false, true, Host, false, "rules bind"),
    CheckoutRegister { .. } => ("checkout.register", false, true, Agent, true, "Make a new folder for this agent from an accepted shared revision"),
    WorkspaceConnect { .. } => ("workspace.connect", false, true, Owner, false, "Connect a folder you named to this goal for an agent"),
    CheckoutBindSession { .. } => ("checkout.bind_session", false, true, Agent, true, "Bind this session explicitly to the checkout it uses"),
    Checkouts { .. } => ("checkouts", true, true, Agent, true, "Read your local checkout bindings and active operations"),
    WorkspaceRecoveryParentCheck { .. } => ("workspace.recovery.parent", true, true, Agent, false, "Check a canonical recovery parent is outside every local managed tree before staging"),
    WorkspaceOperationPrepare { .. } => ("workspace.operation.prepare", false, true, Agent, false, "Durably register an exact captured candidate or prepared file transition"),
    WorkspaceOperationShow { .. } => ("workspace.operation.show", true, true, Agent, true, "Read an exact local workspace operation and its receipt"),
    WorkspaceOperations { .. } => ("workspace.operations", true, true, Agent, true, "Read your durable local workspace operations"),
    WorkspaceOperationComplete { .. } => ("workspace.operation.complete", false, true, Agent, false, "Finalize a verified file transition and its checkout base atomically"),
    WorkspaceEpochSet { .. } => ("workspace.epoch", false, true, Host, false, "Fence workspace history and pin explicit policy plus an exact checkpoint or restoration"),
    WorkspaceHead { .. } => ("workspace.head", true, true, Agent, true, "Read accepted workspace authority and independent content availability"),
    WorkspaceTree { .. } => ("workspace.tree", true, true, Agent, true, "List an exact retained workspace tree with validated content"),
    WorkspaceRead { .. } => ("workspace.read", true, true, Agent, true, "Read one inert file from an exact retained workspace revision"),
    WorkspaceProposals { .. } => ("workspace.proposals", true, true, Agent, true, "Read exact workspace proposals and whether their parents are stale"),
    WorkspaceProposal { .. } => ("workspace.proposal", true, true, Agent, true, "Read one exact candidate, provenance and review state"),
    WorkspaceRevision { .. } => ("workspace.revision", true, true, Agent, true, "Read an exact workspace selection and retained-lineage status"),
    WorkspacePublish { .. } => ("workspace.publish", false, true, Agent, true, "Publish the exact frozen candidate named by your prepared operation"),
    WorkspaceIntegrate { .. } => ("workspace.integrate", false, true, Agent, true, "Integrate an exact reviewed candidate at the prepared expected epoch and head"),
    Board { .. } => ("board", true, true, Agent, true, "Lists every task in the goal, the only read that does, each with its title, creator, the attempts and results of its current round, and whether it is completed, selected or closed."),
    Task { .. } => ("task.show", true, true, Agent, true, "Shows one task in full: its text, named inputs, parent, task type and the rules in effect for its current round as JSON, with the summary the board gives. Read it before working on the task."),
    Event { .. } => ("event.show", true, true, Agent, true, "Shows one event in full: its author, kind and standing, its signed body, its text when held here, the task it concerns and whether each content object it names is held. Use it to judge a result."),
    TaskOpen { .. } => ("task.open", false, true, Agent, true, "Opens a task from its text, whose first line is its title, with named inputs, an optional task type and an optional parent. A task with a parent follows that parent's rules, others the goal's current rules."),
    TaskRevise { .. } => ("task.revise", false, true, Host, false, "task revise"),
    WorkOffer { .. } => ("work.offer", false, true, Agent, true, "Offers a task to one named member, where the goal's rules let the caller offer to that member. The member then starts an attempt with the offer or declines it; an offer alone is not an attempt."),
    AttemptStart { .. } => ("attempt.start", false, true, Agent, true, "Takes a task: starts an attempt on it, or recovers this session's claim on the same task and offer. The goal's rules and the agent's level decide whether it may."),
    AttemptTakeover { .. } => ("attempt.takeover", false, true, Agent, true, "Takes over an attempt another session of this agent holds, after that session's work was checked. A held claim does not prove a process is running."),
    WorkDecline { .. } => ("work.decline", false, true, Agent, true, "Declines a work offer made to the caller, so the task is not started through it. Only the offer's recipient may decline, and only an offer it has not yet accepted or declined."),
    AttemptCancel { .. } => ("attempt.cancel", false, true, Agent, true, "Asks the worker to stop one attempt; only its worker or the member who offered it may ask. Once the request arrives, the attempt takes no more results or progress, and its worker must acknowledge it."),
    AttemptReport { .. } => ("attempt.report", false, true, Agent, true, "Report progress or end an attempt; completed requires a published contribution naming the attempt, not review or workspace integration"),
    ContributionPublish { .. } => ("contribution.publish", false, true, Agent, true, "Publishes a result or finding with a summary, declared sources and artifacts. With an attempt and its generation it is that attempt's result; with neither it is a goal-wide finding that members see in context."),
    Contributions { .. } => ("contributions", true, true, Agent, true, "Lists the goal's results and findings, or one task's, each with its author, attempt, text, declared sources, artifacts and whether it is approved or selected. Reuse goal-wide findings and other tasks' results; on a task you are attempting, publish your result before reading other members' results on it."),
    ContributionInspect { .. } => ("contribution.inspect", true, true, Agent, true, "Inspect a contribution, its author-declared sources and exact attempt/task chain; declarations are not proof of model use"),
    CompletionDeclare { .. } => ("completion.declare", false, true, Agent, true, "Declares one exact result, document revision or workspace proposal complete. It is refused unless the completion rule in effect accepts a declaration from the caller, for example from the result's own author."),
    ReviewRecord { .. } => ("review.record", false, true, Agent, true, "Records an approval or a reject of one exact result. A member's latest review of a result is the one that counts. A pick, a plan text or a file change already recorded on an earlier approval is not undone. Where the rule asks for no review, a review is an opinion and changes nothing about counting."),
    CheckAttest { .. } => ("check.attest", false, true, Agent, true, "Records that a named check passed or failed on one exact result, document revision or workspace proposal, with a note. It is refused unless the completion rule in effect names that check and the caller."),
    ScopeSelect { .. } => ("scope.select", false, true, Agent, true, "Selects one result or document revision as the outcome of its task, document or goal; only the member the rules name may select, and expected names the previous selection. Workspace changes are refused here and integrated instead."),
    ScopeClose { .. } => ("scope.close", false, true, Agent, true, "Closes the current round of a task, document or the goal; only the member the rules name to finish it may, and expected names the previous close or reopen. A closed task takes no new attempts until reopened."),
    ScopeReopen { .. } => ("scope.reopen", false, true, Agent, true, "Reopens a task, document or the goal closed in its current round; only the member the rules name to finish it may, and expected names that close. A reopened task accepts attempts as it did before the close."),
    DeliveryAcknowledge { .. } => ("delivery.acknowledge", false, true, Agent, true, "Records that the caller, a recipient of one durable delivery, received and handled it; pending then shows it acknowledged. It does not replace review, execution or scope decisions."),
    CancelAcknowledge { .. } => ("cancel.acknowledge", false, true, Agent, true, "Acknowledge cancellation after checking local execution: stopped ends the attempt, completed requires its published result, uncertain keeps work fenced but permits ending"),
    PendingPage { .. } => ("pending.page", true, true, Agent, true, "Read a page of pending work, with complete category counts and revision-bound continuation"),
    Pending { .. } => ("pending", true, true, Agent, true, "Lists what the caller can do next in a goal: tasks to start, results to review, claims, cancellations and deliveries. Tasks to start come least-attended first, each with the members already attempting it; results to review carry the approvals so far and the number needed."),
    Wait { .. } => ("wait", true, true, Agent, true, "Waits until the goal changes or the timeout passes, then answers as pending does. Tasks to start come least-attended first, each with the members already attempting it; results to review carry the approvals so far and the number needed."),
    Events { .. } => ("events", true, true, Agent, true, "Pages through the goal's feed in the order events took effect here, each with its position, author, kind and standing, at most 256 per page. Pass the last position as after; reading acknowledges nothing."),
    DocRead { .. } => ("doc.read", true, true, Agent, true, "Shows the goal's plan or summary document: the selected revision and its text when held here, and the other proposed revisions, each readable as an event."),
    DocRevise { .. } => ("doc.revise", false, true, Agent, true, "Proposes a new revision of the goal's plan or summary document as full text, with base naming the revision it builds on. Until it is selected, reading the document lists it as a proposal."),
    BlobPut { .. } => ("blob.put", false, true, Agent, false, "Stores bytes as one sealed content object of the goal and returns its hash. Use it before naming the object as a result's artifact or a task's input."),
    BlobGet { .. } => ("blob.get", true, true, Agent, false, "Returns the plaintext of one content object the goal names, if held here and not withdrawn. A missing object is requested from peers, and the answer is unavailable until it arrives."),
    BlobStat { .. } => ("blob.stat", true, true, Agent, true, "Reports for each hash asked about whether the goal's content object is held here, requested from peers, unavailable or unknown to the goal. It fetches nothing; use it to check artifacts before relying on them."),
    BlobWithdraw { .. } => ("blob.withdraw", false, true, Agent, true, "Withdraws one content object of the goal on this computer: peers are no longer served it and local reads refuse it, but the stored bytes stay as evidence. Other computers keep their copies."),
    FormationDraftCreate { .. } => ("formation.draft.create", false, false, Author, true, "Creates a private formation draft under a new identifier with expected revision 0, from source that may be incomplete or invalid. Start a reusable definition here; publishing validates it."),
    FormationDraftUpdate { .. } => ("formation.draft.update", false, false, Author, true, "Replaces a draft's source when expected revision matches, returning the draft at its next revision. A mismatch is refused with the current draft, so local edits can be merged before retrying."),
    FormationDraft { .. } => ("formation.draft.show", true, false, Author, true, "Returns one of the caller's drafts with its revision, exact source and source hash. Read it before an update or a publish, since both need the current revision."),
    FormationDrafts => ("formation.drafts", true, false, Author, true, "Lists all of the caller's private formation drafts, each with its identifier, revision, exact source and source hash. Use it to find a draft to edit or publish."),
    FormationPublish { .. } => ("formation.publish", false, false, Author, true, "Publishes the exact draft revision and source hash under a new identifier as an immutable, validated definition with its normalized form and semantic hash. Repeating the same request returns the same publication."),
    FormationPublication { .. } => ("formation.show", true, false, Author, true, "Returns one of the caller's immutable publications: the draft and revision it came from, its exact source and source hash, the normalized definition and its semantic hash."),
    FormationPublications => ("formation.list", true, false, Author, true, "Lists all of the caller's immutable formation publications, each with the draft and revision it came from, its source, normalized definition and semantic hash."),
    FormationPresentation { .. } => ("formation.presentation.show", true, false, Author, true, "Returns a draft's presentation layout, its JSON data and revision. The layout is kept apart from the source and never enters any semantic hash."),
    FormationPresentationUpdate { .. } => ("formation.presentation.update", false, false, Author, true, "Replaces a draft's presentation layout JSON when expected revision matches. The layout has its own revision and never changes the source or any publication's semantic identity."),
    FormationValidate { .. } => ("formation.validate", true, false, Author, true, "Checks formation source text without saving it and returns whether it is valid, with diagnostics and, when valid, the normalized definition, semantic hash and explanation. Use it while editing, before publishing."),
    FormationExplain { .. } => ("formation.explain", true, false, Author, true, "Inspects formation source text without saving it, as validation does; when valid, the explanation summarizes the definition and lists required roles and inputs, authority roles and contextual checks."),
    Context { .. } => ("context.read", true, true, Agent, true, "Read a coherent goal or task brief with attributed findings, progress, review reasons and pending actions; reads do not acknowledge content"),
    ContextAcknowledge { .. } => ("context.acknowledge", false, true, Agent, true, "Acknowledge the complete content delivered to this execution session using its exact receipt; other sessions remain unread"),
    InvitationInspect { .. } => ("invitation.inspect", true, false, Agent, false, "Verify a signed invitation and preview its issuer, goal and sharing boundary without joining"),
    GoalInvitations { .. } => ("invitation.list", true, true, Host, false, "List issued invitations and their current redemption, expiry or revocation state without revealing ticket secrets"),
    InvitationRevoke { .. } => ("invitation.revoke", false, true, Host, false, "Revoke an issued invitation; existing membership remains a separate decision"),
}
impl Request {
    pub fn name(&self) -> &'static str {
        self.operation().name
    }
    pub fn is_read_only(&self) -> bool {
        self.operation().read_only
    }
    pub fn goal(&self) -> Option<GoalId> {
        match self {
            Self::FarmOn { goal, .. }
            | Self::FarmOff { goal }
            | Self::FarmShow { goal }
            | Self::FarmConsent { goal, .. } => Some(*goal),
            Self::FarmStatus => None,
            Self::Status => None,
            Self::Shutdown => None,
            Self::AgentEnroll { .. } => None,
            Self::AuthorEnroll { .. } => None,
            Self::AgentRevoke { .. } => None,
            Self::AgentReconnect { .. } => None,
            Self::SessionReport { .. } => None,
            Self::Session { .. } => None,
            Self::Sessions => None,
            Self::SessionDrop { .. } => None,
            Self::GoalCreate { .. } => None,
            Self::GoalJoin { .. } => None,
            Self::GoalInvite { goal, .. } => Some(*goal),
            Self::GoalLeave { goal, .. } => Some(*goal),
            Self::GoalContinue { goal } => Some(*goal),
            Self::LevelSet { goal, .. }
            | Self::TaskAllow { goal, .. }
            | Self::TaskDisallow { goal, .. } => Some(*goal),
            Self::GoalStatus { goal, .. } => Some(*goal),
            Self::MemberRemove { goal, .. } => Some(*goal),
            Self::RoleGive { goal, .. } | Self::RoleTake { goal, .. } => Some(*goal),
            Self::RulesBind { goal, .. } => Some(*goal),
            Self::WorkspaceRecoveryParentCheck { goal, .. } => Some(*goal),
            Self::WorkspaceEpochSet { goal, .. }
            | Self::WorkspaceHead { goal }
            | Self::WorkspaceTree { goal, .. }
            | Self::WorkspaceRead { goal, .. }
            | Self::WorkspaceProposals { goal }
            | Self::WorkspaceProposal { goal, .. }
            | Self::WorkspaceRevision { goal, .. }
            | Self::WorkspacePublish { goal, .. }
            | Self::WorkspaceIntegrate { goal, .. } => Some(*goal),
            Self::CheckoutRegister { goal, .. }
            | Self::WorkspaceConnect { goal, .. }
            | Self::CheckoutBindSession { goal, .. }
            | Self::Checkouts { goal }
            | Self::WorkspaceOperationPrepare { goal, .. }
            | Self::WorkspaceOperationShow { goal, .. }
            | Self::WorkspaceOperations { goal }
            | Self::WorkspaceOperationComplete { goal, .. } => Some(*goal),
            Self::Board { goal, .. } => Some(*goal),
            Self::Task { goal, .. } => Some(*goal),
            Self::Event { goal, .. } => Some(*goal),
            Self::TaskOpen { goal, .. } => Some(*goal),
            Self::TaskRevise { goal, .. } => Some(*goal),
            Self::WorkOffer { goal, .. } => Some(*goal),
            Self::AttemptStart { goal, .. } => Some(*goal),
            Self::AttemptTakeover { goal, .. } => Some(*goal),
            Self::WorkDecline { goal, .. } => Some(*goal),
            Self::AttemptCancel { goal, .. } => Some(*goal),
            Self::AttemptReport { goal, .. } => Some(*goal),
            Self::ContributionPublish { goal, .. } => Some(*goal),
            Self::Contributions { goal, .. } | Self::ContributionInspect { goal, .. } => {
                Some(*goal)
            }
            Self::CompletionDeclare { goal, .. } => Some(*goal),
            Self::ReviewRecord { goal, .. } => Some(*goal),
            Self::CheckAttest { goal, .. } => Some(*goal),
            Self::ScopeSelect { goal, .. } => Some(*goal),
            Self::ScopeClose { goal, .. } => Some(*goal),
            Self::ScopeReopen { goal, .. } => Some(*goal),
            Self::DeliveryAcknowledge { goal, .. } => Some(*goal),
            Self::CancelAcknowledge { goal, .. } => Some(*goal),
            Self::Pending { goal, .. } | Self::PendingPage { goal, .. } => Some(*goal),
            Self::Wait { goal, .. } => Some(*goal),
            Self::Events { goal, .. } => Some(*goal),
            Self::DocRead { goal, .. } => Some(*goal),
            Self::DocRevise { goal, .. } => Some(*goal),
            Self::BlobPut { goal, .. } => Some(*goal),
            Self::BlobGet { goal, .. } => Some(*goal),
            Self::BlobStat { goal, .. } => Some(*goal),
            Self::BlobWithdraw { goal, .. } => Some(*goal),
            Self::FormationDraftCreate { .. } => None,
            Self::FormationDraftUpdate { .. } => None,
            Self::FormationDraft { .. } => None,
            Self::FormationDrafts => None,
            Self::FormationPublish { .. } => None,
            Self::FormationPublication { .. } => None,
            Self::FormationPublications => None,
            Self::FormationPresentation { .. } => None,
            Self::FormationPresentationUpdate { .. } => None,
            Self::FormationValidate { .. } => None,
            Self::FormationExplain { .. } => None,
            Self::Context { goal, .. }
            | Self::ContextAcknowledge { goal, .. }
            | Self::GoalInvitations { goal }
            | Self::InvitationRevoke { goal, .. } => Some(*goal),
            Self::InvitationInspect { .. } => None,
        }
    }
    pub fn check(&self) -> Result<(), ApiError> {
        match self {
            Self::GoalCreate { name, .. } | Self::GoalJoin { name, .. }
                if !crate::event::is_member_name(name) =>
            {
                Err(ApiError::new(
                    ErrorCode::Invalid,
                    "member name must contain 1 to 64 bytes with no outer spaces or control characters, and cannot look like a key (8 to 64 hex digits)",
                ))
            }
            Self::GoalInvite {
                role: Some(role), ..
            }
            | Self::RoleGive { role, .. }
            | Self::RoleTake { role, .. }
                if !crate::organization::is_role_name(role) =>
            {
                Err(ApiError::new(
                    ErrorCode::Invalid,
                    "role name must contain visible text and no control characters",
                ))
            }
            Self::RoleGive { expected, .. } | Self::RoleTake { expected, .. }
                if !expected.is_sorted_by(|a, b| a < b) =>
            {
                Err(ApiError::new(
                    ErrorCode::Invalid,
                    "expected role holders must be distinct and ascending",
                ))
            }
            Self::AgentEnroll { name, .. } | Self::AuthorEnroll { name, .. }
                if !is_agent_name(name) =>
            {
                Err(ApiError::new(
                    ErrorCode::Invalid,
                    "principal name must contain 1 to 32 a-z, 0-9 or - characters",
                ))
            }
            Self::ContributionPublish {
                attempt,
                generation,
                ..
            } if attempt.is_some() != generation.is_some() => Err(ApiError::new(
                ErrorCode::Invalid,
                "attempt and generation must be supplied together",
            )),
            Self::ContributionPublish { artifacts, .. } if artifacts.len() > MAX_ARTIFACTS => Err(
                ApiError::new(ErrorCode::LimitExceeded, "too many contribution artifacts"),
            ),
            Self::BlobStat { hashes, .. } if hashes.len() > MAX_STAT_HASHES => Err(ApiError::new(
                ErrorCode::LimitExceeded,
                "too many content hashes",
            )),
            Self::SessionReport { record } => record.check(),
            _ => Ok(()),
        }
    }
    pub fn is_answered_by(&self, response: &Response) -> bool {
        match self {
            Self::FarmStatus => matches!(response, Response::Farms(_)),
            Self::FarmOn { .. }
            | Self::FarmOff { .. }
            | Self::FarmShow { .. }
            | Self::FarmConsent { .. } => matches!(response, Response::FarmPreview(_)),
            Self::Status => matches!(response, Response::Status(_)),
            Self::Context { .. } => matches!(response, Response::Context(_)),
            Self::ContextAcknowledge { .. } => matches!(response, Response::ContextAcknowledged(_)),
            Self::InvitationInspect { .. } => {
                matches!(response, Response::InvitationInspected { .. })
            }
            Self::GoalInvitations { .. } => matches!(response, Response::Invitations { .. }),
            Self::InvitationRevoke { .. } => matches!(
                response,
                Response::InvitationRevoked { .. } | Response::InvitationsRevoked { .. }
            ),
            Self::LevelSet { .. } | Self::TaskAllow { .. } => {
                matches!(response, Response::Abilities(_))
            }
            Self::TaskDisallow { .. } => matches!(response, Response::TaskDisallowed { .. }),

            Self::Shutdown => matches!(response, Response::Done),
            Self::AgentEnroll { .. } => matches!(response, Response::AgentEnrolled { .. }),
            Self::AuthorEnroll { .. } => matches!(response, Response::AuthorEnrolled { .. }),
            Self::AgentRevoke { .. } => matches!(response, Response::Done),
            Self::AgentReconnect { .. } => matches!(response, Response::Done),
            Self::SessionReport { .. } => matches!(response, Response::Done),
            Self::Session { .. } => matches!(response, Response::Session(_)),
            Self::Sessions => matches!(response, Response::Sessions(_)),
            Self::SessionDrop { .. } => matches!(response, Response::Done),
            Self::GoalCreate { .. } => matches!(response, Response::GoalCreated { .. }),
            Self::GoalJoin { .. } => matches!(response, Response::Joined { .. }),
            Self::GoalInvite { .. } => matches!(response, Response::Invited { .. }),
            Self::GoalLeave { .. } => matches!(response, Response::Recorded { .. }),
            Self::GoalContinue { .. } => matches!(response, Response::Continued { .. }),
            Self::GoalStatus { .. } => matches!(response, Response::GoalStatus(_)),
            Self::MemberRemove { .. } => matches!(response, Response::Recorded { .. }),
            Self::RoleGive { .. } => matches!(response, Response::Recorded { .. }),
            Self::RoleTake { .. } => matches!(response, Response::Recorded { .. } | Response::Done),
            Self::RulesBind { .. } => matches!(response, Response::Recorded { .. }),
            Self::WorkspaceRecoveryParentCheck { .. } => matches!(response, Response::Done),
            Self::WorkspaceEpochSet { .. } => matches!(response, Response::Recorded { .. }),
            Self::WorkspaceHead { .. } => matches!(response, Response::Workspace(_)),
            Self::WorkspaceTree { .. } => matches!(response, Response::WorkspaceTree(_)),
            Self::WorkspaceRead { .. } => matches!(response, Response::WorkspaceFile(_)),
            Self::WorkspaceProposals { .. } => matches!(response, Response::WorkspaceProposals(_)),
            Self::WorkspaceProposal { .. } => matches!(response, Response::WorkspaceProposal(_)),
            Self::WorkspaceRevision { .. } => matches!(response, Response::WorkspaceRevision(_)),
            Self::WorkspacePublish { .. } | Self::WorkspaceIntegrate { .. } => {
                matches!(response, Response::WorkspaceOperation(_))
            }
            Self::CheckoutRegister { .. } => matches!(response, Response::Checkout(_)),
            Self::WorkspaceConnect { .. } => matches!(response, Response::Checkout(_)),
            Self::CheckoutBindSession { .. } => matches!(response, Response::Checkout(_)),
            Self::Checkouts { .. } => matches!(response, Response::Checkouts(_)),
            Self::WorkspaceOperationPrepare { .. }
            | Self::WorkspaceOperationShow { .. }
            | Self::WorkspaceOperationComplete { .. } => {
                matches!(response, Response::WorkspaceOperation(_))
            }
            Self::WorkspaceOperations { .. } => {
                matches!(response, Response::WorkspaceOperations(_))
            }
            Self::Board { .. } => matches!(response, Response::Board(_)),
            Self::Task { .. } => matches!(response, Response::Task(_)),
            Self::Event { .. } => matches!(response, Response::Event(_)),
            Self::TaskOpen { .. } => matches!(response, Response::Recorded { .. }),
            Self::TaskRevise { .. } => matches!(response, Response::Recorded { .. }),
            Self::WorkOffer { .. } => matches!(response, Response::Recorded { .. }),
            Self::AttemptStart { .. } => matches!(response, Response::Claimed(_)),
            Self::AttemptTakeover { .. } => matches!(response, Response::Claimed(_)),
            Self::WorkDecline { .. } => matches!(response, Response::Recorded { .. }),
            Self::AttemptCancel { .. } => matches!(response, Response::Recorded { .. }),
            Self::AttemptReport { .. } => matches!(response, Response::Recorded { .. }),
            Self::ContributionPublish { .. } => matches!(response, Response::Recorded { .. }),
            Self::Contributions { .. } => matches!(response, Response::Contributions(_)),
            Self::ContributionInspect { .. } => {
                matches!(response, Response::ContributionInspected(_))
            }
            Self::CompletionDeclare { .. } => matches!(response, Response::Recorded { .. }),
            Self::ReviewRecord { .. } => matches!(response, Response::Recorded { .. }),
            Self::CheckAttest { .. } => matches!(response, Response::Recorded { .. }),
            Self::ScopeSelect { .. } => matches!(response, Response::Recorded { .. }),
            Self::ScopeClose { .. } => matches!(response, Response::Recorded { .. }),
            Self::ScopeReopen { .. } => matches!(response, Response::Recorded { .. }),
            Self::DeliveryAcknowledge { .. } => matches!(response, Response::Recorded { .. }),
            Self::CancelAcknowledge { .. } => matches!(response, Response::Recorded { .. }),
            Self::Pending { .. } => matches!(response, Response::Pending(_)),
            Self::PendingPage { .. } => matches!(response, Response::PendingPage(_)),
            Self::Wait { .. } => matches!(response, Response::Waited(_)),
            Self::Events { .. } => matches!(response, Response::Events(_)),
            Self::DocRead { .. } => matches!(response, Response::Doc(_)),
            Self::DocRevise { .. } => matches!(response, Response::Recorded { .. }),
            Self::BlobPut { .. } => matches!(response, Response::BlobStored { .. }),
            Self::BlobGet { .. } => matches!(response, Response::Blob { .. }),
            Self::BlobStat { .. } => matches!(response, Response::BlobStates(_)),
            Self::BlobWithdraw { .. } => matches!(response, Response::Done),
            Self::FormationDraftCreate { .. } => matches!(response, Response::FormationDraft(_)),
            Self::FormationDraftUpdate { .. } => matches!(response, Response::FormationDraft(_)),
            Self::FormationDraft { .. } => matches!(response, Response::FormationDraft(_)),
            Self::FormationDrafts => matches!(response, Response::FormationDrafts(_)),
            Self::FormationPublish { .. } => matches!(response, Response::FormationPublication(_)),
            Self::FormationPublication { .. } => {
                matches!(response, Response::FormationPublication(_))
            }
            Self::FormationPublications => matches!(response, Response::FormationPublications(_)),
            Self::FormationPresentation { .. } => {
                matches!(response, Response::FormationPresentation(_))
            }
            Self::FormationPresentationUpdate { .. } => {
                matches!(response, Response::FormationPresentation(_))
            }
            Self::FormationValidate { .. } => {
                matches!(response, Response::FormationInspection { .. })
            }
            Self::FormationExplain { .. } => {
                matches!(response, Response::FormationInspection { .. })
            }
        }
    }
}
/// Generated full request schema. The immutable schema is generated once;
/// callers receive an owned copy they can inspect or modify independently.
pub fn request_schema() -> serde_json::Value {
    schema::request().clone()
}

/// Offline discovery from the same types and registry used by the daemon,
/// CLI and MCP bridge. No credentials or local state are inspected.
pub fn contract() -> serde_json::Value {
    use serde_json::json;
    json!({
        "api_version": crate::API_VERSION,
        "protocol_version": crate::PROTOCOL_VERSION,
        "formation_schema_version": crate::organization::SCHEMA_VERSION,
        "operations": OPERATIONS.iter().map(|operation| json!({
            "name": operation.name,
            "summary": operation.summary,
            "read_only": operation.read_only,
            "goal_scoped": operation.goal_scoped,
            "audience": match operation.audience {
                Audience::Owner => "owner",
                Audience::Agent => "agent",
                Audience::Host => "host",
                Audience::Author => "author",
            },
            "mcp_tool": operation.tool.then(|| operation.tool_name()),
        })).collect::<Vec<_>>(),
        "request_schema": request_schema(),
        "response_schema": schema_for!(Response),
        "event_schema": schema_for!(crate::event::Body),
        "farm_snapshot": crate::farm::snapshot_schema(),
        "error_schema": schema_for!(ApiError),
        "refusal_schema": schema_for!(Refused),
    })
}
/// What a request answers with when it succeeds; which variant answers which
/// request is [`Request::is_answered_by`]. No response ever carries a secret.
// One response is built per request and moved into its frame; boxing the
// goal status would add an allocation to the most common read.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Response {
    Workspace(WorkspaceView),
    WorkspaceTree(WorkspaceTreeView),
    WorkspaceFile(WorkspaceFileView),
    WorkspaceProposals(Vec<WorkspaceProposalView>),
    WorkspaceProposal(WorkspaceProposalView),
    WorkspaceRevision(WorkspaceRevisionView),
    Checkout(Checkout),
    Checkouts(Vec<Checkout>),
    WorkspaceOperation(WorkspaceOperation),
    WorkspaceOperations(Vec<WorkspaceOperation>),
    FarmPreview(crate::farm::FarmPreview),
    Farms(Vec<crate::farm::FarmStatus>),
    /// The request succeeded and has nothing to return.
    Done,
    /// Answers `status`.
    Status(DaemonStatus),
    /// Answers `agent.enroll`. Carries no credential: the client already has
    /// the one it enrolled.
    AgentEnrolled {
        /// The new principal's key, generated and held by the daemon.
        agent: PublicKey,
    },
    /// Answers `goal.create`.
    GoalCreated {
        /// The new goal: the digest of its genesis record.
        goal: GoalId,
    },
    /// Answers `goal.invite`.
    Invited {
        /// The text to hand to the invitee. It is the capability.
        ticket: Ticket,
    },
    /// What the caller joined, as the ticket named it. `membership` is
    /// `Joining` until the host's admission has arrived.
    Joined {
        host_name: String,
        /// The goal the ticket named.
        goal: GoalId,
        /// The goal's signing key, checked against its first record during joining.
        governance: PublicKey,
        /// `Joining` or `Member`.
        membership: Membership,
        /// The local level recorded for the joining agent.
        level: Level,
    },
    /// Answers `goal.status`.
    GoalStatus(GoalStatus),
    /// Answers `goal.continue`: how many of this daemon's keys in the goal
    /// were held and are not now. The person's word, not proof that this
    /// daemon caught up.
    Continued {
        keys: u32,
    },
    /// Answers `board`: every task of the goal.
    Board(Vec<TaskView>),
    /// Answers `task.show`.
    Task(TaskDetail),
    /// Answers `event.show`.
    Event(Box<EventDetail>),
    /// The request was signed and recorded as this event, which identifies
    /// what it created: a task, an attempt, a result, a cancellation, a
    /// note or a revision.
    Recorded {
        event: EventId,
    },
    /// Answers `attempt.start` and `attempt.takeover` with the claim the calling
    /// session now holds.
    Claimed(Claim),
    /// Answers `pending`.
    Pending(PendingWork),
    PendingPage(PendingPage),
    /// Answers `wait`.
    Waited(WaitOutcome),
    /// Answers `events`: feed entries in ascending position.
    Events(Vec<EventView>),
    /// Read attributed contributions, including findings without tasks.
    Contributions(Vec<ContributionView>),
    ContributionInspected(Box<ContributionInspection>),
    /// Answers `doc.read`.
    Doc(DocView),
    /// Answers `blob.put`.
    BlobStored {
        /// Hash of the stored, sealed object.
        hash: BlobHash,
    },
    /// Answers `blob.get`.
    Blob {
        /// The object's plaintext.
        #[serde(with = "codec::bytes")]
        #[schemars(with = "Vec<u8>")]
        bytes: Vec<u8>,
    },
    /// Answers `blob.stat`: one entry per hash asked about, in the order
    /// asked.
    BlobStates(Vec<BlobStatus>),
    /// Answers `session.show`.
    Session(SessionView),
    /// Answers `sessions`.
    Sessions(Vec<SessionView>),
    AuthorEnrolled {
        author: PublicKey,
    },
    FormationDraft(Draft),
    FormationDrafts(Vec<Draft>),
    FormationPublication(Publication),
    FormationPublications(Vec<Publication>),
    FormationPresentation(Presentation),
    FormationInspection {
        json: String,
    },
    Context(Box<ContextView>),
    ContextAcknowledged(ContextAcknowledgment),
    InvitationInspected {
        preview: InvitationPreview,
    },
    Invitations {
        invitations: Vec<InvitationSummary>,
    },
    InvitationRevoked {
        invitation: InvitationSummary,
    },
    InvitationsRevoked {
        count: u32,
    },
    Abilities(Abilities),
    /// The local allowance or request removed by `task.disallow`, including
    /// records hidden from the current abilities view by a closed task.
    TaskDisallowed {
        abilities: Abilities,
        changed: bool,
        was_allowed: bool,
    },
}

/// The one view: what waits for the person, each goal with each agent's
/// standing in it, and the agents. The owner sees everything; an agent sees
/// its own entries, worded about its owner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DaemonStatus {
    /// The daemon's own version, for people.
    pub daemon_version: String,
    /// Absent until the transport is running.
    pub endpoint: Option<EndpointId>,
    /// Every enrolled principal for the owner; only the calling principal
    /// for an agent.
    pub agents: Vec<AgentView>,
    /// What a command of the person settles, oldest first, each with that
    /// command. Nothing that waits for another computer or for nobody.
    pub waiting: Vec<WaitingForYou>,
    /// One entry per goal and local agent in it: all of them for the
    /// owner, only the calling agent's own for an agent.
    pub goals: Vec<GoalSummary>,
    /// For the owner, the goals this computer signed in that this copy of
    /// its Locust data does not hold, found at the last start that put the
    /// data back from a copy; 0 again after the next ordinary start. A goal
    /// hosted here cannot come back from such a copy; one joined needs its
    /// ticket again.
    pub lost_goals: u32,
}

/// One thing only the person can settle, with the line that settles it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WaitingForYou {
    pub goal: GoalId,
    pub title: Option<String>,
    /// The agent that waits, when one does.
    pub agent: Option<PublicKey>,
    /// Its name in the goal, else its local name.
    pub agent_name: Option<String>,
    pub kind: WaitingKind,
    /// Complete: it runs as printed.
    pub command: String,
}

/// Externally tagged, as `{"allow_task": {..}}`: binary frames carry it too.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WaitingKind {
    /// The agent tried to take a task and its level, ask, refused.
    AllowTask {
        task: TaskId,
        task_title: Option<String>,
    },
    /// The agent tried to take a task while set to read; an allowance would
    /// not help, so the command sets it to ask.
    SetAsk {
        task: TaskId,
        task_title: Option<String>,
    },
    /// The goal is catching up after this computer's data was put back from
    /// a copy, and no other computer can end the hold: only the person can
    /// say the copy is the newest. The command continues the goal.
    CatchingUp { holds: Vec<GuardView> },
}

/// One enrolled principal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgentView {
    /// The principal's key; the daemon holds the signing half.
    pub agent: PublicKey,
    /// Unique on this daemon; satisfies [`is_agent_name`].
    pub name: String,
    /// Whether this credential is limited to private formation authoring.
    pub author_only: bool,
    /// True once the credential was revoked. The name stays taken.
    pub revoked: bool,
}

/// How a local principal stands in a goal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Membership {
    /// An invitation was redeemed here; the host's admission has not
    /// arrived yet.
    Joining,
    /// Admitted and taking part.
    Member,
    /// The host removed the principal. History stays readable;
    /// nothing new is authored.
    Removed,
    /// The principal asked to leave and no longer takes part.
    Left,
    /// The host refused this local join attempt. No goal content is
    /// readable until a subsequent invitation is admitted.
    Refused,
}

/// One local agent in one goal, as `status` lists it: its name there, its
/// standing, the host, and what the person may still have to do about the
/// goal's invitations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GoalSummary {
    /// The goal this entry is about.
    pub goal: GoalId,
    /// Absent until the goal's founding text is held locally.
    pub title: Option<String>,
    /// The local agent this entry is about.
    pub member: PublicKey,
    /// The agent's name in the goal; the name asked for until admitted.
    pub name: String,
    /// How that agent stands in the goal.
    pub membership: Membership,
    /// The name of the host's agent in the goal, once its record is held.
    pub host_name: Option<String>,
    /// Open invitations: only for the owner, on a goal this daemon hosts.
    pub invitations_open: u32,
    /// When the latest open invitation expires, in ms since the Unix epoch.
    pub invitations_expire_ms: Option<u64>,
    /// Set while the goal's decisions cannot advance here, or this agent
    /// cannot sign in it.
    pub halted: Option<Halt>,
    pub abilities: Abilities,
    /// The hold on this agent and, where this daemon hosts the goal, on the
    /// governance key.
    pub guard: Vec<GuardView>,
    /// Set while this daemon is catching up after its data was put back
    /// from a copy: the number of invitations that restore revoked.
    pub restored: Option<u32>,
}

/// Why a goal's decisions cannot advance on this daemon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Halt {
    /// The authority's history conflicts with itself. The goal stays
    /// readable and exportable; requests that sign fail with
    /// [`ErrorCode::Halted`].
    AuthorityConflict,
    /// This daemon's signer for the goal is catching up after its data was
    /// put back from a copy: it cannot yet show what it already signed, so it
    /// signs nothing. Requests that sign fail with [`ErrorCode::ReadOnly`].
    SignerRecovery,
    /// This agent has two records at one position in the goal and signs
    /// nothing more in it. The goal itself is not halted.
    SignerConflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ScopeHalt {
    pub context: Context,
    pub reason: Halt,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GoalStatus {
    pub goal: GoalId,
    pub title: Option<String>,
    /// The goal's signing key, from the first record or, before it is held,
    /// from the ticket. It is never a member.
    pub governance: PublicKey,
    /// True on the computer that holds the goal's signing key.
    pub hosted_here: bool,
    /// The host's agent, named by the first record; absent until it is held.
    pub host: Option<PublicKey>,
    pub host_name: Option<String>,
    pub roles: BTreeMap<String, Vec<PublicKey>>,
    pub deciding: BTreeSet<String>,
    /// Roles an earlier binding lets one holder act on alone: a review one
    /// approval settles with the author not excluded, or any duty other than
    /// review. Open tasks may still follow those rules, so binding rules that
    /// count such a role's reviews gives it to no one.
    pub acting_alone: BTreeSet<String>,
    pub governance_head: Option<EventId>,
    pub current_rules: Option<EventId>,
    pub scope_halts: Vec<ScopeHalt>,
    pub members: Vec<MemberView>,
    pub halted: Option<Halt>,
    pub workspace: Option<WorkspaceView>,
    pub abilities: Vec<Abilities>,
    pub stalled: Vec<Stalled>,
    pub peers: Vec<PeerView>,
    /// For the owner, every key of this daemon held in the goal for its own
    /// reason; for an agent, its own and, where this daemon hosts the goal,
    /// the governance key's.
    pub guard: Vec<GuardView>,
    /// Set while this daemon is catching up after its data was put back
    /// from a copy: the number of invitations that restore revoked.
    pub restored: Option<u32>,
}
/// One current member of a goal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MemberView {
    pub name: String,
    /// The member's key.
    pub member: PublicKey,
    /// The daemon its admission bound it to.
    pub endpoint: EndpointId,
    /// True for principals this daemon holds keys for.
    pub local: bool,
    /// Where the admission sits on the host's chain; a lower number joined
    /// earlier. The member list is in this order.
    pub admitted: u64,
}

/// What this daemon knows about reaching one peer of a goal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PeerView {
    /// The peer daemon's transport identity.
    pub endpoint: EndpointId,
    /// True while a link to the peer is up.
    pub connected: bool,
    /// Last successful synchronization, Unix milliseconds by the local clock.
    /// State written by the peer after this is unknown here.
    pub last_sync_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TaskView {
    pub task: TaskId,
    pub context: Context,
    pub creator: PublicKey,
    /// True when the goal's signing key opened the task: a stage's task.
    pub by_host: bool,
    pub title: Option<String>,
    pub attempts: Vec<EventId>,
    pub contributions: Vec<EventId>,
    pub completed: bool,
    pub selected: Option<EventId>,
    pub closed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TaskDetail {
    pub view: TaskView,
    pub text: Option<String>,
    pub inputs: BTreeMap<String, BlobHash>,
    pub parent: Option<TaskId>,
    pub task_type: Option<String>,
    pub effective_rules_json: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Claim {
    pub goal: GoalId,
    pub task: TaskId,
    pub attempt: EventId,
    pub instance: InstanceId,
    pub generation: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkItem {
    pub task: TaskId,
    /// No member has a running attempt in this task's current round, as heard here.
    pub unattended: bool,
    pub offer: Option<EventId>,
    pub attempting: Vec<Attempting>,
    pub results: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CancelItem {
    pub task: TaskId,
    pub attempt: EventId,
    pub cancel: EventId,
    pub generation: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReviewItem {
    pub subject: EventId,
    pub context: Context,
    pub approvals: u32,
    pub needed: u32,
    pub verdicts: Vec<Verdict>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeliveryItem {
    pub effect: EffectId,
    pub context: Context,
    pub acknowledged: bool,
    /// The local daemon durably holds this recipient inbox item.
    pub received: bool,
    /// False when the effect or recipient no longer has effective authority.
    pub available: bool,
    pub action: String,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PendingWork {
    pub workspace: Option<Box<WorkspaceStatus>>,
    pub revision: u64,
    /// Shared content not yet acknowledged by this execution session.
    pub context_news: Option<ContextNews>,
    pub ask_first: Vec<WorkItem>,
    pub to_start: Vec<WorkItem>,
    pub claimed: Vec<Claim>,
    pub held_elsewhere: Vec<Claim>,
    pub to_acknowledge: Vec<CancelItem>,
    pub to_review: Vec<ReviewItem>,
    pub deliveries: Vec<DeliveryItem>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContributionView {
    pub contribution: EventId,
    /// Signed author declarations. They do not prove content was used.
    pub sources: Vec<EventId>,
    pub author: PublicKey,
    pub context: Context,
    pub attempt: Option<EventId>,
    pub approved: bool,
    pub selected: bool,
    pub evidence: Vec<EventId>,
    pub artifacts: Vec<BlobHash>,
    pub text: Option<String>,
}
/// Attributed provenance of one signed contribution. Inspecting never
/// acknowledges context or changes approval/selection eligibility.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContributionInspection {
    pub contribution: EventDetail,
    pub declared_sources: Vec<ContributionSource>,
    pub attempt: Option<ContributionSource>,
    /// Exact signed round event, not a newer current task round.
    pub task_round: Option<ContributionSource>,
}

/// One referenced event and its locally available header, standing and content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContributionSource {
    pub event: EventId,
    /// None when this replica does not yet hold the event. Content availability
    /// and attributed text, when held, are reported inside the detail.
    pub detail: Option<EventDetail>,
}

/// How an event stands in the goal's replicated state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Standing {
    /// Authorized and applied: its transition took effect.
    Effective,
    /// Held, but something it depends on has not arrived, so it has not been
    /// judged yet.
    Pending,
    /// Kept as attributed evidence; it grants nothing. Its author was not
    /// entitled to it, the transition was not allowed, or it belongs to a
    /// conflicting history.
    Excluded,
    /// Retained evidence whose authority proof conflicts within its exact scope.
    Disputed,
}

/// One event as the feed lists it: who wrote what kind of event, and how it
/// stands. The content is read with [`Request::Event`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EventView {
    /// Position in the goal's feed. The feed numbers events from 1 in the
    /// order they took effect here: the order this daemon applied them, as
    /// `Effective` or as `Excluded`. A `Pending` event has no position yet.
    /// Feed positions are local to this daemon and are not storage log
    /// positions, which follow arrival.
    pub position: Option<u64>,
    /// The event's identifier.
    pub event: EventId,
    /// The principal whose key signed it.
    pub author: PublicKey,
    /// [`crate::event::Body::kind`].
    pub kind: String,
    /// The author's clock. Diagnostic only.
    pub at_ms: u64,
    /// How the event stands in the goal's replicated state.
    pub standing: Standing,
    /// The participant directly asked the daemon to sign for this agent.
    pub by_owner: bool,
    /// The goal's signing key signed it: governance or a host's step.
    pub by_host: bool,
}

/// Whether the daemon can serve a content object of a goal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlobState {
    /// Held here; [`Request::BlobGet`] returns it.
    Held,
    /// Not held; the daemon has noted the want and is asking peers.
    Requested,
    /// Not obtainable now: withdrawn here, or no reachable peer holds it.
    Unavailable,
    /// Nothing in this goal names the hash.
    Unknown,
}

/// One content object of a goal and whether the daemon can serve it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BlobStatus {
    /// Hash of the stored, sealed object.
    pub hash: BlobHash,
    /// Whether it can be served now.
    pub state: BlobState,
}

/// One event in full: everything a host needs to judge a submitted
/// result or a proposed revision before deciding on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EventDetail {
    /// The event's feed entry: identifier, author, kind and standing.
    pub view: EventView,
    /// The decision the event anchors to; absent only on genesis.
    pub anchor: Option<EventId>,
    /// The signed, typed content: for a result its attempt, sources
    /// and artifacts; for a revision its document and base.
    pub body: Body,
    /// Names the event's text, if it has any.
    pub payload: Option<PayloadRef>,
    /// The payload as text: a result's summary, a revision's text. Absent
    /// when there is no payload or it is not held locally.
    pub text: Option<String>,
    /// The task the event concerns, resolved through its attempt, result
    /// or cancellation; absent for events that concern no task.
    pub task: Option<TaskId>,
    /// Every content object the event names, in the order of
    /// [`Header::blobs`](crate::event::Header::blobs) (payload first), with
    /// whether it is held.
    pub content: Vec<BlobStatus>,
}

/// A shared document: its selected revision and retained proposals.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocView {
    /// Which document this is.
    pub doc: Doc,
    /// The selected revision, if any.
    pub selected: Option<EventId>,
    /// Text of the selected revision; absent when it is not held locally.
    pub text: Option<String>,
    /// Revisions proposed against the selected one. Read
    /// one with [`Request::Event`].
    pub proposals: Vec<EventId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WaitOutcome {
    /// The goal changed since the revision the caller had seen. The lists
    /// may be empty when the change needs nothing from the caller; the feed
    /// and the board show what happened.
    Work(Box<PendingWork>),
    /// The timeout passed and nothing changed.
    NoEvent,
    /// The timeout passed, nothing changed, and no peer of this goal is
    /// currently reachable, so remote changes cannot arrive.
    Disconnected,
}

/// How far a client session has come, as its adapter observed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// The launch intent is recorded; the client process is not started yet.
    Launching,
    /// The client process exists. Nothing is known about its readiness.
    Started,
    /// The client is initialized and its tools or hooks answer.
    Ready,
    /// The client waits for the user: an approval, a sign-in, a prompt.
    Blocked,
    /// The client process ended.
    Exited,
    /// The adapter cannot tell, for example after an interruption.
    Unknown,
}

/// What a session's client and adapter support, each reported on its own.
/// None is inferred from another.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionCapabilities {
    /// The model can call Locust operations.
    pub tools: bool,
    /// Pending work can be surfaced while the session is active.
    pub active_delivery: bool,
    /// An idle session can be woken.
    pub idle_wake: bool,
    /// The session can be resumed by an explicit local action.
    pub manual_resume: bool,
    /// The session can be recovered after an interruption.
    pub recovery: bool,
    /// The whole client process tree runs under a participant-selected
    /// supervisor.
    pub confinement: bool,
}

/// What an adapter records about one client session. Typed where the daemon
/// or the owner reads it; `detail` is the adapter's own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionRecord {
    /// Canonical adapter report, approved for public display only by local consent.
    pub harness: crate::farm::Harness,
    /// The client and its version as the adapter reports them, for example
    /// `claude-code 2.1.0`. At most [`MAX_LOCAL_TEXT_BYTES`].
    pub client: String,
    /// How far the session has come.
    pub state: SessionState,
    /// The client's own identifier for the session, used to resume it. It
    /// proves nothing to the daemon. At most [`MAX_LOCAL_TEXT_BYTES`].
    pub client_session: Option<String>,
    /// What the client and adapter support in this session.
    pub capabilities: SessionCapabilities,
    /// Opaque to the daemon: encoded and owned by `locust-adapter`. At most
    /// [`MAX_SESSION_DETAIL_BYTES`].
    #[serde(with = "codec::bytes")]
    #[schemars(with = "Vec<u8>")]
    pub detail: Vec<u8>,
}

impl SessionRecord {
    /// Refuses a record whose strings or detail exceed their limits.
    pub fn check(&self) -> Result<(), ApiError> {
        let strings_fit = self.client.len() <= MAX_LOCAL_TEXT_BYTES
            && self
                .client_session
                .as_ref()
                .is_none_or(|session| session.len() <= MAX_LOCAL_TEXT_BYTES);
        if strings_fit && self.detail.len() <= MAX_SESSION_DETAIL_BYTES {
            Ok(())
        } else {
            Err(ApiError::new(
                ErrorCode::LimitExceeded,
                "session record is too large",
            ))
        }
    }
}

/// One session as the daemon holds it: its record, and what the daemon
/// itself knows about it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionView {
    /// The session's public handle; see [`SessionSecret::instance`].
    pub instance: InstanceId,
    /// The principal the session belongs to.
    pub principal: PublicKey,
    /// What the session's adapter last reported.
    pub record: SessionRecord,
    /// When the record was last written, Unix milliseconds by the daemon's
    /// clock.
    pub updated_ms: u64,
    /// True while a connection that presented this session's secret is open.
    pub attached: bool,
    /// Claims the session holds on unfinished attempts, in any goal.
    pub claims: Vec<Claim>,
}

/// Stable error categories. Clients branch on the code, never on the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The request is not the caller's to make: the credential is unknown or
    /// revoked, the caller lacks the authority for the requested goal act,
    /// or the host refused an invitation.
    Denied,
    /// The named thing does not exist, or is in a goal the caller is not
    /// part of.
    NotFound,
    /// The request is malformed, or needs something the connection lacks: a
    /// session for a claim-bound request, a principal for the owner.
    Invalid,
    /// A precondition no longer holds, for example a stale base, a task
    /// already reassigned or a name already taken.
    Conflict,
    /// Another session holds the claim.
    ClaimHeld,
    /// The caller's claim, generation, attempt or attempt is not current.
    Superseded,
    /// The idempotency key was already used for a different request.
    IdempotencyMismatch,
    /// The request exceeds a published or configured limit.
    LimitExceeded,
    /// Needed content or a needed peer is not reachable now.
    Unavailable,
    /// The goal's authority history conflicts, so nothing more is signed for
    /// it; see [`Halt::AuthorityConflict`].
    Halted,
    /// The client, a ticket or an event uses a version this daemon does not
    /// speak.
    UnsupportedVersion,
    /// Local state failed an integrity check.
    Corrupted,
    /// The daemon failed: storage, or a fault of its own. Not the caller's
    /// doing, and retrying the same request may succeed.
    Internal,
    /// The action needs a higher local level or a task allowance.
    LevelRequired,
    /// The shared goal rules do not qualify this principal for the action.
    NotEligible,
    /// This daemon's signer for the goal is in restore recovery and signs
    /// nothing; see [`Halt::SignerRecovery`]. Reads still work.
    ReadOnly,
}

impl ErrorCode {
    /// Stable name for JSON output and logs; equal to the JSON form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Denied => "denied",
            Self::NotFound => "not_found",
            Self::Invalid => "invalid",
            Self::Conflict => "conflict",
            Self::ClaimHeld => "claim_held",
            Self::Superseded => "superseded",
            Self::IdempotencyMismatch => "idempotency_mismatch",
            Self::LimitExceeded => "limit_exceeded",
            Self::Unavailable => "unavailable",
            Self::Halted => "halted",
            Self::UnsupportedVersion => "unsupported_version",
            Self::Corrupted => "corrupted",
            Self::Internal => "internal",
            Self::LevelRequired => "level_required",
            Self::NotEligible => "not_eligible",
            Self::ReadOnly => "read_only",
        }
    }
}

/// Why a request was refused. `Display` is `<code>: <message>`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ApiError {
    /// What clients branch on.
    pub code: ErrorCode,
    /// For people. Written by the daemon; never contains peer-written text.
    pub message: String,
    /// Structured correction details encoded as JSON for wire-safe transport.
    pub details_json: Option<String>,
}

impl ApiError {
    /// An error with a message written by the daemon or this crate, never
    /// copied from a peer.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details_json: None,
        }
    }
}

impl ApiError {
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details_json = Some(details.to_string());
        self
    }

    /// The structured first refusal, when this error is an action refusal.
    pub fn refused(&self) -> Option<Refused> {
        serde_json::from_str(self.details_json.as_deref()?).ok()
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for ApiError {}

/// A failed medium is the daemon's problem (`Internal`); stored data that
/// fails an integrity check is `Corrupted`.
impl From<StoreError> for ApiError {
    fn from(error: StoreError) -> Self {
        let code = match &error {
            StoreError::Failed(_) => ErrorCode::Internal,
            StoreError::Corrupted(_) => ErrorCode::Corrupted,
        };
        Self::new(code, error.to_string())
    }
}

/// Another protocol version is `UnsupportedVersion`; a stored event that no
/// longer matches its identifier is `Corrupted`; any other structural fault
/// is `Invalid`.
impl From<EventError> for ApiError {
    fn from(error: EventError) -> Self {
        let code = match error {
            EventError::UnsupportedVersion(_) => ErrorCode::UnsupportedVersion,
            EventError::IdMismatch => ErrorCode::Corrupted,
            _ => ErrorCode::Invalid,
        };
        Self::new(code, error.to_string())
    }
}

/// Every manifest fault is `Invalid`.
impl From<ManifestError> for ApiError {
    fn from(error: ManifestError) -> Self {
        Self::new(ErrorCode::Invalid, error.to_string())
    }
}

/// Another protocol version is `UnsupportedVersion`; any other fault in a
/// ticket is `Invalid`.
impl From<InviteError> for ApiError {
    fn from(error: InviteError) -> Self {
        let code = match error {
            InviteError::UnsupportedVersion(_) => ErrorCode::UnsupportedVersion,
            InviteError::InvalidSignature => ErrorCode::Denied,
            InviteError::NotATicket
            | InviteError::TooLong
            | InviteError::Malformed
            | InviteError::BadName
            | InviteError::BadHints => ErrorCode::Invalid,
        };
        Self::new(code, error.to_string())
    }
}

/// Bytes that do not decode are `Invalid`. A value that cannot be encoded is
/// never the client's doing, so it is `Internal`.
impl From<CodecError> for ApiError {
    fn from(error: CodecError) -> Self {
        let code = match error {
            CodecError::Encode => ErrorCode::Internal,
            CodecError::Decode | CodecError::TrailingBytes | CodecError::NotCanonical => {
                ErrorCode::Invalid
            }
        };
        Self::new(code, error.to_string())
    }
}

/// One operation's fields and reachable definitions, extracted from the
/// authoritative request schema. Unreferenced definitions are omitted.
pub fn operation_schema(name: &str) -> Option<serde_json::Value> {
    schema::operation(name).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn round_trip<T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(
        value: &T,
    ) {
        assert_eq!(
            codec::decode_canonical::<T>(&codec::encode(value).unwrap()).unwrap(),
            *value
        );
    }
    #[test]
    fn credentials_and_sessions_round_trip_only_binary_and_redact_text() {
        let credential = Credential([42; 32]);
        let session = SessionSecret([7; 32]);
        round_trip(&credential);
        round_trip(&session);
        assert_eq!(
            serde_json::to_string(&credential).unwrap(),
            "\"<redacted>\""
        );
        assert!(serde_json::from_str::<Credential>("\"<redacted>\"").is_err());
        assert_ne!(credential.digest(), Credential([43; 32]).digest());
        assert_ne!(session.instance(), SessionSecret([8; 32]).instance());
    }
    #[test]
    fn current_handshake_and_frames_round_trip() {
        let hello = ClientHello {
            api_version: API_VERSION,
            credential: Credential([1; 32]),
            session: Some(SessionSecret([2; 32])),
        };
        round_trip(&hello);
        let frame = RequestFrame {
            id: 4,
            idempotency: Some(IdempotencyKey([3; 16])),
            on_behalf: None,
            request: Request::GoalCreate {
                agent: PublicKey([4; 32]),
                title: "open".into(),
                formation_json: None,
                name: "member".into(),
                inputs: BTreeMap::new(),
            },
        };
        round_trip(&frame);
        let error = ApiError::new(ErrorCode::Conflict, "stale source")
            .with_details(serde_json::json!({"revision":2,"source":"preserved"}));
        round_trip(&ResponseFrame {
            id: 4,
            result: Err(error.clone()),
        });
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(error.details_json.as_ref().unwrap())
                .unwrap()["source"],
            "preserved"
        );
    }
    #[test]
    fn unsupported_handshake_is_refused_before_decoding_its_body() {
        for version in (0..API_VERSION).chain([API_VERSION + 1, u16::MAX]) {
            let bytes = codec::encode(&version).unwrap();
            assert_eq!(
                ClientHello::decode(&bytes).unwrap_err().code,
                ErrorCode::UnsupportedVersion
            );
        }
        assert_eq!(
            ClientHello::decode(&[]).unwrap_err().code,
            ErrorCode::Invalid
        );
    }
    #[test]
    fn registry_and_generated_field_schema_are_exhaustive() {
        for operation in OPERATIONS {
            let input = operation_schema(operation.name).unwrap();
            assert_eq!(input["type"], "object", "{}", operation.name);
            assert_eq!(input["additionalProperties"], false);
        }
    }
    #[test]
    fn the_contributions_summary_keeps_attempts_independent() {
        // The skill asks a member to post its result on a task before reading
        // other members' results on it; the tool's own help must not undo that.
        let summary = Request::Contributions {
            goal: GoalId([1; 32]),
            task: None,
        }
        .operation()
        .summary;
        assert!(!summary.contains("before publishing"), "{summary}");
        assert!(
            summary.contains(
                "on a task you are attempting, publish your result before reading other members' results on it"
            ),
            "{summary}"
        );
    }
    #[test]
    fn claim_bound_publish_requires_matching_generation_shape() {
        let mut request = Request::ContributionPublish {
            goal: GoalId([1; 32]),
            attempt: Some(EventId([2; 32])),
            generation: None,
            summary: "finding".into(),
            sources: Vec::new(),
            artifacts: vec![],
        };
        assert_eq!(request.check().unwrap_err().code, ErrorCode::Invalid);
        if let Request::ContributionPublish { attempt, .. } = &mut request {
            *attempt = None;
        }
        assert!(request.check().is_ok());
        assert!(request.is_answered_by(&Response::Recorded {
            event: EventId([3; 32])
        }));
        assert!(!request.is_answered_by(&Response::Done));
    }
    #[test]
    fn obsolete_operations_and_invented_fields_are_rejected() {
        for name in [
            "task.assign",
            "task.claim",
            "task.submit",
            "result.accept",
            "note.add",
            "doc.accept",
        ] {
            assert!(serde_json::from_value::<Request>(serde_json::json!({name:{}})).is_err());
        }
        assert!(
            serde_json::from_value::<Request>(
                serde_json::json!({"goal.status":{"goal":GoalId([1;32]),"invented":true}})
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod names_and_roles_tests {
    use super::*;
    #[test]
    fn a_request_with_an_unusable_name_or_role_is_invalid() {
        let agent = PublicKey([1; 32]);
        let goal = GoalId([2; 32]);
        for name in ["", " padded", "line\nbreak"] {
            assert_eq!(
                Request::GoalCreate {
                    agent,
                    name: name.into(),
                    title: "Goal".into(),
                    formation_json: None,
                    inputs: BTreeMap::new()
                }
                .check()
                .unwrap_err()
                .code,
                ErrorCode::Invalid
            );
            assert_eq!(
                Request::GoalJoin {
                    agent,
                    name: name.into(),
                    ticket: Ticket("unused".into()),
                    level: Level::Auto
                }
                .check()
                .unwrap_err()
                .code,
                ErrorCode::Invalid
            );
        }
        for role in ["", "  ", "a\nb"] {
            assert_eq!(
                Request::GoalInvite {
                    goal,
                    expires_ms: 1,
                    role: Some(role.into())
                }
                .check()
                .unwrap_err()
                .code,
                ErrorCode::Invalid
            );
        }
        assert_eq!(
            Request::RoleGive {
                goal,
                member: agent,
                role: "lead".into(),
                expected: vec![PublicKey([2; 32]), agent]
            }
            .check()
            .unwrap_err()
            .code,
            ErrorCode::Invalid
        );
    }
}
