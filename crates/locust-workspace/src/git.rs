//! Reads one commit's tree and file contents with Git's plumbing commands.
//!
//! Every command starts from an empty environment, so no inherited `GIT_*`
//! variable can redirect it, and with no system or user configuration and no
//! system or user attributes file (with `HOME` and `XDG_CONFIG_HOME` unset,
//! Git has nowhere to find them). Replace refs are ignored, so the objects
//! read are the ones the commit names. A partial clone never fetches a
//! missing object, and every transport is disabled, so no configured network
//! or SSH command can run.
//!
//! The repository's own configuration is still read, because Git needs it to
//! open the repository. The commands used here (`rev-parse`, `ls-tree`, and
//! `cat-file --batch` without `--filters` or `--textconv`) never read the
//! index and run no hook, filter, text conversion, fsmonitor or pager,
//! whatever that configuration says. `tests/user_config.rs` checks this.

use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::export::ExportError;

const ISOLATED_ENV: [(&str, &str); 5] = [
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_ATTR_NOSYSTEM", "1"),
    ("GIT_NO_REPLACE_OBJECTS", "1"),
    // Git 2.44 and later; `protocol.allow=never` covers older versions.
    ("GIT_NO_LAZY_FETCH", "1"),
];

/// Command-line settings take precedence over repository configuration and
/// reach any Git subprocess, such as a lazy fetch.
const ISOLATED_CONFIG: [&str; 2] = ["-c", "protocol.allow=never"];

fn git(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(dir).env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    command
        .envs(ISOLATED_ENV)
        .args(ISOLATED_CONFIG)
        .stdin(Stdio::null());
    command
}

