//! Explicit ordinary-directory actions and exact durable publication handles.
use super::{LocalClient, Output, acting_agent, confirm, connection, resolve_goal};
use crate::failure::Failure;
use clap::{Arg, ArgAction, ArgMatches, Command};
use locust_proto::api::{
    Checkout, DirectoryIdentity, ErrorCode, GoalStatus, Request, Response, SessionSecret,
    SessionState, WorkspaceAuthority, WorkspaceCandidate, WorkspaceOperation,
    WorkspaceOperationKind, WorkspaceOperationState, WorkspaceProposalView, WorkspaceRecovery,
    WorkspaceRevisionView, WorkspaceView,
};
use locust_proto::event::{Body, Context, RulesBinding, Scope, WorkspaceCheckpoint};
use locust_proto::id::{
    BlobHash, CheckoutId, EventId, GoalId, IdempotencyKey, InstanceId, PublicKey,
    WorkspaceOperationId,
};
use locust_proto::local;
use locust_proto::organization::{Authority, Formation, WorkspacePolicy};
use locust_workspace::{
    BlobSink, BlobSource, CaptureMode, CheckoutFacts, FrozenTree, SessionOwnership, TreeChange,
    WorkspaceError, checkout_disposition,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::error::Error;
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

fn option(name: &'static str, help: &'static str, required: bool) -> Arg {
    Arg::new(name).long(name).help(help).required(required)
}
fn flag(name: &'static str, help: &'static str) -> Arg {
    option(name, help, false).action(ArgAction::SetTrue)
}
fn goal() -> Arg {
    option("goal", "Goal title, full identifier or unique prefix", true)
}
fn checkout() -> Arg {
    option("checkout", "Exact local checkout identifier", true)
}
fn selected(command: Command) -> Command {
    command
        .arg(
            option(
                "path",
                "Exact root-relative regular file; repeat to select additions",
                false,
            )
            .action(ArgAction::Append),
        )
        .arg(option(
            "paths-from",
            "UTF-8 file with one exact path per line, or - for stdin",
            false,
        ))
}
fn publication(command: Command) -> Command {
    command.arg(flag(
        "publish",
        "Publish this one frozen candidate after its complete preview is captured",
    ))
}
pub(super) fn commands() -> [Command; 1] {
    [Command::new("workspace").about("Versioned shared file trees and explicit local checkouts").subcommand_required(true)
        .subcommand(confirm::flags(publication(selected(Command::new("init").about("Freeze an explicit seed and prepare its workspace policy")
            .arg(goal()).arg(option("root", "Absolute capture input directory", false).required_unless_present("empty"))
            .arg(flag("empty", "Explicitly seed an empty tree").conflicts_with_all(["root", "path", "paths-from", "commit"]))
            .arg(option("commit", "Optional named Git commit import; excludes dirty and untracked files", false).conflicts_with_all(["path", "paths-from", "empty"]))
            .arg(option("completion", "Exact workspace CompletionRule JSON; defaults to the goal's rule", false))))))
        .subcommand(Command::new("head").about("Read accepted authority and independent content readiness").arg(goal()))
        .subcommand(Command::new("pending").about("Read exact proposals and their current blockers").arg(goal()))
        .subcommand(Command::new("tree").about("List an exact accepted tree").arg(goal())
            .arg(option("revision", "Exact revision or prefix; defaults to accepted head", false))
            .arg(option("path", "Exact file or directory prefix", false))
            .arg(option("after-path", "Exclusive path pagination cursor", false))
            .arg(option("limit", "Explicit maximum entries to return", false)))
        .subcommand(Command::new("read").about("Read exact verified file bytes from an accepted revision").arg(goal())
            .arg(option("revision", "Exact revision or prefix; defaults to accepted head", false))
            .arg(option("path", "Exact relative regular-file path", true))
            .arg(option("offset", "Byte offset, defaults to zero", false))
            .arg(option("length", "Explicit maximum byte count", false)))
        .subcommand(confirm::flags(Command::new("connect").about("Copy an accepted revision into a fresh ordinary directory and connect it for an agent").arg(goal())
            .arg(option("revision", "Exact revision or prefix; defaults to accepted head", false))
            .arg(option("folder", "Absolute fresh destination directory", true))
            .arg(option("checkout", "Caller-selected checkout identifier, otherwise generated", false))
            .arg(option("task", "Optional associated task identifier or title", false))
            .arg(option("attempt", "Optional associated attempt event identifier", false).requires("task"))))
        .subcommand(Command::new("status").about("Observe local modifications, additions and recovery without capturing").arg(goal()).arg(checkout()))
        .subcommand(Command::new("bind").about("Bind this authenticated execution session explicitly to its checkout").arg(goal()).arg(checkout()))
        .subcommand(publication(selected(Command::new("propose").about("Freeze managed changes plus exact selected additions")
            .arg(goal()).arg(option("checkout", "Exact bound checkout identifier", true))
            .arg(flag("only", "Capture exactly the supplied path selection").conflicts_with("replace"))
            .arg(flag("replace", "Capture a complete replacement without reading the parent tree"))
            .arg(option("parent", "Exact broken/current parent revision for a replacement", false).requires("replace"))
            )))
        .subcommand(Command::new("publish").about("Publish the exact stored preview; never reread local files").arg(goal())
            .arg(option("operation", "Exact durable capture operation identifier", true)))
        .subcommand(Command::new("review").about("Review one exact proposal; optionally materialize it for owner-authorized checks").arg(goal())
            .arg(option("proposal", "Exact workspace proposal event or prefix", true))
            .arg(option("destination", "Optional absolute fresh review directory; no checkout binding", false)))
        .subcommand(publication(Command::new("compose").about("Compose ordered exact source proposals onto an accepted revision").arg(goal())
            .arg(option("head", "Exact target accepted revision; defaults to accepted head", false))
            .arg(option("source", "Exact source proposal, repeated in composition order", true).action(ArgAction::Append))))
        .subcommand(Command::new("integrate").about("Integrate the exact reviewed proposal with a durable receipt").arg(goal())
            .arg(option("proposal", "Exact workspace proposal event or prefix", true))
            .arg(option("expected-head", "Pin the current accepted revision", false).conflicts_with("expected-empty"))
            .arg(flag("expected-empty", "Explicitly pin an unseeded boundary"))
            .arg(option("expected-epoch", "Pin the active workspace epoch", false)))
        .subcommand(Command::new("update").about("Preserve compatible local work and recoverably update a bound checkout").arg(goal()).arg(checkout())
            .arg(option("revision", "Exact target revision; defaults to accepted head", false)))
        .subcommand(Command::new("recover").about("Resume an exact uncertain publication, integration or file update").arg(goal())
            .arg(option("operation", "Exact durable operation identifier", true)))]
}

pub(super) fn run(
    matches: &ArgMatches,
    operation: &str,
    args: &ArgMatches,
) -> Result<Output, Failure> {
    if operation == "workspace.init" && matches.get_one::<String>("agent").is_some() {
        return Err(Failure::usage(
            "host commands are the host's own and name no agent; drop --agent",
        ));
    }
    if operation == "workspace.connect" && !matches.get_flag("owner") {
        return Err(Failure::usage("workspace connect requires --owner"));
    }
    let home = connection::home(matches)?;
    let socket = local::socket_path(&home)?;
    let mut client = connection::open(matches, &home)?;
    let goal = resolve_goal(&mut client, &socket, value(args, "goal"), None)?;
    let agent_write = matches!(
        operation,
        "workspace.propose"
            | "workspace.publish"
            | "workspace.compose"
            | "workspace.integrate"
            | "workspace.update"
            | "workspace.recover"
            | "workspace.bind"
    );
    let on_behalf = if matches.get_flag("owner")
        && (matches.get_one::<String>("agent").is_some() || agent_write)
        && !matches!(operation, "workspace.init" | "workspace.connect")
    {
        Some(acting_agent(&mut client, &socket, matches, Some(goal))?)
    } else {
        None
    };
    let connecting_agent = if operation == "workspace.connect" {
        Some(acting_agent(&mut client, &socket, matches, Some(goal))?)
    } else {
        None
    };
    let session = connection::session_path(matches)?
        .as_deref()
        .map(connection::read_secret)
        .transpose()?
        .map(|secret| SessionSecret(secret).instance());
    if operation == "workspace.bind" && session.is_none() {
        return Err(Failure::usage(
            "workspace bind requires --session or LOCUST_SESSION",
        ));
    }
    let mut api = Objects {
        session,
        client: &mut client,
        socket: &socket,
        goal,
        on_behalf,
    };
    if operation == "workspace.init" {
        if !matches.get_flag("owner") {
            return Err(Failure::new(
                ErrorCode::Denied,
                "workspace init is your owner's command; use locust --owner workspace init",
            ));
        }
        api.on_behalf = Some(hosts_agent(&mut api)?);
    }
    let caller_key = matches
        .get_one::<String>("idempotency-key")
        .map(|text| {
            text.parse()
                .map_err(|_| Failure::usage("--idempotency-key requires 16 bytes in hex"))
        })
        .transpose()?;
    if matches!(
        operation,
        "workspace.init"
            | "workspace.propose"
            | "workspace.compose"
            | "workspace.integrate"
            | "workspace.update"
    ) && let Some(key) = caller_key
        && let Some(saved) = api
            .operations()?
            .into_iter()
            .find(|saved| saved.idempotency_key == key)
    {
        let compatible = matches!(
            (&saved.kind, operation),
            (
                WorkspaceOperationKind::Capture { .. },
                "workspace.init" | "workspace.propose" | "workspace.compose"
            ) | (
                WorkspaceOperationKind::Integrate { .. },
                "workspace.integrate"
            ) | (WorkspaceOperationKind::Update { .. }, "workspace.update")
        );
        if !compatible {
            return Err(Failure::new(
                ErrorCode::IdempotencyMismatch,
                "retry key names a different workspace operation kind",
            ));
        }
        if operation == "workspace.update" {
            return recover(&mut api, saved);
        }
        if operation == "workspace.integrate" {
            return response_output(api.call(Request::WorkspaceIntegrate {
                goal,
                operation: saved.id,
            })?);
        }
        return captured_output(
            &mut api,
            saved,
            args.get_flag("publish"),
            None,
            true,
            "frozen_scope",
        );
    }
    match operation {
        "workspace.head" => output(api.head()?),
        "workspace.pending" => response_output(api.call(Request::WorkspaceProposals { goal })?),
        "workspace.init" => review_init(matches, &mut api, args, caller_key),
        "workspace.propose" => propose(&mut api, args, caller_key),
        "workspace.publish" => {
            let operation = parse(value(args, "operation"), "operation")?;
            response_output(api.call(Request::WorkspacePublish { goal, operation })?)
        }
        "workspace.integrate" => integrate(&mut api, args, caller_key),
        "workspace.compose" => compose(&mut api, args, caller_key),
        "workspace.review" => review(&mut api, args),
        "workspace.connect" => review_connect(
            matches,
            &mut api,
            args,
            caller_key,
            connecting_agent.expect("checked owner"),
        ),
        "workspace.status" => status(&mut api, args),
        "workspace.bind" => response_output(api.call(Request::CheckoutBindSession {
            goal,
            checkout: parse(value(args, "checkout"), "checkout")?,
        })?),
        "workspace.update" => update(&mut api, args, caller_key),
        "workspace.recover" => {
            let operation = parse(value(args, "operation"), "operation")?;
            let saved = api.operation(operation)?;
            recover(&mut api, saved)
        }
        "workspace.tree" | "workspace.read" => {
            read_tree(&mut api, args, operation == "workspace.read")
        }
        _ => unreachable!("workspace dispatch"),
    }
}

/// The agent the first files are captured for: the goal's host's agent. Only
/// the computer that hosts the goal can start an epoch, and there the agent
/// must be connected, because only its change counts as the first files and
/// nothing acts for a disconnected agent.
fn hosts_agent(api: &mut Objects<'_>) -> Result<PublicKey, Failure> {
    let status = api.status()?;
    let host = match status.host {
        Some(host) if status.hosted_here => host,
        _ => {
            return Err(Failure::new(
                ErrorCode::Denied,
                "this goal is hosted on another computer; its host decides",
            ));
        }
    };
    let known = super::status(api.client, api.socket, None)?;
    let agent = known.agents.iter().find(|agent| agent.agent == host);
    if agent.is_none_or(|agent| agent.revoked) {
        let name = agent
            .map(|agent| super::presentation::safe(&agent.name))
            .unwrap_or_else(|| host.to_string());
        return Err(Failure::new(
            ErrorCode::Conflict,
            format!(
                "{name} is disconnected; only {name} can share this goal's first files\n  Connect it again: locust --owner agent reconnect --agent {name}"
            ),
        ));
    }
    Ok(host)
}

type InitialPolicy = (EventId, RulesBinding, Formation, bool);

fn init_plan(
    api: &mut Objects<'_>,
    args: &ArgMatches,
) -> Result<(confirm::Plan, WorkspaceView, InitialPolicy), Failure> {
    let head = api.head()?;
    let status = api.status()?;
    let policy = initial_policy(api, args)?;
    if let Some(epoch) = head.epoch
        && args.get_one::<String>("completion").is_some()
    {
        verify_pinned_initial_policy(api, epoch, args)?;
    }
    let review = json!({
        "goal": api.goal,
        "title": status.title,
        "current_rules": policy.0,
        "workspace_policy": policy.2.workspace,
        "epoch": head.epoch,
        "head": head.head.as_ref().map(|revision| revision.revision),
        "enabled": head.enabled,
        "authority": head.authority,
        "root": args.get_one::<String>("root"),
        "empty": args.get_flag("empty"),
        "commit": args.get_one::<String>("commit"),
        "path": args.get_many::<String>("path").map(|paths| paths.cloned().collect::<Vec<_>>()),
        "paths_from": args.get_one::<String>("paths-from"),
        "completion": args.get_one::<String>("completion"),
        "publish": args.get_flag("publish"),
    });
    let plan = confirm::Plan {
        command: "workspace init",
        human: format!(
            "Share the first files of \"{}\".\nSeed options: {}\nWorkspace epoch: {}; accepted head: {}.",
            super::presentation::safe(status.title.as_deref().unwrap_or("untitled goal")),
            serde_json::to_string_pretty(&review).map_err(internal)?,
            head.epoch
                .map(|id| id.to_string())
                .unwrap_or_else(|| "none".into()),
            head.head
                .as_ref()
                .map(|revision| revision.revision.to_string())
                .unwrap_or_else(|| "none".into()),
        ),
        review,
        warning: None,
        again: String::new(),
    };
    Ok((plan, head, policy))
}

fn review_init(
    matches: &ArgMatches,
    api: &mut Objects<'_>,
    args: &ArgMatches,
    key: Option<IdempotencyKey>,
) -> Result<Output, Failure> {
    let (plan, _, _) = init_plan(api, args)?;
    if confirm::decide(matches, args, &plan)? == confirm::Decision::Show {
        return Ok(plan.shown());
    }
    let (fresh, head, policy) = init_plan(api, args)?;
    confirm::bound(&plan.id(), &fresh)?;
    init(api, args, key, head, policy)
}

fn init(
    api: &mut Objects<'_>,
    args: &ArgMatches,
    key: Option<IdempotencyKey>,
    head: WorkspaceView,
    policy: InitialPolicy,
) -> Result<Output, Failure> {
    let current = api.head()?;
    if head.epoch != current.epoch
        || head.head.as_ref().map(|revision| revision.revision)
            != current.head.as_ref().map(|revision| revision.revision)
        || head.enabled != current.enabled
        || head.authority != current.authority
    {
        return Err(conflict("the plan changed; run --plan again"));
    }
    if head.head.is_some() {
        return Err(conflict(
            "workspace already has an accepted seed; use propose",
        ));
    }
    if head.epoch.is_some() && (!head.enabled || head.authority != WorkspaceAuthority::Ready) {
        return Err(conflict("existing unseeded workspace epoch is not ready"));
    }
    if let Some(epoch) = head.epoch
        && args.get_one::<String>("completion").is_some()
    {
        verify_pinned_initial_policy(api, epoch, args)?;
    }
    let paths = paths(args)?;
    let frozen = if let Some(commit) = args.get_one::<String>("commit") {
        let report = locust_workspace::export(&absolute(args, "root")?, commit, api)
            .map_err(workspace_error)?;
        if !report.refused.is_empty() {
            return Err(Failure::invalid(format!(
                "commit import contains refused paths: {:?}",
                report.refused
            )));
        }
        let (_, files) =
            locust_workspace::inspect_tree(report.manifest_id, api).map_err(workspace_error)?;
        FrozenTree {
            manifest_id: report.manifest_id,
            manifest: report.manifest,
            captured_paths: files.keys().cloned().collect(),
            changes: files
                .into_iter()
                .map(|(path, value)| TreeChange {
                    path,
                    before: None,
                    after: Some(value),
                })
                .collect(),
            already_included: false,
        }
    } else {
        let root = if args.get_flag("empty") {
            std::env::current_dir().map_err(io_failure)?
        } else {
            absolute(args, "root")?
        };
        locust_workspace::capture_seed(&root, &paths, args.get_flag("empty"), api)
            .map_err(workspace_error)?
    };
    let epoch = match head.epoch {
        Some(epoch) => epoch,
        None => initial_epoch(api, policy)?,
    };
    let candidate = WorkspaceCandidate {
        context: Context {
            scope: Scope::Workspace,
            round: epoch,
        },
        parent: None,
        result_manifest: frozen.manifest_id,
        sources: vec![],
        captured_paths: frozen.captured_paths.clone(),
        replacement: false,
    };
    let saved = prepare_capture(api, key, None, candidate)?;
    captured_output(
        api,
        saved,
        args.get_flag("publish"),
        Some(&frozen),
        false,
        "explicit_seed",
    )
}

fn verify_pinned_initial_policy(
    api: &mut Objects<'_>,
    epoch: EventId,
    args: &ArgMatches,
) -> Result<(), Failure> {
    let Response::Event(event) = api.call(Request::Event {
        goal: api.goal,
        event: epoch,
    })?
    else {
        return Err(Failure::internal("expected workspace epoch event"));
    };
    let Body::WorkspaceEpoch { rules, .. } = event.body else {
        return Err(Failure::invalid(
            "workspace epoch has a different event kind",
        ));
    };
    let Response::Event(event) = api.call(Request::Event {
        goal: api.goal,
        event: rules,
    })?
    else {
        return Err(Failure::internal("expected pinned rules event"));
    };
    let Body::RulesBound { binding, .. } = event.body else {
        return Err(Failure::invalid(
            "workspace epoch does not pin a formation binding",
        ));
    };
    let mut formation: Formation =
        serde_json::from_slice(&api.bytes(binding.definition.object.hash)?)
            .map_err(|error| Failure::invalid(format!("pinned formation definition: {error}")))?;
    let pinned = formation
        .workspace
        .clone()
        .ok_or_else(|| conflict("pinned workspace policy is disabled"))?;
    let requested = formation.workspace.as_mut().expect("checked policy");
    if let Some(source) = args.get_one::<String>("completion") {
        requested.completion = serde_json::from_str(source)
            .map_err(|error| Failure::usage(format!("--completion: {error}")))?;
    }
    let inspected =
        locust_core::organization::inspect(&serde_json::to_string(&formation).map_err(internal)?);
    let requested = inspected
        .normalized
        .and_then(|formation| formation.workspace);
    if requested.as_ref() != Some(&pinned) {
        return Err(conflict(
            "policy is already pinned and differs from supplied options; use rules bind to change it",
        ));
    }
    Ok(())
}

fn initial_policy(api: &mut Objects<'_>, args: &ArgMatches) -> Result<InitialPolicy, Failure> {
    let status = api.status()?;
    let rules = status
        .current_rules
        .ok_or_else(|| conflict("current formation rules are unavailable"))?;
    let Response::Event(event) = api.call(Request::Event {
        goal: api.goal,
        event: rules,
    })?
    else {
        unreachable!()
    };
    let Body::RulesBound { binding, .. } = event.body else {
        return Err(Failure::invalid(
            "current rules do not name a formation binding",
        ));
    };
    let bytes = api.bytes(binding.definition.object.hash)?;
    let mut formation: Formation = serde_json::from_slice(&bytes)
        .map_err(|error| Failure::invalid(format!("formation definition: {error}")))?;
    let host = status
        .host
        .ok_or_else(|| conflict("the host's agent has not arrived"))?;
    let explicit = args.get_one::<String>("completion").is_some();
    let mut rebind = formation.workspace.is_none() || explicit;
    let completion = args
        .get_one::<String>("completion")
        .map(|source| {
            serde_json::from_str(source)
                .map_err(|error| Failure::usage(format!("--completion: {error}")))
        })
        .transpose()?;
    match &mut formation.workspace {
        Some(policy) => {
            if let Some(completion) = completion {
                policy.completion = completion;
            }
            // A named accepting member who has left would leave the first
            // files with nobody to accept them; as with an emptied role, the
            // host's agent takes over, and the rebind shows it in the plan.
            if let Authority::Participant { key } = &policy.integrator
                && !status
                    .members
                    .iter()
                    .any(|member| member.member.to_string() == *key)
            {
                policy.integrator = Authority::Participant {
                    key: host.to_string(),
                };
                rebind = true;
            }
        }
        None => {
            formation.workspace = Some(WorkspacePolicy {
                integrator: Authority::Participant {
                    key: host.to_string(),
                },
                completion: completion.unwrap_or_else(|| formation.decisions.completion.clone()),
            });
        }
    }
    let inspected =
        locust_core::organization::inspect(&serde_json::to_string(&formation).map_err(internal)?);
    if !inspected.valid {
        let detail = inspected
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.path, diagnostic.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(Failure::invalid(format!(
            "the files need a completion rule they can meet: {detail}; use --completion to give the files their own rule"
        )));
    }
    Ok((
        rules,
        binding,
        inspected.normalized.expect("valid formation"),
        rebind,
    ))
}

fn initial_epoch(api: &mut Objects<'_>, policy: InitialPolicy) -> Result<EventId, Failure> {
    let (rules, binding, formation, rebind) = policy;
    let rules = if rebind {
        recorded(api.call(Request::RulesBind {
            no_role: true,
            goal: api.goal,
            expected: rules,
            formation_json: serde_json::to_string(&formation).map_err(internal)?,
            inputs: binding.inputs,
        })?)?
    } else {
        rules
    };
    recorded(api.call(Request::WorkspaceEpochSet {
        goal: api.goal,
        expected_epoch: None,
        rules,
        checkpoint: WorkspaceCheckpoint::Unseeded,
    })?)
}

fn propose(
    api: &mut Objects<'_>,
    args: &ArgMatches,
    key: Option<IdempotencyKey>,
) -> Result<Output, Failure> {
    let head = writable_head(api)?;
    let selected = paths(args)?;
    let (checkout_id, parent, frozen, replacement) = if args.get_flag("replace") {
        let parent = args
            .get_one::<String>("parent")
            .map(|text| api.event_id(text))
            .transpose()?
            .or_else(|| head.head.as_ref().map(|revision| revision.revision))
            .ok_or_else(|| conflict("replacement requires an explicit parent revision"))?;
        let revision = api.revision(parent)?;
        if !revision.in_lineage {
            return Err(conflict(
                "replacement parent is outside the retained workspace lineage",
            ));
        }
        let bound = api.checkout(parse(value(args, "checkout"), "checkout")?)?;
        check_checkout(&bound)?;
        let root = PathBuf::from(&bound.root);
        (
            Some(bound.id),
            Some(parent),
            locust_workspace::capture_seed(&root, &selected, args.get_flag("empty"), api)
                .map_err(workspace_error)?,
            true,
        )
    } else {
        let bound = api.checkout(parse(value(args, "checkout"), "checkout")?)?;
        check_checkout(&bound)?;
        if bound.active_operation.is_some() {
            return Err(conflict(
                "checkout has unresolved recovery; resume its exact operation",
            ));
        }
        let mode = if args.get_flag("only") {
            CaptureMode::Only
        } else {
            CaptureMode::ManagedAndSelected
        };
        let frozen = locust_workspace::capture_tree(
            Path::new(&bound.root),
            bound.base_manifest,
            &selected,
            mode,
            api,
        )
        .map_err(workspace_error)?;
        if frozen.changes.is_empty() {
            return output(
                json!({"checkout":bound.id,"parent":bound.base_revision,"no_changes":true,"captured_paths":frozen.captured_paths}),
            );
        }
        (Some(bound.id), Some(bound.base_revision), frozen, false)
    };
    let candidate = WorkspaceCandidate {
        context: Context {
            scope: Scope::Workspace,
            round: head.epoch.expect("writable head has epoch"),
        },
        parent,
        result_manifest: frozen.manifest_id,
        sources: vec![],
        captured_paths: frozen.captured_paths.clone(),
        replacement,
    };
    let saved = prepare_capture(api, key, checkout_id, candidate)?;
    let mode = if replacement {
        "full_replacement"
    } else if args.get_flag("only") {
        "only"
    } else {
        "managed_and_selected"
    };
    captured_output(
        api,
        saved,
        args.get_flag("publish"),
        Some(&frozen),
        false,
        mode,
    )
}

fn prepare_capture(
    api: &mut Objects<'_>,
    key: Option<IdempotencyKey>,
    checkout: Option<CheckoutId>,
    candidate: WorkspaceCandidate,
) -> Result<WorkspaceOperation, Failure> {
    let key = operation_key(key)?;
    api.prepare(WorkspaceOperation {
        id: WorkspaceOperationId(key.0),
        checkout,
        idempotency_key: key,
        kind: WorkspaceOperationKind::Capture { candidate },
        state: WorkspaceOperationState::Prepared,
    })
}
fn captured_output(
    api: &mut Objects<'_>,
    mut saved: WorkspaceOperation,
    publish: bool,
    frozen: Option<&FrozenTree>,
    resumed: bool,
    selection_mode: &str,
) -> Result<Output, Failure> {
    let WorkspaceOperationKind::Capture { candidate } = &saved.kind else {
        return Err(Failure::invalid("operation is not a frozen capture"));
    };
    let candidate = candidate.clone();
    if matches!(saved.state, WorkspaceOperationState::Recorded { .. }) {
        return output(
            json!({"operation":saved,"candidate":candidate,"preview":null,"selection_mode":"frozen_scope","review_mode":"recorded_receipt","resumed_frozen_capture":true,"published":true,
        "publication_boundary":"objects and operation stored by this daemon; peer replication is separate"}),
        );
    }
    let (review_mode, preview) = if let Some(frozen) = frozen {
        (
            if candidate.parent.is_none() {
                "seed_tree"
            } else if candidate.replacement {
                "full_replacement"
            } else {
                "base_diff"
            },
            serde_json::to_value(locust_workspace::review_changes(&frozen.changes))
                .map_err(internal)?,
        )
    } else {
        candidate_review(api, candidate.parent, candidate.result_manifest)?
    };
    if publish {
        saved = api.publication(saved.id)?;
    }
    output(
        json!({"operation":saved,"candidate":candidate,"preview":preview,"selection_mode":selection_mode,"review_mode":review_mode,"resumed_frozen_capture":resumed,
        "published":matches!(saved.state, WorkspaceOperationState::Recorded { .. }),
        "publication_boundary":"objects and operation stored by this daemon; peer replication is separate"}),
    )
}
fn integrate(
    api: &mut Objects<'_>,
    args: &ArgMatches,
    key: Option<IdempotencyKey>,
) -> Result<Output, Failure> {
    let head = writable_head(api)?;
    let expected_epoch = args
        .get_one::<String>("expected-epoch")
        .map(|text| api.event_id(text))
        .transpose()?
        .unwrap_or(head.epoch.unwrap());
    let expected_head = if args.get_flag("expected-empty") {
        None
    } else {
        args.get_one::<String>("expected-head")
            .map(|text| api.event_id(text))
            .transpose()?
            .or_else(|| head.head.as_ref().map(|revision| revision.revision))
    };
    if expected_epoch != head.epoch.unwrap()
        || expected_head != head.head.as_ref().map(|revision| revision.revision)
    {
        return Err(conflict("expected workspace epoch or head has changed"));
    }
    let proposal_id = api.event_id(value(args, "proposal"))?;
    let proposal = api.proposal(proposal_id)?;
    if proposal.context.round != expected_epoch || proposal.parent != expected_head {
        return Err(conflict(
            "proposal must be composed against the exact expected epoch and head",
        ));
    }
    let _ =
        locust_workspace::inspect_tree(proposal.result_manifest, api).map_err(workspace_error)?;
    let key = operation_key(key)?;
    let saved = api.prepare(WorkspaceOperation {
        id: WorkspaceOperationId(key.0),
        checkout: None,
        idempotency_key: key,
        kind: WorkspaceOperationKind::Integrate {
            expected_epoch,
            expected_head,
            proposal: proposal_id,
        },
        state: WorkspaceOperationState::Prepared,
    })?;
    response_output(api.call(Request::WorkspaceIntegrate {
        goal: api.goal,
        operation: saved.id,
    })?)
}
fn compose(
    api: &mut Objects<'_>,
    args: &ArgMatches,
    key: Option<IdempotencyKey>,
) -> Result<Output, Failure> {
    let head = writable_head(api)?;
    let revision = selected_revision(api, args, "head")?;
    let sources = args
        .get_many::<String>("source")
        .expect("required sources")
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut ids = Vec::new();
    let mut current = revision.result_manifest;
    let mut seen = BTreeSet::new();
    for text in sources {
        let source_id = api.event_id(text)?;
        if !seen.insert(source_id) {
            return Err(Failure::usage(
                "source proposals must be unique; order is preserved",
            ));
        }
        let source = api.proposal(source_id)?;
        if !source.usable_as_source {
            return Err(conflict(
                "source proposal is not eligible under the current epoch checkpoint",
            ));
        }
        let base = match source.parent {
            Some(parent) => {
                require_ancestor(api, parent, revision.revision)?;
                api.revision(parent)?.result_manifest
            }
            None => {
                return Err(conflict(
                    "seed sources require a fresh proposal against the accepted tree",
                ));
            }
        };
        current = locust_workspace::compose_trees(base, source.result_manifest, current, api)
            .map_err(workspace_error)?
            .manifest_id;
        ids.push(source_id);
    }
    let changes = locust_workspace::diff_trees(revision.result_manifest, current, api)
        .map_err(workspace_error)?;
    if changes.is_empty() {
        return output(
            json!({"already_included":true,"head":revision.revision,"sources":ids,"manifest":current}),
        );
    }
    let (manifest, _) = locust_workspace::inspect_tree(current, api).map_err(workspace_error)?;
    let frozen = FrozenTree {
        manifest_id: current,
        manifest,
        captured_paths: changes.iter().map(|change| change.path.clone()).collect(),
        changes,
        already_included: false,
    };
    let candidate = WorkspaceCandidate {
        context: Context {
            scope: Scope::Workspace,
            round: head.epoch.unwrap(),
        },
        parent: Some(revision.revision),
        result_manifest: current,
        sources: ids,
        captured_paths: frozen.captured_paths.clone(),
        replacement: false,
    };
    let saved = prepare_capture(api, key, None, candidate)?;
    captured_output(
        api,
        saved,
        args.get_flag("publish"),
        Some(&frozen),
        false,
        "ordered_composition",
    )
}
fn require_ancestor(
    api: &mut Objects<'_>,
    ancestor: EventId,
    target: EventId,
) -> Result<(), Failure> {
    let mut next = Some(target);
    let mut seen = BTreeSet::new();
    while let Some(id) = next {
        if !seen.insert(id) {
            return Err(conflict("workspace lineage contains a cycle"));
        }
        let revision = api.revision(id)?;
        if !revision.in_lineage {
            return Err(conflict(
                "composition references an excluded workspace revision",
            ));
        }
        if id == ancestor {
            return Ok(());
        }
        next = revision.parent;
    }
    Err(conflict(
        "source base is not an ancestor of the target revision; capture a fresh candidate",
    ))
}

fn review(api: &mut Objects<'_>, args: &ArgMatches) -> Result<Output, Failure> {
    let id = api.event_id(value(args, "proposal"))?;
    let proposal = api.proposal(id)?;
    let (mode, changes) = candidate_review(api, proposal.parent, proposal.result_manifest)?;
    let (manifest, _) =
        locust_workspace::inspect_tree(proposal.result_manifest, api).map_err(workspace_error)?;
    let destination = args
        .get_one::<String>("destination")
        .map(|_| absolute(args, "destination"))
        .transpose()?;
    if let Some(destination) = &destination {
        locust_workspace::materialize(&manifest, api, destination).map_err(workspace_error)?;
    }
    output(
        json!({"proposal":proposal,"review_mode":mode,"changes":changes,"destination":destination,"checkout_registered":false}),
    )
}
fn candidate_review(
    api: &mut Objects<'_>,
    parent: Option<EventId>,
    manifest: BlobHash,
) -> Result<(&'static str, Value), Failure> {
    let (_, files) = locust_workspace::inspect_tree(manifest, api).map_err(workspace_error)?;
    if let Some(parent) = parent {
        let delta = api.revision(parent).and_then(|revision| {
            locust_workspace::diff_trees(revision.result_manifest, manifest, api)
                .map_err(workspace_error)
        });
        match delta {
            Ok(delta) => {
                return Ok((
                    "base_diff",
                    serde_json::to_value(locust_workspace::review_changes(&delta))
                        .map_err(internal)?,
                ));
            }
            Err(error)
                if matches!(
                    error.code,
                    ErrorCode::Unavailable
                        | ErrorCode::Invalid
                        | ErrorCode::Corrupted
                        | ErrorCode::NotFound
                ) => {}
            Err(error) => return Err(error),
        }
    }
    let changes = files
        .into_iter()
        .map(|(path, value)| TreeChange {
            path,
            before: None,
            after: Some(value),
        })
        .collect::<Vec<_>>();
    Ok((
        if parent.is_some() {
            "full_replacement_base_unavailable"
        } else {
            "seed_tree"
        },
        serde_json::to_value(locust_workspace::review_changes(&changes)).map_err(internal)?,
    ))
}
struct ConnectSelection {
    revision: WorkspaceRevisionView,
    task: Option<locust_proto::event::TaskId>,
    attempt: Option<EventId>,
}

fn connect_plan(
    api: &mut Objects<'_>,
    args: &ArgMatches,
    agent: PublicKey,
) -> Result<(confirm::Plan, ConnectSelection), Failure> {
    let status = api.status()?;
    let agent_name = connected_agent_name(api, agent)?;
    let revision = selected_revision(api, args, "revision")?;
    let folder = absolute(args, "folder")?;
    let exists = match std::fs::symlink_metadata(&folder) {
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(io_failure(error)),
    };
    let task = args
        .get_one::<String>("task")
        .map(|text| super::selectors::resolve_task(api.client, api.socket, api.goal, text, None))
        .transpose()?;
    let attempt = args
        .get_one::<String>("attempt")
        .map(|text| api.event_id(text))
        .transpose()?;
    let review = json!({
        "goal": api.goal,
        "title": status.title,
        "agent": agent,
        "agent_name": agent_name,
        "revision": revision.revision,
        "folder": folder,
        "folder_exists": exists,
        "checkout": args.get_one::<String>("checkout"),
        "task": task,
        "attempt": attempt,
    });
    let plan = confirm::Plan {
        command: "workspace connect",
        human: format!(
            "Connect revision {} of \"{}\" for agent {} at {}.\nFolder exists: {}. The agent may propose changes from it.",
            &revision.revision.to_string()[..8],
            super::presentation::safe(status.title.as_deref().unwrap_or("untitled goal")),
            super::presentation::safe(&agent_name),
            folder.display(),
            exists,
        ),
        review,
        warning: None,
        again: String::new(),
    };
    Ok((
        plan,
        ConnectSelection {
            revision,
            task,
            attempt,
        },
    ))
}

fn review_connect(
    matches: &ArgMatches,
    api: &mut Objects<'_>,
    args: &ArgMatches,
    key: Option<IdempotencyKey>,
    agent: PublicKey,
) -> Result<Output, Failure> {
    let (plan, _) = connect_plan(api, args, agent)?;
    if confirm::decide(matches, args, &plan)? == confirm::Decision::Show {
        return Ok(plan.shown());
    }
    let (fresh, selected) = connect_plan(api, args, agent)?;
    confirm::bound(&plan.id(), &fresh)?;
    connect(api, args, key, agent, selected)
}

fn connect(
    api: &mut Objects<'_>,
    args: &ArgMatches,
    key: Option<IdempotencyKey>,
    agent: PublicKey,
    selected: ConnectSelection,
) -> Result<Output, Failure> {
    let destination = absolute(args, "folder")?;
    let id = args
        .get_one::<String>("checkout")
        .map(|text| parse(text, "checkout"))
        .transpose()?
        .unwrap_or(CheckoutId(operation_key(key)?.0));
    if let Some(saved) = api.checkouts()?.into_iter().find(|bound| bound.id == id) {
        check_checkout(&saved)?;
        if Path::new(&saved.root) != destination.canonicalize().map_err(io_failure)? {
            return Err(conflict("checkout identifier belongs to another directory"));
        }
        if let Some(text) = args.get_one::<String>("revision")
            && api.event_id(text)? != saved.base_revision
        {
            return Err(conflict(
                "retry revision differs from the registered checkout base",
            ));
        }
        return output(json!({"checkout":saved,"resumed_registered_checkout":true}));
    }
    let revision = api.revision(selected.revision.revision)?;
    if !revision.in_lineage || revision != selected.revision {
        return Err(conflict("the plan changed; run --plan again"));
    }
    // Preflight selectors and bindings before filesystem publication so an
    // invalid task/attempt (or a disputed revision) does not leave an
    // unregistered directory behind. This cannot be atomic against concurrent
    // server changes; the in-lineage guard after publication still retries.
    let task = selected.task;
    let attempt = selected.attempt;
    let (manifest, _) =
        locust_workspace::inspect_tree(revision.result_manifest, api).map_err(workspace_error)?;
    locust_workspace::materialize(&manifest, api, &destination).map_err(workspace_error)?;
    let root = destination.canonicalize().map_err(io_failure)?;
    let identity = directory_identity(&root)?;
    let checkout = Checkout {
        id,
        root: root.to_string_lossy().into_owned(),
        root_identity: identity,
        base_revision: revision.revision,
        base_manifest: revision.result_manifest,
        task,
        attempt,
        session: None,
        active_operation: None,
    };
    if !api.revision(revision.revision)?.in_lineage {
        return Err(conflict(
            "files copied, but revision became disputed before checkout registration",
        ));
    }
    let response = api
        .call(Request::WorkspaceConnect {
            goal: api.goal,
            agent,
            checkout,
        })
        .map_err(|error| {
            after_action(
                error,
                &format!(
                    "files copied at {}; checkout {} registration is uncertain",
                    root.display(),
                    id
                ),
            )
        })?;
    let Response::Checkout(connected) = response else {
        return Err(Failure::internal("expected connected checkout"));
    };
    let title = api
        .status()?
        .title
        .unwrap_or_else(|| "untitled goal".into());
    let agent_name = connected_agent_name(api, agent)?;
    let human = format!(
        "{} holds revision {} of \"{}\" and is connected for {}. {} may propose changes from it.",
        super::presentation::safe(&connected.root),
        &connected.base_revision.to_string()[..8],
        super::presentation::safe(&title),
        super::presentation::safe(&agent_name),
        super::presentation::safe(&agent_name),
    );
    Ok(Output::success(
        serde_json::to_value(Response::Checkout(connected)).map_err(internal)?,
        human,
    ))
}
fn connected_agent_name(api: &mut Objects<'_>, agent: PublicKey) -> Result<String, Failure> {
    super::status(api.client, api.socket, None)?
        .agents
        .into_iter()
        .find(|known| known.agent == agent)
        .map(|known| known.name)
        .ok_or_else(|| Failure::new(ErrorCode::NotFound, "connected agent is not enrolled"))
}
fn status(api: &mut Objects<'_>, args: &ArgMatches) -> Result<Output, Failure> {
    let bound = api.checkout(parse(value(args, "checkout"), "checkout")?)?;
    let head = api.head()?;
    let observations = check_checkout(&bound).and_then(|_| {
        locust_workspace::inspect_local_tree(Path::new(&bound.root)).map_err(workspace_error)
    });
    let base = locust_workspace::inspect_tree(bound.base_manifest, api).map_err(workspace_error);
    let mut dirty = Vec::new();
    let mut untracked = Vec::new();
    let mut conflicts = Vec::new();
    let mut inspection_known = observations.is_ok() && base.is_ok();
    let inspection_error = observations
        .as_ref()
        .err()
        .or_else(|| base.as_ref().err())
        .map(|error| error.message.clone());
    if let (Ok(observed), Ok((_, base))) = (&observations, &base) {
        let base_digests = locust_workspace::file_digests(base);
        dirty = base_digests
            .iter()
            .filter(|(path, before)| observed.files.get(*path) != Some(*before))
            .map(|(path, _)| path.clone())
            .collect();
        untracked = observed
            .files
            .keys()
            .filter(|path| !base_digests.contains_key(*path))
            .cloned()
            .collect();
        if let Some(target) = &head.head {
            match locust_workspace::inspect_tree(target.result_manifest, api) {
                Ok((_, target)) => {
                    let target_digests = locust_workspace::file_digests(&target);
                    if let Err(error) = locust_workspace::three_way_tree(
                        &base_digests,
                        &observed.files,
                        &target_digests,
                    ) {
                        conflicts.push(error.to_string());
                    }
                }
                Err(_) => inspection_known = false,
            }
        }
    }
    let disposition = disposition_facts(
        api,
        &bound,
        inspection_known,
        dirty.clone(),
        untracked.clone(),
        conflicts,
    )?;
    let operations = api
        .operations()?
        .into_iter()
        .filter(|operation| operation.checkout == Some(bound.id))
        .collect::<Vec<_>>();
    output(
        json!({"checkout":bound,"workspace":head,"dirty_paths":dirty,"untracked_paths":untracked,"operations":operations,
        "disposition":disposition,"newer_revision":head.head.as_ref().is_some_and(|head| head.revision != bound.base_revision),
        "inspection":if inspection_known {"known"} else {"unknown"},"inspection_error":inspection_error}),
    )
}
fn disposition_facts(
    api: &mut Objects<'_>,
    bound: &Checkout,
    inspection_known: bool,
    dirty_paths: Vec<String>,
    untracked_paths: Vec<String>,
    path_conflicts: Vec<String>,
) -> Result<locust_workspace::CheckoutDisposition, Failure> {
    let session = match bound.session {
        None => SessionOwnership::Unbound,
        Some(instance) if api.session == Some(instance) => SessionOwnership::Caller,
        Some(instance) => match api.call(Request::Session {
            instance: Some(instance),
        }) {
            Ok(Response::Session(session)) if session.attached => SessionOwnership::OtherActive,
            Ok(Response::Session(session)) => match session.record.state {
                SessionState::Exited => SessionOwnership::OtherInactive,
                SessionState::Unknown => SessionOwnership::Unknown,
                _ => SessionOwnership::OtherActive,
            },
            _ => SessionOwnership::Unknown,
        },
    };
    let publication_pending = api.operations()?.iter().any(|operation| {
        operation.checkout == Some(bound.id)
            && matches!(operation.kind, WorkspaceOperationKind::Capture { .. })
            && matches!(operation.state, WorkspaceOperationState::Prepared)
    });
    Ok(checkout_disposition(CheckoutFacts {
        session,
        inspection_known,
        recovery_pending: bound.active_operation.is_some(),
        publication_pending,
        publication_receipt_known: !publication_pending,
        dirty_paths,
        untracked_paths,
        path_conflicts,
    }))
}

fn update(
    api: &mut Objects<'_>,
    args: &ArgMatches,
    key: Option<IdempotencyKey>,
) -> Result<Output, Failure> {
    let bound = api.checkout(parse(value(args, "checkout"), "checkout")?)?;
    check_checkout(&bound)?;
    if let Some(operation) = bound.active_operation {
        return Err(conflict(&format!(
            "checkout requires recovery of operation {operation}"
        )));
    }
    let disposition = disposition_facts(api, &bound, true, vec![], vec![], vec![])?;
    if !disposition.update_allowed {
        return Err(conflict(&format!(
            "checkout update blocked: {:?}",
            disposition.update_blockers
        )));
    }
    let revision = selected_revision(api, args, "revision")?;
    let roots = managed_roots(api)?;
    let canonical_root = Path::new(&bound.root).canonicalize().map_err(io_failure)?;
    let parent = canonical_root
        .parent()
        .ok_or_else(|| conflict("checkout has no sibling recovery location"))?;
    api.call(Request::WorkspaceRecoveryParentCheck {
        goal: api.goal,
        parent: parent.to_string_lossy().into_owned(),
    })?;
    let plan = locust_workspace::plan_update(
        Path::new(&bound.root),
        bound.base_manifest,
        revision.result_manifest,
        api,
    )
    .map_err(workspace_error)?;
    let mut prepared = locust_workspace::prepare_update(Path::new(&bound.root), &roots, &plan)
        .map_err(workspace_error)?;
    let descriptor = prepared.descriptor().clone();
    let key = operation_key(key)?;
    let saved = api.prepare(WorkspaceOperation {
        id: WorkspaceOperationId(key.0),
        checkout: Some(bound.id),
        idempotency_key: key,
        kind: WorkspaceOperationKind::Update {
            expected_revision: bound.base_revision,
            target_revision: revision.revision,
            target_manifest: revision.result_manifest,
            recovery: recovery_descriptor(&descriptor),
        },
        state: WorkspaceOperationState::Prepared,
    })?;
    let report = prepared.execute(&descriptor).map_err(workspace_error)?;
    let complete = api
        .call(Request::WorkspaceOperationComplete {
            goal: api.goal,
            operation: saved.id,
        })
        .map_err(|error| {
            after_action(
                error,
                &format!(
                    "files applied; base completion is uncertain for operation {}",
                    saved.id
                ),
            )
        })?;
    prepared
        .mark_completed(&descriptor)
        .map_err(workspace_error)?;
    output(
        json!({"operation":complete,"files":report,"target_in_lineage_at_completion":completion_authority(&complete)?}),
    )
}
fn recover(api: &mut Objects<'_>, saved: WorkspaceOperation) -> Result<Output, Failure> {
    if matches!(saved.state, WorkspaceOperationState::Completed { .. }) {
        if let WorkspaceOperationKind::Update { recovery, .. } = &saved.kind {
            // A side marker permits later operations; failure cannot negate the
            // daemon receipt or require reapplying a user's changed checkout.
            let marker = locust_workspace::mark_update_completed(&update_descriptor(recovery));
            return output(
                json!({"operation":Response::WorkspaceOperation(saved.clone()),"target_in_lineage_at_completion":completion_authority(&Response::WorkspaceOperation(saved))?,"resumed_completed_receipt":true,
                "journal_completed":marker.is_ok(),"journal_error":marker.err().map(|error|error.to_string())}),
            );
        }
        return output(
            json!({"operation":Response::WorkspaceOperation(saved.clone()),"target_in_lineage_at_completion":completion_authority(&Response::WorkspaceOperation(saved))?,"resumed_completed_receipt":true}),
        );
    }
    match &saved.kind {
        WorkspaceOperationKind::Capture { .. } => {
            captured_output(api, saved, true, None, true, "frozen_scope")
        }
        WorkspaceOperationKind::Integrate { .. } => {
            response_output(api.call(Request::WorkspaceIntegrate {
                goal: api.goal,
                operation: saved.id,
            })?)
        }
        WorkspaceOperationKind::Update { recovery, .. } => {
            let descriptor = update_descriptor(recovery);
            let roots = managed_roots(api)?;
            let mut prepared =
                locust_workspace::reopen_update(&descriptor, &roots).map_err(workspace_error)?;
            let report = prepared.execute(&descriptor).map_err(workspace_error)?;
            let complete = api
                .call(Request::WorkspaceOperationComplete {
                    goal: api.goal,
                    operation: saved.id,
                })
                .map_err(|error| {
                    after_action(
                        error,
                        &format!(
                            "files applied; recover operation {} to reconcile its base",
                            saved.id
                        ),
                    )
                })?;
            prepared
                .mark_completed(&descriptor)
                .map_err(workspace_error)?;
            output(
                json!({"operation":complete,"files":report,"target_in_lineage_at_completion":completion_authority(&complete)?}),
            )
        }
    }
}
fn completion_authority(response: &Response) -> Result<bool, Failure> {
    match response {
        Response::WorkspaceOperation(WorkspaceOperation {
            state:
                WorkspaceOperationState::Completed {
                    target_in_lineage_at_completion,
                },
            ..
        }) => Ok(*target_in_lineage_at_completion),
        _ => Err(Failure::internal("expected completed workspace operation")),
    }
}

fn managed_roots(api: &mut Objects<'_>) -> Result<Vec<PathBuf>, Failure> {
    Ok(api
        .checkouts()?
        .into_iter()
        .map(|bound| PathBuf::from(bound.root))
        .collect())
}
fn recovery_descriptor(descriptor: &locust_workspace::UpdateDescriptor) -> WorkspaceRecovery {
    WorkspaceRecovery {
        root: descriptor.root.to_string_lossy().into_owned(),
        root_identity: DirectoryIdentity {
            device: descriptor.root_identity.device,
            inode: descriptor.root_identity.inode,
        },
        recovery_directory: descriptor.recovery_directory.to_string_lossy().into_owned(),
        recovery_identity: DirectoryIdentity {
            device: descriptor.recovery_identity.device,
            inode: descriptor.recovery_identity.inode,
        },
        plan_digest: descriptor.plan_digest,
    }
}
fn update_descriptor(descriptor: &WorkspaceRecovery) -> locust_workspace::UpdateDescriptor {
    locust_workspace::UpdateDescriptor {
        root: descriptor.root.clone().into(),
        root_identity: locust_workspace::DirectoryIdentity {
            device: descriptor.root_identity.device,
            inode: descriptor.root_identity.inode,
        },
        recovery_directory: descriptor.recovery_directory.clone().into(),
        recovery_identity: locust_workspace::DirectoryIdentity {
            device: descriptor.recovery_identity.device,
            inode: descriptor.recovery_identity.inode,
        },
        plan_digest: descriptor.plan_digest,
    }
}

fn read_tree(api: &mut Objects<'_>, args: &ArgMatches, read: bool) -> Result<Output, Failure> {
    let revision = args
        .get_one::<String>("revision")
        .map(|text| api.event_id(text))
        .transpose()?;
    if read {
        let path = value(args, "path");
        let Response::WorkspaceFile(file) = api.call(Request::WorkspaceRead {
            goal: api.goal,
            revision,
            path: path.into(),
        })?
        else {
            return Err(Failure::internal("expected workspace file"));
        };
        let offset = number(args, "offset")?.unwrap_or(0);
        let end = number(args, "length")?
            .map(|length| offset.saturating_add(length))
            .unwrap_or(file.bytes.len());
        if offset > file.bytes.len() {
            return Err(Failure::usage("--offset exceeds the file size"));
        }
        let bytes = &file.bytes[offset..end.min(file.bytes.len())];
        let text = std::str::from_utf8(bytes)
            .ok()
            .filter(|_| !bytes.contains(&0));
        output(
            json!({"revision":file.revision,"path":path,"executable":file.executable,"size":file.bytes.len(),"offset":offset,"length":bytes.len(),
            "text":text,"binary":text.is_none().then(|| json!({"bytes":bytes,"offset":offset,"length":bytes.len()}))}),
        )
    } else {
        let Response::WorkspaceTree(tree) = api.call(Request::WorkspaceTree {
            goal: api.goal,
            revision,
        })?
        else {
            return Err(Failure::internal("expected workspace tree"));
        };
        let manifest = tree.manifest;
        let prefix = args.get_one::<String>("path");
        let after = args.get_one::<String>("after-path");
        let limit = number(args, "limit")?;
        let selected: Vec<_> = manifest
            .entries
            .iter()
            .filter(|entry| {
                prefix.is_none_or(|prefix| {
                    entry.path == *prefix
                        || entry
                            .path
                            .starts_with(&format!("{}/", prefix.trim_end_matches('/')))
                })
            })
            .filter(|entry| after.is_none_or(|after| entry.path > *after))
            .collect();
        let count = limit.unwrap_or(selected.len()).min(selected.len());
        let entries = &selected[..count];
        let next = (selected.len() > count)
            .then(|| entries.last().map(|entry| entry.path.clone()))
            .flatten();
        output(json!({"revision":tree.revision,"entries":entries,"next_after_path":next}))
    }
}
fn writable_head(api: &mut Objects<'_>) -> Result<WorkspaceView, Failure> {
    let head = api.head()?;
    if !head.enabled || head.epoch.is_none() || head.authority != WorkspaceAuthority::Ready {
        return Err(conflict(
            "workspace epoch is not ready for publication or integration",
        ));
    }
    Ok(head)
}
fn selected_revision(
    api: &mut Objects<'_>,
    args: &ArgMatches,
    key: &str,
) -> Result<WorkspaceRevisionView, Failure> {
    let revision = match args.get_one::<String>(key) {
        Some(text) => {
            let id = api.event_id(text)?;
            api.revision(id)?
        }
        None => api.head()?.head.ok_or_else(|| {
            Failure::new(ErrorCode::Unavailable, "workspace has no accepted revision")
        })?,
    };
    if !revision.in_lineage {
        return Err(conflict(
            "revision is outside the retained accepted workspace lineage",
        ));
    }
    Ok(revision)
}
fn paths(args: &ArgMatches) -> Result<Vec<String>, Failure> {
    let mut paths = args
        .get_many::<String>("path")
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    if let Some(source) = args.get_one::<String>("paths-from") {
        let bytes = if source == "-" {
            let mut bytes = Vec::new();
            io::stdin().read_to_end(&mut bytes).map_err(io_failure)?;
            bytes
        } else {
            std::fs::read(source).map_err(io_failure)?
        };
        paths.extend(locust_workspace::parse_paths(&bytes).map_err(workspace_error)?);
    }
    // Reuse exact path parsing to reject duplicates across flags and file lists.
    if paths.iter().any(|path| path.contains('\n')) {
        return Err(Failure::usage("selected paths cannot contain newlines"));
    }
    let encoded = if paths.is_empty() {
        String::new()
    } else {
        format!("{}\n", paths.join("\n"))
    };
    locust_workspace::parse_paths(encoded.as_bytes()).map_err(workspace_error)
}
fn check_checkout(checkout: &Checkout) -> Result<(), Failure> {
    if directory_identity(Path::new(&checkout.root))? != checkout.root_identity {
        return Err(conflict("bound checkout directory identity changed"));
    }
    Ok(())
}
fn directory_identity(path: &Path) -> Result<DirectoryIdentity, Failure> {
    let metadata = std::fs::symlink_metadata(path).map_err(io_failure)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(conflict(
            "checkout root is not its original regular directory",
        ));
    }
    Ok(DirectoryIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}
fn operation_key(key: Option<IdempotencyKey>) -> Result<IdempotencyKey, Failure> {
    if let Some(key) = key {
        return Ok(key);
    }
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes)
        .map_err(|error| Failure::internal(format!("operation identity: {error}")))?;
    Ok(IdempotencyKey(bytes))
}
fn value<'a>(args: &'a ArgMatches, key: &str) -> &'a str {
    args.get_one::<String>(key).expect("required option")
}
fn parse<T: std::str::FromStr>(text: &str, key: &str) -> Result<T, Failure> {
    text.parse()
        .map_err(|_| Failure::usage(format!("--{key} requires a full identifier")))
}
fn absolute(args: &ArgMatches, key: &str) -> Result<PathBuf, Failure> {
    let path = PathBuf::from(value(args, key));
    if !path.is_absolute() {
        return Err(Failure::usage(format!("--{key} must be an absolute path")));
    }
    Ok(path)
}
fn number(args: &ArgMatches, key: &str) -> Result<Option<usize>, Failure> {
    args.get_one::<String>(key)
        .map(|value| {
            value
                .parse()
                .map_err(|_| Failure::usage(format!("--{key} requires a nonnegative integer")))
        })
        .transpose()
}
fn output(result: impl Serialize) -> Result<Output, Failure> {
    let result = serde_json::to_value(result).map_err(internal)?;
    let human = serde_json::to_string_pretty(&result).map_err(internal)?;
    Ok(Output::success(result, human))
}
fn response_output(response: Response) -> Result<Output, Failure> {
    output(response)
}
fn recorded(response: Response) -> Result<EventId, Failure> {
    match response {
        Response::Recorded { event } => Ok(event),
        _ => Err(Failure::internal("expected recorded event")),
    }
}
fn internal(error: impl std::fmt::Display) -> Failure {
    Failure::internal(error.to_string())
}
fn io_failure(error: io::Error) -> Failure {
    Failure::new(ErrorCode::Unavailable, error.to_string())
}
fn conflict(reason: &str) -> Failure {
    Failure::new(ErrorCode::Conflict, reason)
}
fn after_action(mut error: Failure, context: &str) -> Failure {
    error.message = format!("{context}: {}", error.message);
    error
}
fn workspace_error(error: impl Error + 'static) -> Failure {
    let mut cause: Option<&(dyn Error + 'static)> = Some(&error);
    while let Some(current) = cause {
        if let Some(failure) = current.downcast_ref::<Failure>() {
            let mut failure = failure.clone();
            failure.message = error.to_string();
            return failure;
        }
        cause = current
            .downcast_ref::<io::Error>()
            .and_then(|error| error.get_ref().map(|inner| inner as &(dyn Error + 'static)))
            .or_else(|| current.source());
    }
    let code = match (&error as &dyn Error).downcast_ref::<WorkspaceError>() {
        Some(WorkspaceError::MissingObject(_)) => ErrorCode::Unavailable,
        Some(WorkspaceError::Conflict { .. } | WorkspaceError::RecoveryRequired { .. }) => {
            ErrorCode::Conflict
        }
        _ => ErrorCode::Invalid,
    };
    Failure::new(code, error.to_string())
}
struct Objects<'a> {
    session: Option<InstanceId>,
    client: &'a mut LocalClient,
    socket: &'a Path,
    goal: GoalId,
    on_behalf: Option<PublicKey>,
}
impl Objects<'_> {
    fn call(&mut self, request: Request) -> Result<Response, Failure> {
        let on_behalf = if matches!(
            request,
            Request::RulesBind { .. }
                | Request::WorkspaceEpochSet { .. }
                | Request::WorkspaceConnect { .. }
        ) {
            None
        } else {
            self.on_behalf
        };
        self.client
            .call_with(request, None, on_behalf)
            .map_err(|error| connection::client_error(error, self.socket))
    }
    fn status(&mut self) -> Result<GoalStatus, Failure> {
        match self.call(Request::GoalStatus { goal: self.goal })? {
            Response::GoalStatus(status) => Ok(status),
            _ => unreachable!(),
        }
    }
    fn head(&mut self) -> Result<WorkspaceView, Failure> {
        match self.call(Request::WorkspaceHead { goal: self.goal })? {
            Response::Workspace(head) => Ok(head),
            _ => unreachable!(),
        }
    }
    fn event_id(&mut self, text: &str) -> Result<EventId, Failure> {
        super::selectors::resolve_event(self.client, self.socket, self.goal, text, self.on_behalf)
    }
    fn proposal(&mut self, proposal: EventId) -> Result<WorkspaceProposalView, Failure> {
        match self.call(Request::WorkspaceProposal {
            goal: self.goal,
            proposal,
        })? {
            Response::WorkspaceProposal(proposal) => Ok(proposal),
            _ => unreachable!(),
        }
    }
    fn revision(&mut self, revision: EventId) -> Result<WorkspaceRevisionView, Failure> {
        match self.call(Request::WorkspaceRevision {
            goal: self.goal,
            revision,
        })? {
            Response::WorkspaceRevision(revision) => Ok(revision),
            _ => unreachable!(),
        }
    }
    fn operations(&mut self) -> Result<Vec<WorkspaceOperation>, Failure> {
        match self.call(Request::WorkspaceOperations { goal: self.goal })? {
            Response::WorkspaceOperations(operations) => Ok(operations),
            _ => unreachable!(),
        }
    }
    fn operation(
        &mut self,
        operation: WorkspaceOperationId,
    ) -> Result<WorkspaceOperation, Failure> {
        match self.call(Request::WorkspaceOperationShow {
            goal: self.goal,
            operation,
        })? {
            Response::WorkspaceOperation(operation) => Ok(operation),
            _ => unreachable!(),
        }
    }
    fn checkouts(&mut self) -> Result<Vec<Checkout>, Failure> {
        match self.call(Request::Checkouts { goal: self.goal })? {
            Response::Checkouts(checkouts) => Ok(checkouts),
            _ => unreachable!(),
        }
    }
    fn checkout(&mut self, id: CheckoutId) -> Result<Checkout, Failure> {
        self.checkouts()?
            .into_iter()
            .find(|checkout| checkout.id == id)
            .ok_or_else(|| {
                Failure::new(
                    ErrorCode::NotFound,
                    "no bound checkout with this identifier",
                )
            })
    }
    fn prepare(&mut self, operation: WorkspaceOperation) -> Result<WorkspaceOperation, Failure> {
        let id = operation.id;
        let key = operation.idempotency_key;
        match self
            .call(Request::WorkspaceOperationPrepare {
                goal: self.goal,
                operation,
            })
            .map_err(|error| {
                after_action(
                    error,
                    &format!("exact operation {id} with retry key {key} requires reconciliation"),
                )
            })? {
            Response::WorkspaceOperation(operation) => Ok(operation),
            _ => unreachable!(),
        }
    }
    fn publication(
        &mut self,
        operation: WorkspaceOperationId,
    ) -> Result<WorkspaceOperation, Failure> {
        match self
            .call(Request::WorkspacePublish {
                goal: self.goal,
                operation,
            })
            .map_err(|error| {
                after_action(
                    error,
                    &format!("publication uncertain; recover exact operation {operation}"),
                )
            })? {
            Response::WorkspaceOperation(operation) => Ok(operation),
            _ => unreachable!(),
        }
    }
    fn bytes(&mut self, hash: BlobHash) -> Result<Vec<u8>, Failure> {
        match self.call(Request::BlobGet {
            goal: self.goal,
            hash,
        })? {
            Response::Blob { bytes } => Ok(bytes),
            _ => unreachable!(),
        }
    }
}
impl BlobSink for Objects<'_> {
    fn store(&mut self, bytes: &[u8]) -> io::Result<BlobHash> {
        match self
            .call(Request::BlobPut {
                goal: self.goal,
                bytes: bytes.to_vec(),
            })
            .map_err(io::Error::other)?
        {
            Response::BlobStored { hash } => Ok(hash),
            _ => unreachable!(),
        }
    }
}
impl BlobSource for Objects<'_> {
    fn fetch(&mut self, hash: &BlobHash) -> io::Result<Option<Vec<u8>>> {
        self.bytes(*hash).map(Some).map_err(io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(fields: &[&str]) -> Result<ArgMatches, clap::Error> {
        super::super::args::command()
            .try_get_matches_from(std::iter::once("locust").chain(fields.iter().copied()))
    }
    #[test]
    fn workspace_commands_replace_old_patch_and_binding_routes() {
        assert!(parse(&["patch", "create"]).is_err());
        assert!(parse(&["workspace", "materialize"]).is_err());
        assert!(parse(&["workspace", "init", "--goal", "goal", "--empty"]).is_ok());
        assert!(
            parse(&[
                "workspace",
                "init",
                "--goal",
                "goal",
                "--root",
                "/input",
                "--paths-from",
                "-"
            ])
            .is_ok()
        );
        assert!(
            parse(&[
                "workspace",
                "init",
                "--goal",
                "goal",
                "--empty",
                "--path",
                "file"
            ])
            .is_err()
        );
        assert!(
            parse(&[
                "workspace",
                "propose",
                "--goal",
                "goal",
                "--checkout",
                "01010101010101010101010101010101",
                "--only",
                "--paths-from",
                "/list",
                "--publish"
            ])
            .is_ok()
        );
        assert!(
            parse(&[
                "workspace",
                "integrate",
                "--goal",
                "goal",
                "--proposal",
                "01010101",
                "--expected-empty",
                "--expected-head",
                "02020202"
            ])
            .is_err()
        );
    }
    #[test]
    fn file_and_argument_lists_refuse_duplicates_and_empty_paths() {
        let matches = parse(&[
            "workspace",
            "propose",
            "--goal",
            "goal",
            "--checkout",
            "01010101010101010101010101010101",
            "--path",
            "file",
            "--path",
            "file",
        ])
        .unwrap();
        assert!(paths(super::super::args::selected(&matches).1).is_err());
    }
}
