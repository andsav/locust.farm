//! The side of an exchange that a peer opened.

use locust_proto::PROTOCOL_VERSION;
use locust_proto::event::AuthorPoint;
use locust_proto::id::{BlobHash, EndpointId, GoalId, PublicKey};
use locust_proto::invite::JoinRequest;
use locust_proto::limits::MAX_INVENTORY_POINTS;
use locust_proto::sync::{Frontier, Refusal, SyncMessage};

use super::outbox::{Outbox, Work};
use super::{Ended, Host, Replica};

/// Serves one accepted exchange. Fed each received frame in order, it
/// appends the frames that answer it; nothing is sent unasked.
///
/// Before the remote endpoint is known to speak for a member of the goal its
/// `Hello` names, only `Join` is served; any other request is refused with
/// `NotAMember`, also for a goal this daemon does not hold.
#[derive(Debug)]
pub struct Responder {
    remote: EndpointId,
    goal: Option<GoalId>,
    admitted: bool,
    ended: Option<Ended>,
    outbox: Outbox,
}

impl Responder {
    /// An exchange accepted from the authenticated endpoint `remote`.
    pub fn new(remote: EndpointId) -> Self {
        Self {
            remote,
            goal: None,
            admitted: false,
            ended: None,
            outbox: Outbox::default(),
        }
    }

    /// The authenticated remote endpoint.
    pub fn remote(&self) -> EndpointId {
        self.remote
    }

    /// The goal the accepted `Hello` named.
    pub fn goal(&self) -> Option<GoalId> {
        self.goal
    }

    /// True once the remote endpoint speaks for a member of the goal, so its
    /// frames are read at the peer limit.
    pub fn is_admitted(&self) -> bool {
        self.admitted
    }

    /// True once the exchange is over: after `Done`, or after a refusal.
    pub fn is_finished(&self) -> bool {
        self.ended.is_some()
    }

    /// How the exchange ended, once it has.
    pub fn ended(&self) -> Option<Ended> {
        (!self.outbox.pending()).then_some(self.ended).flatten()
    }

    /// Handles the next received frame and appends its answer to `out`.
    /// Frames after the end are ignored.
    pub fn receive(
        &mut self,
        host: &mut dyn Host,
        frame: SyncMessage,
        now_ms: u64,
        out: &mut Vec<SyncMessage>,
    ) {
        if self.ended.is_some() {
            return;
        }
        let outcome = match (self.goal, frame) {
            (None, SyncMessage::Hello { version, goal }) => self.hello(host, version, goal),
            (None, _) | (Some(_), SyncMessage::Hello { .. }) => Err(Refusal::ProtocolError),
            (Some(goal), SyncMessage::Join(request)) => self.join(host, goal, &request, now_ms),
            (Some(_), SyncMessage::Done) => {
                self.outbox.clear();
                self.ended = Some(Ended::Completed);
                Ok(())
            }
            // The initiator gave up; there is nothing to answer.
            (Some(_), SyncMessage::Refused(refusal)) => {
                self.outbox.clear();
                self.ended = Some(Ended::Refused(refusal));
                Ok(())
            }
            (Some(_), _) if !self.admitted => Err(Refusal::NotAMember),
            (Some(goal), request) => self.serve(host, goal, request),
        };
        if let Err(refusal) = outcome {
            self.outbox.clear();
            self.outbox.push(SyncMessage::Refused(refusal));
            self.ended = Some(Ended::Refused(refusal));
        }
        self.pump(host, out);
    }

    pub fn writable(&mut self, host: &mut dyn Host, out: &mut Vec<SyncMessage>) {
        self.outbox.writable();
        self.pump(host, out);
    }

    fn pump(&mut self, host: &mut dyn Host, out: &mut Vec<SyncMessage>) {
        if let Some(goal) = self.goal {
            if self.admitted && !host.speaks_for_member(&goal, &self.remote) {
                self.outbox.clear();
                self.outbox.push(SyncMessage::Refused(Refusal::NotAMember));
                self.ended = Some(Ended::Refused(Refusal::NotAMember));
                self.admitted = false;
            }
            if let Some(replica) = host.replica(&goal) {
                self.outbox.pump(replica, out);
            } else {
                self.outbox.pump_frame(out);
            }
        } else {
            self.outbox.pump_frame(out);
        }
    }

