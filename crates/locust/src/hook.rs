//! Local hook runtime: authenticated reads, private chat marks and one waiting
//! worker per execution session. Native formats live entirely in the adapters.
//!
//! Silence rule: a chat that has not used Locust never hears from a hook, and
//! a callback Locust cannot attribute to a root chat is ignored. A failure is
//! said only to a chat that has used Locust, once until a callback succeeds.
mod marks;

use std::collections::BTreeSet;
use std::fs;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use locust_adapter::hooks::core::{self, Event, GoalWork, Marks, Outcome, Snapshot};
use locust_adapter::hooks::{ChatIdentity, Parsed};
use locust_proto::api::{
    Caller, Credential, ErrorCode, GoalSummary, Membership, Request, Response, SessionSecret,
    Standing, WaitOutcome,
};
use locust_proto::client::{Client, ClientError};
use locust_proto::event::Body;
use locust_proto::id::{EventId, GoalId};
use locust_proto::{crypto, local};

use crate::{connection, failure::Failure};

pub(crate) struct Config {
    pub home: PathBuf,
    pub credential: PathBuf,
    pub session: PathBuf,
    /// The adapter's native limit for the stop command, the only one that waits.
    pub stop_timeout_seconds: u32,
}

/// Start, tool, and the reads before a stop decision, end within this.
const QUICK: Duration = Duration::from_secs(5);
/// The longest an idle worker's turn end parks.
const MAX_WAIT: Duration = Duration::from_secs(270);
/// Room left inside a native limit for the reads after a wake and the reply.
const MARGIN: Duration = Duration::from_secs(30);

struct Auth {
    socket: PathBuf,
    credential: Credential,
    session: SessionSecret,
    deadline: Instant,
}

/// What a terminal acknowledgment's cancellation turned out to be.
enum Target {
    Attempt(EventId),
    /// It will never name a target: removed, excluded, disputed or not a
    /// cancellation.
    Gone,
    /// Not known yet; ask again at a later callback.
    Later,
}

