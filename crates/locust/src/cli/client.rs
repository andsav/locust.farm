//! Explicit participant-owned client launch. Native output streams to stderr;
//! stdout remains the CLI's single versioned result envelope.
mod profile;
mod signals;
use super::{LocalClient, Output, connection, resolve_goal};
use crate::failure::Failure;
use clap::{Arg, ArgAction, ArgMatches, Command};
use locust_adapter::{
    config::{BridgePaths, Client, StdioServer},
    delivery::{self, DeliveryBinding, DeliveryReceipt},
    managed::{self, AttemptBinding, Binding, LaunchSpec, Mode},
};
use locust_proto::api::{
    Caller, ErrorCode, PendingWork, Request, Response, SessionSecret, SessionState, SessionView,
};
use locust_proto::{id::EventId, local};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn opt(name: &'static str, required: bool) -> Arg {
    Arg::new(name).long(name).required(required)
}
pub(super) fn commands() -> Command {
    Command::new("client").about("Explicit local client launch, status and pending recovery; native run output goes to stderr").subcommand_required(true)
        .subcommand(Command::new("run").disable_version_flag(true).about("Launch a chosen client locally; never triggered by peer events")
            .arg(opt("client",true).value_parser(["codex","claude-code","factory-droid","kimi-code","pi"]))
            .arg(opt("executable",true)).arg(opt("workspace",true)).arg(opt("profile",true))
            .arg(opt("client-version",true).help("Exact observed client version retained as qualification context"))
            .arg(opt("prompt",true).allow_hyphen_values(true))
            .arg(opt("arg",false).action(ArgAction::Append).allow_hyphen_values(true))
            .arg(opt("global-arg",false).action(ArgAction::Append).allow_hyphen_values(true))
            .arg(opt("goal",false)).arg(opt("attempt",false).requires("goal"))
            .arg(opt("resume",false)).arg(opt("native-session",false)))
        .subcommand(Command::new("status").about("Read this authenticated session's lifecycle record"))
        .subcommand(Command::new("recover").about("Mark an interrupted launch uncertain without spawning or signaling"))
        .subcommand(Command::new("pending").about("Read authoritative IDs/status; cancellation is requested until explicitly acknowledged").arg(opt("goal",false)))
}
fn output(result: Value) -> Output {
    Output::success(
        result.clone(),
        serde_json::to_string_pretty(&result).unwrap(),
    )
}
fn absolute(args: &ArgMatches, name: &str) -> Result<PathBuf, Failure> {
    let path = PathBuf::from(args.get_one::<String>(name).unwrap());
    if !path.is_absolute() {
        return Err(Failure::usage(format!("--{name} must be absolute")));
    }
    Ok(path)
}
fn call(api: &mut LocalClient, socket: &Path, request: Request) -> Result<Response, Failure> {
    api.call(request)
        .map_err(|e| connection::client_error(e, socket))
}
fn session(api: &mut LocalClient, socket: &Path) -> Result<SessionView, Failure> {
    match call(api, socket, Request::Session { instance: None })? {
        Response::Session(v) => Ok(v),
        _ => unreachable!(),
    }
}
fn pending(
    api: &mut LocalClient,
    socket: &Path,
    goal: locust_proto::id::GoalId,
) -> Result<PendingWork, Failure> {
    match call(api, socket, Request::Pending { goal })? {
        Response::Pending(v) => Ok(v),
        _ => unreachable!(),
    }
}
fn report(
    api: &mut LocalClient,
    socket: &Path,
    record: &locust_proto::api::SessionRecord,
) -> Result<(), Failure> {
    call(
        api,
        socket,
        Request::SessionReport {
            record: record.clone(),
        },
    )?;
    Ok(())
}
fn adapter(e: managed::Error) -> Failure {
    Failure::invalid(e.to_string())
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
fn client(value: &str) -> Client {
    match value {
        "codex" => Client::Codex,
        "claude-code" => Client::ClaudeCode,
        "factory-droid" => Client::FactoryDroid,
        "kimi-code" => Client::KimiCode,
        "pi" => Client::Pi,
        _ => unreachable!(),
    }
}

pub(super) fn run(
    matches: &ArgMatches,
    operation: &str,
    args: &ArgMatches,
) -> Result<Output, Failure> {
    if matches.get_flag("owner")
        || matches.get_one::<String>("as").is_some()
        || matches.get_one::<String>("idempotency-key").is_some()
    {
        return Err(Failure::usage(
            "client commands require an enrolled agent credential and its session, without --owner, --as or --idempotency-key",
        ));
    }
    let home = connection::home(matches)?;
    let socket = local::socket_path(&home)?;
    let secret_path = connection::session_path(matches)?
        .ok_or_else(|| Failure::usage("client commands require an existing --session file"))?;
    let instance = SessionSecret(connection::read_secret(&secret_path)?).instance();
    let mut api = connection::open(matches, &home)?;
    let Caller::Agent(principal) = api.caller() else {
        return Err(Failure::new(
            ErrorCode::Denied,
            "managed clients require enrolled agent authority",
        ));
    };
    if operation == "client.status" {
        return session(&mut api, &socket).map(|v| output(json!({"schema":1,"session":v})));
    }
    if operation == "client.pending" {
        let view = session(&mut api, &socket)?;
        let metadata = managed::decode(&view.record).map_err(adapter)?;
        let goal = if let Some(goal) = args.get_one::<String>("goal") {
            let goal = resolve_goal(&mut api, &socket, goal, None)?;
            if metadata.binding.goal.is_some_and(|bound| bound != goal) {
                return Err(Failure::new(
                    ErrorCode::Conflict,
                    "goal differs from managed session binding",
                ));
            }
            goal
        } else {
            metadata
                .binding
                .goal
                .ok_or_else(|| Failure::usage("unbound session requires --goal"))?
        };
        let work = pending(&mut api, &socket, goal)?;
        let previous = DeliveryReceipt::decode(&metadata.delivery)?;
        let prepared = delivery::prepare(
            DeliveryBinding {
                goal,
                principal,
                instance,
                claim: metadata.binding.claim,
            },
            &view,
            work,
            previous.as_ref(),
        )?;
        return Ok(output(
            json!({"schema":1,"mechanism":"ordinary_tool","notice":prepared.notice,"duplicate_notification":prepared.duplicate,"task_ownership_changed":false,"cancellation_acknowledged":false}),
        ));
    }
    let secret_parent = profile::directory(secret_path.parent().unwrap())?;
    let lock_name = format!(
        "{}.launcher.lock",
        secret_path.file_name().unwrap().to_string_lossy()
    );
    let _lock = profile::lock(&secret_parent, &lock_name)?;
    let previous = match session(&mut api, &socket) {
        Ok(v) => Some(v),
        Err(e) if e.code == ErrorCode::NotFound => None,
        Err(e) => return Err(e),
    };
    if operation == "client.recover" {
        let view = previous.ok_or_else(|| {
            Failure::new(ErrorCode::NotFound, "no managed session record to recover")
        })?;
        let record = managed::recover(&view.record).map_err(adapter)?;
        report(&mut api, &socket, &record)?;
        return Ok(output(
            json!({"schema":1,"session":record,"spawned":false,"signaled":false,"resolution":"unknown launch outcomes require explicit local reconciliation; historical PIDs are not process authority"}),
        ));
    }
    if previous
        .as_ref()
        .is_some_and(|v| v.record.state != SessionState::Exited)
    {
        return Err(Failure::new(
            ErrorCode::Conflict,
            "session already has an active or uncertain launch; inspect or recover it, then resolve uncertainty locally before launching",
        ));
    }
    let chosen = client(args.get_one::<String>("client").unwrap());
    let executable = absolute(args, "executable")?;
    let workspace = absolute(args, "workspace")?;
    let profile_path = absolute(args, "profile")?;
    let profile_dir = profile::directory(&profile_path)?;
    let _workspace_dir = profile::directory(&workspace)?;
    let _profile_lock = profile::lock(&profile_dir, ".locust-client.lock")?;
    let goal = args
        .get_one::<String>("goal")
        .map(|g| resolve_goal(&mut api, &socket, g, None))
        .transpose()?;
    let attempt = args
        .get_one::<String>("attempt")
        .map(|s| {
            s.parse::<EventId>()
                .map_err(|_| Failure::usage("--attempt requires a full event identifier"))
        })
        .transpose()?;
    let mut binding = Binding {
        instance,
        principal,
        goal,
        attempt: None,
        claim: None,
    };
    if let Some(attempt) = attempt {
        let Response::Board(board) = call(
            &mut api,
            &socket,
            Request::Board {
                goal: goal.unwrap(),
            },
        )?
        else {
            unreachable!()
        };
        let task = board
            .into_iter()
            .find(|t| t.attempts.contains(&attempt))
            .ok_or_else(|| Failure::new(ErrorCode::Superseded, "attempt is not current"))?;
        if task.closed {
            return Err(Failure::new(ErrorCode::Conflict, "attempt task is closed"));
        }
        binding.attempt = Some(AttemptBinding {
            task: task.task,
            attempt,
        });
        let work = pending(&mut api, &socket, goal.unwrap())?;
        binding.claim = work.claimed.iter().copied().find(|claim| {
            claim.attempt == attempt && claim.task == task.task && claim.instance == instance
        });
        if work
            .held_elsewhere
            .iter()
            .any(|claim| claim.attempt == attempt)
        {
            return Err(Failure::new(
                ErrorCode::ClaimHeld,
                "attempt is held by another session; explicit takeover is required before this launch",
            ));
        }
    }
    if previous
        .as_ref()
        .is_some_and(|v| v.claims.iter().any(|claim| Some(*claim) != binding.claim))
    {
        return Err(Failure::new(
            ErrorCode::Conflict,
            "session has an unfinished claim outside the selected attempt",
        ));
    }
    let mode = if let Some(id) = args.get_one::<String>("resume") {
        let prev = previous.as_ref().ok_or_else(|| {
            Failure::new(
                ErrorCode::Conflict,
                "resume requires a known exited managed session",
            )
        })?;
        let metadata = managed::decode(&prev.record).map_err(adapter)?;
        if prev.record.client_session.as_ref() != Some(id)
            || metadata.executable != executable
            || metadata.workspace != workspace
            || metadata.profile != profile_path
            || metadata.binding.goal != goal
            || metadata.binding.attempt != binding.attempt
            || !prev
                .record
                .client
                .starts_with(&format!("{} ", args.get_one::<String>("client").unwrap()))
        {
            return Err(Failure::new(
                ErrorCode::Conflict,
                "resume must match the exact exited native session, client, paths and attempt",
            ));
        }
        Mode::Resume(id.clone())
    } else {
        Mode::New
    };
    let nonce = locust_proto::id::EventId(
        crate::secret::generate()
            .map_err(|_| Failure::internal("cannot generate launch identifier"))?,
    )
    .to_string();
    let receipt_name = format!(".locust-launch-{nonce}.jsonl");
    let mut receipt = profile::private_file(&secret_parent, &receipt_name)?;
    let receipt_path = secret_path.parent().unwrap().join(&receipt_name);
    // A launch receipt records no prompt, runtime keys or transcript.
    writeln!(
        receipt,
        "{}",
        json!({"schema":1,"event":"launch_receipt","launch_id":nonce})
    )
    .map_err(|_| Failure::internal("cannot initialize lifecycle receipt"))?;
    receipt
        .sync_all()
        .map_err(|_| Failure::internal("cannot sync lifecycle receipt"))?;
    let bridge = StdioServer {
        executable: std::env::current_exe()
            .map_err(|_| Failure::internal("cannot locate Locust executable"))?,
        arguments: vec![
            "mcp".into(),
            "--lifecycle-receipt".into(),
            receipt_path.to_string_lossy().into_owned(),
        ],
        paths: BridgePaths {
            home: home.clone(),
            credential: connection::credential_path(matches, &home)?,
            session: secret_path,
        },
    };
    let mut environment: BTreeMap<_, _> = std::env::vars_os()
        .filter(|(k, _)| !k.to_string_lossy().starts_with("LOCUST_"))
        .collect();
    environment.insert("HOME".into(), profile_path.as_os_str().into());
    environment.insert(
        "CODEX_HOME".into(),
        profile_path.join(".codex").into_os_string(),
    );
    environment.insert(
        "CLAUDE_CONFIG_DIR".into(),
        profile_path.join(".claude").into_os_string(),
    );
    for (key, dir) in [
        ("XDG_CONFIG_HOME", ".config"),
        ("XDG_DATA_HOME", ".local/share"),
        ("XDG_CACHE_HOME", ".cache"),
    ] {
        environment.insert(key.into(), profile_path.join(dir).into_os_string());
    }
    environment.insert(
        "PI_CODING_AGENT_DIR".into(),
        profile_path.join(".pi/agent").into_os_string(),
    );
    environment.insert(
        "KIMI_CODE_HOME".into(),
        profile_path.join(".kimi-code").into_os_string(),
    );
    let relative = match chosen {
        Client::FactoryDroid => Some(Path::new(".factory/mcp.json")),
        Client::KimiCode => Some(Path::new(".kimi-code/mcp.json")),
        Client::Pi => Some(Path::new(".pi/agent/mcp.json")),
        _ => None,
    };
    let mut overlay = relative
        .map(|r| profile::Overlay::inspect(&profile_path, r, &nonce))
        .transpose()?;
    let baseline = overlay
        .as_ref()
        .and_then(|o| o.baseline.as_ref())
        .map(|b| {
            serde_json::from_slice(b).map_err(|_| {
                Failure::new(ErrorCode::Corrupted, "invalid existing MCP profile JSON")
            })
        })
        .transpose()?
        .unwrap_or(json!({}));
    let occupied = occupied_names(chosen, &profile_path, &workspace, &baseline)?;
    let pi_session = args
        .get_one::<String>("native-session")
        .map(|_| absolute(args, "native-session"))
        .transpose()?;
    if let Some(path) = pi_session.as_ref() {
        crate::secret::create_private_dir(
            path.parent()
                .ok_or_else(|| Failure::usage("native session path needs a parent directory"))?,
        )
        .map_err(|_| Failure::invalid("cannot prepare explicit native session parent"))?;
        match fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.is_file() => {
                return Err(Failure::invalid("native session must be a regular file"));
            }
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err(Failure::invalid("cannot inspect native session file")),
        }
    }
    if let Some(previous) = previous
        .as_ref()
        .filter(|_| matches!(mode, Mode::Resume(_)))
        && managed::decode(&previous.record)
            .map_err(adapter)?
            .native_session_file
            != pi_session
    {
        return Err(Failure::new(
            ErrorCode::Conflict,
            "native session file differs from exited session",
        ));
    }
    let spec = LaunchSpec {
        client: chosen,
        version: args.get_one::<String>("client-version").unwrap().clone(),
        executable,
        workspace,
        profile: profile_path,
        bridge,
        launch_id: nonce,
        binding,
        mode,
        arguments: args
            .get_many::<String>("arg")
            .into_iter()
            .flatten()
            .map(Into::into)
            .collect(),
        global_arguments: args
            .get_many::<String>("global-arg")
            .into_iter()
            .flatten()
            .map(Into::into)
            .collect(),
        environment,
        prompt: args.get_one::<String>("prompt").unwrap().clone(),
        pi_session,
        lifecycle_receipt: receipt_path.clone(),
    };
    let plan = managed::prepare(spec, &occupied, &baseline).map_err(adapter)?;
    let signals = signals::Listener::new()?;
    if let Some(config) = &plan.configuration {
        overlay
            .as_mut()
            .unwrap()
            .apply(serde_json::to_vec_pretty(&config.document).unwrap())?;
    }
    let mut failure = None;
    let launch = managed::launch(plan, now(), |record| {
        report(&mut api, &socket, record).map_err(|e| {
            failure = Some(e);
            managed::Error::report_failed()
        })
    });
    let mut owned = match launch {
        Ok(o) => o,
        Err(e) => {
            let primary = failure.unwrap_or_else(|| adapter(e));
            if let Some(o) = overlay.as_mut()
                && let Err(restore) = o.restore()
            {
                return Err(Failure::new(
                    primary.code,
                    format!(
                        "{}; profile recovery also required: {}",
                        primary.message, restore.message
                    ),
                ));
            }
            return Err(primary);
        }
    };
    let result = observe(
        &mut owned,
        &mut api,
        &socket,
        receipt,
        failure.take(),
        &signals,
    );
    let restoration = overlay.as_mut().map(|o| o.restore()).transpose();
    match (result, restoration) {
        (Err(e), Err(restore)) => Err(Failure::new(
            e.code,
            format!(
                "{}; profile recovery also required: {}",
                e.message, restore.message
            ),
        )),
        (Err(e), Ok(_)) => Err(e),
        (Ok(_), Err(e)) => Err(e),
        (Ok(value), Ok(_)) => Ok(output(value)),
    }
}

