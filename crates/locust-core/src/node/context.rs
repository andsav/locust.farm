//! Observational context snapshots and durable acknowledgments of exact content.
//!
//! Delivery receipts are signed with this daemon's durable identity. Reads need
//! no commit, and a lost response cannot consume anything. Explicit receipt
//! acknowledgment writes one `Space::Cursor` record per session/event/version;
//! retaining versions makes delayed receipts safe after newer acknowledgments.

use std::collections::BTreeSet;

use locust_proto::api::{
    ApiError, BlobState, ContextAcknowledgment, ContextCursor, ContextItem, ContextNews,
    ContextReceipt, ContextSeen, ContextView, ContextViewMode, ErrorCode, Response,
};
use locust_proto::codec;
use locust_proto::crypto::{self, Keypair};
use locust_proto::engine::Entropy;
use locust_proto::event::{Event, TaskId};
use locust_proto::id::{BlobHash, EventId, GoalId, InstanceId, PublicKey, Signature};
use locust_proto::store::{LocalWrite, Space, Store, StoreError};

use super::Node;
use super::callers::Actor;
use super::commit::Tx;
use super::entry::Entry;
use super::records;
use super::requests::{Plan, Planned, answer};

const ACK: u8 = b'a';
const RECEIPT_DOMAIN: &str = "locust context receipt v3";

#[derive(Default)]
pub(super) struct Acknowledgments {
    seen: BTreeSet<(PublicKey, InstanceId, ContextSeen)>,
}

pub(super) fn is_acknowledgment(key: &[u8]) -> bool {
    key.first() == Some(&ACK)
}

pub(super) fn goal_of(key: &[u8]) -> Result<GoalId, StoreError> {
    records::part(key, 1)
        .map(GoalId)
        .ok_or_else(records::bad_key)
}

impl Acknowledgments {
    pub fn absorb(&mut self, key: &[u8], value: &[u8]) -> Result<(), StoreError> {
        let principal_at = 1 + GoalId::LEN;
        let session_at = principal_at + PublicKey::LEN;
        let event_at = session_at + InstanceId::LEN;
        let version_at = event_at + EventId::LEN;
        if key.len() != version_at + BlobHash::LEN {
            return Err(records::bad_key());
        }
        records::read::<()>(value)?;
        let principal = PublicKey(records::part(key, principal_at).ok_or_else(records::bad_key)?);
        let session = InstanceId(records::part(key, session_at).ok_or_else(records::bad_key)?);
        let event = EventId(records::part(key, event_at).ok_or_else(records::bad_key)?);
        let version = BlobHash(records::part(key, version_at).ok_or_else(records::bad_key)?);
        self.seen
            .insert((principal, session, ContextSeen { event, version }));
        Ok(())
    }

    fn contains(&self, principal: PublicKey, session: InstanceId, seen: ContextSeen) -> bool {
        self.seen.contains(&(principal, session, seen))
    }

    fn write(
        goal: GoalId,
        principal: PublicKey,
        session: InstanceId,
        seen: ContextSeen,
    ) -> LocalWrite {
        records::put(
            Space::Cursor,
            records::key(
                ACK,
                &[
                    &goal.0,
                    &principal.0,
                    &session.0,
                    &seen.event.0,
                    &seen.version.0,
                ],
            ),
            &(),
        )
    }
}

fn receipt_digest(receipt: &ContextReceipt) -> [u8; 32] {
    crypto::content_hash(
        &codec::encode(&(
            receipt.goal,
            receipt.principal,
            receipt.session,
            receipt.revision,
            &receipt.entries,
        ))
        .expect("context receipts encode"),
    )
    .0
}

/// Task context includes goal-wide findings, governance and shared documents,
/// as well as that task's history. It excludes unrelated task threads.
fn relevant(entry: &Entry, event: &Event, task: Option<TaskId>) -> bool {
    task.is_none_or(|task| {
        entry
            .goal
            .task_of(&event.id())
            .is_none_or(|found| found == task)
    })
}

impl<S: Store, E: Entropy> Node<S, E> {
    /// Version only metadata: immutable payload identity and readable status
    /// suffice to detect new text without loading every payload for news counts.
    fn context_seen(&self, entry: &Entry, event: &Event, actor: &Actor) -> (ContextSeen, bool) {
        let payload_state = event.header().payload.map(|payload| {
            (
                payload,
                self.blob_state(entry, &payload.hash, actor.principal.as_ref()),
            )
        });
        let unavailable = payload_state.is_some_and(|(_, state)| state != BlobState::Held);
        (
            ContextSeen {
                event: event.id(),
                version: crypto::content_hash(
                    &codec::encode(&(entry.event_view(event), payload_state))
                        .expect("context versions encode"),
                ),
            },
            unavailable,
        )
    }

