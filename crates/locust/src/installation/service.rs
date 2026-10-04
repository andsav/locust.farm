//! Native per-user service descriptions and explicit control.
//!
//! Rendering never writes a profile, starts a process, or grants ownership.
//! The installer owns atomic writes, private directories, byte-for-byte unit
//! records, and the decision to call [`ServiceSpec::run_control`].

use crate::failure::Failure;
use crate::package;
use locust_proto::api::ErrorCode;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceKind {
    None,
    Launchd,
    Systemd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceAction {
    Start,
    Stop,
    Status,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceState {
    Disabled,
    Running,
    /// Registered with launchd, but its process is not running.
    Loaded,
    Stopped,
}

/// A program and separate arguments. No entry is interpreted by a shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    pub program: &'static str,
    pub args: Vec<String>,
}

impl CommandSpec {
    fn new(program: &'static str, args: impl IntoIterator<Item = String>) -> Self {
        Self {
            program,
            args: args.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceSpec {
    kind: ServiceKind,
    /// The same home-derived label on both hosts; the file extension differs.
    label: String,
    unit_path: PathBuf,
    profile_home: PathBuf,
    executable: PathBuf,
    daemon_home: PathBuf,
    log_dir: PathBuf,
}

fn path_text<'a>(path: &'a Path, role: &str) -> Result<&'a str, Failure> {
    let text = path
        .to_str()
        .ok_or_else(|| Failure::invalid(format!("{role} must be UTF-8")))?;
    if !path.is_absolute()
        || text.chars().any(char::is_control)
        || text
            .split('/')
            .skip(1)
            .any(|part| part.is_empty() || part == "." || part == "..")
        || path
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err(Failure::invalid(format!(
            "{role} must be an absolute path without control characters or dot components"
        )));
    }
    Ok(text)
}

fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Quote one systemd ExecStart argument, including its two expansion passes.
/// `%%` is a literal percent specifier; `$$` is a literal dollar sign.
fn systemd_arg(text: &str) -> String {
    let mut result = String::from("\"");
    for character in text.chars() {
        match character {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '%' => result.push_str("%%"),
            '$' => result.push_str("$$"),
            _ => result.push(character),
        }
    }
    result.push('"');
    result
}

/// Environment= does not perform ExecStart's `$` expansion.
fn systemd_env(text: &str) -> String {
    let mut result = String::from("\"");
    for character in text.chars() {
        match character {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '%' => result.push_str("%%"),
            _ => result.push(character),
        }
    }
    result.push('"');
    result
}

fn systemd_log_path(path: &Path) -> Result<String, Failure> {
    let text = path_text(path, "log path")?;
    // StandardOutput=append:path is a single scalar, not an ExecStart word.
    // Quotes and backslashes have parser-dependent meaning here; refuse them.
    if text.contains('"') || text.contains('\\') {
        return Err(Failure::invalid(
            "systemd log paths cannot contain quotes or backslashes",
        ));
    }
    Ok(text.replace('%', "%%"))
}

impl ServiceSpec {
    pub fn kind(&self) -> ServiceKind {
        self.kind
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn unit_path(&self) -> &Path {
        &self.unit_path
    }

    pub fn profile_home(&self) -> &Path {
        &self.profile_home
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub fn daemon_home(&self) -> &Path {
        &self.daemon_home
    }

    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }

    pub fn new(
        kind: ServiceKind,
        profile_home: &Path,
        executable: &Path,
        daemon_home: &Path,
        log_dir: &Path,
    ) -> Result<Self, Failure> {
        Self::new_for_host(
            kind,
            profile_home,
            executable,
            daemon_home,
            log_dir,
            std::env::consts::OS,
            std::env::consts::ARCH,
        )
    }

    pub(super) fn new_for_host(
        kind: ServiceKind,
        profile_home: &Path,
        executable: &Path,
        daemon_home: &Path,
        log_dir: &Path,
        os: &str,
        arch: &str,
    ) -> Result<Self, Failure> {
        let profile = path_text(profile_home, "profile home")?;
        path_text(executable, "installed executable")?;
        let home = path_text(daemon_home, "daemon home")?;
        path_text(log_dir, "log directory")?;
        if matches!(kind, ServiceKind::Launchd) && (os, arch) != ("macos", "aarch64")
            || matches!(kind, ServiceKind::Systemd) && (os, arch) != ("linux", "x86_64")
        {
            return Err(Failure::new(
                ErrorCode::UnsupportedVersion,
                "selected service manager is unsupported on this host",
            ));
        }
        let hash = package::sha256(home.as_bytes());
        let label = format!("farm.locust.{}", &hash[..16]);
        let unit_path = match kind {
            ServiceKind::None => PathBuf::new(),
            ServiceKind::Launchd => Path::new(profile)
                .join("Library/LaunchAgents")
                .join(format!("{label}.plist")),
            ServiceKind::Systemd => Path::new(profile)
                .join(".config/systemd/user")
                .join(format!("{label}.service")),
        };
        Ok(Self {
            kind,
            label,
            unit_path,
            profile_home: profile_home.to_path_buf(),
            executable: executable.to_path_buf(),
            daemon_home: daemon_home.to_path_buf(),
            log_dir: log_dir.to_path_buf(),
        })
    }

    pub fn render(&self) -> Result<Vec<u8>, Failure> {
        let executable = path_text(&self.executable, "installed executable")?;
        let home = path_text(&self.daemon_home, "daemon home")?;
        let profile = path_text(&self.profile_home, "profile home")?;
        let config_home = self.profile_home.join(".config");
        let data_home = self.profile_home.join(".local/share");
        let stdout = self.log_dir.join("stdout.log");
        let stderr = self.log_dir.join("stderr.log");
        let text = match self.kind {
            ServiceKind::None => return Ok(Vec::new()),
            ServiceKind::Launchd => format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
                 <plist version=\"1.0\"><dict>\n\
                 <key>Label</key><string>{}</string>\n\
                 <key>ProgramArguments</key><array><string>{}</string><string>--home</string><string>{}</string><string>daemon</string><string>run</string></array>\n\
                 <key>EnvironmentVariables</key><dict><key>HOME</key><string>{}</string><key>XDG_CONFIG_HOME</key><string>{}</string><key>XDG_DATA_HOME</key><string>{}</string></dict>\n\
                 <key>RunAtLoad</key><true/>\n\
                 <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>\n\
                 <key>StandardOutPath</key><string>{}</string>\n\
                 <key>StandardErrorPath</key><string>{}</string>\n\
                 </dict></plist>\n",
                xml(&self.label),
                xml(executable),
                xml(home),
                xml(profile),
                xml(path_text(&config_home, "XDG config home")?),
                xml(path_text(&data_home, "XDG data home")?),
                xml(path_text(&stdout, "stdout path")?),
                xml(path_text(&stderr, "stderr path")?),
            ),
            ServiceKind::Systemd => format!(
                "[Unit]\nDescription=Locust daemon\n[Service]\nType=exec\nExecStart={} --home {} daemon run\nEnvironment={} {} {}\nRestart=on-failure\nRestartSec=2s\nStandardOutput=append:{}\nStandardError=append:{}\n[Install]\nWantedBy=default.target\n",
                systemd_arg(executable),
                systemd_arg(home),
                systemd_env(&format!("HOME={profile}")),
                systemd_env(&format!(
                    "XDG_CONFIG_HOME={}",
                    path_text(&config_home, "XDG config home")?
                )),
                systemd_env(&format!(
                    "XDG_DATA_HOME={}",
                    path_text(&data_home, "XDG data home")?
                )),
                systemd_log_path(&stdout)?,
                systemd_log_path(&stderr)?,
            ),
        };
        Ok(text.into_bytes())
    }

    fn launchd_domain() -> String {
        format!("gui/{}", rustix::process::getuid().as_raw())
    }

    fn unit_name(&self) -> String {
        format!("{}.service", self.label)
    }

    fn launchd_state(&self, output: &str, domain: &str) -> Result<ServiceState, Failure> {
        let expected_header = format!("{domain}/{} = {{", self.label);
        if output.lines().next() != Some(expected_header.as_str()) {
            return Err(Failure::unavailable(
                "launchd service identity is ambiguous",
            ));
        }
        let field = |name: &str| -> Result<&str, Failure> {
            let prefix = format!("\t{name} = ");
            let mut values = output.lines().filter_map(|line| line.strip_prefix(&prefix));
            let value = values
                .next()
                .ok_or_else(|| Failure::unavailable("launchd service details are incomplete"))?;
            if values.next().is_some() {
                return Err(Failure::unavailable(
                    "launchd service details are ambiguous",
                ));
            }
            Ok(value)
        };
        let expected_path = self
            .unit_path
            .to_str()
            .ok_or_else(|| Failure::invalid("service unit path is not UTF-8"))?;
        let expected_program = self
            .executable
            .to_str()
            .ok_or_else(|| Failure::invalid("service executable path is not UTF-8"))?;
        let expected_home = self
            .daemon_home
            .to_str()
            .ok_or_else(|| Failure::invalid("service home path is not UTF-8"))?;
        let stdout = self.log_dir.join("stdout.log");
        let stderr = self.log_dir.join("stderr.log");
        if field("path")? != expected_path
            || field("type")? != "LaunchAgent"
            || field("program")? != expected_program
            || field("stdout path")? != stdout.to_string_lossy()
            || field("stderr path")? != stderr.to_string_lossy()
        {
            return Err(Failure::new(
                ErrorCode::Conflict,
                "launchd label is loaded from another unit definition",
            ));
        }
        let mut lines = output.lines();
        let mut arguments = None;
        while let Some(line) = lines.next() {
            if line == "\targuments = {" {
                if arguments.is_some() {
                    return Err(Failure::unavailable("launchd arguments are ambiguous"));
                }
                let mut values = Vec::new();
                loop {
                    let next = lines
                        .next()
                        .ok_or_else(|| Failure::unavailable("launchd arguments are incomplete"))?;
                    if next == "\t}" {
                        break;
                    }
                    let value = next
                        .strip_prefix("\t\t")
                        .ok_or_else(|| Failure::unavailable("launchd arguments are ambiguous"))?;
                    values.push(value);
                }
                arguments = Some(values);
            }
        }
        if arguments.as_deref()
            != Some(&[expected_program, "--home", expected_home, "daemon", "run"][..])
        {
            return Err(Failure::new(
                ErrorCode::Conflict,
                "launchd label has different program arguments",
            ));
        }
        let mut lines = output.lines();
        let mut environment = None;
        while let Some(line) = lines.next() {
            if line == "\tenvironment = {" {
                if environment.is_some() {
                    return Err(Failure::unavailable("launchd environment is ambiguous"));
                }
                let mut values = Vec::new();
                loop {
                    let next = lines
                        .next()
                        .ok_or_else(|| Failure::unavailable("launchd environment is incomplete"))?;
                    if next == "\t}" {
                        break;
                    }
                    let pair = next
                        .strip_prefix("\t\t")
                        .and_then(|line| line.split_once(" => "))
                        .ok_or_else(|| Failure::unavailable("launchd environment is ambiguous"))?;
                    values.push(pair);
                }
                environment = Some(values);
            }
        }
        let values =
            environment.ok_or_else(|| Failure::unavailable("launchd environment is absent"))?;
        for (name, expected) in [
            ("HOME", self.profile_home.to_string_lossy().into_owned()),
            (
                "XDG_CONFIG_HOME",
                self.profile_home
                    .join(".config")
                    .to_string_lossy()
                    .into_owned(),
            ),
            (
                "XDG_DATA_HOME",
                self.profile_home
                    .join(".local/share")
                    .to_string_lossy()
                    .into_owned(),
            ),
        ] {
            if values.iter().filter(|(key, _)| *key == name).count() != 1
                || values
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| *value)
                    != Some(expected.as_str())
            {
                return Err(Failure::new(
                    ErrorCode::Conflict,
                    "launchd label has different profile environment",
                ));
            }
        }
        Ok(if field("state")? == "running" {
            ServiceState::Running
        } else {
            ServiceState::Loaded
        })
    }

    pub fn status_commands(&self) -> Vec<CommandSpec> {
        match self.kind {
            ServiceKind::None => Vec::new(),
            ServiceKind::Launchd => {
                let domain = Self::launchd_domain();
                vec![
                    CommandSpec::new("/bin/launchctl", ["print".into(), domain.clone()]),
                    CommandSpec::new(
                        "/bin/launchctl",
                        ["print".into(), format!("{domain}/{}", self.label)],
                    ),
                ]
            }
            ServiceKind::Systemd => vec![CommandSpec::new(
                "/usr/bin/systemctl",
                [
                    "--user".into(),
                    "show".into(),
                    "--property=LoadState".into(),
                    "--property=ActiveState".into(),
                    "--property=FragmentPath".into(),
                    self.unit_name(),
                ],
            )],
        }
    }

    /// An explicit start of an already-running unit restarts that owned unit.
    pub fn start_commands(&self, already_loaded: bool) -> Vec<CommandSpec> {
        match self.kind {
            ServiceKind::None => Vec::new(),
            ServiceKind::Launchd => {
                let domain = Self::launchd_domain();
                if already_loaded {
                    vec![CommandSpec::new(
                        "/bin/launchctl",
                        [
                            "kickstart".into(),
                            "-k".into(),
                            format!("{domain}/{}", self.label),
                        ],
                    )]
                } else {
                    vec![CommandSpec::new(
                        "/bin/launchctl",
                        [
                            "bootstrap".into(),
                            domain,
                            self.unit_path.display().to_string(),
                        ],
                    )]
                }
            }
            ServiceKind::Systemd => {
                let mut commands = vec![CommandSpec::new(
                    "/usr/bin/systemctl",
                    ["--user".into(), "daemon-reload".into()],
                )];
                if already_loaded {
                    commands.push(CommandSpec::new(
                        "/usr/bin/systemctl",
                        ["--user".into(), "enable".into(), self.unit_name()],
                    ));
                    commands.push(CommandSpec::new(
                        "/usr/bin/systemctl",
                        ["--user".into(), "restart".into(), self.unit_name()],
                    ));
                } else {
                    commands.push(CommandSpec::new(
                        "/usr/bin/systemctl",
                        [
                            "--user".into(),
                            "enable".into(),
                            "--now".into(),
                            self.unit_name(),
                        ],
                    ));
                }
                commands
            }
        }
    }

    pub fn stop_commands(&self) -> Vec<CommandSpec> {
        match self.kind {
            ServiceKind::None => Vec::new(),
            ServiceKind::Launchd => vec![CommandSpec::new(
                "/bin/launchctl",
                [
                    "bootout".into(),
                    format!("{}/{}", Self::launchd_domain(), self.label),
                ],
            )],
            ServiceKind::Systemd => vec![
                CommandSpec::new(
                    "/usr/bin/systemctl",
                    [
                        "--user".into(),
                        "disable".into(),
                        "--now".into(),
                        self.unit_name(),
                    ],
                ),
                CommandSpec::new(
                    "/usr/bin/systemctl",
                    ["--user".into(), "daemon-reload".into()],
                ),
            ],
        }
    }

    pub fn run_control(
        &self,
        action: ServiceAction,
        ownership_claimed: bool,
    ) -> Result<ServiceState, Failure> {
        self.run_control_with(action, ownership_claimed, |command| {
            let output = Command::new(command.program).args(&command.args).output()?;
            Ok(CommandResult {
                success: output.status.success(),
                code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        })
    }

    fn run_control_with(
        &self,
        action: ServiceAction,
        ownership_claimed: bool,
        mut runner: impl FnMut(&CommandSpec) -> io::Result<CommandResult>,
    ) -> Result<ServiceState, Failure> {
        if self.kind == ServiceKind::None {
            return Ok(ServiceState::Disabled);
        }
        if action != ServiceAction::Status && !ownership_claimed {
            return Err(Failure::new(
                ErrorCode::Conflict,
                "service control requires an installer ownership record for this exact unit",
            ));
        }
        let status_commands = self.status_commands();
        let probe = run(&mut runner, &status_commands[0])?;
        let state = match self.kind {
            ServiceKind::Launchd => {
                if !probe.success {
                    return Err(Failure::unavailable(
                        "launchd GUI user domain is unavailable",
                    ));
                }
                let service = run(&mut runner, &status_commands[1])?;
                if service.success {
                    self.launchd_state(&service.stdout, &Self::launchd_domain())?
                } else if service.code == Some(113) {
                    // launchctl's no-such-service status, after the GUI domain
                    // itself was successfully checked above.
                    ServiceState::Stopped
                } else {
                    return Err(Failure::unavailable("cannot inspect launchd service label"));
                }
            }
            ServiceKind::Systemd => {
                if !probe.success {
                    return Err(Failure::unavailable("systemd user manager is unavailable"));
                }
                let active = probe
                    .stdout
                    .lines()
                    .any(|line| line == "ActiveState=active");
                self.check_systemd_fragment(&probe, active)?;
                if active {
                    ServiceState::Running
                } else {
                    ServiceState::Stopped
                }
            }
            ServiceKind::None => unreachable!(),
        };
        let systemd_absent = if self.kind == ServiceKind::Systemd && action == ServiceAction::Stop {
            let fragment = systemd_property(&probe.stdout, "FragmentPath")?;
            if fragment.is_empty() {
                if systemd_property(&probe.stdout, "LoadState")? == "not-found"
                    && systemd_property(&probe.stdout, "ActiveState")? == "inactive"
                {
                    true
                } else {
                    return Err(Failure::new(
                        ErrorCode::Conflict,
                        "systemd label has no installed unit fragment; refusing to stop another unit",
                    ));
                }
            } else {
                false
            }
        } else {
            false
        };
        match action {
            ServiceAction::Status => Ok(state),
            ServiceAction::Start => {
                for (index, command) in self
                    .start_commands(state != ServiceState::Stopped)
                    .into_iter()
                    .enumerate()
                {
                    require_success(run(&mut runner, &command)?, "service start")?;
                    if self.kind == ServiceKind::Systemd && index == 0 {
                        // Reload may reveal a higher-priority unit file. Never
                        // start that unit under our derived label.
                        let refreshed = run(&mut runner, &self.status_commands()[0])?;
                        if !refreshed.success {
                            return Err(Failure::unavailable(
                                "systemd user manager is unavailable",
                            ));
                        }
                        self.check_systemd_fragment(&refreshed, true)?;
                    }
                }
                if self.run_control_with(ServiceAction::Status, false, runner)?
                    != ServiceState::Running
                {
                    return Err(Failure::unavailable(
                        "service has not reached running state",
                    ));
                }
                Ok(ServiceState::Running)
            }
            ServiceAction::Stop => {
                if systemd_absent {
                    return Ok(ServiceState::Stopped);
                }
                if state != ServiceState::Stopped || self.kind == ServiceKind::Systemd {
                    for command in self.stop_commands() {
                        require_success(run(&mut runner, &command)?, "service stop")?;
                    }
                }
                if self.run_control_with(ServiceAction::Status, false, runner)?
                    != ServiceState::Stopped
                {
                    return Err(Failure::unavailable("service remains loaded after stop"));
                }
                Ok(ServiceState::Stopped)
            }
        }
    }

    fn check_systemd_fragment(
        &self,
        probe: &CommandResult,
        require_expected: bool,
    ) -> Result<(), Failure> {
        let fragment = systemd_property(&probe.stdout, "FragmentPath")?;
        if !fragment.is_empty() && fragment != self.unit_path.to_string_lossy() {
            return Err(Failure::new(
                ErrorCode::Conflict,
                "service label is loaded from another unit path",
            ));
        }
        if require_expected && fragment.is_empty() {
            return Err(Failure::unavailable(
                "systemd has not loaded the installed user unit",
            ));
        }
        Ok(())
    }
}

fn systemd_property<'a>(output: &'a str, name: &str) -> Result<&'a str, Failure> {
    let prefix = format!("{name}=");
    let mut values = output.lines().filter_map(|line| line.strip_prefix(&prefix));
    let value = values
        .next()
        .ok_or_else(|| Failure::unavailable("systemd unit status is incomplete"))?;
    if values.next().is_some() {
        return Err(Failure::unavailable("systemd unit status is ambiguous"));
    }
    Ok(value)
}

