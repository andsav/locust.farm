//! The restore guard across daemons on several computers, through the request
//! API and the transport: what a start finds, what ends a hold, what a held
//! daemon signs and answers, and whom it asks.

use super::authorization::{governance_key, join_local};
use super::delivery::{Network, pipeline_request};
use super::lifecycle::finding;
use super::*;
use crate::sync::Host;
use locust_proto::api::{
    DaemonStatus, GoalStatus, GoalSummary, GuardReason, GuardView, Halt, InvitationState,
    InvitationSummary, Level, Membership,
};
use locust_proto::engine::{ExchangeId, PeerEngine, PeerInput, PeerOutput, PeerTime};
use locust_proto::event::{Body, Event, Scope};
use locust_proto::id::{EndpointId, EventId, GoalId};
use locust_proto::invite::{Invitation, JoinRequest, Ticket};
use locust_proto::store::Store;
use locust_proto::sync::{Refusal, SyncMessage};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The credential tag of the host's agent, on daemon 0.
const HOST: u8 = 1;

/// The credential tag of the agent daemon `i` joins with.
fn tag(i: usize) -> u8 {
    10 + i as u8
}

/// The peer-review preset: a result needs another member's approval, except
/// while the goal has one member.
fn peer_review() -> String {
    let formation = locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == "peer-review")
        .unwrap()
        .formation;
    serde_json::to_string(&formation).unwrap()
}

/// A goal hosted on daemon `i` under the peer-review preset, whose host's
/// agent is enrolled under `tag` and acts at auto.
fn hosted(net: &mut Network, i: usize, tag: u8) -> (GoalId, PublicKey) {
    let agent = net.nodes[i].enroll("host", tag);
    let owner = net.nodes[i].owner();
    let Response::GoalCreated { goal } = net.nodes[i].ok(
        owner,
        Request::GoalCreate {
            name: "host".into(),
            agent,
            title: "Guarded".into(),
            formation_json: Some(peer_review()),
            inputs: Default::default(),
        },
    ) else {
        panic!()
    };
    net.nodes[i].ok(
        owner,
        Request::LevelSet {
            goal,
            agent,
            level: Level::Auto,
        },
    );
    (goal, agent)
}

/// A goal hosted on daemon `i` under the default rules, whose host's agent is
/// enrolled under `tag` and acts at ask.
fn hosted_at_ask(net: &mut Network, i: usize, tag: u8) -> (GoalId, PublicKey) {
    let agent = net.nodes[i].enroll("host", tag);
    let owner = net.nodes[i].owner();
    let Response::GoalCreated { goal } = net.nodes[i].ok(
        owner,
        Request::GoalCreate {
            name: "host".into(),
            agent,
            title: "Guarded".into(),
            formation_json: Some("{\"schema_version\":2}".into()),
            inputs: BTreeMap::new(),
        },
    ) else {
        panic!()
    };
    net.nodes[i].ok(
        owner,
        Request::LevelSet {
            goal,
            agent,
            level: Level::Ask,
        },
    );
    (goal, agent)
}

fn invite(daemon: &mut Daemon, goal: GoalId) -> Ticket {
    let owner = daemon.owner();
    let Response::Invited { ticket } = daemon.ok(
        owner,
        Request::GoalInvite {
            role: None,
            goal,
            expires_ms: 604_801_000,
        },
    ) else {
        panic!()
    };
    ticket
}

fn join_request(agent: PublicKey, ticket: Ticket) -> Request {
    Request::GoalJoin {
        name: "member".into(),
        agent,
        ticket,
        level: Level::Auto,
    }
}

/// An agent enrolled under `tag` on daemon `at` joins the goal hosted on
/// daemon `host` under a name of its own, and the network settles.
fn join(net: &mut Network, host: usize, at: usize, goal: GoalId, tag: u8) -> PublicKey {
    let agent = net.nodes[at].enroll(&format!("member-{tag}"), tag);
    let ticket = invite(&mut net.nodes[host], goal);
    let owner = net.nodes[at].owner();
    net.nodes[at].ok(
        owner,
        Request::GoalJoin {
            name: format!("member-{tag}"),
            agent,
            ticket,
            level: Level::Auto,
        },
    );
    settle(net);
    assert!(net.nodes[host].node.goals[&goal].is_member(&agent));
    assert!(net.nodes[at].node.goals[&goal].is_member(&agent));
    agent
}

/// Daemon `member` joins the goal daemon `host` hosts with a new agent of
/// credential `tag`, named as [`join_request`] names it, and the two exchange
/// until the goal is the same on both. Answers the agent and its ticket, so a
/// test can sign the same join again.
fn join_with_ticket(
    net: &mut Network,
    host: usize,
    member: usize,
    goal: GoalId,
    tag: u8,
) -> (PublicKey, Ticket) {
    let ticket = invite(&mut net.nodes[host], goal);
    let agent = net.nodes[member].enroll("member", tag);
    let owner = net.nodes[member].owner();
    net.nodes[member].ok(owner, join_request(agent, ticket.clone()));
    net.poll(1);
    assert!(net.nodes[host].node.goals[&goal].is_member(&agent));
    settle(net);
    assert!(net.nodes[member].node.goals[&goal].is_member(&agent));
    (agent, ticket)
}

/// A goal hosted on daemon 0 at ask, joined from each of `members`. Answers
/// the goal, the host's agent and each member's agent and ticket by daemon.
fn shared_goal(
    net: &mut Network,
    members: &[usize],
) -> (GoalId, PublicKey, BTreeMap<usize, (PublicKey, Ticket)>) {
    let (goal, host) = hosted_at_ask(net, 0, HOST);
    let joined = members
        .iter()
        .map(|&i| (i, join_with_ticket(net, 0, i, goal, tag(i))))
        .collect();
    (goal, host, joined)
}

/// Lets every daemon that is up exchange until nothing is left to send.
fn settle(net: &mut Network) {
    for _ in 0..3 {
        net.poll(31_000);
    }
}

/// The agent enrolled under `tag` on daemon `i` posts a finding. The test
/// clock stands still, so each finding's text carries the daemon's request
/// count: a finding posted again at a lost record's position is another
/// record, not the lost one.
fn post(net: &mut Network, i: usize, tag: u8, goal: GoalId) -> Result<EventId, ApiError> {
    let agent = net.nodes[i].connect(credential(tag), None);
    let text = format!("finding {} of daemon {i}", net.nodes[i].next_request);
    let result = net.nodes[i].call(agent, finding(goal, &text));
    net.nodes[i].node.disconnect(agent);
    result.map(|response| {
        let Response::Recorded { event } = response else {
            panic!("{response:?}")
        };
        event
    })
}

/// As [`post`], and it must be recorded: the record.
fn posted(net: &mut Network, i: usize, tag: u8, goal: GoalId) -> Event {
    let event = post(net, i, tag, goal).expect("the finding is recorded");
    record(&net.nodes[i], &event)
}

/// How many records of `key` the daemon holds in the goal.
fn signed_by(daemon: &Daemon, goal: GoalId, key: PublicKey) -> u64 {
    daemon
        .store
        .log(&goal, 0, usize::MAX)
        .unwrap()
        .iter()
        .filter(|(_, event)| event.header().author == key)
        .count() as u64
}

/// The ids of the records `author` holds in `goal` on `daemon`.
fn records_of(daemon: &Daemon, goal: GoalId, author: &PublicKey) -> Vec<EventId> {
    daemon.node.goals[&goal]
        .goal
        .points(author)
        .iter()
        .map(|point| point.id)
        .collect()
}

fn record(daemon: &Daemon, id: &EventId) -> Event {
    daemon.store.event(id).unwrap().unwrap()
}

/// `author` posts a result on `daemon`, and the daemon stops after the commit
/// that holds it and before the one that signs the steps it starts.
fn post_and_stop(daemon: &mut Daemon, goal: GoalId, author: PublicKey) -> EventId {
    use crate::node::{callers::Actor, commit::Tx};
    let entry = &daemon.node.goals[&goal];
    let mut tx = Tx::none();
    let id = daemon
        .node
        .sign_for(
            &Actor {
                caller: Caller::Agent(author),
                principal: Some(author),
                owner_act: false,
                session: None,
            },
            entry,
            &author,
            Body::ContributionPublished {
                context: entry.goal.current_context(Scope::Goal).unwrap(),
                attempt: None,
                sources: vec![],
                artifacts: vec![],
            },
            Some("a result that asks for review"),
            1_000,
            &mut tx,
        )
        .unwrap();
    daemon.node.land_once(tx).unwrap();
    assert!(waiting(daemon, goal) > 0, "the result asks for a review");
    id
}

/// Steps the goal's rules want signed and nobody has signed yet.
fn waiting(daemon: &Daemon, goal: GoalId) -> usize {
    daemon.node.goals[&goal]
        .goal
        .evaluation()
        .desired_effects
        .len()
}

fn status(daemon: &mut Daemon, goal: GoalId) -> GoalStatus {
    let owner = daemon.owner();
    let Response::GoalStatus(status) = daemon.ok(owner, Request::GoalStatus { goal }) else {
        panic!()
    };
    daemon.node.disconnect(owner);
    status
}

/// The summary of `agent` in `goal`, as the owner's `status` lists it.
fn summary(daemon: &mut Daemon, goal: GoalId, agent: PublicKey) -> GoalSummary {
    let owner = daemon.owner();
    let Response::Status(DaemonStatus { goals, .. }) = daemon.ok(owner, Request::Status) else {
        panic!()
    };
    daemon.node.disconnect(owner);
    goals
        .into_iter()
        .find(|summary| summary.goal == goal && summary.member == agent)
        .expect("the agent is in the goal")
}

/// The owner continues the goal: the number of keys released.
fn continued(daemon: &mut Daemon, goal: GoalId) -> u32 {
    let owner = daemon.owner();
    let Response::Continued { keys } = daemon.ok(owner, Request::GoalContinue { goal }) else {
        panic!()
    };
    daemon.node.disconnect(owner);
    keys
}

