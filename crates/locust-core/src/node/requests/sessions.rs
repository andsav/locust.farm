//! Adapter session records and their live claims.
use super::{Plan, Planned, answer};
use crate::node::{
    Node,
    access::{conflict, denied, not_found},
    callers::Actor,
    commit::Tx,
    sessions::{SessionEntry, Sessions},
};
use locust_proto::api::{Caller, Claim, Response, SessionRecord, SessionView};
use locust_proto::engine::Entropy;
use locust_proto::event::AttemptStatus;
use locust_proto::id::InstanceId;
use locust_proto::store::Store;

impl<S: Store, E: Entropy> Node<S, E> {
    fn session_claims(&self, instance: &InstanceId) -> Vec<Claim> {
        self.goals
            .iter()
            .flat_map(|(goal, entry)| {
                entry.claims.iter().filter_map(move |(id, claim)| {
                    let attempt = entry.state().attempts.get(id)?;
                    (claim.instance == *instance
                        && entry.goal.current_context(attempt.context.scope)
                            == Some(attempt.context)
                        && matches!(attempt.status, None | Some(AttemptStatus::Progress)))
                    .then(|| claim.view(*goal, *id))
                })
            })
            .collect()
    }

    fn session_view(&self, instance: &InstanceId, entry: &SessionEntry) -> Option<SessionView> {
        let (record, updated_ms) = entry.record.as_ref()?;
        Some(SessionView {
            instance: *instance,
            principal: entry.principal,
            record: record.clone(),
            updated_ms: *updated_ms,
            attached: self
                .conns
                .values()
                .any(|conn| conn.session == Some(*instance)),
            claims: self.session_claims(instance),
        })
    }

    pub(super) fn session_report(&self, actor: &Actor, record: SessionRecord, now: u64) -> Plan {
        let instance = actor.session()?;
        let principal = actor.principal()?;
        self.sessions.bind(&instance, &principal)?;
        let mut tx = Tx::none();
        tx.local(Sessions::write(
            &instance,
            &SessionEntry {
                principal,
                record: Some((record, now)),
            },
        ));
        Ok(Planned {
            response: Response::Done,
            tx,
        })
    }

    pub(super) fn session_show(&self, actor: &Actor, instance: Option<InstanceId>) -> Plan {
        let instance = match instance {
            Some(instance) => instance,
            None => actor.session()?,
        };
        let entry = self
            .sessions
            .get(&instance)
            .ok_or_else(|| not_found("no such session"))?;
        if actor
            .principal
            .is_some_and(|principal| principal != entry.principal)
        {
            return Err(not_found("no such session"));
        }
        answer(Response::Session(
            self.session_view(&instance, entry)
                .ok_or_else(|| not_found("the session has no record"))?,
        ))
    }

    pub(super) fn sessions_list(&self, actor: &Actor) -> Plan {
        answer(Response::Sessions(
            self.sessions
                .iter()
                .filter(|(_, entry)| {
                    actor
                        .principal
                        .is_none_or(|principal| principal == entry.principal)
                })
                .filter_map(|(instance, entry)| self.session_view(instance, entry))
                .collect(),
        ))
    }

    pub(super) fn session_drop(&self, actor: &Actor, instance: InstanceId) -> Plan {
        if !matches!(actor.caller, Caller::Owner) && actor.session != Some(instance) {
            return Err(denied("only the session itself or the owner may drop it"));
        }
        if let Some(entry) = self.sessions.get(&instance)
            && actor
                .principal
                .is_some_and(|principal| principal != entry.principal)
        {
            return Err(denied("the session belongs to another principal"));
        }
        if !self.session_claims(&instance).is_empty() {
            return Err(conflict("the session still holds an unfinished attempt"));
        }
        let mut tx = Tx::none();
        // Keep the principal binding: dropping a record never transfers a secret.
        if let Some(entry) = self.sessions.get(&instance) {
            tx.local(Sessions::write(
                &instance,
                &SessionEntry {
                    principal: entry.principal,
                    record: None,
                },
            ));
        }
        Ok(Planned {
            response: Response::Done,
            tx,
        })
    }
}
