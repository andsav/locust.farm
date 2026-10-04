use std::future::{Future, poll_fn};
use std::io;
use std::net::Ipv4Addr;
use std::task::Poll;
use std::time::Duration;

use iroh::SecretKey;
use locust_proto::id::{BlobHash, GoalId};
use locust_proto::sync::{Refusal, SyncMessage};
use locust_proto::{PROTOCOL_VERSION, codec};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::testkit::memory_pair;
use super::*;

const LIMITS: FrameLimits = FrameLimits {
    max_send_bytes: 1024,
    max_receive_bytes: 1024,
};
const LEFT: EndpointId = EndpointId([1; 32]);
const RIGHT: EndpointId = EndpointId([2; 32]);

fn raw_link(bytes: &[u8], maximum: usize) -> FramedLink<&[u8], tokio::io::Sink> {
    FramedLink::new(
        RIGHT,
        bytes,
        tokio::io::sink(),
        FrameLimits {
            max_receive_bytes: maximum,
            ..LIMITS
        },
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
        version: PROTOCOL_VERSION,
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
        version: PROTOCOL_VERSION,
        goal: GoalId([7; 32]),
    };
    let encoded = wire(&message);
    let mut reader = raw_link(&encoded, LIMITS.max_receive_bytes);
    assert_eq!(reader.recv().await.unwrap(), Some(message.clone()));
    let (writer, mut raw_reader) = tokio::io::duplex(4096);
    let mut link = FramedLink::new(RIGHT, tokio::io::empty(), writer, LIMITS);
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
        version: PROTOCOL_VERSION,
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
        version: PROTOCOL_VERSION,
        goal: GoalId([3; 32]),
    });
    for length in 1..complete.len() {
        let mut link = raw_link(&complete[..length], 1024);
        assert!(
            matches!(link.recv().await, Err(FrameError::Truncated)),
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
            Err(FrameError::Protocol(Refusal::ProtocolError))
        ));
        assert!(matches!(link.recv().await, Err(FrameError::Unusable)));
    }
}

