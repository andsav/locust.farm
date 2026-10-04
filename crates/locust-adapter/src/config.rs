//! Pure MCP configuration preparation. No profiles, credentials or files are read or changed.
//!
//! Codex and Claude Code receive launch arguments, inserted before any existing
//! `--` separator. Droid and Pi receive a user-file merge proposal. The caller
//! must supply the effective occupied names (including project and managed
//! configuration), read the existing target document and apply the proposal
//! without racing another writer. A blank document is suitable only for a new,
//! isolated profile. The returned file path is relative to an explicitly chosen
//! profile home; preparing it does not select a profile or change `HOME`.
//!
//! Pass arguments directly to `std::process::Command`, never through a shell.
//! Server-specific environment paths never belong in the client's environment.
//! This module does not change approvals, sandbox rules, trust, network access,
//! tool exposure or wake behavior. Readiness requires independent qualification.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

use locust_proto::local::{CREDENTIAL_ENV, HOME_ENV, SESSION_ENV};
use serde_json::{Value, json};

/// Stable across sessions: binding and authentication use the session file,
/// not a server label. Never overwrite an occupied label, even another Locust.
pub const SERVER_NAME: &str = "locust";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Client {
    Codex,
    ClaudeCode,
    FactoryDroid,
    KimiCode,
    Pi,
}

/// Explicit references to daemon state and protected session/credential files.
/// The credential field is a file path, never credential bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct BridgePaths {
    pub home: PathBuf,
    pub session: PathBuf,
    pub credential: PathBuf,
}

impl fmt::Debug for BridgePaths {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BridgePaths").finish_non_exhaustive()
    }
}

/// Client-launched server with an absolute executable and mandatory bridge paths.
/// Arguments must not contain secret values: arbitrary secret text cannot be
/// detected here, and returned configuration may appear in client process argv.
#[derive(Clone, PartialEq, Eq)]
pub struct StdioServer {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub paths: BridgePaths,
}

impl fmt::Debug for StdioServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StdioServer")
            .field("argument_count", &self.arguments.len())
            .finish_non_exhaustive()
    }
}

/// Complete JSON after merging one server into a caller-supplied baseline.
/// Apply only to the same baseline, with appropriate file locking/atomic writes.
/// `relative_path` is relative to the caller's selected profile home. This is
/// persistent configuration unless the caller uses an isolated disposable home;
/// neither Droid nor Pi has a documented equivalent to Codex's `-c` overlay.
#[derive(Clone, PartialEq, Eq)]
pub struct ConfigFileOverlay {
    pub relative_path: PathBuf,
    pub document: Value,
}

impl fmt::Debug for ConfigFileOverlay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigFileOverlay").finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    InvalidServerName,
    ServerNameOccupied,
    ExecutableMustBeAbsolute,
    HomeMustBeAbsolute,
    SessionMustBeAbsolute,
    CredentialMustBeAbsolute,
    NonUtf8Path,
    InteriorNul,
    ClientExpansion,
    ConfigurationFileRequired,
    LaunchArgumentsRequired,
    InvalidBaseline,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidServerName => "MCP server name must use ASCII letters, digits, _ or -",
            Self::ServerNameOccupied => "MCP server name is already in use",
            Self::ExecutableMustBeAbsolute => "MCP executable must have an absolute path",
            Self::HomeMustBeAbsolute => "Locust state directory must have an absolute path",
            Self::SessionMustBeAbsolute => "Locust session file must have an absolute path",
            Self::CredentialMustBeAbsolute => "Locust credential file must have an absolute path",
            Self::NonUtf8Path => "MCP configuration requires UTF-8 paths",
            Self::InteriorNul => "MCP configuration cannot contain a NUL byte",
            Self::ClientExpansion => "MCP configuration contains syntax this client expands",
            Self::ConfigurationFileRequired => {
                "this client requires an MCP configuration file proposal"
            }
            Self::LaunchArgumentsRequired => "this client uses MCP launch arguments",
            Self::InvalidBaseline => "MCP baseline and its mcpServers field must be JSON objects",
        })
    }
}

impl std::error::Error for ConfigError {}

