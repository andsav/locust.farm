//! Everything derived from the held events, and the fold that computes it
//! from scratch: the chain first, then every event in canonical order.
//!
//! Canonical order: decisions in chain order; a contribution directly after
//! the decision it anchors to; contributions sharing an anchor in ascending
//! (author, position). It depends only on the held set.

use std::collections::BTreeMap;

use locust_proto::event::{AuthorPoint, Body, Event};
use locust_proto::id::{EventId, PublicKey};

use super::chain::Chain;
use super::commitments::Commitments;
use super::history::{AuthorLog, History, Slot};
use super::ids::IdSet;
use super::standing::{Exclusion, Standing, Waiting};
use super::state::State;
use super::transition;

/// What the scan of one author's usable prefix has seen so far.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Trail {
    /// The latest chain position any of the author's events anchored to.
    reach: Option<u32>,
    /// True once an event's anchor was not a decision of the chain: the
    /// author's later events wait behind it.
    gated: bool,
}

/// Where one event of a usable prefix goes.
pub(super) enum Spot {
    /// A decision body by someone who is not the coordinator.
    NotCoordinator,
    /// A decision of the chain; the walk judges it.
    Decision,
    /// Behind an earlier event of its author that waits for its anchor.
    Behind,
    /// Its own anchor is not a decision of the chain.
    Unanchored(EventId),
    /// A contribution anchored at this chain position.
    At { position: u32, regressed: bool },
}

impl Trail {
    /// Notes an anchor at `position`. False if the author anchored later
    /// than that before, which makes the event a regression.
    pub fn reach(&mut self, position: u32) -> bool {
        if self.reach.is_some_and(|reach| position < reach) {
            return false;
        }
        self.reach = Some(position);
        true
    }

    /// Places the next event of the author's usable prefix.
    pub fn place(&mut self, chain: &Chain, event: &Event, is_coordinator: bool) -> Spot {
        let header = event.header();
        if header.body.is_decision() {
            if !is_coordinator {
                return Spot::NotCoordinator;
            }
            // A decision anchors to the decision before it.
            if let Some(before) = chain.position(&event.id()).and_then(|at| at.checked_sub(1)) {
                self.reach(before);
            }
            return Spot::Decision;
        }
        if self.gated {
            return Spot::Behind;
        }
        let Some(anchor) = header.anchor else {
            return Spot::Behind;
        };
        match chain.position(&anchor) {
            Some(position) => Spot::At {
                position,
                regressed: !self.reach(position),
            },
            None => {
                self.gated = true;
                Spot::Unanchored(anchor)
            }
        }
    }
}

/// What a decision's reference to a contribution allows.
enum Reference {
    Clear,
    Wait(EventId),
    Refuse(&'static str),
}

/// A contribution placed in the canonical order.
#[derive(Clone, Copy, Default)]
struct Place {
    position: u32,
    slot: Slot,
    regressed: bool,
}

/// The chain, every standing and the state: a function of the held set.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Folded {
    pub chain: Chain,
    /// The standing of every held event, by slot.
    pub standings: Vec<Standing>,
    pub state: State,
    /// How many decisions of the chain are applied. Fewer than the chain
    /// holds while a decision waits for the contribution it names.
    pub applied: u32,
    /// The last contribution whose transition was tried under the applied
    /// head, as its place in the order: (author, position).
    pub last: Option<(PublicKey, u64)>,
    /// The contribution the first unapplied decision waits for.
    pub awaited: Option<EventId>,
    pub trails: BTreeMap<PublicKey, Trail>,
    /// Anchors that are not decisions of the chain and hold an author back.
    pub gates: IdSet,
}

impl Folded {
    /// True while a decision of the chain waits to be applied.
    pub fn stalled(&self) -> bool {
        (self.applied as usize) < self.chain.links.len()
    }

    /// Judges the next decision of the chain. False if it has to wait.
    pub fn decide(&mut self, history: &History, order: &mut Vec<Slot>) -> bool {
        let link = &self.chain.links[self.applied as usize];
        let (slot, epoch) = (link.slot, link.epoch);
        let decision = &history.events[slot as usize];
        let standing = match link.verdict {
            Some(exclusion) => Standing::Excluded(exclusion),
            None => match self.reference(history, decision) {
                Reference::Wait(id) => {
                    self.awaited = Some(id);
                    self.standings[slot as usize] = Standing::Pending(Waiting::Reference);
                    return false;
                }
                Reference::Refuse(reason) => Standing::Excluded(Exclusion::Precondition(reason)),
                Reference::Clear => applied(transition::decide(&mut self.state, decision)),
            },
        };
        transition::advance(&mut self.state, decision, epoch);
        self.standings[slot as usize] = standing;
        self.applied += 1;
        self.last = None;
        order.push(slot);
        true
    }

