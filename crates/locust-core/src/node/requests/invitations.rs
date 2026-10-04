//! Local invitation issuance and durable intent to join a remote goal.

use locust_proto::api::{
    ApiError, Caller, ErrorCode, InvitationState, InvitationSummary, Membership, Response,
};
use locust_proto::engine::Entropy;
use locust_proto::id::{BlobHash, EndpointId, GoalId, PublicKey};
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
    pub goal_title: Option<String>,
    pub administrator: PublicKey,
    pub created_ms: u64,
    pub expires_ms: Option<u64>,
    pub revoked_ms: Option<u64>,
    pub redeemed: Option<(PublicKey, EndpointId)>,
    pub redeemed_ms: Option<u64>,
}

impl InviteRecord {
    fn summary(&self, digest: &[u8], now_ms: u64) -> InvitationSummary {
        let state = if self.redeemed.is_some() {
            InvitationState::Redeemed
        } else if self.revoked_ms.is_some() {
            InvitationState::Revoked
        } else if self.expires_ms.is_some_and(|expires| expires <= now_ms) {
            InvitationState::Expired
        } else {
            InvitationState::Pending
        };
        InvitationSummary {
            invitation: digest.iter().map(|byte| format!("{byte:02x}")).collect(),
            goal: self.goal,
            goal_title: self.goal_title.clone(),
            administrator: self.administrator,
            created_ms: self.created_ms,
            expires_ms: self.expires_ms,
            state,
            revoked_ms: self.revoked_ms,
            redeemed_ms: self.redeemed_ms,
            redeemed_by: self.redeemed.map(|(member, _)| member),
            redeemed_endpoint: self.redeemed.map(|(_, endpoint)| endpoint),
        }
    }
}

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn invitation_inspect(&self, ticket: Ticket, now_ms: u64) -> Plan {
        let invitation = Invitation::from_ticket(ticket.as_str()).map_err(invite_error)?;
        answer(Response::InvitationInspected {
            preview: invitation.preview(now_ms).map_err(invite_error)?,
        })
    }

    pub(super) fn goal_invitations(&self, actor: &Actor, goal: GoalId, now_ms: u64) -> Plan {
        if actor.principal.is_some() {
            self.administrator(actor, &goal)?;
        } else {
            self.readable(actor, &goal)?;
        }
        let mut invitations = Vec::new();
        for (digest, bytes) in self.store.scan(Space::Invite, &[])? {
            let invitation: InviteRecord = records::read(&bytes)?;
            if invitation.goal == goal {
                invitations.push(invitation.summary(&digest, now_ms));
            }
        }
        invitations
            .sort_by(|a, b| (a.created_ms, &a.invitation).cmp(&(b.created_ms, &b.invitation)));
        answer(Response::Invitations { invitations })
    }

    pub(super) fn invitation_revoke(
        &self,
        actor: &Actor,
        goal: GoalId,
        invitation: String,
        now_ms: u64,
    ) -> Plan {
        if actor.caller != Caller::Owner {
            return Err(denied("invitation revocation is the owner's decision"));
        }
        self.readable(actor, &goal)?;
        let digest = invitation.parse::<BlobHash>().map_err(|_| {
            ApiError::new(
                ErrorCode::Invalid,
                "invitation requires its full inventory identifier",
            )
        })?;
        let bytes = self
            .store
            .get(Space::Invite, &digest.0)?
            .ok_or_else(|| crate::node::access::not_found("no such invitation in this goal"))?;
        let mut record: InviteRecord = records::read(&bytes)?;
        if record.goal != goal {
            return Err(crate::node::access::not_found(
                "no such invitation in this goal",
            ));
        }
        if record.redeemed.is_some() {
            return Err(conflict(
                "this invitation was redeemed; use member remove to end membership; received copies cannot be retracted",
            ));
        }
        if record.revoked_ms.is_some() {
            return answer(Response::InvitationRevoked {
                invitation: record.summary(&digest.0, now_ms),
            });
        }
        record.revoked_ms = Some(now_ms);
        let mut tx = Tx::none();
        tx.local(records::put(Space::Invite, digest.0.to_vec(), &record));
        Ok(Planned {
            response: Response::InvitationRevoked {
                invitation: record.summary(&digest.0, now_ms),
            },
            tx,
        })
    }

    /// The person selects an existing local principal and confirms the exact
    /// verified ticket. This never writes local grants or a workspace binding.
    pub(super) fn invitation_join(
        &self,
        actor: &Actor,
        principal: PublicKey,
        ticket: Ticket,
        review: String,
        now_ms: u64,
    ) -> Plan {
        if actor.caller != Caller::Owner {
            return Err(denied(
                "reviewed invitation joining is the owner's decision",
            ));
        }
        let participant = self.principals.active(&principal).ok_or_else(|| {
            crate::node::access::not_found("no active enrolled principal has that key")
        })?;
        if participant.record.author_only {
            return Err(denied("an authoring principal cannot join goals"));
        }
        let invitation = Invitation::from_ticket(ticket.as_str()).map_err(invite_error)?;
        if invitation.preview(now_ms).map_err(invite_error)?.review != review {
            return Err(conflict(
                "the reviewed invitation differs; inspect this exact ticket and confirm its review identifier",
            ));
        }
        self.goal_join(
            &Actor {
                principal: Some(principal),
                owner_act: true,
                ..*actor
            },
            ticket,
            now_ms,
        )
    }

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
        let goal_title = self.title(entry, Some(&administrator));
        let mut invitation = Invitation::signed(
            goal,
            goal_title.clone(),
            own.endpoint,
            own.hints.iter().take(MAX_HINTS).cloned().collect(),
            secret,
            expires_ms,
            self.signer(&administrator)?,
        )
        .map_err(invite_error)?;
        invitation.publication = entry
            .state()
            .publication
            .as_ref()
            .map(
                |(event, publication)| locust_proto::api::InvitationPublication {
                    event: *event,
                    publication: publication.clone(),
                },
            );
        invitation
            .sign(self.signer(&administrator)?)
            .map_err(invite_error)?;
        let ticket = invitation.to_ticket().map_err(invite_error)?;
        let mut tx = Tx::none();
        tx.local(records::put(
            Space::Invite,
            secret.digest().to_vec(),
            &InviteRecord {
                goal,
                goal_title,
                administrator,
                created_ms: now_ms,
                expires_ms,
                revoked_ms: None,
                redeemed: None,
                redeemed_ms: None,
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
                if !publication_matches(&entry.goal, invitation.publication.as_ref()) {
                    return Err(conflict(
                        "the advertised publication policy is not effective in the held goal",
                    ));
                }
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
                    return Err(denied(
                        "the inviter refused this invitation; request a fresh invitation from the administrator",
                    ));
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
        if invitation
            .expires_ms
            .is_some_and(|expires| expires <= now_ms)
        {
            return Err(denied(
                "the invitation has expired; request a fresh invitation from the administrator",
            ));
        }
        if invitation.endpoint == own {
            if !self.goals.get(&goal).is_some_and(|entry| {
                publication_matches(&entry.goal, invitation.publication.as_ref())
            }) {
                return Err(conflict(
                    "the advertised publication policy is not effective in the held goal",
                ));
            }
            let request = locust_proto::invite::JoinRequest::sign(
                goal,
                own,
                invitation.secret,
                self.signer(&principal)?,
            );
            let mut tx = self
                .plan_join(&own, &request, now_ms, Some(actor))
                .map_err(|_| denied("the inviter refused this invitation; it may be revoked, expired or used; request a fresh invitation from the administrator"))?;
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
                publication: invitation.publication.clone(),
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
    let code = match error {
        InviteError::UnsupportedVersion(_) => ErrorCode::UnsupportedVersion,
        InviteError::InvalidSignature => ErrorCode::Denied,
        _ => ErrorCode::Invalid,
    };
    ApiError::new(code, error.to_string())
}

/// An invitation may name an older policy, but that exact signed event must
/// exist in effective history before admission is presented as reconciled.
pub(in crate::node) fn publication_matches(
    goal: &crate::goal::Goal,
    advertised: Option<&locust_proto::api::InvitationPublication>,
) -> bool {
    advertised.is_none_or(|advertised| {
        goal.standing(&advertised.event) == Some(crate::goal::Standing::Effective)
            && goal.event(&advertised.event).is_some_and(|event| {
                matches!(&event.header().body, locust_proto::event::Body::PublicationSet(publication) if publication == &advertised.publication)
            })
    })
}
