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
//! the owner, one enrolled principal, or a viewer of one principal. Every
//! goal-scoped request names its goal. There is no current or default goal,
//! and a missing credential never selects a wider scope. A principal sees
//! only the goals it is part of; to it, any other goal does not exist
//! ([`ErrorCode::NotFound`]).
//!
//! A viewer reads exactly what its principal reads and makes no other
//! request ([`Caller::Viewer`]). Wherever this module speaks of the calling
//! principal, a viewer's is the principal it was enrolled for.
//!
//! The owner authors no events. Directly it may make the owner-only requests,
//! every read-only request and `session.drop`. Any other request acts as a
//! principal, so the owner names one in [`RequestFrame::on_behalf`]; that
//! direct act is the authorization, and the principal's grants are not
//! consulted.
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
//! # Authorization
//!
//! [`Grants`] are daemon-wide and [`GoalGrants`] are per goal; together they
//! are what the local participant lets a principal do without asking again.
//! A request that is the caller's to make but that no grant covers fails with
//! [`ErrorCode::AuthorizationRequired`] and waits for the owner. A request
//! that is not the caller's to make at all is [`ErrorCode::Denied`].
//!
//! # Pending work and waiting
//!
//! What needs a caller is computed from task state and local records each
//! time it is asked ([`Request::Pending`]); it never depends on a
//! notification having been delivered. Every goal has a change counter, its
//! revision, raised in the same commit as any change to the goal's events or
//! to a local record that feeds pending work. [`Request::Wait`] answers as
//! soon as the revision differs from the one the caller has seen, so a change
//! between two calls is never missed.
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
pub mod invitations;
pub mod permissions;
pub use context::*;
pub use invitations::*;
pub use permissions::*;

use crate::organization::catalog::{Draft, Presentation, Publication};
use schemars::{JsonSchema, schema_for};
use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};

use crate::API_VERSION;
use crate::codec::{self, CodecError};
use crate::crypto::{self, domain};
use crate::event::{
    AttemptStatus, Body, CancelOutcome, Context, Doc, EventError, PayloadRef, ReviewVerdict, Scope,
    TaskId,
};
use crate::id::{
    BlobHash, EffectId, EndpointId, EventId, GoalId, IdempotencyKey, InstanceId, PublicKey,
};
use crate::invite::{InviteError, Ticket};
use crate::limits::{MAX_ARTIFACTS, MAX_SESSION_DETAIL_BYTES};
use crate::manifest::ManifestError;
use crate::store::StoreError;

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
/// principal, or one viewer. It scopes what a client may ask for and
/// attributes its requests; it is not protection against another process
/// that can read the same user's files.
///
/// Whoever generates a credential writes it to its file first (see
/// [`crate::local`]); a principal's or a viewer's is generated by the client
/// that enrolls it. The daemon stores only [`Credential::digest`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Credential(pub [u8; 32]);

impl Credential {
    /// What the daemon stores and looks up in place of the secret, and what
    /// [`Request::AgentEnroll`] and [`Request::ViewerEnroll`] carry.
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
    /// connection can read and coordinate but cannot hold a claim. A viewer's
    /// hello never carries one.
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
    /// The local participant: enrolls principals, sets their grants and
    /// authorizes what no grant covers. Authors no events itself.
    Owner,
    /// An enrolled principal acting within its grants.
    Agent(PublicKey),
    /// A viewer of the enrolled principal it names: a read-only credential
    /// the owner enrolled with [`Request::ViewerEnroll`]. It sees exactly
    /// what that principal sees and changes nothing:
    ///
    /// - It may make every read-only operation ([`Operation::read_only`]),
    ///   answered as the principal would be answered. Every other request is
    ///   [`ErrorCode::Denied`], among them the session operations that
    ///   write, `session.report` and `session.drop`.
    /// - It never names [`RequestFrame::on_behalf`] (`Denied`).
    /// - It is never an execution session. A hello that presents a viewer's
    ///   credential with a session is refused as [`ErrorCode::Invalid`], so a
    ///   viewer holds no claim and `pending` lists no `claimed` work for it.
    /// - Its reads store nothing. `events` reads after the position it passes,
    ///   or from the start without one. Context acknowledgment is unavailable. Its `blob.get` of an object not
    ///   held is `Unavailable` without noting a want. Its idempotency keys
    ///   are not recorded, so a repeated read runs again.
    ///
    /// Revoking the principal revokes its viewers.
    Viewer(PublicKey),
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
    /// a viewer's credential presented with a session. The layout of
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

/// What the local participant has authorized a principal to do on this
/// daemon without asking again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Grants {
    /// Create goals, issue invitations, join and leave.
    pub manage_goals: bool,
}

