//! Frozen protocol-2 encodings. No executable protocol-1 vectors remain.
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
                variation: None,
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
const BLOB: &str = "cf6a83d7cf3fcb4678fd78de20b5434bc68a5302e16df0a61283cbf4967da9c7";
const EFFECT: &str = "cb2fc8cf3cc75bb4dd6234b682fe34a06b2958249b720c44b513fb6cb6436cc6";
const ID0: &str = "1d7c88c89792c5768e021d7a2e00c9b50201abe6b0ca5e3a3a35fc5690670689";
const SIGNATURE0: &str = "50867675d7bc16549e0c97364bda4e5dbbe2c361a4be1ee8c670d6e477b0095cb3a5392bf1ac20b28b443f452f32176b97c2417250aa959611cfbed86bad000e";
const HEADER0: &str = "02639f881b9103be44e44480ac83c28d8febd106db2bcbe63c432d782af0f01e7f8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0000000080d8c1a28c3400008a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c71cbf6d00b266da95e6e80c3c90674da7460c0729f4119ee2f8450f98398ca7300000000000000000000000000000000";
const ID1: &str = "9036fc0a4f61959f85ea9f5988897370b4fffed4f9fff09b54943bfa6faff934";
const SIGNATURE1: &str = "a14bcd9e9a578aa3ec55ed0277144472491b43a604879e262995f5fc63750797c1a0be19f207224ad10205eeb09e3785b4e03681620a94273d18e21ad8fbe300";
const ID2: &str = "d46413b2c890f43a8a505ab2d8f3f09f179e4ec3d043bbaa0dc5d40ee0ab098e";
const SIGNATURE2: &str = "b246e849871ac010f8dab5cbc118eca8c888b29a4e46abba2069f49a4fbbe5a1a8c2c922d52b9c1d86b30dcb06aa3f2433e66a7f96029fd8d9f2930db0579907";
const ID3: &str = "190b4e631a36757a4a8a326d6c9806406aaa728f6822cc083b6c0fe2f220e9a8";
const SIGNATURE3: &str = "5292c5d1e6f51b5db09ce1e053ee53130931bfc3a0766d6c910599244d8199365159bad760918ba490d4788b6b5f143f2d27dbd521594d64adfc88e30688a20e";
const ID4: &str = "f207213d04d31958e5f0e0582369770103b81d473a834157bbb7e10ccfadccb8";
const SIGNATURE4: &str = "0e2fdecd892e8c32e6c1d371fa37e662ee80cf0f60c55c38e2522a4f8213dfba4ddab304706d71bd3e2a0fd2907727ee43535933eeb627856132190cbde7bb00";
const ID5: &str = "0a78423ab8b109d39ff046a45f25c5a06eb6ddd91da494705458437042fa6f60";
const SIGNATURE5: &str = "0f8db78dddebd6e7b1c00a7d7be77326c4ef453ab527f784a511b962ef8ce5c9e813f0b071282561fee05d52bd34057fceee3706357a89a6f933e598b228ee0d";
const HEADER5: &str = "02639f881b9103be44e44480ac83c28d8febd106db2bcbe63c432d782af0f01e7f8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394000001f207213d04d31958e5f0e0582369770103b81d473a834157bbb7e10ccfadccb80080d8c1a28c340005f207213d04d31958e5f0e0582369770103b81d473a834157bbb7e10ccfadccb800000000";
const ID6: &str = "6674407c45d00a829ff3131e8afe86d45e40c7dc7e454ec6a400e7ac83d122e1";
const SIGNATURE6: &str = "2faad3f0a154a9b5013c2b895553a0cf64ae93ca1413e626c169ec51bf1ff27d04e50fd09f76541cf1066d44634e310acbcacce7a6012f44ec584a4d66c87603";
const ID7: &str = "e9b2cec3cdae2208186b05e49a047c0dc82ec9cb24088cd38c95a25bf362852d";
const SIGNATURE7: &str = "b176719116b59266797dd44461b427bf1867d6d9b33d38da1ae8ee0b830053677f89ad4f0380d4df5532ae34e2920e742523f5472df3e44d9ef80bfcdaa5b509";
const ID8: &str = "2442e60220090aad320c162e4ed6fca6dcbdd66dee073578d3b2cf852d14b458";
const SIGNATURE8: &str = "04ed6fa19ecd3b1425f4d6142734666e4d2b26a205b13c0d978c8f80deed2d550b52c10bb841033db0a84d301d6ac045be96ce49b533870a0cce7e9a4bbabb0d";
const BODY_DIGEST: &str = "92461f69f4f7b32205f92ad39c43b0a222c5ef4a72a14486ee22037aae86dc76";

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
