//! Complete receipt delivery, independent sessions, and observation-only reads.

use super::lifecycle::{authorize, event, finding, offered, progress, setup};
use super::*;
use locust_proto::api::{ContextCursor, ContextReceipt, ContextView};
use locust_proto::event::{Doc, ReviewVerdict, TaskId};
use locust_proto::id::{EventId, GoalId};
use locust_proto::store::{Blob, Space, Store};

fn read(
    goal: GoalId,
    task: Option<TaskId>,
    after: Option<ContextCursor>,
    limit: u32,
    preview_chars: Option<u32>,
    unread_only: bool,
) -> Request {
    Request::Context {
        goal,
        task,
        after,
        limit,
        preview_chars,
        unread_only,
    }
}

fn context(
    d: &mut Daemon,
    conn: ConnId,
    goal: GoalId,
    task: Option<TaskId>,
    unread: bool,
) -> ContextView {
    let Response::Context(context) = d.ok(conn, read(goal, task, None, 1000, None, unread)) else {
        panic!()
    };
    *context
}

fn acknowledge(d: &mut Daemon, conn: ConnId, receipt: ContextReceipt) -> Response {
    d.ok(
        conn,
        Request::ContextAcknowledge {
            goal: receipt.goal,
            receipt,
        },
    )
}

fn contains(context: &ContextView, event: EventId) -> bool {
    context
        .items
        .iter()
        .any(|item| item.event.view.event == event)
}

#[test]
fn context_reads_and_lost_responses_consume_nothing_and_sessions_ack_independently() {
    let (mut d, _, _, first, goal) = setup();
    let second = d.connect(credential(1), Some(session(2)));
    let finding = event(d.ok(
        first,
        finding(goal, "Use the shared parser; do not duplicate it."),
    ));
    let initial = d.store.scan(Space::Cursor, &[]).unwrap();
    let lost = context(&mut d, first, goal, None, true);
    assert!(contains(&lost, finding));
    assert_eq!(context(&mut d, first, goal, None, true), lost);
    assert_eq!(d.store.scan(Space::Cursor, &[]).unwrap(), initial);
    let receipt = lost.receipt.unwrap();
    assert_eq!(
        code(d.call(
            second,
            Request::ContextAcknowledge {
                goal,
                receipt: receipt.clone()
            }
        )),
        ErrorCode::Denied
    );
    let acknowledged = acknowledge(&mut d, first, receipt.clone());
    assert_eq!(acknowledge(&mut d, first, receipt), acknowledged);
    assert!(!contains(
        &context(&mut d, first, goal, None, true),
        finding
    ));
    assert!(contains(
        &context(&mut d, second, goal, None, true),
        finding
    ));
    let Response::Pending(pending) = d.ok(first, Request::Pending { goal }) else {
        panic!()
    };
    assert_eq!(pending.context_news.unwrap().unacknowledged, 0);
    let Response::Pending(pending) = d.ok(second, Request::Pending { goal }) else {
        panic!()
    };
    assert!(pending.context_news.unwrap().unacknowledged > 0);
}

#[test]
fn lost_ack_response_and_unacknowledged_receipt_survive_restart() {
    let (mut d, _, _, first, goal) = setup();
    let finding = event(d.ok(first, finding(goal, "Durable context")));
    let receipt = context(&mut d, first, goal, None, true).receipt.unwrap();
    d.restart();
    let first = d.connect(credential(1), Some(session(1)));
    let lost_response = acknowledge(&mut d, first, receipt.clone());
    d.restart();
    let first = d.connect(credential(1), Some(session(1)));
    assert_eq!(acknowledge(&mut d, first, receipt), lost_response);
    assert!(!contains(
        &context(&mut d, first, goal, None, true),
        finding
    ));
    let second = d.connect(credential(1), Some(session(2)));
    assert!(contains(
        &context(&mut d, second, goal, None, true),
        finding
    ));
}

