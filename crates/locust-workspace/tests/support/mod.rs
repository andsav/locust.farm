//! Shared test fixtures: an in-memory object store and real Git
//! repositories in temporary directories.

#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

use locust_proto::crypto::content_hash;
use locust_proto::id::BlobHash;
use locust_proto::manifest::{Entry, Manifest};
use locust_workspace::{BlobSink, BlobSource};

/// Stands in for the daemon: stores plaintext under its content hash.
#[derive(Default)]
pub struct MemBlobs {
    pub objects: HashMap<BlobHash, Vec<u8>>,
}

impl BlobSink for MemBlobs {
    fn store(&mut self, plaintext: &[u8]) -> io::Result<BlobHash> {
        let hash = content_hash(plaintext);
        self.objects.insert(hash, plaintext.to_vec());
        Ok(hash)
    }
}

impl BlobSource for MemBlobs {
    fn fetch(&mut self, hash: &BlobHash) -> io::Result<Option<Vec<u8>>> {
        Ok(self.objects.get(hash).cloned())
    }
}

/// A file as (path, contents, executable).
pub type File<'a> = (&'a str, &'a [u8], bool);

/// Stores `files` and returns their manifest.
pub fn manifest_of(store: &mut MemBlobs, files: &[File]) -> Manifest {
    let mut entries: Vec<Entry> = files
        .iter()
        .map(|&(path, contents, executable)| Entry {
            path: path.to_owned(),
            executable,
            size: contents.len() as u64,
            content: store.store(contents).unwrap(),
        })
        .collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Manifest { entries }
}

/// Every file under `root` as (contents, executable), keyed by its relative
/// path. Fails on anything that is not a regular file or a directory.
pub fn read_tree(root: &Path) -> BTreeMap<String, (Vec<u8>, bool)> {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeMap<String, (Vec<u8>, bool)>) {
        for item in fs::read_dir(dir).unwrap() {
            let path = item.unwrap().path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            if metadata.is_dir() {
                walk(root, &path, files);
            } else {
                assert!(
                    metadata.is_file(),
                    "{} is not a regular file",
                    path.display()
                );
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned();
                let executable = metadata.permissions().mode() & 0o111 != 0;
                files.insert(relative, (fs::read(&path).unwrap(), executable));
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}

/// Names of the entries directly inside `dir`, sorted.
pub fn list_dir(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|item| item.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// A Git repository in a temporary directory. The fixture's own Git commands
/// read no system or user configuration.
pub struct TestRepo {
    dir: tempfile::TempDir,
}

impl TestRepo {
    pub fn new() -> Self {
        let repo = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        repo.git(["init", "-q"]);
        repo
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn write(&self, path: &str, contents: &[u8]) {
        let path = self.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    pub fn write_executable(&self, path: &str, contents: &[u8]) {
        self.write(path, contents);
        fs::set_permissions(self.path().join(path), fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// Commits everything in the work tree and returns the commit's name.
    pub fn commit_all(&self) -> String {
        self.git(["add", "-A"]);
        self.commit()
    }

    /// Commits the index and returns the commit's name.
    pub fn commit(&self) -> String {
        self.git(["commit", "-q", "--allow-empty", "-m", "test"]);
        self.git(["rev-parse", "HEAD"])
    }

    /// Adds an index entry directly, for paths and modes a work tree cannot
    /// hold or that should not be checked out.
    pub fn add_index_entry(&self, mode: &str, object: &str, path: &[u8]) {
        let mut entry = format!("{mode},{object},").into_bytes();
        entry.extend_from_slice(path);
        self.git([
            OsStr::new("-c"),
            OsStr::new("core.protectNTFS=false"),
            OsStr::new("update-index"),
            OsStr::new("--add"),
            OsStr::new("--cacheinfo"),
            std::os::unix::ffi::OsStrExt::from_bytes(entry.as_slice()),
        ]);
    }

    /// Writes `contents` as a blob and returns its object name.
    pub fn hash_object(&self, contents: &[u8]) -> String {
        let mut child = git_command(self.path())
            .args(["hash-object", "-w", "--stdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(contents).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    pub fn git<I, S>(&self, args: I) -> String
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = git_command(self.path()).args(args).output().unwrap();
        assert!(
            output.status.success(),
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }
}

fn git_command(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args([
            "-c",
            "user.name=Locust Test",
            "-c",
            "user.email=test@locust.invalid",
            "-c",
            "commit.gpgSign=false",
            "-c",
            "core.autocrlf=false",
        ]);
    command
}