#[tokio::test]
async fn canceled_receive_resumes_at_each_prefix_and_payload_boundary() {
    let message = SyncMessage::Hello {
        version: PROTOCOL_VERSION,
        goal: GoalId([9; 32]),
    };
    let encoded = wire(&message);
    for split in 0..encoded.len() {
        let (mut raw, stream) = tokio::io::duplex(4096);
        let mut link = FramedLink::new(RIGHT, stream, tokio::io::sink(), LIMITS);
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
    let message = SyncMessage::BlobChunk {
        hash: BlobHash([3; 32]),
        offset: 0,
        total: 200,
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

fn local_config(key: SecretKey) -> EndpointConfig {
    EndpointConfig {
        lookup: crate::Lookup::DISABLED,
        secret_key: key.to_bytes(),
        relays: RelayConfig::Disabled,
        ip_transport: IpTransport::Bind((Ipv4Addr::LOCALHOST, 0).into()),
        port_mapping: false,
        budget: TransportBudget::default(),
    }
}

/// Explicit environment qualifications, separate from the isolated loopback suite.
#[tokio::test]
#[ignore = "requires local multicast networking; run explicitly for T1 qualification"]
async fn mdns_finds_a_peer_by_key_without_contact_hints() {
    let config = || EndpointConfig {
        lookup: Lookup {
            local_network: true,
            mainline: false,
        },
        ip_transport: IpTransport::Default,
        ..local_config(SecretKey::generate())
    };
    let left = Endpoint::bind(config()).await.unwrap();
    let right = Endpoint::bind(config()).await.unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(30),
        key_only_roundtrip(&left, &right, false),
    )
    .await;
    tokio::join!(left.close(), right.close());
    result
        .expect("mDNS qualification watchdog")
        .expect("mDNS key-only exchange");
}

#[tokio::test]
#[ignore = "uses public Mainline DHT and n0 relays; run explicitly for T1 qualification"]
async fn mainline_finds_a_peer_by_key_without_contact_hints() {
    let config = || EndpointConfig {
        lookup: Lookup {
            local_network: false,
            mainline: true,
        },
        relays: RelayConfig::N0,
        ip_transport: IpTransport::Default,
        ..local_config(SecretKey::generate())
    };
    let left = Endpoint::bind(config()).await.unwrap();
    let right = Endpoint::bind(config()).await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(120), async {
        tokio::join!(left.online(), right.online());
        key_only_roundtrip(&left, &right, true).await
    })
    .await;
    tokio::join!(left.close(), right.close());
    result
        .expect("Mainline qualification watchdog")
        .expect("Mainline key-only exchange");
}

async fn key_only_roundtrip(left: &Endpoint, right: &Endpoint, retry: bool) -> Result<(), String> {
    let (outgoing, incoming) = loop {
        match tokio::try_join!(left.connect(right.id(), &[]), async {
            right
                .accept()
                .await
                .ok_or(TransportError::Handshake)?
                .accept()
                .await
        }) {
            Ok(pair) => break pair,
            Err(_) if retry => tokio::time::sleep(Duration::from_secs(2)).await,
            Err(error) => return Err(error.to_string()),
        }
    };
    let mut sent = outgoing
        .open_link(FrameLimits::hello())
        .await
        .map_err(|e| e.to_string())?;
    let hello = SyncMessage::Hello {
        version: PROTOCOL_VERSION,
        goal: GoalId([83; 32]),
    };
    sent.send(&hello).await.map_err(|e| e.to_string())?;
    let mut received = incoming
        .accept_link(FrameLimits::hello())
        .await
        .map_err(|e| e.to_string())?;
    assert_eq!(
        received.recv().await.map_err(|e| e.to_string())?,
        Some(hello)
    );
    assert_eq!(outgoing.remote_id(), right.id());
    assert_eq!(incoming.remote_id(), left.id());
    eprintln!("key-only exchange paths: {:?}", outgoing.path_snapshot());
    Ok(())
}

#[tokio::test]
async fn online_without_relays_remains_pending_and_can_be_canceled() {
    let endpoint = Endpoint::bind(local_config(SecretKey::from_bytes(&[13; 32])))
        .await
        .unwrap();
    let mut online = Box::pin(endpoint.online());
    assert!(
        poll_fn(|cx| Poll::Ready(online.as_mut().poll(cx)))
            .await
            .is_pending(),
        "a bound direct-only endpoint must not count as relay-online"
    );
    drop(online);
    endpoint.close().await;
}

#[tokio::test]
async fn loopback_iroh_authenticates_both_peers_and_carries_frames() {
    let left_key = SecretKey::from_bytes(&[11; 32]);
    let right_key = SecretKey::from_bytes(&[12; 32]);
    let left_id = EndpointId(*left_key.public().as_bytes());
    let right_id = EndpointId(*right_key.public().as_bytes());
    let left = Endpoint::bind(local_config(left_key)).await.unwrap();
    let right = Endpoint::bind(local_config(right_key)).await.unwrap();
    assert_eq!(left.id(), left_id);
    assert_eq!(right.id(), right_id);

    // A test watchdog, not an application timeout or retry policy. The endpoint
    // shutdown below also runs if this exchange reports an error or times out.
    let exchange = tokio::time::timeout(Duration::from_secs(15), async {
        let hints = right.hints();
        let (outgoing, incoming) = tokio::join!(left.connect(right.id(), &hints), async {
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
        for connection in [&outgoing, &incoming] {
            let paths = connection.path_snapshot();
            assert!(!paths.is_empty());
            assert!(paths.iter().all(|path| path.kind == PathKind::Direct));
            assert_eq!(paths.iter().filter(|path| path.selected).count(), 1);
        }
        // Link halves retain the connection after these handles are dropped.
        drop(outgoing);
        drop(incoming);
        assert_eq!(right_link.recv().await.unwrap(), Some(SyncMessage::Done));
        right_link
            .send(&SyncMessage::Refused(Refusal::NotAMember))
            .await
            .unwrap();
        let (mut right_send, mut right_recv) = right_link.into_split();
        let (acknowledged, received_left) = tokio::join!(right_send.finish_acknowledged(), async {
            assert_eq!(
                left_link.recv().await.unwrap(),
                Some(SyncMessage::Refused(Refusal::NotAMember))
            );
            left_link.recv().await.unwrap()
        });
        acknowledged.unwrap();
        assert_eq!(received_left, None);
        assert!(matches!(
            right_send.send(&SyncMessage::Done).await,
            Err(FrameError::Unusable)
        ));
        let (acknowledged, received_right) =
            tokio::join!(left_link.finish_acknowledged(), right_recv.recv());
        acknowledged.unwrap();
        assert_eq!(received_right.unwrap(), None);
    })
    .await;
    tokio::join!(left.close(), right.close());
    exchange.expect("loopback exchange exceeded the test watchdog");
    assert!(left.accept().await.is_none());
    assert!(right.accept().await.is_none());
}

#[tokio::test]
async fn acknowledged_finish_reports_peer_stop_and_preserves_its_code() {
    let left = Endpoint::bind(local_config(SecretKey::from_bytes(&[14; 32])))
        .await
        .unwrap();
    let right = Endpoint::bind(local_config(SecretKey::from_bytes(&[15; 32])))
        .await
        .unwrap();
    let exchange = tokio::time::timeout(Duration::from_secs(15), async {
        let hints = right.hints();
        let (outgoing, incoming) = tokio::join!(left.connect(right.id(), &hints), async {
            right.accept().await.unwrap().accept().await
        });
        let outgoing = outgoing.unwrap();
        let incoming = incoming.unwrap();
        let mut link = outgoing.open_link(LIMITS).await.unwrap();
        link.send(&SyncMessage::Done).await.unwrap();
        let (_send, mut recv) = incoming.0.accept_bi().await.unwrap();
        let stop_code = 37u32.into();
        recv.stop(stop_code).unwrap();
        let error = link.finish_acknowledged().await.unwrap_err();
        assert!(matches!(error, FrameError::PeerReset { code: 37 }));
        assert!(matches!(
            link.finish_acknowledged().await,
            Err(FrameError::Unusable)
        ));
    })
    .await;
    tokio::join!(left.close(), right.close());
    exchange.expect("peer-stop exchange exceeded the test watchdog");
}

fn chunk(size: usize) -> SyncMessage {
    SyncMessage::BlobChunk {
        hash: BlobHash([5; 32]),
        offset: 0,
        total: size as u64,
        bytes: vec![6; size],
    }
}

#[tokio::test]
async fn admission_changes_only_at_frame_boundaries_including_partial_prefixes() {
    let message = chunk(5000);
    let encoded = wire(&message);
    // Raising admission cannot retroactively admit an already started prefix.
    let (mut raw, stream) = tokio::io::duplex(8192);
    let mut link = FramedLink::new(RIGHT, stream, tokio::io::sink(), FrameLimits::hello());
    raw.write_all(&encoded[..1]).await.unwrap();
    let mut pending = Box::pin(link.recv());
    assert!(
        poll_fn(|cx| Poll::Ready(pending.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    drop(pending);
    link.set_receive_limit(locust_proto::limits::MAX_PEER_FRAME_BYTES);
    raw.write_all(&encoded[1..4]).await.unwrap();
    assert!(matches!(
        link.recv().await,
        Err(FrameError::TooLarge { maximum: 4096, .. })
    ));

    // Lowering admission leaves the current admitted payload intact, then
    // rejects the next large prefix. No bytes of its payload are necessary.
    let (mut raw, stream) = tokio::io::duplex(8192);
    let mut link = FramedLink::new(RIGHT, stream, tokio::io::sink(), FrameLimits::peer());
    raw.write_all(&encoded[..17]).await.unwrap();
    let mut pending = Box::pin(link.recv());
    assert!(
        poll_fn(|cx| Poll::Ready(pending.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    drop(pending);
    link.set_receive_limit(locust_proto::limits::MAX_HELLO_FRAME_BYTES);
    raw.write_all(&encoded[17..]).await.unwrap();
    assert_eq!(link.recv().await.unwrap(), Some(message));
    raw.write_all(&encoded[..4]).await.unwrap();
    assert!(matches!(
        link.recv().await,
        Err(FrameError::TooLarge { maximum: 4096, .. })
    ));
}

#[tokio::test]
async fn sync_validation_rejects_versions_and_invalid_chunk_ranges() {
    for (message, expected) in [
        (
            SyncMessage::Hello {
                version: PROTOCOL_VERSION + 1,
                goal: GoalId([0; 32]),
            },
            Refusal::UnsupportedVersion,
        ),
        (
            SyncMessage::BlobChunk {
                hash: BlobHash([0; 32]),
                offset: 5,
                total: 4,
                bytes: Vec::new(),
            },
            Refusal::ProtocolError,
        ),
        (
            chunk(locust_proto::limits::BLOB_CHUNK_BYTES + 1),
            Refusal::LimitExceeded,
        ),
    ] {
        let encoded = wire(&message);
        let mut link = raw_link(&encoded, locust_proto::limits::MAX_PEER_FRAME_BYTES);
        assert!(
            matches!(link.recv().await, Err(FrameError::Protocol(refusal)) if refusal == expected)
        );
        assert!(matches!(link.recv().await, Err(FrameError::Unusable)));
    }
}

async fn half_lifecycle<R: tokio::io::AsyncRead + Unpin, W: tokio::io::AsyncWrite + Unpin>(
    left: FramedLink<R, W>,
    right: FramedLink<R, W>,
) {
    let (mut left_send, mut left_recv) = left.into_split();
    let (mut right_send, mut right_recv) = right.into_split();
    // Both directions continue independently under byte backpressure.
    let message = chunk(600);
    let (a, b, c, d) = tokio::join!(
        left_send.send(&message),
        right_recv.recv(),
        right_send.send(&message),
        left_recv.recv()
    );
    a.unwrap();
    c.unwrap();
    assert_eq!(b.unwrap(), Some(message.clone()));
    assert_eq!(d.unwrap(), Some(message));
    drop(left_send);
    assert_eq!(right_recv.recv().await.unwrap(), None);
    drop(left_recv);
    // QUIC stop notification is asynchronous; a queued write may precede it.
    // Both transports must eventually fail the writer instead of hanging.
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match right_send.send(&SyncMessage::Done).await {
                Ok(()) => tokio::task::yield_now().await,
                Err(FrameError::PeerReset { .. }) => break,
                error => panic!("dropped receiver must abort the writer: {error:?}"),
            }
        }
    })
    .await
    .expect("dropped receiver did not release writer");
}

#[tokio::test]
async fn memory_half_drop_and_backpressure_match_network_lifecycle() {
    let (left, right) = memory_pair(LEFT, RIGHT, LIMITS, LIMITS, 32).unwrap();
    half_lifecycle(left, right).await;
}

async fn connected(left: &Endpoint, right: &Endpoint) -> (PeerConnection, PeerConnection) {
    let hints = right.hints();
    let (outgoing, incoming) = tokio::join!(left.connect(right.id(), &hints), async {
        right.accept().await.unwrap().accept().await
    });
    (outgoing.unwrap(), incoming.unwrap())
}

#[tokio::test]
async fn loopback_half_drop_and_backpressure_match_memory_lifecycle() {
    let left = Endpoint::bind(local_config(SecretKey::from_bytes(&[21; 32])))
        .await
        .unwrap();
    let right = Endpoint::bind(local_config(SecretKey::from_bytes(&[22; 32])))
        .await
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        let (outgoing, incoming) = connected(&left, &right).await;
        let mut link = outgoing.open_link(LIMITS).await.unwrap();
        link.send(&SyncMessage::Hello {
            version: PROTOCOL_VERSION,
            goal: GoalId([1; 32]),
        })
        .await
        .unwrap();
        let mut remote = incoming.accept_link(LIMITS).await.unwrap();
        remote.recv().await.unwrap();
        half_lifecycle(link, remote).await;
    })
    .await;
    tokio::join!(left.close(), right.close());
    result.unwrap();
}

#[tokio::test]
async fn loopback_final_refusal_survives_acknowledged_finish_and_close() {
    let left = Endpoint::bind(local_config(SecretKey::from_bytes(&[23; 32])))
        .await
        .unwrap();
    let right = Endpoint::bind(local_config(SecretKey::from_bytes(&[24; 32])))
        .await
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        for _ in 0..5 {
            let (outgoing, incoming) = connected(&left, &right).await;
            let (request, response) = tokio::join!(
                async {
                    let mut link = outgoing.open_link(FrameLimits::hello()).await.unwrap();
                    link.send(&SyncMessage::Hello {
                        version: PROTOCOL_VERSION,
                        goal: GoalId([0; 32]),
                    })
                    .await
                    .unwrap();
                    assert_eq!(
                        link.recv().await.unwrap(),
                        Some(SyncMessage::Refused(Refusal::NotAMember))
                    );
                    assert_eq!(link.recv().await.unwrap(), None);
                },
                async {
                    let mut link = incoming.accept_link(FrameLimits::hello()).await.unwrap();
                    assert!(matches!(
                        link.recv().await.unwrap(),
                        Some(SyncMessage::Hello { .. })
                    ));
                    link.send(&SyncMessage::Refused(Refusal::NotAMember))
                        .await
                        .unwrap();
                    link.finish_acknowledged().await.unwrap();
                    drop(link);
                    incoming.close();
                    assert_eq!(incoming.closed().await, CloseReason::LocalClosed);
                }
            );
            let _ = (request, response);
            assert_eq!(outgoing.closed().await, CloseReason::PeerClosed);
        }
    })
    .await;
    tokio::join!(left.close(), right.close());
    result.unwrap();
}

#[tokio::test]
async fn loopback_admission_and_canceled_receive_resume_the_same_exchange() {
    let left = Endpoint::bind(local_config(SecretKey::from_bytes(&[25; 32])))
        .await
        .unwrap();
    let right = Endpoint::bind(local_config(SecretKey::from_bytes(&[26; 32])))
        .await
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        let (outgoing, incoming) = connected(&left, &right).await;
        let mut link = outgoing.open_link(FrameLimits::peer()).await.unwrap();
        let hello = SyncMessage::Hello {
            version: PROTOCOL_VERSION,
            goal: GoalId([0; 32]),
        };
        link.send(&hello).await.unwrap();
        let mut remote = incoming.accept_link(FrameLimits::hello()).await.unwrap();
        assert_eq!(remote.recv().await.unwrap(), Some(hello));
        assert!(
            tokio::time::timeout(Duration::from_millis(20), remote.recv())
                .await
                .is_err()
        );
        remote.set_receive_limit(locust_proto::limits::MAX_PEER_FRAME_BYTES);
        let message = chunk(5000);
        let (sent, received) = tokio::join!(link.send(&message), remote.recv());
        sent.unwrap();
        assert_eq!(received.unwrap(), Some(message));
        remote.set_receive_limit(locust_proto::limits::MAX_HELLO_FRAME_BYTES);
        link.send(&chunk(5000)).await.unwrap();
        assert!(matches!(
            remote.recv().await,
            Err(FrameError::TooLarge { maximum: 4096, .. })
        ));
        remote
            .send(&SyncMessage::Refused(Refusal::LimitExceeded))
            .await
            .unwrap();
        assert_eq!(
            link.recv().await.unwrap(),
            Some(SyncMessage::Refused(Refusal::LimitExceeded))
        );
    })
    .await;
    tokio::join!(left.close(), right.close());
    result.unwrap();
}

