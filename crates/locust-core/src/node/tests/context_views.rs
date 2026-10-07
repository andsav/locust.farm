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
        let refused = d.call(conn, request).unwrap_err();
        assert_eq!(refused.code, ErrorCode::Conflict);
        names_no_operation(&refused);
    }
}

#[test]
fn a_context_limit_above_the_page_maximum_is_capped_and_continues() {
    use locust_proto::api::MAX_CONTEXT_PAGE;
    let (mut d, _, _, agent, goal) = setup();
    let findings: Vec<_> = (0..=MAX_CONTEXT_PAGE)
        .map(|index| event(d.ok(agent, finding(goal, &format!("Finding {index}")))))
        .collect();
    assert_eq!(findings.len(), 33);
    let first = context(
        &mut d,
        agent,
        read(goal, ContextViewMode::Full, None, u32::MAX),
    );
    assert_eq!(first.items.len(), MAX_CONTEXT_PAGE as usize);
    let next = first.next.clone().unwrap();
    assert_eq!(next.limit, MAX_CONTEXT_PAGE);
    // The capped cursor also follows with any other limit at or above the cap.
    let other = context(
        &mut d,
        agent,
        read(goal, ContextViewMode::Full, Some(next), 1000),
    );
    let mut seen = Vec::new();
    let mut page = first;
    let mut pages = 1;
    loop {
        assert!(page.items.len() <= MAX_CONTEXT_PAGE as usize);
        let delivered: Vec<_> = page
            .items
            .iter()
            .map(|item| item.event.view.event)
            .collect();
        // Each receipt lists only what its own page delivered.
        let receipt = page.receipt.unwrap();
        assert!(
            receipt
                .entries
                .iter()
                .all(|entry| delivered.contains(&entry.event))
        );
        for finding in delivered.iter().filter(|id| findings.contains(id)) {
            assert!(receipt.entries.iter().any(|entry| entry.event == *finding));
        }
        seen.extend(delivered);
        let Some(after) = page.next else {
            break;
        };
        page = context(
            &mut d,
            agent,
            read(goal, ContextViewMode::Full, Some(after), u32::MAX),
        );
        if pages == 1 {
            assert_eq!(page, other);
        }
        pages += 1;
    }
    assert!(pages >= 2);
    for finding in &findings {
        assert_eq!(seen.iter().filter(|id| *id == finding).count(), 1);
    }
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), seen.len());
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
    assert!(!full.ask_first.is_empty());
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
        full.ask_first.len()
            + full.to_start.len()
            + full.claimed.len()
            + full.held_elsewhere.len()
            + full.to_acknowledge.len()
            + full.to_review.len()
            + full.deliveries.len()
    );
    assert_eq!(d.store.scan(Space::Cursor, &[]).unwrap(), before);
    for kind in [
        PendingKind::AskFirst,
        PendingKind::ToStart,
        PendingKind::Claimed,
        PendingKind::HeldElsewhere,
        PendingKind::ToAcknowledge,
        PendingKind::ToReview,
        PendingKind::Deliveries,
    ] {
        let page = pending(&mut d, agent, goal, Some(kind), None, u32::MAX);
        let count = match kind {
            PendingKind::AskFirst => full.ask_first.len(),
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
            (PendingKind::AskFirst, PendingItem::AskFirst(_))
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
        (agent, Some(PendingKind::AskFirst), 1),
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
    let refused = d
        .call(
            agent,
            Request::PendingPage {
                goal,
                kind: None,
                after: Some(next),
                limit: 1,
            },
        )
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::Conflict);
    names_no_operation(&refused);
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
    // Acknowledge every page, so the unread read below has seen it all.
    let mut after = None;
    loop {
        let page = context(
            &mut d,
            agent,
            read(goal, ContextViewMode::Full, after, u32::MAX),
        );
        d.ok(
            agent,
            Request::ContextAcknowledge {
                goal,
                receipt: page.receipt.unwrap(),
            },
        );
        after = page.next;
        if after.is_none() {
            break;
        }
    }
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

fn collaboration_view_setup(
    completion: locust_proto::organization::CompletionRule,
) -> (Daemon, GoalId, [PublicKey; 3], [ConnId; 3]) {
    let (mut d, host, owner, agent, goal) = setup();
    let (second, _) = super::authorization::join_local(&mut d, agent, goal, 2);
    let (third, _) = super::authorization::join_local(&mut d, agent, goal, 3);
    let second_conn = d.connect(credential(2), Some(session(2)));
    let third_conn = d.connect(credential(3), Some(session(3)));
    let members = [host, second, third];
    for member in members {
        d.ok(
            owner,
            Request::LevelSet {
                goal,
                agent: member,
                level: locust_proto::api::Level::Auto,
            },
        );
    }
    let mut formation = locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == "open")
        .unwrap()
        .formation;
    formation.decisions.completion = completion;
    let expected = d.node.goals[&goal].state().current_rules.unwrap();
    d.ok(
        owner,
        Request::RulesBind {
            no_role: false,
            goal,
            expected,
            formation_json: serde_json::to_string(&formation).unwrap(),

            inputs: Default::default(),
        },
    );
    (d, goal, members, [agent, second_conn, third_conn])
}

