//! Review is generated from authenticated blob plaintext, never from a patch
//! command, external diff driver or contributor-supplied review prose.

use locust_proto::crypto::content_hash;
use serde::Serialize;

/// Review exact plaintext tree changes without loading a serialized patch.
pub fn review_changes(changes: &[crate::TreeChange]) -> Vec<ChangeReview> {
    let summary = |file: &crate::FileValue| FileSummary {
        bytes: file.bytes.len() as u64,
        executable: file.executable,
        plaintext_hash: content_hash(&file.bytes).to_string(),
    };
    changes
        .iter()
        .map(|change| ChangeReview {
            path: change.path.clone(),
            before: change.before.as_ref().map(summary),
            after: change.after.as_ref().map(summary),
            unified_diff: text_diff(
                &change.path,
                change.before.as_ref().map(|file| file.bytes.as_slice()),
                change.after.as_ref().map(|file| file.bytes.as_slice()),
            ),
        })
        .collect()
}

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
