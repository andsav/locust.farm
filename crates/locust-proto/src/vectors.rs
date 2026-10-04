//! Golden bytes for protocol version 1.
//!
//! These pin the wire format: the header encoding, identifier derivation,
//! signature, the founding self-admission, enum variant indices and the
//! invitation ticket. A change that makes one of these tests fail changes
//! what existing peers and stored goals understand, so it needs a new
//! protocol version, not an updated constant.

use serde::Serialize;

use crate::PROTOCOL_VERSION;
use crate::codec;
use crate::crypto::content_hash;
use crate::event::{Body, CancelOutcome, Doc, Event, PayloadRef};
use crate::id::{EndpointId, Hex};
use crate::invite::{Invitation, InviteSecret};
use crate::store::Blob;
use crate::testkit::{self, Author};

const OWNER: &str = "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c";
const MEMBER: &str = "8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394";
const GOAL: &str = "0e3b678262546bfdce982c2a35432985908cf52de085300e2a1635d70907fd99";

const GENESIS_HEADER: &str = "010e3b678262546bfdce982c2a35432985908cf52de085300e2a1635d70907fd998a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0000000080d8c1a28c3400008a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c00000000000000000000000000000000";
const GENESIS_ID: &str = "999ef1e6f79e2bd804b174fbe68e83621bfad8707fa0685ec4be4457adbfd128";
const GENESIS_SIGNATURE: &str = "647dd39f1a8942460cba0aae84917fe8214e76f09318bd1695a2bf203dd890c6ab68b14179767c5b00522d3062d929a77d233ea38418620c2ed293143292ff02";

/// The coordinator daemon's endpoint, also the redeeming endpoint of the ticket.
const COORDINATOR_ENDPOINT: EndpointId = EndpointId([7; 32]);
const ADMISSION_HEADER: &str = "010e3b678262546bfdce982c2a35432985908cf52de085300e2a1635d70907fd998a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0101999ef1e6f79e2bd804b174fbe68e83621bfad8707fa0685ec4be4457adbfd12801999ef1e6f79e2bd804b174fbe68e83621bfad8707fa0685ec4be4457adbfd1280081d8c1a28c3400018a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0707070707070707070707070707070707070707070707070707070707070707";
const ADMISSION_ID: &str = "dc5bd51b96699d71ec3f634d7b0c7f2ddb24f6bc6e0721831e7228e85d65d147";
const ADMISSION_SIGNATURE: &str = "5d69a11d98cf5f92df3a848614d4b3e0edec94b1eff56c69688d0f9c01e395716f3f84bd3b0b8c4975e9c54c404a4fb22b7e9c88c13290a7ee2a1a9fa7cfb005";

/// The task text. The header names its sealed form: the text sealed for the
/// goal at epoch 0 under the published test key `testkit::content_key(1)`.
const TASK_TEXT: &[u8] = b"Add a login page";
const TASK_PAYLOAD: &str = "0100000000bdc41277a2427bf52e74c059362b225a8ff048c81d7f51c982170560d42bb3d6ef6d824124b48f0efd65b0ea22d17422721a182f38bf7e36";
const TASK_PAYLOAD_HASH: &str = "02bb4f55de669a4a8ff8e23c6c4a0ed64d83f6762ce643f236103cae5d2cd0d7";
const TASK_HEADER: &str = "010e3b678262546bfdce982c2a35432985908cf52de085300e2a1635d70907fd998139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394000001999ef1e6f79e2bd804b174fbe68e83621bfad8707fa0685ec4be4457adbfd1280080d8c1a28c340102bb4f55de669a4a8ff8e23c6c4a0ed64d83f6762ce643f236103cae5d2cd0d73d0008000001a0e5c7a28c340103";
const TASK_ID: &str = "b5a7df4c6b12e0100c774d036879ef612170c7b2dc7b99b7abfac0b74f8c416d";
const TASK_SIGNATURE: &str = "6ad1d987958fbe0ca55118623bce853f5ef3994efabad767d85a4b37e26d80d627428c5b058110f682b8b89f4282d362eed4650b631455ffe0e2b5f5564dea03";

/// Running digest of the coordinator's log after genesis and self-admission.
const FOUNDING_LOG_DIGEST: &str =
    "cb75a6886331e78ab4f41e8ce2e26a50f0fb9e2d124148014ed2ca2412258dd9";

