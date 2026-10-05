//! Locally requested client launch and conservative interruption recovery.
//!
//! The caller supplies explicit environment/configuration and persists reports
//! through the authenticated session API. No peer event initiates a launch.
//! Child ownership is retained only in this process; persisted PIDs never
//! authorize signaling or respawning. Output observations are structured JSON,
//! and neither prompts, environment values nor transcripts enter the record.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use locust_proto::api::{Claim, SessionCapabilities, SessionRecord, SessionState, SessionView};
use locust_proto::id::{GoalId, InstanceId, PublicKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{self, Client, ConfigFileOverlay, StdioServer};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub instance: InstanceId,
    pub principal: PublicKey,
    pub goal: Option<GoalId>,
    pub attempt: Option<AttemptBinding>,
    pub claim: Option<Claim>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptBinding {
    pub task: locust_proto::event::TaskId,
    pub attempt: locust_proto::id::EventId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathRefs {
    pub home: PathBuf,
    pub credential: PathBuf,
    pub session: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessEvidence {
    /// Historical correlation only. Never use this PID to signal on recovery.
    pub pid: u32,
    pub started_ms: u64,
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub termination_signal: Option<i32>,
    pub exited: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    pub schema: u32,
    pub launch_id: String,
    pub binding: Binding,
    pub executable: PathBuf,
    pub workspace: PathBuf,
    pub profile: PathBuf,
    pub bridge: PathRefs,
    pub process: Option<ProcessEvidence>,
    pub initialized: bool,
    pub tools_ready: bool,
    pub resume_from: Option<String>,
    pub native_session_file: Option<PathBuf>,
    pub lifecycle_receipt: PathBuf,
    /// Adapter delivery receipts share the existing operational session record.
    #[serde(default)]
    pub delivery: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    New,
    Resume(String),
}

/// Constructed only for an explicit local invocation. Permission/authentication
/// flags are the participant's arguments, never synthesized by this adapter.
pub struct LaunchSpec {
    pub client: Client,
    pub version: String,
    pub executable: PathBuf,
    pub workspace: PathBuf,
    pub profile: PathBuf,
    pub bridge: StdioServer,
    pub launch_id: String,
    pub binding: Binding,
    pub mode: Mode,
    pub arguments: Vec<OsString>,
    pub global_arguments: Vec<OsString>,
    pub environment: BTreeMap<OsString, OsString>,
    pub prompt: String,
    /// Pi selects a native session file for new sessions and explicit resume.
    pub pi_session: Option<PathBuf>,
    /// Fresh protected receipt file for this launch, applied to MCP argv by caller.
    pub lifecycle_receipt: PathBuf,
}

pub struct LaunchPlan {
    spec: LaunchSpec,
    pub arguments: Vec<OsString>,
    pub configuration: Option<ConfigFileOverlay>,
}

#[derive(Debug)]
pub struct Error(String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
impl Error {
    pub fn report_failed() -> Self {
        invalid("durable session report failed")
    }
}
fn invalid(message: &str) -> Error {
    Error(message.into())
}

fn client_name(client: Client) -> &'static str {
    match client {
        Client::Codex => "codex",
        Client::ClaudeCode => "claude-code",
        Client::FactoryDroid => "factory-droid",
        Client::KimiCode => "kimi-code",
        Client::Pi => "pi",
    }
}

/// Prepare argv and a merge proposal. The caller applies file proposals with
/// baseline/concurrent-writer protection before invoking `launch`.
pub fn prepare(
    spec: LaunchSpec,
    occupied: &[String],
    baseline: &Value,
) -> Result<LaunchPlan, Error> {
    if !spec.executable.is_absolute()
        || !spec.workspace.is_absolute()
        || !spec.profile.is_absolute()
        || !spec.lifecycle_receipt.is_absolute()
        || spec.launch_id.is_empty()
        || spec.version.is_empty()
    {
        return Err(invalid(
            "launch requires explicit absolute paths, identifier and client version",
        ));
    }
    if spec.binding.claim.is_some() && spec.binding.attempt.is_none() {
        return Err(invalid(
            "claimed launch requires exact attempt and attempt binding",
        ));
    }
    if let Some(claim) = spec.binding.claim
        && (Some(claim.goal) != spec.binding.goal
            || claim.instance != spec.binding.instance
            || claim.generation == 0)
    {
        return Err(invalid("claim does not match launch binding"));
    }
    if spec
        .arguments
        .iter()
        .chain(&spec.global_arguments)
        .any(|arg| {
            let arg = arg.to_string_lossy();
            arg == "--"
                || arg == "resume"
                || arg == "--resume"
                || arg.starts_with("--resume=")
                || arg == "--session-id"
                || arg.starts_with("--session-id=")
                || arg == "--session"
                || arg.starts_with("--session=")
                || arg == "--continue"
                || arg.starts_with("--continue=")
                || arg == "--fork-session"
                || arg == "--fork"
                || (spec.client == Client::KimiCode && (arg.starts_with("-S") || arg == "-c"))
                || (matches!(spec.client, Client::ClaudeCode | Client::Pi)
                    && (arg == "-c" || arg == "-r"))
        })
    {
        return Err(invalid(
            "native resume/session arguments belong in explicit launch mode",
        ));
    }
    if let Some(attempt) = &spec.binding.attempt {
        if spec.binding.goal.is_none() {
            return Err(invalid("attempt binding requires a goal"));
        }
        if spec
            .binding
            .claim
            .is_some_and(|claim| claim.task != attempt.task || claim.attempt != attempt.attempt)
        {
            return Err(invalid("claim differs from bound attempt"));
        }
    }
    let mut argv = spec.global_arguments.clone();
    let mut configuration = None;
    match spec.client {
        Client::Codex | Client::ClaudeCode => argv.extend(
            config::mcp_arguments(spec.client, config::SERVER_NAME, &spec.bridge, occupied)
                .map_err(|e| Error(e.to_string()))?,
        ),
        Client::FactoryDroid | Client::KimiCode | Client::Pi => {
            configuration = Some(
                config::mcp_file_overlay(
                    spec.client,
                    config::SERVER_NAME,
                    &spec.bridge,
                    occupied,
                    baseline,
                )
                .map_err(|e| Error(e.to_string()))?,
            )
        }
    }
    let resume = match &spec.mode {
        Mode::New => None,
        Mode::Resume(id) if !id.is_empty() && !id.contains('\0') => Some(id),
        Mode::Resume(_) => {
            return Err(invalid(
                "resume requires an exact native session identifier",
            ));
        }
    };
    match spec.client {
        Client::Codex => {
            argv.extend(["exec".into(), "--json".into()]);
            argv.extend(spec.arguments.clone());
            if let Some(id) = resume {
                argv.extend(["resume".into(), id.into()]);
            }
        }
        Client::ClaudeCode => {
            argv.extend([
                "-p".into(),
                "--verbose".into(),
                "--output-format".into(),
                "stream-json".into(),
            ]);
            argv.extend(spec.arguments.clone());
            if let Some(id) = resume {
                argv.extend(["--resume".into(), id.into()]);
            }
        }
        Client::FactoryDroid => {
            argv.extend([
                "exec".into(),
                "--output-format".into(),
                "stream-json".into(),
            ]);
            argv.extend(spec.arguments.clone());
            if let Some(id) = resume {
                argv.extend(["--session-id".into(), id.into()]);
            }
        }
        Client::KimiCode => {
            argv.extend(["--output-format".into(), "stream-json".into()]);
            argv.extend(spec.arguments.clone());
            if let Some(id) = resume {
                argv.extend(["--session".into(), id.into()]);
            }
            argv.extend(["--prompt".into(), spec.prompt.clone().into()]);
        }
        Client::Pi => {
            let path = spec
                .pi_session
                .as_ref()
                .ok_or_else(|| invalid("Pi requires an explicit native session file"))?;
            if !path.is_absolute() {
                return Err(invalid("Pi native session path must be absolute"));
            }
            // Pi resumes a native session FILE, not its header's UUID.
            argv.extend([
                "--print".into(),
                "--mode".into(),
                "json".into(),
                "--session".into(),
                path.as_os_str().into(),
            ]);
            argv.extend(spec.arguments.clone());
        }
    }
    if spec.client != Client::KimiCode {
        argv.extend(["--".into(), spec.prompt.clone().into()]);
    }
    Ok(LaunchPlan {
        spec,
        arguments: argv,
        configuration,
    })
}

pub fn decode(record: &SessionRecord) -> Result<Metadata, Error> {
    record
        .check()
        .map_err(|_| invalid("session record exceeds protocol limits"))?;
    let metadata: Metadata = serde_json::from_slice(&record.detail)
        .map_err(|_| invalid("invalid managed session metadata"))?;
    if metadata.schema != 1 {
        return Err(invalid("unsupported managed metadata schema"));
    }
    Ok(metadata)
}

fn set_metadata(record: &mut SessionRecord, metadata: &Metadata) -> Result<(), Error> {
    let mut candidate = record.clone();
    candidate.detail = serde_json::to_vec(metadata)
        .map_err(|_| invalid("cannot encode managed session metadata"))?;
    candidate
        .check()
        .map_err(|_| invalid("managed session exceeds protocol limits"))?;
    // Preserve capacity for a subsequently observed exit even when delivery
    // receipts are updated while the process is alive.
    let mut largest = metadata.clone();
    if let Some(process) = largest.process.as_mut() {
        process.exit_code = Some(i32::MIN);
        process.termination_signal = Some(i32::MIN);
        process.exited = false;
    }
    let mut capacity = candidate.clone();
    capacity.detail = serde_json::to_vec(&largest)
        .map_err(|_| invalid("cannot encode managed session metadata"))?;
    capacity
        .check()
        .map_err(|_| invalid("managed session exceeds protocol limits"))?;
    *record = candidate;
    Ok(())
}

/// Reconcile without trustworthy owned-child evidence. Even Launching may have
/// spawned immediately before the interruption; no automatic retry is safe.
pub fn recover(record: &SessionRecord) -> Result<SessionRecord, Error> {
    decode(record)?;
    let mut recovered = record.clone();
    if recovered.state != SessionState::Exited {
        recovered.state = SessionState::Unknown;
        recovered.capabilities.tools = false;
        recovered.capabilities.active_delivery = false;
    }
    Ok(recovered)
}

pub struct OwnedLaunch {
    child: Child,
    client: Client,
    record: SessionRecord,
    metadata: Metadata,
}

impl OwnedLaunch {
    /// The retained Child is the only authority for process observation/control.
    /// The caller consumes piped output promptly; this adapter keeps no queue.
    pub fn child_mut(&mut self) -> &mut Child {
        &mut self.child
    }
    pub fn record(&self) -> &SessionRecord {
        &self.record
    }
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
    pub fn set_delivery(&mut self, receipts: Value) -> Result<(), Error> {
        let mut candidate = self.metadata.clone();
        candidate.delivery = receipts;
        set_metadata(&mut self.record, &candidate)?;
        self.metadata = candidate;
        Ok(())
    }

    /// Claims come only from a fresh authenticated daemon SessionView, never
    /// from process/native identity or notification emission.
    pub fn observe_session(&mut self, view: &SessionView) -> Result<(), Error> {
        if view.instance != self.metadata.binding.instance
            || view.principal != self.metadata.binding.principal
        {
            return Err(invalid("daemon session view differs from managed binding"));
        }
        let mut candidate = self.metadata.clone();
        if let Some(attempt) = &candidate.binding.attempt {
            candidate.binding.claim = view.claims.iter().copied().find(|claim| {
                Some(claim.goal) == candidate.binding.goal
                    && claim.task == attempt.task
                    && claim.attempt == attempt.attempt
                    && claim.instance == view.instance
                    && claim.generation > 0
            });
        }
        set_metadata(&mut self.record, &candidate)?;
        self.metadata = candidate;
        Ok(())
    }

    fn readiness(&mut self) -> Result<(), Error> {
        if self.metadata.initialized
            && self.metadata.tools_ready
            && self.record.state != SessionState::Blocked
        {
            self.record.state = SessionState::Ready;
            self.record.capabilities.tools = true;
            self.record.capabilities.manual_resume = self.metadata.resume_from.as_deref()
                == self.record.client_session.as_deref()
                && self.metadata.resume_from.is_some();
        }
        set_metadata(&mut self.record, &self.metadata)
    }

    /// Exact structured event/header identification, never a prose startup line.
    /// Pi's event is read from the selected native session file by the caller.
    pub fn observe_native(&mut self, event: &Value) -> Result<bool, Error> {
        if matches!(
            self.record.state,
            SessionState::Exited | SessionState::Unknown
        ) {
            return Ok(false);
        }
        let id = match self.client {
            Client::Codex
                if event.get("type").and_then(Value::as_str) == Some("thread.started") =>
            {
                event.get("thread_id")
            }
            Client::ClaudeCode | Client::FactoryDroid
                if event.get("type").and_then(Value::as_str) == Some("system")
                    && event.get("subtype").and_then(Value::as_str) == Some("init") =>
            {
                event.get("session_id")
            }
            // Kimi 0.42 emits persisted identity in completion metadata.
            // Startup text and assistant-generated lookalikes are not identity.
            Client::KimiCode
                if event.get("role").and_then(Value::as_str) == Some("meta")
                    && event.get("type").and_then(Value::as_str) == Some("session.resume_hint") =>
            {
                event.get("session_id")
            }
            Client::Pi if event.get("type").and_then(Value::as_str) == Some("session") => {
                event.get("id")
            }
            _ => None,
        }
        .and_then(Value::as_str);
        let Some(id) = id.filter(|id| !id.is_empty()) else {
            return Ok(false);
        };
        if self
            .metadata
            .resume_from
            .as_deref()
            .is_some_and(|expected| expected != id)
            || self
                .record
                .client_session
                .as_deref()
                .is_some_and(|expected| expected != id)
        {
            self.record.state = SessionState::Unknown;
            self.record.capabilities.tools = false;
            return Err(invalid(
                "native session identity changed; explicit local rebind required",
            ));
        }
        let old_record = self.record.clone();
        let old_metadata = self.metadata.clone();
        self.record.client_session = Some(id.into());
        self.metadata.initialized = true;
        if let Err(error) = self.readiness() {
            self.record = old_record;
            self.metadata = old_metadata;
            return Err(error);
        }
        Ok(true)
    }

    /// Caller reads only the protected per-launch MCP receipt path. The MCP
    /// bridge reports this AFTER authenticated status and tools/list output.
    pub fn observe_tools(&mut self, receipt: &Value) -> Result<bool, Error> {
        if matches!(
            self.record.state,
            SessionState::Exited | SessionState::Unknown
        ) {
            return Ok(false);
        }
        if receipt.get("schema").and_then(Value::as_u64) != Some(1)
            || receipt.get("event").and_then(Value::as_str) != Some("tools_ready")
            || receipt.get("instance").and_then(Value::as_str)
                != Some(self.metadata.binding.instance.to_string().as_str())
            || receipt
                .get("pid")
                .and_then(Value::as_u64)
                .is_none_or(|pid| pid == 0 || pid > u32::MAX as u64)
        {
            return Ok(false);
        }
        self.metadata.tools_ready = true;
        self.readiness()?;
        Ok(true)
    }

    /// Called only on qualified structured user-action evidence. Reason text
    /// remains with the client; no transcript is copied into daemon state.
    pub fn blocked(&mut self) -> Result<(), Error> {
        if matches!(
            self.record.state,
            SessionState::Exited | SessionState::Unknown
        ) {
            return Err(invalid("inactive session cannot become blocked"));
        }
        self.record.state = SessionState::Blocked;
        set_metadata(&mut self.record, &self.metadata)
    }

    pub fn poll(&mut self) -> Result<bool, Error> {
        let Some(status) = self
            .child
            .try_wait()
            .map_err(|_| invalid("cannot observe owned client process"))?
        else {
            return Ok(false);
        };
        self.record.state = SessionState::Exited;
        self.record.capabilities.tools = false;
        self.record.capabilities.active_delivery = false;
        if let Some(process) = self.metadata.process.as_mut() {
            process.exited = true;
            process.exit_code = status.code();
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                process.termination_signal = status.signal();
            }
        }
        set_metadata(&mut self.record, &self.metadata)?;
        Ok(true)
    }
}

/// Caller must have applied any file proposal and supplies daemon wall time.
/// A report failure before spawn prevents spawn; a report failure after spawn
/// returns the retained Child so it cannot become an unowned side effect.
pub fn launch<F>(plan: LaunchPlan, started_ms: u64, mut report: F) -> Result<OwnedLaunch, Error>
where
    F: FnMut(&SessionRecord) -> Result<(), Error>,
{
    let spec = plan.spec;
    let mut metadata = Metadata {
        schema: 1,
        launch_id: spec.launch_id,
        binding: spec.binding,
        executable: spec.executable.clone(),
        workspace: spec.workspace.clone(),
        profile: spec.profile,
        bridge: PathRefs {
            home: spec.bridge.paths.home,
            credential: spec.bridge.paths.credential,
            session: spec.bridge.paths.session,
        },
        process: None,
        initialized: false,
        tools_ready: false,
        resume_from: match spec.mode {
            Mode::New => None,
            Mode::Resume(id) => Some(id),
        },
        native_session_file: spec.pi_session,
        lifecycle_receipt: spec.lifecycle_receipt,
        delivery: Value::Null,
    };
    let mut record = SessionRecord {
        harness: match spec.client {
            Client::Codex => locust_proto::farm::Harness::Codex,
            Client::ClaudeCode => locust_proto::farm::Harness::ClaudeCode,
            Client::FactoryDroid => locust_proto::farm::Harness::FactoryDroid,
            Client::KimiCode => locust_proto::farm::Harness::KimiCode,
            Client::Pi => locust_proto::farm::Harness::Pi,
        },
        client: format!("{} {}", client_name(spec.client), spec.version),
        state: SessionState::Launching,
        client_session: None,
        capabilities: SessionCapabilities {
            manual_resume: false,
            ..Default::default()
        },
        detail: Vec::new(),
    };
    // Validate the largest process correlation before any spawn, so adding a
    // PID after the side effect cannot fail the protocol metadata limit.
    metadata.process = Some(ProcessEvidence {
        pid: u32::MAX,
        started_ms: u64::MAX,
        exit_code: Some(i32::MIN),
        termination_signal: Some(i32::MIN),
        exited: false,
    });
    set_metadata(&mut record, &metadata)?;
    metadata.process = None;
    set_metadata(&mut record, &metadata)?;
    report(&record)?;
    let child = match Command::new(spec.executable)
        .args(plan.arguments)
        .current_dir(spec.workspace)
        .env_clear()
        .envs(spec.environment)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            record.state = SessionState::Exited;
            report(&record)?;
            return Err(invalid("client process could not be started"));
        }
    };
    metadata.process = Some(ProcessEvidence {
        pid: child.id(),
        started_ms,
        exit_code: None,
        termination_signal: None,
        exited: false,
    });
    record.state = SessionState::Started;
    set_metadata(&mut record, &metadata)?;
    // Keep ownership even if the post-spawn durable report fails. The caller
    // receives Unknown and must reconcile/report through the real daemon API.
    if report(&record).is_err() {
        record.state = SessionState::Unknown;
    }
    Ok(OwnedLaunch {
        child,
        client: spec.client,
        record,
        metadata,
    })
}

#[cfg(test)]
mod tests;
