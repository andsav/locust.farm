//! Explicit local software installation. A signed manifest identifies each immutable
//! release; one atomic symlink chooses it. Daemon data and service/config changes
//! are separate operations. A persisted withdrawal watermark survives uninstall.
use crate::{
    failure::Failure,
    package::{self, Verified},
};
use locust_proto::api::ErrorCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io,
    path::{Component, Path, PathBuf},
    process::Command,
};

pub mod onboarding;
pub mod service;
pub mod service_install;
pub mod setup;

const POLICY: &str = "trust-state.json";

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    format: String,
    key: [u8; 32],
    withdrawals: Vec<u8>,
    signature: Vec<u8>,
}
impl Policy {
    fn from_verified(value: &Verified) -> Self {
        Self {
            format: "locust-install-trust-v1".into(),
            key: value.trust_key,
            withdrawals: value.withdrawals_bytes.clone(),
            signature: value.withdrawals_signature.to_vec(),
        }
    }
    fn validate(&self) -> Result<package::Withdrawals, Failure> {
        if self.format != "locust-install-trust-v1" {
            return Err(corrupt("unknown installation trust format"));
        }
        let sig: [u8; 64] = self
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| corrupt("invalid saved policy signature"))?;
        package::verify_signature(&self.key, &self.withdrawals, &sig)?;
        package::validate_withdrawals(&self.withdrawals)
    }
}

#[derive(Clone, Serialize)]
pub struct Plan {
    pub format: &'static str,
    pub prefix: PathBuf,
    pub action: &'static str,
    pub current_manifest: Option<String>,
    pub current_policy_sha256: Option<String>,
    pub target_manifest: Option<String>,
    pub target_policy_sha256: Option<String>,
    pub allow_downgrade: bool,
    pub executable: PathBuf,
    pub daemon_data_changed: bool,
}
impl Plan {
    pub fn digest(&self) -> Result<String, Failure> {
        Ok(package::sha256(&encode(self)?))
    }
    pub fn json(&self) -> Result<Value, Failure> {
        Ok(json!({"plan":self,"plan_sha256":self.digest()?}))
    }
}
fn corrupt(message: &str) -> Failure {
    Failure::new(ErrorCode::Corrupted, message)
}
fn conflict(message: &str) -> Failure {
    Failure::new(ErrorCode::Conflict, message)
}
fn io_error(error: io::Error) -> Failure {
    Failure::invalid(format!("installation filesystem: {error}"))
}
fn encode(value: &impl Serialize) -> Result<Vec<u8>, Failure> {
    serde_json::to_vec(value).map_err(|_| Failure::internal("cannot encode installation state"))
}
fn exists(path: &Path) -> Result<bool, Failure> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(io_error(e)),
    }
}
/// Resolve existing ancestor aliases once and reject lexical traversal. Returned
/// plans name the same absolute prefix even when HOME has an OS-provided alias.
pub fn absolute(path: &Path) -> Result<PathBuf, Failure> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(Failure::usage(
            "installation paths must be absolute without . or ..",
        ));
    }
    let mut ancestor = path.to_path_buf();
    let mut missing = Vec::new();
    while !exists(&ancestor)? {
        missing.push(
            ancestor
                .file_name()
                .ok_or_else(|| Failure::invalid("path has no filename"))?
                .to_os_string(),
        );
        ancestor.pop();
    }
    // The selected leaf itself may not redirect to another installation.
    if ancestor == path
        && fs::symlink_metadata(path)
            .map_err(io_error)?
            .file_type()
            .is_symlink()
    {
        return Err(Failure::invalid(
            "selected installation path must not be a symlink",
        ));
    }
    let mut resolved = ancestor.canonicalize().map_err(io_error)?;
    for name in missing.iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}