fn invitations(daemon: &mut Daemon, goal: GoalId) -> Vec<InvitationSummary> {
    let owner = daemon.owner();
    let Response::Invitations { invitations } = daemon.ok(owner, Request::GoalInvitations { goal })
    else {
        panic!()
    };
    daemon.node.disconnect(owner);
    invitations
}

fn reasons(guard: &[GuardView]) -> Vec<(PublicKey, GuardReason)> {
    guard.iter().map(|view| (view.key, view.reason)).collect()
}

/// The keys whose own hold is `unheard`, when every hold is.
fn unheard(guard: &[GuardView]) -> BTreeSet<PublicKey> {
    assert!(guard.iter().all(|view| view.reason == GuardReason::Unheard));
    guard.iter().map(|view| view.key).collect()
}

fn governance_of(daemon: &Daemon, goal: GoalId) -> PublicKey {
    daemon.node.goals[&goal].state().governance.unwrap()
}

/// A record of the same key at the same position as `event`, and another.
fn twin(event: &Event, key: &locust_proto::crypto::Keypair) -> Event {
    let mut header = event.header().clone();
    header.at_ms += 1;
    let twin = Event::sign(header, key).unwrap();
    assert_ne!(twin.id(), event.id());
    twin
}

fn receive(daemon: &mut Daemon, goal: GoalId, event: &Event) {
    Host::replica(&mut daemon.node, &goal)
        .unwrap()
        .receive(vec![event.to_wire()])
        .unwrap();
}

#[test]
fn an_ordinary_restart_holds_nothing_and_signs_at_once() {
    // A host alone: its goal's two agents are both on its computer.
    let mut net = Network::with(1);
    let (goal, host) = hosted(&mut net, 0, 1);
    let agent = net.nodes[0].connect(credential(1), None);
    join_local(&mut net.nodes[0], agent, goal, 2);
    post_and_stop(&mut net.nodes[0], goal, host);
    let store = net.nodes[0].store.reopen();
    net.start_over(0, store);
    assert_eq!(waiting(&net.nodes[0], goal), 0, "signed inside the start");
    assert!(net.opened.is_empty());
    let view = status(&mut net.nodes[0], goal);
    assert!(view.guard.is_empty());
    assert_eq!((view.halted, view.restored), (None, None));
    post(&mut net, 0, 1, goal).unwrap();

    // A host whose only member is on a computer that cannot be reached.
    let mut net = Network::with(2);
    let (goal, host) = hosted(&mut net, 0, 1);
    join(&mut net, 0, 1, goal, 2);
    net.down.insert(1);
    post_and_stop(&mut net.nodes[0], goal, host);
    let opened = net.opened.len();
    let store = net.nodes[0].store.reopen();
    net.start_over(0, store);
    assert_eq!(waiting(&net.nodes[0], goal), 0, "signed inside the start");
    assert_eq!(net.opened.len(), opened, "before any exchange");
    let view = status(&mut net.nodes[0], goal);
    assert!(view.guard.is_empty());
    assert_eq!((view.halted, view.restored), (None, None));
    assert!(summary(&mut net.nodes[0], goal, host).guard.is_empty());
    post(&mut net, 0, 1, goal).unwrap();
}

#[test]
fn only_the_goals_that_are_behind_are_held() {
    let mut net = Network::with(2);
    let (behind, host) = hosted(&mut net, 0, 1);
    join(&mut net, 0, 1, behind, 2);
    let owner = net.nodes[0].owner();
    let Response::GoalCreated { goal: quiet } = net.nodes[0].ok(
        owner,
        Request::GoalCreate {
            name: "host".into(),
            agent: host,
            title: "Quiet".into(),
            formation_json: Some(peer_review()),
            inputs: Default::default(),
        },
    ) else {
        panic!()
    };
    net.nodes[0].ok(
        owner,
        Request::LevelSet {
            goal: quiet,
            agent: host,
            level: Level::Auto,
        },
    );
    join(&mut net, 0, 1, quiet, 3);
    let copy = snapshot(&net.nodes[0].store);
    let lost = post(&mut net, 0, 1, behind).unwrap();
    let lost = record(&net.nodes[0], &lost);
    settle(&mut net);
    // The result and the review it asked for.
    let signed = signed_by(&net.nodes[1], behind, host);
    assert_eq!(signed, lost.header().seq + 2);
    net.down.insert(1);
    net.start_over(0, copy);
    assert!(!net.nodes[0].store.has_event(&lost.id()).unwrap());

    let refused = post(&mut net, 0, 1, behind).unwrap_err();
    assert_eq!(refused.code, ErrorCode::ReadOnly);
    let view = status(&mut net.nodes[0], behind);
    assert_eq!(
        reasons(&view.guard),
        [(
            host,
            GuardReason::Behind {
                held: lost.header().seq,
                signed
            }
        )]
    );
    assert!(!view.guard[0].by_host);
    assert_eq!(view.guard[0].waiting, [Network::endpoint(1)]);
    // The governance key is not behind, so the goal is not halted for the host.
    assert_eq!(view.halted, None);
    assert_eq!(
        summary(&mut net.nodes[0], behind, host).halted,
        Some(Halt::SignerRecovery)
    );

    // The goal in which nothing was signed since the copy signs at once.
    let view = status(&mut net.nodes[0], quiet);
    assert!(view.guard.is_empty());
    assert_eq!(view.halted, None);
    assert_eq!(summary(&mut net.nodes[0], quiet, host).halted, None);
    post(&mut net, 0, 1, quiet).unwrap();
    // Both goals were found restored, from one copy of one file.
    assert_eq!(view.restored, Some(0));
    assert_eq!(status(&mut net.nodes[0], behind).restored, Some(0));
}

/// The owner binds the goal's rules again: a record of the governance key.
fn rebind(daemon: &mut Daemon, goal: GoalId) -> Result<Response, ApiError> {
    let expected = daemon.node.goals[&goal].state().current_rules.unwrap();
    let owner = daemon.owner();
    let result = daemon.call(
        owner,
        Request::RulesBind {
            no_role: true,
            goal,
            expected,
            formation_json: peer_review(),
            inputs: Default::default(),
        },
    );
    daemon.node.disconnect(owner);
    result
}

#[test]
fn a_goal_never_shared_is_not_held_by_a_missing_record() {
    for second_agent in [false, true] {
        let mut net = Network::with(1);
        let (goal, _) = hosted(&mut net, 0, 1);
        let agent = net.nodes[0].connect(credential(1), None);
        if second_agent {
            join_local(&mut net.nodes[0], agent, goal, 2);
        }
        let copy = snapshot(&net.nodes[0].store);
        let Response::Recorded { event: lost_rules } = rebind(&mut net.nodes[0], goal).unwrap()
        else {
            panic!()
        };
        let lost_rules = record(&net.nodes[0], &lost_rules);
        let lost = post(&mut net, 0, 1, goal).unwrap();
        let lost = record(&net.nodes[0], &lost);
        if second_agent {
            post(&mut net, 0, 2, goal).unwrap();
        }
        net.start_over(0, copy);
        assert!(!net.nodes[0].store.has_event(&lost_rules.id()).unwrap());
        assert!(!net.nodes[0].store.has_event(&lost.id()).unwrap());

        let view = status(&mut net.nodes[0], goal);
        assert!(view.guard.is_empty(), "{:?}", view.guard);
        assert_eq!(view.halted, None);
        assert_eq!(view.restored, Some(0));
        let Response::Recorded { event } = rebind(&mut net.nodes[0], goal).unwrap() else {
            panic!()
        };
        // The lost record reached no other computer: its position is free.
        let rules = record(&net.nodes[0], &event);
        assert_eq!(rules.header().seq, lost_rules.header().seq);
        let again = post(&mut net, 0, 1, goal).unwrap();
        let again = record(&net.nodes[0], &again);
        assert_eq!(again.header().seq, lost.header().seq);
        if second_agent {
            post(&mut net, 0, 2, goal).unwrap();
        }
    }
}

#[test]
fn on_the_hosts_computer_no_agent_signs_while_the_hosts_own_records_are_missing() {
    let mut net = Network::with(2);
    let (goal, host) = hosted(&mut net, 0, 1);
    let governance = governance_of(&net.nodes[0], goal);
    // The copy is older than the second admission: in it the host's agent is
    // the goal's only member, whose results count as posted.
    let copy = snapshot(&net.nodes[0].store);
    let member = join(&mut net, 0, 1, goal, 2);
    let signed = signed_by(&net.nodes[0], goal, governance);
    net.down.insert(1);
    net.start_over(0, copy);
    assert!(!net.nodes[0].node.goals[&goal].is_member(&member));

    let refused = post(&mut net, 0, 1, goal).unwrap_err();
    assert_eq!(refused.code, ErrorCode::ReadOnly);
    assert_eq!(code(rebind(&mut net.nodes[0], goal)), ErrorCode::ReadOnly);
    let view = status(&mut net.nodes[0], goal);
    assert_eq!(view.halted, Some(Halt::SignerRecovery));
    assert_eq!(
        reasons(&view.guard),
        [(
            governance,
            GuardReason::Behind {
                held: signed - 1,
                signed
            }
        )]
    );
    assert!(view.guard[0].by_host);
    // The agent has no hold of its own: the governance key's is its reason.
    let mine = summary(&mut net.nodes[0], goal, host);
    assert_eq!(mine.halted, Some(Halt::SignerRecovery));
    assert_eq!(mine.guard, view.guard);

    // The member's computer calls, is called back, and the admission returns.
    net.down.clear();
    settle(&mut net);
    assert!(net.nodes[0].node.goals[&goal].is_member(&member));
    let view = status(&mut net.nodes[0], goal);
    assert!(view.guard.is_empty(), "{:?}", view.guard);
    assert_eq!(view.halted, None);
    let result = post(&mut net, 0, 1, goal).unwrap();
    assert!(!net.nodes[0].node.goals[&goal].state().contributions[&result].approved);
    let reviewer = net.nodes[1].connect(credential(2), None);
    settle(&mut net);
    net.nodes[1].ok(
        reviewer,
        Request::ReviewRecord {
            goal,
            subject: result,
            verdict: locust_proto::event::ReviewVerdict::Approve,
            text: "approved".into(),
        },
    );
    settle(&mut net);
    assert!(net.nodes[0].node.goals[&goal].state().contributions[&result].approved);
}

