//! The responder and initiator alone: what each frame is answered with.

use locust_proto::PROTOCOL_VERSION;
use locust_proto::codec;
use locust_proto::event::{Event, WireEvent};
use locust_proto::id::{BlobHash, EventId, GoalId, PublicKey, Signature};
use locust_proto::limits::{
    BLOB_CHUNK_BYTES, MAX_EVENTS_PER_BATCH, MAX_HEADER_BYTES, MAX_PEER_FRAME_BYTES,
};
use locust_proto::store::Blob;
use locust_proto::sync::{AuthorFrontier, Frontier, Refusal, SyncMessage};
use locust_proto::testkit::Author;

use super::host::TestHost;
use super::{Founded, endpoint, host};
use crate::sync::batch::{Batch, encoded_len};
use crate::sync::{Ended, Initiator, Replica, Responder};

fn hello(goal: GoalId) -> SyncMessage {
    SyncMessage::Hello {
        version: PROTOCOL_VERSION,
        goal,
    }
}

/// Feeds `frames` to a responder for `remote`; returns it and every answer.
fn respond(
    host: &mut TestHost,
    remote: u8,
    frames: Vec<SyncMessage>,
) -> (Responder, Vec<SyncMessage>) {
    let mut responder = Responder::new(endpoint(remote));
    let mut out = Vec::new();
    for frame in frames {
        responder.receive(host, frame, 0, &mut out);
        loop {
            let before = out.len();
            responder.writable(host, &mut out);
            if out.len() == before {
                break;
            }
        }
    }
    (responder, out)
}

/// A member host at endpoint 1 whose goal holds the genesis and 600 notes
/// by author 2.
fn served() -> (Founded, TestHost, Vec<EventId>) {
    let founded = Founded::new();
    let notes = founded.notes(&mut Author::new(2), 600);
    let ids = notes.iter().map(|event| event.id()).collect();
    let host = host(1, founded.replica(&notes), &[1, 2]);
    (founded, host, ids)
}

