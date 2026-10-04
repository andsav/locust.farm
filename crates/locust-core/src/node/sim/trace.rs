//! Short descriptions of delivered inputs, for the run's digest and log.

use locust_proto::engine::{ExchangeId, PeerInput};
use locust_proto::sync::SyncMessage;

pub(super) fn input_tag(input: &PeerInput) -> u64 {
    let (kind, exchange) = match input {
        PeerInput::Endpoint { .. } => (1, None),
        PeerInput::Connection { connected, .. } => (2 + u64::from(*connected), None),
        PeerInput::Opened(x) => (4, Some(x)),
        PeerInput::OpenFailed(x) => (5, Some(x)),
        PeerInput::Accepted { exchange, .. } => (6, Some(exchange)),
        PeerInput::Frame { exchange, frame } => (16 + frame_kind(frame), Some(exchange)),
        PeerInput::Writable(x) => (8, Some(x)),
        PeerInput::Finished(x) => (9, Some(x)),
        PeerInput::Closed(x) => (10, Some(x)),
        PeerInput::Poll => (11, None),
    };
    let number = match exchange {
        Some(ExchangeId::Dialed(n)) => 2 * n,
        Some(ExchangeId::Accepted(n)) => 2 * n + 1,
        None => 0,
    };
    kind | number << 8
}

fn frame_kind(frame: &SyncMessage) -> u64 {
    match frame {
        SyncMessage::Hello { .. } => 0,
        SyncMessage::Join(_) => 1,
        SyncMessage::Refused(_) => 2,
        SyncMessage::Frontier(f) => 3 + ((f.authors.len() as u64) << 8),
        SyncMessage::Events(e) => 4 + ((e.len() as u64) << 8),
        SyncMessage::InventoryRequest { .. } => 5,
        SyncMessage::Inventory { points, .. } => 6 + ((points.len() as u64) << 8),
        SyncMessage::EventRequest(ids) => 7 + ((ids.len() as u64) << 8),
        SyncMessage::KeyRequest { epoch } => 8 + (u64::from(*epoch) << 8),
        SyncMessage::Key { epoch, .. } => 9 + (u64::from(*epoch) << 8),
        SyncMessage::BlobRequest { offset, .. } => 10 + (offset << 8),
        SyncMessage::BlobChunk { offset, .. } => 11 + (offset << 8),
        SyncMessage::BlobUnavailable(_) => 12,
        SyncMessage::Done => 13,
        SyncMessage::HaltProof(_) => 14,
        SyncMessage::DeliverEffect { .. } => 15,
        SyncMessage::EffectReceipt { .. } => 16,
    }
}

pub(super) fn brief(input: &PeerInput) -> String {
    match input {
        PeerInput::Frame { exchange, frame } => {
            let what = match frame {
                SyncMessage::Events(events) => format!("Events({})", events.len()),
                SyncMessage::Frontier(f) => format!("Frontier({} authors)", f.authors.len()),
                SyncMessage::Inventory { points, more, .. } => {
                    format!("Inventory({} points, more={more})", points.len())
                }
                SyncMessage::EventRequest(ids) => format!("EventRequest({})", ids.len()),
                SyncMessage::BlobChunk { offset, total, .. } => {
                    format!("BlobChunk({offset}/{total})")
                }
                SyncMessage::Join(_) => "Join".into(),
                SyncMessage::Key { epoch, .. } => format!("Key({epoch})"),
                SyncMessage::Hello { .. } => "Hello".into(),
                other => format!("{other:?}"),
            };
            format!("Frame {exchange:?} {what}")
        }
        PeerInput::Endpoint { .. } => "Endpoint".into(),
        PeerInput::Connection { connected, .. } => format!("Connection connected={connected}"),
        PeerInput::Accepted { exchange, .. } => format!("Accepted {exchange:?}"),
        other => format!("{other:?}"),
    }
}
