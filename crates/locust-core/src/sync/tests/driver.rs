//! Completion is a transport-confirmed outcome, not an emitted intention.
use super::{Founded, endpoint, host};
use crate::sync::{Driver, Ended, Replica};
use locust_proto::engine::{ExchangeId, PeerInput, PeerOutput};
use locust_proto::sync::SyncMessage;

#[test]
fn finish_remains_in_flight_until_confirmed_and_failed_finish_backs_off() {
    for success in [false, true] {
        let founded = Founded::new();
        let mut host = host(1, founded.replica(&[]), &[1, 2]);
        let mut driver = Driver::new();
        let mut out = Vec::new();
        driver.handle(
            &mut host,
            PeerInput::Poll,
            locust_proto::engine::PeerTime {
                unix_ms: 0,
                elapsed_ms: 0,
            },
            &mut out,
        );
        let PeerOutput::Open { exchange, .. } = out.remove(0) else {
            panic!("open");
        };
        assert!(matches!(exchange, ExchangeId::Dialed(_)));
        driver.handle(
            &mut host,
            PeerInput::Opened(exchange),
            locust_proto::engine::PeerTime {
                unix_ms: 1,
                elapsed_ms: 1,
            },
            &mut out,
        );
        driver.handle(
            &mut host,
            PeerInput::Writable(exchange),
            locust_proto::engine::PeerTime {
                unix_ms: 2,
                elapsed_ms: 2,
            },
            &mut out,
        );
        driver.handle(
            &mut host,
            PeerInput::Writable(exchange),
            locust_proto::engine::PeerTime {
                unix_ms: 3,
                elapsed_ms: 3,
            },
            &mut out,
        );
        let frontier = host.replica_mut(&founded.goal).frontier();
        driver.handle(
            &mut host,
            PeerInput::Frame {
                exchange,
                frame: SyncMessage::Frontier(frontier),
            },
            locust_proto::engine::PeerTime {
                unix_ms: 4,
                elapsed_ms: 4,
            },
            &mut out,
        );
        driver.handle(
            &mut host,
            PeerInput::Writable(exchange),
            locust_proto::engine::PeerTime {
                unix_ms: 5,
                elapsed_ms: 5,
            },
            &mut out,
        );
        assert!(
            out.iter()
                .any(|output| matches!(output, PeerOutput::Finish(id) if *id == exchange))
        );
        assert!(host.reports.is_empty());
        assert!(driver.in_flight(&founded.goal, &endpoint(2)));
        let input = if success {
            PeerInput::Finished(exchange)
        } else {
            PeerInput::Closed(exchange)
        };
        driver.handle(
            &mut host,
            input,
            locust_proto::engine::PeerTime {
                unix_ms: 6,
                elapsed_ms: 6,
            },
            &mut out,
        );
        assert_eq!(host.reports.len(), 1);
        assert_eq!(
            host.reports[0].ended,
            if success {
                Ended::Completed
            } else {
                Ended::Aborted
            }
        );
        assert!(!driver.in_flight(&founded.goal, &endpoint(2)));
        out.clear();
        driver.handle(
            &mut host,
            PeerInput::Poll,
            locust_proto::engine::PeerTime {
                unix_ms: 7,
                elapsed_ms: 7,
            },
            &mut out,
        );
        assert!(out.is_empty());
        if !success {
            driver.handle(
                &mut host,
                PeerInput::Poll,
                locust_proto::engine::PeerTime {
                    unix_ms: 1006,
                    elapsed_ms: 1006,
                },
                &mut out,
            );
            assert!(matches!(out.as_slice(), [PeerOutput::Open { .. }]));
        }
    }
}

/// Two daemons whose exchanges failed together retry apart: each wait is cut
/// short by a part of up to half drawn from its host, so a bad order of
/// events does not repeat on the same schedule for ever.
#[test]
fn failed_exchanges_retry_at_jittered_times() {
    let founded = Founded::new();
    let mut retried = Vec::new();
    // The first wait is one second, and may lose up to 500 ms of it.
    for draw in [0, 300, 1_700] {
        let mut host = host(1, founded.replica(&[]), &[1, 2]);
        host.random = vec![draw];
        let mut driver = Driver::new();
        let mut out = Vec::new();
        driver.handle(
            &mut host,
            PeerInput::Poll,
            locust_proto::engine::PeerTime {
                unix_ms: 0,
                elapsed_ms: 0,
            },
            &mut out,
        );
        let [PeerOutput::Open { exchange, .. }] = out.as_slice() else {
            panic!("open");
        };
        let exchange = *exchange;
        out.clear();
        driver.handle(
            &mut host,
            PeerInput::OpenFailed(exchange),
            locust_proto::engine::PeerTime {
                unix_ms: 6,
                elapsed_ms: 6,
            },
            &mut out,
        );
        retried.push((7..=1_006).find(|&now| {
            driver.handle(
                &mut host,
                PeerInput::Poll,
                locust_proto::engine::PeerTime {
                    unix_ms: now,
                    elapsed_ms: now,
                },
                &mut out,
            );
            !out.is_empty()
        }));
    }
    assert_eq!(retried, [Some(1_006), Some(706), Some(806)]);
}

