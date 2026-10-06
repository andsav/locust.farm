//! Frozen protocol-7 encodings. Unsupported protocol versions have no reader.
use crate::event::*;
use crate::id::*;
use crate::organization::{Authority, CompletionRule, Formation, Selector};
use crate::testkit::{self, Author};
use std::collections::BTreeMap;
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn transcript() -> (Vec<Event>, Effect, crate::store::Blob) {
    let mut definition = Formation::default();
    definition.decisions.completion = CompletionRule::Reviews {
        by: Selector::Members,
        count: 1,
        exclude_author: true,
    };
    definition.decisions.selection = Some(Authority::Participant {
        key: testkit::keypair(4).public().to_string(),
    });
    // The governance key signs the founding prefix and the host's agent, a
    // member, decides.
    let mut governance = Author::new(1);
    let mut worker = Author::new(2);
    let mut reviewer = Author::new(3);
    let mut host = Author::new(4);
    let genesis = governance.genesis_with(host.key.public(), &definition);
    let goal = genesis.header().goal;
    let mut anchor = genesis.id();
    let mut events = vec![genesis];
    for key in [
        host.key.public(),
        worker.key.public(),
        reviewer.key.public(),
    ] {
        let event = governance.event(
            goal,
            Some(anchor),
            Body::MemberAdmitted {
                member: key,
                endpoint: EndpointId(key.0),
            },
        );
        anchor = event.id();
        events.push(event);
    }
    let (binding, blob) = testkit::rules_binding(&goal, 0, &definition, BTreeMap::new());
    let bound = governance.event(
        goal,
        Some(anchor),
        Body::RulesBound {
            expected: None,
            binding,
        },
    );
    anchor = bound.id();
    events.push(bound);
    let task = worker.event(
        goal,
        Some(anchor),
        Body::TaskOpened {
            binding: TaskBinding {
                rules: anchor,
                task_type: None,
                inputs: BTreeMap::new(),
                parent: None,
                stage: None,
            },
        },
    );
    let context = Context {
        scope: Scope::Task(TaskId::Authored(task.id())),
        round: task.id(),
    };
    events.push(task);
    let subject = worker.event(
        goal,
        Some(anchor),
        Body::ContributionPublished {
            context,
            attempt: None,
            sources: vec![events[4].id()],
            artifacts: Vec::new(),
        },
    );
    let subject_id = subject.id();
    events.push(subject);
    let review = reviewer.event(
        goal,
        Some(anchor),
        Body::ReviewRecorded {
            context,
            subject: subject_id,
            verdict: ReviewVerdict::Approve,
        },
    );
    let review_id = review.id();
    events.push(review);
    let select = host.event(
        goal,
        Some(anchor),
        Body::ScopeDecided {
            context,
            previous: None,
            action: DecisionAction::Select {
                subject: subject_id,
            },
            evidence: vec![review_id],
        },
    );
    events.push(select);
    let effect = Effect {
        context,
        transition: "review".into(),
        trigger: Trigger::Contribution(subject_id),
        target_slot: reviewer.key.public().to_string(),
        action: EffectAction::RequestReview {
            context,
            subject: subject_id,
            recipient: reviewer.key.public(),
        },
        evidence: vec![subject_id],
    };
    (events, effect, blob)
}
const GOAL: &str = "18df46ce1036892230c7fda35e6a126a5d53292e3b108ff7a6d84281f5500a42";
const DEFINITION: &str = "3ae37e5bb845dca7c3b9d7857b6ad41b4d8fb6b926650b11ffc4996bcdac56d2";
const BLOB: &str = "2b74dd0380d0e39290b060e98235b60a1f888eb737206416cf831e38a9103056";
const EFFECT: &str = "770d299d316365cf7b6396fd844f98eae55e2092aa226ed58622d5d46350a07c";
const ID0: &str = "b0048b3e7a2b5c07935317e61b4f4ecab84069a8b14a36786c8d8cf4f1ea0f0e";
const SIGNATURE0: &str = "e6ec5dacdc02f17a7be1990956a02818d6d5ba35c502bb43b896b3b98cc512a88993cb0ffdd047c8cd47d52f59637c7f3dcbd0e4b8391b552a4410747269db0e";
const HEADER0: &str = "0718df46ce1036892230c7fda35e6a126a5d53292e3b108ff7a6d84281f5500a428a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0000000080d8c1a28c3400028a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5cca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c3ae37e5bb845dca7c3b9d7857b6ad41b4d8fb6b926650b11ffc4996bcdac56d200000000000000000000000000000000";
const ID1: &str = "2f2f9053bc168b17050beb18b647628c45fb70133ba0818d4be10580d57e3205";
const SIGNATURE1: &str = "e6a6c6fa7d646d29085ea995948591d45d4f1bbb96a7f27c6199e5152bf0c5a24b76fbd6b29d7015895de76535623ea2e2fba907d49802f6c1a3e1aab6a1d609";
const ID2: &str = "ae0561399222c74ed4353cb4c845e8771be9b4ddff012ef1f31af938d11f4912";
const SIGNATURE2: &str = "6d29fa208d1bf2bf83e9eef9c0b80b5c724a33548f11c38e6d621837f37152aa704538c7e55da35f3795c365b97cf100b73e8bf7b3c3fc0083697a8a315b9300";
const ID3: &str = "72a45d5815ed38ea4de90e890adb62c0826ed8194a72472b67171144c4e7da80";
const SIGNATURE3: &str = "d6b8b43a43cca4b99ffc3b7d40a5920ae0715eddd1051f6c0c50bf89dad37247d6420e7fc8e54feb8250bca5451d63b519c5365de8bd7669065cb800b80fe307";
const ID4: &str = "d88a5c0f85c8e7530f0387f75212e471aba4a9de289e76741d5baa82bb390006";
const SIGNATURE4: &str = "7959314eefac182cb4b4659806151c6c55629cf876fcaf22d7461c9122e9795303338b8cd22f1f1364affbfaa9ee529a547f3c5223ab13462eac7bc5de2ba002";
const ID5: &str = "4b20ce85c33d8851323356aad597d78e5d3ce761effc9811391256451184832d";
const SIGNATURE5: &str = "358c3de098cfc7ab0eb3d8a3aa510b551a7bbdaeb9243023b177b81f3d3bf3f37deb7b566e976fc5d79af78872a87bdcb6f2003529cd04c6861bef50849e5502";
const HEADER5: &str = "0718df46ce1036892230c7fda35e6a126a5d53292e3b108ff7a6d84281f5500a428139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394000001d88a5c0f85c8e7530f0387f75212e471aba4a9de289e76741d5baa82bb3900060080d8c1a28c340009d88a5c0f85c8e7530f0387f75212e471aba4a9de289e76741d5baa82bb39000600000000";
const ID6: &str = "ca408fc9b61fb62d84f1ae87947e573ee3d4756ecf1ac539a952af7e34d57ed3";
const SIGNATURE6: &str = "272016c565beb4067b6dd9ba597b1f2f36af8ed369ec6332bd737ae8cce38a4b1cb50ccb6852514ac04f0bcb9e2e8bd3dd9497933bcf3a5c235429f903cf1b00";
const ID7: &str = "09739b9c5aa9631e47ee5c0edd50c22b831a48ce9caaa0d693be3a351e0db24b";
const SIGNATURE7: &str = "c8d8885b05dd428d5af09ed21ed6b5c83b4475be4fccb09799d817566d09c7c6b305444b96ad3a88bed6f5297344c414eb0a2ec6cac876255cfcf352aafe6506";
const ID8: &str = "3a3dbf6f30b74ad09820e7c0f2545f446d42fd3d51de77b77d4198f4a92d3309";
const SIGNATURE8: &str = "ed0c20568aad62bde1c30ac1e6a8ff035028cfc4101521224c0af0d99a45e93a958f91863f5016c3931ee552f7b5d078d6ed3d839da49be89834848623223b0b";
const BODY_DIGEST: &str = "b59a8195644a97c668b11a0471d09dbe4819388a125225763992e7d505bf7494";

