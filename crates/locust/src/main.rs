//! The `locust` binary: the daemon and its command-line client.
use std::process::ExitCode;
mod cli;
mod connection;
mod daemon;
mod failure;
mod installation;
mod mcp;
mod package;
mod secret;
#[cfg(test)]
mod testdir;
mod version;
fn main() -> ExitCode {
    ExitCode::from(cli::run())
}
