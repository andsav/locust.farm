//! Frozen protocol-5 encodings. Unsupported protocol versions have no reader.
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
const EFFECT: &str = "640c7744fc62e8b468c8306c0280ce3b2ce5248d6cb6245645f4aa393699fc25";
const ID0: &str = "55e68266bbe2e44bfa76b69d088afe4e85ca4f715ff02785579ff587d3b977d4";
const SIGNATURE0: &str = "cdcdb9c3c91880c5ed3620d0dda3646bc5c03e83a38a053f412354b4bb58491f45c70895252d58e55edadb03335c0a1e413eab85ab665e994d2865735bf9810e";
const HEADER0: &str = "05639f881b9103be44e44480ac83c28d8febd106db2bcbe63c432d782af0f01e7f8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0000000080d8c1a28c3400028a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c71cbf6d00b266da95e6e80c3c90674da7460c0729f4119ee2f8450f98398ca7300000000000000000000000000000000";
const ID1: &str = "8f8a84bdbb403119b88694a2935ce4989e6f2d96bfa381bbbbe7cc8b72750e69";
const SIGNATURE1: &str = "195852d7d59f51f2d983d37568f9f3a26078ed3a55cb04dec4a7fbd26daf67e4d7e67a714d3859ad1d2d17916f0ce766130634dac191116e1d251d0aa3112803";
const ID2: &str = "7a405f1c68085c9a6a4b0af0ffc3ec854b3e16e59e805a7d776e8ff3b4c24c65";
const SIGNATURE2: &str = "e6a328d3ab35ae69609f27f00d0397c339c8aa15a1726abf5dee5ef0d26cba48aee1a9604e505d3d6c29327c61dd537f61e62718b9edb2aed0b0e72c29772e05";
const ID3: &str = "10b3b638f8befbb4d3ccf95f82d35c9b8ec1da78d9fac8e75e1d15978920b554";
const SIGNATURE3: &str = "696f3e49dd8fd17a2ef6033d4dbc834e196042d988183aa9a325e4e334261b2d70a9748184a032504a3ac3d5f990c7fde71efad77565054f47f760b72f59c10e";
const ID4: &str = "8e846a59c90c83de7ecdcc40aa2c7a4b675be645fbf80a8d5f10c3afbc3b70cb";
const SIGNATURE4: &str = "9120b1a50b2d2efdc3ce6c88be88ae55e93e25eb5dc616f159401845a40b7181aab537a36e4f3607911c9e42f3f3d5e5b442c45a6bcd9af52e9c4487b8a3240b";
const ID5: &str = "81356a9a5ce41997fd682edfc0f13353700c009289d10686c17b23d386930711";
const SIGNATURE5: &str = "3fa4cd78fab02e51566aae01ca9491ac0cfceac1a7c3f4b7549458988ef6f3268100009df8fe7e62f0d7fad9e1a2faf9ce23ecb353d04059d71cdf019f8e0704";
const HEADER5: &str = "05639f881b9103be44e44480ac83c28d8febd106db2bcbe63c432d782af0f01e7f8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b3940000018e846a59c90c83de7ecdcc40aa2c7a4b675be645fbf80a8d5f10c3afbc3b70cb0080d8c1a28c3400078e846a59c90c83de7ecdcc40aa2c7a4b675be645fbf80a8d5f10c3afbc3b70cb00000000";
const ID6: &str = "19bb4ce391555cb65d01d3b8e907ea8b88306df8aa9c023d0f639940807c13eb";
const SIGNATURE6: &str = "7bce611a0c14000f431565509ddb2009ded7c521861e34e9506cf7a2a2baf3ed3e22c9b001260340ce13513b5402c18cfe22f0a16e605e9e5a36364bd2327e0f";
const ID7: &str = "2939c07c52f4b391e82cdfbe71c588a08eac8120dcd30b8c2761e5c8511279d9";
const SIGNATURE7: &str = "9ee33bebd0019b4d17febf44b46e76b9b397ac4ffe0a3670b4814cffa02e985921edcfe86cd6facd122ce29ddfc2c9384fe41e644df274cbeb796a7f65f5ec0b";
const ID8: &str = "250b6a42c00fa4a8188e82a55edd92974708f877f9e95603e7a4d5c2b28f5242";
const SIGNATURE8: &str = "80cd9f671bd65340ae0a317dae3590ddb4ec2aab09bc1b41cdae0c23fbe202e47821f7261ad16f32e01cdb8783e5509eb4071cbea6bf628bec260ecfd132750e";
const BODY_DIGEST: &str = "41df82389151669888d8c594455605305381aa9e88779606d3179db14310ffa3";

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
