//! Frozen protocol-4 encodings. Unsupported protocol versions have no reader.
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
const EFFECT: &str = "65a1680d5aafb9daceb345765320110874f9dd237cddd0cd0ecf9c185d381d84";
const ID0: &str = "9f07c694ca0fe466108a28d2daa2002cc26c3ef44e92e6f8097033cedcb98bb6";
const SIGNATURE0: &str = "f1e4fdf8315ba6afa11ce34e3ee975c2dcf9244fc5338438203bf1554e67ab9e451423c8f538a98eb7dba2d10f8ceb564c58a765effd528b4244b7adc263ea03";
const HEADER0: &str = "04639f881b9103be44e44480ac83c28d8febd106db2bcbe63c432d782af0f01e7f8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0000000080d8c1a28c3400008a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c71cbf6d00b266da95e6e80c3c90674da7460c0729f4119ee2f8450f98398ca7300000000000000000000000000000000";
const ID1: &str = "d16b9cf4cd3400f86b9dd120e096df6b49434000140bc94db891ee42df128f55";
const SIGNATURE1: &str = "186a1e34526cbcefdf0608fefa6912d7bc133adef04445ecd7613c23195032bfe9faa7d9525405176d42c1ceb0adb44ebeab5abf8d506b71eec288cb9ffba608";
const ID2: &str = "ab1ee7e46280ed95bd371bf7f1a0af41fb931a424cb9aac940febc3a03d9a6a9";
const SIGNATURE2: &str = "5f27608bcc61141441f0f006ee3aaa2ed295475574d410998982ca6d499b233391ab15dbd34acdcd57b7aa2b821d3ec06d5aa8f3c913d04f1a414d54a0ef4605";
const ID3: &str = "792b2268ccb494d1613455a8ad19e15d801323426c2a5a04cb43d6ac58c0424e";
const SIGNATURE3: &str = "8c17e60cb04e21a793e1e3b5071a32847cd82135bb8536c706c27bad67d0e8013eac4d7fbc30a0629738db36fd91e8220012e6e95293c2f393a1e41f0c429f08";
const ID4: &str = "8b70d6f24940730512839d67afe7f74b96d714e67eeeba780ee5e9a2a5526d22";
const SIGNATURE4: &str = "02f1fa753846db55c9231382352fd3265d66c7f936706875b92df327ac61de9e53694933afa0fca8bdd12378862fc49a1e1daad8b410a8f648ddf0be0da9800c";
const ID5: &str = "b4419b00ba9a0f57833acac79f0a02c8cc25eff89932d31b8dfc0b0250c8a719";
const SIGNATURE5: &str = "0b01df96f5e2f21b0eed097674aa167a03c5c24434f99b26c3acb22d794a5e4243bbbb74104ea73c8d0adea212e6ca08acf9428b81b809f05b5ea5d2267ce900";
const HEADER5: &str = "04639f881b9103be44e44480ac83c28d8febd106db2bcbe63c432d782af0f01e7f8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b3940000018b70d6f24940730512839d67afe7f74b96d714e67eeeba780ee5e9a2a5526d220080d8c1a28c3400058b70d6f24940730512839d67afe7f74b96d714e67eeeba780ee5e9a2a5526d2200000000";
const ID6: &str = "5fc84c102edbcc1f2887e35f4eb7c6af7556b7a95c13c36d827f6326f23c4796";
const SIGNATURE6: &str = "eae5f97d6e9f4e6d5f73c537b9296950a8a750f34c52b62ac0660e913d629ea0501e079996bacd1571a34a52b74a41eb91259326e3cc38433cedf7ac7f7c460a";
const ID7: &str = "b2cb8c47e596cfe3649851a8809e66ab60720fa991492d56fca91d63c0e38e33";
const SIGNATURE7: &str = "12a6d603b511fa7daccabc4701f81b0ca449c2ca95898b1e4ba2762225f497ff2046612c60f7a032688dc3d92d613e4dbf1da95ecc33a62d16b541654eacda05";
const ID8: &str = "4029f1bbda462cced076a7aad1b7d06cc3ea2b36d8d423299977deb1a2af233b";
const SIGNATURE8: &str = "07cbb9b6e5bdad6ca45a5288ef2217eb3119b82b9162c9acebeb2289f25f36d200d4d95dbf39560b5e89e9b152b7dde3767d14b11edfee0ca82210b7e85a640c";
const BODY_DIGEST: &str = "d98a222c1baef81ede5b4ee6685b9b9a3b83b4a3262b87d0fc075ec50e33d972";

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
