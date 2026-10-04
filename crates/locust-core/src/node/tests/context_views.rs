//! Compact context preserves complete event text and explicit obligation access.

use super::lifecycle::{authorize, event, finding, offered, setup};
use super::*;
use locust_proto::api::{
    ContextCursor, ContextSummary, ContextView, ContextViewMode, PendingCounts, PendingCursor,
    PendingItem, PendingKind, PendingPage,
};
use locust_proto::event::ReviewVerdict;
use locust_proto::id::GoalId;
use locust_proto::store::{Space, Store};

fn read(goal: GoalId, view: ContextViewMode, after: Option<ContextCursor>, limit: u32) -> Request {
    Request::Context {
        goal,
        task: None,
        after,
        limit,
        preview_chars: None,
        unread_only: false,
        view,
    }
}

fn context(d: &mut Daemon, agent: ConnId, request: Request) -> ContextView {
    let response = d.ok(agent, request);
    let encoded = locust_proto::codec::encode(&response).unwrap();
    assert_eq!(
        locust_proto::codec::decode::<Response>(&encoded).unwrap(),
        response
    );
    let Response::Context(view) = response else {
        panic!("expected context")
    };
    *view
}

fn pending(
    d: &mut Daemon,
    agent: ConnId,
    goal: GoalId,
    kind: Option<PendingKind>,
    after: Option<PendingCursor>,
    limit: u32,
) -> PendingPage {
    let response = d.ok(
        agent,
        Request::PendingPage {
            goal,
            kind,
            after,
            limit,
        },
    );
    let encoded = locust_proto::codec::encode(&response).unwrap();
    assert_eq!(
        locust_proto::codec::decode::<Response>(&encoded).unwrap(),
        response
    );
    let Response::PendingPage(page) = response else {
        panic!("expected pending page")
    };
    page
}

#[test]
fn compact_and_full_share_complete_items_and_first_page_summary_only() {
    let (mut d, principal, _, agent, goal) = setup();
    offered(&mut d, agent, goal, principal);
    let text = "Complete attributed evidence é".repeat(128);
    let finding = event(d.ok(agent, finding(goal, &text)));
    let full = context(&mut d, agent, read(goal, ContextViewMode::Full, None, 2));
    let compact = context(&mut d, agent, read(goal, ContextViewMode::Compact, None, 2));
    assert_eq!(compact.items, full.items);
    assert_eq!(compact.receipt, full.receipt);
    let Some(ContextSummary::Full(snapshot)) = &full.summary else {
        panic!()
    };
    let Some(ContextSummary::Compact(brief)) = &compact.summary else {
        panic!()
    };
    assert_eq!(brief.pending, PendingCounts::from(&snapshot.pending));
    assert_eq!(brief.context_news, snapshot.pending.context_news);
    assert_eq!(brief.goal, snapshot.status.goal);
    assert_eq!(brief.current_rules, snapshot.status.current_rules);
    assert_eq!(brief.documents.len(), snapshot.documents.len());
    let mut all = compact.items;
    let mut after = compact.next;
    while after.is_some() {
        let page = context(
            &mut d,
            agent,
            read(goal, ContextViewMode::Compact, after, 2),
        );
        assert!(page.summary.is_none());
        all.extend(page.items);
        after = page.next;
    }
    let item = all
        .iter()
        .find(|item| item.event.view.event == finding)
        .unwrap();
    assert!(item.text_complete);
    assert_eq!(item.event.text.as_deref(), Some(text.as_str()));
    let full = context(
        &mut d,
        agent,
        read(goal, ContextViewMode::Full, None, u32::MAX),
    );
    assert_eq!(all, full.items);
    let next = context(&mut d, agent, read(goal, ContextViewMode::Full, None, 1))
        .next
        .unwrap();
    assert!(
        context(
            &mut d,
            agent,
            read(goal, ContextViewMode::Full, Some(next), 1)
        )
        .summary
        .is_none()
    );
}

#[test]
fn context_continuations_bind_view_preview_limit_and_session() {
    let (mut d, _, _, agent, goal) = setup();
    event(d.ok(agent, finding(goal, "Content across pages")));
    let next = context(&mut d, agent, read(goal, ContextViewMode::Compact, None, 1))
        .next
        .unwrap();
    let other = d.connect(credential(1), Some(session(2)));
    for (conn, request) in [
        (
            other,
            read(goal, ContextViewMode::Compact, Some(next.clone()), 1),
        ),
        (
            agent,
            read(goal, ContextViewMode::Full, Some(next.clone()), 1),
        ),
        (
            agent,
            read(goal, ContextViewMode::Compact, Some(next.clone()), 2),
        ),
        (
            agent,
            Request::Context {
                goal,
                task: None,
                after: Some(next),
                limit: 1,
                preview_chars: Some(1),
                unread_only: false,
                view: ContextViewMode::Compact,
            },
        ),
    ] {
        assert_eq!(code(d.call(conn, request)), ErrorCode::Conflict);
    }
}

