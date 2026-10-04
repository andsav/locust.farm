use super::*;
use crate::daemon::{EngineInit, run_with};
use ed25519_dalek::{Signer, SigningKey};
use locust_core::node::Node;
use locust_proto::engine::Entropy;
use locust_store::SqliteStore;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

struct SuppliedEntropy(Box<dyn Entropy>);
impl Entropy for SuppliedEntropy {
    fn fill(&mut self, bytes: &mut [u8]) {
        self.0.fill(bytes);
    }
}

/// The actual core and SQLite store, served through the daemon's Unix socket.
/// Shutdown is injected so no native service or network listener is needed.
struct Running {
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<JoinHandle<Result<(), Failure>>>,
}
impl Running {
    fn start(home: &Path) -> Self {
        let home = home.to_path_buf();
        let (shutdown, stopped) = tokio::sync::oneshot::channel();
        let (ready, waiting) = mpsc::channel();
        let thread = thread::spawn(move || {
            run_with(
                &home,
                |init: EngineInit| {
                    let store = SqliteStore::open(&init.home).map_err(|e| e.to_string())?;
                    Node::open(
                        store,
                        SuppliedEntropy(init.entropy),
                        init.owner_digest,
                        init.daemon_version,
                        0,
                    )
                    .map_err(|e| e.to_string())
                },
                move |_| {
                    ready.send(()).unwrap();
                    Ok(async move {
                        let _ = stopped.await;
                    })
                },
            )
        });
        waiting
            .recv_timeout(Duration::from_secs(15))
            .expect("onboarding test daemon did not start");
        Self {
            shutdown: Some(shutdown),
            thread: Some(thread),
        }
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap().unwrap();
        }
    }
}

struct Fixture {
    // Stop the daemon before deleting the temporary state directory.
    running: Option<Running>,
    _dir: tempfile::TempDir,
    spec: Spec,
}
impl Fixture {
    fn new(client: Client, start: bool) -> Self {
        let dir = crate::testdir::short_dir();
        let base = fs::canonicalize(dir.path()).unwrap();
        let prefix = base.join("software");
        install_stub(&prefix, &base.join("bundle"));
        let profile_home = base.join("profile");
        let workspace = base.join("workspace");
        for path in [&profile_home, &workspace] {
            DirBuilder::new().mode(0o700).create(path).unwrap();
        }
        let spec = Spec {
            prefix,
            client,
            profile_home,
            workspace,
            daemon_home: base.join("daemon"),
            name: Some("onboard-agent".into()),
        };
        let running = start.then(|| Running::start(&spec.daemon_home));
        Self {
            running,
            _dir: dir,
            spec,
        }
    }
    fn restart(&mut self) {
        drop(self.running.take());
        self.running = Some(Running::start(&self.spec.daemon_home));
    }
    fn owner(&self) -> ApiClient<UnixStream> {
        let stream =
            UnixStream::connect(local::socket_path(&self.spec.daemon_home).unwrap()).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        ApiClient::open(
            stream,
            Credential(
                protected_secret(&local::owner_credential_path(&self.spec.daemon_home)).unwrap(),
            ),
            None,
        )
        .unwrap()
    }
    fn agents(&self) -> Vec<locust_proto::api::AgentView> {
        let Response::Status(status) = self.owner().call(Request::Status).unwrap() else {
            panic!("expected owner status");
        };
        status.agents
    }
    fn complete(&self) -> Value {
        let proposed = plan(&self.spec).unwrap();
        apply_with(&proposed, &mut |_| Ok(true), &mut |_| Ok(())).unwrap()
    }
}

