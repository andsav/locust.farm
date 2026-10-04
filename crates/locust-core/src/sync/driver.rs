//! Every exchange of one daemon: which to open, retries, anti-entropy,
//! admission and finishing.

use std::collections::{HashMap, HashSet};

use locust_proto::engine::{ExchangeId, PeerInput, PeerOutput, PeerTime};
use locust_proto::event::WireEvent;
use locust_proto::id::{EndpointId, GoalId, PublicKey};
use locust_proto::invite::JoinRequest;
use locust_proto::sync::{Refusal, SyncMessage};

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
    /// The exact principal whose invitation this exchange presented.
    pub joining_member: Option<PublicKey>,
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

    /// Bounded fork evidence for historical contacts, without ordinary access.
    fn halt_proofs(&self) -> Vec<(GoalId, EndpointId, [WireEvent; 2])> {
        Vec::new()
    }
    /// Whether this authenticated endpoint may deliver author fork evidence.
    fn accepts_halt_proof(&self, _goal: &GoalId, _remote: &EndpointId) -> bool {
        false
    }
    /// Validates and durably holds exactly one author equivocation proof.
    fn receive_halt_proof(
        &mut self,
        _goal: &GoalId,
        _remote: &EndpointId,
        _proof: [WireEvent; 2],
    ) -> Result<(), Refusal> {
        Err(Refusal::NotAMember)
    }

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

    /// A number drawn from the node's entropy, which spreads retries so that
    /// two daemons whose exchange failed together do not retry together.
    fn random(&mut self) -> u64;
}

/// The exchange bookkeeping of one daemon. The node feeds it every
/// [`PeerInput`] except `Endpoint`, which the driver ignores.
///
/// It opens at most one exchange per (goal, endpoint) at a time. A goal that
/// changes while one is in flight runs once more after it ends. An endpoint
/// whose exchange failed is retried after a wait that starts at
/// [`MIN_BACKOFF_MS`] and doubles up to [`MAX_BACKOFF_MS`], each wait cut
/// short by a random part of up to half drawn through [`Host::random`]; a
/// completed exchange clears it, whichever side opened it. Every pair is
/// reconciled at least every [`ANTI_ENTROPY_MS`] even when nothing changed
/// here, which is what finds changes a missed push left behind.
#[derive(Debug, Default)]
pub struct Driver {
    next_dialed: u64,
    dialed: HashMap<u64, Dialed>,
    accepted: HashMap<u64, Responder>,
    links: HashMap<(GoalId, EndpointId), Link>,
    backoff: HashMap<EndpointId, Backoff>,
    /// Frames of the exchange being handled, reused across calls.
    frames: Vec<SyncMessage>,
    finishing_accepted: HashSet<u64>,
    /// Monotonic sample from the current input, never a persisted timestamp.
    elapsed_ms: u64,
}

#[derive(Debug)]
struct Dialed {
    goal: GoalId,
    endpoint: EndpointId,
    join: bool,
    joining_member: Option<PublicKey>,
    initiator: Initiator,
    finishing: Option<Ended>,
    /// A refusal already decoded from the authenticated remote endpoint.
    /// Its fact survives failure to acknowledge our own closing half-stream.
    received_refusal: Option<Refusal>,
    proof: Option<[WireEvent; 2]>,
    proof_sent: bool,
    admitted: bool,
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

    /// Intake is paused while an accepted request still owes its answer.
    pub fn readable(&self, exchange: ExchangeId) -> bool {
        match exchange {
            ExchangeId::Accepted(number) => {
                self.accepted.get(&number).is_none_or(Responder::readable)
            }
            ExchangeId::Dialed(_) => true,
        }
    }

