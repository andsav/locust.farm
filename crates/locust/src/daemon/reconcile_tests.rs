//! Two production daemons whose copies of one author's log diverge, and the
//! reconciliation that follows, recorded where the shell hands frames to the
//! node.
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use locust_proto::api::{
    ApiError, ClientHello, Credential, ErrorCode, GoalGrants, Request, RequestFrame, Response,
    ServerHello, Standing,
};
use locust_proto::client::{Client, ClientError};
use locust_proto::engine::{
    ConnId, Engine, ExchangeId, Parked, PeerEngine, PeerInput, PeerOutput, PeerTime, Step,
};
use locust_proto::farm::{FarmUpload, FarmUploadResult};
use locust_proto::id::{EventId, GoalId, PublicKey};
use locust_proto::store::Store;
use locust_proto::sync::{Refusal, SyncMessage};
use locust_store::SqliteStore;

use super::ProductionNode;
use super::durable_tests::{Running, copy_stopped_home, eventually_observed, recorded};
use crate::testdir::short_dir;

/// What the member wrote before and after its directory was restored.
const LOST: &str = "before the directory was restored";
const RIVAL: &str = "after the directory was restored";

/// Longest a slowed responder waits for its initiator's next frame.
const PUSH_LIMIT: Duration = Duration::from_secs(10);
/// Time for a frame an engine has emitted to reach the other daemon's
/// transport over loopback.
const SETTLE: Duration = Duration::from_millis(100);

/// One peer frame as a daemon's node saw it.
struct Seen {
    daemon: &'static str,
    exchange: ExchangeId,
    sent: bool,
    /// For a received frame: whether its exchange was readable when the
    /// shell delivered it.
    readable: bool,
    frame: SyncMessage,
}

impl Seen {
    fn describe(&self) -> String {
        let frame = match &self.frame {
            SyncMessage::Refused(refusal) => format!("Refused({refusal:?})"),
            other => {
                let text = format!("{other:?}");
                let name = text.find(|c: char| !c.is_alphanumeric());
                text[..name.unwrap_or(text.len())].to_string()
            }
        };
        let direction = if self.sent { "sent" } else { "received" };
        format!("{} {:?} {direction} {frame}", self.daemon, self.exchange)
    }
}

/// Every peer frame of the daemons under test, in the order their nodes saw
/// them.
#[derive(Default)]
struct Trace {
    seen: Mutex<Vec<Seen>>,
    grew: Condvar,
    /// Set once the logs have diverged; see [`Part`].
    released: AtomicBool,
}

impl Trace {
    fn record(&self, seen: Seen) {
        self.seen.lock().unwrap().push(seen);
        self.grew.notify_all();
    }

    fn release(&self) {
        self.released.store(true, Ordering::Release);
    }

    fn released(&self) -> bool {
        self.released.load(Ordering::Acquire)
    }

    /// Where `daemon` sent an inventory as its latest frame on `exchange`.
    fn latest_is_inventory(&self, daemon: &str, exchange: ExchangeId) -> Option<usize> {
        let seen = self.seen.lock().unwrap();
        let latest = seen.iter().rposition(|entry| {
            entry.sent && entry.daemon == daemon && entry.exchange == exchange
        })?;
        matches!(seen[latest].frame, SyncMessage::Inventory { .. }).then_some(latest)
    }

    /// Blocks the calling engine until the initiator that received the frame
    /// at `sent` has sent its next frame, then until that frame has had time
    /// to arrive.
    fn await_pipelined(&self, sent: usize) {
        let seen = self.seen.lock().unwrap();
        drop(
            self.grew
                .wait_timeout_while(seen, PUSH_LIMIT, |seen| pipelined(seen, sent).is_none())
                .unwrap(),
        );
        thread::sleep(SETTLE);
    }

    fn describe(&self, select: impl Fn(&Seen) -> bool) -> Vec<String> {
        let seen = self.seen.lock().unwrap();
        seen.iter()
            .filter(|entry| select(entry))
            .map(Seen::describe)
            .collect()
    }

    /// Frames that ended an exchange as a protocol error, on either side.
    fn protocol_errors(&self) -> Vec<String> {
        self.describe(|entry| entry.frame == SyncMessage::Refused(Refusal::ProtocolError))
    }

