# Versioned shared file tree implementation plan

Status: proposed implementation, 2026-10-04. The user has accepted the product
direction: a versioned shared tree, with changes visible after publication and
integration. The protocol and API design below are recommendations, not implemented
behavior. [Research and source mapping](../research/shared-file-tree-2026-10-04.md)
support this plan.

## Intended behavior

Each workspace-enabled goal has one canonical accepted file tree. Participants
can browse it, check out an exact revision, publish proposed changes, and integrate
an exact reviewed result. All participants with the same verified history derive
the same head or the same explicit dispute. A disconnected participant can have
an older view; there is no claim of instantaneous global agreement.

The shared tree is the common project state. Each agent still uses local files
for its tools, pinned to an identifiable shared revision. Publication shares a
proposal; integration advances the canonical tree; local checkout update makes
that tree available to a running agent. These are separate observable operations.

For example: Alice and Bob start at H0. Alice changes `src/parser.rs`; Bob changes
`docs/usage.md`. Alice's reviewed proposal becomes H1. Bob's proposal can be
composed onto H1, producing a new candidate containing both changes. Review and
checks bind that combined candidate before it becomes H2. Participants can then
update their local checkouts to H2. Conflicting edits remain proposals until
resolved; arrival order never silently chooses file contents.

```mermaid
flowchart LR
    H[Accepted workspace revision] --> A[Alice checkout at H]
    H --> B[Bob checkout at H]
    A --> P[Published proposals]
    B --> P
    P --> C[Compose exact candidate against current head]
    C --> R[Review and checks of candidate]
    R --> I[Authorized integration]
    I --> N[New accepted revision]
    N --> U[Explicit local checkout update]
```

## Decisions recommended for the first version

| Question | Recommendation |
| --- | --- |
| Scope | One canonical tree per workspace-enabled goal. Goals without files can omit it; ordinary findings and task outputs remain first-class contributions. |
| File model | Reuse the existing flat manifest: regular files, relative paths, bytes and executable bit. Empty directories, symlinks and submodules remain unsupported. |
| Integration ordering | One explicit formation-bound integration authority per workspace epoch, using Locust's existing single-participant `Authority` model. This is not a new limit on swarm size or parallel work. |
| Reviews | Reuse completion-rule vocabulary for exact workspace candidates. Contribution approval and task selection do not automatically advance the tree. |
| Concurrent changes | Deterministic three-way path composition; concurrent incompatible changes to a file require resolution. No automatic text merge in the first version. |
| Local updates | Explicit checkout/update operations. Automatically fetch shared objects and announce newer revisions, but do not rewrite an active checkout on event arrival. |
| Agent integration | Agent-agnostic CLI and MCP reads/proposals; native file tools operate in explicitly bound local checkouts. |
| Storage and transport | Reuse signed history, sealed blobs, typed content traversal, SQLite and Iroh. No new crate or dependency is initially justified. |

An integration authority can be a person or an agent. Its host must authorize
signing integration decisions separately from its eligibility in the formation.
When it is offline, publication and independent local work continue, while new
accepted revisions wait. Multi-writer quorum ordering would be a separate protocol
change, not a filesystem toggle.

## Ownership and state

| State | Owner and storage | Identity and meaning |
| --- | --- | --- |
| Workspace policy | Formation definition and signed rules binding | Integration authority and exact candidate completion rule; no local filesystem permission |
| Workspace proposal | Signed goal event plus sealed manifests/patch | Immutable candidate against one workspace revision, with exact source events |
| Workspace revision | Signed integration event | Event ID is the portable revision ID; links previous revision, proposal, result manifest, epoch and evidence |
| Current workspace | Pure goal projection | Distinct from `State.head`, which remains governance; never stored as an independently authoritative mutable pointer |
| Object availability | Each daemon's verified content graph | Manifest known, files pending, complete, withdrawn or invalid; not a vote on acceptance |
| Checkout | Local workspace library and local records | Opaque checkout ID, principal, canonical root, base revision/manifest, optional task/attempt association, update journal |
| Working changes | Local filesystem | Dirty, untracked and ignored paths remain local until explicitly captured |
| Agent context | Context views and adapter binding | Accepted revision, bound checkout base and available updates; reading metadata does not acknowledge file contents |