impl Auth {
    fn stream(&self) -> Result<UnixStream, Failure> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Failure::unavailable("hook time limit reached"));
        }
        let stream = connection::connect(&self.socket)?;
        stream
            .set_read_timeout(Some(remaining))
            .map_err(io_failure)?;
        stream
            .set_write_timeout(Some(remaining))
            .map_err(io_failure)?;
        Ok(stream)
    }
    fn open<'a>(&self, stream: &'a UnixStream) -> Result<Client<&'a UnixStream>, Failure> {
        let client = Client::open(stream, self.credential, Some(self.session))
            .map_err(|error| connection::client_error(error, &self.socket))?;
        if !matches!(client.caller(), Caller::Agent(_)) {
            return Err(Failure::usage("hooks require an agent execution session"));
        }
        Ok(client)
    }
    /// One absolute deadline for the handshake and every read on this socket,
    /// including fragmented frames and a snapshot spanning many requests.
    fn with_stream<T>(
        &self,
        action: impl FnOnce(&UnixStream) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let stream = self.stream()?;
        std::thread::scope(|scope| {
            let (done, receive) = mpsc::channel();
            let watched = &stream;
            scope.spawn(move || {
                if receive
                    .recv_timeout(self.deadline.saturating_duration_since(Instant::now()))
                    .is_err()
                {
                    let _ = watched.shutdown(Shutdown::Both);
                }
            });
            let result = action(&stream);
            let _ = done.send(());
            result
        })
    }
    fn goals(&self) -> Result<Vec<GoalSummary>, Failure> {
        self.with_stream(|stream| {
            let mut client = self.open(stream)?;
            let Response::Status(status) = client
                .call(Request::Status)
                .map_err(|error| connection::client_error(error, &self.socket))?
            else {
                return Err(Failure::internal("hook status response"));
            };
            Ok(status.goals)
        })
    }

    /// A goal the daemon refuses to answer for is left out, keeping its
    /// baseline; only a failed connection fails the whole snapshot.
    fn snapshot(
        &self,
        current: &[GoalSummary],
        resume: bool,
        observed: Option<&Marks>,
    ) -> Result<Snapshot, Failure> {
        self.with_stream(|stream| {
            let mut client = self.open(stream)?;
            let mut goals = Vec::new();
            for goal in current.iter().filter(|goal| eligible(goal, resume)) {
                let baseline = observed.and_then(|marks| marks.goals.get(&goal.goal));
                let changed = match baseline {
                    Some(baseline) => match client.call(Request::Wait {
                        goal: goal.goal,
                        seen: baseline.revision,
                        timeout_ms: 0,
                    }) {
                        Ok(Response::Waited(WaitOutcome::Work(pending))) => Some(*pending),
                        Ok(Response::Waited(WaitOutcome::NoEvent | WaitOutcome::Disconnected)) => {
                            continue;
                        }
                        Ok(_) => return Err(Failure::internal("hook poll response")),
                        // A baseline ahead of the goal, after its database
                        // alone was put back, is read afresh.
                        Err(ClientError::Api(_)) => None,
                        Err(error) => return Err(connection::client_error(error, &self.socket)),
                    },
                    None => None,
                };
                let pending = match changed {
                    Some(pending) => pending,
                    None => match client.call(Request::Pending { goal: goal.goal }) {
                        Ok(Response::Pending(pending)) => pending,
                        Ok(_) => return Err(Failure::internal("hook pending response")),
                        Err(ClientError::Api(_)) => continue,
                        Err(error) => return Err(connection::client_error(error, &self.socket)),
                    },
                };
                goals.push(GoalWork {
                    goal: goal.goal,
                    pending,
                });
            }
            Ok(Snapshot {
                instance: self.session.instance(),
                goals,
                released: BTreeSet::new(),
            })
        })
    }

    fn cancellation_target(&self, goal: GoalId, cancel: EventId) -> Target {
        let answer = self.with_stream(|stream| {
            Ok(self.open(stream)?.call(Request::Event {
                goal,
                event: cancel,
            }))
        });
        match answer {
            Ok(Ok(Response::Event(detail))) => {
                if detail.view.event != cancel {
                    return Target::Gone;
                }
                match (detail.view.standing, detail.body) {
                    (Standing::Effective, Body::CancelRequested { attempt }) => {
                        Target::Attempt(attempt)
                    }
                    (Standing::Pending, Body::CancelRequested { .. }) => Target::Later,
                    _ => Target::Gone,
                }
            }
            Ok(Err(ClientError::Api(error))) if error.code == ErrorCode::NotFound => Target::Gone,
            _ => Target::Later,
        }
    }

    /// One socket per goal; the first changed answer cancels the others. No
    /// detached thread or daemon wait survives this hook invocation.
    fn wait(&self, snapshot: &Snapshot, deadline: Instant) -> Result<bool, Failure> {
        let timeout = deadline.saturating_duration_since(Instant::now());
        if timeout.is_zero() || snapshot.goals.is_empty() {
            return Ok(false);
        }
        let timeout_ms = timeout.as_millis().min(u128::from(u32::MAX)) as u32;
        let mut streams = Vec::new();
        for work in &snapshot.goals {
            let stream = self.stream()?;
            // Keep shutdown handles outside the workers so an answer on one
            // goal releases every other parked connection immediately.
            streams.push((work.goal, work.pending.revision, stream));
        }
        std::thread::scope(|scope| {
            let (send, receive) = mpsc::channel();
            for (goal, seen, stream) in &streams {
                let send = send.clone();
                scope.spawn(move || {
                    let result = self.open(stream).and_then(|mut client| {
                        client
                            .call(Request::Wait {
                                goal: *goal,
                                seen: *seen,
                                timeout_ms,
                            })
                            .map_err(|error| connection::client_error(error, &self.socket))
                    });
                    let _ = send.send(result);
                });
            }
            drop(send);
            let result = loop {
                match receive.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(Ok(Response::Waited(WaitOutcome::Work(_)))) => break Ok(true),
                    Ok(Ok(Response::Waited(WaitOutcome::NoEvent | WaitOutcome::Disconnected))) => {}
                    Ok(Ok(_)) => break Err(Failure::internal("hook wait response")),
                    Ok(Err(error)) => break Err(error),
                    Err(mpsc::RecvTimeoutError::Timeout | mpsc::RecvTimeoutError::Disconnected) => {
                        break Ok(false);
                    }
                }
            };
            for (_, _, stream) in &streams {
                let _ = stream.shutdown(Shutdown::Both);
            }
            result
        })
    }
}

