//! Daemon-owned folders for an agent's own checkout request.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, PathBuf};

use locust_core::node::CheckoutFiles;
use locust_proto::api::{ApiError, DirectoryIdentity, ErrorCode};
use locust_proto::id::{BlobHash, CheckoutId};
use locust_proto::manifest::Manifest;
use locust_workspace::BlobSource;

pub(super) struct LocalCheckoutFiles {
    pub(super) home: PathBuf,
}

struct Fetch<'a> {
    fetch: &'a mut dyn FnMut(BlobHash) -> Result<Vec<u8>, ApiError>,
}

impl BlobSource for Fetch<'_> {
    fn fetch(&mut self, hash: &BlobHash) -> io::Result<Option<Vec<u8>>> {
        (self.fetch)(*hash).map(Some).map_err(io::Error::other)
    }
}

impl CheckoutFiles for LocalCheckoutFiles {
    fn destination(&self, checkout: CheckoutId) -> Result<String, ApiError> {
        let path = self.home.join("checkouts").join(checkout.to_string());
        let mut ancestor = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .map_err(checkout_directory_error)?
                .join(path)
        };
        let mut missing = Vec::new();
        let mut resolved = loop {
            match ancestor.canonicalize() {
                Ok(path) => break path,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    let parent = ancestor.parent().ok_or_else(|| {
                        ApiError::new(ErrorCode::Unavailable, "checkout directory has no parent")
                    })?;
                    missing.push(ancestor.strip_prefix(parent).unwrap().to_path_buf());
                    ancestor = parent.to_path_buf();
                }
                Err(error) => return Err(checkout_directory_error(error)),
            }
        };
        for suffix in missing.into_iter().rev() {
            match suffix.components().next() {
                Some(Component::ParentDir) => {
                    resolved.pop();
                }
                Some(Component::CurDir) => {}
                Some(Component::Normal(_)) => resolved.push(suffix),
                _ => unreachable!("suffix is one relative path component"),
            }
        }
        Ok(resolved.to_string_lossy().into_owned())
    }

    fn materialize(
        &self,
        checkout: CheckoutId,
        manifest: &Manifest,
        fetch: &mut dyn FnMut(BlobHash) -> Result<Vec<u8>, ApiError>,
    ) -> Result<(String, DirectoryIdentity), ApiError> {
        let parent = self.home.join("checkouts");
        crate::secret::create_private_dir(&parent).map_err(|error| {
            ApiError::new(
                ErrorCode::Unavailable,
                format!("checkout directory: {error}"),
            )
        })?;
        let path = parent.join(checkout.to_string());
        if path.exists() {
            // Materialization may have finished before a failed store commit.
            // Reuse only the exact files, never an arbitrary existing folder.
            let observed = locust_workspace::inspect_local_tree(&path).map_err(|error| {
                ApiError::new(ErrorCode::Conflict, format!("existing checkout: {error}"))
            })?;
            let mut expected = BTreeMap::new();
            let mut directories = BTreeSet::new();
            for entry in &manifest.entries {
                let bytes = fetch(entry.content)?;
                expected.insert(
                    entry.path.clone(),
                    locust_workspace::FileDigest {
                        digest: locust_proto::crypto::content_hash(&bytes),
                        executable: entry.executable,
                        size: entry.size,
                    },
                );
                let mut parent = std::path::Path::new(&entry.path).parent();
                while let Some(path) = parent.filter(|path| !path.as_os_str().is_empty()) {
                    directories.insert(path.to_string_lossy().into_owned());
                    parent = path.parent();
                }
            }
            if observed.files != expected
                || observed.directories.into_iter().collect::<BTreeSet<_>>() != directories
            {
                return Err(ApiError::new(
                    ErrorCode::Conflict,
                    "existing checkout differs from the accepted files",
                ));
            }
        } else {
            let mut source = Fetch { fetch };
            locust_workspace::materialize(manifest, &mut source, &path).map_err(|error| {
                ApiError::new(
                    ErrorCode::Unavailable,
                    format!("checkout materialization: {error}"),
                )
            })?;
        }
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            ApiError::new(
                ErrorCode::Unavailable,
                format!("checkout directory: {error}"),
            )
        })?;
        if !metadata.is_dir() {
            return Err(ApiError::new(
                ErrorCode::Invalid,
                "checkout is not a directory",
            ));
        }
        let root = path.canonicalize().map_err(|error| {
            ApiError::new(
                ErrorCode::Unavailable,
                format!("checkout directory: {error}"),
            )
        })?;
        Ok((
            root.to_string_lossy().into_owned(),
            DirectoryIdentity {
                device: metadata.dev(),
                inode: metadata.ino(),
            },
        ))
    }
}

fn checkout_directory_error(error: io::Error) -> ApiError {
    ApiError::new(
        ErrorCode::Unavailable,
        format!("checkout directory: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_proto::manifest::Entry;

    #[test]
    fn destination_does_not_create_checkout_directories() {
        let home = tempfile::tempdir().unwrap();
        let files = LocalCheckoutFiles {
            home: home.path().to_path_buf(),
        };
        let checkout = CheckoutId([8; 16]);
        let destination = files.destination(checkout).unwrap();
        assert!(!home.path().join("checkouts").exists());

        let manifest = Manifest { entries: vec![] };
        let (root, _) = files
            .materialize(checkout, &manifest, &mut |_| unreachable!())
            .unwrap();
        assert_eq!(destination, root);
    }

    #[test]
    fn destination_resolves_an_existing_home_symlink() {
        let home = tempfile::tempdir().unwrap();
        let alias = home.path().join("alias");
        std::os::unix::fs::symlink(home.path(), &alias).unwrap();
        let files = LocalCheckoutFiles { home: alias };
        let checkout = CheckoutId([9; 16]);

        let destination = files.destination(checkout).unwrap();
        assert_eq!(
            destination,
            home.path()
                .canonicalize()
                .unwrap()
                .join("checkouts")
                .join(checkout.to_string())
                .to_string_lossy()
        );
        assert!(!home.path().join("checkouts").exists());
    }

    #[test]
    fn an_uncertain_registration_reuses_only_its_exact_private_files() {
        let home = tempfile::tempdir().unwrap();
        let files = LocalCheckoutFiles {
            home: home.path().to_path_buf(),
        };
        let checkout = CheckoutId([7; 16]);
        let content = b"accepted bytes";
        let hash = locust_proto::crypto::content_hash(content);
        let manifest = Manifest {
            entries: vec![Entry {
                path: "src/main.txt".into(),
                executable: false,
                size: content.len() as u64,
                content: hash,
            }],
        };
        let fetch = &mut |requested| {
            assert_eq!(requested, hash);
            Ok(content.to_vec())
        };
        let (root, first) = files.materialize(checkout, &manifest, fetch).unwrap();
        assert_eq!(
            std::fs::read(PathBuf::from(&root).join("src/main.txt")).unwrap(),
            content
        );
        let (same, second) = files.materialize(checkout, &manifest, fetch).unwrap();
        assert_eq!(same, root);
        assert_eq!(second, first);
        std::fs::write(PathBuf::from(&root).join("src/main.txt"), b"local edit").unwrap();
        assert_eq!(
            files
                .materialize(checkout, &manifest, fetch)
                .unwrap_err()
                .code,
            ErrorCode::Conflict
        );
    }
}