#[tokio::test]
async fn loopback_canceled_send_can_only_be_reset_not_reused() {
    let left = Endpoint::bind(local_config(SecretKey::from_bytes(&[27; 32])))
        .await
        .unwrap();
    let mut config = local_config(SecretKey::from_bytes(&[28; 32]));
    config.budget.stream_receive_bytes = 4096;
    config.budget.connection_receive_bytes = 8192;
    let right = Endpoint::bind(config).await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        let (outgoing, incoming) = connected(&left, &right).await;
        let mut link = outgoing.open_link(FrameLimits::peer()).await.unwrap();
        link.send(&SyncMessage::Done).await.unwrap();
        let mut remote = incoming.accept_link(FrameLimits::hello()).await.unwrap();
        assert_eq!(remote.recv().await.unwrap(), Some(SyncMessage::Done));
        let message = chunk(locust_proto::limits::BLOB_CHUNK_BYTES);
        // Fill transport credit and the finite local send queue. Cancellation
        // must leave no opportunity to append a frame behind partial bytes.
        assert!(
            tokio::time::timeout(Duration::from_millis(100), async {
                loop {
                    link.send(&message).await.unwrap();
                }
            })
            .await
            .is_err()
        );
        assert!(matches!(
            link.send(&SyncMessage::Done).await,
            Err(FrameError::Unusable)
        ));
        link.abort_send().unwrap();
        assert!(matches!(
            remote.recv().await,
            Err(FrameError::PeerReset { code: 0 }) | Err(FrameError::TooLarge { .. })
        ));
    })
    .await;
    tokio::join!(left.close(), right.close());
    result.unwrap();
}

