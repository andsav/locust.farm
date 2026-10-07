//! Explicit context summary shapes and complete, revision-bound work pages.

use std::cmp::Reverse;

use locust_proto::api::{
    ApiError, BRIEF_FINDINGS, ContextBrief, ContextDocument, ContextDocumentSelection, ContextNews,
    ContextSnapshot, ContextSummary, ContextViewMode, CurrentFindings, ErrorCode,
    FINDING_LINE_CHARS, FindingHeadline, PendingCounts, PendingCursor, PendingItem, PendingKind,
    PendingPage, PendingWork, Response,
};
use locust_proto::engine::Entropy;
use locust_proto::event::{Event, Scope, TaskId};
use locust_proto::id::{GoalId, PublicKey};
use locust_proto::store::Store;

use super::Node;
use super::access::not_found;
use super::callers::Actor;
use super::entry::Entry;
use super::requests::{Plan, answer};
use crate::goal::{Standing, is_goal_finding};

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn context_summary(
        &self,
        entry: &Entry,
        actor: &Actor,
        task: Option<TaskId>,
        view: ContextViewMode,
        unread_only: bool,
        news: Option<ContextNews>,
    ) -> Result<ContextSummary, ApiError> {
        let scope = task.map_or(Scope::Goal, Scope::Task);
        let selected = task.and_then(|task| entry.goal.selected_task(task));
        let found = task
            .map(|task| {
                entry
                    .state()
                    .tasks
                    .get(&task)
                    .or(selected.as_ref())
                    .ok_or_else(|| not_found("no such task"))
            })
            .transpose()?;
        let context = found
            .map(|task| task.rounds[&task.current_round].context)
            .or_else(|| entry.goal.current_context(scope));
        let pending = self.pending_work_with_news(entry, actor, news);
        if view == ContextViewMode::Compact {
            let findings = self.current_findings(entry, actor, unread_only)?;
            return Ok(ContextSummary::Compact(Box::new(ContextBrief {
                workspace: self.workspace_view(entry, actor)?,
                checkout: self.bound_checkout(entry, actor),
                goal: entry.id(),
                title: self.title(entry, actor.principal.as_ref()),
                host: entry.state().host,
                host_name: Self::host_name(entry),
                governance_head: entry.state().head,
                current_rules: entry.state().current_rules,
                halted: self.halt(entry, actor.principal.as_ref()),
                context,
                documents: entry
                    .state()
                    .documents
                    .iter()
                    .map(|(doc, state)| ContextDocumentSelection {
                        doc: *doc,
                        selected: state.selected,
                    })
                    .collect(),
                pending: PendingCounts::from(&pending),
                context_news: pending.context_news,
                findings,
            })));
        }
        let Response::GoalStatus(status) = self.goal_status(actor, entry.id())?.response else {
            unreachable!("goal status has one response type")
        };
        let task = found.map(|task| self.task_detail(entry, task, actor.principal.as_ref()));
        let (effective_rules_json, inputs) = match &task {
            Some(task) => (task.effective_rules_json.clone(), task.inputs.clone()),
            None => {
                let rules = context
                    .and_then(|context| entry.goal.effective_rules(context, &entry.definitions));
                let inputs = entry
                    .state()
                    .current_rules
                    .and_then(|id| entry.state().rules.get(&id))
                    .map(|rules| rules.binding.inputs.clone())
                    .unwrap_or_default();
                (
                    serde_json::to_string(&rules).expect("effective rules encode"),
                    inputs,
                )
            }
        };
        Ok(ContextSummary::Full(Box::new(ContextSnapshot {
            checkout: self.bound_checkout(entry, actor),
            status,
            task,
            effective_rules_json,
            inputs,
            documents: entry
                .state()
                .documents
                .iter()
                .map(|(doc, state)| ContextDocument {
                    doc: *doc,
                    selected: state.selected,
                    revisions: state.revisions.iter().copied().collect(),
                })
                .collect(),
            pending,
        })))
    }

    /// Current findings are effective goal-wide contributions whose payloads
    /// are not withdrawn here. Missing payloads stay current without a line.
    /// Every candidate is checked, including those beyond the newest headlines.
    fn current_findings(
        &self,
        entry: &Entry,
        reader: &Actor,
        unread_only: bool,
    ) -> Result<CurrentFindings, ApiError> {
        let mut events = Vec::new();
        for contribution in entry.state().contributions.values() {
            let Some(event) = entry.goal.event(&contribution.id) else {
                continue;
            };
            if !is_goal_finding(event)
                || entry.goal.standing(&event.id()) != Some(Standing::Effective)
            {
                continue;
            }
            if let Some(payload) = event.header().payload
                && super::requests::content::blob_record(&self.store, &entry.id(), &payload.hash)?
                    .is_some_and(|record| record.withdrawn)
            {
                continue;
            }
            events.push(event);
        }
        events.sort_by_key(|event| {
            Reverse((
                entry.feed.position(&event.id()).unwrap_or(u64::MAX),
                event.id(),
            ))
        });
        let total = events.len() as u64;
        let newest_event = events.first().map(|event| event.id());
        // Check all current findings, not just the headlines. Only a session
        // with prior acknowledgments and no unread finding suppresses the list.
        let show = !unread_only
            || reader
                .principal
                .zip(reader.session)
                .is_none_or(|(principal, session)| {
                    !entry.context.has_any(principal, session)
                        || events.iter().any(|event| {
                            let (seen, _) = self.context_seen(entry, event, reader);
                            !entry.context.contains(principal, session, seen)
                        })
                });
        let newest = if show {
            events
                .into_iter()
                .take(BRIEF_FINDINGS)
                .map(|event| self.finding_headline(entry, event, reader.principal.as_ref()))
                .collect()
        } else {
            Vec::new()
        };
        Ok(CurrentFindings {
            total,
            newest_event,
            newest,
        })
    }

    /// One current finding's headline: the author's current member name and
    /// the payload's first line cut to `FINDING_LINE_CHARS` characters. An
    /// author the members map does not know is simply unnamed; a payload that
    /// is not readable here shows no line.
    pub(super) fn finding_headline(
        &self,
        entry: &Entry,
        event: &Event,
        reader: Option<&PublicKey>,
    ) -> FindingHeadline {
        FindingHeadline {
            event: event.id(),
            author: event.header().author,
            name: entry
                .state()
                .members
                .get(&event.header().author)
                .map(|member| member.name.clone()),
            line: entry.text(&self.store, event, reader).map(|text| {
                let mut line = text.lines().next().unwrap_or_default().to_owned();
                if let Some((byte, _)) = line.char_indices().nth(FINDING_LINE_CHARS) {
                    line.truncate(byte);
                }
                line
            }),
        }
    }

    pub(super) fn pending_page(
        &self,
        actor: &Actor,
        goal: GoalId,
        kind: Option<PendingKind>,
        after: Option<PendingCursor>,
        limit: u32,
    ) -> Plan {
        let entry = self.readable(actor, &goal)?;
        if limit == 0 {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "pending page limit must be positive",
            ));
        }
        let session = actor.session;
        if let (Some(principal), Some(session)) = (actor.principal, session) {
            self.sessions.bind(&session, &principal)?;
        }
        let revision = entry.revision();
        if after.as_ref().is_some_and(|cursor| {
            cursor.goal != goal
                || cursor.revision != revision
                || cursor.reader != actor.principal
                || cursor.session != session
                || cursor.kind != kind
                || cursor.limit != limit
        }) {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "pending continuation changed; restart the pending page read without `after` at the current revision",
            ));
        }
        let pending = self.pending_work(entry, actor);
        let counts = PendingCounts::from(&pending);
        let news = pending.context_news;
        let entries = pending_items(pending, kind);
        let start = after.as_ref().map_or(0, |cursor| cursor.offset);
        if start > entries.len() as u64 {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "pending continuation is beyond this query",
            ));
        }
        let mut remaining = entries.into_iter().skip(start as usize).peekable();
        let items: Vec<_> = remaining.by_ref().take(limit as usize).collect();
        let next = remaining.peek().is_some().then_some(PendingCursor {
            goal,
            revision,
            reader: actor.principal,
            session,
            kind,
            limit,
            offset: start + items.len() as u64,
        });
        answer(Response::PendingPage(PendingPage {
            revision,
            counts,
            context_news: news,
            items,
            next,
        }))
    }
}