/// Install a signed, structurally valid executable fixture. Installation probes
/// are injected; onboarding only reads this release and writes client bindings.
fn install_stub(prefix: &Path, root: &Path) {
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
    let skill = b"signed skill";
    package::create_file(&root.join(package::BINARY), &binary, 0o755).unwrap();
    package::create_file(&root.join(package::SKILL), skill, 0o644).unwrap();
    package::create_file(&root.join(package::MANUAL), b"manual", 0o644).unwrap();
    let manifest = encode(&json!({
        "format":"locust-release-v2", "source_commit":"c".repeat(40),
        "version":"0.1.0", "target":target, "machine_format":format,
        "api_version":locust_proto::API_VERSION,
        "protocol_version":locust_proto::PROTOCOL_VERSION,
        "toolchain":"1.96.1",
        "files":[
            {"path":package::BINARY,"sha256":package::sha256(&binary),"size":binary.len(),"mode":493},
            {"path":package::SKILL,"sha256":package::sha256(skill),"size":skill.len(),"mode":420},
            {"path":package::MANUAL,"sha256":package::sha256(b"manual"),"size":6,"mode":420}
        ]
    })).unwrap();
    let withdrawals = encode(&json!({
        "format":"locust-withdrawals-v1", "sequence":1, "withdrawn_manifest_sha256":[]
    }))
    .unwrap();
    let verified = Verified {
        root: root.to_path_buf(),
        manifest: package::validate_manifest(&manifest).unwrap(),
        manifest_sha256: package::sha256(&manifest),
        signature: key.sign(&manifest).to_bytes(),
        manifest_bytes: manifest,
        trust_key: key.verifying_key().to_bytes(),
        withdrawals: package::validate_withdrawals(&withdrawals).unwrap(),
        withdrawals_signature: key.sign(&withdrawals).to_bytes(),
        withdrawals_sha256: package::sha256(&withdrawals),
        withdrawals_bytes: withdrawals,
    };
    let reviewed = super::super::plan(prefix, &verified, false).unwrap();
    super::super::apply_with_probe(
        prefix,
        &verified,
        false,
        &reviewed.digest().unwrap(),
        |_, _, _| Ok(()),
    )
    .unwrap();
}

