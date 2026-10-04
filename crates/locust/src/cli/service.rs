use super::Output;
use crate::{
    failure::Failure,
    installation::{
        self,
        service::{ServiceAction, ServiceKind, ServiceSpec, ServiceState},
        service_install,
    },
};
use clap::{Arg, ArgMatches, Command};
use locust_proto::api::ErrorCode;
use serde_json::json;
use std::path::Path;
fn path(name: &'static str) -> Arg {
    Arg::new(name).long(name).required(true)
}
pub(super) fn commands() -> Command {
    let mut command = Command::new("service")
        .about("Review and control an owned per-user daemon service")
        .subcommand_required(true)
        .arg_required_else_help(true);
    for operation in [
        "plan",
        "apply",
        "remove-plan",
        "remove",
        "status",
        "start",
        "stop",
    ] {
        let mut sub = Command::new(operation)
            .arg(path("prefix"))
            .arg(path("kind").value_parser(["launchd", "systemd", "none"]))
            .arg(path("profile-home"))
            .arg(path("daemon-home"))
            .arg(path("log-dir"));
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
    let kind = match args.get_one::<String>("kind").unwrap().as_str() {
        "launchd" => ServiceKind::Launchd,
        "systemd" => ServiceKind::Systemd,
        _ => ServiceKind::None,
    };
    let spec = ServiceSpec::new(
        kind,
        &installation::absolute(file("profile-home"))?,
        &prefix.join("current/locust"),
        &installation::absolute(file("daemon-home"))?,
        &installation::absolute(file("log-dir"))?,
    )?;
    if matches!(operation, "service.apply" | "service.start") {
        let installed = installation::status(&prefix)?;
        if installed["installed"] != true || installed["withdrawn"] != false {
            return Err(Failure::new(
                ErrorCode::Denied,
                "service start requires an installed, verified, non-withdrawn release",
            ));
        }
    }
    let value = match operation {
        "service.plan" => service_install::plan(&prefix, &spec, false)?.json()?,
        "service.remove-plan" => service_install::plan(&prefix, &spec, true)?.json()?,
        "service.apply" => service_install::apply(
            &prefix,
            &spec,
            args.get_one::<String>("expect-plan").unwrap(),
        )?,
        "service.remove" => service_install::remove(
            &prefix,
            &spec,
            args.get_one::<String>("expect-plan").unwrap(),
        )?,
        "service.status" => service_install::status(&prefix, &spec)?,
        "service.start" | "service.stop" => {
            let state = service_install::control(
                &prefix,
                &spec,
                if operation == "service.start" {
                    ServiceAction::Start
                } else {
                    ServiceAction::Stop
                },
            )?;
            let state = match state {
                ServiceState::Disabled => "disabled",
                ServiceState::Running => "running",
                ServiceState::Stopped => "stopped",
                ServiceState::Loaded => "loaded",
            };
            json!({"state":state,"daemon_api_readiness_observed":false,"label":spec.label()})
        }
        _ => return Err(Failure::usage("unknown service operation")),
    };
    let human = serde_json::to_string_pretty(&value)
        .map_err(|_| Failure::internal("cannot render service result"))?;
    Ok(Output::success(value, human))
}
