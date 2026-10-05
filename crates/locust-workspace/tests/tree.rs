mod support;

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::os::unix::fs::{PermissionsExt, symlink};

use locust_proto::crypto::content_hash;
use locust_proto::id::BlobHash;
use locust_proto::manifest::{Entry, Manifest};
use locust_workspace::{
    BlobSink, BlobSource, CaptureMode, FileValue, WorkspaceError, capture_seed, capture_tree,
    compose_trees, diff_trees, inspect_local_tree, inspect_tree, materialize, parse_paths,
    plan_update, three_way_tree,
};
use support::{MemBlobs, manifest_of, read_tree};

fn stored(store: &mut MemBlobs, files: &[support::File<'_>]) -> (BlobHash, Manifest) {
    let tree = manifest_of(store, files);
    (store.store(&tree.encode().unwrap()).unwrap(), tree)
}

fn file(bytes: &[u8], executable: bool) -> FileValue {
    FileValue {
        bytes: bytes.to_vec(),
        executable,
    }
}

fn map(files: &[(&str, &[u8], bool)]) -> BTreeMap<String, FileValue> {
    files
        .iter()
        .map(|(path, bytes, executable)| (path.to_string(), file(bytes, *executable)))
        .collect()
}

#[test]
fn exact_utf8_path_lists_preserve_spaces_and_refuse_ambiguous_scope() {
    assert_eq!(
        parse_paths(b"z file\na file\n").unwrap(),
        ["a file", "z file"]
    );
    assert_eq!(parse_paths(b"a").unwrap(), ["a"]);
    assert!(parse_paths(b"").unwrap().is_empty());
    assert_eq!(
        parse_paths("日本語.txt\n".as_bytes()).unwrap(),
        ["日本語.txt"]
    );
    for paths in [
        b"a\na\n".as_slice(),
        b"a\n\n",
        b"\n",
        b"a\r\n",
        b"a\xff",
        b"/absolute\n",
        b"../outside\n",
        b"directory/\n",
        b".git/config\n",
        b".env\n",
        b".locust/journal\n",
        b".LOCUST-APPLY-journal/original\n",
        b".locust-recovery-local/original\n",
        b"a\0b\n",
    ] {
        assert!(parse_paths(paths).is_err(), "{paths:?}");
    }
}

#[test]
fn ordinary_selected_and_empty_seeds_require_no_git_and_never_recurse() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir(directory.path().join("src")).unwrap();
    fs::write(directory.path().join("src/lib.rs"), b"selected").unwrap();
    fs::write(directory.path().join("private"), b"not selected").unwrap();
    let mut store = MemBlobs::default();
    let seed = capture_seed(
        directory.path(),
        &parse_paths(b"src/lib.rs\n").unwrap(),
        false,
        &mut store,
    )
    .unwrap();
    assert_eq!(seed.manifest.entries.len(), 1);
    assert_eq!(seed.captured_paths, ["src/lib.rs"]);
    let destination = directory.path().join("copy");
    materialize(&seed.manifest, &mut store, &destination).unwrap();
    assert_eq!(
        fs::read(destination.join("src/lib.rs")).unwrap(),
        b"selected"
    );
    assert!(!destination.join("private").exists());
    assert!(!directory.path().join(".git").exists());
    assert!(capture_seed(directory.path(), &["src".into()], false, &mut store).is_err());
    assert!(capture_seed(directory.path(), &[], false, &mut store).is_err());
    assert!(capture_seed(directory.path(), &["private".into()], true, &mut store).is_err());
    let empty = capture_seed(directory.path(), &[], true, &mut store).unwrap();
    assert!(empty.manifest.entries.is_empty());
    assert_eq!(
        Manifest::decode(store.objects.get(&empty.manifest_id).unwrap()).unwrap(),
        Manifest::default()
    );
}