Replace the single per-principal `WorkspaceBinding` record with records keyed by
goal, principal and checkout ID. This supports multiple tasks and sessions for
one identity without overwriting its workspace metadata. Bind a session explicitly
to a checkout rather than guessing from the agent identity. Keep machine paths,
Git provenance, update journals and session bindings local.

## Protocol and authority

Names in this section are proposed contract names. Phase 1 must freeze their exact
encoding and proof semantics before callers are implemented.

### Policy and candidates

Add an optional workspace policy to `Formation` containing an explicit integrator
`Authority` and a candidate `CompletionRule`. Reuse member/role binding validation.
There is no implicit administrator fallback when the policy is absent. A workspace
setup command can construct an explicit policy using the goal creator as integrator.

Add `Scope::Workspace` for review and check evidence, with the rules binding as
the context round. Add a typed `WorkspaceProposed` subject:

```text
WorkspaceProposed {
    context: workspace scope + pinned rules binding,
    epoch: rules-binding or reconciliation-checkpoint event ID,
    parent: optional accepted workspace revision,
    base_manifest: optional sealed manifest,
    result_manifest: sealed manifest,
    patch: optional sealed canonical contribution,
    sources: exact contributing event IDs
}
```

The seed proposal has no parent/base/patch and can contain an empty manifest.
Every later proposal names a real parent, its exact manifest, and a patch whose
base/head/delta exactly match the candidate. Proposal sources are typed causal
references in the same goal, unlike today's advisory contribution citations.
Review/check/declaration validation must accept workspace proposals as exact
subjects. Existing task contribution and document semantics stay intact.

The workspace epoch is a signed authority/fencing identity, distinct from the
numeric content-encryption key epoch. Disabling workspace writes preserves its
lineage as read-only; enabling them again checkpoints that lineage instead of
silently starting a second seed tree.

For ordinary task contributions that contain workspace changes, add an optional
signed `workspace_base` revision reference and require its manifest to match the
contribution base. Generic artifacts need no workspace binding. Composition must
not guess a source revision from a bare manifest hash; an unbound external patch
must first be made into a new candidate against an explicit shared revision.

A candidate may be prepared directly from a checkout or composed from published
contributions. Referenced source events must be authenticated and admissible under
the proposal's proof; their existence is not proof that a merge is correct. A task's
selection remains a task decision. Workspace policy independently determines who
can accept code into the shared tree.

### Integration

Add a dedicated `WorkspaceAdvanced` event with the proposal, expected predecessor,
pinned workspace epoch/rules, and exact review/check proof roots. Use the proposal's
result manifest as the tree identity; avoid a second independently changeable hash.
The resulting event ID is the workspace revision.

Validate on both local authoring and replicated replay:

1. The author is the one integration authority in the authenticated active epoch.
2. The proposal belongs to this workspace context; its parent is the expected
   predecessor. The seed transition occurs only once in an epoch with no checkpoint.
3. The predecessor and its entire required ancestry are valid in this workspace
   lineage; parent IDs cannot be borrowed from another goal or task.
4. Manifests and the patch are canonical and the exact delta reproduces the result.
   Missing metadata produces pending state; malformed or wrong-goal content fails.
5. The exact candidate satisfies the pinned completion rule using compatible proof
   roots. Old reviews of source patches are not substituted for reviews of a new
   composition. Check attestations bind to this candidate, not a moving head name.
6. There is no competing eligible successor of the same predecessor in the same
   epoch. A later fork retracts the affected projection and reports a dispute.

Local integration takes `expected_head` and an idempotency key. A stale head
returns a conflict with the observed head and changes nothing. Retrying the same
request returns the same outcome. Receiving peers do not use their transient
arrival-order head to reject valid history: replay checks immutable predecessor
relationships and competing successors over the held event set.

