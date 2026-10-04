//! One local connection: a hello, then requests answered in order.
//!
//! The task does only I/O. It decodes each frame, hands it to the engine
//! thread and writes back what the engine answers. A frame that cannot be
//! read or decoded has no request to answer under, so it ends this
//! connection and no other.

use std::io;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use locust_proto::API_VERSION;
use locust_proto::api::{
    ApiError, ClientHello, ErrorCode, RequestFrame, ResponseFrame, ServerHello,
};
use locust_proto::codec;
use locust_proto::engine::ConnId;
use locust_proto::limits::{MAX_HELLO_FRAME_BYTES, MAX_LOCAL_FRAME_BYTES};
use tokio::net::UnixStream;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::sync::watch;
use tokio::time::Instant;

use super::frames::Frames;
use super::worker::{Answer, Job};

/// What every connection task shares with the shell.
#[derive(Clone)]
pub(crate) struct Shared {
    /// The engine thread's inbox.
    pub(crate) jobs: Sender<Job>,
    /// True once the daemon is stopping.
    pub(crate) stop: watch::Receiver<bool>,
    /// The daemon's own version, for a hello the shell refuses itself.
    pub(crate) daemon_version: Arc<str>,
    /// Dropped when the task ends, which is how the shell learns that every
    /// connection has finished.
    pub(crate) _alive: mpsc::Sender<()>,
}

/// Completes once the daemon is stopping.
pub(crate) async fn stopping(stop: &mut watch::Receiver<bool>) {
    // An error means the shell itself is gone, which is a stop too.
    let _ = stop.wait_for(|stop| *stop).await;
}

/// Serves one accepted connection until it ends.
pub(crate) async fn serve(stream: UnixStream, conn: ConnId, mut shared: Shared) {
    let mut frames = Frames::new(stream);
    let Some(mut answers) = greet(&mut frames, conn, &mut shared).await else {
        return;
    };
    answer_requests(&mut frames, conn, &mut shared, &mut answers).await;
    let _ = shared.jobs.send(Job::Disconnect { conn });
}

/// Reads the hello and answers it. Returns the channel the engine thread
/// answers this connection on once the engine welcomed it and the welcome
/// was written; from then on the engine must be told when the connection
/// ends.
async fn greet(
    frames: &mut Frames,
    conn: ConnId,
    shared: &mut Shared,
) -> Option<UnboundedReceiver<Answer>> {
    let hello = tokio::select! {
        biased;
        () = stopping(&mut shared.stop) => return None,
        frame = frames.read(MAX_HELLO_FRAME_BYTES) => match frame {
            Ok(Some(bytes)) => ClientHello::decode(bytes),
            Ok(None) => return None,
            Err(error) if error.kind() == io::ErrorKind::InvalidData => Err(ApiError::new(
                ErrorCode::Invalid,
                "hello frame is too large",
            )),
            Err(_) => return None,
        },
    };
    let hello = match hello {
        Ok(hello) => hello,
        Err(error) => {
            // The engine never sees a hello it could not read.
            let refusal = ServerHello::Refused {
                error,
                api_version: API_VERSION,
                daemon_version: shared.daemon_version.to_string(),
            };
            let _ = frames.write(&refusal).await;
            return None;
        }
    };

    let (sender, mut answers) = mpsc::unbounded_channel();
    let connect = Job::Connect {
        conn,
        hello,
        answers: sender,
    };
    shared.jobs.send(connect).ok()?;
    let Some(Answer::Hello(answer)) = answers.recv().await else {
        return None;
    };
    let welcomed = matches!(answer, ServerHello::Welcome { .. });
    let written = frames.write(&answer).await.is_ok();
    if welcomed && !written {
        let _ = shared.jobs.send(Job::Disconnect { conn });
    }
    (welcomed && written).then_some(answers)
}

async fn answer_requests(
    frames: &mut Frames,
    conn: ConnId,
    shared: &mut Shared,
    answers: &mut UnboundedReceiver<Answer>,
) {
    loop {
        let frame: RequestFrame = tokio::select! {
            biased;
            () = stopping(&mut shared.stop) => return,
            frame = frames.read(MAX_LOCAL_FRAME_BYTES) => match frame {
                Ok(Some(bytes)) => match codec::decode(bytes) {
                    Ok(frame) => frame,
                    Err(_) => return,
                },
                Ok(None) | Err(_) => return,
            },
        };
        let arrived = Instant::now();
        let request_id = frame.id;
        if shared.jobs.send(Job::Request { conn, frame }).is_err() {
            return;
        }
        // The request is in flight: it is answered even if the daemon is
        // stopping, unless the engine parks it.
        let reply = match answers.recv().await {
            Some(Answer::Reply(reply)) => reply,
            Some(Answer::Parked { timeout_ms }) => {
                let deadline = arrived + Duration::from_millis(u64::from(timeout_ms));
                match parked(frames, conn, request_id, deadline, shared, answers).await {
                    Some(reply) => reply,
                    None => return,
                }
            }
            Some(Answer::Hello(_)) | None => return,
        };
        if write_reply(frames, &reply).await.is_err() {
            return;
        }
    }
}

/// Holds a parked request until the engine answers it. Tells the engine
/// thread when the request's time is up. Returns `None`, which ends the
/// connection, when the peer closes or the daemon stops first.
async fn parked(
    frames: &mut Frames,
    conn: ConnId,
    request_id: u64,
    deadline: Instant,
    shared: &mut Shared,
    answers: &mut UnboundedReceiver<Answer>,
) -> Option<ResponseFrame> {
    let timeout = tokio::time::sleep_until(deadline);
    tokio::pin!(timeout);
    let mut expired = false;
    // Until the peer sends something ahead of its answer, a read is how its
    // closing is noticed.
    let mut watching = true;
    loop {
        tokio::select! {
            answer = answers.recv() => {
                return match answer {
                    Some(Answer::Reply(reply)) => Some(reply),
                    _ => None,
                };
            }
            () = &mut timeout, if !expired => {
                expired = true;
                shared.jobs.send(Job::Expire { conn, request_id }).ok()?;
            }
            stays = frames.peer_stays(), if watching => {
                if !stays {
                    return None;
                }
                watching = false;
            }
            () = stopping(&mut shared.stop) => return None,
        }
    }
}

/// Writes a reply. One that would not fit the frame the client admits is
/// replaced by an error the client can read.
async fn write_reply(frames: &mut Frames, reply: &ResponseFrame) -> io::Result<()> {
    if frames.encode(reply)? > MAX_LOCAL_FRAME_BYTES {
        frames.encode(&ResponseFrame {
            id: reply.id,
            result: Err(ApiError::new(
                ErrorCode::LimitExceeded,
                "the answer is too large for one frame",
            )),
        })?;
    }
    frames.flush().await
}