/// Explicit local authorization; shared formation eligibility never grants it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GoalGrants {
    pub administer: bool,
    pub contribute: bool,
    pub execute: bool,
    pub review: bool,
    pub select: bool,
    pub flow: bool,
    pub takeover: bool,
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
    /// owner's direct act is the authorization, so the principal's grants are
    /// not consulted. From any other caller this is `Denied`; an unknown or
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

/// Current greenfield API. Formation rules and local grants are separate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub enum Request {
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "daemon.stop")]
    Shutdown,
    #[serde(rename = "agent.enroll")]
    AgentEnroll {
        name: String,
        grants: Grants,
        credential: [u8; 32],
    },
    #[serde(rename = "author.enroll")]
    AuthorEnroll { name: String, credential: [u8; 32] },
    #[serde(rename = "agent.grant")]
    AgentGrant { agent: PublicKey, grants: Grants },
    #[serde(rename = "agent.revoke")]
    AgentRevoke { agent: PublicKey },
    #[serde(rename = "viewer.enroll")]
    ViewerEnroll {
        agent: PublicKey,
        credential: [u8; 32],
    },
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
        title: String,
        formation_json: Option<String>,
        roles: BTreeMap<String, Vec<PublicKey>>,
        inputs: BTreeMap<String, BlobHash>,
    },
    #[serde(rename = "goal.join")]
    GoalJoin { ticket: Ticket },
    #[serde(rename = "goal.invite")]
    GoalInvite {
        goal: GoalId,
        expires_ms: Option<u64>,
    },
    #[serde(rename = "goal.leave")]
    GoalLeave { goal: GoalId },
    #[serde(rename = "goal.grant")]
    GoalGrant {
        goal: GoalId,
        agent: PublicKey,
        grants: GoalGrants,
    },
    #[serde(rename = "goal.status")]
    GoalStatus { goal: GoalId },
    #[serde(rename = "member.remove")]
    MemberRemove { goal: GoalId, member: PublicKey },
    #[serde(rename = "rules.bind")]
    RulesBind {
        goal: GoalId,
        expected: EventId,
        formation_json: String,
        roles: BTreeMap<String, Vec<PublicKey>>,
        inputs: BTreeMap<String, BlobHash>,
    },
    #[serde(rename = "workspace.set")]
    WorkspaceSet {
        goal: GoalId,
        binding: WorkspaceBinding,
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
    #[serde(rename = "task.authorize")]
    TaskAuthorize {
        goal: GoalId,
        task: TaskId,
        agent: PublicKey,
        takeover: bool,
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
        task: Option<TaskId>,
        attempt: Option<EventId>,
        generation: Option<u32>,
        summary: String,
        base: Option<BlobHash>,
        patch: Option<BlobHash>,
        artifacts: Vec<BlobHash>,
    },
    #[serde(rename = "contributions")]
    Contributions { goal: GoalId, task: Option<TaskId> },
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
        task: Option<TaskId>,
        after: Option<ContextCursor>,
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
    InvitationRevoke { goal: GoalId, invitation: String },
    #[serde(rename = "invitation.join")]
    InvitationJoin {
        principal: PublicKey,
        ticket: Ticket,
        review: String,
    },
    #[serde(rename = "permission.inspect")]
    Permissions { goal: GoalId, agent: PublicKey },
    #[serde(rename = "permission.allow")]
    PermissionAllow {
        goal: GoalId,
        agent: PublicKey,
        permissions: Vec<GoalPermission>,
    },
    #[serde(rename = "permission.task.allow")]
    PermissionTaskAllow {
        goal: GoalId,
        agent: PublicKey,
        task: TaskId,
        takeover: bool,
    },
    #[serde(rename = "permission.task.revoke")]
    PermissionTaskRevoke {
        goal: GoalId,
        agent: PublicKey,
        task: TaskId,
    },
    #[serde(rename = "permission.revoke")]
    PermissionRevoke {
        goal: GoalId,
        agent: PublicKey,
        permissions: Vec<GoalPermission>,
    },
    #[serde(rename = "inbox")]
    Inbox,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Audience {
    /// The owner credential only, never on behalf of a principal.
    Owner,
    /// Any enrolled principal within its grants, or the owner (on behalf of a
    /// principal unless the request is read-only).
    Agent,
    /// The administrator of the goal, or the owner on its behalf.
    Administrator,
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
    Status => ("status", true, false, Agent, true, "status"),
    Shutdown => ("daemon.stop", false, false, Owner, false, "daemon stop"),
    AgentEnroll { .. } => ("agent.enroll", false, false, Owner, false, "agent enroll"),
    AuthorEnroll { .. } => ("author.enroll", false, false, Owner, false, "author enroll"),
    AgentGrant { .. } => ("agent.grant", false, false, Owner, false, "agent grant"),
    AgentRevoke { .. } => ("agent.revoke", false, false, Owner, false, "agent revoke"),
    ViewerEnroll { .. } => ("viewer.enroll", false, false, Owner, false, "viewer enroll"),
    SessionReport { .. } => ("session.report", false, false, Agent, false, "session report"),
    Session { .. } => ("session.show", true, false, Agent, false, "session show"),
    Sessions => ("sessions", true, false, Agent, false, "sessions"),
    SessionDrop { .. } => ("session.drop", false, false, Agent, false, "session drop"),
    GoalCreate { .. } => ("goal.create", false, false, Agent, true, "goal create"),
    GoalJoin { .. } => ("goal.join", false, false, Agent, false, "goal join"),
    GoalInvite { .. } => ("goal.invite", false, true, Administrator, false, "goal invite"),
    GoalLeave { .. } => ("goal.leave", false, true, Agent, true, "goal leave"),
    GoalGrant { .. } => ("goal.grant", false, true, Owner, false, "goal grant"),
    GoalStatus { .. } => ("goal.status", true, true, Agent, true, "goal status"),
    MemberRemove { .. } => ("member.remove", false, true, Administrator, true, "member remove"),
    RulesBind { .. } => ("rules.bind", false, true, Administrator, true, "rules bind"),
    WorkspaceSet { .. } => ("workspace.set", false, true, Agent, true, "workspace set"),
    Board { .. } => ("board", true, true, Agent, true, "board"),
    Task { .. } => ("task.show", true, true, Agent, true, "task show"),
    Event { .. } => ("event.show", true, true, Agent, true, "event show"),
    TaskOpen { .. } => ("task.open", false, true, Agent, true, "task open"),
    TaskRevise { .. } => ("task.revise", false, true, Administrator, true, "task revise"),
    WorkOffer { .. } => ("work.offer", false, true, Agent, true, "work offer"),
    TaskAuthorize { .. } => ("task.authorize", false, true, Owner, false, "task authorize"),
    AttemptStart { .. } => ("attempt.start", false, true, Agent, true, "attempt start"),
    AttemptTakeover { .. } => ("attempt.takeover", false, true, Agent, true, "attempt takeover"),
    WorkDecline { .. } => ("work.decline", false, true, Agent, true, "work decline"),
    AttemptCancel { .. } => ("attempt.cancel", false, true, Agent, true, "attempt cancel"),
    AttemptReport { .. } => ("attempt.report", false, true, Agent, true, "attempt report"),
    ContributionPublish { .. } => ("contribution.publish", false, true, Agent, true, "contribution publish"),
    Contributions { .. } => ("contributions", true, true, Agent, true, "contributions"),
    CompletionDeclare { .. } => ("completion.declare", false, true, Agent, true, "completion declare"),
    ReviewRecord { .. } => ("review.record", false, true, Agent, true, "review record"),
    CheckAttest { .. } => ("check.attest", false, true, Agent, true, "check attest"),
    ScopeSelect { .. } => ("scope.select", false, true, Agent, true, "scope select"),
    ScopeClose { .. } => ("scope.close", false, true, Agent, true, "scope close"),
    ScopeReopen { .. } => ("scope.reopen", false, true, Agent, true, "scope reopen"),
    DeliveryAcknowledge { .. } => ("delivery.acknowledge", false, true, Agent, true, "delivery acknowledge"),
    CancelAcknowledge { .. } => ("cancel.acknowledge", false, true, Agent, true, "cancel acknowledge"),
    Pending { .. } => ("pending", true, true, Agent, true, "pending"),
    Wait { .. } => ("wait", true, true, Agent, true, "wait"),
    Events { .. } => ("events", true, true, Agent, true, "events"),
    DocRead { .. } => ("doc.read", true, true, Agent, true, "doc read"),
    DocRevise { .. } => ("doc.revise", false, true, Agent, true, "doc revise"),
    BlobPut { .. } => ("blob.put", false, true, Agent, false, "blob put"),
    BlobGet { .. } => ("blob.get", true, true, Agent, false, "blob get"),
    BlobStat { .. } => ("blob.stat", true, true, Agent, true, "blob stat"),
    BlobWithdraw { .. } => ("blob.withdraw", false, true, Agent, true, "blob withdraw"),
    FormationDraftCreate { .. } => ("formation.draft.create", false, false, Author, true, "formation draft create"),
    FormationDraftUpdate { .. } => ("formation.draft.update", false, false, Author, true, "formation draft update"),
    FormationDraft { .. } => ("formation.draft.show", true, false, Author, true, "formation draft show"),
    FormationDrafts => ("formation.drafts", true, false, Author, true, "formation drafts"),
    FormationPublish { .. } => ("formation.publish", false, false, Author, true, "formation publish"),
    FormationPublication { .. } => ("formation.show", true, false, Author, true, "formation show"),
    FormationPublications => ("formation.list", true, false, Author, true, "formation list"),
    FormationPresentation { .. } => ("formation.presentation.show", true, false, Author, true, "formation presentation show"),
    FormationPresentationUpdate { .. } => ("formation.presentation.update", false, false, Author, true, "formation presentation update"),
    FormationValidate { .. } => ("formation.validate", true, false, Author, true, "formation validate"),
    FormationExplain { .. } => ("formation.explain", true, false, Author, true, "formation explain"),
    Context { .. } => ("context.read", true, true, Agent, true, "Read a coherent goal or task brief with attributed findings, progress, review reasons and pending actions; reads do not acknowledge content"),
    ContextAcknowledge { .. } => ("context.acknowledge", false, true, Agent, true, "Acknowledge the complete content delivered to this execution session using its exact receipt; other sessions remain unread"),
    InvitationInspect { .. } => ("invitation.inspect", true, false, Agent, false, "Verify a signed invitation and preview its issuer, goal and sharing boundary without joining"),
    GoalInvitations { .. } => ("invitation.list", true, true, Administrator, false, "List issued invitations and their current redemption, expiry or revocation state without revealing ticket secrets"),
    InvitationRevoke { .. } => ("invitation.revoke", false, true, Owner, false, "Revoke an issued invitation; existing membership remains a separate decision"),
    InvitationJoin { .. } => ("invitation.join", false, false, Owner, false, "Join as the selected enrolled principal after reviewing this exact signed invitation; grants remain unchanged"),
    Permissions { .. } => ("permission.inspect", true, true, Owner, false, "Inspect an enrolled principal's explicit local permissions and membership"),
    PermissionAllow { .. } => ("permission.allow", false, true, Owner, false, "Allow only the selected local goal permissions for an enrolled principal"),
    PermissionTaskAllow { .. } => ("permission.task.allow", false, true, Owner, false, "Allow local execution for this task, preserving existing takeover permission"),
    PermissionTaskRevoke { .. } => ("permission.task.revoke", false, true, Owner, false, "Remove every local execution authorization for this agent and task; running processes are not terminated"),
    PermissionRevoke { .. } => ("permission.revoke", false, true, Owner, false, "Revoke only the selected local goal permissions; this does not terminate a running process"),
    Inbox => ("inbox", true, false, Owner, false, "Show local participants and work needing permission, action or review across goals without acknowledging it"),
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
            Self::Status => None,
            Self::Shutdown => None,
            Self::AgentEnroll { .. } => None,
            Self::AuthorEnroll { .. } => None,
            Self::AgentGrant { .. } => None,
            Self::AgentRevoke { .. } => None,
            Self::ViewerEnroll { .. } => None,
            Self::SessionReport { .. } => None,
            Self::Session { .. } => None,
            Self::Sessions => None,
            Self::SessionDrop { .. } => None,
            Self::GoalCreate { .. } => None,
            Self::GoalJoin { .. } => None,
            Self::GoalInvite { goal, .. } => Some(*goal),
            Self::GoalLeave { goal, .. } => Some(*goal),
            Self::GoalGrant { goal, .. } => Some(*goal),
            Self::GoalStatus { goal, .. } => Some(*goal),
            Self::MemberRemove { goal, .. } => Some(*goal),
            Self::RulesBind { goal, .. } => Some(*goal),
            Self::WorkspaceSet { goal, .. } => Some(*goal),
            Self::Board { goal, .. } => Some(*goal),
            Self::Task { goal, .. } => Some(*goal),
            Self::Event { goal, .. } => Some(*goal),
            Self::TaskOpen { goal, .. } => Some(*goal),
            Self::TaskRevise { goal, .. } => Some(*goal),
            Self::WorkOffer { goal, .. } => Some(*goal),
            Self::TaskAuthorize { goal, .. } => Some(*goal),
            Self::AttemptStart { goal, .. } => Some(*goal),
            Self::AttemptTakeover { goal, .. } => Some(*goal),
            Self::WorkDecline { goal, .. } => Some(*goal),
            Self::AttemptCancel { goal, .. } => Some(*goal),
            Self::AttemptReport { goal, .. } => Some(*goal),
            Self::ContributionPublish { goal, .. } => Some(*goal),
            Self::Contributions { goal, .. } => Some(*goal),
            Self::CompletionDeclare { goal, .. } => Some(*goal),
            Self::ReviewRecord { goal, .. } => Some(*goal),
            Self::CheckAttest { goal, .. } => Some(*goal),
            Self::ScopeSelect { goal, .. } => Some(*goal),
            Self::ScopeClose { goal, .. } => Some(*goal),
            Self::ScopeReopen { goal, .. } => Some(*goal),
            Self::DeliveryAcknowledge { goal, .. } => Some(*goal),
            Self::CancelAcknowledge { goal, .. } => Some(*goal),
            Self::Pending { goal, .. } => Some(*goal),
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
            | Self::InvitationRevoke { goal, .. }
            | Self::Permissions { goal, .. }
            | Self::PermissionAllow { goal, .. }
            | Self::PermissionRevoke { goal, .. }
            | Self::PermissionTaskRevoke { goal, .. }
            | Self::PermissionTaskAllow { goal, .. } => Some(*goal),
            Self::InvitationInspect { .. } | Self::InvitationJoin { .. } | Self::Inbox => None,
        }
    }
    pub fn check(&self) -> Result<(), ApiError> {
        match self {
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
            Self::WorkspaceSet { binding, .. } => binding.check(),
            _ => Ok(()),
        }
    }
    pub fn is_answered_by(&self, response: &Response) -> bool {
        match self {
            Self::Status => matches!(response, Response::Status(_)),
            Self::Context { .. } => matches!(response, Response::Context(_)),
            Self::ContextAcknowledge { .. } => matches!(response, Response::ContextAcknowledged(_)),
            Self::InvitationInspect { .. } => {
                matches!(response, Response::InvitationInspected { .. })
            }
            Self::GoalInvitations { .. } => matches!(response, Response::Invitations { .. }),
            Self::InvitationRevoke { .. } => matches!(response, Response::InvitationRevoked { .. }),
            Self::InvitationJoin { .. } => matches!(response, Response::Joined { .. }),
            Self::Permissions { .. }
            | Self::PermissionAllow { .. }
            | Self::PermissionRevoke { .. }
            | Self::PermissionTaskRevoke { .. }
            | Self::PermissionTaskAllow { .. } => matches!(response, Response::Permissions(_)),
            Self::Inbox => matches!(response, Response::Inbox(_)),

            Self::Shutdown => matches!(response, Response::Done),
            Self::AgentEnroll { .. } => matches!(response, Response::AgentEnrolled { .. }),
            Self::AuthorEnroll { .. } => matches!(response, Response::AuthorEnrolled { .. }),
            Self::AgentGrant { .. } => matches!(response, Response::Done),
            Self::AgentRevoke { .. } => matches!(response, Response::Done),
            Self::ViewerEnroll { .. } => matches!(response, Response::Done),
            Self::SessionReport { .. } => matches!(response, Response::Done),
            Self::Session { .. } => matches!(response, Response::Session(_)),
            Self::Sessions => matches!(response, Response::Sessions(_)),
            Self::SessionDrop { .. } => matches!(response, Response::Done),
            Self::GoalCreate { .. } => matches!(response, Response::GoalCreated { .. }),
            Self::GoalJoin { .. } => matches!(response, Response::Joined { .. }),
            Self::GoalInvite { .. } => matches!(response, Response::Invited { .. }),
            Self::GoalLeave { .. } => matches!(response, Response::Recorded { .. }),
            Self::GoalGrant { .. } => matches!(response, Response::Done),
            Self::GoalStatus { .. } => matches!(response, Response::GoalStatus(_)),
            Self::MemberRemove { .. } => matches!(response, Response::Recorded { .. }),
            Self::RulesBind { .. } => matches!(response, Response::Recorded { .. }),
            Self::WorkspaceSet { .. } => matches!(response, Response::Done),
            Self::Board { .. } => matches!(response, Response::Board(_)),
            Self::Task { .. } => matches!(response, Response::Task(_)),
            Self::Event { .. } => matches!(response, Response::Event(_)),
            Self::TaskOpen { .. } => matches!(response, Response::Recorded { .. }),
            Self::TaskRevise { .. } => matches!(response, Response::Recorded { .. }),
            Self::WorkOffer { .. } => matches!(response, Response::Recorded { .. }),
            Self::TaskAuthorize { .. } => matches!(response, Response::Done),
            Self::AttemptStart { .. } => matches!(response, Response::Claimed(_)),
            Self::AttemptTakeover { .. } => matches!(response, Response::Claimed(_)),
            Self::WorkDecline { .. } => matches!(response, Response::Recorded { .. }),
            Self::AttemptCancel { .. } => matches!(response, Response::Recorded { .. }),
            Self::AttemptReport { .. } => matches!(response, Response::Recorded { .. }),
            Self::ContributionPublish { .. } => matches!(response, Response::Recorded { .. }),
            Self::Contributions { .. } => matches!(response, Response::Contributions(_)),
            Self::CompletionDeclare { .. } => matches!(response, Response::Recorded { .. }),
            Self::ReviewRecord { .. } => matches!(response, Response::Recorded { .. }),
            Self::CheckAttest { .. } => matches!(response, Response::Recorded { .. }),
            Self::ScopeSelect { .. } => matches!(response, Response::Recorded { .. }),
            Self::ScopeClose { .. } => matches!(response, Response::Recorded { .. }),
            Self::ScopeReopen { .. } => matches!(response, Response::Recorded { .. }),
            Self::DeliveryAcknowledge { .. } => matches!(response, Response::Recorded { .. }),
            Self::CancelAcknowledge { .. } => matches!(response, Response::Recorded { .. }),
            Self::Pending { .. } => matches!(response, Response::Pending(_)),
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
/// Generated full request schema. MCP consumers select the matching operation
/// branch and retain root definitions rather than hand-copying field schemas.
pub fn request_schema() -> serde_json::Value {
    serde_json::to_value(schema_for!(Request)).expect("request schema serializes")
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
                Audience::Administrator => "administrator",
                Audience::Author => "author",
            },
            "mcp_tool": operation.tool.then(|| operation.tool_name()),
        })).collect::<Vec<_>>(),
        "request_schema": request_schema(),
        "response_schema": schema_for!(Response),
        "event_schema": schema_for!(crate::event::Body),
        "error_schema": schema_for!(ApiError),
    })
}
/// What a request answers with when it succeeds; which variant answers which
/// request is [`Request::is_answered_by`]. No response ever carries a secret.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Response {
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
    /// `Joining` until the administrator's admission has arrived.
    Joined {
        /// The goal the ticket named.
        goal: GoalId,
        /// The administrator's key, to show as a fingerprint.
        administrator: PublicKey,
        /// `Joining` or `Member`.
        membership: Membership,
    },
    /// Answers `goal.status`.
    GoalStatus(GoalStatus),
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
    /// Answers `wait`.
    Waited(WaitOutcome),
    /// Answers `events`: feed entries in ascending position.
    Events(Vec<EventView>),
    /// Read attributed contributions, including findings without tasks.
    Contributions(Vec<ContributionView>),
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
    Permissions(GoalPermissions),
    Inbox(Vec<AttentionEntry>),
}

