use super::Output;
use crate::{failure::Failure, installation, package};
use clap::{Arg, ArgAction, ArgMatches, Command};
use std::path::Path;
fn path(name: &'static str) -> Arg {
    Arg::new(name).long(name).required(true)
}
fn inputs(command: Command) -> Command {
    command
        .arg(path("prefix"))
        .arg(path("bundle"))
        .arg(path("trust-key"))
        .arg(path("withdrawals"))
        .arg(
            Arg::new("allow-downgrade")
                .long("allow-downgrade")
                .action(ArgAction::SetTrue),
        )
}
pub(super) fn commands() -> Command {
    Command::new("install")
        .about("Review and apply a signed local software installation")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(inputs(
            Command::new("plan").about("Read-only installation plan and content digest"),
        ))
        .subcommand(
            inputs(
                Command::new("apply")
                    .about("Recheck the reviewed plan and atomically activate verified bytes"),
            )
            .arg(path("expect-plan")),
        )
        .subcommand(Command::new("status").arg(path("prefix")))
        .subcommand(
            Command::new("uninstall-plan")
                .about("Plan software removal; preserves data and withdrawal history")
                .arg(path("prefix")),
        )
        .subcommand(
            Command::new("uninstall")
                .about("Remove unchanged owned software; preserves daemon data and client settings")
                .arg(path("prefix"))
                .arg(path("expect-plan")),
        )
}
pub(super) fn run(operation: &str, args: &ArgMatches) -> Result<Output, Failure> {
    let file = |key: &str| Path::new(args.get_one::<String>(key).expect("required path"));
    let prefix = file("prefix");
    let value = match operation {
        "install.plan" | "install.apply" => {
            let verified = package::verify(file("bundle"), file("trust-key"), file("withdrawals"))?;
            if operation == "install.plan" {
                installation::plan(prefix, &verified, args.get_flag("allow-downgrade"))?.json()?
            } else {
                installation::apply(
                    prefix,
                    &verified,
                    args.get_flag("allow-downgrade"),
                    args.get_one::<String>("expect-plan").unwrap(),
                )?
            }
        }
        "install.status" => installation::status(prefix)?,
        "install.uninstall-plan" => installation::uninstall_plan(prefix)?.json()?,
        "install.uninstall" => {
            installation::uninstall(prefix, args.get_one::<String>("expect-plan").unwrap())?
        }
        _ => return Err(Failure::usage("unknown installation operation")),
    };
    let human = serde_json::to_string_pretty(&value)
        .map_err(|_| Failure::internal("cannot render installation result"))?;
    Ok(Output::success(value, human))
}