#[test]
fn default_capture_includes_managed_changes_and_only_selected_additions() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("checkout");
    let mut store = MemBlobs::default();
    let (base_id, base) = stored(
        &mut store,
        &[
            ("edit", b"old", false),
            ("delete", b"gone", false),
            ("mode", b"same", false),
        ],
    );
    materialize(&base, &mut store, &root).unwrap();
    fs::write(root.join("edit"), b"new").unwrap();
    fs::remove_file(root.join("delete")).unwrap();
    fs::set_permissions(root.join("mode"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(root.join("add"), b"\0binary").unwrap();
    fs::write(root.join("unselected"), b"private").unwrap();
    fs::write(root.join(".env"), b"credential").unwrap();
    let result = capture_tree(
        &root,
        base_id,
        &["add".into()],
        CaptureMode::ManagedAndSelected,
        &mut store,
    )
    .unwrap();
    assert_eq!(result.captured_paths, ["add", "delete", "edit", "mode"]);
    assert_eq!(result.changes.len(), 4);
    assert_eq!(
        result
            .manifest
            .entries
            .iter()
            .find(|entry| entry.path == "mode")
            .unwrap()
            .content,
        base.entries
            .iter()
            .find(|entry| entry.path == "mode")
            .unwrap()
            .content,
        "mode-only edits reuse the unchanged immutable byte object"
    );
    assert!(
        !result
            .manifest
            .entries
            .iter()
            .any(|entry| entry.path == "unselected" || entry.path == ".env")
    );
    let subset = capture_tree(
        &root,
        base_id,
        &["edit".into()],
        CaptureMode::Only,
        &mut store,
    )
    .unwrap();
    assert_eq!(subset.changes.len(), 1);
    assert!(
        subset
            .manifest
            .entries
            .iter()
            .any(|entry| entry.path == "delete")
    );
    assert!(
        !subset
            .manifest
            .entries
            .iter()
            .find(|entry| entry.path == "mode")
            .unwrap()
            .executable
    );
    assert!(capture_tree(&root, base_id, &[], CaptureMode::Only, &mut store).is_err());
    assert!(
        capture_tree(
            &root,
            base_id,
            &[".env".into()],
            CaptureMode::Only,
            &mut store
        )
        .is_err()
    );
}

#[test]
fn capture_keeps_one_frozen_candidate_even_when_sink_edits_live_files() {
    struct MutatingStore {
        store: MemBlobs,
        path: std::path::PathBuf,
        mutated: bool,
    }
    impl BlobSource for MutatingStore {
        fn fetch(&mut self, id: &BlobHash) -> io::Result<Option<Vec<u8>>> {
            self.store.fetch(id)
        }
    }
    impl BlobSink for MutatingStore {
        fn store(&mut self, bytes: &[u8]) -> io::Result<BlobHash> {
            if !self.mutated {
                fs::write(&self.path, b"after freeze")?;
                self.mutated = true;
            }
            self.store.store(bytes)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("selected");
    fs::write(&path, b"frozen").unwrap();
    let mut store = MutatingStore {
        store: MemBlobs::default(),
        path,
        mutated: false,
    };
    let candidate = capture_seed(root.path(), &["selected".into()], false, &mut store).unwrap();
    assert_eq!(
        candidate.changes[0].after.as_ref().unwrap().bytes,
        b"frozen"
    );
    assert_eq!(
        store
            .fetch(&candidate.manifest.entries[0].content)
            .unwrap()
            .unwrap(),
        b"frozen"
    );
    fs::write(root.path().join("selected"), b"after preview").unwrap();
    assert_eq!(
        store
            .fetch(&candidate.manifest.entries[0].content)
            .unwrap()
            .unwrap(),
        b"frozen"
    );
}

#[test]
fn selected_capture_refuses_symlinks_hardlinks_reserved_and_missing_paths() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("file"), b"bytes").unwrap();
    symlink(root.path().join("file"), root.path().join("link")).unwrap();
    fs::hard_link(root.path().join("file"), root.path().join("hard")).unwrap();
    fs::create_dir(root.path().join(".locust")).unwrap();
    fs::write(root.path().join(".locust/journal"), b"metadata").unwrap();
    let mut store = MemBlobs::default();
    for path in ["link", "hard", "file", ".locust/journal", "missing"] {
        assert!(
            capture_seed(root.path(), &[path.into()], false, &mut store).is_err(),
            "{path}"
        );
    }
    assert!(store.objects.is_empty());
}

#[test]
fn path_algebra_covers_binary_modes_add_delete_identical_and_conflicts() {
    let base = map(&[
        ("edit", b"old", false),
        ("delete", b"gone", false),
        ("mode", b"bytes", false),
    ]);
    let proposed = map(&[
        ("edit", b"new", false),
        ("mode", b"bytes", true),
        ("added", b"\0\xff", false),
    ]);
    let mut current = base.clone();
    current.insert("other".into(), file(b"concurrent", false));
    let combined = three_way_tree(&base, &proposed, &current).unwrap();
    assert_eq!(combined.get("edit").unwrap().bytes, b"new");
    assert!(combined.get("mode").unwrap().executable);
    assert!(!combined.contains_key("delete"));
    assert_eq!(combined.get("other").unwrap().bytes, b"concurrent");
    assert_eq!(
        three_way_tree(&base, &proposed, &combined).unwrap(),
        combined
    );
    let empty = BTreeMap::new();
    let first = map(&[("same", b"first", false)]);
    let second = map(&[("same", b"second", false)]);
    assert!(three_way_tree(&empty, &first, &second).is_err());
    assert_eq!(three_way_tree(&empty, &first, &first).unwrap(), first);
    assert!(three_way_tree(&first, &empty, &second).is_err());
    assert_eq!(three_way_tree(&first, &empty, &empty).unwrap(), empty);
    assert!(three_way_tree(&first, &map(&[("same", b"first", true)]), &second).is_err());
    assert!(
        three_way_tree(
            &empty,
            &map(&[("node", b"file", false)]),
            &map(&[("node/child", b"child", false)])
        )
        .is_err()
    );
}

#[test]
fn composition_compares_plaintext_across_epochs_and_reuses_current_objects() {
    let mut store = MemBlobs::default();
    let (base_id, base) = stored(
        &mut store,
        &[("same", b"unchanged", false), ("edit", b"old", false)],
    );
    let (proposed_id, _) = stored(
        &mut store,
        &[("same", b"unchanged", false), ("edit", b"new", false)],
    );
    // Simulate different authenticated sealed IDs for the same plaintext.
    let mut current = base;
    for entry in &mut current.entries {
        let original = store.objects[&entry.content].clone();
        let mut epoch_bytes = b"new epoch".to_vec();
        epoch_bytes.extend_from_slice(&original);
        entry.content = content_hash(&epoch_bytes);
        store.objects.insert(entry.content, original);
    }
    let current_id = store.store(&current.encode().unwrap()).unwrap();
    assert!(
        diff_trees(base_id, current_id, &mut store)
            .unwrap()
            .is_empty()
    );
    let result = compose_trees(base_id, proposed_id, current_id, &mut store).unwrap();
    assert!(!result.already_included);
    assert_eq!(result.changes.len(), 1);
    assert_eq!(
        result
            .manifest
            .entries
            .iter()
            .find(|entry| entry.path == "same")
            .unwrap()
            .content,
        current
            .entries
            .iter()
            .find(|entry| entry.path == "same")
            .unwrap()
            .content
    );
    let count = store.objects.len();
    let included = compose_trees(base_id, proposed_id, result.manifest_id, &mut store).unwrap();
    assert!(included.already_included);
    assert_eq!(included.manifest_id, result.manifest_id);
    assert_eq!(store.objects.len(), count);
}

#[test]
fn managed_file_directory_transitions_capture_only_explicit_new_files() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("checkout");
    let mut store = MemBlobs::default();
    let (base_id, base) = stored(&mut store, &[("node", b"old file", false)]);
    materialize(&base, &mut store, &root).unwrap();
    fs::remove_file(root.join("node")).unwrap();
    fs::create_dir(root.join("node")).unwrap();
    fs::write(root.join("node/selected"), b"child").unwrap();
    fs::write(root.join("node/unselected"), b"private").unwrap();
    let deletion = capture_tree(
        &root,
        base_id,
        &[],
        CaptureMode::ManagedAndSelected,
        &mut store,
    )
    .unwrap();
    assert!(deletion.manifest.entries.is_empty());
    let result = capture_tree(
        &root,
        base_id,
        &["node/selected".into()],
        CaptureMode::ManagedAndSelected,
        &mut store,
    )
    .unwrap();
    assert_eq!(result.manifest.entries.len(), 1);
    assert_eq!(result.manifest.entries[0].path, "node/selected");
    assert!(
        capture_tree(
            &root,
            base_id,
            &["node".into(), "node/selected".into()],
            CaptureMode::Only,
            &mut store
        )
        .is_err()
    );

    fs::remove_file(root.join("node/selected")).unwrap();
    fs::remove_file(root.join("node/unselected")).unwrap();
    fs::remove_dir(root.join("node")).unwrap();
    fs::write(root.join("node"), b"replacement").unwrap();
    let result = capture_tree(
        &root,
        result.manifest_id,
        &["node".into(), "node/selected".into()],
        CaptureMode::Only,
        &mut store,
    )
    .unwrap();
    assert_eq!(result.manifest.entries.len(), 1);
    assert_eq!(result.manifest.entries[0].path, "node");
}

