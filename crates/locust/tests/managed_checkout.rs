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
    let capture = daemon.run(
        true,
        &[
            "workspace",
            "init",
            "--goal",
            &goal,
            "--root",
            seed.path().to_str().unwrap(),
            "--path",
            "seed.txt",
        ],
    );
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