#[test]
fn a_member_that_lacks_the_later_records_does_not_open_the_guard() {
    let mut net = Network::with(3);
    let (goal, host) = hosted(&mut net, 0, 1);
    join(&mut net, 0, 1, goal, 2);
    join(&mut net, 0, 2, goal, 3);
    let copy = snapshot(&net.nodes[0].store);
    // The later result reaches the second member only.
    net.down.insert(1);
    let lost = post(&mut net, 0, 1, goal).unwrap();
    settle(&mut net);
    assert!(net.nodes[2].store.has_event(&lost).unwrap());
    net.down = [2].into();
    net.start_over(0, copy);
    settle(&mut net);
    assert!(!net.nodes[1].store.has_event(&lost).unwrap());

    // The first member answered and holds less than the mark.
    assert_eq!(code(post(&mut net, 0, 1, goal)), ErrorCode::ReadOnly);
    let view = status(&mut net.nodes[0], goal);
    assert_eq!(reasons(&view.guard).len(), 1);
    assert_eq!(view.guard[0].key, host);
    assert!(matches!(view.guard[0].reason, GuardReason::Behind { .. }));
    assert_eq!(view.guard[0].heard, [Network::endpoint(1)]);
    assert_eq!(view.guard[0].waiting, [Network::endpoint(2)]);

    // The other member answers with the record.
    net.down.clear();
    settle(&mut net);
    assert!(net.nodes[0].store.has_event(&lost).unwrap());
    assert!(status(&mut net.nodes[0], goal).guard.is_empty());
    let after = post(&mut net, 0, 1, goal).unwrap();
    let after = record(&net.nodes[0], &after);
    assert_eq!(after.header().seq, signed_by(&net.nodes[2], goal, host));
    settle(&mut net);
    for daemon in &net.nodes {
        assert_eq!(daemon.node.goals[&goal].goal.fork_point(&host), None);
    }
}

#[test]
fn a_copy_of_unknown_age_on_the_hosts_computer_waits_for_the_person() {
    let mut net = Network::with(3);
    let (goal, host) = hosted(&mut net, 0, 1);
    join(&mut net, 0, 1, goal, 2);
    join(&mut net, 0, 2, goal, 3);
    let governance = governance_of(&net.nodes[0], goal);
    post_and_stop(&mut net.nodes[0], goal, host);
    let copy = snapshot_all(&net.nodes[0].store);
    net.start_over(0, copy);
    assert!(waiting(&net.nodes[0], goal) > 0, "the step waits");
    settle(&mut net);
    let view = status(&mut net.nodes[0], goal);
    // Every key this daemon holds in the goal is unheard.
    assert_eq!(unheard(&view.guard), BTreeSet::from([governance, host]));
    assert_eq!(
        view.guard[0].heard,
        [Network::endpoint(1), Network::endpoint(2)]
    );
    assert!(view.guard[0].waiting.is_empty());
    assert_eq!(view.halted, Some(Halt::SignerRecovery));
    assert_eq!(view.restored, Some(0));
    // Both other computers were heard from, and nothing is signed.
    assert!(waiting(&net.nodes[0], goal) > 0);
    let refused = post(&mut net, 0, 1, goal).unwrap_err();
    assert_eq!(refused.code, ErrorCode::ReadOnly);
    assert!(
        refused.message.contains("waiting for its owner"),
        "{refused}"
    );
    assert_eq!(code(rebind(&mut net.nodes[0], goal)), ErrorCode::ReadOnly);

    assert_eq!(continued(&mut net.nodes[0], goal), 2);
    assert_eq!(waiting(&net.nodes[0], goal), 0, "signed at once");
    let view = status(&mut net.nodes[0], goal);
    assert!(view.guard.is_empty());
    assert_eq!(view.halted, None);
    post(&mut net, 0, 1, goal).unwrap();

    // The trace the rule answers: a computer the copy lists can answer with
    // nothing because it was removed since, and one admitted since is not
    // listed at all, so hearing from the listed ones shows nothing.
    let mut net = Network::with(3);
    let (goal, host) = hosted(&mut net, 0, 1);
    let removed = join(&mut net, 0, 1, goal, 2);
    let governance = governance_of(&net.nodes[0], goal);
    let copy = snapshot_all(&net.nodes[0].store);
    // After the copy one member was removed and another admitted, while the
    // removed member's computer was away: it holds what the copy holds.
    net.down.insert(1);
    let admitted = join(&mut net, 0, 2, goal, 3);
    let owner = net.nodes[0].owner();
    net.nodes[0].ok(
        owner,
        Request::MemberRemove {
            goal,
            member: removed,
        },
    );
    settle(&mut net);
    let later = net.nodes[0].store.log(&goal, 0, usize::MAX).unwrap().len();

    net.down = [2].into();
    net.start_over(0, copy);
    let since = net.opened.len();
    let held = net.nodes[0].store.log(&goal, 0, usize::MAX).unwrap().len();
    assert!(held < later);
    settle(&mut net);
    // The removed member's computer answers and brings nothing.
    let view = status(&mut net.nodes[0], goal);
    assert_eq!(unheard(&view.guard), BTreeSet::from([governance, host]));
    assert_eq!(view.guard[0].heard, [Network::endpoint(1)]);
    assert!(
        view.guard[0].waiting.is_empty(),
        "{:?} {:?}",
        view.guard,
        net.opened
    );
    assert_eq!(
        net.nodes[0].store.log(&goal, 0, usize::MAX).unwrap().len(),
        held
    );
    assert!(!net.opened[since..].contains(&(0, 2)));

    // The admitted member's computer, which the copy does not list, calls,
    // is called back and returns the later records.
    net.down.clear();
    net.poll_from(&[2], 31_000);
    assert_eq!(
        net.nodes[0].store.log(&goal, 0, usize::MAX).unwrap().len(),
        held,
        "a caller's own exchange brings nothing"
    );
    let called = net.opened.len();
    net.poll_from(&[0], 1);
    assert!(net.opened[called..].contains(&(0, 2)));
    assert_eq!(
        net.nodes[0].store.log(&goal, 0, usize::MAX).unwrap().len(),
        later
    );
    let entry = &net.nodes[0].node.goals[&goal];
    assert!(entry.is_member(&admitted) && !entry.is_member(&removed));
    // A goal this computer hosts waits for the person all the same.
    let view = status(&mut net.nodes[0], goal);
    assert_eq!(unheard(&view.guard), BTreeSet::from([governance, host]));
    assert_eq!(continued(&mut net.nodes[0], goal), 2);
    assert!(status(&mut net.nodes[0], goal).guard.is_empty());
}

/// The mark of `key` in `goal` as the daemon's marks read now.
fn mark(daemon: &Daemon, goal: GoalId, key: PublicKey) -> Option<locust_proto::store::Mark> {
    daemon
        .store
        .marks()
        .unwrap()
        .kept
        .unwrap()
        .into_iter()
        .find(|mark| mark.goal == goal && mark.key == key)
}

#[test]
fn a_mark_short_of_the_store_is_raised_at_the_start_before_any_exchange() {
    let mut net = Network::with(2);
    let (goal, _) = hosted(&mut net, 0, 1);
    join(&mut net, 0, 1, goal, 2);
    let governance = governance_of(&net.nodes[0], goal);
    let before = net.nodes[0].store.marks_handle().copy();
    let Response::Recorded { event: last } = rebind(&mut net.nodes[0], goal).unwrap() else {
        panic!()
    };
    let last = record(&net.nodes[0], &last);
    assert_eq!(
        mark(&net.nodes[0], goal, governance).unwrap().point.id,
        last.id()
    );
    // The database commit landed and the marks were put back as they were
    // before it, as after a crash before the marks file was synced.
    net.down.insert(1);
    let records = net.nodes[0].store.log(&goal, 0, usize::MAX).unwrap().len();
    let store = net.nodes[0].store.reopen().with_marks(before);
    let short = store.marks().unwrap().kept.unwrap();
    let short = short
        .iter()
        .find(|mark| mark.goal == goal && mark.key == governance)
        .unwrap();
    assert!(short.point.seq < last.header().seq);
    let opened = net.opened.len();
    net.start_over(0, store);

    // The start raised the mark in its own commit: it signed nothing, and no
    // exchange had opened.
    assert_eq!(net.opened.len(), opened);
    assert_eq!(
        net.nodes[0].store.log(&goal, 0, usize::MAX).unwrap().len(),
        records
    );
    let raised = mark(&net.nodes[0], goal, governance).unwrap();
    assert_eq!(raised.point.id, last.id());
    assert_eq!(raised.point.seq, last.header().seq);
    assert!(raised.shared);
    // The start was ordinary.
    let view = status(&mut net.nodes[0], goal);
    assert!(view.guard.is_empty());
    assert_eq!(view.restored, None);
    let Response::Recorded { event } = rebind(&mut net.nodes[0], goal).unwrap() else {
        panic!()
    };
    let next = record(&net.nodes[0], &event);
    assert_eq!(next.header().seq, last.header().seq + 1);
    assert_eq!(next.header().prev, Some(last.id()));
}

