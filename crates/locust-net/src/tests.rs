use std::future::{Future, poll_fn};
use std::io;
use std::net::Ipv4Addr;
use std::task::Poll;
use std::time::Duration;

use iroh::SecretKey;
use iroh::endpoint::{NetReportConfig, PortmapperConfig, presets};
use locust_proto::codec::{self, CodecError};
use locust_proto::id::{BlobHash, GoalId};
use locust_proto::sync::{Refusal, SyncMessage};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;

const LIMITS: FrameLimits = FrameLimits {
    max_send_bytes: 1024,
    max_receive_bytes: 1024,
};
const LEFT: EndpointId = EndpointId([1; 32]);
const RIGHT: EndpointId = EndpointId([2; 32]);

fn raw_link(bytes: &[u8], maximum: u32) -> FramedLink<&[u8], tokio::io::Sink> {
    FramedLink::new(
        RIGHT,
        bytes,
        tokio::io::sink(),
        FrameLimits {
            max_receive_bytes: maximum,
            ..LIMITS
        },
        None,
    )
}

fn wire(message: &SyncMessage) -> Vec<u8> {
    let mut bytes = Vec::new();
    codec::write_frame(&mut bytes, &codec::encode(message).unwrap()).unwrap();
    bytes
}

#[tokio::test]
async fn memory_link_preserves_order_identity_and_half_close() {
    let (mut left, mut right) = memory_pair(LEFT, RIGHT, LIMITS, LIMITS, 4096).unwrap();
    assert_eq!(left.remote_id(), RIGHT);
    assert_eq!(right.remote_id(), LEFT);
    let hello = SyncMessage::Hello {
        version: 0,
        goal: GoalId([3; 32]),
    };
    left.send(&hello).await.unwrap();
    left.send(&SyncMessage::Done).await.unwrap();
    left.finish().await.unwrap();
    assert_eq!(right.recv().await.unwrap(), Some(hello));
    assert_eq!(right.recv().await.unwrap(), Some(SyncMessage::Done));
    assert_eq!(right.recv().await.unwrap(), None);
    assert_eq!(right.recv().await.unwrap(), None);
    right
        .send(&SyncMessage::Refused(Refusal::NotAMember))
        .await
        .unwrap();
    assert_eq!(
        left.recv().await.unwrap(),
        Some(SyncMessage::Refused(Refusal::NotAMember))
    );
    assert!(matches!(
        left.send(&SyncMessage::Done).await,
        Err(FrameError::Unusable)
    ));
}

#[tokio::test]
async fn asynchronous_frames_match_the_contract_codec() {
    let message = SyncMessage::Hello {
        version: 0,
        goal: GoalId([7; 32]),
    };
    let encoded = wire(&message);
    let mut reader = raw_link(&encoded, LIMITS.max_receive_bytes);
    assert_eq!(reader.recv().await.unwrap(), Some(message.clone()));
    let (writer, mut raw_reader) = tokio::io::duplex(4096);
    let mut link = FramedLink::new(RIGHT, tokio::io::empty(), writer, LIMITS, None);
    link.send(&message).await.unwrap();
    link.finish().await.unwrap();
    let mut actual = Vec::new();
    raw_reader.read_to_end(&mut actual).await.unwrap();
    assert_eq!(actual, encoded);
    assert_eq!(
        codec::read_frame(&mut actual.as_slice(), 1024).unwrap(),
        Some(codec::encode(&message).unwrap())
    );
}

#[tokio::test]
async fn prefix_admission_happens_before_reading_or_allocating_payload() {
    let bytes = u32::MAX.to_le_bytes();
    let mut link = raw_link(&bytes, 64);
    assert!(
        matches!(link.recv().await, Err(FrameError::TooLarge { length, maximum: 64 }) if length == u32::MAX as usize)
    );
    assert!(matches!(link.recv().await, Err(FrameError::Unusable)));
    // An explicit refusal can still be sent after rejecting incoming framing.
    link.send(&SyncMessage::Refused(Refusal::LimitExceeded))
        .await
        .unwrap();
}

#[tokio::test]
async fn outgoing_oversize_rejection_does_not_write_or_poison() {
    let limits = FrameLimits {
        max_send_bytes: 1,
        ..LIMITS
    };
    let (mut left, mut right) = memory_pair(LEFT, RIGHT, limits, LIMITS, 16).unwrap();
    let too_big = SyncMessage::Hello {
        version: 0,
        goal: GoalId([3; 32]),
    };
    assert!(matches!(
        left.send(&too_big).await,
        Err(FrameError::TooLarge { maximum: 1, .. })
    ));
    left.send(&SyncMessage::Done).await.unwrap();
    assert_eq!(right.recv().await.unwrap(), Some(SyncMessage::Done));
}

