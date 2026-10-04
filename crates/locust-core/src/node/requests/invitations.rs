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
    pub administrator: PublicKey,
    pub expires_ms: Option<u64>,
    pub redeemed: Option<(PublicKey, EndpointId)>,
}

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn goal_invite(
        &self,
        actor: &Actor,
        goal: GoalId,
        expires_ms: Option<u64>,
        now_ms: u64,
    ) -> Plan {
        if expires_ms.is_some_and(|expires| expires <= now_ms) {
            return Err(crate::node::access::conflict(
                "the invitation has already expired",
            ));
        }
        self.manages_goals(actor)?;
        let (entry, administrator) = self.administrator(actor, &goal)?;
        if entry.goal.evaluation().admin_halt.as_ref().is_some() {
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
            administrator,
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
                administrator,
                expires_ms,
                redeemed: None,
            },
        ));
        Ok(Planned {
            response: Response::Invited { ticket },
            tx,
        })
    }

    pub(super) fn goal_join(&self, actor: &Actor, ticket: Ticket, now_ms: u64) -> Plan {
        let principal = self.manages_goals(actor)?;
        let own = self.own_endpoint()?.endpoint;
        let invitation = Invitation::from_ticket(ticket.as_str()).map_err(invite_error)?;
        if invitation
            .expires_ms
            .is_some_and(|expires| expires <= now_ms)
        {
            return Err(denied("the invitation has expired"));
        }
        let goal = invitation.goal;
        if let Some(entry) = self.goals.get(&goal) {
            if entry
                .state()
                .administrator
                .is_some_and(|key| key != invitation.administrator)
            {
                return Err(conflict(
                    "the ticket's administrator differs from the held goal",
                ));
            }
            if entry
                .state()
                .members
                .get(&invitation.administrator)
                .is_some_and(|member| member.endpoint != invitation.endpoint)
            {
                return Err(conflict(
                    "the ticket's endpoint differs from the held administrator admission",
                ));
            }
            if entry.membership(&principal) == Some(Membership::Left) && entry.is_member(&principal)
            {
                return Err(conflict(
                    "the departure must be acknowledged by removal before rejoining",
                ));
            }
            if entry.membership(&principal) == Some(Membership::Member) {
                return answer(Response::Joined {
                    goal,
                    administrator: invitation.administrator,
                    membership: Membership::Member,
                });
            }
            if let Some(join) = entry.local.joins.get(&principal) {
                let same = join.secret == invitation.secret
                    && join.endpoint == invitation.endpoint
                    && join.administrator == invitation.administrator;
                if same && join.refused {
                    return Err(denied("the inviter refused this invitation"));
                }
                if same {
                    return answer(Response::Joined {
                        goal,
                        administrator: invitation.administrator,
                        membership: Membership::Joining,
                    });
                }
                if !join.refused {
                    return Err(conflict(
                        "another invitation is being redeemed; check status and retry a fresh invitation after refusal",
                    ));
                }
            }
        }
        if invitation.endpoint == own {
            let request = locust_proto::invite::JoinRequest::sign(
                goal,
                own,
                invitation.secret,
                self.signer(&principal)?,
            );
            let mut tx = self
                .plan_join(&own, &request, now_ms)
                .map_err(|_| denied("the inviter refused this invitation"))?;
            tx.local(local::part_write(&goal, &principal, false))
                .touch(goal);
            return Ok(Planned {
                response: Response::Joined {
                    goal,
                    administrator: invitation.administrator,
                    membership: Membership::Member,
                },
                tx,
            });
        }
        let mut tx = Tx::none();
        tx.local(local::join_write(
            &goal,
            &principal,
            &local::JoinRecord {
                administrator: invitation.administrator,
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
                administrator: invitation.administrator,
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
