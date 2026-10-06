//! Reviewed owner handoff to an already enrolled participant on this daemon.
use super::{
    LocalClient, Output, connection, presentation, print, resolve_goal, resolve_principal, status,
};
use crate::failure::Failure;
use clap::{Arg, ArgAction, ArgMatches, Command};
use locust_proto::api::{Caller, ErrorCode, GoalStatus, Membership, Request, Response};
use locust_proto::id::{GoalId, IdempotencyKey, PublicKey};
use locust_proto::local;
use serde_json::json;
use std::io::{self, IsTerminal};
use std::path::Path;

pub(super) fn command() -> Command {
    Command::new("add-local")
        .about("Review whole-goal sharing with an enrolled local agent; permissions stay unchanged")
        .arg(
            Arg::new("goal")
                .long("goal")
                .required(true)
                .help("Visible goal title, identifier or unique hex prefix"),
        )
        .arg(
            Arg::new("agent")
                .long("agent")
                .required(true)
                .help("Enrolled local agent name or full principal key"),
        )
        .arg(
            Arg::new("plan")
                .long("plan")
                .action(ArgAction::SetTrue)
                .conflicts_with("yes")
                .help("Inspect sharing and membership without issuing an invitation"),
        )
        .arg(
            Arg::new("yes")
                .long("yes")
                .action(ArgAction::SetTrue)
                .help("Approve sharing this selected goal with this selected local agent"),
        )
}

fn call(
    client: &mut LocalClient,
    socket: &Path,
    request: Request,
    key: Option<IdempotencyKey>,
    principal: Option<PublicKey>,
) -> Result<Response, Failure> {
    client
        .call_with(request, key, principal)
        .map_err(|error| connection::client_error(error, socket))
}
fn inspect(client: &mut LocalClient, socket: &Path, goal: GoalId) -> Result<GoalStatus, Failure> {
    let Response::GoalStatus(observed) =
        call(client, socket, Request::GoalStatus { goal }, None, None)?
    else {
        unreachable!("typed client checks response kind")
    };
    Ok(observed)
}
// The daemon persists response replay atomically with each write. Binding the
// retry key to the current governance head also permits a deliberately reviewed
// re-admission after removal. No capability is printed or saved by this CLI.
fn retry_key(observed: &GoalStatus, member: PublicKey, expires_ms: u64) -> IdempotencyKey {
    let material = serde_json::to_vec(&(
        "locust-add-local-v1",
        observed.goal,
        observed.host,
        observed.governance_head,
        member,
        expires_ms,
    ))
    .expect("public selection encodes");
    IdempotencyKey(
        locust_proto::crypto::content_hash(&material).0[..16]
            .try_into()
            .unwrap(),
    )
}

