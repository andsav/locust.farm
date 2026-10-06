//! Resumable owner-driven enrollment followed by reviewed client setup.
//!
//! A private journal reserves one identity per client/profile before secrets or
//! enrollment are created. Secrets are durable before the daemon learns them.
//! Setup keeps its own file transaction; onboarding puts the agent in no goal.
pub mod diagnostics;

use super::*;
use crate::connection;
use locust_proto::api::{Caller, Credential, DaemonStatus, Request, Response, SessionSecret};
use locust_proto::client::Client as ApiClient;
use locust_proto::id::{InstanceId, PublicKey};
use locust_proto::local;
use setup::{Client, SetupSpec};
use std::io::Read;
use std::os::unix::net::UnixStream;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub prefix: PathBuf,
    pub client: Client,
    pub profile_home: PathBuf,
    pub workspace: PathBuf,
    pub daemon_home: PathBuf,
    pub name: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    format: String,
    spec: Spec,
    owner_sha256: String,
    credential_sha256: Option<String>,
    session_sha256: Option<String>,
    principal: Option<PublicKey>,
    instance: Option<InstanceId>,
    reviewed_setup_sha256: Option<String>,
    configured: bool,
}

#[derive(Clone)]
pub struct Plan {
    pub spec: Spec,
    pub name_generated: bool,
    review: Value,
}
impl Plan {
    pub fn json(&self) -> Result<Value, Failure> {
        Ok(json!({"plan":self.review,"plan_sha256":self.digest()?}))
    }
    fn digest(&self) -> Result<String, Failure> {
        Ok(package::sha256(&encode(&self.review)?))
    }
}

pub fn client_name(client: Client) -> &'static str {
    match client {
        Client::Codex => "codex",
        Client::Claude => "claude",
        Client::Pi => "pi",
        Client::Droid => "droid",
        Client::Shell => "shell",
    }
}

fn normalize(spec: &Spec) -> Result<Spec, Failure> {
    let mut s = spec.clone();
    for path in [
        &mut s.prefix,
        &mut s.profile_home,
        &mut s.workspace,
        &mut s.daemon_home,
    ] {
        *path = absolute(path)?;
        if path
            .to_str()
            .is_none_or(|text| text.chars().any(char::is_control))
        {
            return Err(Failure::usage(
                "onboarding paths must be UTF-8 without control characters",
            ));
        }
    }
    local::socket_path(&s.daemon_home)?;
    if !s.workspace.is_dir() || !s.profile_home.is_dir() {
        return Err(Failure::usage(
            "the selected profile home and workspace must already be directories",
        ));
    }
    if s.name
        .as_ref()
        .is_some_and(|name| !locust_proto::api::is_agent_name(name))
    {
        return Err(Failure::usage(
            "agent name must be 1 to 32 characters from a-z, 0-9 and -",
        ));
    }
    Ok(s)
}

