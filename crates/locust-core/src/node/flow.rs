//! Deterministic materialization. Each signed effect and its durable recipient
//! outbox records commit together. Transport receipts, recipient acknowledgment,
//! and execution remain separate facts. See `delivery` for retry and inbox state.
use super::{Node, commit::Tx};
use locust_proto::api::ApiError;
use locust_proto::engine::Entropy;
use locust_proto::event::Body;
use locust_proto::id::GoalId;
use locust_proto::store::Store;
impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn drive_flow(&mut self, goal: GoalId) -> Result<(), ApiError> {
        if let Some(entry) = self.goals.get_mut(&goal) {
            entry.failed_effects.clear();
        }
        loop {
            let Some(entry) = self.goals.get(&goal) else {
                return Ok(());
            };
            let governance = entry.state().governance;
            let next = entry
                .goal
                .evaluation()
                .desired_effects
                .values()
                .find(|desired| {
                    let runner = &desired.runner;
                    let can_sign_here = if governance.as_ref() == Some(runner) {
                        self.hosts(entry)
                    } else {
                        entry.local.part.get(runner) != Some(&true)
                            && entry.is_member(runner)
                            && self.principals.active(runner).is_some()
                    };
                    !entry.state().effects.contains_key(&desired.id)
                        && !entry.failed_effects.contains(&desired.id)
                        && can_sign_here
                        && entry.goal.next(runner).is_some()
                })
                .cloned();
            let Some(desired) = next else {
                return Ok(());
            };
            let mut tx = Tx::none();
            // Derived actions carry no scheduling clock. Their identity and
            // authority depend on the authenticated trigger and pinned rules.
            let result = self
                .author_alone(
                    entry,
                    &desired.runner,
                    Body::EffectMaterialized {
                        effect: desired.effect,
                    },
                    &mut tx,
                )
                .and_then(|_| self.land_once(tx));
            if let Err(error) = result {
                // A failed store still stops the node. An impossible effect
                // must not prevent unrelated work, joining or startup.
                if self.failed {
                    return Err(error);
                }
                self.entry_mut(goal).failed_effects.insert(desired.id);
            }
        }
    }
}
