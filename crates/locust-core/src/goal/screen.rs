//! Which received events are retained, decided before anything is stored.
//! What is dropped here is offered again by a later reconciliation and kept
//! then if it has become admissible.

use std::collections::HashMap;

use locust_proto::event::{Body, Event};
use locust_proto::id::{EventId, GoalId, PublicKey};

use super::chain::Chain;
use super::commitments::Commitments;
use super::history::History;
use super::ids::IdSet;

/// Largest number of different events retained at one position of one
/// author's log. Two are enough to prove a fork.
pub const MAX_FORK_VARIANTS: usize = 16;

/// Largest number of events of one author retained past its usable prefix.
pub const MAX_WAITING_PER_AUTHOR: usize = 1024;

/// One author's log as it will stand once the batch so far is held.
struct Projected {
    /// The length of the usable prefix.
    usable: u64,
    /// The last event of the usable prefix.
    tip: Option<EventId>,
    /// Events past the usable prefix.
    waiting: usize,
}

/// The subset of `events` to retain for `goal`, in the order given.
pub(super) fn screen(goal: GoalId, history: &History, events: Vec<Event>) -> Vec<Event> {
    // The coordinator is the author of any genesis of this goal; one that
    // arrives in this batch founds it for the rest of the batch.
    let of_goal = |event: &&Event| event.header().goal == goal;
    let coordinator = history.coordinator.or_else(|| {
        let mut founding = events.iter().filter(of_goal);
        founding.find_map(|event| match event.header().body {
            Body::Genesis(genesis) => Some(genesis.coordinator),
            _ => None,
        })
    });
    let Some(coordinator) = coordinator else {
        return Vec::new();
    };
    // Admissions count wherever they stand in the batch, so a member's
    // events are not dropped for arriving before the decision that admits it.
    let admitted: Vec<PublicKey> = events
        .iter()
        .filter(of_goal)
        .filter(|event| event.header().author == coordinator)
        .filter_map(|event| match event.header().body {
            Body::MemberAdmitted { member, .. } => Some(member),
            _ => None,
        })
        .collect();

    // Most exchanges are ordinary prefix extension. Build the canonical
    // dependency projection only when a quota could drop an event.
    let mut counts = HashMap::<(PublicKey, u64), usize>::new();
    let mut per_author = HashMap::<PublicKey, usize>::new();
    for event in events.iter().filter(of_goal) {
        *counts
            .entry((event.header().author, event.header().seq))
            .or_default() += 1;
        *per_author.entry(event.header().author).or_default() += 1;
    }
    let quota = counts.iter().any(|((author, seq), count)| {
        history.log(author).map_or(0, |log| log.variants(*seq)) + count > MAX_FORK_VARIANTS
    }) || per_author.iter().any(|(author, count)| {
        history.log(author).map_or(0, |log| log.waiting()) + count > MAX_WAITING_PER_AUTHOR
    });
    let required = if quota {
        // Only the unbroken coordinator chain can authorize retention;
        // project the whole batch so arrival order cannot hide its refs.
        let mut projected = History::default();
        for event in history.events.iter().chain(events.iter().filter(of_goal)) {
            projected.insert(event);
        }
        Commitments::build(&projected, &Chain::build(&projected)).required
    } else {
        IdSet::default()
    };
    let mut seen = IdSet::default();
    let mut variants: HashMap<(PublicKey, u64), usize> = HashMap::new();
    let mut logs: HashMap<PublicKey, Projected> = HashMap::new();
    let mut kept = Vec::with_capacity(events.len());
    for event in events {
        let header = event.header();
        let (author, seq) = (header.author, header.seq);
        if header.goal != goal || history.slot(&event.id()).is_some() || !seen.insert(event.id()) {
            continue;
        }
        if author != coordinator
            && !history.admits(&coordinator, &author)
            && !admitted.contains(&author)
        {
            continue;
        }
        let held = history.log(&author);
        let at_position = variants
            .entry((author, seq))
            .or_insert_with(|| held.map_or(0, |log| log.variants(seq)));
        if *at_position >= MAX_FORK_VARIANTS && !required.contains(&event.id()) {
            continue;
        }
        let log = logs.entry(author).or_insert_with(|| Projected {
            usable: held.map_or(0, |log| log.usable as u64),
            tip: held.and_then(|log| log.tip()),
            waiting: held.map_or(0, |log| log.waiting()),
        });
        // The event that extends the usable prefix is never one too many:
        // it is what the waiting ones wait for.
        if seq == log.usable && *at_position == 0 && header.prev == log.tip {
            log.usable += 1;
            log.tip = Some(event.id());
        } else if *at_position == 1 {
            // First conflicting evidence must survive a full waiting set.
            log.waiting += 1;
        } else if log.waiting >= MAX_WAITING_PER_AUTHOR && !required.contains(&event.id()) {
            continue;
        } else {
            log.waiting += 1;
        }
        *at_position += 1;
        kept.push(event);
    }
    kept
}
