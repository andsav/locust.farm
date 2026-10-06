//! Person-facing invitation review and membership decisions.

use super::{
    Output, connection,
    presentation::{self, safe},
    resolve_goal,
};
use crate::failure::Failure;
use clap::{Arg, ArgAction, ArgMatches, Command};
use locust_proto::api::{ApiError, InvitationPreview, InvitationState, Request, Response};
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

pub(super) fn ticket_input(command: Command) -> Command {
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
        .subcommand(ticket_input(Command::new("inspect").about(
            "Verify and preview an invitation offline without redeeming it",
        )))
        .subcommand(
            Command::new("list")
                .about("Show issued invitation states without capabilities")
                .arg(option("goal", "Goal identifier or unique prefix")),
        )
        .subcommand(
            Command::new("revoke")
                .about("Stop unused invitation admission immediately")
                .arg(option("goal", "Goal identifier or unique prefix"))
                .arg(
                    Arg::new("invitation")
                        .long("invitation")
                        .required_unless_present("all")
                        .conflicts_with("all")
                        .help("Full invitation identifier from invitation list"),
                )
                .arg(
                    Arg::new("all")
                        .long("all")
                        .action(ArgAction::SetTrue)
                        .help("Revoke every pending invitation for this goal"),
                ),
        )
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
    let inspected = if operation == "invitation.inspect" {
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
        return output(Response::InvitationInspected { preview }, now_ms);
    }
    if matches.get_one::<String>("agent").is_some() {
        return Err(Failure::usage(
            "host commands are the host's own and name no agent; drop --agent",
        ));
    }
    let home = connection::home(matches)?;
    let socket = local::socket_path(&home)?;
    let mut client = connection::open(matches, &home)?;
    let goal = resolve_goal(&mut client, &socket, value(args, "goal"), None)?;
    let request = match operation {
        "invitation.list" => Request::GoalInvitations { goal },
        _ => return Err(Failure::usage("unknown invitation operation")),
    };
    let response = client
        .call(request)
        .map_err(|error| connection::client_error(error, &socket))?;
    output(response, now_ms)
}

fn value<'a>(args: &'a ArgMatches, name: &str) -> &'a str {
    args.get_one::<String>(name).expect("required argument")
}

pub(super) fn read_ticket(args: &ArgMatches) -> Result<Ticket, Failure> {
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
    Ok(Ticket(text.trim_end_matches(['\r', '\n']).to_owned()))
}

fn output(response: Response, now_ms: u64) -> Result<Output, Failure> {
    let human = match &response {
        Response::InvitationInspected { preview } => render_preview(preview, now_ms),
        Response::Invitations { invitations } => {
            if invitations.is_empty() {
                "No invitations have been issued for this goal.".into()
            } else {
                invitations
                    .iter()
                    .map(|invitation| {
                        format!(
                            "{}  {}{}{}",
                            invitation.invitation,
                            state(invitation.state),
                            if invitation.state == InvitationState::Pending {
                                invitation.expires_ms.map_or_else(String::new, |time| {
                                    format!("  expires {}", presentation::expires_in(time, now_ms))
                                })
                            } else {
                                String::new()
                            },
                            invitation
                                .redeemed_by
                                .map_or_else(String::new, |member| format!("  member {member}")),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
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

fn render_preview(preview: &InvitationPreview, now_ms: u64) -> String {
    let title = preview
        .goal_title
        .as_deref()
        .map_or_else(|| "(not supplied)".into(), safe);
    let expires = preview.expires_ms.map_or_else(
        || "unspecified".into(),
        |time| {
            format!(
                "{} ({})",
                presentation::expires_in(time, now_ms),
                presentation::utc(time)
            )
        },
    );
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
    text.push_str("To accept, run locust --owner goal join with the same ticket input. To decline, take no action; inspection made no changes.");
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
        let rendered = render_preview(&preview, 0);
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
            read_ticket_bytes(&b"locust-invite-aabb\n"[..])
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
