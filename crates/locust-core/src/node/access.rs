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
            Some(principal)
                if entry.membership(&principal) == Some(Membership::Refused)
                    && entry.goal.read_epoch(&principal).is_none() =>
            {
                Err(denied(
                    "the invitation was refused; join with a fresh invitation",
                ))
            }
            Some(principal) if entry.goal.read_epoch(&principal).is_none() => Err(ApiError::new(
                ErrorCode::Unavailable,
                "canonical admission has not arrived for this principal",
            )),
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
                "the host's admission has not arrived yet",
            )),
            _ => Err(denied("the principal is not a current member of the goal")),
        }
    }

    /// Whether this daemon holds the key that signs goal governance.
    pub(super) fn hosts(&self, entry: &Entry) -> bool {
        entry
            .state()
            .governance
            .is_some_and(|key| self.principals.holds(&key))
    }

    /// The hosted goal and its governance signer.
    pub(super) fn host(
        &self,
        actor: &Actor,
        goal: &GoalId,
    ) -> Result<(&Entry, PublicKey), ApiError> {
        let entry = self.readable(actor, goal)?;
        let principal = entry.state().governance.ok_or_else(|| not_found(NO_GOAL))?;
        if !self.hosts(entry) {
            return Err(denied(
                "this goal is hosted on another computer; its host decides",
            ));
        }
        if self.principals.active(&principal).is_none()
            || entry.local.part.get(&principal) == Some(&true)
        {
            return Err(denied(
                "the host agent is disconnected; nothing can sign for this goal",
            ));
        }
        Ok((entry, principal))
    }

    pub(super) fn require_grant(
        &self,
        actor: &Actor,
        entry: &Entry,
        allowed: bool,
    ) -> Result<(), ApiError> {
        let _ = entry;
        if actor.owner_act || allowed {
            Ok(())
        } else {
            Err(authorization_required(
                "this operation requires a local grant",
            ))
        }
    }

    /// Resolve an owner-named agent for a person's goal request.
    pub(super) fn local_agent(&self, actor: &Actor, agent: PublicKey) -> Result<Actor, ApiError> {
        let principal = self
            .principals
            .active(&agent)
            .ok_or_else(|| not_found("no active enrolled principal has that key"))?;
        if principal.record.author_only {
            return Err(denied("an authoring principal cannot act in goals"));
        }
        Ok(Actor {
            principal: Some(agent),
            owner_act: true,
            ..*actor
        })
    }
}
