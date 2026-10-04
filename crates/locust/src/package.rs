//! Local release verification. The caller supplies an independently trusted
//! Ed25519 public key; nothing in a downloaded bundle can select its own trust.
//! Signatures cover exact manifest/withdrawal bytes, including their format tag.
use crate::failure::Failure;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use locust_proto::api::ErrorCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

pub const MANIFEST: &str = "manifest.json";
pub const SIGNATURE: &str = "manifest.sig";
pub const BINARY: &str = "locust";
pub const MANUAL: &str = "manual.tar";
pub const SKILL: &str = "skills/locust/SKILL.md";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: String,
    pub source_commit: String,
    pub version: String,
    pub target: String,
    pub machine_format: String,
    pub api_version: u32,
    pub protocol_version: u32,
    pub toolchain: String,
    pub files: Vec<Payload>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Payload {
    pub path: String,
    pub sha256: String,
    pub size: u64,
    pub mode: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Withdrawals {
    pub format: String,
    pub sequence: u64,
    pub withdrawn_manifest_sha256: Vec<String>,
}

/// Verified metadata, not a claim that the caller's source directory is immutable.
/// An installer must rehash its private copied files before activating them.
pub struct Verified {
    pub root: PathBuf,
    pub manifest: Manifest,
    pub manifest_bytes: Vec<u8>,
    pub signature: [u8; 64],
    pub manifest_sha256: String,
    pub trust_key: [u8; 32],
    pub withdrawals: Withdrawals,
    pub withdrawals_sha256: String,
    pub withdrawals_bytes: Vec<u8>,
    pub withdrawals_signature: [u8; 64],
}

fn invalid(message: &str) -> Failure {
    Failure::new(ErrorCode::Corrupted, message)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn is_hex(text: &str, lengths: &[usize]) -> bool {
    lengths.contains(&text.len())
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn host_target() -> Result<&'static str, Failure> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
        _ => Err(Failure::new(
            ErrorCode::UnsupportedVersion,
            "installation is supported on macOS arm64 and Linux x86_64",
        )),
    }
}

/// Open one regular, singly linked file, refusing a symlink at its leaf.
pub fn regular(path: &Path) -> Result<File, Failure> {
    let file = OpenOptions::new()
        .read(true)
        // A FIFO must not block before fstat can reject it. O_NONBLOCK has
        // no effect on regular files, whose type and link count we check below.
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)
        .map_err(|e| {
            Failure::invalid(format!("cannot open regular file {}: {e}", path.display()))
        })?;
    let meta = file
        .metadata()
        .map_err(|_| invalid("cannot inspect package file"))?;
    if !meta.is_file() || meta.nlink() != 1 {
        return Err(invalid(
            "package input must be a singly linked regular file",
        ));
    }
    Ok(file)
}

/// Every component below the selected package root must be a plain directory.
pub fn payload_file(root: &Path, relative: &str) -> Result<File, Failure> {
    let components = Path::new(relative).components().collect::<Vec<_>>();
    if components.is_empty()
        || components
            .iter()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(invalid("package path is not a relative file path"));
    }
    let mut path = root.to_path_buf();
    for component in &components[..components.len() - 1] {
        path.push(component.as_os_str());
        let meta =
            fs::symlink_metadata(&path).map_err(|_| invalid("package directory is absent"))?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(invalid("package directory must not be a symlink"));
        }
    }
    path.push(components.last().expect("nonempty").as_os_str());
    regular(&path)
}

pub fn read_regular(path: &Path) -> Result<Vec<u8>, Failure> {
    let mut bytes = Vec::new();
    regular(path)?
        .read_to_end(&mut bytes)
        .map_err(|_| invalid("cannot read package metadata"))?;
    Ok(bytes)
}

pub fn exact_file<const N: usize>(path: &Path) -> Result<[u8; N], Failure> {
    exact_bytes(regular(path)?)
}

fn exact_bytes<const N: usize>(mut reader: impl Read) -> Result<[u8; N], Failure> {
    let mut bytes = [0; N];
    reader
        .read_exact(&mut bytes)
        .map_err(|_| invalid("key or signature has the wrong byte length"))?;
    let mut trailing = [0; 1];
    if reader
        .read(&mut trailing)
        .map_err(|_| invalid("cannot read key or signature"))?
        != 0
    {
        return Err(invalid("key or signature has the wrong byte length"));
    }
    Ok(bytes)
}