#[test]
fn an_exchange_that_brought_records_does_not_count_as_hearing() {
    let mut net = Network::with(2);
    let (goal, _) = hosted(&mut net, 0, 1);
    let member = join(&mut net, 0, 1, goal, 2);
    // A copy of unknown age on the member's computer, older than a result.
    let copy = snapshot_all(&net.nodes[1].store);
    let later = post(&mut net, 0, 1, goal).unwrap();
    net.start_over(1, copy);
    assert!(!net.nodes[1].store.has_event(&later).unwrap());
    let held = |net: &mut Network| reasons(&summary(&mut net.nodes[1], goal, member).guard);
    assert_eq!(held(&mut net), [(member, GuardReason::Unheard)]);

    // Only the member's computer dials. Its first exchange brings the result.
    let opened = net.opened.len();
    net.poll_from(&[1], 1);
    assert_eq!(net.opened[opened..], [(1, 0)]);
    assert!(net.nodes[1].store.has_event(&later).unwrap());
    assert_eq!(held(&mut net), [(member, GuardReason::Unheard)]);
    assert_eq!(code(post(&mut net, 1, 2, goal)), ErrorCode::ReadOnly);
    let view = &summary(&mut net.nodes[1], goal, member).guard[0];
    assert!(view.heard.is_empty());
    assert_eq!(view.waiting, [Network::endpoint(0)]);

    // The next, opened at once because the goal changed, brings nothing.
    net.poll_from(&[1], 1);
    assert_eq!(net.opened[opened..], [(1, 0), (1, 0)]);
    assert!(held(&mut net).is_empty());
    post(&mut net, 1, 2, goal).unwrap();
}

/// Hands daemon `i` the records `pick` chooses from `from`'s log of the goal,
/// as an exchange would.
fn hand(net: &mut Network, from: usize, i: usize, goal: GoalId, pick: impl Fn(&Event) -> bool) {
    let events: Vec<_> = net.nodes[from]
        .store
        .log(&goal, 0, usize::MAX)
        .unwrap()
        .into_iter()
        .map(|(_, event)| event)
        .filter(|event| pick(event))
        .map(|event| event.to_wire())
        .collect();
    assert!(!events.is_empty());
    Host::replica(&mut net.nodes[i].node, &goal)
        .unwrap()
        .receive(events)
        .unwrap();
}

#[test]
fn a_key_admitted_while_catching_up_is_held_with_the_rest() {
    let mut net = Network::with(2);
    let (goal, _) = hosted(&mut net, 0, 1);
    join(&mut net, 0, 1, goal, 2);
    let governance = governance_of(&net.nodes[0], goal);
    // A second agent on the member's computer asks to join; the copy is
    // taken while it waits for its admission.
    let local = net.nodes[1].enroll("local", 3);
    let ticket = invite(&mut net.nodes[0], goal);
    let owner = net.nodes[1].owner();
    net.nodes[1].ok(
        owner,
        Request::GoalJoin {
            name: "local".into(),
            agent: local,
            ticket,
            level: Level::Auto,
        },
    );
    let copy = snapshot(&net.nodes[1].store);
    settle(&mut net);
    let admission = net.nodes[0].node.goals[&goal].state().members[&local].admission;
    post(&mut net, 1, 3, goal).unwrap();
    settle(&mut net);
    let signed = signed_by(&net.nodes[0], goal, local);
    assert!(signed > 0);
    net.down.insert(0);
    net.start_over(1, copy);
    assert!(!net.nodes[1].node.goals[&goal].is_member(&local));

    // The admission arrives while this computer is catching up, before the
    // agent's own records.
    hand(&mut net, 0, 1, goal, |event| {
        event.header().author == governance && event.id() == admission
    });
    let entry = &net.nodes[1].node.goals[&goal];
    assert!(entry.is_member(&local));
    assert!(entry.goal.points(&local).is_empty());
    assert_eq!(code(post(&mut net, 1, 3, goal)), ErrorCode::ReadOnly);
    assert_eq!(
        reasons(&summary(&mut net.nodes[1], goal, local).guard),
        [(local, GuardReason::Behind { held: 0, signed })]
    );
    assert_eq!(
        summary(&mut net.nodes[1], goal, local).halted,
        Some(Halt::SignerRecovery)
    );

    // Its own log returns. It was just admitted here, so it still waits to
    // hear from the host's computer.
    hand(&mut net, 0, 1, goal, |event| event.header().author == local);
    assert_eq!(
        reasons(&summary(&mut net.nodes[1], goal, local).guard),
        [(local, GuardReason::Admitted)]
    );
    let refused = post(&mut net, 1, 3, goal).unwrap_err();
    assert_eq!(refused.code, ErrorCode::Unavailable);
    assert!(refused.message.contains("host's computer"), "{refused}");
    assert_eq!(summary(&mut net.nodes[1], goal, local).halted, None);
    net.down.clear();
    settle(&mut net);
    assert!(summary(&mut net.nodes[1], goal, local).guard.is_empty());
    let after = post(&mut net, 1, 3, goal).unwrap();
    assert_eq!(record(&net.nodes[1], &after).header().seq, signed);
}

#[test]
fn on_a_members_computer_the_hosts_computer_or_all_the_others_end_the_hold() {
    for by_host in [true, false] {
        let mut net = Network::with(3);
        let (goal, _) = hosted(&mut net, 0, 1);
        let member = join(&mut net, 0, 1, goal, 2);
        join(&mut net, 0, 2, goal, 3);
        let copy = snapshot_all(&net.nodes[1].store);
        net.down = [0, 2].into();
        net.start_over(1, copy);
        settle(&mut net);
        let view = summary(&mut net.nodes[1], goal, member);
        assert_eq!(reasons(&view.guard), [(member, GuardReason::Unheard)]);
        assert_eq!(view.halted, Some(Halt::SignerRecovery));
        assert_eq!(
            view.guard[0].waiting,
            [Network::endpoint(0), Network::endpoint(2)]
        );
        assert_eq!(code(post(&mut net, 1, 2, goal)), ErrorCode::ReadOnly);

        // The host's computer alone, or every other computer but the host's.
        net.down = if by_host { [2].into() } else { [0].into() };
        settle(&mut net);
        let view = summary(&mut net.nodes[1], goal, member);
        assert!(view.guard.is_empty(), "{by_host}: {:?}", view.guard);
        assert_eq!(view.halted, None);
        post(&mut net, 1, 2, goal).unwrap();
    }
}

#[test]
fn with_every_member_unreachable_nothing_is_signed_until_the_person_continues() {
    for marks_kept in [true, false] {
        let mut net = Network::with(3);
        let (goal, host) = hosted(&mut net, 0, 1);
        join(&mut net, 0, 1, goal, 2);
        join(&mut net, 0, 2, goal, 3);
        let governance = governance_of(&net.nodes[0], goal);
        let copy = if marks_kept {
            snapshot(&net.nodes[0].store)
        } else {
            snapshot_all(&net.nodes[0].store)
        };
        post(&mut net, 0, 1, goal).unwrap();
        settle(&mut net);
        let signed = signed_by(&net.nodes[1], goal, host);
        net.down = [1, 2].into();
        net.start_over(0, copy);
        let records = net.nodes[0].store.log(&goal, 0, usize::MAX).unwrap().len();

        // No clock ends the hold.
        for _ in 0..24 {
            net.poll(3_600_000);
        }
        assert_eq!(code(post(&mut net, 0, 1, goal)), ErrorCode::ReadOnly);
        if !marks_kept {
            assert_eq!(code(rebind(&mut net.nodes[0], goal)), ErrorCode::ReadOnly);
        }
        assert_eq!(
            net.nodes[0].store.log(&goal, 0, usize::MAX).unwrap().len(),
            records
        );
        let view = status(&mut net.nodes[0], goal);
        let expected = if marks_kept {
            vec![(host, GuardReason::Behind { held: 0, signed })]
        } else {
            let mut keys = vec![
                (governance, GuardReason::Unheard),
                (host, GuardReason::Unheard),
            ];
            keys.sort_by_key(|(key, _)| *key);
            keys
        };
        assert_eq!(reasons(&view.guard), expected);
        // Status names the computers not heard from.
        for view in &view.guard {
            assert!(view.heard.is_empty());
            assert_eq!(view.waiting, [Network::endpoint(1), Network::endpoint(2)]);
        }

        // The person's command is the only way out.
        let released = continued(&mut net.nodes[0], goal);
        assert_eq!(released, if marks_kept { 1 } else { 2 });
        let view = status(&mut net.nodes[0], goal);
        assert!(view.guard.is_empty());
        assert_eq!(view.halted, None);
        post(&mut net, 0, 1, goal).unwrap();
    }
}

#[test]
fn a_copy_from_before_the_first_admission_catches_up_when_a_member_calls() {
    let mut net = Network::with(2);
    let (goal, host) = hosted(&mut net, 0, 1);
    let governance = governance_of(&net.nodes[0], goal);
    let copy = snapshot(&net.nodes[0].store);
    let member = join(&mut net, 0, 1, goal, 2);
    assert!(mark(&net.nodes[0], goal, governance).unwrap().shared);
    net.start_over(0, copy);

    // The copy shows nobody, and the mark says the goal was shared.
    let view = status(&mut net.nodes[0], goal);
    assert_eq!(view.guard.len(), 1);
    assert_eq!(view.guard[0].key, governance);
    assert!(matches!(view.guard[0].reason, GuardReason::Behind { .. }));
    assert!(view.guard[0].heard.is_empty() && view.guard[0].waiting.is_empty());
    let since = net.opened.len();
    for _ in 0..3 {
        net.poll_from(&[0], 31_000);
    }
    assert!(net.opened[since..].is_empty(), "nobody to dial");
    assert_eq!(code(post(&mut net, 0, 1, goal)), ErrorCode::ReadOnly);

    // The member's computer calls; the host dials it back.
    net.poll_from(&[1], 31_000);
    assert_eq!(net.opened[since..], [(1, 0)]);
    assert!(!net.nodes[0].node.goals[&goal].is_member(&member));
    net.poll_from(&[0], 1);
    assert_eq!(net.opened[since..][1], (0, 1));
    // Its own log returns and the hold ends.
    assert!(net.nodes[0].node.goals[&goal].is_member(&member));
    assert_eq!(
        signed_by(&net.nodes[0], goal, governance),
        signed_by(&net.nodes[1], goal, governance)
    );
    assert!(status(&mut net.nodes[0], goal).guard.is_empty());
    assert!(summary(&mut net.nodes[0], goal, host).guard.is_empty());
    post(&mut net, 0, 1, goal).unwrap();
    settle(&mut net);
    for daemon in &net.nodes {
        assert_eq!(daemon.node.goals[&goal].goal.fork_point(&governance), None);
    }
}

