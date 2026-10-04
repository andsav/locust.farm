//! The `locust` binary: the daemon and its command-line client.
use std::process::ExitCode;
mod cli;
mod daemon;
mod failure;
mod secret;
#[cfg(test)]
mod testdir;
mod version;
fn main() -> ExitCode {
    ExitCode::from(cli::run())
}
