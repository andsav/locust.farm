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
        driver.handle(&mut host, PeerInput::Poll, 0, &mut out);
        let PeerOutput::Open { exchange, .. } = out.remove(0) else {
            panic!("open");
        };
        assert!(matches!(exchange, ExchangeId::Dialed(_)));
        driver.handle(&mut host, PeerInput::Opened(exchange), 1, &mut out);
        driver.handle(&mut host, PeerInput::Writable(exchange), 2, &mut out);
        driver.handle(&mut host, PeerInput::Writable(exchange), 3, &mut out);
        let frontier = host.replica_mut(&founded.goal).frontier();
        driver.handle(
            &mut host,
            PeerInput::Frame {
                exchange,
                frame: SyncMessage::Frontier(frontier),
            },
            4,
            &mut out,
        );
        driver.handle(&mut host, PeerInput::Writable(exchange), 5, &mut out);
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
        driver.handle(&mut host, input, 6, &mut out);
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
        driver.handle(&mut host, PeerInput::Poll, 7, &mut out);
        assert!(out.is_empty());
        if !success {
            driver.handle(&mut host, PeerInput::Poll, 1006, &mut out);
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
        driver.handle(&mut host, PeerInput::Poll, 0, &mut out);
        let [PeerOutput::Open { exchange, .. }] = out.as_slice() else {
            panic!("open");
        };
        let exchange = *exchange;
        out.clear();
        driver.handle(&mut host, PeerInput::OpenFailed(exchange), 6, &mut out);
        retried.push((7..=1_006).find(|&now| {
            driver.handle(&mut host, PeerInput::Poll, now, &mut out);
            !out.is_empty()
        }));
    }
    assert_eq!(retried, [Some(1_006), Some(706), Some(806)]);
}
