//! Public Engine transcripts: claims, reading, fencing, cancellation and replay.
use super::*;
use locust_proto::api::{PendingWork, SessionCapabilities, SessionRecord, SessionState, TaskState};
use locust_proto::engine::{PeerEngine, PeerInput};
use locust_proto::event::{CancelOutcome, Doc};
use locust_proto::id::{EndpointId, EventId, GoalId};
use locust_proto::store::Store;

pub(super) fn setup() -> (Daemon, PublicKey, ConnId, ConnId, GoalId) {
    let mut daemon = Daemon::new(21);
    daemon.node.peer(
        PeerInput::Endpoint {
            endpoint: EndpointId([21; 32]),
            hints: vec![],
        },
        0,
        &mut Vec::new(),
    );
    let principal = daemon.enroll("coordinator", 1, true);
    let owner = daemon.owner();
    let agent = daemon.connect(credential(1), Some(session(1)));
    let Response::GoalCreated { goal } = daemon.ok(
        agent,
        Request::GoalCreate {
            title: "A test goal".into(),
        },
    ) else {
        panic!()
    };
    (daemon, principal, owner, agent, goal)
}
pub(super) fn event(response: Response) -> EventId {
    let Response::Recorded { event } = response else {
        panic!("{response:?}")
    };
    event
}
fn pending(daemon: &mut Daemon, conn: ConnId, goal: GoalId) -> PendingWork {
    let Response::Pending(work) = daemon.ok(conn, Request::Pending { goal }) else {
        panic!()
    };
    work
}
fn assigned(
    daemon: &mut Daemon,
    agent: ConnId,
    goal: GoalId,
    principal: PublicKey,
) -> (EventId, EventId) {
    let task = event(daemon.ok(
        agent,
        Request::TaskPropose {
            goal,
            text: "Read and implement\nAcceptance details".into(),
            input: None,
            depends_on: vec![],
            deadline_ms: None,
            max_attempts: Some(2),
        },
    ));
    let assignment = event(daemon.ok(
        agent,
        Request::TaskAssign {
            goal,
            task,
            assignee: principal,
        },
    ));
    (task, assignment)
}
fn progress(goal: GoalId, assignment: EventId, generation: u32) -> Request {
    Request::TaskProgress {
        goal,
        assignment,
        generation,
        text: "still working".into(),
    }
}
fn submit(goal: GoalId, assignment: EventId, generation: u32) -> Request {
    Request::TaskSubmit {
        goal,
        assignment,
        generation,
        summary: "Completed with evidence".into(),
        base: None,
        patch: None,
        artifacts: vec![],
    }
}
fn authorize(daemon: &mut Daemon, owner: ConnId, goal: GoalId, assignment: EventId) {
    daemon.ok(
        owner,
        Request::TaskAuthorize {
            goal,
            assignment,
            takeover: true,
        },
    );
}

#[test]
fn complete_transcript_keeps_submitted_review_and_replays_after_restart() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    let (task, assignment) = assigned(&mut daemon, agent, goal, principal);
    assert_eq!(
        pending(&mut daemon, owner, goal).to_authorize[0].assignment,
        assignment
    );
    assert_eq!(
        code(daemon.call(agent, Request::TaskClaim { goal, assignment })),
        ErrorCode::AuthorizationRequired
    );
    authorize(&mut daemon, owner, goal, assignment);
    assert_eq!(
        pending(&mut daemon, agent, goal).to_claim[0].assignment,
        assignment
    );
    let Response::Claimed(claim) = daemon.ok(agent, Request::TaskClaim { goal, assignment }) else {
        panic!()
    };
    assert_eq!(claim.generation, 1);
    assert_eq!(pending(&mut daemon, agent, goal).claimed, vec![claim]);
    daemon.ok(agent, progress(goal, assignment, 1));
    let result = event(daemon.ok(agent, submit(goal, assignment, 1)));
    assert_eq!(
        code(daemon.call(agent, progress(goal, assignment, 1))),
        ErrorCode::Conflict
    );
    assert_eq!(
        pending(&mut daemon, agent, goal).to_review[0].result,
        result
    );
    let Response::Event(detail) = daemon.ok(
        agent,
        Request::Event {
            goal,
            event: result,
        },
    ) else {
        panic!()
    };
    assert_eq!(detail.text.as_deref(), Some("Completed with evidence"));
    assert_eq!(detail.task, Some(task));
    let Response::Task(detail) = daemon.ok(agent, Request::Task { goal, task }) else {
        panic!()
    };
    assert_eq!(detail.view.state, TaskState::Submitted);
    daemon.ok(
        agent,
        Request::ResultAccept {
            goal,
            result,
            head: None,
        },
    );
    assert!(pending(&mut daemon, agent, goal).to_review.is_empty());
    let expected = daemon.ok(agent, Request::Board { goal });
    let feed = daemon.ok(
        agent,
        Request::Events {
            goal,
            after: Some(0),
            limit: 256,
        },
    );
    daemon.restart();
    let agent = daemon.connect(credential(1), Some(session(1)));
    assert_eq!(daemon.ok(agent, Request::Board { goal }), expected);
    assert_eq!(
        daemon.ok(
            agent,
            Request::Events {
                goal,
                after: Some(0),
                limit: 256
            }
        ),
        feed
    );
    assert!(pending(&mut daemon, agent, goal).claimed.is_empty());
}