/// Never fails: anything that goes wrong becomes silence or, for a chat that
/// has used Locust, the core's one failure line.
pub(crate) fn run(config: Result<Config, Failure>, parsed: Parsed) -> Outcome {
    let started = Instant::now();
    let (chat, event) = match parsed {
        Parsed::Ignored => return Outcome::default(),
        Parsed::Invalid { chat, .. } => (chat, None),
        Parsed::Input(input) => (input.chat, Some(input.event)),
    };
    // Without its paths or secrets no chat can be found to be a Locust chat.
    match config.and_then(|config| Local::open(&config, &chat, started)) {
        Ok(local) => local.handle(event),
        Err(_) => Outcome::default(),
    }
}

struct Local {
    auth: Auth,
    home: PathBuf,
    /// Native chat identity serialized by the adapter, never used as a path.
    identity: Vec<u8>,
    saved: PathBuf,
    started: Instant,
    stop_limit: Duration,
}

impl Local {
    fn open(config: &Config, chat: &ChatIdentity, started: Instant) -> Result<Self, Failure> {
        for path in [&config.home, &config.credential, &config.session] {
            if !path.is_absolute() {
                return Err(Failure::usage("hook paths must be absolute"));
            }
        }
        let credential = Credential(connection::read_secret(&config.credential)?);
        let session = SessionSecret(connection::read_secret(&config.session)?);
        let identity =
            serde_json::to_vec(chat).map_err(|error| Failure::internal(error.to_string()))?;
        let saved = config
            .home
            .join("hook-marks")
            .join(locust_proto::id::BlobHash(credential.digest()).to_string())
            .join(session.instance().to_string())
            .join(format!("{}.json", crypto::content_hash(&identity)));
        Ok(Self {
            auth: Auth {
                socket: local::socket_path(&config.home)?,
                credential,
                session,
                deadline: started + QUICK,
            },
            home: config.home.clone(),
            identity,
            saved,
            started,
            stop_limit: Duration::from_secs(u64::from(config.stop_timeout_seconds)),
        })
    }

    fn directory(&self) -> Result<marks::Directory, Failure> {
        marks::Directory::open(&self.home, self.auth.credential, self.auth.session)
    }

    fn handle(mut self, event: Option<Event>) -> Outcome {
        let Ok(associated) = fs::exists(&self.saved) else {
            return Outcome::default();
        };
        let Some(event) = event else {
            return if associated {
                self.directory()
                    .map_or_else(|_| Outcome::default(), |directory| self.failed(&directory))
            } else {
                Outcome::default()
            };
        };
        if !associated {
            if !matches!(event, Event::Tool { own_call: Some(_) }) {
                return Outcome::default();
            }
            // First association requires live authentication. An existing
            // chat retains a validated own write before reconnecting, so a
            // daemon outage cannot erase that explanation for a later loss.
            if self
                .auth
                .with_stream(|stream| self.auth.open(stream).map(|_| ()))
                .is_err()
            {
                return Outcome::default();
            }
        }
        let Ok(directory) = self.directory() else {
            return Outcome::default();
        };
        let Ok(mut chat) = directory.chat(&self.identity, self.auth.deadline) else {
            return Outcome::default();
        };
        let (outcome, snapshot) = match self.decide(&directory, &mut chat, event) {
            Ok(decided) => decided,
            Err(_) => (core::fail(&mut chat.marks), None),
        };
        if self.keep(&directory, &mut chat).is_err() {
            return Outcome::default();
        }
        match snapshot {
            Some(snapshot) if outcome.wait => {
                drop(chat);
                self.park(&directory, snapshot)
            }
            _ => outcome,
        }
    }

    fn decide(
        &self,
        directory: &marks::Directory,
        chat: &mut marks::Chat,
        event: Event,
    ) -> Result<(Outcome, Option<Snapshot>), Failure> {
        if let Event::Tool {
            own_call: Some(call),
        } = &event
        {
            // A native successful write is evidence even if the following read
            // fails. Persist it before polling so a later call cannot invent a
            // loss that this exact write already explains.
            core::observe(call, self.auth.session.instance(), &mut chat.marks);
            self.keep(directory, chat)?;
        }
        let resume = event == Event::Start;
        let goals = self.auth.goals()?;
        resolve_releases(&self.auth, &mut chat.marks, &goals, resume);
        let observed = matches!(event, Event::Tool { .. }).then_some(&chat.marks);
        let mut snapshot = self.auth.snapshot(&goals, resume, observed)?;
        snapshot.released = directory.released()?;
        let outcome = core::decide(event, &snapshot, &mut chat.marks);
        Ok((outcome, Some(snapshot)))
    }