#[test]
fn exact_tree_review_reads_a_full_replacement_without_loading_broken_history() {
    let mut store = MemBlobs::default();
    let (replacement_id, _) = stored(&mut store, &[("restored", b"good", true)]);
    let broken_id = BlobHash([0xff; 32]);
    assert!(diff_trees(broken_id, replacement_id, &mut store).is_err());
    let count = store.objects.len();
    let (manifest, files) = inspect_tree(replacement_id, &mut store).unwrap();
    assert_eq!(manifest.entries.len(), 1);
    assert_eq!(files["restored"], file(b"good", true));
    assert_eq!(store.objects.len(), count);
}

#[test]
fn tree_operations_work_with_git_unavailable_on_path() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "git_free_tree_loop_child"])
        .env("PATH", "/locust-no-programs-on-path")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn git_free_tree_loop_child() {
    let temp = tempfile::tempdir().unwrap();
    let seed_root = temp.path().join("seed");
    fs::create_dir(&seed_root).unwrap();
    fs::write(seed_root.join("a"), b"base").unwrap();
    fs::write(seed_root.join("b"), b"base").unwrap();
    let mut store = MemBlobs::default();
    let seed = capture_seed(
        &seed_root,
        &parse_paths(b"a\nb\n").unwrap(),
        false,
        &mut store,
    )
    .unwrap();
    let alice = temp.path().join("alice");
    let bob = temp.path().join("bob");
    materialize(&seed.manifest, &mut store, &alice).unwrap();
    materialize(&seed.manifest, &mut store, &bob).unwrap();
    fs::write(alice.join("a"), b"alice").unwrap();
    fs::write(bob.join("b"), b"bob").unwrap();
    let first = capture_tree(
        &alice,
        seed.manifest_id,
        &[],
        CaptureMode::ManagedAndSelected,
        &mut store,
    )
    .unwrap();
    let second = capture_tree(
        &bob,
        seed.manifest_id,
        &[],
        CaptureMode::ManagedAndSelected,
        &mut store,
    )
    .unwrap();
    let result = compose_trees(
        seed.manifest_id,
        second.manifest_id,
        first.manifest_id,
        &mut store,
    )
    .unwrap();
    let local = plan_update(&bob, seed.manifest_id, result.manifest_id, &mut store).unwrap();
    assert_eq!(local.changes.len(), 1);
    assert_eq!(local.changes[0].path, "a");
    assert!(local.dirty_paths.is_empty());
    let (manifest, files) = inspect_tree(result.manifest_id, &mut store).unwrap();
    assert_eq!(files["a"].bytes, b"alice");
    assert_eq!(files["b"].bytes, b"bob");
    materialize(&manifest, &mut store, &temp.path().join("accepted")).unwrap();
    assert!(!alice.join(".git").exists());
    assert!(!bob.join(".git").exists());
}