#[test]
fn preview_and_unavailable_payload_cannot_acknowledge_text_and_late_payload_is_news() {
    let (mut d, _, _, agent, goal) = setup();
    let finding = event(d.ok(
        agent,
        finding(goal, "évidence in full, never silently truncated"),
    ));
    let Response::Context(preview) = d.ok(agent, read(goal, None, None, 1000, Some(1), true))
    else {
        panic!()
    };
    let item = preview
        .items
        .iter()
        .find(|item| item.event.view.event == finding)
        .unwrap();
    assert_eq!(item.event.text.as_deref(), Some("é"));
    assert!(!item.text_complete);
    if let Some(receipt) = preview.receipt {
        assert!(!receipt.entries.iter().any(|entry| entry.event == finding));
        acknowledge(&mut d, agent, receipt);
    }
    assert!(contains(&context(&mut d, agent, goal, None, true), finding));
    let hash = d.node.goals[&goal]
        .goal
        .event(&finding)
        .unwrap()
        .header()
        .payload
        .unwrap()
        .hash;
    let bytes = d.store.blob(&hash).unwrap().unwrap();
    let mut drop = crate::node::commit::Tx::none();
    drop.commit.drop_blobs.push(hash);
    drop.touch(goal);
    d.node.land(drop).unwrap();
    let unavailable = context(&mut d, agent, goal, None, true);
    let item = unavailable
        .items
        .iter()
        .find(|item| item.event.view.event == finding)
        .unwrap();
    assert_eq!(item.event.text, None);
    assert!(!item.text_complete);
    assert!(
        unavailable
            .receipt
            .as_ref()
            .is_none_or(|receipt| receipt.entries.iter().all(|entry| entry.event != finding))
    );
    assert!(unavailable.pending.context_news.unwrap().unavailable > 0);
    if let Some(receipt) = unavailable.receipt {
        acknowledge(&mut d, agent, receipt);
    }
    d.restart();
    let agent = d.connect(credential(1), Some(session(1)));
    let mut arrive = crate::node::commit::Tx::none();
    arrive.commit.blobs.push(Blob::new(bytes));
    arrive.touch(goal);
    d.node.land(arrive).unwrap();
    let arrived = context(&mut d, agent, goal, None, true);
    let item = arrived
        .items
        .iter()
        .find(|item| item.event.view.event == finding)
        .unwrap();
    assert!(item.text_complete);
    assert_eq!(
        item.event.text.as_deref(),
        Some("évidence in full, never silently truncated")
    );
    acknowledge(&mut d, agent, arrived.receipt.unwrap());
    assert!(!contains(
        &context(&mut d, agent, goal, None, true),
        finding
    ));
}

#[test]
fn viewers_owners_and_event_pagination_never_consume_session_context() {
    let (mut d, principal, owner, agent, goal) = setup();
    let finding = event(d.ok(agent, finding(goal, "Visible to observers")));
    d.ok(
        owner,
        Request::ViewerEnroll {
            agent: principal,
            credential: credential(9).digest(),
        },
    );
    let viewer = d.connect(credential(9), None);
    let before = d.store.scan(Space::Cursor, &[]).unwrap();
    assert!(context(&mut d, viewer, goal, None, true).receipt.is_none());
    assert!(context(&mut d, owner, goal, None, true).receipt.is_none());
    let events = d.ok(
        agent,
        Request::Events {
            goal,
            after: None,
            limit: 256,
        },
    );
    d.ok(
        agent,
        Request::Events {
            goal,
            after: Some(999),
            limit: 256,
        },
    );
    d.ok(
        viewer,
        Request::Events {
            goal,
            after: Some(999),
            limit: 256,
        },
    );
    assert_eq!(
        events,
        d.ok(
            agent,
            Request::Events {
                goal,
                after: None,
                limit: 256
            }
        )
    );
    assert_eq!(d.store.scan(Space::Cursor, &[]).unwrap(), before);
    assert!(contains(&context(&mut d, agent, goal, None, true), finding));
}

