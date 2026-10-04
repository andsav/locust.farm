//! Every exchange of one daemon: which to open, retries, anti-entropy,
//! admission and finishing.

use std::collections::HashMap;

use locust_proto::engine::{PeerInput, PeerOutput};
use locust_proto::id::{EndpointId, GoalId};
use locust_proto::invite::JoinRequest;
use locust_proto::sync::Refusal;

use super::{Initiator, Replica, Responder};

/// Longest a (goal, endpoint) pair goes without an exchange this daemon opens.
pub const ANTI_ENTROPY_MS: u64 = 30_000;

/// Wait before retrying an endpoint after its first failure.
pub const MIN_BACKOFF_MS: u64 = 1_000;

/// Longest wait before retrying an endpoint; the wait doubles up to it.
pub const MAX_BACKOFF_MS: u64 = 60_000;

/// A join this daemon has in progress: the invitation's goal, the inviting
/// daemon and the signed request to present to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Joining {
    /// The goal being joined.
    pub goal: GoalId,
    /// The inviting daemon, which signs the admission.
    pub endpoint: EndpointId,
    /// Contact hints from the ticket.
    pub hints: Vec<String>,
    /// Signed by the joining principal, naming this daemon's endpoint.
    pub request: JoinRequest,
}

/// How an exchange ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    /// It ran to `Done`: both sides held the same events when it began, apart
    /// from what it carried.
    Completed,
    /// One side refused, sent or received.
    Refused(Refusal),
    /// It could not be opened, or the stream ended without `Done`.
    Aborted,
}

/// The outcome of one exchange about a goal, for the node's peer view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    /// The goal the exchange was about.
    pub goal: GoalId,
    /// The remote endpoint.
    pub endpoint: EndpointId,
    /// True when this daemon opened it.
    pub dialed: bool,
    /// True when it presented a [`Joining`] request.
    pub join: bool,
    /// How it ended.
    pub ended: Ended,
    /// When it ended.
    pub at_ms: u64,
}

/// What the driver asks of the node: its goals, who speaks for their members,
/// and the replica of each. Implemented by the node; every call returns at
/// once.
pub trait Host {
    /// Every (goal, endpoint) pair to keep synchronized: each goal held with
    /// each endpoint bound to another current member, ascending. Never this
    /// daemon's own endpoint.
    fn peers(&self) -> Vec<(GoalId, EndpointId)>;

    /// The stored contact hints of `endpoint`, possibly none.
    fn hints(&self, endpoint: &EndpointId) -> Vec<String>;

    /// True when `endpoint` is bound to a current member of `goal`; false for
    /// a goal this daemon does not hold.
    fn speaks_for_member(&self, goal: &GoalId, endpoint: &EndpointId) -> bool;

    /// The replica of a goal held or being joined.
    fn replica(&mut self, goal: &GoalId) -> Option<&mut dyn Replica>;

    /// Joins in progress. A join is presented to its endpoint until the node
    /// stops listing it.
    fn joins(&self) -> Vec<Joining>;

    /// A `Join` received on an exchange whose authenticated remote endpoint
    /// is `remote`. `Ok` when the key is admitted, now or already;
    /// otherwise the refusal to send, normally `InvitationRefused`.
    fn join(
        &mut self,
        remote: &EndpointId,
        request: &JoinRequest,
        now_ms: u64,
    ) -> Result<(), Refusal>;

    /// Goals whose events changed since the driver last asked. Separate from
    /// what the shell takes through `Engine::take_changed`.
    fn take_changed(&mut self) -> Vec<GoalId>;

    /// Records the outcome of an exchange about a goal. Not called for an
    /// accepted exchange that never named one.
    fn exchange_ended(&mut self, report: Report);
}

/// The exchange bookkeeping of one daemon. The node feeds it every
/// [`PeerInput`] except `Endpoint`, which the driver ignores.
#[derive(Debug, Default)]
pub struct Driver {
    next_dialed: u64,
    dialed: HashMap<u64, Dialed>,
    accepted: HashMap<u64, Accepted>,
    links: HashMap<(GoalId, EndpointId), Link>,
    backoff: HashMap<EndpointId, Backoff>,
}

#[derive(Debug)]
struct Dialed {
    goal: GoalId,
    endpoint: EndpointId,
    join: bool,
    initiator: Initiator,
}

#[derive(Debug)]
struct Accepted {
    remote: EndpointId,
    responder: Responder,
}

/// One (goal, endpoint) pair this daemon opens exchanges for.
#[derive(Debug, Default)]
struct Link {
    /// The exchange in flight, if any.
    in_flight: Option<u64>,
    /// Run an exchange as soon as none is in flight and backoff allows.
    due: bool,
    /// When the last exchange was opened.
    last_open_ms: Option<u64>,
}

#[derive(Debug)]
struct Backoff {
    delay_ms: u64,
    retry_at_ms: u64,
}

impl Driver {
    /// No exchanges, no history of failures.
    pub fn new() -> Self {
        Self::default()
    }

    /// Handles one input and appends what the transport should do to `out`.
    pub fn handle(
        &mut self,
        host: &mut dyn Host,
        input: PeerInput,
        now_ms: u64,
        out: &mut Vec<PeerOutput>,
    ) {
        let _ = (host, input, now_ms, out);
        todo!()
    }

    /// True while an exchange this daemon opened about `goal` with
    /// `endpoint` is in flight.
    pub fn in_flight(&self, goal: &GoalId, endpoint: &EndpointId) -> bool {
        self.links
            .get(&(*goal, *endpoint))
            .is_some_and(|link| link.in_flight.is_some())
    }
}
