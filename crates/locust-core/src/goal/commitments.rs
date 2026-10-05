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

/// Persistent radix tree of occupied bitmap words. Ancestor closures share
/// unchanged branches instead of retaining a history-sized bitmap per event.
#[derive(Clone, Default)]
pub(super) struct EventSet {
    root: Option<Arc<BitNode>>,
    count: usize,
}

enum BitNode {
    Word {
        index: usize,
        bits: u64,
    },
    Branch {
        prefix: usize,
        bit: usize,
        left: Arc<BitNode>,
        right: Arc<BitNode>,
    },
}

impl BitNode {
    fn prefix(&self) -> usize {
        match self {
            Self::Word { index, .. } => *index,
            Self::Branch { prefix, .. } => *prefix,
        }
    }

    fn bit(&self) -> usize {
        match self {
            Self::Word { .. } => 0,
            Self::Branch { bit, .. } => *bit,
        }
    }

    fn contains(&self, index: usize, mask: u64) -> bool {
        match self {
            Self::Word { index: held, bits } => *held == index && bits & mask != 0,
            Self::Branch {
                bit, left, right, ..
            } => {
                if index & bit == 0 {
                    left.contains(index, mask)
                } else {
                    right.contains(index, mask)
                }
            }
        }
    }

    fn branch(original: &Arc<Self>, left: Arc<Self>, right: Arc<Self>) -> Arc<Self> {
        let Self::Branch {
            prefix,
            bit,
            left: old_left,
            right: old_right,
        } = original.as_ref()
        else {
            unreachable!("only branches are rebuilt");
        };
        if Arc::ptr_eq(&left, old_left) && Arc::ptr_eq(&right, old_right) {
            original.clone()
        } else {
            Arc::new(Self::Branch {
                prefix: *prefix,
                bit: *bit,
                left,
                right,
            })
        }
    }

    fn union(a: &Arc<Self>, b: &Arc<Self>) -> Arc<Self> {
        if Arc::ptr_eq(a, b) {
            return a.clone();
        }
        let different = a.prefix() ^ b.prefix();
        let split = if different == 0 {
            0
        } else {
            1usize << (usize::BITS - 1 - different.leading_zeros())
        };
        if split > a.bit().max(b.bit()) {
            let (left, right) = if a.prefix() & split == 0 {
                (a, b)
            } else {
                (b, a)
            };
            return Arc::new(Self::Branch {
                prefix: a.prefix() & !(split | (split - 1)),
                bit: split,
                left: left.clone(),
                right: right.clone(),
            });
        }
        match (a.as_ref(), b.as_ref()) {
            (
                Self::Word {
                    index,
                    bits: a_bits,
                },
                Self::Word { bits: b_bits, .. },
            ) => {
                let bits = a_bits | b_bits;
                if bits == *a_bits {
                    a.clone()
                } else if bits == *b_bits {
                    b.clone()
                } else {
                    Arc::new(Self::Word {
                        index: *index,
                        bits,
                    })
                }
            }
            (
                Self::Branch {
                    bit: a_bit,
                    left: a_left,
                    right: a_right,
                    ..
                },
                Self::Branch {
                    bit: b_bit,
                    left: b_left,
                    right: b_right,
                    ..
                },
            ) if a_bit == b_bit => Self::branch(
                a,
                Self::union(a_left, b_left),
                Self::union(a_right, b_right),
            ),
            _ if a.bit() < b.bit() => Self::union(b, a),
            (
                Self::Branch {
                    bit, left, right, ..
                },
                _,
            ) => {
                if b.prefix() & bit == 0 {
                    Self::branch(a, Self::union(left, b), right.clone())
                } else {
                    Self::branch(a, left.clone(), Self::union(right, b))
                }
            }
            _ => unreachable!("unequal words split above"),
        }
    }
}

