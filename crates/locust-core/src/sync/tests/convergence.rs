//! Replicas converge on the union of what they hold, whatever the order of
//! starts, duplicated or cut-off exchanges and restarts.

use std::collections::BTreeSet;

use locust_proto::event::{Body, Event};
use locust_proto::id::{EndpointId, EventId};
use locust_proto::limits::MAX_INVENTORY_POINTS;
use locust_proto::testkit::{self, Author};

use super::net::Net;
use super::{Founded, host};

fn union(sets: &[&[Event]]) -> BTreeSet<EventId> {
    sets.iter()
        .flat_map(|events| events.iter().map(Event::id))
        .collect()
}

/// Three replicas: the owner's notes, a member's and another member's, each
/// held by one replica only.
fn three() -> (Founded, Net, BTreeSet<EventId>) {
    let mut founded = Founded::new();
    let mut owner = std::mem::replace(&mut founded.owner, Author::new(0));
    let mine = founded.notes(&mut owner, 5);
    let theirs = founded.notes(&mut Author::new(2), 300);
    let others = founded.notes(&mut Author::new(3), 7);
    let all = union(&[&mine, &theirs, &others, &[founded.genesis.clone()]]);
    let net = Net::new(vec![
        host(1, founded.replica(&mine), &[1, 2, 3]),
        host(2, founded.replica(&theirs), &[1, 2, 3]),
        host(3, founded.replica(&others), &[1, 2, 3]),
    ]);
    (founded, net, all)
}

fn assert_converged(net: &Net, founded: &Founded, all: &BTreeSet<EventId>) {
    for node in &net.nodes {
        assert_eq!(&node.host.ids(&founded.goal), all);
    }
}

#[test]
fn two_replicas_converge_on_the_union_of_their_events() {
    let mut founded = Founded::new();
    let mut owner = std::mem::replace(&mut founded.owner, Author::new(0));
    let mine = founded.notes(&mut owner, 3);
    let theirs = founded.notes(&mut Author::new(2), 2);
    let all = union(&[&mine, &theirs, &[founded.genesis.clone()]]);
    let mut net = Net::new(vec![
        host(1, founded.replica(&mine), &[1, 2]),
        host(2, founded.replica(&theirs), &[1, 2]),
    ]);

    net.poll(&[0]);
    assert_converged(&net, &founded, &all);
    net.quiesce();
    assert_converged(&net, &founded, &all);
    assert_eq!(net.oversized, 0);
}

#[test]
fn three_replicas_converge_whatever_order_they_start_in() {
    for order in [[0, 1, 2], [2, 1, 0], [1, 2, 0], [1, 0, 2]] {
        let (founded, mut net, all) = three();
        net.poll(&order);
        net.quiesce();
        assert_converged(&net, &founded, &all);
    }
}

#[test]
fn replicas_converge_when_one_starts_alone_and_others_join_later() {
    let (founded, mut net, all) = three();
    net.set_online(1, false);
    net.set_online(2, false);
    net.poll(&[0]);
    net.set_online(2, true);
    net.tick(61_000);
    net.set_online(1, true);
    net.quiesce();
    assert_converged(&net, &founded, &all);
}

#[test]
fn concurrent_exchanges_in_both_directions_converge() {
    let (founded, mut net, all) = three();
    // Every node opens to every other at once, before any frame moves.
    net.poll_without_settling(&[0, 1, 2]);
    net.poll_without_settling(&[2, 1, 0]);
    net.settle();
    net.quiesce();
    assert_converged(&net, &founded, &all);
}

#[test]
fn exchanges_cut_off_after_any_frame_lose_nothing_and_converge_once_whole() {
    for cut in 0..24 {
        let (founded, mut net, all) = three();
        net.cut = Some(cut);
        net.poll(&[0, 1, 2]);
        for _ in 0..5 {
            net.tick(61_000);
        }
        // What arrived before a cut is kept.
        for node in &net.nodes {
            assert!(node.host.ids(&founded.goal).is_subset(&all));
        }
        net.cut = None;
        net.tick(61_000);
        net.quiesce();
        assert_converged(&net, &founded, &all);
    }
}

#[test]
fn a_restarted_replica_catches_up_from_its_store() {
    let (founded, mut net, all) = three();
    net.poll(&[0, 1, 2]);
    // Node 0 goes down; the others keep writing.
    net.set_online(0, false);
    let later = founded.notes(&mut Author::new(3), 4);
    net.host(2).replica_mut(&founded.goal).insert(&later);
    net.tick(1_000);
    // Node 0 restarts from its store.
    let store = net.host(0).replica_mut(&founded.goal).store.reopen();
    let reopened = super::replica::TestReplica::open(founded.goal, store);
    net.host(0).replicas.insert(founded.goal, reopened);
    net.restart(0);
    net.set_online(0, true);
    net.quiesce();
    let all: BTreeSet<EventId> = all
        .union(&super::convergence::ids(&later))
        .copied()
        .collect();
    assert_converged(&net, &founded, &all);
}

pub fn ids(events: &[Event]) -> BTreeSet<EventId> {
    events.iter().map(Event::id).collect()
}

