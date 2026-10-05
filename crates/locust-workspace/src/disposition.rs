//! Pure local checkout disposition shared by status and explicit update.
//! Observation does not grant process launch, signaling, reuse or cleanup rights.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOwnership {
    Unbound,
    /// The caller owns the bound session and requests an explicit work boundary.
    Caller,
    OtherActive,
    OtherInactive,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckoutFacts {
    pub session: SessionOwnership,
    pub inspection_known: bool,
    pub recovery_pending: bool,
    pub publication_pending: bool,
    pub publication_receipt_known: bool,
    pub dirty_paths: Vec<String>,
    pub untracked_paths: Vec<String>,
    pub path_conflicts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateBlocker {
    UnknownInspection,
    UnknownSessionOwnership,
    AnotherActiveSession,
    RecoveryPending,
    PathConflict { path: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckoutDisposition {
    pub session: SessionOwnership,
    pub dirty_paths: Vec<String>,
    pub untracked_paths: Vec<String>,
    pub update_blockers: Vec<UpdateBlocker>,
    pub update_allowed: bool,
    /// Reconcile the exact frozen request instead of recapturing it. This does
    /// not block an unrelated compatible file update or a separate publication.
    pub publication_requires_reconciliation: bool,
}

pub fn checkout_disposition(facts: CheckoutFacts) -> CheckoutDisposition {
    let mut update_blockers = Vec::new();
    if !facts.inspection_known {
        update_blockers.push(UpdateBlocker::UnknownInspection);
    }
    match facts.session {
        SessionOwnership::Unknown => update_blockers.push(UpdateBlocker::UnknownSessionOwnership),
        SessionOwnership::OtherActive => update_blockers.push(UpdateBlocker::AnotherActiveSession),
        _ => {}
    }
    if facts.recovery_pending {
        update_blockers.push(UpdateBlocker::RecoveryPending);
    }
    update_blockers.extend(
        facts
            .path_conflicts
            .into_iter()
            .map(|path| UpdateBlocker::PathConflict { path }),
    );
    CheckoutDisposition {
        session: facts.session,
        dirty_paths: facts.dirty_paths,
        untracked_paths: facts.untracked_paths,
        update_allowed: update_blockers.is_empty(),
        update_blockers,
        publication_requires_reconciliation: facts.publication_pending
            && !facts.publication_receipt_known,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compatible_dirty() -> CheckoutFacts {
        CheckoutFacts {
            session: SessionOwnership::Caller,
            inspection_known: true,
            recovery_pending: false,
            publication_pending: true,
            publication_receipt_known: false,
            dirty_paths: vec!["unpublished.rs".into()],
            untracked_paths: vec!["private.txt".into()],
            path_conflicts: vec![],
        }
    }

    #[test]
    fn compatible_dirty_edits_and_uncertain_publication_do_not_block_update() {
        let status = checkout_disposition(compatible_dirty());
        assert!(status.update_allowed);
        assert!(status.publication_requires_reconciliation);
        assert_eq!(status.dirty_paths, ["unpublished.rs"]);
        assert_eq!(status.untracked_paths, ["private.txt"]);
    }

    #[test]
    fn unknown_active_and_recovery_facts_block_even_with_a_publication_receipt() {
        let mut facts = compatible_dirty();
        facts.session = SessionOwnership::Unknown;
        facts.inspection_known = false;
        facts.recovery_pending = true;
        facts.publication_receipt_known = true;
        let status = checkout_disposition(facts);
        assert!(!status.update_allowed);
        assert!(!status.publication_requires_reconciliation);
        assert_eq!(
            status.update_blockers,
            [
                UpdateBlocker::UnknownInspection,
                UpdateBlocker::UnknownSessionOwnership,
                UpdateBlocker::RecoveryPending
            ]
        );
        let mut facts = compatible_dirty();
        facts.session = SessionOwnership::OtherActive;
        assert!(!checkout_disposition(facts).update_allowed);
        let mut facts = compatible_dirty();
        facts.path_conflicts.push("incoming.rs".into());
        assert!(!checkout_disposition(facts).update_allowed);
    }
}