const TICKET: &str = "locust-invite-010e3b678262546bfdce982c2a35432985908cf52de085300e2a1635d70907fd998a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0707070707070707070707070707070707070707070707070707070707070707011568747470733a2f2f72656c61792e6578616d706c650909090909090909090909090909090909090909090909090909090909090909018090dbcb8c34";

fn genesis() -> (Author, Event) {
    let mut owner = Author::new(1);
    let genesis = owner.genesis();
    (owner, genesis)
}

/// The two events goal creation commits: genesis and the coordinator's
/// admission of itself.
fn founding() -> (Event, Event) {
    Author::new(1).found_goal(COORDINATOR_ENDPOINT)
}

fn task_payload(goal: &crate::id::GoalId) -> (PayloadRef, Blob) {
    testkit::sealed_payload(goal, 0, TASK_TEXT)
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
        Some(task_payload(&genesis.header().goal).0),
    );
    (member, task)
}

/// The frozen declaration index and name of every [`Body`] variant. The match
/// is exhaustive, so a new variant does not compile until it is entered here,
/// and a reordered one fails [`enum_variant_indices_are_frozen`].
fn body_variant(body: &Body) -> (u8, &'static str) {
    match body {
        Body::Genesis(_) => (0, "genesis"),
        Body::MemberAdmitted { .. } => (1, "member_admitted"),
        Body::MemberRemoved { .. } => (2, "member_removed"),
        Body::TaskAssigned { .. } => (3, "task_assigned"),
        Body::CancelRequested { .. } => (4, "cancel_requested"),
        Body::ResultAccepted { .. } => (5, "result_accepted"),
        Body::ResultRejected { .. } => (6, "result_rejected"),
        Body::RevisionAccepted { .. } => (7, "revision_accepted"),
        Body::TaskProposed { .. } => (8, "task_proposed"),
        Body::AssignmentAccepted { .. } => (9, "assignment_accepted"),
        Body::AssignmentDeclined { .. } => (10, "assignment_declined"),
        Body::Progress { .. } => (11, "progress"),
        Body::ResultSubmitted { .. } => (12, "result_submitted"),
        Body::AttemptFailed { .. } => (13, "attempt_failed"),
        Body::CancelAcknowledged { .. } => (14, "cancel_acknowledged"),
        Body::Note { .. } => (15, "note"),
        Body::Revision { .. } => (16, "revision"),
        Body::LeaveRequested => (17, "leave_requested"),
    }
}

/// The frozen index and name of every [`CancelOutcome`]; exhaustive like
/// [`body_variant`].
fn cancel_outcome_variant(outcome: CancelOutcome) -> (u8, &'static str) {
    match outcome {
        CancelOutcome::Stopped => (0, "stopped"),
        CancelOutcome::Completed => (1, "completed"),
        CancelOutcome::Uncertain => (2, "uncertain"),
    }
}

/// The frozen index and name of every [`Doc`]; exhaustive like
/// [`body_variant`].
fn doc_variant(doc: Doc) -> (u8, &'static str) {
    match doc {
        Doc::Plan => (0, "plan"),
        Doc::Summary => (1, "summary"),
    }
}

/// The tag a variant renders under in JSON: a unit variant renders as its
/// tag, any other as an object with that one key.
fn json_tag<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value).unwrap() {
        serde_json::Value::String(tag) => tag,
        serde_json::Value::Object(map) if map.len() == 1 => map.into_iter().next().unwrap().0,
        other => panic!("{other} is not an enum variant"),
    }
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
fn coordinator_self_admission() {
    let (genesis, admission) = founding();
    assert_eq!(genesis.id().to_string(), GENESIS_ID);
    assert_eq!(Hex(admission.header_bytes()).to_string(), ADMISSION_HEADER);
    assert_eq!(admission.id().to_string(), ADMISSION_ID);
    assert_eq!(admission.signature().to_string(), ADMISSION_SIGNATURE);
}

