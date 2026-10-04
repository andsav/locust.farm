use super::Output;
use crate::{
    failure::Failure,
    installation::{
        self,
        setup::{self, Client, SetupSpec},
    },
};
use clap::{Arg, ArgMatches, Command};
use std::path::Path;
fn path(name: &'static str) -> Arg {
    Arg::new(name).long(name).required(true)
}
pub(super) fn commands() -> Command {
    let mut command = Command::new("setup")
        .about("Install the operating skill, bound CLI launcher and scoped MCP registration")
        .subcommand_required(true)
        .arg_required_else_help(true);
    for operation in ["plan", "apply", "remove-plan", "remove", "status"] {
        let mut sub = Command::new(operation)
            .arg(path("prefix"))
            .arg(path("client").value_parser(["codex", "claude", "pi", "droid", "shell"]))
            .arg(path("profile-home"))
            .arg(path("workspace"))
            .arg(path("daemon-home"))
            .arg(path("credential-file"))
            .arg(path("session-file"));
        if matches!(operation, "apply" | "remove") {
            sub = sub.arg(path("expect-plan"));
        }
        command = command.subcommand(sub);
    }
    command
}
pub(super) fn run(operation: &str, args: &ArgMatches) -> Result<Output, Failure> {
    let file = |name: &str| Path::new(args.get_one::<String>(name).expect("required path"));
    let prefix = installation::absolute(file("prefix"))?;
    let client = match args.get_one::<String>("client").unwrap().as_str() {
        "codex" => Client::Codex,
        "claude" => Client::Claude,
        "pi" => Client::Pi,
        "droid" => Client::Droid,
        "shell" => Client::Shell,
        _ => unreachable!("validated client"),
    };
    let spec = SetupSpec {
        executable: prefix.join("current/locust"),
        skill_source: prefix.join("current/skills/locust/SKILL.md"),
        prefix,
        client,
        profile_home: installation::absolute(file("profile-home"))?,
        workspace: installation::absolute(file("workspace"))?,
        daemon_home: installation::absolute(file("daemon-home"))?,
        credential: installation::absolute(file("credential-file"))?,
        session: installation::absolute(file("session-file"))?,
    };
    let value = match operation {
        "setup.plan" => setup::plan(&spec, false)?.json()?,
        "setup.remove-plan" => setup::plan(&spec, true)?.json()?,
        "setup.apply" => setup::apply(&spec, args.get_one::<String>("expect-plan").unwrap())?,
        "setup.remove" => setup::remove(&spec, args.get_one::<String>("expect-plan").unwrap())?,
        "setup.status" => setup::status(&spec)?,
        _ => return Err(Failure::usage("unknown setup operation")),
    };
    let human = serde_json::to_string_pretty(&value)
        .map_err(|_| Failure::internal("cannot render setup result"))?;
    Ok(Output::success(value, human))
}