fn work_view(d: &mut Daemon, agent: ConnId, goal: GoalId) -> locust_proto::api::PendingWork {
    let Response::Pending(work) = d.ok(agent, Request::Pending { goal }) else {
        panic!()
    };
    work
}

#[test]
fn to_start_shows_other_attempts_results_and_sorts_least_attended_first() {
    use locust_proto::event::{AttemptStatus, TaskId};
    let (mut d, goal, members, agents) = collaboration_view_setup(Default::default());
    let tasks: Vec<_> = (0..3)
        .map(|index| {
            TaskId::Authored(event(d.ok(
                agents[0],
                Request::TaskOpen {
                    goal,
                    text: format!("Work {index}"),
                    task_type: None,
                    inputs: Default::default(),
                    parent: None,
                },
            )))
        })
        .collect();
    let mut claims = Vec::new();
    for task in &tasks[1..] {
        let Response::Claimed(claim) = d.ok(
            agents[1],
            Request::AttemptStart {
                goal,
                task: *task,
                offer: None,
            },
        ) else {
            panic!()
        };
        claims.push(claim);
    }
    d.ok(
        agents[1],
        Request::AttemptReport {
            goal,
            attempt: claims[0].attempt,
            generation: claims[0].generation,
            status: AttemptStatus::Progress,
            text: "Working".into(),
        },
    );
    d.ok(
        agents[1],
        Request::ContributionPublish {
            goal,
            attempt: Some(claims[1].attempt),
            generation: Some(claims[1].generation),
            summary: "One result".into(),
            sources: vec![],
            artifacts: vec![],
        },
    );
    let work = work_view(&mut d, agents[2], goal);
    assert_eq!(
        work.to_start
            .iter()
            .map(|item| item.task)
            .collect::<Vec<_>>(),
        tasks
    );
    assert!(work.to_start[0].attempting.is_empty());
    assert_eq!(
        work.to_start[1].attempting,
        vec![locust_proto::api::Attempting {
            member: members[1],
            status: Some(AttemptStatus::Progress)
        }]
    );
    assert_eq!(work.to_start[1].results, 0);
    assert_eq!(
        work.to_start[2].attempting,
        vec![locust_proto::api::Attempting {
            member: members[1],
            status: None
        }]
    );
    assert_eq!(work.to_start[2].results, 1);
    d.ok(
        agents[1],
        Request::AttemptReport {
            goal,
            attempt: claims[0].attempt,
            generation: claims[0].generation,
            status: AttemptStatus::Failed,
            text: "Stopped".into(),
        },
    );
    let work = work_view(&mut d, agents[2], goal);
    assert!(
        work.to_start
            .iter()
            .find(|item| item.task == tasks[1])
            .unwrap()
            .attempting
            .is_empty()
    );
    assert_eq!(work.to_start.last().unwrap().task, tasks[2]);
}