pub fn validate_manifest(bytes: &[u8]) -> Result<Manifest, Failure> {
    let manifest: Manifest =
        serde_json::from_slice(bytes).map_err(|_| invalid("invalid release manifest"))?;
    if manifest.format != "locust-release-v2"
        || !is_hex(&manifest.source_commit, &[40, 64])
        || semver::Version::parse(&manifest.version).is_err()
        || manifest.toolchain.is_empty()
    {
        return Err(invalid(
            "invalid release format, version or source identity",
        ));
    }
    let format = match manifest.target.as_str() {
        "aarch64-apple-darwin" => "mach-o-arm64",
        "x86_64-unknown-linux-gnu" => "elf-x86_64",
        _ => {
            return Err(Failure::new(
                ErrorCode::UnsupportedVersion,
                "unsupported release target",
            ));
        }
    };
    if manifest.machine_format != format || manifest.files.len() != 3 {
        return Err(invalid("unexpected package format or payload set"));
    }
    let mut paths = BTreeSet::new();
    for file in &manifest.files {
        let mode = match file.path.as_str() {
            BINARY => 0o755,
            SKILL | MANUAL => 0o644,
            _ => return Err(invalid("manifest contains an unsupported payload path")),
        };
        if file.mode != mode
            || !is_hex(&file.sha256, &[64])
            || file.size == 0
            || !paths.insert(file.path.clone())
        {
            return Err(invalid("invalid or duplicate release payload"));
        }
    }
    Ok(manifest)
}

pub fn validate_withdrawals(bytes: &[u8]) -> Result<Withdrawals, Failure> {
    let list: Withdrawals =
        serde_json::from_slice(bytes).map_err(|_| invalid("invalid withdrawal registry"))?;
    let mut unique = BTreeSet::new();
    if list.format != "locust-withdrawals-v1"
        || list
            .withdrawn_manifest_sha256
            .iter()
            .any(|digest| !is_hex(digest, &[64]) || !unique.insert(digest))
    {
        return Err(invalid("invalid withdrawal registry format or digest"));
    }
    Ok(list)
}

pub fn verify_signature(key: &[u8; 32], bytes: &[u8], signature: &[u8; 64]) -> Result<(), Failure> {
    VerifyingKey::from_bytes(key)
        .map_err(|_| invalid("invalid Ed25519 trust key"))?
        .verify_strict(bytes, &Signature::from_bytes(signature))
        .map_err(|_| {
            Failure::new(
                ErrorCode::Denied,
                "release signature does not match the selected trust key",
            )
        })
}

pub fn hash_file(file: &mut File) -> Result<(String, u64), Failure> {
    file.seek(SeekFrom::Start(0))
        .map_err(|_| invalid("cannot seek package payload"))?;
    let mut hasher = Sha256::new();
    let mut bytes = [0; 64 * 1024];
    let mut size = 0u64;
    loop {
        let count = file
            .read(&mut bytes)
            .map_err(|_| invalid("cannot read package payload"))?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .ok_or_else(|| invalid("payload length overflow"))?;
        hasher.update(&bytes[..count]);
    }
    Ok((hex(&hasher.finalize()), size))
}

pub fn verify_payloads(root: &Path, manifest: &Manifest) -> Result<(), Failure> {
    let meta = fs::symlink_metadata(root).map_err(|_| invalid("package root is absent"))?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(invalid("package root must be a plain directory"));
    }
    for payload in &manifest.files {
        let mut file = payload_file(root, &payload.path)?;
        let mode = file
            .metadata()
            .map_err(|_| invalid("cannot inspect payload"))?
            .permissions()
            .mode()
            & 0o7777;
        let (hash, size) = hash_file(&mut file)?;
        if hash != payload.sha256 || size != payload.size || mode != payload.mode {
            return Err(invalid(
                "release payload hash, length or mode does not match manifest",
            ));
        }
        if payload.path == BINARY {
            file.seek(SeekFrom::Start(0))
                .map_err(|_| invalid("cannot seek executable"))?;
            let mut header = [0u8; 64];
            let count = file
                .read(&mut header)
                .map_err(|_| invalid("cannot inspect executable"))?;
            let matches = match manifest.target.as_str() {
                "aarch64-apple-darwin" => {
                    count >= 32
                        && header[..4] == [0xcf, 0xfa, 0xed, 0xfe]
                        && header[4..8] == 0x0100000cu32.to_le_bytes()
                }
                "x86_64-unknown-linux-gnu" => {
                    count >= 64
                        && header[..6] == [0x7f, b'E', b'L', b'F', 2, 1]
                        && header[18..20] == 62u16.to_le_bytes()
                }
                _ => false,
            };
            if !matches {
                return Err(invalid(
                    "executable machine format does not match manifest target",
                ));
            }
        }
    }
    Ok(())
}