#[tokio::test]
async fn truncated_prefix_and_payload_are_not_clean_eof() {
    let complete = wire(&SyncMessage::Hello {
        version: 0,
        goal: GoalId([3; 32]),
    });
    for length in 1..complete.len() {
        let mut link = raw_link(&complete[..length], 1024);
        assert!(
            matches!(link.recv().await, Err(FrameError::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof),
            "truncation at byte {length}"
        );
        assert!(matches!(link.recv().await, Err(FrameError::Unusable)));
    }
    assert_eq!(raw_link(&[], 1024).recv().await.unwrap(), None);
}

#[tokio::test]
async fn invalid_encoding_and_trailing_bytes_poison_the_receiver() {
    for payload in [vec![], vec![255], {
        let mut bytes = codec::encode(&SyncMessage::Done).unwrap();
        bytes.push(0);
        bytes
    }] {
        let mut bytes = Vec::new();
        codec::write_frame(&mut bytes, &payload).unwrap();
        let mut link = raw_link(&bytes, 1024);
        assert!(matches!(
            link.recv().await,
            Err(FrameError::Codec(
                CodecError::Decode | CodecError::TrailingBytes
            ))
        ));
        assert!(matches!(link.recv().await, Err(FrameError::Unusable)));
    }
}

#[tokio::test]
async fn canceled_receive_resumes_at_each_prefix_and_payload_boundary() {
    let message = SyncMessage::Hello {
        version: 0,
        goal: GoalId([9; 32]),
    };
    let encoded = wire(&message);
    for split in 0..encoded.len() {
        let (mut raw, stream) = tokio::io::duplex(4096);
        let mut link = FramedLink::new(RIGHT, stream, tokio::io::sink(), LIMITS, None);
        raw.write_all(&encoded[..split]).await.unwrap();
        let mut pending = Box::pin(link.recv());
        assert!(
            poll_fn(|cx| Poll::Ready(pending.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        drop(pending);
        raw.write_all(&encoded[split..]).await.unwrap();
        assert_eq!(
            link.recv().await.unwrap(),
            Some(message.clone()),
            "resuming after byte {split}"
        );
    }
}

#[tokio::test]
async fn canceled_partial_send_cannot_append_another_frame() {
    let (mut left, _right) = memory_pair(LEFT, RIGHT, LIMITS, LIMITS, 1).unwrap();
    let message = SyncMessage::Done;
    let mut pending = Box::pin(left.send(&message));
    assert!(
        poll_fn(|cx| Poll::Ready(pending.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    drop(pending);
    assert!(matches!(
        left.send(&message).await,
        Err(FrameError::Unusable)
    ));
    assert!(matches!(left.finish().await, Err(FrameError::Unusable)));
}

#[tokio::test]
async fn split_halves_make_progress_under_backpressure() {
    let (left, right) = memory_pair(LEFT, RIGHT, LIMITS, LIMITS, 1).unwrap();
    let (mut left_send, mut left_recv) = left.into_split();
    let (mut right_send, mut right_recv) = right.into_split();
    assert_eq!(left_send.remote_id(), RIGHT);
    assert_eq!(left_recv.remote_id(), RIGHT);
    let message = SyncMessage::Blob {
        hash: BlobHash([3; 32]),
        bytes: vec![7; 200],
    };
    let (sent_left, received_right, sent_right, received_left) = tokio::join!(
        left_send.send(&message),
        right_recv.recv(),
        right_send.send(&message),
        left_recv.recv()
    );
    sent_left.unwrap();
    sent_right.unwrap();
    assert_eq!(received_right.unwrap(), Some(message.clone()));
    assert_eq!(received_left.unwrap(), Some(message));
}

#[test]
fn memory_capacity_is_explicit_and_nonzero() {
    assert_eq!(
        memory_pair(LEFT, RIGHT, LIMITS, LIMITS, 0)
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
}

fn local_builder(key: SecretKey) -> Builder {
    iroh::Endpoint::builder(presets::Minimal)
        .secret_key(key)
        .relay_mode(iroh::RelayMode::Disabled)
        .clear_address_lookup()
        .portmapper_config(PortmapperConfig::Disabled)
        .net_report_config(NetReportConfig::minimal())
        .clear_ip_transports()
        .bind_addr((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
}

#[tokio::test]
async fn loopback_iroh_authenticates_both_peers_and_carries_frames() {
    let left_key = SecretKey::from_bytes(&[11; 32]);
    let right_key = SecretKey::from_bytes(&[12; 32]);
    let left_id = EndpointId(*left_key.public().as_bytes());
    let right_id = EndpointId(*right_key.public().as_bytes());
    let left = Endpoint::bind(local_builder(left_key)).await.unwrap();
    let right = Endpoint::bind(local_builder(right_key)).await.unwrap();
    assert_eq!(left.id(), left_id);
    assert_eq!(right.id(), right_id);

    // A test watchdog, not an application timeout or retry policy. The endpoint
    // shutdown below also runs if this exchange reports an error or times out.
    let exchange = tokio::time::timeout(Duration::from_secs(15), async {
        let (outgoing, incoming) = tokio::join!(left.connect(right.addr()), async {
            right.accept().await.unwrap().accept().await
        });
        let outgoing = outgoing.unwrap();
        let incoming = incoming.unwrap();
        assert_eq!(outgoing.remote_id(), right_id);
        assert_eq!(incoming.remote_id(), left_id);
        let mut left_link = outgoing.open_link(LIMITS).await.unwrap();
        left_link.send(&SyncMessage::Done).await.unwrap();
        let mut right_link = incoming.accept_link(LIMITS).await.unwrap();
        assert_eq!(left_link.remote_id(), right_id);
        assert_eq!(right_link.remote_id(), left_id);
        // Link halves retain the connection after these handles are dropped.
        drop(outgoing);
        drop(incoming);
        assert_eq!(right_link.recv().await.unwrap(), Some(SyncMessage::Done));
        right_link
            .send(&SyncMessage::Refused(Refusal::NotAMember))
            .await
            .unwrap();
        right_link.finish().await.unwrap();
        assert_eq!(
            left_link.recv().await.unwrap(),
            Some(SyncMessage::Refused(Refusal::NotAMember))
        );
        assert_eq!(left_link.recv().await.unwrap(), None);
    })
    .await;
    tokio::join!(left.close(), right.close());
    exchange.expect("loopback exchange exceeded the test watchdog");
    assert!(left.accept().await.is_none());
    assert!(right.accept().await.is_none());
}
