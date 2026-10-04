//! Durable transport delivery. Event projection and recipient records share one
//! transaction; network receipts never imply agent acknowledgment or execution.
use super::{Node, commit::Tx, records};
use locust_proto::engine::Entropy;
use locust_proto::event::{Context, EffectAction};
use locust_proto::id::{EffectId, EndpointId, GoalId, PublicKey};
use locust_proto::store::{LocalWrite, Space, Store, StoreError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Delivery {
    pub context: Context,
    pub action: String,
    pub endpoint: EndpointId,
    pub received: bool,
    pub delivered: bool,
    pub available: bool,
}

pub(super) fn subject(key: &[u8]) -> Result<(GoalId, EffectId, PublicKey), StoreError> {
    if key.len() != 1 + GoalId::LEN + EffectId::LEN + PublicKey::LEN || key[0] != b'd' {
        return Err(records::bad_key());
    }
    Ok((
        GoalId(records::part(key, 1).ok_or_else(records::bad_key)?),
        EffectId(records::part(key, 1 + GoalId::LEN).ok_or_else(records::bad_key)?),
        PublicKey(
            records::part(key, 1 + GoalId::LEN + EffectId::LEN).ok_or_else(records::bad_key)?,
        ),
    ))
}

pub(super) fn write(
    goal: GoalId,
    effect: EffectId,
    recipient: PublicKey,
    record: &Delivery,
) -> LocalWrite {
    records::put(
        Space::Pending,
        records::key(b'd', &[&goal.0, &effect.0, &recipient.0]),
        record,
    )
}

impl<S: Store, E: Entropy> Node<S, E> {
    /// Called after tentative event projection, before its single store commit.
    /// No delivery is exposed from tentative state. Old records survive lost
    /// authority so a recipient can distinguish withdrawal from disappearing work.
    pub(super) fn project_deliveries(&self, goal: GoalId, tx: &mut Tx) {
        let Some(entry) = self.goals.get(&goal) else {
            return;
        };
        let mut projected = entry.deliveries.clone();
        // Include explicit receipt writes already planned in this transaction.
        for item in &tx.commit.local {
            if let LocalWrite::Put {
                space: Space::Pending,
                key,
                value,
            } = item
                && let Ok((id, effect, recipient)) = subject(key)
                && id == goal
                && let Ok(record) = records::read::<Delivery>(value)
            {
                projected.insert((effect, recipient), record);
            }
        }
        for record in projected.values_mut() {
            record.available = false;
        }
        for effect in entry.state().effects.values() {
            for recipient in &effect.recipients {
                let Some(member) = entry
                    .state()
                    .members
                    .get(recipient)
                    .filter(|member| member.is_active())
                else {
                    continue;
                };
                let local = self.principals.active(recipient).is_some()
                    && self
                        .identity
                        .endpoint
                        .as_ref()
                        .is_some_and(|endpoint| endpoint.endpoint == member.endpoint);
                let record = projected
                    .entry((effect.id, *recipient))
                    .or_insert_with(|| Delivery {
                        context: effect.effect.context,
                        action: match effect.effect.action {
                            EffectAction::OpenTask { .. } => "open_task",
                            EffectAction::Offer { .. } => "offer",
                            EffectAction::RequestReview { .. } => "request_review",
                        }
                        .into(),
                        endpoint: member.endpoint,
                        received: false,
                        delivered: false,
                        available: true,
                    });
                if record.endpoint != member.endpoint {
                    record.endpoint = member.endpoint;
                    record.delivered = false;
                    record.received = false;
                }
                record.available = true;
                record.received |= local;
                record.delivered |= local;
            }
        }
        for ((effect, recipient), record) in projected {
            if entry.deliveries.get(&(effect, recipient)) != Some(&record) {
                tx.local(write(goal, effect, recipient, &record))
                    .touch(goal);
            }
        }
    }
}