#[test]
fn wall_clock_steps_do_not_delay_retries_or_advance_anti_entropy() {
    use locust_proto::engine::PeerTime;
    for wall in [0, 9_000_000] {
        let founded = Founded::new();
        let mut host = host(1, founded.replica(&[]), &[1, 2]);
        let mut driver = Driver::new();
        let mut out = Vec::new();
        let time = |unix_ms, elapsed_ms| PeerTime {
            unix_ms,
            elapsed_ms,
        };
        driver.handle(&mut host, PeerInput::Poll, time(1_000_000, 100), &mut out);
        let PeerOutput::Open { exchange, .. } = out.remove(0) else {
            panic!("open")
        };
        driver.handle(
            &mut host,
            PeerInput::OpenFailed(exchange),
            time(1_000_000, 100),
            &mut out,
        );
        assert_eq!(host.reports.last().unwrap().at_ms, 1_000_000);
        driver.handle(&mut host, PeerInput::Poll, time(wall, 1_099), &mut out);
        assert!(out.is_empty(), "wall step bypassed the retry delay");
        driver.handle(&mut host, PeerInput::Poll, time(wall, 1_100), &mut out);
        let PeerOutput::Open { exchange, .. } = out.remove(0) else {
            panic!("retry on elapsed time")
        };
        // Complete a real exchange, so its periodic deadline is exercised.
        driver.handle(
            &mut host,
            PeerInput::Opened(exchange),
            time(wall, 1_100),
            &mut out,
        );
        for _ in 0..2 {
            driver.handle(
                &mut host,
                PeerInput::Writable(exchange),
                time(wall, 1_100),
                &mut out,
            );
        }
        let frontier = host.replica_mut(&founded.goal).frontier();
        driver.handle(
            &mut host,
            PeerInput::Frame {
                exchange,
                frame: SyncMessage::Frontier(frontier),
            },
            time(wall, 1_100),
            &mut out,
        );
        driver.handle(
            &mut host,
            PeerInput::Writable(exchange),
            time(wall, 1_100),
            &mut out,
        );
        driver.handle(
            &mut host,
            PeerInput::Finished(exchange),
            time(wall, 1_100),
            &mut out,
        );
        assert_eq!(
            host.reports.last().unwrap().at_ms,
            wall,
            "reports retain wall time"
        );
        out.clear();
        driver.handle(&mut host, PeerInput::Poll, time(wall, 31_099), &mut out);
        assert!(out.is_empty(), "wall step bypassed anti-entropy interval");
        driver.handle(&mut host, PeerInput::Poll, time(wall, 31_100), &mut out);
        assert!(matches!(out.as_slice(), [PeerOutput::Open { .. }]));
    }
}

/// The responder may close its unadmitted connection after delivering its
/// terminal refusal, before our own FIN is acknowledged. That transport close
/// must not erase a refusal we have already decoded from this authenticated peer.
#[test]
fn a_received_invitation_refusal_survives_a_failed_local_finish() {
    use crate::sync::Joining;
    use locust_proto::engine::PeerTime;
    use locust_proto::invite::{InviteSecret, JoinRequest};
    use locust_proto::sync::Refusal;
    use locust_proto::testkit::Author;
    for received_refusal in [false, true] {
        let founded = Founded::new();
        let mut host = host(1, founded.replica(&[]), &[1]);
        host.joins.push(Joining {
            goal: founded.goal,
            endpoint: endpoint(2),
            hints: vec![],
            request: JoinRequest::sign(
                founded.goal,
                endpoint(1),
                "member".into(),
                InviteSecret([9; 32]),
                &Author::new(4).key,
            ),
        });
        let mut driver = Driver::new();
        let mut out = Vec::new();
        let time = PeerTime {
            unix_ms: 1,
            elapsed_ms: 1,
        };
        driver.handle(&mut host, PeerInput::Poll, time, &mut out);
        let PeerOutput::Open { exchange, .. } = out.remove(0) else {
            panic!("join open")
        };
        driver.handle(&mut host, PeerInput::Opened(exchange), time, &mut out);
        driver.handle(&mut host, PeerInput::Writable(exchange), time, &mut out);
        driver.handle(&mut host, PeerInput::Writable(exchange), time, &mut out);
        out.clear();
        let frame = if received_refusal {
            SyncMessage::Refused(Refusal::InvitationRefused)
        } else {
            // This malformed response causes our own ProtocolError refusal.
            // Merely queuing an outbound refusal is not remote delivery.
            SyncMessage::Done
        };
        driver.handle(
            &mut host,
            PeerInput::Frame { exchange, frame },
            time,
            &mut out,
        );
        driver.handle(&mut host, PeerInput::Writable(exchange), time, &mut out);
        assert!(
            out.iter()
                .any(|output| matches!(output, PeerOutput::Finish(id) if *id == exchange))
        );
        assert!(host.reports.is_empty());
        driver.handle(&mut host, PeerInput::Closed(exchange), time, &mut out);
        assert_eq!(host.reports.len(), 1);
        assert!(host.reports[0].join);
        assert_eq!(
            host.reports[0].ended,
            if received_refusal {
                Ended::Refused(Refusal::InvitationRefused)
            } else {
                Ended::Aborted
            }
        );
        assert_eq!(host.joins.is_empty(), received_refusal);
    }
}