/// What `status` shows: the daemon, and as much of its principals and goals
/// as the caller may see.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DaemonStatus {
    /// The daemon's own version, for people.
    pub daemon_version: String,
    /// Absent until the transport is running.
    pub endpoint: Option<EndpointId>,
    /// Every enrolled principal for the owner; only the calling principal
    /// for an agent or a viewer.
    pub agents: Vec<AgentView>,
    /// One entry per goal and local principal in it: all of them for the
    /// owner, only the calling principal's own for an agent or a viewer.
    pub goals: Vec<GoalSummary>,
}

/// One enrolled principal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgentView {
    /// The principal's key; the daemon holds the signing half.
    pub agent: PublicKey,
    /// Unique on this daemon; satisfies [`is_agent_name`].
    pub name: String,
    /// Its daemon-wide grants. Per-goal grants are in [`GoalStatus`].
    pub grants: Grants,
    /// True once the credential was revoked. The name stays taken.
    pub revoked: bool,
}

/// How a local principal stands in a goal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Membership {
    /// An invitation was redeemed here; the administrator's admission has not
    /// arrived yet.
    Joining,
    /// Admitted and taking part.
    Member,
    /// The administrator removed the principal. History stays readable;
    /// nothing new is authored.
    Removed,
    /// The principal asked to leave and no longer takes part.
    Left,
    /// The administrator refused this local join attempt. No goal content is
    /// readable until a subsequent invitation is admitted.
    Refused,
}