#[test]
fn pending_delivery_page_roundtrips_native_codec() {
    use locust_proto::api::DeliveryItem;
    use locust_proto::event::{Context, Scope};
    use locust_proto::id::{EffectId, EventId};

    let response = Response::PendingPage(PendingPage {
        revision: 7,
        counts: PendingCounts {
            deliveries: 1,
            ..Default::default()
        },
        context_news: None,
        items: vec![PendingItem::Deliveries(DeliveryItem {
            effect: EffectId([1; 32]),
            context: Context {
                scope: Scope::Goal,
                round: EventId([2; 32]),
            },
            acknowledged: false,
            received: true,
            available: true,
            action: "Inspect received evidence".into(),
        })],
        next: None,
    });
    let encoded = locust_proto::codec::encode(&response).unwrap();
    assert_eq!(
        locust_proto::codec::decode::<Response>(&encoded).unwrap(),
        response
    );
}

#[test]
fn pending_pages_are_complete_match_full_and_filter_each_category() {
    let (mut d, principal, owner, agent, goal) = setup();
    let mut offers = Vec::new();
    for index in 0..12 {
        let (task, offer) = offered(&mut d, agent, goal, principal);
        offers.push((task, offer));
        if index % 2 == 0 {
            authorize(&mut d, owner, goal, task, principal);
        }
        event(d.ok(agent, finding(goal, &format!("Evidence {index}"))));
    }
    let Response::Claimed(claim) = d.ok(
        agent,
        Request::AttemptStart {
            goal,
            task: offers[0].0,
            offer: Some(offers[0].1),
        },
    ) else {
        panic!()
    };
    d.ok(
        agent,
        Request::AttemptCancel {
            goal,
            attempt: claim.attempt,
        },
    );
    let other = d.connect(credential(1), Some(session(2)));
    d.ok(
        other,
        Request::AttemptStart {
            goal,
            task: offers[2].0,
            offer: Some(offers[2].1),
        },
    );
    let Response::Pending(full) = d.ok(agent, Request::Pending { goal }) else {
        panic!()
    };
    assert!(!full.to_authorize.is_empty());
    assert!(!full.to_start.is_empty());
    assert!(!full.claimed.is_empty());
    assert!(!full.held_elsewhere.is_empty());
    assert!(!full.to_acknowledge.is_empty());
    assert!(!full.to_review.is_empty());
    let expected = crate::node::context_views::pending_items(full.clone(), None);
    let before = d.store.scan(Space::Cursor, &[]).unwrap();
    let mut after = None;
    let mut all = Vec::new();
    loop {
        let page = pending(&mut d, agent, goal, None, after, 3);
        assert_eq!(page.revision, full.revision);
        assert_eq!(page.counts, PendingCounts::from(&full));
        assert_eq!(page.context_news, full.context_news);
        all.extend(page.items);
        after = page.next;
        if after.is_none() {
            break;
        }
    }
    assert_eq!(all, expected);
    assert_eq!(
        all.len(),
        full.to_authorize.len()
            + full.to_start.len()
            + full.claimed.len()
            + full.held_elsewhere.len()
            + full.to_acknowledge.len()
            + full.to_review.len()
            + full.deliveries.len()
    );
    assert_eq!(d.store.scan(Space::Cursor, &[]).unwrap(), before);
    for kind in [
        PendingKind::ToAuthorize,
        PendingKind::ToStart,
        PendingKind::Claimed,
        PendingKind::HeldElsewhere,
        PendingKind::ToAcknowledge,
        PendingKind::ToReview,
        PendingKind::Deliveries,
    ] {
        let page = pending(&mut d, agent, goal, Some(kind), None, u32::MAX);
        let count = match kind {
            PendingKind::ToAuthorize => full.to_authorize.len(),
            PendingKind::ToStart => full.to_start.len(),
            PendingKind::Claimed => full.claimed.len(),
            PendingKind::HeldElsewhere => full.held_elsewhere.len(),
            PendingKind::ToAcknowledge => full.to_acknowledge.len(),
            PendingKind::ToReview => full.to_review.len(),
            PendingKind::Deliveries => full.deliveries.len(),
        };
        assert_eq!(page.items.len(), count);
        assert!(page.items.iter().all(|item| matches!(
            (kind, item),
            (PendingKind::ToAuthorize, PendingItem::ToAuthorize(_))
                | (PendingKind::ToStart, PendingItem::ToStart(_))
                | (PendingKind::Claimed, PendingItem::Claimed(_))
                | (PendingKind::HeldElsewhere, PendingItem::HeldElsewhere(_))
                | (PendingKind::ToAcknowledge, PendingItem::ToAcknowledge(_))
                | (PendingKind::ToReview, PendingItem::ToReview(_))
                | (PendingKind::Deliveries, PendingItem::Deliveries(_))
        )));
        assert_eq!(
            page.items,
            crate::node::context_views::pending_items(full.clone(), Some(kind))
        );
        assert!(page.next.is_none());
    }
}