#[test]
fn to_review_counts_approvals_and_shows_each_members_latest_verdict() {
    use locust_proto::organization::{CompletionRule, Selector};
    let (mut d, goal, members, agents) = collaboration_view_setup(CompletionRule::Reviews {
        by: Selector::Members,
        count: 2,
        exclude_author: true,
    });
    let subject = event(d.ok(agents[0], finding(goal, "Review this evidence")));
    let review = |verdict| Request::ReviewRecord {
        goal,
        subject,
        verdict,
        text: "Evidence checked".into(),
    };
    d.ok(agents[1], review(ReviewVerdict::Approve));
    d.ok(agents[1], review(ReviewVerdict::Approve));
    let rejected = event(d.ok(agents[1], review(ReviewVerdict::Reject)));
    let work = work_view(&mut d, agents[2], goal);
    let item = work
        .to_review
        .iter()
        .find(|item| item.subject == subject)
        .unwrap();
    assert_eq!(item.needed, 2);
    // A member's latest effective review withdraws its earlier approval.
    assert_eq!(item.approvals, 0);
    assert_eq!(
        item.verdicts,
        vec![locust_proto::api::Verdict {
            opinion: false,
            member: members[1],
            approve: false,
            event: rejected
        }]
    );
    assert!(
        !work_view(&mut d, agents[1], goal)
            .to_review
            .iter()
            .any(|item| item.subject == subject)
    );
    d.ok(agents[2], review(ReviewVerdict::Approve));
    assert!(
        !work_view(&mut d, agents[2], goal)
            .to_review
            .iter()
            .any(|item| item.subject == subject)
    );
}

#[test]
fn to_review_lists_a_result_the_agent_may_attest() {
    use locust_proto::organization::{CompletionRule, Selector};
    let (mut d, goal, _, agents) = collaboration_view_setup(CompletionRule::Check {
        name: "build".into(),
        by: Selector::Members,
    });
    let subject = event(d.ok(agents[0], finding(goal, "Candidate needing a build")));
    let work = work_view(&mut d, agents[1], goal);
    let item = work
        .to_review
        .iter()
        .find(|item| item.subject == subject)
        .unwrap();
    assert_eq!(item.needed, 0);
    assert_eq!(item.approvals, 0);
    assert!(item.verdicts.is_empty());
    d.ok(
        agents[1],
        Request::CheckAttest {
            goal,
            subject,
            name: "build".into(),
            passed: false,
            text: "Build failed".into(),
        },
    );
    assert!(
        !work_view(&mut d, agents[1], goal)
            .to_review
            .iter()
            .any(|item| item.subject == subject)
    );
    assert!(
        work_view(&mut d, agents[2], goal)
            .to_review
            .iter()
            .any(|item| item.subject == subject)
    );
}

