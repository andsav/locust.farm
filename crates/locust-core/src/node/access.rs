//! Grants and authorization: which goal a caller may read, write to or
//! decide for.
//!
//! A principal sees only the goals it takes or took part in; any other goal
//! does not exist for it (`NotFound`). A request that is not the caller's to
//! make is `Denied`; one that is the caller's but that no grant covers is
//! `AuthorizationRequired`. The owner acting on a principal's behalf is the
//! authorization, so grants are not consulted then.

use locust_proto::api::{ApiError, ErrorCode, Membership};
use locust_proto::engine::Entropy;
use locust_proto::id::{GoalId, PublicKey};
use locust_proto::store::Store;

use super::Node;
use super::callers::Actor;
use super::entry::Entry;

pub(super) fn not_found(message: &'static str) -> ApiError {
    ApiError::new(ErrorCode::NotFound, message)
}

pub(super) fn denied(message: &'static str) -> ApiError {
    ApiError::new(ErrorCode::Denied, message)
}

pub(super) fn conflict(message: &'static str) -> ApiError {
    ApiError::new(ErrorCode::Conflict, message)
}

pub(super) fn authorization_required(message: &'static str) -> ApiError {
    ApiError::new(ErrorCode::AuthorizationRequired, message)
}

const NO_GOAL: &str = "no such goal";

impl<S: Store, E: Entropy> Node<S, E> {
    /// The goal as `actor` may read it. The owner asking directly reads
    /// every goal; a principal only those it takes or took part in.
    pub(super) fn readable(&self, actor: &Actor, goal: &GoalId) -> Result<&Entry, ApiError> {
        let entry = self.goals.get(goal).ok_or_else(|| not_found(NO_GOAL))?;
        match actor.principal {
            Some(principal) if entry.membership(&principal).is_none() => Err(not_found(NO_GOAL)),
            _ => Ok(entry),
        }
    }

    /// The goal and the principal `actor` acts as in it, which must be a
    /// current member that has not left.
    pub(super) fn member(
        &self,
        actor: &Actor,
        goal: &GoalId,
    ) -> Result<(&Entry, PublicKey), ApiError> {
        let entry = self.readable(actor, goal)?;
        let principal = actor.principal()?;
        match entry.membership(&principal) {
            Some(Membership::Member) => Ok((entry, principal)),
            Some(Membership::Joining) => Err(ApiError::new(
                ErrorCode::Unavailable,
                "the coordinator's admission has not arrived yet",
            )),
            _ => Err(denied("the principal is not a current member of the goal")),
        }
    }

    /// The goal and the principal `actor` acts as, which must coordinate it
    /// and hold the `decide` grant.
    pub(super) fn coordinator(
        &self,
        actor: &Actor,
        goal: &GoalId,
    ) -> Result<(&Entry, PublicKey), ApiError> {
        let (entry, principal) = self.member(actor, goal)?;
        if entry.state().coordinator != Some(principal) {
            return Err(denied("only the goal's coordinator makes this request"));
        }
        if !actor.owner_act && !entry.local.grants(&principal).decide {
            return Err(authorization_required(
                "the principal has no grant to decide in this goal",
            ));
        }
        Ok((entry, principal))
    }

    /// Requires the daemon-wide `manage_goals` grant of the acting
    /// principal, which the owner's direct act stands in for.
    pub(super) fn manages_goals(&self, actor: &Actor) -> Result<PublicKey, ApiError> {
        let principal = actor.principal()?;
        let granted = self
            .principals
            .active(&principal)
            .is_some_and(|found| found.record.grants.manage_goals);
        if actor.owner_act || granted {
            Ok(principal)
        } else {
            Err(denied("the principal has no grant to manage goals"))
        }
    }
}
