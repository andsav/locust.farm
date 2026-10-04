//! Current signed organization protocol. Exact canonical header bytes are hashed
//! and signed; replay never reinterprets an earlier protocol. Governance has one
//! administrator chain; work and scope-specific decisions are separate facts.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::PROTOCOL_VERSION;
use crate::codec::{self, CodecError};
use crate::crypto::{self, Keypair, domain};
use crate::id::{
    BlobHash, DefinitionHash, EffectId, EndpointId, EventId, GoalId, PublicKey, Signature,
};
use crate::limits::{MAX_HEADER_BYTES, MAX_PARENTS, MAX_PAYLOAD_BYTES};
use crate::seal;
use crate::store::Blob;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Header {
    pub version: u8,
    pub goal: GoalId,
    pub author: PublicKey,
    pub seq: u64,
    pub prev: Option<EventId>,
    /// Exact governance position; a governance event names its predecessor.
    pub anchor: Option<EventId>,
    /// Causal hints only. Authority follows typed references.
    pub parents: Vec<EventId>,
    /// Diagnostic wall clock; never orders authority or resolves conflict.
    pub at_ms: u64,
    pub payload: Option<PayloadRef>,
    pub body: Body,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PayloadRef {
    pub hash: BlobHash,
    pub len: u32,
    pub key_epoch: u32,
}

impl PayloadRef {
    pub fn admits(&self, blob: &Blob) -> bool {
        blob.hash() == self.hash
            && u64::try_from(blob.bytes().len()) == Ok(u64::from(self.len))
            && seal::epoch_of(blob.bytes()) == Ok(self.key_epoch)
    }
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
pub struct AuthorPoint {
    pub seq: u64,
    pub id: EventId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Genesis {
    pub administrator: PublicKey,
    pub definition: DefinitionHash,
    pub salt: [u8; 16],
}

impl Genesis {
    pub fn goal_id(&self) -> GoalId {
        let mut hasher = crypto::domain_hasher(domain::GOAL_ID);
        hasher.update(&self.administrator.0);
        hasher.update(&self.definition.0);
        hasher.update(&self.salt);
        GoalId(*hasher.finalize().as_bytes())
    }
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Doc {
    Plan,
    Summary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TaskId {
    Authored(EventId),
    Derived(EffectId),
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authored(id) => write!(f, "task:{id}"),
            Self::Derived(id) => write!(f, "effect:{id}"),
        }
    }
}

impl std::str::FromStr for TaskId {
    type Err = crate::id::ParseIdError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if let Some(id) = text.strip_prefix("task:") {
            return id.parse().map(Self::Authored);
        }
        if let Some(id) = text.strip_prefix("effect:") {
            return id.parse().map(Self::Derived);
        }
        Err(crate::id::ParseIdError)
    }
}

impl Serialize for TaskId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            return serializer.collect_str(self);
        }
        match self {
            Self::Authored(id) => serializer.serialize_newtype_variant("TaskId", 0, "authored", id),
            Self::Derived(id) => serializer.serialize_newtype_variant("TaskId", 1, "derived", id),
        }
    }
}

impl<'de> Deserialize<'de> for TaskId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            return String::deserialize(deserializer)?
                .parse()
                .map_err(serde::de::Error::custom);
        }
        #[derive(Deserialize)]
        enum Binary {
            Authored(EventId),
            Derived(EffectId),
        }
        match Binary::deserialize(deserializer)? {
            Binary::Authored(id) => Ok(Self::Authored(id)),
            Binary::Derived(id) => Ok(Self::Derived(id)),
        }
    }
}

