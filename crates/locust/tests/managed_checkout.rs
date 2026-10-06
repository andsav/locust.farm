//! A real daemon creates an agent's working folder without an owner connection.
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

struct Daemon {
    home: tempfile::TempDir,
    process: Child,
}

impl Daemon {
    fn start() -> Self {
        let home = tempfile::Builder::new()
            .prefix("lc-owned-")
            .tempdir_in("/tmp")
            .unwrap();
        fs::set_permissions(home.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let log = fs::File::create(home.path().join("test-daemon.log")).unwrap();
        let process = command(home.path())
            .args(["daemon", "run"])
            .stdout(Stdio::null())
            .stderr(Stdio::from(log))
            .spawn()
            .unwrap();
        let mut daemon = Self { home, process };
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            let status = command(daemon.home.path())
                .args(["--owner", "--json", "status"])
                .output()
                .unwrap();
            if status.status.success() {
                break;
            }
            assert!(
                daemon.process.try_wait().unwrap().is_none(),
                "daemon exited during startup: {}",
                fs::read_to_string(daemon.home.path().join("test-daemon.log")).unwrap()
            );
            assert!(Instant::now() < until, "daemon did not start");
            thread::sleep(Duration::from_millis(20));
        }
        daemon
    }

    fn approved(&self, args: &[&str]) -> Value {
        let mut planned = args.to_vec();
        planned.push("--plan");
        let plan = self.run(true, &planned);
        assert_eq!(plan["action"], "review_required");
        assert_eq!(plan["changed"], false);
        let mut confirmed = args.to_vec();
        confirmed.extend(["--confirm", plan["plan_id"].as_str().unwrap()]);
        self.run(true, &confirmed)
    }