#[test]
fn finished_task_allowance_survives_retracted_approval_and_restart_on_same_round() {
    use locust_proto::api::Level;
    use locust_proto::event::{AttemptStatus, Event, TaskId};
    use locust_proto::organization::{CompletionRule, Selector};
    let (mut d, goal, members, agents) = collaboration_view_setup(CompletionRule::Reviews {
        by: Selector::Members,
        count: 1,
        exclude_author: true,
    });
    let owner = d.owner();
    d.ok(
        owner,
        Request::LevelSet {
            goal,
            agent: members[0],
            level: Level::Ask,
        },
    );
    let task = TaskId::Authored(event(d.ok(
        agents[0],
        Request::TaskOpen {
            goal,
            text: "Reopened work".into(),
            task_type: None,
            inputs: Default::default(),
            parent: None,
        },
    )));
    let round = d.node.goals[&goal].state().tasks[&task].current_round;
    d.ok(
        owner,
        Request::TaskAllow {
            goal,
            agent: members[0],
            task,
        },
    );
    let Response::Claimed(claim) = d.ok(
        agents[0],
        Request::AttemptStart {
            goal,
            task,
            offer: None,
        },
    ) else {
        panic!()
    };
    let subject = event(d.ok(
        agents[0],
        Request::ContributionPublish {
            goal,
            attempt: Some(claim.attempt),
            generation: Some(claim.generation),
            summary: "Completed candidate".into(),
            sources: vec![],
            artifacts: vec![],
        },
    ));
    d.ok(
        agents[0],
        Request::AttemptReport {
            goal,
            attempt: claim.attempt,
            generation: claim.generation,
            status: AttemptStatus::Completed,
            text: "Posted".into(),
        },
    );
    let approval = event(d.ok(
        agents[1],
        Request::ReviewRecord {
            goal,
            subject,
            verdict: ReviewVerdict::Approve,
            text: "Approved".into(),
        },
    ));
    assert!(d.node.goals[&goal].state().tasks[&task].rounds[&round].completed);
    let Response::GoalStatus(status) = d.ok(owner, Request::GoalStatus { goal }) else {
        panic!()
    };
    assert!(
        status
            .abilities
            .iter()
            .find(|view| view.agent == members[0])
            .unwrap()
            .allowed_tasks
            .is_empty()
    );
    assert_eq!(
        d.node.goals[&goal]
            .local
            .start_level(task, round, members[0]),
        Level::Ask
    );
    let error = d
        .call(
            agents[0],
            Request::AttemptStart {
                goal,
                task,
                offer: None,
            },
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
    let mut header = d.node.goals[&goal]
        .goal
        .event(&approval)
        .unwrap()
        .header()
        .clone();
    header.at_ms += 1;
    let fork = Event::sign(header, d.node.signer(&members[1]).unwrap()).unwrap();
    let mut tx = crate::node::commit::Tx::none();
    tx.commit.events.push(fork);
    d.node.land(tx).unwrap();
    assert!(!d.node.goals[&goal].state().tasks[&task].rounds[&round].completed);
    assert_eq!(
        d.node.goals[&goal].state().tasks[&task].current_round,
        round
    );
    d.restart();
    let agent = d.connect(credential(1), Some(session(1)));
    let work = work_view(&mut d, agent, goal);
    assert!(work.to_start.iter().any(|item| item.task == task));
    assert!(matches!(
        d.ok(
            agent,
            Request::AttemptStart {
                goal,
                task,
                offer: None
            }
        ),
        Response::Claimed(_)
    ));
}

#[test]
fn goal_status_reports_each_stalled_runner_condition() {
    use crate::node::{
        authoring::sign_at, callers::Actor, commit::Tx, identity::Principals, local,
    };
    use locust_proto::api::{Caller, Stall};
    use locust_proto::event::{Body, Scope};
    use locust_proto::id::EventId;
    use locust_proto::organization::{CompletionRule, Selector};
    for reason in [
        Stall::RunnerRevoked,
        Stall::RunnerLeft,
        Stall::RunnerNotMember,
        Stall::Halted,
    ] {
        let (mut d, goal, members, _) = collaboration_view_setup(CompletionRule::Reviews {
            by: Selector::Members,
            count: 1,
            exclude_author: true,
        });
        let owner = d.owner();
        let runner = members[1];
        let entry = &d.node.goals[&goal];
        let context = entry.goal.current_context(Scope::Goal).unwrap();
        let mut tx = Tx::none();
        d.node
            .sign_for(
                &Actor {
                    caller: Caller::Agent(runner),
                    principal: Some(runner),
                    owner_act: false,
                    session: None,
                },
                entry,
                &runner,
                Body::ContributionPublished {
                    context,
                    attempt: None,
                    sources: vec![],
                    artifacts: vec![],
                },
                Some("Review trigger"),
                1000,
                &mut tx,
            )
            .unwrap();
        // Pause at the durable trigger commit, before drive_flow signs its effects.
        d.node.land_once(tx).unwrap();
        assert!(
            !d.node.goals[&goal]
                .goal
                .evaluation()
                .desired_effects
                .is_empty()
        );
        match reason {
            Stall::CannotMaterialize => {
                unreachable!("tested with an unsignable stage in delivery tests")
            }
            Stall::RunnerRevoked => {
                let mut record = d.node.principals.get(&runner).unwrap().record.clone();
                record.revoked = true;
                let mut tx = Tx::none();
                tx.local(Principals::principal_write(&runner, &record));
                d.node.land_once(tx).unwrap();
            }
            Stall::RunnerLeft => {
                let mut tx = Tx::none();
                tx.local(local::part_write(&goal, &runner, true));
                d.node.land_once(tx).unwrap();
            }
            Stall::RunnerNotMember => {
                d.ok(
                    owner,
                    Request::MemberRemove {
                        goal,
                        member: runner,
                    },
                );
            }
            Stall::Halted => {
                let mut place = d.node.next_place(&d.node.goals[&goal], &runner).unwrap();
                place.seq += 1;
                place.prev = Some(EventId([99; 32]));
                let mut tx = Tx::none();
                sign_at(
                    goal,
                    d.node.signer(&runner).unwrap(),
                    place,
                    Body::ContributionPublished {
                        context,
                        attempt: None,
                        sources: vec![],
                        artifacts: vec![],
                    },
                    None,
                    1001,
                    &mut tx,
                )
                .unwrap();
                // A replica can receive a later record before its predecessor.
                tx.authored = false;
                d.node.land_once(tx).unwrap();
                assert!(d.node.goals[&goal].goal.next(&runner).is_none());
            }
        }
        let Response::GoalStatus(status) = d.ok(owner, Request::GoalStatus { goal }) else {
            panic!()
        };
        assert!(!status.stalled.is_empty(), "{reason:?}");
        assert!(
            status
                .stalled
                .iter()
                .all(|item| item.runner == runner && item.reason == reason),
            "{reason:?}: {:?}",
            status.stalled
        );
        let before = d.store.log(&goal, 0, usize::MAX).unwrap().len();
        d.node.drive_flow(goal).unwrap();
        assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before);
    }
}

