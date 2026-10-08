//! Explicit owner publication controls. Public labels are entered independently
//! from private goal, task and enrolled-principal names.
use super::{Output, acting_agent, confirm, connection, presentation, resolve_goal};
use crate::failure::Failure;
use clap::{Arg, ArgAction, ArgMatches, Command};
use locust_proto::api::{Act, Caller};
use locust_proto::id::{IdempotencyKey, PublicKey};
use locust_proto::local;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn goal() -> Arg {
    Arg::new("goal")
        .long("goal")
        .required(true)
        .help("Visible goal title, identifier or unique hex prefix")
}
pub(super) fn commands() -> Command {
    Command::new("farm")
        .about("Owner-approved public goal views; local preview, consent and publication")
        .subcommand_required(true)
        .subcommand(Command::new("show").about("Preview the exact public snapshot and consent eligibility").arg(goal()))
        .subcommand(Command::new("status").about("Show local publication receipts and pending controls"))
        .subcommand(confirm::flags(Command::new("off").about("Stop publication and durably request deletion; receipt may be pending").arg(goal())))
        .subcommand(confirm::flags(Command::new("on")
            .about("Request publication with explicit public labels; members must consent separately")
            .arg(goal())
            .arg(Arg::new("service").long("service").default_value("https://locust.farm").help("HTTPS service origin, or HTTP loopback origin for local testing"))
            .arg(Arg::new("listed").long("listed").action(ArgAction::SetTrue).help("Also list this farm in the public gallery"))
            .arg(Arg::new("title").long("title").help("Optional public title, entered explicitly"))
            .arg(Arg::new("formation").long("formation").default_value("Locust farm").help("Public formation label, not a private formation selector"))
            .arg(Arg::new("stage-label").long("stage-label").action(ArgAction::Append).help("Explicit public label as private-stage-id=public label; repeat for each label"))
            .arg(Arg::new("role-label").long("role-label").action(ArgAction::Append).help("Explicit public label as private-role-id=public label; repeat for each label"))
            .arg(Arg::new("recent-changes").long("recent-changes").default_value("50").value_parser(clap::value_parser!(u32)).help("Number of recent changes included in the public snapshot"))))
        .subcommand(confirm::flags(Command::new("consent")
            .about("Approve or revoke one local principal's publication under the current exact policy")
            .arg(goal())
            .arg(Arg::new("accept").long("accept").required_unless_present("decline").conflicts_with("decline").requires("name").action(ArgAction::SetTrue))
            .arg(Arg::new("decline").long("decline").action(ArgAction::SetTrue))
            .arg(Arg::new("name").long("name").help("Approved public display name; never copied from enrollment"))
            .arg(Arg::new("group-label").long("group-label").help("Optional owner-reported public daemon group label"))))
}

fn labels(args: &ArgMatches, key: &str) -> Result<BTreeMap<String, String>, Failure> {
    let mut labels = BTreeMap::new();
    for item in args.get_many::<String>(key).into_iter().flatten() {
        let (id, label) = item
            .split_once('=')
            .ok_or_else(|| Failure::usage(format!("--{key} requires id=public label")))?;
        if id.trim().is_empty()
            || label.trim().is_empty()
            || labels.insert(id.into(), label.into()).is_some()
        {
            return Err(Failure::usage(format!(
                "--{key} requires a unique nonempty id and nonempty label"
            )));
        }
    }
    Ok(labels)
}

