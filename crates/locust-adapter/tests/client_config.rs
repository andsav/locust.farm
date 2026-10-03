use std::path::PathBuf;

use locust_adapter::config::{
    BridgePaths, Client, ConfigError, SERVER_NAME, StdioServer, mcp_arguments, mcp_file_overlay,
};
use serde_json::{Value, json};

fn server() -> StdioServer {
    StdioServer {
        executable: "/Applications/Locust App/locust".into(),
        arguments: vec!["mcp".into(), "a\"b\\c\n\u{7f} $(touch /no)".into()],
        paths: BridgePaths {
            home: "/tmp/locust home".into(),
            session: "/tmp/session.json".into(),
            credential: "/tmp/credential.json".into(),
        },
    }
}

fn prepare(client: Client, server: &StdioServer) -> Result<(), ConfigError> {
    match client {
        Client::Codex | Client::ClaudeCode => {
            mcp_arguments(client, SERVER_NAME, server, &[]).map(|_| ())
        }
        Client::FactoryDroid | Client::Pi => {
            mcp_file_overlay(client, SERVER_NAME, server, &[], &json!({})).map(|_| ())
        }
    }
}

#[test]
fn claude_is_one_equals_bound_argument_with_exact_json_escaping() {
    let args = mcp_arguments(Client::ClaudeCode, SERVER_NAME, &server(), &[]).unwrap();
    assert_eq!(args.len(), 1);
    assert_eq!(
        args[0].to_str().unwrap(),
        concat!(
            "--mcp-config={\"mcpServers\":{\"locust\":{\"args\":[\"mcp\",\"a\\\"b\\\\c\\n\u{7f} $(touch /no)\"],",
            "\"command\":\"/Applications/Locust App/locust\",\"env\":{\"LOCUST_CREDENTIAL\":\"/tmp/credential.json\",",
            "\"LOCUST_HOME\":\"/tmp/locust home\",\"LOCUST_SESSION\":\"/tmp/session.json\"},\"type\":\"stdio\"}}}"
        )
    );
    let actual: Value = serde_json::from_str(
        args[0]
            .to_str()
            .unwrap()
            .strip_prefix("--mcp-config=")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        actual["mcpServers"]["locust"]["args"],
        json!(server().arguments)
    );
}

#[test]
fn codex_exact_toml_escaping_and_required_server_environment() {
    let args = mcp_arguments(Client::Codex, SERVER_NAME, &server(), &[]).unwrap();
    assert_eq!(args.len(), 2);
    assert_eq!(args[0], "-c");
    assert_eq!(
        args[1].to_str().unwrap(),
        concat!(
            "mcp_servers.locust={command=\"/Applications/Locust App/locust\",args=[\"mcp\",\"a\\\"b\\\\c\\u000A\\u007F $(touch /no)\"],",
            "env={LOCUST_CREDENTIAL=\"/tmp/credential.json\",LOCUST_HOME=\"/tmp/locust home\",LOCUST_SESSION=\"/tmp/session.json\"}}"
        )
    );
}

#[test]
fn file_proposals_preserve_unrelated_fields_and_never_change_policy() {
    let baseline = json!({"mcpServers":{"existing":{"command":"/usr/bin/true"}},"autoEnableCodemode":false,"unrelated":{"keep":true}});
    for (client, path) in [
        (Client::FactoryDroid, ".factory/mcp.json"),
        (Client::Pi, ".pi/agent/mcp.json"),
    ] {
        let proposal = mcp_file_overlay(
            client,
            SERVER_NAME,
            &server(),
            &["existing".into()],
            &baseline,
        )
        .unwrap();
        assert_eq!(proposal.relative_path, PathBuf::from(path));
        let mut restored = proposal.document.clone();
        restored["mcpServers"]
            .as_object_mut()
            .unwrap()
            .remove(SERVER_NAME);
        assert_eq!(restored, baseline);
        assert_eq!(
            proposal.document["mcpServers"][SERVER_NAME],
            json!({
                "type":"stdio", "command":"/Applications/Locust App/locust", "args":server().arguments,
                "env":{"LOCUST_HOME":"/tmp/locust home","LOCUST_SESSION":"/tmp/session.json","LOCUST_CREDENTIAL":"/tmp/credential.json"}
            })
        );
        assert_eq!(
            mcp_arguments(client, SERVER_NAME, &server(), &[]),
            Err(ConfigError::ConfigurationFileRequired)
        );
    }
}