/// A retained worker fork produces halt-proof evidence. An endpoint that is
/// still an active member must keep reconciling ordinarily; only a contact
/// that appears solely as a halt-proof recipient gets evidence-only delivery.
#[test]
fn active_peers_reconcile_despite_retained_halt_proof_evidence() {
    use locust_proto::engine::PeerTime;
    let founded = Founded::new();
    // endpoint 2 is an active member; endpoint 3 is a historical contact
    // (not in the member set) that still owes fork evidence.
    let mut host = host(1, founded.replica(&[]), &[1, 2]);
    let mut worker = locust_proto::testkit::Author::new(5);
    let fork = founded.notes(&mut worker, 2);
    let proof = [fork[0].to_wire(), fork[1].to_wire()];
    host.halt_proofs
        .insert((founded.goal, endpoint(2)), vec![proof.clone()]);
    host.halt_proofs
        .insert((founded.goal, endpoint(3)), vec![proof.clone()]);
    let mut driver = Driver::new();
    let mut out = Vec::new();
    driver.handle(
        &mut host,
        PeerInput::Poll,
        PeerTime {
            unix_ms: 0,
            elapsed_ms: 0,
        },
        &mut out,
    );
    let mut opens = out
        .iter()
        .filter_map(|output| match output {
            PeerOutput::Open {
                exchange, endpoint, ..
            } => Some((*exchange, *endpoint)),
            _ => None,
        })
        .collect::<Vec<_>>();
    opens.sort_by_key(|(_, endpoint)| endpoint.0[0]);
    assert_eq!(
        opens.len(),
        2,
        "one exchange per active and historical peer"
    );
    let [
        (active_exchange, active_endpoint),
        (historical_exchange, historical_endpoint),
    ] = opens.as_slice()
    else {
        unreachable!()
    };
    assert_eq!(*active_endpoint, endpoint(2));
    assert_eq!(*historical_endpoint, endpoint(3));
    out.clear();
    // The active peer reconciles: Hello then Frontier, no Evidence preamble.
    driver.handle(
        &mut host,
        PeerInput::Opened(*active_exchange),
        PeerTime {
            unix_ms: 1,
            elapsed_ms: 1,
        },
        &mut out,
    );
    assert!(
        !out.iter()
            .any(|output| matches!(output, PeerOutput::Evidence(_))),
        "active peer must not get evidence-only delivery"
    );
    assert!(
        out.iter().any(|output| matches!(
            output,
            PeerOutput::Send {
                frame: SyncMessage::Hello { .. },
                ..
            }
        )),
        "active peer gets a normal hello"
    );
    // The outbox pumps one frame per writable; the frontier follows.
    out.clear();
    driver.handle(
        &mut host,
        PeerInput::Writable(*active_exchange),
        PeerTime {
            unix_ms: 1,
            elapsed_ms: 1,
        },
        &mut out,
    );
    assert!(
        out.iter().any(|output| matches!(
            output,
            PeerOutput::Send {
                frame: SyncMessage::Frontier(_),
                ..
            }
        )),
        "active peer gets a frontier for ordinary reconciliation"
    );
    out.clear();
    // The historical contact gets evidence-only delivery: Evidence then Hello,
    // and the HaltProof frame on the first writable.
    driver.handle(
        &mut host,
        PeerInput::Opened(*historical_exchange),
        PeerTime {
            unix_ms: 2,
            elapsed_ms: 2,
        },
        &mut out,
    );
    assert!(
        out.iter()
            .any(|output| matches!(output, PeerOutput::Evidence(_))),
        "historical contact gets evidence preamble"
    );
    let hello = out.iter().any(|output| {
        matches!(
            output,
            PeerOutput::Send {
                frame: SyncMessage::Hello { .. },
                ..
            }
        )
    });
    assert!(hello, "historical contact gets a hello after evidence");
    assert!(
        !out.iter().any(|output| matches!(
            output,
            PeerOutput::Send {
                frame: SyncMessage::Frontier(_),
                ..
            }
        )),
        "historical contact gets no frontier"
    );
    out.clear();
    driver.handle(
        &mut host,
        PeerInput::Writable(*historical_exchange),
        PeerTime {
            unix_ms: 3,
            elapsed_ms: 3,
        },
        &mut out,
    );
    assert!(
        out.iter().any(|output| matches!(
            output,
            PeerOutput::Send {
                frame: SyncMessage::HaltProof(_),
                ..
            }
        )),
        "historical contact receives the halt proof"
    );
}