/// Per-launch overlays for Codex and Claude Code. The caller supplies effective
/// names; an empty list does not mean the profile has been inspected. Values may
/// contain sensitive paths: do not routinely log the returned arguments.
pub fn mcp_arguments(
    client: Client,
    name: &str,
    server: &StdioServer,
    occupied_names: &[String],
) -> Result<Vec<OsString>, ConfigError> {
    let (executable, environment) = validate(client, name, server, occupied_names)?;
    match client {
        Client::Codex => {
            let arguments = server
                .arguments
                .iter()
                .map(|argument| toml_string(argument))
                .collect::<Vec<_>>()
                .join(",");
            let env = environment
                .as_object()
                .expect("constructed environment object")
                .iter()
                .map(|(key, value)| {
                    format!(
                        "{key}={}",
                        toml_string(value.as_str().expect("constructed path string"))
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            Ok(vec![
                "-c".into(),
                format!(
                    "mcp_servers.{name}={{command={},args=[{arguments}],env={{{env}}}}}",
                    toml_string(executable)
                )
                .into(),
            ])
        }
        Client::ClaudeCode => {
            let configuration =
                json!({"mcpServers": {name: definition(executable, server, environment)}});
            // An equals-bound value prevents Claude's variadic option from
            // consuming a following positional prompt as a second config file.
            Ok(vec![format!("--mcp-config={configuration}").into()])
        }
        Client::FactoryDroid | Client::KimiCode | Client::Pi => {
            Err(ConfigError::ConfigurationFileRequired)
        }
    }
}

/// Merge one server into user-level Droid/Kimi/Pi configuration. Existing root
/// fields and unrelated servers survive; a collision is rejected. These files
/// do not bypass organization policy or Pi's project trust. Pi's default
/// codemode exposure is preserved: discovery/direct tools need qualification.
pub fn mcp_file_overlay(
    client: Client,
    name: &str,
    server: &StdioServer,
    occupied_names: &[String],
    baseline: &Value,
) -> Result<ConfigFileOverlay, ConfigError> {
    let relative_path = match client {
        Client::FactoryDroid => ".factory/mcp.json",
        Client::Pi => ".pi/agent/mcp.json",
        Client::KimiCode => ".kimi-code/mcp.json",
        Client::Codex | Client::ClaudeCode => return Err(ConfigError::LaunchArgumentsRequired),
    };
    let (executable, environment) = validate(client, name, server, occupied_names)?;
    let mut document = baseline.clone();
    let root = document
        .as_object_mut()
        .ok_or(ConfigError::InvalidBaseline)?;
    let servers = root
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or(ConfigError::InvalidBaseline)?;
    if servers
        .keys()
        .any(|occupied| names_collide(client, name, occupied))
    {
        return Err(ConfigError::ServerNameOccupied);
    }
    let mut entry = definition(executable, server, environment);
    if client == Client::KimiCode {
        entry.as_object_mut().unwrap().remove("type");
    }
    servers.insert(name.to_owned(), entry);
    Ok(ConfigFileOverlay {
        relative_path: relative_path.into(),
        document,
    })
}

fn definition(executable: &str, server: &StdioServer, environment: Value) -> Value {
    json!({"type": "stdio", "command": executable, "args": server.arguments, "env": environment})
}

fn names_collide(client: Client, name: &str, occupied: &str) -> bool {
    name == occupied
        || (client == Client::Pi && name.replace('-', "_") == occupied.replace('-', "_"))
}

fn validate<'a>(
    client: Client,
    name: &str,
    server: &'a StdioServer,
    occupied_names: &[String],
) -> Result<(&'a str, Value), ConfigError> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(ConfigError::InvalidServerName);
    }
    if occupied_names
        .iter()
        .any(|occupied| names_collide(client, name, occupied))
    {
        return Err(ConfigError::ServerNameOccupied);
    }
    let executable = absolute_path(&server.executable, ConfigError::ExecutableMustBeAbsolute)?;
    for argument in &server.arguments {
        no_nul(argument)?;
    }
    let home = absolute_path(&server.paths.home, ConfigError::HomeMustBeAbsolute)?;
    let session = absolute_path(&server.paths.session, ConfigError::SessionMustBeAbsolute)?;
    let credential = absolute_path(
        &server.paths.credential,
        ConfigError::CredentialMustBeAbsolute,
    )?;
    let environment = json!({HOME_ENV: home, SESSION_ENV: session, CREDENTIAL_ENV: credential});
    // Claude interpolates all fields. Droid/Pi interpolate env only. Pi also
    // expands leading ~/ arguments; no absolute path can start with !command.
    let environment_expands = [home, session, credential]
        .iter()
        .any(|value| value.contains("${"));
    let claude_expands = client == Client::ClaudeCode
        && (executable.contains("${") || server.arguments.iter().any(|value| value.contains("${")));
    let pi_expands =
        client == Client::Pi && server.arguments.iter().any(|value| value.starts_with("~/"));
    if (client != Client::Codex && environment_expands) || claude_expands || pi_expands {
        return Err(ConfigError::ClientExpansion);
    }
    Ok((executable, environment))
}

fn no_nul(value: &str) -> Result<(), ConfigError> {
    if value.contains('\0') {
        Err(ConfigError::InteriorNul)
    } else {
        Ok(())
    }
}

fn absolute_path(path: &Path, error: ConfigError) -> Result<&str, ConfigError> {
    if !path.is_absolute() {
        return Err(error);
    }
    let text = path.to_str().ok_or(ConfigError::NonUtf8Path)?;
    no_nul(text)?;
    Ok(text)
}

/// TOML basic-string escapes, including DEL which JSON emits literally.
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