Reuse [scope proof concepts](../crates/locust-core/src/goal/commitments.rs), but do
not force this chain into the existing task-selection stream. It must span tasks
and carry explicit checkpoints across rules changes. Do not add workspace updates
to the administrator chain; a workspace dispute must leave membership and unrelated
task collaboration usable.

### Epoch handoff and dispute recovery

Extend workspace-enabled rules rebinding with a signed workspace checkpoint. It
pins the exact accepted predecessor and evidence closure from the prior epoch,
then fences that epoch: old-epoch advancements outside the checkpoint ancestry
cannot advance the new tree, even if delivered late. The administrator must observe
and name that checkpoint explicitly; changing policy is not permission to infer
the latest tree from timestamps. Removing the current integrator stops advancement
until an authorized policy handoff is recorded.

For equivocation recovery, use an explicit administrator-authored workspace
reconciliation checkpoint, anchored to the governance chain. It names the disputed
epoch, competing known decisions, and one exact branch checkpoint or the last
uncontested ancestor. A branch checkpoint must be intrinsically valid, including
candidate bytes and review proof; reconciliation may resolve the successor dispute
but may not bless a malformed or unreviewed tree. Open a new epoch, and preserve
the other branch proposals for fresh composition. Record the chosen history and
the exclusion of other old-epoch successors visibly, including ones arriving later.

This is new protocol work. Phase 1 must model normal handoff, withheld old decisions,
reviewer forks, authority removal and administrator forks. A checkpoint must not
create globally usable authority from a proof accepted only inside that checkpoint.
If the model cannot establish these properties, revise this mechanism before
shipping workspace advancement. The current scope-conflict code is a starting
point, not proof that this extension is safe.

## Content validation and replication

Extract pure manifest/delta validation from
[workspace contribution loading](../crates/locust-workspace/src/contribution.rs)
into `locust-proto`, shared by the CLI and core. Keep file reads, Git operations,
directory mutation and command execution out of the pure evaluator.

Add a verified workspace-content lookup to core, patterned after
[definition loading](../crates/locust-core/src/node/definitions.rs). It must see
pending transaction objects as well as committed storage, refresh on object/key
arrival or withdrawal, and rebuild after restart. Extend the fold's dependency
reporting and wakeup path rather than accepting a trusted-CLI boolean.

Extend [typed content roots](../crates/locust-core/src/node/content_graph.rs) and
event blob/reference enumeration for proposal manifests, patches, revisions and
checkpoints. Authorized proposal roots must be fetchable before the proposal can
finish validation, avoiding a validation/fetch deadlock. Preserve goal membership,
epoch bounds, withdrawal and opaque-file-leaf rules. Keep source history and
referenced objects reachable for review and reconstruction; do not introduce
implicit history pruning or a revision-count limit.

Expose authority and availability separately. A fully validated metadata chain may
name a revision whose file objects are still downloading on a peer. The API must
say so, return missing-content errors for those paths, and never substitute an
older file beneath the new revision label. Invalid or unavailable metadata cannot
be presented as a validated current head. Withdrawal can make a formerly usable
revision unavailable; retain its signed history without quietly switching trees.

Retain unchanged sealed file references when possible. Where two epochs give the
same bytes different IDs, compare authenticated plaintext and mode for composition.
Do not use `Manifest::plain_digest` as a cross-epoch whole-tree equality proof.
Membership removal still rotates content keys; none of this recalls old copies.

## Composition and conflict semantics

Let B be the source base, L its proposed result, and C the current accepted tree.
Compare each path's optional file value, including bytes and executable bit:

| Condition | Result |
| --- | --- |
| L equals B | Keep C |
| C equals B | Take L |
| L equals C | Keep the common value |
| Otherwise | Report conflict; produce no integrable result |

Absence is a value, so add/add, edit/delete, delete/delete and mode changes have
defined behavior. Check the complete output for file/directory prefix conflicts
after per-path composition. Detect case/Unicode aliases during target filesystem
preflight; report incompatibility rather than renaming or dropping a file.