/// Lane B's B-R3: the same genesis and different decisions at position 1
/// gave equal frontiers and never reconciled.
#[test]
fn equal_length_divergent_histories_converge_with_both_events_on_both_sides() {
    let mut owner = Author::new(1);
    let mut twin = Author::new(1);
    let genesis = owner.genesis(testkit::keypair(9).public());
    assert_eq!(twin.genesis(testkit::keypair(9).public()), genesis);
    let goal = genesis.header().goal;
    let admit = |signer: &mut Author, endpoint: u8| {
        signer.event(
            goal,
            Some(genesis.id()),
            Body::MemberAdmitted {
                name: "member".into(),
                role: None,
                member: testkit::keypair(3).public(),
                endpoint: EndpointId([endpoint; 32]),
            },
        )
    };
    let left = admit(&mut owner, 1);
    let right = admit(&mut twin, 2);
    let founded = Founded {
        goal,
        genesis: genesis.clone(),
        owner: Author::new(0),
    };
    let all = union(&[&[genesis.clone(), left.clone(), right.clone()]]);
    let mut net = Net::new(vec![
        host(1, founded.replica(std::slice::from_ref(&left)), &[1, 2]),
        host(2, founded.replica(std::slice::from_ref(&right)), &[1, 2]),
    ]);
    net.poll(&[0]);
    assert_converged(&net, &founded, &all);
    net.quiesce();
    assert_converged(&net, &founded, &all);
}

#[test]
fn a_forked_author_with_more_variants_than_one_inventory_page_converges() {
    let founded = Founded::new();
    let variants = MAX_INVENTORY_POINTS * 2 + 100;
    // One author signs `variants` different events at position 0, split
    // between two replicas with one in common.
    let fork: Vec<Event> = (0..variants)
        .map(|n| {
            let mut author = Author::new(2);
            author.event(
                founded.goal,
                founded.anchor(),
                Body::ContributionPublished {
                    context: locust_proto::event::Context {
                        scope: locust_proto::event::Scope::Goal,
                        round: EventId([(n % 251) as u8; 32]),
                    },
                    attempt: None,
                    sources: Vec::new(),
                    artifacts: vec![],
                },
            )
        })
        .collect();
    let (left, right) = fork.split_at(variants / 3);
    let mut right = right.to_vec();
    right.push(left[0].clone());
    let all = union(&[&fork, std::slice::from_ref(&founded.genesis)]);
    let mut net = Net::new(vec![
        host(1, founded.replica(left), &[1, 2]),
        host(2, founded.replica(&right), &[1, 2]),
    ]);
    net.poll(&[0]);
    assert_converged(&net, &founded, &all);
}

#[test]
fn a_one_event_difference_uses_no_inventory_in_either_dial_direction() {
    use locust_proto::sync::SyncMessage;
    let founded = Founded::new();
    let events = founded.notes(&mut Author::new(2), 1000);
    for dialer in [0, 1] {
        let mut net = Net::new(vec![
            host(1, founded.replica(&events[..999]), &[1, 2]),
            host(2, founded.replica(&events), &[1, 2]),
        ]);
        net.poll(&[dialer]);
        assert_eq!(
            net.nodes[0].host.ids(&founded.goal),
            net.nodes[1].host.ids(&founded.goal)
        );
        assert!(net.log.iter().all(|(_, frame)| !matches!(
            frame,
            SyncMessage::Inventory { .. } | SyncMessage::InventoryRequest { .. }
        )));
        assert_eq!(
            net.log
                .iter()
                .filter_map(|(_, frame)| if let SyncMessage::Events(events) = frame {
                    Some(events.len())
                } else {
                    None
                })
                .sum::<usize>(),
            1
        );
    }
}

#[test]
fn unequal_divergent_prefixes_still_exchange_inventory_and_converge() {
    use locust_proto::sync::SyncMessage;
    let founded = Founded::new();
    let mut a = Author::new(2);
    let mut b = Author::new(2);
    let left = vec![a.event(
        founded.goal,
        founded.anchor(),
        Body::ContributionPublished {
            context: locust_proto::event::Context {
                scope: locust_proto::event::Scope::Goal,
                round: locust_proto::id::EventId([1; 32]),
            },
            attempt: None,
            sources: Vec::new(),
            artifacts: vec![],
        },
    )];
    let mut right = vec![b.event(
        founded.goal,
        founded.anchor(),
        Body::ContributionPublished {
            context: locust_proto::event::Context {
                scope: locust_proto::event::Scope::Goal,
                round: founded.genesis.id(),
            },
            attempt: None,
            sources: Vec::new(),
            artifacts: vec![],
        },
    )];
    right.extend(founded.notes(&mut b, 4));
    let all = union(&[&left, &right, std::slice::from_ref(&founded.genesis)]);
    for dialer in [0, 1] {
        let mut net = Net::new(vec![
            host(1, founded.replica(&left), &[1, 2]),
            host(2, founded.replica(&right), &[1, 2]),
        ]);
        net.poll(&[dialer]);
        assert_converged(&net, &founded, &all);
        assert!(
            net.log
                .iter()
                .any(|(_, frame)| matches!(frame, SyncMessage::Inventory { .. }))
        );
    }
}
