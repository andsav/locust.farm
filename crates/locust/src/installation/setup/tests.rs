use super::*;
use ed25519_dalek::{Signer, SigningKey};
fn fixture(client: Client) -> (tempfile::TempDir, SetupSpec) {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();
    let prefix = base.join("software");
    let root = base.join("bundle");
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
    package::create_file(&root.join(package::SKILL), b"signed skill", 0o644).unwrap();
    let bytes=encode(&json!({"format":"locust-release-v1","source_commit":"c".repeat(40),"version":"0.1.0","target":target,"machine_format":format,"api_version":locust_proto::API_VERSION,"protocol_version":locust_proto::PROTOCOL_VERSION,"toolchain":"1.96.1","files":[{"path":package::BINARY,"sha256":package::sha256(&binary),"size":binary.len(),"mode":493},{"path":package::SKILL,"sha256":package::sha256(b"signed skill"),"size":12,"mode":420}]})).unwrap();
    let registry = encode(
        &json!({"format":"locust-withdrawals-v1","sequence":1,"withdrawn_manifest_sha256":[]}),
    )
    .unwrap();
    let value = Verified {
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
    };
    let install = super::super::plan(&prefix, &value, false).unwrap();
    super::super::apply_with_probe(
        &prefix,
        &value,
        false,
        &install.digest().unwrap(),
        |_, _, _| Ok(()),
    )
    .unwrap();
    let profile = base.join("profile");
    fs::create_dir(&profile).unwrap();
    let workspace = base.join("workspace");
    fs::create_dir(&workspace).unwrap();
    let credential = base.join("credential");
    let session = base.join("session");
    package::create_file(&credential, &[17; 32], 0o600).unwrap();
    package::create_file(&session, &[18; 32], 0o600).unwrap();
    let s = SetupSpec {
        executable: prefix.join("current/locust"),
        skill_source: prefix.join("current/skills/locust/SKILL.md"),
        prefix,
        client,
        profile_home: profile,
        workspace,
        daemon_home: base.join("daemon"),
        credential,
        session,
    };
    (dir, normalize(&s).unwrap())
}
fn put(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
#[test]
fn all_clients_roundtrip_preserves_original_and_readiness_boundary() {
    for client in [Client::Codex, Client::Claude, Client::Pi] {
        let (_dir, s) = fixture(client);
        let p = paths(&s).unwrap();
        let baseline = if client == Client::Codex {
            b"# keep comment\nmodel = 'chosen'\n".as_slice()
        } else {
            b"{\"unrelated\":{\"api_key\":\"DO_NOT_PRINT\"}}".as_slice()
        };
        put(&p.config, baseline);
        let reviewed = plan(&s, false).unwrap();
        assert!(!p.record.exists());
        assert!(
            !reviewed
                .json()
                .unwrap()
                .to_string()
                .contains("DO_NOT_PRINT")
        );
        apply(&s, &reviewed.digest().unwrap()).unwrap();
        assert_eq!(status(&s).unwrap()["configured"], true);
        assert_eq!(status(&s).unwrap()["api_ready"], false);
        let again = plan(&s, false).unwrap();
        assert_eq!(
            apply(&s, &again.digest().unwrap()).unwrap()["changed"],
            false
        );
        let remove_plan = plan(&s, true).unwrap();
        remove(&s, &remove_plan.digest().unwrap()).unwrap();
        assert_eq!(fs::read(&p.config).unwrap(), baseline);
        assert!(!p.skill.exists());
    }
}
#[test]
fn remove_preserves_unrelated_edits_and_rejects_owned_edits() {
    for client in [Client::Codex, Client::Claude, Client::Pi] {
        let (_d, s) = fixture(client);
        let p = paths(&s).unwrap();
        let a = plan(&s, false).unwrap();
        apply(&s, &a.digest().unwrap()).unwrap();
        let mut bytes = fs::read(&p.config).unwrap();
        if client == Client::Codex {
            bytes.extend_from_slice(b"\n[extra]\nvalue = 'preserve'\n");
        } else {
            let mut v: Value = serde_json::from_slice(&bytes).unwrap();
            v["extra"] = json!("preserve");
            bytes = encode(&v).unwrap();
        }
        put(&p.config, &bytes);
        let r = plan(&s, true).unwrap();
        remove(&s, &r.digest().unwrap()).unwrap();
        assert!(
            String::from_utf8(fs::read(&p.config).unwrap())
                .unwrap()
                .contains("preserve")
        );
        assert!(
            entry(client, &snapshot(&p.config).unwrap())
                .unwrap()
                .is_none()
        );
        let a = plan(&s, false).unwrap();
        apply(&s, &a.digest().unwrap()).unwrap();
        put(&p.skill, b"user edit");
        assert!(plan(&s, true).is_err());
        assert_eq!(fs::read(&p.skill).unwrap(), b"user edit");
    }
}
#[test]
fn interrupted_writes_resume_original_review_and_reject_unknown_state() {
    for stop in 0..3 {
        let (_d, s) = fixture(Client::Codex);
        let a = plan(&s, false).unwrap();
        let digest = a.digest().unwrap();
        assert!(
            execute(&s, &digest, false, |i| if i == stop {
                Err(Failure::internal("injected interruption"))
            } else {
                Ok(())
            })
            .is_err()
        );
        assert_eq!(plan(&s, false).unwrap().digest().unwrap(), digest);
        apply(&s, &digest).unwrap();
        assert_eq!(status(&s).unwrap()["configured"], true);
        assert_eq!(status(&s).unwrap()["pending"], false);
    }
    let (_d, s) = fixture(Client::Claude);
    let a = plan(&s, false).unwrap();
    assert!(
        execute(&s, &a.digest().unwrap(), false, |_| Err(Failure::internal(
            "stop"
        )))
        .is_err()
    );
    let p = paths(&s).unwrap();
    put(&p.config, b"{\"user\":true}");
    assert!(apply(&s, &a.digest().unwrap()).is_err());
    assert_eq!(fs::read(&p.config).unwrap(), b"{\"user\":true}");
}
#[test]
fn stale_plan_collisions_permissions_and_symlinks_are_rejected() {
    let (_d, s) = fixture(Client::Codex);
    let a = plan(&s, false).unwrap();
    let p = paths(&s).unwrap();
    put(&p.config, b"model='new'\n");
    assert!(apply(&s, &a.digest().unwrap()).is_err());
    assert!(!p.record.exists());
    let project = s.workspace.join(".codex/config.toml");
    put(&project, b"[mcp_servers.locust]\ncommand='other'\n");
    assert!(plan(&s, false).is_err());
    fs::remove_file(project).unwrap();
    fs::set_permissions(&s.session, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(plan(&s, false).is_err());
    fs::set_permissions(&s.session, fs::Permissions::from_mode(0o600)).unwrap();
    fs::remove_file(&p.config).unwrap();
    std::os::unix::fs::symlink(&s.session, &p.config).unwrap();
    assert!(plan(&s, false).is_err());
}
#[test]
fn remove_after_software_uninstall_does_not_require_secret_files() {
    let (_d, s) = fixture(Client::Pi);
    let a = plan(&s, false).unwrap();
    apply(&s, &a.digest().unwrap()).unwrap();
    let u = super::super::uninstall_plan(&s.prefix).unwrap();
    super::super::uninstall(&s.prefix, &u.digest().unwrap()).unwrap();
    fs::remove_file(&s.session).unwrap();
    fs::remove_file(&s.credential).unwrap();
    let r = plan(&s, true).unwrap();
    remove(&s, &r.digest().unwrap()).unwrap();
    assert_eq!(status(&s).unwrap()["owned"], false);
}

#[test]
fn reapply_retains_intervening_unrelated_edits_on_removal() {
    for client in [Client::Codex, Client::Claude, Client::Pi] {
        let (_d, s) = fixture(client);
        let p = paths(&s).unwrap();
        let a = plan(&s, false).unwrap();
        apply(&s, &a.digest().unwrap()).unwrap();
        let mut bytes = fs::read(&p.config).unwrap();
        if client == Client::Codex {
            bytes.extend_from_slice(b"\n[extra]\nkeep=true\n");
        } else {
            let mut v: Value = serde_json::from_slice(&bytes).unwrap();
            v["extra"] = json!({"keep":true});
            bytes = encode(&v).unwrap();
        }
        put(&p.config, &bytes);
        let a = plan(&s, false).unwrap();
        apply(&s, &a.digest().unwrap()).unwrap();
        let r = plan(&s, true).unwrap();
        remove(&s, &r.digest().unwrap()).unwrap();
        assert!(
            String::from_utf8(fs::read(&p.config).unwrap())
                .unwrap()
                .contains("keep")
        );
        assert!(
            entry(client, &snapshot(&p.config).unwrap())
                .unwrap()
                .is_none()
        );
    }
}
#[test]
fn interrupted_apply_rechecks_source_binding_and_project_dependencies() {
    for mutation in 0..5 {
        let (_d, s) = fixture(Client::Claude);
        let p = paths(&s).unwrap();
        let a = plan(&s, false).unwrap();
        assert!(
            execute(&s, &a.digest().unwrap(), false, |_| Err(Failure::internal(
                "stop"
            )))
            .is_err()
        );
        let before = fs::read(&p.config).unwrap();
        match mutation {
            0 => fs::set_permissions(&s.credential, fs::Permissions::from_mode(0o644)).unwrap(),
            1 => put(&s.session, &[99; 32]),
            2 => put(
                &s.workspace.join(".mcp.json"),
                b"{\"mcpServers\":{\"locust\":{\"command\":\"other\"}}}",
            ),
            3 => {
                let u = super::super::uninstall_plan(&s.prefix).unwrap();
                super::super::uninstall(&s.prefix, &u.digest().unwrap()).unwrap();
            }
            _ => {
                let installed = super::super::status(&s.prefix).unwrap();
                let mut saved: super::super::Policy =
                    serde_json::from_slice(&fs::read(s.prefix.join("trust-state.json")).unwrap())
                        .unwrap();
                saved.withdrawals=encode(&json!({"format":"locust-withdrawals-v1","sequence":2,"withdrawn_manifest_sha256":[installed["manifest_sha256"]]})).unwrap();
                saved.signature = SigningKey::from_bytes(&[19; 32])
                    .sign(&saved.withdrawals)
                    .to_bytes()
                    .to_vec();
                put(&s.prefix.join("trust-state.json"), &encode(&saved).unwrap());
            }
        }
        assert!(apply(&s, &a.digest().unwrap()).is_err());
        assert!(p.intent.exists());
        assert_eq!(fs::read(&p.config).unwrap(), before);
        assert!(!p.skill.exists());
    }
}
#[test]
fn unowned_removal_keeps_empty_skill_directory_and_missing_client_directories() {
    let (_d, s) = fixture(Client::Pi);
    let p = paths(&s).unwrap();
    let r = plan(&s, true).unwrap();
    remove(&s, &r.digest().unwrap()).unwrap();
    assert!(!s.profile_home.join(".pi").exists());
    fs::create_dir_all(p.skill.parent().unwrap()).unwrap();
    let r = plan(&s, true).unwrap();
    remove(&s, &r.digest().unwrap()).unwrap();
    assert!(p.skill.parent().unwrap().exists());
}
#[test]
fn interrupted_removal_resumes_after_every_write() {
    for stop in 0..3 {
        let (_d, s) = fixture(Client::Pi);
        let p = paths(&s).unwrap();
        let a = plan(&s, false).unwrap();
        apply(&s, &a.digest().unwrap()).unwrap();
        let r = plan(&s, true).unwrap();
        let digest = r.digest().unwrap();
        assert!(
            execute(&s, &digest, true, |i| if i == stop {
                Err(Failure::internal("stop"))
            } else {
                Ok(())
            })
            .is_err()
        );
        assert_eq!(plan(&s, true).unwrap().digest().unwrap(), digest);
        remove(&s, &digest).unwrap();
        assert!(!p.config.exists());
        assert!(!p.skill.exists());
        assert!(!p.record.exists());
    }
}

#[test]
fn status_separates_owned_integrity_from_requested_binding_match() {
    let (_d, mut s) = fixture(Client::Codex);
    assert_eq!(status(&s).unwrap()["binding_matches"], false);
    let a = plan(&s, false).unwrap();
    apply(&s, &a.digest().unwrap()).unwrap();
    assert_eq!(status(&s).unwrap()["binding_matches"], true);
    let p = paths(&s).unwrap();
    let before = fs::read(&p.config).unwrap();
    s.session = s.session.with_file_name("another-session");
    let observed = status(&s).unwrap();
    assert_eq!(observed["binding_matches"], false);
    assert_eq!(observed["configured"], true);
    assert_eq!(observed["api_ready"], false);
    assert_eq!(fs::read(&p.config).unwrap(), before);
}

#[test]
fn interrupted_apply_can_be_removed_after_uninstall_without_old_secrets() {
    for stop in 0..3 {
        let (_d, s) = fixture(Client::Claude);
        let p = paths(&s).unwrap();
        let baseline = b"{\"unrelated\":\"keep\"}";
        put(&p.config, baseline);
        let a = plan(&s, false).unwrap();
        assert!(
            execute(&s, &a.digest().unwrap(), false, |i| if i == stop {
                Err(Failure::internal("stop apply"))
            } else {
                Ok(())
            })
            .is_err()
        );
        let uninstall = super::super::uninstall_plan(&s.prefix).unwrap();
        super::super::uninstall(&s.prefix, &uninstall.digest().unwrap()).unwrap();
        fs::remove_file(&s.credential).unwrap();
        fs::remove_file(&s.session).unwrap();
        assert!(plan(&s, false).is_err());
        let cleanup = plan(&s, true).unwrap();
        let digest = cleanup.digest().unwrap();
        assert!(
            execute(&s, &digest, true, |i| if i == stop {
                Err(Failure::internal("stop cleanup"))
            } else {
                Ok(())
            })
            .is_err()
        );
        assert_eq!(plan(&s, true).unwrap().digest().unwrap(), digest);
        remove(&s, &digest).unwrap();
        assert_eq!(fs::read(&p.config).unwrap(), baseline);
        assert!(!p.skill.exists());
        assert!(!p.record.exists());
        assert!(!p.intent.exists());
    }
}
#[test]
fn interrupted_reapply_cleanup_preserves_intervening_edits_and_refuses_unknown_bytes() {
    let (_d, s) = fixture(Client::Claude);
    let p = paths(&s).unwrap();
    let a = plan(&s, false).unwrap();
    apply(&s, &a.digest().unwrap()).unwrap();
    let mut config: Value = serde_json::from_slice(&fs::read(&p.config).unwrap()).unwrap();
    config["unrelated"] = json!("keep");
    put(&p.config, &encode(&config).unwrap());
    let a = plan(&s, false).unwrap();
    assert!(
        execute(&s, &a.digest().unwrap(), false, |_| Err(Failure::internal(
            "stop apply"
        )))
        .is_err()
    );
    let cleanup = plan(&s, true).unwrap();
    let digest = cleanup.digest().unwrap();
    assert!(
        execute(&s, &digest, true, |_| Err(Failure::internal(
            "stop cleanup"
        )))
        .is_err()
    );
    assert_eq!(plan(&s, true).unwrap().digest().unwrap(), digest);
    remove(&s, &digest).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&p.config).unwrap()).unwrap()["unrelated"],
        "keep"
    );
    assert!(
        entry(s.client, &snapshot(&p.config).unwrap())
            .unwrap()
            .is_none()
    );
    let a = plan(&s, false).unwrap();
    assert!(
        execute(&s, &a.digest().unwrap(), false, |_| Err(Failure::internal(
            "stop apply"
        )))
        .is_err()
    );
    put(&p.config, b"{\"user_modified\":true}");
    assert!(plan(&s, true).is_err());
    assert_eq!(fs::read(&p.config).unwrap(), b"{\"user_modified\":true}");
}

#[test]
fn generated_mcp_command_matches_actual_cli_definition() {
    for client in [Client::Codex, Client::Claude, Client::Pi] {
        let (_d, s) = fixture(client);
        let definition = desired(&s).unwrap();
        let (command, arguments) = if client == Client::Codex {
            let parsed: DocumentMut = definition.parse().unwrap();
            let server = &parsed["mcp_servers"]["locust"];
            (
                server["command"].as_str().unwrap().to_owned(),
                server["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect::<Vec<_>>(),
            )
        } else {
            let parsed: Value = serde_json::from_str(&definition).unwrap();
            (
                parsed["command"].as_str().unwrap().to_owned(),
                parsed["args"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect::<Vec<_>>(),
            )
        };
        let matches = crate::cli::command_for_test()
            .try_get_matches_from(std::iter::once(command).chain(arguments))
            .unwrap();
        assert_eq!(matches.subcommand_name(), Some("mcp"));
    }
}
