//! Session-bound notification receipts. Pending task state remains authoritative.
//!
//! Only the single managed lifecycle owner may persist receipt metadata;
//! SessionReport has no compare-and-swap. Hook callbacks must remain read-only.
//! Persist `PreparedDelivery::receipt` in managed metadata before emitting a hook
//! notification. Record emission only after the local transport succeeds. A
//! prepared receipt is retried; an emitted duplicate may be suppressed by a hook,
//! but ordinary tools always return the full notice. Neither receipt claims work,
//! acknowledges cancellation, launches a client, or proves model observation.

use locust_proto::api::{ApiError, Claim, ErrorCode, PendingWork, SessionState, SessionView};
use locust_proto::id::{BlobHash, GoalId, InstanceId, PublicKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryBinding {
    pub goal: GoalId,
    pub principal: PublicKey,
    pub instance: InstanceId,
    pub claim: Option<Claim>,
}

/// Daemon-authored identifiers and local session status only. Cancellation items
/// mean requested cancellation, never an observed executor outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryNotice {
    pub binding: DeliveryBinding,
    pub session_state: SessionState,
    pub pending: PendingWork,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryChannel {
    OrdinaryTool,
    QualifiedHook,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum DeliveryState {
    Prepared,
    Emitted { channel: DeliveryChannel },
}

/// One receipt for the managed session's bound goal, not a collaboration cursor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryReceipt {
    pub schema: u32,
    pub binding: DeliveryBinding,
    pub session_state: SessionState,
    pub work_fingerprint: BlobHash,
    pub state: DeliveryState,
}

pub struct PreparedDelivery {
    /// Returned even when `duplicate` is true; pending recovery never depends on
    /// notification bookkeeping.
    pub notice: DeliveryNotice,
    pub receipt: DeliveryReceipt,
    pub duplicate: bool,
}

fn error(code: ErrorCode, message: &str) -> ApiError {
    ApiError::new(code, message)
}

impl DeliveryReceipt {
    pub fn decode(value: &Value) -> Result<Option<Self>, ApiError> {
        if value.is_null() {
            return Ok(None);
        }
        let receipt: Self = serde_json::from_value(value.clone())
            .map_err(|_| error(ErrorCode::Corrupted, "invalid delivery receipt"))?;
        if receipt.schema != 1 {
            return Err(error(
                ErrorCode::UnsupportedVersion,
                "unsupported delivery receipt",
            ));
        }
        Ok(Some(receipt))
    }

    /// The single lifecycle owner merges this value into managed metadata and reports the whole
    /// session record. SessionRecord::check enforces the existing detail limit.
    pub fn encode(&self) -> Result<Value, ApiError> {
        serde_json::to_value(self)
            .map_err(|_| error(ErrorCode::Internal, "cannot encode delivery receipt"))
    }

    /// Records transport completion, not delivery to the model or task ownership.
    /// Revalidates a fresh snapshot so a takeover cannot emit a stale receipt.
    pub fn emitted(
        &self,
        session: &SessionView,
        pending: &PendingWork,
        channel: DeliveryChannel,
    ) -> Result<Self, ApiError> {
        if self.schema != 1 {
            return Err(error(
                ErrorCode::UnsupportedVersion,
                "unsupported delivery receipt",
            ));
        }
        validate(&self.binding, session, pending)?;
        if self.work_fingerprint != fingerprint(pending)? {
            return Err(error(
                ErrorCode::Superseded,
                "pending work changed before emission receipt",
            ));
        }
        if channel == DeliveryChannel::QualifiedHook
            && (!session.record.capabilities.active_delivery
                || !matches!(
                    session.record.state,
                    SessionState::Ready | SessionState::Blocked
                ))
        {
            return Err(error(
                ErrorCode::Invalid,
                "session has no qualified active hook",
            ));
        }
        let mut receipt = self.clone();
        receipt.session_state = session.record.state;
        receipt.state = DeliveryState::Emitted { channel };
        Ok(receipt)
    }
}

fn fingerprint(pending: &PendingWork) -> Result<BlobHash, ApiError> {
    let mut normalized = pending.clone();
    normalized.revision = 0;
    let bytes = serde_json::to_vec(&normalized)
        .map_err(|_| error(ErrorCode::Internal, "cannot fingerprint pending work"))?;
    Ok(locust_proto::crypto::content_hash(&bytes))
}

fn validate(
    binding: &DeliveryBinding,
    session: &SessionView,
    pending: &PendingWork,
) -> Result<(), ApiError> {
    if binding.instance != session.instance || binding.principal != session.principal {
        return Err(error(
            ErrorCode::Denied,
            "delivery belongs to another session or principal",
        ));
    }
    if let Some(claim) = binding.claim
        && (claim.goal != binding.goal
            || claim.instance != binding.instance
            || !session.claims.contains(&claim)
            || !pending.claimed.contains(&claim))
    {
        return Err(error(
            ErrorCode::Superseded,
            "delivery claim is no longer current",
        ));
    }
    let claims: Vec<_> = session
        .claims
        .iter()
        .copied()
        .filter(|claim| claim.goal == binding.goal)
        .collect();
    if pending
        .claimed
        .iter()
        .any(|claim| claim.goal != binding.goal || claim.instance != binding.instance)
        || claims.len() != pending.claimed.len()
        || claims.iter().any(|claim| !pending.claimed.contains(claim))
    {
        return Err(error(
            ErrorCode::Superseded,
            "session and pending claim snapshots differ",
        ));
    }
    for cancel in &pending.to_acknowledge {
        let claim = pending
            .claimed
            .iter()
            .find(|claim| claim.attempt == cancel.attempt && claim.task == cancel.task);
        if cancel.generation != claim.map(|claim| claim.generation) {
            return Err(error(
                ErrorCode::Superseded,
                "cancellation generation is no longer current",
            ));
        }
    }
    Ok(())
}

/// `pending` must be fetched with Request::Pending for `binding.goal` on the
/// same authenticated session. PendingWork has no goal field: the API request
/// context supplies that binding. Fetch fresh snapshots when validation detects
/// concurrent claim changes; propagate daemon API errors without recategorizing.
pub fn prepare(
    binding: DeliveryBinding,
    session: &SessionView,
    pending: PendingWork,
    previous: Option<&DeliveryReceipt>,
) -> Result<PreparedDelivery, ApiError> {
    validate(&binding, session, &pending)?;
    let work_fingerprint = fingerprint(&pending)?;
    let duplicate = previous.is_some_and(|receipt| {
        receipt.schema == 1
            && receipt.binding == binding
            && receipt.work_fingerprint == work_fingerprint
            && matches!(receipt.state, DeliveryState::Emitted { .. })
    });
    let notice = DeliveryNotice {
        binding,
        session_state: session.record.state,
        pending,
    };
    let receipt = DeliveryReceipt {
        schema: 1,
        binding: notice.binding.clone(),
        session_state: notice.session_state,
        work_fingerprint,
        state: if duplicate {
            previous.expect("duplicate requires receipt").state.clone()
        } else {
            DeliveryState::Prepared
        },
    };
    Ok(PreparedDelivery {
        notice,
        receipt,
        duplicate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_proto::api::{CancelItem, SessionCapabilities, SessionRecord};
    use locust_proto::id::EventId;

    fn fixture() -> (DeliveryBinding, SessionView, PendingWork) {
        let claim = Claim {
            goal: GoalId([1; 32]),
            task: locust_proto::event::TaskId::Authored(EventId([2; 32])),
            attempt: EventId([3; 32]),
            instance: InstanceId([4; 16]),
            generation: 1,
        };
        let binding = DeliveryBinding {
            goal: claim.goal,
            principal: PublicKey([5; 32]),
            instance: claim.instance,
            claim: Some(claim),
        };
        let session = SessionView {
            instance: binding.instance,
            principal: binding.principal,
            record: SessionRecord {
                harness: locust_proto::farm::Harness::Unknown,
                client: "isolated test client".into(),
                state: SessionState::Ready,
                client_session: None,
                capabilities: SessionCapabilities::default(),
                detail: Vec::new(),
            },
            updated_ms: 1,
            attached: true,
            claims: vec![claim],
        };
        let pending = PendingWork {
            revision: 1,
            claimed: vec![claim],
            to_acknowledge: vec![CancelItem {
                task: claim.task,
                attempt: claim.attempt,
                cancel: EventId([6; 32]),
                generation: Some(1),
            }],
            ..PendingWork::default()
        };
        (binding, session, pending)
    }

    #[test]
    fn lost_notification_retries_but_emission_never_hides_pending() {
        let (binding, session, mut pending) = fixture();
        let first = prepare(binding.clone(), &session, pending.clone(), None).unwrap();
        let recovered = DeliveryReceipt::decode(&first.receipt.encode().unwrap())
            .unwrap()
            .unwrap();
        assert!(
            !prepare(binding.clone(), &session, pending.clone(), Some(&recovered))
                .unwrap()
                .duplicate
        );
        let emitted = recovered
            .emitted(&session, &pending, DeliveryChannel::OrdinaryTool)
            .unwrap();
        // Writing adapter metadata raises revision without changing pending work.
        pending.revision = 3;
        let replay = prepare(binding, &session, pending.clone(), Some(&emitted)).unwrap();
        assert!(replay.duplicate);
        assert_eq!(replay.notice.pending, pending);
        assert_eq!(replay.notice.pending.to_acknowledge.len(), 1);
    }

    #[test]
    fn takeover_invalidates_saved_binding_and_late_emission() {
        let (binding, mut session, mut pending) = fixture();
        let prepared = prepare(binding.clone(), &session, pending.clone(), None).unwrap();
        session.claims[0].generation = 2;
        pending.claimed[0].generation = 2;
        pending.to_acknowledge[0].generation = Some(2);
        assert_eq!(
            prepare(binding, &session, pending.clone(), None)
                .err()
                .unwrap()
                .code,
            ErrorCode::Superseded
        );
        assert_eq!(
            prepared
                .receipt
                .emitted(&session, &pending, DeliveryChannel::OrdinaryTool)
                .unwrap_err()
                .code,
            ErrorCode::Superseded
        );
    }

    #[test]
    fn stale_cancel_and_mismatched_session_are_rejected() {
        let (binding, mut session, mut pending) = fixture();
        pending.to_acknowledge[0].generation = Some(2);
        assert_eq!(
            prepare(binding.clone(), &session, pending.clone(), None)
                .err()
                .unwrap()
                .code,
            ErrorCode::Superseded
        );
        session.instance = InstanceId([9; 16]);
        assert_eq!(
            prepare(binding, &session, pending, None)
                .err()
                .unwrap()
                .code,
            ErrorCode::Denied
        );
    }

    #[test]
    fn exit_does_not_erase_work_and_hooks_require_independent_capability() {
        let (binding, mut session, pending) = fixture();
        let receipt = prepare(binding.clone(), &session, pending.clone(), None)
            .unwrap()
            .receipt;
        assert_eq!(
            receipt
                .emitted(&session, &pending, DeliveryChannel::QualifiedHook)
                .unwrap_err()
                .code,
            ErrorCode::Invalid
        );
        session.record.capabilities.active_delivery = true;
        assert!(
            receipt
                .emitted(&session, &pending, DeliveryChannel::QualifiedHook)
                .is_ok()
        );
        session.record.state = SessionState::Exited;
        let recovered = prepare(binding, &session, pending.clone(), Some(&receipt)).unwrap();
        assert_eq!(recovered.notice.pending, pending);
        assert_eq!(
            receipt
                .emitted(&session, &pending, DeliveryChannel::QualifiedHook)
                .unwrap_err()
                .code,
            ErrorCode::Invalid
        );
    }

    #[test]
    fn notification_change_cannot_be_recorded_as_old_emission() {
        let (binding, session, mut pending) = fixture();
        let receipt = prepare(binding.clone(), &session, pending.clone(), None)
            .unwrap()
            .receipt;
        pending.to_acknowledge.clear();
        assert_eq!(
            receipt
                .emitted(&session, &pending, DeliveryChannel::OrdinaryTool)
                .unwrap_err()
                .code,
            ErrorCode::Superseded
        );
        assert!(
            !prepare(binding, &session, pending, Some(&receipt))
                .unwrap()
                .duplicate
        );
        assert_eq!(
            DeliveryReceipt::decode(&serde_json::json!({"schema": 1}))
                .unwrap_err()
                .code,
            ErrorCode::Corrupted
        );
    }
    #[test]
    fn durable_receipt_size_does_not_grow_with_pending_work() {
        let (binding, session, mut pending) = fixture();
        let first = prepare(binding.clone(), &session, pending.clone(), None).unwrap();
        for _ in 0..2000 {
            pending.to_start.push(locust_proto::api::WorkItem {
                task: locust_proto::event::TaskId::Authored(EventId([8; 32])),
                offer: Some(EventId([9; 32])),
                attempting: Vec::new(),
                results: 0,
            });
        }
        let large = prepare(binding, &session, pending, None).unwrap();
        assert_eq!(
            serde_json::to_vec(&first.receipt.encode().unwrap())
                .unwrap()
                .len(),
            serde_json::to_vec(&large.receipt.encode().unwrap())
                .unwrap()
                .len()
        );
        assert_ne!(
            first.receipt.work_fingerprint,
            large.receipt.work_fingerprint
        );
        assert_eq!(large.notice.pending.to_start.len(), 2000);
    }
}
