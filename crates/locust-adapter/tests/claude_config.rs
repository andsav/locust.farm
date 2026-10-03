//! Opt-in installed-client check: isolated HOME/config, no credentials/provider.
//! Claude spawns the recorder before reporting missing authentication. This
//! verifies argv/env parsing only, not MCP initialization or tool execution.
#![cfg(unix)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use locust_adapter::config::{BridgePaths, Client, SERVER_NAME, StdioServer, mcp_arguments};
use serde_json::{Value, json};

#[test]
#[ignore = "set LOCUST_CLAUDE_BIN to an absolute installed Claude executable; no login/model needed"]
fn claude_parses_equals_overlay_and_delivers_server_only_environment() {
    let executable =
        PathBuf::from(std::env::var_os("LOCUST_CLAUDE_BIN").expect("LOCUST_CLAUDE_BIN"));
    assert!(executable.is_absolute());
    let profile = tempfile::tempdir().unwrap();
    let record = profile.path().join("record.json");
    let script = profile.path().join("record.py");
    fs::write(&script, concat!(
        "import json, os, sys\n",
        "with open(sys.argv[1], 'w') as out:\n",
        " json.dump({'args': sys.argv[2:], 'env': {k: os.environ.get(k) for k in ['LOCUST_HOME', 'LOCUST_SESSION', 'LOCUST_CREDENTIAL']}}, out)\n",
    )).unwrap();
    let server = StdioServer {
        executable: "/usr/bin/python3".into(),
        arguments: vec![
            script.to_str().unwrap().into(),
            record.to_str().unwrap().into(),
            "a\"b\\c\n\u{7f} $(touch /no)".into(),
        ],
        paths: BridgePaths {
            home: profile.path().join("locust home"),
            session: profile.path().join("session.json"),
            credential: profile.path().join("credential.json"),
        },
    };
    let config = mcp_arguments(Client::ClaudeCode, SERVER_NAME, &server, &[]).unwrap();
    let stderr = fs::File::create(profile.path().join("stderr")).unwrap();
    let stdout = fs::File::create(profile.path().join("stdout")).unwrap();
    let mut command = Command::new(executable);
    command
        .env_clear()
        .current_dir(profile.path())
        .env("HOME", profile.path())
        .env("CLAUDE_CONFIG_DIR", profile.path())
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .args(["--bare", "-p"])
        .args(config)
        // Following prompt is intentionally positional, without a separator.
        .arg("reply ok")
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr);
    assert!(
        !command
            .get_envs()
            .any(|(key, _)| key.to_string_lossy().starts_with("LOCUST_"))
    );
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("Claude configuration probe did not exit within 20 seconds");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(!status.success(), "no authentication should be available");
    let actual: Value = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
    assert_eq!(actual["args"], json!([server.arguments[2]]));
    assert_eq!(
        actual["env"],
        json!({
            "LOCUST_HOME": server.paths.home,
            "LOCUST_SESSION": server.paths.session,
            "LOCUST_CREDENTIAL": server.paths.credential,
        })
    );
    let output = fs::read_to_string(profile.path().join("stdout")).unwrap();
    let errors = fs::read_to_string(profile.path().join("stderr")).unwrap();
    assert!(!errors.contains("MCP config file not found"));
    assert!(
        output.contains("Not logged in")
            || output.contains("API key")
            || errors.contains("Not logged in")
            || errors.contains("API key"),
        "missing-auth result not observed"
    );
}
