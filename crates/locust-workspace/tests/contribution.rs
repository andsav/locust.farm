mod support;

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};

use locust_workspace::{
    BlobSink, ContributionError, apply_contribution, create_committed, create_selected, export,
    materialize, review_contribution,
};
use support::{MemBlobs, TestRepo, manifest_of};

#[test]
fn selected_review_apply_and_retry_preserve_unrelated_files() {
    let mut store = MemBlobs::default();
    let base = manifest_of(
        &mut store,
        &[
            ("edit", b"old\n", false),
            ("delete", b"remove", false),
            ("mode", b"same", false),
        ],
    );
    let base_id = store.store(&base.encode().unwrap()).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let contributor = temp.path().join("contributor");
    materialize(&base, &mut store, &contributor).unwrap();
    fs::write(contributor.join("edit"), b"new\n").unwrap();
    fs::remove_file(contributor.join("delete")).unwrap();
    fs::set_permissions(contributor.join("mode"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(contributor.join("add"), b"\0binary").unwrap();
    let report = create_selected(
        &contributor,
        &["edit".into(), "delete".into(), "mode".into(), "add".into()],
        base_id,
        &mut store,
    )
    .unwrap();
    let review = review_contribution(report.contribution_id, &mut store).unwrap();
    assert_eq!(review.changes.len(), 4);
    assert!(
        review
            .changes
            .iter()
            .find(|c| c.path == "edit")
            .unwrap()
            .unified_diff
            .as_ref()
            .unwrap()
            .contains("+new\n")
    );
    assert!(
        review
            .changes
            .iter()
            .find(|c| c.path == "add")
            .unwrap()
            .unified_diff
            .is_none()
    );
    assert!(
        review
            .changes
            .iter()
            .find(|c| c.path == "mode")
            .unwrap()
            .after
            .as_ref()
            .unwrap()
            .executable
    );
    let destination = temp.path().join("destination");
    materialize(&base, &mut store, &destination).unwrap();
    fs::write(destination.join("unrelated"), b"local WIP").unwrap();
    let applied = apply_contribution(
        report.contribution_id,
        &mut store,
        &destination,
        base_id,
        report.contribution.head,
        None,
    )
    .unwrap();
    assert_eq!(applied.applied.len(), 4);
    assert_eq!(
        fs::read(destination.join("unrelated")).unwrap(),
        b"local WIP"
    );
    assert_eq!(fs::read(destination.join("edit")).unwrap(), b"new\n");
    assert!(!destination.join("delete").exists());
    assert_eq!(
        fs::read(applied.recovery_directory.unwrap().join("original-2")).unwrap(),
        b"old\n"
    );
    let retried = apply_contribution(
        report.contribution_id,
        &mut store,
        &destination,
        base_id,
        report.contribution.head,
        None,
    )
    .unwrap();
    assert!(retried.applied.is_empty());
    assert_eq!(retried.already_applied.len(), 4);
}

#[test]
fn conflict_preflight_changes_no_files_and_refuses_links() {
    let mut store = MemBlobs::default();
    let base = manifest_of(&mut store, &[("a", b"old", false), ("z", b"old", false)]);
    let base_id = store.store(&base.encode().unwrap()).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let contributor = temp.path().join("source");
    materialize(&base, &mut store, &contributor).unwrap();
    fs::write(contributor.join("a"), b"new").unwrap();
    fs::write(contributor.join("z"), b"new").unwrap();
    let report =
        create_selected(&contributor, &["a".into(), "z".into()], base_id, &mut store).unwrap();
    let destination = temp.path().join("destination");
    materialize(&base, &mut store, &destination).unwrap();
    fs::write(destination.join("z"), b"WIP").unwrap();
    assert!(matches!(
        apply_contribution(
            report.contribution_id,
            &mut store,
            &destination,
            base_id,
            report.contribution.head,
            None
        ),
        Err(ContributionError::Conflict { .. })
    ));
    assert_eq!(fs::read(destination.join("a")).unwrap(), b"old");
    fs::remove_file(destination.join("z")).unwrap();
    symlink(contributor.join("z"), destination.join("z")).unwrap();
    assert!(matches!(
        apply_contribution(
            report.contribution_id,
            &mut store,
            &destination,
            base_id,
            report.contribution.head,
            None
        ),
        Err(ContributionError::Unsupported { .. })
    ));
    fs::remove_file(destination.join("z")).unwrap();
    fs::hard_link(destination.join("a"), destination.join("z")).unwrap();
    assert!(matches!(
        create_selected(&destination, &["a".into()], base_id, &mut store),
        Err(ContributionError::Unsupported { .. })
    ));
}

#[test]
fn committed_capture_ignores_dirty_worktree_and_index() {
    let repo = TestRepo::new();
    repo.write("file", b"base");
    let first = repo.commit_all();
    let mut store = MemBlobs::default();
    let base_id = export(repo.path(), &first, &mut store).unwrap().manifest_id;
    repo.write("file", b"committed");
    let head = repo.commit_all();
    repo.write("file", b"staged WIP");
    repo.git(["add", "file"]);
    repo.write("file", b"unstaged WIP");
    repo.write("untracked", b"private local");
    let status = repo.git(["status", "--porcelain"]);
    let report = create_committed(repo.path(), &head, base_id, &mut store).unwrap();
    let review = review_contribution(report.contribution_id, &mut store).unwrap();
    assert_eq!(review.changes.len(), 1);
    assert!(
        review.changes[0]
            .unified_diff
            .as_ref()
            .unwrap()
            .contains("+committed")
    );
    assert_eq!(repo.git(["status", "--porcelain"]), status);
    assert_eq!(fs::read(repo.path().join("file")).unwrap(), b"unstaged WIP");
    assert!(matches!(
        apply_contribution(
            report.contribution_id,
            &mut store,
            repo.path(),
            base_id,
            report.contribution.head,
            Some(&first)
        ),
        Err(ContributionError::HeadChanged { .. })
    ));
}

#[test]
fn mid_apply_failure_reports_recovery_with_originals() {
    let mut store = MemBlobs::default();
    let base = manifest_of(
        &mut store,
        &[("a", b"old a", false), ("blocked/z", b"old z", false)],
    );
    let base_id = store.store(&base.encode().unwrap()).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let contributor = temp.path().join("source");
    materialize(&base, &mut store, &contributor).unwrap();
    fs::write(contributor.join("a"), b"new a").unwrap();
    fs::write(contributor.join("blocked/z"), b"new z").unwrap();
    let report = create_selected(
        &contributor,
        &["a".into(), "blocked/z".into()],
        base_id,
        &mut store,
    )
    .unwrap();
    let destination = temp.path().join("destination");
    materialize(&base, &mut store, &destination).unwrap();
    let blocked = destination.join("blocked");
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o555)).unwrap();
    // Elevated test runners can bypass directory permissions; do not pretend
    // to have exercised failure injection when the OS grants that authority.
    let probe = blocked.join("permission-probe");
    if fs::write(&probe, b"").is_ok() {
        fs::remove_file(probe).unwrap();
        fs::set_permissions(blocked, fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let result = apply_contribution(
        report.contribution_id,
        &mut store,
        &destination,
        base_id,
        report.contribution.head,
        None,
    );
    fs::set_permissions(&blocked, fs::Permissions::from_mode(0o755)).unwrap();
    let Err(ContributionError::RecoveryRequired { path, .. }) = result else {
        panic!("expected a recoverable partial application");
    };
    assert_eq!(fs::read(path.join("original-0")).unwrap(), b"old a");
    assert_eq!(
        fs::read(path.join("contribution")).unwrap(),
        report.contribution.encode().unwrap()
    );
    assert_eq!(fs::read(destination.join("a")).unwrap(), b"new a");
    assert_eq!(fs::read(destination.join("blocked/z")).unwrap(), b"old z");
    let retry = apply_contribution(
        report.contribution_id,
        &mut store,
        &destination,
        base_id,
        report.contribution.head,
        None,
    )
    .unwrap();
    assert_eq!(retry.already_applied, ["a"]);
    assert_eq!(retry.applied, ["blocked/z"]);
}

#[test]
fn committed_file_directory_transitions_and_unrelated_descendant_conflict() {
    for directory_first in [false, true] {
        let repo = TestRepo::new();
        let first_path = if directory_first {
            "shape/child"
        } else {
            "shape"
        };
        let second_path = if directory_first {
            "shape"
        } else {
            "shape/child"
        };
        repo.write(first_path, b"old");
        let first = repo.commit_all();
        let mut store = MemBlobs::default();
        let base = export(repo.path(), &first, &mut store).unwrap();
        fs::remove_file(repo.path().join(first_path)).unwrap();
        if directory_first {
            fs::remove_dir(repo.path().join("shape")).unwrap();
        }
        repo.write(second_path, b"new");
        let head = repo.commit_all();
        let report = create_committed(repo.path(), &head, base.manifest_id, &mut store).unwrap();
        let selected = create_selected(
            repo.path(),
            &[first_path.into(), second_path.into()],
            base.manifest_id,
            &mut store,
        )
        .unwrap();
        assert_eq!(selected.head_manifest, report.head_manifest);
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("destination");
        materialize(&base.manifest, &mut store, &destination).unwrap();
        if directory_first {
            fs::write(destination.join("shape/unrelated"), b"WIP").unwrap();
            assert!(matches!(
                apply_contribution(
                    report.contribution_id,
                    &mut store,
                    &destination,
                    base.manifest_id,
                    report.contribution.head,
                    None
                ),
                Err(ContributionError::Conflict { .. })
            ));
            assert_eq!(fs::read(destination.join(first_path)).unwrap(), b"old");
            fs::remove_file(destination.join("shape/unrelated")).unwrap();
        }
        apply_contribution(
            report.contribution_id,
            &mut store,
            &destination,
            base.manifest_id,
            report.contribution.head,
            None,
        )
        .unwrap();
        assert_eq!(fs::read(destination.join(second_path)).unwrap(), b"new");
        let retry = apply_contribution(
            report.contribution_id,
            &mut store,
            &destination,
            base.manifest_id,
            report.contribution.head,
            None,
        )
        .unwrap();
        assert!(retry.applied.is_empty());
        assert_eq!(retry.already_applied.len(), 2);
    }
}
