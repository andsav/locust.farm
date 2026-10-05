//! CLI/filesystem qualification through an isolated typed Unix API fixture.
//! This does not claim signed daemon replay or peer replication qualification.
use locust_proto::API_VERSION;
use locust_proto::api::*;
use locust_proto::codec;
use locust_proto::crypto::content_hash;
use locust_proto::event::{Body, DefinitionRef, PayloadRef, RulesBinding, WorkspaceCheckpoint};
use locust_proto::id::{
    BlobHash, CheckoutId, DefinitionHash, EventId, GoalId, InstanceId, PublicKey,
    WorkspaceOperationId,
};
use locust_proto::limits::{MAX_HELLO_FRAME_BYTES, MAX_LOCAL_FRAME_BYTES};
use locust_proto::manifest::Manifest;
use locust_proto::organization::Formation;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;

const GOAL: GoalId = GoalId([0x31; 32]);
const PRINCIPAL: PublicKey = PublicKey([0x42; 32]);
const FIRST_RULES: EventId = EventId([0x53; 32]);

struct State {
    objects: HashMap<BlobHash, Vec<u8>>,
    rules: EventId,
    rules_bindings: BTreeMap<EventId, RulesBinding>,
    epoch: Option<EventId>,
    epoch_rules: Option<EventId>,
    lose_epoch_reply: bool,
    head: Option<EventId>,
    proposals: BTreeMap<EventId, WorkspaceProposalView>,
    revisions: BTreeMap<EventId, WorkspaceRevisionView>,
    operations: BTreeMap<WorkspaceOperationId, WorkspaceOperation>,
    checkouts: BTreeMap<CheckoutId, Checkout>,
    requests: Vec<RequestFrame>,
    next_event: u8,
    lose_publish_reply: bool,
    lose_complete_reply: bool,
    drop_reply: bool,
    reject_recovery_parent: bool,
    caller_session: Option<InstanceId>,
    session_active: bool,
}
impl State {
    fn new() -> Self {
        let source = serde_json::to_vec(&Formation::default()).unwrap();
        let hash = content_hash(&source);
        Self {
            objects: HashMap::from([(hash, source.clone())]),
            rules: FIRST_RULES,
            rules_bindings: BTreeMap::from([(
                FIRST_RULES,
                RulesBinding {
                    definition: DefinitionRef {
                        semantic: DefinitionHash([0x77; 32]),
                        object: PayloadRef {
                            hash,
                            len: source.len() as u32,
                            key_epoch: 0,
                        },
                    },
                    roles: BTreeMap::new(),
                    inputs: BTreeMap::new(),
                },
            )]),
            epoch: None,
            epoch_rules: None,
            lose_epoch_reply: false,
            head: None,
            proposals: BTreeMap::new(),
            revisions: BTreeMap::new(),
            operations: BTreeMap::new(),
            checkouts: BTreeMap::new(),
            requests: vec![],
            next_event: 0x80,
            lose_publish_reply: false,
            lose_complete_reply: false,
            drop_reply: false,
            reject_recovery_parent: false,
            caller_session: None,
            session_active: true,
        }
    }
    fn event(&mut self) -> EventId {
        self.next_event += 1;
        EventId([self.next_event; 32])
    }
    fn content(&self, hash: BlobHash) -> WorkspaceContent {
        let Some(bytes) = self.objects.get(&hash) else {
            return WorkspaceContent::ManifestMissing { manifest: hash };
        };
        let tree = match Manifest::decode(bytes) {
            Ok(tree) => tree,
            Err(error) => {
                return WorkspaceContent::InvalidManifest {
                    manifest: hash,
                    reason: error.to_string(),
                };
            }
        };
        let missing = tree
            .entries
            .iter()
            .filter(|entry| !self.objects.contains_key(&entry.content))
            .map(|entry| entry.content)
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            WorkspaceContent::FilesMissing {
                manifest: hash,
                missing,
            }
        } else {
            WorkspaceContent::Complete {
                files: tree.entries.len() as u64,
                bytes: tree.total_size().unwrap(),
            }
        }
    }
    fn respond(&mut self, frame: RequestFrame) -> Result<Response, ApiError> {
        self.requests.push(frame.clone());
        if !matches!(frame.request, Request::Status) {
            assert_eq!(frame.on_behalf, Some(PRINCIPAL));
        }
        match frame.request {
            Request::Status => Ok(Response::Status(DaemonStatus { daemon_version:"fixture".into(), endpoint:None,
                agents:vec![AgentView { agent:PRINCIPAL,name:"worker".into(),grants:Grants::default(),revoked:false }],
                goals:vec![GoalSummary { goal:GOAL,title:Some("workspace".into()),member:PRINCIPAL,membership:Membership::Member,halted:None }] })),
            Request::GoalStatus { goal } => Ok(Response::GoalStatus(serde_json::from_value(json!({
                "goal":goal,"title":"workspace","administrator":PRINCIPAL,"governance_head":self.rules,"current_rules":self.rules,
                "scope_halts":[],"members":[],"halted":null,"grants":GoalGrants::default(),"peers":[]
            })).unwrap())),
            Request::BlobPut { goal, bytes } => { assert_eq!(goal, GOAL); let hash = content_hash(&bytes); self.objects.insert(hash,bytes); Ok(Response::BlobStored {hash}) }
            Request::BlobGet { hash, .. } => self.objects.get(&hash).cloned().map(|bytes| Response::Blob {bytes}).ok_or_else(|| ApiError::new(ErrorCode::Unavailable,"fixture object unavailable")),
            Request::Event { event, .. } => {
                if Some(event)==self.epoch {
                    return Ok(Response::Event(Box::new(EventDetail {view:EventView{position:Some(1),event,author:PRINCIPAL,kind:"workspace_epoch".into(),at_ms:1,standing:Standing::Effective},anchor:None,body:Body::WorkspaceEpoch{expected_epoch:None,rules:self.epoch_rules.unwrap(),checkpoint:WorkspaceCheckpoint::Unseeded},payload:None,text:None,task:None,content:vec![]})));
                }
                let binding = self.rules_bindings.get(&event).ok_or_else(|| ApiError::new(ErrorCode::NotFound,"fixture event not found"))?.clone();
                Ok(Response::Event(Box::new(EventDetail { view:EventView { position:Some(1),event,author:PRINCIPAL,kind:"rules_bound".into(),at_ms:1,standing:Standing::Effective },
                    anchor:None,body:Body::RulesBound {expected:None,binding},payload:None,text:None,task:None,content:vec![] })))
            }
            Request::RulesBind { expected, formation_json, roles, inputs, .. } => {
                assert_eq!(expected,self.rules);
                let formation: Formation = serde_json::from_str(&formation_json).unwrap();
                assert!(formation.workspace.is_some());
                let bytes = formation_json.into_bytes(); let hash = content_hash(&bytes); self.objects.insert(hash,bytes.clone());
                let event = self.event(); self.rules = event;
                self.rules_bindings.insert(event,RulesBinding { definition:DefinitionRef { semantic:DefinitionHash([2;32]),object:PayloadRef { hash,len:bytes.len() as u32,key_epoch:0 } },roles,inputs });
                Ok(Response::Recorded {event})
            }
            Request::WorkspaceEpochSet { expected_epoch, rules, checkpoint, .. } => {
                assert_eq!(expected_epoch,self.epoch); assert_eq!(rules,self.rules); assert_eq!(checkpoint,WorkspaceCheckpoint::Unseeded);
                let event = self.event(); self.epoch = Some(event); self.epoch_rules = Some(rules);
                if self.lose_epoch_reply {self.lose_epoch_reply=false;self.drop_reply=true;}
                Ok(Response::Recorded {event})
            }
            Request::WorkspaceHead { .. } => {
                let head = self.head.map(|id| self.revisions[&id].clone());
                Ok(Response::Workspace(WorkspaceView { epoch:self.epoch,checkpoint:None,
                    content:head.as_ref().map(|head| self.content(head.result_manifest)),head,enabled:self.epoch.is_some(),
                    authority:if self.epoch.is_some() { WorkspaceAuthority::Ready } else { WorkspaceAuthority::Uninitialized } }))
            }
            Request::WorkspaceOperationPrepare { operation, .. } => {
                if let Some(old) = self.operations.get(&operation.id) { return Ok(Response::WorkspaceOperation(old.clone())); }
                assert_eq!(frame.idempotency,None,"phase requests use durable operation identity");
                if let WorkspaceOperationKind::Update { recovery,.. } = &operation.kind {
                    assert!(!self.checkouts.values().any(|bound| std::path::Path::new(&recovery.recovery_directory).starts_with(&bound.root)));
                    self.checkouts.get_mut(&operation.checkout.unwrap()).unwrap().active_operation = Some(operation.id);
                }
                self.operations.insert(operation.id,operation.clone()); Ok(Response::WorkspaceOperation(operation))
            }
            Request::WorkspaceOperations { .. } => Ok(Response::WorkspaceOperations(self.operations.values().cloned().collect())),
            Request::WorkspaceOperationShow { operation,.. } => self.operations.get(&operation).cloned().map(Response::WorkspaceOperation).ok_or_else(|| ApiError::new(ErrorCode::NotFound,"fixture operation not found")),
            Request::WorkspacePublish { operation,.. } => {
                let mut saved = self.operations[&operation].clone();
                if matches!(saved.state,WorkspaceOperationState::Recorded {..}) { return Ok(Response::WorkspaceOperation(saved)); }
                let WorkspaceOperationKind::Capture { candidate } = &saved.kind else { panic!("not capture") };
                assert_eq!(candidate.context.round,self.epoch.unwrap());
                let event = self.event();
                self.proposals.insert(event,WorkspaceProposalView { proposal:event,context:candidate.context,author:PRINCIPAL,parent:candidate.parent,result_manifest:candidate.result_manifest,
                    sources:candidate.sources.clone(),source_authors:vec![],standing:Standing::Effective,usable_as_source:true,integrated_as:vec![],status:WorkspaceProposalStatus::AwaitingEvidence,approved:false,evidence:vec![],stale:candidate.parent != self.head,content:self.content(candidate.result_manifest) });
                saved.state = WorkspaceOperationState::Recorded {event}; self.operations.insert(operation,saved.clone());
                if self.lose_publish_reply { self.lose_publish_reply=false; self.drop_reply=true; }
                Ok(Response::WorkspaceOperation(saved))
            }
            Request::WorkspaceProposal {proposal,..} => self.proposals.get(&proposal).cloned().map(Response::WorkspaceProposal).ok_or_else(|| ApiError::new(ErrorCode::NotFound,"fixture proposal not found")),
            Request::WorkspaceProposals {..} => Ok(Response::WorkspaceProposals(self.proposals.values().cloned().collect())),
            Request::CompletionDeclare {subject,..} => { let event=self.event(); let proposal=self.proposals.get_mut(&subject).unwrap(); proposal.approved=true;proposal.evidence.push(event);Ok(Response::Recorded {event}) }
            Request::WorkspaceIntegrate {operation,..} => {
                let mut saved=self.operations[&operation].clone();
                if matches!(saved.state,WorkspaceOperationState::Recorded {..}) { return Ok(Response::WorkspaceOperation(saved)); }
                let WorkspaceOperationKind::Integrate {expected_epoch,expected_head,proposal} = saved.kind else {panic!("not integrate")};
                if Some(expected_epoch)!=self.epoch || expected_head!=self.head { return Err(ApiError::new(ErrorCode::Conflict,"expected head changed")); }
                let proposal=self.proposals[&proposal].clone();
                if !proposal.approved { return Err(ApiError::new(ErrorCode::Conflict,"exact declaration required")); }
                let event=self.event();
                self.revisions.insert(event,WorkspaceRevisionView {revision:event,context:proposal.context,proposal:proposal.proposal,parent:expected_head,result_manifest:proposal.result_manifest,in_lineage:true});
                self.head=Some(event); saved.state=WorkspaceOperationState::Recorded {event};self.operations.insert(operation,saved.clone());Ok(Response::WorkspaceOperation(saved))
            }
            Request::WorkspaceRevision {revision,..} => self.revisions.get(&revision).cloned().map(Response::WorkspaceRevision).ok_or_else(|| ApiError::new(ErrorCode::NotFound,"fixture revision not found")),
            Request::WorkspaceTree {revision,..} => { let id=revision.or(self.head).unwrap(); let view=self.revisions[&id].clone(); let manifest=Manifest::decode(&self.objects[&view.result_manifest]).unwrap(); Ok(Response::WorkspaceTree(WorkspaceTreeView {revision:view,manifest})) }
            Request::WorkspaceRead {revision,path,..} => { let id=revision.or(self.head).unwrap(); let manifest=Manifest::decode(&self.objects[&self.revisions[&id].result_manifest]).unwrap(); let entry=manifest.entries.iter().find(|entry| entry.path==path).unwrap(); Ok(Response::WorkspaceFile(WorkspaceFileView {revision:id,path,executable:entry.executable,bytes:self.objects[&entry.content].clone()})) }
            Request::CheckoutBindSession {checkout,..} => { let bound=self.checkouts.get_mut(&checkout).unwrap(); bound.session=self.caller_session; Ok(Response::Checkout(bound.clone())) }
            Request::Session {instance} => Ok(Response::Session(SessionView {instance:instance.unwrap(),principal:PRINCIPAL,record:SessionRecord {harness:locust_proto::farm::Harness::Codex,client:"fixture".into(),state:if self.session_active {SessionState::Ready} else {SessionState::Exited},client_session:None,capabilities:SessionCapabilities::default(),detail:vec![]},updated_ms:1,attached:self.session_active,claims:vec![]})),
            Request::Checkouts {..} => Ok(Response::Checkouts(self.checkouts.values().cloned().collect())),
            Request::CheckoutRegister {checkout,..} => {self.checkouts.insert(checkout.id,checkout.clone());Ok(Response::Checkout(checkout))}
            Request::WorkspaceRecoveryParentCheck {..} => if self.reject_recovery_parent { Err(ApiError::new(ErrorCode::Denied,"fixture parent overlaps another managed tree")) } else { Ok(Response::Done) },
            Request::WorkspaceOperationComplete {operation,..} => {
                let mut saved=self.operations[&operation].clone();
                let WorkspaceOperationKind::Update {target_revision,target_manifest,..}=saved.kind else {panic!("not update")};
                let bound=self.checkouts.get_mut(&saved.checkout.unwrap()).unwrap();bound.base_revision=target_revision;bound.base_manifest=target_manifest;bound.active_operation=None;
                saved.state=WorkspaceOperationState::Completed {target_in_lineage_at_completion:true};self.operations.insert(operation,saved.clone());
                if self.lose_complete_reply {self.lose_complete_reply=false;self.drop_reply=true;}
                Ok(Response::WorkspaceOperation(saved))
            }
            other=>panic!("unexpected fixture request {other:?}"),
        }
    }
}
struct Fixture {
    home: tempfile::TempDir,
    state: Arc<Mutex<State>>,
    stopped: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}