fn occupied_names(
    client: Client,
    profile: &Path,
    workspace: &Path,
    baseline: &Value,
) -> Result<Vec<String>, Failure> {
    let mut names: Vec<String> = baseline
        .get("mcpServers")
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let sources = match client {
        Client::Codex => vec![
            profile.join(".codex/config.toml"),
            workspace.join(".codex/config.toml"),
        ],
        Client::ClaudeCode => vec![profile.join(".claude.json"), workspace.join(".mcp.json")],
        Client::FactoryDroid => vec![workspace.join(".factory/mcp.json")],
        Client::KimiCode => vec![workspace.join(".kimi-code/mcp.json")],
        Client::Pi => vec![workspace.join(".pi/mcp.json")],
    };
    for path in sources {
        match fs::read_to_string(&path) {
            Ok(text) => {
                if client == Client::Codex {
                    if text.contains("mcp_servers") && text.contains("locust") {
                        names.push("locust".into());
                    }
                } else {
                    let value: Value = serde_json::from_str(&text).map_err(|_| {
                        Failure::new(
                            ErrorCode::Corrupted,
                            "cannot inspect existing client MCP configuration",
                        )
                    })?;
                    collect_names(&value, &mut names);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => {
                return Err(Failure::invalid(
                    "cannot inspect effective client MCP configuration",
                ));
            }
        }
    }
    Ok(names)
}
fn collect_names(value: &Value, names: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                if k == "mcpServers" {
                    if let Some(map) = v.as_object() {
                        names.extend(map.keys().cloned());
                    }
                } else {
                    collect_names(v, names);
                }
            }
        }
        Value::Array(a) => {
            for v in a {
                collect_names(v, names)
            }
        }
        _ => (),
    }
}