#[test]
fn composition_authenticates_all_file_sizes_even_for_unchanged_paths() {
    let mut store = MemBlobs::default();
    let (base_id, mut bad) = stored(&mut store, &[("file", b"old", false)]);
    bad.entries[0].size += 1;
    let bad_id = store.store(&bad.encode().unwrap()).unwrap();
    assert!(matches!(
        compose_trees(base_id, base_id, bad_id, &mut store),
        Err(WorkspaceError::WrongSize { .. })
    ));
    store.objects.remove(&bad.entries[0].content);
    assert!(matches!(
        compose_trees(base_id, base_id, base_id, &mut store),
        Err(WorkspaceError::MissingObject(_))
    ));
}

#[test]
fn update_receives_other_paths_preserves_compatible_dirty_work_and_adopts_equal_additions() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("checkout");
    let mut store = MemBlobs::default();
    let (base_id, base) = stored(
        &mut store,
        &[
            ("docs", b"old", false),
            ("parser", b"old", false),
            ("local", b"old", false),
        ],
    );
    materialize(&base, &mut store, &root).unwrap();
    fs::write(root.join("docs"), b"integrated").unwrap();
    fs::write(root.join("local"), b"unpublished").unwrap();
    fs::write(root.join("added"), b"equal").unwrap();
    fs::write(root.join(".env"), b"private").unwrap();
    fs::create_dir(root.join("empty-untracked")).unwrap();
    let (target_id, _) = stored(
        &mut store,
        &[
            ("docs", b"integrated", false),
            ("parser", b"incoming", false),
            ("local", b"old", false),
            ("added", b"equal", false),
        ],
    );
    let original = read_tree(&root);
    let observed = inspect_local_tree(&root).unwrap();
    assert_eq!(observed.files[".env"].digest, content_hash(b"private"));
    assert_eq!(observed.directories, ["empty-untracked"]);
    let plan = plan_update(&root, base_id, target_id, &mut store).unwrap();
    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.changes[0].path, "parser");
    assert_eq!(plan.adopted_paths, ["added"]);
    assert_eq!(plan.dirty_paths, ["local"]);
    assert_eq!(plan.untracked_paths, [".env"]);
    assert_eq!(
        plan.final_files["local"].digest,
        content_hash(b"unpublished")
    );
    assert_eq!(plan.final_files[".env"].digest, content_hash(b"private"));
    assert_eq!(plan.final_directories, ["empty-untracked"]);
    assert_eq!(read_tree(&root), original);
    assert_eq!(
        fs::read_dir(temp.path()).unwrap().count(),
        1,
        "layout probe is removed"
    );
}