#[test]
fn an_unknown_copy_that_shows_nobody_waits_for_the_person() {
    let mut net = Network::with(2);
    let (goal, host) = hosted_at_ask(&mut net, 0, HOST);
    // Both directories of the host's computer, from before anyone joined.
    let copy = snapshot_all(&net.nodes[0].store);
    let (member, _) = join_with_ticket(&mut net, 0, 1, goal, tag(1));
    let lost = posted(&mut net, 0, HOST, goal);
    settle(&mut net);
    assert!(net.nodes[1].store.has_event(&lost.id()).unwrap());
    net.start_over(0, copy);
    let governance = governance_of(&net.nodes[0], goal);
    let signed = |net: &Network| {
        let mut ids = records_of(&net.nodes[0], goal, &governance);
        ids.extend(records_of(&net.nodes[0], goal, &host));
        ids
    };

    // The copy names no other computer, so no hearing can show it is whole.
    let held = status(&mut net.nodes[0], goal);
    assert_eq!(held.halted, Some(Halt::SignerRecovery));
    assert_eq!(held.restored, Some(0));
    let view = held
        .guard
        .iter()
        .find(|view| view.key == governance)
        .expect("the governance key is held");
    assert!(view.by_host);
    assert_eq!(view.reason, GuardReason::Unheard);
    assert!(view.waiting.is_empty());
    let error = post(&mut net, 0, HOST, goal).unwrap_err();
    assert_eq!(error.code, ErrorCode::ReadOnly);
    assert!(error.message.contains("may be an old copy"), "{error}");

    // The member's computer calls, is called back and returns every record,
    // the host's own among them. The hold lasts all the same.
    settle(&mut net);
    let entry = &net.nodes[0].node.goals[&goal];
    assert!(entry.is_member(&member));
    assert!(entry.goal.holds(&lost.id()));
    let before = signed(&net);
    settle(&mut net);
    assert_eq!(signed(&net), before, "nothing is signed while held");
    assert_eq!(code(post(&mut net, 0, HOST, goal)), ErrorCode::ReadOnly);
    let held = summary(&mut net.nodes[0], goal, host);
    assert_eq!(held.halted, Some(Halt::SignerRecovery));
    assert!(
        held.guard
            .iter()
            .any(|view| view.by_host && view.reason == GuardReason::Unheard)
    );

    // The person's word: the governance key and the host's agent.
    assert_eq!(continued(&mut net.nodes[0], goal), 2);
    let after = posted(&mut net, 0, HOST, goal);
    assert_eq!(after.header().seq, lost.header().seq + 1);
    assert_eq!(after.header().prev, Some(lost.id()));
    let released = status(&mut net.nodes[0], goal);
    assert_eq!(released.halted, None);
    assert!(released.guard.is_empty());
}

#[test]
fn a_record_no_other_computer_holds_is_given_up_for_an_agent_key_and_kept_for_the_governance_key() {
    // The host's agent's record reached no other computer.
    let mut net = Network::with(2);
    let (goal, host, _) = shared_goal(&mut net, &[1]);
    let copy = snapshot(&net.nodes[0].store);
    net.down.insert(1);
    let unseen = posted(&mut net, 0, HOST, goal);
    net.start_over(0, copy);
    let held = summary(&mut net.nodes[0], goal, host);
    assert_eq!(held.halted, Some(Halt::SignerRecovery));
    assert_eq!(held.guard.len(), 1, "{:?}", held.guard);
    assert_eq!(held.guard[0].key, host);
    assert!(!held.guard[0].by_host);
    assert_eq!(
        held.guard[0].reason,
        GuardReason::Behind {
            held: unseen.header().seq,
            signed: unseen.header().seq + 1,
        }
    );
    assert!(held.guard[0].heard.is_empty());
    assert_eq!(held.guard[0].waiting, vec![Network::endpoint(1)]);
    let error = post(&mut net, 0, HOST, goal).unwrap_err();
    assert_eq!(error.code, ErrorCode::ReadOnly);
    assert!(
        error.message.contains("older than what it signed"),
        "{error}"
    );
    // Every other computer answered and none had it: the key gives it up.
    net.down.remove(&1);
    settle(&mut net);
    let released = summary(&mut net.nodes[0], goal, host);
    assert_eq!(released.halted, None);
    assert!(released.guard.is_empty());
    let again = posted(&mut net, 0, HOST, goal);
    assert_eq!(again.header().seq, unseen.header().seq);
    assert_ne!(again.id(), unseen.id());
    settle(&mut net);
    for daemon in &net.nodes {
        let entry = &daemon.node.goals[&goal];
        assert!(entry.goal.holds(&again.id()));
        assert_eq!(entry.goal.fork_point(&host), None);
    }

    // The governance key's record reached no other computer.
    let mut net = Network::with(2);
    let (goal, host, _) = shared_goal(&mut net, &[1]);
    let governance = governance_of(&net.nodes[0], goal);
    let copy = snapshot(&net.nodes[0].store);
    net.down.insert(1);
    let owner = net.nodes[0].owner();
    let (local, _) = join_local(&mut net.nodes[0], owner, goal, 3);
    let admission = net.nodes[0].node.goals[&goal].state().members[&local].admission;
    let unseen = net.nodes[0].store.event(&admission).unwrap().unwrap();
    assert_eq!(unseen.header().author, governance);
    net.start_over(0, copy);
    net.down.remove(&1);
    settle(&mut net);
    // Every other computer answered and the hold lasts.
    let held = status(&mut net.nodes[0], goal);
    assert_eq!(held.halted, Some(Halt::SignerRecovery));
    let view = held
        .guard
        .iter()
        .find(|view| view.key == governance)
        .expect("the governance key is held");
    assert!(view.by_host);
    assert_eq!(
        view.reason,
        GuardReason::Behind {
            held: unseen.header().seq,
            signed: unseen.header().seq + 1,
        }
    );
    assert_eq!(view.heard, vec![Network::endpoint(1)]);
    assert!(view.waiting.is_empty());
    // The host's agent is held with it and has no view of its own.
    let agent = summary(&mut net.nodes[0], goal, host);
    assert_eq!(agent.halted, Some(Halt::SignerRecovery));
    assert!(agent.guard.iter().all(|view| view.key == governance));
    assert_eq!(code(post(&mut net, 0, HOST, goal)), ErrorCode::ReadOnly);
    let expected = net.nodes[0].node.goals[&goal]
        .state()
        .current_rules
        .unwrap();
    let owner = net.nodes[0].owner();
    assert_eq!(
        code(net.nodes[0].call(owner, pipeline_request(goal, expected))),
        ErrorCode::ReadOnly
    );
    settle(&mut net);
    assert!(status(&mut net.nodes[0], goal).halted.is_some());
    // Only the person gives the record up.
    assert_eq!(continued(&mut net.nodes[0], goal), 2);
    let owner = net.nodes[0].owner();
    let (again, _) = join_local(&mut net.nodes[0], owner, goal, 4);
    let admission = net.nodes[0].node.goals[&goal].state().members[&again].admission;
    let again = net.nodes[0].store.event(&admission).unwrap().unwrap();
    assert_eq!(again.header().seq, unseen.header().seq);
    posted(&mut net, 0, HOST, goal);
    settle(&mut net);
    for daemon in &net.nodes {
        let entry = &daemon.node.goals[&goal];
        assert!(entry.goal.holds(&again.id()));
        assert_eq!(entry.goal.fork_point(&governance), None);
        assert!(entry.halted().is_none());
    }
}

#[test]
fn a_forked_key_is_reported_as_a_conflict_and_not_as_catching_up() {
    let mut net = Network::with(2);
    let (goal, host, _) = shared_goal(&mut net, &[1]);
    let copy = snapshot(&net.nodes[0].store);
    let first = posted(&mut net, 0, HOST, goal);
    let second = twin(&first, net.nodes[0].node.signer(&host).unwrap());
    net.start_over(0, copy);
    assert_eq!(
        summary(&mut net.nodes[0], goal, host).halted,
        Some(Halt::SignerRecovery)
    );
    // The other record at the marked position is not the marked record.
    receive(&mut net.nodes[0], goal, &second);
    assert_eq!(
        summary(&mut net.nodes[0], goal, host).halted,
        Some(Halt::SignerRecovery)
    );
    receive(&mut net.nodes[0], goal, &first);
    assert_eq!(
        net.nodes[0].node.goals[&goal].goal.fork_point(&host),
        Some(first.header().seq)
    );
    let forked = summary(&mut net.nodes[0], goal, host);
    assert_eq!(forked.halted, Some(Halt::SignerConflict));
    assert!(forked.guard.is_empty(), "{:?}", forked.guard);
    let error = post(&mut net, 0, HOST, goal).unwrap_err();
    assert_eq!(error.code, ErrorCode::Halted);
    assert_eq!(error.message, crate::node::guard::CONFLICT);
    // The goal is not halted: its governance key still signs.
    let goal_status = status(&mut net.nodes[0], goal);
    assert_eq!(goal_status.halted, None);
    assert!(goal_status.guard.is_empty());
    assert!(net.nodes[0].node.goals[&goal].halted().is_none());
    let owner = net.nodes[0].owner();
    let (local, _) = join_local(&mut net.nodes[0], owner, goal, 3);
    assert!(net.nodes[0].node.goals[&goal].is_member(&local));

    // The governance key's fork halts the goal, as it always has.
    let admission = net.nodes[0].node.goals[&goal].state().members[&local].admission;
    let admitted = net.nodes[0].store.event(&admission).unwrap().unwrap();
    let forged = twin(&admitted, &governance_key(&net.nodes[0], goal));
    receive(&mut net.nodes[0], goal, &forged);
    let goal_status = status(&mut net.nodes[0], goal);
    assert_eq!(goal_status.halted, Some(Halt::AuthorityConflict));
    assert!(goal_status.guard.is_empty());
    assert_eq!(
        summary(&mut net.nodes[0], goal, host).halted,
        Some(Halt::AuthorityConflict)
    );
    let owner = net.nodes[0].owner();
    let member = net.nodes[0].node.goals[&goal]
        .state()
        .members
        .keys()
        .copied()
        .find(|member| *member != host && *member != local)
        .unwrap();
    assert_eq!(
        code(net.nodes[0].call(owner, Request::MemberRemove { goal, member })),
        ErrorCode::Halted
    );
}

