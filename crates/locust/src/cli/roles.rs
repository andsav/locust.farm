//! Goal roles are immediate host acts. Undo commands use signed identities.
use super::{LocalClient, Output, connection, presentation, resolve_goal, selectors};
use crate::failure::Failure;
use clap::{Arg, ArgMatches, Command};
use locust_core::organization::{RoleDuty, role_duties};
use locust_proto::api::{GoalStatus, Request, Response};
use locust_proto::event::Body;
use locust_proto::id::PublicKey;
use locust_proto::organization::{CompletionRule, Formation, Selector};
use serde_json::json;
use std::path::Path;

pub(super) fn command(name: &'static str) -> Command {
    Command::new(name)
        .arg(Arg::new("goal").long("goal").required(true))
        .arg(
            Arg::new("member")
                .long("member")
                .required(true)
                .help("Member name, key or unique prefix"),
        )
        .arg(Arg::new("role").required(true))
}

pub(super) fn current_formation(
    client: &mut LocalClient,
    socket: &Path,
    view: &GoalStatus,
) -> Result<Option<Formation>, Failure> {
    let Some(event) = view.current_rules else {
        return Ok(None);
    };
    let response = client
        .call(Request::Event {
            goal: view.goal,
            event,
        })
        .map_err(|error| connection::client_error(error, socket))?;
    let Response::Event(event) = response else {
        unreachable!("typed response")
    };
    let Body::RulesBound { binding, .. } = event.body else {
        return Err(Failure::invalid(
            "current rules do not name a formation binding",
        ));
    };
    let Response::Blob { bytes } = client
        .call(Request::BlobGet {
            goal: view.goal,
            hash: binding.definition.object.hash,
        })
        .map_err(|error| connection::client_error(error, socket))?
    else {
        unreachable!("typed response")
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| Failure::invalid(format!("formation definition: {error}")))
}

pub(super) use locust_core::organization::counting_role;

pub(super) fn selected_role(
    client: &mut LocalClient,
    socket: &Path,
    args: &ArgMatches,
    view: &GoalStatus,
) -> Result<Option<String>, Failure> {
    if args.get_flag("no-role") {
        return Ok(None);
    }
    if let Some(role) = args.get_one::<String>("role") {
        return Ok(Some(role.clone()));
    }
    Ok(current_formation(client, socket, view)?.and_then(|formation| counting_role(&formation)))
}
fn reviewer_requirement(rule: &CompletionRule, role: &str) -> Option<(u32, bool)> {
    match rule {
        CompletionRule::Reviews {
            by: Selector::Role { name },
            count,
            exclude_author,
        } if name == role => Some((*count, *exclude_author)),
        CompletionRule::All { rules } | CompletionRule::Any { rules } => rules
            .iter()
            .filter_map(|rule| reviewer_requirement(rule, role))
            .max(),
        _ => None,
    }
}
pub(super) fn missing_reviewers(
    formation: &Formation,
    role: &str,
    holders: usize,
) -> Option<usize> {
    let (count, exclude) = reviewer_requirement(&formation.decisions.completion, role)?;
    Some((count as usize + usize::from(exclude)).saturating_sub(holders))
}
pub(super) fn initial_reviewers(formation: &Formation, host: PublicKey, name: &str) -> String {
    let Some(role) = counting_role(formation) else {
        return String::new();
    };
    let missing = missing_reviewers(formation, &role, 1).unwrap_or(0);
    format!(
        "\nReviewers now: {} ({}, the host's agent). {missing} more are needed; members you add or invite become {}s.",
        presentation::safe(name),
        &host.to_string()[..8],
        presentation::safe(&role)
    )
}

