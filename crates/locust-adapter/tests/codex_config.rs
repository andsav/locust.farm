//! Optional real-client configuration parsing check. It uses no provider,
//! credentials or user profile and does not launch an MCP server.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use locust_adapter::config::{Client, StdioServer, mcp_arguments};
use serde_json::{Value, json};

struct Profile(PathBuf);

impl Profile {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("locust-config-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }

    fn command(&self, executable: &PathBuf) -> Command {
        let mut command = Command::new(executable);
        command.env_clear().current_dir(&self.0);
        for key in ["HOME", "CODEX_HOME", "XDG_CONFIG_HOME", "TMPDIR"] {
            command.env(key, &self.0);
        }
        command.env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
        command
    }
}

impl Drop for Profile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn successful(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
#[ignore = "set LOCUST_CODEX_BIN to an absolute installed Codex executable; no login/model needed"]
fn codex_parses_overlay_and_preserves_other_server_and_profile() {
    let executable = PathBuf::from(std::env::var_os("LOCUST_CODEX_BIN").expect("LOCUST_CODEX_BIN"));
    assert!(executable.is_absolute());
    let profile = Profile::new();
    let baseline = concat!(
        "approval_policy = \"on-request\"\n",
        "[sandbox_workspace_write]\nnetwork_access = false\n",
        "[mcp_servers.existing]\ncommand = \"/usr/bin/true\"\nargs = [\"preserve-me\"]\n",
    );
    let config_path = profile.0.join("config.toml");
    fs::write(&config_path, baseline).unwrap();
    let server = StdioServer {
        executable: PathBuf::from("/tmp/locust app/locust"),
        arguments: vec!["mcp".into(), "a\"b\\c\n\u{7f} $(touch /no)".into()],
        locust_home: Some(PathBuf::from("/tmp/locust home")),
    };
    let args = mcp_arguments(Client::Codex, "locust_probe", &server, &["existing".into()]).unwrap();
    let read = |name: &str| -> Value {
        let output = profile
            .command(&executable)
            .args(&args)
            .args(["mcp", "get", name, "--json"])
            .output()
            .unwrap();
        serde_json::from_slice(&successful(output)).unwrap()
    };
    let generated = read("locust_probe");
    assert_eq!(
        generated["transport"]["command"],
        server.executable.to_str().unwrap()
    );
    assert_eq!(generated["transport"]["args"], json!(server.arguments));
    assert_eq!(
        generated["transport"]["env"]["LOCUST_HOME"],
        "/tmp/locust home"
    );
    let existing = read("existing");
    assert_eq!(existing["transport"]["command"], "/usr/bin/true");
    assert_eq!(existing["transport"]["args"], json!(["preserve-me"]));
    assert_eq!(fs::read_to_string(config_path).unwrap(), baseline);
    let version = successful(
        profile
            .command(&executable)
            .arg("--version")
            .output()
            .unwrap(),
    );
    println!(
        "{}: overlay parsed; unrelated server and profile preserved",
        String::from_utf8_lossy(&version).trim()
    );
}
