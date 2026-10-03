//! Golden bytes for protocol version 0.
//!
//! These pin the wire format: the header encoding, identifier derivation,
//! signature and invitation ticket. A change that makes one of these tests
//! fail changes what existing peers and stored goals understand, so it needs
//! a new protocol version, not an updated constant.

use crate::PROTOCOL_VERSION;
use crate::crypto::content_hash;
use crate::event::{Body, Event, PayloadRef};
use crate::id::{EndpointId, Hex};
use crate::invite::Invitation;
use crate::testkit::Author;

const OWNER: &str = "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c";
const MEMBER: &str = "8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394";
const GOAL: &str = "0e3b678262546bfdce982c2a35432985908cf52de085300e2a1635d70907fd99";

const GENESIS_HEADER: &str = "000e3b678262546bfdce982c2a35432985908cf52de085300e2a1635d70907fd998a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0000000080d8c1a28c3400008a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c00000000000000000000000000000000";
const GENESIS_ID: &str = "5b311e10d8dd54e55b4f9e9c755fbfa9d71714aa1cb29e085368c007e1aadc19";
const GENESIS_SIGNATURE: &str = "062a87a9af38f6a13a7702ba62a9de4b0f6482b7183091c0d88d06ff46ae9818b243eadfa332c5e5146595de64ad88e8879d38a932be59b06f58bff5e23ed50f";

const TASK_TEXT: &[u8] = b"Add a login page";
const TASK_TEXT_HASH: &str = "19a633ea60fc719a2cbdfd2a0abe650bef4053e17c8f118d75ef311d0cbc4406";
const TASK_HEADER: &str = "000e3b678262546bfdce982c2a35432985908cf52de085300e2a1635d70907fd998139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b3940000015b311e10d8dd54e55b4f9e9c755fbfa9d71714aa1cb29e085368c007e1aadc190080d8c1a28c340119a633ea60fc719a2cbdfd2a0abe650bef4053e17c8f118d75ef311d0cbc4406100008000001a0e5c7a28c340103";
const TASK_ID: &str = "11e803a59fe8c5205a3a04e0765f4343699b555b60942f0bef8527ea3843c6de";
const TASK_SIGNATURE: &str = "1631d241359785ad01edf6f4a0aca8a7623a1310bb141fcb56cef0313cbd108efbb85645823d8a6d5db96a827612aef68b7aa13a888401ea0bf04120fd2c2903";

const TICKET: &str = "locust-invite-000e3b678262546bfdce982c2a35432985908cf52de085300e2a1635d70907fd998a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0707070707070707070707070707070707070707070707070707070707070707011568747470733a2f2f72656c61792e6578616d706c650909090909090909090909090909090909090909090909090909090909090909018090dbcb8c34";

fn genesis() -> (Author, Event) {
    let mut owner = Author::new(1);
    let genesis = owner.genesis();
    (owner, genesis)
}

fn task(genesis: &Event) -> (Author, Event) {
    let mut member = Author::new(2);
    let task = member.event_with(
        genesis.header().goal,
        Some(genesis.id()),
        Body::TaskProposed {
            input: None,
            depends_on: Vec::new(),
            deadline_ms: Some(1_790_000_100_000),
            max_attempts: Some(3),
        },
        Some(PayloadRef {
            hash: content_hash(TASK_TEXT),
            len: TASK_TEXT.len() as u32,
            key_epoch: None,
        }),
    );
    (member, task)
}

#[test]
fn keys_and_goal_identifier() {
    let (owner, genesis) = genesis();
    assert_eq!(owner.key.public().to_string(), OWNER);
    assert_eq!(Author::new(2).key.public().to_string(), MEMBER);
    assert_eq!(genesis.header().goal.to_string(), GOAL);
}

#[test]
fn genesis_event() {
    let (_, genesis) = genesis();
    assert_eq!(Hex(genesis.header_bytes()).to_string(), GENESIS_HEADER);
    assert_eq!(genesis.id().to_string(), GENESIS_ID);
    assert_eq!(genesis.signature().to_string(), GENESIS_SIGNATURE);
}

#[test]
fn task_proposal_with_a_payload() {
    let (_, genesis) = genesis();
    let (_, task) = task(&genesis);
    assert_eq!(content_hash(TASK_TEXT).to_string(), TASK_TEXT_HASH);
    assert_eq!(Hex(task.header_bytes()).to_string(), TASK_HEADER);
    assert_eq!(task.id().to_string(), TASK_ID);
    assert_eq!(task.signature().to_string(), TASK_SIGNATURE);
}

#[test]
fn published_bytes_decode_to_the_same_events() {
    let (_, expected_genesis) = genesis();
    let (_, expected_task) = task(&expected_genesis);
    for (header, signature, expected) in [
        (GENESIS_HEADER, GENESIS_SIGNATURE, expected_genesis),
        (TASK_HEADER, TASK_SIGNATURE, expected_task),
    ] {
        let bytes = crate::id::hex_to_vec(header).unwrap();
        let decoded = Event::decode(&bytes, signature.parse().unwrap()).unwrap();
        assert_eq!(decoded, expected);
    }
}

#[test]
fn invitation_ticket() {
    let (owner, genesis) = genesis();
    let invitation = Invitation {
        version: PROTOCOL_VERSION,
        goal: genesis.header().goal,
        coordinator: owner.key.public(),
        endpoint: EndpointId([7; 32]),
        hints: vec!["https://relay.example".to_string()],
        secret: [9; 32],
        expires_ms: Some(1_790_086_400_000),
    };
    assert_eq!(invitation.to_ticket().unwrap(), TICKET);
    assert_eq!(Invitation::from_ticket(TICKET), Ok(invitation));
}