/// One local principal's part in one goal, as `status` lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GoalSummary {
    /// The goal this entry is about.
    pub goal: GoalId,
    /// Absent until the goal's founding text is held locally.
    pub title: Option<String>,
    /// The local principal this entry is about.
    pub member: PublicKey,
    /// How that principal stands in the goal.
    pub membership: Membership,
    /// Set while the goal's decisions cannot advance here.
    pub halted: Option<Halt>,
}

/// Why a goal's decisions cannot advance on this daemon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Halt {
    /// The authority's history conflicts with itself. The goal stays
    /// readable and exportable; requests that sign fail with
    /// [`ErrorCode::Halted`].
    AuthorityConflict,
    /// This daemon's signer for the goal is in restore recovery: it cannot
    /// yet show what it already signed, so it signs nothing. Requests that
    /// sign fail with [`ErrorCode::ReadOnly`].
    SignerRecovery,
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
    pub administrator: PublicKey,
    pub governance_head: Option<EventId>,
    pub current_rules: Option<EventId>,
    pub scope_halts: Vec<ScopeHalt>,
    pub members: Vec<MemberView>,
    pub halted: Option<Halt>,
    pub workspace: Option<WorkspaceBinding>,
    pub grants: GoalGrants,
    pub peers: Vec<PeerView>,
}
/// One current member of a goal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MemberView {
    /// The member's key.
    pub member: PublicKey,
    /// The daemon its admission bound it to.
    pub endpoint: EndpointId,
    /// True for principals this daemon holds keys for.
    pub local: bool,
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