    /// Handles one input and appends what the transport should do to `out`.
    pub fn handle(
        &mut self,
        host: &mut dyn Host,
        input: PeerInput,
        time: PeerTime,
        out: &mut Vec<PeerOutput>,
    ) {
        self.elapsed_ms = time.elapsed_ms;
        let now_ms = time.unix_ms;
        match input {
            PeerInput::Poll => self.poll(host, now_ms, out),
            PeerInput::Writable(ExchangeId::Dialed(number)) => {
                self.dialed_writable(host, number, out)
            }
            PeerInput::Writable(ExchangeId::Accepted(number)) => {
                self.accepted_writable(host, number, out)
            }
            PeerInput::Finished(ExchangeId::Dialed(number)) => {
                if let Some(ended) = self.dialed.get(&number).and_then(|dialed| dialed.finishing) {
                    self.end_dialed(host, number, ended, now_ms);
                }
            }
            PeerInput::Finished(ExchangeId::Accepted(number)) => {
                if self.finishing_accepted.remove(&number)
                    && let Some(responder) = self.accepted.remove(&number)
                {
                    let ended = responder.ended().unwrap_or(Ended::Aborted);
                    end_accepted(host, &responder, ended, now_ms);
                }
            }
            PeerInput::Opened(ExchangeId::Dialed(number)) => {
                self.dialed_step(host, number, None, now_ms, out);
            }
            PeerInput::Frame {
                exchange: ExchangeId::Dialed(number),
                frame,
            } => self.dialed_step(host, number, Some(frame), now_ms, out),
            PeerInput::OpenFailed(ExchangeId::Dialed(number)) => {
                self.end_dialed(host, number, Ended::Aborted, now_ms);
            }
            PeerInput::Closed(ExchangeId::Dialed(number)) => {
                // A remote terminal refusal is an observed protocol fact. The
                // responder can close its unadmitted connection once that
                // frame is acknowledged, before our own FIN is acknowledged.
                // Completed and locally emitted refusals still require finish
                // confirmation; a close alone never proves their delivery.
                let ended = self
                    .dialed
                    .get(&number)
                    .and_then(|dialed| dialed.received_refusal)
                    .map_or(Ended::Aborted, Ended::Refused);
                self.end_dialed(host, number, ended, now_ms);
            }
            PeerInput::Accepted {
                exchange: ExchangeId::Accepted(number),
                remote,
            } => {
                self.accepted.insert(number, Responder::new(remote));
            }
            PeerInput::Frame {
                exchange: ExchangeId::Accepted(number),
                frame,
            } => self.accepted_step(host, number, frame, now_ms, out),
            PeerInput::Closed(ExchangeId::Accepted(number)) => {
                self.finishing_accepted.remove(&number);
                if let Some(responder) = self.accepted.remove(&number) {
                    end_accepted(host, &responder, Ended::Aborted, now_ms);
                }
            }
            // `Endpoint` is the node's, and the shell never pairs an input
            // with the other side's numbering.
            _ => {}
        }
    }

    /// True while an exchange this daemon opened about `goal` with
    /// `endpoint` is in flight.
    pub fn in_flight(&self, goal: &GoalId, endpoint: &EndpointId) -> bool {
        self.links
            .get(&(*goal, *endpoint))
            .is_some_and(|link| link.in_flight.is_some())
    }

    /// Marks changed goals due and opens every exchange that is due: changed,
    /// failed earlier, or not reconciled for [`ANTI_ENTROPY_MS`]. Joins take
    /// the place of the ordinary exchange for their (goal, endpoint).
    fn poll(&mut self, host: &mut dyn Host, _now_ms: u64, out: &mut Vec<PeerOutput>) {
        let mut changed = host.take_changed();
        changed.sort_unstable();
        let joins = host.joins();
        let proofs = host.halt_proofs();
        let mut live = HashSet::new();
        let pairs = joins
            .iter()
            .map(|join| ((join.goal, join.endpoint), Some(join)))
            .chain(host.peers().into_iter().map(|pair| (pair, None)))
            .chain(
                proofs
                    .iter()
                    .map(|(goal, endpoint, _)| ((*goal, *endpoint), None)),
            );
        for (pair, join) in pairs {
            if !live.insert(pair) {
                continue;
            }
            let link = self.links.entry(pair).or_default();
            link.due |= changed.binary_search(&pair.0).is_ok()
                || link
                    .last_open_ms
                    .is_none_or(|last| self.elapsed_ms.saturating_sub(last) >= ANTI_ENTROPY_MS);
            let ready = self
                .backoff
                .get(&pair.1)
                .is_none_or(|backoff| self.elapsed_ms >= backoff.retry_at_ms);
            if !link.due || link.in_flight.is_some() || !ready {
                continue;
            }
            let number = self.next_dialed;
            self.next_dialed += 1;
            link.due = false;
            link.in_flight = Some(number);
            link.last_open_ms = Some(self.elapsed_ms);
            let (mut initiator, hints) = match join {
                Some(join) => (Initiator::joining(join.request.clone()), join.hints.clone()),
                None => (Initiator::new(pair.0), host.hints(&pair.1)),
            };
            initiator.set_remote(pair.1);
            self.dialed.insert(
                number,
                Dialed {
                    goal: pair.0,
                    endpoint: pair.1,
                    join: join.is_some(),
                    joining_member: join.map(|join| join.request.member),
                    initiator,
                    finishing: None,
                    received_refusal: None,
                    proof: proofs
                        .iter()
                        .find(|(goal, endpoint, _)| (*goal, *endpoint) == pair)
                        .map(|(_, _, proof)| proof.clone()),
                    proof_sent: false,
                    admitted: false,
                },
            );
            out.push(PeerOutput::Open {
                exchange: ExchangeId::Dialed(number),
                endpoint: pair.1,
                hints,
            });
        }
        // Pairs no longer a member or a join are forgotten once idle.
        self.links
            .retain(|pair, link| link.in_flight.is_some() || live.contains(pair));
    }

