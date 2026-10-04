use super::*;
use ed25519_dalek::{Signer, SigningKey};

fn fixture(base: &Path, version: &str, seq: u64) -> Verified {
    let root = base.join(format!("bundle-{version}-{seq}"));
    fs::create_dir_all(root.join("skills/locust")).unwrap();
    let key = SigningKey::from_bytes(&[19; 32]);
    let target = package::host_target().unwrap();
    let mut binary = vec![0; 64];
    let format = if target == "aarch64-apple-darwin" {
        binary[..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
        binary[4..8].copy_from_slice(&0x0100000cu32.to_le_bytes());
        "mach-o-arm64"
    } else {
        binary[..6].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1]);
        binary[18..20].copy_from_slice(&62u16.to_le_bytes());
        "elf-x86_64"
    };
    package::create_file(&root.join(package::BINARY), &binary, 0o755).unwrap();
    package::create_file(&root.join(package::SKILL), b"skill", 0o644).unwrap();
    package::create_file(&root.join(package::MANUAL), b"manual", 0o644).unwrap();
    let bytes = serde_json::to_vec(&json!({"format":"locust-release-v2","source_commit":"c".repeat(40),"version":version,
        "target":target,"machine_format":format,"api_version":locust_proto::API_VERSION,"protocol_version":locust_proto::PROTOCOL_VERSION,
        "toolchain":"1.96.1","files":[{"path":package::BINARY,"sha256":package::sha256(&binary),"size":binary.len(),"mode":493},
        {"path":package::SKILL,"sha256":package::sha256(b"skill"),"size":5,"mode":420},{"path":package::MANUAL,"sha256":package::sha256(b"manual"),"size":6,"mode":420}]})).unwrap();
    let registry = serde_json::to_vec(
        &json!({"format":"locust-withdrawals-v1","sequence":seq,"withdrawn_manifest_sha256":[]}),
    )
    .unwrap();
    Verified {
        root,
        manifest: package::validate_manifest(&bytes).unwrap(),
        manifest_sha256: package::sha256(&bytes),
        signature: key.sign(&bytes).to_bytes(),
        manifest_bytes: bytes,
        trust_key: key.verifying_key().to_bytes(),
        withdrawals: package::validate_withdrawals(&registry).unwrap(),
        withdrawals_signature: key.sign(&registry).to_bytes(),
        withdrawals_sha256: package::sha256(&registry),
        withdrawals_bytes: registry,
    }
}
fn install(prefix: &Path, value: &Verified) -> Value {
    let plan = plan(prefix, value, false).unwrap();
    apply_with_probe(prefix, value, false, &plan.digest().unwrap(), |_, _, _| {
        Ok(())
    })
    .unwrap()
}
#[test]
fn reviewed_install_repeat_upgrade_and_uninstall_preserve_data_and_policy() {
    let dir = tempfile::tempdir().unwrap();
    let prefix = dir.path().join("software");
    let one = fixture(dir.path(), "0.1.0", 1);
    let first = plan(&prefix, &one, false).unwrap();
    assert!(!prefix.exists(), "planning is read-only");
    assert!(apply_with_probe(&prefix, &one, false, "wrong", |_, _, _| panic!()).is_err());
    assert!(!prefix.exists());
    assert_eq!(install(&prefix, &one)["changed"], true);
    assert_eq!(install(&prefix, &one)["changed"], false);
    assert_eq!(status(&prefix).unwrap()["installed"], true);
    assert_eq!(
        fs::read(prefix.join("current/manual.tar")).unwrap(),
        b"manual"
    );
    let data = dir.path().join("daemon-identity");
    fs::write(&data, b"keep").unwrap();
    let two = fixture(dir.path(), "0.2.0", 2);
    install(&prefix, &two);
    assert!(
        apply_with_probe(
            &prefix,
            &one,
            false,
            &first.digest().unwrap(),
            |_, _, _| panic!()
        )
        .is_err()
    );
    let unplan = uninstall_plan(&prefix).unwrap();
    assert_eq!(
        uninstall(&prefix, &unplan.digest().unwrap()).unwrap()["uninstalled"],
        true
    );
    assert_eq!(fs::read(data).unwrap(), b"keep");
    assert!(
        !prefix
            .join("releases")
            .read_dir()
            .unwrap()
            .any(|entry| entry.unwrap().path().join("manual.tar").exists())
    );
    assert!(prefix.join(POLICY).exists());
    assert!(
        plan(&prefix, &one, true).is_err(),
        "uninstall must not reset registry rollback protection"
    );
    assert_eq!(status(&prefix).unwrap()["installed"], false);
}
#[test]
fn source_mutation_and_failed_probe_never_activate_partial_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let prefix = dir.path().join("software");
    let one = fixture(dir.path(), "0.1.0", 1);
    install(&prefix, &one);
    let two = fixture(dir.path(), "0.2.0", 2);
    let digest = plan(&prefix, &two, false).unwrap().digest().unwrap();
    assert!(
        apply_with_probe(&prefix, &two, false, &digest, |_, _, _| Err(
            Failure::unavailable("injected start failure")
        ))
        .is_err()
    );
    assert_eq!(
        status(&prefix).unwrap()["manifest_sha256"],
        one.manifest_sha256
    );
    fs::write(two.root.join(package::SKILL), b"changed after verification").unwrap();
    assert!(
        apply_with_probe(&prefix, &two, false, &digest, |_, _, _| panic!(
            "must not execute changed bytes"
        ))
        .is_err()
    );
    assert_eq!(
        status(&prefix).unwrap()["manifest_sha256"],
        one.manifest_sha256
    );
    fs::write(two.root.join(package::SKILL), b"skill").unwrap();
    install(&prefix, &two);
    assert_eq!(
        status(&prefix).unwrap()["manifest_sha256"],
        two.manifest_sha256
    );
}
#[test]
fn stale_plan_downgrade_policy_equivocation_and_foreign_current_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let prefix = dir.path().join("software");
    let one = fixture(dir.path(), "0.1.0", 3);
    let two = fixture(dir.path(), "0.2.0", 2);
    let stale = plan(&prefix, &one, false).unwrap().digest().unwrap();
    install(&prefix, &two);
    assert!(apply_with_probe(&prefix, &one, true, &stale, |_, _, _| panic!()).is_err());
    assert!(plan(&prefix, &one, false).is_err());
    assert!(plan(&prefix, &one, true).is_ok());
    let mut bad = fixture(dir.path(), "0.3.0", 2);
    bad.withdrawals_bytes.push(b' ');
    assert!(plan(&prefix, &bad, false).is_err());
    let mut bad = fixture(dir.path(), "0.4.0", 4);
    bad.trust_key = [5; 32];
    assert!(plan(&prefix, &bad, false).is_err());
    fs::remove_file(prefix.join("current")).unwrap();
    std::os::unix::fs::symlink("/tmp/foreign", prefix.join("current")).unwrap();
    assert!(plan(&prefix, &one, true).is_err());
}
#[test]
fn modified_releases_are_retained_on_uninstall_and_lock_conflicts_are_explicit() {
    let dir = tempfile::tempdir().unwrap();
    let prefix = dir.path().join("software");
    let one = fixture(dir.path(), "0.1.0", 1);
    install(&prefix, &one);
    let held = lock(&prefix).unwrap();
    let digest = plan(&prefix, &one, false).unwrap().digest().unwrap();
    assert!(apply_with_probe(&prefix, &one, false, &digest, |_, _, _| panic!()).is_err());
    drop(held);
    let binary = prefix
        .join("releases")
        .join(&one.manifest_sha256)
        .join("locust");
    fs::write(&binary, b"user changed").unwrap();
    let unplan = uninstall_plan(&prefix).unwrap();
    let result = uninstall(&prefix, &unplan.digest().unwrap()).unwrap();
    assert_eq!(
        result["retained_modified_or_unknown"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(fs::read(binary).unwrap(), b"user changed");
}