#[test]
fn the_persons_own_command_is_held_like_any_signature() {
    let mut net = Network::with(2);
    let (goal, _, _) = shared_goal(&mut net, &[1]);
    let copy = snapshot_all(&net.nodes[0].store);
    net.start_over(0, copy);
    let daemon = &mut net.nodes[0];
    let owner = daemon.owner();
    let expected = daemon.node.goals[&goal].state().current_rules.unwrap();
    let bind = pipeline_request(goal, expected);
    let error = daemon.call(owner, bind.clone()).unwrap_err();
    assert_eq!(error.code, ErrorCode::ReadOnly);
    assert!(error.message.contains("may be an old copy"), "{error}");
    // Issuing is not held; the admission it leads to is.
    let ticket = invite(daemon, goal);
    let local = daemon.enroll("local", 3);
    let error = daemon
        .call(owner, join_request(local, ticket.clone()))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ReadOnly);
    assert!(error.message.contains("may be an old copy"), "{error}");
    assert!(!daemon.node.goals[&goal].is_member(&local));
    assert!(
        invitations(daemon, goal)
            .iter()
            .any(|invitation| invitation.state == InvitationState::Pending)
    );
    assert_eq!(continued(daemon, goal), 2);
    let owner = daemon.owner();
    daemon.ok(owner, bind);
    assert!(matches!(
        daemon.ok(owner, join_request(local, ticket)),
        Response::Joined {
            membership: Membership::Member,
            ..
        }
    ));
}

#[test]
fn continuing_is_the_owners_alone() {
    let mut net = Network::with(2);
    let (goal, host, _) = shared_goal(&mut net, &[1]);
    let copy = snapshot_all(&net.nodes[0].store);
    net.start_over(0, copy);
    let daemon = &mut net.nodes[0];
    for session in [None, Some(session(HOST))] {
        let agent = daemon.connect(credential(HOST), session);
        let error = daemon
            .call(agent, Request::GoalContinue { goal })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Denied);
    }
    // The owner cannot lend the command to an agent either.
    let owner = daemon.owner();
    assert_eq!(
        code(daemon.on_behalf(owner, host, Request::GoalContinue { goal })),
        ErrorCode::Invalid
    );
    assert_eq!(
        summary(daemon, goal, host).halted,
        Some(Halt::SignerRecovery)
    );
    assert_eq!(continued(daemon, goal), 2);
    assert_eq!(summary(daemon, goal, host).halted, None);
}

// Callers and the transport, frame by frame: what a held daemon sends to a
// computer that calls it, and whom it dials back.

/// Callers daemon 0 dials about `goal` that are bound to no current member.
fn dialed_callers(net: &Network, goal: GoalId) -> BTreeSet<EndpointId> {
    let entry = &net.nodes[0].node.goals[&goal];
    Host::peers(&net.nodes[0].node)
        .into_iter()
        .filter(|(about, endpoint)| {
            *about == goal
                && !entry
                    .state()
                    .members
                    .values()
                    .any(|member| member.is_active() && member.endpoint == *endpoint)
        })
        .map(|(_, endpoint)| endpoint)
        .collect()
}

/// An endpoint on no computer of the network calls daemon 0 about `goal`:
/// its `Hello` is read, then the stream closes.
fn hello_from(net: &mut Network, number: u64, remote: EndpointId, goal: GoalId) {
    let exchange = ExchangeId::Accepted(1 << 40 | number);
    let time = PeerTime {
        unix_ms: net.now,
        elapsed_ms: net.now,
    };
    let node = &mut net.nodes[0].node;
    for input in [
        PeerInput::Accepted { exchange, remote },
        PeerInput::Frame {
            exchange,
            frame: SyncMessage::Hello {
                version: locust_proto::PROTOCOL_VERSION,
                goal,
            },
        },
        PeerInput::Closed(exchange),
    ] {
        node.peer(input, time, &mut Vec::new());
    }
}

/// The endpoints daemon 0 dials when polled now. None of them answers.
fn opens(net: &mut Network) -> BTreeSet<EndpointId> {
    let time = PeerTime {
        unix_ms: net.now,
        elapsed_ms: net.now,
    };
    let node = &mut net.nodes[0].node;
    let mut out = Vec::new();
    node.peer(PeerInput::Poll, time, &mut out);
    let mut opened = BTreeSet::new();
    for output in out {
        if let PeerOutput::Open {
            exchange, endpoint, ..
        } = output
        {
            opened.insert(endpoint);
            node.peer(PeerInput::OpenFailed(exchange), time, &mut Vec::new());
        }
    }
    opened
}

/// One frame a daemon sent, and whether the sender's own records named the
/// receiver's computer as a current member's when it sent it.
struct Sent {
    from: usize,
    to: usize,
    frame: SyncMessage,
    named: bool,
}

/// The transport of `Network`, logging every frame. Used only while the
/// network's own transport is idle, so the two never share an exchange.
#[derive(Default)]
struct Logged {
    routes: BTreeMap<(usize, ExchangeId), (usize, ExchangeId)>,
    queue: VecDeque<(usize, PeerInput)>,
    accepted: u64,
    sent: Vec<Sent>,
}

impl Logged {
    /// Polls `dialers` once after `elapsed` and runs every exchange that
    /// opens to its end.
    fn poll(net: &mut Network, goal: GoalId, dialers: &[usize], elapsed: u64) -> Vec<Sent> {
        let mut logged = Self {
            accepted: 1 << 41,
            ..Self::default()
        };
        net.now += elapsed;
        for &i in dialers {
            logged.input(net, goal, i, PeerInput::Poll);
        }
        let mut steps = 0;
        while let Some((i, input)) = logged.queue.pop_front() {
            steps += 1;
            assert!(steps < 100_000, "test transport failed to settle");
            if let PeerInput::Frame { exchange, .. } = &input {
                if !logged.routes.contains_key(&(i, *exchange)) {
                    continue;
                }
                if !net.nodes[i].node.peer_readable(*exchange) {
                    logged.queue.push_back((i, input));
                    continue;
                }
            }
            logged.input(net, goal, i, input);
        }
        logged.sent
    }

    fn input(&mut self, net: &mut Network, goal: GoalId, i: usize, input: PeerInput) {
        let mut out = vec![];
        let time = PeerTime {
            unix_ms: net.now,
            elapsed_ms: net.now,
        };
        net.nodes[i].node.peer(input, time, &mut out);
        for output in out {
            match output {
                PeerOutput::Open {
                    exchange, endpoint, ..
                } => {
                    let Some(j) = (0..net.nodes.len())
                        .find(|j| Network::endpoint(*j) == endpoint && !net.down.contains(j))
                    else {
                        self.queue.push_back((i, PeerInput::OpenFailed(exchange)));
                        continue;
                    };
                    net.opened.push((i, j));
                    self.accepted += 1;
                    let accepted = ExchangeId::Accepted(self.accepted);
                    self.routes.insert((i, exchange), (j, accepted));
                    self.routes.insert((j, accepted), (i, exchange));
                    self.queue.push_back((
                        j,
                        PeerInput::Accepted {
                            exchange: accepted,
                            remote: Network::endpoint(i),
                        },
                    ));
                    self.queue.push_back((i, PeerInput::Opened(exchange)));
                }
                PeerOutput::Send { exchange, frame } => {
                    let Some(&(j, other)) = self.routes.get(&(i, exchange)) else {
                        continue;
                    };
                    let named = net.nodes[i].node.goals.get(&goal).is_some_and(|entry| {
                        entry.state().members.values().any(|member| {
                            member.is_active() && member.endpoint == Network::endpoint(j)
                        })
                    });
                    let copy = |frame: &SyncMessage| {
                        SyncMessage::decode(&locust_proto::codec::encode(frame).unwrap()).unwrap()
                    };
                    self.sent.push(Sent {
                        from: i,
                        to: j,
                        frame: copy(&frame),
                        named,
                    });
                    self.queue.push_back((
                        j,
                        PeerInput::Frame {
                            exchange: other,
                            frame: copy(&frame),
                        },
                    ));
                    self.queue.push_back((i, PeerInput::Writable(exchange)));
                }
                PeerOutput::Finish(exchange) => {
                    self.queue.push_back((i, PeerInput::Finished(exchange)))
                }
                PeerOutput::Admit(_) | PeerOutput::Evidence(_) => {}
            }
        }
    }
}

