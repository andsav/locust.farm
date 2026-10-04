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
//! A [`Claim`] binds an assignment to the session that took it and carries a
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

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::API_VERSION;
use crate::codec::{self, CodecError};
use crate::crypto::{self, domain};
use crate::event::{Body, CancelOutcome, Doc, EventError, PayloadRef};
use crate::id::{BlobHash, EndpointId, EventId, GoalId, IdempotencyKey, InstanceId, PublicKey};
use crate::invite::{InviteError, Ticket};
use crate::limits::{MAX_ARTIFACTS, MAX_DEPENDENCIES, MAX_SESSION_DETAIL_BYTES};
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
    /// - Its reads store nothing. It has no feed cursor: `events` reads after
    ///   the position it passes, or from the start without one, and moves no
    ///   cursor, the principal's included. Its `blob.get` of an object not
    ///   held is `Unavailable` without noting a want. Its idempotency keys
    ///   are not recorded, so a repeated read runs again.
    ///
    /// Revoking the principal revokes its viewers.
    Viewer(PublicKey),
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grants {
    /// Create goals, issue invitations, join and leave.
    pub manage_goals: bool,
}

/// One goal's standing policy for one local principal. Creating a goal gives
/// its creator `decide` and nothing else; joining gives nothing, so new work
/// waits for [`Request::TaskAuthorize`] or a grant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalGrants {
    /// Take work assigned to the principal. Without it each assignment waits
    /// for the owner.
    pub execute: bool,
    /// Sign coordinator decisions in a goal the principal coordinates.
    pub decide: bool,
    /// Replace another session's claim on the principal's assignments.
    /// `execute` does not imply it.
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

/// Every operation of the local API, in wire order. Once a release exists,
/// variants are appended and never reordered or removed.
///
/// Each variant says who may ask, what answers it and which errors are
/// particular to it; [`OPERATIONS`] carries the name, audience and
/// read-only facts as data, and [`Request::is_answered_by`] the answer. A
/// variant that names no answer returns [`Response::Recorded`] when it signs
/// an event and [`Response::Done`] otherwise. "Coordinator" means the
/// principal that coordinates the goal, holding the `decide` grant. Requests
/// that sign an event fail with [`ErrorCode::Halted`] or
/// [`ErrorCode::ReadOnly`] while the goal is halted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Request {
    // Daemon-wide.
    /// What this daemon is. An agent sees itself and its own goals; the
    /// owner sees every principal and goal. Answer: [`Response::Status`].
    #[serde(rename = "status")]
    Status,
    /// Owner only. The daemon answers [`Response::Done`], then stops.
    #[serde(rename = "daemon.stop")]
    Shutdown,
    /// Owner only. Creates a principal whose credential is the secret that
    /// `credential` is the [`Credential::digest`] of. The client generated
    /// that secret and already wrote it to its file, so no secret is ever in
    /// a response. `name` must satisfy [`is_agent_name`] and be unused;
    /// repeating the request with the same name and digest returns the same
    /// principal, and a taken name with another digest is `Conflict`.
    /// Answer: [`Response::AgentEnrolled`].
    #[serde(rename = "agent.enroll")]
    AgentEnroll {
        name: String,
        grants: Grants,
        credential: [u8; 32],
    },
    /// Owner only. Replaces a principal's daemon-wide grants.
    #[serde(rename = "agent.grant")]
    AgentGrant { agent: PublicKey, grants: Grants },
    /// Owner only. The principal's credential stops working immediately, and
    /// so do the credentials of its viewers. Its name stays taken and its key
    /// stays attributed in history.
    #[serde(rename = "agent.revoke")]
    AgentRevoke { agent: PublicKey },
    /// Stores the connection's own session record, replacing any earlier one.
    /// A launcher makes its launch intent durable with this before it spawns
    /// the client. Needs a session in the hello (`Invalid` without); only a
    /// connection that presented a session's secret can write its record.
    #[serde(rename = "session.report")]
    SessionReport { record: SessionRecord },
    /// One session's record and claims: the connection's own when `instance`
    /// is absent, otherwise the named one, which must belong to the caller's
    /// principal unless the caller is the owner. `NotFound` if the session
    /// has no record. Answer: [`Response::Session`].
    #[serde(rename = "session.show")]
    Session { instance: Option<InstanceId> },
    /// Sessions with a record: those of the caller's principal, or all of
    /// them for the owner. Answer: [`Response::Sessions`].
    #[serde(rename = "sessions")]
    Sessions,
    /// Deletes a session's record: the connection's own session, or any for
    /// the owner. `Conflict` while the session still holds a claim on an
    /// unfinished assignment, so a claim is never left without a visible
    /// holder; finish it, fail it or have it taken over first.
    #[serde(rename = "session.drop")]
    SessionDrop { instance: InstanceId },
    /// The calling principal founds a goal it owns and coordinates, and is
    /// its first member. Needs `manage_goals`. The creator receives the
    /// `decide` grant for the goal. Answer: [`Response::GoalCreated`].
    #[serde(rename = "goal.create")]
    GoalCreate { title: String },
    /// Redeems an invitation for the calling principal. Needs `manage_goals`.
    /// The daemon records the join and asks the inviter; the answer says
    /// `Joining` until the coordinator's admission has arrived. Repeating the
    /// request returns the current state. `Denied` if the inviter refused
    /// the ticket (unknown, expired or bound to another key). Joining grants
    /// nothing in the goal. Answer: [`Response::Joined`].
    #[serde(rename = "goal.join")]
    GoalJoin { ticket: Ticket },

    // Goal-scoped.
    /// Coordinator; needs `manage_goals`. Issues a single-use invitation.
    /// Answer: [`Response::Invited`].
    #[serde(rename = "goal.invite")]
    GoalInvite {
        goal: GoalId,
        /// Expiry in Unix milliseconds by this daemon's clock; absent for none.
        expires_ms: Option<u64>,
    },
    /// The calling principal asks to leave: the daemon signs a leave request
    /// for the coordinator to act on and stops taking part for this principal
    /// at once. Needs `manage_goals`.
    #[serde(rename = "goal.leave")]
    GoalLeave { goal: GoalId },
    /// Owner only. Replaces one local principal's standing grants in a goal.
    #[serde(rename = "goal.grant")]
    GoalGrant {
        goal: GoalId,
        agent: PublicKey,
        grants: GoalGrants,
    },
    /// The goal as this daemon holds it: members, accepted head, peers, and
    /// the caller's grants and workspace binding. Answer:
    /// [`Response::GoalStatus`].
    #[serde(rename = "goal.status")]
    GoalStatus { goal: GoalId },
    /// Coordinator. Removes a member; its later contributions grant nothing
    /// and its open assignments can no longer finalize.
    #[serde(rename = "member.remove")]
    MemberRemove { goal: GoalId, member: PublicKey },
    /// Records where the calling principal's files for the goal live. Local
    /// only; replaces any earlier binding.
    #[serde(rename = "workspace.set")]
    WorkspaceSet {
        goal: GoalId,
        binding: WorkspaceBinding,
    },
    /// Every task of the goal. Answer: [`Response::Board`].
    #[serde(rename = "board")]
    Board { goal: GoalId },
    /// One task with its text and authored policy. Answer:
    /// [`Response::Task`].
    #[serde(rename = "task.show")]
    Task { goal: GoalId, task: EventId },
    /// One event in full: who wrote it, its typed body, its text and whether
    /// the content it names is held. This is how a coordinator reads a
    /// submitted result or a proposed revision before deciding on it.
    /// Answer: [`Response::Event`].
    #[serde(rename = "event.show")]
    Event { goal: GoalId, event: EventId },
    /// Any member. The recorded event identifies the task.
    #[serde(rename = "task.propose")]
    TaskPropose {
        goal: GoalId,
        text: String,
        /// Manifest of the snapshot the work starts from.
        input: Option<BlobHash>,
        depends_on: Vec<EventId>,
        deadline_ms: Option<u64>,
        max_attempts: Option<u32>,
    },
    /// Coordinator. Assigning an already assigned task supersedes the earlier
    /// assignment with a new attempt. The recorded event identifies the
    /// assignment.
    #[serde(rename = "task.assign")]
    TaskAssign {
        goal: GoalId,
        task: EventId,
        assignee: PublicKey,
    },
    /// Coordinator. Requests cancellation of an assignment; the recorded
    /// event identifies the cancellation the executor answers.
    #[serde(rename = "task.cancel")]
    TaskCancel { goal: GoalId, assignment: EventId },
    /// Owner only. Lets the assignee claim this one assignment without an
    /// `execute` grant, and with `takeover` also lets a session of the
    /// assignee take its claim over without a `takeover` grant. Holds for the
    /// life of the assignment.
    #[serde(rename = "task.authorize")]
    TaskAuthorize {
        goal: GoalId,
        assignment: EventId,
        takeover: bool,
    },
    /// The assignee's session takes an assignment, or recovers the claim it
    /// already holds: the same claim with the same generation comes back, so
    /// retrying after a lost response is safe. Needs a session in the hello
    /// (`Invalid` without) and the `execute` grant or an authorization for
    /// this assignment (`AuthorizationRequired` without). `ClaimHeld` if
    /// another session holds the claim; `Superseded` if the assignment is no
    /// longer current. Answer: [`Response::Claimed`].
    #[serde(rename = "task.claim")]
    TaskClaim { goal: GoalId, assignment: EventId },
    /// The assignee's session replaces another session's claim. The
    /// generation rises by one and the earlier holder is fenced. Needs a
    /// session in the hello and the `takeover` grant or an authorization with
    /// `takeover` for this assignment (`AuthorizationRequired` without);
    /// `execute` is not enough. If the calling session already holds the
    /// claim it is returned unchanged, so a retry does not raise the
    /// generation twice. `Conflict` if nobody holds a claim: use
    /// [`Request::TaskClaim`]. Answer: [`Response::Claimed`].
    #[serde(rename = "task.takeover")]
    TaskTakeover { goal: GoalId, assignment: EventId },
    /// The assignee will not do this assignment. Only before it is claimed.
    #[serde(rename = "task.decline")]
    TaskDecline { goal: GoalId, assignment: EventId },
    /// Claim-bound: the connection's session must hold the claim and
    /// `generation` must be current, otherwise `Superseded`; `Invalid`
    /// without a session in the hello.
    #[serde(rename = "task.progress")]
    TaskProgress {
        goal: GoalId,
        assignment: EventId,
        generation: u32,
        text: String,
    },
    /// Claim-bound like [`Request::TaskProgress`]. Submits the result of the
    /// attempt; the recorded event identifies the result. Completion is not
    /// acceptance.
    #[serde(rename = "task.submit")]
    TaskSubmit {
        goal: GoalId,
        assignment: EventId,
        generation: u32,
        summary: String,
        /// Manifest the work was actually done against.
        base: Option<BlobHash>,
        /// Patch against `base`, if the result changes the workspace.
        patch: Option<BlobHash>,
        /// Further output objects, at most [`MAX_ARTIFACTS`].
        artifacts: Vec<BlobHash>,
    },
    /// Claim-bound like [`Request::TaskProgress`]. The attempt ended without
    /// a result.
    #[serde(rename = "task.fail")]
    TaskFail {
        goal: GoalId,
        assignment: EventId,
        generation: u32,
        reason: String,
    },
    /// The executor's answer to a cancellation. While a claim exists for the
    /// assignment, only the session holding it may answer, naming its current
    /// generation (`Superseded` otherwise). When no claim exists, any
    /// connection of the assignee answers with `generation` absent.
    #[serde(rename = "cancel.acknowledge")]
    CancelAcknowledge {
        goal: GoalId,
        cancel: EventId,
        generation: Option<u32>,
        outcome: CancelOutcome,
    },
    /// Coordinator. Accepts a submitted result. With `head`, the accepted
    /// workspace head advances to that manifest, which is valid only if the
    /// result's `base` is the currently accepted head or no head is accepted
    /// yet; otherwise `Conflict`.
    #[serde(rename = "result.accept")]
    ResultAccept {
        goal: GoalId,
        result: EventId,
        head: Option<BlobHash>,
    },
    /// Coordinator. Declines a submitted result and says why.
    #[serde(rename = "result.reject")]
    ResultReject {
        goal: GoalId,
        result: EventId,
        reason: String,
    },
    /// What needs the caller now, computed from task state and local
    /// records. Never depends on a notification having been delivered.
    /// Answer: [`Response::Pending`].
    #[serde(rename = "pending")]
    Pending { goal: GoalId },
    /// Holds the question of [`Request::Pending`] open. Answers
    /// [`WaitOutcome::Work`] as soon as the goal's revision differs from
    /// `seen`, which may be at once, and [`WaitOutcome::NoEvent`] or
    /// [`WaitOutcome::Disconnected`] only when `timeout_ms` has passed.
    /// A revision is never zero, so `seen: 0` asks for the current state.
    /// Answer: [`Response::Waited`].
    #[serde(rename = "wait")]
    Wait {
        goal: GoalId,
        seen: u64,
        timeout_ms: u32,
    },
    /// The goal's event feed after a position, oldest first, at most `limit`
    /// and at most [`MAX_FEED_PAGE`] events. `after: Some(p)` acknowledges
    /// everything up to `p` and stores `p` as the caller's cursor for the
    /// goal; `after: None` resumes from the stored cursor, or from the start
    /// when there is none, without moving it. The cursor belongs to the
    /// caller, so sessions of one principal share it; a reader that needs its
    /// own position passes `after`. A viewer has no cursor: `after: Some(p)`
    /// reads after `p` and stores nothing, and `after: None` reads from the
    /// start. Answer: [`Response::Events`].
    #[serde(rename = "events")]
    Events {
        goal: GoalId,
        after: Option<u64>,
        limit: u32,
    },
    /// Any member. Appends a note or finding, optionally about one event and
    /// optionally correcting an earlier note.
    #[serde(rename = "note.add")]
    NoteAdd {
        goal: GoalId,
        about: Option<EventId>,
        supersedes: Option<EventId>,
        text: String,
    },
    /// Notes of the goal, or only those about one event. Answer:
    /// [`Response::Notes`].
    #[serde(rename = "notes")]
    Notes {
        goal: GoalId,
        about: Option<EventId>,
    },
    /// A shared document: its accepted text and the revisions proposed
    /// against it. Answer: [`Response::Doc`].
    #[serde(rename = "doc.read")]
    DocRead { goal: GoalId, doc: Doc },
    /// Any member. Proposes a revision written against the accepted revision
    /// `base`; the recorded event identifies the revision.
    #[serde(rename = "doc.revise")]
    DocRevise {
        goal: GoalId,
        doc: Doc,
        base: Option<EventId>,
        text: String,
    },
    /// Coordinator. `Conflict` unless the revision's base is the currently
    /// accepted revision.
    #[serde(rename = "doc.accept")]
    DocAccept { goal: GoalId, revision: EventId },
    /// Stores bytes the client selected, as content of this goal. The daemon
    /// seals them and answers with the hash of the stored, sealed object,
    /// which is the name events and manifests use. The daemon never opens a
    /// path a client names; the trusted CLI or adapter reads the file and
    /// sends this. `LimitExceeded` when the sealed object would exceed the
    /// daemon's `max_blob_bytes`. Answer: [`Response::BlobStored`].
    #[serde(rename = "blob.put")]
    BlobPut {
        goal: GoalId,
        #[serde(with = "codec::bytes")]
        bytes: Vec<u8>,
    },
    /// The plaintext of one content object of this goal. `Unavailable`, at
    /// once, when the object is not held: the daemon notes the want and asks
    /// peers, and [`Request::BlobStat`] shows when it has arrived. `NotFound`
    /// when nothing in this goal names the hash. Answer: [`Response::Blob`].
    #[serde(rename = "blob.get")]
    BlobGet { goal: GoalId, hash: BlobHash },
    /// Whether each object is held, in the order asked; at most
    /// [`MAX_STAT_HASHES`]. Answer: [`Response::BlobStates`].
    #[serde(rename = "blob.stat")]
    BlobStat { goal: GoalId, hashes: Vec<BlobHash> },
    /// Stops serving a locally held object of this goal, to peers and to
    /// local callers. Events that name it stay valid.
    #[serde(rename = "blob.withdraw")]
    BlobWithdraw { goal: GoalId, hash: BlobHash },

    // Daemon-wide, appended.
    /// Owner only. Enrolls a viewer of the principal `agent`: a read-only
    /// credential whose secret `credential` is the [`Credential::digest`] of.
    /// [`Caller::Viewer`] says what a viewer may do: read-only operations
    /// only, as the principal, never a session operation that writes and
    /// never on behalf of anyone. As with `agent.enroll`, the client
    /// generated the secret and already stored it, so no secret is ever in a
    /// request or a response. A principal may have several viewers.
    /// Repeating the request with the same principal and digest changes
    /// nothing. `NotFound` if the principal is unknown or revoked;
    /// `Conflict` if the digest is already the credential of anything else.
    /// Revoking the principal ([`Request::AgentRevoke`]) revokes its viewers;
    /// no request revokes one viewer alone.
    #[serde(rename = "viewer.enroll")]
    ViewerEnroll {
        agent: PublicKey,
        credential: [u8; 32],
    },
}

