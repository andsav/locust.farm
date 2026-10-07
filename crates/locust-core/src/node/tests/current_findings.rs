//! The first compact page's goal-wide current-findings list: what counts as
//! current, what it shows, and what a read never changes.

use super::lifecycle::{
    authorize, event, finding, finding_without_payload, offered, progress, setup,
};
use super::*;
use locust_proto::api::{
    BRIEF_FINDINGS, ContextCursor, ContextReceipt, ContextSummary, ContextView, ContextViewMode,
    CurrentFindings, FINDING_LINE_CHARS,
};
use locust_proto::event::{Event, TaskId};
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey};
use locust_proto::store::{Space, Store};

fn compact(
    goal: GoalId,
    task: Option<TaskId>,
    after: Option<ContextCursor>,
    limit: u32,
    unread_only: bool,
) -> Request {
    Request::Context {
        goal,
        task,
        after,
        limit,
        preview_chars: None,
        unread_only,
        view: ContextViewMode::Compact,
    }
}

fn read(d: &mut Daemon, conn: ConnId, request: Request) -> ContextView {
    let response = d.ok(conn, request);
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

fn listed(view: &ContextView) -> &CurrentFindings {
    let Some(ContextSummary::Compact(brief)) = view.summary.as_ref() else {
        panic!("expected a compact first-page summary")
    };
    &brief.findings
}

fn brief(view: &ContextView) -> &locust_proto::api::ContextBrief {
    let Some(ContextSummary::Compact(brief)) = view.summary.as_ref() else {
        panic!("expected a compact first-page summary")
    };
    brief
}

fn acknowledge(d: &mut Daemon, conn: ConnId, receipt: ContextReceipt) {
    d.ok(
        conn,
        Request::ContextAcknowledge {
            goal: receipt.goal,
            receipt,
        },
    );
}

fn payload_hash(d: &Daemon, goal: GoalId, finding: EventId) -> BlobHash {
    d.node.goals[&goal]
        .goal
        .event(&finding)
        .expect("the finding is held")
        .header()
        .payload
        .expect("the finding carries a payload")
        .hash
}

fn member_name(d: &Daemon, goal: GoalId, author: PublicKey) -> Option<String> {
    d.node.goals[&goal]
        .state()
        .members
        .get(&author)
        .map(|member| member.name.clone())
}

fn first_line(text: &str) -> String {
    let mut line = text.lines().next().unwrap_or_default().to_owned();
    if let Some((byte, _)) = line.char_indices().nth(FINDING_LINE_CHARS) {
        line.truncate(byte);
    }
    line
}

#[test]
fn the_first_compact_page_lists_the_newest_findings_goal_wide() {
    let (mut d, principal, _, agent, goal) = setup();
    let (member, member_conn) = super::authorization::join_local(&mut d, agent, goal, 2);
    let mut published: Vec<(EventId, PublicKey, String)> = Vec::new();
    for index in 0..25u32 {
        let text = if index >= 20 {
            format!(
                "Finding {index} é {}\nsecond line never shown",
                "✓".repeat(200)
            )
        } else {
            format!("Finding {index} plain")
        };
        let (author, conn) = if index % 3 == 1 {
            (member, member_conn)
        } else {
            (principal, agent)
        };
        let id = event(d.ok(conn, finding(goal, &text)));
        published.push((id, author, text));
    }
    // Newest first: the list's order is the reverse of publication order.
    published.reverse();
    let page = read(&mut d, agent, compact(goal, None, None, 1000, false));
    let findings = listed(&page);
    assert_eq!(findings.total, 25);
    assert_eq!(findings.newest_event, Some(published[0].0));
    assert_eq!(findings.newest.len(), BRIEF_FINDINGS);
    for (headline, (id, author, text)) in findings.newest.iter().zip(&published) {
        assert_eq!(headline.event, *id);
        assert_eq!(headline.author, *author);
        assert_eq!(headline.name, member_name(&d, goal, *author));
        assert_eq!(headline.line.as_deref(), Some(first_line(text).as_str()));
    }
    // The oldest five are counted but not listed.
    for (id, _, _) in published.iter().skip(BRIEF_FINDINGS) {
        assert!(!findings.newest.iter().any(|h| h.event == *id));
    }
    // The first line is cut at 120 Unicode characters, never the whole text.
    let newest = &findings.newest[0];
    assert_eq!(
        newest.line.as_deref(),
        Some(format!("Finding 24 é {}", "✓".repeat(107)).as_str())
    );
    assert_eq!(
        newest.line.as_ref().unwrap().chars().count(),
        FINDING_LINE_CHARS
    );
    assert!(!newest.line.as_ref().unwrap().contains('\n'));
    assert_eq!(newest.name.as_deref(), Some("host"));
}

#[test]
fn a_task_scoped_read_shows_the_same_findings_and_no_attempt_results() {
    let (mut d, principal, owner, agent, goal) = setup();
    let mut published: Vec<(EventId, String)> = Vec::new();
    for index in 0..3 {
        let text = format!("Global finding {index}");
        let id = event(d.ok(agent, finding(goal, &text)));
        published.push((id, text));
    }
    let (task, offer) = offered(&mut d, agent, goal, principal);
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
    let mut scoped = finding(goal, "An attempt's task result");
    let Request::ContributionPublish {
        attempt,
        generation,
        ..
    } = &mut scoped
    else {
        panic!()
    };
    *attempt = Some(claim.attempt);
    *generation = Some(claim.generation);
    let scoped = event(d.ok(agent, scoped));

    let goal_view = read(&mut d, agent, compact(goal, None, None, 1000, false));
    let task_view = read(&mut d, agent, compact(goal, Some(task), None, 1000, false));
    // The list is goal-wide, even in a task-scoped read.
    assert_eq!(listed(&goal_view), listed(&task_view));
    let findings = listed(&goal_view);
    assert_eq!(findings.total, 3);
    assert_eq!(findings.newest_event, Some(published[2].0));
    assert_eq!(findings.newest.len(), 3);
    for (headline, (id, text)) in findings.newest.iter().zip(published.iter().rev()) {
        assert_eq!(headline.event, *id);
        assert_eq!(headline.author, principal);
        assert_eq!(headline.name.as_deref(), Some("host"));
        assert_eq!(headline.line.as_deref(), Some(text.as_str()));
    }
    // A progress note and an attempt's result are work, not findings.
    for id in [report, scoped] {
        assert!(!findings.newest.iter().any(|headline| headline.event == id));
        assert_ne!(findings.newest_event, Some(id));
    }
}

#[test]
fn a_sessionless_observation_lists_findings_without_a_receipt() {
    let (mut d, _, _, _, goal) = setup();
    let observer = d.connect(credential(1), None);
    let finding = event(d.ok(observer, finding(goal, "Readable without a session")));
    let page = read(&mut d, observer, compact(goal, None, None, 1000, true));
    let findings = listed(&page);
    assert_eq!(findings.total, 1);
    assert_eq!(findings.newest_event, Some(finding));
    assert_eq!(findings.newest.len(), 1);
    assert_eq!(
        findings.newest[0].line.as_deref(),
        Some("Readable without a session")
    );
    assert!(page.receipt.is_none());
}

#[test]
fn listing_marks_nothing_read_and_never_repeats_on_a_continuation() {
    let (mut d, _, _, agent, goal) = setup();
    for index in 0..3 {
        event(d.ok(
            agent,
            finding(goal, &format!("Finding {index} stays unread")),
        ));
    }
    let before = d.store.scan(Space::Cursor, &[]).unwrap();
    let first = read(&mut d, agent, compact(goal, None, None, 1, false));
    assert_eq!(listed(&first).total, 3);
    assert_eq!(listed(&first).newest.len(), 3);
    let news = brief(&first).context_news;
    let receipt = first
        .receipt
        .clone()
        .expect("a session page carries a receipt");
    // One delivered item, one receipt entry: listing adds none.
    assert_eq!(receipt.entries.len(), 1);
    assert_eq!(receipt.entries[0].event, first.items[0].event.view.event);
    assert!(first.next.is_some());
    let again = read(&mut d, agent, compact(goal, None, None, 1, false));
    assert_eq!(again, first);
    assert_eq!(brief(&again).context_news, news);
    assert_eq!(d.store.scan(Space::Cursor, &[]).unwrap(), before);
    // A continuation carries items, never the summary or its findings.
    let continuation = read(
        &mut d,
        agent,
        compact(goal, None, first.next.clone(), 1, false),
    );
    assert!(continuation.summary.is_none());
}

#[test]
fn an_acknowledged_unchanged_checkpoint_hides_the_list_but_not_the_counts() {
    let (mut d, _, _, agent, goal) = setup();
    let oldest = event(d.ok(agent, finding(goal, "Older finding")));
    let newest = event(d.ok(agent, finding(goal, "Newer finding")));
    let Response::Context(full) = d.ok(
        agent,
        Request::Context {
            goal,
            task: None,
            after: None,
            limit: 1000,
            preview_chars: None,
            unread_only: true,
            view: ContextViewMode::Full,
        },
    ) else {
        panic!()
    };
    acknowledge(&mut d, agent, full.receipt.clone().unwrap());
    let suppressed = read(&mut d, agent, compact(goal, None, None, 1000, true));
    let findings = listed(&suppressed);
    assert_eq!(findings.total, 2);
    assert_eq!(findings.newest_event, Some(newest));
    assert!(findings.newest.is_empty());
    // A read without `unread_only` always shows the list.
    let shown = read(&mut d, agent, compact(goal, None, None, 1000, false));
    let findings = listed(&shown);
    assert_eq!(findings.total, 2);
    assert_eq!(findings.newest_event, Some(newest));
    assert_eq!(findings.newest.len(), 2);
    assert_eq!(findings.newest[0].line.as_deref(), Some("Newer finding"));
    assert_eq!(findings.newest[1].line.as_deref(), Some("Older finding"));
    assert_eq!(findings.newest[0].event, newest);
    assert_eq!(findings.newest[1].event, oldest);
    // Sessions acknowledge independently.
    let second = d.connect(credential(1), Some(session(2)));
    let fresh = read(&mut d, second, compact(goal, None, None, 1000, true));
    assert_eq!(listed(&fresh).newest.len(), 2);
    // The suppression is durable across a restart.
    d.restart();
    let agent = d.connect(credential(1), Some(session(1)));
    let after = read(&mut d, agent, compact(goal, None, None, 1000, true));
    assert!(listed(&after).newest.is_empty());
    assert_eq!(listed(&after).total, 2);
    assert_eq!(listed(&after).newest_event, Some(newest));
}

#[test]
fn an_unread_finding_beyond_the_listed_headlines_fills_the_list() {
    let (mut d, _, _, agent, goal) = setup();
    let mut ids = Vec::new();
    for index in 0..25 {
        ids.push(event(
            d.ok(agent, finding(goal, &format!("Finding {index} of many"))),
        ));
    }
    let oldest = ids[0];
    // Acknowledge every event except the oldest finding, one page at a time,
    // so that everything the list would show is read while one finding
    // outside it stays unread.
    let mut after = None;
    loop {
        let Response::Context(page) = d.ok(
            agent,
            Request::Context {
                goal,
                task: None,
                after: after.clone(),
                limit: 1,
                preview_chars: None,
                unread_only: true,
                view: ContextViewMode::Full,
            },
        ) else {
            panic!()
        };
        if let Some(receipt) = &page.receipt
            && receipt.entries.iter().all(|seen| seen.event != oldest)
        {
            acknowledge(&mut d, agent, receipt.clone());
        }
        after = page.next;
        if after.is_none() {
            break;
        }
    }
    let before = d.store.scan(Space::Cursor, &[]).unwrap();
    let page = read(&mut d, agent, compact(goal, None, None, 1000, true));
    let findings = listed(&page);
    assert_eq!(findings.total, 25);
    assert_eq!(findings.newest_event, Some(ids[24]));
    assert_eq!(findings.newest.len(), BRIEF_FINDINGS);
    for (headline, id) in findings.newest.iter().zip(ids[5..].iter().rev()) {
        assert_eq!(headline.event, *id);
    }
    assert!(!findings.newest.iter().any(|h| h.event == oldest));
    assert_eq!(d.store.scan(Space::Cursor, &[]).unwrap(), before);
    // A later read still reports the same page: nothing was consumed.
    assert_eq!(
        read(&mut d, agent, compact(goal, None, None, 1000, true)),
        page
    );
}

#[test]
fn a_withdrawn_payload_removes_a_finding_even_beyond_the_headlines() {
    let (mut d, _, _, agent, goal) = setup();
    let mut ids = Vec::new();
    for index in 0..25 {
        ids.push(event(d.ok(
            agent,
            finding(goal, &format!("Finding {index} of withdrawn")),
        )));
    }
    let before = read(&mut d, agent, compact(goal, None, None, 1000, false));
    assert_eq!(listed(&before).total, 25);
    // An old finding, outside the twenty headlines, withdrawn here.
    d.ok(
        agent,
        Request::BlobWithdraw {
            goal,
            hash: payload_hash(&d, goal, ids[1]),
        },
    );
    let after = read(&mut d, agent, compact(goal, None, None, 1000, false));
    let findings = listed(&after);
    assert_eq!(findings.total, 24);
    assert_eq!(findings.newest_event, Some(ids[24]));
    assert_eq!(findings.newest, listed(&before).newest);
    assert!(!findings.newest.iter().any(|h| h.event == ids[1]));
}

#[test]
fn an_unreadable_withdrawal_record_cannot_be_counted_as_current() {
    let (mut d, _, _, agent, goal) = setup();
    let subject = event(d.ok(agent, finding(goal, "Evidence")));
    let hash = payload_hash(&d, goal, subject);
    let mut tx = crate::node::commit::Tx::none();
    tx.local(locust_proto::store::LocalWrite::Put {
        space: Space::Blob,
        key: crate::node::requests::content::blob_key(&goal, &hash),
        value: vec![255],
    });
    d.store.commit(&tx.commit).unwrap();
    assert_eq!(
        code(d.call(agent, compact(goal, None, None, 1, false))),
        ErrorCode::Corrupted
    );
}

#[test]
fn a_missing_payload_stays_current_without_a_line_until_it_arrives() {
    let (mut d, principal, _, agent, goal) = setup();
    let present = event(d.ok(agent, finding(goal, "Readable finding")));
    let (missing, payload) = finding_without_payload(&mut d, goal, principal, "Late finding");
    let page = read(&mut d, agent, compact(goal, None, None, 1000, false));
    let findings = listed(&page);
    assert_eq!(findings.total, 2);
    assert_eq!(findings.newest_event, Some(missing));
    let [late, readable] = findings.newest.as_slice() else {
        panic!("expected two headlines")
    };
    assert_eq!(late.event, missing);
    assert_eq!(late.author, principal);
    assert_eq!(late.name.as_deref(), Some("host"));
    assert_eq!(late.line, None);
    assert_eq!(readable.event, present);
    assert_eq!(readable.line.as_deref(), Some("Readable finding"));
    // Once the payload arrives here, the same finding has a line.
    let mut arrive = crate::node::commit::Tx::none();
    arrive.commit.blobs.push(payload);
    arrive.touch(goal);
    d.node.land(arrive).unwrap();
    let arrived = read(&mut d, agent, compact(goal, None, None, 1000, false));
    let findings = listed(&arrived);
    assert_eq!(findings.total, 2);
    assert_eq!(findings.newest[0].event, missing);
    assert_eq!(findings.newest[0].line.as_deref(), Some("Late finding"));
}

#[test]
fn a_finding_that_stops_being_effective_is_no_longer_current() {
    let (mut d, _, _, agent, goal) = setup();
    let (member, member_conn) = super::authorization::join_local(&mut d, agent, goal, 2);
    let subject = event(d.ok(member_conn, finding(goal, "Previously effective evidence")));
    let page = read(&mut d, agent, compact(goal, None, None, 1000, false));
    assert_eq!(listed(&page).total, 1);
    assert_eq!(listed(&page).newest_event, Some(subject));
    // A fork of the finding's event makes neither copy effective.
    let mut header = d.node.goals[&goal]
        .goal
        .event(&subject)
        .unwrap()
        .header()
        .clone();
    header.at_ms += 1;
    let fork = Event::sign(header, d.node.signer(&member).unwrap()).unwrap();
    let mut tx = crate::node::commit::Tx::none();
    tx.commit.events.push(fork);
    d.node.land(tx).unwrap();
    let changed = read(&mut d, agent, compact(goal, None, None, 1000, false));
    let findings = listed(&changed);
    assert_eq!(findings.total, 0);
    assert_eq!(findings.newest_event, None);
    assert!(findings.newest.is_empty());
}

#[test]
fn a_removed_members_finding_stays_current_with_its_name() {
    let (mut d, _, _, agent, goal) = setup();
    let (member, member_conn) = super::authorization::join_local(&mut d, agent, goal, 2);
    let held = event(d.ok(member_conn, finding(goal, "Evidence before removal")));
    let page = read(&mut d, agent, compact(goal, None, None, 1000, false));
    assert_eq!(listed(&page).newest[0].name.as_deref(), Some("member"));
    let owner = d.owner();
    let removal = event(d.ok(owner, Request::MemberRemove { goal, member }));
    // Removal keeps the member's record, so their earlier finding stays
    // current and still carries their name.
    let record = d.node.goals[&goal]
        .state()
        .members
        .get(&member)
        .expect("a removed member keeps its record");
    assert_eq!(record.removed, Some(removal));
    let changed = read(&mut d, agent, compact(goal, None, None, 1000, false));
    let list = listed(&changed);
    assert_eq!(list.total, 1);
    assert_eq!(list.newest_event, Some(held));
    assert_eq!(
        list.newest[0].line.as_deref(),
        Some("Evidence before removal")
    );
    assert_eq!(list.newest[0].name.as_deref(), Some("member"));
    assert_eq!(
        code(d.call(member_conn, finding(goal, "after removal"))),
        ErrorCode::Denied
    );
}

#[test]
fn a_headline_for_an_author_the_members_map_does_not_know_is_unnamed() {
    let (mut d, _, _, agent, goal) = setup();
    event(d.ok(agent, finding(goal, "A held finding")));
    let author = locust_proto::crypto::Keypair::from_seed([9; 32]).public();
    let event = Event::sign(
        locust_proto::event::Header {
            version: locust_proto::PROTOCOL_VERSION,
            goal,
            author,
            seq: 0,
            prev: None,
            anchor: Some(EventId([0; 32])),
            parents: vec![],
            at_ms: 1000,
            payload: None,
            body: locust_proto::event::Body::ContributionPublished {
                context: locust_proto::event::Context {
                    scope: locust_proto::event::Scope::Goal,
                    round: EventId([0; 32]),
                },
                attempt: None,
                sources: Vec::new(),
                artifacts: vec![],
            },
        },
        &locust_proto::crypto::Keypair::from_seed([9; 32]),
    )
    .unwrap();
    let headline = d.node.finding_headline(&d.node.goals[&goal], &event, None);
    assert_eq!(headline.event, event.id());
    assert_eq!(headline.author, author);
    assert_eq!(headline.name, None);
    assert_eq!(headline.line, None);
}