    pub(super) fn context_news(&self, entry: &Entry, actor: &Actor) -> Option<ContextNews> {
        let (Some(principal), Some(session)) = (actor.principal, actor.session) else {
            return None;
        };
        if actor.is_viewer() {
            return None;
        }
        let mut news = ContextNews::default();
        for event in entry
            .goal
            .authors()
            .flat_map(|author| entry.goal.points(author))
            .filter_map(|point| entry.goal.event(&point.id))
        {
            let (seen, unavailable) = self.context_seen(entry, event, actor);
            if !entry.context.contains(principal, session, seen) {
                news.unacknowledged += 1;
                news.unavailable += u64::from(unavailable);
            }
        }
        Some(news)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn context_read(
        &self,
        actor: &Actor,
        goal: GoalId,
        task: Option<TaskId>,
        after: Option<ContextCursor>,
        limit: u32,
        preview_chars: Option<u32>,
        unread_only: bool,
        view: ContextViewMode,
    ) -> Plan {
        let entry = self.readable(actor, &goal)?;
        if limit == 0 {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "context page limit must be positive",
            ));
        }
        let session = if actor.is_viewer() {
            None
        } else {
            actor.session
        };
        if let (Some(principal), Some(session)) = (actor.principal, session) {
            // Validate an existing binding, but observational reads bind nothing.
            self.sessions.bind(&session, &principal)?;
        }
        let revision = entry.revision();
        if after.as_ref().is_some_and(|cursor| {
            cursor.goal != goal
                || cursor.revision != revision
                || cursor.task != task
                || cursor.unread_only != unread_only
                || cursor.view != view
                || cursor.preview_chars != preview_chars
                || cursor.limit != limit
                || cursor.reader != actor.principal
                || cursor.session != session
        }) {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "context continuation changed; restart context.read at the current revision",
            ));
        }
        let start = after.as_ref().map_or(0, |cursor| cursor.offset);
        let reader = actor.principal.zip(session);
        let mut news = reader.map(|_| ContextNews::default());
        // The page and its pending summary describe the same snapshot. Compute
        // versions once, including out-of-scope events in goal-wide news.
        let mut events: Vec<_> = entry
            .goal
            .authors()
            .flat_map(|author| entry.goal.points(author))
            .filter_map(|point| entry.goal.event(&point.id))
            .filter_map(|event| {
                let mut acknowledged = None;
                let seen = reader.map(|(principal, session)| {
                    let (seen, unavailable) = self.context_seen(entry, event, actor);
                    let seen_before = entry.context.contains(principal, session, seen);
                    acknowledged = Some(seen_before);
                    if !seen_before {
                        let news = news.as_mut().expect("a session has context news");
                        news.unacknowledged += 1;
                        news.unavailable += u64::from(unavailable);
                    }
                    seen
                });
                relevant(entry, event, task).then_some((event, seen, acknowledged))
            })
            .collect();
        events.sort_by_key(|(event, _, _)| {
            (
                entry.feed.position(&event.id()).unwrap_or(u64::MAX),
                event.id(),
            )
        });
        let total = events.len() as u64;
        if start > total {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "context continuation is beyond this query",
            ));
        }
        let mut delivered = Vec::new();
        // Offsets address the complete relevant ordering, before unread
        // filtering. Acknowledging one page cannot shift later page positions.
        let mut remaining = events
            .into_iter()
            .enumerate()
            .skip(start as usize)
            .filter(|(_, (_, _, acknowledged))| !unread_only || *acknowledged != Some(true))
            .peekable();
        let mut end = start;
        let items: Vec<_> = remaining
            .by_ref()
            .take(limit as usize)
            .map(|(offset, (event, seen, acknowledged))| {
                end = offset as u64 + 1;
                let mut detail = self.event_detail(entry, event, actor.principal.as_ref());
                let mut complete = detail.payload.is_none() || detail.text.is_some();
                if let (Some(text), Some(chars)) = (&mut detail.text, preview_chars)
                    && let Some((byte, _)) = text.char_indices().nth(chars as usize)
                {
                    text.truncate(byte);
                    complete = false;
                }
                if complete && let Some(seen) = seen {
                    delivered.push(seen);
                }
                ContextItem {
                    event: detail,
                    text_complete: complete,
                    acknowledged,
                }
            })
            .collect();
        let next = remaining.peek().is_some().then_some(ContextCursor {
            goal,
            revision,
            reader: actor.principal,
            session,
            task,
            unread_only,
            view,
            preview_chars,
            limit,
            offset: end,
        });
        let receipt = actor
            .principal
            .zip(session)
            .filter(|_| !delivered.is_empty())
            .map(|(principal, session)| {
                let mut receipt = ContextReceipt {
                    goal,
                    principal,
                    session,
                    revision,
                    entries: delivered,
                    signature: Signature([0; 64]),
                };
                receipt.signature = Keypair::from_seed(self.identity.endpoint_secret)
                    .sign(RECEIPT_DOMAIN, &receipt_digest(&receipt));
                receipt
            });
        let summary = after
            .is_none()
            .then(|| self.context_summary(entry, actor, task, view, news))
            .transpose()?;
        answer(Response::Context(Box::new(ContextView {
            revision,
            summary,
            items,
            next,
            receipt,
        })))
    }

    pub(super) fn context_acknowledge(
        &self,
        actor: &Actor,
        goal: GoalId,
        receipt: ContextReceipt,
    ) -> Plan {
        let entry = self.readable(actor, &goal)?;
        let principal = actor.principal()?;
        let session = actor.session()?;
        if actor.is_viewer()
            || receipt.goal != goal
            || receipt.principal != principal
            || receipt.session != session
        {
            return Err(ApiError::new(
                ErrorCode::Denied,
                "context receipt belongs to another goal or session",
            ));
        }
        let signer = Keypair::from_seed(self.identity.endpoint_secret);
        if !crypto::verify(
            &signer.public(),
            RECEIPT_DOMAIN,
            &receipt_digest(&receipt),
            &receipt.signature,
        ) {
            return Err(ApiError::new(
                ErrorCode::Denied,
                "context receipt was not issued by this daemon",
            ));
        }
        let mut tx = Tx::none();
        if let Some(binding) = self.sessions.bind(&session, &principal)? {
            tx.local(binding);
        }
        for seen in &receipt.entries {
            if !entry.context.contains(principal, session, *seen) {
                // An acknowledgment changes this session's unread filter, not
                // the goal content revision or another reader's continuation.
                tx.local(Acknowledgments::write(goal, principal, session, *seen));
            }
        }
        Ok(Planned {
            response: Response::ContextAcknowledged(ContextAcknowledgment {
                goal,
                principal,
                session,
                entries: receipt.entries,
            }),
            tx,
        })
    }
}