#[test]
fn occupied_names_include_external_config_and_pi_normalized_names() {
    for client in [Client::Codex, Client::ClaudeCode] {
        assert_eq!(
            mcp_arguments(client, SERVER_NAME, &server(), &[SERVER_NAME.into()]),
            Err(ConfigError::ServerNameOccupied)
        );
    }
    for client in [Client::FactoryDroid, Client::Pi] {
        assert_eq!(
            mcp_file_overlay(
                client,
                SERVER_NAME,
                &server(),
                &[SERVER_NAME.into()],
                &json!({})
            ),
            Err(ConfigError::ServerNameOccupied)
        );
        assert_eq!(
            mcp_file_overlay(
                client,
                SERVER_NAME,
                &server(),
                &[],
                &json!({"mcpServers":{"locust":{}}})
            ),
            Err(ConfigError::ServerNameOccupied)
        );
    }
    assert_eq!(
        mcp_file_overlay(
            Client::Pi,
            "locust-test",
            &server(),
            &["locust_test".into()],
            &json!({})
        ),
        Err(ConfigError::ServerNameOccupied)
    );
    assert_eq!(
        mcp_file_overlay(
            Client::Pi,
            "locust-test",
            &server(),
            &[],
            &json!({"mcpServers":{"locust_test":{}}})
        ),
        Err(ConfigError::ServerNameOccupied)
    );
}

#[test]
fn unsafe_client_expansion_is_rejected_in_exactly_affected_fields() {
    for field in 0..5 {
        let mut srv = server();
        match field {
            0 => srv.executable = "/tmp/${TEST}/locust".into(),
            1 => srv.arguments.push("${TEST}".into()),
            2 => srv.paths.home = "/tmp/${TEST}".into(),
            3 => srv.paths.session = "/tmp/${TEST}".into(),
            _ => srv.paths.credential = "/tmp/${TEST}".into(),
        }
        assert_eq!(
            prepare(Client::ClaudeCode, &srv),
            Err(ConfigError::ClientExpansion)
        );
        assert_eq!(prepare(Client::Codex, &srv), Ok(()));
        for client in [Client::FactoryDroid, Client::Pi] {
            assert_eq!(
                prepare(client, &srv),
                if field >= 2 {
                    Err(ConfigError::ClientExpansion)
                } else {
                    Ok(())
                }
            );
        }
    }
    let mut srv = server();
    srv.arguments.push("~/expanded".into());
    assert_eq!(prepare(Client::Pi, &srv), Err(ConfigError::ClientExpansion));
    assert_eq!(prepare(Client::FactoryDroid, &srv), Ok(()));
}

#[test]
fn invalid_fields_reject_before_output_and_debug_redacts_paths() {
    for client in [
        Client::Codex,
        Client::ClaudeCode,
        Client::FactoryDroid,
        Client::Pi,
    ] {
        for field in 0..4 {
            let mut srv = server();
            let error = match field {
                0 => {
                    srv.executable = "locust".into();
                    ConfigError::ExecutableMustBeAbsolute
                }
                1 => {
                    srv.paths.home = "relative".into();
                    ConfigError::HomeMustBeAbsolute
                }
                2 => {
                    srv.paths.session = "relative".into();
                    ConfigError::SessionMustBeAbsolute
                }
                _ => {
                    srv.paths.credential = "relative".into();
                    ConfigError::CredentialMustBeAbsolute
                }
            };
            assert_eq!(prepare(client, &srv), Err(error));
        }
        let mut srv = server();
        srv.arguments.push("nul\0tail".into());
        assert_eq!(prepare(client, &srv), Err(ConfigError::InteriorNul));
    }
    for name in ["", "a.b", "a\"b", "a b", "x\n"] {
        assert_eq!(
            mcp_arguments(Client::Codex, name, &server(), &[]),
            Err(ConfigError::InvalidServerName)
        );
    }
    assert_eq!(
        mcp_file_overlay(Client::Pi, SERVER_NAME, &server(), &[], &json!([])),
        Err(ConfigError::InvalidBaseline)
    );
    assert_eq!(
        mcp_file_overlay(
            Client::Pi,
            SERVER_NAME,
            &server(),
            &[],
            &json!({"mcpServers":[]})
        ),
        Err(ConfigError::InvalidBaseline)
    );
    assert_eq!(
        mcp_file_overlay(Client::Codex, SERVER_NAME, &server(), &[], &json!({})),
        Err(ConfigError::LaunchArgumentsRequired)
    );
    let debug = format!("{:?} {:?}", server(), server().paths);
    assert!(!debug.contains("Applications"));
    assert!(!debug.contains("credential.json"));
}

#[cfg(unix)]
#[test]
fn non_utf8_paths_are_not_lossily_rewritten() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let mut srv = server();
    srv.paths.credential = PathBuf::from(OsString::from_vec(b"/tmp/\xff".to_vec()));
    assert_eq!(prepare(Client::Codex, &srv), Err(ConfigError::NonUtf8Path));
}