pub(super) fn run(matches: &ArgMatches, args: &ArgMatches) -> Result<Output, Failure> {
    if !matches.get_flag("owner")
        || ["as", "credential", "session", "idempotency-key"]
            .iter()
            .any(|flag| matches.get_one::<String>(flag).is_some())
    {
        return Err(Failure::usage(
            "goal add-local requires --owner without --as, --credential, --session or --idempotency-key; it resumes membership itself",
        ));
    }
    let home = connection::home(matches)?;
    let socket = local::socket_path(&home)?;
    let mut client = connection::open(matches, &home)?;
    if client.caller() != Caller::Owner {
        return Err(Failure::new(
            ErrorCode::Denied,
            "local membership is an owner decision",
        ));
    }
    let goal = resolve_goal(
        &mut client,
        &socket,
        args.get_one::<String>("goal").expect("required goal"),
        None,
    )?;
    let member = resolve_principal(
        &mut client,
        &socket,
        args.get_one::<String>("agent").expect("required agent"),
    )?;
    let known = status(&mut client, &socket, None)?;
    let agent = known
        .agents
        .iter()
        .find(|agent| agent.agent == member && !agent.revoked)
        .ok_or_else(|| {
            Failure::new(ErrorCode::NotFound, "select an active enrolled local agent")
        })?;
    let observed = inspect(&mut client, &socket, goal)?;
    if !known
        .agents
        .iter()
        .any(|agent| agent.agent == observed.host && !agent.revoked)
        || !observed
            .members
            .iter()
            .any(|entry| entry.member == observed.host && entry.local)
    {
        return Err(Failure::new(
            ErrorCode::Denied,
            "this goal's host is not an active local principal; request an invitation from its owner",
        ));
    }
    let joined = known.goals.iter().any(|entry| {
        entry.goal == goal && entry.member == member && entry.membership == Membership::Member
    });
    if observed.members.iter().any(|entry| entry.member == member) && !joined {
        return Err(Failure::new(
            ErrorCode::Conflict,
            "this principal's local membership is not active; inspect its departure or admission before rejoining",
        ));
    }
    let review = json!({"goal":goal,"title":observed.title,"host":observed.host,
        "agent":member,"name":agent.name,"sharing":"whole_goal","governance_head":observed.governance_head,
        "current_rules":observed.current_rules,"already_member":joined,"permissions_changed":false});
    let human = format!(
        "Goal: {} ({goal})\nParticipant: {} ({member})\nSharing: available goal history and shared content. Local files and private chats stay outside this action.\nLocal permissions remain unchanged.",
        presentation::safe(observed.title.as_deref().unwrap_or("Untitled")),
        presentation::safe(&agent.name)
    );
    let interactive =
        io::stdin().is_terminal() && io::stderr().is_terminal() && !matches.get_flag("json");
    if args.get_flag("plan") || (!args.get_flag("yes") && !interactive) {
        return Ok(Output::success(
            json!({"action":"review_required","plan":review,"changed":false}),
            format!("{human}\nRepeat with --yes to approve this membership, or run interactively."),
        ));
    }
    if !joined && !args.get_flag("yes") {
        print::stderr(format_args!("{human}\nAdd this participant? [y/N] "))
            .map_err(|error| Failure::invalid(error.to_string()))?;
        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .map_err(|error| Failure::invalid(error.to_string()))?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            return Err(Failure::new(
                ErrorCode::Denied,
                "membership was declined; no invitation or permission was changed",
            ));
        }
    }
    if joined {
        return Ok(Output::success(
            json!({"goal":goal,"agent":member,"membership":"member","changed":false,"permissions_changed":false}),
            format!(
                "{} is already a member. Local permissions remain unchanged.",
                presentation::safe(&agent.name)
            ),
        ));
    }
    let current = inspect(&mut client, &socket, goal)?;
    if current.host != observed.host
        || current.governance_head != observed.governance_head
        || current.current_rules != observed.current_rules
    {
        return Err(Failure::new(
            ErrorCode::Conflict,
            "goal membership or rules changed during review; repeat goal add-local to review the current state",
        ));
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Failure::internal("system clock precedes the Unix epoch"))?
        .as_millis() as u64;
    let expires_ms = (now_ms / 86_400_000 + 2) * 86_400_000;
    let Response::Invited { ticket } = call(
        &mut client,
        &socket,
        Request::GoalInvite { goal, expires_ms },
        Some(retry_key(&observed, member, expires_ms)),
        None,
    )?
    else {
        unreachable!("typed response")
    };
    let Response::InvitationInspected { preview } = call(
        &mut client,
        &socket,
        Request::InvitationInspect {
            ticket: ticket.clone(),
        },
        None,
        None,
    )?
    else {
        unreachable!("typed response")
    };
    if preview.goal != goal || preview.governance != observed.host {
        return Err(Failure::new(
            ErrorCode::Conflict,
            "local invitation does not match the reviewed goal",
        ));
    }
    let Response::Joined {
        goal: admitted_goal,
        membership,
        ..
    } = call(
        &mut client,
        &socket,
        Request::GoalJoin {
            agent: member,
            ticket,
        },
        None,
        None,
    )?
    else {
        unreachable!("typed response")
    };
    if admitted_goal != goal
        || membership != Membership::Member
        || !inspect(&mut client, &socket, goal)?
            .members
            .iter()
            .any(|entry| entry.member == member && entry.local)
    {
        return Err(Failure::unavailable(
            "local admission is not complete; inspect status and repeat goal add-local",
        ));
    }
    Ok(Output::success(
        json!({"goal":goal,"agent":member,"membership":"member","changed":true,"permissions_changed":false}),
        format!(
            "{} joined {}.\nLocal permissions remain unchanged. Inspect them with permission inspect, then explicitly allow the work you choose.",
            presentation::safe(&agent.name),
            presentation::safe(observed.title.as_deref().unwrap_or("this goal"))
        ),
    ))
}
