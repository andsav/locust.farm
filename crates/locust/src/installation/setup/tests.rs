use super::*;
use ed25519_dalek::{Signer, SigningKey};
fn fixture(client: Client) -> (tempfile::TempDir, SetupSpec) {
    let dir = tempfile::Builder::new()
        .prefix("lh.")
        .tempdir_in("/tmp")
        .unwrap();
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
    package::create_file(&root.join(package::MANUAL), b"manual", 0o644).unwrap();
    let bytes=encode(&json!({"format":"locust-release-v2","source_commit":"c".repeat(40),"version":"0.1.0","target":target,"machine_format":format,"api_version":locust_proto::API_VERSION,"protocol_version":locust_proto::PROTOCOL_VERSION,"toolchain":"1.96.1","files":[{"path":package::BINARY,"sha256":package::sha256(&binary),"size":binary.len(),"mode":493},{"path":package::SKILL,"sha256":package::sha256(b"signed skill"),"size":12,"mode":420},{"path":package::MANUAL,"sha256":package::sha256(b"manual"),"size":6,"mode":420}]})).unwrap();
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
    for client in [
        Client::Codex,
        Client::Claude,
        Client::Pi,
        Client::Droid,
        Client::Shell,
    ] {
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
        assert!(!p.launcher.exists());
    }
}
#[test]
fn remove_preserves_unrelated_edits_and_rejects_owned_edits() {
    for client in [
        Client::Codex,
        Client::Claude,
        Client::Pi,
        Client::Droid,
        Client::Shell,
    ] {
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
    for stop in 0..5 {
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
    for client in [
        Client::Codex,
        Client::Claude,
        Client::Pi,
        Client::Droid,
        Client::Shell,
    ] {
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
        assert!(!p.launcher.exists());
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
    for stop in 0..5 {
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
        assert!(!p.launcher.exists());
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
    for stop in 0..5 {
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
        assert!(!p.launcher.exists());
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
    for client in [
        Client::Codex,
        Client::Claude,
        Client::Pi,
        Client::Droid,
        Client::Shell,
    ] {
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

#[test]
fn launcher_is_owned_executable_and_skill_preserves_signed_frontmatter() {
    for client in [
        Client::Codex,
        Client::Claude,
        Client::Pi,
        Client::Droid,
        Client::Shell,
    ] {
        let (_d, s) = fixture(client);
        let p = paths(&s).unwrap();
        let source = fs::read(&s.skill_source).unwrap();
        let a = plan(&s, false).unwrap();
        assert_eq!(a.review["launcher"], p.launcher.to_str().unwrap());
        assert_eq!(
            a.review["files"].as_array().unwrap().len(),
            4 + usize::from(p.hook.is_some())
        );
        let result = apply(&s, &a.digest().unwrap()).unwrap();
        assert_eq!(result["launcher_ready"], true);
        assert_eq!(result["launcher"], p.launcher.to_str().unwrap());
        assert_eq!(fs::metadata(&p.launcher).unwrap().mode() & 0o777, 0o700);
        let skill = fs::read_to_string(&p.skill).unwrap();
        assert!(skill.contains(p.launcher.to_str().unwrap()));
        assert!(skill.ends_with(std::str::from_utf8(&source).unwrap()));
        assert_eq!(fs::read(&s.skill_source).unwrap(), source);
        assert!(!skill.contains(s.credential.to_str().unwrap()));
        assert!(!skill.contains(s.session.to_str().unwrap()));
    }
    let (_d, s) = fixture(Client::Codex);
    for newline in ["\n", "\r\n"] {
        let frontmatter = ["---", "name: locust", "description: Keep me", "---", ""].join(newline);
        let body = "\n# Original body\nAn unchanged command and example.\n";
        let source = format!("{frontmatter}{body}");
        let installed = String::from_utf8(
            launcher::skill(source.as_bytes(), Path::new("/profile/locust-cli"), &s).unwrap(),
        )
        .unwrap();
        assert!(installed.starts_with(&frontmatter));
        assert!(installed.ends_with(body));
        assert!(installed.contains("# Your owner's commands"));
        assert!(
            installed.find("# Installed Locust CLI").unwrap()
                < installed.find("# Original body").unwrap()
        );
    }
    assert!(launcher::skill(b"---\nname: locust\n", Path::new("/cli"), &s).is_err());
    assert!(launcher::skill(&[0xff], Path::new("/cli"), &s).is_err());
}

#[test]
fn launcher_quotes_paths_and_preserves_arguments_without_environment_binding() {
    let (_d, mut s) = fixture(Client::Codex);
    let base = s.profile_home.parent().unwrap().to_path_buf();
    s.executable = base.join("cli ' $(touch injected) `touch injected`");
    s.daemon_home = base.join("daemon ' $HOME `uname`");
    s.credential = base.join("credential ' with spaces");
    s.session = base.join("session $value");
    package::create_file(&s.executable, b"#!/bin/sh\nprintf '%s\\0' \"$@\"\n", 0o700).unwrap();
    let script = base.join("launcher");
    package::create_file(&script, &launcher::render(&s).unwrap(), 0o700).unwrap();
    let arguments = [
        "contribution",
        "publish",
        "--goal",
        &"ab".repeat(32),
        "a 'quoted' $value; `not a command`\nnext line",
    ];
    let output = std::process::Command::new(&script)
        .args(arguments)
        .env_clear()
        .env("PATH", "/unavailable")
        .env("LOCUST_HOME", "/wrong-home")
        .env("LOCUST_CREDENTIAL", "/wrong-credential")
        .env("LOCUST_SESSION", "/wrong-session")
        .current_dir(&base)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let values: Vec<_> = output
        .stdout
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| std::str::from_utf8(s).unwrap())
        .collect();
    let mut expected = vec![
        "--home",
        s.daemon_home.to_str().unwrap(),
        "--credential",
        s.credential.to_str().unwrap(),
        "--session",
        s.session.to_str().unwrap(),
    ];
    expected.extend(arguments);
    assert_eq!(values, expected);
    assert!(!base.join("injected").exists());
    let parsed = crate::cli::command_for_test()
        .try_get_matches_from(std::iter::once("locust").chain(values))
        .unwrap();
    assert_eq!(
        parsed.get_one::<String>("home").unwrap(),
        s.daemon_home.to_str().unwrap()
    );
    assert_eq!(
        parsed.get_one::<String>("credential").unwrap(),
        s.credential.to_str().unwrap()
    );
    assert_eq!(
        parsed.get_one::<String>("session").unwrap(),
        s.session.to_str().unwrap()
    );
}

#[test]
fn launcher_refuses_authority_and_binding_overrides_anywhere() {
    let (_d, mut s) = fixture(Client::Claude);
    let base = s.profile_home.parent().unwrap();
    s.executable = base.join("probe");
    package::create_file(
        &s.executable,
        b"#!/bin/sh\nprintf 'unexpected invocation'\n",
        0o700,
    )
    .unwrap();
    let script = base.join("launcher");
    package::create_file(&script, &launcher::render(&s).unwrap(), 0o700).unwrap();
    for flag in ["--owner", "--agent", "--home", "--credential", "--session"] {
        for argument in [flag.to_owned(), format!("{flag}=/override")] {
            for prefix in [
                vec![],
                vec!["status"],
                vec!["task", "show"],
                vec!["client", "run", "--prompt", "--"],
            ] {
                let output = std::process::Command::new(&script)
                    .args(prefix)
                    .arg(&argument)
                    .output()
                    .unwrap();
                assert_eq!(output.status.code(), Some(2), "{argument}");
                assert!(output.stdout.is_empty());
                assert!(String::from_utf8_lossy(&output.stderr).contains("bound to one agent"));
            }
        }
    }
}

#[test]
fn modified_or_unowned_launchers_are_preserved() {
    for mode_only in [false, true] {
        let (_d, s) = fixture(Client::Pi);
        let p = paths(&s).unwrap();
        let a = plan(&s, false).unwrap();
        apply(&s, &a.digest().unwrap()).unwrap();
        if mode_only {
            fs::set_permissions(&p.launcher, fs::Permissions::from_mode(0o600)).unwrap();
        } else {
            put(&p.launcher, b"user launcher");
        }
        let before = snapshot(&p.launcher).unwrap();
        assert_eq!(status(&s).unwrap()["configured"], false);
        assert_eq!(status(&s).unwrap()["launcher_ready"], false);
        assert!(plan(&s, false).is_err());
        assert!(plan(&s, true).is_err());
        assert_eq!(snapshot(&p.launcher).unwrap(), before);
    }
    let (_d, s) = fixture(Client::Claude);
    let p = paths(&s).unwrap();
    put(&p.launcher, b"unowned");
    assert!(plan(&s, false).is_err());
    let cleanup = plan(&s, true).unwrap();
    remove(&s, &cleanup.digest().unwrap()).unwrap();
    assert_eq!(fs::read(&p.launcher).unwrap(), b"unowned");
    assert!(!p.skill.exists());
    assert!(!p.record.exists());
}

fn setup_images(s: &SetupSpec) -> Vec<Image> {
    let p = paths(s).unwrap();
    [
        &p.config,
        &p.skill,
        &p.launcher,
        &p.record,
        &p.intent,
        &s.credential,
        &s.session,
        &s.skill_source,
    ]
    .into_iter()
    .chain(p.hook.as_ref())
    .map(|path| snapshot(path).unwrap())
    .collect()
}

#[test]
fn unsupported_ownership_formats_refuse_every_operation_without_mutation() {
    for client in [
        Client::Codex,
        Client::Claude,
        Client::Pi,
        Client::Droid,
        Client::Shell,
    ] {
        for format in [
            "locust-setup-owner-v1",
            "locust-setup-owner-v2",
            "unknown-setup-format",
        ] {
            let (_d, s) = fixture(client);
            let initial = plan(&s, false).unwrap();
            apply(&s, &initial.digest().unwrap()).unwrap();
            let p = paths(&s).unwrap();
            let mut record: serde_json::Value =
                serde_json::from_slice(&fs::read(&p.record).unwrap()).unwrap();
            record["format"] = json!(format);
            atomic(&p.record, &encode(&record).unwrap()).unwrap();
            let before = setup_images(&s);
            for result in [
                status(&s),
                plan(&s, false).map(|_| Value::Null),
                plan(&s, true).map(|_| Value::Null),
                apply(&s, &initial.digest().unwrap()),
                remove(&s, &initial.digest().unwrap()),
            ] {
                assert_eq!(
                    result.unwrap_err().code,
                    locust_proto::api::ErrorCode::Corrupted
                );
                assert_eq!(setup_images(&s), before);
            }
        }
    }
}

#[test]
fn unsupported_pending_journal_formats_refuse_before_resuming_or_cleanup() {
    for client in [
        Client::Codex,
        Client::Claude,
        Client::Pi,
        Client::Droid,
        Client::Shell,
    ] {
        let (_directory, initial_spec) = fixture(client);
        let change_count = prepare(&initial_spec, false).unwrap().changes.len();
        for applied_paths in 0..=change_count {
            let (_d, s) = fixture(client);
            let p = paths(&s).unwrap();
            let mut tx = prepare(&s, false).unwrap();
            for change in &tx.changes[..applied_paths] {
                write_change(&s, change).unwrap();
            }
            tx.plan.review["format"] = json!("locust-setup-plan-v1");
            private_dir(&s.prefix.join("setup"), true).unwrap();
            atomic(&p.intent, &encode(&tx).unwrap()).unwrap();
            let before = setup_images(&s);
            for result in [
                plan(&s, false).map(|_| Value::Null),
                plan(&s, true).map(|_| Value::Null),
                apply(&s, &tx.plan.digest().unwrap()),
                remove(&s, &tx.plan.digest().unwrap()),
            ] {
                assert_eq!(
                    result.unwrap_err().code,
                    locust_proto::api::ErrorCode::Corrupted
                );
                assert_eq!(setup_images(&s), before);
            }
        }
    }
}

#[test]
fn current_journal_refuses_unsupported_intended_ownership_without_mutation() {
    let (_d, s) = fixture(Client::Codex);
    let p = paths(&s).unwrap();
    let mut tx = prepare(&s, false).unwrap();
    let owned = tx
        .changes
        .iter_mut()
        .find(|change| change.path == p.record)
        .unwrap();
    let mut record: Value = serde_json::from_slice(owned.after.bytes.as_ref().unwrap()).unwrap();
    record["format"] = json!("locust-setup-owner-v1");
    owned.after.bytes = Some(encode(&record).unwrap());
    private_dir(&s.prefix.join("setup"), true).unwrap();
    atomic(&p.intent, &encode(&tx).unwrap()).unwrap();
    let before = setup_images(&s);
    for result in [
        plan(&s, false).map(|_| Value::Null),
        plan(&s, true).map(|_| Value::Null),
        apply(&s, &tx.plan.digest().unwrap()),
        remove(&s, &tx.plan.digest().unwrap()),
    ] {
        assert_eq!(
            result.unwrap_err().code,
            locust_proto::api::ErrorCode::Corrupted
        );
        assert_eq!(setup_images(&s), before);
    }
}

#[test]
fn launcher_binding_changes_require_a_new_plan_and_update_only_owned_content() {
    let (_d, mut s) = fixture(Client::Codex);
    let p = paths(&s).unwrap();
    let old_plan = plan(&s, false).unwrap();
    apply(&s, &old_plan.digest().unwrap()).unwrap();
    let old_launcher = fs::read(&p.launcher).unwrap();
    let old_session = s.session.clone();
    s.session = s.session.with_file_name("new session");
    package::create_file(&s.session, &[81; 32], 0o600).unwrap();
    assert!(apply(&s, &old_plan.digest().unwrap()).is_err());
    assert_eq!(fs::read(&p.launcher).unwrap(), old_launcher);
    let new_plan = plan(&s, false).unwrap();
    apply(&s, &new_plan.digest().unwrap()).unwrap();
    let new_launcher = fs::read_to_string(&p.launcher).unwrap();
    assert!(new_launcher.contains(s.session.to_str().unwrap()));
    assert!(old_session.exists());
    assert_eq!(status(&s).unwrap()["binding_matches"], true);
    assert_eq!(status(&s).unwrap()["launcher_ready"], true);
}

#[test]
fn portable_setup_provides_scoped_cli_without_writing_a_native_client_profile() {
    let (_directory, spec) = fixture(Client::Shell);
    let reviewed = plan(&spec, false).unwrap();
    apply(&spec, &reviewed.digest().unwrap()).unwrap();
    let portable = spec.profile_home.join(".local/share/locust-agent");
    let document: Value =
        serde_json::from_slice(&fs::read(portable.join("mcp.json")).unwrap()).unwrap();
    let server = &document["mcpServers"]["locust"];
    assert_eq!(server["type"], "stdio");
    assert_eq!(server["command"], spec.executable.to_str().unwrap());
    assert_eq!(
        server["env"]["LOCUST_HOME"],
        spec.daemon_home.to_str().unwrap()
    );
    assert_eq!(
        server["env"]["LOCUST_CREDENTIAL"],
        spec.credential.to_str().unwrap()
    );
    assert_eq!(
        server["env"]["LOCUST_SESSION"],
        spec.session.to_str().unwrap()
    );
    assert!(portable.join("skills/locust/locust-cli").is_file());
    assert!(
        fs::read_to_string(portable.join("skills/locust/SKILL.md"))
            .unwrap()
            .contains("locust-cli")
    );
    for path in [
        ".codex",
        ".agents",
        ".claude",
        ".claude.json",
        ".factory",
        ".pi",
    ] {
        assert!(!spec.profile_home.join(path).exists(), "{path}");
    }
}

#[test]
fn droid_setup_refuses_ancestor_overrides_before_writing_its_profile() {
    let (_directory, spec) = fixture(Client::Droid);
    let project = spec.workspace.join(".factory/mcp.json");
    put(
        &project,
        br#"{"mcpServers":{"locust":{"command":"other"}}}"#,
    );
    assert_eq!(
        plan(&spec, false).err().unwrap().code,
        locust_proto::api::ErrorCode::Conflict
    );
    assert!(!spec.profile_home.join(".factory").exists());
    assert_eq!(
        fs::read(&project).unwrap(),
        br#"{"mcpServers":{"locust":{"command":"other"}}}"#
    );
}

#[test]
fn native_hooks_roundtrip_exact_bytes_and_absent_files() {
    for client in [Client::Codex, Client::Claude, Client::Droid] {
        for baseline in [
            None,
            Some(b"{ \"unrelated\": true, \"hooks\": { \"Stop\": [] } }\n".as_slice()),
        ] {
            let (_directory, spec) = fixture(client);
            let p = paths(&spec).unwrap();
            let hook = p.hook.as_ref().unwrap();
            if let Some(bytes) = baseline {
                put(
                    hook,
                    if client == Client::Droid {
                        b"{  \"Stop\": [], \"Notification\": [] }\n"
                    } else {
                        bytes
                    },
                );
                fs::set_permissions(hook, fs::Permissions::from_mode(0o640)).unwrap();
            }
            let original = snapshot(hook).unwrap();
            let reviewed = plan(&spec, false).unwrap();
            assert_eq!(reviewed.review["format"], "locust-setup-plan-v3");
            assert_eq!(reviewed.review["hook_config"], hook.to_str().unwrap());
            apply(&spec, &reviewed.digest().unwrap()).unwrap();
            let (_, record) = read_record(&p).unwrap();
            let record = record.unwrap();
            assert_eq!(record.format, "locust-setup-owner-v3");
            assert_eq!(record.hooks.len(), 1);
            assert!(
                hooks::installed(
                    &hook_document(client, &snapshot(hook).unwrap()).unwrap(),
                    &record.hooks[0].registration
                )
                .unwrap()
            );
            assert_eq!(status(&spec).unwrap()["hooks_ready"], true);
            let reviewed = plan(&spec, true).unwrap();
            remove(&spec, &reviewed.digest().unwrap()).unwrap();
            assert_eq!(snapshot(hook).unwrap(), original);
        }
    }
}

fn put_hook_document(client: Client, path: &Path, document: &Value) {
    let bytes = hooks::render_configuration(adapter_client(client).unwrap(), document)
        .unwrap()
        .expect("JSON adapter renders native bytes");
    put(path, &bytes);
}

#[test]
fn hook_removal_and_reapply_preserve_unrelated_entries() {
    for client in [Client::Codex, Client::Claude, Client::Droid] {
        for reapply in [false, true] {
            let (_directory, spec) = fixture(client);
            let p = paths(&spec).unwrap();
            let hook = p.hook.as_ref().unwrap();
            let reviewed = plan(&spec, false).unwrap();
            apply(&spec, &reviewed.digest().unwrap()).unwrap();
            let mut config = hook_document(client, &snapshot(hook).unwrap()).unwrap();
            let other = json!({"hooks":[{"type":"command","command":"other-agent-hook"}]});
            config["hooks"]["Stop"]
                .as_array_mut()
                .unwrap()
                .push(other.clone());
            let other_event =
                json!({"hooks":[{"type":"command","command":"unrelated-notification-hook"}]});
            config["hooks"]["Notification"] = json!([other_event.clone()]);
            put_hook_document(client, hook, &config);
            if reapply {
                let before = snapshot(hook).unwrap();
                let reviewed = plan(&spec, false).unwrap();
                apply(&spec, &reviewed.digest().unwrap()).unwrap();
                assert_eq!(snapshot(hook).unwrap(), before);
            }
            let reviewed = plan(&spec, true).unwrap();
            remove(&spec, &reviewed.digest().unwrap()).unwrap();
            let after = hook_document(client, &snapshot(hook).unwrap()).unwrap();
            assert_eq!(
                after,
                json!({"hooks":{"Stop":[other],"Notification":[other_event]}})
            );
        }
    }
}

#[test]
fn hand_removed_hook_and_hook_file_stay_out_across_reapply_and_remove() {
    for client in [Client::Codex, Client::Claude, Client::Droid] {
        for remove_file in [false, true] {
            let (_directory, spec) = fixture(client);
            let p = paths(&spec).unwrap();
            let hook = p.hook.as_ref().unwrap();
            let reviewed = plan(&spec, false).unwrap();
            apply(&spec, &reviewed.digest().unwrap()).unwrap();
            let (_, record) = read_record(&p).unwrap();
            let removed = record.unwrap().hooks[0].registration.entries()[0].clone();
            if remove_file {
                fs::remove_file(hook).unwrap();
            } else {
                let mut value = hook_document(client, &snapshot(hook).unwrap()).unwrap();
                value["hooks"]
                    .as_object_mut()
                    .unwrap()
                    .remove(&removed.native_event);
                put_hook_document(client, hook, &value);
            }
            assert_eq!(status(&spec).unwrap()["hooks_ready"], false);
            let before = snapshot(hook).unwrap();
            let reviewed = plan(&spec, false).unwrap();
            apply(&spec, &reviewed.digest().unwrap()).unwrap();
            assert_eq!(snapshot(hook).unwrap(), before);
            let (_, record) = read_record(&p).unwrap();
            let owned = record.unwrap().hooks.remove(0);
            assert!(!owned.registration.entries().contains(&removed));
            if remove_file {
                assert!(owned.registration.entries().is_empty());
            }
            assert_eq!(status(&spec).unwrap()["hooks_ready"], true);
            let reviewed = plan(&spec, true).unwrap();
            remove(&spec, &reviewed.digest().unwrap()).unwrap();
            assert!(!p.record.exists());
            if remove_file {
                assert!(!hook.exists());
            } else {
                let value = hook_document(client, &snapshot(hook).unwrap()).unwrap();
                assert!(!value.to_string().contains("Locust"));
            }
        }
    }
}

#[test]
fn modified_owned_hooks_refuse_apply_remove_and_preserve_bytes() {
    for client in [Client::Codex, Client::Claude, Client::Droid] {
        let (_directory, spec) = fixture(client);
        let p = paths(&spec).unwrap();
        let hook = p.hook.as_ref().unwrap();
        let reviewed = plan(&spec, false).unwrap();
        apply(&spec, &reviewed.digest().unwrap()).unwrap();
        let mut value = hook_document(client, &snapshot(hook).unwrap()).unwrap();
        value["hooks"]["Stop"][0]["hooks"][0]["command"] = json!("user-edited-hook");
        put_hook_document(client, hook, &value);
        let before = setup_images(&spec);
        assert_eq!(status(&spec).unwrap()["hooks_ready"], false);
        assert!(plan(&spec, false).is_err());
        assert!(plan(&spec, true).is_err());
        assert_eq!(setup_images(&spec), before);
    }
}

#[test]
fn unowned_hook_collisions_refuse_without_mutation() {
    for client in [Client::Codex, Client::Claude, Client::Droid] {
        let (_directory, spec) = fixture(client);
        let p = paths(&spec).unwrap();
        let hook = p.hook.as_ref().unwrap();
        let registration =
            hooks::registration(adapter_client(client).unwrap(), &p.launcher).unwrap();
        let existing = hooks::install(&json!({}), &registration).unwrap();
        put_hook_document(client, hook, &existing);
        let before = setup_images(&spec);
        assert!(preflight_new(&spec).is_err());
        assert!(plan(&spec, false).is_err());
        assert_eq!(setup_images(&spec), before);
        let reviewed = plan(&spec, true).unwrap();
        remove(&spec, &reviewed.digest().unwrap()).unwrap();
        assert_eq!(
            hook_document(client, &snapshot(hook).unwrap()).unwrap(),
            existing
        );
    }
}

#[test]
fn a_linked_hook_file_turns_hooks_off_and_setup_goes_on() {
    for client in [Client::Codex, Client::Claude, Client::Droid, Client::Pi] {
        let (directory, spec) = fixture(client);
        let hook = paths(&spec).unwrap().hook_path.unwrap();
        // A dotfile manager keeps the real file elsewhere and links it here.
        let managed = directory.path().join("dotfiles-settings");
        put(&managed, b"{\"managed\": true}\n");
        fs::create_dir_all(hook.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&managed, &hook).unwrap();
        let p = paths(&spec).unwrap();
        assert!(p.hook.is_none());
        let reason = p.hooks_skipped.clone().unwrap();
        assert!(reason.contains("hooks are off for this client"));
        assert!(preflight_new(&spec).is_ok());
        let reviewed = plan(&spec, false).unwrap();
        assert_eq!(reviewed.review["hooks_skipped"], reason);
        assert_eq!(reviewed.review["hook_trust_review_required"], false);
        let applied = apply(&spec, &reviewed.digest().unwrap()).unwrap();
        assert_eq!(applied["hooks_skipped"], reason);
        let state = status(&spec).unwrap();
        assert_eq!(state["mcp_ready"], true);
        assert_eq!(state["hooks_ready"], false);
        assert_eq!(state["hooks_skipped"], reason);
        assert_eq!(state["configured"], true);
        assert!(read_record(&p).unwrap().1.unwrap().hooks.is_empty());
        assert_eq!(fs::read(&managed).unwrap(), b"{\"managed\": true}\n");
        assert!(
            fs::symlink_metadata(&hook)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        // Made a regular file, the next apply installs the hooks.
        fs::remove_file(&hook).unwrap();
        let reviewed = plan(&spec, false).unwrap();
        assert!(reviewed.review["hooks_skipped"].is_null());
        apply(&spec, &reviewed.digest().unwrap()).unwrap();
        assert_eq!(status(&spec).unwrap()["hooks_ready"], true);
        // Linked again after install: setup still goes on and removes cleanly.
        let installed = fs::read(&hook).unwrap();
        fs::remove_file(&hook).unwrap();
        put(&managed, &installed);
        std::os::unix::fs::symlink(&managed, &hook).unwrap();
        assert_eq!(status(&spec).unwrap()["hooks_ready"], false);
        let reviewed = plan(&spec, true).unwrap();
        remove(&spec, &reviewed.digest().unwrap()).unwrap();
        assert!(!p.record.exists());
        assert_eq!(fs::read(&managed).unwrap(), installed);
        assert_eq!(fs::read(&spec.session).unwrap(), vec![18; 32]);
    }
}

#[test]
fn applying_again_brings_installed_hooks_up_to_this_release() {
    for client in [Client::Codex, Client::Claude, Client::Droid] {
        let (_directory, spec) = fixture(client);
        let p = paths(&spec).unwrap();
        let hook = p.hook.as_ref().unwrap();
        let reviewed = plan(&spec, false).unwrap();
        apply(&spec, &reviewed.digest().unwrap()).unwrap();
        let fresh = hooks::registration(adapter_client(client).unwrap(), &p.launcher).unwrap();
        // Make the install look like an older release's: other time limits
        // and no PostToolUse event yet.
        let HookRegistration::JsonGroups { entries } = &fresh else {
            panic!("JSON hooks");
        };
        let old_entries: Vec<_> = entries
            .iter()
            .filter(|entry| entry.native_event != "PostToolUse")
            .cloned()
            .map(|mut entry| {
                entry.group["hooks"][0]["timeout"] = json!(600);
                entry
            })
            .collect();
        let old = HookRegistration::JsonGroups {
            entries: old_entries,
        };
        let document =
            json!({"hooks":{"Notification":[{"hooks":[{"type":"command","command":"mine"}]}]}});
        put_hook_document(client, hook, &document);
        let original = snapshot(hook).unwrap();
        let older = hooks::install(&document, &old).unwrap();
        put_hook_document(client, hook, &older);
        let (_, record) = read_record(&p).unwrap();
        let mut record = record.unwrap();
        record.hooks[0].registration = old;
        record.hooks[0].original = original;
        record.hooks[0].config = snapshot(hook).unwrap();
        put(&p.record, &encode(&record).unwrap());
        assert_eq!(status(&spec).unwrap()["hooks_ready"], false);
        let reviewed = plan(&spec, false).unwrap();
        apply(&spec, &reviewed.digest().unwrap()).unwrap();
        let current = hook_document(client, &snapshot(hook).unwrap()).unwrap();
        assert!(hooks::installed(&current, &fresh).unwrap());
        assert_eq!(
            current["hooks"]["Notification"],
            document["hooks"]["Notification"]
        );
        assert_eq!(status(&spec).unwrap()["hooks_ready"], true);
        // Applying the same release again changes nothing.
        let before = snapshot(hook).unwrap();
        let reviewed = plan(&spec, false).unwrap();
        apply(&spec, &reviewed.digest().unwrap()).unwrap();
        assert_eq!(snapshot(hook).unwrap(), before);
        let reviewed = plan(&spec, true).unwrap();
        remove(&spec, &reviewed.digest().unwrap()).unwrap();
        assert_eq!(
            hook_document(client, &snapshot(hook).unwrap()).unwrap(),
            document
        );
    }
}

#[test]
fn applying_again_replaces_an_older_pi_shim_it_owns() {
    let (_directory, spec) = fixture(Client::Pi);
    let p = paths(&spec).unwrap();
    let hook = p.hook.as_ref().unwrap();
    let reviewed = plan(&spec, false).unwrap();
    apply(&spec, &reviewed.digest().unwrap()).unwrap();
    let fresh = fs::read(hook).unwrap();
    put(hook, b"// an older release's shim\n");
    let (_, record) = read_record(&p).unwrap();
    let mut record = record.unwrap();
    record.hooks[0].registration = HookRegistration::OwnedSource {
        source: Some("// an older release's shim\n".into()),
    };
    record.hooks[0].config = snapshot(hook).unwrap();
    put(&p.record, &encode(&record).unwrap());
    assert_eq!(status(&spec).unwrap()["hooks_ready"], false);
    let reviewed = plan(&spec, false).unwrap();
    apply(&spec, &reviewed.digest().unwrap()).unwrap();
    assert_eq!(fs::read(hook).unwrap(), fresh);
    assert_eq!(status(&spec).unwrap()["hooks_ready"], true);
}

#[test]
fn hook_edits_invalidate_review_and_pending_unknown_state() {
    for client in [Client::Codex, Client::Claude, Client::Droid] {
        let (_directory, spec) = fixture(client);
        let p = paths(&spec).unwrap();
        let hook = p.hook.as_ref().unwrap();
        let reviewed = plan(&spec, false).unwrap();
        put_hook_document(client, hook, &json!({"hooks":{"Notification":[]}}));
        let before = setup_images(&spec);
        assert!(apply(&spec, &reviewed.digest().unwrap()).is_err());
        assert_eq!(setup_images(&spec), before);
        let reviewed = plan(&spec, false).unwrap();
        assert!(
            execute(&spec, &reviewed.digest().unwrap(), false, |_| Err(
                Failure::internal("stop")
            ))
            .is_err()
        );
        put_hook_document(
            client,
            hook,
            &json!({"hooks":{"Notification":[{"hooks":[{"type":"command","command":"unexpected-state-hook"}]}]}}),
        );
        let before = setup_images(&spec);
        assert!(plan(&spec, false).is_err());
        assert!(plan(&spec, true).is_err());
        assert_eq!(setup_images(&spec), before);
    }
}

#[test]
fn v2_ownership_and_journal_refusal_reports_recovery_without_legacy_reader() {
    for journal in [false, true] {
        let (_directory, spec) = fixture(Client::Claude);
        let p = paths(&spec).unwrap();
        let mut tx = prepare(&spec, false).unwrap();
        private_dir(&spec.prefix.join("setup"), true).unwrap();
        if journal {
            tx.plan.review["format"] = json!("locust-setup-plan-v2");
            atomic(&p.intent, &encode(&tx).unwrap()).unwrap();
        } else {
            let owned = tx
                .changes
                .iter()
                .find(|change| change.path == p.record)
                .unwrap();
            let mut value: Value =
                serde_json::from_slice(owned.after.bytes.as_ref().unwrap()).unwrap();
            value["format"] = json!("locust-setup-owner-v2");
            value.as_object_mut().unwrap().remove("hooks");
            atomic(&p.record, &encode(&value).unwrap()).unwrap();
        }
        let before = setup_images(&spec);
        for remove in [false, true] {
            let error = plan(&spec, remove).err().unwrap();
            assert_eq!(error.code, locust_proto::api::ErrorCode::Corrupted);
            assert!(error.message.contains("restore the release"));
            assert!(error.message.contains("original binding"));
            assert!(error.message.contains("setup remove"));
            assert_eq!(setup_images(&spec), before);
        }
    }
}

#[test]
fn launcher_forwards_native_hook_stdin_and_bound_cli_arguments() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let (_directory, mut spec) = fixture(Client::Claude);
    let base = spec.profile_home.parent().unwrap();
    spec.executable = base.join("hook-probe");
    package::create_file(
        &spec.executable,
        b"#!/bin/sh\nprintf '%s\\0' \"$@\"\n/bin/cat\n",
        0o700,
    )
    .unwrap();
    let script = base.join("hook-launcher");
    package::create_file(&script, &launcher::render(&spec).unwrap(), 0o700).unwrap();
    let mut child = Command::new(&script)
        .args(["hook", "stop", "--harness", "claude"])
        .env_clear()
        .env("HOME", base)
        .env("PATH", "/unavailable")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let input = b"{\"hook_event_name\":\"Stop\",\"session_id\":\"native-chat\"}";
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let pieces: Vec<_> = output.stdout.split(|byte| *byte == 0).collect();
    assert_eq!(pieces.last().unwrap(), &input.as_slice());
    let args: Vec<_> = pieces[..pieces.len() - 1]
        .iter()
        .map(|bytes| std::str::from_utf8(bytes).unwrap())
        .collect();
    let matches = crate::cli::command_for_test()
        .try_get_matches_from(std::iter::once("locust").chain(args))
        .unwrap();
    assert_eq!(matches.subcommand_name(), Some("hook"));
    assert_eq!(
        matches.get_one::<String>("session").unwrap(),
        spec.session.to_str().unwrap()
    );
}

#[test]
fn pi_owned_extension_roundtrip_and_mode_only_edit_remove_the_file() {
    for mode in [0o600, 0o640] {
        let (_directory, spec) = fixture(Client::Pi);
        let p = paths(&spec).unwrap();
        let hook = p.hook.as_ref().unwrap();
        assert!(hook.ends_with(".pi/agent/extensions/locust.ts"));
        let reviewed = plan(&spec, false).unwrap();
        assert_eq!(reviewed.review["files"].as_array().unwrap().len(), 5);
        apply(&spec, &reviewed.digest().unwrap()).unwrap();
        let source = fs::read_to_string(hook).unwrap();
        assert!(source.contains("export default function locust"));
        assert!(source.contains(p.launcher.to_str().unwrap()));
        assert_eq!(status(&spec).unwrap()["hooks_ready"], true);
        fs::set_permissions(hook, fs::Permissions::from_mode(mode)).unwrap();
        let reviewed = plan(&spec, true).unwrap();
        remove(&spec, &reviewed.digest().unwrap()).unwrap();
        assert!(
            !hook.exists(),
            "an absent extension must not become an empty .ts file"
        );
        assert!(!p.record.exists());
    }
}

#[test]
fn pi_empty_unowned_and_edited_owned_extensions_conflict_without_mutation() {
    for bytes in [b"".as_slice(), b"export default () => {};\n".as_slice()] {
        let (_directory, spec) = fixture(Client::Pi);
        let p = paths(&spec).unwrap();
        let hook = p.hook.as_ref().unwrap();
        put(hook, bytes);
        let before = setup_images(&spec);
        assert!(preflight_new(&spec).is_err());
        assert!(plan(&spec, false).is_err());
        assert_eq!(setup_images(&spec), before);
        let reviewed = plan(&spec, true).unwrap();
        remove(&spec, &reviewed.digest().unwrap()).unwrap();
        assert_eq!(fs::read(hook).unwrap(), bytes);
    }
    let (_directory, spec) = fixture(Client::Pi);
    let p = paths(&spec).unwrap();
    let reviewed = plan(&spec, false).unwrap();
    apply(&spec, &reviewed.digest().unwrap()).unwrap();
    put(
        p.hook.as_ref().unwrap(),
        b"// person's own extension edit\nexport default () => {};\n",
    );
    let before = setup_images(&spec);
    assert_eq!(status(&spec).unwrap()["hooks_ready"], false);
    assert!(plan(&spec, false).is_err());
    assert!(plan(&spec, true).is_err());
    assert_eq!(setup_images(&spec), before);
}

#[test]
fn pi_hand_removed_source_stays_out_and_recreated_user_source_is_preserved() {
    for recreate in [false, true] {
        let (_directory, spec) = fixture(Client::Pi);
        let p = paths(&spec).unwrap();
        let hook = p.hook.as_ref().unwrap();
        let reviewed = plan(&spec, false).unwrap();
        apply(&spec, &reviewed.digest().unwrap()).unwrap();
        fs::remove_file(hook).unwrap();
        assert_eq!(status(&spec).unwrap()["hooks_ready"], false);
        let reviewed = plan(&spec, false).unwrap();
        apply(&spec, &reviewed.digest().unwrap()).unwrap();
        assert!(!hook.exists());
        let (_, record) = read_record(&p).unwrap();
        assert!(matches!(
            record.unwrap().hooks[0].registration,
            hooks::HookRegistration::OwnedSource { source: None }
        ));
        let user_source = b"// recreated user source\nexport default () => {};\n";
        if recreate {
            put(hook, user_source);
        }
        let reviewed = plan(&spec, true).unwrap();
        remove(&spec, &reviewed.digest().unwrap()).unwrap();
        if recreate {
            assert_eq!(fs::read(hook).unwrap(), user_source);
        } else {
            assert!(!hook.exists());
        }
    }
}

#[test]
fn pi_source_install_and_pending_cleanup_resume_every_owned_write() {
    for stop in 0..5 {
        for cleanup in [false, true] {
            let (_directory, spec) = fixture(Client::Pi);
            let p = paths(&spec).unwrap();
            let reviewed = plan(&spec, false).unwrap();
            let digest = reviewed.digest().unwrap();
            assert!(
                execute(&spec, &digest, false, |index| {
                    if index == stop {
                        Err(Failure::internal("source interruption"))
                    } else {
                        Ok(())
                    }
                })
                .is_err()
            );
            if cleanup {
                let reviewed = plan(&spec, true).unwrap();
                remove(&spec, &reviewed.digest().unwrap()).unwrap();
                assert!(!p.hook.as_ref().unwrap().exists());
                assert!(!p.record.exists());
            } else {
                assert_eq!(plan(&spec, false).unwrap().digest().unwrap(), digest);
                apply(&spec, &digest).unwrap();
                assert!(p.hook.as_ref().unwrap().exists());
                assert_eq!(status(&spec).unwrap()["hooks_ready"], true);
            }
            assert!(!p.intent.exists());
        }
    }
}
