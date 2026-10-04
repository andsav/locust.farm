//! Retention admission is not action authorization. Keep attributed evidence
//! from admitted identities, including forks and exact pending proof closure.
use std::collections::BTreeSet;

use locust_proto::event::{Body, Event};
use locust_proto::id::GoalId;

use super::history::History;

pub(super) fn screen(goal: GoalId, history: &History, events: Vec<Event>) -> Vec<Event> {
    let administrator = history.administrator.or_else(|| {
        events
            .iter()
            .filter(|event| event.header().goal == goal)
            .find_map(|event| match &event.header().body {
                Body::Genesis(genesis) => Some(genesis.administrator),
                _ => None,
            })
    });
    let Some(administrator) = administrator else {
        return Vec::new();
    };
    let admitted: BTreeSet<_> = history
        .events
        .iter()
        .chain(&events)
        .filter(|event| event.header().goal == goal && event.header().author == administrator)
        .filter_map(|event| match &event.header().body {
            Body::MemberAdmitted { member, .. } => Some(*member),
            _ => None,
        })
        .collect();
    let mut seen = BTreeSet::new();
    let mut retained: Vec<_> = events
        .into_iter()
        .filter(|event| {
            event.header().goal == goal
                && (event.header().author == administrator
                    || admitted.contains(&event.header().author))
                && history.get(&event.id()).is_none()
                && seen.insert(event.id())
        })
        .collect();
    retained.sort_by_key(|event| (event.header().author, event.header().seq, event.id()));
    retained
}