    fn hello(&mut self, host: &dyn Host, version: u8, goal: GoalId) -> Result<(), Refusal> {
        if version != PROTOCOL_VERSION {
            return Err(Refusal::UnsupportedVersion);
        }
        self.goal = Some(goal);
        self.admitted = host.speaks_for_member(&goal, &self.remote);
        Ok(())
    }

    /// Admission is the host's; an admitted key, new or repeating its join,
    /// is answered with this daemon's frontier.
    fn join(
        &mut self,
        host: &mut dyn Host,
        goal: GoalId,
        request: &JoinRequest,
        now_ms: u64,
    ) -> Result<(), Refusal> {
        if request.goal != goal {
            return Err(Refusal::ProtocolError);
        }
        host.join(&self.remote, request, now_ms)?;
        let replica = host.replica(&goal).ok_or(Refusal::NotAMember)?;
        self.outbox.push(SyncMessage::Frontier(replica.frontier()));
        self.admitted = true;
        Ok(())
    }

    fn serve(
        &mut self,
        host: &mut dyn Host,
        goal: GoalId,
        request: SyncMessage,
    ) -> Result<(), Refusal> {
        // Keys go only to a current member, checked again per request.
        if !host.speaks_for_member(&goal, &self.remote) {
            return Err(Refusal::NotAMember);
        }
        let replica = host.replica(&goal).ok_or(Refusal::NotAMember)?;
        match request {
            SyncMessage::Frontier(theirs) => answer_frontier(replica, &theirs, &mut self.outbox),
            SyncMessage::Events(events) => {
                replica.receive(events)?;
            }
            SyncMessage::InventoryRequest { author, after } => {
                self.outbox.push(inventory(replica, author, after));
            }
            SyncMessage::EventRequest(ids) => {
                self.outbox.task(Work::Requested(ids.into()));
            }
            SyncMessage::KeyRequest { epoch } => self.outbox.push(match replica.key(epoch) {
                Some(key) => SyncMessage::Key { epoch, key },
                None => SyncMessage::Refused(Refusal::KeyUnavailable),
            }),
            SyncMessage::BlobRequest { hash, offset } => {
                serve_blob(replica, hash, offset, &mut self.outbox)
            }
            _ => return Err(Refusal::ProtocolError),
        }
        Ok(())
    }
}

/// The answer to a peer's frontier: for every author either side holds, the
/// events past the peer's prefix when the peer holds exactly a prefix of this
/// replica's log, otherwise the first page of this replica's inventory; then
/// this replica's frontier.
fn answer_frontier(replica: &dyn Replica, theirs: &Frontier, out: &mut Outbox) {
    let mine = replica.frontier();
    let (mut i, mut j) = (0, 0);
    loop {
        let author = match (mine.authors.get(i), theirs.authors.get(j)) {
            (Some(a), Some(b)) if a.author == b.author => {
                i += 1;
                j += 1;
                a.author
            }
            (Some(a), Some(b)) if a.author < b.author => {
                i += 1;
                a.author
            }
            (Some(a), None) => {
                i += 1;
                a.author
            }
            (_, Some(b)) => {
                j += 1;
                b.author
            }
            (None, None) => break,
        };
        let entry = theirs.get(&author);
        if replica.extends(&entry) {
            let points = replica.points(&author);
            let from = points.partition_point(|point| point.seq < entry.next_seq);
            let after = from.checked_sub(1).map(|i| points[i]);
            out.events(replica, author, after, None, Vec::new());
        } else {
            out.push(inventory(replica, author, None));
        }
    }
    out.push(SyncMessage::Frontier(mine));
}

/// One page of this replica's points of `author` strictly after `after`.
fn inventory(replica: &dyn Replica, author: PublicKey, after: Option<AuthorPoint>) -> SyncMessage {
    let points = replica.points(&author);
    let start = after.map_or(0, |after| points.partition_point(|point| *point <= after));
    let end = points.len().min(start + MAX_INVENTORY_POINTS);
    SyncMessage::Inventory {
        author,
        points: points[start..end].to_vec(),
        more: end < points.len(),
    }
}

/// Chunks of a served object running contiguously from `offset` to its end,
/// one empty chunk when `offset` is the end, or `BlobUnavailable`.
fn serve_blob(replica: &dyn Replica, hash: BlobHash, offset: u64, out: &mut Outbox) {
    let Some(total) = replica.blob_len(&hash).filter(|total| offset <= *total) else {
        out.push(SyncMessage::BlobUnavailable(hash));
        return;
    };
    out.task(Work::Blob {
        hash,
        offset,
        total,
    });
}
