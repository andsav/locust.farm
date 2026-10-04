//! The side of an exchange that this daemon opened.

use std::collections::{HashSet, VecDeque};

use locust_proto::PROTOCOL_VERSION;
use locust_proto::event::{AuthorPoint, WireEvent};
use locust_proto::id::{BlobHash, EffectId, EndpointId, GoalId, PublicKey};
use locust_proto::invite::JoinRequest;
use locust_proto::limits::MAX_EVENTS_PER_BATCH;
use locust_proto::sync::{Frontier, Refusal, SyncMessage};

use super::outbox::Outbox;
use super::{Ended, Replica, Staged};

/// Runs one exchange this daemon opened about one goal: `Hello` (and `Join`
/// when joining), its frontier, the answer and the pushes and requests the
/// reconciliation rule calls for, then the founding object and missing keys,
/// remaining objects one at a time, another pass for still-missing keys, and
/// `Done`. The early key pass makes the founding title readable before bulk
/// content; the final pass can validate keys against newly fetched epoch proofs.
///
/// Requests are pipelined: the responder answers in order, so the machine
/// keeps a queue of the answers it expects, at most two per author
/// inventoried plus one per wanted key.
#[derive(Debug)]
pub struct Initiator {
    goal: GoalId,
    join: Option<JoinRequest>,
    stage: Stage,
    expect: VecDeque<Expect>,
    /// Authors the responder sent an inventory of in its answer: what they
    /// lack is pushed page by page, not by the prefix rule.
    inventoried: HashSet<PublicKey>,
    blob_cursor: Option<BlobHash>,
    remote: Option<EndpointId>,
    delivery_cursor: Option<(EffectId, PublicKey)>,
    outbox: Outbox,
    ended: Option<Ended>,
    /// The authenticated endpoint is a current member according to signed
    /// local state. An invitation or an answering frontier is not proof.
    outbound_authorized: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Start,
    Reconcile,
    Founding,
    EarlyKeys,
    Blobs,
    Keys,
    Deliveries,
    Finished,
}

/// One answer the responder owes, in the order it will come.
#[derive(Clone, Copy, Debug)]
enum Expect {
    Receipt(EffectId, PublicKey),
    /// The answer to `Join`: the responder's frontier.
    Join,
    /// The answer to the frontier: `Events` and `Inventory` frames, then the
    /// responder's frontier.
    Answer,
    /// This many more `Events` frames answering one `EventRequest`.
    Events(usize),
    /// The page of `author`'s inventory after `after`.
    Inventory {
        author: PublicKey,
        after: Option<AuthorPoint>,
    },
    /// The key of one epoch.
    Key(u32),
    /// Chunks of an object from `next`; staged while `keep`.
    Blob {
        hash: BlobHash,
        next: u64,
        total: Option<u64>,
        keep: bool,
        /// One zero-offset retry recovers an unusable resume. Merely hearing
        /// unavailable never deletes staging another exchange may be using.
        retried: bool,
    },
}

/// Why the exchange ends early.
enum Fault {
    /// The responder refused; nothing more is sent.
    Received(Refusal),
    /// This side refuses what it received, and says so.
    Sent(Refusal),
}

const BROKEN: Fault = Fault::Sent(Refusal::ProtocolError);

impl Initiator {
    /// An exchange reconciling `goal`.
    pub fn new(goal: GoalId) -> Self {
        Self {
            goal,
            join: None,
            stage: Stage::Start,
            expect: VecDeque::new(),
            inventoried: HashSet::new(),
            blob_cursor: None,
            remote: None,
            delivery_cursor: None,
            outbox: Outbox::default(),
            ended: None,
            outbound_authorized: true,
        }
    }

    /// An exchange that presents `request` to be admitted to its goal, then
    /// treats the answering frontier as the start of a reconciliation.
    pub fn joining(request: JoinRequest) -> Self {
        let mut initiator = Self::new(request.goal);
        initiator.join = Some(request);
        initiator.outbound_authorized = false;
        initiator
    }

