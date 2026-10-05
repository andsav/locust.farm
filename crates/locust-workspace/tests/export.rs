//! Export from real Git repositories, and the round trip back to files.

mod support;

use std::collections::BTreeMap;
use std::fs;

use locust_proto::crypto::content_hash;
use locust_proto::manifest::Manifest;
use locust_proto::seal::MAX_PLAINTEXT_BYTES;
use locust_workspace::{ExportError, export, materialize};
use support::{File, MemBlobs, TestRepo, read_tree};

#[test]
fn export_and_materialize_reproduce_files_and_modes() {
    let repo = TestRepo::new();
    let files: [File; 6] = [
        ("README.md", b"# demo\n", false),
        ("bin/run.sh", b"#!/bin/sh\necho hi\n", true),
        (
            "data/raw.bin",
            b"\0\x01\xff\r\n\r\nno conversion\r\n",
            false,
        ),
        ("empty.txt", b"", false),
        ("src/lib.rs", b"pub fn f() {}\n", false),
        ("src/nested/deep/mod.rs", b"// deep\n", false),
    ];
    for (path, contents, executable) in files {
        if executable {
            repo.write_executable(path, contents);
        } else {
            repo.write(path, contents);
        }
    }
    let commit = repo.commit_all();

    let mut store = MemBlobs::default();
    let report = export(repo.path(), "HEAD", &mut store).unwrap();

    assert_eq!(report.commit, commit);
    assert_eq!(report.files, files.len());
    let total: usize = files.iter().map(|(_, contents, _)| contents.len()).sum();
    assert_eq!(report.total_bytes, total as u64);
    assert!(report.left_out.is_empty() && report.refused.is_empty());
    let listed: Vec<_> = report
        .manifest
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry.size, entry.executable))
        .collect();
    let expected: Vec<_> = files
        .iter()
        .map(|&(path, contents, executable)| (path, contents.len() as u64, executable))
        .collect();
    assert_eq!(listed, expected);

    let stored = &store.objects[&report.manifest_id];
    assert_eq!(report.manifest_id, content_hash(stored));
    assert_eq!(Manifest::decode(stored).unwrap(), report.manifest);

    let out = tempfile::tempdir().unwrap();
    let destination = out.path().join("workspace");
    materialize(&report.manifest, &mut store, &destination).unwrap();
    let expected: BTreeMap<_, _> = files
        .iter()
        .map(|&(path, contents, executable)| (path.to_owned(), (contents.to_vec(), executable)))
        .collect();
    assert_eq!(read_tree(&destination), expected);
}

#[test]
fn uncommitted_and_untracked_files_are_not_exported() {
    let repo = TestRepo::new();
    repo.write("tracked.txt", b"committed\n");
    repo.commit_all();
    repo.write("tracked.txt", b"edited but not committed\n");
    repo.write("staged.txt", b"staged only\n");
    repo.git(["add", "staged.txt"]);
    repo.write("untracked.txt", b"never added\n");

    let mut store = MemBlobs::default();
    let report = export(repo.path(), "HEAD", &mut store).unwrap();

    let paths: Vec<_> = report
        .manifest
        .entries
        .iter()
        .map(|e| e.path.as_str())
        .collect();
    assert_eq!(paths, ["tracked.txt"]);
    let content = &report.manifest.entries[0].content;
    assert_eq!(store.objects[content], b"committed\n");
}

#[test]
fn an_export_root_below_the_top_takes_only_its_files_relative_to_it() {
    let repo = TestRepo::new();
    repo.write("app/Cargo.toml", b"[package]\n");
    repo.write("app/src/main.rs", b"fn main() {}\n");
    repo.write("other/notes.txt", b"not exported\n");
    repo.write("top.txt", b"not exported\n");
    repo.commit_all();

    let mut store = MemBlobs::default();
    let report = export(&repo.path().join("app"), "HEAD", &mut store).unwrap();
    let paths: Vec<_> = report
        .manifest
        .entries
        .iter()
        .map(|e| e.path.as_str())
        .collect();
    assert_eq!(paths, ["Cargo.toml", "src/main.rs"]);

    let report = export(&repo.path().join("app/src"), "HEAD", &mut store).unwrap();
    let paths: Vec<_> = report
        .manifest
        .entries
        .iter()
        .map(|e| e.path.as_str())
        .collect();
    assert_eq!(paths, ["main.rs"]);

    // A directory the commit does not contain cannot be exported from it.
    fs::create_dir(repo.path().join("new")).unwrap();
    let result = export(&repo.path().join("new"), "HEAD", &mut MemBlobs::default());
    assert!(matches!(result, Err(ExportError::Git(_))), "{result:?}");
}

