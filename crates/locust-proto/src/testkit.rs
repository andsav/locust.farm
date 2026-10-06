//! Deterministic current-protocol keys/builders. Never use these keys outside tests.
use std::collections::BTreeMap;

use crate::PROTOCOL_VERSION;
use crate::crypto::{ContentKey, Keypair};
use crate::event::*;
use crate::id::{BlobHash, DefinitionHash, EndpointId, EventId, GoalId, PublicKey};
use crate::organization::{Formation, semantic_hash};
use crate::seal;
use crate::store::Blob;

pub fn keypair(n: u8) -> Keypair {
    Keypair::from_seed([n; 32])
}
pub fn content_key(n: u8) -> ContentKey {
    ContentKey([n; 32])
}
pub fn definition_hash(formation: &Formation) -> DefinitionHash {
    semantic_hash(formation)
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

pub fn rules_binding(goal: &GoalId, epoch: u32, formation: &Formation) -> (RulesBinding, Blob) {
    let source = serde_json::to_vec(formation).expect("definition JSON encodes");
    let (object, blob) = sealed_payload(goal, epoch, &source);
    (
        RulesBinding {
            definition: DefinitionRef {
                semantic: definition_hash(formation),
                object,
            },
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
    /// The first record of a goal this author's key governs, naming `host`
    /// as the host's agent.
    pub fn genesis(&mut self, host: PublicKey) -> Event {
        self.genesis_with(host, &Formation::default())
    }
    pub fn genesis_with(&mut self, host: PublicKey, definition: &Formation) -> Event {
        let genesis = Genesis {
            governance: self.key.public(),
            host,
            definition: definition_hash(definition),
            salt: [0; 16],
        };
        self.event(genesis.goal_id(), None, Body::Genesis(genesis))
    }
    /// Initial governance prefix: the first record and the admission of the
    /// host's agent. Bind rules explicitly before authoring work.
    pub fn found_goal(&mut self, host: PublicKey, endpoint: EndpointId) -> (Event, Event) {
        let genesis = self.genesis(host);
        let admission = self.event(
            genesis.header().goal,
            Some(genesis.id()),
            Body::MemberAdmitted {
                name: "member".into(),
                role: None,
                member: host,
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
        task_type: None,
        inputs: BTreeMap::new(),
        parent: None,
        stage: None,
    };
    let binding = RulesBinding {
        definition: DefinitionRef {
            semantic: definition_hash(&Formation::default()),
            object: PayloadRef {
                hash,
                len: seal::OVERHEAD_BYTES as u32,
                key_epoch: 0,
            },
        },
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
    let policy = crate::farm::DisclosurePolicy {
        version: crate::farm::FARM_VERSION,
        title: None,
        formation: "Public formation".into(),
        stage_labels: BTreeMap::new(),
        role_labels: BTreeMap::new(),
        recent_changes: 50,
    };
    vec![
        Body::PublicationSet(crate::farm::PublicationSet {
            farm_id: crate::farm::FarmId::from_key(key),
            upload_key: key,
            visibility: Some(crate::farm::FarmVisibility::Link),
            policy: policy.clone(),
        }),
        Body::PublicationConsent(crate::farm::PublicationConsent {
            publication: id,
            policy_digest: policy.digest(),
            accept: true,
            profile: Some(crate::farm::PublicProfile {
                name: "Member".into(),
                group_label: None,
                harness: crate::farm::Harness::Codex,
            }),
        }),
        Body::Genesis(Genesis {
            governance: key,
            host: keypair(2).public(),
            definition: binding.definition.semantic,
            salt: [0; 16],
        }),
        Body::MemberAdmitted {
            name: "member".into(),
            role: None,
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
        Body::WorkspaceEpoch {
            expected_epoch: None,
            rules: id,
            checkpoint: WorkspaceCheckpoint::Unseeded,
        },
        Body::WorkspaceProposed {
            context: Context {
                scope: Scope::Workspace,
                round: id,
            },
            parent: None,
            result_manifest: hash,
            sources: Vec::new(),
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
            closure: Some(id),
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
            sources: Vec::new(),
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
        Body::RoleHolders {
            role: "reviewer".into(),
            holders: vec![key],
        },
    ]
}
