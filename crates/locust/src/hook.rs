//! Local hook runtime: authenticated reads, private chat marks and one waiting
//! worker per execution session. Native formats live entirely in the adapters.
mod marks;

use std::collections::BTreeSet;
use std::fs;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use locust_adapter::hooks::core::{self, Event, GoalWork, Marks, Outcome, Snapshot};
use locust_proto::api::{
    Caller, Credential, GoalSummary, Membership, Request, Response, SessionSecret, Standing,
    WaitOutcome,
};
use locust_proto::client::Client;
use locust_proto::event::Body;
use locust_proto::id::{EventId, GoalId};
use locust_proto::{crypto, local};

use crate::{connection, failure::Failure};

pub(crate) struct Config {
    pub home: PathBuf,
    pub credential: PathBuf,
    pub session: PathBuf,
    /// Native chat identity serialized by the adapter, never used as a path.
    pub chat: Vec<u8>,
    pub wait_limit_ms: u32,
    pub hook_timeout_ms: u32,
}

struct Auth {
    socket: PathBuf,
    credential: Credential,
    session: SessionSecret,
    deadline: Instant,
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
                let pending = if let Some(baseline) =
                    observed.and_then(|marks| marks.goals.get(&goal.goal))
                {
                    match client
                        .call(Request::Wait {
                            goal: goal.goal,
                            seen: baseline.revision,
                            timeout_ms: 0,
                        })
                        .map_err(|error| connection::client_error(error, &self.socket))?
                    {
                        Response::Waited(WaitOutcome::Work(pending)) => *pending,
                        Response::Waited(WaitOutcome::NoEvent | WaitOutcome::Disconnected) => {
                            continue;
                        }
                        _ => return Err(Failure::internal("hook poll response")),
                    }
                } else {
                    let Response::Pending(pending) = client
                        .call(Request::Pending { goal: goal.goal })
                        .map_err(|error| connection::client_error(error, &self.socket))?
                    else {
                        return Err(Failure::internal("hook pending response"));
                    };
                    pending
                };
                goals.push(GoalWork {
                    goal: goal.goal,
                    pending,
                });
            }
            Ok(Snapshot {
                instance: self.session.instance(),
                goals,
            })
        })
    }

    fn cancellation_target(&self, goal: GoalId, cancel: EventId) -> Result<EventId, Failure> {
        self.with_stream(|stream| {
            let mut client = self.open(stream)?;
            let Response::Event(detail) = client
                .call(Request::Event {
                    goal,
                    event: cancel,
                })
                .map_err(|error| connection::client_error(error, &self.socket))?
            else {
                return Err(Failure::internal("hook cancellation response"));
            };
            if detail.view.event != cancel || detail.view.standing != Standing::Effective {
                return Err(Failure::internal("hook cancellation is not effective"));
            }
            let Body::CancelRequested { attempt } = detail.body else {
                return Err(Failure::internal("hook event is not a cancellation"));
            };
            Ok(attempt)
        })
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

pub(crate) fn run(config: Config, event: Event) -> Result<Outcome, Failure> {
    if std::env::var_os("LOCUST_HOOKS").is_some_and(|value| value == "off") {
        return Ok(Outcome::default());
    }
    for path in [&config.home, &config.credential, &config.session] {
        if !path.is_absolute() {
            return Err(Failure::usage("hook paths must be absolute"));
        }
    }
    let credential = Credential(connection::read_secret(&config.credential)?);
    let session = SessionSecret(connection::read_secret(&config.session)?);
    let auth = Auth {
        socket: local::socket_path(&config.home)?,
        credential,
        session,
        deadline: Instant::now() + Duration::from_millis(u64::from(config.hook_timeout_ms)),
    };
    let saved = config
        .home
        .join("hook-marks")
        .join(locust_proto::id::BlobHash(credential.digest()).to_string())
        .join(session.instance().to_string())
        .join(format!("{}.json", crypto::content_hash(&config.chat)));
    let associated = fs::exists(&saved).map_err(io_failure)?;
    if matches!(
        &event,
        Event::Start | Event::Stop | Event::Tool { own_call: None }
    ) && !associated
    {
        return Ok(Outcome::default());
    }
    // First association requires live authentication. An existing protected
    // chat can retain a validated own write before reconnecting, so a daemon
    // outage cannot erase that exact explanation for a later claim loss.
    if !associated {
        auth.with_stream(|stream| auth.open(stream).map(|_| ()))?;
    }
    let resume = event == Event::Start;
    let directory = marks::Directory::open(&config.home, credential, session)?;
    let mut chat = directory.chat(&config.chat, auth.deadline)?;
    if let Event::Tool {
        own_call: Some(call),
    } = &event
    {
        // A native successful write is evidence even if the following read
        // fails. Persist it before polling so a later call cannot invent a loss
        // that this exact write already explains.
        core::observe(call, session.instance(), &mut chat.marks);
        chat.save()?;
    }
    // An acknowledgement can precede the hook's observation of its request.
    // Resolve only that cancellation's typed target, and keep the pending fact
    // if a read fails so a later callback retries before inferring claim loss.
    let goals = auth.goals()?;
    resolve_releases(&auth, &mut chat, &goals, resume)?;
    let observed = matches!(event, Event::Tool { .. }).then_some(&chat.marks);
    let snapshot = auth.snapshot(&goals, resume, observed)?;
    let outcome = core::decide(event, &snapshot, &mut chat.marks);
    chat.save()?;
    if !outcome.wait {
        return Ok(outcome);
    }
    drop(chat);
    let Some(_waiting) = directory.try_wait()? else {
        return Ok(Outcome::default());
    };
    let deadline =
        Instant::now() + Duration::from_millis(u64::from(config.wait_limit_ms.min(270_000)));
    let deadline = deadline.min(auth.deadline);
    let mut snapshot = snapshot;
    loop {
        if !auth.wait(&snapshot, deadline)? {
            return Ok(Outcome::default());
        }
        // Membership and halt can change while parked. Re-read them before
        // deciding whether any newly delivered pending work should block.
        let mut chat = directory.chat(&config.chat, auth.deadline)?;
        let goals = auth.goals()?;
        resolve_releases(&auth, &mut chat, &goals, false)?;
        snapshot = auth.snapshot(&goals, false, None)?;
        let outcome = core::decide(Event::Stop, &snapshot, &mut chat.marks);
        chat.save()?;
        if !outcome.wait {
            return Ok(outcome);
        }
    }
}

fn io_failure(error: std::io::Error) -> Failure {
    Failure::internal(format!("hook: {error}"))
}

fn eligible(goal: &GoalSummary, resume: bool) -> bool {
    goal.membership == Membership::Member && (resume || goal.halted.is_none())
}

fn resolve_releases(
    auth: &Auth,
    chat: &mut marks::Chat,
    goals: &[GoalSummary],
    resume: bool,
) -> Result<(), Failure> {
    let unresolved: BTreeSet<_> = chat
        .marks
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
        let attempt = auth.cancellation_target(goal, cancel)?;
        core::resolve_cancellation(&mut chat.marks, goal, cancel, attempt);
        chat.save()?;
    }
    Ok(())
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