pub(super) fn run(
    matches: &ArgMatches,
    operation: &str,
    args: &ArgMatches,
) -> Result<Output, Failure> {
    if matches.get_one::<String>("agent").is_some() && matches!(operation, "farm.on" | "farm.off") {
        return Err(Failure::usage(
            "host commands are the host's own and name no agent; drop --agent",
        ));
    }
    if matches.get_one::<String>("agent").is_some()
        && matches!(operation, "farm.show" | "farm.status")
    {
        return Err(Failure::usage(
            "farm show and status do not name an agent; drop --agent",
        ));
    }
    let home = connection::home(matches)?;
    let socket = local::socket_path(&home)?;
    let mut client = connection::open(matches, &home)?;
    let goal = if operation == "farm.status" {
        None
    } else {
        Some(resolve_goal(
            &mut client,
            &socket,
            args.get_one::<String>("goal").expect("required goal"),
            None,
        )?)
    };
    let fields = match operation {
        "farm.on" => json!({
            "goal": goal.unwrap(), "base_url": args.get_one::<String>("service").unwrap(),
            "listed": args.get_flag("listed"), "title": args.get_one::<String>("title"),
            "formation": args.get_one::<String>("formation").unwrap(),
            "stage_labels": labels(args, "stage-label")?, "role_labels": labels(args, "role-label")?,
            "recent_changes": args.get_one::<u32>("recent-changes").unwrap(),
        }),
        "farm.consent" => {
            let agent = match client.caller() {
                Caller::Owner => acting_agent(&mut client, &socket, matches, goal)?,
                Caller::Agent(agent) | Caller::Author(agent) => agent,
            };
            json!({"goal": goal.unwrap(), "agent": agent, "accept": args.get_flag("accept"),
                "name": args.get_one::<String>("name"), "group_label": args.get_one::<String>("group-label")})
        }
        "farm.show" | "farm.off" => json!({"goal": goal.unwrap()}),
        "farm.status" => json!({}),
        _ => unreachable!("farm command tree"),
    };
    let request = super::request(operation, fields.as_object().unwrap().clone())?;
    request.check().map_err(Failure::from)?;
    if matches!(operation, "farm.on" | "farm.off" | "farm.consent")
        && client.caller() == Caller::Owner
    {
        // Each signs a publication record: none shows a plan it could not
        // carry out while this computer is catching up in the goal.
        let observed = match client
            .call(locust_proto::api::Request::GoalStatus {
                goal: goal.expect("write goal"),
            })
            .map_err(|error| connection::client_error(error, &socket))?
        {
            locust_proto::api::Response::GoalStatus(observed) => observed,
            _ => unreachable!("typed response"),
        };
        match &fields["agent"] {
            Value::Null => super::only_you::refuse_if_host_held(&observed, Act::Publish, None)?,
            agent => {
                let agent: PublicKey = serde_json::from_value(agent.clone())
                    .map_err(|error| Failure::internal(error.to_string()))?;
                super::only_you::refuse_if_agent_held(&observed, agent, Act::Publish)?;
            }
        }
        let plan = publication_plan(
            &mut client,
            &socket,
            goal.expect("write goal"),
            operation,
            &fields,
        )?;
        if confirm::decide(matches, args, &plan)? == confirm::Decision::Show {
            return Ok(plan.shown());
        }
        confirm::bound(
            &plan.id(),
            &publication_plan(
                &mut client,
                &socket,
                goal.expect("write goal"),
                operation,
                &fields,
            )?,
        )?;
    }
    let key = matches
        .get_one::<String>("idempotency-key")
        .map(|key| {
            key.parse::<IdempotencyKey>()
                .map_err(|_| Failure::usage("--idempotency-key requires 16 bytes in hex"))
        })
        .transpose()?;
    let response = client
        .call_with(request, key, None)
        .map_err(|e| connection::client_error(e, &socket))?;
    let result = serde_json::to_value(response).map_err(|e| Failure::internal(e.to_string()))?;
    let heading = match operation {
        "farm.on" => {
            "Publication requested. Eligibility and the last service receipt are shown below."
        }
        "farm.off" => {
            "Publication stopped locally. Check the service receipt below for deletion acknowledgment."
        }
        "farm.consent" => "Local publication consent recorded for the current policy.",
        "farm.show" => {
            "Local publication preview. Only the snapshot is public; eligibility and policy are local diagnostics."
        }
        _ => "Local publication status and service receipts.",
    };
    let mut human = render(heading, &result)?;
    if operation == "farm.on"
        && let Some(id) = result
            .pointer("/farm_preview/status/farm_id")
            .and_then(Value::as_str)
    {
        let origin = args.get_one::<String>("service").expect("default service");
        human = format!(
            "Farm: {}/farm/{}\n{human}",
            presentation::safe(origin.trim_end_matches('/')),
            presentation::safe(id)
        );
    }
    Ok(Output::success(result, human))
}

