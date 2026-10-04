//! Ownership and recovery for one native user service.
//!
//! A private intent records exact, nonce-marked unit bytes before `create_new`.
//! It can be resumed after a crash without adopting an existing unowned file.
//! Control is separate from writing the unit, and removal requires an observed
//! stopped user service. No operation silently replaces a changed unit.

use super::service::{ServiceAction, ServiceKind, ServiceSpec, ServiceState};
use super::{absolute, atomic, io_error, lock, private_dir, sync};
use crate::{failure::Failure, package};
use locust_proto::api::ErrorCode;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

const FORMAT: &str = "locust-service-ownership-v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Intent,
    Owned,
    Removing,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    format: String,
    phase: Phase,
    label: String,
    unit_path: PathBuf,
    profile_home: PathBuf,
    executable: PathBuf,
    daemon_home: PathBuf,
    log_dir: PathBuf,
    nonce: String,
    unit_sha256: String,
    unit_bytes: Vec<u8>,
}

impl Record {
    fn new(spec: &ServiceSpec, nonce: String) -> Result<Self, Failure> {
        let unit_bytes = marked_unit(spec, &nonce)?;
        Ok(Self {
            format: FORMAT.into(),
            phase: Phase::Intent,
            label: spec.label().into(),
            unit_path: spec.unit_path().into(),
            profile_home: spec.profile_home().into(),
            executable: spec.executable().into(),
            daemon_home: spec.daemon_home().into(),
            log_dir: spec.log_dir().into(),
            nonce,
            unit_sha256: package::sha256(&unit_bytes),
            unit_bytes,
        })
    }

    fn validate(&self, spec: &ServiceSpec) -> Result<(), Failure> {
        if self.format != FORMAT
            || self.label != spec.label()
            || self.unit_path != spec.unit_path()
            || self.profile_home != spec.profile_home()
            || self.executable != spec.executable()
            || self.daemon_home != spec.daemon_home()
            || self.log_dir != spec.log_dir()
        {
            return Err(conflict(
                "service ownership record names another installation",
            ));
        }
        if self.nonce.len() != 64
            || !self
                .nonce
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || self.unit_sha256 != package::sha256(&self.unit_bytes)
            || self.unit_bytes != marked_unit(spec, &self.nonce)?
        {
            return Err(corrupt("service ownership record bytes are invalid"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ServicePlan {
    pub format: &'static str,
    pub operation: &'static str,
    pub prefix: PathBuf,
    pub label: String,
    pub unit_path: PathBuf,
    pub action: &'static str,
    pub rendered_sha256: String,
    pub record_sha256: Option<String>,
    pub record_phase: Option<&'static str>,
    pub observed_unit_state: &'static str,
    pub observed_unit_sha256: Option<String>,
}

impl ServicePlan {
    pub fn digest(&self) -> Result<String, Failure> {
        let bytes = serde_json::to_vec(self)
            .map_err(|_| Failure::internal("cannot encode service plan"))?;
        Ok(package::sha256(&bytes))
    }

    pub fn json(&self) -> Result<Value, Failure> {
        Ok(json!({"plan":self,"plan_sha256":self.digest()?}))
    }
}

fn conflict(message: &str) -> Failure {
    Failure::new(ErrorCode::Conflict, message)
}

fn corrupt(message: &str) -> Failure {
    Failure::new(ErrorCode::Corrupted, message)
}

fn record_path(prefix: &Path, spec: &ServiceSpec) -> PathBuf {
    prefix.join(format!("service-{}.json", spec.label()))
}

fn marker(spec: &ServiceSpec) -> &'static str {
    match spec.kind() {
        ServiceKind::None => "",
        ServiceKind::Launchd => "<!-- locust-install-id:",
        ServiceKind::Systemd => "# locust-install-id:",
    }
}

fn marked_unit(spec: &ServiceSpec, nonce: &str) -> Result<Vec<u8>, Failure> {
    let base = spec.render()?;
    if spec.kind() == ServiceKind::None {
        return Ok(base);
    }
    let mut result = Vec::with_capacity(base.len() + nonce.len() + 40);
    let comment = format!(
        "{}{}{}\n",
        marker(spec),
        nonce,
        if spec.kind() == ServiceKind::Launchd {
            " -->"
        } else {
            ""
        }
    );
    if spec.kind() == ServiceKind::Launchd {
        let first_line = base
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or_else(|| Failure::internal("launchd renderer omitted XML declaration"))?
            + 1;
        result.extend_from_slice(&base[..first_line]);
        result.extend_from_slice(comment.as_bytes());
        result.extend_from_slice(&base[first_line..]);
    } else {
        result.extend_from_slice(comment.as_bytes());
        result.extend_from_slice(&base);
    }
    Ok(result)
}

fn existing(path: &Path) -> Result<bool, Failure> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(error)),
    }
}

fn read_record(prefix: &Path, spec: &ServiceSpec) -> Result<Option<(Record, String)>, Failure> {
    let path = record_path(prefix, spec);
    if !existing(&path)? {
        return Ok(None);
    }
    let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != rustix::process::getuid().as_raw()
        || metadata.mode() & 0o7777 != 0o600
    {
        return Err(corrupt(
            "service ownership record must be a private regular file",
        ));
    }
    let bytes = package::read_regular(&path)?;
    let record: Record = serde_json::from_slice(&bytes)
        .map_err(|_| corrupt("service ownership record cannot be decoded"))?;
    record.validate(spec)?;
    Ok(Some((record, package::sha256(&bytes))))
}

enum UnitState {
    Absent,
    Regular(Vec<u8>),
    Other,
}

fn unit_state(path: &Path) -> Result<UnitState, Failure> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(UnitState::Absent),
        Err(error) => return Err(io_error(error)),
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.uid() != rustix::process::getuid().as_raw()
        || metadata.mode() & 0o7777 != 0o600
    {
        return Ok(UnitState::Other);
    }
    Ok(UnitState::Regular(package::read_regular(path)?))
}

fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Intent => "intent",
        Phase::Owned => "owned",
        Phase::Removing => "removing",
    }
}

/// A read-only review of the private record and the current unit file.
/// `remove` is part of the plan digest; an install approval cannot remove it.
pub fn plan(prefix: &Path, spec: &ServiceSpec, remove: bool) -> Result<ServicePlan, Failure> {
    let prefix = absolute(prefix)?;
    private_dir(&prefix, false)?;
    let record = read_record(&prefix, spec)?;
    let unit = if spec.kind() == ServiceKind::None {
        UnitState::Absent
    } else {
        unit_state(spec.unit_path())?
    };
    let (state, unit_hash) = match &unit {
        UnitState::Absent => ("absent", None),
        UnitState::Regular(bytes) => ("regular", Some(package::sha256(bytes))),
        UnitState::Other => ("other", None),
    };
    let action = if spec.kind() == ServiceKind::None {
        "disabled"
    } else if remove {
        match (&record, &unit) {
            (None, UnitState::Absent) => "absent",
            (None, _) => "collision",
            (Some((record, _)), UnitState::Regular(bytes)) if *bytes == record.unit_bytes => {
                if record.phase == Phase::Removing {
                    "remove_recovery"
                } else {
                    "remove"
                }
            }
            (Some((record, _)), UnitState::Absent) if record.phase == Phase::Removing => {
                "remove_recovery"
            }
            (Some((record, _)), UnitState::Absent) if record.phase == Phase::Intent => "remove",
            (Some(_), _) => "modified",
        }
    } else {
        match (&record, &unit) {
            (None, UnitState::Absent) => "create",
            (None, _) => "collision",
            (Some((record, _)), UnitState::Regular(bytes)) if *bytes == record.unit_bytes => {
                match record.phase {
                    Phase::Intent => "recover",
                    Phase::Owned => "unchanged",
                    Phase::Removing => "remove_recovery",
                }
            }
            (Some((record, _)), UnitState::Absent) => match record.phase {
                Phase::Intent => "recover",
                Phase::Removing => "remove_recovery",
                Phase::Owned => "modified",
            },
            (Some(_), _) => "modified",
        }
    };
    let rendered_sha256 = package::sha256(&spec.render()?);
    Ok(ServicePlan {
        format: "locust-service-plan-v1",
        operation: if remove { "remove" } else { "install" },
        prefix,
        label: spec.label().into(),
        unit_path: spec.unit_path().into(),
        action,
        rendered_sha256,
        record_sha256: record.as_ref().map(|(_, hash)| hash.clone()),
        record_phase: record.as_ref().map(|(record, _)| phase_name(record.phase)),
        observed_unit_state: state,
        observed_unit_sha256: unit_hash,
    })
}

