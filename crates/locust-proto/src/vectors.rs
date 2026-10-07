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
                name: "member".into(),
                role: None,
                member: key,
                endpoint: EndpointId(key.0),
            },
        );
        anchor = event.id();
        events.push(event);
    }
    let (binding, blob) = testkit::rules_binding(&goal, 0, &definition);
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
const EFFECT: &str = "6dd4435871365af8bf420633cd26437eaac9d22f5c42298c46c7750b1ca4a36c";
const ID0: &str = "b0048b3e7a2b5c07935317e61b4f4ecab84069a8b14a36786c8d8cf4f1ea0f0e";
const SIGNATURE0: &str = "e6ec5dacdc02f17a7be1990956a02818d6d5ba35c502bb43b896b3b98cc512a88993cb0ffdd047c8cd47d52f59637c7f3dcbd0e4b8391b552a4410747269db0e";
const HEADER0: &str = "0718df46ce1036892230c7fda35e6a126a5d53292e3b108ff7a6d84281f5500a428a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c0000000080d8c1a28c3400028a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5cca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c3ae37e5bb845dca7c3b9d7857b6ad41b4d8fb6b926650b11ffc4996bcdac56d200000000000000000000000000000000";
const ID1: &str = "259c5fd64821741ebf073b5d2bc5bbf02685d9bb3bdc9eeebdf13f013ceacd67";
const SIGNATURE1: &str = "e28240cfa4ff719f2cf143575e7fa42605f4194dc1292d839ea0b0e5d9b4b070ad1bc75bfae103cf6d7d7a66ca11fdd72f48151f82faa60e08e7f3a14c98180c";
const ID2: &str = "672a8f3a252d3697c757079fa0a0a85e67b87ca80bdfe2f883b3ded94f9d9c63";
const SIGNATURE2: &str = "229c8183084a41907b48e8f766d9769cb58ee33fc8df2b66476bb07f71788e906513f3baa6514c07f06a22f26c1b410a7767af9a405cf2b0239b0b01dc96c107";
const ID3: &str = "1dfb152d373a85a66b4b4127a5254d8366cc23c6b35bcb95afc50855b97bde1d";
const SIGNATURE3: &str = "c46216b58572c3db5fdd2b18db42d796cbbaae4ffc552203e1c81d4468a2835b886a9682634b6a0cb456b4a0a4fc7b6fb2d0c82632075ed8ebef24ff931ed001";
const ID4: &str = "c169c4bd2777e03b134d7203fb073caa358524cf6d1c12c5618838f68da97ea8";
const SIGNATURE4: &str = "94a51c0c2651388cda9202ae0b77a24af516e83f591000a48e76571cab5f988c83a1acf6d18d3f6cddfcd2c71f5e784eae4c1080fa856c91b2779a8bd6984f0a";
const ID5: &str = "a91be5d1feed993d2daf7166664ea4fa0367ba9b265321fab3fac5249f438941";
const SIGNATURE5: &str = "881b4c0a80b35e60ef4e414858e5aa0b038063e68f1cf1da766afb2df0849bb96ed438692f4bdf9065f1e35a321f1674d892d761dcc16273d1b1d0a0e8a7da01";
const HEADER5: &str = "0718df46ce1036892230c7fda35e6a126a5d53292e3b108ff7a6d84281f5500a428139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394000001c169c4bd2777e03b134d7203fb073caa358524cf6d1c12c5618838f68da97ea80080d8c1a28c340009c169c4bd2777e03b134d7203fb073caa358524cf6d1c12c5618838f68da97ea800000000";
const ID6: &str = "8843d40d6d272e67c6962ae56c77356b2461f552cbf1856f68629c4498c3980d";
const SIGNATURE6: &str = "665eec98d09c3b705c9f7b473a70b1b2a9a1576ca839e8c7c695237714712f7b6c901dc984ab3eedcec830abfd473d074799114b41ba2dd86a39ca3efbf77d09";
const ID7: &str = "24c7e300e24ced64a742f0ccbe5cd301f29633d41af5170f995b8138bed72d03";
const SIGNATURE7: &str = "f9a9503cefcb0df6f8bb003f26609bcdd55b5ae7b240bba9a4f3b14bf7cc44bcafd40a7c0c42dcf20e40556d5b533be9f03e7fd257ae73187e38c1e2edfa3305";
const ID8: &str = "19da8201de72c48e2e897e283cd402cdf957ebe25687705b425b183b23ead4ef";
const SIGNATURE8: &str = "d7567a9d017e253782b8422ab4e1a4f57e8cf7caf62a0f11c44940ed533736675af637f23a5e6309ef2729fe1b6466eb6ba483a6c3aa64f94cffb764bd1db10a";
const BODY_DIGEST: &str = "120ac5fc1e75cf9d5e09232176de2cec509199acde9dda906c882c361d612b6e";

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
        "role_holders",
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

#[test]
fn an_admission_with_a_bad_name_is_not_an_event() {
    let (events, _, _) = transcript();
    for name in [
        String::new(),
        "x".repeat(65),
        " member".into(),
        "member ".into(),
        "mem\nber".into(),
        "member\u{7f}".into(),
    ] {
        let mut header = events[1].header().clone();
        let Body::MemberAdmitted { name: value, .. } = &mut header.body else {
            panic!("admission")
        };
        *value = name;
        assert_eq!(
            Event::sign(header.clone(), &testkit::keypair(1)),
            Err(EventError::BadName)
        );
        assert_eq!(
            Event::decode(
                &crate::codec::encode(&header).unwrap(),
                events[1].signature()
            ),
            Err(EventError::BadName)
        );
    }
    for role in ["", "  ", "a\nb"] {
        let mut header = events[1].header().clone();
        let Body::MemberAdmitted { role: value, .. } = &mut header.body else {
            panic!("admission")
        };
        *value = Some(role.into());
        assert_eq!(
            Event::sign(header.clone(), &testkit::keypair(1)),
            Err(EventError::BadName)
        );
        assert_eq!(
            Event::decode(
                &crate::codec::encode(&header).unwrap(),
                events[1].signature()
            ),
            Err(EventError::BadName)
        );
    }
    assert!(is_member_name("Juniper ; North"));
    assert!(is_member_name(&"é".repeat(32)));
    assert!(!is_member_name(&"é".repeat(33)));
}
