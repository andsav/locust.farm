//! Observe client commands and profile paths without executing or modifying them.
use crate::installation::setup::{Client, profile_paths};
use serde::Serialize;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub(super) struct Candidate {
    pub client: Client,
    pub executable: Option<PathBuf>,
    pub config: PathBuf,
    pub skill: PathBuf,
    pub launcher: PathBuf,
    pub profile_present: bool,
}

pub(super) fn discover(profile_home: &Path, search_path: Option<&OsStr>) -> Vec<Candidate> {
    [
        (Client::Codex, "codex"),
        (Client::Claude, "claude"),
        (Client::Pi, "pi"),
    ]
    .into_iter()
    .map(|(client, command)| {
        let (config, skill, launcher) = profile_paths(client, profile_home);
        // A dangling symlink or other collision still occupies a profile path.
        let profile_present =
            fs::symlink_metadata(&config).is_ok() || fs::symlink_metadata(&skill).is_ok();
        Candidate {
            client,
            executable: find_executable(command, search_path),
            config,
            skill,
            launcher,
            profile_present,
        }
    })
    .collect()
}

fn find_executable(command: &str, search_path: Option<&OsStr>) -> Option<PathBuf> {
    std::env::split_paths(search_path?).find_map(|directory| {
        let path = directory.join(command);
        let metadata = fs::metadata(&path).ok()?;
        (metadata.is_file() && metadata.permissions().mode() & 0o111 != 0).then_some(path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn executable(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"#!/bin/sh\nexit 99\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[test]
    fn path_order_selects_first_regular_executable() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        executable(&first.join("codex"));
        executable(&second.join("codex"));
        let search_path = std::env::join_paths([&first, &second]).unwrap();
        let candidates = discover(&directory.path().join("profile"), Some(&search_path));
        assert_eq!(candidates[0].executable, Some(first.join("codex")));
        let reversed = std::env::join_paths([&second, &first]).unwrap();
        assert_eq!(
            discover(directory.path(), Some(&reversed))[0].executable,
            Some(second.join("codex"))
        );
    }

    #[test]
    fn missing_and_nonexecutable_commands_are_not_discovered() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        fs::create_dir_all(first.join("claude")).unwrap();
        fs::write(first.join("codex"), b"not executable").unwrap();
        fs::set_permissions(first.join("codex"), fs::Permissions::from_mode(0o600)).unwrap();
        executable(&second.join("codex"));
        executable(&second.join("claude"));
        let search_path = std::env::join_paths([&first, &second]).unwrap();
        let candidates = discover(directory.path(), Some(&search_path));
        assert_eq!(candidates[0].executable, Some(second.join("codex")));
        assert_eq!(candidates[1].executable, Some(second.join("claude")));
        assert!(candidates[2].executable.is_none());
        assert!(
            discover(directory.path(), None)
                .iter()
                .all(|candidate| candidate.executable.is_none())
        );
        let missing = directory.path().join("missing");
        assert!(
            discover(directory.path(), Some(missing.as_os_str()))
                .iter()
                .all(|candidate| candidate.executable.is_none())
        );
    }

    #[test]
    fn candidates_map_each_standard_profile_path() {
        let directory = tempfile::tempdir().unwrap();
        let candidates = discover(directory.path(), None);
        let expected = [
            (
                Client::Codex,
                ".codex/config.toml",
                ".agents/skills/locust/SKILL.md",
            ),
            (
                Client::Claude,
                ".claude.json",
                ".claude/skills/locust/SKILL.md",
            ),
            (
                Client::Pi,
                ".pi/agent/mcp.json",
                ".pi/agent/skills/locust/SKILL.md",
            ),
        ];
        assert_eq!(candidates.len(), expected.len());
        for (candidate, (client, config, skill)) in candidates.iter().zip(expected) {
            assert_eq!(candidate.client, client);
            assert_eq!(candidate.config, directory.path().join(config));
            assert_eq!(candidate.skill, directory.path().join(skill));
            assert_eq!(
                candidate.launcher,
                candidate.skill.with_file_name("locust-cli")
            );
            assert!(!candidate.profile_present);
        }
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn profile_collisions_are_present_and_discovery_preserves_files() {
        let directory = tempfile::tempdir().unwrap();
        let profile = directory.path().join("profile");
        let (codex_config, _, _) = profile_paths(Client::Codex, &profile);
        let (claude_config, _, _) = profile_paths(Client::Claude, &profile);
        let (_, pi_skill, _) = profile_paths(Client::Pi, &profile);
        fs::create_dir_all(codex_config.parent().unwrap()).unwrap();
        fs::write(&codex_config, b"unreadable configuration\xff").unwrap();
        fs::set_permissions(&codex_config, fs::Permissions::from_mode(0o000)).unwrap();
        symlink("absent-config", &claude_config).unwrap();
        fs::create_dir_all(&pi_skill).unwrap();
        let binary = directory.path().join("bin/codex");
        executable(&binary);
        let marker = directory.path().join("executed");
        let script = format!("#!/bin/sh\ntouch '{}'\n", marker.display());
        fs::write(&binary, &script).unwrap();
        let search_path = binary.parent().unwrap().as_os_str();

        let candidates = discover(&profile, Some(search_path));

        assert!(candidates.iter().all(|candidate| candidate.profile_present));
        assert!(
            candidates
                .iter()
                .all(|candidate| !candidate.launcher.exists())
        );
        assert!(!marker.exists());
        assert_eq!(fs::read(&binary).unwrap(), script.as_bytes());
        assert_eq!(
            fs::read_link(&claude_config).unwrap(),
            Path::new("absent-config")
        );
        assert!(pi_skill.is_dir());
        assert_eq!(
            fs::metadata(&codex_config).unwrap().permissions().mode() & 0o777,
            0
        );
        fs::set_permissions(&codex_config, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            fs::read(codex_config).unwrap(),
            b"unreadable configuration\xff"
        );
    }
}