/// Who may make a request. A viewer ([`Caller::Viewer`]) is outside these
/// audiences: it may make exactly the read-only operations, every one of
/// which has the `Agent` audience.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Audience {
    /// The owner credential only, never on behalf of a principal.
    Owner,
    /// Any enrolled principal within its grants, or the owner (on behalf of a
    /// principal unless the request is read-only).
    Agent,
    /// The principal that coordinates the goal, or the owner on its behalf.
    Coordinator,
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
    /// need not prompt for them. `events` moves only the caller's own
    /// reading position and counts as read-only.
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

const READ: bool = true;
const WRITE: bool = false;
const GOAL: bool = true;
const DAEMON: bool = false;
const TOOL: bool = true;
const NO_TOOL: bool = false;

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
    Status => ("status", READ, DAEMON, Agent, TOOL, "Show the daemon, its principals and the goals the caller is in"),
    Shutdown => ("daemon.stop", WRITE, DAEMON, Owner, NO_TOOL, "Stop the daemon"),
    AgentEnroll { .. } => ("agent.enroll", WRITE, DAEMON, Owner, NO_TOOL, "Enroll a principal under a credential that is already stored"),
    AgentGrant { .. } => ("agent.grant", WRITE, DAEMON, Owner, NO_TOOL, "Set a principal's daemon-wide grants"),
    AgentRevoke { .. } => ("agent.revoke", WRITE, DAEMON, Owner, NO_TOOL, "Revoke a principal's credential"),
    SessionReport { .. } => ("session.report", WRITE, DAEMON, Agent, NO_TOOL, "Record this session's launch and lifecycle state"),
    Session { .. } => ("session.show", READ, DAEMON, Agent, NO_TOOL, "Read one session's record and the claims it holds"),
    Sessions => ("sessions", READ, DAEMON, Agent, NO_TOOL, "List sessions and the claims they hold"),
    SessionDrop { .. } => ("session.drop", WRITE, DAEMON, Agent, NO_TOOL, "Delete the record of a session that holds no claim"),
    GoalCreate { .. } => ("goal.create", WRITE, DAEMON, Agent, TOOL, "Found a goal that the caller owns and coordinates"),
    GoalJoin { .. } => ("goal.join", WRITE, DAEMON, Agent, TOOL, "Redeem an invitation ticket to join a goal"),
    GoalInvite { .. } => ("goal.invite", WRITE, GOAL, Coordinator, TOOL, "Issue a single-use invitation ticket for a goal"),
    GoalLeave { .. } => ("goal.leave", WRITE, GOAL, Agent, TOOL, "Ask to leave a goal and stop taking part in it"),
    GoalGrant { .. } => ("goal.grant", WRITE, GOAL, Owner, NO_TOOL, "Set a principal's standing grants in one goal"),
    GoalStatus { .. } => ("goal.status", READ, GOAL, Agent, TOOL, "Show a goal's members, accepted head, peers and the caller's grants"),
    MemberRemove { .. } => ("member.remove", WRITE, GOAL, Coordinator, TOOL, "Remove a member from a goal"),
    WorkspaceSet { .. } => ("workspace.set", WRITE, GOAL, Agent, TOOL, "Record where the caller's files for a goal live"),
    Board { .. } => ("board", READ, GOAL, Agent, TOOL, "List a goal's tasks and their states"),
    Task { .. } => ("task.show", READ, GOAL, Agent, TOOL, "Read one task with its text and authored policy"),
    Event { .. } => ("event.show", READ, GOAL, Agent, TOOL, "Read one event in full, such as a submitted result or a proposed revision"),
    TaskPropose { .. } => ("task.propose", WRITE, GOAL, Agent, TOOL, "Propose a task"),
    TaskAssign { .. } => ("task.assign", WRITE, GOAL, Coordinator, TOOL, "Assign a task to a member"),
    TaskCancel { .. } => ("task.cancel", WRITE, GOAL, Coordinator, TOOL, "Request cancellation of an assignment"),
    TaskAuthorize { .. } => ("task.authorize", WRITE, GOAL, Owner, NO_TOOL, "Authorize one assignment that no grant covers"),
    TaskClaim { .. } => ("task.claim", WRITE, GOAL, Agent, TOOL, "Take an assignment for this session, or recover this session's claim"),
    TaskTakeover { .. } => ("task.takeover", WRITE, GOAL, Agent, TOOL, "Take over an assignment claimed by another session"),
    TaskDecline { .. } => ("task.decline", WRITE, GOAL, Agent, TOOL, "Decline an assignment"),
    TaskProgress { .. } => ("task.progress", WRITE, GOAL, Agent, TOOL, "Report progress on a claimed assignment"),
    TaskSubmit { .. } => ("task.submit", WRITE, GOAL, Agent, TOOL, "Submit the result of a claimed assignment"),
    TaskFail { .. } => ("task.fail", WRITE, GOAL, Agent, TOOL, "Report that a claimed attempt failed"),
    CancelAcknowledge { .. } => ("cancel.acknowledge", WRITE, GOAL, Agent, TOOL, "Answer a cancellation request with what happened"),
    ResultAccept { .. } => ("result.accept", WRITE, GOAL, Coordinator, TOOL, "Accept a submitted result, optionally advancing the workspace head"),
    ResultReject { .. } => ("result.reject", WRITE, GOAL, Coordinator, TOOL, "Reject a submitted result"),
    Pending { .. } => ("pending", READ, GOAL, Agent, TOOL, "List what needs the caller now"),
    Wait { .. } => ("wait", READ, GOAL, Agent, TOOL, "Wait until a goal changes or a timeout passes"),
    Events { .. } => ("events", READ, GOAL, Agent, TOOL, "Read a goal's event feed from a position or the stored cursor"),
    NoteAdd { .. } => ("note.add", WRITE, GOAL, Agent, TOOL, "Add a note or finding"),
    Notes { .. } => ("notes", READ, GOAL, Agent, TOOL, "Read notes"),
    DocRead { .. } => ("doc.read", READ, GOAL, Agent, TOOL, "Read the plan or summary and its open proposals"),
    DocRevise { .. } => ("doc.revise", WRITE, GOAL, Agent, TOOL, "Propose a revision of the plan or summary"),
    DocAccept { .. } => ("doc.accept", WRITE, GOAL, Coordinator, TOOL, "Accept a proposed revision"),
    BlobPut { .. } => ("blob.put", WRITE, GOAL, Agent, NO_TOOL, "Store content for a goal"),
    BlobGet { .. } => ("blob.get", READ, GOAL, Agent, NO_TOOL, "Fetch content of a goal"),
    BlobStat { .. } => ("blob.stat", READ, GOAL, Agent, TOOL, "Check whether content is held, requested or unavailable"),
    BlobWithdraw { .. } => ("blob.withdraw", WRITE, GOAL, Agent, TOOL, "Stop serving content held here"),
    ViewerEnroll { .. } => ("viewer.enroll", WRITE, DAEMON, Owner, NO_TOOL, "Enroll a read-only viewer of a principal under a credential that is already stored"),
}

impl Request {
    /// Stable operation name, used for command names, MCP tool names
    /// ([`Operation::tool_name`]), logs and errors. Equal to the request's
    /// JSON tag.
    pub fn name(&self) -> &'static str {
        self.operation().name
    }

    /// True if the request authors no event and changes nothing another
    /// caller can observe; see [`Operation::read_only`].
    pub fn is_read_only(&self) -> bool {
        self.operation().read_only
    }

    /// The goal a request is scoped to; `None` for daemon-wide requests.
    pub fn goal(&self) -> Option<GoalId> {
        match self {
            Self::Status
            | Self::Shutdown
            | Self::AgentEnroll { .. }
            | Self::AgentGrant { .. }
            | Self::AgentRevoke { .. }
            | Self::SessionReport { .. }
            | Self::Session { .. }
            | Self::Sessions
            | Self::SessionDrop { .. }
            | Self::GoalCreate { .. }
            | Self::GoalJoin { .. }
            | Self::ViewerEnroll { .. } => None,
            Self::GoalInvite { goal, .. }
            | Self::GoalLeave { goal }
            | Self::GoalGrant { goal, .. }
            | Self::GoalStatus { goal }
            | Self::MemberRemove { goal, .. }
            | Self::WorkspaceSet { goal, .. }
            | Self::Board { goal }
            | Self::Task { goal, .. }
            | Self::Event { goal, .. }
            | Self::TaskPropose { goal, .. }
            | Self::TaskAssign { goal, .. }
            | Self::TaskCancel { goal, .. }
            | Self::TaskAuthorize { goal, .. }
            | Self::TaskClaim { goal, .. }
            | Self::TaskTakeover { goal, .. }
            | Self::TaskDecline { goal, .. }
            | Self::TaskProgress { goal, .. }
            | Self::TaskSubmit { goal, .. }
            | Self::TaskFail { goal, .. }
            | Self::CancelAcknowledge { goal, .. }
            | Self::ResultAccept { goal, .. }
            | Self::ResultReject { goal, .. }
            | Self::Pending { goal }
            | Self::Wait { goal, .. }
            | Self::Events { goal, .. }
            | Self::NoteAdd { goal, .. }
            | Self::Notes { goal, .. }
            | Self::DocRead { goal, .. }
            | Self::DocRevise { goal, .. }
            | Self::DocAccept { goal, .. }
            | Self::BlobPut { goal, .. }
            | Self::BlobGet { goal, .. }
            | Self::BlobStat { goal, .. }
            | Self::BlobWithdraw { goal, .. } => Some(*goal),
        }
    }

    /// Checks what can be checked without the daemon's state: the name rule,
    /// count limits and the sizes of strings stored verbatim. The daemon runs
    /// this before anything else; a client may run it to fail before a round
    /// trip. Passing says nothing about authorization or state.
    pub fn check(&self) -> Result<(), ApiError> {
        match self {
            Self::AgentEnroll { name, .. } if !is_agent_name(name) => Err(ApiError::new(
                ErrorCode::Invalid,
                "a principal's name is 1 to 32 characters from a-z, 0-9 and -",
            )),
            Self::TaskPropose { depends_on, .. } if depends_on.len() > MAX_DEPENDENCIES => Err(
                ApiError::new(ErrorCode::LimitExceeded, "too many task dependencies"),
            ),
            Self::TaskSubmit { artifacts, .. } if artifacts.len() > MAX_ARTIFACTS => Err(
                ApiError::new(ErrorCode::LimitExceeded, "too many result artifacts"),
            ),
            Self::BlobStat { hashes, .. } if hashes.len() > MAX_STAT_HASHES => Err(ApiError::new(
                ErrorCode::LimitExceeded,
                "too many hashes in one request",
            )),
            Self::SessionReport { record } => record.check(),
            Self::WorkspaceSet { binding, .. } => binding.check(),
            _ => Ok(()),
        }
    }

    /// True if `response` is the kind of answer this request gets when it
    /// succeeds. A client treats anything else as a protocol error, so callers
    /// need not handle answers that cannot occur.
    pub fn is_answered_by(&self, response: &Response) -> bool {
        match self {
            Self::Shutdown
            | Self::AgentGrant { .. }
            | Self::AgentRevoke { .. }
            | Self::SessionReport { .. }
            | Self::SessionDrop { .. }
            | Self::GoalGrant { .. }
            | Self::WorkspaceSet { .. }
            | Self::TaskAuthorize { .. }
            | Self::BlobWithdraw { .. }
            | Self::ViewerEnroll { .. } => matches!(response, Response::Done),
            Self::GoalLeave { .. }
            | Self::MemberRemove { .. }
            | Self::TaskPropose { .. }
            | Self::TaskAssign { .. }
            | Self::TaskCancel { .. }
            | Self::TaskDecline { .. }
            | Self::TaskProgress { .. }
            | Self::TaskSubmit { .. }
            | Self::TaskFail { .. }
            | Self::CancelAcknowledge { .. }
            | Self::ResultAccept { .. }
            | Self::ResultReject { .. }
            | Self::NoteAdd { .. }
            | Self::DocRevise { .. }
            | Self::DocAccept { .. } => matches!(response, Response::Recorded { .. }),
            Self::TaskClaim { .. } | Self::TaskTakeover { .. } => {
                matches!(response, Response::Claimed(_))
            }
            Self::Status => matches!(response, Response::Status(_)),
            Self::AgentEnroll { .. } => matches!(response, Response::AgentEnrolled { .. }),
            Self::Session { .. } => matches!(response, Response::Session(_)),
            Self::Sessions => matches!(response, Response::Sessions(_)),
            Self::GoalCreate { .. } => matches!(response, Response::GoalCreated { .. }),
            Self::GoalJoin { .. } => matches!(response, Response::Joined { .. }),
            Self::GoalInvite { .. } => matches!(response, Response::Invited { .. }),
            Self::GoalStatus { .. } => matches!(response, Response::GoalStatus(_)),
            Self::Board { .. } => matches!(response, Response::Board(_)),
            Self::Task { .. } => matches!(response, Response::Task(_)),
            Self::Event { .. } => matches!(response, Response::Event(_)),
            Self::Pending { .. } => matches!(response, Response::Pending(_)),
            Self::Wait { .. } => matches!(response, Response::Waited(_)),
            Self::Events { .. } => matches!(response, Response::Events(_)),
            Self::Notes { .. } => matches!(response, Response::Notes(_)),
            Self::DocRead { .. } => matches!(response, Response::Doc(_)),
            Self::BlobPut { .. } => matches!(response, Response::BlobStored { .. }),
            Self::BlobGet { .. } => matches!(response, Response::Blob { .. }),
            Self::BlobStat { .. } => matches!(response, Response::BlobStates(_)),
        }
    }
}

/// What a request answers with when it succeeds; which variant answers which
/// request is [`Request::is_answered_by`]. No response ever carries a secret.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    /// `Joining` until the coordinator's admission has arrived.
    Joined {
        /// The goal the ticket named.
        goal: GoalId,
        /// The coordinator's key, to show as a fingerprint.
        coordinator: PublicKey,
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
    Event(EventDetail),
    /// The request was signed and recorded as this event, which identifies
    /// what it created: a task, an assignment, a result, a cancellation, a
    /// note or a revision.
    Recorded { event: EventId },
    /// Answers `task.claim` and `task.takeover` with the claim the calling
    /// session now holds.
    Claimed(Claim),
    /// Answers `pending`.
    Pending(PendingWork),
    /// Answers `wait`.
    Waited(WaitOutcome),
    /// Answers `events`: feed entries in ascending position.
    Events(Vec<EventView>),
    /// Answers `notes`.
    Notes(Vec<NoteView>),
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
        bytes: Vec<u8>,
    },
    /// Answers `blob.stat`: one entry per hash asked about, in the order
    /// asked.
    BlobStates(Vec<BlobStatus>),
    /// Answers `session.show`.
    Session(SessionView),
    /// Answers `sessions`.
    Sessions(Vec<SessionView>),
}

/// What `status` shows: the daemon, and as much of its principals and goals
/// as the caller may see.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Membership {
    /// An invitation was redeemed here; the coordinator's admission has not
    /// arrived yet.
    Joining,
    /// Admitted and taking part.
    Member,
    /// The coordinator removed the principal. History stays readable;
    /// nothing new is authored.
    Removed,
    /// The principal asked to leave and no longer takes part.
    Left,
    /// The coordinator refused this local join attempt. No goal content is
    /// readable until a subsequent invitation is admitted.
    Refused,
}

