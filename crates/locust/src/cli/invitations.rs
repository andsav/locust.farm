//! Person-facing invitation review and membership decisions.

use super::{Output, connection, presentation::safe, resolve_goal, resolve_principal};
use crate::failure::Failure;
use clap::{Arg, ArgMatches, Command};
use locust_proto::api::{
    ApiError, InvitationPreview, InvitationState, Membership, Request, Response,
};
use locust_proto::id::IdempotencyKey;
use locust_proto::invite::{Invitation, MAX_TICKET_BYTES, Ticket};
use locust_proto::local;
use serde_json::json;
use std::fs::OpenOptions;
use std::io::{self, Read};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn option(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name).long(name).required(true).help(help)
}

fn ticket_input(command: Command) -> Command {
    command
        .arg(
            Arg::new("ticket-file")
                .long("ticket-file")
                .required_unless_present("ticket")
                .conflicts_with("ticket")
                .help("Owner-only regular file containing the invitation; never echoed"),
        )
        .arg(
            Arg::new("ticket")
                .long("ticket")
                .required_unless_present("ticket-file")
                .help("Use - to read standard input; use --ticket-file for a saved invitation"),
        )
}

pub(super) fn command() -> Command {
    Command::new("invitation")
        .about("Inspect signed sharing facts, review joining, and manage issued invitations")
        .subcommand_required(true)
        .subcommand(ticket_input(Command::new("inspect").about("Verify and preview an invitation offline without redeeming it")))
        .subcommand(Command::new("list").about("Show issued invitation states without capabilities")
            .arg(option("goal", "Goal identifier or unique prefix")))
        .subcommand(Command::new("revoke").about("Revoke an unused invitation; requires --owner")
            .arg(option("goal", "Goal identifier or unique prefix"))
            .arg(option("invitation", "Full invitation identifier from invitation list")))
        .subcommand(ticket_input(Command::new("join").about("Join as an existing local principal after reviewing the exact ticket; requires --owner")
            .arg(option("principal", "Existing enrolled local principal name or full key"))
            .arg(option("review", "Full review identifier shown by invitation inspect"))))
}

pub(super) fn run(
    matches: &ArgMatches,
    operation: &str,
    args: &ArgMatches,
) -> Result<Output, Failure> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Failure::internal("system clock precedes the Unix epoch"))?
        .as_millis() as u64;
    let inspected = if matches!(operation, "invitation.inspect" | "invitation.join") {
        let ticket = read_ticket(args)?;
        let invitation = Invitation::from_ticket(ticket.as_str())
            .map_err(ApiError::from)
            .map_err(Failure::from)?;
        let preview = invitation
            .preview(now_ms)
            .map_err(ApiError::from)
            .map_err(Failure::from)?;
        Some((ticket, preview))
    } else {
        None
    };
    if operation == "invitation.inspect" {
        let (_, preview) = inspected.expect("inspection parsed its ticket");
        return output(Response::InvitationInspected { preview });
    }
    if matches!(
        operation,
        "invitation.join" | "invitation.revoke" | "invitation.list"
    ) {
        if !matches.get_flag("owner") {
            return Err(Failure::usage("this invitation decision requires --owner"));
        }
        if matches.get_one::<String>("as").is_some() {
            return Err(Failure::usage(
                "invitation commands use direct --owner authority; join selects --principal explicitly",
            ));
        }
    }
    if let Some((_, preview)) = &inspected
        && args.get_one::<String>("review") != Some(&preview.review)
    {
        return Err(Failure::new(
            locust_proto::api::ErrorCode::Conflict,
            "the reviewed invitation differs; inspect this exact ticket and confirm its review identifier",
        ));
    }
    let home = connection::home(matches)?;
    let socket = local::socket_path(&home)?;
    let mut client = connection::open(matches, &home)?;
    let on_behalf = matches
        .get_one::<String>("as")
        .map(|name| resolve_principal(&mut client, &socket, name))
        .transpose()?;
    let request = if operation == "invitation.join" {
        let principal = resolve_principal(&mut client, &socket, value(args, "principal"))?;
        let (ticket, _) = inspected.expect("joining parsed its ticket");
        Request::GoalJoin {
            agent: principal,
            ticket,
        }
    } else {
        let goal = resolve_goal(&mut client, &socket, value(args, "goal"), on_behalf)?;
        match operation {
            "invitation.list" => Request::GoalInvitations { goal },
            "invitation.revoke" => Request::InvitationRevoke {
                goal,
                invitation: Some(value(args, "invitation").to_owned()),
            },
            _ => return Err(Failure::usage("unknown invitation operation")),
        }
    };
    let idempotency = matches
        .get_one::<String>("idempotency-key")
        .map(|value| {
            value
                .parse::<IdempotencyKey>()
                .map_err(|_| Failure::usage("--idempotency-key requires 16 bytes in hex"))
        })
        .transpose()?;
    let response = client
        .call_with(request, idempotency, None)
        .map_err(|error| connection::client_error(error, &socket))?;
    output(response)
}

