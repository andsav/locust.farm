//! Principals inspect their own permissions; only the owner can change them.

use locust_proto::api::{
    AttentionEntry, Caller, GoalPermission, GoalPermissions, Membership, Response,
    TaskAuthorization,
};
use locust_proto::engine::Entropy;
use locust_proto::event::TaskId;
use locust_proto::id::{GoalId, PublicKey};
use locust_proto::store::Store;

use super::{Plan, Planned, answer};
use crate::node::Node;
use crate::node::access::{denied, not_found};
use crate::node::callers::Actor;
use crate::node::commit::Tx;
use crate::node::local;

impl<S: Store, E: Entropy> Node<S, E> {
    fn permission_view(
        &self,
        actor: &Actor,
        goal: GoalId,
        agent: PublicKey,
    ) -> Result<GoalPermissions, locust_proto::api::ApiError> {
        if !(actor.caller == Caller::Owner && actor.principal.is_none())
            && actor.principal != Some(agent)
        {
            return Err(denied(
                "only the owner can inspect another principal's local permissions",
            ));
        }
        let entry = self.readable(actor, &goal)?;
        let principal = self
            .principals
            .get(&agent)
            .ok_or_else(|| not_found("no enrolled principal has that key"))?;
        if principal.record.author_only {
            return Err(denied("an authoring principal cannot act in goals"));
        }
        Ok(GoalPermissions {
            goal,
            agent,
            name: principal.record.name.clone(),
            revoked: principal.record.revoked,
            membership: entry.membership(&agent),
            grants: entry.local.grants(&agent),
            task_authorizations: entry
                .local
                .authorized
                .iter()
                .filter(|((_, principal), _)| *principal == agent)
                .map(|((round, _), authorization)| {
                    let task = entry
                        .state()
                        .tasks
                        .values()
                        .find(|task| task.rounds.contains_key(round));
                    TaskAuthorization {
                        task: task.map(|task| task.id),
                        round: *round,
                        current: task.is_some_and(|task| task.current_round == *round),
                        takeover: authorization.takeover,
                    }
                })
                .collect(),
        })
    }

    pub(super) fn permissions(&self, actor: &Actor, goal: GoalId, agent: PublicKey) -> Plan {
        answer(Response::Permissions(
            self.permission_view(actor, goal, agent)?,
        ))
    }

    pub(super) fn permission_change(
        &self,
        actor: &Actor,
        goal: GoalId,
        agent: PublicKey,
        permissions: Vec<GoalPermission>,
        allowed: bool,
    ) -> Plan {
        require_owner(actor)?;
        let mut view = self.permission_view(actor, goal, agent)?;
        for permission in permissions {
            permission.set(&mut view.grants, allowed);
        }
        let mut tx = Tx::none();
        tx.local(local::grants_write(&goal, &agent, &view.grants))
            .touch(goal);
        Ok(Planned {
            response: Response::Permissions(view),
            tx,
        })
    }

    pub(super) fn inbox(&self, actor: &Actor) -> Plan {
        if actor.caller != Caller::Owner || actor.principal.is_some() {
            return Err(denied("only the owner reads the local attention inbox"));
        }
        let mut attention = Vec::new();
        for (goal, entry) in &self.goals {
            for principal in self
                .principals
                .iter()
                .filter(|principal| !principal.record.revoked)
            {
                let agent = principal.key.public();
                if entry.membership(&agent) != Some(Membership::Member) {
                    continue;
                }
                let participant = Actor {
                    caller: Caller::Agent(agent),
                    principal: Some(agent),
                    owner_act: false,
                    session: None,
                };
                let pending = self.pending_work(entry, &participant);
                let has_work = !pending.to_authorize.is_empty()
                    || !pending.to_start.is_empty()
                    || !pending.held_elsewhere.is_empty()
                    || !pending.to_acknowledge.is_empty()
                    || !pending.to_review.is_empty()
                    || pending
                        .deliveries
                        .iter()
                        .any(|delivery| delivery.available && !delivery.acknowledged);
                if !has_work && entry.halted().is_none() {
                    continue;
                }
                attention.push(AttentionEntry {
                    goal: *goal,
                    title: self.title(entry, None),
                    agent,
                    name: principal.record.name.clone(),
                    halted: entry.halted(),
                    grants: entry.local.grants(&agent),
                    pending,
                    tasks: entry
                        .state()
                        .tasks
                        .values()
                        .map(|task| self.task_view(entry, task, None))
                        .collect(),
                });
            }
        }
        answer(Response::Inbox(attention))
    }

    pub(super) fn permission_task_revoke(
        &self,
        actor: &Actor,
        goal: GoalId,
        agent: PublicKey,
        task: TaskId,
    ) -> Plan {
        require_owner(actor)?;
        let mut view = self.permission_view(actor, goal, agent)?;
        let entry = self.readable(actor, &goal)?;
        let task = entry
            .state()
            .tasks
            .get(&task)
            .ok_or_else(|| not_found("no such task"))?;
        let mut tx = Tx::none();
        for round in task.rounds.keys() {
            tx.local(local::authorization_delete(&goal, round, &agent));
        }
        tx.touch(goal);
        view.task_authorizations
            .retain(|authorization| !task.rounds.contains_key(&authorization.round));
        Ok(Planned {
            response: Response::Permissions(view),
            tx,
        })
    }

    pub(super) fn permission_task_allow(
        &self,
        actor: &Actor,
        goal: GoalId,
        agent: PublicKey,
        task: TaskId,
        takeover: bool,
    ) -> Plan {
        require_owner(actor)?;
        let mut view = self.permission_view(actor, goal, agent)?;
        let entry = self.readable(actor, &goal)?;
        if !entry.is_member(&agent) || self.principals.active(&agent).is_none() {
            return Err(not_found("no active local member has that key"));
        }
        let found = entry
            .state()
            .tasks
            .get(&task)
            .ok_or_else(|| not_found("no such task"))?;
        let round = found.current_round;
        let takeover = takeover
            || entry
                .local
                .authorized
                .get(&(round, agent))
                .is_some_and(|authorization| authorization.takeover);
        let authorization = TaskAuthorization {
            task: Some(task),
            round,
            current: true,
            takeover,
        };
        view.task_authorizations
            .retain(|authorization| authorization.round != round);
        view.task_authorizations.push(authorization);
        view.task_authorizations
            .sort_by_key(|authorization| authorization.round);
        let mut tx = Tx::none();
        tx.local(local::authorization_write(
            &goal,
            &round,
            &agent,
            &local::Authorization { takeover },
        ))
        .touch(goal);
        Ok(Planned {
            response: Response::Permissions(view),
            tx,
        })
    }
}

fn require_owner(actor: &Actor) -> Result<(), locust_proto::api::ApiError> {
    if actor.caller != Caller::Owner || actor.principal.is_some() {
        return Err(denied("only the owner changes local goal permissions"));
    }
    Ok(())
}