    fn run(&self, owner: bool, args: &[&str]) -> Value {
        let mut command = command(self.home.path());
        if owner {
            command.arg("--owner");
        } else {
            command
                .arg("--credential")
                .arg(self.home.path().join("agents/maple.credential"));
        }
        let output = command.arg("--json").args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{args:?}\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["result"].clone()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

fn command(home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_locust"));
    command
        .arg("--home")
        .arg(home)
        .env_remove("LOCUST_HOME")
        .env_remove("LOCUST_CREDENTIAL")
        .env_remove("LOCUST_SESSION")
        .env("LOCUST_RELAY", "none")
        .env("LOCUST_LOOKUP", "none")
        .env("LOCUST_BIND", "127.0.0.1:0");
    command
}

#[test]
fn agent_gets_a_daemon_created_folder_and_publishes_without_owner_folder_action() {
    let daemon = Daemon::start();
    let agent = daemon.run(true, &["agent", "enroll", "maple"])["agent_enrolled"]["agent"]
        .as_str()
        .unwrap()
        .to_owned();
    let goal = daemon.run(
        true,
        &[
            "call",
            "goal.create",
            &json!({
                "agent": agent, "title": "Managed files", "formation_json": null,
                "roles": {}, "inputs": {},
            })
            .to_string(),
        ],
    )["goal_created"]["goal"]
        .as_str()
        .unwrap()
        .to_owned();
    daemon.run(
        true,
        &[
            "call",
            "goal.grant",
            &json!({
                "goal": goal, "agent": agent, "grants": {
                    "contribute": true, "execute": false, "review": true,
                    "select": true, "flow": false, "takeover": false,
                },
            })
            .to_string(),
        ],
    );
    let seed = tempfile::tempdir().unwrap();
    fs::write(seed.path().join("seed.txt"), b"shared seed\n").unwrap();
    let capture = daemon.approved(&[
        "workspace",
        "init",
        "--goal",
        &goal,
        "--root",
        seed.path().to_str().unwrap(),
        "--path",
        "seed.txt",
    ]);
    let proposal = daemon.run(
        false,
        &[
            "workspace",
            "publish",
            "--goal",
            &goal,
            "--operation",
            capture["operation"]["id"].as_str().unwrap(),
        ],
    );
    let proposal = proposal["workspace_operation"]["state"]["recorded"]["event"]
        .as_str()
        .unwrap();
    daemon.run(
        false,
        &[
            "completion",
            "declare",
            "--goal",
            &goal,
            "--subject",
            proposal,
        ],
    );
    daemon.run(
        false,
        &[
            "workspace",
            "integrate",
            "--goal",
            &goal,
            "--proposal",
            proposal,
            "--expected-empty",
        ],
    );

    let id = "41".repeat(16);
    let registered = daemon.run(
        false,
        &["checkout", "register", "--goal", &goal, "--checkout", &id],
    );
    let bound = &registered["checkout"];
    let root = Path::new(bound["root"].as_str().unwrap());
    assert_eq!(
        root,
        daemon
            .home
            .path()
            .canonicalize()
            .unwrap()
            .join("checkouts")
            .join(&id)
    );
    assert_eq!(fs::read(root.join("seed.txt")).unwrap(), b"shared seed\n");
    assert_eq!(
        daemon.run(
            false,
            &["checkout", "register", "--goal", &goal, "--checkout", &id]
        ),
        registered
    );

    fs::write(root.join("agent.txt"), b"agent's first proposal\n").unwrap();
    let capture = daemon.run(
        false,
        &[
            "workspace",
            "propose",
            "--goal",
            &goal,
            "--checkout",
            &id,
            "--path",
            "agent.txt",
        ],
    );
    let published = daemon.run(
        false,
        &[
            "workspace",
            "publish",
            "--goal",
            &goal,
            "--operation",
            capture["operation"]["id"].as_str().unwrap(),
        ],
    );
    assert!(published["workspace_operation"]["state"]["recorded"]["event"].is_string());
    assert_eq!(
        fs::read(seed.path().join("seed.txt")).unwrap(),
        b"shared seed\n"
    );
    assert!(!seed.path().join("agent.txt").exists());
}

#[test]
fn invitation_confirmation_follows_shown_state_without_unrelated_rules_binding() {
    let daemon = Daemon::start();
    daemon.run(true, &["agent", "enroll", "maple"]);
    let created = daemon.approved(&["goal", "create", "--title", "Confirmation"]);
    let goal = created["goal_created"]["goal"].as_str().unwrap();
    let args = ["goal", "invite", "--goal", goal, "--plan"];
    let first = daemon.run(true, &args);
    let second = daemon.run(true, &args);
    assert_eq!(first["plan_id"], second["plan_id"]);
    assert_eq!(first["changed"], false);
    let invitations = daemon.run(true, &["invitation", "list", "--goal", goal]);
    assert!(
        invitations["invitations"]["invitations"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let status = daemon.run(true, &["goal", "status", "--goal", goal]);
    let definition = locust_proto::organization::presets()
        .into_iter()
        .find(|preset| preset.name == "peer-review")
        .unwrap()
        .formation;
    daemon.run(
        true,
        &[
            "call",
            "rules.bind",
            &json!({
                "goal": goal,
                "expected": status["goal_status"]["current_rules"],
                "formation_json": serde_json::to_string(&definition).unwrap(),
                "roles": {}, "inputs": {},
            })
            .to_string(),
        ],
    );
    let after_rules = daemon.run(true, &args);
    assert_eq!(first["plan_id"], after_rules["plan_id"]);

    let confirm = [
        "goal",
        "invite",
        "--goal",
        goal,
        "--confirm",
        first["plan_id"].as_str().unwrap(),
    ];
    let issued = command(daemon.home.path())
        .args(["--owner", "--json"])
        .args(confirm)
        .output()
        .unwrap();
    assert!(
        issued.status.success(),
        "{}",
        String::from_utf8_lossy(&issued.stdout)
    );
    assert!(issued.stderr.is_empty());
    let issued: Value = serde_json::from_slice(&issued.stdout).unwrap();
    assert!(issued["result"]["warning"].is_string());
    let replay = command(daemon.home.path())
        .args(["--owner", "--json"])
        .args(confirm)
        .output()
        .unwrap();
    assert_eq!(replay.status.code(), Some(7));
    let replay: Value = serde_json::from_slice(&replay.stdout).unwrap();
    assert_eq!(replay["error"]["code"], "conflict");
    let invitations = daemon.run(true, &["invitation", "list", "--goal", goal]);
    assert_eq!(
        invitations["invitations"]["invitations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    daemon.run(true, &["invitation", "revoke", "--goal", goal, "--all"]);
    let after_revoke = daemon.run(true, &args);
    assert_eq!(after_revoke["plan"]["pending_invitations"], 0);
    assert_eq!(after_revoke["plan"]["issued_invitations"], 1);
    assert_ne!(after_revoke["plan_id"], first["plan_id"]);
    let replay = command(daemon.home.path())
        .args(["--owner", "--json"])
        .args(confirm)
        .output()
        .unwrap();
    assert_eq!(replay.status.code(), Some(7));
    let invitations = daemon.run(true, &["invitation", "list", "--goal", goal]);
    assert_eq!(
        invitations["invitations"]["invitations"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