impl schemars::JsonSchema for TaskId {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "TaskId".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"type":"string", "pattern":"^(task|effect):[0-9a-f]{64}$"})
    }
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Goal,
    Task(TaskId),
    Document(Doc),
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
pub struct Context {
    pub scope: Scope,
    /// RulesBound for goal/document scopes; TaskOpened/TaskRevised/effect for tasks.
    pub round: EventId,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DecisionPurpose {
    Selection,
    Closure,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
pub struct ScopeKey {
    pub context: Context,
    pub purpose: DecisionPurpose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DefinitionRef {
    pub semantic: DefinitionHash,
    /// Sealed canonical JSON source; semantic identity is independently checked.
    pub object: PayloadRef,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RulesBinding {
    pub definition: DefinitionRef,
    pub roles: BTreeMap<String, Vec<PublicKey>>,
    pub inputs: BTreeMap<String, BlobHash>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskBinding {
    pub rules: EventId,
    pub variation: Option<String>,
    pub inputs: BTreeMap<String, BlobHash>,
    pub parent: Option<Context>,
    /// Only configured materialization may set this stage identity.
    pub stage: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttemptStatus {
    Progress,
    Completed,
    Failed,
    Abandoned,
    Uncertain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CancelOutcome {
    Stopped,
    Completed,
    Uncertain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReviewVerdict {
    Approve,
    Reject,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecisionAction {
    Select { subject: EventId },
    Close,
    Reopen,
}

impl DecisionAction {
    pub fn purpose(&self) -> DecisionPurpose {
        match self {
            Self::Select { .. } => DecisionPurpose::Selection,
            Self::Close | Self::Reopen => DecisionPurpose::Closure,
        }
    }
}

#[derive(
    Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    Stage { rules: EventId, stage: String },
    Contribution(EventId),
    Completion(Context),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EffectAction {
    OpenTask {
        binding: TaskBinding,
        recipients: Vec<PublicKey>,
    },
    Offer {
        context: Context,
        recipient: PublicKey,
    },
    RequestReview {
        context: Context,
        subject: EventId,
        recipient: PublicKey,
    },
}

impl EffectAction {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::OpenTask { .. } => "open_task",
            Self::Offer { .. } => "offer",
            Self::RequestReview { .. } => "request_review",
        }
    }
}

/// Witness lists do not change logical identity. The evaluator verifies that
/// the resolved action is the exact configured consequence of this trigger.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Effect {
    pub context: Context,
    pub transition: String,
    pub trigger: Trigger,
    pub target_slot: String,
    pub action: EffectAction,
    pub evidence: Vec<EventId>,
}

impl Effect {
    pub fn id(&self, goal: GoalId) -> EffectId {
        let bytes = codec::encode(&(
            goal,
            self.context,
            &self.transition,
            &self.trigger,
            self.action.kind(),
            &self.target_slot,
        ))
        .expect("effect identity encodes");
        EffectId(crypto::domain_hash("locust v2 logical effect", &bytes))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Body {
    Genesis(Genesis),
    MemberAdmitted {
        member: PublicKey,
        endpoint: EndpointId,
    },
    MemberRemoved {
        member: PublicKey,
        admission: EventId,
        last_accepted: Option<AuthorPoint>,
    },
    RulesBound {
        expected: Option<EventId>,
        binding: RulesBinding,
    },
    TaskRevised {
        task: TaskId,
        expected_round: EventId,
        binding: TaskBinding,
    },
    TaskOpened {
        binding: TaskBinding,
    },
    WorkOffered {
        context: Context,
        recipient: PublicKey,
    },
    AttemptStarted {
        context: Context,
        offer: Option<EventId>,
        /// Exact observed position in this round's closure stream. None means
        /// no position has been observed; concurrent offline starts remain valid.
        closure: Option<EventId>,
    },
    AttemptReported {
        attempt: EventId,
        status: AttemptStatus,
    },
    WorkDeclined {
        offer: EventId,
    },
    CancelRequested {
        attempt: EventId,
    },
    CancelAcknowledged {
        cancel: EventId,
        outcome: CancelOutcome,
    },
    ContributionPublished {
        context: Context,
        attempt: Option<EventId>,
        /// Author-declared citations, not causal dependencies or proof of use.
        sources: Vec<EventId>,
        base: Option<BlobHash>,
        patch: Option<BlobHash>,
        artifacts: Vec<BlobHash>,
    },
    CompletionDeclared {
        context: Context,
        subject: EventId,
    },
    ReviewRecorded {
        context: Context,
        subject: EventId,
        verdict: ReviewVerdict,
    },
    CheckAttested {
        context: Context,
        subject: EventId,
        name: String,
        passed: bool,
    },
    ScopeDecided {
        context: Context,
        previous: Option<EventId>,
        action: DecisionAction,
        evidence: Vec<EventId>,
    },
    DocumentRevised {
        context: Context,
        doc: Doc,
        base: Option<EventId>,
    },
    EffectMaterialized {
        effect: Effect,
    },
    DeliveryAcknowledged {
        effect: EffectId,
    },
    LeaveRequested {
        admission: EventId,
    },
}

impl Body {
    pub fn is_governance(&self) -> bool {
        matches!(
            self,
            Self::Genesis(_)
                | Self::MemberAdmitted { .. }
                | Self::MemberRemoved { .. }
                | Self::RulesBound { .. }
                | Self::TaskRevised { .. }
        )
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Genesis(_) => "genesis",
            Self::MemberAdmitted { .. } => "member_admitted",
            Self::MemberRemoved { .. } => "member_removed",
            Self::RulesBound { .. } => "rules_bound",
            Self::TaskRevised { .. } => "task_revised",
            Self::TaskOpened { .. } => "task_opened",
            Self::WorkOffered { .. } => "work_offered",
            Self::AttemptStarted { .. } => "attempt_started",
            Self::AttemptReported { .. } => "attempt_reported",
            Self::WorkDeclined { .. } => "work_declined",
            Self::CancelRequested { .. } => "cancel_requested",
            Self::CancelAcknowledged { .. } => "cancel_acknowledged",
            Self::ContributionPublished { .. } => "contribution_published",
            Self::CompletionDeclared { .. } => "completion_declared",
            Self::ReviewRecorded { .. } => "review_recorded",
            Self::CheckAttested { .. } => "check_attested",
            Self::ScopeDecided { .. } => "scope_decided",
            Self::DocumentRevised { .. } => "document_revised",
            Self::EffectMaterialized { .. } => "effect_materialized",
            Self::DeliveryAcknowledged { .. } => "delivery_acknowledged",
            Self::LeaveRequested { .. } => "leave_requested",
        }
    }

    pub fn context(&self) -> Option<Context> {
        match self {
            Self::WorkOffered { context, .. }
            | Self::AttemptStarted { context, .. }
            | Self::ContributionPublished { context, .. }
            | Self::CompletionDeclared { context, .. }
            | Self::ReviewRecorded { context, .. }
            | Self::CheckAttested { context, .. }
            | Self::ScopeDecided { context, .. }
            | Self::DocumentRevised { context, .. } => Some(*context),
            Self::EffectMaterialized { effect } => Some(effect.context),
            _ => None,
        }
    }

    /// Typed dependencies. Causal hints cannot smuggle a branch into a proof.
    pub fn dependencies(&self) -> Vec<EventId> {
        let mut ids = BTreeSet::new();
        if let Some(context) = self.context() {
            ids.insert(context.round);
        }
        match self {
            Self::MemberRemoved {
                admission,
                last_accepted,
                ..
            } => {
                ids.insert(*admission);
                ids.extend(last_accepted.iter().map(|point| point.id));
            }
            Self::RulesBound { expected, .. } => {
                ids.extend(expected);
            }
            Self::TaskOpened { binding } => {
                ids.insert(binding.rules);
                ids.extend(binding.parent.map(|context| context.round));
            }
            Self::TaskRevised {
                expected_round,
                binding,
                ..
            } => {
                ids.insert(*expected_round);
                ids.insert(binding.rules);
                ids.extend(binding.parent.map(|context| context.round));
            }
            Self::AttemptStarted { offer, closure, .. } => {
                ids.extend(offer);
                ids.extend(closure);
            }
            Self::AttemptReported { attempt, .. } | Self::CancelRequested { attempt } => {
                ids.insert(*attempt);
            }
            Self::WorkDeclined { offer } => {
                ids.insert(*offer);
            }
            Self::CancelAcknowledged { cancel, .. } => {
                ids.insert(*cancel);
            }
            Self::ContributionPublished { attempt, .. } => {
                ids.extend(attempt);
            }
            Self::CompletionDeclared { subject, .. }
            | Self::ReviewRecorded { subject, .. }
            | Self::CheckAttested { subject, .. } => {
                ids.insert(*subject);
            }
            Self::ScopeDecided {
                previous,
                action,
                evidence,
                ..
            } => {
                ids.extend(previous);
                ids.extend(evidence);
                if let DecisionAction::Select { subject } = action {
                    ids.insert(*subject);
                }
            }
            Self::DocumentRevised { base, .. } => {
                ids.extend(base);
            }
            Self::EffectMaterialized { effect } => {
                ids.extend(&effect.evidence);
                match &effect.trigger {
                    Trigger::Stage { rules, .. } => {
                        ids.insert(*rules);
                    }
                    Trigger::Contribution(id) => {
                        ids.insert(*id);
                    }
                    Trigger::Completion(context) => {
                        ids.insert(context.round);
                    }
                }
                match &effect.action {
                    EffectAction::OpenTask { binding, .. } => {
                        ids.insert(binding.rules);
                        ids.extend(binding.parent.map(|context| context.round));
                    }
                    EffectAction::Offer { context, .. } => {
                        ids.insert(context.round);
                    }
                    EffectAction::RequestReview {
                        context, subject, ..
                    } => {
                        ids.insert(context.round);
                        ids.insert(*subject);
                    }
                }
            }
            Self::LeaveRequested { admission } => {
                ids.insert(*admission);
            }
            _ => {}
        }
        ids.into_iter().collect()
    }
}
/// Why bytes were not accepted as a structurally valid event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventError {
    /// The header bytes exceed [`MAX_HEADER_BYTES`].
    TooLarge,
    /// The bytes do not decode as a header.
    Malformed,
    /// The bytes decode, but are not the one encoding of that header.
    NotCanonical,
    /// The header's version byte is not [`PROTOCOL_VERSION`].
    UnsupportedVersion(u8),
    /// `seq` and `prev` disagree, or `seq` exceeds `i64::MAX`.
    BadSequence,
    /// Too many parents, dependencies or artifacts, or parents out of order.
    BadReferences,
    /// A genesis event that does not found the goal it names under this
    /// version's rules, or a later event with no anchor.
    BadAnchor,
    /// The payload reference names more than [`MAX_PAYLOAD_BYTES`].
    /// The payload length is outside what a sealed payload can have: shorter
    /// than an empty sealed object, or above the payload limit.
    BadPayloadLength,
    /// The signing key is not the header's author.
    AuthorMismatch,
    /// The signature does not verify under the header's author.
    BadSignature,
    /// Stored bytes do not hash to the identifier they were stored under.
    IdMismatch,
}

impl fmt::Display for EventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => f.write_str("header exceeds the admitted size"),
            Self::Malformed => f.write_str("bytes are not an event header"),
            Self::NotCanonical => f.write_str("header is not in its canonical encoding"),
            Self::UnsupportedVersion(version) => {
                write!(f, "protocol version {version} is not supported")
            }
            Self::BadSequence => {
                f.write_str("sequence number is out of range or disagrees with the previous event")
            }
            Self::BadReferences => f.write_str("references are out of order or too many"),
            Self::BadAnchor => f.write_str("anchor does not fit the event kind"),
            Self::BadPayloadLength => {
                f.write_str("payload length is not that of an admitted sealed object")
            }
            Self::AuthorMismatch => f.write_str("signing key is not the header's author"),
            Self::BadSignature => f.write_str("signature does not verify"),
            Self::IdMismatch => f.write_str("stored bytes do not match their event identifier"),
        }
    }
}

impl std::error::Error for EventError {}

impl Header {
    pub fn check(&self) -> Result<(), EventError> {
        if self.version != PROTOCOL_VERSION {
            return Err(EventError::UnsupportedVersion(self.version));
        }
        if self.seq > i64::MAX as u64 || (self.seq == 0) != self.prev.is_none() {
            return Err(EventError::BadSequence);
        }
        if self.parents.len() > MAX_PARENTS || !self.parents.is_sorted_by(|a, b| a < b) {
            return Err(EventError::BadReferences);
        }
        let mut payloads: Vec<_> = self.payload.iter().collect();
        if let Body::RulesBound { binding, .. } = &self.body {
            payloads.push(&binding.definition.object);
        }
        if payloads.iter().any(|payload| {
            !(seal::OVERHEAD_BYTES..=MAX_PAYLOAD_BYTES).contains(&(payload.len as usize))
        }) {
            return Err(EventError::BadPayloadLength);
        }
        match &self.body {
            Body::Genesis(genesis) => {
                if self.seq != 0
                    || self.anchor.is_some()
                    || !self.parents.is_empty()
                    || self.author != genesis.administrator
                    || self.goal != genesis.goal_id()
                {
                    return Err(EventError::BadAnchor);
                }
            }
            _ if self.anchor.is_none() => return Err(EventError::BadAnchor),
            _ => {}
        }
        Ok(())
    }

    pub fn blobs(&self) -> Vec<BlobHash> {
        let mut blobs: BTreeSet<_> = self.payload.iter().map(|payload| payload.hash).collect();
        match &self.body {
            Body::RulesBound { binding, .. } => {
                blobs.insert(binding.definition.object.hash);
                blobs.extend(binding.inputs.values());
            }
            Body::TaskOpened { binding } | Body::TaskRevised { binding, .. } => {
                blobs.extend(binding.inputs.values());
            }
            Body::ContributionPublished {
                base,
                patch,
                artifacts,
                ..
            } => {
                blobs.extend(base);
                blobs.extend(patch);
                blobs.extend(artifacts);
            }
            Body::EffectMaterialized { effect } => {
                if let EffectAction::OpenTask { binding, .. } = &effect.action {
                    blobs.extend(binding.inputs.values());
                }
            }
            _ => {}
        }
        blobs.into_iter().collect()
    }
}

fn event_id(header_bytes: &[u8]) -> EventId {
    EventId(crypto::domain_hash(domain::EVENT_ID, header_bytes))
}

/// An event as it travels and as it is stored: the exact signed bytes and the
/// signature over their identifier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WireEvent {
    /// The exact header bytes that were hashed and signed.
    #[serde(with = "codec::bytes")]
    #[schemars(with = "Vec<u8>")]
    pub header: Vec<u8>,
    /// The author's signature over the identifier of `header`.
    pub signature: Signature,
}

impl WireEvent {
    /// The identifier these bytes have if they are a valid event. Hashing is
    /// far cheaper than [`Event::decode`], so a receiver drops identifiers it
    /// already holds before paying for decoding and signature verification.
    pub fn id(&self) -> EventId {
        event_id(&self.header)
    }
}

/// A structurally valid event: decoded, canonical and signed by its author.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    id: EventId,
    header: Header,
    bytes: Box<[u8]>,
    signature: Signature,
}

impl Event {
    /// Encodes and signs a new event. Refuses a header whose author is not
    /// `key`'s principal, one that fails [`Header::check`], and one whose
    /// encoding exceeds [`MAX_HEADER_BYTES`].
    pub fn sign(header: Header, key: &Keypair) -> Result<Self, EventError> {
        if header.author != key.public() {
            return Err(EventError::AuthorMismatch);
        }
        header.check()?;
        let bytes = codec::encode(&header).map_err(|_| EventError::Malformed)?;
        if bytes.len() > MAX_HEADER_BYTES {
            return Err(EventError::TooLarge);
        }
        let id = event_id(&bytes);
        let signature = key.sign(domain::EVENT_SIGNATURE, &id.0);
        Ok(Self {
            id,
            header,
            bytes: bytes.into_boxed_slice(),
            signature,
        })
    }