/// One local principal's part in one goal, as `status` lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

/// One goal as this daemon holds it, with the parts that are about the
/// calling principal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalStatus {
    /// The goal this is about.
    pub goal: GoalId,
    /// Absent until the goal's founding text is held locally.
    pub title: Option<String>,
    /// The key that signs the goal's decisions.
    pub coordinator: PublicKey,
    /// Latest decision applied locally. Absent only while a join is pending
    /// and none of the goal's history has arrived.
    pub decision_head: Option<EventId>,
    /// Current members, by the decisions applied locally.
    pub members: Vec<MemberView>,
    /// Set while the goal's decisions cannot advance here.
    pub halted: Option<Halt>,
    /// The accepted workspace head: the manifest named by the latest accepted
    /// result that carried one.
    pub head: Option<BlobHash>,
    /// The calling principal's workspace binding for this goal.
    pub workspace: Option<WorkspaceBinding>,
    /// The calling principal's standing grants in this goal; all false for
    /// the owner asking directly.
    pub grants: GoalGrants,
    /// The other daemons that speak for members of this goal.
    pub peers: Vec<PeerView>,
}

/// One current member of a goal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberView {
    /// The member's key.
    pub member: PublicKey,
    /// The daemon its admission bound it to.
    pub endpoint: EndpointId,
    /// True for principals this daemon holds keys for.
    pub local: bool,
}

/// What this daemon knows about reaching one peer of a goal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    /// The accepted head last integrated into the participant's own files.
    /// It trails [`GoalStatus::head`] while an accepted result has not been
    /// integrated. The trusted CLI records it with `workspace.set` after it
    /// has applied an accepted head to those files; the daemon stores it as
    /// given and never checks it against the files. [`TaskView::applied`] is
    /// derived from it.
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

/// Where a task stands. Derived from the goal's history; states reported by
/// the worker are shown as reported, and only `Accepted` and `Rejected`
/// reflect a coordinator decision on a result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Proposed, not assigned.
    Proposed,
    /// Assigned; the assignee has not answered.
    Assigned,
    /// The assignee took the assignment.
    Taken,
    /// The assignee declined; awaiting reassignment.
    Declined,
    /// The attempt failed; awaiting reassignment.
    Failed,
    /// A result awaits the coordinator.
    Submitted,
    /// The coordinator accepted a result.
    Accepted,
    /// The coordinator rejected the latest result.
    Rejected,
    /// Cancellation requested; the executor has not answered.
    CancelRequested,
    /// The executor answered a cancellation.
    Cancelled(CancelOutcome),
}

/// One task as the board lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskView {
    /// The event that proposed the task.
    pub task: EventId,
    /// Where the task stands.
    pub state: TaskState,
    /// First line of the task text, when the text is held locally.
    pub title: Option<String>,
    /// Who proposed the task.
    pub proposer: PublicKey,
    /// Who the current assignment names.
    pub assignee: Option<PublicKey>,
    /// The current assignment, if any.
    pub assignment: Option<EventId>,
    /// Attempt number of the current assignment; zero before the first.
    pub attempt: u32,
    /// The latest submitted result of the current assignment. Read it with
    /// [`Request::Event`].
    pub result: Option<EventId>,
    /// True when the task's accepted result carried a workspace head and the
    /// calling principal's workspace binding on this machine records that
    /// head, or a head accepted after it, as
    /// [`integrated`](WorkspaceBinding::integrated). Derived each time it is
    /// read and local to this machine: another daemon, or another principal
    /// here, may answer differently for the same task. False when the task
    /// has no accepted result with a head, when the binding records a
    /// manifest that is not an accepted head of the goal, and for the owner
    /// asking directly, who has no binding. Submitted, accepted and applied
    /// are the three outcomes a task's result goes through.
    pub applied: bool,
}

/// One task in full: its board entry, its text and the policy its proposer
/// authored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDetail {
    /// The task's board entry.
    pub view: TaskView,
    /// The task text; absent when its payload is not held locally.
    pub text: Option<String>,
    /// Manifest of the snapshot the work starts from.
    pub input: Option<BlobHash>,
    /// Tasks this one was proposed as depending on.
    pub depends_on: Vec<EventId>,
    /// Authored absolute deadline in Unix milliseconds; absent means none.
    pub deadline_ms: Option<u64>,
    /// Authored attempt budget; absent means unlimited.
    pub max_attempts: Option<u32>,
    /// An unanswered cancellation request for the current assignment.
    pub cancel: Option<EventId>,
}

/// One session's hold on an assignment. Local daemon state, not a protocol
/// event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    /// The goal the assignment belongs to.
    pub goal: GoalId,
    /// The task the assignment is for.
    pub task: EventId,
    /// The claimed assignment.
    pub assignment: EventId,
    /// The session that holds the claim.
    pub instance: InstanceId,
    /// 1 for the first claim of an assignment; each takeover raises it by
    /// one. Claim-bound requests must name the current value.
    pub generation: u32,
}

/// An assignment and the task it is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssignmentRef {
    /// The task the assignment is for.
    pub task: EventId,
    /// The event that made the assignment.
    pub assignment: EventId,
}

/// A cancellation request the connection may answer now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelItem {
    /// The task the assignment is for.
    pub task: EventId,
    /// The assignment whose cancellation is requested.
    pub assignment: EventId,
    /// The cancellation request to answer.
    pub cancel: EventId,
    /// What [`Request::CancelAcknowledge`] must carry: the generation of the
    /// claim this session holds on the assignment, or absent when nobody
    /// holds a claim.
    pub generation: Option<u32>,
}

/// A submitted result waiting for the coordinator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewItem {
    /// The task the result is for.
    pub task: EventId,
    /// The submitted result; read it with [`Request::Event`].
    pub result: EventId,
}

/// What needs the caller in one goal, as identifiers only. Text written by
/// peers is fetched separately through the scoped read operations. Every
/// item names its task.
///
/// For an agent or a viewer the lists are about the calling principal, and
/// `claimed` about the connection's own session, so it is empty for a
/// viewer. For the owner asking directly only `to_authorize` is filled,
/// across every local principal.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingWork {
    /// The goal's change counter when this answer was computed. The daemon
    /// raises it, in the same commit, whenever the goal's events or any local
    /// record that feeds this answer changes. It starts at 1. Pass it as
    /// `seen` to [`Request::Wait`].
    pub revision: u64,
    /// Current assignments that no grant covers and the owner has not
    /// authorized.
    pub to_authorize: Vec<AssignmentRef>,
    /// Current, unclaimed assignments the caller may claim now.
    pub to_claim: Vec<AssignmentRef>,
    /// Claims this connection's session holds on unfinished assignments,
    /// with the generation its claim-bound requests must name. Empty for a
    /// connection without a session.
    pub claimed: Vec<Claim>,
    /// Unfinished assignments of the caller's principal whose claim another
    /// session holds. This connection cannot report on them; it can only
    /// take them over.
    pub held_elsewhere: Vec<Claim>,
    /// Cancellation requests this connection may answer now. One whose claim
    /// another session holds is that session's to answer and is listed for
    /// it instead.
    pub to_acknowledge: Vec<CancelItem>,
    /// Results waiting for the caller's decision as coordinator.
    pub to_review: Vec<ReviewItem>,
}

impl PendingWork {
    /// True if every list is empty.
    pub fn is_empty(&self) -> bool {
        self.to_authorize.is_empty()
            && self.to_claim.is_empty()
            && self.claimed.is_empty()
            && self.held_elsewhere.is_empty()
            && self.to_acknowledge.is_empty()
            && self.to_review.is_empty()
    }
}

/// How a `wait` ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

/// How an event stands in the goal's replicated state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
}

/// One event as the feed lists it: who wrote what kind of event, and how it
/// stands. The content is read with [`Request::Event`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobStatus {
    /// Hash of the stored, sealed object.
    pub hash: BlobHash,
    /// Whether it can be served now.
    pub state: BlobState,
}

/// One event in full: everything a coordinator needs to judge a submitted
/// result or a proposed revision before deciding on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventDetail {
    /// The event's feed entry: identifier, author, kind and standing.
    pub view: EventView,
    /// The decision the event anchors to; absent only on genesis.
    pub anchor: Option<EventId>,
    /// The signed, typed content: for a result its assignment, base, patch
    /// and artifacts; for a revision its document and base.
    pub body: Body,
    /// Names the event's text, if it has any.
    pub payload: Option<PayloadRef>,
    /// The payload as text: a result's summary, a revision's text. Absent
    /// when there is no payload or it is not held locally.
    pub text: Option<String>,
    /// The task the event concerns, resolved through its assignment, result
    /// or cancellation; absent for events that concern no task.
    pub task: Option<EventId>,
    /// Every content object the event names, in the order of
    /// [`Header::blobs`](crate::event::Header::blobs) (payload first), with
    /// whether it is held.
    pub content: Vec<BlobStatus>,
}

/// One note or finding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteView {
    /// The event that is the note.
    pub note: EventId,
    /// Who wrote it.
    pub author: PublicKey,
    /// The event the note is about, if it names one.
    pub about: Option<EventId>,
    /// The earlier note this one corrects, if any.
    pub supersedes: Option<EventId>,
    /// The author's clock. Diagnostic only.
    pub at_ms: u64,
    /// Absent when the payload is not held locally.
    pub text: Option<String>,
}

/// A shared document: what is accepted and what is proposed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocView {
    /// Which document this is.
    pub doc: Doc,
    /// The accepted revision, if any.
    pub accepted: Option<EventId>,
    /// Text of the accepted revision; absent when it is not held locally.
    pub text: Option<String>,
    /// Revisions proposed against the accepted one and not yet decided. Read
    /// one with [`Request::Event`].
    pub proposals: Vec<EventId>,
}

/// How far a client session has come, as its adapter observed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Claims the session holds on unfinished assignments, in any goal.
    pub claims: Vec<Claim>,
}

/// Stable error categories. Clients branch on the code, never on the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The request is not the caller's to make: the credential is unknown or
    /// revoked, the caller is not the owner, coordinator, assignee or member
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
    /// The caller's claim, generation, attempt or assignment is not current.
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    /// What clients branch on.
    pub code: ErrorCode,
    /// For people. Written by the daemon; never contains peer-written text.
    pub message: String,
}

impl ApiError {
    /// An error with a message written by the daemon or this crate, never
    /// copied from a peer.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
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

#[cfg(test)]
mod tests {
    use serde::de::DeserializeOwned;
    use serde_json::Value;

    use super::*;
    use crate::PROTOCOL_VERSION;
    use crate::invite::{Invitation, InviteSecret};
    use crate::limits::{MAX_BLOB_BYTES, MAX_HELLO_FRAME_BYTES};

    const CREDENTIAL: Credential = Credential([0x5a; 32]);
    const SESSION: SessionSecret = SessionSecret([0x6b; 32]);
    const INVITE: InviteSecret = InviteSecret([0x7c; 32]);

    fn goal() -> GoalId {
        GoalId([0x11; 32])
    }

    fn event(n: u8) -> EventId {
        EventId([n; 32])
    }

    fn blob(n: u8) -> BlobHash {
        BlobHash([n; 32])
    }

    fn key(n: u8) -> PublicKey {
        PublicKey([n; 32])
    }

    fn ticket() -> Ticket {
        Invitation {
            version: PROTOCOL_VERSION,
            goal: goal(),
            coordinator: key(1),
            endpoint: EndpointId([0x0e; 32]),
            hints: vec!["https://relay.example".to_string()],
            secret: INVITE,
            expires_ms: None,
        }
        .to_ticket()
        .unwrap()
    }

    fn record(state: SessionState) -> SessionRecord {
        SessionRecord {
            client: "codex 0.48.0".to_string(),
            state,
            client_session: None,
            capabilities: SessionCapabilities {
                tools: true,
                manual_resume: true,
                ..SessionCapabilities::default()
            },
            detail: b"adapter-owned launch intent".to_vec(),
        }
    }

    fn binding() -> WorkspaceBinding {
        WorkspaceBinding {
            export_root: Some("/home/ada/app".to_string()),
            source_commit: Some("4c7b680".to_string()),
            exported: Some(blob(0x30)),
            destination: None,
            integrated: None,
        }
    }

    fn claim(assignment: EventId, session: SessionSecret, generation: u32) -> Claim {
        Claim {
            goal: goal(),
            task: event(0x20),
            assignment,
            instance: session.instance(),
            generation,
        }
    }

    fn task_view(state: TaskState) -> TaskView {
        TaskView {
            task: event(0x20),
            state,
            title: Some("Add a login page".to_string()),
            proposer: key(1),
            assignee: Some(key(2)),
            assignment: Some(event(0x21)),
            attempt: 1,
            result: Some(event(0x23)),
            applied: false,
        }
    }

    fn event_view(position: u64, event: EventId, author: PublicKey, kind: &str) -> EventView {
        EventView {
            position: Some(position),
            event,
            author,
            kind: kind.to_string(),
            at_ms: 1_790_000_000_000 + position,
            standing: Standing::Effective,
        }
    }

    fn session_view(record: SessionRecord) -> SessionView {
        SessionView {
            instance: SESSION.instance(),
            principal: key(2),
            record,
            updated_ms: 1_790_000_000_000,
            attached: true,
            claims: Vec::new(),
        }
    }

    fn goal_status() -> GoalStatus {
        GoalStatus {
            goal: goal(),
            title: Some("Ship the login page".to_string()),
            coordinator: key(1),
            decision_head: Some(event(0x1e)),
            members: vec![
                MemberView {
                    member: key(1),
                    endpoint: EndpointId([0x0e; 32]),
                    local: false,
                },
                MemberView {
                    member: key(2),
                    endpoint: EndpointId([0x0f; 32]),
                    local: true,
                },
            ],
            halted: None,
            head: None,
            workspace: None,
            grants: GoalGrants::default(),
            peers: vec![PeerView {
                endpoint: EndpointId([0x0e; 32]),
                connected: true,
                last_sync_ms: Some(1_790_000_000_000),
            }],
        }
    }

    /// The result a worker submitted, as `event.show` returns it.
    fn result_detail(result: EventId, assignment: EventId) -> EventDetail {
        EventDetail {
            view: event_view(9, result, key(2), "result_submitted"),
            anchor: Some(event(0x1e)),
            body: Body::ResultSubmitted {
                assignment,
                base: Some(blob(0x30)),
                patch: Some(blob(0x31)),
                artifacts: vec![blob(0x32)],
            },
            payload: Some(PayloadRef {
                hash: blob(0x33),
                len: 64,
                key_epoch: Default::default(),
            }),
            text: Some("Login page added; tests pass.".to_string()),
            task: Some(event(0x20)),
            content: [
                (0x33, BlobState::Held),
                (0x30, BlobState::Held),
                (0x31, BlobState::Held),
                (0x32, BlobState::Requested),
            ]
            .map(|(hash, state)| BlobStatus {
                hash: blob(hash),
                state,
            })
            .to_vec(),
        }
    }