#[test]
fn stages_review_requests_and_admissions_need_no_local_work_setting() {
    use locust_proto::api::{Level, Membership};
    use locust_proto::event::{EffectAction, TaskId};
    use locust_proto::organization::{CompletionRule, Selector};
    let (mut d, goal, members, _) = collaboration_view_setup(CompletionRule::Reviews {
        by: Selector::Members,
        count: 1,
        exclude_author: true,
    });
    let owner = d.owner();
    for agent in members {
        d.ok(
            owner,
            Request::LevelSet {
                goal,
                agent,
                level: Level::Read,
            },
        );
    }
    let subject = event(
        d.on_behalf(owner, members[1], finding(goal, "Owner-posted candidate"))
            .unwrap(),
    );
    assert!(d.node.goals[&goal].state().effects.values().any(|effect| {
        matches!(effect.effect.action, EffectAction::RequestReview { subject: candidate, .. } if candidate == subject)
    }));
    let current = d.node.goals[&goal].state().current_rules.unwrap();
    d.ok(owner, super::delivery::pipeline_request(goal, current));
    assert!(
        d.node.goals[&goal]
            .state()
            .tasks
            .keys()
            .any(|task| matches!(task, TaskId::Derived(_)))
    );
    let member = d.enroll("read-joiner", 42);
    let Response::Invited { ticket } = d.ok(
        owner,
        Request::GoalInvite {
            role: None,
            goal,
            expires_ms: 604_801_000,
        },
    ) else {
        panic!()
    };
    let joined = d.ok(
        owner,
        Request::GoalJoin {
            name: "member".into(),
            agent: member,
            ticket,
            level: Level::Read,
        },
    );
    assert!(matches!(
        joined,
        Response::Joined {
            membership: Membership::Member,
            level: Level::Read,
            ..
        }
    ));
    assert!(
        members
            .iter()
            .chain([&member])
            .all(|agent| d.node.goals[&goal].local.level(agent) == Level::Read)
    );
}
