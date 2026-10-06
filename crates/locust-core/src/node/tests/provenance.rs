//! Signed source declarations are inspectable attribution, never causal proof.
use super::lifecycle::{authorize, event, finding, offered, setup};
use super::*;
use crate::sync::Host;
use locust_proto::api::{ContributionInspection, Standing};
use locust_proto::event::{Body, Context, Doc, Event, Header, ReviewVerdict, Scope};
use locust_proto::id::{EventId, GoalId};

fn inspect(
    d: &mut Daemon,
    conn: ConnId,
    goal: GoalId,
    contribution: EventId,
) -> ContributionInspection {
    let Response::ContributionInspected(detail) =
        d.ok(conn, Request::ContributionInspect { goal, contribution })
    else {
        panic!()
    };
    *detail
}
fn imported(d: &mut Daemon, principal: PublicKey, goal: GoalId, body: Body) -> EventId {
    let next = d.node.goals[&goal].goal.next(&principal).unwrap();
    let event = Event::sign(
        Header {
            version: locust_proto::PROTOCOL_VERSION,
            goal,
            author: principal,
            seq: next.seq,
            prev: next.prev,
            anchor: Some(next.anchor),
            parents: vec![],
            at_ms: 1000,
            payload: None,
            body,
        },
        d.node.signer(&principal).unwrap(),
    )
    .unwrap();
    assert_eq!(
        Host::replica(&mut d.node, &goal)
            .unwrap()
            .receive(vec![event.to_wire()]),
        Ok(1)
    );
    event.id()
}
fn sourced(goal: GoalId, sources: Vec<EventId>) -> Request {
    let mut request = finding(goal, "Declared use of attributed material");
    let Request::ContributionPublish {
        sources: declared, ..
    } = &mut request
    else {
        unreachable!()
    };
    *declared = sources;
    request
}

#[test]
fn local_publish_requires_sources_held_in_the_same_goal_and_inspection_is_observational() {
    let (mut d, principal, owner, agent, goal) = setup();
    let source = event(d.ok(agent, finding(goal, "Original source")));
    assert_eq!(
        code(d.call(agent, sourced(goal, vec![EventId([77; 32])]))),
        ErrorCode::NotFound
    );
    let Response::GoalCreated { goal: other } = d.ok(
        owner,
        Request::GoalCreate {
            name: "host".into(),
            agent: principal,
            title: "Other goal".into(),
            formation_json: Some("{\"schema_version\":2}".into()),

            inputs: Default::default(),
        },
    ) else {
        panic!()
    };
    let foreign = d.node.goals[&other].state().head.unwrap();
    assert_eq!(
        code(d.call(agent, sourced(goal, vec![foreign]))),
        ErrorCode::NotFound
    );
    let result = event(d.ok(agent, sourced(goal, vec![source])));
    let before = d.ok(agent, Request::Pending { goal });
    let detail = inspect(&mut d, owner, goal, result);
    assert_eq!(detail.declared_sources[0].event, source);
    assert_eq!(
        detail.declared_sources[0]
            .detail
            .as_ref()
            .unwrap()
            .text
            .as_deref(),
        Some("Original source")
    );
    assert_eq!(
        detail.declared_sources[0]
            .detail
            .as_ref()
            .unwrap()
            .view
            .standing,
        Standing::Effective
    );
    assert!(detail.attempt.is_none());
    assert!(detail.task_round.is_none());
    assert_eq!(d.ok(agent, Request::Pending { goal }), before);
    let Response::Contributions(list) = d.ok(agent, Request::Contributions { goal, task: None })
    else {
        panic!()
    };
    assert_eq!(
        list.iter()
            .find(|entry| entry.contribution == result)
            .unwrap()
            .sources,
        [source]
    );
    d.restart();
    let reader = d.owner();
    assert_eq!(inspect(&mut d, reader, goal, result), detail);
    let source_hash = detail.declared_sources[0]
        .detail
        .as_ref()
        .unwrap()
        .payload
        .unwrap()
        .hash;
    d.on_behalf(
        reader,
        principal,
        Request::BlobWithdraw {
            goal,
            hash: source_hash,
        },
    )
    .unwrap();
    let unavailable = inspect(&mut d, reader, goal, result);
    let source_detail = unavailable.declared_sources[0].detail.as_ref().unwrap();
    assert!(source_detail.text.is_none());
    assert_eq!(
        source_detail.content[0].state,
        locust_proto::api::BlobState::Unavailable
    );
    assert_eq!(
        code(d.call(
            reader,
            Request::ContributionInspect {
                goal,
                contribution: foreign
            }
        )),
        ErrorCode::NotFound
    );
    let noncontribution = d.node.goals[&goal].state().head.unwrap();
    assert_eq!(
        code(d.call(
            reader,
            Request::ContributionInspect {
                goal,
                contribution: noncontribution
            }
        )),
        ErrorCode::Invalid
    );
}

