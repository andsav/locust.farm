//! Public Engine transcripts: independent evidence, local claims and replay.
use super::*;
use locust_proto::api::{PendingWork, SessionCapabilities, SessionRecord, SessionState};
use locust_proto::engine::{PeerEngine, PeerInput};
use locust_proto::event::{AttemptStatus, CancelOutcome, Doc, ReviewVerdict, TaskId};
use locust_proto::id::{EndpointId, EventId, GoalId};
use locust_proto::store::Store;

pub(super) fn setup() -> (Daemon, PublicKey, ConnId, ConnId, GoalId) {
    let mut daemon = Daemon::new(21);
    daemon.node.peer(
        PeerInput::Endpoint {
            endpoint: EndpointId([21; 32]),
            hints: vec![],
        },
        locust_proto::engine::PeerTime {
            unix_ms: 0,
            elapsed_ms: 0,
        },
        &mut Vec::new(),
    );
    let principal = daemon.enroll("host", 1);
    let owner = daemon.owner();
    let agent = daemon.connect(credential(1), Some(session(1)));
    let mut formation = locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == "directed")
        .unwrap()
        .formation;
    formation.context.inputs.insert(
        "workspace".into(),
        locust_proto::organization::Input {
            kind: locust_proto::organization::InputKind::Artifact,
            required: false,
        },
    );
    let Response::GoalCreated { goal } = daemon.ok(
        owner,
        Request::GoalCreate {
            name: "host".into(),
            agent: principal,
            title: "A test goal".into(),
            formation_json: Some(serde_json::to_string(&formation).unwrap()),

            inputs: Default::default(),
        },
    ) else {
        panic!()
    };
    daemon.ok(
        owner,
        Request::LevelSet {
            goal,
            agent: principal,
            level: locust_proto::api::Level::Ask,
        },
    );
    (daemon, principal, owner, agent, goal)
}
pub(super) fn event(response: Response) -> EventId {
    let Response::Recorded { event } = response else {
        panic!("{response:?}")
    };
    event
}
pub(super) fn finding(goal: GoalId, text: &str) -> Request {
    Request::ContributionPublish {
        goal,
        attempt: None,
        generation: None,
        summary: text.into(),
        sources: Vec::new(),
        artifacts: vec![],
    }
}
/// Accepts an authentic event of `author` as a peer delivers it. Only the
/// event is delivered, not its payload object.
pub(super) fn receive_payload(
    daemon: &mut Daemon,
    goal: locust_proto::id::GoalId,
    author: PublicKey,
    payload: locust_proto::event::PayloadRef,
    mut body: locust_proto::event::Body,
) -> locust_proto::id::EventId {
    use crate::sync::Host;
    use locust_proto::event::{Event, Header};
    let entry = &daemon.node.goals[&goal];
    match &mut body {
        locust_proto::event::Body::TaskOpened { binding } => {
            binding.rules = entry.state().current_rules.unwrap()
        }
        locust_proto::event::Body::ContributionPublished { context, .. } => {
            context.round = entry.state().current_rules.unwrap()
        }
        _ => (),
    }
    let next = entry.goal.next(&author).unwrap();
    let event = Event::sign(
        Header {
            version: locust_proto::PROTOCOL_VERSION,
            goal,
            author,
            seq: next.seq,
            prev: next.prev,
            anchor: Some(next.anchor),
            parents: vec![],
            at_ms: 1000,
            payload: Some(payload),
            body,
        },
        daemon.node.signer(&author).unwrap(),
    )
    .unwrap();
    assert_eq!(
        Host::replica(&mut daemon.node, &goal)
            .unwrap()
            .receive(vec![event.to_wire()]),
        Ok(1)
    );
    event.id()
}

/// Delivers `author`'s finding of `text` as a peer would, without its
/// payload: the event is held and its payload has not arrived. Returns the
/// event and the sealed payload, to deliver later.
pub(super) fn finding_without_payload(
    daemon: &mut Daemon,
    goal: GoalId,
    author: PublicKey,
    text: &str,
) -> (EventId, locust_proto::store::Blob) {
    use locust_proto::event::{Body, Context, PayloadRef, Scope};
    let (epoch, key) = daemon.node.goals[&goal].keys.last_key_value().unwrap();
    let blob = locust_proto::store::Blob::new(
        locust_proto::seal::seal(&goal, *epoch, key, text.as_bytes()).unwrap(),
    );
    let payload = PayloadRef {
        hash: blob.hash(),
        len: blob.bytes().len() as u32,
        key_epoch: *epoch,
    };
    let event = receive_payload(
        daemon,
        goal,
        author,
        payload,
        Body::ContributionPublished {
            context: Context {
                scope: Scope::Goal,
                round: EventId([0; 32]),
            },
            attempt: None,
            sources: Vec::new(),
            artifacts: vec![],
        },
    );
    (event, blob)
}
fn pending(daemon: &mut Daemon, conn: ConnId, goal: GoalId) -> PendingWork {
    let Response::Pending(work) = daemon.ok(conn, Request::Pending { goal }) else {
        panic!()
    };
    work
}
pub(super) fn offered(
    daemon: &mut Daemon,
    agent: ConnId,
    goal: GoalId,
    principal: PublicKey,
) -> (TaskId, EventId) {
    let task = TaskId::Authored(event(daemon.ok(
        agent,
        Request::TaskOpen {
            goal,
            text: "Read and implement\nAcceptance details".into(),
            task_type: None,
            inputs: Default::default(),
            parent: None,
        },
    )));
    let offer = event(daemon.ok(
        agent,
        Request::WorkOffer {
            goal,
            task,
            recipient: principal,
        },
    ));
    (task, offer)
}
pub(super) fn authorize(
    daemon: &mut Daemon,
    owner: ConnId,
    goal: GoalId,
    task: TaskId,
    agent: PublicKey,
) {
    daemon.ok(owner, Request::TaskAllow { goal, task, agent });
}
pub(super) fn progress(goal: GoalId, attempt: EventId, generation: u32) -> Request {
    Request::AttemptReport {
        goal,
        attempt,
        generation,
        status: AttemptStatus::Progress,
        text: "still working".into(),
    }
}
fn publish(goal: GoalId, attempt: EventId, generation: u32) -> Request {
    Request::ContributionPublish {
        goal,
        attempt: Some(attempt),
        generation: Some(generation),
        summary: "Completed with evidence".into(),
        sources: Vec::new(),
        artifacts: vec![],
    }
}