fn observe(
    owned: &mut managed::OwnedLaunch,
    api: &mut LocalClient,
    socket: &Path,
    mut receipt: File,
    initial_failure: Option<Failure>,
    signals: &signals::Listener,
) -> Result<Value, Failure> {
    let (sender, receiver) = mpsc::sync_channel(0);
    let stdout = owned.child_mut().stdout.take().unwrap();
    let stderr = owned.child_mut().stderr.take().unwrap();
    let mut readers = [
        stream_reader(stdout, true, sender.clone()),
        stream_reader(stderr, false, sender.clone()),
    ];
    drop(sender);
    let mut tail = Vec::new();
    let mut saved_error = initial_failure;
    let mut last = owned.record().clone();
    let mut exited = false;
    let mut direct_exited = false;
    while !exited {
        let mut snapshot_consistent = true;
        if !direct_exited {
            if let Err(error) = forward_explicit_signals(owned.child_mut(), &signals.events) {
                saved_error.get_or_insert(error);
            }
            match owned.child_mut().try_wait() {
                Ok(Some(_)) => {
                    direct_exited = true;
                    for reader in &mut readers {
                        reader.stop();
                    }
                }
                Ok(None) => (),
                Err(_) => {
                    saved_error.get_or_insert_with(|| {
                        Failure::internal("cannot observe owned native exit")
                    });
                }
            }
        }
        let mut streams_closed = false;
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok((stdout, Ok(bytes))) => {
                if std::io::stderr().write_all(&bytes).is_err() {
                    saved_error.get_or_insert_with(|| {
                        Failure::internal("cannot forward native client output")
                    });
                }
                if stdout
                    && let Ok(event) = serde_json::from_slice(&bytes)
                    && let Err(e) = owned.observe_native(&event)
                {
                    saved_error.get_or_insert_with(|| adapter(e));
                }
                if stdout
                    && let Ok(event) = serde_json::from_slice(&bytes)
                    && blocked_event(&event)
                    && let Err(e) = owned.blocked()
                {
                    saved_error.get_or_insert_with(|| adapter(e));
                }
            }
            Ok((_, Err(error))) => {
                saved_error.get_or_insert(error);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                streams_closed = true;
                std::thread::sleep(Duration::from_millis(100))
            }
        }
        let mut added = Vec::new();
        use std::io::Read;
        if let Err(_e) = receipt.read_to_end(&mut added) {
            saved_error
                .get_or_insert_with(|| Failure::internal("cannot read lifecycle receipt updates"));
        }
        tail.extend(added);
        while let Some(end) = tail.iter().position(|b| *b == b'\n') {
            let line: Vec<_> = tail.drain(..=end).collect();
            if let Ok(value) = serde_json::from_slice(&line)
                && let Err(e) = owned.observe_tools(&value)
            {
                saved_error.get_or_insert_with(|| adapter(e));
            }
        }
        if let Some(path) = owned.metadata().native_session_file.as_ref()
            && let Ok(file) = File::open(path)
        {
            let mut line = String::new();
            if BufReader::new(file).read_line(&mut line).is_ok()
                && let Ok(event) = serde_json::from_str(&line)
                && let Err(e) = owned.observe_native(&event)
            {
                saved_error.get_or_insert_with(|| adapter(e));
            }
        }
        match session(api, socket) {
            Ok(view) => {
                if let Err(e) = owned.observe_session(&view) {
                    saved_error.get_or_insert_with(|| adapter(e));
                }
                if let Some(goal) = owned.metadata().binding.goal {
                    match pending(api, socket, goal) {
                        Ok(work) => {
                            let binding = owned.metadata().binding.clone();
                            let previous = DeliveryReceipt::decode(&owned.metadata().delivery);
                            match previous.and_then(|previous| {
                                delivery::prepare(
                                    DeliveryBinding {
                                        goal,
                                        principal: binding.principal,
                                        instance: binding.instance,
                                        claim: binding.claim,
                                    },
                                    &view,
                                    work,
                                    previous.as_ref(),
                                )
                            }) {
                                Ok(prepared) => {
                                    if let Ok(value) = prepared.receipt.encode()
                                        && !same_delivery(&owned.metadata().delivery, &value)
                                        && let Err(e) = owned.set_delivery(value)
                                    {
                                        saved_error.get_or_insert_with(|| adapter(e));
                                    }
                                }
                                Err(e) if e.code == ErrorCode::Superseded => {
                                    // Session and Pending are separate reads. A claim or
                                    // cancellation acknowledgment may commit between them.
                                    // Retry fresh snapshots, including before final exit.
                                    snapshot_consistent = false;
                                }
                                Err(e) => {
                                    saved_error.get_or_insert_with(|| Failure::from(e));
                                }
                            }
                        }
                        Err(e) => {
                            saved_error.get_or_insert(e);
                        }
                    }
                }
            }
            Err(e) => {
                saved_error.get_or_insert(e);
            }
        }
        if direct_exited && streams_closed && snapshot_consistent {
            match owned.poll() {
                Ok(done) => exited = done,
                Err(e) => {
                    saved_error.get_or_insert_with(|| adapter(e));
                }
            }
        }
        if *owned.record() != last {
            match report(api, socket, owned.record()) {
                Ok(()) => last = owned.record().clone(),
                Err(e) => {
                    saved_error.get_or_insert(e);
                }
            }
        }
    }
    // Readers were stopped and drained before Exited, preserving initialization
    // already buffered at native exit. Descendants are never signaled here.
    for reader in readers {
        let _ = reader.thread.join();
    }
    if let Some(error) = saved_error {
        return Err(Failure::new(
            error.code,
            format!(
                "client exited; lifecycle observation/report needs reconciliation: {}",
                error.message
            ),
        ));
    }
    Ok(
        json!({"schema":1,"session":owned.record(),"exit_code":owned.metadata().process.as_ref().and_then(|p|p.exit_code),"delivery_mechanism":"ordinary_tool","native_output":"stderr","automatic_wake":false,"cancellation_acknowledged":false}),
    )
}
/// Each explicit local request is forwarded while the Child remains unreaped.
/// No persisted PID, retry timer or implicit escalation authorizes a signal.
fn forward_explicit_signals(
    child: &mut std::process::Child,
    events: &mpsc::Receiver<rustix::process::Signal>,
) -> Result<(), Failure> {
    for signal in events.try_iter() {
        if let Some(pid) = rustix::process::Pid::from_raw(child.id() as i32) {
            rustix::process::kill_process(pid, signal).map_err(|error| {
                Failure::internal(format!("cannot forward explicit local interrupt: {error}"))
            })?;
        }
    }
    Ok(())
}