#[test]
fn held_excluded_sources_remain_attributed_and_do_not_exclude_the_result() {
    let (mut d, principal, _, agent, goal) = setup();
    let round = d.node.goals[&goal].state().current_rules.unwrap();
    let excluded = imported(
        &mut d,
        principal,
        goal,
        Body::ContributionPublished {
            context: Context {
                scope: Scope::Document(Doc::Plan),
                round,
            },
            attempt: None,
            sources: vec![],
            artifacts: vec![],
        },
    );
    assert_eq!(
        inspect(&mut d, agent, goal, excluded)
            .contribution
            .view
            .standing,
        Standing::Excluded
    );
    let result = event(d.ok(agent, sourced(goal, vec![excluded])));
    let detail = inspect(&mut d, agent, goal, result);
    assert_eq!(detail.contribution.view.standing, Standing::Effective);
    assert_eq!(
        detail.declared_sources[0]
            .detail
            .as_ref()
            .unwrap()
            .view
            .standing,
        Standing::Excluded
    );
}

#[test]
fn a_replica_missing_a_declared_source_can_approve_and_inspect_without_inventing_details() {
    let (mut d, principal, _, agent, goal) = setup();
    let round = d.node.goals[&goal].state().current_rules.unwrap();
    let absent = EventId([79; 32]);
    let result = imported(
        &mut d,
        principal,
        goal,
        Body::ContributionPublished {
            context: Context {
                scope: Scope::Goal,
                round,
            },
            attempt: None,
            sources: vec![absent],
            artifacts: vec![],
        },
    );
    let detail = inspect(&mut d, agent, goal, result);
    assert_eq!(detail.contribution.view.standing, Standing::Effective);
    assert_eq!(detail.declared_sources[0].event, absent);
    assert!(detail.declared_sources[0].detail.is_none());
    d.ok(
        agent,
        Request::ReviewRecord {
            goal,
            subject: result,
            verdict: ReviewVerdict::Approve,
            text: "Ordinary completion evidence".into(),
        },
    );
    let Response::Contributions(list) = d.ok(agent, Request::Contributions { goal, task: None })
    else {
        panic!()
    };
    assert!(
        list.iter()
            .find(|entry| entry.contribution == result)
            .unwrap()
            .approved
    );
    assert!(
        inspect(&mut d, agent, goal, result).declared_sources[0]
            .detail
            .is_none()
    );
}

#[test]
fn inspection_traces_the_exact_attempt_and_task_round() {
    let (mut d, principal, owner, agent, goal) = setup();
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
    let mut request = sourced(goal, vec![offer]);
    let Request::ContributionPublish {
        attempt,
        generation,
        ..
    } = &mut request
    else {
        unreachable!()
    };
    *attempt = Some(claim.attempt);
    *generation = Some(claim.generation);
    let result = event(d.ok(agent, request));
    let detail = inspect(&mut d, agent, goal, result);
    assert_eq!(detail.attempt.as_ref().unwrap().event, claim.attempt);
    assert!(matches!(
        detail
            .attempt
            .as_ref()
            .unwrap()
            .detail
            .as_ref()
            .unwrap()
            .body,
        Body::AttemptStarted { .. }
    ));
    let round = detail.task_round.unwrap();
    assert!(matches!(
        round.detail.unwrap().body,
        Body::TaskOpened { .. }
    ));
    assert_eq!(
        round.event,
        match task {
            locust_proto::event::TaskId::Authored(id) => id,
            _ => panic!(),
        }
    );
}