    /// Share this chat's own releases with its session, then save its marks.
    fn keep(&self, directory: &marks::Directory, chat: &mut marks::Chat) -> Result<(), Failure> {
        if !chat.marks.released.is_empty() {
            directory.share(&chat.marks.released, self.auth.deadline)?;
            chat.marks.released.clear();
        }
        chat.save()
    }

    fn failed(&self, directory: &marks::Directory) -> Outcome {
        let Ok(mut chat) = directory.chat(&self.identity, self.auth.deadline) else {
            return Outcome::default();
        };
        let outcome = core::fail(&mut chat.marks);
        match chat.save() {
            Ok(()) => outcome,
            Err(_) => Outcome::default(),
        }
    }

    /// An idle worker, in a chat no person types in, waits for work.
    fn park(&mut self, directory: &marks::Directory, mut snapshot: Snapshot) -> Outcome {
        let Ok(Some(_waiting)) = directory.try_wait() else {
            return Outcome::default();
        };
        let cap = self.started + self.stop_limit.saturating_sub(Duration::from_secs(1));
        let wait_limit = MAX_WAIT.min(self.stop_limit.saturating_sub(MARGIN));
        let deadline = (Instant::now() + wait_limit).min(cap);
        loop {
            self.auth.deadline = deadline;
            match self.auth.wait(&snapshot, deadline) {
                Ok(false) => return Outcome::default(),
                Ok(true) => {}
                Err(_) => {
                    self.auth.deadline = (Instant::now() + QUICK).min(cap);
                    return self.failed(directory);
                }
            }
            // Membership and halt can change while parked. Re-read them before
            // deciding whether any newly delivered pending work should block.
            self.auth.deadline = (Instant::now() + QUICK).min(cap);
            let Ok(mut chat) = directory.chat(&self.identity, self.auth.deadline) else {
                return Outcome::default();
            };
            let outcome = match self.decide(directory, &mut chat, Event::Stop { unattended: true })
            {
                Ok((outcome, Some(next))) => {
                    snapshot = next;
                    outcome
                }
                Ok((outcome, None)) => outcome,
                Err(_) => core::fail(&mut chat.marks),
            };
            if self.keep(directory, &mut chat).is_err() {
                return Outcome::default();
            }
            if !outcome.wait {
                return outcome;
            }
        }
    }
}

fn io_failure(error: std::io::Error) -> Failure {
    Failure::internal(format!("hook: {error}"))
}

fn eligible(goal: &GoalSummary, resume: bool) -> bool {
    goal.membership == Membership::Member && (resume || goal.halted.is_none())
}

/// Resolve each terminal acknowledgment's target goal by goal. A read that
/// fails defers only its own goal; one that can never answer is dropped.
fn resolve_releases(auth: &Auth, marks: &mut Marks, goals: &[GoalSummary], resume: bool) {
    let unresolved: BTreeSet<_> = marks
        .unresolved
        .iter()
        .filter(|release| {
            goals
                .iter()
                .any(|goal| goal.goal == release.goal && eligible(goal, resume))
        })
        .map(|release| (release.goal, release.cancel))
        .collect();
    for (goal, cancel) in unresolved {
        match auth.cancellation_target(goal, cancel) {
            Target::Attempt(attempt) => core::resolve_cancellation(marks, goal, cancel, attempt),
            Target::Gone => core::abandon_cancellation(marks, goal, cancel),
            Target::Later => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;

    #[test]
    fn invocation_deadline_stops_a_peer_that_keeps_dribbling_bytes() {
        let home = tempfile::Builder::new()
            .prefix("lh.")
            .tempdir_in("/tmp")
            .unwrap();
        let socket = home.path().join("hook.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let auth = Auth {
            socket,
            credential: Credential([1; 32]),
            session: SessionSecret([2; 32]),
            deadline: Instant::now() + Duration::from_millis(100),
        };
        std::thread::scope(|scope| {
            scope.spawn(|| {
                let (mut stream, _) = listener.accept().unwrap();
                for _ in 0..30 {
                    if stream.write_all(&[1]).is_err() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            });
            let result = auth.with_stream(|mut stream| {
                let mut bytes = [0; 30];
                stream.read_exact(&mut bytes).map_err(io_failure)
            });
            assert!(
                result.is_err(),
                "a stream of partial frames must not reset the invocation deadline"
            );
        });
    }
}