    /// Whether the contribution a decision names can be decided on. It must
    /// be held, judged and effective. One that is held and waits can still
    /// become that only if it is anchored before the decision; the chain up
    /// to the decision is fixed, so anything else never will.
    fn reference(&self, history: &History, decision: &Event) -> Reference {
        let named = match &decision.header().body {
            Body::TaskAssigned { task, .. } => task,
            Body::ResultAccepted { result, .. } | Body::ResultRejected { result } => result,
            Body::RevisionAccepted { revision } => revision,
            _ => return Reference::Clear,
        };
        let Some(slot) = history.slot(named) else {
            return Reference::Wait(*named);
        };
        let target = &history.events[slot as usize];
        if !matches!(
            (&decision.header().body, &target.header().body),
            (Body::TaskAssigned { .. }, Body::TaskProposed { .. })
                | (
                    Body::ResultAccepted { .. } | Body::ResultRejected { .. },
                    Body::ResultSubmitted { .. }
                )
                | (Body::RevisionAccepted { .. }, Body::Revision { .. })
        ) {
            return Reference::Refuse("the event named has the wrong kind");
        }
        let position = target
            .header()
            .anchor
            .and_then(|anchor| self.chain.position(&anchor));
        let Some(position) = position.filter(|position| *position < self.applied) else {
            return Reference::Refuse("the event named is not anchored before the decision");
        };
        if self.excluded(target, position, false).is_some() {
            return Reference::Refuse("the event named grants nothing");
        }
        match self.standings[slot as usize] {
            Standing::Effective => Reference::Clear,
            Standing::Excluded(_) => Reference::Refuse("the event named grants nothing"),
            Standing::Pending(_) => Reference::Wait(*named),
        }
    }

    /// Why a contribution anchored at `position` grants nothing, where the
    /// chain and its author's log alone decide it.
    pub fn excluded(&self, event: &Event, position: u32, regressed: bool) -> Option<Exclusion> {
        let header = event.header();
        let point = AuthorPoint {
            seq: header.seq,
            id: event.id(),
        };
        if self.chain.past_removal(&header.author, &point, position) {
            Some(Exclusion::PastRemoval)
        } else if regressed {
            Some(Exclusion::AnchorRegressed)
        } else if !self.chain.is_member_at(&header.author, position) {
            Some(Exclusion::NotAMember)
        } else if header
            .payload
            .is_some_and(|payload| payload.key_epoch != self.chain.epoch_at(position))
        {
            Some(Exclusion::BadEpoch)
        } else {
            None
        }
    }

    /// Tries the transition of an authorized contribution under the head.
    pub fn contribute(&mut self, event: &Event) -> Standing {
        let header = event.header();
        self.last = Some((header.author, header.seq));
        applied(transition::contribute(&mut self.state, event))
    }

    /// Scans one author's log: settles what the log and the chain decide by
    /// themselves and collects the contributions the walk has to judge.
    fn scan(
        &mut self,
        history: &History,
        log: &AuthorLog,
        is_coordinator: bool,
        places: &mut Vec<Place>,
        order: &mut Vec<Slot>,
        commitments: &Commitments,
    ) -> Trail {
        let mut trail = Trail::default();
        let cut = if is_coordinator {
            self.chain.halt_seq
        } else {
            log.fork
        };
        let cut_reason = if is_coordinator {
            Exclusion::AfterHalt
        } else {
            Exclusion::Forked
        };
        let mut predecessor = None;
        let mut next_seq = 0;
        for (index, (point, &slot)) in log.points.iter().zip(&log.slots).enumerate() {
            let event = &history.events[slot as usize];
            let pinned =
                commitments.pins.get(&(event.header().author, point.seq)) == Some(&point.id);
            let contiguous = point.seq == next_seq && event.header().prev == predecessor;
            let standing = &mut self.standings[slot as usize];
            if cut.is_some_and(|cut| point.seq >= cut) && (is_coordinator || !pinned) {
                *standing = Standing::Excluded(cut_reason);
                order.push(slot);
            } else if index >= log.usable && !(pinned && contiguous) {
                *standing = Standing::Pending(Waiting::Predecessor);
            } else {
                next_seq = point.seq + 1;
                predecessor = Some(point.id);
                match trail.place(&self.chain, event, is_coordinator) {
                    Spot::NotCoordinator => {
                        *standing = Standing::Excluded(Exclusion::NotCoordinator);
                        order.push(slot);
                    }
                    Spot::Behind => *standing = Standing::Pending(Waiting::Predecessor),
                    Spot::Unanchored(anchor) => {
                        self.gates.insert(anchor);
                    }
                    Spot::Decision => {}
                    Spot::At {
                        position,
                        regressed,
                    } => places.push(Place {
                        position,
                        slot,
                        regressed,
                    }),
                }
            }
        }
        trail
    }
}