#[test]
fn task_context_joins_scoped_progress_findings_reviews_and_documents_at_one_revision() {
    let (mut d, principal, owner, agent, goal) = setup();
    let (task, offer) = offered(&mut d, agent, goal, principal);
    let (unrelated_task, _) = offered(&mut d, agent, goal, principal);
    authorize(&mut d, owner, goal, task, principal);
    let Response::Claimed(claim) = d.ok(
        agent,
        Request::AttemptStart {
            goal,
            task,
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    let report = event(d.ok(agent, progress(goal, claim.attempt, claim.generation)));
    let global = event(d.ok(agent, finding(goal, "Shared constraint")));
    let mut scoped = finding(goal, "Task finding");
    let Request::ContributionPublish { task: scope, .. } = &mut scoped else {
        panic!()
    };
    *scope = Some(task);
    let scoped = event(d.ok(agent, scoped));
    let review = event(d.ok(
        agent,
        Request::ReviewRecord {
            goal,
            subject: scoped,
            verdict: ReviewVerdict::Reject,
            text: "Missing retry evidence".into(),
        },
    ));
    let document = event(d.ok(
        agent,
        Request::DocRevise {
            goal,
            doc: Doc::Plan,
            base: None,
            text: "Shared implementation plan".into(),
        },
    ));
    let view = context(&mut d, agent, goal, Some(task), false);
    assert_eq!(view.revision, view.pending.revision);
    assert_eq!(view.task.as_ref().unwrap().view.task, task);
    for id in [report, global, scoped, review, document] {
        assert!(contains(&view, id));
    }
    assert!(
        view.items
            .iter()
            .all(|item| item.event.task != Some(unrelated_task))
    );
    let review = view
        .items
        .iter()
        .find(|item| item.event.view.event == review)
        .unwrap();
    assert_eq!(review.event.text.as_deref(), Some("Missing retry evidence"));
    assert_eq!(review.event.view.author, principal);
    assert!(!view.effective_rules_json.is_empty());
}

#[test]
fn continuation_is_complete_and_rejects_mixed_revisions_and_sessions() {
    let (mut d, _, _, agent, goal) = setup();
    event(d.ok(agent, finding(goal, "Finding to paginate")));
    let all = context(&mut d, agent, goal, None, false);
    let mut ids = Vec::new();
    let mut after = None;
    loop {
        let Response::Context(page) = d.ok(agent, read(goal, None, after, 1, None, false)) else {
            panic!()
        };
        assert_eq!(page.revision, all.revision);
        ids.extend(page.items.iter().map(|item| item.event.view.event));
        after = page.next;
        if after.is_none() {
            break;
        }
    }
    assert_eq!(
        ids,
        all.items
            .iter()
            .map(|item| item.event.view.event)
            .collect::<Vec<_>>()
    );
    let Response::Context(first) = d.ok(agent, read(goal, None, None, 1, None, false)) else {
        panic!()
    };
    let second = d.connect(credential(1), Some(session(2)));
    assert_eq!(
        code(d.call(second, read(goal, None, first.next.clone(), 1, None, false))),
        ErrorCode::Conflict
    );
    event(d.ok(agent, finding(goal, "A concurrent change")));
    assert_eq!(
        code(d.call(agent, read(goal, None, first.next, 1, None, false))),
        ErrorCode::Conflict
    );
}

#[test]
fn receipts_are_not_forgeable_or_replayable_through_another_sessions_idempotency() {
    let (mut d, _, _, agent, goal) = setup();
    let mut receipt = context(&mut d, agent, goal, None, true).receipt.unwrap();
    receipt.entries[0].version.0[0] ^= 1;
    assert_eq!(
        code(d.call(agent, Request::ContextAcknowledge { goal, receipt })),
        ErrorCode::Denied
    );
    let receipt = context(&mut d, agent, goal, None, true).receipt.unwrap();
    let request = Request::ContextAcknowledge { goal, receipt };
    d.keyed(agent, 23, request.clone()).unwrap();
    let second = d.connect(credential(1), Some(session(2)));
    let error = code(d.keyed(second, 23, request));
    assert!(matches!(
        error,
        ErrorCode::IdempotencyMismatch | ErrorCode::Denied
    ));
}

#[test]
fn unread_continuation_allows_acknowledging_each_page_without_skips() {
    let (mut d, _, _, agent, goal) = setup();
    for finding_text in ["First finding", "Second finding", "Third finding"] {
        event(d.ok(agent, finding(goal, finding_text)));
    }
    let all = context(&mut d, agent, goal, None, true);
    let expected: Vec<_> = all.items.iter().map(|item| item.event.view.event).collect();
    let mut ids = Vec::new();
    let mut after = None;
    loop {
        let Response::Context(page) = d.ok(agent, read(goal, None, after, 1, None, true)) else {
            panic!()
        };
        assert_eq!(page.revision, all.revision);
        ids.extend(page.items.iter().map(|item| item.event.view.event));
        if let Some(receipt) = page.receipt {
            acknowledge(&mut d, agent, receipt);
        }
        after = page.next;
        if after.is_none() {
            break;
        }
    }
    assert_eq!(ids, expected);
    assert!(context(&mut d, agent, goal, None, true).items.is_empty());
}

#[test]
fn acknowledging_an_older_receipt_leaves_later_findings_unread() {
    let (mut d, _, _, agent, goal) = setup();
    let earlier = event(d.ok(agent, finding(goal, "Earlier finding")));
    let older = context(&mut d, agent, goal, None, true).receipt.unwrap();
    let later = event(d.ok(agent, finding(goal, "Later finding")));
    acknowledge(&mut d, agent, older.clone());
    let unread = context(&mut d, agent, goal, None, true);
    assert!(!contains(&unread, earlier));
    assert!(contains(&unread, later));
    acknowledge(&mut d, agent, unread.receipt.unwrap());
    acknowledge(&mut d, agent, older);
    assert!(context(&mut d, agent, goal, None, true).items.is_empty());
}

#[test]
fn task_brief_uses_pinned_task_type_and_task_inputs_instead_of_goal_defaults() {
    use locust_proto::organization::{
        CompletionRule, DecisionRules, Formation, Input, InputKind, Selector, TaskType,
    };
    use std::collections::BTreeMap;

    let (mut d, _, _, agent, goal) = setup();
    let mut formation = Formation::default();
    formation.context.inputs.insert(
        "workspace".into(),
        Input {
            kind: InputKind::Artifact,
            required: false,
        },
    );
    formation.task_types.insert(
        "reviewed".into(),
        TaskType {
            work: None,
            decisions: Some(DecisionRules {
                completion: CompletionRule::Reviews {
                    by: Selector::Members,
                    count: 1,
                    exclude_author: false,
                },
                selection: None,
                finish: None,
            }),
        },
    );
    let expected = context(&mut d, agent, goal, None, false)
        .status
        .current_rules
        .unwrap();
    d.ok(
        agent,
        Request::RulesBind {
            goal,
            expected,
            formation_json: serde_json::to_string(&formation).unwrap(),
            roles: BTreeMap::new(),
            inputs: BTreeMap::new(),
        },
    );
    let Response::BlobStored { hash } = d.ok(
        agent,
        Request::BlobPut {
            goal,
            bytes: b"task input".to_vec(),
        },
    ) else {
        panic!()
    };
    let task = TaskId::Authored(event(d.ok(
        agent,
        Request::TaskOpen {
            goal,
            text: "A task with different completion rules".into(),
            task_type: Some("reviewed".into()),
            inputs: BTreeMap::from([("workspace".into(), hash)]),
            parent: None,
        },
    )));
    let goal_view = context(&mut d, agent, goal, None, false);
    let task_view = context(&mut d, agent, goal, Some(task), false);
    let detail = task_view.task.as_ref().unwrap();
    assert_eq!(detail.task_type.as_deref(), Some("reviewed"));
    assert_eq!(task_view.effective_rules_json, detail.effective_rules_json);
    assert_ne!(
        task_view.effective_rules_json,
        goal_view.effective_rules_json
    );
    assert_eq!(
        task_view.inputs,
        BTreeMap::from([("workspace".into(), hash)])
    );
    assert!(goal_view.inputs.is_empty());
}