    /// Starts a dialed exchange once it is open (`frame` is `None`), or feeds
    /// it the next frame; finishes it after its last frame.
    fn dialed_step(
        &mut self,
        host: &mut dyn Host,
        number: u64,
        frame: Option<SyncMessage>,
        _now_ms: u64,
        out: &mut Vec<PeerOutput>,
    ) {
        let Some(dialed) = self.dialed.get_mut(&number) else {
            return;
        };
        if dialed.finishing.is_some() {
            return;
        }
        let exchange = ExchangeId::Dialed(number);
        if dialed.proof.is_some() || dialed.proof_sent {
            if frame.is_none() {
                out.push(PeerOutput::Evidence(exchange));
                out.push(PeerOutput::Send {
                    exchange,
                    frame: SyncMessage::Hello {
                        version: locust_proto::PROTOCOL_VERSION,
                        goal: dialed.goal,
                    },
                });
            } else if let Some(SyncMessage::Refused(reason)) = frame {
                dialed.finishing = Some(Ended::Refused(reason));
                out.push(PeerOutput::Finish(exchange));
            }
            return;
        }
        let authorized = host.speaks_for_member(&dialed.goal, &dialed.endpoint);
        if authorized && !dialed.admitted {
            out.push(PeerOutput::Admit(exchange));
            dialed.admitted = true;
        }
        dialed.initiator.authorize_outbound(authorized);
        let received_refusal = match &frame {
            Some(SyncMessage::Refused(reason)) => Some(*reason),
            _ => None,
        };
        let ended = match host.replica(&dialed.goal) {
            Some(replica) => {
                match frame {
                    None => dialed.initiator.start(replica, &mut self.frames),
                    Some(frame) => dialed.initiator.receive(replica, frame, &mut self.frames),
                }
                send(exchange, &mut self.frames, out);
                dialed.initiator.ended()
            }
            // The goal is gone; end without a word.
            None => Some(Ended::Aborted),
        };
        if let Some(reason) = received_refusal
            && ended == Some(Ended::Refused(reason))
        {
            dialed.received_refusal = Some(reason);
        }
        if let Some(ended) = ended {
            out.push(PeerOutput::Finish(exchange));
            dialed.finishing = Some(ended);
        }
    }

    /// Forgets a dialed exchange, frees its pair, and backs off its endpoint
    /// unless it completed.
    fn end_dialed(&mut self, host: &mut dyn Host, number: u64, ended: Ended, now_ms: u64) {
        let Some(dialed) = self.dialed.remove(&number) else {
            return;
        };
        let pair = (dialed.goal, dialed.endpoint);
        let link = self.links.entry(pair).or_default();
        if link.in_flight == Some(number) {
            link.in_flight = None;
        }
        if ended == Ended::Completed {
            self.backoff.remove(&dialed.endpoint);
        } else {
            link.due = true;
            let backoff = self.backoff.entry(dialed.endpoint).or_insert(Backoff {
                delay_ms: 0,
                retry_at_ms: 0,
            });
            // Exchanges opened before the last failure do not double it.
            if self.elapsed_ms >= backoff.retry_at_ms {
                backoff.delay_ms = (backoff.delay_ms * 2).clamp(MIN_BACKOFF_MS, MAX_BACKOFF_MS);
                // Both ends of a failed exchange back off; jitter keeps them
                // from retrying in the same order again and again.
                let jitter = host.random() % (backoff.delay_ms / 2);
                backoff.retry_at_ms = self.elapsed_ms.saturating_add(backoff.delay_ms - jitter);
            }
        }
        host.exchange_ended(Report {
            goal: dialed.goal,
            endpoint: dialed.endpoint,
            dialed: true,
            join: dialed.join,
            joining_member: dialed.joining_member,
            ended,
            at_ms: now_ms,
        });
    }

    /// A member's endpoint just completed an exchange it opened, so it is
    /// reachable now: its backoff is cleared. Content and keys travel only on
    /// exchanges this daemon opens, so when the exchange brought new events
    /// and the goal still wants either, one opens at once rather than when
    /// the backoff or anti-entropy says. Without new events nothing is
    /// opened, so two daemons that want content neither holds do not keep
    /// answering each other.
    fn peer_completed(
        &mut self,
        host: &mut dyn Host,
        goal: GoalId,
        endpoint: EndpointId,
        received: bool,
        now_ms: u64,
        out: &mut Vec<PeerOutput>,
    ) {
        self.backoff.remove(&endpoint);
        let wants = received
            && host.replica(&goal).is_some_and(|replica| {
                replica.next_wanted_blob(None).is_some() || !replica.wanted_keys().is_empty()
            });
        if wants {
            self.links.entry((goal, endpoint)).or_default().due = true;
            self.poll(host, now_ms, out);
        }
    }

