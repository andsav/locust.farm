//! Deterministic current-protocol keys/builders. Never use these keys outside tests.
use std::collections::BTreeMap;

use crate::PROTOCOL_VERSION;
use crate::crypto::{ContentKey, Keypair};
use crate::event::*;
use crate::id::{BlobHash, DefinitionHash, EndpointId, EventId, GoalId, PublicKey};
use crate::organization::{Blueprint, semantic_hash};
use crate::seal;
use crate::store::Blob;

pub fn keypair(n: u8) -> Keypair {
    Keypair::from_seed([n; 32])
}
pub fn content_key(n: u8) -> ContentKey {
    ContentKey([n; 32])
}
pub fn definition_hash(blueprint: &Blueprint) -> DefinitionHash {
    semantic_hash(blueprint)
        .parse()
        .expect("semantic hash is hex")
}

pub fn sealed_payload(goal: &GoalId, epoch: u32, text: &[u8]) -> (PayloadRef, Blob) {
    let sealed = seal::seal(goal, epoch, &content_key(1), text).expect("test payload seals");
    let blob = Blob::new(sealed);
    (
        PayloadRef {
            hash: blob.hash(),
            len: blob.bytes().len() as u32,
            key_epoch: epoch,
        },
        blob,
    )
}

pub fn rules_binding(
    goal: &GoalId,
    epoch: u32,
    blueprint: &Blueprint,
    roles: BTreeMap<String, Vec<PublicKey>>,
) -> (RulesBinding, Blob) {
    let source = serde_json::to_vec(blueprint).expect("definition JSON encodes");
    let (object, blob) = sealed_payload(goal, epoch, &source);
    (
        RulesBinding {
            definition: DefinitionRef {
                semantic: definition_hash(blueprint),
                object,
            },
            roles,
            inputs: BTreeMap::new(),
        },
        blob,
    )
}

pub struct Author {
    pub key: Keypair,
    next_seq: u64,
    prev: Option<EventId>,
}
impl Author {
    pub fn new(n: u8) -> Self {
        Self {
            key: keypair(n),
            next_seq: 0,
            prev: None,
        }
    }
    pub fn genesis(&mut self) -> Event {
        self.genesis_with(&Blueprint::default())
    }
    pub fn genesis_with(&mut self, definition: &Blueprint) -> Event {
        let genesis = Genesis {
            administrator: self.key.public(),
            definition: definition_hash(definition),
            salt: [0; 16],
        };
        self.event(genesis.goal_id(), None, Body::Genesis(genesis))
    }
    /// Initial governance prefix. Bind rules explicitly before authoring work.
    pub fn found_goal(&mut self, endpoint: EndpointId) -> (Event, Event) {
        let genesis = self.genesis();
        let admission = self.event(
            genesis.header().goal,
            Some(genesis.id()),
            Body::MemberAdmitted {
                member: self.key.public(),
                endpoint,
            },
        );
        (genesis, admission)
    }
    pub fn event(&mut self, goal: GoalId, anchor: Option<EventId>, body: Body) -> Event {
        self.event_with(goal, anchor, body, None)
    }
    pub fn event_with(
        &mut self,
        goal: GoalId,
        anchor: Option<EventId>,
        body: Body,
        payload: Option<PayloadRef>,
    ) -> Event {
        let header = Header {
            version: PROTOCOL_VERSION,
            goal,
            author: self.key.public(),
            seq: self.next_seq,
            prev: self.prev,
            anchor,
            parents: Vec::new(),
            at_ms: 1_790_000_000_000 + self.next_seq,
            payload,
            body,
        };
        let event = Event::sign(header, &self.key)
            .expect("testkit builds structurally valid current headers");
        self.next_seq += 1;
        self.prev = Some(event.id());
        event
    }
}

pub fn every_body() -> Vec<Body> {
    let id = EventId([1; 32]);
    let key = keypair(1).public();
    let hash = BlobHash([2; 32]);
    let context = Context {
        scope: Scope::Goal,
        round: id,
    };
    let task = TaskId::Authored(id);
    let task_binding = TaskBinding {
        rules: id,
        variation: None,
        inputs: BTreeMap::new(),
        parent: None,
        stage: None,
    };
    let binding = RulesBinding {
        definition: DefinitionRef {
            semantic: definition_hash(&Blueprint::default()),
            object: PayloadRef {
                hash,
                len: seal::OVERHEAD_BYTES as u32,
                key_epoch: 0,
            },
        },
        roles: BTreeMap::new(),
        inputs: BTreeMap::new(),
    };
    let effect = Effect {
        context,
        transition: "review".into(),
        trigger: Trigger::Contribution(id),
        target_slot: key.to_string(),
        action: EffectAction::RequestReview {
            context,
            subject: id,
            recipient: key,
        },
        evidence: vec![id],
    };
    vec![
        Body::Genesis(Genesis {
            administrator: key,
            definition: binding.definition.semantic,
            salt: [0; 16],
        }),
        Body::MemberAdmitted {
            member: key,
            endpoint: EndpointId([3; 32]),
        },
        Body::MemberRemoved {
            member: key,
            admission: id,
            last_accepted: Some(AuthorPoint { seq: 1, id }),
        },
        Body::RulesBound {
            expected: None,
            binding,
        },
        Body::TaskRevised {
            task,
            expected_round: id,
            binding: task_binding.clone(),
        },
        Body::TaskOpened {
            binding: task_binding,
        },
        Body::WorkOffered {
            context,
            recipient: key,
        },
        Body::AttemptStarted {
            context,
            offer: Some(id),
        },
        Body::AttemptReported {
            attempt: id,
            status: AttemptStatus::Progress,
        },
        Body::WorkDeclined { offer: id },
        Body::CancelRequested { attempt: id },
        Body::CancelAcknowledged {
            cancel: id,
            outcome: CancelOutcome::Stopped,
        },
        Body::ContributionPublished {
            context,
            attempt: None,
            base: Some(hash),
            patch: Some(hash),
            artifacts: vec![hash],
        },
        Body::CompletionDeclared {
            context,
            subject: id,
        },
        Body::ReviewRecorded {
            context,
            subject: id,
            verdict: ReviewVerdict::Approve,
        },
        Body::CheckAttested {
            context,
            subject: id,
            name: "unit".into(),
            passed: true,
        },
        Body::ScopeDecided {
            context,
            previous: None,
            action: DecisionAction::Select { subject: id },
            evidence: vec![id],
        },
        Body::DocumentRevised {
            context,
            doc: Doc::Plan,
            base: None,
        },
        Body::EffectMaterialized { effect },
        Body::DeliveryAcknowledged {
            effect: crate::id::EffectId([4; 32]),
        },
        Body::LeaveRequested { admission: id },
    ]
}
