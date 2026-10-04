//! Explicit local file actions over the same authenticated API as MCP.
use super::{LocalClient, Output, connection, resolve_goal, resolve_principal};
use crate::failure::Failure;
use clap::{Arg, ArgAction, ArgMatches, Command};
use locust_proto::api::{ErrorCode, GoalStatus, Request, Response, WorkspaceBinding};
use locust_proto::crypto::content_hash;
use locust_proto::event::Body;
use locust_proto::id::{BlobHash, GoalId, PublicKey};
use locust_proto::local;
use locust_proto::manifest::Manifest;
use locust_workspace::{BlobSink, BlobSource, ContributionError};
use serde_json::{Value, json};
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};

fn option(name: &'static str, help: &'static str, required: bool) -> Arg {
    Arg::new(name).long(name).help(help).required(required)
}
fn goal() -> Arg {
    option("goal", "Goal identifier or unique prefix", true)
}
fn root() -> Arg {
    option(
        "root",
        "Absolute directory explicitly chosen for this operation",
        true,
    )
}
fn patch() -> Arg {
    option("patch", "Full contribution content identifier", true)
}
fn commit() -> Arg {
    option(
        "commit",
        "Exact committed tree to share; dirty and untracked files are excluded",
        true,
    )
}
pub(super) fn commands() -> [Command; 2] {
    [Command::new("workspace").about("Preview, export and materialize explicit workspace snapshots").subcommand_required(true)
        .subcommand(Command::new("preview").about("Review a committed export locally without sharing bytes").arg(root()).arg(commit()))
        .subcommand(Command::new("export").about("Store a committed snapshot in the selected goal and record its local binding").arg(goal()).arg(root()).arg(commit()))
        .subcommand(Command::new("materialize").about("Write a received snapshot into a new directory").arg(goal()).arg(option("manifest", "Full snapshot manifest identifier", true)).arg(option("destination", "Absolute new destination directory", true))),
     Command::new("patch").about("Create, review, publish, select and apply inert workspace contributions").subcommand_required(true)
        .subcommand(Command::new("create").about("Capture a committed tree or explicitly selected regular files against an exact base")
            .arg(goal()).arg(root()).arg(option("base", "Full base snapshot manifest identifier", true))
            .arg(option("commit", "Committed tree to compare with the base", false).required_unless_present("path").conflicts_with("path"))
            .arg(option("path", "Exact relative file to capture, or missing base file to delete; repeat for each path", false).action(ArgAction::Append).required_unless_present("commit")))
        .subcommand(Command::new("review").about("Read exact before/after content and show text diffs or binary summaries").arg(goal()).arg(patch()))
        .subcommand(Command::new("submit").about("Submit a validated contribution using the current claimed generation")
            .arg(goal()).arg(patch()).arg(option("attempt", "Full attempt event identifier", true))
            .arg(option("generation", "Current claim generation", true))
            .arg(Arg::new("summary").required(true).allow_hyphen_values(true).help("Summary text, or - to read standard input")))
        .subcommand(Command::new("select").about("Select an exact reviewed contribution within its scope")
            .arg(goal()).arg(option("subject", "Full contribution event identifier", true)).arg(option("expected", "Current scope selection decision identifier", false)).arg(patch()))
        .subcommand(Command::new("apply").about("Apply a selected contribution to a recorded root; preserve originals for recovery")
            .arg(goal()).arg(root()).arg(patch()).arg(option("subject", "Exact selected contribution event identifier", true)).arg(option("expected-base", "Exact manifest the local changes are expected to start from", true))
            .arg(option("expected-git-head", "Full expected Git HEAD; required when applying to an exported Git root", false)))]
}

