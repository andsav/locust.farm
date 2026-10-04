//! The real binary's offline invitation review and secret-safe input paths.

use locust_proto::crypto::Keypair;
use locust_proto::id::{EndpointId, GoalId};
use locust_proto::invite::{Invitation, InviteSecret};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

fn invitation() -> Invitation {
    Invitation::signed(
        GoalId([1; 32]),
        Some("Reviewable goal".into()),
        EndpointId([2; 32]),
        vec!["127.0.0.1:3140".into()],
        InviteSecret([3; 32]),
        None,
        &Keypair::from_seed([4; 32]),
    )
    .unwrap()
}

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_locust"));
    command
        .args(["--json", "invitation"])
        .env_remove("LOCUST_HOME")
        .env_remove("LOCUST_CREDENTIAL")
        .env_remove("LOCUST_SESSION");
    command
}

#[test]
fn offline_binary_inspection_reads_stdin_and_private_file_without_a_daemon() {
    let ticket = invitation().to_ticket().unwrap();
    let mut child = command()
        .args(["inspect", "--ticket", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(ticket.as_str().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    let preview = &result["result"]["invitation_inspected"]["preview"];
    assert_eq!(preview["goal_title"], "Reviewable goal");
    assert_eq!(preview["signature_verified"], true);
    assert_eq!(preview["sharing"], "whole_goal");
    assert!(!String::from_utf8_lossy(&output.stdout).contains(&"03".repeat(32)));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("locust-invite-"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invite");
    fs::write(&path, ticket.as_str()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let from_file = command()
        .args(["inspect", "--ticket-file"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(from_file.status.success());
    assert_eq!(output.stdout, from_file.stdout);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let public_file = command()
        .args(["inspect", "--ticket-file"])
        .arg(path)
        .output()
        .unwrap();
    assert!(!public_file.status.success());
    assert!(!String::from_utf8_lossy(&public_file.stdout).contains(ticket.as_str()));
}

#[test]
fn literal_or_modified_tickets_fail_without_echoing_capabilities() {
    let invitation = invitation();
    let ticket = invitation.to_ticket().unwrap();
    let literal = command()
        .args(["inspect", "--ticket", ticket.as_str()])
        .output()
        .unwrap();
    assert!(!literal.status.success());
    assert!(!String::from_utf8_lossy(&literal.stdout).contains(ticket.as_str()));
    assert!(!String::from_utf8_lossy(&literal.stderr).contains(ticket.as_str()));
    let mut altered = invitation;
    altered.goal_title = Some("Substituted goal".into());
    let mut child = command()
        .args(["inspect", "--ticket", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(altered.to_ticket().unwrap().as_str().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["error"]["code"], "denied");
    assert!(!String::from_utf8_lossy(&output.stdout).contains("locust-invite-"));
}
