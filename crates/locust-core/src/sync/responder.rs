//! The side of an exchange that a peer opened.

use locust_proto::PROTOCOL_VERSION;
use locust_proto::event::Event;
use locust_proto::id::{BlobHash, EndpointId, GoalId};
use locust_proto::invite::JoinRequest;
use locust_proto::sync::{Refusal, SyncMessage};

use super::outbox::{Outbox, Work, governance_first};
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
    evidence: bool,
    received: bool,
    /// The remote's frontier was served: the exchange has a record stage.
    reconciling: bool,
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
            evidence: false,
            received: false,
            reconciling: false,
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

    pub fn is_evidence(&self) -> bool {
        self.evidence
    }

    /// True once the peer pushed an event this daemon did not hold, or
    /// delivered a halt proof holding one.
    pub fn received(&self) -> bool {
        self.received
    }

    /// True when the exchange counts as hearing from the peer: it ran a
    /// record stage, from the peer's frontier, and brought nothing this
    /// daemon lacked. Read once it has completed. An exchange that only
    /// delivered a halt proof never does.
    pub fn reconciled(&self) -> bool {
        self.reconciling && !self.received
    }

    /// True once the exchange is over: after `Done`, or after a refusal.
    pub fn is_finished(&self) -> bool {
        self.ended.is_some()
    }

    /// How the exchange ended, once it has.
    pub fn ended(&self) -> Option<Ended> {
        (!self.outbox.pending()).then_some(self.ended).flatten()
    }

    pub fn readable(&self) -> bool {
        self.outbox.readable() || self.ended.is_some()
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
        let outcome = if !self.readable()
            && !matches!(frame, SyncMessage::Done | SyncMessage::Refused(_))
        {
            Err(Refusal::ProtocolError)
        } else {
            match (self.goal, frame) {
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
                (Some(goal), SyncMessage::HaltProof(proof)) => {
                    let lacked = host.replica(&goal).is_some_and(|replica| {
                        proof.iter().any(|wire| {
                            Event::from_wire(wire)
                                .is_ok_and(|event| replica.wire_event(&event.id()).is_none())
                        })
                    });
                    let landed = host.receive_halt_proof(&goal, &self.remote, proof);
                    self.received |= landed.is_ok() && lacked;
                    landed
                }
                (Some(_), _) if !self.admitted => Err(Refusal::NotAMember),
                (Some(goal), request) => self.serve(host, goal, request),
            }
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

    fn hello(&mut self, host: &mut dyn Host, version: u8, goal: GoalId) -> Result<(), Refusal> {
        if version != PROTOCOL_VERSION {
            return Err(Refusal::UnsupportedVersion);
        }
        self.goal = Some(goal);
        self.admitted = host.speaks_for_member(&goal, &self.remote);
        self.evidence = host.accepts_halt_proof(&goal, &self.remote);
        if !self.admitted {
            host.note_caller(&goal, &self.remote);
        }
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
            SyncMessage::DeliverEffect { effect, recipient } => {
                let received = replica.receive_delivery(effect, recipient)?;
                self.outbox.push(SyncMessage::EffectReceipt {
                    effect,
                    recipient,
                    received,
                });
            }
            SyncMessage::Frontier(theirs) => {
                self.reconciling = true;
                let mine = replica.frontier();
                let queue = governance_first(replica, &mine);
                self.outbox.task(Work::Frontier {
                    theirs,
                    mine,
                    queue,
                });
            }
            SyncMessage::Events(events) => {
                self.received |= replica.receive(events)? > 0;
            }
            SyncMessage::InventoryRequest { author, after } => {
                self.outbox.task(Work::Inventory { author, after });
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