#[tokio::test]
async fn contact_wait_is_cancelable_and_terminates_on_local_close() {
    let endpoint = Endpoint::bind(local_config(SecretKey::from_bytes(&[29; 32])))
        .await
        .unwrap();
    let hints = endpoint.hints();
    assert!(
        tokio::time::timeout(Duration::from_millis(20), endpoint.hints_changed(&hints))
            .await
            .is_err()
    );
    endpoint.close().await;
    assert_eq!(endpoint.hints_changed(&endpoint.hints()).await, None);
    assert_eq!(endpoint.hints_changed(&[]).await, None);
}

#[test]
fn endpoint_configuration_redacts_identity_and_rejects_secret_relays() {
    let config = local_config(SecretKey::from_bytes(&[77; 32]));
    let debug = format!("{config:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("77, 77"));
    for url in [
        "https://user:secret@example.com",
        "https://example.com?token=secret",
        "http://example.com",
        "https://example.com#secret",
    ] {
        assert!(relay_url(url).is_none());
    }
}

#[tokio::test]
async fn loopback_unaccepted_stream_flood_respects_quic_budget() {
    let left = Endpoint::bind(local_config(SecretKey::from_bytes(&[30; 32])))
        .await
        .unwrap();
    let mut config = local_config(SecretKey::from_bytes(&[31; 32]));
    // A smaller operator budget makes flow control observable cheaply. This
    // is an explicit resource budget, not an operation deadline.
    config.budget = TransportBudget {
        max_bidirectional_streams: 2,
        stream_receive_bytes: 128 * 1024,
        connection_receive_bytes: 192 * 1024,
    };
    let right = Endpoint::bind(config).await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        let (outgoing, incoming) = connected(&left, &right).await;
        assert_eq!(outgoing.0.max_datagram_size(), None);
        assert!(
            tokio::time::timeout(Duration::from_millis(20), outgoing.0.open_uni())
                .await
                .is_err()
        );
        let (mut first, _first_read) = outgoing.0.open_bi().await.unwrap();
        let (mut second, _second_read) = outgoing.0.open_bi().await.unwrap();
        first.write_all(&[0]).await.unwrap();
        second.write_all(&[0]).await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(20), outgoing.0.open_bi())
                .await
                .is_err()
        );
        // The daemon deliberately never accepts these streams. Delivering
        // beyond the connection's byte credit cannot be acknowledged. A FIN
        // ACK would establish all bytes had been received, so test that real
        // transport guarantee rather than inspecting config fields.
        let bytes = vec![0; 128 * 1024 - 1];
        let (first_blocked, second_blocked) = tokio::join!(
            tokio::time::timeout(Duration::from_millis(100), async {
                first.write_all(&bytes).await.unwrap();
                first.finish().unwrap();
                first.stopped().await.unwrap()
            }),
            tokio::time::timeout(Duration::from_millis(100), async {
                second.write_all(&bytes).await.unwrap();
                second.finish().unwrap();
                second.stopped().await.unwrap()
            }),
        );
        assert!(first_blocked.is_err() || second_blocked.is_err(), "the aggregate byte budget must block at least one FIN despite each stream fitting its own window");
        outgoing.close();
        assert_eq!(incoming.closed().await, CloseReason::PeerClosed);
    })
    .await;
    tokio::join!(left.close(), right.close());
    result.unwrap();
}