    /// Feeds an accepted exchange its next frame. Emits `Admit` as soon as
    /// the remote endpoint speaks for a member, before the answer.
    fn accepted_step(
        &mut self,
        host: &mut dyn Host,
        number: u64,
        frame: SyncMessage,
        now_ms: u64,
        out: &mut Vec<PeerOutput>,
    ) {
        if self.finishing_accepted.contains(&number) {
            return;
        }
        let Some(responder) = self.accepted.get_mut(&number) else {
            return;
        };
        let exchange = ExchangeId::Accepted(number);
        let admitted = responder.is_admitted();
        let evidence = responder.is_evidence();
        responder.receive(host, frame, now_ms, &mut self.frames);
        if !admitted && responder.is_admitted() {
            out.push(PeerOutput::Admit(exchange));
        }
        if !evidence && responder.is_evidence() && !responder.is_admitted() {
            out.push(PeerOutput::Evidence(exchange));
        }
        send(exchange, &mut self.frames, out);
        if let Some(ended) = responder.ended() {
            out.push(PeerOutput::Finish(exchange));
            self.finishing_accepted.insert(number);
            // The peer's `Done` arrived, whatever the transport makes of
            // the finish that follows.
            if let (Ended::Completed, Some(goal), true) =
                (ended, responder.goal(), responder.is_admitted())
            {
                let (endpoint, received) = (responder.remote(), responder.received());
                self.peer_completed(host, goal, endpoint, received, now_ms, out);
            }
        }
    }
    fn dialed_writable(&mut self, host: &mut dyn Host, number: u64, out: &mut Vec<PeerOutput>) {
        let Some(dialed) = self.dialed.get_mut(&number) else {
            return;
        };
        if dialed.finishing.is_some() {
            return;
        }
        let exchange = ExchangeId::Dialed(number);
        if let Some(proof) = dialed.proof.take() {
            dialed.proof_sent = true;
            out.push(PeerOutput::Send {
                exchange,
                frame: SyncMessage::HaltProof(proof),
            });
            return;
        }
        if dialed.proof_sent {
            out.push(PeerOutput::Send {
                exchange,
                frame: SyncMessage::Done,
            });
            out.push(PeerOutput::Finish(exchange));
            dialed.finishing = Some(Ended::Completed);
            return;
        }
        let authorized = host.speaks_for_member(&dialed.goal, &dialed.endpoint);
        if authorized && !dialed.admitted {
            out.push(PeerOutput::Admit(exchange));
            dialed.admitted = true;
        }
        dialed.initiator.authorize_outbound(authorized);
        if let Some(replica) = host.replica(&dialed.goal) {
            dialed.initiator.writable(replica, &mut self.frames);
            send(exchange, &mut self.frames, out);
            if let Some(ended) = dialed.initiator.ended() {
                dialed.finishing = Some(ended);
                out.push(PeerOutput::Finish(exchange));
            }
        }
    }

    fn accepted_writable(&mut self, host: &mut dyn Host, number: u64, out: &mut Vec<PeerOutput>) {
        if self.finishing_accepted.contains(&number) {
            return;
        }
        let Some(responder) = self.accepted.get_mut(&number) else {
            return;
        };
        let exchange = ExchangeId::Accepted(number);
        responder.writable(host, &mut self.frames);
        send(exchange, &mut self.frames, out);
        if responder.ended().is_some() {
            self.finishing_accepted.insert(number);
            out.push(PeerOutput::Finish(exchange));
        }
    }
}

fn send(exchange: ExchangeId, frames: &mut Vec<SyncMessage>, out: &mut Vec<PeerOutput>) {
    out.extend(
        frames
            .drain(..)
            .map(|frame| PeerOutput::Send { exchange, frame }),
    );
}

/// Reports an accepted exchange whose remote endpoint was admitted; the rest
/// concern no peer of the goal.
fn end_accepted(host: &mut dyn Host, responder: &Responder, ended: Ended, now_ms: u64) {
    if let (Some(goal), true) = (responder.goal(), responder.is_admitted()) {
        host.exchange_ended(Report {
            goal,
            endpoint: responder.remote(),
            dialed: false,
            join: false,
            joining_member: None,
            ended,
            at_ms: now_ms,
        });
    }
}