fn checked_plan(
    prefix: &Path,
    spec: &ServiceSpec,
    remove: bool,
    expected: &str,
) -> Result<ServicePlan, Failure> {
    let proposed = plan(prefix, spec, remove)?;
    if proposed.digest()? != expected {
        return Err(conflict("service plan changed; review a fresh plan"));
    }
    Ok(proposed)
}

fn profile_home(spec: &ServiceSpec) -> Result<&Path, Failure> {
    let levels = match spec.kind() {
        ServiceKind::None => return Err(Failure::invalid("disabled service has no profile path")),
        ServiceKind::Launchd => 3,
        ServiceKind::Systemd => 4,
    };
    spec.unit_path()
        .ancestors()
        .nth(levels)
        .ok_or_else(|| Failure::invalid("service unit path has no profile home"))
}

fn safe_directory(path: &Path, create: bool) -> Result<(), Failure> {
    if !existing(path)? {
        if !create {
            return Err(Failure::invalid("service unit directory is absent"));
        }
        let parent = path
            .parent()
            .ok_or_else(|| Failure::invalid("service unit directory has no parent"))?;
        safe_directory(parent, true)?;
        let mut builder = DirBuilder::new();
        builder.mode(0o700);
        match builder.create(path) {
            Ok(()) => sync(parent)?,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io_error(error)),
        }
    }
    let meta = fs::symlink_metadata(path).map_err(io_error)?;
    if !meta.is_dir()
        || meta.file_type().is_symlink()
        || meta.uid() != rustix::process::getuid().as_raw()
        || meta.mode() & 0o022 != 0
    {
        return Err(Failure::new(
            ErrorCode::Denied,
            "service unit path must stay in user-owned directories without group/other write",
        ));
    }
    Ok(())
}

fn prepare_unit_parent(spec: &ServiceSpec) -> Result<(), Failure> {
    let profile = profile_home(spec)?;
    safe_directory(profile, false)?;
    let relative = spec
        .unit_path()
        .parent()
        .and_then(|parent| parent.strip_prefix(profile).ok())
        .ok_or_else(|| Failure::invalid("service unit is outside the selected profile"))?;
    let mut current = profile.to_path_buf();
    for component in relative.components() {
        current.push(component);
        safe_directory(&current, true)?;
    }
    Ok(())
}

fn prepare_logs(spec: &ServiceSpec) -> Result<(), Failure> {
    safe_directory(spec.log_dir(), true)?;
    let meta = fs::symlink_metadata(spec.log_dir()).map_err(io_error)?;
    if meta.mode() & 0o7777 != 0o700 {
        return Err(Failure::new(
            ErrorCode::Denied,
            "service log directory must be private mode 0700",
        ));
    }
    Ok(())
}

fn create_unit(path: &Path, bytes: &[u8]) -> Result<(), Failure> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                conflict("service unit path is already occupied")
            } else {
                io_error(error)
            }
        })?;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(io_error)?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(io_error)?;
    sync(path.parent().expect("unit has a parent"))
}

fn write_record_new(path: &Path, record: &Record) -> Result<(), Failure> {
    let bytes = serde_json::to_vec(record)
        .map_err(|_| Failure::internal("cannot encode service ownership intent"))?;
    package::create_file(path, &bytes, 0o600)?;
    sync(path.parent().expect("record has a parent"))
}

