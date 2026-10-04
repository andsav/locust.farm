//! Embeds the Git commit the binary is built from, for `locust --version`.
//!
//! The value is the 12-character commit, with `-dirty` appended when tracked
//! files differ from it, or `unknown` when Git or the repository is absent.
//! Setting `LOCUST_BUILD_COMMIT` in the build environment overrides it, for
//! builds from an archive. The build never fails over this.

use std::path::Path;
use std::process::Command;

const VARIABLE: &str = "LOCUST_BUILD_COMMIT";

fn main() {
    println!("cargo:rerun-if-env-changed={VARIABLE}");
    let given = std::env::var(VARIABLE)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value.chars().all(|c| c.is_ascii_graphic()));
    let commit = given
        .or_else(from_git)
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env={VARIABLE}={commit}");
}

/// Output of one Git command, trimmed; `None` when Git is missing or fails.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_string())
}

fn from_git() -> Option<String> {
    let commit = git(&["rev-parse", "--short=12", "HEAD"])?;
    if commit.is_empty() || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    watch_repository();
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
        .is_some_and(|changes| !changes.is_empty());
    Some(if dirty {
        format!("{commit}-dirty")
    } else {
        commit
    })
}

/// Asks Cargo to run this script again when the commit or the sources that
/// decide `-dirty` change: the head's log, and the workspace's crates and
/// manifests.
fn watch_repository() {
    if let Some(git_dir) = git(&["rev-parse", "--absolute-git-dir"]) {
        for name in ["HEAD", "logs/HEAD"] {
            let path = Path::new(&git_dir).join(name);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
    if let Some(root) = git(&["rev-parse", "--show-toplevel"]) {
        for name in ["crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"] {
            let path = Path::new(&root).join(name);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}
