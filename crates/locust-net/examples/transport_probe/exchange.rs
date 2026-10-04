//! Deliberately small transport fixture. It never joins or reads a real goal.

use locust_net::{FrameError, FrameReceiver, FramedLink};
use locust_proto::PROTOCOL_VERSION;
use locust_proto::id::GoalId;
use locust_proto::sync::Refusal;
use locust_proto::sync::SyncMessage;
use tokio::io::{AsyncRead, AsyncWrite};

use super::{Failure, Result};

pub async fn connect<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    link: &mut FramedLink<R, W>,
) -> Result<()> {
    // A separate ephemeral public key supplies an unpredictable 32-byte
    // challenge. It has no relation to a persisted goal or transport identity.
    let goal = GoalId(*iroh::SecretKey::generate().public().as_bytes());
    let hello = SyncMessage::Hello {
        version: PROTOCOL_VERSION,
        goal,
    };
    link.send(&hello).await.map_err(|_| Failure("send"))?;
    match link.recv().await.map_err(receive_failure)? {
        Some(SyncMessage::Hello {
            version,
            goal: echo,
        }) if version == PROTOCOL_VERSION && echo == goal => {}
        Some(SyncMessage::Hello { version, .. }) if version != PROTOCOL_VERSION => {
            return Err(Failure("unsupported_version"));
        }
        Some(SyncMessage::Hello { .. }) => return Err(Failure("challenge_mismatch")),
        _ => return Err(Failure("expected_hello")),
    }
    link.send(&SyncMessage::Done)
        .await
        .map_err(|_| Failure("send"))
}

pub async fn listen<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    link: &mut FramedLink<R, W>,
) -> Result<()> {
    let hello = match link.recv().await.map_err(receive_failure)? {
        Some(
            hello @ SyncMessage::Hello {
                version: PROTOCOL_VERSION,
                ..
            },
        ) => hello,
        Some(SyncMessage::Hello { .. }) => return Err(Failure("unsupported_version")),
        _ => return Err(Failure("expected_hello")),
    };
    link.send(&hello).await.map_err(|_| Failure("send"))?;
    if link.recv().await.map_err(receive_failure)? != Some(SyncMessage::Done) {
        return Err(Failure("expected_done"));
    }
    // Respond only after consuming the connector's completion message.
    link.send(&SyncMessage::Done)
        .await
        .map_err(|_| Failure("send"))
}

pub async fn completion<R: AsyncRead + Unpin>(
    receiver: &mut FrameReceiver<R>,
    connecting: bool,
) -> Result<()> {
    if connecting
        && receiver.recv().await.map_err(|_| Failure("receive"))? != Some(SyncMessage::Done)
    {
        return Err(Failure("expected_done"));
    }
    if receiver
        .recv()
        .await
        .map_err(|_| Failure("receive"))?
        .is_some()
    {
        return Err(Failure("expected_eof"));
    }
    Ok(())
}

fn receive_failure(error: FrameError) -> Failure {
    match error {
        FrameError::Protocol(Refusal::UnsupportedVersion) => Failure("unsupported_version"),
        _ => Failure("receive"),
    }
}

#[cfg(all(test, feature = "testkit"))]
mod tests {
    use locust_net::FrameLimits;
    use locust_net::testkit::{MemoryLink, memory_pair};
    use locust_proto::id::EndpointId;
    use locust_proto::limits::MAX_HELLO_FRAME_BYTES;

    use super::*;

    fn pair() -> (MemoryLink, MemoryLink) {
        let limits = FrameLimits {
            max_send_bytes: MAX_HELLO_FRAME_BYTES,
            max_receive_bytes: MAX_HELLO_FRAME_BYTES,
        };
        memory_pair(EndpointId([1; 32]), EndpointId([2; 32]), limits, limits, 64).unwrap()
    }

    async fn finish_memory(link: MemoryLink, connecting: bool) -> Result<()> {
        let (mut sender, mut receiver) = link.into_split();
        tokio::try_join!(
            async { sender.finish().await.map_err(|_| Failure("delivery")) },
            completion(&mut receiver, connecting),
        )?;
        Ok(())
    }

    #[tokio::test]
    async fn fixture_round_trip_mutually_consumes_done_and_eof() {
        let (mut connector, mut listener) = pair();
        let (connected, listened) = tokio::join!(
            async {
                connect(&mut connector).await?;
                finish_memory(connector, true).await
            },
            async {
                listen(&mut listener).await?;
                finish_memory(listener, false).await
            },
        );
        assert_eq!(connected, Ok(()));
        assert_eq!(listened, Ok(()));
    }

    #[tokio::test]
    async fn listener_rejects_unexpected_frames_and_versions() {
        for (message, expected) in [
            (SyncMessage::Done, Failure("expected_hello")),
            (
                SyncMessage::Hello {
                    version: PROTOCOL_VERSION + 1,
                    goal: GoalId([0; 32]),
                },
                Failure("unsupported_version"),
            ),
        ] {
            let (mut sender, mut listener) = pair();
            let (sent, result) = tokio::join!(sender.send(&message), listen(&mut listener));
            sent.unwrap();
            assert_eq!(result, Err(expected));
        }
    }

    #[tokio::test]
    async fn connector_rejects_wrong_echo() {
        let (mut connector, mut peer) = pair();
        let (result, _) = tokio::join!(connect(&mut connector), async {
            peer.recv().await.unwrap();
            peer.send(&SyncMessage::Hello {
                version: PROTOCOL_VERSION,
                goal: GoalId([0; 32]),
            })
            .await
            .unwrap();
        });
        assert_eq!(result, Err(Failure("challenge_mismatch")));
    }

    #[tokio::test]
    async fn completion_rejects_extra_frames_and_early_eof() {
        let (connector, mut peer) = pair();
        let (_, mut receiver) = connector.into_split();
        let (_, result) = tokio::join!(peer.finish(), completion(&mut receiver, true));
        assert_eq!(result, Err(Failure("expected_done")));

        let (connector, mut peer) = pair();
        let (_, mut receiver) = connector.into_split();
        let (_, result) = tokio::join!(
            peer.send(&SyncMessage::Done),
            completion(&mut receiver, false)
        );
        assert_eq!(result, Err(Failure("expected_eof")));
    }
}