fn directory(s: &Spec) -> Result<PathBuf, Failure> {
    let id = package::sha256(&encode(&(s.client, &s.profile_home))?);
    Ok(s.daemon_home.join("onboarding").join(id))
}
fn record_path(s: &Spec) -> Result<PathBuf, Failure> {
    Ok(directory(s)?.join("state.json"))
}
fn setup_spec(s: &Spec) -> Result<SetupSpec, Failure> {
    let directory = directory(s)?;
    Ok(SetupSpec {
        prefix: s.prefix.clone(),
        client: s.client,
        profile_home: s.profile_home.clone(),
        workspace: s.workspace.clone(),
        daemon_home: s.daemon_home.clone(),
        executable: s.prefix.join("current/locust"),
        skill_source: s.prefix.join("current/skills/locust/SKILL.md"),
        credential: directory.join("credential"),
        session: directory.join("session"),
    })
}
fn protected_bytes(path: &Path) -> Result<Vec<u8>, Failure> {
    let mut file = package::regular(path)?;
    let meta = file.metadata().map_err(io_error)?;
    if meta.uid() != rustix::process::getuid().as_raw() || meta.mode() & 0o7777 != 0o600 {
        return Err(Failure::new(
            ErrorCode::Denied,
            format!("{} must be owned and mode 0600", path.display()),
        ));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(io_error)?;
    Ok(bytes)
}
fn protected_secret(path: &Path) -> Result<[u8; 32], Failure> {
    protected_bytes(path)?
        .try_into()
        .map_err(|_| Failure::invalid(format!("{} must contain 32 secret bytes", path.display())))
}
fn read_record(s: &Spec) -> Result<Option<(Record, String)>, Failure> {
    private_dir(&s.daemon_home, false)?;
    private_dir(&s.daemon_home.join("onboarding"), false)?;
    private_dir(&directory(s)?, false)?;
    let path = record_path(s)?;
    if !exists(&path)? {
        if directory(s)?.is_dir()
            && fs::read_dir(directory(s)?)
                .map_err(io_error)?
                .next()
                .is_some()
        {
            return Err(conflict(
                "onboarding directory has files without its ownership journal; preserve them for inspection",
            ));
        }
        return Ok(None);
    }
    let bytes = protected_bytes(&path)?;
    let record: Record =
        serde_json::from_slice(&bytes).map_err(|_| corrupt("invalid onboarding journal"))?;
    let valid_hash = |hash: &str| {
        hash.len() == 64
            && hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    };
    if record.format != "locust-onboarding-v1"
        || record.spec.name.is_none()
        || normalize(&record.spec).ok().as_ref() != Some(&record.spec)
        || !valid_hash(&record.owner_sha256)
        || [
            &record.credential_sha256,
            &record.session_sha256,
            &record.reviewed_setup_sha256,
        ]
        .into_iter()
        .flatten()
        .any(|hash| !valid_hash(hash))
    {
        return Err(corrupt("unknown or incomplete onboarding journal"));
    }
    let mut expected = s.clone();
    if expected.name.is_none() {
        expected.name.clone_from(&record.spec.name);
    }
    if record.spec != expected {
        return Err(conflict(
            "onboarding already owns this client/profile with different selections; retry with its original prefix, daemon home, workspace and name",
        ));
    }
    if record.principal.is_some()
        && (record.credential_sha256.is_none()
            || record.session_sha256.is_none()
            || record.instance.is_none())
        || record.session_sha256.is_some() != record.instance.is_some()
        || record.session_sha256.is_some() && record.credential_sha256.is_none()
        || record.reviewed_setup_sha256.is_some() && record.principal.is_none()
        || record.configured
            && (record.principal.is_none() || record.reviewed_setup_sha256.is_none())
    {
        return Err(corrupt("onboarding journal stages are inconsistent"));
    }
    Ok(Some((record, package::sha256(&bytes))))
}
fn verify_secret(path: &Path, expected: &Option<String>) -> Result<(), Failure> {
    if let Some(expected) = expected
        && package::sha256(&protected_secret(path)?) != *expected
    {
        return Err(conflict(
            "an onboarding secret changed; preserve the identity and restore its original protected file",
        ));
    }
    Ok(())
}
fn generated_name(client: Client) -> Result<String, Failure> {
    let random = crate::secret::generate().map_err(io_error)?;
    let words = [
        "cedar", "birch", "maple", "willow", "aspen", "hazel", "alder", "pine",
    ];
    Ok(format!(
        "{}-{}-{}",
        client_name(client),
        words[usize::from(random[0]) % words.len()],
        package::hex(&random[1..5])
    ))
}

/// No local state is created, including when the daemon has not started yet.
pub fn plan(spec: &Spec) -> Result<Plan, Failure> {
    let mut s = normalize(spec)?;
    let installed = super::status(&s.prefix)?;
    if installed["installed"] != true || installed["withdrawn"] != false {
        return Err(Failure::new(
            ErrorCode::Denied,
            "onboarding requires an installed, verified, non-withdrawn release; install a reviewed bundle first",
        ));
    }
    let prior = read_record(&s)?;
    if let Some((record, _)) = &prior {
        s.name.clone_from(&record.spec.name);
    }
    let name_generated = s.name.is_none();
    if name_generated {
        s.name = Some(generated_name(s.client)?);
    }
    let binding = setup_spec(&s)?;
    let profile = if let Some((record, _)) = &prior {
        verify_secret(&binding.credential, &record.credential_sha256)?;
        verify_secret(&binding.session, &record.session_sha256)?;
        if record.principal.is_some() {
            setup::plan(&binding, false)?.json()?
        } else {
            setup::preflight_new(&binding)?
        }
    } else {
        setup::preflight_new(&binding)?
    };
    let review = json!({"format":"locust-onboarding-plan-v1","spec":s,
        "source_manifest_sha256":installed["manifest_sha256"],
        "journal":record_path(&s)?,"journal_sha256":prior.as_ref().map(|(_,hash)|hash),
        "credential_file":binding.credential,"session_file":binding.session,
        "principal":prior.as_ref().and_then(|(r,_)|r.principal),
        "profile":profile,"client_approval_policy_changed":false,
        "model_ready":false,"binding_scope":"explicit selected profile and fixed Locust session"});
    Ok(Plan {
        spec: s,
        name_generated,
        review,
    })
}

fn guard(home: &Path) -> Result<File, Failure> {
    private_dir(home, false)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(home.join("onboarding.lock"))
        .map_err(io_error)?;
    let meta = file.metadata().map_err(io_error)?;
    if !meta.is_file()
        || meta.nlink() != 1
        || meta.uid() != rustix::process::getuid().as_raw()
        || meta.mode() & 0o7777 != 0o600
    {
        return Err(Failure::new(
            ErrorCode::Denied,
            "onboarding lock must be a private owned regular file",
        ));
    }
    file.try_lock()
        .map_err(|_| conflict("another onboarding operation holds this daemon home"))?;
    Ok(file)
}
fn save(record: &Record) -> Result<(), Failure> {
    atomic(&record_path(&record.spec)?, &encode(record)?)
}
fn ensure_secret(path: &Path, expected: &mut Option<String>) -> Result<(), Failure> {
    verify_secret(path, expected)?;
    if expected.is_none() {
        if exists(path)? {
            protected_secret(path)?;
        }
        crate::secret::read_or_create(path).map_err(io_error)?;
        *expected = Some(package::sha256(&protected_secret(path)?));
    }
    Ok(())
}
fn open(
    home: &Path,
    credential: Credential,
    session: Option<SessionSecret>,
) -> Result<ApiClient<UnixStream>, Failure> {
    let socket = local::socket_path(home)?;
    ApiClient::open(connection::connect(&socket)?, credential, session)
        .map_err(|error| connection::client_error(error, &socket))
}
fn status(client: &mut ApiClient<UnixStream>, home: &Path) -> Result<DaemonStatus, Failure> {
    match client.call(Request::Status).map_err(|error| {
        connection::client_error(error, &local::socket_path(home).expect("checked home"))
    })? {
        Response::Status(status) => Ok(status),
        _ => Err(Failure::internal(
            "daemon returned another response to status",
        )),
    }
}
/// Authenticated owner readiness, separate from service-manager state.
pub fn owner_ready(home: &Path, timeout: Option<Duration>) -> Result<Value, Failure> {
    private_dir(home, false)?;
    let owner_path = local::owner_credential_path(home);
    if !exists(&owner_path)? {
        return Err(Failure::unavailable(
            "daemon has not created its owner credential yet",
        ));
    }
    let credential = Credential(protected_secret(&owner_path)?);
    let socket = local::socket_path(home)?;
    let stream = connection::connect(&socket)?;
    // Bound hello/status reads too when the caller selected a readiness wait.
    // Unix socket timeouts reject zero, so a check-once probe uses the smallest
    // positive duration instead of silently becoming an unbounded read.
    let timeout = timeout.map(|timeout| timeout.max(Duration::from_nanos(1)));
    stream.set_read_timeout(timeout).map_err(io_error)?;
    stream.set_write_timeout(timeout).map_err(io_error)?;
    let mut owner = ApiClient::open(stream, credential, None)
        .map_err(|error| connection::client_error(error, &socket))?;
    if owner.caller() != Caller::Owner {
        return Err(Failure::new(
            ErrorCode::Denied,
            "selected home does not authenticate as owner",
        ));
    }
    let found = status(&mut owner, home)?;
    Ok(json!({"daemon_version":found.daemon_version,"endpoint":found.endpoint,"api_ready":true}))
}

pub fn apply(
    proposed: &Plan,
    review: &mut impl FnMut(&Value) -> Result<bool, Failure>,
) -> Result<Value, Failure> {
    apply_with(proposed, review, &mut |_| Ok(()))
}
fn apply_with(
    proposed: &Plan,
    review: &mut impl FnMut(&Value) -> Result<bool, Failure>,
    checkpoint: &mut impl FnMut(&str) -> Result<(), Failure>,
) -> Result<Value, Failure> {
    let s = &proposed.spec;
    let owner_secret = protected_secret(&local::owner_credential_path(&s.daemon_home))?;
    let mut owner = open(&s.daemon_home, Credential(owner_secret), None)?;
    if owner.caller() != Caller::Owner {
        return Err(Failure::new(
            ErrorCode::Denied,
            "onboarding requires the selected daemon's owner",
        ));
    }
    let _guard = guard(&s.daemon_home)?;
    if plan(s)?.digest()? != proposed.digest()? {
        return Err(conflict(
            "onboarding selections or files changed since review; run up again to review them",
        ));
    }
    let mut record = if let Some((record, _)) = read_record(s)? {
        record
    } else {
        if status(&mut owner, &s.daemon_home)?
            .agents
            .iter()
            .any(|a| Some(&a.name) == s.name.as_ref())
        {
            return Err(conflict(
                "the selected agent name is already enrolled; choose a different --name",
            ));
        }
        private_dir(&s.daemon_home.join("onboarding"), true)?;
        private_dir(&directory(s)?, true)?;
        let record = Record {
            format: "locust-onboarding-v1".into(),
            spec: s.clone(),
            owner_sha256: package::sha256(&owner_secret),
            credential_sha256: None,
            session_sha256: None,
            principal: None,
            instance: None,
            reviewed_setup_sha256: None,
            configured: false,
        };
        save(&record)?;
        record
    };
    if record.owner_sha256 != package::sha256(&owner_secret) {
        return Err(conflict(
            "daemon owner identity changed; onboarding will not recreate its enrolled agent",
        ));
    }
    checkpoint("journal_created")?;
    let binding = setup_spec(s)?;
    ensure_secret(&binding.credential, &mut record.credential_sha256)?;
    save(&record)?;
    checkpoint("credential_saved")?;
    ensure_secret(&binding.session, &mut record.session_sha256)?;
    let credential = Credential(protected_secret(&binding.credential)?);
    let session = SessionSecret(protected_secret(&binding.session)?);
    if record
        .instance
        .is_some_and(|instance| instance != session.instance())
    {
        return Err(conflict("onboarding session identity changed"));
    }
    record.instance = Some(session.instance());
    save(&record)?;
    checkpoint("session_saved")?;
    checkpoint("secrets_saved")?;
    let mut agent = match open(&s.daemon_home, credential, Some(session)) {
        Ok(agent) => agent,
        Err(error) if error.code == ErrorCode::Denied && record.principal.is_none() => {
            if status(&mut owner, &s.daemon_home)?
                .agents
                .iter()
                .any(|a| Some(&a.name) == s.name.as_ref())
            {
                return Err(conflict(
                    "the onboarding name belongs to a different or revoked credential; preserve the journal and identity",
                ));
            }
            owner
                .call(Request::AgentEnroll {
                    name: s.name.clone().expect("planned name"),
                    credential: credential.digest(),
                })
                .map_err(|error| {
                    connection::client_error(
                        error,
                        &local::socket_path(&s.daemon_home).expect("checked home"),
                    )
                })?;
            checkpoint("enrollment_sent")?;
            open(&s.daemon_home, credential, Some(session))?
        }
        Err(error) => return Err(error),
    };
    let Caller::Agent(principal) = agent.caller() else {
        return Err(conflict(
            "onboarding credential must authenticate as an agent",
        ));
    };
    let observed = status(&mut agent, &s.daemon_home)?;
    if !observed
        .agents
        .iter()
        .any(|a| a.agent == principal && Some(&a.name) == s.name.as_ref() && !a.revoked)
        || record
            .principal
            .is_some_and(|expected| expected != principal)
    {
        return Err(conflict(
            "onboarding credential no longer matches its recorded agent",
        ));
    }
    record.principal = Some(principal);
    save(&record)?;
    checkpoint("enrolled")?;
    let setup_plan = setup::plan(&binding, false)?;
    let reviewed = setup_plan.json()?;
    let digest = setup_plan.digest()?;
    let unchanged = reviewed["plan"]["files"]
        .as_array()
        .is_some_and(|files| files.iter().all(|f| f["before"] == f["after"]));
    if !unchanged && !review(&reviewed)? {
        return Err(Failure::new(
            ErrorCode::Denied,
            "client setup was not approved; the enrolled identity is saved, so repeat the same command to resume",
        ));
    }
    record.reviewed_setup_sha256 = Some(digest.clone());
    save(&record)?;
    checkpoint("setup_reviewed")?;
    let applied = setup::apply(&binding, &digest)?;
    checkpoint("setup_applied")?;
    let configured = setup::status(&binding)?;
    if configured["configured"] != true
        || configured["binding_matches"] != true
        || configured["pending"] != false
    {
        return Err(Failure::unavailable(
            "client files are not ready; repeat onboarding to inspect its saved stages",
        ));
    }
    // Recheck the authenticated agent after the profile write, without touching
    // native session metadata or treating configuration as model discovery.
    verify_secret(&binding.credential, &record.credential_sha256)?;
    verify_secret(&binding.session, &record.session_sha256)?;
    let mut current = open(
        &s.daemon_home,
        Credential(protected_secret(&binding.credential)?),
        Some(SessionSecret(protected_secret(&binding.session)?)),
    )?;
    if current.caller() != Caller::Agent(principal) {
        return Err(conflict("onboarding agent binding changed during setup"));
    }
    status(&mut current, &s.daemon_home)?;
    record.configured = true;
    save(&record)?;
    Ok(
        json!({"client":s.client,"name":s.name,"principal":principal,"instance":session.instance(),
        "profile_home":s.profile_home,"workspace":s.workspace,"credential_file":binding.credential,"session_file":binding.session,
        "journal":record_path(s)?,"launcher":configured["launcher"],"configuration_ready":true,"daemon_api_ready":true,
        "model_ready":false,"changed":applied["changed"],
        "next_action":format!("Start a fresh client chat and verify Locust tool discovery. {} is connected and in no goal. Start one (locust --owner goal create --help) or join one from a ticket (locust --owner goal join --help); both ask you first.", s.name.as_deref().unwrap_or("Agent"))}),
    )
}

#[cfg(test)]
mod tests;