    /// Frames a responder would refuse for arriving while it still owed part
    /// of an answer: delivered to an accepted exchange that was not readable.
    fn delivered_early(&self) -> Vec<String> {
        self.describe(|entry| {
            !entry.sent
                && !entry.readable
                && matches!(entry.exchange, ExchangeId::Accepted(_))
                && !matches!(entry.frame, SyncMessage::Done | SyncMessage::Refused(_))
        })
    }

    /// Every inventory of `author` a responder sent in an answer that its
    /// initiator pushed events into.
    fn pushes_into_an_answer(&self, author: &PublicKey) -> Vec<Push> {
        let seen = self.seen.lock().unwrap();
        let mut found = Vec::new();
        for (inventory, entry) in seen.iter().enumerate() {
            if !entry.sent
                || !matches!(&entry.frame, SyncMessage::Inventory { author: listed, .. } if listed == author)
            {
                continue;
            }
            let Some(sent) = pipelined(&seen, inventory) else {
                continue;
            };
            let SyncMessage::Events(events) = &seen[sent].frame else {
                continue;
            };
            let answering = |at: &usize| {
                seen[*at].daemon == entry.daemon && seen[*at].exchange == entry.exchange
            };
            let frontier = (inventory + 1..seen.len())
                .filter(answering)
                .find(|&at| seen[at].sent && matches!(seen[at].frame, SyncMessage::Frontier(_)));
            let delivered = (sent + 1..seen.len())
                .filter(answering)
                .find(|&at| !seen[at].sent && seen[at].frame == seen[sent].frame);
            if let (Some(frontier), Some(delivered)) = (frontier, delivered) {
                found.push(Push {
                    events: events.iter().map(|event| event.id()).collect(),
                    sent,
                    frontier,
                    delivered,
                });
            }
        }
        found
    }
}

/// The first frame an initiator sent after it received the frame at `sent`,
/// on the same exchange.
fn pipelined(seen: &[Seen], sent: usize) -> Option<usize> {
    let frame = &seen[sent];
    let received = (sent + 1..seen.len()).find(|&at| {
        !seen[at].sent && seen[at].daemon != frame.daemon && seen[at].frame == frame.frame
    })?;
    (received + 1..seen.len()).find(|&at| {
        seen[at].sent
            && seen[at].daemon == seen[received].daemon
            && seen[at].exchange == seen[received].exchange
    })
}

/// Positions in the trace of one push into an inventory answer.
#[derive(Debug)]
struct Push {
    events: Vec<EventId>,
    /// The initiator's node emitted the pushed events.
    sent: usize,
    /// The responder's node emitted the frontier that closes its answer.
    frontier: usize,
    /// The responder's shell delivered the pushed events.
    delivered: usize,
}

/// How a daemon takes part once started.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    /// Untouched.
    Plain,
    /// Opens no exchange until the trace is released: the node receives no
    /// `Poll`. A restored daemon whose peers are out of reach is in this
    /// state, and signs without having synchronized.
    Isolated,
    /// After the release, pauses whenever it has sent an inventory and owes
    /// more of that answer, until the initiator's next frame has reached its
    /// transport. Without the pause the answer drains over loopback before
    /// that frame arrives, and a shell that held nothing would pass.
    Slow,
}

/// The production node, with every peer frame recorded on the way through.
struct Observed {
    node: ProductionNode,
    daemon: &'static str,
    part: Part,
    trace: Arc<Trace>,
}

impl Engine for Observed {
    fn connect(&mut self, conn: ConnId, hello: &ClientHello, now_ms: u64) -> ServerHello {
        self.node.connect(conn, hello, now_ms)
    }
    fn request(&mut self, conn: ConnId, frame: RequestFrame, now_ms: u64) -> Step {
        self.node.request(conn, frame, now_ms)
    }
    fn resume(&mut self, conn: ConnId, parked: &Parked, timed_out: bool, now_ms: u64) -> Step {
        self.node.resume(conn, parked, timed_out, now_ms)
    }
    fn take_changed(&mut self) -> Vec<GoalId> {
        Engine::take_changed(&mut self.node)
    }
    fn disconnect(&mut self, conn: ConnId) {
        self.node.disconnect(conn);
    }
    fn stop_requested(&self) -> bool {
        self.node.stop_requested()
    }
    fn failure(&self) -> Option<ApiError> {
        self.node.failure()
    }
    fn farm_poll(&mut self, now_ms: u64) -> Vec<FarmUpload> {
        self.node.farm_poll(now_ms)
    }
    fn farm_complete(&mut self, result: FarmUploadResult, now_ms: u64) -> Result<(), ApiError> {
        self.node.farm_complete(result, now_ms)
    }
}

