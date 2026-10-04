//! The side of an exchange that a peer opened.

use locust_proto::id::{EndpointId, GoalId};
use locust_proto::sync::SyncMessage;

use super::Host;

/// Serves one accepted exchange. Fed each received frame in order, it
/// appends the frames that answer it; nothing is sent unasked.
#[derive(Debug)]
pub struct Responder {
    remote: EndpointId,
    state: State,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Waiting for `Hello`.
    Hello,
    /// `Hello` named this goal; the remote endpoint does not speak for a
    /// member of it, so only `Join` is served.
    Stranger(GoalId),
    /// The remote endpoint speaks for a member of this goal.
    Member(GoalId),
    /// The exchange is over.
    Finished,
}

impl Responder {
    /// An exchange accepted from the authenticated endpoint `remote`.
    pub fn new(remote: EndpointId) -> Self {
        Self {
            remote,
            state: State::Hello,
        }
    }

    /// The goal the accepted `Hello` named.
    pub fn goal(&self) -> Option<GoalId> {
        match self.state {
            State::Stranger(goal) | State::Member(goal) => Some(goal),
            State::Hello | State::Finished => None,
        }
    }

    /// True once the remote endpoint speaks for a member of the goal, so its
    /// frames are read at the peer limit.
    pub fn is_admitted(&self) -> bool {
        matches!(self.state, State::Member(_))
    }

    /// True once the exchange is over: after `Done`, or after the refusal
    /// this machine sent last.
    pub fn is_finished(&self) -> bool {
        self.state == State::Finished
    }

    /// Handles the next received frame and appends its answer to `out`.
    pub fn receive(
        &mut self,
        host: &mut dyn Host,
        frame: SyncMessage,
        now_ms: u64,
        out: &mut Vec<SyncMessage>,
    ) {
        let _ = (host, frame, now_ms, out, self.remote);
        todo!()
    }
}