/// The daemon's shell stops reading an accepted exchange after each frame
/// until the exchange is readable, then delivers the next frame without
/// asking again; the in-memory loops ask before every delivery. The two
/// agree because only an exchange's own next frame makes it stop reading:
/// not a poll, unasked transport capacity, a goal that grew, another
/// exchange, or the remote member's removal.
#[test]
fn only_its_own_next_frame_makes_a_readable_accepted_exchange_stop_reading() {
    use super::host::TestHost;
    use locust_proto::engine::PeerTime;
    use locust_proto::sync::{Frontier, Refusal};
    use locust_proto::testkit::Author;
    let founded = Founded::new();
    let notes = founded.notes(&mut Author::new(2), 600);
    let held = notes.iter().map(|event| event.id()).collect();
    let mut host = host(1, founded.replica(&notes), &[1, 2, 3]);
    let mut driver = Driver::new();
    let mut out = Vec::new();
    let mut now = 0;
    let mut handle = |driver: &mut Driver, host: &mut TestHost, input| {
        now += 1;
        let time = PeerTime {
            unix_ms: now,
            elapsed_ms: now,
        };
        driver.handle(host, input, time, &mut out);
    };
    let (exchange, other) = (ExchangeId::Accepted(1), ExchangeId::Accepted(2));
    for (exchange, remote) in [(exchange, 2), (other, 3)] {
        let remote = endpoint(remote);
        handle(
            &mut driver,
            &mut host,
            PeerInput::Accepted { exchange, remote },
        );
        let frame = SyncMessage::Hello {
            version: locust_proto::PROTOCOL_VERSION,
            goal: founded.goal,
        };
        handle(&mut driver, &mut host, PeerInput::Frame { exchange, frame });
    }
    let mut bystander = Author::new(3);
    let mut drained = 0;
    for frame in [
        SyncMessage::Frontier(Frontier::default()),
        SyncMessage::InventoryRequest {
            author: Author::new(2).key.public(),
            after: None,
        },
        SyncMessage::EventRequest(held),
        SyncMessage::KeyRequest { epoch: 0 },
        SyncMessage::Events(Vec::new()),
    ] {
        let note = founded.notes(&mut bystander, 1);
        host.replica_mut(&founded.goal).insert(&note);
        let elsewhere = if driver.readable(other) {
            PeerInput::Frame {
                exchange: other,
                frame: SyncMessage::Frontier(Frontier::default()),
            }
        } else {
            PeerInput::Writable(other)
        };
        for input in [
            PeerInput::Poll,
            PeerInput::Writable(exchange),
            elsewhere,
            PeerInput::Poll,
        ] {
            let described = format!("{input:?}");
            handle(&mut driver, &mut host, input);
            assert!(
                driver.readable(exchange),
                "{described} stopped an idle exchange from reading"
            );
        }
        handle(&mut driver, &mut host, PeerInput::Frame { exchange, frame });
        while !driver.readable(exchange) {
            drained += 1;
            handle(&mut driver, &mut host, PeerInput::Writable(exchange));
        }
    }
    assert!(drained > 0, "no request was answered in several frames");
    assert!(!driver.readable(other));
    host.members
        .get_mut(&founded.goal)
        .unwrap()
        .remove(&endpoint(2));
    for input in [
        PeerInput::Poll,
        PeerInput::Writable(exchange),
        PeerInput::Writable(exchange),
    ] {
        handle(&mut driver, &mut host, input);
        assert!(
            driver.readable(exchange),
            "the member's removal stopped its exchange from reading"
        );
    }
    assert!(out.contains(&PeerOutput::Send {
        exchange,
        frame: SyncMessage::Refused(Refusal::NotAMember),
    }));
}

