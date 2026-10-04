//! The side of an exchange that this daemon opened.

use std::collections::VecDeque;

use locust_proto::id::{GoalId, PublicKey};
use locust_proto::invite::JoinRequest;
use locust_proto::sync::SyncMessage;

use super::{Ended, Replica};

/// Most content objects one exchange asks for; the rest wait for the next.
pub const MAX_BLOBS_PER_EXCHANGE: usize = 256;

/// Runs one exchange this daemon opened about one goal.
#[derive(Debug)]
pub struct Initiator {
    goal: GoalId,
    join: Option<JoinRequest>,
    ended: Option<Ended>,
}

impl Initiator {
    /// An exchange reconciling `goal`.
    pub fn new(goal: GoalId) -> Self {
        Self {
            goal,
            join: None,
            ended: None,
        }
    }

    /// An exchange that presents `request` to be admitted to its goal, then
    /// reconciles it.
    pub fn joining(request: JoinRequest) -> Self {
        Self {
            goal: request.goal,
            join: Some(request),
            ended: None,
        }
    }

    /// The goal this exchange is about.
    pub fn goal(&self) -> GoalId {
        self.goal
    }

    /// How the exchange ended, once it has.
    pub fn ended(&self) -> Option<Ended> {
        self.ended
    }

    /// The first frames, once the stream is open.
    pub fn start(&mut self, replica: &dyn Replica, out: &mut Vec<SyncMessage>) {
        let _ = (replica, out);
        todo!()
    }

    /// Handles the next received frame and appends what to send to `out`.
    pub fn receive(
        &mut self,
        replica: &mut dyn Replica,
        frame: SyncMessage,
        out: &mut Vec<SyncMessage>,
    ) {
        let _ = (replica, frame, out);
        let _: Option<(VecDeque<()>, PublicKey)> = None;
        todo!()
    }
}
