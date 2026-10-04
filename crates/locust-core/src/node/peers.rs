//! The transport's side of the node: which exchanges to open, and what each
//! received frame leads to.

use locust_proto::engine::{Entropy, PeerEngine, PeerInput, PeerOutput};
use locust_proto::store::Store;

use super::Node;
use super::commit::Tx;
use super::identity::{EndpointRecord, Identity};

impl<S: Store, E: Entropy> PeerEngine for Node<S, E> {
    fn endpoint_secret(&self) -> [u8; 32] {
        self.identity.endpoint_secret
    }

    fn peer(&mut self, input: PeerInput, _now_ms: u64, _out: &mut Vec<PeerOutput>) {
        if let PeerInput::Endpoint { endpoint, hints } = input {
            let record = EndpointRecord { endpoint, hints };
            if self.identity.endpoint.as_ref() != Some(&record) {
                let mut tx = Tx::none();
                tx.local(Identity::endpoint_write(&record));
                // A failed commit leaves the earlier endpoint in place; the
                // shell reports it again with its next hint change.
                let _ = self.land(tx);
            }
        }
    }
}
