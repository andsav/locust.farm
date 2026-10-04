//! Frozen protocol-4 encodings. Unsupported protocol versions have no reader.
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
const BLOB: &str = "cf6a83d7cf3fcb4678fd78de20b5434bc68a5302e16df0a61283cbf4967da9c7";
const EFFECT: &str = "78a0016b2d7ce9a414d4a5e8c2f9ab117d4785ff24756af506c2da798025daee";
const ID0: &str = "9f07c694ca0fe466108a28d2daa2002cc26c3ef44e92e6f8097033cedcb98bb6";
const SIGNATURE0: &str = "f1e4fdf8315ba6afa11ce34e3ee975c2dcf9244fc5338438203bf1554e67ab9e451423c8f538a98eb7dba2d10f8ceb564c58a765effd528b4244b7adc263ea03";
const HEADER0: &str = "04639f881b9103be44e44480ac83c28d8febd106db2bcbe63c432d782af0f01e7f8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0000000080d8c1a28c3400008a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c71cbf6d00b266da95e6e80c3c90674da7460c0729f4119ee2f8450f98398ca7300000000000000000000000000000000";
const ID1: &str = "d16b9cf4cd3400f86b9dd120e096df6b49434000140bc94db891ee42df128f55";
const SIGNATURE1: &str = "186a1e34526cbcefdf0608fefa6912d7bc133adef04445ecd7613c23195032bfe9faa7d9525405176d42c1ceb0adb44ebeab5abf8d506b71eec288cb9ffba608";
const ID2: &str = "ab1ee7e46280ed95bd371bf7f1a0af41fb931a424cb9aac940febc3a03d9a6a9";
const SIGNATURE2: &str = "5f27608bcc61141441f0f006ee3aaa2ed295475574d410998982ca6d499b233391ab15dbd34acdcd57b7aa2b821d3ec06d5aa8f3c913d04f1a414d54a0ef4605";
const ID3: &str = "792b2268ccb494d1613455a8ad19e15d801323426c2a5a04cb43d6ac58c0424e";
const SIGNATURE3: &str = "8c17e60cb04e21a793e1e3b5071a32847cd82135bb8536c706c27bad67d0e8013eac4d7fbc30a0629738db36fd91e8220012e6e95293c2f393a1e41f0c429f08";
const ID4: &str = "f9c5170a155d3e10fc016d59c1c3e743be2a9e568e4e46eaaaf09c7e41569cf5";
const SIGNATURE4: &str = "184abf7dccf874a2d0a4d3476116ed939536f3aeb7432c56d425f4de5017a20761d75d8b4a41d8238c5fb04c166345b883a49c6a72f98b3c135b687ba1f6e708";
const ID5: &str = "50f59fd2f0c32ed5a2f3d31e6db11ff61fb8ab9aa22f8331cf03ab71cd0d0297";
const SIGNATURE5: &str = "9b82bcece26fcd31bb0483fbb39ae6eb02e70eaee2bfdce5b888018dc20b3b9768a480b0ba46ace1529755a2c53b3fb9513cd3f3d9dc3112089025ff59480c0a";
const HEADER5: &str = "04639f881b9103be44e44480ac83c28d8febd106db2bcbe63c432d782af0f01e7f8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394000001f9c5170a155d3e10fc016d59c1c3e743be2a9e568e4e46eaaaf09c7e41569cf50080d8c1a28c340005f9c5170a155d3e10fc016d59c1c3e743be2a9e568e4e46eaaaf09c7e41569cf500000000";
const ID6: &str = "4908cac603d0b94604088e67b3c1f5b37ab70641e1ad5452511fd324d46c6234";
const SIGNATURE6: &str = "9db280d33a2f0b4b71b5d00b4a63530b2ace4417c195d9cf32f6da3cb7519393a10edd07002fc0ef6dc8b817666b3259db639585bad913904672d5ef66623404";
const ID7: &str = "be731ea7ec3b3576c483539dfae0629cf444ff61009df8db647b7c2f80e87feb";
const SIGNATURE7: &str = "e87fa51a4398e217d457ec897ca2e9043734617a71d9662e4fafba35559161974d6aae72c8bed959a6e1ac9367e95c49eb01e7c2960023e9d0e40b0f78baf501";
const ID8: &str = "c2a7f63b6c582ec3350d6fa7e61be5b2ccd7325d65b639fb5e410cf565c772ab";
const SIGNATURE8: &str = "95ac68e6897bd4060b32c304a66ebed991654d3a6c0a3ae99b03d5691ba646319d1681ca6d9c53b378cf9bb991623d42e0e5504f6e2f8f08cb28bee6ef8aac0d";
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