#[test]
fn an_event_request_longer_than_a_batch_is_answered_with_one_frame_per_run() {
    let (founded, mut host, held) = served();
    // 600 identifiers, every fifth one unknown.
    let asked: Vec<EventId> = held
        .iter()
        .enumerate()
        .map(|(n, id)| if n % 5 == 0 { EventId([0xee; 32]) } else { *id })
        .collect();
    let (responder, out) = respond(
        &mut host,
        2,
        vec![
            hello(founded.goal),
            SyncMessage::EventRequest(asked.clone()),
        ],
    );
    assert!(!responder.is_finished());
    assert_eq!(out.len(), 3);
    for (frame, run) in out.iter().zip(asked.chunks(MAX_EVENTS_PER_BATCH)) {
        let SyncMessage::Events(events) = frame else {
            panic!("an Events frame per run, not {frame:?}");
        };
        let expected: Vec<EventId> = run
            .iter()
            .filter(|id| id.0 != [0xee; 32])
            .copied()
            .collect();
        assert_eq!(
            events.iter().map(WireEvent::id).collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn a_non_member_learns_nothing_even_about_whether_the_goal_exists() {
    let (founded, mut host, held) = served();
    let requests = [
        SyncMessage::Frontier(Frontier::default()),
        SyncMessage::InventoryRequest {
            author: Author::new(2).key.public(),
            after: None,
        },
        SyncMessage::EventRequest(held[..3].to_vec()),
        SyncMessage::KeyRequest { epoch: 0 },
        SyncMessage::BlobRequest {
            hash: BlobHash([1; 32]),
            offset: 0,
        },
        SyncMessage::Events(Vec::new()),
    ];
    for goal in [founded.goal, GoalId([0x77; 32])] {
        for request in requests.clone() {
            let (responder, out) = respond(&mut host, 9, vec![hello(goal), request]);
            assert_eq!(out, [SyncMessage::Refused(Refusal::NotAMember)]);
            assert!(!responder.is_admitted());
            assert_eq!(responder.ended(), Some(Ended::Refused(Refusal::NotAMember)));
        }
    }
    // A member asking about a goal this daemon does not hold is a stranger.
    let (_, out) = respond(
        &mut host,
        2,
        vec![
            hello(GoalId([0x77; 32])),
            SyncMessage::Frontier(Frontier::default()),
        ],
    );
    assert_eq!(out, [SyncMessage::Refused(Refusal::NotAMember)]);
}

#[test]
fn a_frame_that_breaks_the_grammar_is_refused_and_ends_the_exchange() {
    let (founded, mut host, _) = served();
    let goal = founded.goal;
    let broken = [
        vec![SyncMessage::Done],
        vec![SyncMessage::Frontier(Frontier::default())],
        vec![hello(goal), hello(goal)],
        vec![hello(goal), SyncMessage::BlobUnavailable(BlobHash([1; 32]))],
        vec![
            hello(goal),
            SyncMessage::Inventory {
                author: PublicKey([1; 32]),
                points: Vec::new(),
                more: false,
            },
        ],
    ];
    for frames in broken {
        let (responder, out) = respond(&mut host, 2, frames);
        assert_eq!(out, [SyncMessage::Refused(Refusal::ProtocolError)]);
        assert!(responder.is_finished());
    }
    // Frames after the end are ignored.
    let (_, out) = respond(
        &mut host,
        2,
        vec![
            hello(goal),
            SyncMessage::Done,
            SyncMessage::KeyRequest { epoch: 0 },
        ],
    );
    assert!(out.is_empty());
}

#[test]
fn another_protocol_version_is_refused() {
    let (founded, mut host, _) = served();
    let hello = SyncMessage::Hello {
        version: PROTOCOL_VERSION + 1,
        goal: founded.goal,
    };
    let (responder, out) = respond(&mut host, 2, vec![hello]);
    assert_eq!(out, [SyncMessage::Refused(Refusal::UnsupportedVersion)]);
    assert!(responder.is_finished());
}

#[test]
fn a_frontier_is_answered_with_full_batches_in_ascending_order_then_the_frontier() {
    let (founded, mut host, held) = served();
    let genesis_only = AuthorFrontier::from_points(
        founded.genesis.header().author,
        host.replica_mut(&founded.goal)
            .points(&founded.genesis.header().author),
    );
    let theirs = Frontier {
        authors: vec![genesis_only],
    };
    let (_, out) = respond(
        &mut host,
        2,
        vec![hello(founded.goal), SyncMessage::Frontier(theirs)],
    );
    let (last, batches) = out.split_last().unwrap();
    assert_eq!(
        last,
        &SyncMessage::Frontier(host.replica_mut(&founded.goal).frontier())
    );
    let sizes: Vec<usize> = batches
        .iter()
        .map(|frame| match frame {
            SyncMessage::Events(events) => events.len(),
            other => panic!("only events are missing, not {other:?}"),
        })
        .collect();
    assert_eq!(sizes, [256, 256, 88]);
    let sent: Vec<EventId> = batches
        .iter()
        .flat_map(|frame| match frame {
            SyncMessage::Events(events) => events.iter().map(WireEvent::id).collect(),
            _ => Vec::new(),
        })
        .collect();
    assert_eq!(sent, held);
}

/// A receiver without the governance log screens every other author's
/// events out, so a peer that holds nothing is given that log first,
/// whatever the keys' order; the other authors follow ascending.
#[test]
fn a_frontier_answer_starts_with_the_governance_keys_log() {
    let founded = Founded::new();
    let governance = founded.owner.key.public();
    let mut lower = (2..=40)
        .map(Author::new)
        .find(|author| author.key.public() < governance)
        .expect("some key sorts below the governance key");
    let mut higher = (2..=40)
        .map(Author::new)
        .find(|author| author.key.public() > governance)
        .expect("some key sorts above the governance key");
    let mut notes = founded.notes(&mut lower, 3);
    notes.extend(founded.notes(&mut higher, 2));
    let mut replica = founded.replica(&notes);
    replica.first_author = Some(governance);
    let mut host = host(1, replica, &[1, 2]);
    let (_, out) = respond(
        &mut host,
        2,
        vec![
            hello(founded.goal),
            SyncMessage::Frontier(Frontier::default()),
        ],
    );
    let authors: Vec<PublicKey> = out
        .iter()
        .filter_map(|frame| match frame {
            SyncMessage::Events(events) => {
                Some(Event::from_wire(&events[0]).unwrap().header().author)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        authors,
        [governance, lower.key.public(), higher.key.public()]
    );
    let frontier = host.replica_mut(&founded.goal).frontier();
    assert!(frontier.authors.is_sorted_by(|a, b| a.author < b.author));
    assert_eq!(out.last(), Some(&SyncMessage::Frontier(frontier)));
}

#[test]
fn a_batch_closes_a_frame_before_it_would_exceed_the_byte_limit() {
    let event = |len: usize| WireEvent {
        header: vec![7; len],
        signature: Signature([1; 64]),
    };
    for len in [0, 1, 127, 128, 16_383, 16_384, MAX_HEADER_BYTES] {
        let one = codec::encode(&SyncMessage::Events(vec![event(len)])).unwrap();
        let none = codec::encode(&SyncMessage::Events(Vec::new())).unwrap();
        assert_eq!(encoded_len(&event(len)), one.len() - none.len());
    }
    // Oversized headers make the byte limit bind before the count limit.
    let mut out = Vec::new();
    let mut batch = Batch::default();
    for _ in 0..MAX_EVENTS_PER_BATCH {
        batch.push(event(40_000), &mut out);
    }
    batch.flush(&mut out);
    assert!(out.len() > 1);
    let mut count = 0;
    for frame in &out {
        assert!(codec::encode(frame).unwrap().len() <= MAX_PEER_FRAME_BYTES);
        let SyncMessage::Events(events) = frame else {
            unreachable!()
        };
        count += events.len();
    }
    assert_eq!(count, MAX_EVENTS_PER_BATCH);
    // The frame before each split was as full as the limit allowed.
    let SyncMessage::Events(first) = &out[0] else {
        unreachable!()
    };
    let mut with_one_more = first.clone();
    with_one_more.push(event(40_000));
    let encoded = codec::encode(&SyncMessage::Events(with_one_more)).unwrap();
    assert!(encoded.len() > MAX_PEER_FRAME_BYTES);
}

#[test]
fn an_object_is_served_as_contiguous_chunks_from_the_asked_offset() {
    let (founded, mut host, _) = served();
    let object: Vec<u8> = (0..2 * BLOB_CHUNK_BYTES + 5).map(|n| n as u8).collect();
    let blob = Blob::new(object.clone());
    let hash = blob.hash();
    host.replica_mut(&founded.goal).add_blob(blob);
    let total = object.len() as u64;
    let ask = |host: &mut TestHost, offset: u64| {
        respond(
            host,
            2,
            vec![
                hello(founded.goal),
                SyncMessage::BlobRequest { hash, offset },
            ],
        )
        .1
    };

    let chunks = ask(&mut host, 7);
    let mut at = 7u64;
    for frame in &chunks {
        let SyncMessage::BlobChunk {
            hash: given,
            offset,
            total: whole,
            bytes,
        } = frame
        else {
            panic!("chunks only, not {frame:?}");
        };
        assert_eq!((*given, *offset, *whole), (hash, at, total));
        assert!(bytes.len() <= BLOB_CHUNK_BYTES);
        assert_eq!(
            bytes.as_slice(),
            &object[at as usize..at as usize + bytes.len()]
        );
        at += bytes.len() as u64;
    }
    assert_eq!((at, chunks.len()), (total, 2));

    assert_eq!(
        ask(&mut host, total),
        [SyncMessage::BlobChunk {
            hash,
            offset: total,
            total,
            bytes: Vec::new(),
        }]
    );
    assert_eq!(
        ask(&mut host, total + 1),
        [SyncMessage::BlobUnavailable(hash)]
    );
    let unknown = BlobHash([3; 32]);
    let (_, out) = respond(
        &mut host,
        2,
        vec![
            hello(founded.goal),
            SyncMessage::BlobRequest {
                hash: unknown,
                offset: 0,
            },
        ],
    );
    assert_eq!(out, [SyncMessage::BlobUnavailable(unknown)]);
}

#[test]
fn an_initiator_refuses_an_answer_out_of_order() {
    let (founded, mut host, _) = served();
    let replica = host.replica_mut(&founded.goal);
    let mut initiator = Initiator::new(founded.goal);
    let mut out = Vec::new();
    initiator.start(replica, &mut out);
    initiator.writable(replica, &mut out);
    initiator.writable(replica, &mut out);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], hello(founded.goal));
    out.clear();
    initiator.receive(replica, SyncMessage::KeyRequest { epoch: 0 }, &mut out);
    assert_eq!(out, [SyncMessage::Refused(Refusal::ProtocolError)]);
    initiator.writable(replica, &mut out);
    assert_eq!(
        initiator.ended(),
        Some(Ended::Refused(Refusal::ProtocolError))
    );
    out.clear();
    initiator.receive(replica, SyncMessage::Done, &mut out);
    assert!(out.is_empty());
}

#[test]
fn an_initiator_stops_at_a_refusal_without_answering_it() {
    let (founded, mut host, _) = served();
    let replica = host.replica_mut(&founded.goal);
    let mut initiator = Initiator::new(founded.goal);
    let mut out = Vec::new();
    initiator.start(replica, &mut out);
    initiator.writable(replica, &mut out);
    initiator.writable(replica, &mut out);
    out.clear();
    initiator.receive(replica, SyncMessage::Refused(Refusal::NotAMember), &mut out);
    assert!(out.is_empty());
    assert_eq!(initiator.ended(), Some(Ended::Refused(Refusal::NotAMember)));
}

#[test]
fn revocation_is_rechecked_for_every_request_and_between_chunks() {
    let (founded, mut host, _) = served();
    for request in [
        SyncMessage::Frontier(Frontier::default()),
        SyncMessage::InventoryRequest {
            author: PublicKey([2; 32]),
            after: None,
        },
        SyncMessage::BlobRequest {
            hash: BlobHash([3; 32]),
            offset: 0,
        },
    ] {
        host.members
            .get_mut(&founded.goal)
            .unwrap()
            .insert(endpoint(2));
        let mut responder = Responder::new(endpoint(2));
        let mut out = Vec::new();
        responder.receive(&mut host, hello(founded.goal), 0, &mut out);
        host.members
            .get_mut(&founded.goal)
            .unwrap()
            .remove(&endpoint(2));
        responder.receive(&mut host, request, 0, &mut out);
        assert_eq!(out, [SyncMessage::Refused(Refusal::NotAMember)]);
    }
    host.members
        .get_mut(&founded.goal)
        .unwrap()
        .insert(endpoint(2));
    let blob = Blob::new(vec![8; BLOB_CHUNK_BYTES * 3]);
    let hash = blob.hash();
    host.replica_mut(&founded.goal).add_blob(blob);
    let mut responder = Responder::new(endpoint(2));
    let mut out = Vec::new();
    responder.receive(&mut host, hello(founded.goal), 0, &mut out);
    responder.receive(
        &mut host,
        SyncMessage::BlobRequest { hash, offset: 0 },
        0,
        &mut out,
    );
    assert_eq!(out.len(), 1);
    host.members
        .get_mut(&founded.goal)
        .unwrap()
        .remove(&endpoint(2));
    out.clear();
    responder.writable(&mut host, &mut out);
    assert_eq!(out, [SyncMessage::Refused(Refusal::NotAMember)]);
}

#[test]
fn history_answers_materialize_only_one_frame_per_transport_credit() {
    let (founded, mut host, _) = served();
    let mut responder = Responder::new(endpoint(2));
    let mut out = Vec::new();
    responder.receive(&mut host, hello(founded.goal), 0, &mut out);
    responder.receive(
        &mut host,
        SyncMessage::Frontier(Frontier::default()),
        0,
        &mut out,
    );
    assert_eq!(out.len(), 1);
    out.clear();
    responder.writable(&mut host, &mut out);
    assert_eq!(out.len(), 1);
}

#[test]
fn early_refusal_discards_output_waiting_for_transport_capacity() {
    let (founded, mut host, _) = served();
    let replica = host.replica_mut(&founded.goal);
    let mut initiator = Initiator::new(founded.goal);
    let mut out = Vec::new();
    initiator.start(replica, &mut out);
    assert_eq!(out, [hello(founded.goal)]);
    out.clear();
    initiator.receive(replica, SyncMessage::Refused(Refusal::NotAMember), &mut out);
    initiator.writable(replica, &mut out);
    assert!(
        out.is_empty(),
        "queued frames must not follow a refusal: {out:?}"
    );
    assert_eq!(initiator.ended(), Some(Ended::Refused(Refusal::NotAMember)));
}

#[test]
fn peer_termination_discards_the_remaining_response_cursor() {
    for terminal in [
        SyncMessage::Done,
        SyncMessage::Refused(Refusal::ProtocolError),
    ] {
        let (founded, mut host, _) = served();
        let mut responder = Responder::new(endpoint(2));
        let mut out = Vec::new();
        responder.receive(&mut host, hello(founded.goal), 0, &mut out);
        responder.receive(
            &mut host,
            SyncMessage::Frontier(Frontier::default()),
            0,
            &mut out,
        );
        assert_eq!(out.len(), 1);
        out.clear();
        responder.receive(&mut host, terminal, 0, &mut out);
        responder.writable(&mut host, &mut out);
        assert!(
            out.is_empty(),
            "queued response after peer termination: {out:?}"
        );
        assert!(responder.ended().is_some());
    }
}

#[test]
fn empty_request_has_no_answer_and_response_blocks_further_intake() {
    let (founded, mut host, _) = served();
    let mut responder = Responder::new(endpoint(2));
    let mut out = Vec::new();
    responder.receive(&mut host, hello(founded.goal), 0, &mut out);
    responder.receive(&mut host, SyncMessage::EventRequest(vec![]), 0, &mut out);
    assert!(out.is_empty());
    assert!(responder.readable());
    responder.receive(
        &mut host,
        SyncMessage::Frontier(Frontier::default()),
        0,
        &mut out,
    );
    assert!(!responder.readable());
    let mut frames = out.len();
    while !responder.readable() {
        out.clear();
        responder.writable(&mut host, &mut out);
        assert!(out.len() <= 1);
        frames += out.len();
    }
    assert_eq!(frames, 5); // genesis, three note batches, terminal frontier
}

#[test]
fn joining_an_untrusted_endpoint_discloses_no_held_headers() {
    use locust_proto::invite::{InviteSecret, JoinRequest};
    let (founded, mut host, _) = served();
    let request = JoinRequest::sign(
        founded.goal,
        endpoint(1),
        "member".into(),
        InviteSecret([9; 32]),
        &Author::new(4).key,
    );
    let mut initiator = Initiator::joining(request);
    let replica = host.replica_mut(&founded.goal);
    let mut out = Vec::new();
    initiator.start(replica, &mut out);
    initiator.writable(replica, &mut out);
    initiator.writable(replica, &mut out);
    initiator.receive(
        replica,
        SyncMessage::Frontier(Frontier::default()),
        &mut out,
    );
    initiator.writable(replica, &mut out);
    assert!(
        out.iter().all(|frame| match frame {
            SyncMessage::Hello { .. } | SyncMessage::Join(_) | SyncMessage::Refused(_) => true,
            SyncMessage::Frontier(frontier) => frontier.authors.is_empty(),
            _ => false,
        }),
        "unexpected disclosure: {out:?}"
    );
}

#[test]
fn repeated_requests_cannot_accumulate_responses_behind_a_stalled_writer() {
    let (founded, mut host, held) = served();
    let mut responder = Responder::new(endpoint(2));
    let mut out = Vec::new();
    responder.receive(&mut host, hello(founded.goal), 0, &mut out);
    responder.receive(
        &mut host,
        SyncMessage::EventRequest(held.clone()),
        0,
        &mut out,
    );
    assert_eq!(out.len(), 1);
    for _ in 0..100 {
        responder.receive(
            &mut host,
            SyncMessage::EventRequest(held.clone()),
            0,
            &mut out,
        );
    }
    assert_eq!(out.len(), 1, "no transport capacity was released");
    out.clear();
    responder.writable(&mut host, &mut out);
    assert_eq!(out, [SyncMessage::Refused(Refusal::ProtocolError)]);
    responder.writable(&mut host, &mut out);
    assert_eq!(
        responder.ended(),
        Some(Ended::Refused(Refusal::ProtocolError))
    );
}
