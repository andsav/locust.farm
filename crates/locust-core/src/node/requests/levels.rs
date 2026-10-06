//! Owner-set local levels and single-task allowances.

use locust_proto::api::{
    Abilities, ApiError, ErrorCode, Level, Membership, Response, Stall, Stalled, WantedTask,
};
use locust_proto::engine::Entropy;
use locust_proto::event::TaskId;
use locust_proto::id::{GoalId, PublicKey};
use locust_proto::store::Store;

use super::{Plan, Planned};
use crate::node::Node;
use crate::node::access::{conflict, not_found};
use crate::node::callers::Actor;
use crate::node::commit::Tx;
use crate::node::entry::Entry;
use crate::node::local::{self, Allowance};

impl<S: Store, E: Entropy> Node<S, E> {
    pub(in crate::node) fn abilities(&self, entry: &Entry, agent: PublicKey) -> Abilities {
        let name = self
            .principals
            .get(&agent)
            .map(|principal| principal.record.name.clone())
            .unwrap_or_else(|| agent.to_string().chars().take(8).collect());
        let membership = entry.membership(&agent);
        let level = entry.local.level(&agent);
        let rules = entry.goal.abilities(agent, level, &entry.definitions);
        let roles = entry
            .state()
            .roles
            .iter()
            .filter_map(|(name, members)| members.contains(&agent).then_some(name.clone()))
            .collect();
        let mut allowed_tasks = Vec::new();
        let mut wanted_tasks = Vec::new();
        for ((task, principal), allowance) in &entry.local.allowances {
            if *principal != agent {
                continue;
            }
            let Some(found) = entry.state().tasks.get(task) else {
                continue;
            };
            let Some(round) = found.rounds.get(&found.current_round) else {
                continue;
            };
            if round.closed || round.completed || round.selected.is_some() {
                continue;
            }
            match allowance {
                Allowance::Allowed { round } if *round == found.current_round => {
                    allowed_tasks.push(*task)
                }
                Allowance::Wanted { since_ms } => {
                    let title = entry
                        .goal
                        .event(&found.created)
                        .and_then(|event| entry.text(&self.store, event, None));
                    wanted_tasks.push(WantedTask {
                        task: *task,
                        title,
                        since_ms: *since_ms,
                    });
                }
                _ => {}
            }
        }
        let claims = entry
            .claims
            .iter()
            .filter(|(attempt, claim)| {
                claim.principal == agent
                    && entry.state().attempts.get(attempt).is_some_and(|found| {
                        matches!(
                            found.status,
                            None | Some(locust_proto::event::AttemptStatus::Progress)
                        )
                    })
            })
            .map(|(attempt, claim)| claim.view(entry.id(), *attempt))
            .collect();
        Abilities {
            goal: entry.id(),
            agent,
            name,
            membership,
            level,
            host: entry.state().host,
            hosted_here: self.hosts(entry),
            roles,
            rules,
            allowed_tasks,
            wanted_tasks,
            claims,
        }
    }

    /// Desired steps this daemon should sign and cannot. The governance key
    /// is no agent and no member: its one stall here is a halt.
    pub(in crate::node) fn stalled(&self, entry: &Entry) -> Vec<Stalled> {
        let governance = entry.state().governance;
        entry
            .goal
            .evaluation()
            .desired_effects
            .values()
            .filter_map(|desired| {
                if entry.state().effects.contains_key(&desired.id) {
                    return None;
                }
                let runner = &desired.runner;
                let reason = if governance.as_ref() == Some(runner) {
                    if !self.hosts(entry) {
                        return None;
                    }
                    if entry.goal.next(runner).is_none() {
                        Stall::Halted
                    } else {
                        return None;
                    }
                } else if !self.principals.holds(runner) {
                    return None;
                } else if self.principals.active(runner).is_none() {
                    Stall::RunnerRevoked
                } else if entry.local.part.get(runner) == Some(&true) {
                    Stall::RunnerLeft
                } else if !entry.is_member(runner) {
                    Stall::RunnerNotMember
                } else if entry.goal.next(runner).is_none() {
                    Stall::Halted
                } else {
                    return None;
                };
                Some(Stalled {
                    effect: desired.id,
                    runner: desired.runner,
                    reason,
                })
            })
            .collect()
    }