fn private_dir(path: &Path, create: bool) -> Result<(), Failure> {
    if !exists(path)? {
        if !create {
            return Ok(());
        }
        let parent = path
            .parent()
            .ok_or_else(|| Failure::invalid("directory has no parent"))?;
        if !exists(parent)? {
            private_dir(parent, true)?;
        }
        match DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => sync(parent)?,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(io_error(e)),
        }
    }
    let meta = fs::symlink_metadata(path).map_err(io_error)?;
    if !meta.is_dir()
        || meta.file_type().is_symlink()
        || meta.uid() != rustix::process::getuid().as_raw()
        || meta.mode() & 0o7777 != 0o700
    {
        return Err(Failure::new(
            ErrorCode::Denied,
            format!(
                "installation directory {} must be user-owned, plain and mode 0700",
                path.display()
            ),
        ));
    }
    Ok(())
}
fn sync(path: &Path) -> Result<(), Failure> {
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(io_error)
}
fn nonce() -> Result<String, Failure> {
    Ok(package::hex(&crate::secret::generate().map_err(io_error)?))
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<(), Failure> {
    let parent = path
        .parent()
        .ok_or_else(|| Failure::invalid("file has no parent"))?;
    let temporary = parent.join(format!(".write-{}", nonce()?));
    package::create_file(&temporary, bytes, 0o600)?;
    let result = fs::rename(&temporary, path)
        .map_err(io_error)
        .and_then(|()| sync(parent));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
fn lock(prefix: &Path) -> Result<File, Failure> {
    private_dir(prefix, true)?;
    let path = prefix.join("install.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)
        .map_err(io_error)?;
    let meta = file.metadata().map_err(io_error)?;
    if !meta.is_file()
        || meta.nlink() != 1
        || meta.uid() != rustix::process::getuid().as_raw()
        || meta.mode() & 0o7777 != 0o600
    {
        return Err(Failure::new(
            ErrorCode::Denied,
            "installation lock must be a private owned regular file",
        ));
    }
    file.try_lock()
        .map_err(|_| conflict("another installation operation holds this prefix"))?;
    Ok(file)
}
fn policy(prefix: &Path) -> Result<Option<(Policy, String)>, Failure> {
    let path = prefix.join(POLICY);
    if !exists(&path)? {
        return Ok(None);
    }
    let bytes = package::read_regular(&path)?;
    let value: Policy =
        serde_json::from_slice(&bytes).map_err(|_| corrupt("invalid saved installation policy"))?;
    value.validate()?;
    Ok(Some((value, package::sha256(&bytes))))
}
fn current(prefix: &Path) -> Result<Option<String>, Failure> {
    let path = prefix.join("current");
    if !exists(&path)? {
        return Ok(None);
    }
    let link = fs::read_link(path)
        .map_err(|_| conflict("current is not an installer-owned release link"))?;
    let parts: Vec<_> = link.components().collect();
    if parts.len() != 2 || parts[0].as_os_str() != "releases" {
        return Err(conflict(
            "current does not name a local content-addressed release",
        ));
    }
    let hash = parts[1]
        .as_os_str()
        .to_str()
        .ok_or_else(|| corrupt("invalid release link"))?;
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(corrupt("invalid release link digest"));
    }
    Ok(Some(hash.into()))
}
fn release_manifest(
    prefix: &Path,
    hash: &str,
    key: &[u8; 32],
) -> Result<package::Manifest, Failure> {
    private_dir(&prefix.join("releases"), false)?;
    let root = prefix.join("releases").join(hash);
    private_dir(&root, false)?;
    let bytes = package::read_regular(&root.join(package::MANIFEST))?;
    if package::sha256(&bytes) != hash {
        return Err(corrupt("installed manifest does not match its directory"));
    }
    package::verify_signature(
        key,
        &bytes,
        &package::exact_file::<64>(&root.join(package::SIGNATURE))?,
    )?;
    package::validate_manifest(&bytes)
}
fn check_policy(old: Option<&Policy>, new: &Verified) -> Result<(), Failure> {
    if let Some(old) = old {
        if old.key != new.trust_key {
            return Err(Failure::new(
                ErrorCode::Denied,
                "installation trust key replacement requires a separate trust migration",
            ));
        }
        let known = old.validate()?;
        if new.withdrawals.sequence < known.sequence
            || (new.withdrawals.sequence == known.sequence
                && old.withdrawals != new.withdrawals_bytes)
        {
            return Err(Failure::new(
                ErrorCode::Denied,
                "withdrawal registry rollback or same-sequence replacement refused",
            ));
        }
        if known
            .withdrawn_manifest_sha256
            .iter()
            .any(|hash| !new.withdrawals.withdrawn_manifest_sha256.contains(hash))
        {
            return Err(Failure::new(
                ErrorCode::Denied,
                "a withdrawn manifest cannot be reinstated by dropping it from the registry",
            ));
        }
    }
    Ok(())
}
pub fn plan(prefix: &Path, value: &Verified, allow_downgrade: bool) -> Result<Plan, Failure> {
    let prefix = absolute(prefix)?;
    private_dir(&prefix, false)?;
    if value.manifest.target != package::host_target()?
        || value.manifest.api_version != u32::from(locust_proto::API_VERSION)
        || value.manifest.protocol_version != u32::from(locust_proto::PROTOCOL_VERSION)
    {
        return Err(Failure::new(
            ErrorCode::UnsupportedVersion,
            "candidate target or API/protocol differs from this installer",
        ));
    }
    let previous = policy(&prefix)?;
    check_policy(previous.as_ref().map(|p| &p.0), value)?;
    let active = current(&prefix)?;
    if let Some(hash) = &active {
        let trusted = previous
            .as_ref()
            .ok_or_else(|| corrupt("installed release has no trust state"))?;
        let old = release_manifest(&prefix, hash, &trusted.0.key)?;
        if !allow_downgrade
            && semver::Version::parse(&value.manifest.version).unwrap()
                < semver::Version::parse(&old.version).unwrap()
        {
            return Err(Failure::new(
                ErrorCode::Denied,
                "downgrade requires --allow-downgrade in the reviewed plan",
            ));
        }
    }
    let target_policy_sha256 = package::sha256(&encode(&Policy::from_verified(value))?);
    let unchanged = active.as_ref() == Some(&value.manifest_sha256)
        && previous.as_ref().map(|p| &p.1) == Some(&target_policy_sha256);
    Ok(Plan {
        format: "locust-install-plan-v1",
        executable: prefix.join("current/locust"),
        prefix,
        action: if unchanged { "unchanged" } else { "activate" },
        current_manifest: active,
        current_policy_sha256: previous.map(|p| p.1),
        target_manifest: Some(value.manifest_sha256.clone()),
        target_policy_sha256: Some(target_policy_sha256),
        allow_downgrade,
        daemon_data_changed: false,
    })
}
fn version_probe(
    executable: &Path,
    home: &Path,
    manifest: &package::Manifest,
) -> Result<(), Failure> {
    let output = Command::new(executable)
        .arg("--version")
        .env_clear()
        .env("HOME", home)
        .env("PATH", "/usr/bin:/bin")
        .output()
        .map_err(|e| Failure::unavailable(format!("verified candidate could not start: {e}")))?;
    let expected = format!(
        "locust {} ({}) api {} protocol {}\n",
        manifest.version,
        &manifest.source_commit[..12],
        manifest.api_version,
        manifest.protocol_version
    );
    if !output.status.success() || output.stdout != expected.as_bytes() || !output.stderr.is_empty()
    {
        return Err(corrupt(
            "verified executable version does not match its signed manifest",
        ));
    }
    Ok(())
}
pub fn apply(
    prefix: &Path,
    value: &Verified,
    allow_downgrade: bool,
    expected: &str,
) -> Result<Value, Failure> {
    apply_with_probe(prefix, value, allow_downgrade, expected, version_probe)
}
fn apply_with_probe(
    prefix: &Path,
    value: &Verified,
    allow_downgrade: bool,
    expected: &str,
    probe: impl FnOnce(&Path, &Path, &package::Manifest) -> Result<(), Failure>,
) -> Result<Value, Failure> {
    let proposed = plan(prefix, value, allow_downgrade)?;
    if proposed.digest()? != expected {
        return Err(conflict("installation plan changed; review a fresh plan"));
    }
    let prefix = &proposed.prefix;
    let _guard = lock(prefix)?;
    let checked = plan(prefix, value, allow_downgrade)?;
    if checked.digest()? != expected {
        return Err(conflict(
            "installation changed while acquiring its lock; review a fresh plan",
        ));
    }
    private_dir(&prefix.join("releases"), true)?;
    let destination = prefix.join("releases").join(&value.manifest_sha256);
    if exists(&destination)? {
        release_manifest(prefix, &value.manifest_sha256, &value.trust_key)?;
        package::verify_payloads(&destination, &value.manifest)?;
        // Even a completed staging directory from an interrupted install must
        // pass the executable identity check before it can become current.
        probe(&destination.join(package::BINARY), prefix, &value.manifest)?;
    } else {
        let staging = prefix.join(format!(".stage-{}", nonce()?));
        private_dir(&staging, true)?;
        // Failures deliberately leave a private, never-active staging directory
        // for inspection. Future attempts use a new random directory.
        for payload in &value.manifest.files {
            let target = staging.join(&payload.path);
            let parent = target.parent().expect("payload parent");
            private_dir(parent, true)?;
            let mut source = package::payload_file(&value.root, &payload.path)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(payload.mode)
                .open(&target)
                .map_err(io_error)?;
            output
                .set_permissions(fs::Permissions::from_mode(payload.mode))
                .map_err(io_error)?;
            io::copy(&mut source, &mut output).map_err(io_error)?;
            output.sync_all().map_err(io_error)?;
            sync(parent)?;
        }
        package::create_file(
            &staging.join(package::MANIFEST),
            &value.manifest_bytes,
            0o644,
        )?;
        package::create_file(&staging.join(package::SIGNATURE), &value.signature, 0o644)?;
        package::verify_payloads(&staging, &value.manifest)?;
        probe(&staging.join(package::BINARY), &staging, &value.manifest)?;
        fs::rename(&staging, &destination).map_err(io_error)?;
        sync(&prefix.join("releases"))?;
    }
    // Persist policy first: a crash can leave the old release selected, but can
    // never activate new code while losing a newer withdrawal watermark.
    atomic(
        &prefix.join(POLICY),
        &encode(&Policy::from_verified(value))?,
    )?;
    let temporary = prefix.join(format!(".current-{}", nonce()?));
    std::os::unix::fs::symlink(
        Path::new("releases").join(&value.manifest_sha256),
        &temporary,
    )
    .map_err(io_error)?;
    fs::rename(&temporary, prefix.join("current")).map_err(io_error)?;
    sync(prefix)?;
    Ok(
        json!({"installed":true,"changed":checked.action != "unchanged","executable":checked.executable,
        "manifest_sha256":value.manifest_sha256,"service_restart_required":checked.current_manifest.is_some() && checked.current_manifest.as_ref()!=Some(&value.manifest_sha256),"daemon_data_changed":false}),
    )
}
pub fn status(prefix: &Path) -> Result<Value, Failure> {
    let prefix = absolute(prefix)?;
    private_dir(&prefix, false)?;
    let saved = policy(&prefix)?;
    let active = current(&prefix)?;
    let mut manifest = None;
    let mut withdrawn = false;
    if let Some(hash) = &active {
        let policy = saved
            .as_ref()
            .ok_or_else(|| corrupt("installed release has no trust state"))?;
        let value = release_manifest(&prefix, hash, &policy.0.key)?;
        package::verify_payloads(&prefix.join("releases").join(hash), &value)?;
        withdrawn = policy
            .0
            .validate()?
            .withdrawn_manifest_sha256
            .contains(hash);
        manifest = Some(value);
    }
    Ok(
        json!({"prefix":prefix,"installed":active.is_some(),"manifest_sha256":active,"manifest":manifest,"withdrawn":withdrawn,
        "withdrawals_sequence": saved.as_ref().map(|p| p.0.validate().map(|v| v.sequence)).transpose()?,
        "executable":prefix.join("current/locust"),"daemon_data_changed":false}),
    )
}
pub fn uninstall_plan(prefix: &Path) -> Result<Plan, Failure> {
    let prefix = absolute(prefix)?;
    private_dir(&prefix, false)?;
    let active = current(&prefix)?;
    let saved = policy(&prefix)?;
    if let Some(hash) = &active {
        release_manifest(
            &prefix,
            hash,
            &saved
                .as_ref()
                .ok_or_else(|| corrupt("installed release has no trust state"))?
                .0
                .key,
        )?;
    }
    Ok(Plan {
        format: "locust-install-plan-v1",
        executable: prefix.join("current/locust"),
        prefix,
        action: "uninstall",
        current_manifest: active,
        current_policy_sha256: saved.map(|p| p.1),
        target_manifest: None,
        target_policy_sha256: None,
        allow_downgrade: false,
        daemon_data_changed: false,
    })
}
pub fn uninstall(prefix: &Path, expected: &str) -> Result<Value, Failure> {
    let proposed = uninstall_plan(prefix)?;
    if proposed.digest()? != expected {
        return Err(conflict("uninstall plan changed; review a fresh plan"));
    }
    let prefix = &proposed.prefix;
    let _guard = lock(prefix)?;
    if uninstall_plan(prefix)?.digest()? != expected {
        return Err(conflict("installation changed while acquiring its lock"));
    }
    let mut retained = Vec::new();
    if proposed.current_manifest.is_some() {
        fs::remove_file(prefix.join("current")).map_err(io_error)?;
        sync(prefix)?;
    }
    if let Some((policy, _)) = policy(prefix)? {
        let releases = prefix.join("releases");
        private_dir(&releases, false)?;
        if exists(&releases)? {
            for entry in fs::read_dir(&releases).map_err(io_error)? {
                let entry = entry.map_err(io_error)?;
                let path = entry.path();
                let hash = entry.file_name().to_string_lossy().into_owned();
                let Ok(manifest) = release_manifest(prefix, &hash, &policy.key) else {
                    retained.push(path);
                    continue;
                };
                if package::verify_payloads(&path, &manifest).is_err() {
                    retained.push(path);
                    continue;
                }
                for file in [
                    package::BINARY,
                    package::SKILL,
                    package::MANUAL,
                    package::MANIFEST,
                    package::SIGNATURE,
                ] {
                    fs::remove_file(path.join(file)).map_err(io_error)?;
                }
                for subdir in ["skills/locust", "skills"] {
                    let _ = fs::remove_dir(path.join(subdir));
                }
                if fs::remove_dir(&path).is_err() {
                    retained.push(path);
                }
            }
            sync(&releases)?;
        }
    }
    Ok(
        json!({"uninstalled":true,"retained_modified_or_unknown":retained,"trust_state_retained":true,"daemon_data_changed":false,
        "service_and_client_configuration_removed":false}),
    )
}

#[cfg(test)]
mod tests;
