//! The decision chain, and what follows from decisions alone: the halt,
//! membership over time, removal cutoffs and key epochs. Computed in a first
//! pass, so it is known for every chain position before any contribution is
//! judged.

use std::collections::BTreeMap;

use locust_proto::event::{AuthorPoint, Body, Event};
use locust_proto::id::{EventId, PublicKey};

use super::history::{History, Slot};
use super::ids::IdMap;
use super::standing::{Exclusion, Halt};

/// One decision of the chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Link {
    pub id: EventId,
    pub slot: Slot,
    /// The key epoch at this decision: the `MemberRemoved` decisions at or
    /// before it, counted whether or not they took effect.
    pub epoch: u32,
    /// Why the decision grants nothing, where the chain alone decides it: a
    /// payload of another epoch, or the removal of a non-member.
    pub verdict: Option<Exclusion>,
}

/// One removal's cutoff: the removed member's events past `last` grant
/// nothing when anchored before `until`, its next admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cutoff {
    last: Option<AuthorPoint>,
    until: u32,
}

/// When one key was a member, in chain positions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Tenure {
    /// Half-open spans `[from, until)`, ascending; the last is open while
    /// `until` is [`OPEN`].
    spans: Vec<(u32, u32)>,
    cutoffs: Vec<Cutoff>,
}

const OPEN: u32 = u32::MAX;

impl Tenure {
    fn is_member(&self) -> bool {
        self.spans.last().is_some_and(|span| span.1 == OPEN)
    }
}

/// The coordinator's decisions in order, up to the halt.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Chain {
    pub links: Vec<Link>,
    positions: IdMap<u32>,
    tenures: BTreeMap<PublicKey, Tenure>,
    pub halt: Option<Halt>,
    /// The coordinator position at which the halt starts.
    pub halt_seq: Option<u64>,
}

impl Chain {
    /// Reads the chain off the coordinator's usable prefix.
    pub fn build(history: &History) -> Self {
        let mut chain = Self::default();
        let Some(log) = history.coordinator.and_then(|key| history.log(&key)) else {
            return chain;
        };
        for &slot in &log.slots[..log.usable] {
            let event = &history.events[slot as usize];
            if !event.header().body.is_decision() {
                continue;
            }
            if !chain.succeeds(event) {
                chain.halt = Some(Halt::BrokenChain { event: event.id() });
                chain.halt_seq = Some(event.header().seq);
                return chain;
            }
            chain.push(slot, event);
        }
        if let Some(seq) = log.fork {
            let events = log.points.iter().filter(|point| point.seq == seq);
            chain.halt = Some(Halt::Fork {
                seq,
                events: events.map(|point| point.id).collect(),
            });
            chain.halt_seq = Some(seq);
        }
        chain
    }

    /// True if `decision` names the chain's last decision as its anchor.
    pub fn succeeds(&self, decision: &Event) -> bool {
        decision.header().anchor == self.links.last().map(|link| link.id)
    }

    /// Appends the next decision and applies what it means for membership.
    pub fn push(&mut self, slot: Slot, decision: &Event) {
        let position = self.links.len() as u32;
        let header = decision.header();
        let removal = matches!(header.body, Body::MemberRemoved { .. });
        let epoch = self.links.last().map_or(0, |link| link.epoch) + u32::from(removal);
        let mut verdict = match header.payload {
            Some(payload) if payload.key_epoch != epoch => Some(Exclusion::BadEpoch),
            _ => None,
        };
        if verdict.is_none() {
            match &header.body {
                Body::MemberAdmitted { member, .. } => {
                    let tenure = self.tenures.entry(*member).or_default();
                    if tenure.is_member() {
                        verdict = Some(Exclusion::Precondition("the key is already a member"));
                    } else {
                        tenure.spans.push((position, OPEN));
                        if let Some(cutoff) = tenure.cutoffs.last_mut() {
                            cutoff.until = position;
                        }
                    }
                }
                Body::MemberRemoved {
                    member,
                    last_accepted,
                } => match self.tenures.get_mut(member) {
                    Some(tenure) if tenure.is_member() => {
                        if let Some(span) = tenure.spans.last_mut() {
                            span.1 = position;
                        }
                        tenure.cutoffs.push(Cutoff {
                            last: *last_accepted,
                            until: OPEN,
                        });
                    }
                    _ => verdict = Some(Exclusion::Precondition("the key is not a member")),
                },
                _ => {}
            }
        }
        self.positions.insert(decision.id(), position);
        self.links.push(Link {
            id: decision.id(),
            slot,
            epoch,
            verdict,
        });
    }

    /// Latest content epoch granted by canonical membership. Readmission
    /// grants the complete earlier history again.
    pub fn read_epoch(&self, member: &PublicKey, applied: u32) -> Option<u32> {
        let (_, until) = *self
            .tenures
            .get(member)?
            .spans
            .iter()
            .rev()
            .find(|(from, _)| *from < applied)?;
        Some(if until >= applied {
            self.links.get(applied.checked_sub(1)? as usize)?.epoch
        } else {
            self.epoch_at(until).saturating_sub(1)
        })
    }

    /// The chain position of the decision `id`.
    pub fn position(&self, id: &EventId) -> Option<u32> {
        self.positions.get(id).copied()
    }

    /// The key epoch at chain position `position`.
    pub fn epoch_at(&self, position: u32) -> u32 {
        self.links[position as usize].epoch
    }

    /// True if `key` is a member as of the decision at `position`.
    pub fn is_member_at(&self, key: &PublicKey, position: u32) -> bool {
        self.tenures.get(key).is_some_and(|tenure| {
            let spans = tenure.spans.iter().rev();
            spans
                .take_while(|span| position < span.1)
                .any(|span| span.0 <= position)
        })
    }

    /// True if `point` of `author`'s log, anchored at `position`, lies beyond
    /// a removal cutoff. Each removal's cutoff holds for everything anchored
    /// before the member's next admission, so work cannot be backdated.
    pub fn past_removal(&self, author: &PublicKey, point: &AuthorPoint, position: u32) -> bool {
        let Some(tenure) = self.tenures.get(author) else {
            return false;
        };
        tenure.cutoffs.iter().any(|cutoff| {
            position < cutoff.until
                && match cutoff.last {
                    None => true,
                    Some(last) => {
                        point.seq > last.seq || (point.seq == last.seq && point.id != last.id)
                    }
                }
        })
    }
}