#[tokio::test]
async fn loopback_stream_errors_distinguish_abort_peer_close_and_local_close() {
    let left = Endpoint::bind(local_config(SecretKey::from_bytes(&[32; 32])))
        .await
        .unwrap();
    let right = Endpoint::bind(local_config(SecretKey::from_bytes(&[33; 32])))
        .await
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        let (outgoing, incoming) = connected(&left, &right).await;
        let mut link = outgoing.open_link(FrameLimits::hello()).await.unwrap();
        link.send(&SyncMessage::Done).await.unwrap();
        let mut remote = incoming.accept_link(FrameLimits::hello()).await.unwrap();
        assert_eq!(remote.recv().await.unwrap(), Some(SyncMessage::Done));
        link.abort_send().unwrap();
        assert!(matches!(
            remote.recv().await,
            Err(FrameError::PeerReset { code: 0 })
        ));

        let mut link = outgoing.open_link(FrameLimits::hello()).await.unwrap();
        link.send(&SyncMessage::Done).await.unwrap();
        let mut remote = incoming.accept_link(FrameLimits::hello()).await.unwrap();
        remote.recv().await.unwrap();
        outgoing.close();
        assert!(matches!(
            remote.recv().await,
            Err(FrameError::ConnectionLost(CloseReason::PeerClosed))
        ));
        assert!(matches!(link.recv().await, Err(FrameError::LocalClosed)));
        assert_eq!(outgoing.closed().await, CloseReason::LocalClosed);
        assert_eq!(incoming.closed().await, CloseReason::PeerClosed);
    })
    .await;
    tokio::join!(left.close(), right.close());
    result.unwrap();
}