#[test]
fn complete_transcript_keeps_contribution_review_and_selection_separate_after_restart() {
    let (mut d, p, owner, a, goal) = setup();
    let (task, offer) = offered(&mut d, a, goal, p);
    assert_eq!(pending(&mut d, owner, goal).ask_first[0].offer, Some(offer));
    assert_eq!(
        code(d.call(
            a,
            Request::AttemptStart {
                goal,
                task: Some(task),
                offer: Some(offer)
            }
        )),
        ErrorCode::LevelRequired
    );
    authorize(&mut d, owner, goal, task, p);
    let Response::Claimed(claim) = d.ok(
        a,
        Request::AttemptStart {
            goal,
            task: Some(task),
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    assert_eq!(pending(&mut d, a, goal).claimed, vec![claim]);
    d.ok(a, progress(goal, claim.attempt, 1));
    let contribution = event(d.ok(a, publish(goal, claim.attempt, 1)));
    assert!(
        pending(&mut d, a, goal)
            .to_review
            .iter()
            .any(|item| item.subject == contribution)
    );
    let Response::Task(detail) = d.ok(a, Request::Task { goal, task }) else {
        panic!()
    };
    assert!(!detail.view.completed);
    assert_eq!(detail.view.selected, None);
    d.ok(
        a,
        Request::ReviewRecord {
            goal,
            subject: contribution,
            verdict: ReviewVerdict::Approve,
            text: "reviewed".into(),
        },
    );
    let Response::Task(detail) = d.ok(a, Request::Task { goal, task }) else {
        panic!()
    };
    assert!(detail.view.completed);
    assert_eq!(detail.view.selected, None);
    d.ok(
        a,
        Request::ScopeSelect {
            goal,
            subject: contribution,
            expected: None,
        },
    );
    d.ok(
        a,
        Request::AttemptReport {
            goal,
            attempt: claim.attempt,
            generation: 1,
            status: AttemptStatus::Completed,
            text: "done".into(),
        },
    );
    let expected = d.ok(a, Request::Board { goal });
    let feed = d.ok(
        a,
        Request::Events {
            goal,
            after: Some(0),
            limit: 256,
        },
    );
    d.restart();
    let a = d.connect(credential(1), Some(session(1)));
    assert_eq!(d.ok(a, Request::Board { goal }), expected);
    assert_eq!(
        d.ok(
            a,
            Request::Events {
                goal,
                after: Some(0),
                limit: 256
            }
        ),
        feed
    );
    assert!(pending(&mut d, a, goal).claimed.is_empty());
}

#[test]
fn results_on_a_closed_picked_or_revised_task_are_in_nobodys_review_list() {
    let (mut d, p, owner, a, goal) = setup();
    let (task, offer) = offered(&mut d, a, goal, p);
    authorize(&mut d, owner, goal, task, p);
    let Response::Claimed(claim) = d.ok(
        a,
        Request::AttemptStart {
            goal,
            task: Some(task),
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    let first = event(d.ok(a, publish(goal, claim.attempt, 1)));
    let second = event(d.ok(a, publish(goal, claim.attempt, 1)));
    let listed = |d: &mut Daemon, conn, subject| {
        pending(d, conn, goal)
            .to_review
            .iter()
            .any(|item| item.subject == subject)
    };
    assert!(listed(&mut d, a, first) && listed(&mut d, a, second));
    // Closed: neither is asked about; reopened: both are again.
    let closed = event(d.ok(
        a,
        Request::ScopeClose {
            goal,
            scope: locust_proto::event::Scope::Task(task),
            expected: None,
        },
    ));
    assert!(!listed(&mut d, a, first) && !listed(&mut d, a, second));
    d.ok(
        a,
        Request::ScopeReopen {
            goal,
            scope: locust_proto::event::Scope::Task(task),
            expected: Some(closed),
        },
    );
    assert!(listed(&mut d, a, first) && listed(&mut d, a, second));
    // Picked: the result that lost leaves every list, a later reviewer's too.
    d.ok(
        a,
        Request::ReviewRecord {
            goal,
            subject: first,
            verdict: ReviewVerdict::Approve,
            text: "reviewed".into(),
        },
    );
    d.ok(
        a,
        Request::ScopeSelect {
            goal,
            subject: first,
            expected: None,
        },
    );
    assert!(!listed(&mut d, a, second));
    let (_, reviewer) = super::authorization::join_local(&mut d, a, goal, 2);
    assert!(!listed(&mut d, reviewer, second));
    // Revised: the old round's results feed nothing current.
    let (other, other_offer) = offered(&mut d, a, goal, p);
    authorize(&mut d, owner, goal, other, p);
    let Response::Claimed(claim) = d.ok(
        a,
        Request::AttemptStart {
            goal,
            task: Some(other),
            offer: Some(other_offer),
        },
    ) else {
        panic!()
    };
    let third = event(d.ok(a, publish(goal, claim.attempt, 1)));
    assert!(listed(&mut d, a, third));
    let expected_round = d.node.goals[&goal].state().tasks[&other].current_round;
    d.ok(
        owner,
        Request::TaskRevise {
            goal,
            task: other,
            expected_round,
            task_type: None,
        },
    );
    assert!(!listed(&mut d, a, third));
    assert!(!listed(&mut d, reviewer, third));
}

#[test]
fn findings_and_revisions_stay_in_review_lists_after_new_rules_are_bound() {
    let (mut d, _, owner, a, goal) = setup();
    let revision = event(d.ok(
        a,
        Request::DocRevise {
            goal,
            doc: Doc::Plan,
            base: None,
            text: "a plan".into(),
        },
    ));
    let posted = event(d.ok(a, finding(goal, "a finding")));
    let listed = |d: &mut Daemon, conn, subject| {
        pending(d, conn, goal)
            .to_review
            .iter()
            .any(|item| item.subject == subject)
    };
    assert!(listed(&mut d, a, revision) && listed(&mut d, a, posted));
    // The engine keeps asking for their review under the rules they were
    // posted under, so the list keeps them after a new binding too.
    let expected = d.node.goals[&goal].state().current_rules.unwrap();
    let formation = locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == "directed")
        .unwrap()
        .formation;
    d.ok(
        owner,
        Request::RulesBind {
            goal,
            expected,
            formation_json: serde_json::to_string(&formation).unwrap(),
            inputs: Default::default(),
            no_role: false,
        },
    );
    assert!(listed(&mut d, a, revision) && listed(&mut d, a, posted));
    let (member, reviewer) = super::authorization::join_local(&mut d, a, goal, 2);
    let holders = d.node.goals[&goal].state().roles["reviewer"].clone();
    d.ok(
        owner,
        Request::RoleGive {
            goal,
            role: "reviewer".into(),
            member,
            expected: holders,
        },
    );
    assert!(listed(&mut d, reviewer, revision) && listed(&mut d, reviewer, posted));
    assert_eq!(pending(&mut d, reviewer, goal).to_review.len(), 2);
}

#[test]
fn takeover_a_b_a_fences_old_generation_even_when_secret_returns() {
    let (mut d, p, owner, a, goal) = setup();
    let (task, offer) = offered(&mut d, a, goal, p);
    authorize(&mut d, owner, goal, task, p);
    let Response::Claimed(first) = d.ok(
        a,
        Request::AttemptStart {
            goal,
            task: Some(task),
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    let attempt = first.attempt;
    let b = d.connect(credential(1), Some(session(2)));
    let Response::Claimed(taken) = d.ok(b, Request::AttemptTakeover { goal, attempt }) else {
        panic!()
    };
    assert_eq!(taken.generation, 2);
    assert_eq!(
        code(d.call(a, progress(goal, attempt, 1))),
        ErrorCode::Superseded
    );
    let Response::Claimed(back) = d.ok(a, Request::AttemptTakeover { goal, attempt }) else {
        panic!()
    };
    assert_eq!(back.generation, 3);
    assert_eq!(
        code(d.call(b, publish(goal, attempt, 2))),
        ErrorCode::Superseded
    );
    d.restart();
    let a = d.connect(credential(1), Some(session(1)));
    assert_eq!(
        d.ok(
            a,
            Request::AttemptStart {
                goal,
                task: Some(task),
                offer: Some(offer)
            }
        ),
        Response::Claimed(back)
    );
    d.ok(a, progress(goal, attempt, 3));
}

#[test]
fn cancellation_requires_holder_generation_and_is_not_completion_evidence() {
    let (mut d, p, owner, a, goal) = setup();
    let (task, offer) = offered(&mut d, a, goal, p);
    authorize(&mut d, owner, goal, task, p);
    let Response::Claimed(claim) = d.ok(
        a,
        Request::AttemptStart {
            goal,
            task: Some(task),
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    let subject = event(d.ok(a, publish(goal, claim.attempt, 1)));
    let cancel = event(d.ok(
        a,
        Request::AttemptCancel {
            goal,
            attempt: claim.attempt,
        },
    ));
    assert_eq!(pending(&mut d, a, goal).to_acknowledge[0].cancel, cancel);
    let b = d.connect(credential(1), Some(session(2)));
    assert_eq!(
        code(d.call(
            b,
            Request::CancelAcknowledge {
                goal,
                cancel,
                generation: Some(1),
                outcome: CancelOutcome::Stopped
            }
        )),
        ErrorCode::Superseded
    );
    d.ok(
        a,
        Request::CancelAcknowledge {
            goal,
            cancel,
            generation: Some(1),
            outcome: CancelOutcome::Completed,
        },
    );
    assert!(pending(&mut d, a, goal).to_acknowledge.is_empty());
    assert!(pending(&mut d, a, goal).claimed.is_empty());
    assert_eq!(
        d.node.goals[&goal].state().attempts[&claim.attempt].status,
        Some(AttemptStatus::Completed)
    );
    assert_eq!(
        code(d.call(
            a,
            Request::ScopeSelect {
                goal,
                subject,
                expected: None
            }
        )),
        ErrorCode::Conflict
    );
    let Response::Task(detail) = d.ok(a, Request::Task { goal, task }) else {
        panic!()
    };
    assert!(!detail.view.completed);
}

#[test]
fn sessions_survive_restart_drop_requires_finished_claim_and_binding_is_permanent() {
    let (mut d, p, owner, a, goal) = setup();
    let record = SessionRecord {
        harness: locust_proto::farm::Harness::Unknown,
        client: "test".into(),
        state: SessionState::Ready,
        client_session: Some("session".into()),
        capabilities: SessionCapabilities::default(),
        detail: vec![],
    };
    d.ok(
        a,
        Request::SessionReport {
            record: record.clone(),
        },
    );
    let (task, offer) = offered(&mut d, a, goal, p);
    authorize(&mut d, owner, goal, task, p);
    let Response::Claimed(claim) = d.ok(
        a,
        Request::AttemptStart {
            goal,
            task: Some(task),
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    assert_eq!(
        code(d.call(
            a,
            Request::SessionDrop {
                instance: session(1).instance()
            }
        )),
        ErrorCode::Conflict
    );
    d.restart();
    let a = d.connect(credential(1), Some(session(1)));
    let Response::Session(view) = d.ok(a, Request::Session { instance: None }) else {
        panic!()
    };
    assert_eq!(view.record, record);
    assert_eq!(view.claims, vec![claim]);
    d.ok(
        a,
        Request::AttemptReport {
            goal,
            attempt: claim.attempt,
            generation: 1,
            status: AttemptStatus::Failed,
            text: "failed".into(),
        },
    );
    d.ok(
        a,
        Request::SessionDrop {
            instance: session(1).instance(),
        },
    );
    d.enroll("other", 2);
    let other = d.connect(credential(2), Some(session(1)));
    assert_eq!(
        code(d.call(other, Request::SessionReport { record })),
        ErrorCode::Denied
    );
}

#[test]
fn document_review_and_idempotent_finding_survive_replay() {
    let (mut d, _, _, a, goal) = setup();
    let request = finding(goal, "A finding");
    let response = d.keyed(a, 44, request.clone()).unwrap();
    assert_eq!(d.keyed(a, 44, request.clone()).unwrap(), response);
    let revision = event(d.ok(
        a,
        Request::DocRevise {
            goal,
            doc: Doc::Plan,
            base: None,
            text: "The plan".into(),
        },
    ));
    d.ok(
        a,
        Request::ReviewRecord {
            goal,
            subject: revision,
            verdict: ReviewVerdict::Approve,
            text: "reviewed".into(),
        },
    );
    d.ok(
        a,
        Request::ScopeSelect {
            goal,
            subject: revision,
            expected: None,
        },
    );
    let document = d.ok(
        a,
        Request::DocRead {
            goal,
            doc: Doc::Plan,
        },
    );
    let count = d.store.log(&goal, 0, 1000).unwrap().len();
    d.restart();
    let a = d.connect(credential(1), None);
    assert_eq!(d.keyed(a, 44, request).unwrap(), response);
    assert_eq!(d.store.log(&goal, 0, 1000).unwrap().len(), count);
    assert_eq!(
        d.ok(
            a,
            Request::DocRead {
                goal,
                doc: Doc::Plan
            }
        ),
        document
    );
}

#[test]
fn removal_seals_new_epoch_proof_and_stops_member_writes() {
    let (mut d, p, owner, a, goal) = setup();
    assert_eq!(
        code(d.call(owner, Request::MemberRemove { goal, member: p })),
        ErrorCode::Conflict
    );
    assert_eq!(
        code(d.call(owner, Request::GoalLeave { goal, agent: p })),
        ErrorCode::Conflict
    );
    let (member, conn) = super::authorization::join_local(&mut d, a, goal, 2);
    let removal = event(d.ok(owner, Request::MemberRemove { goal, member }));
    let event = d.store.event(&removal).unwrap().unwrap();
    let payload = event.header().payload.unwrap();
    assert_eq!(payload.key_epoch, 1);
    assert_eq!(
        locust_proto::seal::open(
            &goal,
            &d.node.goals[&goal].keys[&1],
            &d.store.blob(&payload.hash).unwrap().unwrap()
        )
        .unwrap(),
        b""
    );
    assert_eq!(
        code(d.call(conn, finding(goal, "removed"))),
        ErrorCode::Denied
    );
}

#[test]
fn waits_observe_committed_revisions_timeout_and_recheck_revocation() {
    use locust_proto::api::WaitOutcome;
    let (mut d, p, owner, a, goal) = setup();
    let seen = pending(&mut d, a, goal).revision;
    let frame = d.frame(Request::Wait {
        goal,
        seen,
        timeout_ms: 1000,
    });
    let Step::Park(parked) = d.step(a, frame, 100) else {
        panic!()
    };
    assert!(matches!(
        d.node.resume(a, &parked, false, 101),
        Step::Park(_)
    ));
    assert!(matches!(
        d.node.resume(a, &parked, true, 1100),
        Step::Reply(locust_proto::api::ResponseFrame {
            result: Ok(Response::Waited(WaitOutcome::NoEvent)),
            ..
        })
    ));
    let frame = d.frame(Request::Wait {
        goal,
        seen,
        timeout_ms: 1000,
    });
    let Step::Park(parked) = d.step(a, frame, 1200) else {
        panic!()
    };
    d.ok(a, finding(goal, "changed"));
    let Step::Reply(reply) = d.node.resume(a, &parked, false, 1201) else {
        panic!()
    };
    let Response::Waited(WaitOutcome::Work(work)) = reply.result.unwrap() else {
        panic!()
    };
    assert!(work.revision > seen);
    let frame = d.frame(Request::Wait {
        goal,
        seen: work.revision,
        timeout_ms: 1000,
    });
    let Step::Park(parked) = d.step(a, frame, 1300) else {
        panic!()
    };
    d.ok(owner, Request::AgentRevoke { agent: p });
    let Step::Reply(reply) = d.node.resume(a, &parked, false, 1301) else {
        panic!()
    };
    assert_eq!(code(reply.result), ErrorCode::Denied);
}

#[test]
fn declining_one_offer_does_not_impose_an_attempt_budget() {
    let (mut d, p, owner, a, goal) = setup();
    let (task, first) = offered(&mut d, a, goal, p);
    authorize(&mut d, owner, goal, task, p);
    d.ok(a, Request::WorkDecline { goal, offer: first });
    assert_eq!(
        code(d.call(
            a,
            Request::AttemptStart {
                goal,
                task: Some(task),
                offer: Some(first)
            }
        )),
        ErrorCode::Conflict
    );
    let second = event(d.ok(
        a,
        Request::WorkOffer {
            goal,
            task,
            recipient: p,
        },
    ));
    let Response::Claimed(claim) = d.ok(
        a,
        Request::AttemptStart {
            goal,
            task: Some(task),
            offer: Some(second),
        },
    ) else {
        panic!()
    };
    d.ok(
        a,
        Request::AttemptReport {
            goal,
            attempt: claim.attempt,
            generation: 1,
            status: AttemptStatus::Failed,
            text: "failed".into(),
        },
    );
    let third = event(d.ok(
        a,
        Request::WorkOffer {
            goal,
            task,
            recipient: p,
        },
    ));
    assert!(matches!(
        d.ok(
            a,
            Request::AttemptStart {
                goal,
                task: Some(task),
                offer: Some(third)
            }
        ),
        Response::Claimed(_)
    ));
}

#[test]
fn completion_requires_an_attempt_result_but_not_review_or_integration() {
    let (mut d, p, owner, a, goal) = setup();
    let (task, offer) = offered(&mut d, a, goal, p);
    authorize(&mut d, owner, goal, task, p);
    let Response::Claimed(claim) = d.ok(
        a,
        Request::AttemptStart {
            goal,
            task: Some(task),
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    let complete = Request::AttemptReport {
        goal,
        attempt: claim.attempt,
        generation: 1,
        status: AttemptStatus::Completed,
        text: "done".into(),
    };
    let count = d.store.log(&goal, 0, 1000).unwrap().len();
    assert_eq!(code(d.call(a, complete.clone())), ErrorCode::Conflict);
    assert_eq!(d.store.log(&goal, 0, 1000).unwrap().len(), count);
    // A goal-scope finding is not a result associated with this attempt.
    let mut note = publish(goal, claim.attempt, 1);
    if let Request::ContributionPublish {
        attempt,
        generation,
        ..
    } = &mut note
    {
        *attempt = None;
        *generation = None;
    }
    d.ok(a, note);
    assert_eq!(code(d.call(a, complete.clone())), ErrorCode::Conflict);
    let result = event(d.ok(a, publish(goal, claim.attempt, 1)));
    d.ok(a, complete);
    assert!(pending(&mut d, a, goal).claimed.is_empty());
    let Response::Task(detail) = d.ok(a, Request::Task { goal, task }) else {
        panic!()
    };
    assert!(!detail.view.completed);
    assert_eq!(detail.view.selected, None);
    assert!(
        pending(&mut d, a, goal)
            .to_review
            .iter()
            .any(|item| item.subject == result)
    );
}

#[test]
fn stopped_cancellation_commits_a_terminal_report_and_retries_without_new_events() {
    let (mut d, p, owner, a, goal) = setup();
    let (task, offer) = offered(&mut d, a, goal, p);
    authorize(&mut d, owner, goal, task, p);
    let Response::Claimed(claim) = d.ok(
        a,
        Request::AttemptStart {
            goal,
            task: Some(task),
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    let cancel = event(d.ok(
        a,
        Request::AttemptCancel {
            goal,
            attempt: claim.attempt,
        },
    ));
    let ack = Request::CancelAcknowledge {
        goal,
        cancel,
        generation: Some(1),
        outcome: CancelOutcome::Stopped,
    };
    let before = d.store.log(&goal, 0, 1000).unwrap().len();
    let answer = d.ok(a, ack.clone());
    let events = d.store.log(&goal, 0, 1000).unwrap();
    assert_eq!(events.len(), before + 2);
    assert_eq!(
        events[before + 1].1.header().prev,
        Some(events[before].1.id())
    );
    assert!(pending(&mut d, a, goal).claimed.is_empty());
    assert!(pending(&mut d, a, goal).to_acknowledge.is_empty());
    assert_eq!(d.ok(a, ack.clone()), answer);
    assert_eq!(d.store.log(&goal, 0, 1000).unwrap().len(), before + 2);
    // Reconstruct a replica in reverse delivery order from the signed records.
    let mut replica = crate::goal::Goal::new(goal);
    for (_, event) in events.iter().rev() {
        replica.apply(
            std::slice::from_ref(event),
            &d.node.goals[&goal].definitions,
        );
    }
    assert_eq!(
        replica.state().attempts[&claim.attempt].status,
        Some(AttemptStatus::Abandoned)
    );
    d.restart();
    let a = d.connect(credential(1), Some(session(1)));
    assert_eq!(d.ok(a, ack), answer);
    assert!(pending(&mut d, a, goal).claimed.is_empty());
    assert_eq!(
        code(d.call(a, progress(goal, claim.attempt, 1))),
        ErrorCode::Conflict
    );
    d.ok(
        a,
        Request::SessionDrop {
            instance: session(1).instance(),
        },
    );
}

#[test]
fn uncertain_cancellation_fences_work_but_allows_ending_and_completed_needs_result() {
    let (mut d, p, owner, a, goal) = setup();
    let (task, offer) = offered(&mut d, a, goal, p);
    authorize(&mut d, owner, goal, task, p);
    let Response::Claimed(claim) = d.ok(
        a,
        Request::AttemptStart {
            goal,
            task: Some(task),
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    let cancel = event(d.ok(
        a,
        Request::AttemptCancel {
            goal,
            attempt: claim.attempt,
        },
    ));
    let completed = Request::CancelAcknowledge {
        goal,
        cancel,
        generation: Some(1),
        outcome: CancelOutcome::Completed,
    };
    assert_eq!(code(d.call(a, completed)), ErrorCode::Conflict);
    let end = Request::AttemptReport {
        goal,
        attempt: claim.attempt,
        generation: 1,
        status: AttemptStatus::Abandoned,
        text: "stopped locally".into(),
    };
    assert_eq!(code(d.call(a, end.clone())), ErrorCode::Conflict);
    d.ok(
        a,
        Request::CancelAcknowledge {
            goal,
            cancel,
            generation: Some(1),
            outcome: CancelOutcome::Uncertain,
        },
    );
    assert_eq!(pending(&mut d, a, goal).claimed, vec![claim]);
    assert_eq!(
        code(d.call(a, progress(goal, claim.attempt, 1))),
        ErrorCode::Conflict
    );
    assert_eq!(
        code(d.call(a, publish(goal, claim.attempt, 1))),
        ErrorCode::Conflict
    );
    d.ok(a, end);
    assert!(pending(&mut d, a, goal).claimed.is_empty());
}

#[test]
fn completed_round_is_not_startable_and_revision_restores_eligibility() {
    let (mut d, p, owner, a, goal) = setup();
    let (task, offer) = offered(&mut d, a, goal, p);
    authorize(&mut d, owner, goal, task, p);
    let Response::Claimed(claim) = d.ok(
        a,
        Request::AttemptStart {
            goal,
            task: Some(task),
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    let result = event(d.ok(a, publish(goal, claim.attempt, claim.generation)));
    d.ok(
        a,
        Request::AttemptReport {
            goal,
            attempt: claim.attempt,
            generation: claim.generation,
            status: AttemptStatus::Completed,
            text: "Result posted".into(),
        },
    );
    // This still-unanswered offer was made before the result finished the round.
    let offer = event(d.ok(
        a,
        Request::WorkOffer {
            goal,
            task,
            recipient: p,
        },
    ));
    d.ok(
        a,
        Request::ReviewRecord {
            goal,
            subject: result,
            verdict: ReviewVerdict::Approve,
            text: "checked".into(),
        },
    );
    assert!(
        !pending(&mut d, a, goal)
            .to_start
            .iter()
            .any(|item| item.task == task)
    );
    assert!(
        !pending(&mut d, owner, goal)
            .ask_first
            .iter()
            .any(|item| item.task == task)
    );
    assert_eq!(
        code(d.call(
            a,
            Request::AttemptStart {
                goal,
                task: Some(task),
                offer: Some(offer)
            }
        )),
        ErrorCode::Conflict
    );
    // Revising the task creates a fresh round with a new allowance.
    let round = d.node.goals[&goal].state().tasks[&task].current_round;
    d.ok(
        owner,
        Request::TaskRevise {
            goal,
            task,
            expected_round: round,
            task_type: None,
        },
    );
    let offer = event(d.ok(
        a,
        Request::WorkOffer {
            goal,
            task,
            recipient: p,
        },
    ));
    authorize(&mut d, owner, goal, task, p);
    assert!(
        pending(&mut d, a, goal)
            .to_start
            .iter()
            .any(|item| item.task == task && item.offer == Some(offer))
    );
    d.ok(
        a,
        Request::AttemptStart {
            goal,
            task: Some(task),
            offer: Some(offer),
        },
    );
}

#[test]
fn an_ended_attempt_lists_no_cancellation_request() {
    let (mut d, p, owner, a, goal) = setup();
    let (task, offer) = offered(&mut d, a, goal, p);
    authorize(&mut d, owner, goal, task, p);
    let Response::Claimed(claim) = d.ok(
        a,
        Request::AttemptStart {
            goal,
            task: Some(task),
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    let attempt = claim.attempt;
    let first = event(d.ok(a, Request::AttemptCancel { goal, attempt }));
    event(d.ok(a, Request::AttemptCancel { goal, attempt }));
    let work = pending(&mut d, a, goal);
    assert_eq!(work.claimed, vec![claim]);
    assert_eq!(work.to_acknowledge.len(), 2);
    assert!(work.to_acknowledge.iter().all(|c| c.generation == Some(1)));
    d.ok(
        a,
        Request::CancelAcknowledge {
            goal,
            cancel: first,
            generation: Some(1),
            outcome: CancelOutcome::Stopped,
        },
    );
    // The other request is moot. Listed with a generation and no claim, it
    // would fail the managed launcher's snapshot check until answered.
    let work = pending(&mut d, a, goal);
    assert!(work.claimed.is_empty());
    assert!(work.to_acknowledge.is_empty());

    // The same holds for a request made after the worker reported its end.
    let (task, offer) = offered(&mut d, a, goal, p);
    authorize(&mut d, owner, goal, task, p);
    let Response::Claimed(claim) = d.ok(
        a,
        Request::AttemptStart {
            goal,
            task: Some(task),
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    let attempt = claim.attempt;
    d.ok(
        a,
        Request::AttemptReport {
            goal,
            attempt,
            generation: 1,
            status: AttemptStatus::Failed,
            text: "failed".into(),
        },
    );
    d.ok(a, Request::AttemptCancel { goal, attempt });
    assert!(pending(&mut d, a, goal).to_acknowledge.is_empty());
}

#[test]
fn an_invalid_formation_is_refused_without_naming_an_api_operation() {
    let (mut d, principal, owner, _, _) = setup();
    let refused = d
        .call(
            owner,
            Request::GoalCreate {
                name: "host".into(),
                agent: principal,
                title: "Another goal".into(),
                formation_json: Some("not a formation".into()),

                inputs: Default::default(),
            },
        )
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::Invalid);
    names_no_operation(&refused);
}

/// A task close or reopen carries no subject, and the engine's `decision`
/// check requires no evidence for `Close` or `Reopen`: only the finish
/// authority, the same-stream `previous` and the proof (whose roots are
/// `context.round` and `previous`) matter. The former node code embedded
/// every approved contribution (and each one's own evidence) in one header,
/// so with enough approved work a single close exceeded the signed header
/// cap and could not be recorded. The sufficient witness is empty.
#[test]
fn closing_a_task_with_many_approved_contributions_stays_within_the_header_cap() {
    use locust_proto::event::{Body, Scope};
    use locust_proto::limits::MAX_HEADER_BYTES;
    use locust_proto::organization::{
        Authority, CompletionRule, DecisionRules, Formation, Selector,
    };

    let (mut d, principal, owner, agent, _) = setup();
    // A formation under which a contribution counts as complete the moment it
    // is published (its author is a member), and whose finish decider is the
    // principal, so the principal may close and reopen the task.
    let formation = Formation {
        decisions: DecisionRules {
            completion: CompletionRule::Contribution {
                by: Selector::Members,
            },
            selection: None,
            finish: Some(Authority::Participant {
                key: principal.to_string(),
            }),
        },
        ..Default::default()
    };
    let Response::GoalCreated { goal } = d.ok(
        owner,
        Request::GoalCreate {
            name: "host".into(),
            agent: principal,
            title: "Many results".into(),
            formation_json: Some(serde_json::to_string(&formation).unwrap()),

            inputs: Default::default(),
        },
    ) else {
        panic!()
    };
    d.ok(
        owner,
        Request::LevelSet {
            goal,
            agent: principal,
            level: locust_proto::api::Level::Ask,
        },
    );
    let task = TaskId::Authored(event(d.ok(
        agent,
        Request::TaskOpen {
            goal,
            text: "Produce many results".into(),
            task_type: None,
            inputs: Default::default(),
            parent: None,
        },
    )));
    // More approved contributions than fit in a 16 KiB header as evidence:
    // each approved contribution added its own id (32 bytes) under the old
    // code, so 600 of them alone occupy ~18.8 KiB before any header overhead.
    const APPROVED: usize = 600;
    authorize(&mut d, owner, goal, task, principal);
    let Response::Claimed(claim) = d.ok(
        agent,
        Request::AttemptStart {
            goal,
            task: Some(task),
            offer: None,
        },
    ) else {
        panic!()
    };
    for index in 0..APPROVED {
        let contribution = event(d.ok(
            agent,
            Request::ContributionPublish {
                goal,
                attempt: Some(claim.attempt),
                generation: Some(claim.generation),
                summary: format!("result {index}"),
                sources: Vec::new(),
                artifacts: vec![],
            },
        ));
        let Response::Contributions(contributions) = d.ok(
            agent,
            Request::Contributions {
                goal,
                task: Some(task),
            },
        ) else {
            panic!()
        };
        let found = contributions
            .iter()
            .find(|view| view.contribution == contribution)
            .expect("the contribution is recorded");
        assert!(found.approved, "result {index} should count as complete");
    }
    let before = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    let closed = event(d.ok(
        agent,
        Request::ScopeClose {
            goal,
            scope: Scope::Task(task),
            expected: None,
        },
    ));
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), before + 1);
    let close_event = d.store.event(&closed).unwrap().unwrap();
    let Body::ScopeDecided { evidence, .. } = &close_event.header().body else {
        panic!("close event: {:?}", close_event.header().body)
    };
    assert!(
        evidence.is_empty(),
        "a close carries no subject and needs no evidence"
    );
    assert!(close_event.header_bytes().len() <= MAX_HEADER_BYTES);
    assert!(
        d.node.goals[&goal].state().tasks[&task]
            .rounds
            .values()
            .next()
            .unwrap()
            .closed
    );
    let reopened = event(d.ok(
        agent,
        Request::ScopeReopen {
            goal,
            scope: Scope::Task(task),
            expected: Some(closed),
        },
    ));
    let reopen_event = d.store.event(&reopened).unwrap().unwrap();
    let Body::ScopeDecided { evidence, .. } = &reopen_event.header().body else {
        panic!("reopen event: {:?}", reopen_event.header().body)
    };
    assert!(evidence.is_empty());
    assert!(reopen_event.header_bytes().len() <= MAX_HEADER_BYTES);
    assert!(
        !d.node.goals[&goal].state().tasks[&task]
            .rounds
            .values()
            .next()
            .unwrap()
            .closed
    );
    // The approved contributions remain readable and still count; the close
    // did not drop or reinterpret any work.
    let Response::Contributions(contributions) = d.ok(
        agent,
        Request::Contributions {
            goal,
            task: Some(task),
        },
    ) else {
        panic!()
    };
    assert_eq!(contributions.len(), APPROVED);
    assert!(contributions.iter().all(|view| view.approved));
}

#[test]
fn the_governance_key_is_stored_with_the_goal_and_signs_after_a_restart() {
    use super::authorization::{author_of, governance_key, join_local};
    let (mut d, host, owner, agent, goal) = setup();
    let governance = governance_key(&d, goal);
    assert_eq!(
        d.node.goals[&goal].state().governance,
        Some(governance.public())
    );
    assert_ne!(governance.public(), host);
    let records = d.store.log(&goal, 0, usize::MAX).unwrap().len();
    let Response::GoalStatus(status) = d.ok(owner, Request::GoalStatus { goal }) else {
        panic!()
    };
    assert!(status.hosted_here);
    d.restart();
    let reloaded = governance_key(&d, goal);
    assert_eq!(reloaded.seed(), governance.seed());
    assert_eq!(reloaded.public(), governance.public());
    assert_eq!(d.store.log(&goal, 0, usize::MAX).unwrap().len(), records);
    let owner = d.owner();
    let Response::GoalStatus(status) = d.ok(owner, Request::GoalStatus { goal }) else {
        panic!()
    };
    assert!(status.hosted_here);
    assert_eq!(status.governance, governance.public());
    assert_eq!(status.host, Some(host));
    // The restarted daemon signs admissions and rules with the same key.
    let (member, _) = join_local(&mut d, agent, goal, 2);
    let admission = d.node.goals[&goal].state().members[&member].admission;
    assert_eq!(author_of(&d, &admission), governance.public());
    let expected = d.node.goals[&goal].state().current_rules.unwrap();
    let rules = event(d.ok(
        owner,
        Request::RulesBind {
            no_role: false,
            goal,
            expected,
            formation_json: "{\"schema_version\":2}".into(),

            inputs: Default::default(),
        },
    ));
    let record = d.store.event(&rules).unwrap().unwrap();
    assert_eq!(record.header().author, governance.public());
    assert!(locust_proto::event::Event::from_wire(&record.to_wire()).is_ok());
    assert_eq!(d.node.goals[&goal].state().current_rules, Some(rules));
}