fn put(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
fn interrupt(proposed: &Plan, stage: &str) {
    let mut reached = false;
    let error = apply_with(proposed, &mut |_| Ok(true), &mut |current| {
        if current == stage {
            reached = true;
            return Err(Failure::internal("injected onboarding interruption"));
        }
        Ok(())
    })
    .unwrap_err();
    assert!(reached, "checkpoint {stage} was not reached: {error:?}");
    assert_eq!(error.message, "injected onboarding interruption");
}

#[test]
fn planning_does_not_create_daemon_state_identity_or_profile_files() {
    let fixture = Fixture::new(Client::Codex, false);
    let mut spec = fixture.spec.clone();
    spec.name = None;
    let proposed = plan(&spec).unwrap();
    assert!(locust_proto::api::is_agent_name(
        proposed.spec.name.as_deref().unwrap()
    ));
    assert!(!spec.daemon_home.exists());
    assert_eq!(fs::read_dir(&spec.profile_home).unwrap().count(), 0);
    assert!(!record_path(&proposed.spec).unwrap().exists());
    let binding = setup_spec(&proposed.spec).unwrap();
    assert!(!binding.credential.exists());
    assert!(!binding.session.exists());
    let review = proposed.json().unwrap();
    assert_eq!(review["plan"]["grants_added"], false);
    assert_eq!(review["plan"]["model_ready"], false);
}

#[test]
fn each_client_enrolls_without_grants_and_reuses_identity_session_and_config() {
    for client in [
        Client::Codex,
        Client::Claude,
        Client::Pi,
        Client::Droid,
        Client::Shell,
    ] {
        let fixture = Fixture::new(client, true);
        let (config, _, _) = setup::profile_paths(client, &fixture.spec.profile_home);
        let baseline = if client == Client::Codex {
            b"# preserve comment\nmodel = 'chosen'\n".as_slice()
        } else {
            b"{\"unrelated\":{\"token\":\"DO_NOT_DISCLOSE\"}}".as_slice()
        };
        put(&config, baseline);
        assert!(
            !plan(&fixture.spec)
                .unwrap()
                .json()
                .unwrap()
                .to_string()
                .contains("DO_NOT_DISCLOSE")
        );
        let first = fixture.complete();
        assert_eq!(first["changed"], true);
        assert_eq!(first["model_ready"], false);
        assert_eq!(first["configuration_ready"], true);
        let binding = setup_spec(&fixture.spec).unwrap();
        let credential = protected_secret(&binding.credential).unwrap();
        let session = protected_secret(&binding.session).unwrap();
        let config_bytes = fs::read(&config).unwrap();
        if client == Client::Codex {
            let text = std::str::from_utf8(&config_bytes).unwrap();
            assert!(text.contains("# preserve comment"));
            assert!(text.contains("model = 'chosen'"));
        } else {
            let document: Value = serde_json::from_slice(&config_bytes).unwrap();
            assert_eq!(document["unrelated"]["token"], "DO_NOT_DISCLOSE");
        }
        let second = fixture.complete();
        assert_eq!(second["changed"], false);
        assert_eq!(first["principal"], second["principal"]);
        assert_eq!(first["instance"], second["instance"]);
        assert_eq!(second["instance"], json!(SessionSecret(session).instance()));
        assert_eq!(protected_secret(&binding.credential).unwrap(), credential);
        assert_eq!(protected_secret(&binding.session).unwrap(), session);
        assert_eq!(fs::read(config).unwrap(), config_bytes);
        let agents = fixture.agents();
        assert_eq!(agents.len(), 1);
        assert!(!agents[0].grants.manage_goals);
        assert!(!agents[0].revoked);
        assert_eq!(second["principal"], json!(agents[0].agent));
    }
}

#[test]
fn every_checkpoint_resumes_after_restart_without_duplicate_principals() {
    for stage in [
        "journal_created",
        "credential_saved",
        "session_saved",
        "secrets_saved",
        "enrollment_sent",
        "enrolled",
        "setup_reviewed",
        "setup_applied",
    ] {
        let mut fixture = Fixture::new(Client::Codex, true);
        let proposed = plan(&fixture.spec).unwrap();
        interrupt(&proposed, stage);
        let binding = setup_spec(&fixture.spec).unwrap();
        let credential_before = binding
            .credential
            .exists()
            .then(|| protected_secret(&binding.credential).unwrap());
        let session_before = binding
            .session
            .exists()
            .then(|| protected_secret(&binding.session).unwrap());
        let agents_before = fixture.agents();
        assert!(agents_before.len() <= 1, "{stage}");
        if matches!(
            stage,
            "enrollment_sent" | "enrolled" | "setup_reviewed" | "setup_applied"
        ) {
            assert_eq!(agents_before.len(), 1, "{stage}");
        }
        fixture.restart();
        let resumed = fixture.complete();
        let agents = fixture.agents();
        assert_eq!(agents.len(), 1, "{stage}");
        assert!(!agents[0].grants.manage_goals, "{stage}");
        if let Some(before) = agents_before.first() {
            assert_eq!(agents[0].agent, before.agent, "{stage}");
        }
        if let Some(before) = credential_before {
            assert_eq!(
                protected_secret(&binding.credential).unwrap(),
                before,
                "{stage}"
            );
        }
        if let Some(before) = session_before {
            assert_eq!(
                protected_secret(&binding.session).unwrap(),
                before,
                "{stage}"
            );
        }
        if stage == "setup_applied" {
            assert_eq!(resumed["changed"], false);
        }
        let (journal, _) = read_record(&fixture.spec).unwrap().unwrap();
        assert!(journal.configured, "{stage}");
        assert_eq!(journal.principal, Some(agents[0].agent));
        let repeated = fixture.complete();
        assert_eq!(repeated["changed"], false, "{stage}");
        assert_eq!(resumed["principal"], repeated["principal"]);
        assert_eq!(resumed["instance"], repeated["instance"]);
        assert_eq!(fixture.agents().len(), 1);
    }
}

#[test]
fn missing_or_modified_saved_secrets_are_refused_without_replacement() {
    for session in [false, true] {
        for missing in [false, true] {
            for completed in [false, true] {
                let fixture = Fixture::new(Client::Codex, true);
                if completed {
                    fixture.complete();
                } else {
                    let proposed = plan(&fixture.spec).unwrap();
                    interrupt(
                        &proposed,
                        if session {
                            "session_saved"
                        } else {
                            "credential_saved"
                        },
                    );
                }
                let proposed = plan(&fixture.spec).unwrap();
                let binding = setup_spec(&fixture.spec).unwrap();
                let damaged = if session {
                    &binding.session
                } else {
                    &binding.credential
                };
                let other = if session {
                    &binding.credential
                } else {
                    &binding.session
                };
                let original = protected_secret(damaged).unwrap();
                let other_before = fs::read(other).ok();
                let journal_before = fs::read(record_path(&fixture.spec).unwrap()).unwrap();
                let agents_before = fixture.agents();
                if missing {
                    fs::remove_file(damaged).unwrap();
                } else {
                    let mut changed = original;
                    changed[0] ^= 0xff;
                    put(damaged, &changed);
                }
                let damaged_before = fs::read(damaged).ok();
                assert!(plan(&fixture.spec).is_err());
                assert!(apply_with(&proposed, &mut |_| Ok(true), &mut |_| Ok(())).is_err());
                assert_eq!(fs::read(damaged).ok(), damaged_before);
                assert_eq!(fs::read(other).ok(), other_before);
                assert_eq!(
                    fs::read(record_path(&fixture.spec).unwrap()).unwrap(),
                    journal_before
                );
                assert_eq!(fixture.agents(), agents_before);
            }
        }
    }
}

#[test]
fn repeating_completed_onboarding_preserves_subsequent_owner_grants() {
    let mut fixture = Fixture::new(Client::Codex, true);
    let first = fixture.complete();
    let principal = fixture.agents()[0].agent;
    assert_eq!(
        fixture
            .owner()
            .call(Request::AgentGrant {
                agent: principal,
                grants: Grants { manage_goals: true },
            })
            .unwrap(),
        Response::Done
    );
    fixture.restart();
    let repeated = fixture.complete();
    assert_eq!(repeated["changed"], false);
    assert_eq!(first["principal"], repeated["principal"]);
    assert_eq!(first["instance"], repeated["instance"]);
    let agents = fixture.agents();
    assert_eq!(agents.len(), 1);
    assert!(agents[0].grants.manage_goals);
}

#[test]
fn existing_name_or_client_profile_is_not_adopted() {
    let fixture = Fixture::new(Client::Codex, true);
    fixture
        .owner()
        .call(Request::AgentEnroll {
            name: fixture.spec.name.clone().unwrap(),
            grants: Grants { manage_goals: true },
            credential: Credential([91; 32]).digest(),
        })
        .unwrap();
    let before = fixture.agents();
    let proposed = plan(&fixture.spec).unwrap();
    let error = apply_with(&proposed, &mut |_| Ok(true), &mut |_| Ok(())).unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
    assert_eq!(fixture.agents(), before);
    assert!(!directory(&fixture.spec).unwrap().exists());

    for collision in ["config", "skill", "launcher"] {
        let fixture = Fixture::new(Client::Codex, true);
        let (config, skill, launcher) =
            setup::profile_paths(Client::Codex, &fixture.spec.profile_home);
        let path = match collision {
            "config" => config,
            "skill" => skill,
            _ => launcher,
        };
        let content = if collision == "config" {
            b"[mcp_servers.locust]\ncommand = 'user-selected'\n".as_slice()
        } else {
            b"unowned user file".as_slice()
        };
        put(&path, content);
        assert_eq!(plan(&fixture.spec).err().unwrap().code, ErrorCode::Conflict);
        assert_eq!(fs::read(path).unwrap(), content);
        assert!(fixture.agents().is_empty());
        assert!(!directory(&fixture.spec).unwrap().exists());
    }
}

#[test]
fn declining_setup_saves_one_enrolled_identity_for_reviewed_resume() {
    let fixture = Fixture::new(Client::Claude, true);
    let proposed = plan(&fixture.spec).unwrap();
    let mut reviews = 0;
    let error = apply_with(
        &proposed,
        &mut |_| {
            reviews += 1;
            Ok(false)
        },
        &mut |_| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Denied);
    assert_eq!(reviews, 1);
    let (journal, _) = read_record(&fixture.spec).unwrap().unwrap();
    assert!(!journal.configured);
    assert!(journal.reviewed_setup_sha256.is_none());
    let agents = fixture.agents();
    assert_eq!(agents.len(), 1);
    assert_eq!(journal.principal, Some(agents[0].agent));
    assert!(!agents[0].grants.manage_goals);
    assert_eq!(fs::read_dir(&fixture.spec.profile_home).unwrap().count(), 0);
    let binding = setup_spec(&fixture.spec).unwrap();
    let credential = protected_secret(&binding.credential).unwrap();
    let session = protected_secret(&binding.session).unwrap();
    let resumed = fixture.complete();
    assert_eq!(resumed["principal"], json!(agents[0].agent));
    assert_eq!(
        resumed["instance"],
        json!(SessionSecret(session).instance())
    );
    assert_eq!(protected_secret(&binding.credential).unwrap(), credential);
    assert_eq!(protected_secret(&binding.session).unwrap(), session);
    assert_eq!(fixture.agents().len(), 1);
}

#[test]
fn unknown_journal_version_and_changed_bindings_preserve_reserved_state() {
    let fixture = Fixture::new(Client::Codex, true);
    let proposed = plan(&fixture.spec).unwrap();
    interrupt(&proposed, "secrets_saved");
    let journal_path = record_path(&fixture.spec).unwrap();
    let journal_before = fs::read(&journal_path).unwrap();
    let binding = setup_spec(&fixture.spec).unwrap();
    let credential = protected_secret(&binding.credential).unwrap();
    let session = protected_secret(&binding.session).unwrap();
    let mut journal: Value = serde_json::from_slice(&journal_before).unwrap();
    journal["format"] = json!("locust-onboarding-v999");
    put(&journal_path, &encode(&journal).unwrap());
    let unknown_before = fs::read(&journal_path).unwrap();
    assert_eq!(
        plan(&fixture.spec).err().unwrap().code,
        ErrorCode::Corrupted
    );
    assert_eq!(fs::read(&journal_path).unwrap(), unknown_before);
    put(&journal_path, &journal_before);
    let other_workspace = fixture.spec.workspace.with_file_name("another-workspace");
    DirBuilder::new()
        .mode(0o700)
        .create(&other_workspace)
        .unwrap();
    let mut changed = fixture.spec.clone();
    changed.workspace = other_workspace;
    assert_eq!(plan(&changed).err().unwrap().code, ErrorCode::Conflict);
    changed = fixture.spec.clone();
    changed.name = Some("other-name".into());
    assert_eq!(plan(&changed).err().unwrap().code, ErrorCode::Conflict);
    assert_eq!(fs::read(&journal_path).unwrap(), journal_before);
    assert_eq!(protected_secret(&binding.credential).unwrap(), credential);
    assert_eq!(protected_secret(&binding.session).unwrap(), session);
    assert!(fixture.agents().is_empty());
    let resumed = fixture.complete();
    assert_eq!(
        resumed["instance"],
        json!(SessionSecret(session).instance())
    );
    assert_eq!(fixture.agents().len(), 1);
}

#[test]
fn stale_profile_review_refuses_before_reserving_identity() {
    let fixture = Fixture::new(Client::Codex, true);
    let proposed = plan(&fixture.spec).unwrap();
    let (config, _, _) = setup::profile_paths(Client::Codex, &fixture.spec.profile_home);
    let changed = b"model = 'changed-since-review'\n";
    put(&config, changed);
    let mut reviews = 0;
    let error = apply_with(
        &proposed,
        &mut |_| {
            reviews += 1;
            Ok(true)
        },
        &mut |_| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
    assert_eq!(reviews, 0);
    assert!(fixture.agents().is_empty());
    assert!(!directory(&fixture.spec).unwrap().exists());
    assert_eq!(fs::read(config).unwrap(), changed);
    fixture.complete();
    assert_eq!(fixture.agents().len(), 1);
    assert!(apply_with(&proposed, &mut |_| Ok(true), &mut |_| Ok(())).is_err());
    assert_eq!(fixture.agents().len(), 1);
}

#[test]
fn secret_changes_during_setup_review_refuse_readiness_and_preserve_identity() {
    for session in [false, true] {
        let fixture = Fixture::new(Client::Codex, true);
        let proposed = plan(&fixture.spec).unwrap();
        let binding = setup_spec(&fixture.spec).unwrap();
        let secret = if session {
            &binding.session
        } else {
            &binding.credential
        };
        let mut original = None;
        let mut reviews = 0;
        let error = apply_with(
            &proposed,
            &mut |_| {
                reviews += 1;
                let before = protected_secret(secret).unwrap();
                let mut changed = before;
                changed[0] ^= 0xff;
                original = Some(before);
                put(secret, &changed);
                Ok(true)
            },
            &mut |_| Ok(()),
        )
        .unwrap_err();
        assert_eq!(reviews, 1);
        assert_eq!(error.code, ErrorCode::Conflict);
        let (journal, _) = read_record(&fixture.spec).unwrap().unwrap();
        assert!(!journal.configured);
        let agents = fixture.agents();
        assert_eq!(agents.len(), 1);
        assert_eq!(journal.principal, Some(agents[0].agent));
        let (config, skill, launcher) =
            setup::profile_paths(Client::Codex, &fixture.spec.profile_home);
        assert!(!config.exists());
        assert!(!skill.exists());
        assert!(!launcher.exists());
        assert!(plan(&fixture.spec).is_err());
        put(secret, &original.unwrap());
        let resumed = fixture.complete();
        assert_eq!(resumed["principal"], json!(agents[0].agent));
        assert_eq!(fixture.agents().len(), 1);
    }
}

#[test]
fn secret_changes_after_profile_write_do_not_mark_journal_ready() {
    for session in [false, true] {
        let fixture = Fixture::new(Client::Codex, true);
        let proposed = plan(&fixture.spec).unwrap();
        let binding = setup_spec(&fixture.spec).unwrap();
        let secret = if session {
            &binding.session
        } else {
            &binding.credential
        };
        let mut original = None;
        let error = apply_with(&proposed, &mut |_| Ok(true), &mut |stage| {
            if stage == "setup_applied" {
                let before = protected_secret(secret).unwrap();
                let mut changed = before;
                changed[0] ^= 0xff;
                original = Some(before);
                put(secret, &changed);
            }
            Ok(())
        })
        .unwrap_err();
        assert!(
            original.is_some(),
            "profile write checkpoint was not reached"
        );
        assert_eq!(error.code, ErrorCode::Conflict);
        let (journal, _) = read_record(&fixture.spec).unwrap().unwrap();
        assert!(
            !journal.configured,
            "final readiness must succeed before marking the journal configured"
        );
        let agents = fixture.agents();
        assert_eq!(agents.len(), 1);
        assert_eq!(journal.principal, Some(agents[0].agent));
        put(secret, &original.unwrap());
        let resumed = fixture.complete();
        assert_eq!(resumed["changed"], false);
        assert_eq!(resumed["principal"], json!(agents[0].agent));
        assert_eq!(fixture.agents().len(), 1);
    }
}

#[test]
fn owner_revocation_during_setup_is_observed_before_readiness_and_never_reenrolled() {
    for after_write in [false, true] {
        let fixture = Fixture::new(Client::Codex, true);
        let proposed = plan(&fixture.spec).unwrap();
        let mut revoked = false;
        let revoke = || {
            let agent = fixture.agents()[0].agent;
            assert_eq!(
                fixture
                    .owner()
                    .call(Request::AgentRevoke { agent })
                    .unwrap(),
                Response::Done
            );
        };
        let error = apply_with(
            &proposed,
            &mut |_| {
                if !after_write {
                    revoke();
                }
                Ok(true)
            },
            &mut |stage| {
                if after_write && stage == "setup_applied" {
                    revoke();
                    revoked = true;
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert!(!after_write || revoked);
        assert_eq!(error.code, ErrorCode::Denied);
        let (journal, _) = read_record(&fixture.spec).unwrap().unwrap();
        assert!(!journal.configured);
        let agents = fixture.agents();
        assert_eq!(agents.len(), 1);
        assert!(agents[0].revoked);
        assert_eq!(journal.principal, Some(agents[0].agent));
        let retry = plan(&fixture.spec).unwrap();
        assert_eq!(
            apply_with(&retry, &mut |_| Ok(true), &mut |_| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::Denied
        );
        assert_eq!(fixture.agents(), agents);
    }
}

#[test]
fn malformed_saved_names_hashes_and_stages_are_rejected_without_changes() {
    let fixture = Fixture::new(Client::Codex, true);
    fixture.complete();
    let path = record_path(&fixture.spec).unwrap();
    let original = fs::read(&path).unwrap();
    let baseline: Value = serde_json::from_slice(&original).unwrap();
    let binding = setup_spec(&fixture.spec).unwrap();
    let credential = protected_secret(&binding.credential).unwrap();
    let session = protected_secret(&binding.session).unwrap();
    let agents = fixture.agents();
    for mutation in ["name", "owner", "credential", "session", "review", "stages"] {
        let mut changed = baseline.clone();
        match mutation {
            "name" => changed["spec"]["name"] = json!("Malformed Name!"),
            "owner" => changed["owner_sha256"] = json!("g".repeat(64)),
            "credential" => changed["credential_sha256"] = json!("a".repeat(63)),
            "session" => changed["session_sha256"] = json!("A".repeat(64)),
            "review" => changed["reviewed_setup_sha256"] = json!("not-a-digest"),
            _ => changed["instance"] = Value::Null,
        }
        let bytes = encode(&changed).unwrap();
        put(&path, &bytes);
        let mut selected = fixture.spec.clone();
        selected.name = None;
        assert_eq!(
            plan(&selected).err().unwrap().code,
            ErrorCode::Corrupted,
            "{mutation}"
        );
        assert_eq!(fs::read(&path).unwrap(), bytes, "{mutation}");
        assert_eq!(protected_secret(&binding.credential).unwrap(), credential);
        assert_eq!(protected_secret(&binding.session).unwrap(), session);
        assert_eq!(fixture.agents(), agents);
    }
    put(&path, &original);
    assert_eq!(fixture.complete()["changed"], false);
}

#[test]
fn saved_session_without_credential_fingerprint_is_not_adopted() {
    let fixture = Fixture::new(Client::Codex, true);
    let proposed = plan(&fixture.spec).unwrap();
    interrupt(&proposed, "session_saved");
    let path = record_path(&fixture.spec).unwrap();
    let original = fs::read(&path).unwrap();
    let mut changed: Value = serde_json::from_slice(&original).unwrap();
    changed["credential_sha256"] = Value::Null;
    let bytes = encode(&changed).unwrap();
    put(&path, &bytes);
    let binding = setup_spec(&fixture.spec).unwrap();
    let credential = protected_secret(&binding.credential).unwrap();
    let session = protected_secret(&binding.session).unwrap();
    assert_eq!(
        plan(&fixture.spec).err().unwrap().code,
        ErrorCode::Corrupted
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(protected_secret(&binding.credential).unwrap(), credential);
    assert_eq!(protected_secret(&binding.session).unwrap(), session);
    assert!(fixture.agents().is_empty());
    put(&path, &original);
    fixture.complete();
    assert_eq!(fixture.agents().len(), 1);
}

#[test]
fn symlinked_and_hardlinked_private_secrets_and_journals_are_refused() {
    for symlinked in [false, true] {
        for target in ["credential", "session", "journal"] {
            let fixture = Fixture::new(Client::Codex, true);
            fixture.complete();
            let proposed = plan(&fixture.spec).unwrap();
            let binding = setup_spec(&fixture.spec).unwrap();
            let path = match target {
                "credential" => binding.credential,
                "session" => binding.session,
                _ => record_path(&fixture.spec).unwrap(),
            };
            let original = fs::read(&path).unwrap();
            let alias = fixture.spec.daemon_home.join("saved-private-file");
            if symlinked {
                fs::rename(&path, &alias).unwrap();
                std::os::unix::fs::symlink(&alias, &path).unwrap();
            } else {
                fs::hard_link(&path, &alias).unwrap();
            }
            let agents = fixture.agents();
            assert_eq!(
                plan(&fixture.spec).err().unwrap().code,
                if symlinked {
                    ErrorCode::Invalid
                } else {
                    ErrorCode::Corrupted
                },
                "{target}"
            );
            assert!(apply_with(&proposed, &mut |_| Ok(true), &mut |_| Ok(())).is_err());
            assert_eq!(fs::read(&alias).unwrap(), original);
            assert_eq!(fs::read(&path).unwrap(), original);
            assert_eq!(fixture.agents(), agents);
            if symlinked {
                assert!(
                    fs::symlink_metadata(&path)
                        .unwrap()
                        .file_type()
                        .is_symlink()
                );
                fs::remove_file(&path).unwrap();
                fs::rename(alias, &path).unwrap();
            } else {
                assert_eq!(fs::metadata(&path).unwrap().nlink(), 2);
                fs::remove_file(alias).unwrap();
            }
            assert_eq!(fixture.complete()["changed"], false);
        }
    }
}

#[test]
fn diagnostics_inspect_saved_profiles_without_rewriting_files_or_identities() {
    for client in [
        Client::Codex,
        Client::Claude,
        Client::Pi,
        Client::Droid,
        Client::Shell,
    ] {
        let fixture = Fixture::new(client, true);
        let result = fixture.complete();
        let before = fs::read(record_path(&fixture.spec).unwrap()).unwrap();
        let selected = diagnostics::select(
            &fixture.spec.daemon_home,
            client,
            &fixture.spec.profile_home,
        )
        .unwrap();
        assert_eq!(selected.spec, fixture.spec);
        assert!(selected.completed());
        let identity = selected.identity().unwrap();
        assert_eq!(identity["principal"], result["principal"]);
        assert_eq!(identity["instance"], result["instance"]);
        let state = setup::status(&selected.binding).unwrap();
        for field in [
            "configured",
            "mcp_ready",
            "skill_ready",
            "launcher_ready",
            "binding_matches",
        ] {
            assert_eq!(state[field], true, "{field}");
        }
        assert_eq!(state["discovered"], false);
        assert_eq!(state["api_ready"], false);
        assert_eq!(
            fs::read(record_path(&fixture.spec).unwrap()).unwrap(),
            before
        );
        assert_eq!(fixture.agents().len(), 1);
    }
}

#[test]
fn diagnostics_report_revoked_or_replaced_credentials_without_enrolling() {
    let fixture = Fixture::new(Client::Codex, true);
    fixture.complete();
    let selected = diagnostics::select(
        &fixture.spec.daemon_home,
        Client::Codex,
        &fixture.spec.profile_home,
    )
    .unwrap();
    let principal = fixture.agents()[0].agent;
    fixture
        .owner()
        .call(Request::AgentRevoke { agent: principal })
        .unwrap();
    assert!(selected.identity().is_err());
    assert_eq!(fixture.agents().len(), 1);
    assert!(fixture.agents()[0].revoked);
    fs::write(&selected.binding.credential, [9; 32]).unwrap();
    let error = selected.identity().unwrap_err();
    assert!(error.message.contains("secret changed"));
    assert_eq!(fs::read(&selected.binding.credential).unwrap(), [9; 32]);
}

#[test]
fn diagnostics_do_not_follow_a_journal_redirect_to_another_profile() {
    let fixture = Fixture::new(Client::Claude, true);
    fixture.complete();
    let path = record_path(&fixture.spec).unwrap();
    let mut record: Record = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    record.spec.profile_home = fixture.spec.workspace.clone();
    fs::write(&path, encode(&record).unwrap()).unwrap();
    assert!(
        diagnostics::select(
            &fixture.spec.daemon_home,
            Client::Claude,
            &fixture.spec.profile_home
        )
        .is_err()
    );
}

#[test]
fn setup_diagnostics_distinguish_changed_skill_from_intact_mcp_and_launcher() {
    let fixture = Fixture::new(Client::Codex, true);
    fixture.complete();
    let binding = setup_spec(&fixture.spec).unwrap();
    let (_, skill, _) = setup::profile_paths(Client::Codex, &fixture.spec.profile_home);
    fs::write(&skill, b"user modification").unwrap();
    let status = setup::status(&binding).unwrap();
    assert_eq!(status["configured"], false);
    assert_eq!(status["skill_ready"], false);
    assert_eq!(status["mcp_ready"], true);
    assert_eq!(status["launcher_ready"], true);
    assert_eq!(fs::read(skill).unwrap(), b"user modification");
}

#[test]
fn setup_diagnostics_distinguish_mcp_removal_and_launcher_permission_changes() {
    let fixture = Fixture::new(Client::Codex, true);
    fixture.complete();
    let binding = setup_spec(&fixture.spec).unwrap();
    let (config, _, launcher) = setup::profile_paths(Client::Codex, &fixture.spec.profile_home);
    fs::write(&config, b"").unwrap();
    let state = setup::status(&binding).unwrap();
    assert_eq!(state["mcp_ready"], false);
    assert_eq!(state["skill_ready"], true);
    assert_eq!(state["launcher_ready"], true);
    fs::set_permissions(&launcher, fs::Permissions::from_mode(0o600)).unwrap();
    let state = setup::status(&binding).unwrap();
    assert_eq!(state["launcher_ready"], false);
    assert_eq!(fs::metadata(launcher).unwrap().mode() & 0o7777, 0o600);
}
