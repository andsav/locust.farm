//! Strict public farm contract. Private goal views are never serialized here.
use crate::{
    crypto::{self, Keypair},
    id::{PublicKey, Signature},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const FARM_VERSION: u16 = 1;
const SIGN_DOMAIN: &str = "locust v1 farm request signature";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct FarmId(pub String);
impl FarmId {
    pub fn from_key(key: PublicKey) -> Self {
        let digest = crypto::domain_hash("locust v1 farm identity", &key.0);
        Self(digest[..16].iter().map(|b| format!("{b:02x}")).collect())
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.0.len() == 32
            && self
                .0
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(())
        } else {
            Err("farm id must be 32 lowercase hex characters".into())
        }
    }
}
impl std::fmt::Display for FarmId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::str::FromStr for FarmId {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let id = Self(s.into());
        id.validate()?;
        Ok(id)
    }
}

macro_rules! public_enum { ($name:ident { $($variant:ident),* $(,)? }) => {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
    #[serde(rename_all="snake_case")]
    pub enum $name { $($variant),* }
}; }
public_enum!(FarmVisibility { Link, Listed });
public_enum!(FarmAvailability {
    Available,
    Unavailable
});
pub type FarmServiceStatus = FarmAvailability;
public_enum!(Harness {
    Codex,
    ClaudeCode,
    FactoryDroid,
    Pi,
    Unknown,
    Multiple
});
public_enum!(FarmGoalState {
    Open,
    Ended,
    Unavailable,
    Disputed
});
public_enum!(FarmTaskState {
    Open,
    Reported,
    AwaitingEvidence,
    Completed,
    Closed,
    Unavailable,
    Disputed
});
public_enum!(FarmAttemptState {
    Started,
    Progress,
    Completed,
    Failed,
    Abandoned,
    Uncertain
});
public_enum!(FarmOperation {
    Upload,
    CheckIn,
    Suspend,
    Delete
});
public_enum!(FarmChangeKind {
    Membership,
    Task,
    Attempt,
    Contribution,
    Evidence,
    Closure,
    Retraction,
    Publication
});

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicProfile {
    pub name: String,
    pub group_label: Option<String>,
    pub harness: Harness,
}
impl PublicProfile {
    pub fn validate(&self) -> Result<(), String> {
        label(&self.name, 80)?;
        if let Some(s) = &self.group_label {
            label(s, 80)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FarmAgent {
    pub id: u32,
    pub group: u32,
    pub name: String,
    pub harness: Harness,
    pub roles: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FarmGroup {
    pub id: u32,
    pub label: Option<String>,
    pub last_sync_at_ms: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FarmStage {
    pub id: u32,
    pub label: String,
    pub prerequisites: Vec<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FarmTask {
    pub id: u32,
    pub reference: String,
    pub stage: Option<u32>,
    pub round: u32,
    pub state: FarmTaskState,
    pub completed: bool,
    pub closed: bool,
    pub selected_candidate: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FarmAttempt {
    pub id: u32,
    pub agent: u32,
    pub task: u32,
    pub round: u32,
    pub state: FarmAttemptState,
    pub observed_at_ms: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FarmCandidate {
    pub id: u32,
    pub agent: u32,
    pub task: Option<u32>,
    pub round: u32,
    pub completed: bool,
    pub selected: bool,
    pub evidence_count: u32,
    pub requirement: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FarmChange {
    pub id: u64,
    pub kind: FarmChangeKind,
    pub agent: Option<u32>,
    pub task: Option<u32>,
    pub observed_at_ms: Option<u64>,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FarmSnapshot {
    pub version: u16,
    pub farm_id: FarmId,
    pub title: Option<String>,
    pub formation: String,
    pub goal_state: FarmGoalState,
    pub observed_at_ms: Option<u64>,
    pub agents: Vec<FarmAgent>,
    pub groups: Vec<FarmGroup>,
    pub stages: Vec<FarmStage>,
    pub tasks: Vec<FarmTask>,
    pub attempts: Vec<FarmAttempt>,
    pub candidates: Vec<FarmCandidate>,
    pub changes: Vec<FarmChange>,
    pub omitted_changes: u64,
}
fn label(s: &str, max: usize) -> Result<(), String> {
    if !s.is_empty() && s.chars().count() <= max && !s.chars().any(char::is_control) {
        Ok(())
    } else {
        Err("invalid public label".into())
    }
}
fn unique(ids: impl Iterator<Item = u32>) -> Result<BTreeSet<u32>, String> {
    let mut set = BTreeSet::new();
    for id in ids {
        if id == 0 || !set.insert(id) {
            return Err("duplicate or zero public id".into());
        }
    }
    Ok(set)
}
impl FarmSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != FARM_VERSION {
            return Err("unsupported farm version".into());
        }
        self.farm_id.validate()?;
        label(&self.formation, 120)?;
        if let Some(s) = &self.title {
            label(s, 160)?;
        }
        let agents = unique(self.agents.iter().map(|x| x.id))?;
        let groups = unique(self.groups.iter().map(|x| x.id))?;
        let stages = unique(self.stages.iter().map(|x| x.id))?;
        let tasks = unique(self.tasks.iter().map(|x| x.id))?;
        unique(self.attempts.iter().map(|x| x.id))?;
        let candidates = unique(self.candidates.iter().map(|x| x.id))?;
        for x in &self.agents {
            label(&x.name, 80)?;
            if !groups.contains(&x.group) {
                return Err("unknown agent group".into());
            }
            if x.roles.iter().collect::<BTreeSet<_>>().len() != x.roles.len() {
                return Err("duplicate role label".into());
            }
            for r in &x.roles {
                label(r, 80)?;
            }
        }
        for x in &self.groups {
            if let Some(s) = &x.label {
                label(s, 80)?;
            }
        }
        for x in &self.stages {
            label(&x.label, 80)?;
            if x.prerequisites.iter().collect::<BTreeSet<_>>().len() != x.prerequisites.len() {
                return Err("duplicate stage edge".into());
            }
            if x.prerequisites
                .iter()
                .any(|id| *id == x.id || !stages.contains(id))
            {
                return Err("invalid stage edge".into());
            }
        }
        let mut done = BTreeSet::new();
        loop {
            let before = done.len();
            for x in &self.stages {
                if x.prerequisites.iter().all(|id| done.contains(id)) {
                    done.insert(x.id);
                }
            }
            if done.len() == stages.len() {
                break;
            }
            if done.len() == before {
                return Err("cyclic stage graph".into());
            }
        }
        let mut refs = BTreeSet::new();
        for x in &self.tasks {
            if x.reference.len() < 4
                || x.reference.len() > 32
                || !x.reference.bytes().all(|b| b.is_ascii_alphanumeric())
                || !refs.insert(&x.reference)
                || x.round == 0
                || x.stage.is_some_and(|id| !stages.contains(&id))
                || x.selected_candidate
                    .is_some_and(|id| !candidates.contains(&id))
            {
                return Err("invalid task references".into());
            }
        }
        for x in &self.tasks {
            let current_attempt = self
                .attempts
                .iter()
                .any(|a| a.task == x.id && a.round == x.round);
            let current_candidates: Vec<_> = self
                .candidates
                .iter()
                .filter(|c| c.task == Some(x.id) && c.round == x.round)
                .collect();
            let expected = if x.closed {
                FarmTaskState::Closed
            } else if x.completed {
                FarmTaskState::Completed
            } else if !current_candidates.is_empty() {
                FarmTaskState::AwaitingEvidence
            } else if current_attempt {
                FarmTaskState::Reported
            } else {
                FarmTaskState::Open
            };
            if !matches!(
                x.state,
                FarmTaskState::Unavailable | FarmTaskState::Disputed
            ) && (x.state != expected
                || x.completed != current_candidates.iter().any(|c| c.completed))
            {
                return Err("task state contradicts completion evidence".into());
            }
        }
        for x in &self.attempts {
            if !agents.contains(&x.agent)
                || !tasks.contains(&x.task)
                || x.round == 0
                || self
                    .tasks
                    .iter()
                    .find(|t| t.id == x.task)
                    .is_some_and(|t| x.round > t.round)
            {
                return Err("invalid attempt association".into());
            }
        }
        for x in &self.candidates {
            label(&x.requirement, 160)?;
            if let Some(task) = x.task {
                if let Some(t) = self.tasks.iter().find(|t| t.id == task)
                    && (x.round > t.round
                        || x.selected != (t.selected_candidate == Some(x.id))
                        || (x.selected && !x.completed))
                {
                    return Err("candidate round or selection mismatch".into());
                }
            } else if x.selected {
                return Err("unattached candidate cannot be selected".into());
            }
            if !agents.contains(&x.agent)
                || x.task.is_some_and(|id| !tasks.contains(&id))
                || x.round == 0
            {
                return Err("invalid candidate association".into());
            }
        }
        for x in &self.tasks {
            if let Some(id) = x.selected_candidate {
                let c = self.candidates.iter().find(|c| c.id == id).unwrap();
                if c.task != Some(x.id) || c.round != x.round || !c.selected {
                    return Err("selected candidate belongs to another round".into());
                }
            }
        }
        let mut change_ids = BTreeSet::new();
        let mut last_change = 0;
        for x in &self.changes {
            label(&x.text, 240)?;
            if x.id <= last_change
                || !change_ids.insert(x.id)
                || x.agent.is_some_and(|id| !agents.contains(&id))
                || x.task.is_some_and(|id| !tasks.contains(&id))
            {
                return Err("invalid change reference".into());
            }
            last_change = x.id;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FarmUploadBody {
    pub visibility: FarmVisibility,
    pub snapshot: FarmSnapshot,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FarmControlBody {}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignedFarmRequest {
    pub version: u16,
    pub operation: FarmOperation,
    pub farm_id: FarmId,
    pub sequence: u64,
    pub public_key: PublicKey,
    pub body: String,
    pub signature: Signature,
}
impl SignedFarmRequest {
    /// Canonical envelope: version u16 BE, operation byte (0..3), 16 decoded
    /// farm-id bytes, sequence u64 BE, then BLAKE3 digest of exact UTF-8 body.
    pub fn signing_digest(&self) -> [u8; 32] {
        let mut bytes = Vec::new();
        bytes.extend(self.version.to_be_bytes());
        bytes.push(match self.operation {
            FarmOperation::Upload => 0,
            FarmOperation::CheckIn => 1,
            FarmOperation::Suspend => 2,
            FarmOperation::Delete => 3,
        });
        for pair in self.farm_id.0.as_bytes().chunks(2) {
            let s = std::str::from_utf8(pair).unwrap_or("");
            bytes.push(u8::from_str_radix(s, 16).unwrap_or(0));
        }
        bytes.extend(self.sequence.to_be_bytes());
        bytes.extend(crypto::content_hash(self.body.as_bytes()).0);
        crypto::domain_hash("locust v1 farm request envelope", &bytes)
    }
    pub fn sign(key: &Keypair, operation: FarmOperation, sequence: u64, body: String) -> Self {
        let mut r = Self {
            version: FARM_VERSION,
            operation,
            farm_id: FarmId::from_key(key.public()),
            sequence,
            public_key: key.public(),
            body,
            signature: Signature([0; 64]),
        };
        r.signature = key.sign(SIGN_DOMAIN, &r.signing_digest());
        r
    }
    pub fn verify(&self) -> Result<(), String> {
        self.farm_id.validate()?;
        if self.version != FARM_VERSION
            || self.sequence == 0
            || self.farm_id != FarmId::from_key(self.public_key)
            || !crypto::verify(
                &self.public_key,
                SIGN_DOMAIN,
                &self.signing_digest(),
                &self.signature,
            )
        {
            return Err("invalid signed farm request".into());
        }
        match self.operation {
            FarmOperation::Upload => {
                let b: FarmUploadBody =
                    serde_json::from_str(&self.body).map_err(|e| e.to_string())?;
                b.snapshot.validate()?;
                if b.snapshot.farm_id != self.farm_id {
                    return Err("snapshot farm mismatch".into());
                }
            }
            _ => {
                let _: FarmControlBody =
                    serde_json::from_str(&self.body).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    pub fn request_digest(&self) -> String {
        self.signing_digest()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FarmReceipt {
    pub farm_id: FarmId,
    pub sequence: u64,
    pub request_digest: String,
    pub stream_version: u64,
    pub received_at_ms: u64,
}

/// Internal engine-to-transport handoff. Never part of a public service view.
#[derive(Clone, Debug)]
pub struct FarmUpload {
    pub goal: crate::id::GoalId,
    pub base_url: String,
    pub request: SignedFarmRequest,
}

/// Internal HTTP outcome. Receipt matching is rechecked by the durable owner.
#[derive(Clone, Debug)]
pub struct FarmUploadResult {
    pub goal: crate::id::GoalId,
    pub farm_id: FarmId,
    pub sequence: u64,
    pub outcome: Result<FarmReceipt, String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FarmServiceView {
    pub visibility: Option<FarmVisibility>,
    pub status: FarmAvailability,
    pub farm_id: FarmId,
    pub stream_version: u64,
    pub service_time_ms: u64,
    pub received_at_ms: Option<u64>,
    pub snapshot: Option<FarmSnapshot>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FarmGallery {
    pub farms: Vec<FarmServiceView>,
    pub next_cursor: Option<String>,
}
pub fn snapshot_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(FarmSnapshot)).expect("farm schema serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signatures_bind_body_operation_sequence_and_key() {
        let key = Keypair::from_seed([7; 32]);
        let request = SignedFarmRequest::sign(&key, FarmOperation::CheckIn, 1, "{}".into());
        assert!(request.verify().is_ok());
        let mut changed = request.clone();
        changed.body = "{ }".into();
        assert!(changed.verify().is_err());
        changed = request.clone();
        changed.operation = FarmOperation::Delete;
        assert!(changed.verify().is_err());
        changed = request.clone();
        changed.sequence = 2;
        assert!(changed.verify().is_err());
        changed = request;
        changed.public_key = Keypair::from_seed([8; 32]).public();
        assert!(changed.verify().is_err());
    }
    #[test]
    fn fixture_and_runtime_invariants() {
        let snapshot: FarmSnapshot =
            serde_json::from_str(include_str!("../fixtures/farm-snapshot.json")).unwrap();
        snapshot.validate().unwrap();
        let mut bad = snapshot.clone();
        bad.stages[0].prerequisites = vec![bad.stages[1].id];
        assert!(bad.validate().is_err());
        let mut bad = snapshot.clone();
        bad.tasks[0].completed = false;
        assert!(bad.validate().is_err());
        let mut bad = snapshot.clone();
        bad.candidates[0].selected = false;
        assert!(bad.validate().is_err());
        let mut bad = snapshot.clone();
        bad.attempts[0].round = u32::MAX;
        assert!(bad.validate().is_err());
        let mut bad = snapshot.clone();
        bad.agents[0].roles = vec!["duplicate".into(), "duplicate".into()];
        assert!(bad.validate().is_err());
        let mut json = serde_json::to_value(&snapshot).unwrap();
        json["tasks"][0]["private"] = serde_json::json!("canary");
        assert!(serde_json::from_value::<FarmSnapshot>(json).is_err());
    }
    #[test]
    fn cross_language_signature_vector_is_frozen() {
        let vector: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/farm-signature-vector.json")).unwrap();
        let expected: SignedFarmRequest =
            serde_json::from_value(vector["request"].clone()).unwrap();
        let actual = SignedFarmRequest::sign(
            &Keypair::from_seed([7; 32]),
            FarmOperation::CheckIn,
            1,
            "{}".into(),
        );
        assert_eq!(actual, expected);
        assert_eq!(
            actual.request_digest(),
            vector["envelope_digest"].as_str().unwrap()
        );
        actual.verify().unwrap();
    }
    #[test]
    fn configured_origin_is_shared_and_strict() {
        for url in [
            "https://locust.farm",
            "http://localhost:3000",
            "http://127.0.0.1:3000",
            "http://[::1]:3000",
        ] {
            assert!(service_origin(url).is_ok(), "{url}");
        }
        for url in [
            "https://",
            "https://locust.farm/hidden",
            "https://user@locust.farm",
            "https://locust.farm?next=evil",
            "https://locust.farm#ignored",
            "http://example.com",
        ] {
            assert!(service_origin(url).is_err(), "{url}");
        }
    }
    #[test]
    fn nested_unknown_fields_are_rejected() {
        assert!(
            serde_json::from_str::<PublicProfile>(
                r#"{"name":"A","group_label":null,"harness":"codex","secret":"canary"}"#
            )
            .is_err()
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DisclosurePolicy {
    pub version: u16,
    pub title: Option<String>,
    pub formation: String,
    pub stage_labels: std::collections::BTreeMap<String, String>,
    pub role_labels: std::collections::BTreeMap<String, String>,
    pub recent_changes: u32,
}
impl DisclosurePolicy {
    pub fn digest(&self) -> String {
        crypto::domain_hash(
            "locust v1 farm disclosure policy",
            &serde_json::to_vec(self).expect("policy JSON"),
        )
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != FARM_VERSION || self.recent_changes == 0 {
            return Err("invalid disclosure policy".into());
        }
        label(&self.formation, 120)?;
        if let Some(s) = &self.title {
            label(s, 160)?;
        }
        for s in self.stage_labels.values().chain(self.role_labels.values()) {
            label(s, 80)?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicationSet {
    pub farm_id: FarmId,
    pub upload_key: PublicKey,
    pub visibility: Option<FarmVisibility>,
    pub policy: DisclosurePolicy,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicationConsent {
    pub publication: crate::id::EventId,
    pub policy_digest: String,
    pub accept: bool,
    pub profile: Option<PublicProfile>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FarmStatus {
    pub goal: crate::id::GoalId,
    pub farm_id: FarmId,
    pub desired: Option<FarmVisibility>,
    pub eligible: bool,
    pub reason: Option<String>,
    pub pending: Option<FarmOperation>,
    pub receipt: Option<FarmReceipt>,
    pub last_error: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FarmPreview {
    pub snapshot: Option<FarmSnapshot>,
    pub policy: Option<DisclosurePolicy>,
    pub status: Option<FarmStatus>,
}

/// Normalizes a configured service origin before any durable publication intent.
/// Paths, credentials and redirects cannot change the signed mutation destination.
pub fn service_origin(value: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(value).map_err(|_| "invalid farm service origin")?;
    let local = url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    if url.host_str().is_none()
        || (url.scheme() != "https" && !(url.scheme() == "http" && local))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err("farm service must be an HTTPS origin or loopback HTTP origin".into());
    }
    Ok(url)
}