fn save_record(path: &Path, record: &Record) -> Result<(), Failure> {
    let bytes = serde_json::to_vec(record)
        .map_err(|_| Failure::internal("cannot encode service ownership record"))?;
    atomic(path, &bytes)
}

/// Commit a new unit or finish an interrupted intent. Activation is separate.
pub fn apply(prefix: &Path, spec: &ServiceSpec, expected: &str) -> Result<Value, Failure> {
    let proposed = checked_plan(prefix, spec, false, expected)?;
    if proposed.action == "disabled" {
        return Ok(json!({"installed":false,"service_kind":"none"}));
    }
    if !matches!(proposed.action, "create" | "recover" | "unchanged") {
        return Err(conflict(
            "service unit is not available for this installation",
        ));
    }
    let _guard = lock(&proposed.prefix)?;
    let checked = checked_plan(&proposed.prefix, spec, false, expected)?;
    if checked.action == "unchanged" {
        return Ok(
            json!({"installed":true,"changed":false,"unit":spec.unit_path(),"label":spec.label()}),
        );
    }
    prepare_unit_parent(spec)?;
    prepare_logs(spec)?;
    let record_file = record_path(&proposed.prefix, spec);
    let mut record = match read_record(&proposed.prefix, spec)? {
        Some((record, _)) => record,
        None => {
            let nonce = package::hex(&crate::secret::generate().map_err(io_error)?);
            let record = Record::new(spec, nonce)?;
            write_record_new(&record_file, &record)?;
            record
        }
    };
    match unit_state(spec.unit_path())? {
        UnitState::Absent => create_unit(spec.unit_path(), &record.unit_bytes)?,
        UnitState::Regular(bytes) if bytes == record.unit_bytes => {}
        _ => return Err(conflict("service unit changed while installing")),
    }
    record.phase = Phase::Owned;
    save_record(&record_file, &record)?;
    Ok(
        json!({"installed":true,"changed":true,"unit":spec.unit_path(),"label":spec.label(),"unit_sha256":record.unit_sha256}),
    )
}

/// Filesystem ownership and observed manager state for an unchanged owned unit.
pub fn status(prefix: &Path, spec: &ServiceSpec) -> Result<Value, Failure> {
    let proposed = plan(prefix, spec, false)?;
    let state = if spec.kind() == ServiceKind::None {
        "disabled"
    } else if proposed.action == "unchanged" {
        match spec.run_control(ServiceAction::Status, true)? {
            ServiceState::Disabled => "disabled",
            ServiceState::Running => "running",
            ServiceState::Loaded => "loaded",
            ServiceState::Stopped => "stopped",
        }
    } else {
        "unconfigured"
    };
    Ok(
        json!({"plan":proposed,"plan_sha256":proposed.digest()?,"state":state,"daemon_api_readiness_observed":false}),
    )
}

/// Start, stop, or inspect only the exact unchanged owned unit.
pub fn control(
    prefix: &Path,
    spec: &ServiceSpec,
    action: ServiceAction,
) -> Result<ServiceState, Failure> {
    if spec.kind() == ServiceKind::None {
        return Ok(ServiceState::Disabled);
    }
    let prefix = absolute(prefix)?;
    let _guard = lock(&prefix)?;
    let proposed = plan(&prefix, spec, false)?;
    if action != ServiceAction::Status && proposed.action != "unchanged" {
        return Err(conflict("service control requires an unchanged owned unit"));
    }
    if action == ServiceAction::Start {
        let installed = super::status(&prefix)?;
        if installed["installed"] != true || installed["withdrawn"] != false {
            return Err(Failure::new(
                ErrorCode::Denied,
                "service start requires an installed, verified, non-withdrawn release",
            ));
        }
    }
    if action == ServiceAction::Start {
        prepare_logs(spec)?;
    }
    spec.run_control(action, proposed.action == "unchanged")
}

