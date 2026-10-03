//! Pure per-run MCP registration. No profile files are read or changed.
//!
//! The caller supplies names from the client's effective configuration for
//! collision checking, then inserts the returned arguments before any
//! existing `--` separator, preserving option/value pairs. For Claude, a `--`
//! separator must precede the positional prompt:
//! `--mcp-config` accepts multiple values. Pass each argument directly to
//! `std::process::Command`, never through a shell.
//!
//! This prepares configuration only. Readiness still requires a successful
//! MCP initialization, tool discovery and authenticated daemon roundtrip.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

use locust_proto::id::InstanceId;
use serde_json::json;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Client {
    Codex,
    ClaudeCode,
}

/// A server launched by the client, using an absolute executable path.
///
/// Caller convention: arguments must not contain credentials, since generated
/// configuration appears in client process arguments. Arbitrary secret text
/// cannot be detected here. `locust_home` is a directory reference,
/// not a credential. Protected session credential provisioning is a separate
/// daemon/adapter contract; it is not inferred from the server name.
#[derive(Clone, PartialEq, Eq)]
pub struct StdioServer {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub locust_home: Option<PathBuf>,
}

impl fmt::Debug for StdioServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StdioServer")
            .field("argument_count", &self.arguments.len())
            .field("has_locust_home", &self.locust_home.is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    InvalidServerName,
    ServerNameOccupied,
    ExecutableMustBeAbsolute,
    HomeMustBeAbsolute,
    NonUtf8Path,
    InteriorNul,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidServerName => "MCP server name must use ASCII letters, digits, _ or -",
            Self::ServerNameOccupied => "MCP server name is already in use",
            Self::ExecutableMustBeAbsolute => "MCP executable must have an absolute path",
            Self::HomeMustBeAbsolute => "Locust state directory must have an absolute path",
            Self::NonUtf8Path => "MCP configuration requires UTF-8 paths",
            Self::InteriorNul => "MCP configuration cannot contain a NUL byte",
        })
    }
}

impl std::error::Error for ConfigError {}

/// Names are scoped to a launch/session. They are labels, not authentication.
pub fn session_server_name(instance: InstanceId) -> String {
    format!("locust_{instance}")
}

/// Arguments targeting one server without emitting unrelated configuration or
/// policy changes. Effective client behavior requires separate qualification.
/// `occupied_names` must come from the client's
/// effective configuration; an empty list is not a claim it was inspected.
///
/// The return value may contain sensitive paths/arguments. Do not log it as
/// routine diagnostics. This function never enables approval bypass, workspace
/// trust, network access, writable roots or global MCP-configuration replacement.
pub fn mcp_arguments(
    client: Client,
    name: &str,
    server: &StdioServer,
    occupied_names: &[String],
) -> Result<Vec<OsString>, ConfigError> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(ConfigError::InvalidServerName);
    }
    if occupied_names.iter().any(|occupied| occupied == name) {
        return Err(ConfigError::ServerNameOccupied);
    }
    if !server.executable.is_absolute() {
        return Err(ConfigError::ExecutableMustBeAbsolute);
    }
    let executable = path_text(&server.executable)?;
    for argument in &server.arguments {
        no_nul(argument)?;
    }
    let home = server
        .locust_home
        .as_deref()
        .map(|path| {
            if !path.is_absolute() {
                return Err(ConfigError::HomeMustBeAbsolute);
            }
            path_text(path)
        })
        .transpose()?;

    match client {
        Client::Codex => {
            let arguments = server
                .arguments
                .iter()
                .map(|argument| toml_string(argument))
                .collect::<Vec<_>>()
                .join(",");
            let mut fields = format!("command={},args=[{}]", toml_string(executable), arguments,);
            if let Some(home) = home {
                fields.push_str(&format!(",env={{LOCUST_HOME={}}}", toml_string(home)));
            }
            Ok(vec![
                "-c".into(),
                format!("mcp_servers.{name}={{{fields}}}").into(),
            ])
        }
        Client::ClaudeCode => {
            let mut definition = json!({
                "type": "stdio",
                "command": executable,
                "args": server.arguments,
            });
            if let Some(home) = home {
                definition["env"] = json!({"LOCUST_HOME": home});
            }
            let configuration = json!({"mcpServers": {name: definition}});
            Ok(vec![
                "--mcp-config".into(),
                configuration.to_string().into(),
            ])
        }
    }
}

