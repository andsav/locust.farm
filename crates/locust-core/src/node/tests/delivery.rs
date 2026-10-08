//! Real Node + Driver delivery across encoded transport frames, with a lost receipt.
use super::authorization::{governance_key, join_local};
use super::*;
use crate::node::guard::Hold;
use locust_proto::engine::{ExchangeId, PeerEngine, PeerInput, PeerOutput, PeerTime};
use locust_proto::id::{EffectId, EndpointId, GoalId};
use locust_proto::store::{Space, Store};
use locust_proto::sync::SyncMessage;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

type Side = (usize, ExchangeId);
pub(super) struct Network {
    pub(super) nodes: Vec<Daemon>,
    /// Daemons that are stopped or cut off: never polled, and an exchange
    /// opened to one fails.
    pub(super) down: BTreeSet<usize>,
    /// Every exchange opened, as (dialer, dialed), in order.
    pub(super) opened: Vec<(usize, usize)>,
    routes: BTreeMap<Side, Side>,
    queue: VecDeque<(usize, PeerInput)>,
    pub(super) now: u64,
    accepted: u64,
    drop_receipt: bool,
    receipts_dropped: usize,
    sent: Vec<(usize, EffectId, PublicKey)>,
}
impl Network {
    pub(super) fn new() -> Self {
        Self::with(2)
    }
    /// `n` daemons on `n` computers, each knowing its endpoint.
    pub(super) fn with(n: usize) -> Self {
        let mut net = Self {
            nodes: (0..n).map(|i| Daemon::new(71 + i as u8)).collect(),
            down: BTreeSet::new(),
            opened: vec![],
            routes: BTreeMap::new(),
            queue: VecDeque::new(),
            now: 1000,
            accepted: 0,
            drop_receipt: false,
            receipts_dropped: 0,
            sent: vec![],
        };
        for i in 0..n {
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
                    let Some(j) = (0..self.nodes.len())
                        .find(|j| Self::endpoint(*j) == endpoint && !self.down.contains(j))
                    else {
                        self.queue.push_back((i, PeerInput::OpenFailed(exchange)));
                        continue;
                    };
                    self.opened.push((i, j));
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
        let up: Vec<_> = (0..self.nodes.len())
            .filter(|i| !self.down.contains(i))
            .collect();
        self.poll_from(&up, elapsed);
    }
    /// Lets time pass and polls only `dialers`, so only they open exchanges;
    /// every daemon that is up still answers.
    pub(super) fn poll_from(&mut self, dialers: &[usize], elapsed: u64) {
        self.now += elapsed;
        for &i in dialers {
            self.input(i, PeerInput::Poll);
        }
        self.settle();
    }
    fn settle(&mut self) {
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
    /// Stops daemon `i` and starts it again over `store`: its own store
    /// reopened, or a copy put in its place.
    pub(super) fn start_over(&mut self, i: usize, store: MemStore) {
        self.routes.clear();
        self.queue.clear();
        self.nodes[i].store = store;
        self.nodes[i].restart();
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
            name: "host".into(),
            agent: source,
            title: "Durable delivery".into(),
            formation_json: Some("{\"schema_version\":2}".into()),

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
            role: None,
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
            name: "member".into(),
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
        no_role: false,
        goal,
        expected,
        formation_json: serde_json::to_string(&formation).unwrap(),

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
    for (index, daemon) in net.nodes.iter_mut().enumerate() {
        let owner = daemon.owner();
        let Response::GoalStatus(status) = daemon.ok(owner, Request::GoalStatus { goal }) else {
            panic!()
        };
        if index == 0 {
            assert!(!status.stalled.is_empty());
            assert!(
                status
                    .stalled
                    .iter()
                    .all(|step| step.runner == status.governance
                        && step.reason == locust_proto::api::Stall::Halted)
            );
        } else {
            assert!(status.stalled.is_empty());
        }
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

/// The host's agent asks to publish a finding: its own work, signed by its
/// own key.
fn try_host_note(
    net: &mut Network,
    goal: GoalId,
    summary: &str,
) -> Result<locust_proto::event::Event, ApiError> {
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
    let Response::Recorded { event } = net.nodes[0].call(
        a,
        Request::ContributionPublish {
            goal,
            attempt: None,
            generation: None,
            sources: vec![],
            artifacts: vec![],
            summary: summary.into(),
        },
    )?
    else {
        panic!()
    };
    let event = net.nodes[0].store.event(&event).unwrap().unwrap();
    assert_eq!(event.header().author, host);
    Ok(event)
}

/// The host's agent publishes a finding.
fn host_note(net: &mut Network, goal: GoalId, summary: &str) -> locust_proto::event::Event {
    try_host_note(net, goal, summary).unwrap_or_else(|error| panic!("publish failed: {error}"))
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

/// Stops the host and starts it again over `backup` in place of its data
/// directory. Which marks it finds is the backup's choice: `snapshot` keeps
/// the original's, `snapshot_all` loses them.
fn restore_host(net: &mut Network, backup: MemStore) {
    net.start_over(0, backup);
}

/// Hands `events` to `node` as one received batch, then the content they
/// name from `source`, as an exchange would in its record and content
/// stages; no computer is heard from.
fn deliver(
    node: &mut TestNode,
    source: &MemStore,
    goal: GoalId,
    events: &[locust_proto::event::Event],
) {
    use crate::sync::Host;
    Host::replica(node, &goal)
        .unwrap()
        .receive(events.iter().map(|event| event.to_wire()).collect())
        .unwrap();
    for event in events {
        for hash in event.header().blobs() {
            if let Some(bytes) = source.blob(&hash).unwrap() {
                Host::replica(node, &goal)
                    .unwrap()
                    .stage(&hash, 0, bytes.len() as u64, &bytes);
            }
        }
    }
}

/// Every record of `goal` that `store` holds and `older` does not, in log
/// order.
fn lost_since(store: &MemStore, older: &MemStore, goal: GoalId) -> Vec<locust_proto::event::Event> {
    store
        .log(&goal, 0, usize::MAX)
        .unwrap()
        .into_iter()
        .map(|(_, event)| event)
        .filter(|event| !older.has_event(&event.id()).unwrap())
        .collect()
}

fn hold(net: &Network, i: usize, goal: GoalId, key: &PublicKey) -> Option<Hold> {
    let node = &net.nodes[i].node;
    node.hold(&node.goals[&goal], key)
}

#[test]
fn a_restored_hosts_agent_is_held_until_its_own_records_return_and_then_extends() {
    let (mut net, goal, source, _) = unbound_network();
    for _ in 0..2 {
        net.poll(31_000);
    }
    let governance = net.nodes[0].node.goals[&goal].state().governance.unwrap();
    assert_ne!(governance, source);
    // Marks kept: the data directory is put back from this copy while the
    // marks beside it survive.
    let backup = snapshot(&net.nodes[0].store);
    let old = host_note(&mut net, goal, "held by peer, absent from backup");
    net.poll(31_000);
    assert!(net.nodes[1].store.has_event(&old.id()).unwrap());
    restore_host(&mut net, backup);
    assert!(!net.nodes[0].store.has_event(&old.id()).unwrap());
    // The agent's mark names a record the copy lacks; the governance key
    // signed nothing since the copy and is not held.
    assert_eq!(
        hold(&net, 0, goal, &source),
        Some(Hold::Behind {
            mark: locust_proto::event::AuthorPoint {
                seq: old.header().seq,
                id: old.id(),
            }
        })
    );
    assert_eq!(hold(&net, 0, goal, &governance), None);
    let refused = try_host_note(&mut net, goal, "would reuse the lost position").unwrap_err();
    assert_eq!(refused.code, ErrorCode::ReadOnly);
    let (agent, view) = super::this_computer(&refused);
    assert_eq!(
        (agent.agent, view.key, view.by_host),
        (source, source, false)
    );
    assert!(
        refused
            .message
            .contains("older than what this computer signed in the goal (this computer)"),
        "{refused}"
    );
    assert!(!net.nodes[0].node.failed);
    // The member's computer sends the record back; the mark is met.
    for _ in 0..2 {
        net.poll(31_000);
    }
    assert!(net.nodes[0].store.has_event(&old.id()).unwrap());
    assert_eq!(hold(&net, 0, goal, &source), None);
    let new = host_note(&mut net, goal, "signed after restore");
    assert_eq!(new.header().seq, old.header().seq + 1);
    assert_eq!(new.header().prev, Some(old.id()));
    for _ in 0..3 {
        net.poll(31_000);
    }
    for node in &net.nodes {
        let folded = &node.node.goals[&goal].goal;
        assert_eq!(folded.fork_point(&source), None);
        assert_eq!(folded.fork_point(&governance), None);
        assert!(folded.evaluation().host_halt.is_none());
        for id in [old.id(), new.id()] {
            assert!(!folded.standing(&id).unwrap().is_pending(), "{id}");
        }
    }
    // The governance key was never held: the restored daemon admits a
    // joiner and nothing forks.
    let newcomer = net.nodes[1].enroll("newcomer", 3);
    let owner = net.nodes[0].owner();
    let Response::Invited { ticket } = net.nodes[0].ok(
        owner,
        Request::GoalInvite {
            role: None,
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
            name: "member".into(),
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
        assert_eq!(entry.goal.fork_point(&source), None);
        assert!(entry.goal.evaluation().host_halt.is_none());
    }
}

#[test]
fn a_restored_host_signs_no_stage_step_until_caught_up() {
    use locust_proto::event::Body;
    for intervening_work in [false, true] {
        let (mut net, goal, _, _) = unbound_network();
        for _ in 0..2 {
            net.poll(31_000);
        }
        let governance = net.nodes[0].node.goals[&goal].state().governance.unwrap();
        // Marks kept: the data directory is put back from this copy while
        // the marks beside it survive.
        let backup = snapshot(&net.nodes[0].store);
        // Another governance record takes the step's position in the copy
        // unless the guard holds: an admission before the rules.
        if intervening_work {
            host_admits(&mut net, goal, 3);
        }
        bind_stages(&mut net, goal);
        let lost = lost_since(&net.nodes[0].store, &backup, goal);
        let (step, input): (Vec<_>, Vec<_>) = lost
            .into_iter()
            .partition(|event| matches!(event.header().body, Body::EffectMaterialized { .. }));
        let [step] = step.as_slice() else {
            panic!("one stage step")
        };
        assert_eq!(step.header().author, governance);
        for _ in 0..2 {
            net.poll(31_000);
        }
        assert!(net.nodes[1].store.has_event(&step.id()).unwrap());
        restore_host(&mut net, backup);
        // The records the step follows from return first, as a responder
        // that sends them in one batch and the step in the next would.
        let member = net.nodes[1].store.reopen();
        deliver(&mut net.nodes[0].node, &member, goal, &input);
        let waiting = |net: &Network| {
            let entry = &net.nodes[0].node.goals[&goal];
            entry
                .goal
                .evaluation()
                .desired_effects
                .values()
                .any(|desired| {
                    desired.runner == governance && !entry.state().effects.contains_key(&desired.id)
                })
        };
        assert!(waiting(&net), "the step is due here");
        assert!(matches!(
            hold(&net, 0, goal, &governance),
            Some(Hold::Behind { mark }) if mark.id == step.id()
        ));
        // Neither landing the batch nor a start signs it.
        net.nodes[0].restart();
        assert!(waiting(&net));
        assert!(!net.nodes[0].node.failed);
        let tip = |net: &Network| {
            *net.nodes[0].node.goals[&goal]
                .goal
                .points(&governance)
                .last()
                .unwrap()
        };
        assert_eq!(tip(&net).id, input.last().unwrap().id());
        deliver(
            &mut net.nodes[0].node,
            &member,
            goal,
            std::slice::from_ref(step),
        );
        assert_eq!(hold(&net, 0, goal, &governance), None);
        assert!(!waiting(&net));
        assert_eq!(tip(&net).id, step.id());
        for _ in 0..3 {
            net.poll(31_000);
        }
        for node in &net.nodes {
            let entry = &node.node.goals[&goal];
            assert_eq!(entry.goal.fork_point(&governance), None);
            assert!(entry.goal.evaluation().host_halt.is_none());
        }
    }
}

#[test]
fn a_stage_step_signed_twice_from_one_input_is_one_record() {
    use crate::sync::Host;
    use locust_proto::event::Body;
    let (mut net, goal, _, _) = unbound_network();
    for _ in 0..2 {
        net.poll(31_000);
    }
    // Marks lost: a whole-computer copy, the second store that holds the
    // governance key. Its owner continues, since a copy of unknown age on
    // the host's computer waits for the person.
    let mut twin = Daemon::new(90);
    twin.store = snapshot_all(&net.nodes[0].store);
    twin.restart();
    let owner = twin.owner();
    assert!(matches!(
        twin.ok(owner, Request::GoalContinue { goal }),
        Response::Continued { keys } if keys > 0
    ));
    bind_stages(&mut net, goal);
    let lost = lost_since(&net.nodes[0].store, &twin.store, goal);
    let (step, input): (Vec<_>, Vec<_>) = lost
        .into_iter()
        .partition(|event| matches!(event.header().body, Body::EffectMaterialized { .. }));
    let [step] = step.as_slice() else {
        panic!("one stage step")
    };
    // The second store receives the input and signs the step itself.
    let original = net.nodes[0].store.reopen();
    deliver(&mut twin.node, &original, goal, &input);
    let replay = lost_since(&twin.store, &net.nodes[1].store, goal)
        .into_iter()
        .find(|event| matches!(event.header().body, Body::EffectMaterialized { .. }))
        .expect("the second store signed the step");
    assert_eq!(replay.id(), step.id());
    assert_eq!(replay.header().seq, step.header().seq);
    // Both copies of the step are one record wherever they meet.
    assert_eq!(
        Host::replica(&mut net.nodes[0].node, &goal)
            .unwrap()
            .receive(vec![replay.to_wire()]),
        Ok(0)
    );
    for _ in 0..2 {
        net.poll(31_000);
    }
    let member = &mut net.nodes[1].node;
    assert!(member.goals[&goal].goal.holds(&step.id()));
    assert_eq!(
        Host::replica(member, &goal)
            .unwrap()
            .receive(vec![replay.to_wire()]),
        Ok(0)
    );
    let governance = member.goals[&goal].state().governance.unwrap();
    assert_eq!(member.goals[&goal].goal.fork_point(&governance), None);
    assert!(member.goals[&goal].goal.evaluation().host_halt.is_none());
}

#[test]
fn a_restored_host_admits_nobody_until_caught_up_and_its_old_invitation_is_revoked() {
    use crate::sync::Host;
    use locust_proto::invite::{Invitation, JoinRequest, Ticket};
    use locust_proto::sync::Refusal;
    let (mut net, goal, _, _) = unbound_network();
    for _ in 0..2 {
        net.poll(31_000);
    }
    let newcomer = net.nodes[1].enroll("newcomer", 3);
    let invite = |net: &mut Network| {
        let owner = net.nodes[0].owner();
        let Response::Invited { ticket } = net.nodes[0].ok(
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
    };
    let join = |net: &mut Network, ticket: &Ticket| {
        let invitation = Invitation::from_ticket(ticket.as_str()).unwrap();
        let request = JoinRequest::sign(
            goal,
            Network::endpoint(1),
            "member".into(),
            invitation.secret,
            net.nodes[1].node.signer(&newcomer).unwrap(),
        );
        let now = net.now;
        Host::join(&mut net.nodes[0].node, &Network::endpoint(1), &request, now)
    };
    let old_ticket = invite(&mut net);
    let governance = net.nodes[0].node.goals[&goal].state().governance.unwrap();
    // Marks kept: the data directory is put back from this copy while the
    // marks beside it survive. The copy holds the old ticket as pending.
    let backup = snapshot(&net.nodes[0].store);
    // The record lost to the restore is an admission: a governance record at
    // the position the next admission would take.
    let (earlier, lost) = host_admits(&mut net, goal, 4);
    net.poll(31_000);
    assert!(net.nodes[1].store.has_event(&lost.id()).unwrap());
    assert!(net.nodes[1].node.goals[&goal].is_member(&earlier));
    restore_host(&mut net, backup);
    assert!(!net.nodes[0].store.has_event(&lost.id()).unwrap());
    let entry = &net.nodes[0].node.goals[&goal];
    assert_eq!(
        entry.local.restored.map(|restored| restored.revoked),
        Some(1)
    );
    assert!(matches!(
        hold(&net, 0, goal, &governance),
        Some(Hold::Behind { mark }) if mark.id == lost.id()
    ));
    // The copy cannot know whether the old ticket was used or revoked after
    // it was taken, so the start revoked it for good.
    assert_eq!(join(&mut net, &old_ticket), Err(Refusal::InvitationRefused));
    // Issuing a ticket signs no record at a position and is not held;
    // admitting on it is, and uses up nothing.
    let new_ticket = invite(&mut net);
    assert_eq!(join(&mut net, &new_ticket), Err(Refusal::CatchingUp));
    let joining_owner = net.nodes[1].owner();
    net.nodes[1].ok(
        joining_owner,
        Request::GoalJoin {
            name: "member".into(),
            agent: newcomer,
            ticket: new_ticket.clone(),
            level: locust_proto::api::Level::Auto,
        },
    );
    // The joiner's computer asks; it is told to ask again, not refused.
    net.poll_from(&[1], 1);
    assert_eq!(net.opened.last(), Some(&(1, 0)));
    assert!(!net.nodes[0].node.goals[&goal].is_member(&newcomer));
    assert!(!net.nodes[1].node.goals[&goal].local.joins[&newcomer].refused);
    // An exchange the host opens brings its admission back; the mark is met.
    net.poll_from(&[0], 1);
    assert!(net.nodes[0].store.has_event(&lost.id()).unwrap());
    assert_eq!(hold(&net, 0, goal, &governance), None);
    assert_eq!(join(&mut net, &old_ticket), Err(Refusal::InvitationRefused));
    // The joiner's computer asks again and is admitted at the next free
    // position.
    for _ in 0..3 {
        net.poll(31_000);
    }
    let admission = net.nodes[0].node.goals[&goal].state().members[&newcomer].admission;
    let admitted = net.nodes[0].store.event(&admission).unwrap().unwrap();
    assert_eq!(admitted.header().author, governance);
    assert_eq!(admitted.header().seq, lost.header().seq + 1);
    assert_eq!(admitted.header().prev, Some(lost.id()));
    for node in &net.nodes {
        let entry = &node.node.goals[&goal];
        assert!(entry.is_member(&newcomer));
        assert!(entry.is_member(&earlier));
        assert_eq!(entry.goal.fork_point(&governance), None);
        assert!(entry.goal.evaluation().host_halt.is_none());
    }
}

#[test]
fn a_restored_host_with_a_gap_stays_held_until_the_missing_record_arrives() {
    let (mut net, goal, source, target) = unbound_network();
    let governance = net.nodes[0].node.goals[&goal].state().governance.unwrap();
    // Marks kept: the data directory is put back from this copy while the
    // marks beside it survive.
    let backup = snapshot(&net.nodes[0].store);
    // The gap is in the governance log: two admissions, of which only the
    // second arrives first.
    let (_, first) = host_admits(&mut net, goal, 3);
    let (_, second) = host_admits(&mut net, goal, 4);
    assert_eq!(second.header().seq, first.header().seq + 1);
    let original = net.nodes[0].store.reopen();
    restore_host(&mut net, backup);
    deliver(
        &mut net.nodes[0].node,
        &original,
        goal,
        std::slice::from_ref(&second),
    );
    let entry = &net.nodes[0].node.goals[&goal];
    assert!(entry.goal.holds(&second.id()));
    assert!(entry.goal.evaluation().host_halt.is_none());
    // The marked record is held but not usable: it waits for its
    // predecessor.
    assert!(matches!(
        hold(&net, 0, goal, &governance),
        Some(Hold::Behind { mark }) if mark.id == second.id()
    ));
    let owner = net.nodes[0].owner();
    let remove = Request::MemberRemove {
        goal,
        member: target,
    };
    assert_eq!(
        code(net.nodes[0].call(owner, remove.clone())),
        ErrorCode::ReadOnly
    );
    // The host's agent's own log is whole, but on the host's computer it is
    // held with the governance key: this copy of who is in is known to be
    // old.
    assert_eq!(
        try_host_note(&mut net, goal, "the governance log has a gap")
            .unwrap_err()
            .code,
        ErrorCode::ReadOnly
    );
    assert!(matches!(
        hold(&net, 0, goal, &source),
        Some(Hold::Behind { mark }) if mark.id == second.id()
    ));
    deliver(
        &mut net.nodes[0].node,
        &original,
        goal,
        std::slice::from_ref(&first),
    );
    assert_eq!(hold(&net, 0, goal, &governance), None);
    assert_eq!(hold(&net, 0, goal, &source), None);
    let owner = net.nodes[0].owner();
    let Response::Recorded { event } = net.nodes[0].ok(owner, remove) else {
        panic!()
    };
    let new = net.nodes[0].store.event(&event).unwrap().unwrap();
    assert_eq!(new.header().author, governance);
    assert_eq!(new.header().seq, second.header().seq + 1);
    assert_eq!(new.header().prev, Some(second.id()));
    let aside = host_note(&mut net, goal, "the agent signs again");
    assert_eq!(aside.header().author, source);
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

#[test]
fn an_unsignable_step_stalls_without_failing_open_join_or_receive() {
    use crate::sync::Host;
    use locust_proto::api::Stall;
    use locust_proto::event::{Body, Event};
    use locust_proto::invite::{Invitation, JoinRequest};
    use locust_proto::organization::{Formation, Selector, Stage};
    let (mut net, goal, _, target) = unbound_network();
    let owner = net.nodes[0].owner();
    let expected = net.nodes[0].node.goals[&goal]
        .state()
        .current_rules
        .unwrap();
    let mut formation = Formation::default();
    // A valid formation whose repeated stage name makes its effect header
    // exceed the wire limit. No enormous member fixture is needed.
    for name in [
        "x".repeat(locust_proto::limits::MAX_HEADER_BYTES),
        "small".into(),
    ] {
        formation.flow.insert(
            name,
            Stage {
                task_type: None,
                requires: vec![],
                recipients: Selector::Members,
            },
        );
    }
    net.nodes[0].ok(
        owner,
        Request::RulesBind {
            no_role: false,
            goal,
            expected,
            formation_json: serde_json::to_string(&formation).unwrap(),
            inputs: BTreeMap::new(),
        },
    );
    let entry = &net.nodes[0].node.goals[&goal];
    assert_eq!(entry.state().effects.len(), 1, "the other stage still runs");
    let stalled = net.nodes[0].node.stalled(entry);
    assert_eq!(stalled.len(), 1);
    assert_eq!(stalled[0].reason, Stall::CannotMaterialize);
    assert_eq!(Some(stalled[0].runner), entry.state().governance);
    let desired = &entry.goal.evaluation().desired_effects[&stalled[0].effect];
    let mut tx = crate::node::commit::Tx::none();
    assert_eq!(
        net.nodes[0]
            .node
            .author_alone(
                entry,
                &desired.runner,
                Body::EffectMaterialized {
                    effect: desired.effect.clone()
                },
                &mut tx
            )
            .unwrap_err()
            .code,
        ErrorCode::Invalid
    );
    net.restart(); // Node::open drives the same unsignable step.
    let owner = net.nodes[0].owner();
    let Response::Invited { ticket } = net.nodes[0].ok(
        owner,
        Request::GoalInvite {
            goal,
            role: None,
            expires_ms: 604_801_000,
        },
    ) else {
        panic!()
    };
    let newcomer = net.nodes[1].enroll("newcomer", 3);
    let invitation = Invitation::from_ticket(ticket.as_str()).unwrap();
    let request = JoinRequest::sign(
        goal,
        Network::endpoint(1),
        "newcomer".into(),
        invitation.secret,
        net.nodes[1].node.signer(&newcomer).unwrap(),
    );
    Host::join(
        &mut net.nodes[0].node,
        &Network::endpoint(1),
        &request,
        net.now,
    )
    .unwrap();
    assert!(net.nodes[0].node.goals[&goal].is_member(&newcomer));
    // A real authenticated member record arrives at the host while the step
    // remains unsignable. Replica::receive must still acknowledge the batch.
    let entry = &net.nodes[0].node.goals[&goal];
    let next = entry.goal.next(&target).unwrap();
    let header = locust_proto::event::Header {
        version: locust_proto::PROTOCOL_VERSION,
        goal,
        author: target,
        seq: next.seq,
        prev: next.prev,
        anchor: Some(next.anchor),
        parents: vec![],
        at_ms: 1,
        payload: None,
        body: Body::ContributionPublished {
            context: entry
                .goal
                .current_context(locust_proto::event::Scope::Goal)
                .unwrap(),
            attempt: None,
            sources: vec![],
            artifacts: vec![],
        },
    };
    let incoming = Event::sign(header, net.nodes[1].node.signer(&target).unwrap()).unwrap();
    assert_eq!(
        Host::replica(&mut net.nodes[0].node, &goal)
            .unwrap()
            .receive(vec![incoming.to_wire()]),
        Ok(1)
    );
    assert_eq!(
        net.nodes[0].node.goals[&goal].goal.standing(&incoming.id()),
        Some(crate::goal::Standing::Effective)
    );
    assert_eq!(
        net.nodes[0].node.stalled(&net.nodes[0].node.goals[&goal])[0].reason,
        Stall::CannotMaterialize
    );
    assert!(!net.nodes[0].node.failed);
}
