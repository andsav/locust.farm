//! One seeded run: the world, what the scenario has been told so far, and
//! the operator's patience.
//!
//! The operator is the person following the guide: they issue one command
//! at a time on a machine that is up, and re-run a read until it shows what
//! the guide says to wait for. Waiting is bounded in simulated time, and
//! only time without an injected fault in effect counts against the bound,
//! so a run fails when the system stops making progress, not because a
//! laptop slept for an hour.

use locust_proto::api::{ApiError, NoteView, Request, Response, TaskView};
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey};

use super::machine::{Power, Who};
use super::rng::Rng;
use super::world::{MS, Micros, SEC, World};

/// How long a wait may go without progress while no fault is in effect.
pub const PATIENCE: Micros = 600 * SEC;

/// Simulated time after which a run is abandoned whatever it is doing.
const HARD_STOP: Micros = 2 * 24 * 3_600 * SEC;

/// Why a run failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fail {
    pub step: &'static str,
    pub what: String,
}

/// A write the daemon acknowledged to its caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Acked {
    pub what: &'static str,
    pub machine: usize,
    pub event: EventId,
}

pub struct Run {
    pub w: World,
    /// The scenario's own choices.
    pub rng: Rng,
    pub step: &'static str,
    pub principals: Vec<PublicKey>,
    pub goal: Option<GoalId>,
    pub acked: Vec<Acked>,
    /// Notes acknowledged, with the text each must show everywhere.
    pub notes: Vec<(EventId, String)>,
    pub task: Option<EventId>,
    pub assignment: Option<EventId>,
    pub result: Option<EventId>,
    /// The generation the session that submitted was given.
    pub generation: Option<u32>,
    /// A content object of several chunks the result names, with its bytes.
    pub artifact: Option<(BlobHash, Vec<u8>)>,
    /// The longest any wait took, counting only fault-free time.
    pub longest_wait: Micros,
    pub longest_wait_for: &'static str,
    /// How often the operator looks again while waiting.
    pub look: Micros,
    /// Whether this run injects faults at all.
    pub faults: bool,
}

impl Run {
    pub fn new(seed: u64, machines: usize) -> Self {
        Self {
            w: World::new(seed, machines),
            rng: Rng::new(seed).fork(3),
            step: "start",
            principals: Vec::new(),
            goal: None,
            acked: Vec::new(),
            notes: Vec::new(),
            task: None,
            assignment: None,
            result: None,
            generation: None,
            artifact: None,
            longest_wait: 0,
            longest_wait_for: "",
            look: 250 * MS,
            faults: true,
        }
    }

    pub fn fail<T>(&self, what: impl Into<String>) -> Result<T, Fail> {
        Err(Fail {
            step: self.step,
            what: what.into(),
        })
    }

    pub fn goal(&self) -> GoalId {
        self.goal.expect("the goal exists by now")
    }

    /// Looks again every `look` until `seen` holds.
    pub fn wait(
        &mut self,
        what: &'static str,
        mut seen: impl FnMut(&mut Self) -> bool,
    ) -> Result<(), Fail> {
        let mut quiet: Micros = 0;
        loop {
            if seen(self) {
                if quiet > self.longest_wait {
                    self.longest_wait = quiet;
                    self.longest_wait_for = what;
                }
                return Ok(());
            }
            if quiet >= PATIENCE {
                return self.fail(format!(
                    "waited {} s with no fault in effect for: {what}",
                    quiet / SEC
                ));
            }
            if self.w.now >= HARD_STOP {
                return self.fail(format!("the run never ended; last waiting for: {what}"));
            }
            let calm = self.w.chaos.quiet();
            self.w.run_for(self.look);
            if calm && self.w.chaos.quiet() {
                quiet += self.look;
            }
        }
    }

    /// Waits until machine `m` runs.
    pub fn up(&mut self, m: usize) -> Result<(), Fail> {
        if self.w.machines[m].running() {
            return Ok(());
        }
        self.wait("the machine to be up", |r| r.w.machines[m].running())
    }

