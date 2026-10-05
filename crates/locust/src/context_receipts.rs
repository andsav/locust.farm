//! Private client-side storage for exact daemon receipts. Reads never acknowledge
//! content. CLI and MCP share references only within one credential and session.
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use locust_proto::api::{ContextReceipt, Credential, Response, SessionSecret};
use locust_proto::crypto;
use rustix::fs::{Mode, OFlags};
use serde_json::{Value, json};

use crate::failure::Failure;

/// Which client is speaking, so its messages name the context read the way
/// its caller runs it. The API's operation name, `context.read`, is neither a
/// command nor a tool.
#[derive(Clone, Copy)]
pub(crate) enum Surface {
    Cli,
    Mcp,
}

impl Surface {
    fn context_read(self) -> &'static str {
        match self {
            Self::Cli => "locust context read",
            Self::Mcp => "locust_context_read",
        }
    }
}

pub(crate) struct Cache {
    home: PathBuf,
    credential: Credential,
    session: Option<SessionSecret>,
    surface: Surface,
}

impl Cache {
    pub(crate) fn new(
        home: &Path,
        credential: Credential,
        session: Option<SessionSecret>,
        surface: Surface,
    ) -> Self {
        Self {
            home: home.to_owned(),
            credential,
            session,
            surface,
        }
    }