#[test]
fn update_refuses_conflicts_and_untracked_descendants_without_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("checkout");
    let mut store = MemBlobs::default();
    let (base_id, base) = stored(
        &mut store,
        &[("edit", b"base", false), ("dir/managed", b"base", false)],
    );
    materialize(&base, &mut store, &root).unwrap();
    fs::write(root.join("edit"), b"local").unwrap();
    let (target_id, _) = stored(
        &mut store,
        &[("edit", b"remote", false), ("dir/managed", b"base", false)],
    );
    let before = read_tree(&root);
    assert!(plan_update(&root, base_id, target_id, &mut store).is_err());
    assert_eq!(read_tree(&root), before);
    fs::write(root.join("edit"), b"base").unwrap();
    fs::write(root.join("dir/untracked"), b"keep").unwrap();
    let (target_id, _) = stored(
        &mut store,
        &[("edit", b"base", false), ("dir", b"replacement", false)],
    );
    let before = read_tree(&root);
    assert!(plan_update(&root, base_id, target_id, &mut store).is_err());
    assert_eq!(read_tree(&root), before);
    fs::remove_file(root.join("dir/untracked")).unwrap();
    let plan = plan_update(&root, base_id, target_id, &mut store).unwrap();
    assert_eq!(
        plan.changes
            .iter()
            .map(|change| change.path.as_str())
            .collect::<Vec<_>>(),
        ["dir", "dir/managed"]
    );
    assert!(!plan.final_directories.contains(&"dir".into()));
}

