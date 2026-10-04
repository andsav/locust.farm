//! Exact branches named by the canonical coordinator chain. Selection is
//! derived from the held set, never from a replica's earlier applied state.
use std::collections::BTreeMap;

use locust_proto::event::{Body, Event};
use locust_proto::id::{EventId, PublicKey};

use super::chain::Chain;
use super::history::History;
use super::ids::IdSet;
use super::standing::Standing;

#[derive(Clone, Default)]
pub(super) struct Commitments {
    pub pins: BTreeMap<(PublicKey, u64), EventId>,
    pub required: IdSet,
    pub pending: BTreeMap<EventId, EventId>,
}

/// Semantic event dependencies, excluding hints in `parents` and `about`.
fn dependencies(event: &Event) -> Vec<EventId> {
    let mut ids: Vec<_> = event.header().prev.into_iter().collect();
    match &event.header().body {
        Body::TaskAssigned { task, .. } => ids.push(*task),
        Body::ResultAccepted { result, .. } | Body::ResultRejected { result } => ids.push(*result),
        Body::RevisionAccepted { revision } => ids.push(*revision),
        Body::AssignmentAccepted { assignment }
        | Body::AssignmentDeclined { assignment }
        | Body::Progress { assignment }
        | Body::AttemptFailed { assignment }
        | Body::ResultSubmitted { assignment, .. }
        | Body::CancelRequested { assignment } => ids.push(*assignment),
        Body::CancelAcknowledged { cancel, .. } => ids.push(*cancel),
        Body::Revision { base, .. } => ids.extend(base),
        Body::TaskProposed { depends_on, .. } => ids.extend(depends_on),
        _ => {}
    }
    ids
}

impl Commitments {
    pub fn build(history: &History, chain: &Chain) -> Self {
        let mut selected = Self::default();
        let mut validated = Vec::new();
        let member_fork = history
            .logs
            .iter()
            .any(|(author, log)| Some(*author) != history.coordinator && log.fork.is_some());
        // Screening also asks for required headers in a healthy history.
        // There all tentative pins select the same usable prefixes, so one
        // ordinary fold validates every decision instead of replaying each.
        let ordinary = (!member_fork)
            .then(|| super::fold::fold_with_commitments(history, chain.clone(), &selected).0);
        for (position, link) in chain.links.iter().enumerate() {
            if link.verdict.is_some() {
                continue;
            }
            let decision = &history.events[link.slot as usize];
            if !chain.is_member_at(&decision.header().author, position.saturating_sub(1) as u32) {
                continue;
            }
            let reference = match decision.header().body {
                Body::TaskAssigned { task, .. } => task,
                Body::ResultAccepted { result, .. } | Body::ResultRejected { result } => result,
                Body::RevisionAccepted { revision } => revision,
                _ => continue,
            };
            selected.required.insert(reference);
            let mut eligible = true;
            if let Some(target) = history.get(&reference) {
                let right_kind = matches!(
                    (&decision.header().body, &target.header().body),
                    (Body::TaskAssigned { .. }, Body::TaskProposed { .. })
                        | (
                            Body::ResultAccepted { .. } | Body::ResultRejected { .. },
                            Body::ResultSubmitted { .. }
                        )
                        | (Body::RevisionAccepted { .. }, Body::Revision { .. })
                );
                let anchor = target
                    .header()
                    .anchor
                    .and_then(|anchor| chain.position(&anchor));
                if !right_kind
                    || anchor.is_none_or(|anchor| {
                        anchor as usize >= position
                            || !chain.is_member_at(&target.header().author, anchor)
                            || target
                                .header()
                                .payload
                                .is_some_and(|payload| payload.key_epoch != chain.epoch_at(anchor))
                    })
                {
                    eligible = false;
                }
            }
            // A direct header is enough to prove an invalid kind or anchor.
            // Such a decision must not exempt arbitrary same-author ancestry.
            if !eligible {
                continue;
            }
            let mut pending = vec![reference];
            let mut required = IdSet::default();
            let mut pins = BTreeMap::new();
            let mut compatible = eligible;
            while let Some(id) = pending.pop() {
                if !required.insert(id) {
                    continue;
                }
                let Some(event) = history.get(&id) else {
                    continue;
                };
                let header = event.header();
                if Some(header.author) == history.coordinator {
                    // Coordinator history is never branch-selected: its own
                    // fork remains an authority halt.
                    continue;
                }
                if let Some(previous) = header.prev.and_then(|id| history.get(&id))
                    && (previous.header().author != header.author
                        || previous.header().seq.checked_add(1) != Some(header.seq))
                {
                    compatible = false;
                }
                let position = (header.author, header.seq);
                if selected.pins.get(&position) == Some(&id) {
                    // Its ancestry was already retained and checked for an
                    // earlier canonical commitment.
                    continue;
                }
                if pins
                    .insert(position, id)
                    .is_some_and(|previous| previous != id)
                    || selected
                        .pins
                        .get(&position)
                        .is_some_and(|previous| *previous != id)
                {
                    compatible = false;
                }
                pending.extend(dependencies(event));
            }
            if !compatible {
                continue;
            }
            let tentative;
            let folded = if let Some(ordinary) = &ordinary {
                ordinary
            } else {
                let mut candidate = Self {
                    pins: selected.pins.clone(),
                    pending: selected.pending.clone(),
                    ..Self::default()
                };
                candidate.pins.extend(pins.clone());
                tentative =
                    super::fold::fold_with_commitments(history, chain.clone(), &candidate).0;
                &tentative
            };
            if member_fork
                && validated
                    .iter()
                    .any(|&slot: &usize| folded.standings[slot] != Standing::Effective)
            {
                continue;
            }
            match folded.standings[link.slot as usize] {
                Standing::Effective => {
                    selected.pins.extend(pins);
                    selected.required.extend(required);
                    validated.push(link.slot as usize);
                }
                Standing::Pending(_) => {
                    // Missing evidence may make this decision valid later.
                    // Retain its closure, but publish no unvalidated branch.
                    selected.required.extend(required);
                    selected.pending.insert(link.id, reference);
                }
                Standing::Excluded(_) => {}
            }
        }
        selected
    }
}