type NativeMessage = (bool, Result<Vec<u8>, Failure>);
struct NativeReader {
    thread: std::thread::JoinHandle<()>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
}
impl NativeReader {
    fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

fn native_lines(
    partial: &mut Vec<u8>,
    bytes: &[u8],
    stdout: bool,
    sender: &mpsc::SyncSender<NativeMessage>,
) -> Result<(), Failure> {
    partial.extend_from_slice(bytes);
    while let Some(end) = partial.iter().position(|byte| *byte == b'\n') {
        let line = partial.drain(..=end).collect();
        sender
            .send((stdout, Ok(line)))
            .map_err(|_| Failure::internal("native output receiver closed"))?;
    }
    Ok(())
}

fn stream_reader<T: std::os::fd::AsFd + std::os::fd::AsRawFd + Send + 'static>(
    stream: T,
    stdout: bool,
    sender: mpsc::SyncSender<NativeMessage>,
) -> NativeReader {
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let thread = std::thread::spawn(move || {
        let read = || -> Result<(), Failure> {
            let flags = rustix::fs::fcntl_getfl(&stream)
                .map_err(|_| Failure::internal("cannot inspect native pipe"))?;
            rustix::fs::fcntl_setfl(&stream, flags | rustix::fs::OFlags::NONBLOCK)
                .map_err(|_| Failure::internal("cannot configure native pipe"))?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| Failure::internal("cannot initialize native pipe reader"))?;
            runtime.block_on(async {
                let stream = tokio::io::unix::AsyncFd::new(stream)
                    .map_err(|_| Failure::internal("cannot register native pipe"))?;
                let mut partial = Vec::new();
                let mut bytes = [0u8; 8192];
                tokio::pin!(stopped);
                loop {
                    tokio::select! {
                        biased;
                        _ = &mut stopped => {
                            // Snapshot kernel-buffered bytes once. A descendant
                            // continuously writing cannot extend this drain.
                            let mut remaining = rustix::io::ioctl_fionread(stream.get_ref())
                                .map_err(|_| Failure::internal("cannot inspect buffered native output"))?;
                            while remaining > 0 {
                                let capacity = bytes.len().min(usize::try_from(remaining).unwrap_or(usize::MAX));
                                match rustix::io::read(stream.get_ref(), &mut bytes[..capacity]) {
                                    Ok(0) => break,
                                    Ok(count) => {
                                        remaining -= count as u64;
                                        native_lines(&mut partial, &bytes[..count], stdout, &sender)?;
                                    }
                                    Err(rustix::io::Errno::INTR) => continue,
                                    Err(rustix::io::Errno::AGAIN) => break,
                                    Err(_) => return Err(Failure::internal("cannot drain native output")),
                                }
                            }
                            break;
                        }
                        readable = stream.readable() => {
                            let mut ready = readable.map_err(|_| Failure::internal("cannot observe native pipe readiness"))?;
                            match ready.try_io(|fd| rustix::io::read(fd.get_ref(), &mut bytes).map_err(std::io::Error::from)) {
                                Ok(Ok(0)) => break,
                                Ok(Ok(count)) => native_lines(&mut partial, &bytes[..count], stdout, &sender)?,
                                Ok(Err(error)) if error.kind() == std::io::ErrorKind::Interrupted => (),
                                Ok(Err(_)) => return Err(Failure::internal("cannot read native output")),
                                Err(_) => (),
                            }
                        }
                    }
                }
                if !partial.is_empty() {
                    sender.send((stdout, Ok(partial))).map_err(|_| Failure::internal("native output receiver closed"))?;
                }
                Ok(())
            })
        };
        if let Err(error) = read() {
            let _ = sender.send((stdout, Err(error)));
        }
    });
    NativeReader {
        thread,
        stop: Some(stop),
    }
}

