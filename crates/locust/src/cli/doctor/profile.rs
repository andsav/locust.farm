use super::*;
use crate::installation::{
    self,
    onboarding::{self, diagnostics},
    service::{ServiceKind, ServiceSpec},
    service_install,
    setup::{self, Client},
};
use std::path::PathBuf;

fn report(checks: &mut Vec<Value>, name: &str, result: Result<String, Failure>, recovery: &str) {
    check(checks, name, result);
    let entry = checks.last_mut().expect("added check");
    if entry["ok"] != true {
        entry["recovery"] = json!(recovery);
    }
}

fn home() -> Result<PathBuf, Failure> {
    std::env::var_os("HOME")
        .ok_or_else(|| Failure::usage("HOME is unset; select the profile home explicitly"))
        .and_then(|path| installation::absolute(Path::new(&path)))
}
fn path(args: &ArgMatches, name: &str) -> Result<Option<PathBuf>, Failure> {
    args.get_one::<String>(name)
        .map(|value| installation::absolute(Path::new(value)))
        .transpose()
}
fn flag(ok: bool, success: &str, failure: &str) -> Result<String, Failure> {
    if ok {
        Ok(success.into())
    } else {
        Err(Failure::unavailable(failure))
    }
}

fn selected(
    args: &ArgMatches,
    daemon_home: &Path,
    client: Client,
) -> Result<diagnostics::Selected, Failure> {
    let explicit = path(args, "profile-home")?;
    let profile = explicit.clone().map(Ok).unwrap_or_else(home)?;
    // Match onboarding's standard-profile selection: do not silently inspect
    // HOME when the native client selects a different configuration directory.
    if explicit.is_none() {
        let (variable, suffix) = match client {
            Client::Codex => ("CODEX_HOME", ".codex"),
            Client::Claude => ("CLAUDE_CONFIG_DIR", ".claude"),
            Client::Pi => ("PI_CODING_AGENT_DIR", ".pi/agent"),
        };
        if let Some(value) = std::env::var_os(variable).filter(|value| !value.is_empty())
            && (client == Client::Claude
                || installation::absolute(Path::new(&value))?
                    != installation::absolute(&profile.join(suffix))?)
        {
            return Err(Failure::usage(format!(
                "{variable} selects another configuration layout; select --profile-home explicitly for the standard layout"
            )));
        }
    }
    let selected = diagnostics::select(daemon_home, client, &profile)?;
    for (option, recorded) in [
        ("prefix", &selected.spec.prefix),
        ("workspace", &selected.spec.workspace),
    ] {
        if path(args, option)?
            .as_ref()
            .is_some_and(|value| value != recorded)
        {
            return Err(Failure::usage(format!(
                "--{option} differs from the selected onboarding journal ({})",
                recorded.display()
            )));
        }
    }
    Ok(selected)
}