/// The category order is fixed; each category retains its full view order.
/// Filtering happens before paging and never removes an item from an all-kind read.
pub(super) fn pending_items(work: PendingWork, kind: Option<PendingKind>) -> Vec<PendingItem> {
    let mut items = Vec::new();
    if kind.is_none_or(|kind| kind == PendingKind::AskFirst) {
        items.extend(work.ask_first.into_iter().map(PendingItem::AskFirst));
    }
    if kind.is_none_or(|kind| kind == PendingKind::ToStart) {
        items.extend(work.to_start.into_iter().map(PendingItem::ToStart));
    }
    if kind.is_none_or(|kind| kind == PendingKind::Claimed) {
        items.extend(work.claimed.into_iter().map(PendingItem::Claimed));
    }
    if kind.is_none_or(|kind| kind == PendingKind::HeldElsewhere) {
        items.extend(
            work.held_elsewhere
                .into_iter()
                .map(PendingItem::HeldElsewhere),
        );
    }
    if kind.is_none_or(|kind| kind == PendingKind::ToAcknowledge) {
        items.extend(
            work.to_acknowledge
                .into_iter()
                .map(PendingItem::ToAcknowledge),
        );
    }
    if kind.is_none_or(|kind| kind == PendingKind::ToReview) {
        items.extend(work.to_review.into_iter().map(PendingItem::ToReview));
    }
    if kind.is_none_or(|kind| kind == PendingKind::Deliveries) {
        items.extend(work.deliveries.into_iter().map(PendingItem::Deliveries));
    }
    items
}