fn same_delivery(left: &Value, right: &Value) -> bool {
    fn normalize(value: &Value) -> Value {
        let mut value = value.clone();
        if let Some(pending) = value
            .pointer_mut("/notice/pending")
            .and_then(Value::as_object_mut)
        {
            pending.remove("revision");
        }
        value
    }
    normalize(left) == normalize(right)
}
fn droid_blocked(event: &Value) -> bool {
    if event.get("type").and_then(Value::as_str) != Some("tool_result")
        || event.get("isError").and_then(Value::as_bool) != Some(true)
        || event.pointer("/error/type").and_then(Value::as_str) != Some("tool_error")
    {
        return false;
    }
    let Some(tool) = event.get("toolId").and_then(Value::as_str) else {
        return false;
    };
    event.pointer("/error/message").and_then(Value::as_str)==Some(format!("Error: Exec ended early: MCP tool {tool} requires higher autonomy to proceed. Re-run with --auto medium or --auto high. For destructive commands, use --skip-permissions-unsafe.").as_str())
}
fn claude_rejected(event: &Value) -> bool {
    if event.get("type").and_then(Value::as_str) != Some("user")
        || event.pointer("/message/role").and_then(Value::as_str) != Some("user")
    {
        return false;
    }
    let Some(metadata) = event.get("tool_result_meta").and_then(Value::as_array) else {
        return false;
    };
    let Some(content) = event.pointer("/message/content").and_then(Value::as_array) else {
        return false;
    };
    metadata.iter().any(|meta| {
        meta.get("non_execution_kind").and_then(Value::as_str) == Some("user-rejected")
            && meta.get("id").and_then(Value::as_str).is_some_and(|id| {
                content.iter().any(|result| {
                    result.get("type").and_then(Value::as_str) == Some("tool_result")
                        && result.get("is_error").and_then(Value::as_bool) == Some(true)
                        && result.get("tool_use_id").and_then(Value::as_str) == Some(id)
                })
            })
    })
}
fn blocked_event(event: &Value) -> bool {
    claude_rejected(event)
        || droid_blocked(event)
        || (event.get("type").and_then(Value::as_str) == Some("item.completed")
            && event.pointer("/item/type").and_then(Value::as_str) == Some("mcp_tool_call")
            && event.pointer("/item/status").and_then(Value::as_str) == Some("failed")
            && event.pointer("/item/error/message").and_then(Value::as_str)
                == Some("MCP tool call requires approval, but approval policy is never"))
        || (event.get("type").and_then(Value::as_str) == Some("result")
            && event
                .get("permission_denials")
                .and_then(Value::as_array)
                .is_some_and(|items| !items.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_explicit_signals_reach_retained_child() {
        use std::process::{Command, Stdio};
        struct Reap(std::process::Child);
        impl Drop for Reap {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = Reap(Command::new("/bin/sh")
            .args(["-c", "trap 'echo ignored' INT; trap 'echo terminated; exit 0' TERM; echo ready; while :; do read unused; done"])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap());
        let stdout = child.0.stdout.take().unwrap();
        let (lines, received) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if lines.send(line.unwrap()).is_err() {
                    break;
                }
            }
        });
        let next = || received.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(next(), "ready");
        let (requests, events) = mpsc::channel();
        requests.send(rustix::process::Signal::INT).unwrap();
        forward_explicit_signals(&mut child.0, &events).unwrap();
        assert_eq!(next(), "ignored");
        requests.send(rustix::process::Signal::TERM).unwrap();
        forward_explicit_signals(&mut child.0, &events).unwrap();
        assert_eq!(next(), "terminated");
        assert!(child.0.wait().unwrap().success());
        reader.join().unwrap();
    }

    #[test]
    fn claude_permission_rejection_requires_matching_structured_error() {
        let mut event = json!({"type":"user","tool_result_meta":[{"id":"call_fixture_1","non_execution_kind":"user-rejected"}],"message":{"role":"user","content":[{"type":"tool_result","is_error":true,"tool_use_id":"call_fixture_1","content":"uninterpreted text"}]}});
        assert!(blocked_event(&event));
        event["message"]["content"][0]["tool_use_id"] = json!("another_call");
        assert!(!blocked_event(&event));
        event["message"]["content"][0]["tool_use_id"] = json!("call_fixture_1");
        event["message"]["content"][0]["is_error"] = json!(false);
        assert!(!blocked_event(&event));
        event["message"]["content"][0]["is_error"] = json!(true);
        event["tool_result_meta"][0]["non_execution_kind"] = json!("other");
        assert!(!blocked_event(&event));
        assert!(!blocked_event(
            &json!({"type":"user","message":{"role":"user","content":"Claude requested permissions"}})
        ));
    }
}
