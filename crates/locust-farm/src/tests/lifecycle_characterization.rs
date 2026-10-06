//! Receipt replay is idempotency, not a restored publisher freshness check.
use super::*;

#[test]
fn old_identical_check_in_returns_its_receipt_after_newer_requests_and_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("farm.sqlite");
    let key = Keypair::from_seed([81; 32]);
    let old = control(&key, 2, FarmOperation::CheckIn);
    let receipt = {
        let service = Service::open(&path, config()).unwrap();
        apply(&service, &upload_request(&key, 1, true));
        let receipt = apply(&service, &old);
        apply(&service, &control(&key, 4, FarmOperation::CheckIn));
        receipt
    };
    let service = Service::open(&path, config()).unwrap();
    let before = service.read(&old.farm_id.0).unwrap();
    assert_eq!(before.stream_version, 3);
    let regenerated = control(&key, 2, FarmOperation::CheckIn);
    assert_eq!(regenerated, old);
    assert_eq!(apply(&service, &regenerated), receipt);
    assert_eq!(receipt.stream_version, 2);
    assert_eq!(service.read(&old.farm_id.0).unwrap().stream_version, 3);
    for rejected in [
        control(&key, 2, FarmOperation::Suspend),
        control(&key, 3, FarmOperation::CheckIn),
    ] {
        assert_eq!(
            service
                .mutate(&rejected.farm_id.0, rejected.operation, &rejected)
                .unwrap_err()
                .status,
            StatusCode::CONFLICT
        );
    }
}
