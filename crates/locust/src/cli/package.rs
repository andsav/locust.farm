use super::Output;
use crate::{failure::Failure, package};
use clap::{Arg, ArgMatches, Command};
use serde_json::json;
use std::path::Path;

fn path(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name).long(name).required(true).help(help)
}

pub(super) fn commands() -> Command {
    Command::new("package").about("Verify or sign an identified local release bundle")
        .subcommand_required(true).arg_required_else_help(true)
        .subcommand(Command::new("keygen").about("Generate a new explicit signing key pair; never selects production trust")
            .arg(path("secret-key", "New raw 32-byte private key file"))
            .arg(path("public-key", "New raw 32-byte public key file")))
        .subcommand(Command::new("sign").about("Sign exact manifest bytes after checking all payloads")
            .arg(path("bundle", "Existing extracted candidate directory"))
            .arg(path("secret-key", "Existing private signing key; mode 0600")))
        .subcommand(Command::new("sign-withdrawals").about("Sign an explicit release withdrawal registry")
            .arg(path("registry", "Existing locust-withdrawals-v1 JSON file"))
            .arg(path("secret-key", "Existing private signing key; mode 0600")))
        .subcommand(Command::new("verify").about("Verify signatures, withdrawal status and every payload without executing the candidate")
            .arg(path("bundle", "Existing extracted candidate directory"))
            .arg(path("trust-key", "Independently trusted raw 32-byte public key"))
            .arg(path("withdrawals", "Signed registry; detached signature at <path>.sig")))
}

pub(super) fn run(operation: &str, args: &ArgMatches) -> Result<Output, Failure> {
    let file = |key: &str| Path::new(args.get_one::<String>(key).expect("required path"));
    let value = match operation {
        "package.keygen" => {
            package::keygen(file("secret-key"), file("public-key"))?;
            json!({"created":true,"production_trust_selected":false})
        }
        "package.sign" => {
            package::sign_release(file("bundle"), file("secret-key"))?;
            json!({"signed":true})
        }
        "package.sign-withdrawals" => {
            package::sign_withdrawals(file("registry"), file("secret-key"))?;
            json!({"signed":true})
        }
        "package.verify" => {
            let verified = package::verify(file("bundle"), file("trust-key"), file("withdrawals"))?;
            json!({"verified":true,"manifest_sha256":verified.manifest_sha256,"manifest":verified.manifest,
                "trust_key_sha256":package::sha256(&verified.trust_key),"withdrawals_sequence":verified.withdrawals.sequence,
                "withdrawals_sha256":verified.withdrawals_sha256,"executed":false})
        }
        _ => return Err(Failure::usage("unknown package operation")),
    };
    Ok(Output::success(
        value.clone(),
        serde_json::to_string_pretty(&value)
            .map_err(|_| Failure::internal("cannot render package result"))?,
    ))
}
