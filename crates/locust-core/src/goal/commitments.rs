//! Exact scope-local proof closure. Shared closure indexes avoid retraversing
//! author history for every decision; each decision still validates its own
//! scope, branch compatibility, signer ordering and governance anchor.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use locust_proto::event::{Body, DecisionAction, Event};
use locust_proto::id::EventId;

use super::chain::Chain;
use super::history::History;
use super::rules::invalid;
use super::standing::{Dependency, Standing, Waiting};

/// An exact set of events in the immutable history of one evaluation. There is
/// no protocol/event-count limit: storage grows with the held history.
#[derive(Clone, Default)]
pub(super) struct EventSet(Vec<u64>);
impl EventSet {
    pub fn new(count: usize) -> Self {
        Self(vec![0; count.div_ceil(64)])
    }
    fn insert(&mut self, slot: usize) {
        self.0[slot / 64] |= 1 << (slot % 64);
    }
    pub fn contains(&self, slot: usize) -> bool {
        self.0[slot / 64] & (1 << (slot % 64)) != 0
    }
    pub fn extend(&mut self, other: &Self) {
        for (word, incoming) in self.0.iter_mut().zip(&other.0) {
            *word |= incoming;
        }
    }
    pub fn slots(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(index, word)| {
            let mut remaining = *word;
            std::iter::from_fn(move || {
                if remaining == 0 {
                    return None;
                }
                let bit = remaining.trailing_zeros() as usize;
                remaining &= remaining - 1;
                Some(index * 64 + bit)
            })
        })
    }
}

#[derive(Clone, Default)]
struct Closure {
    events: EventSet,
    max_anchor: usize,
}
struct Frame {
    id: EventId,
    closure: Closure,
    dependencies: Vec<EventId>,
    next: usize,
}

/// Structural closures are reusable as immutable events append while the
/// governance ordering stays identical. Errors are never carried into another
/// evaluation; branch conflicts are reindexed from the complete held history.
#[derive(Clone, Default)]
pub(super) struct Index {
    closures: BTreeMap<EventId, Result<Arc<Closure>, Standing>>,
    conflicting_slots: Vec<Vec<usize>>,
}
impl Index {
    pub fn refresh(&mut self, history: &History) {
        self.closures.retain(|_, result| result.is_ok());
        self.conflicting_slots.clear();
        for log in history.logs.values().filter(|log| log.fork.is_some()) {
            let mut start = 0;
            while start < log.points.len() {
                let seq = log.points[start].seq;
                let end = start + log.points[start..].partition_point(|point| point.seq == seq);
                if end - start > 1 {
                    self.conflicting_slots.push(log.slots[start..end].to_vec());
                }
                start = end;
            }
        }
    }

    fn prepare(
        history: &History,
        chain: &Chain,
        id: EventId,
        missing: &mut BTreeSet<Dependency>,
    ) -> Result<Frame, Standing> {
        let Some(slot) = history.slot(&id) else {
            missing.insert(Dependency::Event(id));
            return Err(Standing::Pending(Waiting::Reference));
        };
        let event = &history.events[slot];
        let h = event.header();
        let mut closure = Closure {
            events: EventSet::new(history.events.len()),
            max_anchor: 0,
        };
        closure.events.insert(slot);
        if let Some(anchor) = h.anchor {
            let Some(position) = chain.position(&anchor) else {
                missing.insert(Dependency::Event(anchor));
                return Err(Standing::Pending(Waiting::Anchor));
            };
            closure.max_anchor = position;
            closure
                .events
                .insert(history.slot(&anchor).expect("verified anchor exists"));
        }
        let mut dependencies = Vec::new();
        if h.body.is_governance() {
            // Governance is validated by its own chain, never branch-selected.
            if chain.position(&id).is_none() {
                return Err(Standing::Pending(Waiting::Anchor));
            }
        } else {
            if let Some(previous) = h.prev {
                let Some(prior) = history.get(&previous) else {
                    missing.insert(Dependency::Event(previous));
                    return Err(Standing::Pending(Waiting::Predecessor));
                };
                if prior.header().author != h.author
                    || prior.header().seq.checked_add(1) != Some(h.seq)
                {
                    return Err(invalid("proof has broken author ancestry"));
                }
                dependencies.push(previous);
            }
            dependencies.extend(h.body.dependencies());
        }
        Ok(Frame {
            id,
            closure,
            dependencies,
            next: 0,
        })
    }

