//! Frozen protocol-6 encodings. Unsupported protocol versions have no reader.
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
const GOAL: &str = "a3a97225c4b1e8eff8c2d73eb352f5909d23d2c34fee3e7de88a025215e3b150";
const DEFINITION: &str = "0e6c742b1744ebdda81b1b4b2c163af2c4bd95d6de135608029627138f840fcd";
const BLOB: &str = "8c65e631c90bad8090d18fa266ac2b1ead0fb1b7131b761302f8c140b0845237";
const EFFECT: &str = "d07983538fa8289f7ed2a7b74b83a9044f0a1572cbcfbb819a4e8b3b08a3df4e";
const ID0: &str = "b5389fd0a6861e30f6fb052111b4514744d112eee9e5b587d781242f8b08eddf";
const SIGNATURE0: &str = "6ec748205a6878bcc432ee1e98bf0f6c7b48b8706c7e2c3d6b15f81dfae73877f8c42e81e85f76babac212d9787bf65740e2e0ac1f38a4f990dfe6dc4e5b250e";
const HEADER0: &str = "06a3a97225c4b1e8eff8c2d73eb352f5909d23d2c34fee3e7de88a025215e3b1508a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0000000080d8c1a28c3400028a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0e6c742b1744ebdda81b1b4b2c163af2c4bd95d6de135608029627138f840fcd00000000000000000000000000000000";
const ID1: &str = "093563a6d20722b1baea21750699536fc76f272d9b45f3aad1a84abee12ebff6";
const SIGNATURE1: &str = "b45e994b238545b3f03a88e92869565bb1a324846953af9b32028f8894e2da65af85eb11ffda768d7bc021fd2cfbde6c2f85c8092cf51b83ed4f93ded3ca0a0e";
const ID2: &str = "831c37b8c07924f8d8df16c53c6224a3217421b0311a19a37fd244da3f89da49";
const SIGNATURE2: &str = "271683071c8fcc462e33839342d4399f3d3803a50c7d77192e763380ed0628d439abd70fe2d16abce738047be6610d1c62ece2f3a699a915a3ad0cbb962c8904";
const ID3: &str = "bca23038e2c71498faf6a2d797c6adc7791a3251e44c42e2320f4c34e2d86c5c";
const SIGNATURE3: &str = "d03ff39bc85cc027d472650d894d63a2e8225f9d741f5f04861d64ea2c0a6c33144046f84f023ee291c70016324d3b4ed491c0835222ae1ab2c9b823b9e08f08";
const ID4: &str = "10cf50475ef67ec08ff5445a9283a480934bb1cb07352a950e63988d56b04c90";
const SIGNATURE4: &str = "93347f4561c239927e4377d36574e4323973286ba0b1cb813d78e13cc167571d731f1e38f579a9c1b24e82ac9017d6fa3b267fa0b42b7eaca3650be9f0d5c908";
const ID5: &str = "b4ec7c333c51faf0f3d91aa7f7509bf77a4feb0e9bd0b1b217bc642f2efc38f3";
const SIGNATURE5: &str = "3ea51ee20510954ab833393e336c13838bdc8f346308c118833f9fde3917278237c1abdf4feecfced57798dbcc45280a660de2cf05e094de55b4836ee50abf02";
const HEADER5: &str = "06a3a97225c4b1e8eff8c2d73eb352f5909d23d2c34fee3e7de88a025215e3b1508139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b39400000110cf50475ef67ec08ff5445a9283a480934bb1cb07352a950e63988d56b04c900080d8c1a28c34000910cf50475ef67ec08ff5445a9283a480934bb1cb07352a950e63988d56b04c9000000000";
const ID6: &str = "abb2fb15d6e3c8232e70811181d6f6314302cc53c1226599075b8d8d5c0ebdea";
const SIGNATURE6: &str = "b1174edfc4c26fb0d55f5a0121d86f1ab2f261f0dfca85f7a5d4e0e32d8589ec60b3fea9ba3125cff9c3d89babcbe3fbd4d1c7c4ff02a98f23cd1cc321f3a706";
const ID7: &str = "58a072e25e1ffc5da3c9f2052118dd2d68c778534380cf59bc1e5c0f13f8a449";
const SIGNATURE7: &str = "c0a09bfbb22b4e45187594fa4cf2b3ff55afa21d1e51c496fe70dad7c3fc9ced0ca5927c67083607008512c5e206a1992f74788065b5a3044553205178634b00";
const ID8: &str = "6e80f1b3e5edc231f0df9d8c72910007898ca73451d5544d53135886d878f471";
const SIGNATURE8: &str = "b612c42b9a9bb0c9e51d6a5b57d3407b52be61cd73645a3c2fffcc3f88d299e5acf1ba99554ac4b37ce1f09033af0746bace38b21a5047282a0434b1ae8fb006";
const BODY_DIGEST: &str = "b827b3c10a894e389b88c9c8325100ae60e2d65f994a0b33c605e6a7cc429da3";

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