pub(super) fn run(
    matches: &ArgMatches,
    operation: &str,
    args: &ArgMatches,
) -> Result<Output, Failure> {
    if matches.get_one::<String>("idempotency-key").is_some() {
        return Err(Failure::usage(
            "workspace and patch commands perform multiple operations; use the exact same object identifiers to retry, without --idempotency-key",
        ));
    }
    if operation == "workspace.preview" {
        let root = absolute(args, "root")?;
        let report = locust_workspace::export(&root, value(args, "commit"), &mut Preview)
            .map_err(workspace_error)?;
        let result = export_json(&report, false);
        return output(result);
    }
    let home = connection::home(matches)?;
    let socket = local::socket_path(&home)?;
    let mut client = connection::open(matches, &home)?;
    let on_behalf = matches
        .get_one::<String>("as")
        .map(|name| resolve_principal(&mut client, &socket, name))
        .transpose()?;
    let goal = resolve_goal(&mut client, &socket, value(args, "goal"), on_behalf)?;
    let mut api = Objects {
        client: &mut client,
        socket: &socket,
        goal,
        on_behalf,
    };
    let status = api.status()?;
    let mut binding = status.workspace.clone().unwrap_or_default();
    match operation {
        "workspace.export" => {
            let root = absolute(args, "root")?;
            let report = locust_workspace::export(&root, value(args, "commit"), &mut api)
                .map_err(workspace_error)?;
            binding.export_root = Some(root.to_string_lossy().into_owned());
            binding.source_commit = Some(report.commit.clone());
            binding.exported = Some(report.manifest_id);
            // Export reads a commit, not the working copy; a prior integration
            // marker cannot attest to the newly selected local files.
            binding.integrated = None;
            api.bind(binding).map_err(|e| {
                after_action(e, "snapshot stored; workspace binding was not recorded")
            })?;
            output(export_json(&report, true))
        }
        "workspace.materialize" => {
            let id = hash(args, "manifest")?;
            let destination = absolute(args, "destination")?;
            let bytes = api.bytes(id)?;
            let manifest = Manifest::decode(&bytes).map_err(|e| Failure::invalid(e.to_string()))?;
            locust_workspace::materialize(&manifest, &mut api, &destination)
                .map_err(workspace_error)?;
            binding.destination = Some(destination.to_string_lossy().into_owned());
            binding.integrated = Some(id);
            api.bind(binding).map_err(|e| {
                after_action(
                    e,
                    "snapshot materialized; workspace binding was not recorded",
                )
            })?;
            output(
                json!({"manifest":id,"destination":destination,"files":manifest.entries.len(),"integrated":true}),
            )
        }
        "patch.create" => {
            let root = absolute(args, "root")?;
            require_binding(&binding, &root)?;
            let base = hash(args, "base")?;
            let report = if let Some(commit) = args.get_one::<String>("commit") {
                locust_workspace::create_committed(&root, commit, base, &mut api)
            } else {
                let paths = args
                    .get_many::<String>("path")
                    .expect("clap requires selected paths")
                    .cloned()
                    .collect::<Vec<_>>();
                locust_workspace::create_selected(&root, &paths, base, &mut api)
            }
            .map_err(workspace_error)?;
            output(serde_json::to_value(report).map_err(|e| Failure::internal(e.to_string()))?)
        }
        "patch.review" => {
            let review = locust_workspace::review_contribution(hash(args, "patch")?, &mut api)
                .map_err(workspace_error)?;
            output(serde_json::to_value(review).map_err(|e| Failure::internal(e.to_string()))?)
        }
        "patch.submit" => {
            if connection::session_path(matches)?.is_none() {
                return Err(Failure::usage(
                    "patch submit requires --session or LOCUST_SESSION for the claimed generation",
                ));
            }
            let patch = hash(args, "patch")?;
            let review =
                locust_workspace::review_contribution(patch, &mut api).map_err(workspace_error)?;
            let attempt = value(args, "attempt")
                .parse()
                .map_err(|_| Failure::usage("--attempt requires a full event identifier"))?;
            let generation = value(args, "generation")
                .parse()
                .map_err(|_| Failure::usage("--generation requires an unsigned 32-bit integer"))?;
            let summary = if value(args, "summary") == "-" {
                super::stdin_text()?
            } else {
                value(args, "summary").to_owned()
            };
            let Response::Pending(work) = api.call(Request::Pending { goal })? else {
                unreachable!()
            };
            let task = work
                .claimed
                .iter()
                .find(|claim| claim.attempt == attempt && claim.generation == generation)
                .ok_or_else(|| {
                    Failure::new(
                        ErrorCode::Superseded,
                        "this session does not hold the current attempt generation",
                    )
                })?
                .task;
            let response = api.call(Request::ContributionPublish {
                goal,
                task: Some(task),
                attempt: Some(attempt),
                generation: Some(generation),
                summary,
                base: Some(review.base),
                patch: Some(patch),
                artifacts: vec![review.head],
            })?;
            response_output(response)
        }
        "patch.select" => {
            let subject = value(args, "subject")
                .parse()
                .map_err(|_| Failure::usage("--subject requires a full event identifier"))?;
            let patch = hash(args, "patch")?;
            let review =
                locust_workspace::review_contribution(patch, &mut api).map_err(workspace_error)?;
            let Response::Event(event) = api.call(Request::Event {
                goal,
                event: subject,
            })?
            else {
                return Err(Failure::internal("expected event detail"));
            };
            match event.body {
                Body::ContributionPublished {
                    base: Some(base),
                    patch: Some(stored),
                    ..
                } if base == review.base && stored == patch => {}
                _ => {
                    return Err(Failure::new(
                        ErrorCode::Conflict,
                        "subject does not name this exact contribution and base",
                    ));
                }
            }
            let expected = args
                .get_one::<String>("expected")
                .map(|text| text.parse())
                .transpose()
                .map_err(|_| Failure::usage("--expected requires a full event identifier"))?;
            response_output(api.call(Request::ScopeSelect {
                goal,
                subject,
                expected,
            })?)
        }
        "patch.apply" => {
            let root = absolute(args, "root")?;
            require_binding(&binding, &root)?;
            let expected_base = hash(args, "expected-base")?;
            let patch = hash(args, "patch")?;
            let subject = value(args, "subject")
                .parse()
                .map_err(|_| Failure::usage("--subject requires a full event identifier"))?;
            require_selected(&mut api, subject, patch)?;
            let review =
                locust_workspace::review_contribution(patch, &mut api).map_err(workspace_error)?;
            let selected_artifact = review.head;
            let expected_git_head = args
                .get_one::<String>("expected-git-head")
                .map(String::as_str);
            if binding
                .export_root
                .as_deref()
                .is_some_and(|p| same_root(Path::new(p), &root))
                && expected_git_head.is_none()
            {
                return Err(Failure::usage(
                    "applying to an exported Git root requires --expected-git-head with its full current commit identifier",
                ));
            }
            let report = locust_workspace::apply_contribution(
                patch,
                &mut api,
                &root,
                expected_base,
                selected_artifact,
                expected_git_head,
            )
            .map_err(workspace_error)?;
            require_selected(&mut api, subject, patch).map_err(|error| after_action(error,"files applied; scope selection changed and integration binding was not recorded"))?;
            binding.integrated = Some(selected_artifact);
            api.bind(binding).map_err(|e| after_action(e, "files applied; integration binding was not recorded; retry this exact contribution"))?;
            output(serde_json::to_value(report).map_err(|e| Failure::internal(e.to_string()))?)
        }
        _ => unreachable!("workspace dispatch"),
    }
}
fn require_selected(
    api: &mut Objects<'_>,
    subject: locust_proto::id::EventId,
    patch: BlobHash,
) -> Result<(), Failure> {
    let Response::Contributions(contributions) = api.call(Request::Contributions {
        goal: api.goal,
        task: None,
    })?
    else {
        unreachable!()
    };
    if contributions.iter().any(|contribution| {
        contribution.contribution == subject
            && contribution.selected
            && contribution.patch == Some(patch)
    }) {
        Ok(())
    } else {
        Err(Failure::new(
            ErrorCode::Conflict,
            "exact contribution is not currently selected in its scope",
        ))
    }
}
fn value<'a>(args: &'a ArgMatches, key: &str) -> &'a str {
    args.get_one::<String>(key).expect("required option")
}
fn hash(args: &ArgMatches, key: &str) -> Result<BlobHash, Failure> {
    value(args, key)
        .parse()
        .map_err(|_| Failure::usage(format!("--{key} requires a full content identifier")))
}
fn absolute(args: &ArgMatches, key: &str) -> Result<PathBuf, Failure> {
    let path = PathBuf::from(value(args, key));
    if !path.is_absolute() {
        return Err(Failure::usage(format!("--{key} must be an absolute path")));
    }
    Ok(path)
}
fn same_root(a: &Path, b: &Path) -> bool {
    a == b
        || a.canonicalize()
            .ok()
            .zip(b.canonicalize().ok())
            .is_some_and(|(a, b)| a == b)
}
fn require_binding(binding: &WorkspaceBinding, root: &Path) -> Result<(), Failure> {
    if [&binding.export_root, &binding.destination]
        .into_iter()
        .flatten()
        .any(|p| same_root(Path::new(p), root))
    {
        Ok(())
    } else {
        Err(Failure::new(
            ErrorCode::Denied,
            "root is not the recorded export root or materialized destination; select it through workspace export or workspace materialize first",
        ))
    }
}
fn output(result: Value) -> Result<Output, Failure> {
    let human =
        serde_json::to_string_pretty(&result).map_err(|e| Failure::internal(e.to_string()))?;
    Ok(Output::success(result, human))
}
fn response_output(response: Response) -> Result<Output, Failure> {
    output(serde_json::to_value(response).map_err(|e| Failure::internal(e.to_string()))?)
}
fn export_json(report: &locust_workspace::ExportReport, stored: bool) -> Value {
    json!({"stored":stored,"manifest":if stored {Some(report.manifest_id)} else {None},"commit":report.commit,"files":report.files,"total_bytes":report.total_bytes,"entries":report.manifest.entries.iter().map(|e|json!({"path":e.path,"size":e.size,"executable":e.executable})).collect::<Vec<_>>(),"left_out":report.left_out,"refused":report.refused})
}
fn after_action(mut failure: Failure, context: &str) -> Failure {
    failure.message = format!("{context}: {}", failure.message);
    failure
}
fn workspace_error(error: impl Error + 'static) -> Failure {
    let mut cause: Option<&(dyn Error + 'static)> = Some(&error);
    while let Some(e) = cause {
        if let Some(f) = e.downcast_ref::<Failure>() {
            let mut failure = f.clone();
            if (&error as &dyn Error).is::<locust_workspace::MaterializeError>() {
                failure.message = error.to_string();
            }
            return failure;
        }
        cause = e
            .downcast_ref::<io::Error>()
            .and_then(|io| io.get_ref().map(|inner| inner as &(dyn Error + 'static)))
            .or_else(|| e.source());
    }
    let code = if let Some(e) = (&error as &dyn Error).downcast_ref::<ContributionError>() {
        match e {
            ContributionError::MissingObject(_) => ErrorCode::Unavailable,
            ContributionError::Conflict { .. }
            | ContributionError::HeadChanged { .. }
            | ContributionError::RecoveryRequired { .. } => ErrorCode::Conflict,
            _ => ErrorCode::Invalid,
        }
    } else {
        ErrorCode::Invalid
    };
    Failure::new(code, error.to_string())
}
struct Preview;
impl BlobSink for Preview {
    fn store(&mut self, bytes: &[u8]) -> io::Result<BlobHash> {
        Ok(content_hash(bytes))
    }
}
struct Objects<'a> {
    client: &'a mut LocalClient,
    socket: &'a Path,
    goal: GoalId,
    on_behalf: Option<PublicKey>,
}
impl Objects<'_> {
    fn call(&mut self, request: Request) -> Result<Response, Failure> {
        self.client
            .call_with(request, None, self.on_behalf)
            .map_err(|e| connection::client_error(e, self.socket))
    }
    fn status(&mut self) -> Result<GoalStatus, Failure> {
        match self.call(Request::GoalStatus { goal: self.goal })? {
            Response::GoalStatus(status) => Ok(status),
            _ => Err(Failure::internal("expected goal status")),
        }
    }
    fn bind(&mut self, binding: WorkspaceBinding) -> Result<(), Failure> {
        self.call(Request::WorkspaceSet {
            goal: self.goal,
            binding,
        })
        .map(|_| ())
    }
    fn bytes(&mut self, hash: BlobHash) -> Result<Vec<u8>, Failure> {
        match self.call(Request::BlobGet {
            goal: self.goal,
            hash,
        })? {
            Response::Blob { bytes } => Ok(bytes),
            _ => Err(Failure::internal("expected blob content")),
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
            _ => Err(io::Error::other("expected stored blob")),
        }
    }
}
impl BlobSource for Objects<'_> {
    fn fetch(&mut self, hash: &BlobHash) -> io::Result<Option<Vec<u8>>> {
        self.bytes(*hash).map(Some).map_err(io::Error::other)
    }
}