/// Remove only an unchanged owned unit, after an explicit stopped-state check.
pub fn remove(prefix: &Path, spec: &ServiceSpec, expected: &str) -> Result<Value, Failure> {
    remove_with_status(prefix, spec, expected, || {
        spec.run_control(ServiceAction::Status, false)
    })
}

fn remove_with_status(
    prefix: &Path,
    spec: &ServiceSpec,
    expected: &str,
    mut manager_status: impl FnMut() -> Result<ServiceState, Failure>,
) -> Result<Value, Failure> {
    let proposed = checked_plan(prefix, spec, true, expected)?;
    if proposed.action == "disabled" {
        return Ok(json!({"removed":false,"service_kind":"none"}));
    }
    if proposed.action == "absent" {
        return Ok(json!({"removed":false,"unit":spec.unit_path()}));
    }
    if !matches!(proposed.action, "remove" | "remove_recovery") {
        return Err(conflict(
            "cannot remove an unowned or modified service unit",
        ));
    }
    let _guard = lock(&proposed.prefix)?;
    let checked = checked_plan(&proposed.prefix, spec, true, expected)?;
    if manager_status()? != ServiceState::Stopped {
        return Err(conflict("stop the owned service before removing its unit"));
    }
    let record_file = record_path(&checked.prefix, spec);
    let (mut record, _) = read_record(&checked.prefix, spec)?
        .ok_or_else(|| conflict("service ownership record disappeared"))?;
    record.phase = Phase::Removing;
    save_record(&record_file, &record)?;
    match unit_state(spec.unit_path())? {
        UnitState::Regular(bytes) if bytes == record.unit_bytes => {
            fs::remove_file(spec.unit_path()).map_err(io_error)?;
            sync(spec.unit_path().parent().expect("unit has a parent"))?;
        }
        UnitState::Absent => {}
        _ => return Err(conflict("service unit changed while removing")),
    }
    fs::remove_file(&record_file).map_err(io_error)?;
    sync(&checked.prefix)?;
    Ok(json!({"removed":true,"unit":spec.unit_path(),"label":spec.label()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, ServiceSpec) {
        let temporary = TempDir::new().unwrap();
        let profile = temporary.path().join("profile");
        fs::create_dir(&profile).unwrap();
        fs::set_permissions(&profile, fs::Permissions::from_mode(0o700)).unwrap();
        let spec = ServiceSpec::new_for_host(
            ServiceKind::Launchd,
            &profile,
            &temporary.path().join("installed/locust"),
            &temporary.path().join("daemon home"),
            &temporary.path().join("logs"),
            "macos",
            "aarch64",
        )
        .unwrap();
        (temporary, spec)
    }

    #[test]
    fn plan_is_read_only_and_unowned_collision_is_refused() {
        let (temp, spec) = fixture();
        let prefix = temp.path().join("private");
        let first = plan(&prefix, &spec, false).unwrap();
        assert_eq!(first.action, "create");
        assert!(!prefix.exists());
        assert!(!spec.unit_path().exists());
        fs::create_dir_all(spec.unit_path().parent().unwrap()).unwrap();
        fs::write(spec.unit_path(), spec.render().unwrap()).unwrap();
        let collision = plan(&prefix, &spec, false).unwrap();
        assert_eq!(collision.action, "collision");
        assert!(apply(&prefix, &spec, &collision.digest().unwrap()).is_err());
        assert!(spec.unit_path().exists());
    }

    #[test]
    fn status_skips_unconfigured_manager_and_control_respects_prefix_lock() {
        let (temp, spec) = fixture();
        let prefix = temp.path().join("private");
        assert_eq!(status(&prefix, &spec).unwrap()["state"], "unconfigured");
        assert!(!prefix.exists());
        let first = plan(&prefix, &spec, false).unwrap();
        apply(&prefix, &spec, &first.digest().unwrap()).unwrap();
        let _guard = lock(&prefix).unwrap();
        assert_eq!(
            control(&prefix, &spec, ServiceAction::Stop)
                .unwrap_err()
                .code,
            ErrorCode::Conflict
        );
    }

    #[test]
    fn install_repeat_and_removal_preserve_exact_ownership() {
        let (temp, spec) = fixture();
        let prefix = temp.path().join("private");
        let candidate = plan(&prefix, &spec, false).unwrap();
        apply(&prefix, &spec, &candidate.digest().unwrap()).unwrap();
        assert_eq!(
            fs::symlink_metadata(spec.log_dir()).unwrap().mode() & 0o7777,
            0o700
        );
        let owned = plan(&prefix, &spec, false).unwrap();
        assert_eq!(owned.action, "unchanged");
        assert!(owned.observed_unit_sha256.is_some());
        apply(&prefix, &spec, &owned.digest().unwrap()).unwrap();
        let removal = plan(&prefix, &spec, true).unwrap();
        assert_ne!(owned.digest().unwrap(), removal.digest().unwrap());
        let running = remove_with_status(&prefix, &spec, &removal.digest().unwrap(), || {
            Ok(ServiceState::Running)
        });
        assert_eq!(running.unwrap_err().code, ErrorCode::Conflict);
        assert!(spec.unit_path().exists());
        let removed = remove_with_status(&prefix, &spec, &removal.digest().unwrap(), || {
            Ok(ServiceState::Stopped)
        })
        .unwrap();
        assert_eq!(removed["removed"], true);
        assert!(!spec.unit_path().exists());
        assert_eq!(plan(&prefix, &spec, false).unwrap().action, "create");
    }

    #[test]
    fn interrupted_intent_recovers_only_its_nonce_marked_bytes() {
        let (temp, spec) = fixture();
        let prefix = temp.path().join("private");
        private_dir(&prefix, true).unwrap();
        prepare_unit_parent(&spec).unwrap();
        let intent = Record::new(&spec, "a".repeat(64)).unwrap();
        write_record_new(&record_path(&prefix, &spec), &intent).unwrap();
        let absent = plan(&prefix, &spec, false).unwrap();
        assert_eq!(absent.action, "recover");
        apply(&prefix, &spec, &absent.digest().unwrap()).unwrap();
        assert_eq!(plan(&prefix, &spec, false).unwrap().action, "unchanged");
        let bytes = fs::read(spec.unit_path()).unwrap();
        assert!(String::from_utf8(bytes).unwrap().contains(&"a".repeat(64)));
    }

    #[test]
    fn modified_owned_unit_is_never_replaced_or_removed() {
        let (temp, spec) = fixture();
        let prefix = temp.path().join("private");
        let first = plan(&prefix, &spec, false).unwrap();
        apply(&prefix, &spec, &first.digest().unwrap()).unwrap();
        fs::write(spec.unit_path(), b"user modified unit").unwrap();
        let changed = plan(&prefix, &spec, true).unwrap();
        assert_eq!(changed.action, "modified");
        assert!(apply(&prefix, &spec, &changed.digest().unwrap()).is_err());
        assert!(
            remove_with_status(&prefix, &spec, &changed.digest().unwrap(), || {
                Ok(ServiceState::Stopped)
            })
            .is_err()
        );
        assert_eq!(fs::read(spec.unit_path()).unwrap(), b"user modified unit");
    }

    #[test]
    fn removal_intent_can_finish_after_unit_was_deleted() {
        let (temp, spec) = fixture();
        let prefix = temp.path().join("private");
        let first = plan(&prefix, &spec, false).unwrap();
        apply(&prefix, &spec, &first.digest().unwrap()).unwrap();
        let (mut record, _) = read_record(&prefix, &spec).unwrap().unwrap();
        record.phase = Phase::Removing;
        save_record(&record_path(&prefix, &spec), &record).unwrap();
        fs::remove_file(spec.unit_path()).unwrap();
        let recovery = plan(&prefix, &spec, true).unwrap();
        assert_eq!(recovery.action, "remove_recovery");
        remove_with_status(&prefix, &spec, &recovery.digest().unwrap(), || {
            Ok(ServiceState::Stopped)
        })
        .unwrap();
        assert_eq!(plan(&prefix, &spec, false).unwrap().action, "create");
    }
}
