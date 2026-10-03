//! Hostile user-level and repository configuration does not reach an
//! export. This binary holds a single test because it changes the process
//! environment.

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use locust_workspace::export;
use support::{MemBlobs, TestRepo, list_dir};

/// Writes an executable script at `path` that records `name` in `markers`
/// and, for a filter, passes its input through unchanged.
fn marker_script(path: &Path, markers: &Path, name: &str) -> String {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!(
            "#!/bin/sh\ntouch '{}/{name}'\nexec cat\n",
            markers.display()
        ),
    )
    .unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    path.display().to_string()
}

#[test]
fn export_runs_no_filter_hook_or_fsmonitor_named_by_user_or_repository_configuration() {
    let repo = TestRepo::new();
    repo.write(
        ".gitattributes",
        b"user.txt filter=evil diff=evil\nrepo.txt filter=repo\n",
    );
    repo.write("user.txt", b"raw user bytes\n");
    repo.write("repo.txt", b"raw repo bytes\n");
    repo.commit_all();

    let hostile = tempfile::tempdir().unwrap();
    let markers = hostile.path().join("markers");
    fs::create_dir(&markers).unwrap();
    let bin = hostile.path().join("bin");
    let hooks = hostile.path().join("hooks");
    for hook in [
        "post-checkout",
        "post-commit",
        "post-index-change",
        "pre-auto-gc",
        "pre-commit",
        "reference-transaction",
    ] {
        marker_script(&hooks.join(hook), &markers, &format!("hook-{hook}"));
    }
    let script = |name: &str| marker_script(&bin.join(name), &markers, name);

    // User level: a filter and a text conversion for user.txt, hooks and
    // fsmonitor.
    let home = hostile.path().join("home");
    let user_config = format!(
        "[filter \"evil\"]\n\tclean = {}\n\tsmudge = {}\n\trequired = true\n\
         [diff \"evil\"]\n\ttextconv = {}\n\
         [core]\n\thooksPath = {}\n\tfsmonitor = {}\n",
        script("clean-user"),
        script("smudge-user"),
        script("textconv-user"),
        hooks.display(),
        script("fsmonitor-user"),
    );
    fs::create_dir_all(home.join(".config/git")).unwrap();
    fs::write(home.join(".gitconfig"), &user_config).unwrap();
    fs::write(home.join(".config/git/config"), &user_config).unwrap();

    // Repository level, which Git does read during an export: a filter for
    // repo.txt, hooks and fsmonitor.
    repo.git(["config", "filter.repo.smudge", &script("smudge-repo")]);
    repo.git(["config", "filter.repo.clean", &script("clean-repo")]);
    repo.git(["config", "filter.repo.required", "true"]);
    repo.git(["config", "core.hooksPath", &hooks.display().to_string()]);
    repo.git(["config", "core.fsmonitor", &script("fsmonitor-repo")]);

    // SAFETY: this is the only test in this binary, and no other thread
    // reads the environment while it changes.
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_CONFIG_HOME", home.join(".config"));
        std::env::set_var("GIT_CONFIG_GLOBAL", home.join(".gitconfig"));
    }

    // The configuration is live: an ordinary checkout runs both filters, a
    // hook and fsmonitor.
    fs::remove_file(repo.path().join("user.txt")).unwrap();
    fs::remove_file(repo.path().join("repo.txt")).unwrap();
    let checkout = Command::new("git")
        .current_dir(repo.path())
        .args(["checkout", "--", "user.txt", "repo.txt"])
        .output()
        .unwrap();
    assert!(
        checkout.status.success(),
        "{}",
        String::from_utf8_lossy(&checkout.stderr)
    );
    let ran = list_dir(&markers);
    for expected in [
        "fsmonitor-repo",
        "hook-post-checkout",
        "smudge-repo",
        "smudge-user",
    ] {
        assert!(ran.iter().any(|name| name == expected), "{ran:?}");
    }
    for marker in ran {
        fs::remove_file(markers.join(marker)).unwrap();
    }

    // An inherited GIT_DIR naming another repository is ignored too.
    let decoy = TestRepo::new();
    decoy.write("decoy.txt", b"decoy\n");
    decoy.commit_all();
    // SAFETY: as above.
    unsafe { std::env::set_var("GIT_DIR", decoy.path().join(".git")) };

    let mut store = MemBlobs::default();
    let report = export(repo.path(), "HEAD", &mut store).unwrap();

    assert_eq!(list_dir(&markers), Vec::<String>::new());
    let files: Vec<_> = report
        .manifest
        .entries
        .iter()
        .map(|entry| {
            (
                entry.path.as_str(),
                store.objects[&entry.content].as_slice(),
            )
        })
        .collect();
    assert_eq!(
        files,
        [
            (
                ".gitattributes",
                b"user.txt filter=evil diff=evil\nrepo.txt filter=repo\n".as_slice()
            ),
            ("repo.txt", b"raw repo bytes\n"),
            ("user.txt", b"raw user bytes\n"),
        ]
    );
}
