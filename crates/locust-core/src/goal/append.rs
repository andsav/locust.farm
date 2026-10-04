//! The incremental path: an event that lands at the end of the canonical
//! order is applied to the existing fold. Everything else (a late event
//! under an old anchor, a gap filled, a fork, a removal, a decision something
//! already waits for) is answered with "fold again from scratch".

use locust_proto::event::{Body, Event};

use super::fold::{Folded, Spot};
use super::history::{History, Placed, Slot};
use super::standing::{Exclusion, Standing, Waiting};

impl Folded {
    /// Accounts for the event newly held in `slot`. Pushes it to `judged` if
    /// it was judged. False if the fold has to be recomputed instead; the
    /// fold is then left in an unspecified state.
    pub fn append(
        &mut self,
        history: &History,
        slot: Slot,
        placed: Placed,
        judged: &mut Vec<Slot>,
    ) -> bool {
        debug_assert_eq!(slot as usize, self.standings.len());
        let event = &history.events[slot as usize];
        let author = event.header().author;
        let Some(coordinator) = history.coordinator else {
            // Without a genesis nothing can be judged but a fork.
            self.standings.push(Standing::Pending(match placed {
                Placed::Tip => Waiting::Anchor,
                Placed::Beyond => Waiting::Predecessor,
                Placed::Fills | Placed::Forked => return false,
            }));
            return true;
        };
        let is_coordinator = author == coordinator;
        if self.chain.links.is_empty()
            || matches!(placed, Placed::Fills | Placed::Forked)
            || (is_coordinator && self.chain.halt.is_some())
            || self.awaited == Some(event.id())
        {
            return false;
        }

        let mut trail = self.trails.get(&author).copied().unwrap_or_default();
        let standing = if placed == Placed::Beyond {
            Some(Standing::Pending(Waiting::Predecessor))
        } else if is_coordinator && event.header().body.is_decision() {
            if !self.extend(history, slot, event, judged) {
                return false;
            }
            trail.place(&self.chain, event, true);
            None
        } else {
            match trail.place(&self.chain, event, is_coordinator) {
                Spot::NotCoordinator => Some(Standing::Excluded(Exclusion::NotCoordinator)),
                Spot::Behind => Some(Standing::Pending(Waiting::Predecessor)),
                Spot::Unanchored(anchor) => {
                    self.gates.insert(anchor);
                    Some(Standing::Pending(Waiting::Anchor))
                }
                Spot::Decision => return false,
                Spot::At { position, .. } if position >= self.applied => {
                    Some(Standing::Pending(Waiting::Anchor))
                }
                Spot::At {
                    position,
                    regressed,
                } => match self.excluded(event, position, regressed) {
                    Some(exclusion) => Some(Standing::Excluded(exclusion)),
                    None => {
                        // Its transition has to land after everything applied.
                        let header = event.header();
                        let at_end = position + 1 == self.applied
                            && self.last.is_none_or(|last| last < (author, header.seq));
                        if !at_end {
                            return false;
                        }
                        Some(self.contribute(event))
                    }
                },
            }
        };
        if let Some(standing) = standing {
            self.standings.push(standing);
            if !standing.is_pending() {
                judged.push(slot);
            }
        }
        self.trails.insert(author, trail);
        true
    }

    /// Extends the chain by the coordinator's next decision and applies it,
    /// unless an earlier decision still waits. False if the decision does
    /// more than extend the chain.
    fn extend(
        &mut self,
        history: &History,
        slot: Slot,
        decision: &Event,
        judged: &mut Vec<Slot>,
    ) -> bool {
        // A removal reaches back to events already judged; a decision that
        // held events anchor to puts them after it; a wrong anchor halts.
        let selects_branch = matches!(
            decision.header().body,
            Body::TaskAssigned { .. }
                | Body::ResultAccepted { .. }
                | Body::ResultRejected { .. }
                | Body::RevisionAccepted { .. }
        ) && history
            .logs
            .iter()
            .any(|(author, log)| Some(*author) != history.coordinator && log.fork.is_some());
        if matches!(decision.header().body, Body::MemberRemoved { .. })
            || selects_branch
            || self.gates.contains(&decision.id())
            || !self.chain.succeeds(decision)
        {
            return false;
        }
        let stalled = self.stalled();
        self.chain.push(slot, decision);
        self.standings.push(Standing::Pending(Waiting::Anchor));
        if !stalled {
            self.decide(history, judged);
        }
        true
    }
}