impl Fixture {
    fn new() -> Self {
        let home = tempfile::Builder::new()
            .prefix("lc-tree-")
            .tempdir_in("/tmp")
            .unwrap();
        fs::set_permissions(home.path(), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(home.path().join("owner.credential"), [1; 32]).unwrap();
        fs::set_permissions(
            home.path().join("owner.credential"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let listener = UnixListener::bind(home.path().join("daemon.sock")).unwrap();
        let state = Arc::new(Mutex::new(State::new()));
        let stopped = Arc::new(AtomicBool::new(false));
        let server_state = state.clone();
        let server_stopped = stopped.clone();
        let handle = thread::spawn(move || {
            while !server_stopped.load(Ordering::SeqCst) {
                let (mut stream, _) = listener.accept().unwrap();
                if server_stopped.load(Ordering::SeqCst) {
                    break;
                }
                let hello = ClientHello::decode(
                    &codec::read_frame(&mut stream, MAX_HELLO_FRAME_BYTES)
                        .unwrap()
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(hello.credential, Credential([1; 32]));
                server_state.lock().unwrap().caller_session =
                    hello.session.map(|secret| secret.instance());
                let mut out = Vec::new();
                codec::encode_frame(
                    &ServerHello::Welcome {
                        api_version: API_VERSION,
                        daemon_version: "fixture".into(),
                        caller: Caller::Owner,
                        max_blob_bytes: locust_proto::limits::MAX_BLOB_BYTES as u64,
                    },
                    &mut out,
                )
                .unwrap();
                stream.write_all(&out).unwrap();
                while let Some(frame) =
                    codec::read_frame(&mut stream, MAX_LOCAL_FRAME_BYTES).unwrap()
                {
                    let request: RequestFrame = codec::decode(&frame).unwrap();
                    let id = request.id;
                    let mut state = server_state.lock().unwrap();
                    let result = state.respond(request);
                    let drop_reply = state.drop_reply;
                    state.drop_reply = false;
                    drop(state);
                    if drop_reply {
                        break;
                    }
                    out.clear();
                    codec::encode_frame(&ResponseFrame { id, result }, &mut out).unwrap();
                    stream.write_all(&out).unwrap();
                }
            }
        });
        Self {
            home,
            state,
            stopped,
            handle: Some(handle),
        }
    }
    fn cli(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_locust"));
        command
            .env_remove("LOCUST_HOME")
            .env_remove("LOCUST_CREDENTIAL")
            .env_remove("LOCUST_SESSION")
            .env("PATH", "/locust-no-programs-on-path")
            .arg("--home")
            .arg(self.home.path())
            .args(["--json", "--owner", "--as", "worker"]);
        command
    }
    fn run(&self, args: &[&str]) -> Value {
        let mut command = self.cli();
        command.args(args);
        output(command, 0)
    }
    fn seed(&self, root: &std::path::Path) -> (String, String) {
        let mut command = self.cli();
        command
            .args(["workspace", "init", "--goal", &GOAL.to_string(), "--root"])
            .arg(root)
            .args(["--path", "a", "--path", "b", "--publish"]);
        let result = output(command, 0);
        let proposal = result["result"]["operation"]["state"]["recorded"]["event"]
            .as_str()
            .unwrap()
            .to_owned();
        let revision = self.accept(&proposal);
        (proposal, revision)
    }
    fn accept(&self, proposal: &str) -> String {
        self.run(&[
            "completion",
            "declare",
            "--goal",
            &GOAL.to_string(),
            "--subject",
            proposal,
        ]);
        let result = self.run(&[
            "workspace",
            "integrate",
            "--goal",
            &GOAL.to_string(),
            "--proposal",
            proposal,
        ]);
        result["result"]["workspace_operation"]["state"]["recorded"]["event"]
            .as_str()
            .unwrap()
            .to_owned()
    }
    fn checkout(&self, destination: &std::path::Path, revision: &str) -> String {
        let mut command = self.cli();
        command
            .args([
                "workspace",
                "checkout",
                "--goal",
                &GOAL.to_string(),
                "--revision",
                revision,
                "--destination",
            ])
            .arg(destination);
        output(command, 0)["result"]["checkout"]["id"]
            .as_str()
            .unwrap()
            .to_owned()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        let _ = UnixStream::connect(self.home.path().join("daemon.sock"));
        if let Some(handle) = self.handle.take() {
            handle.join().unwrap();
        }
    }
}
fn output(mut command: Command, code: i32) -> Value {
    let out = command.output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(code),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
fn input_output(mut command: Command, text: &[u8], code: i32) -> Value {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(text).unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(code),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn file_stdin_and_empty_seeds_are_explicit_frozen_previews_without_git() {
    let fixture = Fixture::new();
    let files = tempfile::tempdir().unwrap();
    fs::write(files.path().join("selected file"), b"seed\n").unwrap();
    fs::write(files.path().join("private"), b"private").unwrap();
    let mut command = fixture.cli();
    command
        .args(["workspace", "init", "--goal", &GOAL.to_string(), "--root"])
        .arg(files.path())
        .args(["--paths-from", "-"]);
    let preview = input_output(command, b"selected file\n", 0);
    assert_eq!(preview["result"]["published"], false);
    assert_eq!(
        preview["result"]["candidate"]["captured_paths"],
        json!(["selected file"])
    );
    assert!(fixture.state.lock().unwrap().proposals.is_empty());
    assert!(fixture.state.lock().unwrap().head.is_none());
    let operation = preview["result"]["operation"]["id"].as_str().unwrap();
    fs::write(files.path().join("selected file"), b"changed after preview").unwrap();
    let published = fixture.run(&[
        "workspace",
        "publish",
        "--goal",
        &GOAL.to_string(),
        "--operation",
        operation,
    ]);
    let proposal = published["result"]["workspace_operation"]["state"]["recorded"]["event"]
        .as_str()
        .unwrap();
    let reviewed = fixture.run(&[
        "workspace",
        "review",
        "--goal",
        &GOAL.to_string(),
        "--proposal",
        proposal,
    ]);
    assert!(
        reviewed["result"]["changes"][0]["unified_diff"]
            .as_str()
            .unwrap()
            .contains("+seed\n")
    );
    let seed_rule = fixture.state.lock().unwrap().rules;
    let state = fixture.state.lock().unwrap();
    let definition = state.rules_bindings[&seed_rule].definition.object.hash;
    let formation: Formation = serde_json::from_slice(&state.objects[&definition]).unwrap();
    assert_eq!(
        formation.workspace.unwrap().completion,
        locust_proto::organization::CompletionRule::default()
    );
    drop(state);
    let empty = Fixture::new();
    let preview = empty.run(&["workspace", "init", "--goal", &GOAL.to_string(), "--empty"]);
    assert_eq!(preview["result"]["candidate"]["captured_paths"], json!([]));
}

#[test]
fn malformed_selection_leaves_policy_unmodified_and_directories_never_recurse() {
    let fixture = Fixture::new();
    let files = tempfile::tempdir().unwrap();
    fs::create_dir(files.path().join("dir")).unwrap();
    fs::write(files.path().join("dir/file"), b"bytes").unwrap();
    for paths in [
        b"dir\n".as_slice(),
        b"dir/file\ndir/file\n",
        b"\n",
        b"../outside\n",
    ] {
        let mut command = fixture.cli();
        command
            .args(["workspace", "init", "--goal", &GOAL.to_string(), "--root"])
            .arg(files.path())
            .args(["--paths-from", "-"]);
        input_output(command, paths, 6);
        assert!(fixture.state.lock().unwrap().epoch.is_none());
        assert_eq!(fixture.state.lock().unwrap().rules, FIRST_RULES);
    }
}

#[test]
fn no_git_two_checkout_capture_composition_integration_and_dirty_update_loop() {
    let fixture = Fixture::new();
    let files = tempfile::tempdir().unwrap();
    let seed = files.path().join("seed");
    fs::create_dir(&seed).unwrap();
    fs::write(seed.join("a"), b"base\n").unwrap();
    fs::write(seed.join("b"), b"base\n").unwrap();
    let (_, initial) = fixture.seed(&seed);
    let alice = files.path().join("alice");
    let bob = files.path().join("bob");
    let alice_id = fixture.checkout(&alice, &initial);
    let bob_id = fixture.checkout(&bob, &initial);
    fs::write(alice.join("a"), b"alice\n").unwrap();
    fs::write(bob.join("b"), b"bob\n").unwrap();
    fs::write(bob.join("local"), b"untracked private").unwrap();
    let first = fixture.run(&[
        "workspace",
        "propose",
        "--goal",
        &GOAL.to_string(),
        "--checkout",
        &alice_id,
        "--publish",
    ]);
    let first_proposal = first["result"]["operation"]["state"]["recorded"]["event"]
        .as_str()
        .unwrap();
    let first_revision = fixture.accept(first_proposal);
    let second = fixture.run(&[
        "workspace",
        "propose",
        "--goal",
        &GOAL.to_string(),
        "--checkout",
        &bob_id,
        "--publish",
    ]);
    let second_proposal = second["result"]["operation"]["state"]["recorded"]["event"]
        .as_str()
        .unwrap();
    // Current checkpoint proof can retain a source whose raw standing changed.
    fixture
        .state
        .lock()
        .unwrap()
        .proposals
        .get_mut(&second_proposal.parse().unwrap())
        .unwrap()
        .standing = Standing::Pending;
    let composed = fixture.run(&[
        "workspace",
        "compose",
        "--goal",
        &GOAL.to_string(),
        "--head",
        &first_revision,
        "--source",
        second_proposal,
        "--publish",
    ]);
    assert_eq!(
        composed["result"]["candidate"]["sources"],
        json!([second_proposal])
    );
    let composed_proposal = composed["result"]["operation"]["state"]["recorded"]["event"]
        .as_str()
        .unwrap();
    let final_revision = fixture.accept(composed_proposal);
    let status = fixture.run(&[
        "workspace",
        "status",
        "--goal",
        &GOAL.to_string(),
        "--checkout",
        &bob_id,
    ]);
    assert_eq!(status["result"]["dirty_paths"], json!(["b"]));
    assert_eq!(status["result"]["untracked_paths"], json!(["local"]));
    fixture.run(&[
        "workspace",
        "update",
        "--goal",
        &GOAL.to_string(),
        "--checkout",
        &bob_id,
        "--revision",
        &final_revision,
    ]);
    assert_eq!(fs::read(bob.join("a")).unwrap(), b"alice\n");
    assert_eq!(fs::read(bob.join("b")).unwrap(), b"bob\n");
    assert_eq!(fs::read(bob.join("local")).unwrap(), b"untracked private");
    let status = fixture.run(&[
        "workspace",
        "status",
        "--goal",
        &GOAL.to_string(),
        "--checkout",
        &bob_id,
    ]);
    assert_eq!(status["result"]["dirty_paths"], json!([]));
    let read = fixture.run(&[
        "workspace",
        "read",
        "--goal",
        &GOAL.to_string(),
        "--revision",
        &final_revision,
        "--path",
        "a",
    ]);
    assert_eq!(read["result"]["text"], "alice\n");
    let repeated = fixture.run(&[
        "workspace",
        "compose",
        "--goal",
        &GOAL.to_string(),
        "--head",
        &final_revision,
        "--source",
        second_proposal,
    ]);
    assert_eq!(repeated["result"]["already_included"], true);
    assert!(!alice.join(".git").exists());
    assert!(!bob.join(".git").exists());
}

#[test]
fn lost_publish_reply_reuses_same_candidate_after_live_files_change() {
    let fixture = Fixture::new();
    let files = tempfile::tempdir().unwrap();
    fs::write(files.path().join("a"), b"frozen").unwrap();
    let key = "10101010101010101010101010101010";
    fixture.state.lock().unwrap().lose_publish_reply = true;
    let mut command = fixture.cli();
    command
        .args([
            "--idempotency-key",
            key,
            "workspace",
            "init",
            "--goal",
            &GOAL.to_string(),
            "--root",
        ])
        .arg(files.path())
        .args(["--path", "a", "--publish"]);
    assert_eq!(output(command, 8)["error"]["code"], "unavailable");
    fs::write(files.path().join("a"), b"changed after uncertainty").unwrap();
    fixture.state.lock().unwrap().objects.clear();
    let mut command = fixture.cli();
    command
        .args([
            "--idempotency-key",
            key,
            "workspace",
            "init",
            "--goal",
            &GOAL.to_string(),
            "--root",
        ])
        .arg(files.path())
        .args(["--path", "a", "--publish"]);
    let recovered = output(command, 0);
    assert_eq!(recovered["result"]["resumed_frozen_capture"], true);
    assert_eq!(fixture.state.lock().unwrap().proposals.len(), 1);
    assert_eq!(recovered["result"]["preview"], Value::Null);
    assert_eq!(recovered["result"]["review_mode"], "recorded_receipt");
    let state = fixture.state.lock().unwrap();
    let capture_puts = state
        .requests
        .iter()
        .filter(|frame| matches!(frame.request, Request::BlobPut { .. }))
        .count();
    assert_eq!(
        capture_puts, 2,
        "one file and one manifest, retry never recaptures"
    );
}

#[test]
fn lost_update_completion_reply_recovers_registered_plan_and_preserves_dirty_edits() {
    let fixture = Fixture::new();
    let files = tempfile::tempdir().unwrap();
    let seed = files.path().join("seed");
    fs::create_dir(&seed).unwrap();
    fs::write(seed.join("a"), b"base").unwrap();
    fs::write(seed.join("b"), b"base").unwrap();
    let (_, initial) = fixture.seed(&seed);
    let alice = files.path().join("alice");
    let bob = files.path().join("bob");
    let alice_id = fixture.checkout(&alice, &initial);
    let bob_id = fixture.checkout(&bob, &initial);
    fs::write(alice.join("a"), b"incoming").unwrap();
    let proposed = fixture.run(&[
        "workspace",
        "propose",
        "--goal",
        &GOAL.to_string(),
        "--checkout",
        &alice_id,
        "--publish",
    ]);
    let proposal = proposed["result"]["operation"]["state"]["recorded"]["event"]
        .as_str()
        .unwrap();
    let revision = fixture.accept(proposal);
    fs::write(bob.join("b"), b"unpublished").unwrap();
    let key = "20202020202020202020202020202020";
    fixture.state.lock().unwrap().lose_complete_reply = true;
    let mut command = fixture.cli();
    command.args([
        "--idempotency-key",
        key,
        "workspace",
        "update",
        "--goal",
        &GOAL.to_string(),
        "--checkout",
        &bob_id,
        "--revision",
        &revision,
    ]);
    assert_eq!(output(command, 8)["error"]["code"], "unavailable");
    assert_eq!(fs::read(bob.join("a")).unwrap(), b"incoming");
    assert_eq!(fs::read(bob.join("b")).unwrap(), b"unpublished");
    fs::write(bob.join("a"), b"edited after durable completion").unwrap();
    fixture.run(&[
        "workspace",
        "recover",
        "--goal",
        &GOAL.to_string(),
        "--operation",
        key,
    ]);
    let status = fixture.run(&[
        "workspace",
        "status",
        "--goal",
        &GOAL.to_string(),
        "--checkout",
        &bob_id,
    ]);
    assert_eq!(status["result"]["checkout"]["base_revision"], revision);
    assert_eq!(status["result"]["dirty_paths"], json!(["a", "b"]));
    assert_eq!(
        fs::read(bob.join("a")).unwrap(),
        b"edited after durable completion"
    );
}

#[test]
fn update_refuses_conflicts_and_unsafe_recovery_parent_before_mutation() {
    let fixture = Fixture::new();
    let files = tempfile::tempdir().unwrap();
    let seed = files.path().join("seed");
    fs::create_dir(&seed).unwrap();
    fs::write(seed.join("a"), b"base").unwrap();
    fs::write(seed.join("b"), b"base").unwrap();
    let (_, initial) = fixture.seed(&seed);
    let alice = files.path().join("alice");
    let bob = files.path().join("bob");
    let alice_id = fixture.checkout(&alice, &initial);
    let bob_id = fixture.checkout(&bob, &initial);
    fs::write(alice.join("a"), b"incoming").unwrap();
    let proposed = fixture.run(&[
        "workspace",
        "propose",
        "--goal",
        &GOAL.to_string(),
        "--checkout",
        &alice_id,
        "--publish",
    ]);
    let proposal = proposed["result"]["operation"]["state"]["recorded"]["event"]
        .as_str()
        .unwrap();
    let revision = fixture.accept(proposal);
    fs::write(bob.join("a"), b"local").unwrap();
    let mut command = fixture.cli();
    command.args([
        "workspace",
        "update",
        "--goal",
        &GOAL.to_string(),
        "--checkout",
        &bob_id,
        "--revision",
        &revision,
    ]);
    output(command, 7);
    assert_eq!(fs::read(bob.join("a")).unwrap(), b"local");
    fs::write(bob.join("a"), b"base").unwrap();
    fixture.state.lock().unwrap().reject_recovery_parent = true;
    let count = fs::read_dir(files.path()).unwrap().count();
    let mut command = fixture.cli();
    command.args([
        "workspace",
        "update",
        "--goal",
        &GOAL.to_string(),
        "--checkout",
        &bob_id,
        "--revision",
        &revision,
    ]);
    output(command, 3);
    assert_eq!(fs::read_dir(files.path()).unwrap().count(), count);
    assert_eq!(fs::read(bob.join("a")).unwrap(), b"base");
}

#[test]
fn explicit_session_binding_and_shared_disposition_protect_another_active_session() {
    let fixture = Fixture::new();
    let files = tempfile::tempdir().unwrap();
    let seed = files.path().join("seed");
    fs::create_dir(&seed).unwrap();
    fs::write(seed.join("a"), b"base\n").unwrap();
    fs::write(seed.join("b"), b"base\n").unwrap();
    let (_, revision) = fixture.seed(&seed);
    let destination = files.path().join("checkout");
    let checkout = fixture.checkout(&destination, &revision);
    let session_path = fixture.home.path().join("execution.session");
    fs::write(&session_path, [8; 32]).unwrap();
    fs::set_permissions(&session_path, fs::Permissions::from_mode(0o600)).unwrap();
    let mut command = fixture.cli();
    command.arg("--session").arg(&session_path).args([
        "workspace",
        "bind",
        "--goal",
        &GOAL.to_string(),
        "--checkout",
        &checkout,
    ]);
    let bound = output(command, 0);
    assert_eq!(
        bound["result"]["checkout"]["session"],
        json!(SessionSecret([8; 32]).instance())
    );
    let args = [
        "workspace",
        "status",
        "--goal",
        &GOAL.to_string(),
        "--checkout",
        &checkout,
    ];
    let status = fixture.run(&args);
    assert_eq!(status["result"]["disposition"]["session"], "other_active");
    assert_eq!(status["result"]["disposition"]["update_allowed"], false);
    let mut command = fixture.cli();
    command.args([
        "workspace",
        "update",
        "--goal",
        &GOAL.to_string(),
        "--checkout",
        &checkout,
    ]);
    output(command, 7);
    assert!(
        fixture
            .state
            .lock()
            .unwrap()
            .operations
            .values()
            .all(|saved| !matches!(saved.kind, WorkspaceOperationKind::Update { .. }))
    );
    let mut command = fixture.cli();
    command.arg("--session").arg(&session_path).args(args);
    let own = output(command, 0);
    assert_eq!(own["result"]["disposition"]["session"], "caller");
    assert_eq!(own["result"]["disposition"]["update_allowed"], true);
    fixture.state.lock().unwrap().session_active = false;
    let inactive = fixture.run(&args);
    assert_eq!(
        inactive["result"]["disposition"]["session"],
        "other_inactive"
    );
    assert_eq!(inactive["result"]["disposition"]["update_allowed"], true);
}

#[test]
fn lost_initial_epoch_reply_accepts_only_equivalent_pinned_policy() {
    let fixture = Fixture::new();
    let files = tempfile::tempdir().unwrap();
    fs::write(files.path().join("a"), b"seed").unwrap();
    let key = "30303030303030303030303030303030";
    let criterion = r#"{"kind":"declaration","by":{"kind":"contribution_author"}}"#;
    fixture.state.lock().unwrap().lose_epoch_reply = true;
    let mut command = fixture.cli();
    command
        .args([
            "--idempotency-key",
            key,
            "workspace",
            "init",
            "--goal",
            &GOAL.to_string(),
            "--root",
        ])
        .arg(files.path())
        .args([
            "--path",
            "a",
            "--integrator",
            "worker",
            "--completion",
            criterion,
        ]);
    output(command, 8);
    assert!(fixture.state.lock().unwrap().operations.is_empty());
    let before = fixture.state.lock().unwrap().rules_bindings.len();
    let mut command = fixture.cli();
    command
        .args([
            "--idempotency-key",
            key,
            "workspace",
            "init",
            "--goal",
            &GOAL.to_string(),
            "--root",
        ])
        .arg(files.path())
        .args([
            "--path",
            "a",
            "--integrator",
            "worker",
            "--completion",
            r#"{"kind":"contribution","by":{"kind":"contribution_author"}}"#,
        ]);
    assert_eq!(output(command, 7)["error"]["code"], "conflict");
    assert!(fixture.state.lock().unwrap().operations.is_empty());
    // Semantically duplicate criteria normalize to the exact pinned policy.
    let equivalent = r#"{"kind":"all","rules":[{"kind":"declaration","by":{"kind":"contribution_author"}},{"kind":"declaration","by":{"kind":"contribution_author"}}]}"#;
    let mut command = fixture.cli();
    command
        .args([
            "--idempotency-key",
            key,
            "workspace",
            "init",
            "--goal",
            &GOAL.to_string(),
            "--root",
        ])
        .arg(files.path())
        .args([
            "--path",
            "a",
            "--integrator",
            "worker",
            "--completion",
            equivalent,
        ]);
    assert_eq!(output(command, 0)["result"]["published"], false);
    assert_eq!(fixture.state.lock().unwrap().rules_bindings.len(), before);
    assert_eq!(fixture.state.lock().unwrap().operations.len(), 1);
}