#[test]
fn takeover_a_b_a_fences_old_generation_even_when_secret_returns() {
    let (mut daemon, principal, owner, a, goal) = setup();
    let (_, assignment) = assigned(&mut daemon, a, goal, principal);
    authorize(&mut daemon, owner, goal, assignment);
    daemon.ok(a, Request::TaskClaim { goal, assignment });
    let b = daemon.connect(credential(1), Some(session(2)));
    assert_eq!(
        code(daemon.call(b, Request::TaskClaim { goal, assignment })),
        ErrorCode::ClaimHeld
    );
    let Response::Claimed(claim) = daemon.ok(b, Request::TaskTakeover { goal, assignment }) else {
        panic!()
    };
    assert_eq!(claim.generation, 2);
    assert_eq!(
        code(daemon.call(a, progress(goal, assignment, 1))),
        ErrorCode::Superseded
    );
    let Response::Claimed(claim) = daemon.ok(a, Request::TaskTakeover { goal, assignment }) else {
        panic!()
    };
    assert_eq!(claim.generation, 3);
    assert_eq!(
        code(daemon.call(a, progress(goal, assignment, 1))),
        ErrorCode::Superseded
    );
    assert_eq!(
        code(daemon.call(b, submit(goal, assignment, 2))),
        ErrorCode::Superseded
    );
    daemon.restart();
    let a = daemon.connect(credential(1), Some(session(1)));
    let Response::Claimed(claim) = daemon.ok(a, Request::TaskClaim { goal, assignment }) else {
        panic!()
    };
    assert_eq!(claim.generation, 3);
    daemon.ok(a, progress(goal, assignment, 3));
}

