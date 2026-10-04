//! `locust daemon run`: the daemon's shell.
//!
//! The shell owns the state directory, the socket, timers and the clock. It
//! is generic over [`Engine`] and knows nothing about goals or tasks: it
//! reads frames, hands them to the one thread that owns the engine, and
//! writes back what the engine answers.
//!
//! | Module | Does |
//! |---|---|
//! | [`home`] | State directory, lock, owner credential, socket file |
//! | [`shell`] | Accept loop and the clean stop |
//! | [`connection`] | One connection: hello, requests, a parked wait |
//! | [`frames`] | Framing with two reused buffers per connection |
//! | [`worker`] | The engine thread and the waits it holds |
//! | [`system`] | Clock and randomness passed into the engine |
//! | [`network`] | Peer connections, streams, deadlines and transport acknowledgements |

use std::future::Future;
use std::io;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use locust_core::node::Node;
use locust_net::Endpoint;
use locust_proto::engine::PeerInput;
#[cfg(test)]
use locust_proto::engine::{Engine, Entropy};
use locust_store::SqliteStore;
use tokio::signal::unix::{SignalKind, signal};

use crate::failure::Failure;
use crate::version;

mod connection;
mod frames;
pub(crate) mod home;
#[cfg(test)]
mod minimal;
mod network;
mod shell;
mod system;
mod worker;

#[cfg(test)]
mod durable_tests;
#[cfg(test)]
mod tests;

/// What the shell hands the function that builds the engine.
#[cfg(test)]
pub(crate) struct EngineInit {
    /// The state directory, already created and locked for this daemon.
    #[expect(
        dead_code,
        reason = "read by the engine that keeps its store there; the stand-in keeps nothing"
    )]
    pub(crate) home: PathBuf,
    /// Digest of the owner's credential, which the engine recognizes the
    /// owner's hello by.
    pub(crate) owner_digest: [u8; 32],
    /// The daemon's own version, as the hello and `status` report it.
    pub(crate) daemon_version: String,
    /// Randomness for keys, salts and secrets.
    pub(crate) entropy: Box<dyn Entropy>,
}

/// Runs the daemon on `home` in the foreground until SIGINT, SIGTERM or
/// `daemon.stop`.
pub fn run(home: &Path) -> Result<(), Failure> {
    run_networked_with(home, network::bind, |socket| {
        eprintln!(
            "locust: daemon {} listening on {}",
            version::daemon(),
            socket.display()
        );
        signals()
    })?;
    eprintln!("locust: daemon stopped");
    Ok(())
}

/// The production assembly, with injectable endpoint setup and shutdown for
/// local transport tests. The database and state machine are always real.
fn run_networked_with<B, BF, L, W>(home: &Path, bind: B, listening: L) -> Result<(), Failure>
where
    B: FnOnce([u8; 32]) -> BF,
    BF: Future<Output = Result<Endpoint, Failure>>,
    L: FnOnce(&Path) -> io::Result<W>,
    W: Future<Output = ()>,
{
    let mut state = home::StateDir::open(home)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Failure::internal(format!("the runtime could not start: {error}")))?;
    let daemon_version = version::daemon();
    let init_home = state.home().to_path_buf();
    let owner_digest = state.owner().digest();
    let init_version = daemon_version.clone();
    let mut engine = worker::EngineThread::start_networked(
        move || {
            let store = SqliteStore::open(&init_home).map_err(|error| error.to_string())?;
            Node::open(
                store,
                system::OsEntropy,
                owner_digest,
                init_version,
                system::now_ms(),
            )
            .map_err(|error| error.to_string())
        },
        system::now_ms,
    )
    .map_err(|reason| Failure::internal(format!("the engine could not start: {reason}")))?;
    let served = runtime.block_on(async {
        let endpoint = bind(
            engine
                .endpoint_secret
                .expect("networked engine has an endpoint secret"),
        )
        .await?;
        // Queue the endpoint before any local client can create a goal.
        engine
            .jobs
            .send(worker::Job::Peer(PeerInput::Endpoint {
                endpoint: endpoint.id(),
                hints: endpoint.hints(),
            }))
            .map_err(|_| Failure::internal("the engine stopped during endpoint startup"))?;
        let setup = (|| {
            let listener =
                tokio::net::UnixListener::from_std(state.listen()?).map_err(|error| {
                    Failure::internal(format!("the socket could not start: {error}"))
                })?;
            let shutdown = listening(state.socket()).map_err(|error| {
                Failure::internal(format!("shutdown handling could not start: {error}"))
            })?;
            Ok::<_, Failure>((listener, shutdown))
        })();
        let (listener, shutdown) = match setup {
            Ok(setup) => setup,
            Err(error) => {
                endpoint.close().await;
                return Err(error);
            }
        };
        let outgoing = engine
            .outgoing
            .take()
            .expect("networked engine has an output receiver");
        let peer_stop = engine.stop.clone();
        let network = async {
            network::serve(
                endpoint,
                engine.jobs.clone(),
                outgoing,
                engine.stop.subscribe(),
            )
            .await;
            // Unexpected endpoint termination also stops local acceptance.
            peer_stop.send_replace(true);
        };
        tokio::join!(
            shell::serve(listener, &engine, &daemon_version, shutdown),
            network
        );
        Ok(())
    });
    engine.shutdown();
    drop(runtime);
    drop(state);
    served
}

/// Completes on SIGINT or SIGTERM.
fn signals() -> io::Result<impl Future<Output = ()>> {
    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;
    Ok(async move {
        tokio::select! {
            _ = interrupt.recv() => {}
            _ = terminate.recv() => {}
        }
    })
}

/// Runs the daemon with the engine `make_engine` builds. `listening` is
/// called inside the runtime once the socket accepts connections, with the
/// socket's path, and returns the future whose completion stops the daemon
/// from outside.
///
/// Stopping is clean: nothing more is accepted, requests already handed to
/// the engine are answered, the engine is dropped, the socket file is
/// removed and the lock is released, in that order.
#[cfg(test)]
pub(crate) fn run_with<E, F, L, W>(home: &Path, make_engine: F, listening: L) -> Result<(), Failure>
where
    E: Engine + 'static,
    F: FnOnce(EngineInit) -> Result<E, String> + Send + 'static,
    L: FnOnce(&Path) -> io::Result<W>,
    W: Future<Output = ()>,
{
    let mut state = home::StateDir::open(home)?;
    let daemon_version = version::daemon();

    let init_home = state.home().to_path_buf();
    let owner_digest = state.owner().digest();
    let init_version = daemon_version.clone();
    let engine = worker::EngineThread::start(
        move || {
            make_engine(EngineInit {
                home: init_home,
                owner_digest,
                daemon_version: init_version,
                entropy: Box::new(system::OsEntropy),
            })
        },
        system::now_ms,
    )
    .map_err(|reason| Failure::internal(format!("the engine could not start: {reason}")))?;

    // Bound only now, so a daemon whose engine cannot start never answers.
    let listener = state.listen()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Failure::internal(format!("the runtime could not start: {error}")))?;
    let served = runtime.block_on(async {
        let listener = tokio::net::UnixListener::from_std(listener)?;
        let shutdown = listening(state.socket())?;
        shell::serve(listener, &engine, &daemon_version, shutdown).await;
        Ok::<(), io::Error>(())
    });
    engine.shutdown();
    drop(runtime);
    drop(state);
    served.map_err(|error| Failure::internal(format!("the daemon could not serve: {error}")))
}