#[tokio::test]
async fn loopback_canceled_receive_keeps_partial_payload_and_prefix_admission() {
    let left = Endpoint::bind(local_config(SecretKey::from_bytes(&[34; 32])))
        .await
        .unwrap();
    let right = Endpoint::bind(local_config(SecretKey::from_bytes(&[35; 32])))
        .await
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        let (outgoing, incoming) = connected(&left, &right).await;
        let (mut raw, _recv) = outgoing.0.open_bi().await.unwrap();
        let message = SyncMessage::Hello {
            version: PROTOCOL_VERSION,
            goal: GoalId([3; 32]),
        };
        let encoded = wire(&message);
        raw.write_all(&encoded[..17]).await.unwrap();
        let mut remote = incoming.accept_link(FrameLimits::hello()).await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(20), remote.recv())
                .await
                .is_err()
        );
        raw.write_all(&encoded[17..]).await.unwrap();
        assert_eq!(remote.recv().await.unwrap(), Some(message));

        let (mut raw, _recv) = outgoing.0.open_bi().await.unwrap();
        let encoded = wire(&chunk(5000));
        raw.write_all(&encoded[..1]).await.unwrap();
        let mut remote = incoming.accept_link(FrameLimits::hello()).await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(20), remote.recv())
                .await
                .is_err()
        );
        remote.set_receive_limit(locust_proto::limits::MAX_PEER_FRAME_BYTES);
        raw.write_all(&encoded[1..4]).await.unwrap();
        assert!(matches!(
            remote.recv().await,
            Err(FrameError::TooLarge { maximum: 4096, .. })
        ));
    })
    .await;
    tokio::join!(left.close(), right.close());
    result.unwrap();
}