impl PeerEngine for Observed {
    fn endpoint_secret(&self) -> [u8; 32] {
        self.node.endpoint_secret()
    }
    fn peer_readable(&self, exchange: ExchangeId) -> bool {
        self.node.peer_readable(exchange)
    }
    fn peer(&mut self, input: PeerInput, time: PeerTime, out: &mut Vec<PeerOutput>) {
        let released = self.trace.released();
        let mut answering = None;
        match &input {
            PeerInput::Poll if self.part == Part::Isolated && !released => return,
            PeerInput::Frame { exchange, frame } => self.trace.record(Seen {
                daemon: self.daemon,
                exchange: *exchange,
                sent: false,
                readable: self.node.peer_readable(*exchange),
                frame: frame.clone(),
            }),
            PeerInput::Writable(exchange @ ExchangeId::Accepted(_))
                if self.part == Part::Slow && released =>
            {
                answering = self
                    .trace
                    .latest_is_inventory(self.daemon, *exchange)
                    .map(|inventory| (*exchange, inventory));
            }
            _ => {}
        }
        let before = out.len();
        self.node.peer(input, time, out);
        if let Some((exchange, inventory)) = answering
            && !self.node.peer_readable(exchange)
        {
            self.trace.await_pipelined(inventory);
        }
        for output in &out[before..] {
            if let PeerOutput::Send { exchange, frame } = output {
                self.trace.record(Seen {
                    daemon: self.daemon,
                    exchange: *exchange,
                    sent: true,
                    readable: true,
                    frame: frame.clone(),
                });
            }
        }
    }
}

fn observed(home: &Path, trace: &Arc<Trace>, daemon: &'static str, part: Part) -> Running {
    let trace = trace.clone();
    Running::start_observed(home, move |node| Observed {
        node,
        daemon,
        part,
        trace,
    })
}

fn publish(
    client: &mut Client<UnixStream>,
    goal: GoalId,
    summary: &str,
) -> Result<EventId, ClientError> {
    client
        .call(Request::ContributionPublish {
            goal,
            task: None,
            attempt: None,
            generation: None,
            sources: Vec::new(),
            artifacts: vec![],
            summary: summary.into(),
        })
        .map(recorded)
}

/// How an event stands on the client's daemon, with its text once the content
/// is held too; `None` when the daemon does not hold the event.
fn held(
    client: &mut Client<UnixStream>,
    goal: GoalId,
    event: EventId,
) -> Option<(Standing, Option<String>)> {
    match client.call(Request::Event { goal, event }) {
        Ok(Response::Event(detail)) => Some((detail.view.standing, detail.text)),
        _ => None,
    }
}