fn publication_plan(
    client: &mut super::LocalClient,
    socket: &std::path::Path,
    goal: locust_proto::id::GoalId,
    operation: &str,
    fields: &Value,
) -> Result<confirm::Plan, Failure> {
    let preview = client
        .call(locust_proto::api::Request::FarmShow { goal })
        .map_err(|error| connection::client_error(error, socket))?;
    let locust_proto::api::Response::FarmPreview(preview) = preview else {
        return Err(Failure::internal("expected farm preview"));
    };
    let review = json!({
        "request": fields,
        "policy": preview.policy,
        "desired": preview.status.as_ref().and_then(|status| status.desired),
        "eligible": preview.status.as_ref().map(|status| status.eligible),
    });
    let human = if operation == "farm.consent" {
        format!(
            "Review publication consent under this policy:\n{}",
            serde_json::to_string_pretty(&review)
                .map_err(|error| Failure::internal(error.to_string()))?
        )
    } else {
        format!(
            "Review {} for this goal:\n{}",
            operation,
            serde_json::to_string_pretty(&review)
                .map_err(|error| Failure::internal(error.to_string()))?
        )
    };
    Ok(confirm::Plan {
        command: match operation {
            "farm.on" => "farm on",
            "farm.off" => "farm off",
            "farm.consent" => "farm consent",
            _ => unreachable!("publication plan command"),
        },
        review,
        human,
        warning: None,
        again: String::new(),
    })
}
fn render(heading: &str, value: &Value) -> Result<String, Failure> {
    let text = serde_json::to_string_pretty(value).map_err(|e| Failure::internal(e.to_string()))?;
    // Keep the preview valid JSON with exactly the same values while escaping
    // invisible direction controls which could rearrange a terminal display.
    let mut safe = String::new();
    for character in text.chars() {
        if matches!(character, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')
        {
            use std::fmt::Write;
            write!(safe, "\\u{:04x}", character as u32).expect("writing to string");
        } else {
            safe.push(character);
        }
    }
    Ok(format!("{heading}\n{safe}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn printable_preview_remains_equivalent_json() {
        let value = json!({"name": "worker\u{202e}", "title": "line\nbreak"});
        let rendered = render("Preview", &value).unwrap();
        assert!(!rendered.contains('\u{202e}'));
        assert_eq!(
            serde_json::from_str::<Value>(rendered.split_once('\n').unwrap().1).unwrap(),
            value
        );
    }
    #[test]
    fn consent_requires_a_choice_and_explicit_public_name() {
        for args in [
            vec!["farm", "consent", "--goal", "g"],
            vec!["farm", "consent", "--goal", "g", "--accept"],
        ] {
            assert!(commands().try_get_matches_from(args).is_err());
        }
        assert!(
            commands()
                .try_get_matches_from(["farm", "consent", "--goal", "g", "--decline"])
                .is_ok()
        );
        assert!(
            super::super::args::command()
                .try_get_matches_from([
                    "locust",
                    "--owner",
                    "--agent",
                    "a",
                    "farm",
                    "consent",
                    "--goal",
                    "g",
                    "--accept",
                    "--name",
                    "Public worker"
                ])
                .is_ok()
        );
    }
    #[test]
    fn publication_labels_do_not_silently_overwrite_duplicates() {
        let args = commands()
            .try_get_matches_from([
                "farm",
                "on",
                "--goal",
                "g",
                "--stage-label",
                "build=Build",
                "--stage-label",
                "build=Other",
            ])
            .unwrap();
        assert!(labels(args.subcommand().unwrap().1, "stage-label").is_err());
    }
    #[test]
    fn farm_reads_do_not_take_an_acting_agent() {
        for args in [
            vec![
                "locust", "--owner", "--agent", "worker", "farm", "show", "--goal", "g",
            ],
            vec!["locust", "--owner", "--agent", "worker", "farm", "status"],
        ] {
            let parsed = super::super::args::command()
                .try_get_matches_from(args)
                .unwrap();
            let (operation, selected) = super::super::args::selected(&parsed);
            assert!(
                run(&parsed, &operation, selected)
                    .unwrap_err()
                    .message
                    .contains("drop --agent")
            );
        }
    }
}