    pub(super) fn set_remote(&mut self, remote: EndpointId) {
        self.remote = Some(remote);
    }

    pub(super) fn authorize_outbound(&mut self, authorized: bool) {
        if !authorized && self.outbound_authorized && self.stage != Stage::Start {
            self.outbox.clear();
            self.outbox.push(SyncMessage::Refused(Refusal::NotAMember));
            self.ended = Some(Ended::Refused(Refusal::NotAMember));
        }
        self.outbound_authorized = authorized;
    }

    /// The goal this exchange is about.
    pub fn goal(&self) -> GoalId {
        self.goal
    }

    /// How the exchange ended, once it has. After `Completed` the last frame
    /// sent was `Done`.
    pub fn ended(&self) -> Option<Ended> {
        (!self.outbox.pending()).then_some(self.ended).flatten()
    }

    /// The first frames, once the stream is open. Called once.
    pub fn start(&mut self, replica: &dyn Replica, out: &mut Vec<SyncMessage>) {
        if self.stage != Stage::Start {
            return;
        }
        self.stage = Stage::Reconcile;
        self.outbox.push(SyncMessage::Hello {
            version: PROTOCOL_VERSION,
            goal: self.goal,
        });
        match self.join.take() {
            Some(request) => {
                self.outbox.push(SyncMessage::Join(request));
                self.expect.push_back(Expect::Join);
            }
            None => self.send_frontier(replica),
        }
        self.outbox.pump(replica, out);
    }

    /// Handles the next received frame and appends what to send to `out`.
    /// Frames after the end are ignored.
    pub fn receive(
        &mut self,
        replica: &mut dyn Replica,
        frame: SyncMessage,
        out: &mut Vec<SyncMessage>,
    ) {
        if self.ended.is_some() || self.stage == Stage::Start {
            return;
        }
        match self.step(replica, frame) {
            Ok(()) => self.advance(replica),
            Err(Fault::Received(refusal)) => {
                self.outbox.clear();
                self.ended = Some(Ended::Refused(refusal));
            }
            Err(Fault::Sent(refusal)) => {
                self.outbox.clear();
                self.outbox.push(SyncMessage::Refused(refusal));
                self.ended = Some(Ended::Refused(refusal));
            }
        }
        self.outbox.pump(replica, out);
    }

    pub fn writable(&mut self, replica: &dyn Replica, out: &mut Vec<SyncMessage>) {
        self.outbox.writable();
        if self.ended.is_none() {
            self.advance(replica);
        }
        self.outbox.pump(replica, out);
    }

    fn send_frontier(&mut self, replica: &dyn Replica) {
        self.outbox
            .push(SyncMessage::Frontier(if self.outbound_authorized {
                replica.frontier()
            } else {
                Frontier::default()
            }));
        self.expect.push_back(Expect::Answer);
    }