#[test]
fn a_symlink_is_unsupported_content_and_nothing_is_stored() {
    let repo = TestRepo::new();
    repo.write("target.txt", b"target\n");
    std::os::unix::fs::symlink("target.txt", repo.path().join("link")).unwrap();
    repo.commit_all();

    let mut store = MemBlobs::default();
    let error = export(repo.path(), "HEAD", &mut store).unwrap_err();
    assert!(
        matches!(&error, ExportError::Unsupported { path, mode } if path == "link" && mode == "120000"),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "link is a symbolic link; only regular files can be exported"
    );
    assert!(store.objects.is_empty());
}

#[test]
fn a_submodule_entry_is_unsupported_content_and_nothing_is_stored() {
    let repo = TestRepo::new();
    repo.write("README.md", b"readme\n");
    let commit = repo.commit_all();
    repo.add_index_entry("160000", &commit, b"vendor/lib");
    repo.commit();

    let mut store = MemBlobs::default();
    let error = export(repo.path(), "HEAD", &mut store).unwrap_err();
    assert!(
        matches!(&error, ExportError::Unsupported { path, mode } if path == "vendor/lib" && mode == "160000"),
        "{error:?}"
    );
    assert!(store.objects.is_empty());
}

#[test]
fn credentials_and_private_material_are_left_out_and_reported() {
    let repo = TestRepo::new();
    let denied = [
        ".aws/credentials",
        ".env",
        ".env.local",
        ".npmrc",
        "certs/tls.KEY",
        "config/server.pem",
        "home/.gnupg/pubring.kbx",
        "home/.ssh/config",
        "id_rsa",
        "web/.netrc",
    ];
    for path in denied {
        repo.write(path, format!("SECRET in {path}\n").as_bytes());
    }
    repo.write("src/keys.rs", b"// handles keys, holds none\n");
    repo.write("src/main.rs", b"fn main() {}\n");
    repo.commit_all();

    let mut store = MemBlobs::default();
    let report = export(repo.path(), "HEAD", &mut store).unwrap();

    let paths: Vec<_> = report
        .manifest
        .entries
        .iter()
        .map(|e| e.path.as_str())
        .collect();
    assert_eq!(paths, ["src/keys.rs", "src/main.rs"]);
    let mut left_out = report.left_out.clone();
    left_out.sort();
    assert_eq!(left_out, denied);
    assert!(report.refused.is_empty());
    assert!(
        store
            .objects
            .values()
            .all(|bytes| !bytes.windows(6).any(|window| window == b"SECRET"))
    );
}

#[test]
fn denied_repository_ancestors_are_kept_when_export_paths_are_rebased() {
    // Separate repositories also exercise ASCII-case variants on macOS's
    // case-insensitive filesystem.
    for (aws, kube) in [(".aws", ".kube"), (".AWS", ".KuBe")] {
        let repo = TestRepo::new();
        repo.write(&format!("{aws}/credentials"), b"FAKE_AWS_SECRET");
        repo.write(&format!("{aws}/nested/credentials"), b"FAKE_NESTED_SECRET");
        repo.write(&format!("{kube}/config"), b"FAKE_KUBE_SECRET");
        repo.write(&format!("app/{aws}/credentials"), b"FAKE_APP_SECRET");
        repo.write("app/src/main.rs", b"fn main() {}\n");
        repo.commit_all();

        for (root, excluded) in [
            (aws.to_owned(), vec!["credentials", "nested/credentials"]),
            (format!("{aws}/nested"), vec!["credentials"]),
            (kube.to_owned(), vec!["config"]),
            (format!("app/{aws}"), vec!["credentials"]),
        ] {
            let mut store = MemBlobs::default();
            let report = export(&repo.path().join(&root), "HEAD", &mut store).unwrap();
            assert!(report.manifest.entries.is_empty(), "{root}");
            assert_eq!(report.files, 0, "{root}");
            assert_eq!(report.total_bytes, 0, "{root}");
            assert_eq!(report.left_out, excluded, "{root}");
            assert!(report.refused.is_empty(), "{root}");
            assert_eq!(store.objects.len(), 1, "only the empty manifest: {root}");
            assert_eq!(
                Manifest::decode(&store.objects[&report.manifest_id]).unwrap(),
                report.manifest
            );
        }

        let mut store = MemBlobs::default();
        let report = export(&repo.path().join("app"), "HEAD", &mut store).unwrap();
        assert_eq!(report.manifest.entries.len(), 1);
        assert_eq!(report.manifest.entries[0].path, "src/main.rs");
        assert_eq!(report.left_out, [format!("{aws}/credentials")]);
        assert!(
            store
                .objects
                .values()
                .all(|bytes| { !bytes.windows(4).any(|window| window == b"FAKE") })
        );
    }
}