    /// Validates bytes received from a peer or a client: size, version byte,
    /// the one canonical encoding (through [`codec::decode_canonical`]),
    /// [`Header::check`] and the author's signature, in that order.
    pub fn decode(header_bytes: &[u8], signature: Signature) -> Result<Self, EventError> {
        check_size_and_version(header_bytes)?;
        // One header has exactly one encoding, so one identifier.
        let header: Header =
            codec::decode_canonical(header_bytes).map_err(|error| match error {
                CodecError::NotCanonical => EventError::NotCanonical,
                _ => EventError::Malformed,
            })?;
        header.check()?;
        let id = event_id(header_bytes);
        if !crypto::verify(&header.author, domain::EVENT_SIGNATURE, &id.0, &signature) {
            return Err(EventError::BadSignature);
        }
        Ok(Self {
            id,
            header,
            bytes: header_bytes.into(),
            signature,
        })
    }

    /// Rebuilds an event that this daemon validated with [`Event::decode`] or
    /// [`Event::sign`] before persisting it under `id`. Runs
    /// [`Header::check`] and refuses bytes that no longer hash to `id`
    /// ([`EventError::IdMismatch`]), so a damaged record is reported instead
    /// of served under the wrong identifier. Skips the canonical re-encode and
    /// the signature check, which the bytes passed before they were stored;
    /// never use it on input from outside the local store.
    pub fn from_stored(
        id: EventId,
        header_bytes: Vec<u8>,
        signature: Signature,
    ) -> Result<Self, EventError> {
        check_size_and_version(&header_bytes)?;
        let header: Header = codec::decode(&header_bytes).map_err(|_| EventError::Malformed)?;
        header.check()?;
        if event_id(&header_bytes) != id {
            return Err(EventError::IdMismatch);
        }
        Ok(Self {
            id,
            header,
            bytes: header_bytes.into_boxed_slice(),
            signature,
        })
    }

    /// Validates an event received in a sync frame; see [`Event::decode`].
    pub fn from_wire(wire: &WireEvent) -> Result<Self, EventError> {
        Self::decode(&wire.header, wire.signature)
    }

    /// The exact signed bytes and signature, for relaying or storing.
    pub fn to_wire(&self) -> WireEvent {
        WireEvent {
            header: self.bytes.to_vec(),
            signature: self.signature,
        }
    }

    /// The domain-separated digest of the exact header bytes.
    pub fn id(&self) -> EventId {
        self.id
    }

    /// The decoded header.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The exact bytes that were hashed and signed.
    pub fn header_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The author's signature over [`Event::id`].
    pub fn signature(&self) -> Signature {
        self.signature
    }
}

/// Refuses oversized bytes, and reports another version from the first byte
/// before anything else is decoded.
fn check_size_and_version(bytes: &[u8]) -> Result<(), EventError> {
    if bytes.len() > MAX_HEADER_BYTES {
        return Err(EventError::TooLarge);
    }
    match bytes.first() {
        Some(&PROTOCOL_VERSION) => Ok(()),
        Some(&version) => Err(EventError::UnsupportedVersion(version)),
        None => Err(EventError::Malformed),
    }
}