struct CommandResult {
    success: bool,
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run(
    runner: &mut impl FnMut(&CommandSpec) -> io::Result<CommandResult>,
    command: &CommandSpec,
) -> Result<CommandResult, Failure> {
    runner(command).map_err(|error| {
        Failure::unavailable(format!("{} is unavailable: {error}", command.program))
    })
}

fn require_success(result: CommandResult, action: &str) -> Result<(), Failure> {
    if result.success {
        Ok(())
    } else {
        Err(Failure::unavailable(format!(
            "{action} command failed: {}",
            result.stderr.trim()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launchd_print(value: &ServiceSpec, state: &str) -> String {
        format!(
            "{}/{} = {{\n\tpath = {}\n\ttype = LaunchAgent\n\tstate = {}\n\tprogram = {}\n\targuments = {{\n\t\t{}\n\t\t--home\n\t\t{}\n\t\tdaemon\n\t\trun\n\t}}\n\tstdout path = {}\n\tstderr path = {}\n\tenvironment = {{\n\t\tHOME => {}\n\t\tXDG_CONFIG_HOME => {}\n\t\tXDG_DATA_HOME => {}\n\t}}\n}}\n",
            ServiceSpec::launchd_domain(),
            value.label(),
            value.unit_path().display(),
            state,
            value.executable().display(),
            value.executable().display(),
            value.daemon_home().display(),
            value.log_dir().join("stdout.log").display(),
            value.log_dir().join("stderr.log").display(),
            value.profile_home().display(),
            value.profile_home().join(".config").display(),
            value.profile_home().join(".local/share").display(),
        )
    }

    fn spec(kind: ServiceKind, home: &str, os: &str, arch: &str) -> ServiceSpec {
        ServiceSpec::new_for_host(
            kind,
            Path::new("/Users/test profile"),
            Path::new("/opt/locust/bin/locust"),
            Path::new(home),
            Path::new("/Users/test profile/.locust/logs"),
            os,
            arch,
        )
        .unwrap()
    }

    #[test]
    fn labels_and_paths_are_deterministic_per_home() {
        let first = spec(ServiceKind::Launchd, "/tmp/locust home", "macos", "aarch64");
        let second = spec(ServiceKind::Systemd, "/tmp/locust home", "linux", "x86_64");
        assert_eq!(first.label, second.label);
        assert!(first.label.starts_with("farm.locust."));
        assert!(first.unit_path.ends_with(format!("{}.plist", first.label)));
        assert!(
            second
                .unit_path
                .ends_with(format!("{}.service", second.label))
        );
        assert_ne!(
            first.label,
            spec(
                ServiceKind::Launchd,
                "/tmp/another home",
                "macos",
                "aarch64"
            )
            .label
        );
    }

    #[test]
    fn launchd_xml_escapes_paths_and_keeps_failed_exits_only() {
        let value = spec(
            ServiceKind::Launchd,
            "/Users/a & b/<goal>/'\"$%",
            "macos",
            "aarch64",
        );
        let rendered = String::from_utf8(value.render().unwrap()).unwrap();
        assert!(rendered.contains("/Users/a &amp; b/&lt;goal&gt;/&apos;&quot;$%"));
        assert!(rendered.contains("<key>SuccessfulExit</key><false/>"));
        assert!(rendered.contains("<key>RunAtLoad</key><true/>"));
        assert!(rendered.contains("<key>EnvironmentVariables</key>"));
        assert_eq!(value.start_commands(false)[0].args[0], "bootstrap");
        assert_eq!(value.start_commands(true)[0].args[0], "kickstart");
    }

    #[test]
    fn systemd_escapes_exec_words_and_rejects_ambiguous_log_paths() {
        let mut value = spec(
            ServiceKind::Systemd,
            "/home/space and \"quotes\"/$USER/%n;$(touch bad)",
            "linux",
            "x86_64",
        );
        let rendered = String::from_utf8(value.render().unwrap()).unwrap();
        assert!(!rendered.contains("$${USER}"));
        assert!(rendered.contains("$$USER"));
        assert!(rendered.contains("%%n"));
        assert!(rendered.contains("\\\"quotes\\\""));
        assert!(rendered.contains("Restart=on-failure"));
        assert!(
            rendered.contains("StandardOutput=append:/Users/test profile/.locust/logs/stdout.log")
        );
        assert!(!rendered.contains("/bin/sh"));
        value.log_dir = PathBuf::from("/home/odd\"logs");
        assert!(value.render().is_err());
    }

    #[test]
    fn rejects_newlines_relative_paths_and_wrong_host() {
        assert!(
            ServiceSpec::new_for_host(
                ServiceKind::Launchd,
                Path::new("/profile"),
                Path::new("/bin/locust"),
                Path::new("/tmp/evil\n[Service]"),
                Path::new("/logs"),
                "macos",
                "aarch64",
            )
            .is_err()
        );
        assert!(
            ServiceSpec::new_for_host(
                ServiceKind::Systemd,
                Path::new("/profile"),
                Path::new("locust"),
                Path::new("/home"),
                Path::new("/logs"),
                "linux",
                "x86_64",
            )
            .is_err()
        );
        assert!(
            ServiceSpec::new_for_host(
                ServiceKind::Systemd,
                Path::new("/profile"),
                Path::new("/bin/locust"),
                Path::new("/home"),
                Path::new("/logs"),
                "macos",
                "aarch64",
            )
            .is_err()
        );
    }

    #[test]
    fn control_requires_ownership_and_reports_missing_manager() {
        let value = spec(ServiceKind::Launchd, "/home", "macos", "aarch64");
        let conflict = value.run_control_with(ServiceAction::Start, false, |_| {
            panic!("must not call launchctl without ownership")
        });
        assert_eq!(conflict.unwrap_err().code, ErrorCode::Conflict);
        let unavailable = value.run_control_with(ServiceAction::Status, false, |_| {
            Err(io::Error::new(io::ErrorKind::NotFound, "missing launchctl"))
        });
        assert_eq!(unavailable.unwrap_err().code, ErrorCode::Unavailable);
        let linux = spec(ServiceKind::Systemd, "/home", "linux", "x86_64");
        let no_bus = linux.run_control_with(ServiceAction::Status, false, |_| {
            Ok(CommandResult {
                success: false,
                code: Some(1),
                stdout: String::new(),
                stderr: "Failed to connect to bus".into(),
            })
        });
        assert_eq!(no_bus.unwrap_err().code, ErrorCode::Unavailable);
    }

    #[test]
    fn loaded_launchd_service_uses_kickstart_and_systemd_collision_is_refused() {
        let value = spec(ServiceKind::Launchd, "/home", "macos", "aarch64");
        let mut seen = Vec::new();
        value
            .run_control_with(ServiceAction::Start, true, |command| {
                seen.push(command.args[0].clone());
                Ok(CommandResult {
                    success: true,
                    code: Some(0),
                    stdout: if command.args.len() == 2 && command.args[0] == "print" {
                        launchd_print(&value, "running")
                    } else {
                        String::new()
                    },
                    stderr: String::new(),
                })
            })
            .unwrap();
        assert_eq!(seen, ["print", "print", "kickstart", "print", "print"]);

        let foreign = value.run_control_with(ServiceAction::Stop, true, |command| {
            Ok(CommandResult {
                success: true,
                code: Some(0),
                stdout: if command.args.len() == 2 && command.args[0] == "print" {
                    launchd_print(&value, "not running").replace(
                        "\tprogram = /opt/locust/bin/locust",
                        "\tprogram = /tmp/foreign",
                    )
                } else {
                    String::new()
                },
                stderr: String::new(),
            })
        });
        assert_eq!(foreign.unwrap_err().code, ErrorCode::Conflict);

        let loaded = value.run_control_with(ServiceAction::Status, false, |command| {
            Ok(CommandResult {
                success: true,
                code: Some(0),
                stdout: if command.args.len() == 2 && command.args[0] == "print" {
                    launchd_print(&value, "spawn scheduled")
                } else {
                    String::new()
                },
                stderr: String::new(),
            })
        });
        assert_eq!(loaded.unwrap(), ServiceState::Loaded);

        let mut probes = 0;
        let state = value
            .run_control_with(ServiceAction::Start, true, |command| {
                probes += 1;
                if probes == 3 {
                    assert_eq!(command.args[0], "bootstrap");
                }
                Ok(CommandResult {
                    success: probes != 2,
                    code: Some(if probes == 2 { 113 } else { 0 }),
                    stdout: if probes == 5 {
                        launchd_print(&value, "running")
                    } else {
                        String::new()
                    },
                    stderr: String::new(),
                })
            })
            .unwrap();
        assert_eq!(state, ServiceState::Running);

        let value = spec(ServiceKind::Systemd, "/home", "linux", "x86_64");
        let collision = value.run_control_with(ServiceAction::Start, true, |_| {
            Ok(CommandResult {
                success: true,
                code: Some(0),
                stdout: "FragmentPath=/other/locust.service\nActiveState=active\n".into(),
                stderr: String::new(),
            })
        });
        assert_eq!(collision.unwrap_err().code, ErrorCode::Conflict);
        let transient = value.run_control_with(ServiceAction::Status, true, |_| {
            Ok(CommandResult {
                success: true,
                code: Some(0),
                stdout: "FragmentPath=\nActiveState=active\n".into(),
                stderr: String::new(),
            })
        });
        assert_eq!(transient.unwrap_err().code, ErrorCode::Unavailable);
    }

    #[test]
    fn systemd_stop_refuses_ambiguous_empty_fragment_without_mutating() {
        let value = spec(ServiceKind::Systemd, "/home", "linux", "x86_64");
        for load in ["loaded", "masked"] {
            let mut calls = 0;
            let result = value.run_control_with(ServiceAction::Stop, true, |_| {
                calls += 1;
                Ok(CommandResult {
                    success: true,
                    code: Some(0),
                    stdout: format!("LoadState={load}\nActiveState=inactive\nFragmentPath=\n"),
                    stderr: String::new(),
                })
            });
            assert_eq!(result.unwrap_err().code, ErrorCode::Conflict);
            assert_eq!(calls, 1, "no disable or reload may target another unit");
        }

        let mut calls = 0;
        let absent = value.run_control_with(ServiceAction::Stop, true, |_| {
            calls += 1;
            Ok(CommandResult {
                success: true,
                code: Some(0),
                stdout: "LoadState=not-found\nActiveState=inactive\nFragmentPath=\n".into(),
                stderr: String::new(),
            })
        });
        assert_eq!(absent.unwrap(), ServiceState::Stopped);
        assert_eq!(calls, 1, "an absent manager unit needs no mutating call");
    }
}