pub(super) fn quote_role(role: &str) -> String {
    if !role.is_empty()
        && role
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        role.into()
    } else {
        format!("'{}'", role.replace('\'', "'\\''"))
    }
}
fn unique_key(view: &GoalStatus, key: PublicKey) -> String {
    selectors::member_key_prefix(&view.members, key)
}
fn inverse(view: &GoalStatus, verb: &str, key: PublicKey, role: &str) -> String {
    format!(
        "locust --owner role {verb} --goal {} --member {} {}",
        &view.goal.to_string()[..8],
        unique_key(view, key),
        quote_role(role)
    )
}
fn duty_words(duty: RoleDuty) -> &'static str {
    match duty {
        RoleDuty::Propose => "opens tasks",
        RoleDuty::Start => "takes tasks",
        RoleDuty::Offer => "offers work",
        RoleDuty::Receive => "receives work",
        RoleDuty::Publish => "posts results",
        RoleDuty::Declare => "declares completion",
        RoleDuty::Review => "approves results",
        RoleDuty::Attest => "attests checks",
        RoleDuty::Pick => "picks results",
        RoleDuty::Close => "closes tasks",
    }
}
fn duties(formation: Option<&Formation>, role: &str) -> String {
    match formation {
        Some(formation) if !formation.roles.contains_key(role) => format!(
            "{} is not in the current rules. It still applies to work under earlier rules.",
            presentation::safe(role)
        ),
        Some(formation) if role_duties(formation, role).is_empty() => format!(
            "No rule in the current rules names {}, so it changes nothing yet.",
            presentation::safe(role)
        ),
        Some(formation) => format!(
            "A {} here: {}.",
            presentation::safe(role),
            role_duties(formation, role)
                .into_iter()
                .map(duty_words)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        None => "The current rules have not arrived; this role's duties are unavailable.".into(),
    }
}
pub(super) fn run(
    matches: &ArgMatches,
    operation: &str,
    args: &ArgMatches,
    client: &mut LocalClient,
    socket: &Path,
) -> Result<Output, Failure> {
    let goal = resolve_goal(
        client,
        socket,
        args.get_one::<String>("goal").expect("required goal"),
        None,
    )?;
    let member = selectors::resolve_member(
        client,
        socket,
        goal,
        args.get_one::<String>("member").expect("required member"),
    )?;
    let role = args.get_one::<String>("role").expect("required role");
    let Response::GoalStatus(view) = client
        .call(Request::GoalStatus { goal })
        .map_err(|error| connection::client_error(error, socket))?
    else {
        unreachable!("typed response")
    };
    let expected = view.roles.get(role).cloned().unwrap_or_default();
    let formation = current_formation(client, socket, &view)?;
    let give = operation == "role.give";
    let deciding = view.deciding.contains(role);
    let host = view.host;
    let request = if give {
        Request::RoleGive {
            goal,
            role: role.clone(),
            member,
            expected: expected.clone(),
        }
    } else {
        Request::RoleTake {
            goal,
            role: role.clone(),
            member,
            expected: expected.clone(),
        }
    };
    let key = matches
        .get_one::<String>("idempotency-key")
        .map(|key| {
            key.parse()
                .map_err(|_| Failure::usage("--idempotency-key requires 16 bytes in hex"))
        })
        .transpose()?;
    let response = client
        .call_with(request, key, None)
        .map_err(|error| connection::client_error(error, socket))?;
    if matches!(response, Response::Done) {
        return Ok(Output::success(
            json!(response),
            format!(
                "{} stays with the host's agent while nobody else holds it.",
                presentation::safe(role)
            ),
        ));
    }
    let mut holders = if give && deciding {
        vec![member]
    } else {
        expected.clone()
    };
    if give && !deciding {
        holders.push(member);
        holders.sort();
        holders.dedup();
    }
    if !give {
        holders.retain(|holder| *holder != member);
        if holders.is_empty() {
            holders.extend(host);
        }
    }
    let member_name = view
        .members
        .iter()
        .find(|entry| entry.member == member)
        .map(|entry| presentation::chosen_name(&entry.name))
        .unwrap_or_else(|| unique_key(&view, member));
    let mut lines = vec![
        if give {
            format!(
                "{member_name} is a {} in \"{}\".",
                presentation::safe(role),
                presentation::safe(view.title.as_deref().unwrap_or("this goal"))
            )
        } else {
            format!(
                "{member_name} no longer holds {} in \"{}\".",
                presentation::safe(role),
                presentation::safe(view.title.as_deref().unwrap_or("this goal"))
            )
        },
        duties(formation.as_ref(), role),
    ];
    lines.push(format!(
        "{}s now: {}.",
        {
            let safe = presentation::safe(role);
            let mut chars = safe.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default()
        },
        holders
            .iter()
            .map(|holder| {
                if host == Some(*holder) {
                    presentation::member_label_noting(*holder, &view.members, "the host's agent")
                } else {
                    presentation::member_label(*holder, &view.members)
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    ));
    if !give && !deciding && expected.len() == 1 {
        lines.push(format!(
            "Give it back: {}",
            inverse(&view, "give", member, role)
        ));
        if let Some(host) = host {
            lines.push(format!(
                "The host's agent holds {} until you take it: {}",
                presentation::safe(role),
                inverse(&view, "take", host, role)
            ));
        }
    } else {
        let (verb, undo_member) = if give && deciding {
            ("give", expected[0])
        } else if give {
            ("take", member)
        } else {
            ("give", member)
        };
        lines.push(format!("Undo: {}", inverse(&view, verb, undo_member, role)));
    }
    Ok(Output::success(json!(response), lines.join("\n")))
}
