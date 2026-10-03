//! Fixture-friendly pure proposal emitter: no writes, profiles or credential reads.
//! Example: config_probe --client codex --executable /abs/stdio_probe
//! --locust-home /abs/home --session /abs/session.json --credential /abs/key
//! --argument=--fixture. File relative_path values are rooted at harness HOME;
//! the empty baseline is valid only for that isolated, newly created profile.
use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;

use locust_adapter::config::{
    BridgePaths, Client, SERVER_NAME, StdioServer, mcp_arguments, mcp_file_overlay,
};
use serde_json::{Value, json};

fn main() {
    if let Err(error) = run() {
        eprintln!("config_probe: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    println!("{}", proposal(std::env::args_os().skip(1))?);
    Ok(())
}

fn proposal(args: impl IntoIterator<Item = OsString>) -> Result<Value, Box<dyn Error>> {
    let mut client = None;
    let mut executable = None;
    let mut home = None;
    let mut session = None;
    let mut credential = None;
    let mut arguments = Vec::new();
    let mut occupied = Vec::new();
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        let argument = argument.to_str().ok_or("CLI arguments must be UTF-8")?;
        if let Some(value) = argument.strip_prefix("--argument=") {
            arguments.push(value.to_owned());
            continue;
        }
        let value = args.next().ok_or("option requires a value")?;
        let value = value.to_str().ok_or("CLI arguments must be UTF-8")?;
        match argument {
            "--client" => {
                if client.is_some() {
                    return Err("duplicate --client".into());
                }
                client = Some(match value {
                    "codex" => Client::Codex,
                    "claude-code" => Client::ClaudeCode,
                    "factory-droid" => Client::FactoryDroid,
                    "pi" => Client::Pi,
                    _ => return Err("unknown client".into()),
                });
            }
            "--executable" => set_path(&mut executable, value)?,
            "--locust-home" => set_path(&mut home, value)?,
            "--session" => set_path(&mut session, value)?,
            "--credential" => set_path(&mut credential, value)?,
            "--occupied-name" => occupied.push(value.to_owned()),
            _ => return Err("unknown option; use --argument=VALUE for server arguments".into()),
        }
    }
    let client = client.ok_or("missing --client")?;
    let server = StdioServer {
        executable: executable.ok_or("missing --executable")?,
        arguments,
        paths: BridgePaths {
            home: home.ok_or("missing --locust-home")?,
            session: session.ok_or("missing --session")?,
            credential: credential.ok_or("missing --credential")?,
        },
    };
    let (arguments, files) = match client {
        Client::Codex | Client::ClaudeCode => {
            let args = mcp_arguments(client, SERVER_NAME, &server, &occupied)?;
            (
                args.iter()
                    .map(|arg| arg.to_str().ok_or("non-UTF-8 launch argument"))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
                Vec::new(),
            )
        }
        Client::FactoryDroid | Client::Pi => {
            let overlay = mcp_file_overlay(client, SERVER_NAME, &server, &occupied, &json!({}))?;
            (
                Vec::new(),
                vec![
                    json!({"relative_path": overlay.relative_path, "content": overlay.document.to_string()}),
                ],
            )
        }
    };
    Ok(
        json!({"arguments": arguments,"files":files,"environment":{},"transport":"stdio","server_name":SERVER_NAME}),
    )
}

fn set_path(target: &mut Option<PathBuf>, value: &str) -> Result<(), Box<dyn Error>> {
    if target.is_some() {
        return Err("duplicate path option".into());
    }
    *target = Some(value.into());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(client: &str) -> Vec<OsString> {
        [
            "--client",
            client,
            "--executable",
            "/fixture/probe",
            "--locust-home",
            "/fixture/home",
            "--session",
            "/fixture/session",
            "--credential",
            "/fixture/credential",
            "--argument=--fixture",
        ]
        .into_iter()
        .map(OsString::from)
        .collect()
    }

    #[test]
    fn rejects_duplicate_scalar_options() {
        for (option, value, expected) in [
            ("--client", "codex", "duplicate --client"),
            ("--executable", "/other", "duplicate path option"),
            ("--locust-home", "/other", "duplicate path option"),
            ("--session", "/other", "duplicate path option"),
            ("--credential", "/other", "duplicate path option"),
        ] {
            let mut arguments = args("codex");
            arguments.extend([option.into(), value.into()]);
            assert_eq!(proposal(arguments).unwrap_err().to_string(), expected);
        }
    }

    #[test]
    fn missing_options_and_values_are_errors_without_echoing_input() {
        for option in [
            "--client",
            "--executable",
            "--locust-home",
            "--session",
            "--credential",
        ] {
            let mut arguments = args("codex");
            let index = arguments
                .iter()
                .position(|argument| argument == option)
                .unwrap();
            arguments.drain(index..index + 2);
            assert_eq!(
                proposal(arguments).unwrap_err().to_string(),
                format!("missing {option}")
            );
        }
        let mut arguments = args("codex");
        arguments.push("--occupied-name".into());
        assert_eq!(
            proposal(arguments).unwrap_err().to_string(),
            "option requires a value"
        );
        let mut arguments = args("codex");
        arguments.extend([
            "--private-unrecognized-value".into(),
            "private-value".into(),
        ]);
        let error = proposal(arguments).unwrap_err().to_string();
        assert!(!error.contains("private"));
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_options_values_and_inline_arguments_are_redacted_errors() {
        use std::os::unix::ffi::OsStringExt;
        for bytes in [
            b"--private-\xff".as_slice(),
            b"--argument=private-\xff".as_slice(),
        ] {
            let mut arguments = args("codex");
            arguments.push(OsString::from_vec(bytes.to_vec()));
            assert_eq!(
                proposal(arguments).unwrap_err().to_string(),
                "CLI arguments must be UTF-8"
            );
        }
        let mut arguments = args("codex");
        arguments[3] = OsString::from_vec(b"/private-\xff".to_vec());
        assert_eq!(
            proposal(arguments).unwrap_err().to_string(),
            "CLI arguments must be UTF-8"
        );
    }

    #[test]
    fn each_client_produces_its_real_configuration_interface() {
        for client in ["codex", "claude-code", "factory-droid", "pi"] {
            let proposal = proposal(args(client)).unwrap();
            assert_eq!(proposal["environment"], json!({}));
            assert_eq!(proposal["server_name"], SERVER_NAME);
            assert_eq!(proposal["transport"], "stdio");
            if matches!(client, "factory-droid" | "pi") {
                assert_eq!(proposal["arguments"], json!([]));
                assert_eq!(proposal["files"].as_array().unwrap().len(), 1);
                assert_eq!(
                    proposal["files"][0]["relative_path"],
                    if client == "pi" {
                        ".pi/agent/mcp.json"
                    } else {
                        ".factory/mcp.json"
                    }
                );
                let config: Value =
                    serde_json::from_str(proposal["files"][0]["content"].as_str().unwrap())
                        .unwrap();
                assert_eq!(
                    config["mcpServers"][SERVER_NAME]["args"],
                    json!(["--fixture"])
                );
                assert_eq!(
                    config["mcpServers"][SERVER_NAME]["env"]["LOCUST_SESSION"],
                    "/fixture/session"
                );
            } else {
                assert_eq!(proposal["files"], json!([]));
                let arguments = proposal["arguments"].as_array().unwrap();
                assert_eq!(arguments.len(), if client == "codex" { 2 } else { 1 });
                assert!(
                    arguments[0]
                        .as_str()
                        .unwrap()
                        .starts_with(if client == "codex" {
                            "-c"
                        } else {
                            "--mcp-config="
                        })
                );
            }
        }
    }
}