Compose sources in an explicit recorded order. Require the source base revision
to be an ancestor of the target within the accepted lineage, or require an explicit
new proposal against the current tree. A manifest match alone does not imply
historical ancestry. Revisions excluded during reconciliation require a fresh
proposal and review before integration.

The result is a new exact proposal against C, retaining source-event provenance.
No conflict markers or partially composed tree are promoted automatically. If the
composition changes the candidate base, prior approvals do not transfer. Even
non-overlapping files can interact semantically, so rerun required checks on the
combined candidate. The first implementation treats two different concurrent edits
to one file as a conflict, even when a text merge tool could combine them.

An unchanged result is reported as already included, not manufactured into an
empty patch merely to advance a counter. Reverting is a new proposal based on the
current revision; it does not erase history. Renames initially use deletion and
addition. A later local Git merge helper may prepare a candidate, but every replica
validates its exact resulting delta without invoking Git.

## Local checkout lifecycle

`checkout` materializes an exact complete revision into a fresh directory, then
records its local binding. No daemon request may supply an arbitrary host path
to read or write. The CLI resolves owner-approved local bindings and invokes the
workspace library. An MCP server remains a daemon client; it must not grow an
implicit filesystem executor through a new tree operation.

`status` compares managed paths to the bound base and reports newer accepted
revisions, local modifications, untracked collisions and recovery state. Capture
uses explicit paths or a named Git commit and preserves the existing export
exclusions. A directory's mere presence in a checkout does not authorize publishing
all its contents. Build outputs and credentials must not become incidental uploads.

`update` takes a target revision, checks authority/content and preflights the full
transition before mutation. Default behavior refuses managed local edits; capture
or preserve them explicitly before updating. Untracked files remain untouched, and
an incoming path collision stops the update. A successful update records the exact
new base only after the final managed-tree bytes and modes are verified.

Reuse descriptor-relative no-follow operations and recoverable application, adding
a durable journal for target revision, original binding and progress. Keep same-root
Locust updates serialized across processes/principals. A crash after files change
but before binding update must be recoverable without falsely reporting completion.
On restart, finish or recover the recorded operation explicitly; never infer success
from only the binding or Git HEAD. Recheck authority at completion and report a
locally applied but subsequently disputed revision accurately.

An advisory lock cannot stop arbitrary external editor or agent writes. Update is
an explicit quiescent-checkout operation, with repeated identity/content checks
that detect interference. It is not an atomic multi-file transaction visible to
uncooperative tools. For uninterrupted consumers, materialize a new revision into
a fresh directory and bind future runs there; keep active runs on their old path.
Do not change a running process's cwd by swapping a symlink.

## Proposed agent and user surface

These commands are proposed; they do not exist yet.

| Operation | Purpose and boundary |
| --- | --- |
| `workspace init` | Preview/import a seed tree, publish a seed proposal, then integrate through explicit policy. No files are shared merely by binding a directory. |
| `workspace head` | Return revision, manifest, authority state, content readiness and known disputes. |
| `workspace tree --revision R --path PREFIX` | Paginated path listing pinned to R; no implicit full-tree insertion into agent prompts. |
| `workspace read --revision R --path FILE` | Read exact file bytes or a typed binary reference, with explicit pagination/ranges where applicable. |
| `workspace checkout --revision R --destination DIR` | Local fresh-directory creation and binding. |
| `workspace status --checkout ID` | Local base, accepted head, dirty paths and recovery state. |
| `workspace propose --checkout ID --path PATH` | Capture explicitly selected changes and publish an immutable candidate. Repeated paths supported. |
| `workspace compose --head R --source EVENT` | Prepare a candidate against R or return structured conflicts. Multiple sources have an explicit order. |
| `workspace integrate --proposal P --expected-head R` | Author one shared transition if exact policy/evidence permits it. |
| `workspace update --checkout ID --revision R` | Explicit local update with preflight and recovery. |