#[test]
fn update_checks_complete_target_case_and_unicode_layout_on_actual_filesystem() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("checkout");
    fs::create_dir(&root).unwrap();
    let mut store = MemBlobs::default();
    let (base_id, _) = stored(&mut store, &[]);
    for (first, alias) in [("a", "A"), ("é", "e\u{301}")] {
        // Detect this filesystem's semantics without assuming APFS/ext4.
        fs::write(root.join(first), b"test").unwrap();
        let aliased = root.join(alias).exists();
        fs::remove_file(root.join(first)).unwrap();
        let tree = Manifest {
            entries: {
                let mut entries = vec![
                    Entry {
                        path: first.into(),
                        executable: false,
                        size: 1,
                        content: store.store(b"1").unwrap(),
                    },
                    Entry {
                        path: alias.into(),
                        executable: false,
                        size: 1,
                        content: store.store(b"2").unwrap(),
                    },
                ];
                entries.sort_by(|a, b| a.path.cmp(&b.path));
                entries
            },
        };
        let target_id = store.store(&tree.encode().unwrap()).unwrap();
        let result = plan_update(&root, base_id, target_id, &mut store);
        assert_eq!(result.is_err(), aliased, "{first:?} / {alias:?}");
        assert!(fs::read_dir(&root).unwrap().next().is_none());
    }
}

#[test]
fn large_untracked_file_is_preserved_and_capture_limit_is_enforced() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("checkout");
    let mut store = MemBlobs::default();
    let (base_id, base) = stored(&mut store, &[("managed", b"base", false)]);
    materialize(&base, &mut store, &root).unwrap();
    // A large sparse untracked file (over the 64 MiB object limit).
    let large_size = locust_proto::seal::MAX_PLAINTEXT_BYTES + 1024;
    let large = root.join("large.bin");
    let file = std::fs::File::create(&large).unwrap();
    file.set_len(large_size as u64).unwrap();
    drop(file);
    // inspect_local_tree and plan_update work despite the large file.
    let observed = inspect_local_tree(&root).unwrap();
    assert!(observed.files.contains_key("large.bin"));
    assert_eq!(observed.files["large.bin"].size, large_size as u64);
    let (target_id, _) = stored(&mut store, &[("managed", b"changed", false)]);
    let plan = plan_update(&root, base_id, target_id, &mut store).unwrap();
    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.changes[0].path, "managed");
    assert!(plan.final_files.contains_key("large.bin"));
    // The large file's bytes are not in the plan's final_files (only digests).
    assert_eq!(plan.final_files["large.bin"].size, large_size as u64);
    // Capturing the large file for publication fails: it exceeds the object limit.
    let result = capture_tree(
        &root,
        base_id,
        &["large.bin".into()],
        CaptureMode::Only,
        &mut store,
    );
    assert!(result.is_err(), "capture should refuse the large file");
}