#[tokio::test]
async fn canceled_frame_debug_never_shows_wire_payloads() {
    let marker = b"private_wire_payload_marker";
    let message = SyncMessage::BlobChunk {
        hash: BlobHash([0; 32]),
        offset: 0,
        total: marker.len() as u64,
        bytes: marker.to_vec(),
    };
    let (mut left, _right) = memory_pair(LEFT, RIGHT, LIMITS, LIMITS, 1).unwrap();
    let mut sending = Box::pin(left.send(&message));
    assert!(
        poll_fn(|cx| Poll::Ready(sending.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    drop(sending);
    let debug = format!("{left:?}");
    assert!(!debug.contains("private_wire_payload_marker"));
    assert!(!debug.contains("112, 114, 105, 118, 97"));

    let encoded = wire(&message);
    let (mut raw, stream) = tokio::io::duplex(4096);
    let mut link = FramedLink::new(RIGHT, stream, tokio::io::sink(), LIMITS);
    raw.write_all(&encoded[..encoded.len() - 1]).await.unwrap();
    let mut receiving = Box::pin(link.recv());
    assert!(
        poll_fn(|cx| Poll::Ready(receiving.as_mut().poll(cx)))
            .await
            .is_pending()
    );
    drop(receiving);
    let debug = format!("{link:?}");
    assert!(!debug.contains("112, 114, 105, 118, 97"));
}