/// A member's daemon is restored from a copy of its directory taken before
/// the member wrote anything, and signs at the position the host already
/// holds another event for. The two daemons then reconcile over loopback
/// Iroh: the host answers the member's frontier with an inventory and still
/// owes its closing frontier when the member pushes its event. The host's
/// shell must hold that push until the answer has drained; a responder
/// refuses it otherwise.
#[test]
fn a_diverged_author_log_reconciles_between_two_real_daemons() {
    let trace = Arc::new(Trace::default());
    let (first, second, copy) = (short_dir(), short_dir(), short_dir());
    let mut host = observed(first.path(), &trace, "host", Part::Slow);
    let mut member = observed(second.path(), &trace, "member", Part::Plain);
    host.enroll(1);
    let author = member.enroll(2);
    let mut administrator = host.client(Credential([1; 32]), None);
    let Ok(Response::GoalCreated { goal }) = administrator.call(Request::GoalCreate {
        title: "Diverged log".into(),
        formation_json: None,
        roles: Default::default(),
        inputs: Default::default(),
    }) else {
        panic!("goal not created")
    };
    let Ok(Response::Invited { ticket }) = administrator.call(Request::GoalInvite {
        goal,
        expires_ms: None,
    }) else {
        panic!("no invitation")
    };
    let mut agent = member.client(Credential([2; 32]), None);
    agent.call(Request::GoalJoin { ticket }).unwrap();
    eventually_observed(
        "the member's admission",
        || agent.call(Request::GoalStatus { goal }),
        |observed| match observed {
            Ok(Response::GoalStatus(status))
                if status.title.is_some()
                    && status.members.iter().any(|entry| entry.member == author) =>
            {
                Some(())
            }
            _ => None,
        },
    );
    member
        .owner()
        .call(Request::GoalGrant {
            goal,
            agent: author,
            grants: GoalGrants {
                contribute: true,
                ..Default::default()
            },
        })
        .unwrap();
    drop(agent);
    member.stop();
    // The copy holds the goal and the member's key, and nothing the member wrote.
    copy_stopped_home(second.path(), copy.path());

    let mut member = observed(second.path(), &trace, "member after the copy", Part::Plain);
    let mut agent = member.client(Credential([2; 32]), None);
    let lost = publish(&mut agent, goal, LOST).unwrap();
    let effective = |text: &str| Some((Standing::Effective, Some(text.to_string())));
    eventually_observed(
        "the host holding the member's event",
        || held(&mut administrator, goal, lost),
        |held| (*held == effective(LOST)).then_some(()),
    );
    drop(agent);
    member.stop();

    let mut restored = observed(copy.path(), &trace, "restored member", Part::Isolated);
    let mut agent = restored.client(Credential([2; 32]), None);
    let rival = publish(&mut agent, goal, RIVAL).unwrap();
    assert_ne!(rival, lost);
    assert_eq!(held(&mut agent, goal, rival), effective(RIVAL));
    assert_eq!(held(&mut agent, goal, lost), None);
    assert_eq!(held(&mut administrator, goal, rival), None);
    trace.release();

    eventually_observed(
        "both daemons holding both events as a fork",
        || {
            let daemons = [&mut administrator, &mut agent]
                .map(|client| [lost, rival].map(|event| held(client, goal, event)));
            (trace.protocol_errors(), daemons)
        },
        |(refused, daemons)| {
            assert!(
                refused.is_empty(),
                "an exchange ended as a protocol error: {refused:?}"
            );
            // A daemon that knows of the fork stops counting either event.
            let forked = |text: &str| Some((Standing::Pending, Some(text.to_string())));
            daemons
                .iter()
                .all(|held| *held == [forked(LOST), forked(RIVAL)])
                .then_some(())
        },
    );
    // The member's own daemon reports the fork by signing nothing more.
    assert!(matches!(
        publish(&mut agent, goal, "after the fork is known"),
        Err(ClientError::Api(error)) if error.code == ErrorCode::Unavailable
    ));
    for client in [&mut administrator, &mut agent] {
        let Ok(Response::GoalStatus(status)) = client.call(Request::GoalStatus { goal }) else {
            panic!("no goal status")
        };
        assert_eq!(status.halted, None);
    }
    drop((administrator, agent));
    restored.stop();
    host.stop();

    for home in [first.path(), copy.path()] {
        let store = SqliteStore::open(home).unwrap();
        let [first, second] = [lost, rival].map(|id| store.event(&id).unwrap().unwrap());
        assert_eq!(first.header().author, author);
        assert_eq!(second.header().author, author);
        assert_eq!(first.header().seq, second.header().seq);
    }
    assert_eq!(trace.protocol_errors(), Vec::<String>::new());
    assert_eq!(trace.delivered_early(), Vec::<String>::new());
    // The member pushed its event while the host still owed the frontier
    // that closes its answer, and the host's shell delivered the push only
    // after that frontier.
    let pushes = trace.pushes_into_an_answer(&author);
    assert!(
        pushes.iter().any(|push| push.events.contains(&rival)
            && push.sent < push.frontier
            && push.frontier < push.delivered),
        "no push overtook the answer it was sent into: {pushes:?}"
    );
}