    pub(super) fn level_set(
        &self,
        actor: &Actor,
        goal: GoalId,
        agent: PublicKey,
        level: Level,
    ) -> Plan {
        let _actor = self.local_agent(actor, agent)?;
        let entry = self
            .goals
            .get(&goal)
            .ok_or_else(|| not_found("no such goal"))?;
        if !matches!(
            entry.membership(&agent),
            Some(Membership::Joining | Membership::Member)
        ) {
            return Err(conflict("the agent has not joined this goal"));
        }
        let mut tx = Tx::none();
        if entry.local.levels.get(&agent) != Some(&level) {
            tx.local(local::level_write(&goal, &agent, &level))
                .touch(goal);
        }
        let mut abilities = self.abilities(entry, agent);
        abilities.level = level;
        for ability in &mut abilities.rules {
            ability.allowed = ability.eligible && level >= ability.needs;
        }
        Ok(Planned {
            response: Response::Abilities(abilities),
            tx,
        })
    }

    pub(super) fn task_allow(
        &self,
        actor: &Actor,
        goal: GoalId,
        agent: PublicKey,
        task: TaskId,
    ) -> Plan {
        let actor = self.local_agent(actor, agent)?;
        let entry = self.readable(&actor, &goal)?;
        if entry.membership(&agent) != Some(Membership::Member) {
            return Err(not_found("no current local member has that key"));
        }
        let found = entry
            .state()
            .tasks
            .get(&task)
            .ok_or_else(|| not_found("no such task"))?;
        let round = found
            .rounds
            .get(&found.current_round)
            .ok_or_else(|| not_found("no current task round"))?;
        if round.closed || round.completed || round.selected.is_some() {
            return Err(self.refused_error(
                entry,
                agent,
                &crate::node::access::Attempted::Resume { task },
                locust_proto::api::Why::State {
                    reason: "this task is closed, finished or picked".into(),
                },
            ));
        }
        let mut tx = Tx::none();
        let next = Allowance::Allowed {
            round: found.current_round,
        };
        if entry.local.allowances.get(&(task, agent)) != Some(&next) {
            tx.local(local::allowance_write(&goal, &task, &agent, &next))
                .touch(goal);
        }
        let mut abilities = self.abilities(entry, agent);
        abilities.wanted_tasks.retain(|wanted| wanted.task != task);
        if !abilities.allowed_tasks.contains(&task) {
            abilities.allowed_tasks.push(task);
            abilities.allowed_tasks.sort();
        }
        Ok(Planned {
            response: Response::Abilities(abilities),
            tx,
        })
    }

    pub(super) fn task_disallow(
        &self,
        actor: &Actor,
        goal: GoalId,
        agent: PublicKey,
        task: TaskId,
    ) -> Plan {
        let actor = self.local_agent(actor, agent)?;
        let entry = self.readable(&actor, &goal)?;
        let mut tx = Tx::none();
        let previous = entry.local.allowances.get(&(task, agent));
        if previous.is_some() {
            tx.local(local::allowance_delete(&goal, &task, &agent))
                .touch(goal);
        }
        let mut abilities = self.abilities(entry, agent);
        abilities.allowed_tasks.retain(|allowed| *allowed != task);
        abilities.wanted_tasks.retain(|wanted| wanted.task != task);
        Ok(Planned {
            response: Response::TaskDisallowed {
                abilities,
                changed: previous.is_some(),
                was_allowed: matches!(previous, Some(Allowance::Allowed { .. })),
            },
            tx,
        })
    }

    pub(super) fn note_task_want(
        &mut self,
        goal: GoalId,
        task: TaskId,
        agent: PublicKey,
        now_ms: u64,
    ) -> Result<(), ApiError> {
        let entry = self
            .goals
            .get(&goal)
            .ok_or_else(|| ApiError::new(ErrorCode::NotFound, "no such goal"))?;
        let current = entry
            .state()
            .tasks
            .get(&task)
            .map(|task| task.current_round);
        let existing = entry.local.allowances.get(&(task, agent));
        if matches!(existing, Some(Allowance::Wanted { .. }))
            || matches!((existing, current), (Some(Allowance::Allowed { round }), Some(current)) if *round == current)
        {
            return Ok(());
        }
        let mut tx = Tx::none();
        tx.local(local::allowance_write(
            &goal,
            &task,
            &agent,
            &Allowance::Wanted { since_ms: now_ms },
        ))
        .touch(goal);
        self.land(tx)
    }
}