    /// One request of every variant, in declaration order.
    fn requests() -> Vec<Request> {
        let goal = goal();
        let (task, assignment, result) = (event(0x20), event(0x21), event(0x23));
        vec![
            Request::Status,
            Request::Shutdown,
            Request::AgentEnroll {
                name: "worker".to_string(),
                grants: Grants { manage_goals: true },
                credential: CREDENTIAL.digest(),
            },
            Request::AgentGrant {
                agent: key(2),
                grants: Grants::default(),
            },
            Request::AgentRevoke { agent: key(2) },
            Request::SessionReport {
                record: record(SessionState::Launching),
            },
            Request::Session { instance: None },
            Request::Sessions,
            Request::SessionDrop {
                instance: SESSION.instance(),
            },
            Request::GoalCreate {
                title: "Ship the login page".to_string(),
            },
            Request::GoalJoin { ticket: ticket() },
            Request::GoalInvite {
                goal,
                expires_ms: Some(1_790_086_400_000),
            },
            Request::GoalLeave { goal },
            Request::GoalGrant {
                goal,
                agent: key(2),
                grants: GoalGrants {
                    execute: true,
                    ..GoalGrants::default()
                },
            },
            Request::GoalStatus { goal },
            Request::MemberRemove {
                goal,
                member: key(3),
            },
            Request::WorkspaceSet {
                goal,
                binding: binding(),
            },
            Request::Board { goal },
            Request::Task { goal, task },
            Request::Event {
                goal,
                event: result,
            },
            Request::TaskPropose {
                goal,
                text: "Add a login page".to_string(),
                input: Some(blob(0x30)),
                depends_on: vec![event(0x1f)],
                deadline_ms: Some(1_790_000_100_000),
                max_attempts: Some(3),
            },
            Request::TaskAssign {
                goal,
                task,
                assignee: key(2),
            },
            Request::TaskCancel { goal, assignment },
            Request::TaskAuthorize {
                goal,
                assignment,
                takeover: false,
            },
            Request::TaskClaim { goal, assignment },
            Request::TaskTakeover { goal, assignment },
            Request::TaskDecline { goal, assignment },
            Request::TaskProgress {
                goal,
                assignment,
                generation: 1,
                text: "Form renders".to_string(),
            },
            Request::TaskSubmit {
                goal,
                assignment,
                generation: 1,
                summary: "Login page added".to_string(),
                base: Some(blob(0x30)),
                patch: Some(blob(0x31)),
                artifacts: vec![blob(0x32)],
            },
            Request::TaskFail {
                goal,
                assignment,
                generation: 1,
                reason: "The build is broken upstream".to_string(),
            },
            Request::CancelAcknowledge {
                goal,
                cancel: event(0x24),
                generation: Some(1),
                outcome: CancelOutcome::Stopped,
            },
            Request::ResultAccept {
                goal,
                result,
                head: Some(blob(0x34)),
            },
            Request::ResultReject {
                goal,
                result,
                reason: "Tests are missing".to_string(),
            },
            Request::Pending { goal },
            Request::Wait {
                goal,
                seen: 7,
                timeout_ms: 30_000,
            },
            Request::Events {
                goal,
                after: Some(4),
                limit: 50,
            },
            Request::NoteAdd {
                goal,
                about: Some(task),
                supersedes: None,
                text: "The form needs a CSRF token".to_string(),
            },
            Request::Notes { goal, about: None },
            Request::DocRead {
                goal,
                doc: Doc::Plan,
            },
            Request::DocRevise {
                goal,
                doc: Doc::Plan,
                base: None,
                text: "1. Login page".to_string(),
            },
            Request::DocAccept {
                goal,
                revision: event(0x25),
            },
            Request::BlobPut {
                goal,
                bytes: b"diff --git a/login.rs b/login.rs".to_vec(),
            },
            Request::BlobGet {
                goal,
                hash: blob(0x31),
            },
            Request::BlobStat {
                goal,
                hashes: vec![blob(0x31), blob(0x32)],
            },
            Request::BlobWithdraw {
                goal,
                hash: blob(0x32),
            },
            Request::ViewerEnroll {
                agent: key(2),
                credential: CREDENTIAL.digest(),
            },
        ]
    }

    /// One response of every variant, in declaration order.
    fn responses() -> Vec<Response> {
        let goal = goal();
        vec![
            Response::Done,
            Response::Status(DaemonStatus {
                daemon_version: "0.1.0".to_string(),
                endpoint: Some(EndpointId([0x0f; 32])),
                agents: vec![AgentView {
                    agent: key(2),
                    name: "worker".to_string(),
                    grants: Grants { manage_goals: true },
                    revoked: false,
                }],
                goals: vec![GoalSummary {
                    goal,
                    title: None,
                    member: key(2),
                    membership: Membership::Joining,
                    halted: Some(Halt::SignerRecovery),
                }],
            }),
            Response::AgentEnrolled { agent: key(2) },
            Response::GoalCreated { goal },
            Response::Invited { ticket: ticket() },
            Response::Joined {
                goal,
                coordinator: key(1),
                membership: Membership::Joining,
            },
            Response::GoalStatus(GoalStatus {
                workspace: Some(binding()),
                ..goal_status()
            }),
            Response::Board(vec![task_view(TaskState::Submitted)]),
            Response::Task(TaskDetail {
                view: task_view(TaskState::Cancelled(CancelOutcome::Stopped)),
                text: Some("Add a login page".to_string()),
                input: Some(blob(0x30)),
                depends_on: vec![event(0x1f)],
                deadline_ms: None,
                max_attempts: Some(3),
                cancel: None,
            }),
            Response::Event(result_detail(event(0x23), event(0x21))),
            Response::Recorded { event: event(0x23) },
            Response::Claimed(claim(event(0x21), SESSION, 1)),
            Response::Pending(PendingWork {
                revision: 7,
                to_authorize: vec![AssignmentRef {
                    task: event(0x20),
                    assignment: event(0x21),
                }],
                to_claim: Vec::new(),
                claimed: vec![claim(event(0x26), SESSION, 3)],
                held_elsewhere: vec![claim(event(0x27), SessionSecret([0x6d; 32]), 2)],
                to_acknowledge: vec![CancelItem {
                    task: event(0x20),
                    assignment: event(0x26),
                    cancel: event(0x24),
                    generation: Some(3),
                }],
                to_review: vec![ReviewItem {
                    task: event(0x20),
                    result: event(0x23),
                }],
            }),
            Response::Waited(WaitOutcome::Disconnected),
            Response::Events(vec![event_view(9, event(0x23), key(2), "result_submitted")]),
            Response::Notes(vec![NoteView {
                note: event(0x28),
                author: key(2),
                about: Some(event(0x20)),
                supersedes: None,
                at_ms: 1_790_000_000_000,
                text: None,
            }]),
            Response::Doc(DocView {
                doc: Doc::Summary,
                accepted: None,
                text: None,
                proposals: vec![event(0x25)],
            }),
            Response::BlobStored { hash: blob(0x31) },
            Response::Blob {
                bytes: b"diff --git a/login.rs b/login.rs".to_vec(),
            },
            Response::BlobStates(vec![BlobStatus {
                hash: blob(0x31),
                state: BlobState::Unknown,
            }]),
            Response::Session(session_view(record(SessionState::Ready))),
            Response::Sessions(vec![session_view(record(SessionState::Exited))]),
        ]
    }

    /// The JSON tag of an enum value: the string a unit variant renders as,
    /// or the single key of any other variant.
    fn json_tag<T: Serialize>(value: &T) -> String {
        match serde_json::to_value(value).unwrap() {
            Value::String(tag) => tag,
            Value::Object(tagged) if tagged.len() == 1 => tagged.keys().next().unwrap().clone(),
            other => panic!("not the rendering of an enum: {other}"),
        }
    }

