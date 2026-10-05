//! Public Engine transcripts: independent evidence, local claims and replay.
use super::*;
use locust_proto::api::{
    GoalGrants, PendingWork, SessionCapabilities, SessionRecord, SessionState,
};
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
    let principal = daemon.enroll("administrator", 1, true);
    let owner = daemon.owner();
    let agent = daemon.connect(credential(1), Some(session(1)));
    let mut formation = locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == "coordinator")
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
        agent,
        Request::GoalCreate {
            title: "A test goal".into(),
            formation_json: Some(serde_json::to_string(&formation).unwrap()),
            roles: std::collections::BTreeMap::from([("coordinator".into(), vec![principal])]),
            inputs: Default::default(),
        },
    ) else {
        panic!()
    };
    daemon.ok(
        owner,
        Request::GoalGrant {
            goal,
            agent: principal,
            grants: GoalGrants {
                administer: true,
                contribute: true,
                review: true,
                select: true,
                flow: true,
                ..Default::default()
            },
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
        task: None,
        attempt: None,
        generation: None,
        summary: text.into(),
        sources: Vec::new(),
        artifacts: vec![],
    }
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
    daemon.ok(
        owner,
        Request::TaskAuthorize {
            goal,
            task,
            agent,
            takeover: true,
        },
    );
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
fn publish(goal: GoalId, task: TaskId, attempt: EventId, generation: u32) -> Request {
    Request::ContributionPublish {
        goal,
        task: Some(task),
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
    assert_eq!(
        pending(&mut d, owner, goal).to_authorize[0].offer,
        Some(offer)
    );
    assert_eq!(
        code(d.call(
            a,
            Request::AttemptStart {
                goal,
                task,
                offer: Some(offer)
            }
        )),
        ErrorCode::AuthorizationRequired
    );
    authorize(&mut d, owner, goal, task, p);
    let Response::Claimed(claim) = d.ok(
        a,
        Request::AttemptStart {
            goal,
            task,
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    assert_eq!(pending(&mut d, a, goal).claimed, vec![claim]);
    d.ok(a, progress(goal, claim.attempt, 1));
    let contribution = event(d.ok(a, publish(goal, task, claim.attempt, 1)));
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
fn takeover_a_b_a_fences_old_generation_even_when_secret_returns() {
    let (mut d, p, owner, a, goal) = setup();
    let (task, offer) = offered(&mut d, a, goal, p);
    authorize(&mut d, owner, goal, task, p);
    let Response::Claimed(first) = d.ok(
        a,
        Request::AttemptStart {
            goal,
            task,
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
        code(d.call(b, publish(goal, task, attempt, 2))),
        ErrorCode::Superseded
    );
    d.restart();
    let a = d.connect(credential(1), Some(session(1)));
    assert_eq!(
        d.ok(
            a,
            Request::AttemptStart {
                goal,
                task,
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
            task,
            offer: Some(offer),
        },
    ) else {
        panic!()
    };
    let subject = event(d.ok(a, publish(goal, task, claim.attempt, 1)));
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
fn viewer_reads_are_observational_and_revocation_is_immediate() {
    let (mut d, p, owner, a, goal) = setup();
    let (task, _) = offered(&mut d, a, goal, p);
    d.ok(
        owner,
        Request::ViewerEnroll {
            agent: p,
            credential: credential(9).digest(),
        },
    );
    let viewer = d.connect(credential(9), None);
    assert!(matches!(
        d.ok(viewer, Request::Task { goal, task }),
        Response::Task(_)
    ));
    assert_eq!(code(d.call(viewer, finding(goal, "no"))), ErrorCode::Denied);
    let before = d.ok(
        a,
        Request::Events {
            goal,
            after: None,
            limit: 256,
        },
    );
    d.ok(
        viewer,
        Request::Events {
            goal,
            after: Some(500),
            limit: 256,
        },
    );
    assert_eq!(
        d.ok(
            a,
            Request::Events {
                goal,
                after: None,
                limit: 256
            }
        ),
        before
    );
    assert!(matches!(
        d.hello(credential(9), Some(session(9))).1,
        ServerHello::Refused {
            error: ApiError {
                code: ErrorCode::Invalid,
                ..
            },
            ..
        }
    ));
    d.ok(owner, Request::AgentRevoke { agent: p });
    assert_eq!(
        code(d.call(viewer, Request::Board { goal })),
        ErrorCode::Denied
    );
    assert_eq!(code(d.call(a, Request::Board { goal })), ErrorCode::Denied);
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
            task,
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
    d.enroll("other", 2, false);
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
    let (mut d, p, _, a, goal) = setup();
    assert_eq!(
        code(d.call(a, Request::MemberRemove { goal, member: p })),
        ErrorCode::Conflict
    );
    assert_eq!(
        code(d.call(a, Request::GoalLeave { goal })),
        ErrorCode::Conflict
    );
    let (member, conn) = super::authorization::join_local(&mut d, a, goal, 2);
    let removal = event(d.ok(a, Request::MemberRemove { goal, member }));
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
                task,
                offer: Some(first)
            }
        )),
        ErrorCode::Denied
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
            task,
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
                task,
                offer: Some(third)
            }
        ),
        Response::Claimed(_)
    ));
}
