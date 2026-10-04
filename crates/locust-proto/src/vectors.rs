//! Frozen protocol-3 encodings. Unsupported protocol versions have no reader.
use crate::event::*;
use crate::id::*;
use crate::organization::{Authority, Blueprint, CompletionRule, Selector};
use crate::testkit::{self, Author};
use std::collections::BTreeMap;
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn transcript() -> (Vec<Event>, Effect, crate::store::Blob) {
    let mut definition = Blueprint::default();
    definition.decisions.completion = CompletionRule::Reviews {
        by: Selector::Members,
        count: 1,
        exclude_author: true,
    };
    definition.decisions.selection = Some(Authority::Participant {
        key: testkit::keypair(1).public().to_string(),
    });
    let mut admin = Author::new(1);
    let mut worker = Author::new(2);
    let mut reviewer = Author::new(3);
    let genesis = admin.genesis_with(&definition);
    let goal = genesis.header().goal;
    let mut anchor = genesis.id();
    let mut events = vec![genesis];
    for key in [
        admin.key.public(),
        worker.key.public(),
        reviewer.key.public(),
    ] {
        let event = admin.event(
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
    let bound = admin.event(
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
            base: None,
            patch: None,
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
    let select = admin.event(
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
const GOAL: &str = "639f881b9103be44e44480ac83c28d8febd106db2bcbe63c432d782af0f01e7f";
const DEFINITION: &str = "71cbf6d00b266da95e6e80c3c90674da7460c0729f4119ee2f8450f98398ca73";
const BLOB: &str = "ef4de45fd29a350be4462273a0026c0250ad04da2a4fa084ba45657d6eddd917";
const EFFECT: &str = "9327ad0fdbcb38d8271a10ab9221376c180e8ad23dd85a202f23e3bd21dd2fca";
const ID0: &str = "7cdab266af29cc3d6df5a2076022651afa140624e3fc014130a2dd8a87d01397";
const SIGNATURE0: &str = "01f8394498eaeab26a111f2f181d14fc860cf844c5a79b8e8463b5ad34047074a5beafbfde597aaffbb864cb6291c14a01987f1a1e95553d9e5b6d3ae458900a";
const HEADER0: &str = "03639f881b9103be44e44480ac83c28d8febd106db2bcbe63c432d782af0f01e7f8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0000000080d8c1a28c3400008a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c71cbf6d00b266da95e6e80c3c90674da7460c0729f4119ee2f8450f98398ca7300000000000000000000000000000000";
const ID1: &str = "3456dd607c7168227a763f3d410bf7a2b5f925fb43039510d5e9db9afd53bea7";
const SIGNATURE1: &str = "3c3fc56b70b80033c88d6aabffce9a46cd4f9aba42931e18c389386cabcd61fd1f47c7a847217418be7dc7954a6cb1ef6c1566e87ff2bd5827ff5106e5dda30c";
const ID2: &str = "3e2724d199b3bed78ad9239769732ad929b30bddce542cc93b92fb22afb1fbd4";
const SIGNATURE2: &str = "4265884e586a4a18ab96389e1ea38b95757e4af7c84d0f21efd389612ce9912d602db37c4b72a14ba3e43880ca65cc0e6fea019219f2fc0a41c691fc3dacbd0d";
const ID3: &str = "7e1f9e38f5de0403b16abc5e6997231b15fb518f46f4982d90ff28084a31e4a1";
const SIGNATURE3: &str = "ca53a6cdf75c1bc4859570823d2b7a0aebc8e28749b1bc25f660f796b4920f6040c960cb5a9aa0182653691c47fa461e7af37e9575bbc4e477632c5366c96c00";
const ID4: &str = "baa9aaab3844a27e0bcafa5d8158fc9a86cca361ccce2236c0651dd1d9894326";
const SIGNATURE4: &str = "93f2f8de706da804c4d7d22c49cc2f76fa2368674bc6bb8b7736b31e549edbb4f562aa7462b9f8fe18147b11f4706e5f3e656aea9cb9dc5dfab2541d3988270c";
const ID5: &str = "dcc438646acd32fd82a16c4627b61c2b6e99f6fa0f7b4088e83cfac136cf9bc0";
const SIGNATURE5: &str = "d4789df38f47666e35507d961f13afa2e166bd8fb6d9b30fa401e442fb900c16800e22c51005c22808a8af2fee5e92efe162508d157045f241618d9a4e346105";
const HEADER5: &str = "03639f881b9103be44e44480ac83c28d8febd106db2bcbe63c432d782af0f01e7f8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394000001baa9aaab3844a27e0bcafa5d8158fc9a86cca361ccce2236c0651dd1d98943260080d8c1a28c340005baa9aaab3844a27e0bcafa5d8158fc9a86cca361ccce2236c0651dd1d989432600000000";
const ID6: &str = "f9e1d785c78183b3737e32a160df3e3c683b264a4fae13101e109222167c4975";
const SIGNATURE6: &str = "723c4e3dfc6917ddc0e7c5b04541da9c2ecd0fc8250afc493a528be4cde8a50b47fbd145f5ade108dab9f9566e9f3b5e4012c911a08f255fde49d3750c80cb09";
const ID7: &str = "c8ef93711475d7df6a30752c11281e5f403ff9dfd4037eb527adc0ed57f63c8f";
const SIGNATURE7: &str = "ea7044f83b7223d7c487a330f1771f4829964713131b1a679f2ba3abfa189ee871dd8ac3294913e2bbe2b9364e653bc9d4fa08ff6d48db5e6cb72bfb042eb702";
const ID8: &str = "eb5aa4e49047ecac778d31ee8adaf26cd162b5465c07d8f492889726b1d40890";
const SIGNATURE8: &str = "2dee63be7e446bb622762c3f234337382d2c73eca3fce3e5826f2b8daf40232b6de9f7d8f55c5710fcb852726e9f7a30b1bc767b2041251ab6bf465621469305";
const BODY_DIGEST: &str = "109c24784a98c14ebb7b8c0def9cde95a22a425b1ff480ef4ae3805c5cb24c5c";

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
        "genesis",
        "member_admitted",
        "member_removed",
        "rules_bound",
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