/// A historical contact that owes more than one fork proof receives them in
/// fair rotation: repeated completed exchanges deliver each proof in turn,
/// instead of always selecting the first and starving the rest.
#[test]
fn historical_contact_rotates_through_multiple_halt_proofs() {
    use locust_proto::engine::PeerTime;
    let founded = Founded::new();
    // endpoint 2 is an active member; endpoint 3 is a historical contact
    // (not in the member set) that owes two distinct fork proofs.
    let mut host = host(1, founded.replica(&[]), &[1, 2]);
    let mut worker_a = locust_proto::testkit::Author::new(5);
    let mut worker_b = locust_proto::testkit::Author::new(6);
    let notes_a = founded.notes(&mut worker_a, 2);
    let notes_b = founded.notes(&mut worker_b, 2);
    let proof_a = [notes_a[0].to_wire(), notes_a[1].to_wire()];
    let proof_b = [notes_b[0].to_wire(), notes_b[1].to_wire()];
    assert_ne!(proof_a[0].id(), proof_b[0].id());
    host.halt_proofs.insert(
        (founded.goal, endpoint(3)),
        vec![proof_a.clone(), proof_b.clone()],
    );
    let mut driver = Driver::new();
    let mut out = Vec::new();
    // Drive four completed exchanges and record which proof each delivered.
    let mut delivered = Vec::new();
    for step in 0..4 {
        let elapsed = step * crate::sync::ANTI_ENTROPY_MS;
        driver.handle(
            &mut host,
            PeerInput::Poll,
            PeerTime {
                unix_ms: elapsed,
                elapsed_ms: elapsed,
            },
            &mut out,
        );
        let exchange = out
            .iter()
            .find_map(|output| match output {
                PeerOutput::Open {
                    exchange,
                    endpoint: ep,
                    ..
                } if *ep == endpoint(3) => Some(*exchange),
                _ => None,
            })
            .expect("historical contact opens an exchange");
        out.clear();
        // Opened: evidence preamble and hello.
        driver.handle(
            &mut host,
            PeerInput::Opened(exchange),
            PeerTime {
                unix_ms: elapsed,
                elapsed_ms: elapsed,
            },
            &mut out,
        );
        out.clear();
        // First writable: the halt proof frame.
        driver.handle(
            &mut host,
            PeerInput::Writable(exchange),
            PeerTime {
                unix_ms: elapsed,
                elapsed_ms: elapsed,
            },
            &mut out,
        );
        let proof = out
            .iter()
            .find_map(|output| match output {
                PeerOutput::Send {
                    frame: SyncMessage::HaltProof(proof),
                    ..
                } => Some(proof.clone()),
                _ => None,
            })
            .expect("halt proof delivered");
        delivered.push(proof[0].id());
        out.clear();
        // Second writable: Done and Finish, marking the exchange Completed.
        driver.handle(
            &mut host,
            PeerInput::Writable(exchange),
            PeerTime {
                unix_ms: elapsed,
                elapsed_ms: elapsed,
            },
            &mut out,
        );
        assert!(
            out.iter().any(|output| matches!(
                output,
                PeerOutput::Send {
                    frame: SyncMessage::Done,
                    ..
                }
            )),
            "proof exchange completes with Done"
        );
        assert!(
            out.iter()
                .any(|output| matches!(output, PeerOutput::Finish(_))),
            "proof exchange finishes"
        );
        out.clear();
        driver.handle(
            &mut host,
            PeerInput::Finished(exchange),
            PeerTime {
                unix_ms: elapsed,
                elapsed_ms: elapsed,
            },
            &mut out,
        );
        assert_eq!(host.reports.last().unwrap().ended, Ended::Completed);
        out.clear();
    }
    // Fair rotation: a, b, a, b — both proofs delivered, neither starved.
    assert_eq!(
        delivered,
        [
            proof_a[0].id(),
            proof_b[0].id(),
            proof_a[0].id(),
            proof_b[0].id()
        ],
        "proofs rotate fairly across completed exchanges"
    );
}

