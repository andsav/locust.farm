//! Notes and the shared documents.

use locust_proto::api::{DocView, NoteView, Response};
use locust_proto::engine::Entropy;
use locust_proto::event::{Body, Doc};
use locust_proto::id::{EventId, GoalId};
use locust_proto::store::Store;

use super::tasks::recorded;
use super::{Plan, answer};
use crate::node::Node;
use crate::node::access::{conflict, not_found};
use crate::node::callers::Actor;
use crate::node::commit::Tx;

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn note_add(
        &self,
        actor: &Actor,
        goal: GoalId,
        about: Option<EventId>,
        supersedes: Option<EventId>,
        text: String,
        now_ms: u64,
    ) -> Plan {
        let (entry, author) = self.member(actor, &goal)?;
        let body = Body::Note { about, supersedes };
        let mut tx = Tx::none();
        let event = self.author(entry, &author, body, Some(&text), now_ms, &mut tx)?;
        recorded(event, tx)
    }

    pub(super) fn notes(&self, actor: &Actor, goal: GoalId, about: Option<EventId>) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let notes = entry
            .state()
            .notes
            .iter()
            .filter(|note| about.is_none() || note.about == about)
            .map(|note| NoteView {
                note: note.id,
                author: note.author,
                about: note.about,
                supersedes: note.supersedes,
                at_ms: note.at_ms,
                text: entry
                    .goal
                    .event(&note.id)
                    .and_then(|event| entry.text(&self.store, event)),
            })
            .collect();
        answer(Response::Notes(notes))
    }

    pub(super) fn doc_read(&self, actor: &Actor, goal: GoalId, doc: Doc) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let document = entry.state().document(doc);
        let accepted = document.and_then(|document| document.accepted);
        answer(Response::Doc(DocView {
            doc,
            accepted,
            text: accepted
                .and_then(|revision| entry.goal.event(&revision))
                .and_then(|event| entry.text(&self.store, event)),
            proposals: document
                .into_iter()
                .flat_map(|document| document.open())
                .map(|revision| revision.id)
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
        now_ms: u64,
    ) -> Plan {
        let (entry, author) = self.member(actor, &goal)?;
        let body = Body::Revision { doc, base };
        let mut tx = Tx::none();
        let event = self.author(entry, &author, body, Some(&text), now_ms, &mut tx)?;
        recorded(event, tx)
    }

    pub(super) fn doc_accept(
        &self,
        actor: &Actor,
        goal: GoalId,
        revision: EventId,
        now_ms: u64,
    ) -> Plan {
        let (entry, coordinator) = self.coordinator(actor, &goal)?;
        let state = entry.state();
        let (doc, found) = state
            .revision(&revision)
            .ok_or_else(|| not_found("no such revision"))?;
        let accepted = state.document(doc).and_then(|document| document.accepted);
        if found.base != accepted {
            return Err(conflict(
                "the revision was not written against the accepted revision",
            ));
        }
        let body = Body::RevisionAccepted { revision };
        let mut tx = Tx::none();
        let event = self.author(entry, &coordinator, body, None, now_ms, &mut tx)?;
        recorded(event, tx)
    }
}
