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
const EFFECT: &str = "e468b0d8ccaa717e3e7730c3fdc552499199d7c599a2e55fb8991706b678eb2c";
const ID0: &str = "1d7c88c89792c5768e021d7a2e00c9b50201abe6b0ca5e3a3a35fc5690670689";
const SIGNATURE0: &str = "50867675d7bc16549e0c97364bda4e5dbbe2c361a4be1ee8c670d6e477b0095cb3a5392bf1ac20b28b443f452f32176b97c2417250aa959611cfbed86bad000e";
const HEADER0: &str = "02639f881b9103be44e44480ac83c28d8febd106db2bcbe63c432d782af0f01e7f8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0000000080d8c1a28c3400008a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c71cbf6d00b266da95e6e80c3c90674da7460c0729f4119ee2f8450f98398ca7300000000000000000000000000000000";
const ID1: &str = "9036fc0a4f61959f85ea9f5988897370b4fffed4f9fff09b54943bfa6faff934";
const SIGNATURE1: &str = "a14bcd9e9a578aa3ec55ed0277144472491b43a604879e262995f5fc63750797c1a0be19f207224ad10205eeb09e3785b4e03681620a94273d18e21ad8fbe300";
const ID2: &str = "d46413b2c890f43a8a505ab2d8f3f09f179e4ec3d043bbaa0dc5d40ee0ab098e";
const SIGNATURE2: &str = "b246e849871ac010f8dab5cbc118eca8c888b29a4e46abba2069f49a4fbbe5a1a8c2c922d52b9c1d86b30dcb06aa3f2433e66a7f96029fd8d9f2930db0579907";
const ID3: &str = "190b4e631a36757a4a8a326d6c9806406aaa728f6822cc083b6c0fe2f220e9a8";
const SIGNATURE3: &str = "5292c5d1e6f51b5db09ce1e053ee53130931bfc3a0766d6c910599244d8199365159bad760918ba490d4788b6b5f143f2d27dbd521594d64adfc88e30688a20e";
const ID4: &str = "15f5fd0eeef68b7c5dba5ce585f26412a1e3bd8749f2ff3da01a9bdefebef101";
const SIGNATURE4: &str = "9863bc933ee6d939400d68a38f18e1d190266db2d9e57454e7b61e2adea277bf39a8d0b9aca1531f2709ceb5f19c99d5d7dba5150059bc0ee80f30b2bde51d0f";
const ID5: &str = "ddeb5b101a751ca850a0dc3c93634323ebbf71632cdc114b16c41babc601da7f";
const SIGNATURE5: &str = "6faca6d52c0cfc42d062ecb4974432bf71c2958335345fdd7d3e1370a2cd2bb46a549c16f298fb2dd30300e3d86590c3558325565077ee711c8e50110ea6a805";
const HEADER5: &str = "02639f881b9103be44e44480ac83c28d8febd106db2bcbe63c432d782af0f01e7f8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b39400000115f5fd0eeef68b7c5dba5ce585f26412a1e3bd8749f2ff3da01a9bdefebef1010080d8c1a28c34000515f5fd0eeef68b7c5dba5ce585f26412a1e3bd8749f2ff3da01a9bdefebef10100000000";
const ID6: &str = "8b3eb7a00d5af5ef786a2155f16a15419c655a25d0da547601179625e07e47ec";
const SIGNATURE6: &str = "1e2800b6eeca03d4cdc0a7fb71e34014ab7145d23c817903e7ba5d8acae7244839aa2bb6223fedf4459be79e99661a9b49d726644b489365915952cf64c2800a";
const ID7: &str = "a2250ddf210fe82ca528babd22a853ec1e3600484cf3ab777f939aa2fa77adb4";
const SIGNATURE7: &str = "4b032d9db36203d55b19eb299d7afed306348028d1af05050213ab5d0f81339dfc0105e99545e6a7b0c6f5c48bb735055688862da723c2249d439003a6c8ad05";
const ID8: &str = "f2e6679c71288cce9011f11065e0cc07a9d596beb9500206d303e77c46615e70";
const SIGNATURE8: &str = "dcf65c3d8f18d2d6f6ee9a416169925f080ba1c93c3361f165e5155bb710f3c7fcbcd628e29d6cf618825794baca07d440cac53da16599146858cde345ca8f04";
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
