//! Goal roles are immediate host acts. Undo commands use signed identities.
use super::{LocalClient, Output, connection, presentation, resolve_goal, selectors};
use crate::failure::Failure;
use clap::{Arg, ArgMatches, Command};
use locust_core::organization::{RoleDuty, role_duties};
use locust_proto::api::{Act, GoalStatus, Request, Response};
use locust_proto::event::Body;
use locust_proto::id::PublicKey;
use locust_proto::organization::{CompletionRule, Formation, Selector};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
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
/// The reviews part that names `role`, alone or inside an any-of choice: its
/// count, whether it excludes the author, and every role the choice names.
fn reviewer_requirement(rule: &CompletionRule, role: &str) -> Option<(u32, bool, Vec<String>)> {
    fn roles_named(selector: &Selector, found: &mut Vec<String>) {
        match selector {
            Selector::Role { name } => found.push(name.clone()),
            Selector::Any { selectors } => {
                for selector in selectors {
                    roles_named(selector, found);
                }
            }
            _ => {}
        }
    }
    match rule {
        CompletionRule::Reviews {
            by,
            count,
            exclude_author,
        } => {
            let mut named = Vec::new();
            roles_named(by, &mut named);
            if named.iter().any(|name| name == role) {
                Some((*count, *exclude_author, named))
            } else {
                None
            }
        }
        CompletionRule::All { rules } | CompletionRule::Any { rules } => rules
            .iter()
            .filter_map(|rule| reviewer_requirement(rule, role))
            .max_by_key(|(count, exclude, _)| (*count, *exclude)),
        _ => None,
    }
}
/// How many more members must hold one of the roles that count toward the
/// review rule naming `role`, given the goal's role lists.
pub(super) fn missing_reviewers(
    formation: &Formation,
    role: &str,
    roles: &BTreeMap<String, Vec<PublicKey>>,
) -> Option<usize> {
    let (count, exclude, named) = reviewer_requirement(&formation.decisions.completion, role)?;
    let holders: BTreeSet<_> = named
        .iter()
        .filter_map(|name| roles.get(name))
        .flatten()
        .collect();
    Some((count as usize + usize::from(exclude)).saturating_sub(holders.len()))
}
pub(super) fn initial_reviewers(formation: &Formation, host: PublicKey, name: &str) -> String {
    let Some(role) = counting_role(formation) else {
        return String::new();
    };
    let held = BTreeMap::from([(role.clone(), vec![host])]);
    let missing = missing_reviewers(formation, &role, &held)
        .map(|missing| format!(" {missing} more are needed;"))
        .unwrap_or_default();
    format!(
        "\nReviewers now: {} ({}, the host's agent).{missing} members you add or invite become {}s.",
        presentation::safe(name),
        &host.to_string()[..8],
        presentation::safe(&role)
    )
}

/// A role name as the last word of a printed command: bare when plain,
/// else single-quoted, and after `--` when it starts with a dash so clap
/// does not read it as a flag. None when the name holds characters the
/// terminal would hide; the caller then prints no command for it.
pub(super) fn quote_role(role: &str) -> Option<String> {
    if presentation::safe(role) != role {
        return None;
    }
    let word = if !role.is_empty()
        && role
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        role.to_owned()
    } else {
        format!("'{}'", role.replace('\'', "'\\''"))
    };
    Some(if role.starts_with('-') {
        format!("-- {word}")
    } else {
        word
    })
}
/// `label: command` for a role change, or, when the role's name cannot be
/// printed as a command word, a sentence that says what to type.
pub(super) fn role_command_line(
    label: &str,
    verb: &str,
    goal: &str,
    member: &str,
    role: &str,
) -> String {
    match quote_role(role) {
        Some(word) => {
            format!("{label}: locust --owner role {verb} --goal {goal} --member {member} {word}")
        }
        None => format!(
            "{label} with role {verb} --goal {goal} --member {member} and the role's name; it holds hidden characters, so no line is printed for it."
        ),
    }
}
fn unique_key(view: &GoalStatus, key: PublicKey) -> String {
    selectors::member_key_prefix(&view.members, key)
}
fn inverse(
    view: &GoalStatus,
    goal: &str,
    label: &str,
    verb: &str,
    key: PublicKey,
    role: &str,
) -> String {
    role_command_line(label, verb, goal, &unique_key(view, key), role)
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
    // Nothing changes while the goal's own key is held: no undo line.
    super::only_you::refuse_if_host_held(&view, Act::GiveRole, None)?;
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
    let goal_cut = super::only_you::cut_goal(client, socket, goal);
    if !give && !deciding && expected.len() == 1 {
        lines.push(inverse(
            &view,
            &goal_cut,
            "Give it back",
            "give",
            member,
            role,
        ));
        if let Some(host) = host {
            lines.push(inverse(
                &view,
                &goal_cut,
                &format!(
                    "The host's agent holds {} until you take it",
                    presentation::safe(role)
                ),
                "take",
                host,
                role,
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
        lines.push(inverse(&view, &goal_cut, "Undo", verb, undo_member, role));
    }
    Ok(Output::success(json!(response), lines.join("\n")))
}
