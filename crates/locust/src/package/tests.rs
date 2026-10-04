use super::*;

#[test]
fn package_format_cannot_accept_arbitrary_paths_or_duplicate_payloads() {
    let value = serde_json::json!({"format":"locust-release-v1","source_commit":"a".repeat(40),"version":"0.1.0",
        "target":"aarch64-apple-darwin","machine_format":"mach-o-arm64","api_version":1,"protocol_version":1,
        "toolchain":"1.96.1","files":[{"path":"locust","sha256":"b".repeat(64),"size":64,"mode":493},
        {"path":"skills/locust/SKILL.md","sha256":"c".repeat(64),"size":1,"mode":420}]});
    assert!(validate_manifest(&serde_json::to_vec(&value).unwrap()).is_ok());
    for path in [
        "../locust",
        "/tmp/locust",
        "locust",
        "skills/locust/../../../escape",
    ] {
        let mut bad = value.clone();
        bad["files"][1]["path"] = path.into();
        assert!(validate_manifest(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
}

#[test]
fn strict_signatures_reject_changed_bytes_and_other_keys() {
    let key = SigningKey::from_bytes(&[3; 32]);
    let bytes = b"signed exact bytes\n";
    let signature = key.sign(bytes).to_bytes();
    assert!(verify_signature(&key.verifying_key().to_bytes(), bytes, &signature).is_ok());
    assert!(
        verify_signature(
            &key.verifying_key().to_bytes(),
            b"signed exact bytes",
            &signature
        )
        .is_err()
    );
    assert!(
        verify_signature(
            &SigningKey::from_bytes(&[4; 32]).verifying_key().to_bytes(),
            bytes,
            &signature
        )
        .is_err()
    );
}

struct Bundle {
    directory: tempfile::TempDir,
    root: PathBuf,
    secret: PathBuf,
    trust: PathBuf,
    registry: PathBuf,
}

impl Bundle {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("bundle");
        fs::create_dir_all(root.join("skills/locust")).unwrap();
        // A native-format fixture; never executed. Executability remains a
        // separate build/installation qualification, not signature verification.
        let mut binary = [0u8; 64];
        binary[..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
        binary[4..8].copy_from_slice(&0x0100000cu32.to_le_bytes());
        let skill = b"# Locust fixture\n";
        create_file(&root.join(BINARY), &binary, 0o755).unwrap();
        create_file(&root.join(SKILL), skill, 0o644).unwrap();
        let manifest = Manifest {
            format: "locust-release-v1".into(),
            source_commit: "a".repeat(40),
            version: "0.1.0".into(),
            target: "aarch64-apple-darwin".into(),
            machine_format: "mach-o-arm64".into(),
            api_version: 1,
            protocol_version: 1,
            toolchain: "1.96.1".into(),
            files: vec![
                Payload {
                    path: BINARY.into(),
                    sha256: sha256(&binary),
                    size: binary.len() as u64,
                    mode: 0o755,
                },
                Payload {
                    path: SKILL.into(),
                    sha256: sha256(skill),
                    size: skill.len() as u64,
                    mode: 0o644,
                },
            ],
        };
        create_file(
            &root.join(MANIFEST),
            &serde_json::to_vec(&manifest).unwrap(),
            0o644,
        )
        .unwrap();
        let secret = directory.path().join("signing.seed");
        let trust = directory.path().join("trusted.pub");
        let registry = directory.path().join("withdrawals.json");
        let key = SigningKey::from_bytes(&[23; 32]);
        create_file(&secret, &key.to_bytes(), 0o600).unwrap();
        create_file(&trust, &key.verifying_key().to_bytes(), 0o644).unwrap();
        sign_release(&root, &secret).unwrap();
        let bundle = Self {
            directory,
            root,
            secret,
            trust,
            registry,
        };
        bundle.withdrawals(Vec::new());
        bundle
    }

    fn verify(&self) -> Result<Verified, Failure> {
        verify(&self.root, &self.trust, &self.registry)
    }

    fn withdrawals(&self, withdrawn: Vec<String>) {
        let list = Withdrawals {
            format: "locust-withdrawals-v1".into(),
            sequence: 2,
            withdrawn_manifest_sha256: withdrawn,
        };
        fs::write(&self.registry, serde_json::to_vec(&list).unwrap()).unwrap();
        let signature = signature_path(&self.registry);
        if signature.exists() {
            fs::remove_file(signature).unwrap();
        }
        sign_withdrawals(&self.registry, &self.secret).unwrap();
    }
}

#[test]
fn full_verification_preserves_exact_signed_bytes_and_rejects_tampering() {
    let bundle = Bundle::new();
    let verified = bundle.verify().unwrap();
    assert_eq!(
        verified.manifest_bytes,
        fs::read(bundle.root.join(MANIFEST)).unwrap()
    );
    assert_eq!(
        verified.withdrawals_bytes,
        fs::read(&bundle.registry).unwrap()
    );
    assert_eq!(
        verified.signature.as_slice(),
        fs::read(bundle.root.join(SIGNATURE)).unwrap()
    );
    assert_eq!(
        verified.withdrawals_signature.as_slice(),
        fs::read(signature_path(&bundle.registry)).unwrap()
    );
    for mutation in [
        "manifest",
        "payload",
        "length",
        "mode",
        "key",
        "registry",
        "registry-signature",
        "signature",
    ] {
        let bundle = Bundle::new();
        match mutation {
            "manifest" => {
                let mut bytes = fs::read(bundle.root.join(MANIFEST)).unwrap();
                bytes.push(b'\n'); // Same parsed manifest, different signed bytes.
                fs::write(bundle.root.join(MANIFEST), bytes).unwrap();
            }
            "payload" => {
                let path = bundle.root.join(BINARY);
                let mut bytes = fs::read(&path).unwrap();
                bytes[63] ^= 1;
                fs::write(path, bytes).unwrap();
            }
            "length" => {
                fs::write(bundle.root.join(SKILL), b"short").unwrap();
            }
            "mode" => {
                fs::set_permissions(bundle.root.join(BINARY), fs::Permissions::from_mode(0o777))
                    .unwrap();
            }
            "key" => {
                fs::write(
                    &bundle.trust,
                    SigningKey::from_bytes(&[24; 32]).verifying_key().to_bytes(),
                )
                .unwrap();
            }
            "registry" => {
                fs::write(&bundle.registry, b"{\"format\":\"locust-withdrawals-v1\",\"sequence\":1,\"withdrawn_manifest_sha256\":[]}").unwrap();
            }
            "registry-signature" => {
                fs::write(signature_path(&bundle.registry), [0; 64]).unwrap();
            }
            "signature" => {
                fs::write(bundle.root.join(SIGNATURE), [0; 64]).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(bundle.verify().is_err(), "accepted changed {mutation}");
    }
}

#[test]
fn a_signed_withdrawal_denies_only_the_named_manifest() {
    let bundle = Bundle::new();
    let digest = bundle.verify().unwrap().manifest_sha256;
    bundle.withdrawals(vec!["b".repeat(64)]);
    assert!(bundle.verify().is_ok());
    bundle.withdrawals(vec![digest]);
    assert_eq!(bundle.verify().err().unwrap().code, ErrorCode::Denied);
}

#[test]
fn package_metadata_and_payloads_reject_symlinks_and_hardlinks() {
    for hard in [false, true] {
        for name in [
            "manifest",
            "signature",
            "binary",
            "skill",
            "trust",
            "registry",
            "registry-signature",
        ] {
            let bundle = Bundle::new();
            let path = match name {
                "manifest" => bundle.root.join(MANIFEST),
                "signature" => bundle.root.join(SIGNATURE),
                "binary" => bundle.root.join(BINARY),
                "skill" => bundle.root.join(SKILL),
                "trust" => bundle.trust.clone(),
                "registry" => bundle.registry.clone(),
                "registry-signature" => signature_path(&bundle.registry),
                _ => unreachable!(),
            };
            let held = bundle.directory.path().join("linked-input");
            fs::rename(&path, &held).unwrap();
            if hard {
                fs::hard_link(&held, &path).unwrap();
            } else {
                std::os::unix::fs::symlink(&held, &path).unwrap();
            }
            assert!(
                bundle.verify().is_err(),
                "accepted {name}, hard link: {hard}"
            );
        }
    }
    let bundle = Bundle::new();
    let held = bundle.directory.path().join("skills");
    fs::rename(bundle.root.join("skills"), &held).unwrap();
    std::os::unix::fs::symlink(held, bundle.root.join("skills")).unwrap();
    assert!(
        bundle.verify().is_err(),
        "accepted a linked parent directory"
    );
}

#[test]
fn fifo_package_input_is_rejected_without_waiting_for_a_writer() {
    use std::sync::mpsc;
    use std::time::Duration;
    let bundle = Bundle::new();
    let path = bundle.root.join(MANIFEST);
    fs::remove_file(&path).unwrap();
    // rustix does not expose mkfifo on macOS; use the standard Unix utility.
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    let (sent, received) = mpsc::channel();
    let input = path.clone();
    let reader = std::thread::spawn(move || {
        let _ = sent.send(regular(&input).is_err());
    });
    let result = received.recv_timeout(Duration::from_secs(2));
    if result.is_err() {
        // Release a regressed blocking open before failing, leaving no thread
        // or FIFO writer behind in the test process.
        let _writer = OpenOptions::new().write(true).open(path).unwrap();
        reader.join().unwrap();
        panic!("regular-file validation blocked opening a FIFO");
    }
    reader.join().unwrap();
    assert!(result.unwrap());
}

#[test]
fn fixed_size_keys_and_signatures_read_only_their_format_length_plus_one() {
    struct Bytes {
        read: usize,
    }
    impl Read for Bytes {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            assert!(
                self.read + bytes.len() <= 65,
                "read past the signature format boundary"
            );
            bytes.fill(1);
            self.read += bytes.len();
            Ok(bytes.len())
        }
    }
    assert!(exact_bytes::<64>(Bytes { read: 0 }).is_err());
    assert_eq!(
        exact_bytes::<32>(std::io::Cursor::new([7; 32])).unwrap(),
        [7; 32]
    );
    assert!(exact_bytes::<32>(std::io::Cursor::new([7; 31])).is_err());
}

#[test]
fn signing_requires_private_key_permissions_and_never_overwrites_a_signature() {
    let bundle = Bundle::new();
    let output = bundle.directory.path().join("new.sig");
    fs::set_permissions(&bundle.secret, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        sign_bytes(b"message", &bundle.secret, &output)
            .unwrap_err()
            .code,
        ErrorCode::Denied
    );
    assert!(!output.exists());
    fs::set_permissions(&bundle.secret, fs::Permissions::from_mode(0o600)).unwrap();
    sign_bytes(b"message", &bundle.secret, &output).unwrap();
    let signature = fs::read(&output).unwrap();
    assert!(sign_bytes(b"other message", &bundle.secret, &output).is_err());
    assert_eq!(fs::read(output).unwrap(), signature);
}