#[test]
fn paths_a_manifest_cannot_carry_are_refused_and_reported() {
    let repo = TestRepo::new();
    let blob = repo.hash_object(b"content\n");
    for path in [
        b"ok.txt".as_slice(),
        b"back\\slash.txt",
        b"control\x01char.txt",
        b"not-utf8-\xff.txt",
    ] {
        repo.add_index_entry("100644", &blob, path);
    }
    repo.commit();

    let mut store = MemBlobs::default();
    let report = export(repo.path(), "HEAD", &mut store).unwrap();

    let paths: Vec<_> = report
        .manifest
        .entries
        .iter()
        .map(|e| e.path.as_str())
        .collect();
    assert_eq!(paths, ["ok.txt"]);
    let mut refused = report.refused.clone();
    refused.sort();
    assert_eq!(
        refused,
        [
            "back\\slash.txt",
            "control\x01char.txt",
            "not-utf8-\u{fffd}.txt"
        ]
    );
}

#[test]
fn a_file_over_the_object_limit_fails_before_anything_is_stored() {
    let repo = TestRepo::new();
    repo.write("small.txt", b"small\n");
    repo.git(["add", "small.txt"]);
    let big = repo.hash_object(&vec![0; MAX_PLAINTEXT_BYTES + 1]);
    repo.add_index_entry("100644", &big, b"big.bin");
    repo.commit();

    let mut store = MemBlobs::default();
    let error = export(repo.path(), "HEAD", &mut store).unwrap_err();
    assert!(
        matches!(&error, ExportError::TooLarge { path, size } if path == "big.bin" && *size == MAX_PLAINTEXT_BYTES as u64 + 1),
        "{error:?}"
    );
    assert!(store.objects.is_empty());
}

#[test]
fn roots_outside_a_work_tree_and_unknown_commits_are_refused() {
    let repo = TestRepo::new();
    repo.write("a.txt", b"a\n");
    repo.commit_all();
    let mut store = MemBlobs::default();

    let result = export(&repo.path().join(".git"), "HEAD", &mut store);
    assert!(
        matches!(result, Err(ExportError::NotInWorkTree)),
        "{result:?}"
    );

    let plain = tempfile::tempdir().unwrap();
    let result = export(plain.path(), "HEAD", &mut store);
    assert!(matches!(result, Err(ExportError::Git(_))), "{result:?}");

    for name in ["no-such-branch", "HEAD~5", "", "--output=written-by-git"] {
        let result = export(repo.path(), name, &mut store);
        assert!(
            matches!(result, Err(ExportError::Git(_))),
            "{name:?}: {result:?}"
        );
    }
    assert!(!repo.path().join("written-by-git").exists());
    assert!(store.objects.is_empty());
}

#[test]
fn reserved_workspace_metadata_never_reaches_the_sink_including_ancestors() {
    let repo = TestRepo::new();
    repo.write("src/main.rs", b"fn main() {}\n");
    for path in [
        ".locust/plan.json",
        ".locust-apply-123/original-0",
        ".locust-workspace-456/manifest.json",
        ".locust-recovery-789/completed.json",
        "sub/.locust-apply-111/replacement-0",
        "sub/.locust-recovery-222/phase-0",
    ] {
        repo.write(path, b"RESERVED METADATA CONTENT\n");
    }
    repo.commit_all();

    let mut store = MemBlobs::default();
    let report = export(repo.path(), "HEAD", &mut store).unwrap();
    assert_eq!(
        report
            .manifest
            .entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>(),
        ["src/main.rs"]
    );
    let mut left_out = report.left_out.clone();
    left_out.sort();
    assert_eq!(
        left_out,
        [
            ".locust-apply-123/original-0",
            ".locust-recovery-789/completed.json",
            ".locust-workspace-456/manifest.json",
            ".locust/plan.json",
            "sub/.locust-apply-111/replacement-0",
            "sub/.locust-recovery-222/phase-0",
        ]
    );
    assert!(
        store
            .objects
            .values()
            .all(|bytes| { !bytes.windows(8).any(|window| window == b"RESERVED") }),
        "reserved metadata bytes reached the sink"
    );

    let mut store = MemBlobs::default();
    let report = export(
        &repo.path().join("sub").join(".locust-apply-111"),
        "HEAD",
        &mut store,
    )
    .unwrap();
    assert!(report.manifest.entries.is_empty());
    assert_eq!(report.left_out, ["replacement-0"]);
    assert_eq!(store.objects.len(), 1, "only the empty manifest stored");
}