CLI wrappers can orchestrate these operations, but must persist/reuse the exact
proposal and idempotency key when a multi-step result is uncertain. Do not blindly
rerun capture against a changed workdir after losing a publication response.

Expose daemon-side head/tree/read/proposal/integration operations through the typed
API registry and generated MCP schema. Local capture, checkout and update stay
CLI operations called through the agent's own authorized tools. The tree API never
treats source files as instructions from Locust and never executes repository hooks.

Add accepted head, readiness and the session's bound checkout base to compact/full
context and pending updates. Include workspace events in session news. Do not mark
files read merely because the tree was listed or a checkout was updated. Update
Codex, Claude Code, Droid and pi launch/setup guidance to use an explicit checkout
and report a stale base; no provider-specific runtime becomes mandatory.

## Implementation sequence and exit criteria

Each row is a coherent implementation stage, with scoped verified commits inside
it as needed. No stage should ship a command that implies later guarantees.

| Stage | Main work and files | Exit criterion |
| --- | --- | --- |
| 1. Freeze and model authority | Protocol proposal, workspace epoch/checkpoint rules, [organization model](../research/tla/organization.md); extend model scenarios before Rust | Delivery-order convergence, forks, exact evidence, handoff fencing and scope isolation checked; counterexample mutations fail. New claims remain bounded-model evidence. |
| 2. Pure tree operations | [manifest](../crates/locust-proto/src/manifest.rs), [contribution](../crates/locust-proto/src/contribution.rs), [workspace capture/review](../crates/locust-workspace/src/lib.rs) | Shared canonical delta validator and deterministic composition pass add/delete/mode/binary/prefix-conflict and cross-epoch cases. No filesystem dependency in protocol/core. |
| 3. Signed workspace lifecycle | [event types](../crates/locust-proto/src/event.rs), [formation contract](../crates/locust-proto/src/organization.rs), [goal evaluator](../crates/locust-core/src/goal/mod.rs), [node requests](../crates/locust-core/src/node/requests/mod.rs), [atomic commit](../crates/locust-core/src/node/commit.rs) | Seed, candidate, review, advance, epoch handoff and recovery work under replicated replay, stale writes, removal and same-author forks. Local grants are enforced without becoming replicated permissions. |
| 4. Content and restart | [content graph](../crates/locust-core/src/node/content_graph.rs), [entry](../crates/locust-core/src/node/entry.rs), [replica](../crates/locust-core/src/node/replica.rs), verified workspace-content lookup | Head metadata and transitive files recover on a second peer, including missing objects/keys, late arrival, withdrawal and restart. Hostile direct API clients cannot bypass byte validation. |
| 5. Checkout workflow | [workspace library](../crates/locust-workspace/src/lib.rs), [CLI](../crates/locust/src/cli/workspace.rs), [local bindings](../crates/locust-core/src/node/local.rs) | Multiple independent checkouts, capture, compose, update, dirty refusal, collision handling, process-race checks and interrupted-update recovery pass. |
| 6. Agent-facing completion | [typed API](../crates/locust-proto/src/api.rs), [MCP](../crates/locust/src/mcp.rs), [context](../crates/locust-proto/src/api/context.rs), [adapters](../crates/locust-adapter/src), manual and generated contracts | An agent can discover the current tree, bind a checkout, publish, review/integrate and update through documented operations without manually reconstructing hashes. |
| 7. Qualify the complete workflow | Existing [workspace CLI tests](../crates/locust/tests/workspace.rs), [graph tests](../crates/locust-core/src/node/content_graph_tests.rs), new multi-peer and real-agent scenarios | Two participants on two daemons produce one reproducible combined tree; offline/restart/conflict cases pass. Separate two-machine evidence is retained before WAN qualification claims. |

Stages 2 and 3 depend on the stage 1 contract. Content readiness and replay in stage
4 are required before local update is called safe. Stage 6 includes documentation
and removal of superseded entry points; it is not optional polish after release.

Use the existing local `select` grant for integration and `contribute` for proposal
publication; formation eligibility remains an independent check. Local directory
binding/update authorization remains host-side. If implementing this would overload
`select` ambiguously in the permission UI, resolve that during stage 1 and update
all permission surfaces together rather than silently broadening an existing grant.

