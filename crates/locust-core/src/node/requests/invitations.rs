//! Local invitation issuance and durable intent to join a remote goal.

use locust_proto::api::{ApiError, ErrorCode, Membership, Response};
use locust_proto::engine::Entropy;
use locust_proto::id::{EndpointId, GoalId, PublicKey};
use locust_proto::invite::{Invitation, InviteError, InviteSecret, MAX_HINTS, Ticket};
use locust_proto::store::{Space, Store};
use serde::{Deserialize, Serialize};

use super::{Plan, Planned, answer};
use crate::node::Node;
use crate::node::access::{conflict, denied};
use crate::node::callers::Actor;
use crate::node::commit::Tx;
use crate::node::{local, records};

/// The lookup record keeps only the digest. An explicitly idempotent invite
/// also retains the issued ticket in the protected response replay cache.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::node) struct InviteRecord {
    pub goal: GoalId,
    pub coordinator: PublicKey,
    pub expires_ms: Option<u64>,
    pub redeemed: Option<(PublicKey, EndpointId)>,
}

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn goal_invite(
        &self,
        actor: &Actor,
        goal: GoalId,
        expires_ms: Option<u64>,
        _now_ms: u64,
    ) -> Plan {
        self.manages_goals(actor)?;
        let (entry, coordinator) = self.coordinator(actor, &goal)?;
        if entry.goal.halt().is_some() {
            return Err(ApiError::new(
                ErrorCode::Halted,
                "the goal's authority is halted",
            ));
        }
        let own = self.own_endpoint()?;
        let secret = InviteSecret(self.random());
        let invitation = Invitation {
            version: locust_proto::PROTOCOL_VERSION,
            goal,
            coordinator,
            endpoint: own.endpoint,
            hints: own.hints.iter().take(MAX_HINTS).cloned().collect(),
            secret,
            expires_ms,
        };
        let ticket = invitation.to_ticket().map_err(invite_error)?;
        let mut tx = Tx::none();
        tx.local(records::put(
            Space::Invite,
            secret.digest().to_vec(),
            &InviteRecord {
                goal,
                coordinator,
                expires_ms,
                redeemed: None,
            },
        ));
        Ok(Planned {
            response: Response::Invited { ticket },
            tx,
        })
    }

    pub(super) fn goal_join(&self, actor: &Actor, ticket: Ticket, _now_ms: u64) -> Plan {
        let principal = self.manages_goals(actor)?;
        self.own_endpoint()?;
        let invitation = Invitation::from_ticket(ticket.as_str()).map_err(invite_error)?;
        let goal = invitation.goal;
        if let Some(entry) = self.goals.get(&goal) {
            if entry
                .state()
                .coordinator
                .is_some_and(|key| key != invitation.coordinator)
            {
                return Err(conflict(
                    "the ticket's coordinator differs from the held goal",
                ));
            }
            if entry.membership(&principal) == Some(Membership::Member) {
                return answer(Response::Joined {
                    goal,
                    coordinator: invitation.coordinator,
                    membership: Membership::Member,
                });
            }
            if let Some(join) = entry.local.joins.get(&principal) {
                let same = join.secret == invitation.secret
                    && join.endpoint == invitation.endpoint
                    && join.coordinator == invitation.coordinator;
                if same && join.refused {
                    return Err(denied("the inviter refused this invitation"));
                }
                if same {
                    return answer(Response::Joined {
                        goal,
                        coordinator: invitation.coordinator,
                        membership: Membership::Joining,
                    });
                }
                if !join.refused {
                    return Err(conflict(
                        "another invitation is already being redeemed for this goal",
                    ));
                }
            }
        }
        let mut tx = Tx::none();
        tx.local(local::join_write(
            &goal,
            &principal,
            &local::JoinRecord {
                coordinator: invitation.coordinator,
                endpoint: invitation.endpoint,
                hints: invitation.hints.clone(),
                secret: invitation.secret,
                refused: false,
            },
        ))
        .local(local::part_write(&goal, &principal, false))
        .local(records::put(
            Space::Peer,
            invitation.endpoint.0.to_vec(),
            &invitation.hints,
        ))
        .touch(goal);
        Ok(Planned {
            response: Response::Joined {
                goal,
                coordinator: invitation.coordinator,
                membership: Membership::Joining,
            },
            tx,
        })
    }
}

fn invite_error(error: InviteError) -> ApiError {
    let code = if matches!(error, InviteError::UnsupportedVersion(_)) {
        ErrorCode::UnsupportedVersion
    } else {
        ErrorCode::Invalid
    };
    ApiError::new(code, error.to_string())
}
