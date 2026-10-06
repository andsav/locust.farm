//! Explicit context summary shapes and complete, revision-bound work pages.

use locust_proto::api::{
    ApiError, ContextBrief, ContextDocument, ContextDocumentSelection, ContextNews,
    ContextSnapshot, ContextSummary, ContextViewMode, ErrorCode, PendingCounts, PendingCursor,
    PendingItem, PendingKind, PendingPage, PendingWork, Response,
};
use locust_proto::engine::Entropy;
use locust_proto::event::{Scope, TaskId};
use locust_proto::id::GoalId;
use locust_proto::store::Store;

use super::Node;
use super::access::not_found;
use super::callers::Actor;
use super::entry::Entry;
use super::requests::{Plan, answer};

impl<S: Store, E: Entropy> Node<S, E> {
    pub(super) fn context_summary(
        &self,
        entry: &Entry,
        actor: &Actor,
        task: Option<TaskId>,
        view: ContextViewMode,
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
            let governance = entry
                .state()
                .governance
                .or_else(|| {
                    entry
                        .local
                        .joins
                        .values()
                        .next()
                        .map(|join| join.governance)
                })
                .ok_or_else(|| not_found("no such goal"))?;
            return Ok(ContextSummary::Compact(Box::new(ContextBrief {
                workspace: self.workspace_view(entry, actor)?,
                checkout: self.bound_checkout(entry, actor),
                goal: entry.id(),
                title: self.title(entry, actor.principal.as_ref()),
                host: governance,
                governance_head: entry.state().head,
                current_rules: entry.state().current_rules,
                halted: entry.halted(),
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
    if kind.is_none_or(|kind| kind == PendingKind::ToAuthorize) {
        items.extend(work.to_authorize.into_iter().map(PendingItem::ToAuthorize));
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