    /// One command on machine `m`, once it is up. The answer may be an error.
    pub fn ask(
        &mut self,
        m: usize,
        who: Who,
        request: Request,
    ) -> Result<Result<Response, ApiError>, Fail> {
        self.up(m)?;
        self.w.chaos_point();
        self.up(m)?;
        Ok(self.w.call(m, who, request))
    }

    /// One command right now, with no fault in between; it must succeed.
    pub fn now(&mut self, m: usize, who: Who, request: Request) -> Result<Response, Fail> {
        let name = request.name();
        match self.w.call(m, who, request) {
            Ok(response) => Ok(response),
            Err(error) => self.fail(format!("m{} {name} failed: {error}", m + 1)),
        }
    }

    /// One command that the guide expects to succeed.
    pub fn op(&mut self, m: usize, who: Who, request: Request) -> Result<Response, Fail> {
        let name = request.name();
        match self.ask(m, who, request)? {
            Ok(response) => Ok(response),
            Err(error) => self.fail(format!("m{} {name} failed: {error}", m + 1)),
        }
    }

    /// A command that signs an event; records the acknowledgement.
    pub fn record(
        &mut self,
        m: usize,
        who: Who,
        what: &'static str,
        request: Request,
    ) -> Result<EventId, Fail> {
        match self.op(m, who, request)? {
            Response::Recorded { event } => {
                self.acked.push(Acked {
                    what,
                    machine: m,
                    event,
                });
                Ok(event)
            }
            other => self.fail(format!("{what}: unexpected answer {other:?}")),
        }
    }

    /// A read on machine `m` as its principal; `None` while it is not up or
    /// the read is refused.
    pub fn read(&mut self, m: usize, request: Request) -> Option<Response> {
        if !self.w.machines[m].running() {
            return None;
        }
        self.w.call(m, Who::Agent, request).ok()
    }

    pub fn board(&mut self, m: usize) -> Option<Vec<TaskView>> {
        let goal = self.goal();
        match self.read(m, Request::Board { goal })? {
            Response::Board(board) => Some(board),
            _ => None,
        }
    }

    pub fn note_views(&mut self, m: usize) -> Option<Vec<NoteView>> {
        let goal = self.goal();
        match self.read(m, Request::Notes { goal, about: None })? {
            Response::Notes(notes) => Some(notes),
            _ => None,
        }
    }

    /// True when machine `m` shows `note` with its text.
    pub fn shows_note(&mut self, m: usize, note: EventId, text: &str) -> bool {
        self.note_views(m).is_some_and(|notes| {
            notes
                .iter()
                .any(|view| view.note == note && view.text.as_deref() == Some(text))
        })
    }

    /// The decrypted text machine `m` shows for `event`, if it holds both.
    pub fn event_text(&mut self, m: usize, event: EventId) -> Option<String> {
        let goal = self.goal();
        match self.read(m, Request::Event { goal, event })? {
            Response::Event(detail) => detail.text,
            _ => None,
        }
    }

    /// How many members machine `m` lists, once it also shows the title.
    pub fn members(&mut self, m: usize) -> Option<usize> {
        let goal = self.goal();
        match self.read(m, Request::GoalStatus { goal })? {
            Response::GoalStatus(status) if status.title.is_some() => Some(status.members.len()),
            _ => None,
        }
    }

    /// The scenario stops machine `m` and keeps it stopped.
    pub fn hold_stopped(&mut self, m: usize) -> Result<(), Fail> {
        if self.w.machines[m].power == Power::Asleep {
            self.wait("the machine to wake before stopping it", |r| {
                r.w.machines[m].power != Power::Asleep
            })?;
        }
        self.w.machines[m].held = true;
        self.w.stop(m, true);
        Ok(())
    }

    /// The scenario starts a machine it stopped.
    pub fn release(&mut self, m: usize) {
        self.w.machines[m].held = false;
        match self.w.machines[m].power {
            Power::Stopped => self.w.start(m),
            Power::Asleep => self.w.wake(m),
            Power::Running => {}
        }
    }

    /// The scenario puts machine `m` to sleep and keeps it asleep.
    pub fn hold_asleep(&mut self, m: usize) -> Result<(), Fail> {
        self.up(m)?;
        self.w.machines[m].held = true;
        self.w.sleep(m);
        Ok(())
    }
}