## Replacement and contract cleanup

This is a greenfield replacement of the manual patch-to-local-binding workflow,
not a parallel second workspace runtime. Keep export/materialize and inert patch
formats as reusable primitives, and retain generic contributions/task selection
because they serve non-workspace collaboration.

Replace public `patch select`/`patch apply` and their `--local-choice` route with
workspace integration/update for canonical workspace work. Arbitrary local edits
remain ordinary local edits and can be proposed explicitly; there is no second
Locust-maintained local accepted-head concept. Remove the old `integrated` binding
field, obsolete binding write shape, dispatch branches and documentation. Adapt
their meaningful safety tests to the new operations. Keep `scope.select` for actual
task/document decisions and patch capture/review only where they remain used.

Bump protocol/API and formation schema versions as required by changed encodings;
regenerate frozen vectors, runtime/organization contracts, conformance fixtures,
reference docs, built-in formations and the formation editor's authoring contract.
Do not add old-format readers, migrations, fallback runtimes or compatibility flags.
Coordinate the preview release as an explicit fresh-state/protocol break; never
automatically delete a user's existing local data.

## Verification matrix

| Boundary | Required scenarios |
| --- | --- |
| Tree algebra | Disjoint edits, identical edits, different edits to one file, add/add, edit/delete, rename-as-delete/add, binary/mode changes, file/directory collisions, empty seed, no-op composition, unchanged-file reuse across epochs |
| Authority | Unauthorized advance, stale expected head, wrong predecessor/goal, wrong-context review, stale composition approvals, missing proof, authority removal, rules change, late old-epoch event, same-author fork, explicit reconciliation, governance fork |
| Replay | Permuted delivery, duplicate events, missing parent then arrival, missing metadata then arrival, invalid patch/result relation, idempotent lost response, reopen from persisted state, projection retraction |
| Content | Typed root discovery, opaque leaf preservation, nested object fetching, old/new key epochs, missing file, wrong size/hash, withdrawal and recovery, no cross-goal reads |
| Filesystem | Clean update, dirty refusal, untracked path collision, multiple checkouts for one principal, same-root concurrent update, symlink/hardlink refusal, case/Unicode alias, file/directory replacement, disk/write failure, kill before/after binding commit, interference from an external writer |
| Agent workflow | Two agent types start from H0, publish independent edits, integrate H1 then recomposed H2, read news, update explicitly, observe identical accepted files; conflicting proposals require an explicit new candidate |
| Network qualification | Two real hosts, offline publication, authority unavailable, reconnect catch-up, daemon restart during object transfer; identical revision IDs and independently checked materialized bytes/modes |

For Rust implementation commits, run `cargo fmt --all --check`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`, and
`cargo test --locked --workspace`. Add tests for new behavior at the boundaries
above. Run the existing model/conformance and generated-contract checks relevant
to changed contracts as described in [testing](testing.md). If formation-editor
sources under `sites/locust.farm/` change, run its lint, check, test and build gates.
Stage documentation and indexes before `python3 scripts/check_docs.py`.

The first usable release includes discovery, seed, proposals, integration,
replication, explicit checkout/update, conflict reporting and recovery. Recursive
Merkle directories, automatic text merging, live editing, path-level private
membership and multi-authority consensus are subsequent decisions, not promises
hidden inside this plan. Current object/manifest bounds must be reported honestly;
do not add new arbitrary agent, work, traversal or execution caps.

## Readiness

The storage and local file foundations are substantial enough to proceed without
replacing the transport. The highest-risk work is exact workspace authority across
rules changes and forks, followed by replicated content validation and crash-safe
checkout bookkeeping. Start with the stage 1 executable protocol scenarios; do not
start by adding a mutable `current_manifest` database field or a directory watcher.

This planning change requires documentation/link review only. It does not prove
that any proposed shared-tree behavior works; implementation and the verification
matrix above remain future work.