/// A transport failure before a proof is delivered does not advance the
/// cursor: the same proof is retried on the next exchange, so an aborted
/// delivery loses no obligation.
#[test]
fn aborted_proof_delivery_retries_the_same_proof() {
    use locust_proto::engine::PeerTime;
    let founded = Founded::new();
    let mut host = host(1, founded.replica(&[]), &[1, 2]);
    let mut worker_a = locust_proto::testkit::Author::new(5);
    let mut worker_b = locust_proto::testkit::Author::new(6);
    let notes_a = founded.notes(&mut worker_a, 2);
    let notes_b = founded.notes(&mut worker_b, 2);
    let proof_a = [notes_a[0].to_wire(), notes_a[1].to_wire()];
    let proof_b = [notes_b[0].to_wire(), notes_b[1].to_wire()];
    host.halt_proofs.insert(
        (founded.goal, endpoint(3)),
        vec![proof_a.clone(), proof_b.clone()],
    );
    let mut driver = Driver::new();
    let mut out = Vec::new();
    // First exchange opens with proof_a (cursor 0) but fails to open.
    driver.handle(
        &mut host,
        PeerInput::Poll,
        PeerTime {
            unix_ms: 0,
            elapsed_ms: 0,
        },
        &mut out,
    );
    let exchange = out
        .iter()
        .find_map(|output| match output {
            PeerOutput::Open {
                exchange,
                endpoint: ep,
                ..
            } if *ep == endpoint(3) => Some(*exchange),
            _ => None,
        })
        .expect("historical contact opens an exchange");
    out.clear();
    driver.handle(
        &mut host,
        PeerInput::OpenFailed(exchange),
        PeerTime {
            unix_ms: 0,
            elapsed_ms: 0,
        },
        &mut out,
    );
    assert_eq!(host.reports.last().unwrap().ended, Ended::Aborted);
    out.clear();
    // After the backoff window, the retry still selects proof_a: the cursor
    // did not advance on the aborted delivery.
    driver.handle(
        &mut host,
        PeerInput::Poll,
        PeerTime {
            unix_ms: crate::sync::MIN_BACKOFF_MS,
            elapsed_ms: crate::sync::MIN_BACKOFF_MS,
        },
        &mut out,
    );
    let exchange = out
        .iter()
        .find_map(|output| match output {
            PeerOutput::Open {
                exchange,
                endpoint: ep,
                ..
            } if *ep == endpoint(3) => Some(*exchange),
            _ => None,
        })
        .expect("historical contact retries after backoff");
    out.clear();
    driver.handle(
        &mut host,
        PeerInput::Opened(exchange),
        PeerTime {
            unix_ms: crate::sync::MIN_BACKOFF_MS,
            elapsed_ms: crate::sync::MIN_BACKOFF_MS,
        },
        &mut out,
    );
    out.clear();
    driver.handle(
        &mut host,
        PeerInput::Writable(exchange),
        PeerTime {
            unix_ms: crate::sync::MIN_BACKOFF_MS,
            elapsed_ms: crate::sync::MIN_BACKOFF_MS,
        },
        &mut out,
    );
    let retried = out
        .iter()
        .find_map(|output| match output {
            PeerOutput::Send {
                frame: SyncMessage::HaltProof(proof),
                ..
            } => Some(proof[0].id()),
            _ => None,
        })
        .expect("halt proof delivered on retry");
    assert_eq!(
        retried,
        proof_a[0].id(),
        "aborted delivery retries the same proof"
    );
}

#[test]
fn historical_proof_refusal_survives_close_before_finish_and_rotates() {
    use locust_proto::{engine::PeerTime, sync::Refusal};
    let step = |driver: &mut Driver,
                host: &mut dyn crate::sync::Host,
                input: PeerInput,
                elapsed_ms: u64| {
        let mut out = Vec::new();
        driver.handle(
            host,
            input,
            PeerTime {
                unix_ms: elapsed_ms,
                elapsed_ms,
            },
            &mut out,
        );
        out
    };
    let opened = |out: Vec<PeerOutput>| {
        out.into_iter()
            .find_map(|output| match output {
                PeerOutput::Open {
                    exchange,
                    endpoint: remote,
                    ..
                } if remote == endpoint(3) => Some(exchange),
                _ => None,
            })
            .expect("historical contact opens")
    };
    for finish_pending in [false, true] {
        let founded = Founded::new();
        let mut host = host(1, founded.replica(&[]), &[1]);
        let mut worker_a = locust_proto::testkit::Author::new(5);
        let mut worker_b = locust_proto::testkit::Author::new(6);
        let notes_a = founded.notes(&mut worker_a, 2);
        let notes_b = founded.notes(&mut worker_b, 2);
        let proof_b = [notes_b[0].to_wire(), notes_b[1].to_wire()];
        host.halt_proofs.insert(
            (founded.goal, endpoint(3)),
            vec![
                [notes_a[0].to_wire(), notes_a[1].to_wire()],
                proof_b.clone(),
            ],
        );
        let mut driver = Driver::new();
        let exchange = opened(step(&mut driver, &mut host, PeerInput::Poll, 0));
        step(&mut driver, &mut host, PeerInput::Opened(exchange), 0);
        step(&mut driver, &mut host, PeerInput::Writable(exchange), 0);
        if finish_pending {
            let out = step(&mut driver, &mut host, PeerInput::Writable(exchange), 0);
            assert!(out.contains(&PeerOutput::Finish(exchange)));
        }
        step(
            &mut driver,
            &mut host,
            PeerInput::Frame {
                exchange,
                frame: SyncMessage::Refused(Refusal::NotAMember),
            },
            0,
        );
        step(&mut driver, &mut host, PeerInput::Closed(exchange), 0);
        assert_eq!(
            host.reports.last().unwrap().ended,
            Ended::Refused(Refusal::NotAMember),
            "received refusal is definitive even with finish_pending={finish_pending}"
        );
        let retry_at = crate::sync::MIN_BACKOFF_MS;
        let exchange = opened(step(&mut driver, &mut host, PeerInput::Poll, retry_at));
        step(
            &mut driver,
            &mut host,
            PeerInput::Opened(exchange),
            retry_at,
        );
        let out = step(
            &mut driver,
            &mut host,
            PeerInput::Writable(exchange),
            retry_at,
        );
        assert!(out.contains(&PeerOutput::Send {
            exchange,
            frame: SyncMessage::HaltProof(proof_b),
        }));
    }
}