/// Where one local principal's files for a goal live and how far they have
/// come. Local and never replicated. The daemon stores the strings verbatim
/// and never opens the paths; the workspace code in the CLI or adapter reads
/// and writes them. Each string is at most [`MAX_LOCAL_TEXT_BYTES`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceBinding {
    /// The directory the participant chose to share from.
    pub export_root: Option<String>,
    /// The Git commit the last export was taken from. Provenance only: the
    /// exported manifest, not the commit, identifies what was shared.
    pub source_commit: Option<String>,
    /// Manifest of the last export.
    pub exported: Option<BlobHash>,
    /// The separate directory that received snapshots are materialized into.
    pub destination: Option<String>,
    /// Last artifact explicitly applied locally; no goal-wide selection implication.
    pub integrated: Option<BlobHash>,
}

impl WorkspaceBinding {
    /// Refuses a string longer than [`MAX_LOCAL_TEXT_BYTES`].
    pub fn check(&self) -> Result<(), ApiError> {
        let strings = [&self.export_root, &self.source_commit, &self.destination];
        if strings
            .into_iter()
            .flatten()
            .all(|text| text.len() <= MAX_LOCAL_TEXT_BYTES)
        {
            Ok(())
        } else {
            Err(ApiError::new(
                ErrorCode::LimitExceeded,
                "a workspace path or commit name is too long",
            ))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TaskView {
    pub task: TaskId,
    pub context: Context,
    pub creator: PublicKey,
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
    pub offer: Option<EventId>,
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
    pub revision: u64,
    /// Shared content not yet acknowledged by this execution session.
    pub context_news: Option<ContextNews>,
    pub to_authorize: Vec<WorkItem>,
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
    pub author: PublicKey,
    pub context: Context,
    pub attempt: Option<EventId>,
    pub approved: bool,
    pub selected: bool,
    pub evidence: Vec<EventId>,
    pub base: Option<BlobHash>,
    pub patch: Option<BlobHash>,
    pub artifacts: Vec<BlobHash>,
    pub text: Option<String>,
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

/// One event in full: everything a administrator needs to judge a submitted
/// result or a proposed revision before deciding on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EventDetail {
    /// The event's feed entry: identifier, author, kind and standing.
    pub view: EventView,
    /// The decision the event anchors to; absent only on genesis.
    pub anchor: Option<EventId>,
    /// The signed, typed content: for a result its attempt, base, patch
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
    Work(PendingWork),
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
    /// revoked, the caller is not the owner, administrator, assignee or member
    /// the request is for, a daemon-wide grant is missing, the caller is a
    /// viewer and the request is not read-only, or the inviter refused an
    /// invitation.
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
    /// The request is the caller's to make and will be allowed once the
    /// local participant authorizes it: a claim, takeover or decision that no
    /// goal grant covers. The owner answers with `task.authorize`, with
    /// `goal.grant`, or by making the request on the principal's behalf.
    AuthorizationRequired,
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
            Self::AuthorizationRequired => "authorization_required",
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

/// One operation's fields, extracted from the authoritative request schema.
pub fn operation_schema(name: &str) -> Option<serde_json::Value> {
    let schema = request_schema();
    for branch in schema
        .get("oneOf")
        .or_else(|| schema.get("anyOf"))?
        .as_array()?
    {
        if branch.get("const").and_then(serde_json::Value::as_str) == Some(name)
            || branch
                .get("enum")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(name)))
        {
            return Some(
                serde_json::json!({"type":"object","properties":{},"additionalProperties":false}),
            );
        }
        if let Some(input) = branch
            .get("properties")
            .and_then(|properties| properties.get(name))
        {
            let mut input = input.clone();
            if let Some(definitions) = schema.get("$defs") {
                input["$defs"] = definitions.clone();
            }
            return Some(input);
        }
    }
    None
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
                title: "open".into(),
                formation_json: None,
                roles: BTreeMap::new(),
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
    fn claim_bound_publish_requires_matching_generation_shape() {
        let mut request = Request::ContributionPublish {
            goal: GoalId([1; 32]),
            task: None,
            attempt: Some(EventId([2; 32])),
            generation: None,
            summary: "finding".into(),
            base: None,
            patch: None,
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
        assert_eq!(
            GoalGrants::default(),
            GoalGrants {
                administer: false,
                contribute: false,
                execute: false,
                review: false,
                select: false,
                flow: false,
                takeover: false
            }
        );
    }
}