    /// The JSON field names of a request.
    fn json_fields(request: &Request) -> Vec<String> {
        match serde_json::to_value(request).unwrap() {
            Value::Object(tagged) => tagged
                .values()
                .flat_map(|fields| fields.as_object().unwrap().keys().cloned())
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Checks that `values` holds one value per variant of `T` in declaration
    /// order: each encodes with its position as the variant index, and the
    /// index after the last names no variant. A list that covers the enum
    /// today therefore fails as soon as a variant is added without a value.
    fn assert_covers<T: Serialize + DeserializeOwned + fmt::Debug>(values: &[T]) {
        for (index, value) in values.iter().enumerate() {
            let encoded = codec::encode(value).unwrap();
            assert_eq!(usize::from(encoded[0]), index, "{value:?}");
        }
        let past_the_end = [u8::try_from(values.len()).unwrap()];
        assert!(
            matches!(
                postcard::from_bytes::<T>(&past_the_end),
                Err(postcard::Error::SerdeDeCustom)
            ),
            "a variant after {:?} has no value in the list",
            values.last()
        );
    }

    fn tags<T: Serialize + DeserializeOwned + fmt::Debug>(values: &[T]) -> Vec<String> {
        assert_covers(values);
        values.iter().map(json_tag).collect()
    }

    #[test]
    fn the_operations_table_and_the_request_enum_agree() {
        let requests = requests();
        assert_covers(&requests);
        assert_eq!(requests.len(), OPERATIONS.len());
        for (index, request) in requests.iter().enumerate() {
            let operation = request.operation();
            assert_eq!(operation, &OPERATIONS[index]);
            assert_eq!(request.name(), operation.name);
            assert_eq!(request.is_read_only(), operation.read_only);
            assert_eq!(
                request.goal().is_some(),
                operation.goal_scoped,
                "{}",
                operation.name
            );
            assert!(!operation.summary.is_empty() && !operation.summary.ends_with('.'));
        }
    }

    #[test]
    fn operation_names_are_the_published_ones_and_stay_distinct_as_tool_names() {
        let names: Vec<&str> = OPERATIONS.iter().map(|operation| operation.name).collect();
        assert_eq!(
            names,
            [
                "status",
                "daemon.stop",
                "agent.enroll",
                "agent.grant",
                "agent.revoke",
                "session.report",
                "session.show",
                "sessions",
                "session.drop",
                "goal.create",
                "goal.join",
                "goal.invite",
                "goal.leave",
                "goal.grant",
                "goal.status",
                "member.remove",
                "workspace.set",
                "board",
                "task.show",
                "event.show",
                "task.propose",
                "task.assign",
                "task.cancel",
                "task.authorize",
                "task.claim",
                "task.takeover",
                "task.decline",
                "task.progress",
                "task.submit",
                "task.fail",
                "cancel.acknowledge",
                "result.accept",
                "result.reject",
                "pending",
                "wait",
                "events",
                "note.add",
                "notes",
                "doc.read",
                "doc.revise",
                "doc.accept",
                "blob.put",
                "blob.get",
                "blob.stat",
                "blob.withdraw",
                "viewer.enroll",
            ]
        );

        let mut tool_names: Vec<String> = OPERATIONS.iter().map(Operation::tool_name).collect();
        assert_eq!(tool_names[18], "locust_task_show");
        assert_eq!(tool_names[0], "locust_status");
        tool_names.sort();
        tool_names.dedup();
        assert_eq!(tool_names.len(), OPERATIONS.len());
    }

    #[test]
    fn the_json_tag_of_every_request_is_its_operation_name() {
        for request in requests() {
            assert_eq!(json_tag(&request), request.name());
            // And the name alone selects the variant when reading JSON back.
            let json = serde_json::to_string(&request).unwrap();
            assert_eq!(serde_json::from_str::<Request>(&json).unwrap(), request);
        }
    }

    #[test]
    fn read_only_operations_are_exactly_the_ones_that_change_nothing_shared() {
        let read_only: Vec<&str> = OPERATIONS
            .iter()
            .filter(|operation| operation.read_only)
            .map(|operation| operation.name)
            .collect();
        assert_eq!(
            read_only,
            [
                "status",
                "session.show",
                "sessions",
                "goal.status",
                "board",
                "task.show",
                "event.show",
                "pending",
                "wait",
                "events",
                "notes",
                "doc.read",
                "blob.get",
                "blob.stat",
            ]
        );
        // A viewer may make exactly these, and every one of them is in the
        // audience a viewer reads as: its principal's.
        assert!(
            OPERATIONS
                .iter()
                .filter(|operation| operation.read_only)
                .all(|operation| operation.audience == Audience::Agent)
        );
    }

    #[test]
    fn owner_only_byte_carrying_and_session_operations_are_not_tools() {
        let not_tools: Vec<&str> = OPERATIONS
            .iter()
            .filter(|operation| !operation.tool)
            .map(|operation| operation.name)
            .collect();
        assert_eq!(
            not_tools,
            [
                "daemon.stop",
                "agent.enroll",
                "agent.grant",
                "agent.revoke",
                "session.report",
                "session.show",
                "sessions",
                "session.drop",
                "goal.grant",
                "task.authorize",
                "blob.put",
                "blob.get",
                "viewer.enroll",
            ]
        );
        let owner_only: Vec<&str> = OPERATIONS
            .iter()
            .filter(|operation| operation.audience == Audience::Owner)
            .map(|operation| operation.name)
            .collect();
        assert_eq!(
            owner_only,
            [
                "daemon.stop",
                "agent.enroll",
                "agent.grant",
                "agent.revoke",
                "goal.grant",
                "task.authorize",
                "viewer.enroll",
            ]
        );
        let coordinator: Vec<&str> = OPERATIONS
            .iter()
            .filter(|operation| operation.audience == Audience::Coordinator)
            .map(|operation| operation.name)
            .collect();
        assert_eq!(
            coordinator,
            [
                "goal.invite",
                "member.remove",
                "task.assign",
                "task.cancel",
                "result.accept",
                "result.reject",
                "doc.accept",
            ]
        );
    }

    #[test]
    fn every_goal_scoped_request_reports_exactly_its_goal() {
        let daemon_wide: Vec<&str> = requests()
            .iter()
            .filter(|request| request.goal().is_none())
            .map(Request::name)
            .collect();
        assert_eq!(
            daemon_wide,
            [
                "status",
                "daemon.stop",
                "agent.enroll",
                "agent.grant",
                "agent.revoke",
                "session.report",
                "session.show",
                "sessions",
                "session.drop",
                "goal.create",
                "goal.join",
                "viewer.enroll",
            ]
        );
        // Every other request names the goal it carries, not some other one.
        for request in requests() {
            let Some(goal) = request.goal() else { continue };
            assert_eq!(goal, self::goal(), "{}", request.name());
            let json = serde_json::to_value(&request).unwrap();
            assert_eq!(
                json[request.name()]["goal"],
                Value::String(goal.to_string()),
                "{}",
                request.name()
            );
        }
        for request in requests() {
            if request.goal().is_none() {
                assert!(!json_fields(&request).contains(&"goal".to_string()));
            }
        }
    }

    #[test]
    fn rendered_enums_use_their_stable_snake_case_names() {
        let codes = [
            ErrorCode::Denied,
            ErrorCode::NotFound,
            ErrorCode::Invalid,
            ErrorCode::Conflict,
            ErrorCode::ClaimHeld,
            ErrorCode::Superseded,
            ErrorCode::IdempotencyMismatch,
            ErrorCode::LimitExceeded,
            ErrorCode::Unavailable,
            ErrorCode::Halted,
            ErrorCode::UnsupportedVersion,
            ErrorCode::Corrupted,
            ErrorCode::Internal,
            ErrorCode::AuthorizationRequired,
            ErrorCode::ReadOnly,
        ];
        assert_eq!(tags(&codes), codes.map(ErrorCode::as_str));
        assert_eq!(
            tags(&codes),
            [
                "denied",
                "not_found",
                "invalid",
                "conflict",
                "claim_held",
                "superseded",
                "idempotency_mismatch",
                "limit_exceeded",
                "unavailable",
                "halted",
                "unsupported_version",
                "corrupted",
                "internal",
                "authorization_required",
                "read_only",
            ]
        );
        assert_eq!(
            tags(&[
                TaskState::Proposed,
                TaskState::Assigned,
                TaskState::Taken,
                TaskState::Declined,
                TaskState::Failed,
                TaskState::Submitted,
                TaskState::Accepted,
                TaskState::Rejected,
                TaskState::CancelRequested,
                TaskState::Cancelled(CancelOutcome::Uncertain),
            ]),
            [
                "proposed",
                "assigned",
                "taken",
                "declined",
                "failed",
                "submitted",
                "accepted",
                "rejected",
                "cancel_requested",
                "cancelled",
            ]
        );
        assert_eq!(
            tags(&[Caller::Owner, Caller::Agent(key(2)), Caller::Viewer(key(2))]),
            ["owner", "agent", "viewer"]
        );
        assert_eq!(
            tags(&[
                ServerHello::Welcome {
                    api_version: API_VERSION,
                    daemon_version: "0.1.0".to_string(),
                    caller: Caller::Owner,
                    max_blob_bytes: MAX_BLOB_BYTES as u64,
                },
                ServerHello::Refused {
                    error: ApiError::new(ErrorCode::Denied, "credential is not known"),
                    api_version: API_VERSION,
                    daemon_version: "0.1.0".to_string(),
                },
            ]),
            ["welcome", "refused"]
        );
        assert_eq!(
            tags(&[
                WaitOutcome::Work(PendingWork::default()),
                WaitOutcome::NoEvent,
                WaitOutcome::Disconnected,
            ]),
            ["work", "no_event", "disconnected"]
        );
        assert_eq!(
            tags(&[Standing::Effective, Standing::Pending, Standing::Excluded]),
            ["effective", "pending", "excluded"]
        );
        assert_eq!(
            tags(&[
                Membership::Joining,
                Membership::Member,
                Membership::Removed,
                Membership::Left,
                Membership::Refused,
            ]),
            ["joining", "member", "removed", "left", "refused"]
        );
        assert_eq!(
            tags(&[Halt::AuthorityConflict, Halt::SignerRecovery]),
            ["authority_conflict", "signer_recovery"]
        );
        assert_eq!(
            tags(&[
                SessionState::Launching,
                SessionState::Started,
                SessionState::Ready,
                SessionState::Blocked,
                SessionState::Exited,
                SessionState::Unknown,
            ]),
            [
                "launching",
                "started",
                "ready",
                "blocked",
                "exited",
                "unknown"
            ]
        );
        assert_eq!(
            tags(&[
                BlobState::Held,
                BlobState::Requested,
                BlobState::Unavailable,
                BlobState::Unknown,
            ]),
            ["held", "requested", "unavailable", "unknown"]
        );
        assert_eq!(
            tags(&responses()),
            [
                "done",
                "status",
                "agent_enrolled",
                "goal_created",
                "invited",
                "joined",
                "goal_status",
                "board",
                "task",
                "event",
                "recorded",
                "claimed",
                "pending",
                "waited",
                "events",
                "notes",
                "doc",
                "blob_stored",
                "blob",
                "blob_states",
                "session",
                "sessions",
            ]
        );
    }

    #[test]
    fn responses_render_as_json_with_hex_identifiers() {
        let response = Response::Recorded {
            event: EventId([0xab; 32]),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(
            json,
            format!("{{\"recorded\":{{\"event\":\"{}\"}}}}", "ab".repeat(32))
        );
        let error = ApiError::new(ErrorCode::ClaimHeld, "another session holds the claim");
        assert_eq!(
            serde_json::to_string(&error).unwrap(),
            r#"{"code":"claim_held","message":"another session holds the claim"}"#
        );
        assert_eq!(
            error.to_string(),
            "claim_held: another session holds the claim"
        );
    }

    #[test]
    fn every_request_has_exactly_one_kind_of_answer() {
        let responses = responses();
        for request in requests() {
            let kinds = responses
                .iter()
                .filter(|response| request.is_answered_by(response))
                .count();
            assert_eq!(kinds, 1, "{}", request.name());
        }
        // And no kind of response is left without a request it answers.
        for response in &responses {
            assert!(
                requests()
                    .iter()
                    .any(|request| request.is_answered_by(response)),
                "{response:?}"
            );
        }
    }

    #[test]
    fn frames_round_trip_in_the_binary_encoding() {
        for (id, request) in (1..).zip(requests()) {
            let frame = RequestFrame {
                id,
                idempotency: Some(IdempotencyKey([3; 16])),
                on_behalf: Some(key(2)),
                request,
            };
            let bytes = codec::encode(&frame).unwrap();
            assert_eq!(codec::decode::<RequestFrame>(&bytes), Ok(frame));
        }
        for (id, response) in (1..).zip(responses()) {
            let frame = ResponseFrame {
                id,
                result: Ok(response),
            };
            let bytes = codec::encode(&frame).unwrap();
            assert_eq!(codec::decode::<ResponseFrame>(&bytes), Ok(frame));
        }
        let refusal = ResponseFrame {
            id: 7,
            result: Err(ApiError::new(
                ErrorCode::Superseded,
                "generation 1 was taken over",
            )),
        };
        let bytes = codec::encode(&refusal).unwrap();
        assert_eq!(codec::decode::<ResponseFrame>(&bytes), Ok(refusal));
    }

    #[test]
    fn a_request_is_read_from_json_by_its_operation_name_and_refuses_unknown_fields() {
        let (goal, assignment) = (goal(), event(0x21));
        let json = format!(r#"{{"task.claim":{{"goal":"{goal}","assignment":"{assignment}"}}}}"#);
        assert_eq!(
            serde_json::from_str::<Request>(&json).unwrap(),
            Request::TaskClaim { goal, assignment }
        );
        assert_eq!(
            serde_json::from_str::<Request>(r#""status""#).unwrap(),
            Request::Status
        );
        // An optional field may be left out.
        let json = format!(r#"{{"events":{{"goal":"{goal}","limit":20}}}}"#);
        assert_eq!(
            serde_json::from_str::<Request>(&json).unwrap(),
            Request::Events {
                goal,
                after: None,
                limit: 20
            }
        );

        // A misspelled or surplus field is refused, not silently dropped. In
        // particular nobody can name a session in a claim request.
        let instance = SESSION.instance();
        let json = format!(
            r#"{{"task.claim":{{"goal":"{goal}","assignment":"{assignment}","instance":"{instance}"}}}}"#
        );
        assert!(serde_json::from_str::<Request>(&json).is_err());
        let json = format!(r#"{{"events":{{"goal":"{goal}","limit":20,"afterr":3}}}}"#);
        assert!(serde_json::from_str::<Request>(&json).is_err());
        // Only the operation name selects a request.
        assert!(serde_json::from_str::<Request>(r#""Status""#).is_err());
        assert!(serde_json::from_str::<Request>(r#""task.steal""#).is_err());

        let frame = r#"{"id":1,"request":"status"}"#;
        assert_eq!(
            serde_json::from_str::<RequestFrame>(frame).unwrap(),
            RequestFrame {
                id: 1,
                idempotency: None,
                on_behalf: None,
                request: Request::Status,
            }
        );
        let frame = r#"{"id":1,"request":"status","as":"owner"}"#;
        assert!(serde_json::from_str::<RequestFrame>(frame).is_err());
    }

    #[test]
    fn a_request_is_checked_against_the_published_limits() {
        for request in requests() {
            assert_eq!(request.check(), Ok(()), "{}", request.name());
        }
        let code = |request: Request| request.check().unwrap_err().code;
        let goal = goal();

        let enroll = |name: &str| Request::AgentEnroll {
            name: name.to_string(),
            grants: Grants::default(),
            credential: [0; 32],
        };
        assert_eq!(code(enroll("Worker")), ErrorCode::Invalid);
        assert_eq!(code(enroll("")), ErrorCode::Invalid);
        assert_eq!(enroll(&"a".repeat(MAX_AGENT_NAME_BYTES)).check(), Ok(()));

        let propose = |dependencies: usize| Request::TaskPropose {
            goal,
            text: String::new(),
            input: None,
            depends_on: vec![event(1); dependencies],
            deadline_ms: None,
            max_attempts: None,
        };
        assert_eq!(propose(MAX_DEPENDENCIES).check(), Ok(()));
        assert_eq!(
            code(propose(MAX_DEPENDENCIES + 1)),
            ErrorCode::LimitExceeded
        );

        let submit = |artifacts: usize| Request::TaskSubmit {
            goal,
            assignment: event(1),
            generation: 1,
            summary: String::new(),
            base: None,
            patch: None,
            artifacts: vec![blob(1); artifacts],
        };
        assert_eq!(submit(MAX_ARTIFACTS).check(), Ok(()));
        assert_eq!(code(submit(MAX_ARTIFACTS + 1)), ErrorCode::LimitExceeded);

        let stat = |hashes: usize| Request::BlobStat {
            goal,
            hashes: vec![blob(1); hashes],
        };
        assert_eq!(stat(MAX_STAT_HASHES).check(), Ok(()));
        assert_eq!(code(stat(MAX_STAT_HASHES + 1)), ErrorCode::LimitExceeded);

        let report = |change: fn(&mut SessionRecord)| {
            let mut record = record(SessionState::Launching);
            change(&mut record);
            Request::SessionReport { record }
        };
        assert_eq!(
            report(|record| record.detail = vec![0; MAX_SESSION_DETAIL_BYTES]).check(),
            Ok(())
        );
        assert_eq!(
            code(report(
                |record| record.detail = vec![0; MAX_SESSION_DETAIL_BYTES + 1]
            )),
            ErrorCode::LimitExceeded
        );
        assert_eq!(
            code(report(
                |record| record.client = "c".repeat(MAX_LOCAL_TEXT_BYTES + 1)
            )),
            ErrorCode::LimitExceeded
        );
        assert_eq!(
            code(report(|record| {
                record.client_session = Some("s".repeat(MAX_LOCAL_TEXT_BYTES + 1));
            })),
            ErrorCode::LimitExceeded
        );
        assert_eq!(
            report(|record| record.client_session = Some("s".repeat(MAX_LOCAL_TEXT_BYTES))).check(),
            Ok(())
        );

        let bind = |change: fn(&mut WorkspaceBinding)| {
            let mut binding = binding();
            change(&mut binding);
            Request::WorkspaceSet { goal, binding }
        };
        assert_eq!(
            code(bind(
                |binding| binding.export_root = Some("p".repeat(MAX_LOCAL_TEXT_BYTES + 1))
            )),
            ErrorCode::LimitExceeded
        );
        assert_eq!(
            code(bind(
                |binding| binding.source_commit = Some("p".repeat(MAX_LOCAL_TEXT_BYTES + 1))
            )),
            ErrorCode::LimitExceeded
        );
        assert_eq!(
            code(bind(
                |binding| binding.destination = Some("p".repeat(MAX_LOCAL_TEXT_BYTES + 1))
            )),
            ErrorCode::LimitExceeded
        );
        assert_eq!(
            bind(|binding| binding.destination = Some("p".repeat(MAX_LOCAL_TEXT_BYTES))).check(),
            Ok(())
        );
        assert_eq!(
            bind(|binding| *binding = WorkspaceBinding::default()).check(),
            Ok(())
        );
    }

    #[test]
    fn a_principal_name_is_lowercase_letters_digits_and_hyphens() {
        for name in ["a", "worker", "worker-2", "0", "-", &"a".repeat(32)] {
            assert!(is_agent_name(name), "{name:?}");
        }
        for name in [
            "",
            "Worker",
            "work er",
            "worker_1",
            "worker.1",
            "wörker",
            "../x",
            "a/b",
            "worker\n",
            &"a".repeat(33),
        ] {
            assert!(!is_agent_name(name), "{name:?}");
        }
    }

    #[test]
    fn a_hello_from_another_api_version_is_refused_whatever_its_layout() {
        // A future layout: the version, then fields this version knows
        // nothing about.
        for version in [API_VERSION + 1, 300, u16::MAX] {
            let frame = codec::encode(&(version, "a future layout", 7u64)).unwrap();
            assert_eq!(ClientHello::api_version_of(&frame), Some(version));
            assert_eq!(
                ClientHello::decode(&frame).unwrap_err().code,
                ErrorCode::UnsupportedVersion
            );
        }
        // The version decides, even when the rest would decode.
        let newer = ClientHello {
            api_version: API_VERSION + 1,
            credential: CREDENTIAL,
            session: None,
        };
        assert_eq!(
            ClientHello::decode(&codec::encode(&newer).unwrap())
                .unwrap_err()
                .code,
            ErrorCode::UnsupportedVersion
        );

        let hello = ClientHello {
            api_version: API_VERSION,
            credential: CREDENTIAL,
            session: Some(SESSION),
        };
        let frame = codec::encode(&hello).unwrap();
        // The version, the credential, the option tag and the session secret.
        assert_eq!(frame.len(), 1 + 32 + 1 + 32);
        assert_eq!(ClientHello::api_version_of(&frame), Some(API_VERSION));
        assert_eq!(ClientHello::decode(&frame), Ok(hello));

        let mut trailing = frame.clone();
        trailing.push(0);
        for damaged in [
            &frame[..frame.len() - 1],
            &trailing[..],
            &[][..],
            &[0xff, 0xff, 0xff, 0xff][..],
        ] {
            assert_eq!(
                ClientHello::decode(damaged).unwrap_err().code,
                ErrorCode::Invalid
            );
        }
        assert_eq!(ClientHello::api_version_of(&[]), None);
    }

    #[test]
    fn a_session_is_named_by_a_handle_derived_from_its_secret() {
        let instance = SESSION.instance();
        assert_eq!(instance.to_string(), "2f7f582dd77ae53ab4507baefb97d5e5");
        let digest = crypto::domain_hash(domain::SESSION_INSTANCE, &SESSION.0);
        assert_eq!(instance.0[..], digest[..InstanceId::LEN]);
        // Another secret is another session; the handle does not lead back
        // to the secret, and it is not the credential's digest domain.
        assert_ne!(instance, SessionSecret([0x6c; 32]).instance());
        assert_ne!(instance.0[..], SESSION.0[..InstanceId::LEN]);
        assert_ne!(
            instance.0[..],
            Credential(SESSION.0).digest()[..InstanceId::LEN]
        );

        assert_eq!(codec::encode(&SESSION).unwrap(), vec![0x6b; 32]);
        assert_eq!(codec::decode::<SessionSecret>(&[0x6b; 32]), Ok(SESSION));
        assert_eq!(codec::encode(&CREDENTIAL).unwrap(), vec![0x5a; 32]);
        assert_eq!(codec::decode::<Credential>(&[0x5a; 32]), Ok(CREDENTIAL));
    }

    #[test]
    fn no_rendering_of_a_hello_request_or_response_shows_a_secret() {
        fn assert_hides(rendering: &str) {
            for secret in [
                "5a5a5a",
                "6b6b6b",
                "7c7c7c",
                "90, 90, 90",
                "90,90,90",
                "107, 107, 107",
                "107,107,107",
                "124, 124, 124",
                "124,124,124",
            ] {
                assert!(!rendering.contains(secret), "a secret shows in {rendering}");
            }
        }

        let hello = ClientHello {
            api_version: API_VERSION,
            credential: CREDENTIAL,
            session: Some(SESSION),
        };
        assert_eq!(
            format!("{hello:?}"),
            format!(
                "ClientHello {{ api_version: {API_VERSION}, credential: Credential(..), \
                 session: Some(SessionSecret(..)) }}"
            )
        );
        let json = serde_json::to_string(&hello).unwrap();
        assert_eq!(
            json,
            format!(
                r#"{{"api_version":{API_VERSION},"credential":"<redacted>","session":"<redacted>"}}"#
            )
        );
        // What was rendered can never be read back as the real thing.
        assert!(serde_json::from_str::<ClientHello>(&json).is_err());
        for text in ["5a", "6b"] {
            let hex = format!("\"{}\"", text.repeat(32));
            assert!(serde_json::from_str::<Credential>(&hex).is_err());
            assert!(serde_json::from_str::<SessionSecret>(&hex).is_err());
        }

        // A ticket is the one capability JSON shows, because showing it is
        // how it is handed over. Debug output never shows it.
        let ticket = ticket();
        assert!(ticket.as_str().contains("7c7c7c"));
        for request in requests() {
            let frame = RequestFrame {
                id: 1,
                idempotency: None,
                on_behalf: None,
                request,
            };
            assert_hides(&format!("{frame:?}"));
            let json = serde_json::to_string(&frame).unwrap();
            if matches!(frame.request, Request::GoalJoin { .. }) {
                assert!(json.contains(ticket.as_str()));
            } else {
                assert_hides(&json);
            }
        }
        for response in responses() {
            let frame = ResponseFrame {
                id: 1,
                result: Ok(response),
            };
            assert_hides(&format!("{frame:?}"));
            let json = serde_json::to_string(&frame).unwrap();
            if matches!(frame.result, Ok(Response::Invited { .. })) {
                assert!(json.contains(ticket.as_str()));
            } else {
                assert_hides(&json);
            }
        }
        for answer in [
            ServerHello::Welcome {
                api_version: API_VERSION,
                daemon_version: "0.1.0".to_string(),
                caller: Caller::Agent(key(2)),
                max_blob_bytes: MAX_BLOB_BYTES as u64,
            },
            ServerHello::Refused {
                error: ApiError::new(ErrorCode::Denied, "credential is not known"),
                api_version: API_VERSION,
                daemon_version: "0.1.0".to_string(),
            },
        ] {
            assert_hides(&format!("{answer:?}"));
            assert_hides(&serde_json::to_string(&answer).unwrap());
        }

        // Enrollment carries the digest of the credential, never the secret,
        // and its answer carries no credential at all.
        let enroll = Request::AgentEnroll {
            name: "worker".to_string(),
            grants: Grants::default(),
            credential: CREDENTIAL.digest(),
        };
        assert_ne!(CREDENTIAL.digest(), CREDENTIAL.0);
        assert_hides(&serde_json::to_string(&enroll).unwrap());
        let viewer = Request::ViewerEnroll {
            agent: key(2),
            credential: CREDENTIAL.digest(),
        };
        assert_hides(&serde_json::to_string(&viewer).unwrap());
        assert!(viewer.is_answered_by(&Response::Done));
        let enrolled = serde_json::to_value(Response::AgentEnrolled { agent: key(2) }).unwrap();
        let fields: Vec<&String> = enrolled["agent_enrolled"]
            .as_object()
            .unwrap()
            .keys()
            .collect();
        assert_eq!(fields, ["agent"]);
    }

    #[test]
    fn claim_bound_requests_name_a_generation_and_never_a_session() {
        let mut naming_an_instance = Vec::new();
        for request in requests() {
            let (name, fields) = (request.name(), json_fields(&request));
            let claim_bound = matches!(
                name,
                "task.progress" | "task.submit" | "task.fail" | "cancel.acknowledge"
            );
            assert_eq!(
                fields.contains(&"generation".to_string()),
                claim_bound,
                "{name}"
            );
            if fields.contains(&"instance".to_string()) {
                naming_an_instance.push(name);
            }
        }
        // Only reading or deleting a session record names a session, and
        // neither proves anything: the secret in the hello does.
        assert_eq!(naming_an_instance, ["session.show", "session.drop"]);
        assert_eq!(
            json_fields(&Request::TaskClaim {
                goal: goal(),
                assignment: event(0x21),
            }),
            ["assignment", "goal"]
        );
    }

    #[test]
    fn contract_errors_map_onto_api_error_codes() {
        let code = |error: ApiError| error.code;

        let failed = StoreError::Failed("disk full".to_string());
        assert_eq!(code(failed.clone().into()), ErrorCode::Internal);
        assert_eq!(ApiError::from(failed.clone()).message, failed.to_string());
        assert_eq!(
            code(StoreError::Corrupted("bad row".to_string()).into()),
            ErrorCode::Corrupted
        );

        assert_eq!(
            code(EventError::UnsupportedVersion(7).into()),
            ErrorCode::UnsupportedVersion
        );
        assert_eq!(code(EventError::IdMismatch.into()), ErrorCode::Corrupted);
        for error in [
            EventError::TooLarge,
            EventError::Malformed,
            EventError::NotCanonical,
            EventError::BadSequence,
            EventError::BadReferences,
            EventError::BadAnchor,
            EventError::BadPayloadLength,
            EventError::AuthorMismatch,
            EventError::BadSignature,
        ] {
            assert_eq!(code(error.into()), ErrorCode::Invalid, "{error:?}");
        }

        for error in [
            ManifestError::Malformed,
            ManifestError::TooManyEntries,
            ManifestError::UnsafePath,
            ManifestError::Unordered,
            ManifestError::FileUnderFile,
        ] {
            assert_eq!(code(error.into()), ErrorCode::Invalid, "{error:?}");
        }

        assert_eq!(
            code(InviteError::UnsupportedVersion(7).into()),
            ErrorCode::UnsupportedVersion
        );
        for error in [
            InviteError::NotATicket,
            InviteError::TooLong,
            InviteError::Malformed,
            InviteError::BadHints,
        ] {
            assert_eq!(code(error.into()), ErrorCode::Invalid, "{error:?}");
        }
        assert_eq!(
            ApiError::from(InviteError::NotATicket).to_string(),
            "invalid: text is not a Locust invitation"
        );

        assert_eq!(code(CodecError::Encode.into()), ErrorCode::Internal);
        for error in [
            CodecError::Decode,
            CodecError::TrailingBytes,
            CodecError::NotCanonical,
        ] {
            assert_eq!(code(error.into()), ErrorCode::Invalid, "{error:?}");
        }
    }

    #[test]
    fn pending_work_is_empty_only_when_every_list_is() {
        assert!(PendingWork::default().is_empty());
        assert!(
            PendingWork {
                revision: 9,
                ..PendingWork::default()
            }
            .is_empty()
        );
        let reference = AssignmentRef {
            task: event(0x20),
            assignment: event(0x21),
        };
        let held = claim(event(0x21), SESSION, 1);
        let changes: [fn(&mut PendingWork, AssignmentRef, Claim); 6] = [
            |work, reference, _| work.to_authorize.push(reference),
            |work, reference, _| work.to_claim.push(reference),
            |work, _, held| work.claimed.push(held),
            |work, _, held| work.held_elsewhere.push(held),
            |work, reference, _| {
                work.to_acknowledge.push(CancelItem {
                    task: reference.task,
                    assignment: reference.assignment,
                    cancel: reference.task,
                    generation: None,
                });
            },
            |work, reference, _| {
                work.to_review.push(ReviewItem {
                    task: reference.task,
                    result: reference.assignment,
                });
            },
        ];
        for change in changes {
            let mut work = PendingWork::default();
            change(&mut work, reference, held);
            assert!(!work.is_empty(), "{work:?}");
        }
    }

    // The transcripts below carry out whole flows with public operations
    // only. They are types only: no daemon runs. Each step builds the request
    // frame and the response frame a daemon following this contract would
    // send, passes both through their binary encoding, and checks that the
    // caller is one the operation admits and that the answer is of the kind
    // the request gets.

    const OWNER_CREDENTIAL: Credential = Credential([0x50; 32]);
    const LEAD_CREDENTIAL: Credential = Credential([0x51; 32]);
    const WORKER_CREDENTIAL: Credential = Credential([0x52; 32]);
    const VIEWER_CREDENTIAL: Credential = Credential([0x53; 32]);
    const LEAD: PublicKey = PublicKey([1; 32]);
    const WORKER: PublicKey = PublicKey([2; 32]);
    const SESSION_A: SessionSecret = SessionSecret([0xa1; 32]);
    const SESSION_B: SessionSecret = SessionSecret([0xb2; 32]);

    /// One connection, as its hello established it.
    struct Connection {
        caller: Caller,
        session: Option<SessionSecret>,
    }

    /// Opens a connection: the hello and its welcome, each through its frame.
    fn connect(
        credential: Credential,
        session: Option<SessionSecret>,
        caller: Caller,
    ) -> Connection {
        let hello = ClientHello {
            api_version: API_VERSION,
            credential,
            session,
        };
        let frame = codec::encode(&hello).unwrap();
        assert!(frame.len() <= MAX_HELLO_FRAME_BYTES);
        assert_eq!(ClientHello::decode(&frame), Ok(hello));
        assert!(
            session.is_none() || !matches!(caller, Caller::Viewer(_)),
            "a viewer with a session is refused, not welcomed"
        );

        let welcome = ServerHello::Welcome {
            api_version: API_VERSION,
            daemon_version: "0.1.0".to_string(),
            caller,
            max_blob_bytes: MAX_BLOB_BYTES as u64,
        };
        let frame = codec::encode(&welcome).unwrap();
        assert!(frame.len() <= MAX_HELLO_FRAME_BYTES);
        assert_eq!(codec::decode::<ServerHello>(&frame), Ok(welcome));
        Connection { caller, session }
    }

    /// A hello the daemon refuses, and the refusal, each through its frame.
    fn refuse_hello(credential: Credential, session: Option<SessionSecret>, code: ErrorCode) {
        let hello = ClientHello {
            api_version: API_VERSION,
            credential,
            session,
        };
        let frame = codec::encode(&hello).unwrap();
        assert_eq!(ClientHello::decode(&frame), Ok(hello));

        let refusal = ServerHello::Refused {
            error: ApiError::new(code, "refused in this transcript"),
            api_version: API_VERSION,
            daemon_version: "0.1.0".to_string(),
        };
        let frame = codec::encode(&refusal).unwrap();
        assert!(frame.len() <= MAX_HELLO_FRAME_BYTES);
        assert_eq!(codec::decode::<ServerHello>(&frame), Ok(refusal));
    }

    fn owner() -> Connection {
        connect(OWNER_CREDENTIAL, None, Caller::Owner)
    }

    fn lead() -> Connection {
        connect(LEAD_CREDENTIAL, None, Caller::Agent(LEAD))
    }

    /// The worker principal: a plain CLI connection, or one of its sessions.
    fn worker(session: Option<SessionSecret>) -> Connection {
        connect(WORKER_CREDENTIAL, session, Caller::Agent(WORKER))
    }

    fn recorded(event: EventId) -> Result<Response, ApiError> {
        Ok(Response::Recorded { event })
    }

    fn refused(code: ErrorCode) -> Result<Response, ApiError> {
        Err(ApiError::new(code, "refused in this transcript"))
    }

    /// True for requests that only an execution session can make.
    fn needs_session(request: &Request) -> bool {
        matches!(
            request,
            Request::SessionReport { .. }
                | Request::Session { instance: None }
                | Request::TaskClaim { .. }
                | Request::TaskTakeover { .. }
                | Request::TaskProgress { .. }
                | Request::TaskSubmit { .. }
                | Request::TaskFail { .. }
                | Request::CancelAcknowledge {
                    generation: Some(_),
                    ..
                }
        )
    }

    #[derive(Default)]
    struct Transcript {
        last_id: u64,
        names: Vec<&'static str>,
    }

    impl Transcript {
        /// One request and its answer as they cross the socket. Returns the
        /// answer as the client decoded it.
        fn exchange(
            &mut self,
            connection: &Connection,
            on_behalf: Option<PublicKey>,
            request: Request,
            result: Result<Response, ApiError>,
        ) -> Result<Response, ApiError> {
            let operation = request.operation();
            let name = operation.name;
            assert_eq!(request.check(), Ok(()), "{name}");

            if let Ok(response) = &result {
                assert!(request.is_answered_by(response), "{name}");
                let admitted = match (connection.caller, operation.audience) {
                    (Caller::Owner, Audience::Owner) => on_behalf.is_none(),
                    (Caller::Owner, Audience::Agent | Audience::Coordinator) => {
                        on_behalf.is_some() || operation.read_only || name == "session.drop"
                    }
                    (Caller::Agent(_), Audience::Owner) => false,
                    (Caller::Agent(_), Audience::Agent | Audience::Coordinator) => {
                        on_behalf.is_none()
                    }
                    // Whatever the audience: a viewer reads and does nothing
                    // else, and never names a principal to act for.
                    (Caller::Viewer(_), _) => on_behalf.is_none() && operation.read_only,
                };
                assert!(admitted, "{name} is not this caller's to make");
                if operation.audience == Audience::Coordinator {
                    let acting = match connection.caller {
                        Caller::Owner => on_behalf,
                        Caller::Agent(principal) | Caller::Viewer(principal) => Some(principal),
                    };
                    assert_eq!(acting, Some(LEAD), "{name} is the coordinator's");
                }
                assert!(
                    connection.session.is_some() || !needs_session(&request),
                    "{name} needs a session in the hello"
                );
            }

            self.last_id += 1;
            let frame = RequestFrame {
                id: self.last_id,
                idempotency: None,
                on_behalf,
                request,
            };
            let bytes = codec::encode(&frame).unwrap();
            assert_eq!(codec::decode::<RequestFrame>(&bytes).as_ref(), Ok(&frame));
            let json = serde_json::to_string(&frame).unwrap();
            assert_eq!(serde_json::from_str::<RequestFrame>(&json).unwrap(), frame);

            let answer = ResponseFrame {
                id: frame.id,
                result,
            };
            let bytes = codec::encode(&answer).unwrap();
            let received: ResponseFrame = codec::decode(&bytes).unwrap();
            assert_eq!(received, answer);
            serde_json::to_string(&received).unwrap();

            self.names.push(name);
            received.result
        }
    }

    #[test]
    fn the_first_task_flow_uses_public_operations_only() {
        let mut t = Transcript::default();
        let goal = goal();
        let (owner, lead) = (owner(), lead());
        let (worker_a, worker_b) = (worker(Some(SESSION_A)), worker(Some(SESSION_B)));
        let base = blob(0x30);

        // The owner enrolls two principals. Each credential was minted and
        // written to its file by the client; only its digest is sent, and the
        // answer carries the new key and no secret.
        for (name, credential, agent) in [
            ("lead", LEAD_CREDENTIAL, LEAD),
            ("worker", WORKER_CREDENTIAL, WORKER),
        ] {
            let enroll = Request::AgentEnroll {
                name: name.to_string(),
                grants: Grants { manage_goals: true },
                credential: credential.digest(),
            };
            t.exchange(&owner, None, enroll, Ok(Response::AgentEnrolled { agent }))
                .unwrap();
        }

        // The lead creates a goal, records where it shares from and invites.
        let create = Request::GoalCreate {
            title: "Ship the login page".to_string(),
        };
        t.exchange(&lead, None, create, Ok(Response::GoalCreated { goal }))
            .unwrap();
        let set = Request::WorkspaceSet {
            goal,
            binding: WorkspaceBinding {
                export_root: Some("/home/lead/app".to_string()),
                source_commit: Some("4c7b680".to_string()),
                exported: Some(base),
                destination: None,
                integrated: Some(base),
            },
        };
        t.exchange(&lead, None, set, Ok(Response::Done)).unwrap();
        let invite = Request::GoalInvite {
            goal,
            expires_ms: None,
        };
        let invited = Ok(Response::Invited { ticket: ticket() });
        let Ok(Response::Invited { ticket }) = t.exchange(&lead, None, invite, invited) else {
            panic!("the invitation carries a ticket");
        };

        // The worker joins: pending first, admitted when asked again, and is
        // then shown what it joined. Joining granted nothing.
        for membership in [Membership::Joining, Membership::Member] {
            let join = Request::GoalJoin {
                ticket: ticket.clone(),
            };
            let joined = Response::Joined {
                goal,
                coordinator: LEAD,
                membership,
            };
            t.exchange(&worker_a, None, join, Ok(joined)).unwrap();
        }
        let status = Response::GoalStatus(goal_status());
        let Ok(Response::GoalStatus(joined)) =
            t.exchange(&worker_a, None, Request::GoalStatus { goal }, Ok(status))
        else {
            panic!("the worker is shown the goal");
        };
        assert_eq!(joined.coordinator, LEAD);
        assert_eq!(joined.title.as_deref(), Some("Ship the login page"));
        assert_eq!(joined.grants, GoalGrants::default());

        // Propose and assign.
        let (task, assignment) = (event(0x20), event(0x21));
        let propose = Request::TaskPropose {
            goal,
            text: "Add a login page".to_string(),
            input: Some(base),
            depends_on: Vec::new(),
            deadline_ms: None,
            max_attempts: Some(2),
        };
        t.exchange(&lead, None, propose, recorded(task)).unwrap();
        let assign = Request::TaskAssign {
            goal,
            task,
            assignee: WORKER,
        };
        t.exchange(&lead, None, assign, recorded(assignment))
            .unwrap();

        // No grant covers the assignment, so it waits for the owner, who
        // authorizes this one assignment.
        let offered = AssignmentRef { task, assignment };
        let waiting = PendingWork {
            revision: 5,
            to_authorize: vec![offered],
            ..PendingWork::default()
        };
        let pending = Ok(Response::Pending(waiting));
        let Ok(Response::Pending(waiting)) =
            t.exchange(&worker_a, None, Request::Pending { goal }, pending)
        else {
            panic!("pending work is answered");
        };
        let claim_request = Request::TaskClaim { goal, assignment };
        let unauthorized = t.exchange(
            &worker_a,
            None,
            claim_request.clone(),
            refused(ErrorCode::AuthorizationRequired),
        );
        assert_eq!(
            unauthorized.unwrap_err().code,
            ErrorCode::AuthorizationRequired
        );
        let authorize = Request::TaskAuthorize {
            goal,
            assignment,
            takeover: false,
        };
        t.exchange(&owner, None, authorize, Ok(Response::Done))
            .unwrap();

        // Waiting from the revision it saw, the worker learns at once that
        // the assignment is now its to claim.
        let wait = Request::Wait {
            goal,
            seen: waiting.revision,
            timeout_ms: 30_000,
        };
        let claimable = WaitOutcome::Work(PendingWork {
            revision: 6,
            to_claim: vec![offered],
            ..PendingWork::default()
        });
        let Ok(Response::Waited(WaitOutcome::Work(work))) =
            t.exchange(&worker_a, None, wait, Ok(Response::Waited(claimable)))
        else {
            panic!("the wait reports work");
        };
        assert_eq!(work.to_claim, [offered]);
        assert_ne!(work.revision, waiting.revision);

        // The claim. Its response is lost; the same request again returns
        // the same claim with the same generation.
        let granted = Claim {
            goal,
            task,
            assignment,
            instance: SESSION_A.instance(),
            generation: 1,
        };
        let lost = t.exchange(
            &worker_a,
            None,
            claim_request.clone(),
            Ok(Response::Claimed(granted)),
        );
        let retried = t.exchange(
            &worker_a,
            None,
            claim_request,
            Ok(Response::Claimed(granted)),
        );
        assert_eq!(lost, retried);
        let Ok(Response::Claimed(held)) = retried else {
            panic!("the claim is answered");
        };

        // Progress, content, and the result with its base, patch and artifacts.
        let progress = Request::TaskProgress {
            goal,
            assignment,
            generation: held.generation,
            text: "The form renders".to_string(),
        };
        t.exchange(&worker_a, None, progress, recorded(event(0x22)))
            .unwrap();
        let mut stored = Vec::new();
        for (bytes, hash) in [
            (&b"diff --git"[..], blob(0x31)),
            (&b"test log"[..], blob(0x32)),
        ] {
            let put = Request::BlobPut {
                goal,
                bytes: bytes.to_vec(),
            };
            let Ok(Response::BlobStored { hash }) =
                t.exchange(&worker_a, None, put, Ok(Response::BlobStored { hash }))
            else {
                panic!("the content is stored");
            };
            stored.push(hash);
        }
        let (patch, artifact) = (stored[0], stored[1]);
        let result = event(0x23);
        let submit = Request::TaskSubmit {
            goal,
            assignment,
            generation: held.generation,
            summary: "Login page added; tests pass.".to_string(),
            base: Some(base),
            patch: Some(patch),
            artifacts: vec![artifact],
        };
        t.exchange(&worker_a, None, submit, recorded(result))
            .unwrap();

        // The coordinator finds the result, reads all of it, checks that its
        // content is there, and only then accepts it with a new head.
        let review = PendingWork {
            revision: 9,
            to_review: vec![ReviewItem { task, result }],
            ..PendingWork::default()
        };
        let Ok(Response::Pending(review)) = t.exchange(
            &lead,
            None,
            Request::Pending { goal },
            Ok(Response::Pending(review)),
        ) else {
            panic!("pending work is answered");
        };
        let show = Request::Event {
            goal,
            event: review.to_review[0].result,
        };
        let detail = Ok(Response::Event(result_detail(result, assignment)));
        let Ok(Response::Event(detail)) = t.exchange(&lead, None, show, detail) else {
            panic!("the result is shown");
        };
        assert_eq!(detail.view.author, WORKER);
        assert_eq!(detail.view.standing, Standing::Effective);
        assert_eq!(detail.task, Some(task));
        assert_eq!(
            detail.text.as_deref(),
            Some("Login page added; tests pass.")
        );
        let Body::ResultSubmitted {
            assignment: submitted_for,
            base: submitted_base,
            patch: Some(submitted_patch),
            artifacts,
        } = &detail.body
        else {
            panic!("the body is the typed result");
        };
        assert_eq!(
            (*submitted_for, *submitted_base, *submitted_patch),
            (assignment, Some(base), patch)
        );
        assert_eq!(artifacts, &[artifact]);
        let payload = detail.payload.expect("the summary is a payload");
        let named: Vec<BlobHash> = detail.content.iter().map(|status| status.hash).collect();
        assert_eq!(named, [payload.hash, base, patch, artifact]);

        let stat = Request::BlobStat {
            goal,
            hashes: vec![patch, artifact],
        };
        let states = vec![
            BlobStatus {
                hash: patch,
                state: BlobState::Held,
            },
            BlobStatus {
                hash: artifact,
                state: BlobState::Requested,
            },
        ];
        let Ok(Response::BlobStates(states)) =
            t.exchange(&lead, None, stat, Ok(Response::BlobStates(states)))
        else {
            panic!("availability is answered");
        };
        assert_eq!(states[0].state, BlobState::Held);
        // The artifact has not arrived: asking for it fails at once.
        let missing = t.exchange(
            &lead,
            None,
            Request::BlobGet {
                goal,
                hash: artifact,
            },
            refused(ErrorCode::Unavailable),
        );
        assert_eq!(missing.unwrap_err().code, ErrorCode::Unavailable);
        let fetched = Response::Blob {
            bytes: b"diff --git".to_vec(),
        };
        t.exchange(
            &lead,
            None,
            Request::BlobGet { goal, hash: patch },
            Ok(fetched),
        )
        .unwrap();

        let head = blob(0x34);
        let put = Request::BlobPut {
            goal,
            bytes: b"manifest with the patch applied".to_vec(),
        };
        t.exchange(&lead, None, put, Ok(Response::BlobStored { hash: head }))
            .unwrap();
        let accept = Request::ResultAccept {
            goal,
            result,
            head: Some(head),
        };
        let accepted = event(0x29);
        t.exchange(&lead, None, accept, recorded(accepted)).unwrap();

        // The worker sees the acceptance: the wait reports a change that
        // needs nothing from it, and the task, the feed and the goal show it.
        let wait = Request::Wait {
            goal,
            seen: work.revision,
            timeout_ms: 30_000,
        };
        let changed = WaitOutcome::Work(PendingWork {
            revision: 12,
            ..PendingWork::default()
        });
        let Ok(Response::Waited(WaitOutcome::Work(after))) =
            t.exchange(&worker_a, None, wait, Ok(Response::Waited(changed)))
        else {
            panic!("the wait reports the change");
        };
        assert!(after.is_empty());
        let shown = TaskDetail {
            view: TaskView {
                result: Some(result),
                applied: false,
                ..task_view(TaskState::Accepted)
            },
            text: Some("Add a login page".to_string()),
            input: Some(base),
            depends_on: Vec::new(),
            deadline_ms: None,
            max_attempts: Some(2),
            cancel: None,
        };
        let Ok(Response::Task(shown)) = t.exchange(
            &worker_a,
            None,
            Request::Task { goal, task },
            Ok(Response::Task(shown)),
        ) else {
            panic!("the task is shown");
        };
        assert_eq!(shown.view.state, TaskState::Accepted);
        // Accepted is not applied: the worker's own files do not have the
        // new head yet.
        assert!(!shown.view.applied);
        let feed = Request::Events {
            goal,
            after: None,
            limit: MAX_FEED_PAGE,
        };
        let entries = vec![event_view(11, accepted, LEAD, "result_accepted")];
        let Ok(Response::Events(entries)) =
            t.exchange(&worker_a, None, feed, Ok(Response::Events(entries)))
        else {
            panic!("the feed is answered");
        };
        assert_eq!(entries[0].kind, "result_accepted");
        let status = Response::GoalStatus(GoalStatus {
            head: Some(head),
            decision_head: Some(accepted),
            ..goal_status()
        });
        let Ok(Response::GoalStatus(status)) =
            t.exchange(&worker_a, None, Request::GoalStatus { goal }, Ok(status))
        else {
            panic!("the goal is shown");
        };
        assert_eq!(status.head, Some(head));

        // The worker's trusted CLI applies the accepted head to the worker's
        // files and records it as integrated. From then on the board shows
        // the task as applied on this machine.
        let worker_cli = worker(None);
        let integrated = Request::WorkspaceSet {
            goal,
            binding: WorkspaceBinding {
                destination: Some("/home/worker/app".to_string()),
                integrated: Some(head),
                ..WorkspaceBinding::default()
            },
        };
        t.exchange(&worker_cli, None, integrated, Ok(Response::Done))
            .unwrap();
        let board = vec![TaskView {
            result: Some(result),
            applied: true,
            ..task_view(TaskState::Accepted)
        }];
        let Ok(Response::Board(board)) = t.exchange(
            &worker_cli,
            None,
            Request::Board { goal },
            Ok(Response::Board(board)),
        ) else {
            panic!("the board is answered");
        };
        assert_eq!(
            (board[0].task, board[0].state, board[0].applied),
            (task, TaskState::Accepted, true)
        );

        // A second task. The owner now gives the worker a standing execution
        // grant for this goal, so session A claims without asking.
        let grant = Request::GoalGrant {
            goal,
            agent: WORKER,
            grants: GoalGrants {
                execute: true,
                ..GoalGrants::default()
            },
        };
        t.exchange(&owner, None, grant, Ok(Response::Done)).unwrap();
        let (task, assignment) = (event(0x40), event(0x41));
        let propose = Request::TaskPropose {
            goal,
            text: "Rate-limit the login form".to_string(),
            input: Some(head),
            depends_on: vec![event(0x20)],
            deadline_ms: None,
            max_attempts: None,
        };
        t.exchange(&lead, None, propose, recorded(task)).unwrap();
        let assign = Request::TaskAssign {
            goal,
            task,
            assignee: WORKER,
        };
        t.exchange(&lead, None, assign, recorded(assignment))
            .unwrap();
        let held_by = |session: SessionSecret, generation| Claim {
            goal,
            task,
            assignment,
            instance: session.instance(),
            generation,
        };
        let claim = || Request::TaskClaim { goal, assignment };
        let takeover = || Request::TaskTakeover { goal, assignment };
        let first = Ok(Response::Claimed(held_by(SESSION_A, 1)));
        t.exchange(&worker_a, None, claim(), first).unwrap();

        // Session B, same principal and credential, cannot recover A's claim
        // and cannot take it over on the execution grant alone.
        let held = t.exchange(&worker_b, None, claim(), refused(ErrorCode::ClaimHeld));
        assert_eq!(held.unwrap_err().code, ErrorCode::ClaimHeld);
        let unauthorized = t.exchange(
            &worker_b,
            None,
            takeover(),
            refused(ErrorCode::AuthorizationRequired),
        );
        assert_eq!(
            unauthorized.unwrap_err().code,
            ErrorCode::AuthorizationRequired
        );
        let authorize = Request::TaskAuthorize {
            goal,
            assignment,
            takeover: true,
        };
        t.exchange(&owner, None, authorize, Ok(Response::Done))
            .unwrap();
        let second = Ok(Response::Claimed(held_by(SESSION_B, 2)));
        t.exchange(&worker_b, None, takeover(), second).unwrap();

        // A is fenced: its report is refused and it sees who holds the work.
        let stale = Request::TaskProgress {
            goal,
            assignment,
            generation: 1,
            text: "Still going".to_string(),
        };
        let fenced = t.exchange(&worker_a, None, stale, refused(ErrorCode::Superseded));
        assert_eq!(fenced.unwrap_err().code, ErrorCode::Superseded);
        let elsewhere = PendingWork {
            revision: 17,
            held_elsewhere: vec![held_by(SESSION_B, 2)],
            ..PendingWork::default()
        };
        let Ok(Response::Pending(elsewhere)) = t.exchange(
            &worker_a,
            None,
            Request::Pending { goal },
            Ok(Response::Pending(elsewhere)),
        ) else {
            panic!("pending work is answered");
        };
        assert!(elsewhere.claimed.is_empty());
        assert_eq!(elsewhere.held_elsewhere[0].instance, SESSION_B.instance());

        // A takes the work back. Asking twice does not raise the generation
        // twice.
        for _ in 0..2 {
            let third = Ok(Response::Claimed(held_by(SESSION_A, 3)));
            let Ok(Response::Claimed(current)) = t.exchange(&worker_a, None, takeover(), third)
            else {
                panic!("the takeover is answered");
            };
            assert_eq!(current, held_by(SESSION_A, 3));
        }

        // Three submissions for one assignment by one principal. The one A
        // sent during its first hold, delayed until now, names generation 1;
        // the current one names generation 3. They are different requests, so
        // the daemon can refuse the first and record the last.
        let submit = |generation| Request::TaskSubmit {
            goal,
            assignment,
            generation,
            summary: "Rate limiting added".to_string(),
            base: Some(head),
            patch: Some(blob(0x42)),
            artifacts: Vec::new(),
        };
        let (delayed, current) = (submit(1), submit(3));
        assert_ne!(delayed, current);
        assert_ne!(
            codec::encode(&delayed).unwrap(),
            codec::encode(&current).unwrap()
        );
        let late = t.exchange(&worker_a, None, delayed, refused(ErrorCode::Superseded));
        assert_eq!(late.unwrap_err().code, ErrorCode::Superseded);
        let fenced = t.exchange(&worker_b, None, submit(2), refused(ErrorCode::Superseded));
        assert_eq!(fenced.unwrap_err().code, ErrorCode::Superseded);
        t.exchange(&worker_a, None, current, recorded(event(0x43)))
            .unwrap();

        assert_eq!(
            t.names,
            [
                "agent.enroll",
                "agent.enroll",
                "goal.create",
                "workspace.set",
                "goal.invite",
                "goal.join",
                "goal.join",
                "goal.status",
                "task.propose",
                "task.assign",
                "pending",
                "task.claim",
                "task.authorize",
                "wait",
                "task.claim",
                "task.claim",
                "task.progress",
                "blob.put",
                "blob.put",
                "task.submit",
                "pending",
                "event.show",
                "blob.stat",
                "blob.get",
                "blob.get",
                "blob.put",
                "result.accept",
                "wait",
                "task.show",
                "events",
                "goal.status",
                "workspace.set",
                "board",
                "goal.grant",
                "task.propose",
                "task.assign",
                "task.claim",
                "task.claim",
                "task.takeover",
                "task.authorize",
                "task.takeover",
                "task.progress",
                "pending",
                "task.takeover",
                "task.takeover",
                "task.submit",
                "task.submit",
                "task.submit",
            ]
        );
    }

    #[test]
    fn a_proposed_revision_can_be_read_before_it_is_accepted() {
        let mut t = Transcript::default();
        let goal = goal();
        let (lead, worker) = (lead(), worker(None));
        let revision = event(0x50);

        let revise = Request::DocRevise {
            goal,
            doc: Doc::Plan,
            base: None,
            text: "1. Login page\n2. Rate limiting".to_string(),
        };
        t.exchange(&worker, None, revise, recorded(revision))
            .unwrap();

        let read = Request::DocRead {
            goal,
            doc: Doc::Plan,
        };
        let view = DocView {
            doc: Doc::Plan,
            accepted: None,
            text: None,
            proposals: vec![revision],
        };
        let Ok(Response::Doc(view)) = t.exchange(&lead, None, read, Ok(Response::Doc(view))) else {
            panic!("the document is read");
        };

        let show = Request::Event {
            goal,
            event: view.proposals[0],
        };
        let detail = EventDetail {
            view: event_view(14, revision, WORKER, "revision"),
            anchor: Some(event(0x29)),
            body: Body::Revision {
                doc: Doc::Plan,
                base: None,
            },
            payload: Some(PayloadRef {
                hash: blob(0x51),
                len: 76,
                key_epoch: Default::default(),
            }),
            text: Some("1. Login page\n2. Rate limiting".to_string()),
            task: None,
            content: vec![BlobStatus {
                hash: blob(0x51),
                state: BlobState::Held,
            }],
        };
        let Ok(Response::Event(detail)) =
            t.exchange(&lead, None, show, Ok(Response::Event(detail)))
        else {
            panic!("the revision is shown");
        };
        assert_eq!(detail.view.author, WORKER);
        assert_eq!(
            detail.body,
            Body::Revision {
                doc: Doc::Plan,
                base: view.accepted,
            }
        );
        assert!(
            detail
                .text
                .is_some_and(|text| text.contains("Rate limiting"))
        );

        let accept = Request::DocAccept { goal, revision };
        t.exchange(&lead, None, accept, recorded(event(0x52)))
            .unwrap();
        assert_eq!(
            t.names,
            ["doc.revise", "doc.read", "event.show", "doc.accept"]
        );

        // Reading is free: both reads are read-only, goal-scoped tools.
        for request in [
            Request::Event {
                goal,
                event: revision,
            },
            Request::BlobStat {
                goal,
                hashes: Vec::new(),
            },
        ] {
            let operation = request.operation();
            assert!(operation.read_only && operation.goal_scoped && operation.tool);
        }
    }

    #[test]
    fn a_cancellation_is_acknowledged_with_and_without_a_claim() {
        let mut t = Transcript::default();
        let goal = goal();
        let (lead, worker_cli) = (lead(), worker(None));
        let (worker_a, worker_b) = (worker(Some(SESSION_A)), worker(Some(SESSION_B)));
        let task = event(0x20);

        // An assignment session A has claimed: only A answers, naming the
        // generation it holds.
        let (assignment, cancel) = (event(0x60), event(0x61));
        let request = Request::TaskCancel { goal, assignment };
        t.exchange(&lead, None, request, recorded(cancel)).unwrap();
        let held = claim(assignment, SESSION_A, 1);
        let work = PendingWork {
            revision: 20,
            claimed: vec![held],
            to_acknowledge: vec![CancelItem {
                task,
                assignment,
                cancel,
                generation: Some(held.generation),
            }],
            ..PendingWork::default()
        };
        let Ok(Response::Pending(work)) = t.exchange(
            &worker_a,
            None,
            Request::Pending { goal },
            Ok(Response::Pending(work)),
        ) else {
            panic!("pending work is answered");
        };
        let item = work.to_acknowledge[0];
        let acknowledge = |generation| Request::CancelAcknowledge {
            goal,
            cancel: item.cancel,
            generation,
            outcome: CancelOutcome::Stopped,
        };
        // Neither another session nor a plain connection of the same
        // principal can answer for the session that holds the claim.
        for (connection, generation) in [(&worker_b, item.generation), (&worker_cli, None)] {
            let other = t.exchange(
                connection,
                None,
                acknowledge(generation),
                refused(ErrorCode::Superseded),
            );
            assert_eq!(other.unwrap_err().code, ErrorCode::Superseded);
        }
        t.exchange(
            &worker_a,
            None,
            acknowledge(item.generation),
            recorded(event(0x62)),
        )
        .unwrap();

        // An assignment nobody claimed: any connection of the assignee
        // answers, with no generation to name.
        let (assignment, cancel) = (event(0x63), event(0x64));
        let request = Request::TaskCancel { goal, assignment };
        t.exchange(&lead, None, request, recorded(cancel)).unwrap();
        let work = PendingWork {
            revision: 23,
            to_acknowledge: vec![CancelItem {
                task,
                assignment,
                cancel,
                generation: None,
            }],
            ..PendingWork::default()
        };
        let Ok(Response::Pending(work)) = t.exchange(
            &worker_cli,
            None,
            Request::Pending { goal },
            Ok(Response::Pending(work)),
        ) else {
            panic!("pending work is answered");
        };
        let item = work.to_acknowledge[0];
        assert_eq!(item.generation, None);
        let acknowledge = Request::CancelAcknowledge {
            goal,
            cancel: item.cancel,
            generation: item.generation,
            outcome: CancelOutcome::Stopped,
        };
        t.exchange(&worker_cli, None, acknowledge, recorded(event(0x65)))
            .unwrap();

        assert_eq!(
            t.names,
            [
                "task.cancel",
                "pending",
                "cancel.acknowledge",
                "cancel.acknowledge",
                "cancel.acknowledge",
                "task.cancel",
                "pending",
                "cancel.acknowledge",
            ]
        );
    }

    #[test]
    fn a_member_leaves_and_a_member_is_removed() {
        let mut t = Transcript::default();
        let goal = goal();
        let (owner, lead, worker) = (owner(), lead(), worker(None));

        // The worker asks to leave; its daemon stops taking part at once.
        t.exchange(
            &worker,
            None,
            Request::GoalLeave { goal },
            recorded(event(0x70)),
        )
        .unwrap();
        let status = DaemonStatus {
            daemon_version: "0.1.0".to_string(),
            endpoint: Some(EndpointId([0x0f; 32])),
            agents: vec![AgentView {
                agent: WORKER,
                name: "worker".to_string(),
                grants: Grants { manage_goals: true },
                revoked: false,
            }],
            goals: vec![GoalSummary {
                goal,
                title: Some("Ship the login page".to_string()),
                member: WORKER,
                membership: Membership::Left,
                halted: None,
            }],
        };
        let Ok(Response::Status(status)) =
            t.exchange(&worker, None, Request::Status, Ok(Response::Status(status)))
        else {
            panic!("status is answered");
        };
        assert_eq!(status.goals[0].membership, Membership::Left);
        // Having left, it authors nothing more in the goal.
        let note = Request::NoteAdd {
            goal,
            about: None,
            supersedes: None,
            text: "One more thing".to_string(),
        };
        let late = t.exchange(&worker, None, note, refused(ErrorCode::Denied));
        assert_eq!(late.unwrap_err().code, ErrorCode::Denied);

        // The coordinator removes a member, here acting on the leave request.
        let remove = Request::MemberRemove {
            goal,
            member: WORKER,
        };
        t.exchange(&lead, None, remove, recorded(event(0x71)))
            .unwrap();

        // The owner, asking directly, sees every local principal's standing.
        let status = DaemonStatus {
            goals: vec![GoalSummary {
                membership: Membership::Removed,
                ..status.goals[0].clone()
            }],
            ..status
        };
        t.exchange(&owner, None, Request::Status, Ok(Response::Status(status)))
            .unwrap();

        assert_eq!(
            t.names,
            [
                "goal.leave",
                "status",
                "note.add",
                "member.remove",
                "status"
            ]
        );
    }

    #[test]
    fn an_adapter_persists_a_launch_intent_and_reads_it_back_after_a_restart() {
        let mut t = Transcript::default();
        let secret = SessionSecret([0xc3; 32]);
        let instance = secret.instance();

        // Before it spawns the client, the launcher writes the secret to its
        // file and records the launch under it.
        let launcher = worker(Some(secret));
        let intent = record(SessionState::Launching);
        let report = Request::SessionReport {
            record: intent.clone(),
        };
        t.exchange(&launcher, None, report, Ok(Response::Done))
            .unwrap();

        // The launcher is interrupted. A new connection that presents the
        // secret from the file is the same session and reads the intent back.
        let restarted = worker(Some(SessionSecret(secret.0)));
        let view = SessionView {
            instance,
            principal: WORKER,
            record: intent.clone(),
            updated_ms: 1_790_000_000_000,
            attached: true,
            claims: Vec::new(),
        };
        let Ok(Response::Session(view)) = t.exchange(
            &restarted,
            None,
            Request::Session { instance: None },
            Ok(Response::Session(view)),
        ) else {
            panic!("the session is shown");
        };
        assert_eq!(view.instance, instance);
        assert_eq!(view.record, intent);
        assert_eq!(view.record.detail, b"adapter-owned launch intent");

        // The client came up; the record follows, now with the client's own
        // session identifier for a later resume.
        let ready = SessionRecord {
            state: SessionState::Ready,
            client_session: Some("019a0f6e-thread".to_string()),
            ..intent
        };
        let report = Request::SessionReport {
            record: ready.clone(),
        };
        t.exchange(&restarted, None, report, Ok(Response::Done))
            .unwrap();

        // Another session of the same principal cannot write this record: a
        // report always lands on the reporting connection's own session. It
        // can see the record, and so can the owner.
        let other = worker(Some(SESSION_B));
        let listed = vec![SessionView {
            record: ready,
            ..view.clone()
        }];
        let Ok(Response::Sessions(listed)) = t.exchange(
            &other,
            None,
            Request::Sessions,
            Ok(Response::Sessions(listed)),
        ) else {
            panic!("sessions are listed");
        };
        assert_eq!(listed[0].record.state, SessionState::Ready);
        let named = Request::Session {
            instance: Some(instance),
        };
        let shown = Ok(Response::Session(listed[0].clone()));
        t.exchange(&owner(), None, named, shown).unwrap();

        // A connection without a session has no record to write or read.
        let plain = worker(None);
        let report = Request::SessionReport {
            record: record(SessionState::Unknown),
        };
        let no_session = t.exchange(&plain, None, report, refused(ErrorCode::Invalid));
        assert_eq!(no_session.unwrap_err().code, ErrorCode::Invalid);

        // Only the session itself or the owner deletes the record.
        let drop_record = || Request::SessionDrop { instance };
        let denied = t.exchange(&other, None, drop_record(), refused(ErrorCode::Denied));
        assert_eq!(denied.unwrap_err().code, ErrorCode::Denied);
        t.exchange(&restarted, None, drop_record(), Ok(Response::Done))
            .unwrap();

        assert_eq!(
            t.names,
            [
                "session.report",
                "session.show",
                "session.report",
                "sessions",
                "session.show",
                "session.report",
                "session.drop",
                "session.drop",
            ]
        );
        // No request in this flow carried the secret or named the session to
        // write to; the hello did both.
        assert_eq!(
            json_fields(&Request::SessionReport {
                record: record(SessionState::Launching),
            }),
            ["record"]
        );
    }

    #[test]
    fn the_owner_acts_on_behalf_of_a_principal() {
        let mut t = Transcript::default();
        let goal = goal();
        let (task, assignment) = (event(0x20), event(0x80));
        let owner_session = SessionSecret([0xd4; 32]);
        let owner = connect(OWNER_CREDENTIAL, Some(owner_session), Caller::Owner);

        // Directly, the owner authors nothing: a request that acts as a
        // principal names one.
        let assign = || Request::TaskAssign {
            goal,
            task,
            assignee: WORKER,
        };
        let direct = t.exchange(&owner, None, assign(), refused(ErrorCode::Invalid));
        assert_eq!(direct.unwrap_err().code, ErrorCode::Invalid);
        t.exchange(&owner, Some(LEAD), assign(), recorded(assignment))
            .unwrap();

        // On behalf of the worker it takes the assignment. No grant is
        // consulted; the claim still belongs to the connection's session.
        let held = Claim {
            goal,
            task,
            assignment,
            instance: owner_session.instance(),
            generation: 1,
        };
        t.exchange(
            &owner,
            Some(WORKER),
            Request::TaskClaim { goal, assignment },
            Ok(Response::Claimed(held)),
        )
        .unwrap();

        // Reading needs no principal, and owner-only requests admit none.
        let board = Response::Board(vec![task_view(TaskState::Taken)]);
        t.exchange(&owner, None, Request::Board { goal }, Ok(board))
            .unwrap();
        let revoke = || Request::AgentRevoke { agent: WORKER };
        let misuse = t.exchange(&owner, Some(LEAD), revoke(), refused(ErrorCode::Invalid));
        assert_eq!(misuse.unwrap_err().code, ErrorCode::Invalid);

        // Nobody else may speak for a principal, and the owner may speak
        // only for one that is enrolled.
        let agent = t.exchange(&lead(), Some(WORKER), assign(), refused(ErrorCode::Denied));
        assert_eq!(agent.unwrap_err().code, ErrorCode::Denied);
        let unknown = t.exchange(&owner, Some(key(9)), assign(), refused(ErrorCode::NotFound));
        assert_eq!(unknown.unwrap_err().code, ErrorCode::NotFound);

        t.exchange(&owner, None, revoke(), Ok(Response::Done))
            .unwrap();
        t.exchange(&owner, None, Request::Shutdown, Ok(Response::Done))
            .unwrap();
        assert_eq!(
            t.names,
            [
                "task.assign",
                "task.assign",
                "task.claim",
                "board",
                "agent.revoke",
                "task.assign",
                "task.assign",
                "agent.revoke",
                "daemon.stop",
            ]
        );
    }

    #[test]
    fn a_viewer_reads_what_its_principal_reads_and_writes_nothing() {
        let mut t = Transcript::default();
        let goal = goal();
        let owner = owner();

        // The owner enrolls a viewer of the worker. As with a principal, the
        // client stored the credential first and sends only its digest, and
        // repeating the request changes nothing.
        let enroll = |agent, credential: Credential| Request::ViewerEnroll {
            agent,
            credential: credential.digest(),
        };
        for _ in 0..2 {
            t.exchange(
                &owner,
                None,
                enroll(WORKER, VIEWER_CREDENTIAL),
                Ok(Response::Done),
            )
            .unwrap();
        }
        // Only for an enrolled principal, and only under a credential that
        // is nothing else yet.
        let unknown = t.exchange(
            &owner,
            None,
            enroll(key(9), VIEWER_CREDENTIAL),
            refused(ErrorCode::NotFound),
        );
        assert_eq!(unknown.unwrap_err().code, ErrorCode::NotFound);
        let reused = t.exchange(
            &owner,
            None,
            enroll(WORKER, WORKER_CREDENTIAL),
            refused(ErrorCode::Conflict),
        );
        assert_eq!(reused.unwrap_err().code, ErrorCode::Conflict);

        // A viewer is never an execution session: its credential with a
        // session is refused at the hello. Without one it is welcomed as a
        // viewer of the worker.
        refuse_hello(VIEWER_CREDENTIAL, Some(SESSION_A), ErrorCode::Invalid);
        let viewer = connect(VIEWER_CREDENTIAL, None, Caller::Viewer(WORKER));
        let worker = worker(None);

        // The viewer reads the board as the worker does, including which
        // accepted results the worker has applied on this machine.
        let tasks = vec![
            TaskView {
                applied: true,
                ..task_view(TaskState::Accepted)
            },
            TaskView {
                task: event(0x40),
                assignment: Some(event(0x41)),
                result: Some(event(0x43)),
                ..task_view(TaskState::Submitted)
            },
        ];
        let mut boards = Vec::new();
        for connection in [&worker, &viewer] {
            let answer = Ok(Response::Board(tasks.clone()));
            let Ok(Response::Board(board)) =
                t.exchange(connection, None, Request::Board { goal }, answer)
            else {
                panic!("the board is answered");
            };
            boards.push(board);
        }
        assert_eq!(boards[0], boards[1]);
        assert!(boards[1][0].applied && !boards[1][1].applied);

        // Its other reads are answered as the worker's too: the goal with
        // the worker's binding, pending work with no session's claims, and
        // the feed after the position the viewer keeps itself. Reading the
        // feed stores no cursor.
        let status = GoalStatus {
            workspace: Some(binding()),
            ..goal_status()
        };
        t.exchange(
            &viewer,
            None,
            Request::GoalStatus { goal },
            Ok(Response::GoalStatus(status)),
        )
        .unwrap();
        let work = PendingWork {
            revision: 30,
            to_claim: vec![AssignmentRef {
                task: event(0x44),
                assignment: event(0x45),
            }],
            ..PendingWork::default()
        };
        let Ok(Response::Pending(work)) = t.exchange(
            &viewer,
            None,
            Request::Pending { goal },
            Ok(Response::Pending(work)),
        ) else {
            panic!("pending work is answered");
        };
        assert!(work.claimed.is_empty());
        let feed = Request::Events {
            goal,
            after: Some(8),
            limit: MAX_FEED_PAGE,
        };
        let entries = vec![event_view(9, event(0x43), WORKER, "result_submitted")];
        t.exchange(&viewer, None, feed, Ok(Response::Events(entries)))
            .unwrap();

        // It reads for nobody else, not even its own principal by name.
        let behalf = t.exchange(
            &viewer,
            Some(WORKER),
            Request::Board { goal },
            refused(ErrorCode::Denied),
        );
        assert_eq!(behalf.unwrap_err().code, ErrorCode::Denied);

        // Every request that is not read-only is denied to it: those that
        // sign an event, the workspace record, the session operations that
        // write, and the owner's.
        let mut writes = Transcript::default();
        for request in requests().into_iter().filter(|r| !r.is_read_only()) {
            let denied = writes.exchange(&viewer, None, request, refused(ErrorCode::Denied));
            assert_eq!(denied.unwrap_err().code, ErrorCode::Denied);
        }
        let not_read_only: Vec<&str> = OPERATIONS
            .iter()
            .filter(|operation| !operation.read_only)
            .map(|operation| operation.name)
            .collect();
        assert_eq!(writes.names, not_read_only);
        for name in [
            "note.add",
            "task.claim",
            "workspace.set",
            "session.report",
            "session.drop",
            "viewer.enroll",
        ] {
            assert!(writes.names.contains(&name), "{name}");
        }

        // Revoking the worker revokes its viewer with it.
        let revoke = Request::AgentRevoke { agent: WORKER };
        t.exchange(&owner, None, revoke, Ok(Response::Done))
            .unwrap();
        refuse_hello(VIEWER_CREDENTIAL, None, ErrorCode::Denied);

        assert_eq!(
            t.names,
            [
                "viewer.enroll",
                "viewer.enroll",
                "viewer.enroll",
                "viewer.enroll",
                "board",
                "board",
                "goal.status",
                "pending",
                "events",
                "board",
                "agent.revoke",
            ]
        );
    }
}