#[test]
fn cancellation_requires_holder_generation_and_cannot_finalize() {
    let (mut daemon, principal, owner, a, goal) = setup();
    let (_, assignment) = assigned(&mut daemon, a, goal, principal);
    authorize(&mut daemon, owner, goal, assignment);
    daemon.ok(a, Request::TaskClaim { goal, assignment });
    let result = event(daemon.ok(a, submit(goal, assignment, 1)));
    let cancel = event(daemon.ok(a, Request::TaskCancel { goal, assignment }));
    assert_eq!(
        code(daemon.call(
            a,
            Request::ResultAccept {
                goal,
                result,
                head: None
            }
        )),
        ErrorCode::Conflict
    );
    assert_eq!(
        pending(&mut daemon, a, goal).to_acknowledge[0].cancel,
        cancel
    );
    let b = daemon.connect(credential(1), Some(session(2)));
    assert_eq!(
        code(daemon.call(
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
    daemon.ok(
        a,
        Request::CancelAcknowledge {
            goal,
            cancel,
            generation: Some(1),
            outcome: CancelOutcome::Completed,
        },
    );
    assert!(pending(&mut daemon, a, goal).to_acknowledge.is_empty());
    assert_eq!(
        code(daemon.call(
            a,
            Request::ResultAccept {
                goal,
                result,
                head: None
            }
        )),
        ErrorCode::Conflict
    );
}

#[test]
fn viewer_reads_do_not_move_agent_cursor_or_write_and_revocation_is_immediate() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    let (task, assignment) = assigned(&mut daemon, agent, goal, principal);
    daemon.ok(
        owner,
        Request::ViewerEnroll {
            agent: principal,
            credential: credential(9).digest(),
        },
    );
    let viewer = daemon.connect(credential(9), None);
    assert!(matches!(
        daemon.ok(viewer, Request::Task { goal, task }),
        Response::Task(_)
    ));
    assert_eq!(
        code(daemon.call(viewer, progress(goal, assignment, 1))),
        ErrorCode::Denied
    );
    let before = daemon.ok(
        agent,
        Request::Events {
            goal,
            after: None,
            limit: 256,
        },
    );
    daemon.ok(
        viewer,
        Request::Events {
            goal,
            after: Some(500),
            limit: 256,
        },
    );
    assert_eq!(
        daemon.ok(
            agent,
            Request::Events {
                goal,
                after: None,
                limit: 256
            }
        ),
        before
    );
    assert!(pending(&mut daemon, viewer, goal).claimed.is_empty());
    assert!(matches!(
        daemon.hello(credential(9), Some(session(9))).1,
        ServerHello::Refused {
            error: ApiError {
                code: ErrorCode::Invalid,
                ..
            },
            ..
        }
    ));
    daemon.ok(owner, Request::AgentRevoke { agent: principal });
    assert_eq!(
        code(daemon.call(viewer, Request::Board { goal })),
        ErrorCode::Denied
    );
    assert_eq!(
        code(daemon.call(agent, Request::Board { goal })),
        ErrorCode::Denied
    );
}

#[test]
fn sessions_survive_restart_drop_requires_finished_claim_and_binding_is_permanent() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    let record = SessionRecord {
        client: "test adapter".into(),
        state: SessionState::Ready,
        client_session: Some("test-session".into()),
        capabilities: SessionCapabilities::default(),
        detail: vec![1, 2],
    };
    daemon.ok(
        agent,
        Request::SessionReport {
            record: record.clone(),
        },
    );
    let (_, assignment) = assigned(&mut daemon, agent, goal, principal);
    authorize(&mut daemon, owner, goal, assignment);
    daemon.ok(agent, Request::TaskClaim { goal, assignment });
    assert_eq!(
        code(daemon.call(
            agent,
            Request::SessionDrop {
                instance: session(1).instance()
            }
        )),
        ErrorCode::Conflict
    );
    daemon.restart();
    let agent = daemon.connect(credential(1), Some(session(1)));
    let Response::Session(view) = daemon.ok(agent, Request::Session { instance: None }) else {
        panic!()
    };
    assert_eq!(view.record, record);
    assert_eq!(view.claims.len(), 1);
    daemon.ok(
        agent,
        Request::TaskFail {
            goal,
            assignment,
            generation: 1,
            reason: "cannot complete".into(),
        },
    );
    daemon.ok(
        agent,
        Request::SessionDrop {
            instance: session(1).instance(),
        },
    );
    assert_eq!(
        code(daemon.call(agent, Request::Session { instance: None })),
        ErrorCode::NotFound
    );
    daemon.enroll("other", 2, false);
    let other = daemon.connect(credential(2), Some(session(1)));
    assert_eq!(
        code(daemon.call(other, Request::SessionReport { record })),
        ErrorCode::Denied
    );
}

#[test]
fn revisions_notes_and_idempotent_mutation_survive_replay() {
    let (mut daemon, _, _, agent, goal) = setup();
    let request = Request::NoteAdd {
        goal,
        about: None,
        supersedes: None,
        text: "A finding".into(),
    };
    let response = daemon.keyed(agent, 44, request.clone()).unwrap();
    assert_eq!(daemon.keyed(agent, 44, request.clone()).unwrap(), response);
    let note = event(response.clone());
    daemon.ok(
        agent,
        Request::NoteAdd {
            goal,
            about: None,
            supersedes: Some(note),
            text: "Corrected finding".into(),
        },
    );
    let revision = event(daemon.ok(
        agent,
        Request::DocRevise {
            goal,
            doc: Doc::Plan,
            base: None,
            text: "The plan".into(),
        },
    ));
    daemon.ok(agent, Request::DocAccept { goal, revision });
    let notes = daemon.ok(agent, Request::Notes { goal, about: None });
    let document = daemon.ok(
        agent,
        Request::DocRead {
            goal,
            doc: Doc::Plan,
        },
    );
    let count = daemon.store.log(&goal, 0, 1000).unwrap().len();
    daemon.restart();
    let agent = daemon.connect(credential(1), None);
    assert_eq!(daemon.keyed(agent, 44, request).unwrap(), response);
    assert_eq!(daemon.store.log(&goal, 0, 1000).unwrap().len(), count);
    assert_eq!(
        daemon.ok(agent, Request::Notes { goal, about: None }),
        notes
    );
    assert_eq!(
        daemon.ok(
            agent,
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
    let (mut daemon, principal, _, agent, goal) = setup();
    let removal = event(daemon.ok(
        agent,
        Request::MemberRemove {
            goal,
            member: principal,
        },
    ));
    let event = daemon.store.event(&removal).unwrap().unwrap();
    let payload = event.header().payload.unwrap();
    assert_eq!(payload.key_epoch, 1);
    let key = &daemon.node.goals[&goal].keys[&1];
    assert_eq!(
        locust_proto::seal::open(
            &goal,
            key,
            &daemon.store.blob(&payload.hash).unwrap().unwrap()
        )
        .unwrap(),
        b""
    );
    assert_eq!(
        code(daemon.call(
            agent,
            Request::NoteAdd {
                goal,
                about: None,
                supersedes: None,
                text: "removed".into()
            }
        )),
        ErrorCode::Denied
    );
}

#[test]
fn waits_observe_committed_revisions_timeout_and_recheck_revocation() {
    use locust_proto::api::WaitOutcome;
    let (mut daemon, principal, owner, agent, goal) = setup();
    let seen = pending(&mut daemon, agent, goal).revision;
    let frame = daemon.frame(Request::Wait {
        goal,
        seen,
        timeout_ms: 1000,
    });
    let Step::Park(parked) = daemon.step(agent, frame, 100) else {
        panic!()
    };
    assert!(matches!(
        daemon.node.resume(agent, &parked, false, 101),
        Step::Park(_)
    ));
    assert!(matches!(
        daemon.node.resume(agent, &parked, true, 1100),
        Step::Reply(locust_proto::api::ResponseFrame {
            result: Ok(Response::Waited(WaitOutcome::NoEvent)),
            ..
        })
    ));
    let frame = daemon.frame(Request::Wait {
        goal,
        seen,
        timeout_ms: 1000,
    });
    let Step::Park(parked) = daemon.step(agent, frame, 1200) else {
        panic!()
    };
    daemon
        .on_behalf(
            owner,
            principal,
            Request::NoteAdd {
                goal,
                about: None,
                supersedes: None,
                text: "changed".into(),
            },
        )
        .unwrap();
    assert!(daemon.node.take_changed().contains(&goal));
    let Step::Reply(reply) = daemon.node.resume(agent, &parked, false, 1201) else {
        panic!()
    };
    let Response::Waited(WaitOutcome::Work(work)) = reply.result.unwrap() else {
        panic!()
    };
    assert!(work.revision > seen);
    let frame = daemon.frame(Request::Wait {
        goal,
        seen: work.revision,
        timeout_ms: 1000,
    });
    let Step::Park(parked) = daemon.step(agent, frame, 1300) else {
        panic!()
    };
    daemon.ok(owner, Request::AgentRevoke { agent: principal });
    let Step::Reply(reply) = daemon.node.resume(agent, &parked, false, 1301) else {
        panic!()
    };
    assert_eq!(code(reply.result), ErrorCode::Denied);
}

#[test]
fn decline_reassignment_rejection_and_attempt_budget_are_enforced() {
    let (mut daemon, principal, owner, agent, goal) = setup();
    let (task, first) = assigned(&mut daemon, agent, goal, principal);
    daemon.ok(
        agent,
        Request::TaskDecline {
            goal,
            assignment: first,
        },
    );
    assert_eq!(
        code(daemon.call(
            agent,
            Request::TaskClaim {
                goal,
                assignment: first
            }
        )),
        ErrorCode::Conflict
    );
    let second = event(daemon.ok(
        agent,
        Request::TaskAssign {
            goal,
            task,
            assignee: principal,
        },
    ));
    authorize(&mut daemon, owner, goal, second);
    daemon.ok(
        agent,
        Request::TaskClaim {
            goal,
            assignment: second,
        },
    );
    assert_eq!(
        code(daemon.call(agent, progress(goal, first, 1))),
        ErrorCode::Superseded
    );
    let result = event(daemon.ok(agent, submit(goal, second, 1)));
    daemon.ok(
        agent,
        Request::ResultReject {
            goal,
            result,
            reason: "needs revision".into(),
        },
    );
    assert!(pending(&mut daemon, agent, goal).to_review.is_empty());
    assert!(pending(&mut daemon, agent, goal).claimed.is_empty());
    assert_eq!(
        code(daemon.call(
            agent,
            Request::ResultAccept {
                goal,
                result,
                head: None
            }
        )),
        ErrorCode::Conflict
    );
    assert_eq!(
        code(daemon.call(
            agent,
            Request::TaskAssign {
                goal,
                task,
                assignee: principal
            }
        )),
        ErrorCode::Conflict
    );
}