pub fn verify(root: &Path, trust_key: &Path, withdrawals: &Path) -> Result<Verified, Failure> {
    let key = exact_file::<32>(trust_key)?;
    let bytes = read_regular(&root.join(MANIFEST))?;
    let signature = exact_file::<64>(&root.join(SIGNATURE))?;
    verify_signature(&key, &bytes, &signature)?;
    let manifest = validate_manifest(&bytes)?;
    let registry = read_regular(withdrawals)?;
    let registry_sig = exact_file::<64>(&signature_path(withdrawals))?;
    verify_signature(&key, &registry, &registry_sig)?;
    let registry_value = validate_withdrawals(&registry)?;
    let digest = sha256(&bytes);
    if registry_value.withdrawn_manifest_sha256.contains(&digest) {
        return Err(Failure::new(
            ErrorCode::Denied,
            "this release manifest is withdrawn",
        ));
    }
    verify_payloads(root, &manifest)?;
    Ok(Verified {
        root: root.to_path_buf(),
        manifest,
        manifest_bytes: bytes,
        signature,
        manifest_sha256: digest,
        trust_key: key,
        withdrawals: registry_value,
        withdrawals_sha256: sha256(&registry),
        withdrawals_bytes: registry,
        withdrawals_signature: registry_sig,
    })
}

pub fn signature_path(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(".sig");
    value.into()
}

pub fn create_file(path: &Path, bytes: &[u8], mode: u32) -> Result<(), Failure> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)
        .map_err(|e| Failure::invalid(format!("cannot create {}: {e}", path.display())))?;
    file.set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|_| Failure::internal("cannot set file mode"))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| Failure::internal("cannot persist release file"))?;
    File::open(
        path.parent()
            .ok_or_else(|| Failure::invalid("file has no parent"))?,
    )
    .and_then(|f| f.sync_all())
    .map_err(|_| Failure::internal("cannot persist release directory"))?;
    Ok(())
}

pub fn keygen(secret: &Path, public: &Path) -> Result<(), Failure> {
    if secret == public || secret.exists() || public.exists() {
        return Err(Failure::new(
            ErrorCode::Conflict,
            "key output paths must be distinct and absent",
        ));
    }
    let seed =
        crate::secret::generate().map_err(|_| Failure::internal("cannot generate signing key"))?;
    let key = SigningKey::from_bytes(&seed);
    create_file(secret, &seed, 0o600)?;
    create_file(public, &key.verifying_key().to_bytes(), 0o644)
}

fn sign_bytes(bytes: &[u8], secret: &Path, output: &Path) -> Result<(), Failure> {
    let file = regular(secret)?;
    let meta = file
        .metadata()
        .map_err(|_| invalid("cannot inspect signing key"))?;
    if meta.permissions().mode() & 0o7777 != 0o600
        || meta.uid() != rustix::process::getuid().as_raw()
    {
        return Err(Failure::new(
            ErrorCode::Denied,
            "signing key must be owned by this user with mode 0600",
        ));
    }
    let seed = exact_bytes::<32>(file)?;
    let signature = SigningKey::from_bytes(&seed).sign(bytes).to_bytes();
    create_file(output, &signature, 0o644)
}

pub fn sign_release(root: &Path, secret: &Path) -> Result<(), Failure> {
    let bytes = read_regular(&root.join(MANIFEST))?;
    let manifest = validate_manifest(&bytes)?;
    verify_payloads(root, &manifest)?;
    sign_bytes(&bytes, secret, &root.join(SIGNATURE))
}

pub fn sign_withdrawals(path: &Path, secret: &Path) -> Result<(), Failure> {
    let bytes = read_regular(path)?;
    validate_withdrawals(&bytes)?;
    sign_bytes(&bytes, secret, &signature_path(path))
}

#[cfg(test)]
mod tests;
