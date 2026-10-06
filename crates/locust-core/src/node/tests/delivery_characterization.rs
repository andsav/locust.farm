//! Deliberately withheld epoch key, with production replica ingestion.
use super::*;
use crate::node::tests::lifecycle::{event, finding};
use crate::sync::Host;
use locust_proto::event::Doc;

fn missing_key() -> (
    Network,
    GoalId,
    PublicKey,
    PublicKey,
    locust_proto::event::Event,
) {
    let (mut net, goal, host, survivor) = ready_network(false);
    let a = net.nodes[0].connect(credential(1), None);
    let (removed, _) = crate::node::tests::authorization::join_local(&mut net.nodes[0], a, goal, 3);
    net.poll(60_000);
    let owner = net.nodes[0].owner();
    let removal = event(net.nodes[0].ok(
        owner,
        Request::MemberRemove {
            goal,
            member: removed,
        },
    ));
    let removal = net.nodes[0].store.event(&removal).unwrap().unwrap();
    // Deliver the governance record but no key or content, then do not
    // poll the transport again. This fixes the outage boundary exactly.
    net.nodes[1]
        .node
        .replica(&goal)
        .unwrap()
        .receive(vec![removal.to_wire()])
        .unwrap();
    assert_eq!(net.nodes[1].node.goals[&goal].state().epoch, 1);
    assert!(!net.nodes[1].node.goals[&goal].keys.contains_key(&1));
    assert!(net.nodes[0].node.goals[&goal].keys.contains_key(&1));
    (net, goal, host, survivor, removal)
}

#[test]
fn undelivered_removal_key_blocks_survivor_text_but_host_can_write_and_survivor_can_leave() {
    let (mut net, goal, host, survivor, _) = missing_key();
    net.nodes[1].restart();
    assert!(!net.nodes[1].node.goals[&goal].keys.contains_key(&1));
    let owner = net.nodes[1].owner();
    let before = net.nodes[1].store.log(&goal, 0, usize::MAX).unwrap().len();
    for request in [
        finding(goal, "no new key"),
        Request::TaskOpen {
            goal,
            text: "no new key".into(),
            task_type: None,
            inputs: Default::default(),
            parent: None,
        },
        Request::DocRevise {
            goal,
            doc: Doc::Plan,
            base: None,
            text: "no new key".into(),
        },
    ] {
        let error = net.nodes[1]
            .on_behalf(owner, survivor, request)
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Unavailable);
        assert!(error.message.contains("content key"), "{error}");
    }
    assert_eq!(
        net.nodes[1].store.log(&goal, 0, usize::MAX).unwrap().len(),
        before
    );
    let authored = host_note(&mut net, goal, "host still holds the key");
    assert_eq!(authored.header().author, host);
    assert_eq!(authored.header().payload.unwrap().key_epoch, 1);
    // A keyless signature still commits during the outage.
    let leave = net.nodes[1]
        .call(
            owner,
            Request::GoalLeave {
                goal,
                agent: survivor,
            },
        )
        .unwrap();
    let leave = event(leave);
    assert!(
        net.nodes[1]
            .store
            .event(&leave)
            .unwrap()
            .unwrap()
            .header()
            .payload
            .is_none()
    );
}

#[test]
fn verified_delivery_of_withheld_removal_key_restores_survivor_text_writes() {
    let (mut net, goal, _, survivor, removal) = missing_key();
    let owner = net.nodes[1].owner();
    assert_eq!(
        code(net.nodes[1].on_behalf(owner, survivor, finding(goal, "before key"))),
        ErrorCode::Unavailable
    );
    let payload = removal.header().payload.unwrap();
    let bytes = net.nodes[0].store.blob(&payload.hash).unwrap().unwrap();
    let key = net.nodes[0].node.goals[&goal].keys[&1];
    let replica = net.nodes[1].node.replica(&goal).unwrap();
    assert_eq!(
        replica.stage(&payload.hash, 0, bytes.len() as u64, &bytes),
        crate::sync::Staged::Complete
    );
    assert!(replica.offer_key(1, key));
    let authored = event(
        net.nodes[1]
            .on_behalf(owner, survivor, finding(goal, "after key"))
            .unwrap(),
    );
    assert_eq!(
        net.nodes[1]
            .store
            .event(&authored)
            .unwrap()
            .unwrap()
            .header()
            .payload
            .unwrap()
            .key_epoch,
        1
    );
}
