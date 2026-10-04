//! Creation reads either a named committed tree or an explicit list of regular
//! files. It never stages, commits, scans untracked files or runs Git filters.

use std::collections::BTreeMap;
use std::path::Path;

use locust_proto::id::BlobHash;
use locust_proto::manifest::{Entry, Manifest};

use crate::contribution::{self, check_path, file_bytes, invalid};
use crate::{BlobStore, ContributionError, ContributionReport, export, safe_fs};

/// Compares a committed snapshot to `base`, storing a canonical contribution.
/// Uncommitted and untracked local files are never read by this operation.
pub fn create_committed(
    root: &Path,
    commit: &str,
    base: BlobHash,
    store: &mut dyn BlobStore,
) -> Result<ContributionReport, ContributionError> {
    let original = contribution::manifest(store, base)?;
    let report = export(root, commit, store)?;
    if !report.refused.is_empty() {
        return Err(invalid(
            "committed snapshot contains refused paths; review the export before contributing",
        ));
    }
    let mut head = report.manifest;
    // An unchanged file retains its original sealed identifier even when the
    // local sink uses a newer content-key epoch.
    for entry in &mut head.entries {
        if let Ok(index) = original
            .entries
            .binary_search_by(|old| old.path.cmp(&entry.path))
        {
            let old = &original.entries[index];
            if old.size == entry.size && file_bytes(store, old)? == file_bytes(store, entry)? {
                entry.content = old.content;
            }
        }
    }
    contribution::finish(
        base,
        &original,
        head,
        Some(report.commit),
        report.left_out,
        store,
    )
}

/// Captures only the exact relative paths supplied by the caller. A missing
/// selected base file is a deletion. Directories are not recursive selections;
/// the caller must name each added, edited or deleted file for review.
/// Works in a materialized snapshot without any Git repository.
pub fn create_selected(
    root: &Path,
    paths: &[String],
    base: BlobHash,
    store: &mut dyn BlobStore,
) -> Result<ContributionReport, ContributionError> {
    let original = contribution::manifest(store, base)?;
    let directory = safe_fs::root(root)?;
    let mut head: BTreeMap<_, _> = original
        .entries
        .iter()
        .map(|e| (e.path.clone(), e.clone()))
        .collect();
    let mut selected = paths.to_vec();
    selected.sort();
    if selected.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid("selected paths must be unique"));
    }
    let mut captured = Vec::new();
    for path in &selected {
        check_path(path)?;
        let local = read_selected(&directory, path, &selected, &original)?;
        if local.is_none() && !head.contains_key(path) {
            return Err(invalid(format!(
                "selected path {path:?} is absent from both snapshot and local files"
            )));
        }
        captured.push((path.clone(), local));
    }
    // Review scope is stable before sending any selected bytes to the sink.
    for (path, first) in &captured {
        let now = read_selected(&directory, path, &selected, &original)?;
        if first.as_ref().map(|f| &f.identity) != now.as_ref().map(|f| &f.identity) {
            return Err(ContributionError::Conflict {
                path: path.clone(),
                reason: "selected file changed during capture".into(),
            });
        }
    }
    for (path, local) in captured {
        if let Some(local) = local {
            let content = if let Some(old) = head.get(&path) {
                if old.size == local.bytes.len() as u64 && file_bytes(store, old)? == local.bytes {
                    old.content
                } else {
                    store.store(&local.bytes)?
                }
            } else {
                store.store(&local.bytes)?
            };
            head.insert(
                path.clone(),
                Entry {
                    path,
                    executable: local.executable,
                    size: local.bytes.len() as u64,
                    content,
                },
            );
        } else {
            head.remove(&path);
        }
    }
    let head = Manifest {
        entries: head.into_values().collect(),
    };
    contribution::finish(base, &original, head, None, Vec::new(), store)
}

fn read_selected(
    root: &std::fs::File,
    path: &str,
    selected: &[String],
    base: &Manifest,
) -> Result<Option<safe_fs::LocalFile>, ContributionError> {
    match safe_fs::read(root, path) {
        Err(error @ ContributionError::Unsupported { .. })
            if base.entries.iter().any(|e| e.path == path) =>
        {
            if selected
                .iter()
                .any(|child| child.starts_with(&format!("{path}/")))
                && safe_fs::descendants(root, path).is_ok()
            {
                return Ok(None);
            }
            for ancestor in selected
                .iter()
                .filter(|ancestor| path.starts_with(&format!("{ancestor}/")))
            {
                if safe_fs::read(root, ancestor)?.is_some() {
                    return Ok(None);
                }
            }
            Err(error)
        }
        result => result,
    }
}
