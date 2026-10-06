//! Real Node + Driver delivery across encoded transport frames, with a lost receipt.
use super::authorization::{governance_key, join_local};
use super::*;
use locust_proto::engine::{ExchangeId, PeerEngine, PeerInput, PeerOutput, PeerTime};
use locust_proto::id::{EffectId, EndpointId, GoalId};
use locust_proto::store::{Space, Store};
use locust_proto::sync::SyncMessage;
use std::collections::{BTreeMap, VecDeque};

type Side = (usize, ExchangeId);
pub(super) struct Network {
    pub(super) nodes: Vec<Daemon>,
    routes: BTreeMap<Side, Side>,
    queue: VecDeque<(usize, PeerInput)>,
    now: u64,
    accepted: u64,
    drop_receipt: bool,
    receipts_dropped: usize,
    sent: Vec<(usize, EffectId, PublicKey)>,
}
impl Network {
    pub(super) fn new() -> Self {
        let mut net = Self {
            nodes: vec![Daemon::new(71), Daemon::new(72)],
            routes: BTreeMap::new(),
            queue: VecDeque::new(),
            now: 1000,
            accepted: 0,
            drop_receipt: false,
            receipts_dropped: 0,
            sent: vec![],
        };
        for i in 0..2 {
            net.input(
                i,
                PeerInput::Endpoint {
                    endpoint: Self::endpoint(i),
                    hints: vec![],
                },
            );
        }
        net
    }
    pub(super) fn endpoint(i: usize) -> EndpointId {
        EndpointId([71 + i as u8; 32])
    }
    fn input(&mut self, i: usize, input: PeerInput) {
        let mut out = vec![];
        self.nodes[i].node.peer(
            input,
            PeerTime {
                unix_ms: self.now,
                elapsed_ms: self.now,
            },
            &mut out,
        );
        for output in out {
            match output {
                PeerOutput::Open {
                    exchange, endpoint, ..
                } => {
                    let j = (0..2).find(|j| Self::endpoint(*j) == endpoint).unwrap();
                    self.accepted += 1;
                    let accepted = ExchangeId::Accepted(self.accepted);
                    self.routes.insert((i, exchange), (j, accepted));
                    self.routes.insert((j, accepted), (i, exchange));
                    self.queue.push_back((
                        j,
                        PeerInput::Accepted {
                            exchange: accepted,
                            remote: Self::endpoint(i),
                        },
                    ));
                    self.queue.push_back((i, PeerInput::Opened(exchange)));
                }
                PeerOutput::Send { exchange, frame } => {
                    let Some(&(j, other)) = self.routes.get(&(i, exchange)) else {
                        continue;
                    };
                    if let SyncMessage::DeliverEffect { effect, recipient } = frame {
                        self.sent.push((i, effect, recipient));
                    }
                    if self.drop_receipt
                        && matches!(frame, SyncMessage::EffectReceipt { received: true, .. })
                    {
                        self.receipts_dropped += 1;
                        self.routes.remove(&(i, exchange));
                        self.routes.remove(&(j, other));
                        self.queue.push_back((i, PeerInput::Closed(exchange)));
                        self.queue.push_back((j, PeerInput::Closed(other)));
                        continue;
                    }
                    let frame =
                        SyncMessage::decode(&locust_proto::codec::encode(&frame).unwrap()).unwrap();
                    self.queue.push_back((
                        j,
                        PeerInput::Frame {
                            exchange: other,
                            frame,
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
    pub(super) fn poll(&mut self, elapsed: u64) {
        self.now += elapsed;
        for i in 0..2 {
            self.input(i, PeerInput::Poll);
        }
        let mut steps = 0;
        while let Some((i, input)) = self.queue.pop_front() {
            steps += 1;
            assert!(steps < 100_000, "test transport failed to settle");
            if let PeerInput::Frame { exchange, .. } = &input {
                if !self.routes.contains_key(&(i, *exchange)) {
                    continue;
                }
                if !self.nodes[i].node.peer_readable(*exchange) {
                    self.queue.push_back((i, input));
                    continue;
                }
            }
            self.input(i, input);
        }
    }
    pub(super) fn restart(&mut self) {
        self.routes.clear();
        self.queue.clear();
        for node in &mut self.nodes {
            node.restart();
        }
    }
}

fn unbound_network() -> (Network, GoalId, PublicKey, PublicKey) {
    let mut net = Network::new();
    let source = net.nodes[0].enroll("source", 1);
    let target = net.nodes[1].enroll("target", 2);
    let owner = net.nodes[0].owner();
    let Response::GoalCreated { goal } = net.nodes[0].ok(
        owner,
        Request::GoalCreate {
            agent: source,
            title: "Durable delivery".into(),
            formation_json: None,
            roles: BTreeMap::new(),
            inputs: BTreeMap::new(),
        },
    ) else {
        panic!()
    };
    net.nodes[0].ok(
        owner,
        Request::LevelSet {
            goal,
            agent: source,
            level: locust_proto::api::Level::Read,
        },
    );
    let Response::Invited { ticket } = net.nodes[0].ok(
        owner,
        Request::GoalInvite {
            goal,
            expires_ms: 604_801_000,
        },
    ) else {
        panic!()
    };
    let joining_owner = net.nodes[1].owner();
    net.nodes[1].ok(
        joining_owner,
        Request::GoalJoin {
            agent: target,
            ticket,
            level: locust_proto::api::Level::Auto,
        },
    );
    net.poll(1);
    assert!(net.nodes[0].node.goals[&goal].is_member(&target));
    (net, goal, source, target)
}

pub(super) fn pipeline_request(goal: GoalId, expected: locust_proto::id::EventId) -> Request {
    let mut formation = locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == "pipeline")
        .unwrap()
        .formation;
    formation.flow.remove("ship");
    formation.flow.get_mut("draft").unwrap().task_type = None;
    formation.task_types.clear();
    Request::RulesBind {
        goal,
        expected,
        formation_json: serde_json::to_string(&formation).unwrap(),
        roles: BTreeMap::new(),
        inputs: BTreeMap::new(),
    }
}

fn bind_stages(net: &mut Network, goal: GoalId) {
    let expected = net.nodes[0].node.goals[&goal]
        .state()
        .current_rules
        .unwrap();
    let owner = net.nodes[0].owner();
    net.nodes[0].ok(owner, pipeline_request(goal, expected));
}

fn ready_network() -> (Network, GoalId, PublicKey, PublicKey) {
    let (mut net, goal, source, target) = unbound_network();
    bind_stages(&mut net, goal);
    (net, goal, source, target)
}

#[test]
fn lost_receipt_and_both_restarts_retry_one_durable_inbox_without_acknowledging_work() {
    let (mut net, goal, source, target) = ready_network();
    let effect = *net.nodes[0].node.goals[&goal]
        .state()
        .effects
        .keys()
        .next()
        .unwrap();
    let source_entry = &net.nodes[0].node.goals[&goal];
    assert!(source_entry.deliveries[&(effect, source)].received);
    assert!(!source_entry.deliveries[&(effect, target)].delivered);
    let events = net.nodes[0].store.log(&goal, 0, 1000).unwrap().len();
    net.drop_receipt = true;
    // Receiver's reverse exchange fetches the missing definition, then the next
    // sender exchange receives a durable receipt which the transport loses.
    for _ in 0..3 {
        net.poll(31_000);
    }
    assert!(net.receipts_dropped > 0);
    assert!(net.nodes[1].node.goals[&goal].deliveries[&(effect, target)].received);
    assert!(!net.nodes[0].node.goals[&goal].deliveries[&(effect, target)].delivered);
    net.restart();
    net.drop_receipt = false;
    for _ in 0..2 {
        net.poll(31_000);
    }
    assert!(net.nodes[0].node.goals[&goal].deliveries[&(effect, target)].delivered);
    assert_eq!(
        net.nodes[0].store.log(&goal, 0, 1000).unwrap().len(),
        events
    );
    assert!(
        net.sent
            .iter()
            .filter(|(i, id, principal)| *i == 0 && *id == effect && *principal == target)
            .count()
            >= 2
    );
    let b = net.nodes[1].connect(credential(2), None);
    let Response::Pending(pending) = net.nodes[1].ok(b, Request::Pending { goal }) else {
        panic!()
    };
    assert_eq!(pending.deliveries.len(), 1);
    assert!(pending.deliveries[0].received && pending.deliveries[0].available);
    assert!(!pending.deliveries[0].acknowledged);
    assert!(pending.claimed.is_empty());
    assert_eq!(
        net.nodes[1].store.scan(Space::Pending, &[]).unwrap().len(),
        2
    );
}

#[test]
fn fork_retracts_undelivered_work_and_preserves_received_inbox_status_after_restart() {
    use crate::sync::Host;
    let (mut net, goal, source, target) = ready_network();
    for _ in 0..2 {
        net.poll(31_000);
    }
    let effect = *net.nodes[0].node.goals[&goal]
        .state()
        .effects
        .keys()
        .next()
        .unwrap();
    let materialization = *net.nodes[0].node.goals[&goal].state().effects[&effect]
        .events
        .first()
        .unwrap();
    let mut header = net.nodes[0].node.goals[&goal]
        .goal
        .event(&materialization)
        .unwrap()
        .header()
        .clone();
    header.at_ms += 1;
    // A stage step is the governance key's record; forging its twin forks the
    // governance log, not the host's agent's.
    assert_eq!(
        Some(header.author),
        net.nodes[0].node.goals[&goal].state().governance
    );
    assert_ne!(header.author, source);
    let fork =
        locust_proto::event::Event::sign(header, &governance_key(&net.nodes[0], goal)).unwrap();
    // The actual replica ingestion path reprojects status in the event commit.
    for daemon in &mut net.nodes {
        daemon
            .node
            .replica(&goal)
            .unwrap()
            .receive(vec![fork.to_wire()])
            .unwrap();
    }
    assert!(
        net.nodes[0]
            .node
            .replica(&goal)
            .unwrap()
            .next_delivery(Network::endpoint(1), None)
            .is_none()
    );
    assert!(
        !net.nodes[1]
            .node
            .replica(&goal)
            .unwrap()
            .receive_delivery(effect, target)
            .unwrap()
    );
    net.restart();
    let b = net.nodes[1].connect(credential(2), None);
    let Response::Pending(pending) = net.nodes[1].ok(b, Request::Pending { goal }) else {
        panic!()
    };
    assert_eq!(pending.deliveries.len(), 1);
    assert!(pending.deliveries[0].received);
    assert!(!pending.deliveries[0].available);
    assert!(!pending.deliveries[0].acknowledged);
}

#[test]
fn foreign_endpoint_or_recipient_cannot_supply_a_delivery_receipt() {
    use crate::sync::Host;
    let (mut net, goal, _, target) = ready_network();
    let effect = *net.nodes[0].node.goals[&goal]
        .state()
        .effects
        .keys()
        .next()
        .unwrap();
    let replica = net.nodes[0].node.replica(&goal).unwrap();
    assert!(
        replica
            .receive_receipt(EndpointId([99; 32]), effect, target)
            .is_err()
    );
    assert!(!replica.receive_delivery(effect, target).unwrap());
    assert!(
        !replica
            .receive_delivery(EffectId([99; 32]), target)
            .unwrap()
    );
}

pub(super) fn unmaterialized() -> (MemStore, GoalId, PublicKey) {
    let (net, goal, source, _) = unbound_network();
    assert!(net.nodes[0].node.goals[&goal].state().effects.is_empty());
    (net.nodes[0].store.reopen(), goal, source)
}

pub(super) struct FailureFixture {
    pub sender: MemStore,
    pub recipient: MemStore,
    pub goal: GoalId,
    pub effect: EffectId,
    pub target: PublicKey,
    pub incoming: Vec<locust_proto::event::WireEvent>,
    pub recipient_endpoint: EndpointId,
}

pub(super) fn failure_fixture() -> FailureFixture {
    let (mut net, goal, _, target) = unbound_network();
    // Deliver the definition and membership before injecting the effect commit.
    for _ in 0..2 {
        net.poll(31_000);
    }
    bind_stages(&mut net, goal);
    // Prime the recipient with the rule binding and its content, then inject
    // only the materialized effect into the failing commit below.
    let mut prelude = crate::node::commit::Tx::none();
    for (_, event) in net.nodes[0].store.log(&goal, 0, usize::MAX).unwrap() {
        if net.nodes[1].store.has_event(&event.id()).unwrap()
            || matches!(
                event.header().body,
                locust_proto::event::Body::EffectMaterialized { .. }
            )
        {
            continue;
        }
        for hash in event.header().blobs() {
            if let Some(blob) = net.nodes[0].store.blob(&hash).unwrap() {
                prelude
                    .commit
                    .blobs
                    .push(locust_proto::store::Blob::new(blob));
            }
        }
        prelude.commit.events.push(event);
    }
    net.nodes[1].node.land(prelude).unwrap();
    let effect = *net.nodes[0].node.goals[&goal]
        .state()
        .effects
        .keys()
        .next()
        .unwrap();
    let incoming = net.nodes[0]
        .store
        .log(&goal, 0, usize::MAX)
        .unwrap()
        .into_iter()
        .filter(|(_, event)| !net.nodes[1].store.has_event(&event.id()).unwrap())
        .map(|(_, event)| event.to_wire())
        .collect();
    assert!(net.nodes[1].node.goals[&goal].deliveries.is_empty());
    FailureFixture {
        sender: net.nodes[0].store.reopen(),
        recipient: net.nodes[1].store.reopen(),
        goal,
        effect,
        target,
        incoming,
        recipient_endpoint: Network::endpoint(1),
    }
}

/// The host's agent publishes a finding: its own work, signed by its own key.
fn host_note(net: &mut Network, goal: GoalId, summary: &str) -> locust_proto::event::Event {
    let a = net.nodes[0].connect(credential(1), None);
    let owner = net.nodes[0].owner();
    let host = net.nodes[0].node.goals[&goal].state().host.unwrap();
    net.nodes[0].ok(
        owner,
        Request::LevelSet {
            goal,
            agent: host,
            level: locust_proto::api::Level::Ask,
        },
    );
    let Response::Recorded { event } = net.nodes[0].ok(
        a,
        Request::ContributionPublish {
            goal,
            attempt: None,
            generation: None,
            sources: vec![],
            artifacts: vec![],
            summary: summary.into(),
        },
    ) else {
        panic!()
    };
    let event = net.nodes[0].store.event(&event).unwrap().unwrap();
    assert_eq!(event.header().author, host);
    event
}

/// A principal enrolled on the hosting daemon joins there; the admission is
/// the governance key's record.
fn host_admits(
    net: &mut Network,
    goal: GoalId,
    tag: u8,
) -> (PublicKey, locust_proto::event::Event) {
    let a = net.nodes[0].connect(credential(1), None);
    let (member, _) = join_local(&mut net.nodes[0], a, goal, tag);
    let admission = net.nodes[0].node.goals[&goal].state().members[&member].admission;
    let event = net.nodes[0].store.event(&admission).unwrap().unwrap();
    assert_eq!(
        Some(event.header().author),
        net.nodes[0].node.goals[&goal].state().governance
    );
    (member, event)
}

fn restore_host(net: &mut Network, backup: MemStore) {
    net.nodes[0].store = backup;
    net.restart();
}

#[test]
fn a_restored_hosts_agent_forks_only_its_own_log() {
    for recover_first in [false, true] {
        let (mut net, goal, source, _) = unbound_network();
        for _ in 0..2 {
            net.poll(31_000);
        }
        let governance = net.nodes[0].node.goals[&goal].state().governance.unwrap();
        assert_ne!(governance, source);
        let backup = snapshot(&net.nodes[0].store);
        let old = host_note(&mut net, goal, "held by peer, absent from backup");
        net.poll(31_000);
        assert!(net.nodes[1].store.has_event(&old.id()).unwrap());
        restore_host(&mut net, backup);
        assert!(!net.nodes[0].store.has_event(&old.id()).unwrap());
        if recover_first {
            for _ in 0..2 {
                net.poll(31_000);
            }
            assert!(net.nodes[0].store.has_event(&old.id()).unwrap());
        }
        let new = host_note(&mut net, goal, "signed after restore");
        assert_ne!(old.id(), new.id());
        if recover_first {
            assert_eq!(new.header().seq, old.header().seq + 1);
            assert_eq!(new.header().prev, Some(old.id()));
        } else {
            assert_eq!(new.header().seq, old.header().seq);
            assert_eq!(new.header().prev, old.header().prev);
        }
        for _ in 0..3 {
            net.poll(31_000);
        }
        for node in &net.nodes {
            let folded = &node.node.goals[&goal].goal;
            assert_eq!(
                folded.fork_point(&source),
                (!recover_first).then_some(old.header().seq)
            );
            // The agent's fork costs what any member's fork costs: its records
            // from that position on are pending. Governance is untouched.
            assert_eq!(folded.fork_point(&governance), None);
            assert!(folded.evaluation().host_halt.is_none());
            for id in [old.id(), new.id()] {
                assert_eq!(
                    folded.standing(&id).unwrap().is_pending(),
                    !recover_first,
                    "{id}"
                );
            }
        }
        if !recover_first {
            // The forked agent signs nothing more in this goal.
            let a = net.nodes[0].connect(credential(1), None);
            assert_eq!(
                code(net.nodes[0].call(
                    a,
                    Request::ContributionPublish {
                        goal,
                        attempt: None,
                        generation: None,
                        sources: vec![],
                        artifacts: vec![],
                        summary: "known fork blocks signing".into(),
                    }
                )),
                ErrorCode::Unavailable
            );
        }
        // The governance key still signs: the restored daemon admits a joiner
        // and nothing forks.
        let newcomer = net.nodes[1].enroll("newcomer", 3);
        let owner = net.nodes[0].owner();
        let Response::Invited { ticket } = net.nodes[0].ok(
            owner,
            Request::GoalInvite {
                goal,
                expires_ms: 604_801_000,
            },
        ) else {
            panic!()
        };
        let joining_owner = net.nodes[1].owner();
        net.nodes[1].ok(
            joining_owner,
            Request::GoalJoin {
                agent: newcomer,
                ticket,
                level: locust_proto::api::Level::Auto,
            },
        );
        for _ in 0..3 {
            net.poll(31_000);
        }
        for node in &net.nodes {
            let entry = &node.node.goals[&goal];
            assert!(entry.is_member(&newcomer));
            assert_eq!(entry.goal.fork_point(&governance), None);
            assert!(entry.goal.evaluation().host_halt.is_none());
            assert_eq!(
                entry.goal.fork_point(&source),
                (!recover_first).then_some(old.header().seq)
            );
        }
    }
}

#[test]
fn restored_host_automatic_stage_replay_is_identical_unless_another_event_used_its_position() {
    use locust_proto::event::Body;
    for intervening_work in [false, true] {
        let (mut net, goal, _, _) = unbound_network();
        for _ in 0..2 {
            net.poll(31_000);
        }
        let backup = snapshot(&net.nodes[0].store);
        // Another governance record uses the position: an admission.
        let old_work = intervening_work.then(|| host_admits(&mut net, goal, 3).1);
        let enable = |net: &mut Network| {
            bind_stages(net, goal);
            net.nodes[0]
                .store
                .log(&goal, 0, usize::MAX)
                .unwrap()
                .into_iter()
                .map(|(_, e)| e)
                .find(|e| matches!(e.header().body, Body::EffectMaterialized { .. }))
                .unwrap()
        };
        let old_effect = enable(&mut net);
        for _ in 0..2 {
            net.poll(31_000);
        }
        assert!(net.nodes[1].store.has_event(&old_effect.id()).unwrap());
        restore_host(&mut net, backup);
        let replay = enable(&mut net);
        assert_eq!(replay.id() == old_effect.id(), !intervening_work);
        if let Some(old_work) = old_work {
            assert_eq!(replay.header().seq, old_work.header().seq + 1);
            assert_ne!(replay.id(), old_work.id());
        }
        for _ in 0..3 {
            net.poll(31_000);
        }
        for node in &net.nodes {
            assert_eq!(
                node.node.goals[&goal].goal.evaluation().host_halt.is_some(),
                intervening_work
            );
        }
    }
}

#[test]
fn restored_host_redeems_outstanding_invitation_before_recovery_at_an_already_used_position() {
    use crate::sync::Host;
    use locust_proto::invite::{Invitation, JoinRequest};
    let (mut net, goal, source, target) = unbound_network();
    for _ in 0..2 {
        net.poll(31_000);
    }
    let newcomer = net.nodes[1].enroll("newcomer", 3);
    let owner = net.nodes[0].owner();
    let Response::Invited { ticket } = net.nodes[0].ok(
        owner,
        Request::GoalInvite {
            goal,
            expires_ms: 604_801_000,
        },
    ) else {
        panic!()
    };
    let invitation = Invitation::from_ticket(ticket.as_str()).unwrap();
    let governance = net.nodes[0].node.goals[&goal].state().governance.unwrap();
    let backup = snapshot(&net.nodes[0].store);
    // The record lost to the restore is an admission: a governance record at
    // the position the outstanding invitation's admission will take.
    let (earlier, old) = host_admits(&mut net, goal, 3);
    net.poll(31_000);
    assert!(net.nodes[1].store.has_event(&old.id()).unwrap());
    assert!(net.nodes[1].node.goals[&goal].is_member(&earlier));
    restore_host(&mut net, backup);
    assert!(!net.nodes[0].store.has_event(&old.id()).unwrap());
    let request = JoinRequest::sign(
        goal,
        Network::endpoint(1),
        invitation.secret,
        net.nodes[1].node.signer(&newcomer).unwrap(),
    );
    // The authenticated Join handler runs before any peer recovery input.
    Host::join(
        &mut net.nodes[0].node,
        &Network::endpoint(1),
        &request,
        net.now,
    )
    .unwrap();
    let admission = net.nodes[0].node.goals[&goal].state().members[&newcomer].admission;
    let admitted = net.nodes[0].store.event(&admission).unwrap().unwrap();
    assert_eq!(admitted.header().author, governance);
    assert_eq!(admitted.header().seq, old.header().seq);
    assert_ne!(admitted.id(), old.id());
    assert!(net.nodes[0].node.goals[&goal].is_member(&newcomer));
    for _ in 0..3 {
        net.poll(31_000);
    }
    // Two governance records at one position halt governance for good and
    // cut the chain there: neither admission stands, on either daemon.
    for node in &net.nodes {
        let entry = &node.node.goals[&goal];
        assert_eq!(entry.goal.fork_point(&governance), Some(old.header().seq));
        assert_eq!(entry.goal.fork_point(&source), None);
        assert!(matches!(
            entry.goal.evaluation().host_halt,
            Some(crate::goal::Halt::Fork { seq, .. }) if seq == old.header().seq
        ));
        assert!(!entry.is_member(&newcomer));
        assert!(!entry.is_member(&earlier));
    }
    // The halted governance key signs nothing more.
    let owner = net.nodes[0].owner();
    assert_eq!(
        code(net.nodes[0].call(
            owner,
            Request::GoalInvite {
                goal,
                expires_ms: 604_801_000,
            }
        )),
        ErrorCode::Halted
    );
    assert_eq!(
        code(net.nodes[0].call(
            owner,
            Request::MemberRemove {
                goal,
                member: target
            }
        )),
        ErrorCode::Unavailable
    );
}

#[test]
fn restored_host_known_gap_blocks_signing_until_missing_predecessor_arrives() {
    use crate::sync::Host;
    let (mut net, goal, source, target) = unbound_network();
    let governance = net.nodes[0].node.goals[&goal].state().governance.unwrap();
    let backup = snapshot(&net.nodes[0].store);
    // The gap is in the governance log: two admissions, of which only the
    // second arrives before the host signs again.
    let (_, first) = host_admits(&mut net, goal, 3);
    let (_, second) = host_admits(&mut net, goal, 4);
    assert_eq!(second.header().seq, first.header().seq + 1);
    restore_host(&mut net, backup);
    Host::replica(&mut net.nodes[0].node, &goal)
        .unwrap()
        .receive(vec![second.to_wire()])
        .unwrap();
    let entry = &net.nodes[0].node.goals[&goal];
    assert!(entry.goal.next(&governance).is_none());
    assert!(entry.goal.evaluation().host_halt.is_none());
    // The governance key signs nothing while a later record of its own waits
    // for a predecessor; the host's agent's log is intact and it still signs.
    let owner = net.nodes[0].owner();
    assert_eq!(
        code(net.nodes[0].call(
            owner,
            Request::MemberRemove {
                goal,
                member: target
            }
        )),
        ErrorCode::Unavailable
    );
    let aside = host_note(&mut net, goal, "the agent's log has no gap");
    assert_eq!(aside.header().author, source);
    Host::replica(&mut net.nodes[0].node, &goal)
        .unwrap()
        .receive(vec![first.to_wire()])
        .unwrap();
    let owner = net.nodes[0].owner();
    let Response::Recorded { event } = net.nodes[0].ok(
        owner,
        Request::MemberRemove {
            goal,
            member: target,
        },
    ) else {
        panic!()
    };
    let new = net.nodes[0].store.event(&event).unwrap().unwrap();
    assert_eq!(new.header().author, governance);
    assert_eq!(new.header().seq, second.header().seq + 1);
    assert_eq!(new.header().prev, Some(second.id()));
    assert!(
        net.nodes[0].node.goals[&goal]
            .goal
            .evaluation()
            .host_halt
            .is_none()
    );
}

#[path = "delivery_characterization.rs"]
mod lifecycle_characterization;
