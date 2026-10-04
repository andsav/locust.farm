//! Shared documents use the same scoped contribution and selection rules.
use super::tasks::recorded;
use super::{Plan, answer};
use crate::node::Node;
use crate::node::access::conflict;
use crate::node::callers::Actor;
use crate::node::commit::Tx;
use locust_proto::api::{DocView, Response};
use locust_proto::engine::Entropy;
use locust_proto::event::{Body, Doc, Scope};
use locust_proto::id::{EventId, GoalId};
use locust_proto::store::Store;
impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn doc_read(&self, actor: &Actor, goal: GoalId, doc: Doc) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let document = entry.state().documents.get(&doc);
        let selected = document.and_then(|document| document.selected);
        answer(Response::Doc(DocView {
            doc,
            selected,
            text: selected
                .and_then(|revision| entry.goal.event(&revision))
                .and_then(|event| entry.text(&self.store, event, actor.principal.as_ref())),
            proposals: document
                .into_iter()
                .flat_map(|document| document.revisions.iter().copied())
                .filter(|id| Some(*id) != selected)
                .collect(),
        }))
    }
    pub(super) fn doc_revise(
        &self,
        actor: &Actor,
        goal: GoalId,
        doc: Doc,
        base: Option<EventId>,
        text: String,
        now: u64,
    ) -> Plan {
        let (entry, principal) = self.member(actor, &goal)?;
        self.require_grant(actor, entry, entry.local.grants(&principal).contribute)?;
        let context = entry
            .goal
            .current_context(Scope::Document(doc))
            .ok_or_else(|| conflict("no current rules binding"))?;
        let mut tx = Tx::none();
        let event = self.author(
            entry,
            &principal,
            Body::DocumentRevised { context, doc, base },
            Some(&text),
            now,
            &mut tx,
        )?;
        recorded(event, tx)
    }
}