#[test]
fn a_caller_is_sent_nothing_until_this_daemons_own_records_name_it() {
    let mut net = Network::with(3);
    let (goal, _, _) = shared_goal(&mut net, &[1]);
    let copy = snapshot(&net.nodes[0].store);
    // Admitted after the copy while the first member's computer was away:
    // only the host's computer and the caller's hold the admission.
    net.down.insert(1);
    let (caller, _) = join_with_ticket(&mut net, 0, 2, goal, tag(2));
    net.down.insert(2);
    net.down.remove(&1);
    net.start_over(0, copy);
    assert!(!net.nodes[0].node.goals[&goal].is_member(&caller));
    // A record the caller lacks reaches the host's computer.
    let unknown = posted(&mut net, 1, tag(1), goal);
    settle(&mut net);
    assert!(net.nodes[0].store.has_event(&unknown.id()).unwrap());
    assert!(
        net.nodes[0]
            .node
            .admission_hold(&net.nodes[0].node.goals[&goal])
            .is_some()
    );
    assert!(dialed_callers(&net, goal).is_empty());

    // The caller's own exchanges are refused, and remembered.
    net.down.remove(&2);
    net.poll_from(&[2], 1);
    assert_eq!(
        dialed_callers(&net, goal),
        BTreeSet::from([Network::endpoint(2)])
    );
    assert!(!net.nodes[2].store.has_event(&unknown.id()).unwrap());

    // Called back: until the caller's records name it here, the host sends
    // its hello and an empty frontier and nothing else.
    let sent = Logged::poll(&mut net, goal, &[0], 1);
    let to_caller: Vec<_> = sent
        .iter()
        .filter(|sent| sent.from == 0 && sent.to == 2)
        .collect();
    let before: Vec<_> = to_caller.iter().filter(|sent| !sent.named).collect();
    assert!(matches!(before[0].frame, SyncMessage::Hello { .. }));
    assert!(
        matches!(&before[1].frame, SyncMessage::Frontier(frontier) if frontier.authors.is_empty())
    );
    for sent in &before {
        assert!(
            matches!(
                &sent.frame,
                SyncMessage::Hello { .. } | SyncMessage::Frontier(_)
            ),
            "{:?}",
            sent.frame
        );
        if let SyncMessage::Frontier(frontier) = &sent.frame {
            assert!(frontier.authors.is_empty());
        }
    }
    assert!(to_caller.iter().any(|sent| sent.named));
    // Its records returned the host's own, and the hold ended.
    let entry = &net.nodes[0].node.goals[&goal];
    assert!(entry.is_member(&caller));
    assert!(net.nodes[0].node.admission_hold(entry).is_none());
    assert_eq!(net.nodes[0].node.guard.callers(&goal).count(), 0);
    // Named, it is sent what it lacks like any member.
    settle(&mut net);
    assert!(net.nodes[2].store.has_event(&unknown.id()).unwrap());
}

#[test]
fn callers_are_bounded_and_forgotten_when_the_hold_ends() {
    let mut net = Network::with(2);
    let (goal, _, _) = shared_goal(&mut net, &[1]);
    let copy = snapshot_all(&net.nodes[0].store);
    net.start_over(0, copy);
    let stranger = |k: u8| EndpointId([200 + k; 32]);
    for k in 0..10 {
        hello_from(&mut net, u64::from(k), stranger(k), goal);
    }
    // A member's computer and this daemon's own are no callers.
    hello_from(&mut net, 20, Network::endpoint(1), goal);
    hello_from(&mut net, 21, Network::endpoint(0), goal);
    let newest: BTreeSet<_> = (2..10).map(stranger).collect();
    assert_eq!(dialed_callers(&net, goal), newest);
    assert_eq!(net.nodes[0].node.guard.callers(&goal).count(), 8);
    // Calling again makes a caller the newest.
    hello_from(&mut net, 22, stranger(2), goal);
    hello_from(&mut net, 23, stranger(10), goal);
    let newest: BTreeSet<_> = [2, 4, 5, 6, 7, 8, 9, 10]
        .into_iter()
        .map(stranger)
        .collect();
    assert_eq!(dialed_callers(&net, goal), newest);
    // Each is dialed while the hold lasts.
    let opened = opens(&mut net);
    assert!(newest.is_subset(&opened), "{opened:?}");

    continued(&mut net.nodes[0], goal);
    assert!(dialed_callers(&net, goal).is_empty());
    assert_eq!(net.nodes[0].node.guard.callers(&goal).count(), 0);
    // A caller of a goal that is not held is not remembered.
    hello_from(&mut net, 24, stranger(11), goal);
    assert_eq!(net.nodes[0].node.guard.callers(&goal).count(), 0);
}

#[test]
fn a_held_join_is_asked_again_and_not_refused_for_good() {
    let mut net = Network::with(2);
    let (goal, _) = hosted_at_ask(&mut net, 0, HOST);
    let copy = snapshot_all(&net.nodes[0].store);
    net.start_over(0, copy);
    let ticket = invite(&mut net.nodes[0], goal);
    let joiner = net.nodes[1].enroll("member", tag(1));
    let owner = net.nodes[1].owner();
    net.nodes[1].ok(owner, join_request(joiner, ticket));
    let asked = |net: &Network| net.opened.iter().filter(|pair| **pair == (1, 0)).count();
    net.poll(1);
    let first = asked(&net);
    assert!(first >= 1);
    for _ in 0..3 {
        net.poll(61_000);
    }
    assert!(asked(&net) > first, "the joiner's computer asks again");
    let joining = summary(&mut net.nodes[1], goal, joiner);
    assert_eq!(joining.membership, Membership::Joining);
    assert!(!net.nodes[1].node.goals[&goal].local.joins[&joiner].refused);
    assert!(!net.nodes[0].node.goals[&goal].is_member(&joiner));
    // Nothing was used up on the host's computer.
    assert!(
        invitations(&mut net.nodes[0], goal)
            .iter()
            .all(|invitation| invitation.state == InvitationState::Pending)
    );
    continued(&mut net.nodes[0], goal);
    for _ in 0..2 {
        net.poll(61_000);
    }
    assert!(net.nodes[0].node.goals[&goal].is_member(&joiner));
    assert_eq!(
        summary(&mut net.nodes[1], goal, joiner).membership,
        Membership::Member
    );
}

#[test]
fn a_join_already_admitted_is_answered_while_the_host_is_held() {
    let mut net = Network::with(2);
    let (goal, _, members) = shared_goal(&mut net, &[1]);
    let (member, ticket) = members[&1].clone();
    let copy = snapshot_all(&net.nodes[0].store);
    net.start_over(0, copy);
    assert!(
        net.nodes[0]
            .node
            .admission_hold(&net.nodes[0].node.goals[&goal])
            .is_some()
    );
    let invitation = Invitation::from_ticket(ticket.as_str()).unwrap();
    let retry = JoinRequest::sign(
        goal,
        Network::endpoint(1),
        "member".into(),
        invitation.secret,
        net.nodes[1].node.signer(&member).unwrap(),
    );
    let before = net.nodes[0].store.log(&goal, 0, usize::MAX).unwrap();
    assert_eq!(
        Host::join(
            &mut net.nodes[0].node,
            &Network::endpoint(1),
            &retry,
            net.now
        ),
        Ok(())
    );
    assert_eq!(
        net.nodes[0].store.log(&goal, 0, usize::MAX).unwrap(),
        before
    );
    // A join that would be signed waits.
    let fresh = Invitation::from_ticket(invite(&mut net.nodes[0], goal).as_str()).unwrap();
    let newcomer = net.nodes[1].enroll("newcomer", 3);
    let request = JoinRequest::sign(
        goal,
        Network::endpoint(1),
        "newcomer".into(),
        fresh.secret,
        net.nodes[1].node.signer(&newcomer).unwrap(),
    );
    assert_eq!(
        Host::join(
            &mut net.nodes[0].node,
            &Network::endpoint(1),
            &request,
            net.now
        ),
        Err(Refusal::CatchingUp)
    );
    assert_eq!(
        net.nodes[0].store.log(&goal, 0, usize::MAX).unwrap(),
        before
    );
}

#[test]
fn a_restore_revokes_pending_invitations_once_and_only_where_this_daemon_hosts() {
    let mut net = Network::with(2);
    let (hosted, _, _) = shared_goal(&mut net, &[1]);
    let (elsewhere, _) = hosted_at_ask(&mut net, 1, 2);
    let (away, _) = join_with_ticket(&mut net, 1, 0, elsewhere, tag(0));
    let pending = [
        invite(&mut net.nodes[0], hosted),
        invite(&mut net.nodes[0], hosted),
    ];
    invite(&mut net.nodes[1], elsewhere);
    let copy = snapshot(&net.nodes[0].store);
    // A governance record the copy lacks reaches the member's computer.
    let owner = net.nodes[0].owner();
    join_local(&mut net.nodes[0], owner, hosted, 5);
    settle(&mut net);
    net.down.insert(1);
    net.start_over(0, copy);
    assert!(
        net.nodes[0]
            .node
            .admission_hold(&net.nodes[0].node.goals[&hosted])
            .is_some()
    );
    assert_eq!(status(&mut net.nodes[0], hosted).restored, Some(2));
    assert_eq!(status(&mut net.nodes[0], elsewhere).restored, Some(0));
    assert_eq!(
        summary(&mut net.nodes[0], elsewhere, away).restored,
        Some(0)
    );
    let states = |daemon: &mut Daemon, state| {
        invitations(daemon, hosted)
            .iter()
            .filter(|invitation| invitation.state == state)
            .count()
    };
    assert_eq!(states(&mut net.nodes[0], InvitationState::Revoked), 2);
    assert_eq!(states(&mut net.nodes[0], InvitationState::Pending), 0);
    let late = net.nodes[0].enroll("late", 6);
    let owner = net.nodes[0].owner();
    for ticket in &pending {
        assert_eq!(
            code(net.nodes[0].call(owner, join_request(late, ticket.clone()))),
            ErrorCode::Denied
        );
    }
    // Issued since, and kept by a restart while still behind.
    let since = invite(&mut net.nodes[0], hosted);
    net.start_over(0, net.nodes[0].store.reopen());
    assert!(
        net.nodes[0]
            .node
            .admission_hold(&net.nodes[0].node.goals[&hosted])
            .is_some()
    );
    assert_eq!(status(&mut net.nodes[0], hosted).restored, Some(2));
    assert_eq!(states(&mut net.nodes[0], InvitationState::Revoked), 2);
    assert_eq!(states(&mut net.nodes[0], InvitationState::Pending), 1);
    // The host of the other goal was not restored: its invitation stands.
    assert_eq!(
        invitations(&mut net.nodes[1], elsewhere)
            .iter()
            .filter(|invitation| invitation.state == InvitationState::Pending)
            .count(),
        1
    );
    // Caught up, the invitation issued since admits, and an ordinary start
    // forgets the restore.
    net.down.remove(&1);
    settle(&mut net);
    assert!(
        net.nodes[0]
            .node
            .admission_hold(&net.nodes[0].node.goals[&hosted])
            .is_none()
    );
    let owner = net.nodes[0].owner();
    assert!(matches!(
        net.nodes[0].ok(owner, join_request(late, since)),
        Response::Joined {
            membership: Membership::Member,
            ..
        }
    ));
    net.start_over(0, net.nodes[0].store.reopen());
    assert_eq!(status(&mut net.nodes[0], hosted).restored, None);
    assert_eq!(status(&mut net.nodes[0], elsewhere).restored, None);
}