#[test]
fn task_proposal_with_a_sealed_payload() {
    let (_, genesis) = genesis();
    let goal = genesis.header().goal;
    let (_, task) = task(&genesis);
    let (payload, blob) = task_payload(&goal);
    assert_eq!(Hex(blob.bytes()).to_string(), TASK_PAYLOAD);
    assert_eq!(blob.hash().to_string(), TASK_PAYLOAD_HASH);
    assert_eq!(content_hash(blob.bytes()), blob.hash());
    assert!(payload.admits(&blob));
    assert_eq!(
        crate::seal::open(&goal, &testkit::content_key(1), blob.bytes()).as_deref(),
        Ok(TASK_TEXT)
    );
    assert_eq!(Hex(task.header_bytes()).to_string(), TASK_HEADER);
    assert_eq!(task.id().to_string(), TASK_ID);
    assert_eq!(task.signature().to_string(), TASK_SIGNATURE);
}

#[test]
fn founding_log_digest() {
    let (genesis, admission) = founding();
    let points = [genesis, admission].map(|event| crate::event::AuthorPoint {
        seq: event.header().seq,
        id: event.id(),
    });
    let frontier = crate::sync::AuthorFrontier::from_points(Author::new(1).key.public(), &points);
    assert_eq!(frontier.next_seq, 2);
    assert_eq!(Hex(&frontier.digest).to_string(), FOUNDING_LOG_DIGEST);
    assert_eq!(frontier.digest, crate::sync::log_digest(&points));
}

#[test]
fn published_bytes_decode_to_the_same_events() {
    let (_, expected_genesis) = genesis();
    let (_, expected_task) = task(&expected_genesis);
    let (_, expected_admission) = founding();
    for (header, signature, expected) in [
        (GENESIS_HEADER, GENESIS_SIGNATURE, expected_genesis),
        (ADMISSION_HEADER, ADMISSION_SIGNATURE, expected_admission),
        (TASK_HEADER, TASK_SIGNATURE, expected_task),
    ] {
        let bytes = crate::id::hex_to_vec(header).unwrap();
        let decoded = Event::decode(&bytes, signature.parse().unwrap()).unwrap();
        assert_eq!(decoded, expected);
    }
}

#[test]
fn enum_variant_indices_are_frozen() {
    let bodies = testkit::every_body();
    assert_eq!(bodies.len(), 18, "every Body variant, once");
    for (position, body) in bodies.iter().enumerate() {
        let (index, name) = body_variant(body);
        assert_eq!(usize::from(index), position, "{name} is out of order");
        assert_eq!(codec::encode(body).unwrap()[0], index, "{name}");
        assert_eq!(body.kind(), name);
        assert_eq!(json_tag(body), name);
    }

    let outcomes = [
        CancelOutcome::Stopped,
        CancelOutcome::Completed,
        CancelOutcome::Uncertain,
    ];
    for (position, outcome) in outcomes.into_iter().enumerate() {
        let (index, name) = cancel_outcome_variant(outcome);
        assert_eq!(usize::from(index), position, "{name} is out of order");
        assert_eq!(codec::encode(&outcome).unwrap(), [index], "{name}");
        assert_eq!(json_tag(&outcome), name);
    }

    for (position, doc) in [Doc::Plan, Doc::Summary].into_iter().enumerate() {
        let (index, name) = doc_variant(doc);
        assert_eq!(usize::from(index), position, "{name} is out of order");
        assert_eq!(codec::encode(&doc).unwrap(), [index], "{name}");
        assert_eq!(json_tag(&doc), name);
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
        secret: InviteSecret([9; 32]),
        expires_ms: Some(1_790_086_400_000),
    };
    assert_eq!(invitation.to_ticket().unwrap().as_str(), TICKET);
    assert_eq!(Invitation::from_ticket(TICKET), Ok(invitation));
}

#[test]
fn version_zero_events_and_invitations_are_explicitly_refused() {
    let mut header = crate::id::hex_to_vec(GENESIS_HEADER).unwrap();
    header[0] = 0;
    assert_eq!(
        Event::decode(&header, GENESIS_SIGNATURE.parse().unwrap()),
        Err(crate::event::EventError::UnsupportedVersion(0))
    );
    let old_ticket = TICKET.replacen("locust-invite-01", "locust-invite-00", 1);
    assert_eq!(
        Invitation::from_ticket(&old_ticket),
        Err(crate::invite::InviteError::UnsupportedVersion(0))
    );
}