/// Runs one command to completion and returns its standard output.
fn run(dir: &Path, args: &[&OsStr]) -> Result<Vec<u8>, ExportError> {
    let output = git(dir).args(args).output()?;
    if !output.status.success() {
        let operation = args
            .first()
            .map_or("git".into(), |arg| arg.to_string_lossy());
        return Err(ExportError::Git(format!(
            "{operation} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

/// The path from the top of the work tree to `dir`: empty at the top,
/// otherwise ending in `/`.
pub(crate) fn work_tree_prefix(dir: &Path) -> Result<Vec<u8>, ExportError> {
    let output = run(
        dir,
        &["rev-parse", "--is-inside-work-tree", "--show-prefix"].map(OsStr::new),
    )?;
    let prefix = output
        .strip_prefix(b"true\n")
        .ok_or(ExportError::NotInWorkTree)?;
    Ok(prefix.strip_suffix(b"\n").unwrap_or(prefix).to_vec())
}

/// The full object name of the commit `name` resolves to.
pub(crate) fn resolve_commit(dir: &Path, name: &str) -> Result<String, ExportError> {
    let spec = format!("{name}^{{commit}}");
    let output = run(
        dir,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            spec.as_str(),
        ]
        .map(OsStr::new),
    )
    .map_err(|error| match error {
        ExportError::Git(_) => ExportError::Git(format!("{name:?} does not name a commit")),
        other => other,
    })?;
    String::from_utf8(output)
        .ok()
        .and_then(|text| text.strip_suffix('\n').map(str::to_owned))
        .ok_or_else(|| ExportError::Git("rev-parse printed an unexpected answer".into()))
}

/// One entry of a recursive tree listing.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TreeEntry {
    /// Octal mode as Git prints it, such as `100644`.
    pub mode: String,
    /// Object name in hex.
    pub object: String,
    /// Object size; zero for a submodule, which has none.
    pub size: u64,
    /// Path relative to the listed directory, as raw bytes.
    pub path: Vec<u8>,
}

/// Lists every non-directory entry under `prefix` in `commit`, with paths
/// relative to `prefix`.
pub(crate) fn list_tree(
    dir: &Path,
    commit: &str,
    prefix: &[u8],
) -> Result<Vec<TreeEntry>, ExportError> {
    let mut tree = format!("{commit}:").into_bytes();
    tree.extend_from_slice(prefix.strip_suffix(b"/").unwrap_or(prefix));
    let output = run(
        dir,
        &[
            OsStr::new("ls-tree"),
            // Without this, ls-tree would also filter by the directory it
            // runs in, on top of the subtree named here.
            OsStr::new("--full-tree"),
            OsStr::new("-r"),
            OsStr::new("-z"),
            OsStr::new("-l"),
            OsStr::from_bytes(&tree),
        ],
    )?;
    parse_tree(&output)
}

/// Parses `ls-tree -r -z -l` output: `<mode> <type> <object> <size>\t<path>`,
/// each record ending in NUL, the size padded with spaces or `-` when the
/// entry has none.
fn parse_tree(output: &[u8]) -> Result<Vec<TreeEntry>, ExportError> {
    output
        .split(|&byte| byte == 0)
        .filter(|record| !record.is_empty())
        .map(|record| {
            parse_record(record).ok_or_else(|| {
                ExportError::Git(format!(
                    "ls-tree printed an unexpected line: {:?}",
                    String::from_utf8_lossy(record)
                ))
            })
        })
        .collect()
}

fn parse_record(record: &[u8]) -> Option<TreeEntry> {
    let tab = record.iter().position(|&byte| byte == b'\t')?;
    let mut fields = std::str::from_utf8(&record[..tab])
        .ok()?
        .split_ascii_whitespace();
    let (mode, _kind, object, size) = (
        fields.next()?,
        fields.next()?,
        fields.next()?,
        fields.next()?,
    );
    if fields.next().is_some() {
        return None;
    }
    Some(TreeEntry {
        mode: mode.to_owned(),
        object: object.to_owned(),
        size: if size == "-" { 0 } else { size.parse().ok()? },
        path: record[tab + 1..].to_vec(),
    })
}

/// A running `git cat-file --batch`, asked for one object at a time. Git
/// flushes each answer, so a request never waits on unread output.
pub(crate) struct Blobs {
    child: Child,
    requests: ChildStdin,
    answers: BufReader<ChildStdout>,
}

impl Blobs {
    pub(crate) fn open(dir: &Path) -> Result<Self, ExportError> {
        let mut child = git(dir)
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let (Some(requests), Some(answers)) = (child.stdin.take(), child.stdout.take()) else {
            unreachable!("both streams were requested as pipes");
        };
        Ok(Self {
            child,
            requests,
            answers: BufReader::new(answers),
        })
    }

    /// Reads the blob `object`, which must be exactly `size` bytes.
    pub(crate) fn read(&mut self, object: &str, size: u64) -> Result<Vec<u8>, ExportError> {
        let unexpected = || ExportError::Git(format!("cat-file did not return blob {object}"));
        self.requests.write_all(format!("{object}\n").as_bytes())?;
        let mut header = Vec::new();
        self.answers.read_until(b'\n', &mut header)?;
        if header != format!("{object} blob {size}\n").as_bytes() {
            return Err(unexpected());
        }
        let mut bytes = vec![0; usize::try_from(size).map_err(|_| unexpected())?];
        self.answers.read_exact(&mut bytes)?;
        let mut end = [0];
        self.answers.read_exact(&mut end)?;
        if end != *b"\n" {
            return Err(unexpected());
        }
        Ok(bytes)
    }
}

impl Drop for Blobs {
    fn drop(&mut self) {
        // Git may be blocked writing an answer nobody will read.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_listings_parse_including_unusual_paths() {
        let output = b"100644 blob aa      12\tsrc/a b.rs\0\
            100755 blob bb       3\trun\tme\0\
            160000 commit cc       -\tvendor/lib\0\
            100644 blob dd 7\tbad\xffname\0";
        let entries = parse_tree(output).unwrap();
        let summary: Vec<_> = entries
            .iter()
            .map(|entry| {
                (
                    entry.mode.as_str(),
                    entry.object.as_str(),
                    entry.size,
                    entry.path.as_slice(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("100644", "aa", 12, b"src/a b.rs".as_slice()),
                ("100755", "bb", 3, b"run\tme".as_slice()),
                ("160000", "cc", 0, b"vendor/lib".as_slice()),
                ("100644", "dd", 7, b"bad\xffname".as_slice()),
            ]
        );
        assert!(parse_tree(b"").unwrap().is_empty());
    }

    #[test]
    fn malformed_tree_listings_are_errors() {
        for output in [
            b"100644 blob aa 12 src\0".as_slice(),
            b"100644 blob aa\tsrc\0",
            b"100644 blob aa 1x\tsrc\0",
            b"100644 blob aa 1 2\tsrc\0",
        ] {
            assert!(matches!(parse_tree(output), Err(ExportError::Git(_))));
        }
    }
}
