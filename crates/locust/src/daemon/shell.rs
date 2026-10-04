//! The local socket server: accepts connections until the daemon is told to
//! stop, then lets requests already handed to the engine finish.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use locust_proto::engine::ConnId;
use tokio::net::UnixListener;
use tokio::sync::mpsc;

use super::connection::{self, Shared};
use super::worker::EngineThread;

/// How long a stopping daemon waits for requests in flight to be answered
/// and written. A client that stopped reading cannot hold the daemon longer.
const DRAIN: Duration = Duration::from_secs(5);

/// Pause after a failed accept, so a persistent failure such as running out
/// of file descriptors does not spin.
const ACCEPT_BACKOFF: Duration = Duration::from_millis(50);

/// Serves `listener` until `shutdown` completes or the engine reports that
/// an owner asked the daemon to stop. On return nothing is accepted any
/// more and every connection has ended or was given up on after [`DRAIN`].
pub(crate) async fn serve(
    listener: UnixListener,
    engine: &EngineThread,
    daemon_version: &str,
    shutdown: impl Future<Output = ()>,
) {
    let (alive, mut all_ended) = mpsc::channel::<()>(1);
    let shared = Shared {
        jobs: engine.jobs.clone(),
        stop: engine.stop.subscribe(),
        daemon_version: Arc::from(daemon_version),
        _alive: alive,
    };
    let mut stop = engine.stop.subscribe();
    tokio::pin!(shutdown);
    // Connection numbers start at 1 and are never reused.
    let mut connections = 0u64;
    loop {
        tokio::select! {
            biased;
            () = connection::stopping(&mut stop) => break,
            () = &mut shutdown => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    connections += 1;
                    tokio::spawn(connection::serve(stream, ConnId(connections), shared.clone()));
                }
                Err(error) => {
                    super::log(format_args!("locust: accepting a connection failed: {error}"));
                    tokio::time::sleep(ACCEPT_BACKOFF).await;
                }
            },
        }
    }
    drop(listener);
    engine.stop.send_replace(true);
    // Each task holds a sender; the channel closes when the last one ends.
    drop(shared);
    let _ = tokio::time::timeout(DRAIN, all_ended.recv()).await;
}