#[test]
fn pending_continuations_fence_query_sessions_revisions_and_local_authority() {
    let (mut d, principal, owner, agent, goal) = setup();
    let (task, _) = offered(&mut d, agent, goal, principal);
    offered(&mut d, agent, goal, principal);
    let next = pending(&mut d, agent, goal, None, None, 1).next.unwrap();
    let other = d.connect(credential(1), Some(session(2)));
    for (conn, kind, limit) in [
        (other, None, 1),
        (agent, Some(PendingKind::ToAuthorize), 1),
        (agent, None, 2),
    ] {
        assert_eq!(
            code(d.call(
                conn,
                Request::PendingPage {
                    goal,
                    kind,
                    after: Some(next.clone()),
                    limit
                }
            )),
            ErrorCode::Conflict
        );
    }
    authorize(&mut d, owner, goal, task, principal);
    assert_eq!(
        code(d.call(
            agent,
            Request::PendingPage {
                goal,
                kind: None,
                after: Some(next),
                limit: 1
            }
        )),
        ErrorCode::Conflict
    );
    let next = pending(&mut d, agent, goal, None, None, 1).next.unwrap();
    event(d.ok(
        agent,
        finding(goal, "New evidence invalidates pending continuation"),
    ));
    assert_eq!(
        code(d.call(
            agent,
            Request::PendingPage {
                goal,
                kind: None,
                after: Some(next),
                limit: 1
            }
        )),
        ErrorCode::Conflict
    );
    assert_eq!(
        code(d.call(
            agent,
            Request::PendingPage {
                goal,
                kind: None,
                after: None,
                limit: 0
            }
        )),
        ErrorCode::Invalid
    );
}

#[test]
fn compact_pages_acknowledge_without_skipping_items_or_other_sessions() {
    let (mut d, _, _, agent, goal) = setup();
    for index in 0..8 {
        event(d.ok(agent, finding(goal, &format!("Finding {index}"))));
    }
    let other = d.connect(credential(1), Some(session(2)));
    let mut after = None;
    let mut read_ids = Vec::new();
    loop {
        let mut request = read(goal, ContextViewMode::Compact, after, 2);
        let Request::Context { unread_only, .. } = &mut request else {
            unreachable!()
        };
        *unread_only = true;
        let page = context(&mut d, agent, request.clone());
        assert_eq!(context(&mut d, agent, request), page);
        read_ids.extend(page.items.iter().map(|item| item.event.view.event));
        let receipt = page.receipt.unwrap();
        let request = Request::ContextAcknowledge { goal, receipt };
        let answer = d.ok(agent, request.clone());
        assert_eq!(d.ok(agent, request), answer);
        after = page.next;
        if after.is_none() {
            break;
        }
    }
    let all = context(
        &mut d,
        other,
        read(goal, ContextViewMode::Full, None, u32::MAX),
    );
    assert_eq!(
        read_ids,
        all.items
            .iter()
            .map(|item| item.event.view.event)
            .collect::<Vec<_>>()
    );
    let page = pending(&mut d, agent, goal, None, None, 1);
    assert_eq!(page.context_news.unwrap().unacknowledged, 0);
    assert_eq!(
        pending(&mut d, other, goal, None, None, 1)
            .context_news
            .unwrap()
            .unacknowledged as usize,
        read_ids.len()
    );
}

#[test]
fn compact_seen_context_size_does_not_repeat_review_obligations() {
    let (mut d, _, _, agent, goal) = setup();
    for index in 0..256 {
        let subject = event(d.ok(
            agent,
            finding(goal, &format!("Finding {index}: shared context")),
        ));
        if index % 4 == 0 {
            d.ok(
                agent,
                Request::ReviewRecord {
                    goal,
                    subject,
                    verdict: ReviewVerdict::Reject,
                    text: "Additional evidence required".into(),
                },
            );
        }
    }
    let all = context(
        &mut d,
        agent,
        read(goal, ContextViewMode::Full, None, u32::MAX),
    );
    d.ok(
        agent,
        Request::ContextAcknowledge {
            goal,
            receipt: all.receipt.unwrap(),
        },
    );
    let mut request = read(goal, ContextViewMode::Compact, None, 16);
    let Request::Context { unread_only, .. } = &mut request else {
        unreachable!()
    };
    *unread_only = true;
    let compact = d.ok(agent, request.clone());
    let Request::Context { view, .. } = &mut request else {
        unreachable!()
    };
    *view = ContextViewMode::Full;
    let full = d.ok(agent, request);
    let compact_bytes = serde_json::to_vec(&compact).unwrap().len();
    let full_bytes = serde_json::to_vec(&full).unwrap().len();
    println!(
        "context_seen_size findings=256 compact_bytes={compact_bytes} full_bytes={full_bytes}"
    );
    assert!(compact_bytes < full_bytes);
    let Response::Context(compact) = compact else {
        panic!()
    };
    assert!(compact.items.is_empty());
    let Some(ContextSummary::Compact(brief)) = compact.summary else {
        panic!()
    };
    assert_eq!(brief.pending.to_review, 192);
    assert_eq!(
        pending(
            &mut d,
            agent,
            goal,
            Some(PendingKind::ToReview),
            None,
            u32::MAX
        )
        .items
        .len(),
        192
    );
}
