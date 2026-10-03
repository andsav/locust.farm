//! The local daemon API: what the CLI, the stdio MCP bridge and client
//! adapters ask of the daemon over its Unix socket.
//!
//! A connection opens with one [`ClientHello`] frame answered by one
//! [`ServerHello`]; after that the client sends [`RequestFrame`]s and receives
//! one [`ResponseFrame`] per request, matched by `id`. Frames use
//! [`crate::codec`]. The credential presented in the hello fixes the caller
//! for the whole connection.
//!
//! Every goal-scoped request names its goal. There is no current or default
//! goal, and a missing credential never selects a wider scope.
//!
//! Human-readable output is these same types rendered as JSON; identifiers
//! appear as hex and credentials are never rendered.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::codec;
use crate::crypto;
use crate::event::{CancelOutcome, Doc};
use crate::id::{BlobHash, EndpointId, EventId, GoalId, IdempotencyKey, InstanceId, PublicKey};

/// A local bearer secret issued at enrollment. It scopes what a client may
/// ask for and attributes its requests; it is not protection against another
/// process that can read the same user's files.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Credential(pub [u8; 32]);

impl Credential {
    /// What the daemon stores and looks up in place of the secret.
    pub fn digest(&self) -> [u8; 32] {
        crypto::domain_hash(crypto::domain::LOCAL_CREDENTIAL, &self.0)
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientHello {
    pub api_version: u16,
    pub credential: Credential,
}

/// Who a connection acts as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Caller {
    /// The local participant: enrolls principals, sets their grants and
    /// authorizes what no grant covers. Authors no events itself.
    Owner,
    /// An enrolled principal acting within its grants.
    Agent(PublicKey),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerHello {
    Welcome {
        api_version: u16,
        daemon_version: String,
        caller: Caller,
    },
    Refused(ApiError),
}

/// What the local participant has authorized a principal to do without
/// asking again. Anything outside the grants waits for the owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grants {
    /// Create goals, issue invitations, join and leave.
    pub manage_goals: bool,
    /// Take assigned work in goals it has joined. Without this, each
    /// assignment waits for [`Request::TaskAuthorize`].
    pub execute: bool,
    /// Sign coordinator decisions in goals it coordinates.
    pub decide: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestFrame {
    /// Chosen by the client; echoed in the response.
    pub id: u64,
    /// With a key, a repeated request returns its first result. Reusing a key
    /// for a different request fails with [`ErrorCode::IdempotencyMismatch`].
    pub idempotency: Option<IdempotencyKey>,
    pub request: Request,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseFrame {
    pub id: u64,
    pub result: Result<Response, ApiError>,
}

/// Every operation of the local API. Variants are appended, never reordered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Request {
    // Daemon-wide.
    Status,
    /// Owner only. Creates a principal and returns its credential once.
    AgentEnroll {
        name: String,
        grants: Grants,
    },
    /// Owner only.
    AgentGrant {
        agent: PublicKey,
        grants: Grants,
    },
    /// Owner only. The principal's credential stops working immediately.
    AgentRevoke {
        agent: PublicKey,
    },
    /// The calling principal founds a goal it owns and coordinates.
    GoalCreate {
        title: String,
    },
    /// Redeems an invitation ticket for the calling principal.
    GoalJoin {
        ticket: String,
    },

    // Goal-scoped.
    GoalInvite {
        goal: GoalId,
        expires_ms: Option<u64>,
    },
    GoalLeave {
        goal: GoalId,
    },
    GoalStatus {
        goal: GoalId,
    },
    Board {
        goal: GoalId,
    },
    Task {
        goal: GoalId,
        task: EventId,
    },
    TaskPropose {
        goal: GoalId,
        text: String,
        input: Option<BlobHash>,
        depends_on: Vec<EventId>,
        deadline_ms: Option<u64>,
        max_attempts: Option<u32>,
    },
    /// Coordinator. Assigning an already assigned task supersedes the earlier
    /// assignment with a new attempt.
    TaskAssign {
        goal: GoalId,
        task: EventId,
        assignee: PublicKey,
    },
    /// Coordinator.
    TaskCancel {
        goal: GoalId,
        assignment: EventId,
    },
    /// Owner only. Lets the assignee take one assignment its grants do not cover.
    TaskAuthorize {
        goal: GoalId,
        assignment: EventId,
    },
    /// Takes an assignment for one execution session, or recovers that
    /// session's existing claim. `takeover` replaces another session's claim
    /// and invalidates it; it needs the same authorization as a first claim.
    TaskClaim {
        goal: GoalId,
        assignment: EventId,
        instance: InstanceId,
        takeover: bool,
    },
    TaskDecline {
        goal: GoalId,
        assignment: EventId,
    },
    TaskProgress {
        goal: GoalId,
        assignment: EventId,
        instance: InstanceId,
        text: String,
    },
    TaskSubmit {
        goal: GoalId,
        assignment: EventId,
        instance: InstanceId,
        summary: String,
        base: Option<BlobHash>,
        patch: Option<BlobHash>,
        artifacts: Vec<BlobHash>,
    },
    TaskFail {
        goal: GoalId,
        assignment: EventId,
        instance: InstanceId,
        reason: String,
    },
    CancelAcknowledge {
        goal: GoalId,
        cancel: EventId,
        outcome: CancelOutcome,
    },
    /// Coordinator.
    ResultAccept {
        goal: GoalId,
        result: EventId,
        head: Option<BlobHash>,
    },
    /// Coordinator.
    ResultReject {
        goal: GoalId,
        result: EventId,
        reason: String,
    },
    /// What needs the caller now, computed from task state. Never depends on
    /// a notification having been delivered.
    Pending {
        goal: GoalId,
    },
    /// Like [`Request::Pending`], but holds the request open until something
    /// changes after `after` or `timeout_ms` passes.
    Wait {
        goal: GoalId,
        after: u64,
        timeout_ms: u32,
    },
    /// The goal's event feed after a position.
    Events {
        goal: GoalId,
        after: u64,
        limit: u32,
    },
    NoteAdd {
        goal: GoalId,
        about: Option<EventId>,
        supersedes: Option<EventId>,
        text: String,
    },
    Notes {
        goal: GoalId,
        about: Option<EventId>,
    },
    DocRead {
        goal: GoalId,
        doc: Doc,
    },
    DocRevise {
        goal: GoalId,
        doc: Doc,
        base: Option<EventId>,
        text: String,
    },
    /// Coordinator.
    DocAccept {
        goal: GoalId,
        revision: EventId,
    },
    /// Stores bytes the client selected. The daemon never opens a path a
    /// client names; the trusted CLI or adapter reads the file and sends this.
    BlobPut {
        goal: GoalId,
        #[serde(with = "codec::bytes")]
        bytes: Vec<u8>,
    },
    BlobGet {
        goal: GoalId,
        hash: BlobHash,
    },
}

impl Request {
    /// Stable operation name, used for command names, MCP tool names
    /// (`locust_` plus this, dots replaced by underscores), logs and errors.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::AgentEnroll { .. } => "agent.enroll",
            Self::AgentGrant { .. } => "agent.grant",
            Self::AgentRevoke { .. } => "agent.revoke",
            Self::GoalCreate { .. } => "goal.create",
            Self::GoalJoin { .. } => "goal.join",
            Self::GoalInvite { .. } => "goal.invite",
            Self::GoalLeave { .. } => "goal.leave",
            Self::GoalStatus { .. } => "goal.status",
            Self::Board { .. } => "board",
            Self::Task { .. } => "task.show",
            Self::TaskPropose { .. } => "task.propose",
            Self::TaskAssign { .. } => "task.assign",
            Self::TaskCancel { .. } => "task.cancel",
            Self::TaskAuthorize { .. } => "task.authorize",
            Self::TaskClaim { .. } => "task.claim",
            Self::TaskDecline { .. } => "task.decline",
            Self::TaskProgress { .. } => "task.progress",
            Self::TaskSubmit { .. } => "task.submit",
            Self::TaskFail { .. } => "task.fail",
            Self::CancelAcknowledge { .. } => "cancel.acknowledge",
            Self::ResultAccept { .. } => "result.accept",
            Self::ResultReject { .. } => "result.reject",
            Self::Pending { .. } => "pending",
            Self::Wait { .. } => "wait",
            Self::Events { .. } => "events",
            Self::NoteAdd { .. } => "note.add",
            Self::Notes { .. } => "notes",
            Self::DocRead { .. } => "doc.read",
            Self::DocRevise { .. } => "doc.revise",
            Self::DocAccept { .. } => "doc.accept",
            Self::BlobPut { .. } => "blob.put",
            Self::BlobGet { .. } => "blob.get",
        }
    }

