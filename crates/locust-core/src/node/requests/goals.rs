//! Goals and membership: founding, status, grants and the workspace binding.

use locust_proto::api::{
    ApiError, ErrorCode, GoalGrants, GoalStatus, MemberView, Response, WorkspaceBinding,
};
use locust_proto::crypto::ContentKey;
use locust_proto::engine::Entropy;
use locust_proto::event::{Body, Genesis};
use locust_proto::id::{GoalId, PublicKey};
use locust_proto::store::Store;

use super::{Plan, Planned, answer};
use crate::node::Node;
use crate::node::access::not_found;
use crate::node::authoring::{Place, sign_at};
use crate::node::callers::Actor;
use crate::node::commit::Tx;
use crate::node::entry::key_write;
use crate::node::local;

impl<S: Store, E: Entropy> Node<S, E> {
    /// This daemon's endpoint, which admissions and invitations name.
    pub(super) fn own_endpoint(&self) -> Result<&crate::node::identity::EndpointRecord, ApiError> {
        self.identity.endpoint.as_ref().ok_or_else(|| {
            ApiError::new(
                ErrorCode::Unavailable,
                "the transport has not reported this daemon's endpoint yet",
            )
        })
    }

    /// `goal.create`: one commit holds the genesis signed by the creating
    /// principal with the sealed title as its payload, the creator's
    /// self-admission bound to this daemon's endpoint, the epoch-0 key and
    /// the creator's `decide` grant.
    pub(super) fn goal_create(&self, actor: &Actor, title: String, now_ms: u64) -> Plan {
        let creator = self.manages_goals(actor)?;
        let endpoint = self.own_endpoint()?.endpoint;
        let signer = self.signer(&creator)?;
        let genesis = Genesis {
            owner: creator,
            coordinator: creator,
            salt: self.random(),
        };
        let goal = genesis.goal_id();
        let key = ContentKey(self.random());

        let mut tx = Tx::none();
        let founding = sign_at(
            goal,
            signer,
            Place {
                seq: 0,
                prev: None,
                anchor: None,
                epoch: 0,
            },
            Body::Genesis(genesis),
            Some((&key, &title)),
            now_ms,
            &mut tx,
        )?;
        sign_at(
            goal,
            signer,
            Place {
                seq: 1,
                prev: Some(founding.id()),
                anchor: Some(founding.id()),
                epoch: 0,
            },
            Body::MemberAdmitted {
                member: creator,
                endpoint,
            },
            None,
            now_ms,
            &mut tx,
        )?;
        let grants = GoalGrants {
            decide: true,
            ..GoalGrants::default()
        };
        tx.local(key_write(&goal, 0, &key))
            .local(local::title_write(&goal, &title))
            .local(local::grants_write(&goal, &creator, &grants))
            .local(local::part_write(&goal, &creator, false));
        Ok(Planned {
            response: Response::GoalCreated { goal },
            tx,
        })
    }

    /// `goal.status`: the goal as this daemon holds it, with the parts that
    /// are about the calling principal.
    pub(super) fn goal_status(&self, actor: &Actor, goal: GoalId) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let state = entry.state();
        // Before any history has arrived the ticket's word is all there is.
        let coordinator = state
            .coordinator
            .or_else(|| {
                entry
                    .local
                    .joins
                    .values()
                    .next()
                    .map(|join| join.coordinator)
            })
            .ok_or_else(|| not_found("no such goal"))?;
        let own = self
            .identity
            .endpoint
            .as_ref()
            .map(|record| record.endpoint);
        answer(Response::GoalStatus(GoalStatus {
            goal,
            title: self.title(entry, actor.principal.as_ref()),
            coordinator,
            decision_head: state.head,
            members: state
                .members
                .iter()
                .map(|(member, endpoint)| MemberView {
                    member: *member,
                    endpoint: *endpoint,
                    local: self.principals.holds(member),
                })
                .collect(),
            halted: entry.halted(),
            head: state.accepted_head(),
            workspace: actor
                .principal
                .and_then(|principal| entry.local.workspace.get(&principal).cloned()),
            grants: actor
                .principal
                .map(|principal| entry.local.grants(&principal))
                .unwrap_or_default(),
            peers: state
                .endpoints
                .keys()
                .filter(|endpoint| Some(**endpoint) != own)
                .map(|endpoint| self.peer_view(endpoint))
                .collect(),
        }))
    }

    /// `goal.grant`: the owner replaces one local principal's standing
    /// grants in a goal.
    pub(super) fn goal_grant(
        &self,
        actor: &Actor,
        goal: GoalId,
        agent: PublicKey,
        grants: GoalGrants,
    ) -> Plan {
        self.readable(actor, &goal)?;
        if self.principals.get(&agent).is_none() {
            return Err(not_found("no enrolled principal has that key"));
        }
        let mut tx = Tx::none();
        tx.local(local::grants_write(&goal, &agent, &grants))
            .touch(goal);
        Ok(Planned {
            response: Response::Done,
            tx,
        })
    }

    /// `workspace.set`: where the calling principal's files for the goal
    /// live. Local only.
    pub(super) fn workspace_set(
        &self,
        actor: &Actor,
        goal: GoalId,
        binding: WorkspaceBinding,
    ) -> Plan {
        self.readable(actor, &goal)?;
        let principal = actor.principal()?;
        let mut tx = Tx::none();
        tx.local(local::workspace_write(&goal, &principal, &binding))
            .touch(goal);
        Ok(Planned {
            response: Response::Done,
            tx,
        })
    }
}

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn goal_leave(&self, actor: &Actor, goal: GoalId, now: u64) -> Plan {
        self.manages_goals(actor)?;
        let (entry, principal) = self.member(actor, &goal)?;
        if entry.state().coordinator == Some(principal) {
            return Err(crate::node::access::conflict(
                "the coordinator cannot leave before authority handoff",
            ));
        }
        let mut tx = Tx::none();
        let event = self.author(entry, &principal, Body::LeaveRequested, None, now, &mut tx)?;
        tx.local(local::part_write(&goal, &principal, true));
        super::tasks::recorded(event, tx)
    }

    pub(super) fn member_remove(
        &self,
        actor: &Actor,
        goal: GoalId,
        member: PublicKey,
        now: u64,
    ) -> Plan {
        let (entry, coordinator) = self.coordinator(actor, &goal)?;
        if member == coordinator {
            return Err(crate::node::access::conflict(
                "the coordinator cannot remove itself before authority handoff",
            ));
        }
        if !entry.is_member(&member) {
            return Err(crate::node::access::conflict(
                "the principal is not a member",
            ));
        }
        let mut place = self.next_place(entry, &coordinator)?;
        place.epoch = place
            .epoch
            .checked_add(1)
            .ok_or_else(|| crate::node::access::conflict("the key epoch is exhausted"))?;
        let last_accepted = entry
            .goal
            .points(&member)
            .iter()
            .filter(|point| {
                entry
                    .goal
                    .standing(&point.id)
                    .is_some_and(|standing| standing.is_effective())
            })
            .max_by_key(|point| point.seq)
            .copied();
        let key = ContentKey(self.random());
        let mut tx = Tx::none();
        let event = sign_at(
            goal,
            self.signer(&coordinator)?,
            place,
            Body::MemberRemoved {
                member,
                last_accepted,
            },
            Some((&key, "")),
            now,
            &mut tx,
        )?;
        tx.local(key_write(&goal, place.epoch, &key));
        super::tasks::recorded(event.id(), tx)
    }
}