fn value<'a>(args: &'a ArgMatches, name: &str) -> &'a str {
    args.get_one::<String>(name).expect("required argument")
}

fn read_ticket(args: &ArgMatches) -> Result<Ticket, Failure> {
    if let Some(path) = args.get_one::<String>("ticket-file") {
        read_ticket_file(Path::new(path))
    } else if args
        .get_one::<String>("ticket")
        .is_some_and(|ticket| ticket != "-")
    {
        Err(Failure::usage(
            "literal tickets are not accepted here; use --ticket - or --ticket-file to keep the capability out of command history",
        ))
    } else {
        read_ticket_bytes(io::stdin().lock())
    }
}

fn read_ticket_file(path: &Path) -> Result<Ticket, Failure> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)
        .map_err(|error| Failure::invalid(format!("cannot open invitation file: {error}")))?;
    let metadata = file
        .metadata()
        .map_err(|error| Failure::invalid(format!("cannot inspect invitation file: {error}")))?;
    if !metadata.is_file()
        || !matches!(metadata.permissions().mode() & 0o7777, 0o600 | 0o400)
        || metadata.uid() != rustix::process::getuid().as_raw()
    {
        return Err(Failure::invalid(
            "invitation file must be a regular file owned by the current user with mode 0600 or 0400",
        ));
    }
    read_ticket_bytes(file)
}

fn read_ticket_bytes(reader: impl Read) -> Result<Ticket, Failure> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_TICKET_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| Failure::invalid(format!("cannot read invitation: {error}")))?;
    if bytes.len() > MAX_TICKET_BYTES {
        return Err(Failure::invalid(
            "invitation input exceeds the published ticket size limit",
        ));
    }
    let text =
        String::from_utf8(bytes).map_err(|_| Failure::invalid("invitation is not UTF-8 text"))?;
    Ok(Ticket(text.trim().to_owned()))
}