    fn step(&mut self, replica: &mut dyn Replica, frame: SyncMessage) -> Result<(), Fault> {
        let expect = *self.expect.front().ok_or(BROKEN)?;
        match (expect, frame) {
            (
                Expect::Receipt(effect, recipient),
                SyncMessage::EffectReceipt {
                    effect: given,
                    recipient: addressed,
                    received,
                },
            ) if effect == given && recipient == addressed => {
                self.expect.pop_front();
                if received {
                    replica
                        .receive_receipt(self.remote.ok_or(BROKEN)?, effect, recipient)
                        .map_err(Fault::Sent)?;
                }
            }
            (Expect::Join, SyncMessage::Frontier(_)) => {
                self.expect.pop_front();
                self.send_frontier(replica);
            }
            (Expect::Answer, SyncMessage::Events(events)) => receive(replica, events)?,
            (
                Expect::Answer,
                SyncMessage::Inventory {
                    author,
                    points,
                    more,
                },
            ) => {
                if !self.inventoried.insert(author) {
                    return Err(BROKEN);
                }
                self.inventory(replica, author, None, &points, more)?;
            }
            (Expect::Answer, SyncMessage::Frontier(theirs)) => {
                self.expect.pop_front();
                self.push_prefixes(replica, &theirs);
            }
            (Expect::Events(left), SyncMessage::Events(events)) => {
                receive(replica, events)?;
                match self.expect.front_mut() {
                    Some(Expect::Events(count)) if left > 1 => *count -= 1,
                    _ => drop(self.expect.pop_front()),
                }
            }
            (
                Expect::Inventory { author, after },
                SyncMessage::Inventory {
                    author: listed,
                    points,
                    more,
                },
            ) if listed == author => {
                self.expect.pop_front();
                self.inventory(replica, author, after, &points, more)?;
            }
            (Expect::Key(epoch), SyncMessage::Key { epoch: given, key }) if given == epoch => {
                self.expect.pop_front();
                // A key that opens nothing held is dropped; another member
                // or a later exchange supplies the right one.
                replica.offer_key(epoch, key);
            }
            (Expect::Key(_), SyncMessage::Refused(Refusal::KeyUnavailable)) => {
                self.expect.pop_front();
            }
            (
                Expect::Blob { hash, .. },
                SyncMessage::BlobChunk {
                    hash: given,
                    offset,
                    total,
                    bytes,
                },
            ) if given == hash => self.chunk(replica, offset, total, &bytes)?,
            (
                Expect::Blob {
                    hash,
                    next,
                    retried,
                    ..
                },
                SyncMessage::BlobUnavailable(given),
            ) if given == hash => {
                self.expect.pop_front();
                if next > 0 && !retried {
                    self.outbox
                        .push(SyncMessage::BlobRequest { hash, offset: 0 });
                    self.expect.push_front(Expect::Blob {
                        hash,
                        next: 0,
                        total: None,
                        keep: true,
                        retried: true,
                    });
                }
            }
            (_, SyncMessage::Refused(refusal)) => return Err(Fault::Received(refusal)),
            _ => return Err(BROKEN),
        }
        Ok(())
    }

    /// One page of the responder's points of `author` strictly after `after`,
    /// running to `points`' last when `more` and to the end otherwise. Within
    /// that range, requests the events this replica lacks and pushes those
    /// the responder lacks; then asks for the next page.
    fn inventory(
        &mut self,
        replica: &dyn Replica,
        author: PublicKey,
        after: Option<AuthorPoint>,
        points: &[AuthorPoint],
        more: bool,
    ) -> Result<(), Fault> {
        let last = points.last().copied();
        if (more && last.is_none()) || after.zip(points.first()).is_some_and(|(a, f)| *f <= a) {
            return Err(BROKEN);
        }
        let mine = replica.points(&author);
        let missing: Vec<_> = points
            .iter()
            .filter(|point| mine.binary_search(point).is_err())
            .map(|point| point.id)
            .collect();
        let end = if more { last } else { mine.last().copied() };
        if self.outbound_authorized {
            self.outbox
                .events(replica, author, after, end, points.to_vec());
        }

        if !missing.is_empty() {
            let frames = missing.len().div_ceil(MAX_EVENTS_PER_BATCH);
            self.outbox.push(SyncMessage::EventRequest(missing));
            self.expect.push_back(Expect::Events(frames));
        }
        if more {
            self.outbox.push(SyncMessage::InventoryRequest {
                author,
                after: last,
            });
            self.expect.push_back(Expect::Inventory {
                author,
                after: last,
            });
        }
        Ok(())
    }

    /// The responder's frontier: for every author this replica holds and the
    /// responder sent no inventory of, pushes the events past the
    /// responder's prefix when it holds exactly a prefix of this log.
    fn push_prefixes(&mut self, replica: &dyn Replica, theirs: &Frontier) {
        if !self.outbound_authorized {
            return;
        }
        for mine in replica.frontier().authors {
            if self.inventoried.contains(&mine.author) {
                continue;
            }
            let entry = theirs.get(&mine.author);
            if !replica.extends(&entry) {
                self.outbox.push(SyncMessage::InventoryRequest {
                    author: mine.author,
                    after: None,
                });
                self.expect.push_back(Expect::Inventory {
                    author: mine.author,
                    after: None,
                });
                continue;
            }
            let points = replica.points(&mine.author);
            let from = points.partition_point(|point| point.seq < entry.next_seq);
            let after = from.checked_sub(1).map(|i| points[i]);
            self.outbox
                .events(replica, mine.author, after, None, Vec::new());
        }
    }