#[test]
fn signed_current_protocol_vectors_are_frozen() {
    let (events, effect, blob) = transcript();
    assert_eq!(events[0].header().goal.to_string(), GOAL);
    let Body::Genesis(genesis) = events[0].header().body else {
        unreachable!()
    };
    assert_eq!(genesis.definition.to_string(), DEFINITION);
    assert_eq!(blob.hash().to_string(), BLOB);
    assert_eq!(effect.id(events[0].header().goal).to_string(), EFFECT);
    let ids = [ID0, ID1, ID2, ID3, ID4, ID5, ID6, ID7, ID8];
    let signatures = [
        SIGNATURE0, SIGNATURE1, SIGNATURE2, SIGNATURE3, SIGNATURE4, SIGNATURE5, SIGNATURE6,
        SIGNATURE7, SIGNATURE8,
    ];
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event.id().to_string(), ids[index]);
        assert_eq!(event.signature().to_string(), signatures[index]);
        assert_eq!(Event::from_wire(&event.to_wire()).unwrap(), *event);
    }
    assert_eq!(hex(events[0].header_bytes()), HEADER0);
    assert_eq!(hex(events[5].header_bytes()), HEADER5);
}

#[test]
fn body_indices_and_bytes_are_current_contract() {
    let bodies = testkit::every_body();
    let names = [
        "publication_set",
        "publication_consent",
        "genesis",
        "member_admitted",
        "member_removed",
        "rules_bound",
        "workspace_epoch",
        "workspace_proposed",
        "task_revised",
        "task_opened",
        "work_offered",
        "attempt_started",
        "attempt_reported",
        "work_declined",
        "cancel_requested",
        "cancel_acknowledged",
        "contribution_published",
        "completion_declared",
        "review_recorded",
        "check_attested",
        "scope_decided",
        "document_revised",
        "effect_materialized",
        "delivery_acknowledged",
        "leave_requested",
    ];
    assert_eq!(bodies.len(), names.len());
    for (index, body) in bodies.iter().enumerate() {
        assert_eq!(body.kind(), names[index]);
        assert_eq!(crate::codec::encode(body).unwrap()[0], index as u8);
    }
    assert_eq!(
        crate::crypto::content_hash(&crate::codec::encode(&bodies).unwrap()).to_string(),
        BODY_DIGEST
    );
}

