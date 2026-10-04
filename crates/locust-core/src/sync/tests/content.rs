use super::net::Net;
use super::{Founded, host};
use locust_proto::id::BlobHash;
use locust_proto::store::Blob;

#[test]
fn unavailable_prefix_does_not_starve_later_available_objects() {
    let founded = Founded::new();
    let blob = Blob::new(b"available after more than 256 absent objects".to_vec());
    let hash = blob.hash();
    assert!(hash.0[0] > 0);
    let length = blob.bytes().len() as u64;
    let mut left = founded.replica(&[]);
    for index in 0..300u32 {
        let mut bytes = [0; 32];
        bytes[28..].copy_from_slice(&index.to_be_bytes());
        left.wants.insert(BlobHash(bytes), 32);
    }
    left.wants.insert(hash, length);
    let mut right = founded.replica(&[]);
    right.add_blob(blob);
    let mut net = Net::new(vec![host(1, left, &[1, 2]), host(2, right, &[1, 2])]);
    net.poll(&[0]);
    assert!(net.host(0).replica_mut(&founded.goal).holds_blob(&hash));
}

#[test]
fn ciphertext_arrives_before_a_verifiable_key_and_corruption_is_retried() {
    use crate::sync::Replica;
    use locust_proto::testkit::{self, Author};
    let founded = Founded::new();
    let (payload, blob) = testkit::sealed_payload(&founded.goal, 0, b"sealed text");
    let event =
        Author::new(2).event_with(founded.goal, founded.anchor(), super::note(), Some(payload));
    let mut right = founded.replica(std::slice::from_ref(&event));
    right.add_blob(blob);
    right.set_key(0, testkit::content_key(1));
    right.corrupt.insert(payload.hash);
    let mut net = Net::new(vec![
        host(1, founded.replica(&[]), &[1, 2]),
        host(2, right, &[1, 2]),
    ]);
    net.poll(&[0]);
    let left = net.host(0).replica_mut(&founded.goal);
    assert!(!left.holds_blob(&payload.hash));
    assert_eq!(left.key(0), None);
    net.host(1).replica_mut(&founded.goal).corrupt.clear();
    net.tick(35000);
    let left = net.host(0).replica_mut(&founded.goal);
    assert!(left.holds_blob(&payload.hash));
    assert_eq!(left.key(0), Some(testkit::content_key(1)));
}