    fn chunk(
        &mut self,
        replica: &mut dyn Replica,
        offset: u64,
        total: u64,
        bytes: &[u8],
    ) -> Result<(), Fault> {
        let Some(Expect::Blob {
            hash,
            next,
            total: known,
            keep,
            ..
        }) = self.expect.front_mut()
        else {
            return Err(BROKEN);
        };
        let end = offset + bytes.len() as u64;
        // Contiguous from the request, one length throughout, and an empty
        // chunk only at the end.
        if offset != *next || known.is_some_and(|known| known != total) {
            return Err(BROKEN);
        }
        if end != total && bytes.is_empty() {
            return Err(BROKEN);
        }
        if *keep {
            *keep = matches!(replica.stage(hash, offset, total, bytes), Staged::More(_));
        }
        if end == total {
            self.expect.pop_front();
        } else {
            *next = end;
            *known = Some(total);
        }
        Ok(())
    }

    fn request_blob(&mut self, hash: BlobHash, offset: u64) {
        self.outbox.push(SyncMessage::BlobRequest { hash, offset });
        self.expect.push_back(Expect::Blob {
            hash,
            next: offset,
            total: None,
            keep: true,
            retried: false,
        });
    }

    /// Moves to the next stage whenever every expected answer has arrived.
    fn advance(&mut self, replica: &dyn Replica) {
        while self.expect.is_empty() && !self.outbox.pending() {
            match self.stage {
                Stage::Reconcile => {
                    if !self.outbound_authorized {
                        self.outbox.push(SyncMessage::Refused(Refusal::NotAMember));
                        self.ended = Some(Ended::Refused(Refusal::NotAMember));
                        return;
                    }
                    self.stage = Stage::Founding;
                    if let Some((hash, offset)) = replica.founding_blob() {
                        self.request_blob(hash, offset);
                    }
                }
                Stage::Founding => {
                    self.stage = Stage::EarlyKeys;
                    for epoch in replica.wanted_keys() {
                        self.outbox.push(SyncMessage::KeyRequest { epoch });
                        self.expect.push_back(Expect::Key(epoch));
                    }
                }
                Stage::EarlyKeys => self.stage = Stage::Blobs,
                // One object at a time: each answer runs to the object's end.
                Stage::Blobs => match replica.next_wanted_blob(self.blob_cursor) {
                    Some((hash, offset)) => {
                        self.blob_cursor = Some(hash);
                        self.request_blob(hash, offset);
                    }
                    None => {
                        self.stage = Stage::Keys;
                        for epoch in replica.wanted_keys() {
                            self.outbox.push(SyncMessage::KeyRequest { epoch });
                            self.expect.push_back(Expect::Key(epoch));
                        }
                    }
                },
                Stage::Keys => self.stage = Stage::Deliveries,
                Stage::Deliveries => {
                    if let Some((effect, recipient)) = self
                        .remote
                        .and_then(|remote| replica.next_delivery(remote, self.delivery_cursor))
                    {
                        self.delivery_cursor = Some((effect, recipient));
                        self.outbox
                            .push(SyncMessage::DeliverEffect { effect, recipient });
                        self.expect.push_back(Expect::Receipt(effect, recipient));
                        continue;
                    }
                    self.stage = Stage::Finished;
                    self.outbox.push(SyncMessage::Done);
                    self.ended = Some(Ended::Completed);
                }
                Stage::Start | Stage::Finished => return,
            }
        }
    }
}

fn receive(replica: &mut dyn Replica, events: Vec<WireEvent>) -> Result<(), Fault> {
    replica.receive(events).map(drop).map_err(Fault::Sent)
}