#[test]
fn a_record_signed_after_continuing_is_judged_like_any_other() {
    // The host's agent signs again at a position its lost record used.
    let mut net = Network::with(2);
    let (goal, host, _) = shared_goal(&mut net, &[1]);
    let governance = governance_of(&net.nodes[0], goal);
    let copy = snapshot(&net.nodes[0].store);
    let lost = posted(&mut net, 0, HOST, goal);
    settle(&mut net);
    assert!(net.nodes[1].store.has_event(&lost.id()).unwrap());
    net.down.insert(1);
    net.start_over(0, copy);
    assert_eq!(code(post(&mut net, 0, HOST, goal)), ErrorCode::ReadOnly);
    assert_eq!(continued(&mut net.nodes[0], goal), 1);
    let reused = posted(&mut net, 0, HOST, goal);
    assert_eq!(reused.header().seq, lost.header().seq);
    assert_eq!(reused.header().prev, lost.header().prev);
    net.down.remove(&1);
    settle(&mut net);
    for daemon in &net.nodes {
        let folded = &daemon.node.goals[&goal].goal;
        assert_eq!(folded.fork_point(&host), Some(lost.header().seq));
        assert_eq!(folded.fork_point(&governance), None);
        assert!(folded.evaluation().host_halt.is_none());
        for id in [lost.id(), reused.id()] {
            assert!(folded.standing(&id).unwrap().is_pending(), "{id}");
        }
    }
    assert_eq!(
        summary(&mut net.nodes[0], goal, host).halted,
        Some(Halt::SignerConflict)
    );

    // The governance key signs again at a position its lost record used.
    let mut net = Network::with(2);
    let (goal, _, _) = shared_goal(&mut net, &[1]);
    let governance = governance_of(&net.nodes[0], goal);
    let copy = snapshot(&net.nodes[0].store);
    let owner = net.nodes[0].owner();
    let (first, _) = join_local(&mut net.nodes[0], owner, goal, 3);
    let admission = net.nodes[0].node.goals[&goal].state().members[&first].admission;
    let lost = net.nodes[0].store.event(&admission).unwrap().unwrap();
    settle(&mut net);
    assert!(net.nodes[1].store.has_event(&lost.id()).unwrap());
    net.down.insert(1);
    net.start_over(0, copy);
    assert_eq!(continued(&mut net.nodes[0], goal), 2);
    let owner = net.nodes[0].owner();
    let (second, _) = join_local(&mut net.nodes[0], owner, goal, 4);
    let admission = net.nodes[0].node.goals[&goal].state().members[&second].admission;
    let reused = net.nodes[0].store.event(&admission).unwrap().unwrap();
    assert_eq!(reused.header().seq, lost.header().seq);
    net.down.remove(&1);
    settle(&mut net);
    for daemon in &mut net.nodes {
        let entry = &daemon.node.goals[&goal];
        assert_eq!(entry.goal.fork_point(&governance), Some(lost.header().seq));
        assert!(entry.goal.evaluation().host_halt.is_some());
        assert!(!entry.is_member(&first) && !entry.is_member(&second));
        assert_eq!(status(daemon, goal).halted, Some(Halt::AuthorityConflict));
    }
}

#[test]
fn a_second_restore_before_the_first_is_caught_up_is_still_unheard() {
    let mut net = Network::with(3);
    let (goal, host) = hosted(&mut net, 0, 1);
    join(&mut net, 0, 1, goal, 2);
    join(&mut net, 0, 2, goal, 3);
    let governance = governance_of(&net.nodes[0], goal);
    // Both of this computer's keys sign before the backup, so each has a
    // mark to carry the bit.
    posted(&mut net, 0, HOST, goal);
    let backup = snapshot_all(&net.nodes[0].store);
    let signed = |net: &Network| {
        signed_by(&net.nodes[0], goal, governance) + signed_by(&net.nodes[0], goal, host)
    };

    // The whole computer goes back to the backup: a copy of unknown age.
    // The start writes the RESTORED record into the copy and the marks
    // again from it, each mark saying the goal is unheard.
    net.start_over(0, snapshot_all(&backup));
    let count = signed(&net);
    settle(&mut net);
    let view = status(&mut net.nodes[0], goal);
    assert_eq!(unheard(&view.guard), BTreeSet::from([governance, host]));
    assert!(view.guard[0].waiting.is_empty(), "both computers answered");
    for key in [governance, host] {
        assert!(mark(&net.nodes[0], goal, key).unwrap().unheard);
    }
    assert_eq!(signed(&net), count, "nothing is signed while unheard");
    assert_eq!(code(post(&mut net, 0, HOST, goal)), ErrorCode::ReadOnly);

    // Before the owner continued, the data directory goes back to the same
    // backup with the marks kept. The backup holds no RESTORED record; only
    // the marks remember that the copy's age is unknown, and the goal is
    // unheard again.
    let store = snapshot_all(&backup).with_marks(net.nodes[0].store.marks_handle());
    net.start_over(0, store);
    settle(&mut net);
    let view = status(&mut net.nodes[0], goal);
    assert_eq!(unheard(&view.guard), BTreeSet::from([governance, host]));
    assert_eq!(view.restored, Some(0));
    assert_eq!(signed(&net), count, "nothing is signed while unheard");
    assert_eq!(code(post(&mut net, 0, HOST, goal)), ErrorCode::ReadOnly);

    // The owner's word releases the goal, and the marks say so.
    assert_eq!(continued(&mut net.nodes[0], goal), 2);
    let view = status(&mut net.nodes[0], goal);
    assert!(view.guard.is_empty());
    for key in [governance, host] {
        assert!(!mark(&net.nodes[0], goal, key).unwrap().unheard);
    }
    post(&mut net, 0, HOST, goal).unwrap();
}

#[test]
fn records_that_return_raise_the_mark() {
    let mut net = Network::with(2);
    let (goal, _) = hosted(&mut net, 0, 1);
    let member = join(&mut net, 0, 1, goal, 2);
    posted(&mut net, 1, 2, goal);
    settle(&mut net);
    let backup = snapshot_all(&net.nodes[1].store);
    let tip_at_backup = records_of(&net.nodes[1], goal, &member)
        .last()
        .copied()
        .unwrap();
    // The member's agent signs again, and the record reaches the host's
    // computer.
    let own = posted(&mut net, 1, 2, goal);
    settle(&mut net);
    assert!(net.nodes[0].store.has_event(&own.id()).unwrap());

    // The whole computer goes back to the backup: a copy of unknown age,
    // whose marks are written again from the copy's older tip.
    net.start_over(1, snapshot_all(&backup));
    let rewritten = mark(&net.nodes[1], goal, member).unwrap();
    assert!(rewritten.unheard);
    assert_eq!(rewritten.point.id, tip_at_backup);
    assert_eq!(
        reasons(&summary(&mut net.nodes[1], goal, member).guard),
        [(member, GuardReason::Unheard)]
    );

    // The daemon's own records return from the host's computer and the mark
    // rises to their tip. Hearing from the host's computer ends the hold,
    // and the bit is rewritten false.
    settle(&mut net);
    let tip = records_of(&net.nodes[1], goal, &member)
        .last()
        .copied()
        .unwrap();
    let held_seq = record(&net.nodes[1], &tip_at_backup).header().seq;
    let tip_seq = record(&net.nodes[1], &tip).header().seq;
    assert!(tip_seq >= own.header().seq);
    let raised = mark(&net.nodes[1], goal, member).unwrap();
    assert_eq!(raised.point.id, tip);
    assert!(!raised.unheard);
    assert!(summary(&mut net.nodes[1], goal, member).guard.is_empty());

    // The data directory alone goes back to the same backup, the marks
    // kept: the raised mark is ahead of the copy, so the key is behind
    // until the marked record returns again.
    let store = snapshot_all(&backup).with_marks(net.nodes[1].store.marks_handle());
    net.start_over(1, store);
    let [view] = summary(&mut net.nodes[1], goal, member)
        .guard
        .try_into()
        .expect("one hold");
    assert_eq!(view.key, member);
    assert_eq!(
        view.reason,
        GuardReason::Behind {
            held: held_seq + 1,
            signed: tip_seq + 1,
        }
    );
    assert_eq!(status(&mut net.nodes[1], goal).restored, Some(0));
    settle(&mut net);
    assert!(summary(&mut net.nodes[1], goal, member).guard.is_empty());
    post(&mut net, 1, 2, goal).unwrap();
}
