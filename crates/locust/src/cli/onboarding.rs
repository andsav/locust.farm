//! Person-facing orchestration. Discovery never selects profiles implicitly.
mod discovery;
mod service;

use super::{Output, connection};
use crate::failure::Failure;
use crate::installation::{
    self, onboarding,
    service::{ServiceKind, ServiceSpec, ServiceState},
    service_install,
    setup::Client,
};
use clap::{Arg, ArgAction, ArgMatches, Command};
use locust_proto::api::ErrorCode;
use serde_json::{Value, json};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn common(command: Command) -> Command {
    command
        .arg(
            Arg::new("prefix")
                .long("prefix")
                .help("Verified software prefix (inferred when running its installed executable)"),
        )
        .arg(
            Arg::new("profile-home")
                .long("profile-home")
                .help("Selected client profile home (defaults to HOME)"),
        )
        .arg(Arg::new("workspace").long("workspace").help(
            "Workspace whose ancestor configuration is checked (defaults to current directory)",
        ))
        .arg(
            Arg::new("name").long("name").help(
                "Local agent name for one selected client; otherwise generated once and saved",
            ),
        )
        .arg(
            Arg::new("plan")
                .long("plan")
                .action(ArgAction::SetTrue)
                .conflicts_with("yes")
                .help("Show discovery and selected changes without writing files"),
        )
        .arg(
            Arg::new("yes")
                .long("yes")
                .action(ArgAction::SetTrue)
                .help("Approve reviewed changes for explicitly selected clients without prompting"),
        )
}
pub(super) fn up_command() -> Command {
    common(Command::new("up").about("Start the daemon and review resumable client onboarding"))
        .arg(Arg::new("client").long("client").action(ArgAction::Append).value_delimiter(',').value_parser(["codex","claude","pi"]).help("Select a client; repeat to select more than one in this profile home"))
        .arg(Arg::new("service").long("service").value_parser(["launchd","systemd","none"]).help("Per-user service (defaults to this platform's manager); none uses an existing daemon"))
        .arg(Arg::new("service-profile-home").long("service-profile-home").help("Service-manager profile home (defaults to HOME, independently of client profile)"))
        .arg(Arg::new("log-dir").long("log-dir").help("Private daemon logs directory (defaults to the daemon home's logs directory)"))
        .arg(Arg::new("wait-ms").long("wait-ms").value_parser(clap::value_parser!(u64)).help("Optional maximum readiness wait in milliseconds; 0 checks once. Service startup otherwise waits until ready or interrupted"))
}
pub(super) fn add_command() -> Command {
    common(Command::new("add").about("Enroll and configure one client against the running daemon"))
        .arg(
            Arg::new("client")
                .required(true)
                .value_parser(["codex", "claude", "pi"]),
        )
}
fn client(value: &str) -> Client {
    match value {
        "codex" => Client::Codex,
        "claude" => Client::Claude,
        _ => Client::Pi,
    }
}
fn home() -> Result<PathBuf, Failure> {
    let path = std::env::var_os("HOME").ok_or_else(|| {
        Failure::usage("HOME is unset; select --profile-home and --service-profile-home explicitly")
    })?;
    installation::absolute(Path::new(&path))
}
fn selected_path(
    args: &ArgMatches,
    name: &str,
    fallback: impl FnOnce() -> Result<PathBuf, Failure>,
) -> Result<PathBuf, Failure> {
    match args.get_one::<String>(name) {
        Some(path) => installation::absolute(Path::new(path)),
        None => fallback(),
    }
}
fn inferred_prefix(executable: &Path) -> Result<PathBuf, Failure> {
    let parent = executable
        .parent()
        .ok_or_else(|| Failure::usage("select --prefix for the installed release"))?;
    let prefix = if parent.file_name().is_some_and(|name| name == "current") {
        parent.parent()
    } else if parent
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name == "releases")
    {
        parent.parent().and_then(Path::parent)
    } else {
        None
    };
    prefix.map(installation::absolute).transpose()?.ok_or_else(||Failure::usage("run the installed current/locust executable, or select its software directory with --prefix"))
}
fn reject_agent_authority(matches: &ArgMatches) -> Result<(), Failure> {
    if ["credential", "session", "as", "idempotency-key"]
        .iter()
        .any(|name| matches.get_one::<String>(name).is_some())
    {
        return Err(Failure::usage(
            "up and agent add are owner administration and do not accept --credential, --session, --as or --idempotency-key",
        ));
    }
    Ok(())
}
fn validate_profile_environment(
    profile: &Path,
    selected: &[Client],
    explicit: bool,
) -> Result<(), Failure> {
    if explicit {
        return Ok(());
    }
    for kind in selected {
        let (variable, relative) = match kind {
            Client::Codex => ("CODEX_HOME", ".codex"),
            Client::Claude => ("CLAUDE_CONFIG_DIR", ".claude"),
            Client::Pi => ("PI_CODING_AGENT_DIR", ".pi/agent"),
        };
        if let Some(path) = std::env::var_os(variable).filter(|path| !path.is_empty())
            && (*kind == Client::Claude
                || installation::absolute(Path::new(&path))?
                    != installation::absolute(&profile.join(relative))?)
        {
            // CLAUDE_CONFIG_DIR changes the config-file layout even when it
            // equals HOME/.claude; ordinary HOME discovery omits that variable.
            return Err(Failure::usage(format!(
                "{variable} selects a client-specific config directory; unset it or choose --profile-home explicitly for the supported standard layout, then start the client in that selected profile"
            )));
        }
    }
    Ok(())
}
fn read_line(prompt: &str) -> Result<String, Failure> {
    eprint!("{prompt}");
    io::stderr()
        .flush()
        .map_err(|error| Failure::invalid(error.to_string()))?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|error| Failure::invalid(error.to_string()))?;
    Ok(line.trim().to_owned())
}
fn parse_selection(value: &str) -> Result<Vec<Client>, Failure> {
    let mut selected = Vec::new();
    for word in value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
    {
        if !["codex", "claude", "pi"].contains(&word) {
            return Err(Failure::usage("select client names: codex, claude or pi"));
        }
        let next = client(word);
        if !selected.contains(&next) {
            selected.push(next);
        }
    }
    Ok(selected)
}
fn candidates_text(candidates: &[discovery::Candidate]) -> String {
    candidates
        .iter()
        .map(|candidate| {
            format!(
                "{}: {}\n  profile configuration: {}",
                onboarding::client_name(candidate.client),
                candidate
                    .executable
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "command not found on PATH".into()),
                candidate.config.display()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn plan_text(value: &Value) -> String {
    let plan = &value["plan"];
    match plan["format"].as_str() {
        Some("locust-service-plan-v1") => format!(
            "Service: {}\n  unit: {}",
            plan["action"].as_str().unwrap_or("review"),
            plan["unit_path"].as_str().unwrap_or("none")
        ),
        Some("locust-setup-plan-v2") => {
            let files = plan["files"]
                .as_array()
                .map(|files| {
                    files
                        .iter()
                        .filter_map(|file| file["path"].as_str())
                        .map(|path| format!("  {path}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            format!(
                "Client setup: {}\n{files}\n  credential: {}\n  session: {}",
                plan["spec"]["client"].as_str().unwrap_or("client"),
                plan["spec"]["credential"].as_str().unwrap_or(""),
                plan["spec"]["session"].as_str().unwrap_or("")
            )
        }
        _ => {
            let spec = &plan["spec"];
            format!(
                "Agent {} ({})\n  profile: {}\n  workspace: {}\n  daemon: {}",
                spec["name"].as_str().unwrap_or("new"),
                spec["client"].as_str().unwrap_or("client"),
                spec["profile_home"].as_str().unwrap_or(""),
                spec["workspace"].as_str().unwrap_or(""),
                spec["daemon_home"].as_str().unwrap_or("")
            )
        }
    }
}
struct Reviewer {
    yes: bool,
    interactive: bool,
}
impl Reviewer {
    fn confirm(&mut self, value: &Value) -> Result<bool, Failure> {
        eprintln!("{}", plan_text(value));
        if self.yes {
            return Ok(true);
        }
        if !self.interactive {
            return Ok(false);
        }
        Ok(matches!(
            read_line("Apply these changes? [y/N] ")?
                .to_ascii_lowercase()
                .as_str(),
            "y" | "yes"
        ))
    }
}

fn wait_ready(
    home: &Path,
    service: Option<(&Path, &ServiceSpec)>,
    limit: Option<Duration>,
    wait: bool,
) -> Result<Value, Failure> {
    let start = Instant::now();
    loop {
        let result = onboarding::owner_ready(
            home,
            limit.map(|limit| limit.saturating_sub(start.elapsed())),
        );
        match result {
            Ok(value) => return Ok(value),
            Err(error) => {
                if error.code != ErrorCode::Unavailable {
                    return Err(error);
                }
                if !wait || limit.is_some_and(|limit| start.elapsed() >= limit) {
                    return Err(Failure::unavailable(format!(
                        "daemon API is not ready: {}; inspect the daemon logs and repeat the same command to resume",
                        error.message
                    )));
                }
            }
        }
        if let Some((prefix, spec)) = service {
            // A just-accepted manager start can still read as stopped/loaded.
            // Only a manager error aborts; state observations never restart it.
            service_install::status(prefix, spec)?;
        }
        let delay = limit
            .map(|limit| {
                limit
                    .saturating_sub(start.elapsed())
                    .min(Duration::from_millis(100))
            })
            .unwrap_or(Duration::from_millis(100));
        std::thread::sleep(delay);
    }
}

pub(super) fn run(
    matches: &ArgMatches,
    operation: &str,
    args: &ArgMatches,
) -> Result<Output, Failure> {
    reject_agent_authority(matches)?;
    let interactive =
        io::stdin().is_terminal() && io::stderr().is_terminal() && !matches.get_flag("json");
    let profile = selected_path(args, "profile-home", home)?;
    let workspace = selected_path(args, "workspace", || {
        std::env::current_dir()
            .map_err(|error| Failure::invalid(error.to_string()))
            .and_then(|path| installation::absolute(&path))
    })?;
    let candidates = discovery::discover(&profile, std::env::var_os("PATH").as_deref());
    let mut selected = if operation == "agent.add" {
        vec![client(
            args.get_one::<String>("client").expect("required client"),
        )]
    } else {
        args.get_many::<String>("client")
            .map(|values| values.map(|value| client(value)).collect())
            .unwrap_or_default()
    };
    let mut seen = Vec::new();
    selected.retain(|client| {
        if seen.contains(client) {
            false
        } else {
            seen.push(*client);
            true
        }
    });
    if selected.is_empty() {
        if interactive && !args.get_flag("plan") && !args.get_flag("yes") {
            eprintln!("{}", candidates_text(&candidates));
            selected = parse_selection(&read_line(
                "Clients to configure (names separated by spaces; empty cancels): ",
            )?)?;
        }
        if selected.is_empty() {
            let human = format!(
                "{}\n\nSelect clients with --client codex, --client claude or --client pi. No profile has been selected.",
                candidates_text(&candidates)
            );
            if args.get_flag("yes") {
                return Err(Failure::usage(
                    "--yes requires explicit --client selection; discovery alone does not authorize profile changes",
                ));
            }
            return Ok(Output::success(
                json!({"action":"select_clients","candidates":candidates,"changed":false}),
                human,
            ));
        }
    }
    if args.get_one::<String>("name").is_some() && selected.len() != 1 {
        return Err(Failure::usage(
            "--name requires exactly one selected client",
        ));
    }
    validate_profile_environment(
        &profile,
        &selected,
        args.get_one::<String>("profile-home").is_some(),
    )?;
    let prefix = selected_path(args, "prefix", || {
        std::env::current_exe()
            .map_err(|error| Failure::invalid(error.to_string()))
            .and_then(|path| inferred_prefix(&path))
    })?;
    let daemon_home = installation::absolute(&connection::home(matches)?)?;
    let mut plans = Vec::new();
    for client in &selected {
        plans.push(onboarding::plan(&onboarding::Spec {
            prefix: prefix.clone(),
            client: *client,
            profile_home: profile.clone(),
            workspace: workspace.clone(),
            daemon_home: daemon_home.clone(),
            name: args.get_one::<String>("name").cloned(),
        })?);
    }
    let service_spec = if operation == "up" {
        let kind = match args.get_one::<String>("service").map(String::as_str) {
            Some("none") => ServiceKind::None,
            Some("launchd") => ServiceKind::Launchd,
            Some("systemd") => ServiceKind::Systemd,
            _ => {
                if cfg!(target_os = "macos") {
                    ServiceKind::Launchd
                } else {
                    ServiceKind::Systemd
                }
            }
        };
        let service_home = selected_path(args, "service-profile-home", home)?;
        let logs = selected_path(args, "log-dir", || Ok(daemon_home.join("logs")))?;
        Some(ServiceSpec::new(
            kind,
            &service_home,
            &prefix.join("current/locust"),
            &daemon_home,
            &logs,
        )?)
    } else {
        None
    };
    let service_plan = service_spec
        .as_ref()
        .map(|spec| service_install::plan(&prefix, spec, false)?.json())
        .transpose()?;
    let plan_values = plans
        .iter()
        .map(onboarding::Plan::json)
        .collect::<Result<Vec<_>, _>>()?;
    let review_output = json!({"action":"review_required","candidates":candidates,"service":service_plan,"clients":plan_values,"changed":false,"grants_added":false});
    let human = format!(
        "Software: {}\n{}{}\nNo work permission is added.{}",
        prefix.display(),
        service_plan
            .as_ref()
            .map(|plan| format!("{}\n", plan_text(plan)))
            .unwrap_or_default(),
        plan_values
            .iter()
            .map(plan_text)
            .collect::<Vec<_>>()
            .join("\n"),
        if args.get_flag("plan") || args.get_flag("yes") || interactive {
            ""
        } else {
            "\nRepeat with --yes to approve these selections, or run interactively to review each stage."
        }
    );
    if args.get_flag("plan") || (!args.get_flag("yes") && !interactive) {
        return Ok(Output::success(review_output, human));
    }
    eprintln!("{human}");
    let mut reviewer = Reviewer {
        yes: args.get_flag("yes"),
        interactive,
    };
    if !reviewer.yes
        && !matches!(
            read_line("Prepare these selected agents? [y/N] ")?
                .to_ascii_lowercase()
                .as_str(),
            "y" | "yes"
        )
    {
        return Err(Failure::new(
            ErrorCode::Denied,
            "onboarding selections were declined; no changes applied",
        ));
    }
    let mut service_state = ServiceState::Disabled;
    if let Some(spec) = &service_spec {
        service_state = service::prepare(&prefix, spec, &mut |value| reviewer.confirm(value))?;
    }
    let limit = if operation == "up" {
        args.get_one::<u64>("wait-ms")
            .map(|ms| Duration::from_millis(*ms))
    } else {
        None
    };
    let managed = service_spec
        .as_ref()
        .is_some_and(|spec| spec.kind() != ServiceKind::None);
    if managed && service_state != ServiceState::Running {
        eprintln!(
            "Waiting for daemon readiness. Interrupting is safe; repeat the same command to resume."
        );
    }
    let daemon = wait_ready(
        &daemon_home,
        service_spec
            .as_ref()
            .filter(|spec| spec.kind() != ServiceKind::None)
            .map(|spec| (prefix.as_path(), spec)),
        limit,
        managed || limit.is_some(),
    )?;
    let mut results = Vec::new();
    for plan in &plans {
        // The plans were reviewed before service mutation; enrollment checks
        // their exact current inputs again under the independent home lock.
        let mut result = onboarding::apply(plan, &mut |value| reviewer.confirm(value))?;
        let command = candidates
            .iter()
            .find(|candidate| candidate.client == plan.spec.client)
            .and_then(|candidate| candidate.executable.as_ref());
        result["client_command_available"] = json!(command.is_some());
        result["client_command"] = json!(command);
        if command.is_none() {
            result["next_action"] = json!(format!(
                "Install the {} client command, then start a fresh chat in the selected profile and verify Locust tool discovery. Goal membership and work authorization remain separate owner actions.",
                onboarding::client_name(plan.spec.client)
            ));
        }
        results.push(result);
    }
    let human = results
        .iter()
        .map(|result| {
            format!(
                "{} ({}) is configured. Daemon connection verified.\n  launcher: {}\n  {}",
                result["name"].as_str().unwrap_or("agent"),
                result["client"].as_str().unwrap_or("client"),
                result["launcher"].as_str().unwrap_or(""),
                result["next_action"].as_str().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Output::success(
        json!({"configuration_ready":true,"daemon_api_ready":true,"model_ready":false,"daemon":daemon,"clients":results,"grants_added":false}),
        human,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readiness_does_not_hide_a_denied_home_behind_a_missing_owner() {
        use std::os::unix::fs::PermissionsExt;
        let directory = crate::testdir::short_dir();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        let error = wait_ready(
            directory.path(),
            None,
            Some(Duration::from_millis(20)),
            true,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Denied);
    }

    #[test]
    fn readiness_budget_also_bounds_an_unresponsive_socket_hello() {
        use std::os::unix::net::UnixListener;
        use std::sync::mpsc;
        let directory = crate::testdir::short_dir();
        let home = directory.path().to_path_buf();
        crate::secret::read_or_create(&locust_proto::local::owner_credential_path(&home)).unwrap();
        let listener =
            UnixListener::bind(locust_proto::local::socket_path(&home).unwrap()).unwrap();
        let (release, stop) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            stop.recv().unwrap();
        });
        let (send, result) = mpsc::channel();
        let probe = std::thread::spawn(move || {
            send.send(wait_ready(
                &home,
                None,
                Some(Duration::from_millis(20)),
                true,
            ))
            .unwrap();
        });
        let observed = result.recv_timeout(Duration::from_secs(2));
        release.send(()).unwrap();
        server.join().unwrap();
        probe.join().unwrap();
        let error = observed
            .expect("readiness must return before the socket peer is released")
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::Unavailable);
    }
    #[test]
    fn prefix_inference_accepts_only_installed_layouts() {
        let expected = installation::absolute(Path::new("/tmp/software")).unwrap();
        assert_eq!(
            inferred_prefix(Path::new("/tmp/software/current/locust")).unwrap(),
            expected
        );
        assert_eq!(
            inferred_prefix(Path::new("/tmp/software/releases/hash/locust")).unwrap(),
            expected
        );
        assert!(inferred_prefix(Path::new("/tmp/locust")).is_err());
    }
    #[test]
    fn client_selection_is_explicit_and_deduplicated() {
        assert_eq!(
            parse_selection("codex, claude codex").unwrap(),
            vec![Client::Codex, Client::Claude]
        );
        assert!(parse_selection("all").is_err());
        assert!(parse_selection("").unwrap().is_empty());
    }
    #[test]
    fn commands_parse_without_an_owner_or_credential_flag() {
        let up = super::super::args::command()
            .try_get_matches_from(["locust", "up", "--client", "codex,claude", "--plan"])
            .unwrap();
        assert_eq!(super::super::args::selected(&up).0, "up");
        let add = super::super::args::command()
            .try_get_matches_from(["locust", "agent", "add", "codex", "--yes"])
            .unwrap();
        assert_eq!(super::super::args::selected(&add).0, "agent.add");
        let bound = super::super::args::command()
            .try_get_matches_from([
                "locust",
                "--credential",
                "/agent",
                "up",
                "--client",
                "codex",
            ])
            .unwrap();
        assert!(reject_agent_authority(&bound).is_err());
    }
}