fn applied(allowed: Result<(), &'static str>) -> Standing {
    match allowed {
        Ok(()) => Standing::Effective,
        Err(reason) => Standing::Excluded(Exclusion::Precondition(reason)),
    }
}

/// Computes everything from the held set. Also returns the events that were
/// judged, in the order they were applied.
pub(super) fn fold(history: &History) -> (Folded, Vec<Slot>) {
    let chain = Chain::build(history);
    // Without a member fork the ordinary usable prefixes already choose
    // every branch. Dependency retention is computed separately by screening.
    let commitments = if history
        .logs
        .iter()
        .any(|(author, log)| Some(*author) != history.coordinator && log.fork.is_some())
    {
        Commitments::build(history, &chain)
    } else {
        Commitments::default()
    };
    fold_with_commitments(history, chain, &commitments)
}

/// Evaluate tentative canonical branch selections with the ordinary fold.
/// Commitment selection calls this directly, avoiding recursive selection.
pub(super) fn fold_with_commitments(
    history: &History,
    chain: Chain,
    commitments: &Commitments,
) -> (Folded, Vec<Slot>) {
    let mut folded = Folded {
        chain,
        standings: vec![Standing::Pending(Waiting::Anchor); history.events.len()],
        ..Folded::default()
    };
    let mut order = Vec::new();
    let Some(coordinator) = history.coordinator else {
        // Without a genesis nothing can be judged but a fork.
        for log in history.logs.values() {
            for (index, (point, &slot)) in log.points.iter().zip(&log.slots).enumerate() {
                if log.fork.is_some_and(|fork| point.seq >= fork) {
                    folded.standings[slot as usize] = Standing::Excluded(Exclusion::Forked);
                    order.push(slot);
                } else if index >= log.usable {
                    folded.standings[slot as usize] = Standing::Pending(Waiting::Predecessor);
                }
            }
        }
        return (folded, order);
    };

    // Authors ascend and each log ascends, so `places` is in (author,
    // position) order and a stable distribution by anchor finishes the sort.
    let mut places = Vec::new();
    for (author, log) in &history.logs {
        let trail = folded.scan(
            history,
            log,
            *author == coordinator,
            &mut places,
            &mut order,
            commitments,
        );
        folded.trails.insert(*author, trail);
    }
    let links = folded.chain.links.len();
    let mut starts = vec![0usize; links + 1];
    for place in &places {
        starts[place.position as usize + 1] += 1;
    }
    for position in 0..links {
        starts[position + 1] += starts[position];
    }
    let mut next = starts.clone();
    let mut sorted = vec![Place::default(); places.len()];
    for place in places {
        let at = &mut next[place.position as usize];
        sorted[*at] = place;
        *at += 1;
    }

    for position in 0..links {
        let link = &folded.chain.links[position];
        if let Some(reference) = commitments.pending.get(&link.id) {
            folded.standings[link.slot as usize] = Standing::Pending(Waiting::Reference);
            folded.awaited = Some(*reference);
            break;
        }
        if !folded.decide(history, &mut order) {
            break;
        }
        for place in &sorted[starts[position]..starts[position + 1]] {
            let event = &history.events[place.slot as usize];
            let standing = match folded.excluded(event, place.position, place.regressed) {
                Some(exclusion) => Standing::Excluded(exclusion),
                None => folded.contribute(event),
            };
            folded.standings[place.slot as usize] = standing;
            order.push(place.slot);
        }
    }
    (folded, order)
}