fn output(response: Response) -> Result<Output, Failure> {
    let human = match &response {
        Response::InvitationInspected { preview } => render_preview(preview),
        Response::Invitations { invitations } => {
            if invitations.is_empty() {
                "No invitations have been issued for this goal.".into()
            } else {
                invitations
                    .iter()
                    .map(|invitation| {
                        format!(
                            "{}  {}  expires {}{}",
                            invitation.invitation,
                            state(invitation.state),
                            invitation
                                .expires_ms
                                .map_or_else(|| "never".into(), |time| format!("{time} Unix ms")),
                            invitation
                                .redeemed_by
                                .map_or_else(String::new, |member| format!("  member {member}")),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        Response::InvitationRevoked { invitation } => format!(
            "Invitation {} is revoked. It cannot admit a new member.",
            invitation.invitation
        ),
        Response::InvitationsRevoked { count } => format!("Revoked {count} pending invitations."),
        Response::Joined {
            goal,
            governance,
            membership,
        } => match membership {
            Membership::Member => format!(
                "Joined goal {goal} as a member. Host: {governance}.\nMembership granted no execution permission or workspace access."
            ),
            _ => format!(
                "Joining goal {goal}; admission from host {governance} has not arrived.\nRetry the same reviewed invitation to recover the pending result, or check status. No goal content or execution permission is granted while joining."
            ),
        },
        _ => return Err(Failure::internal("unexpected invitation response")),
    };
    Ok(Output::success(json!(response), human))
}

fn state(state: InvitationState) -> &'static str {
    match state {
        InvitationState::Pending => "pending",
        InvitationState::Expired => "expired",
        InvitationState::Revoked => "revoked",
        InvitationState::Redeemed => "redeemed",
    }
}

fn render_preview(preview: &InvitationPreview) -> String {
    let title = preview
        .goal_title
        .as_deref()
        .map_or_else(|| "(not supplied)".into(), safe);
    let expires = preview
        .expires_ms
        .map_or_else(|| "never".into(), |time| format!("{time} Unix ms"));
    let mut text = format!(
        "Goal: {title}\nGoal identifier: {}\nHost fingerprint: {}\nIssuer endpoint: {}\nSignature: verified against the host key.\nTitle: host-signed presentation. A signing key does not verify a human identity.\nGoal authority and admission are confirmed during joining. Inspection does not contact the issuer.\nExpires: {expires}{}\nSharing: whole goal.\n",
        preview.goal,
        preview.governance,
        preview.endpoint,
        if preview.expired {
            " (expired; request a fresh invitation)"
        } else {
            ""
        },
    );
    if let Some(advertised) = &preview.publication {
        text.push_str(&format!(
            "Publication policy at issuance: {}\n",
            safe(&serde_json::to_string(&advertised.publication).expect("publication JSON"))
        ));
        text.push_str("Joining does not consent to public publication. Review the current policy after joining and explicitly consent with farm consent.\n");
    } else {
        text.push_str("Publication policy at issuance: none. Joining does not consent to future public publication.\n");
    }
    for fact in &preview.sharing_facts {
        text.push_str(&format!("- {fact}\n"));
    }
    text.push_str(&format!("Review identifier: {}\nTo accept, use invitation join with the same ticket input, --owner, --principal and --review. To decline, take no action; inspection made no changes.", preview.review));
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_proto::crypto::Keypair;
    use locust_proto::id::{EndpointId, GoalId};
    use locust_proto::invite::InviteSecret;
    use std::fs;

    fn invitation() -> Invitation {
        Invitation::signed(
            GoalId([1; 32]),
            Some("A shared goal\nforged terminal line".into()),
            EndpointId([2; 32]),
            vec![],
            InviteSecret([3; 32]),
            None,
            &Keypair::from_seed([4; 32]),
        )
        .unwrap()
    }

    #[test]
    fn offline_review_explains_provenance_without_printing_capability() {
        let invitation = invitation();
        let preview = invitation.preview(0).unwrap();
        let rendered = render_preview(&preview);
        assert!(rendered.contains("host-signed presentation"));
        assert!(rendered.contains("Joining does not consent"));
        assert!(rendered.contains("does not verify a human identity"));
        assert!(rendered.contains("including available history"));
        assert!(rendered.contains("grants no local execution"));
        assert!(rendered.contains("A shared goal\\u{a}forged terminal line"));
        assert!(!rendered.contains(&"03".repeat(32)));
        assert!(!rendered.contains("locust-invite-"));
    }

    #[test]
    fn file_input_requires_private_owned_regular_file_and_never_follows_symlinks() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("invitation");
        let ticket = invitation().to_ticket().unwrap();
        fs::write(&path, ticket.as_str()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(read_ticket_file(&path).unwrap(), ticket);
        let link = directory.path().join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(read_ticket_file(&link).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read_ticket_file(&path).is_err());
        assert!(read_ticket_file(directory.path()).is_err());
    }

    #[test]
    fn stdin_and_file_reading_share_a_bounded_secret_free_error_path() {
        assert_eq!(
            read_ticket_bytes(&b"  locust-invite-aabb\n"[..])
                .unwrap()
                .as_str(),
            "locust-invite-aabb"
        );
        let input = vec![b'a'; MAX_TICKET_BYTES + 1];
        let error = read_ticket_bytes(input.as_slice()).unwrap_err();
        assert!(!error.message.contains("aaaa"));
        assert!(read_ticket_bytes(&[255][..]).is_err());
    }
}
