//! Review is generated from authenticated blob plaintext, never from a patch
//! command, external diff driver or contributor-supplied review prose.

use locust_proto::crypto::content_hash;
use locust_proto::id::BlobHash;
use locust_proto::manifest::Entry;
use serde::Serialize;

use crate::contribution::{file_bytes, load};
use crate::{BlobSource, ContributionError};

#[derive(Clone, Debug, Serialize)]
pub struct FileSummary {
    pub bytes: u64,
    pub executable: bool,
    /// Plaintext digest for reviewing the file, not its sealed object ID.
    pub plaintext_hash: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChangeReview {
    pub path: String,
    pub before: Option<FileSummary>,
    pub after: Option<FileSummary>,
    /// UTF-8 text without NUL bytes is rendered as a unified diff; binary
    /// changes retain exact size, digest and executable-bit summaries.
    pub unified_diff: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContributionReview {
    pub contribution_id: BlobHash,
    pub base: BlobHash,
    pub head: BlobHash,
    pub changes: Vec<ChangeReview>,
}

pub fn review_contribution(
    id: BlobHash,
    source: &mut dyn BlobSource,
) -> Result<ContributionReview, ContributionError> {
    let (contribution, _, _) = load(id, source)?;
    let mut changes = Vec::new();
    for change in contribution.changes {
        let before = read(source, change.before.as_ref())?;
        let after = read(source, change.after.as_ref())?;
        let unified_diff = text_diff(&change.path, before.as_deref(), after.as_deref());
        changes.push(ChangeReview {
            path: change.path,
            before: summary(change.before.as_ref(), before.as_deref()),
            after: summary(change.after.as_ref(), after.as_deref()),
            unified_diff,
        });
    }
    Ok(ContributionReview {
        contribution_id: id,
        base: contribution.base,
        head: contribution.head,
        changes,
    })
}

fn read(
    source: &mut dyn BlobSource,
    entry: Option<&Entry>,
) -> Result<Option<Vec<u8>>, ContributionError> {
    entry.map(|entry| file_bytes(source, entry)).transpose()
}
fn summary(entry: Option<&Entry>, bytes: Option<&[u8]>) -> Option<FileSummary> {
    entry.zip(bytes).map(|(entry, bytes)| FileSummary {
        bytes: entry.size,
        executable: entry.executable,
        plaintext_hash: content_hash(bytes).to_string(),
    })
}

fn text_diff(path: &str, before: Option<&[u8]>, after: Option<&[u8]>) -> Option<String> {
    let old = before.unwrap_or_default();
    let new = after.unwrap_or_default();
    if old == new {
        return None;
    }
    if old.contains(&0) || new.contains(&0) {
        return None;
    }
    let old: Vec<_> = std::str::from_utf8(old)
        .ok()?
        .split_inclusive('\n')
        .collect();
    let new: Vec<_> = std::str::from_utf8(new)
        .ok()?
        .split_inclusive('\n')
        .collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let start = prefix.saturating_sub(3);
    let old_end = (old.len() - suffix + 3).min(old.len());
    let new_end = (new.len() - suffix + 3).min(new.len());
    let old_name = if before.is_some() {
        format!("a/{path}")
    } else {
        "/dev/null".into()
    };
    let new_name = if after.is_some() {
        format!("b/{path}")
    } else {
        "/dev/null".into()
    };
    let mut result = format!(
        "--- {old_name}\n+++ {new_name}\n@@ -{},{} +{},{} @@\n",
        if old_end == start { start } else { start + 1 },
        old_end - start,
        if new_end == start { start } else { start + 1 },
        new_end - start
    );
    let mut line = |prefix: char, text: &str| {
        result.push(prefix);
        result.push_str(text);
        if !text.ends_with('\n') {
            result.push_str("\n\\ No newline at end of file\n");
        }
    };
    for text in &old[start..prefix] {
        line(' ', text);
    }
    for text in &old[prefix..old.len() - suffix] {
        line('-', text);
    }
    for text in &new[prefix..new.len() - suffix] {
        line('+', text);
    }
    for text in &old[old.len() - suffix..old_end] {
        line(' ', text);
    }
    Some(result)
}