#[test]
fn exact_signatures_and_canonical_headers_are_required() {
    let (events, _, _) = transcript();
    let event = &events[5];
    let mut wire = event.to_wire();
    wire.signature.0[0] ^= 1;
    assert_eq!(Event::from_wire(&wire), Err(EventError::BadSignature));
    let mut wire = event.to_wire();
    wire.header[0] = 1;
    assert_eq!(
        Event::from_wire(&wire),
        Err(EventError::UnsupportedVersion(1))
    );
    let mut wire = event.to_wire();
    wire.header.push(0);
    assert_eq!(Event::from_wire(&wire), Err(EventError::Malformed));
    let mut wire = event.to_wire();
    assert_eq!(wire.header[65], 0);
    wire.header.splice(65..66, [0x80, 0]);
    assert_eq!(Event::from_wire(&wire), Err(EventError::NotCanonical));
}

#[test]
fn task_id_text_and_binary_are_distinct_and_round_trip() {
    for id in [
        TaskId::Authored(EventId([1; 32])),
        TaskId::Derived(EffectId([1; 32])),
    ] {
        assert_eq!(id.to_string().parse::<TaskId>().unwrap(), id);
        assert_eq!(
            serde_json::from_str::<TaskId>(&serde_json::to_string(&id).unwrap()).unwrap(),
            id
        );
        assert_eq!(
            crate::codec::decode::<TaskId>(&crate::codec::encode(&id).unwrap()).unwrap(),
            id
        );
    }
    assert!("01".repeat(32).parse::<TaskId>().is_err());
}

#[test]
fn effect_identity_ignores_witness_choice_but_not_target_or_round() {
    let (events, mut effect, _) = transcript();
    let goal = events[0].header().goal;
    let id = effect.id(goal);
    effect.evidence.push(EventId([9; 32]));
    assert_eq!(effect.id(goal), id);
    effect.target_slot.push('x');
    assert_ne!(effect.id(goal), id);
    effect.target_slot.pop();
    effect.context.round = EventId([8; 32]);
    assert_ne!(effect.id(goal), id);
}

#[test]
fn source_declarations_are_signed_but_not_causal_dependencies() {
    let (events, _, _) = transcript();
    let contribution = &events[6];
    let mut header = contribution.header().clone();
    let before = header.body.dependencies();
    let Body::ContributionPublished { sources, .. } = &mut header.body else {
        unreachable!()
    };
    sources.push(EventId([79; 32]));
    assert_eq!(header.body.dependencies(), before);
    let changed = Event::sign(header, &testkit::keypair(2)).unwrap();
    assert_ne!(changed.id(), contribution.id());
    assert_ne!(changed.signature(), contribution.signature());
    assert_eq!(Event::from_wire(&changed.to_wire()).unwrap(), changed);
}

#[test]
fn signed_event_boundary_rejects_too_many_contribution_artifacts() {
    use crate::limits::MAX_ARTIFACTS;
    let (events, _, _) = transcript();
    let contribution = &events[6];
    // At the limit the contribution still signs and round-trips.
    let mut at_limit = contribution.header().clone();
    let Body::ContributionPublished { artifacts, .. } = &mut at_limit.body else {
        unreachable!()
    };
    artifacts.clear();
    artifacts.extend((0..MAX_ARTIFACTS).map(|i| BlobHash([i as u8; 32])));
    let signed = Event::sign(at_limit, &testkit::keypair(2)).unwrap();
    assert_eq!(Event::from_wire(&signed.to_wire()).unwrap(), signed);
    // One over the limit is refused at the signed-event boundary, not only
    // by the local API that constructs contributions.
    let mut over = contribution.header().clone();
    let Body::ContributionPublished { artifacts, .. } = &mut over.body else {
        unreachable!()
    };
    artifacts.clear();
    artifacts.extend((0..=MAX_ARTIFACTS).map(|i| BlobHash([i as u8; 32])));
    assert_eq!(
        Event::sign(over, &testkit::keypair(2)),
        Err(EventError::BadReferences)
    );
    // A peer cannot smuggle one past by arriving with pre-signed bytes: the
    // wire decoder runs the same check before any signature verification.
    let mut wire = signed.to_wire();
    let mut header: Header = crate::codec::decode(&wire.header).unwrap();
    let Body::ContributionPublished { artifacts, .. } = &mut header.body else {
        unreachable!()
    };
    artifacts.push(BlobHash([MAX_ARTIFACTS as u8; 32]));
    wire.header = crate::codec::encode(&header).unwrap();
    assert_eq!(Event::from_wire(&wire), Err(EventError::BadReferences));
}