    fn directory(&self, create: bool) -> Result<File, Failure> {
        let session = self.session.ok_or_else(|| {
            Failure::invalid("context receipt references require an explicit execution session")
        })?;
        let mut directory = File::from(
            rustix::fs::open(
                &self.home,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(storage)?,
        );
        check_directory(&directory)?;
        for name in [
            "context-receipts".to_owned(),
            locust_proto::id::BlobHash(self.credential.digest()).to_string(),
            session.instance().to_string(),
        ] {
            if create {
                match rustix::fs::mkdirat(&directory, &name, Mode::from_raw_mode(0o700)) {
                    Ok(()) => directory.sync_all().map_err(storage)?,
                    Err(rustix::io::Errno::EXIST) => {}
                    Err(error) => return Err(storage(error)),
                }
            }
            directory = File::from(
                rustix::fs::openat(
                    &directory,
                    &name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(storage)?,
            );
            check_directory(&directory)?;
        }
        Ok(directory)
    }

    pub(crate) fn store(&self, receipt: &ContextReceipt) -> Result<String, Failure> {
        if self.session.map(|session| session.instance()) != Some(receipt.session) {
            return Err(Failure::invalid(
                "daemon context receipt belongs to another execution session",
            ));
        }
        let bytes = serde_json::to_vec(receipt).map_err(storage)?;
        let hash = crypto::content_hash(&bytes).to_string();
        let reference = format!("ctx:{hash}");
        let directory = self.directory(true)?;
        let target = format!("{hash}.json");
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(storage)?;
        let temporary = format!(".{}.tmp", crypto::content_hash(&random));
        let result = (|| {
            let mut file = File::from(
                rustix::fs::openat(
                    &directory,
                    &temporary,
                    OFlags::WRONLY
                        | OFlags::CREATE
                        | OFlags::EXCL
                        | OFlags::NOFOLLOW
                        | OFlags::CLOEXEC,
                    Mode::from_raw_mode(0o600),
                )
                .map_err(storage)?,
            );
            file.write_all(&bytes).map_err(storage)?;
            file.sync_all().map_err(storage)?;
            rustix::fs::renameat(&directory, &temporary, &directory, &target).map_err(storage)?;
            directory.sync_all().map_err(storage)?;
            Ok(reference)
        })();
        if result.is_err() {
            let _ = rustix::fs::unlinkat(&directory, &temporary, rustix::fs::AtFlags::empty());
        }
        result
    }

    pub(crate) fn load(&self, reference: &str) -> Result<ContextReceipt, Failure> {
        let hash = reference.strip_prefix("ctx:").filter(|hash| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))).ok_or_else(|| Failure::invalid(format!("receipt must be the ctx: reference returned by {} in this credential and session", self.surface.context_read())))?;
        let directory = self.directory(false)?;
        let mut file = File::from(
            rustix::fs::openat(
                &directory,
                format!("{hash}.json"),
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(storage)?,
        );
        let metadata = file.metadata().map_err(storage)?;
        if !metadata.is_file()
            || metadata.permissions().mode() & 0o7777 != 0o600
            || metadata.nlink() != 1
            || metadata.uid() != rustix::process::getuid().as_raw()
        {
            return Err(Failure::invalid(
                "context receipt cache files must be owned regular files with mode 0600 and one link",
            ));
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(storage)?;
        if crypto::content_hash(&bytes).to_string() != hash {
            return Err(Failure::invalid(
                "context receipt cache content does not match its reference",
            ));
        }
        let receipt: ContextReceipt = serde_json::from_slice(&bytes).map_err(storage)?;
        if self.session.map(|session| session.instance()) != Some(receipt.session) {
            return Err(Failure::invalid(
                "context receipt belongs to another execution session",
            ));
        }
        Ok(receipt)
    }

    pub(crate) fn present(&self, response: &Response) -> Result<Value, Failure> {
        let mut value = serde_json::to_value(response).map_err(storage)?;
        if let Response::Context(context) = response
            && let Some(receipt) = &context.receipt
        {
            value["context"]["receipt"] = json!(self.store(receipt)?);
        }
        Ok(value)
    }
}

fn check_directory(directory: &File) -> Result<(), Failure> {
    let metadata = directory.metadata().map_err(storage)?;
    if !metadata.is_dir()
        || metadata.permissions().mode() & 0o7777 != 0o700
        || metadata.uid() != rustix::process::getuid().as_raw()
    {
        return Err(Failure::invalid(
            "context receipt cache directories must be owned directories with mode 0700",
        ));
    }
    Ok(())
}
fn storage(error: impl std::fmt::Display) -> Failure {
    Failure::invalid(format!(
        "context receipt cache: {error}; read context again if this session's receipt was removed"
    ))
}

/// Agent-facing schemas use a local reference. The native API remains typed.
pub(crate) fn operation_schema(name: &str, surface: Surface) -> Option<Value> {
    let mut schema = locust_proto::api::operation_schema(name)?;
    if name == "context.acknowledge" {
        schema["properties"]["receipt"] = json!({"type":"string","pattern":"^ctx:[0-9a-f]{64}$","description":format!("The exact receipt reference returned by {} using this credential and session. Acknowledges only the complete content in that page.", surface.context_read())});
        if let Some(definitions) = schema.get_mut("$defs").and_then(Value::as_object_mut) {
            for name in [
                "ContextReceipt",
                "ContextSeen",
                "Signature",
                "InstanceId",
                "BlobHash",
                "PublicKey",
                "EventId",
            ] {
                definitions.remove(name);
            }
        }
    }
    Some(schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_proto::id::{GoalId, PublicKey, Signature};
    use std::os::unix::fs::symlink;
    fn fixture() -> (tempfile::TempDir, Cache, ContextReceipt) {
        let home = tempfile::tempdir().unwrap();
        std::fs::set_permissions(home.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let session = SessionSecret([2; 32]);
        let cache = Cache::new(
            home.path(),
            Credential([1; 32]),
            Some(session),
            Surface::Cli,
        );
        let receipt = ContextReceipt {
            goal: GoalId([3; 32]),
            principal: PublicKey([4; 32]),
            session: session.instance(),
            revision: 1,
            entries: vec![],
            signature: Signature([5; 64]),
        };
        (home, cache, receipt)
    }
    #[test]
    fn persists_exact_receipt_and_isolates_credentials_and_sessions() {
        let (home, cache, receipt) = fixture();
        let reference = cache.store(&receipt).unwrap();
        assert_eq!(reference.len(), 68);
        let reopened = Cache::new(
            home.path(),
            Credential([1; 32]),
            Some(SessionSecret([2; 32])),
            Surface::Cli,
        );
        assert_eq!(reopened.load(&reference).unwrap(), receipt);
        assert_eq!(cache.store(&receipt).unwrap(), reference);
        assert!(
            Cache::new(
                home.path(),
                Credential([9; 32]),
                Some(SessionSecret([2; 32])),
                Surface::Cli,
            )
            .load(&reference)
            .is_err()
        );
        assert!(
            Cache::new(
                home.path(),
                Credential([1; 32]),
                Some(SessionSecret([9; 32])),
                Surface::Cli,
            )
            .load(&reference)
            .is_err()
        );
        assert!(reopened.load("ctx:../../outside").is_err());
    }
    #[test]
    fn each_surface_names_the_context_read_its_caller_can_run() {
        let (home, _, _) = fixture();
        for (surface, name) in [
            (Surface::Cli, "locust context read"),
            (Surface::Mcp, "locust_context_read"),
        ] {
            let session = Some(SessionSecret([2; 32]));
            let cache = Cache::new(home.path(), Credential([1; 32]), session, surface);
            let schema = operation_schema("context.acknowledge", surface).unwrap();
            for text in [
                cache.load("ctx:typo").unwrap_err().message,
                schema["properties"]["receipt"]["description"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            ] {
                assert!(text.contains(name), "{text}");
                assert!(!text.contains("context.read"), "{text}");
            }
        }
    }
    #[test]
    fn rejects_replaced_directory_and_corrupted_or_linked_receipt() {
        let (home, cache, receipt) = fixture();
        let reference = cache.store(&receipt).unwrap();
        let file = home
            .path()
            .join("context-receipts")
            .join(locust_proto::id::BlobHash(Credential([1; 32]).digest()).to_string())
            .join(receipt.session.to_string())
            .join(format!("{}.json", &reference[4..]));
        std::fs::write(&file, b"{}").unwrap();
        assert!(cache.load(&reference).is_err());
        cache.store(&receipt).unwrap();
        std::fs::hard_link(&file, home.path().join("link")).unwrap();
        assert!(cache.load(&reference).is_err());
        std::fs::remove_file(&file).unwrap();
        symlink(home.path().join("link"), &file).unwrap();
        assert!(cache.load(&reference).is_err());
        let other = tempfile::tempdir().unwrap();
        let path = home.path().join("context-receipts");
        std::fs::rename(&path, home.path().join("previous")).unwrap();
        symlink(other.path(), &path).unwrap();
        assert!(cache.store(&receipt).is_err());
    }
}