    /// The goal a request is scoped to; `None` for daemon-wide requests.
    pub fn goal(&self) -> Option<GoalId> {
        match self {
            Self::Status
            | Self::AgentEnroll { .. }
            | Self::AgentGrant { .. }
            | Self::AgentRevoke { .. }
            | Self::GoalCreate { .. }
            | Self::GoalJoin { .. } => None,
            Self::GoalInvite { goal, .. }
            | Self::GoalLeave { goal }
            | Self::GoalStatus { goal }
            | Self::Board { goal }
            | Self::Task { goal, .. }
            | Self::TaskPropose { goal, .. }
            | Self::TaskAssign { goal, .. }
            | Self::TaskCancel { goal, .. }
            | Self::TaskAuthorize { goal, .. }
            | Self::TaskClaim { goal, .. }
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
            | Self::BlobGet { goal, .. } => Some(*goal),
        }
    }

    /// True if the request changes nothing. Bridges advertise these as
    /// read-only so clients need not prompt for them.
    pub fn is_read_only(&self) -> bool {
        matches!(
            self,
            Self::Status
                | Self::GoalStatus { .. }
                | Self::Board { .. }
                | Self::Task { .. }
                | Self::Pending { .. }
                | Self::Wait { .. }
                | Self::Events { .. }
                | Self::Notes { .. }
                | Self::DocRead { .. }
                | Self::BlobGet { .. }
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
    /// The request succeeded and has nothing to return.
    Done,
    Status(DaemonStatus),
    AgentEnrolled {
        agent: PublicKey,
        credential: Credential,
    },
    GoalCreated {
        goal: GoalId,
    },
    Invited {
        ticket: String,
    },
    Joined {
        goal: GoalId,
        /// False while admission by the coordinator is still outstanding.
        admitted: bool,
    },
    GoalStatus(GoalStatus),
    Board(Vec<TaskView>),
    Task(TaskDetail),
    /// The request was recorded as this event.
    Recorded {
        event: EventId,
    },
    Claimed(Claim),
    Pending(PendingWork),
    Waited(WaitOutcome),
    Events(Vec<EventView>),
    Notes(Vec<NoteView>),
    Doc(DocView),
    BlobStored {
        hash: BlobHash,
    },
    Blob {
        #[serde(with = "codec::bytes")]
        bytes: Vec<u8>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub daemon_version: String,
    /// Absent until the transport is running.
    pub endpoint: Option<EndpointId>,
    pub agents: Vec<AgentView>,
    pub goals: Vec<GoalId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentView {
    pub agent: PublicKey,
    pub name: String,
    pub grants: Grants,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalStatus {
    pub goal: GoalId,
    pub title: Option<String>,
    pub coordinator: PublicKey,
    /// Latest decision applied locally.
    pub decision_head: EventId,
    pub members: Vec<MemberView>,
    /// Set when decisions cannot advance: conflicting authority history, or
    /// local signing held in read-only recovery. Says why.
    pub halted: Option<String>,
    pub peers: Vec<PeerView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberView {
    pub member: PublicKey,
    pub endpoint: EndpointId,
    /// True for principals this daemon holds keys for.
    pub local: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerView {
    pub endpoint: EndpointId,
    pub connected: bool,
    /// Last successful synchronization, Unix milliseconds by the local clock.
    /// State written by the peer after this is unknown here.
    pub last_sync_ms: Option<u64>,
}

/// Where a task stands. Derived from the goal's history; states reported by
/// the worker are shown as reported, and only `Accepted` and `Rejected`
/// reflect a coordinator decision on a result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskView {
    pub task: EventId,
    pub state: TaskState,
    /// First line of the task text, when the text is held locally.
    pub title: Option<String>,
    pub proposer: PublicKey,
    pub assignee: Option<PublicKey>,
    /// The current assignment, if any.
    pub assignment: Option<EventId>,
    pub attempt: u32,
    /// The latest submitted result of the current assignment.
    pub result: Option<EventId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDetail {
    pub view: TaskView,
    /// The task text; absent when its payload is not held locally.
    pub text: Option<String>,
    pub input: Option<BlobHash>,
    pub depends_on: Vec<EventId>,
    pub deadline_ms: Option<u64>,
    pub max_attempts: Option<u32>,
    /// An unanswered cancellation request for the current assignment.
    pub cancel: Option<EventId>,
}

/// A session's hold on an assignment. A takeover raises the generation, and
/// a session holding an older one can no longer report or submit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub assignment: EventId,
    pub instance: InstanceId,
    pub generation: u32,
}

/// What needs the caller, as identifiers only. Text written by peers is
/// fetched separately through the scoped read operations.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingWork {
    /// Assignments waiting for the local participant's authorization.
    pub to_authorize: Vec<EventId>,
    /// Assignments the caller may take.
    pub to_claim: Vec<EventId>,
    /// Assignments the caller's principal has taken and not finished.
    pub in_progress: Vec<EventId>,
    /// Cancellation requests the caller has not answered.
    pub to_acknowledge: Vec<EventId>,
    /// Results waiting for the caller's decision as coordinator.
    pub to_review: Vec<EventId>,
    /// Feed position this answer reflects; pass it as `after` to wait.
    pub position: u64,
}

impl PendingWork {
    pub fn is_empty(&self) -> bool {
        self.to_authorize.is_empty()
            && self.to_claim.is_empty()
            && self.in_progress.is_empty()
            && self.to_acknowledge.is_empty()
            && self.to_review.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaitOutcome {
    Work(PendingWork),
    /// Nothing changed before the timeout.
    NoEvent {
        position: u64,
    },
    /// Nothing changed, and no peer of this goal is currently reachable, so
    /// remote changes cannot arrive.
    Disconnected {
        position: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventView {
    pub position: u64,
    pub event: EventId,
    pub author: PublicKey,
    /// [`crate::event::Body::kind`].
    pub kind: String,
    pub at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteView {
    pub note: EventId,
    pub author: PublicKey,
    pub about: Option<EventId>,
    pub supersedes: Option<EventId>,
    pub at_ms: u64,
    /// Absent when the payload is not held locally.
    pub text: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocView {
    pub doc: Doc,
    /// The accepted revision, if any.
    pub accepted: Option<EventId>,
    pub text: Option<String>,
    /// Revisions proposed against the accepted one and not yet decided.
    pub proposals: Vec<EventId>,
}

/// Stable error categories. Clients branch on the code, never on the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    /// The credential is missing, revoked or does not cover the request, or
    /// the local participant has not authorized it.
    Denied,
    NotFound,
    /// The request is malformed.
    Invalid,
    /// A precondition no longer holds, for example a stale base or a task
    /// already reassigned.
    Conflict,
    /// Another session holds the claim.
    ClaimHeld,
    /// The caller's claim, attempt or assignment is no longer current.
    Superseded,
    IdempotencyMismatch,
    LimitExceeded,
    /// Needed content or a needed peer is not reachable now.
    Unavailable,
    /// The goal's decisions are halted; see [`GoalStatus::halted`].
    Halted,
    UnsupportedVersion,
    /// Local state failed an integrity check.
    Corrupted,
    Internal,
}

impl ErrorCode {
    /// Stable name for JSON output and logs.
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
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: ErrorCode,
    /// For people. Written by the daemon; never contains peer-written text.
    pub message: String,
}

impl ApiError {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_in_the_binary_encoding() {
        let request = RequestFrame {
            id: 7,
            idempotency: Some(IdempotencyKey([3; 16])),
            request: Request::TaskSubmit {
                goal: GoalId([1; 32]),
                assignment: EventId([2; 32]),
                instance: InstanceId([4; 16]),
                summary: "done".to_string(),
                base: Some(BlobHash([5; 32])),
                patch: None,
                artifacts: vec![BlobHash([6; 32])],
            },
        };
        let bytes = codec::encode(&request).unwrap();
        assert_eq!(codec::decode::<RequestFrame>(&bytes), Ok(request));

        let response = ResponseFrame {
            id: 7,
            result: Err(ApiError::new(
                ErrorCode::Superseded,
                "attempt 1 was reassigned",
            )),
        };
        let bytes = codec::encode(&response).unwrap();
        assert_eq!(codec::decode::<ResponseFrame>(&bytes), Ok(response));
    }

    #[test]
    fn a_credential_crosses_the_socket_but_is_never_rendered() {
        let hello = ClientHello {
            api_version: crate::API_VERSION,
            credential: Credential([0x5a; 32]),
        };
        let bytes = codec::encode(&hello).unwrap();
        assert_eq!(codec::decode::<ClientHello>(&bytes), Ok(hello.clone()));

        let response = Response::AgentEnrolled {
            agent: PublicKey([1; 32]),
            credential: hello.credential,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("<redacted>"));
        assert!(!json.contains("5a5a") && !json.contains("90,90"));
        assert!(serde_json::from_str::<Response>(&json).is_err());
        assert_eq!(format!("{:?}", hello.credential), "Credential(..)");
    }

    #[test]
    fn responses_render_as_json_with_hex_identifiers() {
        let response = Response::Recorded {
            event: EventId([0xab; 32]),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert_eq!(
            json,
            format!("{{\"Recorded\":{{\"event\":\"{}\"}}}}", "ab".repeat(32))
        );
    }

    #[test]
    fn every_goal_scoped_request_reports_its_goal() {
        let goal = GoalId([1; 32]);
        assert_eq!(Request::Board { goal }.goal(), Some(goal));
        assert_eq!(Request::Status.goal(), None);
        assert_eq!(
            Request::GoalJoin {
                ticket: String::new()
            }
            .goal(),
            None
        );
        assert!(Request::Pending { goal }.is_read_only());
        assert!(
            !Request::TaskDecline {
                goal,
                assignment: EventId([2; 32])
            }
            .is_read_only()
        );
        assert_eq!(Request::Board { goal }.name(), "board");
    }
}