pub(super) fn inspect(args: &ArgMatches, daemon_home: &Path, checks: &mut Vec<Value>) -> Value {
    let client = args
        .get_one::<String>("client")
        .map(|value| match value.as_str() {
            "codex" => Client::Codex,
            "claude" => Client::Claude,
            _ => Client::Pi,
        });
    let selected = client.map(|client| selected(args, daemon_home, client));
    if let Some(selected) = &selected {
        report(
            checks,
            "onboarding_journal",
            selected
                .as_ref()
                .map(|selected| {
                    format!(
                        "{} in {}",
                        selected.spec.name.as_deref().unwrap_or("agent"),
                        selected.spec.profile_home.display()
                    )
                })
                .map_err(Clone::clone),
            "Select the original client profile with --profile-home. If onboarding was interrupted, repeat locust up with its original selections to review and resume.",
        );
    }
    let selected = selected.and_then(Result::ok);
    let prefix = selected
        .as_ref()
        .map(|selected| Ok(Some(selected.spec.prefix.clone())))
        .unwrap_or_else(|| path(args, "prefix"));
    let prefix = match prefix {
        Ok(prefix) => prefix,
        Err(error) => {
            report(
                checks,
                "installation",
                Err(error),
                "Select an absolute software prefix with --prefix.",
            );
            None
        }
    };
    let service_selected = ["service", "service-profile-home", "log-dir"]
        .iter()
        .any(|name| args.get_one::<String>(name).is_some());
    if let Some(prefix) = &prefix {
        let installed = installation::status(prefix).and_then(|status| {
            flag(
                status["installed"] == true && status["withdrawn"] == false,
                "installed payloads and signatures verified; release is not withdrawn",
                "selected prefix has no active non-withdrawn release",
            )
        });
        report(
            checks,
            "installation",
            installed,
            "Install a reviewed signed release at the selected --prefix, then rerun doctor.",
        );
        if client.is_some() || service_selected {
            service(args, daemon_home, prefix, checks);
        }
    } else if service_selected && client.is_none() {
        report(
            checks,
            "service",
            Err(Failure::usage(
                "service inspection needs --prefix or an enrolled --client",
            )),
            "Select --prefix or an enrolled --client before inspecting its service.",
        );
    }
    let Some(selected) = selected else {
        return Value::Null;
    };
    let resume = format!(
        "Repeat locust up --client {} with the same --home, --prefix, --profile-home and --workspace selections to review and resume configuration.",
        onboarding::client_name(selected.spec.client)
    );
    report(
        checks,
        "onboarding_complete",
        flag(
            selected.completed(),
            "onboarding journal is complete",
            "onboarding stopped before configuration completed",
        ),
        &resume,
    );
    let identity = selected.identity();
    report(
        checks,
        "agent_identity",
        identity
            .as_ref()
            .map(|_| {
                "saved credential and session authenticate as the enrolled, active agent".into()
            })
            .map_err(Clone::clone),
        "Inspect the saved enrollment and restore its original protected credential/session if changed. For a revoked agent, review owner enrollment; do not generate replacement secrets in this profile.",
    );
    let state = setup::status(&selected.binding);
    let preserve = "Inspect the selected profile's Locust files and preserve any user edits; restore the owned files or review the same locust up selections to resume an interrupted setup.";
    for (name, field, message) in [
        (
            "mcp_registration",
            "mcp_ready",
            "owned Locust MCP entry matches",
        ),
        ("skill", "skill_ready", "owned Locust skill matches"),
        (
            "launcher",
            "launcher_ready",
            "owned bound launcher bytes and mode match",
        ),
        (
            "profile_binding",
            "binding_matches",
            "profile points to the selected executable, daemon, credential and session",
        ),
    ] {
        report(
            checks,
            name,
            state.as_ref().map_err(Clone::clone).and_then(|state| {
                flag(
                    state[field] == true,
                    message,
                    &format!("{name} is missing, changed or belongs to another binding"),
                )
            }),
            preserve,
        );
    }
    report(
        checks,
        "profile_transaction",
        state.as_ref().map_err(Clone::clone).and_then(|state| {
            flag(
                state["pending"] == false,
                "no pending profile transaction",
                "profile setup has a pending transaction",
            )
        }),
        &resume,
    );
    // Plan is read-only and also detects workspace/ancestor configuration
    // collisions; intact files alone do not establish effective discovery.
    report(
        checks,
        "profile_configuration",
        setup::plan(&selected.binding, false)
            .and_then(|plan| plan.json())
            .and_then(|plan| {
                flag(
                    plan["plan"]["files"].as_array().is_some_and(|files| {
                        files.iter().all(|file| file["before"] == file["after"])
                    }),
                    "selected workspace configuration matches the installed release",
                    "profile files require a reviewed update for the installed release",
                )
            }),
        preserve,
    );
    json!({"client":selected.spec.client,"name":selected.spec.name,
        "profile_home":selected.spec.profile_home,"workspace":selected.spec.workspace,"prefix":selected.spec.prefix,
        "identity":identity.ok(),"configuration":state.ok(),"model_ready":false,"discovery":"unverified"})
}

fn service(args: &ArgMatches, daemon_home: &Path, prefix: &Path, checks: &mut Vec<Value>) {
    let result = (|| {
        let kind = match args.get_one::<String>("service").map(String::as_str) {
            Some("none") => ServiceKind::None,
            Some("launchd") => ServiceKind::Launchd,
            Some("systemd") => ServiceKind::Systemd,
            _ if cfg!(target_os = "macos") => ServiceKind::Launchd,
            _ => ServiceKind::Systemd,
        };
        let profile = path(args, "service-profile-home")?
            .map(Ok)
            .unwrap_or_else(home)?;
        let logs = path(args, "log-dir")?.unwrap_or_else(|| daemon_home.join("logs"));
        let spec = ServiceSpec::new(
            kind,
            &profile,
            &prefix.join("current/locust"),
            daemon_home,
            &logs,
        )?;
        let status = service_install::status(prefix, &spec)?;
        let state = status["state"].as_str().unwrap_or("unknown");
        flag(
            state == "running" || state == "disabled",
            if state == "disabled" {
                "service management disabled; daemon API checked separately"
            } else {
                "owned service unit is running; daemon API checked separately"
            },
            &format!("selected service is {state}"),
        )
    })();
    report(
        checks,
        "service",
        result,
        "Repeat locust up with this service, daemon home, service profile and log directory to review/start it, or select --service none when using an existing foreground daemon.",
    );
}