/// A dialed exchange counts as hearing from the remote endpoint once its
/// record stage ran to the end and brought nothing this replica lacked,
/// whatever this side pushed. One that brought records does not; the next
/// one, opened because the goal changed, does.
#[test]
fn a_dialed_exchange_is_heard_only_when_it_brought_nothing() {
    use super::net::Net;
    use locust_proto::testkit::Author;
    let founded = Founded::new();
    let goal = founded.goal;
    let theirs = founded.notes(&mut Author::new(2), 3);
    let mine = founded.notes(&mut Author::new(3), 2);
    for brings in [true, false] {
        let held = if brings {
            mine.clone()
        } else {
            [mine.clone(), theirs.clone()].concat()
        };
        let mut net = Net::new(vec![
            host(1, founded.replica(&held), &[1, 2]),
            host(2, founded.replica(&theirs), &[1, 2]),
        ]);
        net.poll(&[0]);
        assert_eq!(net.host(0).ids(&goal), net.host(1).ids(&goal));
        if brings {
            assert!(net.host(0).heard.is_empty());
            // The goal changed, so the next poll opens another exchange at
            // once, and it brings nothing.
            net.poll(&[0]);
        }
        assert_eq!(net.host(0).heard, [(goal, endpoint(2))]);
        // A poll that opens nothing hears nothing more.
        net.poll(&[0]);
        assert_eq!(net.host(0).heard, [(goal, endpoint(2))]);
    }
}

/// An accepted exchange counts as hearing from the endpoint that opened it
/// when its `Done` arrives and the initiator pushed nothing this replica
/// lacked. An endpoint that does not speak for a member is never heard.
#[test]
fn an_accepted_exchange_is_heard_at_done_when_nothing_was_pushed() {
    use locust_proto::engine::PeerTime;
    use locust_proto::testkit::Author;
    for (remote, pushed) in [(2, false), (2, true), (3, false)] {
        let founded = Founded::new();
        let goal = founded.goal;
        let mut host = host(
            1,
            founded.replica(&founded.notes(&mut Author::new(2), 2)),
            &[1, 2],
        );
        let mut driver = Driver::new();
        let mut out = Vec::new();
        let exchange = ExchangeId::Accepted(1);
        let time = PeerTime {
            unix_ms: 1,
            elapsed_ms: 1,
        };
        driver.handle(
            &mut host,
            PeerInput::Accepted {
                exchange,
                remote: endpoint(remote),
            },
            time,
            &mut out,
        );
        let frontier = host.replica_mut(&goal).frontier();
        let mut frames = vec![
            SyncMessage::Hello {
                version: locust_proto::PROTOCOL_VERSION,
                goal,
            },
            SyncMessage::Frontier(frontier),
        ];
        if pushed {
            let note = founded.notes(&mut Author::new(4), 1);
            frames.push(SyncMessage::Events(vec![note[0].to_wire()]));
        }
        for frame in frames {
            driver.handle(
                &mut host,
                PeerInput::Frame { exchange, frame },
                time,
                &mut out,
            );
            while !driver.readable(exchange) {
                driver.handle(&mut host, PeerInput::Writable(exchange), time, &mut out);
            }
        }
        assert!(host.heard.is_empty(), "heard before Done");
        driver.handle(
            &mut host,
            PeerInput::Frame {
                exchange,
                frame: SyncMessage::Done,
            },
            time,
            &mut out,
        );
        let heard = remote == 2 && !pushed;
        assert_eq!(
            host.heard,
            if heard {
                vec![(goal, endpoint(2))]
            } else {
                vec![]
            }
        );
        assert_eq!(
            host.callers,
            if remote == 3 {
                vec![(goal, endpoint(3))]
            } else {
                vec![]
            }
        );
    }
}

