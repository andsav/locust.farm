//! Applies only exact reviewed bytes, preserving originals in a local recovery directory.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use locust_proto::id::BlobHash;
use rustix::fs::{self, Mode};
use serde::Serialize;

use crate::contribution::{file_bytes, invalid, load};
use crate::{BlobSource, ContributionError, git, safe_fs};

#[derive(Clone, Debug, Serialize)]
pub struct ApplyReport {
    pub contribution_id: BlobHash,
    pub head: BlobHash,
    pub applied: Vec<String>,
    pub already_applied: Vec<String>,
    /// Original files and the inert plan, retained for explicit local recovery.
    pub recovery_directory: Option<PathBuf>,
}

pub fn apply_contribution(
    id: BlobHash,
    source: &mut dyn BlobSource,
    root: &Path,
    expected_base: BlobHash,
    accepted_head: BlobHash,
    expected_git_head: Option<&str>,
) -> Result<ApplyReport, ContributionError> {
    let (contribution, _, _) = load(id, source)?;
    if contribution.base != expected_base || contribution.head != accepted_head {
        return Err(invalid(
            "contribution does not match the approved base and head",
        ));
    }
    let check_head = || -> Result<(), ContributionError> {
        if let Some(expected) = expected_git_head {
            let actual = git::resolve_commit(root, "HEAD")?;
            if actual != expected {
                return Err(ContributionError::HeadChanged {
                    expected: expected.into(),
                    actual,
                });
            }
        }
        Ok(())
    };
    check_head()?;
    let directory = safe_fs::root(root)?;
    let mut pending = Vec::new();
    let mut remove_directories = Vec::new();
    let mut already_applied = Vec::new();
    for change in &contribution.changes {
        let before = change
            .before
            .as_ref()
            .map(|e| file_bytes(source, e))
            .transpose()?;
        let after = change
            .after
            .as_ref()
            .map(|e| file_bytes(source, e))
            .transpose()?;
        let local = match safe_fs::read(&directory, &change.path) {
            Ok(local) => local,
            Err(error @ ContributionError::Unsupported { .. }) => {
                // A new regular file can replace only a directory whose every
                // regular descendant is explicitly deleted by this delta.
                if change.before.is_none() && change.after.is_some() {
                    if let Ok((files, dirs)) = safe_fs::descendants(&directory, &change.path) {
                        for path in files {
                            if !contribution
                                .changes
                                .iter()
                                .any(|c| c.path == path && c.before.is_some() && c.after.is_none())
                            {
                                return Err(ContributionError::Conflict { path, reason: "unrelated descendant would be removed by directory replacement".into() });
                            }
                        }
                        remove_directories.extend(dirs);
                        None
                    } else if deleted_ancestor(&contribution.changes, &change.path) {
                        None
                    } else {
                        return Err(error);
                    }
                } else if (change.after.is_none()
                    && (already_applied
                        .iter()
                        .any(|parent: &String| change.path.starts_with(&format!("{parent}/")))
                        || safe_fs::descendants(&directory, &change.path).is_ok_and(
                            |(files, _)| {
                                files.iter().all(|path| {
                                    contribution
                                        .changes
                                        .iter()
                                        .any(|c| &c.path == path && c.after.is_some())
                                })
                            },
                        )))
                    || (change.before.is_none()
                        && deleted_ancestor(&contribution.changes, &change.path))
                {
                    None
                } else {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        };
        let matches = |bytes: &Option<Vec<u8>>, executable: Option<bool>| match (
            local.as_ref(),
            bytes.as_ref(),
        ) {
            (None, None) => true,
            (Some(local), Some(bytes)) => {
                local.bytes == *bytes && Some(local.executable) == executable
            }
            _ => false,
        };
        if matches(&after, change.after.as_ref().map(|e| e.executable)) {
            already_applied.push(change.path.clone());
        } else if matches(&before, change.before.as_ref().map(|e| e.executable)) {
            pending.push((change, local, after));
        } else {
            return Err(ContributionError::Conflict {
                path: change.path.clone(),
                reason: "bytes or executable mode differ from both base and approved result".into(),
            });
        }
    }
    // Delete descendants before replacing their parent directory, and delete
    // a regular parent before creating its replacement descendants.
    pending.sort_by(|(a, _, _), (b, _, _)| {
        a.after
            .is_some()
            .cmp(&b.after.is_some())
            .then_with(|| a.path.cmp(&b.path))
    });
    let mut report = ApplyReport {
        contribution_id: id,
        head: accepted_head,
        applied: Vec::new(),
        already_applied,
        recovery_directory: None,
    };
    if pending.is_empty() {
        return Ok(report);
    }
    check_head()?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = loop {
        let name = format!(
            ".locust-apply-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        match fs::mkdirat(&directory, &name, Mode::from_raw_mode(0o700)) {
            Ok(()) => break name,
            Err(e) if e == rustix::io::Errno::EXIST => continue,
            Err(e) => return Err(e.into()),
        }
    };
    let recovery_path = root.join(&name);
    let recovery = safe_fs::dir_at(&directory, &name)?;
    report.recovery_directory = Some(recovery_path.clone());
    let result = (|| -> Result<(), ContributionError> {
        safe_fs::write_new(
            &recovery,
            "contribution",
            &contribution.encode().map_err(|e| invalid(e.to_string()))?,
            false,
        )?;
        let mapping: String = pending
            .iter()
            .enumerate()
            .map(|(index, (change, _, _))| {
                format!("original-{index}\tnew-{index}\t{}\n", change.path)
            })
            .collect();
        safe_fs::write_new(&recovery, "paths.tsv", mapping.as_bytes(), false)?;
        // Probe the actual filesystem's path aliasing rules for additions before
        // changing any workspace file (case folding and Unicode normalization).
        fs::mkdirat(&recovery, "layout", Mode::from_raw_mode(0o700))?;
        let layout = safe_fs::dir_at(&recovery, "layout")?;
        for (change, _, after) in &pending {
            if after.is_some() {
                let (parent, leaf) = safe_fs::parent(&layout, &change.path, true)?;
                safe_fs::write_new(&parent, &leaf, &[], false)?;
            }
        }
        // Stage every replacement before changing any workspace file.
        for (index, (change, _, after)) in pending.iter().enumerate() {
            if let Some(bytes) = after {
                safe_fs::write_new(
                    &recovery,
                    &format!("new-{index}"),
                    bytes,
                    change.after.as_ref().unwrap().executable,
                )?;
            }
        }
        for (index, (change, original, after)) in pending.iter().enumerate() {
            check_head()?;
            if change.after.is_some() {
                for path in remove_directories.iter().filter(|path| {
                    *path == &change.path || path.starts_with(&format!("{}/", change.path))
                }) {
                    safe_fs::remove_empty_directory(&directory, path)?;
                }
            }
            let current = safe_fs::read(&directory, &change.path)?;
            if current.as_ref().map(|f| &f.identity) != original.as_ref().map(|f| &f.identity) {
                return Err(ContributionError::Conflict {
                    path: change.path.clone(),
                    reason: "file changed after preflight".into(),
                });
            }
            let (parent, leaf) = safe_fs::parent(&directory, &change.path, after.is_some())?;
            if original.is_some() {
                let backup = format!("original-{index}");
                safe_fs::rename_new(&parent, &leaf, &recovery, &backup)?;
                let moved = safe_fs::read_at(&recovery, &backup, &change.path)?;
                if moved.as_ref().map(|f| (&f.bytes, f.executable))
                    != original.as_ref().map(|f| (&f.bytes, f.executable))
                {
                    return Err(ContributionError::Conflict {
                        path: change.path.clone(),
                        reason: "file changed during mutation; moved file retained in recovery"
                            .into(),
                    });
                }
            }
            if after.is_some() {
                safe_fs::rename_new(&recovery, &format!("new-{index}"), &parent, &leaf)?;
            }
            report.applied.push(change.path.clone());
        }
        Ok(())
    })();
    if let Err(error) = result {
        return Err(ContributionError::RecoveryRequired {
            path: recovery_path,
            reason: error.to_string(),
        });
    }
    Ok(report)
}

fn deleted_ancestor(changes: &[crate::Change], path: &str) -> bool {
    changes.iter().any(|change| {
        change.before.is_some()
            && change.after.is_none()
            && path.starts_with(&format!("{}/", change.path))
    })
}