    fn closure(
        &mut self,
        history: &History,
        chain: &Chain,
        root: EventId,
        missing: &mut BTreeSet<Dependency>,
    ) -> Result<Arc<Closure>, Standing> {
        if let Some(result) = self.closures.get(&root) {
            return result.clone();
        }
        let first = Self::prepare(history, chain, root, missing)?;
        let mut frames = vec![first];
        let mut visiting = BTreeSet::from([root]);
        while let Some(frame) = frames.last_mut() {
            if let Some(dependency) = frame.dependencies.get(frame.next).copied() {
                if let Some(result) = self.closures.get(&dependency) {
                    match result {
                        Ok(closure) => {
                            frame.closure.events.extend(&closure.events);
                            frame.closure.max_anchor =
                                frame.closure.max_anchor.max(closure.max_anchor);
                            frame.next += 1;
                        }
                        Err(standing) => {
                            let standing = *standing;
                            let frame = frames.pop().expect("frame exists");
                            visiting.remove(&frame.id);
                            self.closures.insert(frame.id, Err(standing));
                        }
                    }
                } else if !visiting.insert(dependency) {
                    let frame = frames.pop().expect("frame exists");
                    visiting.remove(&frame.id);
                    self.closures
                        .insert(frame.id, Err(invalid("cyclic proof dependency")));
                } else {
                    match Self::prepare(history, chain, dependency, missing) {
                        Ok(next) => frames.push(next),
                        Err(standing) => {
                            visiting.remove(&dependency);
                            self.closures.insert(dependency, Err(standing));
                        }
                    }
                }
            } else {
                let frame = frames.pop().expect("frame exists");
                visiting.remove(&frame.id);
                self.closures.insert(frame.id, Ok(Arc::new(frame.closure)));
            }
        }
        self.closures.get(&root).expect("root evaluated").clone()
    }
}

#[derive(Clone, Default)]
pub(super) struct Proof {
    pub retained: EventSet,
    pub roots: BTreeSet<EventId>,
}

pub(super) fn build(
    history: &History,
    chain: &Chain,
    index: &mut Index,
    decision: &Event,
    missing: &mut BTreeSet<Dependency>,
) -> Result<Proof, Standing> {
    let Body::ScopeDecided {
        context,
        previous,
        action,
        evidence,
    } = &decision.header().body
    else {
        return Err(invalid("proof owner is not a scoped decision"));
    };
    let mut proof = Proof {
        retained: EventSet::new(history.events.len()),
        roots: BTreeSet::new(),
    };
    proof.roots.extend(evidence);
    proof.roots.insert(context.round);
    proof.roots.extend(previous);
    if let DecisionAction::Select { subject } = action {
        proof.roots.insert(*subject);
    }
    let decision_anchor = decision
        .header()
        .anchor
        .and_then(|id| chain.position(&id))
        .ok_or(Standing::Pending(Waiting::Anchor))?;
    for root in &proof.roots {
        let closure = index.closure(history, chain, *root, missing)?;
        if closure.max_anchor > decision_anchor {
            return Err(invalid("proof uses governance after the decision anchor"));
        }
        proof.retained.extend(&closure.events);
    }
    if proof
        .retained
        .contains(history.slot(&decision.id()).expect("decision exists"))
    {
        return Err(invalid("decision proof contains itself"));
    }
    if index.conflicting_slots.iter().any(|slots| {
        slots
            .iter()
            .filter(|slot| proof.retained.contains(**slot))
            .take(2)
            .count()
            > 1
    }) {
        return Err(Standing::Disputed);
    }
    let log = history
        .log(&decision.header().author)
        .expect("decision author exists");
    let from = log
        .points
        .partition_point(|point| point.seq < decision.header().seq);
    if log.slots[from..]
        .iter()
        .any(|slot| proof.retained.contains(*slot))
    {
        return Err(invalid("proof is not before its signer decision"));
    }
    Ok(proof)
}