/// An accepted exchange from a member that only delivered a halt proof ran
/// no record stage, so it does not count as hearing, whatever the proof
/// brought. One that also had its frontier served counts only when the
/// proof brought no record this replica lacked.
#[test]
fn an_accepted_halt_proof_does_not_count_as_hearing() {
    use locust_proto::engine::PeerTime;
    use locust_proto::testkit::Author;
    for (frontier, held) in [(false, false), (false, true), (true, false), (true, true)] {
        let founded = Founded::new();
        let goal = founded.goal;
        let fork = founded.notes(&mut Author::new(5), 2);
        let replica = founded.replica(if held { &fork } else { &[] });
        let mut host = host(1, replica, &[1, 2]);
        host.halt_accepts.insert((goal, endpoint(2)));
        let mut driver = Driver::new();
        let mut out = Vec::new();
        let exchange = ExchangeId::Accepted(1);
        let time = PeerTime {
            unix_ms: 1,
            elapsed_ms: 1,
        };
        driver.handle(
            &mut host,
            PeerInput::Accepted {
                exchange,
                remote: endpoint(2),
            },
            time,
            &mut out,
        );
        let mut frames = vec![
            SyncMessage::Hello {
                version: locust_proto::PROTOCOL_VERSION,
                goal,
            },
            SyncMessage::HaltProof([fork[0].to_wire(), fork[1].to_wire()]),
        ];
        if frontier {
            frames.push(SyncMessage::Frontier(host.replica_mut(&goal).frontier()));
        }
        frames.push(SyncMessage::Done);
        for frame in frames {
            driver.handle(
                &mut host,
                PeerInput::Frame { exchange, frame },
                time,
                &mut out,
            );
            while !driver.readable(exchange) {
                driver.handle(&mut host, PeerInput::Writable(exchange), time, &mut out);
            }
        }
        assert_eq!(host.received_halt_proofs.len(), 1);
        assert!(out.contains(&PeerOutput::Finish(exchange)));
        let heard = frontier && held;
        assert_eq!(
            host.heard,
            if heard {
                vec![(goal, endpoint(2))]
            } else {
                vec![]
            },
            "frontier {frontier}, held {held}"
        );
    }
}

/// An endpoint that names a goal without speaking for a member is noted as a
/// caller, and a host that dials callers dials it like a peer. Until this
/// daemon's own records name it a member, that exchange sends `Hello` and an
/// empty frontier, pushes nothing, still takes what the caller sends, and is
/// never heard. Once a member, it is reconciled and heard as any peer.
#[test]
fn an_unknown_caller_is_dialed_back_with_an_empty_frontier() {
    use super::net::Net;
    use locust_proto::sync::{Frontier, Refusal};
    use locust_proto::testkit::Author;
    let founded = Founded::new();
    let goal = founded.goal;
    let theirs = founded.notes(&mut Author::new(2), 3);
    let mine = founded.notes(&mut Author::new(3), 2);
    let mut caught_up = host(1, founded.replica(&mine), &[1]);
    caught_up.dial_callers = true;
    let mut net = Net::new(vec![caught_up, host(2, founded.replica(&theirs), &[1, 2])]);
    // The caller's exchange is refused, and its endpoint is remembered.
    net.poll(&[1]);
    assert_eq!(net.host(0).callers, [(goal, endpoint(2))]);
    assert_eq!(net.host(0).ids(&goal), founded.replica(&mine).ids());
    let sent_by_0 = |net: &Net, from: usize| -> Vec<SyncMessage> {
        net.log[from..]
            .iter()
            .filter(|(to, _)| *to == 1)
            .map(|(_, frame)| frame.clone())
            .collect()
    };
    assert_eq!(
        sent_by_0(&net, 0),
        [SyncMessage::Refused(Refusal::NotAMember)]
    );
    let from = net.log.len();
    net.poll(&[0]);
    assert_eq!(
        sent_by_0(&net, from),
        [
            SyncMessage::Hello {
                version: locust_proto::PROTOCOL_VERSION,
                goal,
            },
            SyncMessage::Frontier(Frontier::default()),
            SyncMessage::Refused(Refusal::NotAMember),
        ]
    );
    // The caller's records arrived; this daemon's own did not leave.
    assert!(
        theirs
            .iter()
            .all(|event| net.host(0).ids(&goal).contains(&event.id()))
    );
    assert!(
        mine.iter()
            .all(|event| !net.host(1).ids(&goal).contains(&event.id()))
    );
    assert!(net.host(0).heard.is_empty());
    let report = net.host(0).reports.last().unwrap();
    assert!(report.dialed);
    assert_eq!(report.endpoint, endpoint(2));
    assert_eq!(report.ended, Ended::Refused(Refusal::NotAMember));
    // This daemon's own records now name the caller a member.
    net.host(0)
        .members
        .get_mut(&goal)
        .unwrap()
        .insert(endpoint(2));
    net.now_ms += crate::sync::MAX_BACKOFF_MS;
    let from = net.log.len();
    net.poll(&[0]);
    let frontier = net.host(0).replica_mut(&goal).frontier();
    assert!(sent_by_0(&net, from).contains(&SyncMessage::Frontier(frontier)));
    assert_eq!(net.host(0).ids(&goal), net.host(1).ids(&goal));
    assert_eq!(net.host(0).heard, [(goal, endpoint(2))]);
}