impl EventSet {
    pub fn new(count: usize) -> Self {
        Self { root: None, count }
    }
    pub(super) fn insert(&mut self, slot: usize) {
        assert!(slot < self.count, "event slot belongs to this history");
        let word = Arc::new(BitNode::Word {
            index: slot / 64,
            bits: 1 << (slot % 64),
        });
        self.root = Some(match &self.root {
            Some(root) => BitNode::union(root, &word),
            None => word,
        });
    }
    pub fn contains(&self, slot: usize) -> bool {
        slot < self.count
            && self
                .root
                .as_ref()
                .is_some_and(|root| root.contains(slot / 64, 1 << (slot % 64)))
    }
    pub fn extend(&mut self, other: &Self) {
        self.count = self.count.max(other.count);
        if let Some(incoming) = &other.root {
            self.root = Some(match &self.root {
                Some(root) => BitNode::union(root, incoming),
                None => incoming.clone(),
            });
        }
    }
    pub fn slots(&self) -> impl Iterator<Item = usize> + '_ {
        let mut stack: Vec<_> = self.root.as_deref().into_iter().collect();
        let mut index = 0;
        let mut remaining = 0u64;
        std::iter::from_fn(move || {
            loop {
                if remaining != 0 {
                    let bit = remaining.trailing_zeros() as usize;
                    remaining &= remaining - 1;
                    return Some(index * 64 + bit);
                }
                match stack.pop()? {
                    BitNode::Word { index: next, bits } => {
                        index = *next;
                        remaining = *bits;
                    }
                    BitNode::Branch { left, right, .. } => {
                        stack.push(right);
                        stack.push(left);
                    }
                }
            }
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
    let mut proof = Proof {
        retained: EventSet::new(history.events.len()),
        roots: BTreeSet::new(),
    };
    match &decision.header().body {
        Body::ScopeDecided {
            context,
            previous,
            action,
            evidence,
        } => {
            proof.roots.extend(evidence);
            proof.roots.insert(context.round);
            proof.roots.extend(previous);
            if let DecisionAction::Select { subject } = action {
                proof.roots.insert(*subject);
            }
        }
        Body::WorkspaceEpoch { rules, .. } => {
            proof.roots.insert(*rules);
            proof.roots.extend(super::workspace::boundary(
                history,
                chain,
                decision.id(),
                missing,
            )?);
        }
        _ => {
            return Err(invalid(
                "proof owner is not a scoped decision or workspace epoch",
            ));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_sets_match_sparse_unions_and_preserve_old_versions() {
        let mut seed = 17u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed as usize
        };
        let mut sets = Vec::new();
        for _ in 0..64 {
            let mut actual = EventSet::new(100_000);
            let mut expected = BTreeSet::new();
            for _ in 0..128 {
                let slot = next() % 100_000;
                actual.insert(slot);
                expected.insert(slot);
            }
            sets.push((actual, expected));
        }
        for _ in 0..128 {
            let a = next() % sets.len();
            let b = next() % sets.len();
            let mut actual = sets[a].0.clone();
            actual.extend(&sets[b].0);
            let expected: BTreeSet<_> = sets[a].1.union(&sets[b].1).copied().collect();
            assert_eq!(actual.slots().collect::<BTreeSet<_>>(), expected);
            for slot in &expected {
                assert!(actual.contains(*slot));
            }
            assert_eq!(sets[a].0.slots().collect::<BTreeSet<_>>(), sets[a].1);
            sets[a] = (actual, expected);
        }
        let mut sparse = EventSet::new(usize::MAX);
        for slot in [0, 63, 64, 127, 128, usize::MAX - 1] {
            sparse.insert(slot);
        }
        assert_eq!(
            sparse.slots().collect::<Vec<_>>(),
            [0, 63, 64, 127, 128, usize::MAX - 1]
        );
        assert!(!sparse.contains(65));
    }

    #[test]
    fn long_interleaved_author_prefixes_share_bitmap_branches() {
        let mut prefix = EventSet::new(1_000_000);
        let mut prefixes = Vec::new();
        for sequence in 0..4096 {
            // Other authors' slots remain excluded, so range compression alone
            // would still retain a quadratic number of ranges.
            prefix.insert(sequence * 17);
            prefixes.push(prefix.clone());
        }
        let mut nodes = std::collections::HashSet::new();
        let mut stack: Vec<_> = prefixes
            .iter()
            .filter_map(|set| set.root.as_ref())
            .collect();
        while let Some(node) = stack.pop() {
            if !nodes.insert(Arc::as_ptr(node)) {
                continue;
            }
            if let BitNode::Branch { left, right, .. } = node.as_ref() {
                stack.extend([left, right]);
            }
        }
        // One leaf and a logarithmic path per append, independent of the
        // declared million-slot history. No ancestor owns a dense bitmap.
        assert!(nodes.len() < prefixes.len() * 16, "{} nodes", nodes.len());
        assert_eq!(prefix.slots().count(), prefixes.len());
        assert!(!prefixes[0].contains(17));
        assert!(prefixes.last().unwrap().contains(4095 * 17));
    }

    #[test]
    fn cached_sets_extend_across_history_growth_without_losing_slots() {
        let mut old = EventSet::new(64);
        old.insert(63);
        let mut grown = EventSet::new(1024);
        grown.insert(1000);
        grown.extend(&old);
        assert_eq!(grown.slots().collect::<Vec<_>>(), [63, 1000]);
        assert!(!old.contains(1000));
        let root = grown.root.clone().unwrap();
        grown.extend(&grown.clone());
        assert!(Arc::ptr_eq(&root, grown.root.as_ref().unwrap()));
    }
}