fn no_nul(value: &str) -> Result<(), ConfigError> {
    if value.contains('\0') {
        Err(ConfigError::InteriorNul)
    } else {
        Ok(())
    }
}

fn path_text(path: &Path) -> Result<&str, ConfigError> {
    let text = path.to_str().ok_or(ConfigError::NonUtf8Path)?;
    no_nul(text)?;
    Ok(text)
}

/// TOML basic-string escapes. Encode controls explicitly, including DEL,
/// which is forbidden literally in TOML but is emitted literally by JSON.
fn toml_string(value: &str) -> String {
    let mut quoted = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\u{0000}'..='\u{001f}' | '\u{007f}' => {
                use std::fmt::Write;
                write!(quoted, "\\u{:04X}", character as u32).expect("writing to a String");
            }
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> StdioServer {
        StdioServer {
            executable: PathBuf::from("/Applications/Locust App/locust"),
            arguments: vec!["mcp".into(), "a\"b\\c\n\u{7f} $(touch /no)".into()],
            locust_home: Some(PathBuf::from("/tmp/locust home")),
        }
    }

    #[test]
    fn claude_overlay_preserves_argument_boundaries() {
        let server = server();
        let args = mcp_arguments(Client::ClaudeCode, "locust_run", &server, &[]).unwrap();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "--mcp-config");
        let config: serde_json::Value = serde_json::from_str(args[1].to_str().unwrap()).unwrap();
        let servers = config["mcpServers"].as_object().unwrap();
        assert_eq!(servers.len(), 1);
        let actual = &servers["locust_run"];
        assert_eq!(actual["command"], server.executable.to_str().unwrap());
        assert_eq!(actual["args"], json!(server.arguments));
        assert_eq!(actual["env"]["LOCUST_HOME"], "/tmp/locust home");
    }

    #[test]
    fn codex_overlay_targets_one_server_and_escapes_toml_controls() {
        let args = mcp_arguments(Client::Codex, "locust_run", &server(), &[]).unwrap();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "-c");
        let value = args[1].to_str().unwrap();
        assert!(value.starts_with("mcp_servers.locust_run={command="));
        assert!(!value.contains('\n'));
        assert!(!value.contains('\u{7f}'));
        assert!(value.contains("\\u000A\\u007F"));
        assert!(value.contains("$(touch /no)")); // Literal argv content, never a shell.
    }

    #[test]
    fn occupied_and_invalid_names_are_rejected_without_overwriting() {
        for client in [Client::Codex, Client::ClaudeCode] {
            assert_eq!(
                mcp_arguments(client, "locust_run", &server(), &["locust_run".into()]),
                Err(ConfigError::ServerNameOccupied),
            );
            for name in ["", "a.b", "a\"b", "a b", "x\n"] {
                assert_eq!(
                    mcp_arguments(client, name, &server(), &[]),
                    Err(ConfigError::InvalidServerName),
                );
            }
        }
    }

    #[test]
    fn paths_and_arguments_are_checked_before_building_config() {
        let mut server = server();
        server.executable = PathBuf::from("locust");
        assert_eq!(
            mcp_arguments(Client::Codex, "x", &server, &[]),
            Err(ConfigError::ExecutableMustBeAbsolute)
        );
        server.executable = PathBuf::from("/bin/locust");
        server.locust_home = Some(PathBuf::from("relative"));
        assert_eq!(
            mcp_arguments(Client::Codex, "x", &server, &[]),
            Err(ConfigError::HomeMustBeAbsolute)
        );
        server.locust_home = None;
        server.arguments.push("secret\0suffix".into());
        assert_eq!(
            mcp_arguments(Client::Codex, "x", &server, &[]),
            Err(ConfigError::InteriorNul)
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_are_not_lossily_rewritten() {
        use std::os::unix::ffi::OsStringExt;
        let mut server = server();
        server.executable = PathBuf::from(OsString::from_vec(b"/tmp/\xff".to_vec()));
        assert_eq!(
            mcp_arguments(Client::Codex, "x", &server, &[]),
            Err(ConfigError::NonUtf8Path)
        );
    }

    #[test]
    fn debug_does_not_reveal_server_configuration() {
        let actual = format!("{:?}", server());
        assert!(!actual.contains("Applications"));
        assert!(!actual.contains("touch"));
        assert!(!actual.contains("locust home"));
    }
}
