# Git-independent shared file tree implementation plan

Status: implementation in progress, revised 2026-10-05 after source-backed review
and the [bounded workspace model](../research/tla/workspace.md).
The user has accepted the product direction: Locust owns a versioned shared tree;
agents work in ordinary directories
and publish selected changes. Git is optional at import/export boundaries. The
protocol and API design below are recommendations, not implemented behavior.
[Research and source mapping](../research/shared-file-tree-2026-10-04.md) support
this plan. The [review findings](../research/shared-file-tree-review-2026-10-05.md)
explain the simpler event model, publication path and local update rules adopted
here. Protocol extensions still require the modeling and implementation below.

The [Gas Town source assessment](../research/gastown-shared-workspace-assessment.md)
informs durable proposal, receipt and checkout recovery. Its supervisor hierarchy,
branch management and dependent-task automation are not first-version requirements.

## Intended behavior

Each workspace-enabled goal has one canonical accepted tree reference. Participants
can browse it, check out an exact revision, publish proposed changes, and integrate
an exact reviewed result. All participants with the same verified history derive
the same head or the same explicit dispute. A disconnected participant can have
an older view; there is no claim of instantaneous global agreement. Acceptance is
a signed authority decision. Content may independently be missing, invalid or
ready; an accepted reference is usable only after its files have been validated.

The shared tree is the common project state: a Locust manifest maps paths to
immutable file contents. Each concurrent worker gets an ordinary local directory,
pinned to an identifiable shared revision. A checkout requires no Git repository,
branch, index or worktree. Publication shares a proposal; integration advances the
canonical tree; local checkout update makes that tree available to a running agent.
These are separate observable operations.

The first implementation completes one loop: **shared head → checkout → propose
→ integrate → update**. Seed creation starts the loop; tree reads, status, proposal
listing and composition support it. None requires another queue service or an
agent supervisor. Updates occur explicitly at work boundaries.

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
| Local working model | One ordinary directory per concurrent worker, with an explicit checkout ID and base revision. Git-independent seed, checkout, capture and update. |
| File model | Reuse the existing flat manifest: regular files, relative paths, bytes and executable bit. Empty directories, symlinks and submodules remain unsupported. |
| Copies | Plain file copies for the first version. Reuse immutable file objects across revisions; never share writable hardlinks between workers or with the object store. |
| Integration ordering | One explicit formation-bound integration authority per workspace epoch, using Locust's existing single-participant `Authority` model. This is not a new limit on swarm size or parallel work. |
| Reviews | Reuse completion-rule vocabulary for exact workspace candidates. Contribution approval and task selection do not automatically advance the tree. |
| Protocol | One result manifest per proposal; reuse workspace-scoped `ScopeDecided` for integration and add one explicit workspace-epoch event. Authority projection does not load workspace manifests. |
| Publication | Workspace proposals are the only carrier of tree changes. Task contributions report outcomes and may cite proposals; they do not carry a second patch. |
| Concurrent changes | Deterministic three-way path composition; concurrent incompatible changes to a file require resolution. No automatic text merge in the first version. |
| Local updates | Explicit per-path update preserves compatible local edits and refuses conflicting transitions before mutation. Fetch shared objects and announce revisions without rewriting active directories. |
| Agent integration | Agent-agnostic CLI and MCP reads/proposals; native file tools operate in explicitly bound local checkouts. |
| Storage and transport | Reuse signed history, sealed blobs, typed content traversal, SQLite and Iroh. No new crate or dependency is initially justified. |
| Git boundary | Optional snapshot import or export of accepted files. No branch management, worktree creation or Git subprocess in the shared-tree loop. |

An integration authority can be a person or an agent. Its host must authorize
signing integration decisions separately from its eligibility in the formation.
When it is offline, publication and independent local work continue, while new
accepted revisions wait. Multi-writer quorum ordering would be a separate protocol
change, not a filesystem toggle.

## Ownership and state

| State | Owner and storage | Identity and meaning |
| --- | --- | --- |
| Workspace policy/epoch | Formation definition, rules binding and administrator-authored workspace-epoch event | Explicitly pinned authority/completion policy and checkpoint; no local filesystem permission |
| Workspace proposal | Signed goal event plus sealed result manifest | Immutable candidate against one workspace revision, with typed composition-source proposals |
| Workspace revision | Workspace-scoped `ScopeDecided` selection | Event ID is the revision ID; references predecessor, proposal, epoch and evidence, with tree identity derived from the proposal |
| Current workspace | Pure goal projection | Distinct from `State.head`, which remains governance; never stored as an independently authoritative mutable pointer |
| Object availability | Each daemon's verified content graph | Manifest known, files pending, complete, withdrawn or invalid; not a vote on acceptance |
| Checkout/operation records | Daemon-owned local store; CLI supplies filesystem observations | Checkout ID, principal, canonical root, base revision/manifest, optional task/attempt association, durable operation state and receipts |
| Working changes | Local filesystem, inspected/mutated by the CLI workspace library | Managed modifications/deletions are previewed for capture; new paths require explicit inclusion |
| Agent context | Context views and adapter binding | Accepted revision, bound checkout base and available updates; reading metadata does not acknowledge file contents |

Replace the single per-principal `WorkspaceBinding` record with records keyed by
goal, principal and checkout ID. This supports multiple tasks and sessions for
one identity without overwriting its workspace metadata. Bind a session explicitly
to a checkout rather than guessing from the agent identity. Keep machine paths,
optional Git provenance, operation journals and session bindings local. The daemon
owns their durable metadata in its local store, outside the managed file tree.
The CLI owns file inspection/mutation and private staging/backup files. Metadata
storage does not grant the daemon authority to execute filesystem operations.

## Git-independent starting point

The existing [workspace library](../crates/locust-workspace/src/lib.rs) already
materializes manifests into ordinary directories. Its
[selected-path capture](../crates/locust-workspace/src/capture.rs) reads explicit
files against a base manifest without Git. The current
[snapshot export](../crates/locust-workspace/src/export.rs) still starts from a
named Git commit; a Git-free seed is new work, not an existing command.

Add seed capture from an ordinary owner-selected directory and explicit relative
file paths, reusing the same safe file reads, exclusions and canonical manifest
construction. Accept repeated `--path` arguments or a UTF-8 path list through
`--paths-from FILE` / `--paths-from -` (stdin). Each line is one exact root-relative
file path; reject duplicates, empty/unsafe paths and recursive directory entries.
Supported manifest paths exclude newlines, so no shell expansion or escaping is
needed. Preview the selected paths before publication. This supports large trees
without requiring one command-line argument per file. Also support an
explicit empty seed without manufacturing a patch or requiring a commit. A
directory argument is not permission to recursively publish every file beneath it.
Exclude Git metadata and Locust's local records/journals as well as existing
credential exclusions. Generated files are shared only when deliberately selected.
The seed's input directory is capture input, not implicitly a managed checkout.
After integration, workers check out the accepted seed into fresh directories.

A revision stores a complete manifest but can retain unchanged file-object
references. Each local checkout initially writes independent regular files; this
costs local disk space and copy time proportional to the materialized tree. Do not
introduce a mount, watcher, sparse checkout, reflink backend or writable hardlinks
to make the first version work. Copy optimizations can follow measured need.

Keep named-commit import as an optional existing boundary. Exporting an accepted
revision means writing its files to an owner-selected destination; any Git commit
or push is a separate explicitly authorized operation. The source commit is
optional provenance, never the shared revision identity or the update base.

## Protocol and authority

Names in this section are proposed contract names. Stage 1 must freeze their exact
encoding and proof semantics before callers are implemented.

### Policy and candidates

Add an optional workspace policy to `Formation` containing an explicit integrator
`Authority` and a candidate `CompletionRule`. Reuse member/role binding validation.
There is no implicit administrator fallback when the policy is absent. A workspace
setup command can construct an explicit policy using the goal creator as integrator.

Add `Scope::Workspace`, with an explicit workspace-epoch event as its context
round. The epoch pins a rules binding; an unrelated `RulesBound` does not open a
new workspace epoch or retarget in-flight proposals. Add a typed proposal subject:

```text
WorkspaceProposed {
    context: workspace scope + workspace-epoch event ID,
    parent: optional accepted workspace revision,
    result_manifest: sealed manifest,
    sources: exact workspace-proposal event IDs used in composition
}
```

The seed proposal has no parent and can contain an empty manifest. Every later
proposal names a revision in this workspace lineage. The parent determines the
base manifest; the result manifest determines the complete proposed tree. Derive
the delta for review/composition, as the existing
[tree comparison](../crates/locust-workspace/src/contribution.rs) does. Do not sign
another base manifest or serialized patch alongside those identities.

Proposal sources are authenticated, typed causal references to proposals in the
same goal. They form an acyclic composition graph, separate from accepted revision
ancestry. Require its complete admissible event closure for provenance-sensitive
checks; missing sources remain pending. Sources record declared use, not proof
that an author could not have copied bytes without attribution.

Review/check/declaration validation must accept workspace proposals as exact
subjects. Review displays the seed directly, an ordinary proposal's derived diff,
or an explicitly labeled full replacement when the parent cannot be read. All
evidence binds the exact proposal and epoch; no moving-head review subject exists.

The workspace epoch is a signed authority/fencing identity, distinct from the
numeric content-encryption key epoch. Disabling workspace writes preserves its
chosen lineage as read-only; ordinary re-enabling checkpoints that lineage.

Workspace proposals are the only publication path for tree changes. Compose
proposals, not task patch contributions; do not add `workspace_base` to generic
contributions. Keep task outcomes, findings and generic artifacts as contributions.
They may cite the exact proposal/revision through their existing advisory source
references, without asserting an integration prerequisite or transferring review
evidence between scopes. Preserve task/attempt associations in checkout operation
records and the task's reporting contribution. A later automated integration
prerequisite would require its own typed, validated relation.

When workspace review policy has `exclude_author`, exclude the candidate author
and every author in its transitive composition-source graph. Do not include parent
revision ancestors: prior project authors do not become permanently ineligible.
Missing source events cannot silently shrink this exclusion set. The existing
[completion predicate](../crates/locust-core/src/goal/fold.rs) only compares the
immediate subject author; extend it for workspace subjects without changing
generic task/document author semantics. Apply it to review eligibility and
completion counting. An insufficient independent reviewer pool leaves the candidate
pending; do not weaken the rule automatically.

### Integration

Reuse `ScopeDecided { context, previous, action: Select { subject }, evidence }`
in `Scope::Workspace`. Its context round identifies the epoch, `subject` identifies
the proposal, and its event ID is the new workspace revision. The selected
proposal supplies the sole result manifest. Do not add `WorkspaceAdvanced` or a
parallel decision-proof implementation. Workspace closure actions are unnecessary;
enable/disable belongs to the explicit epoch policy.

Validate on both local authoring and replicated replay:

1. The author is the one integration authority in the authenticated active epoch.
2. The proposal belongs to this context. Within an epoch, its parent equals
   `previous`; the first decision instead uses the epoch's checkpoint. A seed is
   possible only at an explicitly unseeded boundary.
3. The predecessor, source proposals and required event ancestry are valid in
   this workspace lineage; identities cannot be borrowed from another goal.
4. The exact candidate satisfies the pinned completion rule using compatible proof
   roots. Old reviews of source proposals are not substituted for reviews of a new
   composition. Check attestations bind to this candidate, not a moving head name.
5. There is no competing eligible successor of the same predecessor in the same
   epoch. A later fork retracts the affected projection and reports a dispute.

These are authority checks over signed events and verified policy definitions.
Workspace manifest/file availability or decoding does not participate in the
authority fold. Normal authoring tools validate candidate content before requesting
integration; a hostile direct client is still constrained by event authority and
required evidence, but an authorized choice can reference unusable content.

Local integration takes the expected epoch, `expected_head` and an idempotency key.
A stale head or epoch returns a conflict with the observed head/epoch and changes
nothing. Retrying the same
request returns the same outcome. Receiving peers do not use their transient
arrival-order head to reject valid history: replay checks immutable predecessor
relationships and competing successors over the held event set.

Reuse [scope proofs](../crates/locust-core/src/goal/commitments.rs), named-authority
validation and same-stream dispute handling in the
[decision evaluator](../crates/locust-core/src/goal/fold.rs). A workspace scope spans
tasks naturally. Extend context resolution, subject/evidence validation and
projection for that scope; ordinary workspace selections stay out of the
administrator chain. A workspace dispute must leave unrelated collaboration usable.

### Epoch handoff and dispute recovery

Add one administrator-authored governance event, provisionally `WorkspaceEpoch`:

```text
WorkspaceEpoch {
    expected_epoch: optional prior workspace-epoch event ID,
    rules: exact rules-binding event ID,
    checkpoint: Unseeded | Revision(workspace revision) | RetainBefore(epoch)
}
```

Its event ID is the new workspace context round. Use the administrator chain for
ordering and compare-and-swap on the previous epoch. As with `TaskRevised`, the
[governance chain](../crates/locust-core/src/goal/chain.rs) records the position;
the work evaluator validates checkpoint and workspace semantics. Do not make
`RulesBound` depend on a work proof. A missing checkpoint/evidence leaves workspace
advancement pending, without reopening the prior epoch or blocking membership
changes. Stage 1 must define structural epoch fencing independently of checkpoint
readiness, including a later repair that names the blocked epoch.

The event pins policy explicitly. Ordinary role-binding changes do not replace
that policy; changing workspace authority requires a new epoch. Membership removal
still applies its authorization cutoffs even under pinned rules. A policy without
workspace writes disables advancement while retaining the chosen checkpoint.

Handoff and fork reconciliation use this same event. Name one exact prior-lineage
revision or the last uncontested ancestor and retain its evidence closure. Validate
the checkpoint's own authority and proof with a narrowly scoped mode that ignores
competing workspace successors; do not skip evidence, membership cutoffs, wrong-goal
checks or governance validity. This changes the current invariant that a scoped
decision cannot be selected by another scope's proof, so it requires new modeling.
It does not confer global authority on a disputed signer branch.

Old workspace successors outside the checkpoint ancestry cannot advance the new
epoch, even if revealed later. Administrator forks can still retract the governance
suffix. Preserve excluded proposals for fresh composition and show the chosen
checkpoint and excluded old history. Content validity remains separate: checkpoint
authority does not assert that the selected files are usable.

`Unseeded` explicitly retains the inherited empty starting boundary; it
does not assert that no seed exists. Permit it initially, and on later transitions
only while the immediately inherited structural boundary is empty. It
fences old decisions, including an undisclosed initial seed delivered later. This
is explicit administrator authority to discard that old seed from the current
projection. Authoring requires an explicit unseeded choice and refuses it when a
valid seed is already observed; replay must not depend on observed absence.
Ordinary handoff, disable and re-enable retain a nonempty inherited boundary.

`RetainBefore(epoch)` is a separate explicit administrator restoration. Its target
must be an ancestor of `expected_epoch`, including that epoch itself. It restores
the exact structurally inherited boundary before the named transition and fences
all intervening epochs. The target's attempted checkpoint does not participate in
the restoration proof; the restored revision and its own exact evidence do.
This permits recovery from consecutive invalid or unavailable checkpoint attempts.
Restoration may deliberately return to an earlier empty boundary. It remains
effective even if a canceled attempted checkpoint's evidence arrives later.
Tools must identify this rollback explicitly; ordinary null handoff cannot perform
it. This revises the earlier blanket prohibition on returning to empty after any
named checkpoint, which could strand the workspace behind an invalid first attempt.

The bounded model checks this choice together with delayed checkpoints,
authority/reviewer forks, removal cutoffs and administrator forks. Its evidence
does not substitute for concrete Rust replay tests or transport qualification.

## Content validation and replication

Keep canonical manifest decoding, authenticated file/hash/size checks and safe
path handling in content services and workspace tooling. Reuse the pure
[manifest validator](../crates/locust-proto/src/manifest.rs) and extracted tree/delta
operations; no file reads, Git commands or mutation enter the authority evaluator.
There is no workspace-content lookup in that fold and no signed patch to validate.

Extend [typed content roots](../crates/locust-core/src/node/content_graph.rs) for
proposal result manifests and follow their opaque file leaves. Revisions and epoch
checkpoints retain the referenced proposal/evidence history. Preserve goal/epoch
authorization and withdrawal-aware reachability; an event reference is not
permission to serve cross-goal bytes. Keep retained history and objects available
for audit/reconstruction without adding implicit pruning or a revision-count limit.

Expose content state separately from accepted/disputed authority:

| Content observation | Required behavior |
| --- | --- |
| Manifest or key missing | Report the missing dependency; a later arrival may make the same revision usable. |
| Manifest invalid/noncanonical | Mark invalid/unusable, not downloading. The immutable object cannot be repaired in place. |
| Valid manifest, files incomplete | List missing files; reads never substitute older bytes under the new revision label. |
| File authentication/hash/size invalid | Reject the affected content; do not execute checks or materialize it as a valid tree. |
| Locally withdrawn | Preserve the signed decision and report unavailable content; respect serving restrictions. |
| Complete and validated | Permit reads/checkout/check execution, subject to host permissions and filesystem compatibility. |

A selected invalid tree remains the authoritative reference until a later valid
decision replaces it; do not silently fall back. Repair uses the same proposal
type and normal policy: explicitly capture a complete replacement snapshot against
the broken parent revision, without requiring its manifest to decode. Display the
whole replacement and state that a base diff cannot be produced. Review/check that
exact result, integrate it, and retain the broken ancestor in history. A usable
head therefore does not require every historic manifest to be present or valid.

Do not copy [definition loading](../crates/locust-core/src/node/definitions.rs)'s
full-log rescan for each workspace manifest. The current content graph also
rebuilds on event changes; removing content from the fold does not remove that
cost. Add immutable decoded-manifest caching and incremental root/file indexing,
with separate recomputation of authorization on epoch, membership or withdrawal
changes. Reconstruct indexes from durable state after restart. Measure catch-up
and repeated-commit cost across growing histories/trees; this is a structural
scaling requirement, not a measured performance claim.

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

Compose source proposals in an explicit recorded order. Require the source base
revision to be an ancestor of the target within the accepted lineage, or require an explicit
new proposal against the current tree. A manifest match alone does not imply
historical ancestry. Revisions excluded during reconciliation require a fresh
proposal and review before integration.

The result is a new exact proposal against C, retaining source-event provenance.
No conflict markers or partially composed tree are promoted automatically. If the
composition changes the candidate base, prior approvals do not transfer. Accepted
head movement makes other proposals against that old head stale; their evidence
remains historical evidence for those exact candidates. Review/check the final
candidate against the integration head, with fresh evidence for recomposition. Even
non-overlapping files can interact semantically, so rerun required checks on the
combined candidate. The first implementation treats two different concurrent edits
to one file as a conflict, even when a text merge tool could combine them.

An unchanged composition is reported as already included, not manufactured into
another proposal merely to advance a counter. Reverting is a new proposal based on
the current revision; it does not erase history. Renames initially use deletion and
addition. A worker resolves conflicts in ordinary files and publishes a fresh
candidate. Review tooling derives its exact delta without invoking Git. The
authority fold verifies the decision and its evidence, not a merge algorithm.

## Local checkout lifecycle

`checkout` materializes an exact complete revision as plain file copies into a
fresh ordinary directory, then records its local binding. Each concurrent worker
uses its own directory. No daemon request may supply an arbitrary host path
to read or write. The CLI resolves owner-approved local bindings and invokes the
workspace library. An MCP server remains a daemon client; it must not grow an
implicit filesystem executor through a new tree operation.

`status` compares managed paths to the bound base and reports newer accepted
revisions, local modifications, untracked collisions and recovery state without
Git status or an index. Proposal capture defaults to all modified/deleted managed
paths; repeated `--path` or `--paths-from` adds explicitly selected files to that
default, including new files. `--only` requires a path selection and restricts
capture to exactly that selection instead. Show the chosen mode and every captured
path in the preview. Preserve
path exclusions, resolve selections against the bound root, and reject recursive
directory selection. Never use Git tracking/ignore rules to choose scope.

Freeze selected bytes and modes before presenting the candidate preview;
publication consumes that same capture, not a new scan of changing files. Previously
shared paths can acquire private contents, so managed status is not a secrecy
guarantee. Only explicitly selected new files can enter the candidate. A complete
managed-file default reduces omitted edits but cannot prove the worker tested the
captured result; checks still use an exact materialization of the candidate.

`update` verifies target authority/content and computes the whole transition before
mutation. Apply the composition table with B = checkout base, L = actual local
file and C = target: write C where L = B; preserve L where L = C or C = B; otherwise
report a conflict and mutate nothing. Compare bytes, executable mode and absence.
This lets Bob preserve his already-integrated documentation while receiving Alice's
parser change. Independent unpublished edits remain local when the target left
their base paths unchanged. No automatic text merge is involved.

Preflight the complete resulting layout, including preserved local paths, aliases,
file/directory transitions and untracked collisions. A new target path may adopt
an existing local file only when bytes/mode exactly match; report that adoption
explicitly. Other conflicting untracked paths stop the operation. Never remove
unrelated descendants. After success, set the checkout's base to C and report
remaining local differences as dirty; do not claim its directory necessarily
equals C. Publication itself does not advance the base or clear dirty state.
If the bound base cannot be read, preserve the checkout and materialize a usable
accepted target into a fresh directory instead of guessing a delta.

Reuse [descriptor-relative operations](../crates/locust-workspace/src/safe_fs.rs)
and [recoverable application](../crates/locust-workspace/src/apply.rs). Store durable
bindings, operation plans/progress and receipts in daemon-owned local records,
keyed by goal, principal, checkout and operation ID; never replicate them. The CLI
uses a private sibling recovery directory on the checkout's filesystem, outside
every managed root, for staged files and original bytes. Validate its identity and
same-device placement, deny it from capture, and refuse before mutation if it
cannot be created safely. Existing rename-based recovery must not silently become
a cross-volume operation into the daemon's home directory.

Persist the immutable recovery plan and staged replacements, then durably register
the expected binding, target, plan identity and recovery location before mutation.
The daemon stores opaque local path metadata; only the CLI opens these paths.
Preserve originals and journal progress before destructive steps. After verifying
the planned mixed result and preserved local edits, atomically update the base and
completed operation metadata in the daemon. A lost reply reconciles the same ID.
This metadata transaction is not a multi-file atomic filesystem transaction.

Serialize same-root operations across processes/principals. A crash between file
mutation and binding completion must remain recoverable. Restart distinguishes
untouched, already-written and externally changed paths; ambiguity never permits
an overwrite. Recheck authority at completion and report applied-but-disputed
revisions accurately. Retain the journal and originals until completion is known.

An advisory lock cannot stop arbitrary external editor or agent writes. Update is
an explicit quiescent-checkout operation, with repeated identity/content checks
that detect interference. It is not an atomic multi-file transaction visible to
uncooperative tools. For uninterrupted consumers, materialize a new revision into
a fresh directory and bind future runs there; keep active runs on their old path.
Do not change a running process's cwd by swapping a symlink.

## Proposed agent and user surface

These commands are proposed; they do not exist yet. The five operations above are
the product loop; the remaining commands expose its seed, inspection and review
helpers. They share the same records and do not introduce another runtime.

| Operation | Purpose and boundary |
| --- | --- |
| `workspace init --root DIR --paths-from FILE` or `workspace init --empty` | Prepare an explicit seed selection or empty manifest, publish the seed proposal and integrate through explicit policy. Repeated `--path` and stdin lists are also supported. |
| `workspace head` | Return revision, manifest, authority state, content readiness and known disputes. |
| `workspace pending` | Paginated list derived from signed proposals, decisions and content readiness, with exact IDs and blockers. No queue database or service. |
| `workspace tree --revision R --path PREFIX` | Paginated path listing pinned to R; no implicit full-tree insertion into agent prompts. |
| `workspace read --revision R --path FILE` | Read exact file bytes or a typed binary reference, with explicit pagination/ranges where applicable. |
| `workspace checkout --revision R --destination DIR` | Materialize independent regular files into a fresh directory and record its checkout ID and base. |
| `workspace status --checkout ID` | Local base, accepted head, dirty paths and recovery state. |
| `workspace propose --checkout ID` | Preview/capture changed/deleted managed files plus explicit `--path`/`--paths-from` additions. `--only` restricts capture to that selection. Publish the frozen candidate. |
| `workspace propose --replace --parent R --root DIR --paths-from FILE` | Publish a complete replacement snapshot without decoding R's manifest. An explicit empty replacement is also possible; normal evidence rules apply. This uses the same proposal type. |
| `workspace compose --head R --source P` | Prepare a candidate from typed source proposals against R or return structured conflicts. Multiple sources have an explicit order. |
| `workspace review --proposal P` | Read the exact candidate and available base diff, or label full-tree review when the base is unreadable; optional local `--destination DIR` materializes the candidate for checks. |
| `workspace integrate --proposal P --expected-head R` | Author a workspace `ScopeDecided` if policy/evidence permits. The request also pins P's epoch. Seed integration uses explicit `--expected-empty`, mutually exclusive with `--expected-head`. |
| `workspace update --checkout ID --revision R` | Explicit local update with preflight and recovery. |

CLI wrappers can orchestrate these operations, but must persist/reuse the exact
proposal and idempotency key when a multi-step result is uncertain. Do not blindly
rerun capture against a changed workdir after losing a publication response.
Preview mode returns a durable capture handle; subsequent publication uses that
handle. An explicitly requested noninteractive capture-and-publish operation must
likewise seal and submit the one frozen candidate it reports.

### Ordinary single-worker path

1. Start/claim a task under its existing rules and bind an ordinary checkout at R.
2. Edit files; capture managed changes plus selected additions as one proposal P,
   with a visible exact preview. Retain its task/attempt association locally.
3. If P's parent is stale, compose a new proposal against the accepted head and
   retain P in its source graph. Resolve conflicts explicitly when necessary.
4. Review/check the final proposal at integration time. With an unchanged parent,
   the direct proposal is already the candidate; no extra publication or composition
   review is required. Task approvals are not workspace approvals.
5. Integrate through workspace-scoped selection and retain its revision receipt.
6. Update the checkout per path. Report the task outcome in a generic contribution
   citing the proposal/revision; task completion/selection follows its own policy.

A task can report its proposal before integration when its policy allows it; that
report is not an integration receipt. This is one file-publication path, not a rule
that every task must complete only after integration.

Expose daemon-side head/tree/read/proposal/integration operations through the typed
API registry and generated MCP schema. Local capture, checkout and update stay
CLI operations called through the agent's own authorized tools. The tree API never
treats source files as instructions from Locust and never executes repository hooks.

Add accepted head, readiness and the session's bound checkout base to compact/full
context and pending updates. Include workspace events in session news. Do not mark
files read merely because the tree was listed or a checkout was updated. Update
Codex, Claude Code, Droid and pi launch/setup guidance to use an explicit checkout
and report a stale base; no provider-specific runtime becomes mandatory.

## Proposal recovery and local status

These requirements preserve work across interrupted CLI operations and agent
sessions. They are part of the five-operation workflow; automatic agent retirement,
cleanup and dependent-stage scheduling are deferred.

### Derived pending-proposal list

Project the pending-proposal list from signed proposals, required evidence,
workspace selection decisions and content availability. Return structured
observations for missing objects/keys, awaiting review/checks, stale base, content conflict,
ready, integrated and disputed. Report the exact proposal, candidate manifest,
parent revision and blockers. Persist authoritative outcomes in the existing
history; list order is advisory and cannot select a winner. Do not create a second
mutable queue with independently authoritative proposal state.

The integration authority must be able to resume this work after its session or
daemon restarts. An agent ending its attempt neither removes a pending proposal
nor makes it integrated. A conflict can become an ordinary resolution task naming
the base, candidate and affected paths, without automatically launching an agent.

Run required candidate checks in a fresh materialization of the composed manifest.
The CLI review helper reuses materialization for an unaccepted proposal; this
creates a fresh owner-selected directory without binding it as an accepted
checkout or executing commands automatically. Daemon/MCP review stays read-only
with respect to the host filesystem.
Bind attestations to that exact proposal and policy. Changes to managed inputs
during a check invalidate its applicability to the original candidate. A successful
check on each source separately is insufficient when the combined result fails.
Execution remains host-authorized, and an attestation is the named attestor's
statement, not independent proof that a command ran correctly.

### Publication and integration receipts

Before submitting a write, persist in the daemon's local operation records the
goal, principal, checkout ID/base where applicable, the exact sealed candidate and
request/idempotency key. Record its
proposal or integration receipt when known. A lost response leaves an uncertain
operation to reconcile against durable daemon state. Missing or failed receipt
writes leave publication pending or recovery needed; do not recapture changed
files or declare success because the agent session ended. Existing task/attempt
associations can be retained without requiring a new adapter handoff protocol.

Use the accepted workspace `ScopeDecided` event as the integration receipt,
rechecking its current standing. After a lost response, reconcile that exact request/receipt
before authoring a replacement. A later local update failure does not change a
successful integration into a failed one. Do not automatically close a task because
its code was integrated; task completion/selection rules remain independent.

State the durability boundary: objects and proposal stored by the local daemon do
not prove any peer holds them. Track confirmed replication separately where such
evidence exists. Keep checkout deletion and automatic reuse out of the first
release. A local receipt cannot be advertised as protection against total host/disk
loss.

### One local checkout disposition function

Use one pure disposition function for CLI status and update. Feed it known session
ownership, observed dirty/untracked work, pending journal, pending publication and
durable receipt facts. Separate observations such as dirty/untracked paths from
operation-specific blockers. Update can preserve compatible dirty paths; it blocks
on path/layout conflicts, active mutation or unresolved recovery state. Publication
uncertainty blocks recapture/replacement of that request, not every unrelated local
operation. Unknown required facts cannot authorize mutation. These states can
coexist; task completion or session absence is not proof of safe reuse.

Failed filesystem/process/daemon inspection yields unknown, not permission to
reset or re-materialize an existing directory. Resuming an attempt retains its
checkout and base. Materializing a newer revision for another run does not release
an active checkout. Keep the existing managed-adapter authority boundary:
persisted process IDs and remote events remain insufficient launch/signal authority.
Later adapter lifecycle or cleanup work must consume this same status instead of
inventing weaker reuse rules.

Checkout contents, coordination records, provider configuration and host grants
have separate owners and lifecycles. Importing/updating a tree does not install
provider configuration, change grants or overwrite Locust's coordination state.

## Deferred workflow automation

The first version lets a caller explicitly choose an accepted revision, bind a
checkout and start the next task there. Generic task completion remains separate
from integration. No new integration prerequisite or automatic stage-launch rule
is needed to deliver the shared-tree loop.

If later work adds file-dependent stage scheduling, it should use an explicit
workspace-integration prerequisite with an exact workspace-selection witness.
Pin inputs to that revision or an explicitly chosen accepted descendant containing
all required integrations; keep incompatible prerequisites pending. Ancestry
records incorporation history, not a guarantee that later edits never reverted
those files. Never silently retarget an active attempt to a newer head. This is a
follow-up formation/flow change, not a requirement of this implementation.

Also defer managed-adapter retirement/handoff automation, automatic checkout
cleanup, integration workers and scheduling, batch bisection, named branches and
copy-on-write materialization. No Gas Town supervisor hierarchy, branch manager
or separate queue service is planned. Several source proposals can already
feed one ordered, reviewed candidate without introducing those components.

## Implementation sequence and exit criteria

Each row is a coherent implementation stage, with scoped verified commits inside
it as needed. No stage should ship a command that implies later guarantees.

| Stage | Main work and files | Exit criterion |
| --- | --- | --- |
| 1. Freeze and model authority | Thin proposal, workspace `ScopeDecided`, explicit epoch/checkpoint contract and [organization model](../research/tla/organization.md) | Convergence, evidence, transitive author exclusion, scoped forks, empty-boundary transitions, pending-epoch repair, handoff fencing and governance isolation checked; counterexample mutations fail. Claims remain bounded-model evidence. |
| 2. Tree operations and Git-free seed | [manifest](../crates/locust-proto/src/manifest.rs), reusable delta/composition operations, [workspace capture/review](../crates/locust-workspace/src/lib.rs) | Canonical snapshot capture, deterministic composition and per-path update planning pass add/delete/mode/binary/prefix-conflict and cross-epoch cases. Empty/file-list seeds work without Git. This stage can run alongside stage 1 without freezing event encodings. |
| 3. Signed workspace lifecycle | [event types](../crates/locust-proto/src/event.rs), [formation contract](../crates/locust-proto/src/organization.rs), [goal evaluator](../crates/locust-core/src/goal/mod.rs), [node requests](../crates/locust-core/src/node/requests/mod.rs), [atomic commit](../crates/locust-core/src/node/commit.rs) | Proposal-only publication, reviews, selection, epoch handoff and recovery work under replay without manifest bytes. Missing policy/event evidence still blocks authority. Local grants remain independently enforced. |
| 4. Content and restart | [content graph](../crates/locust-core/src/node/content_graph.rs), [entry](../crates/locust-core/src/node/entry.rs), [replica](../crates/locust-core/src/node/replica.rs), manifest cache and incremental indexing | Authority/content states remain distinct through invalid/missing content, withdrawal and restart. Repair restores usable files without all old manifests. Readers/materializers validate bytes; growing-history catch-up/repeated-commit costs are measured. |
| 5. Ordinary-directory workflow | [workspace library](../crates/locust-workspace/src/lib.rs), [CLI](../crates/locust/src/cli/workspace.rs), [local records](../crates/locust-core/src/node/local.rs) | Copied checkouts, frozen previews, bulk/default/subset capture, review, replacement, per-path update, preserved dirty edits and collision checks work. Daemon metadata/CLI file recovery survives interruptions and same-device preflight failures. |
| 6. Agent-facing surface | [typed API](../crates/locust-proto/src/api.rs), [MCP](../crates/locust/src/mcp.rs), [context](../crates/locust-proto/src/api/context.rs), adapter launch/setup guidance, manual and generated contracts | An agent can discover the tree and pending proposals, bind a checkout, publish, review/integrate and update through documented operations without manually reconstructing hashes. No new supervisor or adapter retirement protocol. |
| 7. Qualify the complete workflow | Existing [workspace CLI tests](../crates/locust/tests/workspace.rs), [graph tests](../crates/locust-core/src/node/content_graph_tests.rs), new multi-peer and real-agent scenarios | With Git unavailable and no repositories/worktrees, two participants on two daemons reproduce the combined tree; offline/restart/conflict cases pass. Separate two-machine evidence is retained before WAN qualification claims. |

Stage 2's pure algebra and Git-free capture can proceed in parallel with stage 1;
they do not depend on event shape. Stage 3 requires the modeled stage 1 contract.
Stage 4 depends on the agreed root/reference shape, and full stage 5 operations
depend on stages 2–4. Stage 6 includes documentation and removal of superseded
entry points; it is not optional polish after release.

Stage 3 derives proposal status from signed history and evidence; stage 4 adds
content readiness. Stage 5 implements local disposition, bindings and durable
operation journals, replacing the old CLI's blanket rejection of workspace
idempotency keys. Stage 6 exposes this state and basic checkout context. Stage 7
exercises lost responses and restarts. Integration-triggered flow rules, managed
retirement and automatic cleanup are outside these stages.

Use the existing local `select` grant for integration and `contribute` for proposal
publication; formation eligibility remains an independent check. Local directory
binding/update authorization remains host-side. If implementing this would overload
`select` ambiguously in the permission UI, resolve that during stage 1 and update
all permission surfaces together rather than silently broadening an existing grant.

## Replacement and contract cleanup

Replace the manual patch-to-local-binding workflow with the one workspace runtime.
Retain snapshot export/materialization and the internal tree/delta/review/application
primitives still needed. Remove public `patch create/review/submit/select/apply`,
their `--local-choice` route, and obsolete dispatch/documentation. Remove task
contribution base/patch fields, unused serialized contribution format and its typed
content traversal once their consumers are replaced; do not keep dormant readers.
Retain generic contributions and scope selection for task/document outcomes.

Remove the old `integrated` binding field, obsolete binding write shape and
exported-root/Git-HEAD guards from the replaced workflow. There is no second
Locust-maintained local accepted head. Adapt meaningful existing safety tests to
workspace operations, including per-path matching and recovery.

Bump protocol/API and formation schema versions as required by changed encodings;
regenerate frozen vectors, runtime/organization contracts, conformance fixtures,
reference docs, built-in formations and the formation editor's authoring contract.
Do not add old-format readers, migrations, fallback runtimes or compatibility flags.
Coordinate the preview release as an explicit fresh-state/protocol break; never
automatically delete a user's existing local data.

## Verification matrix

| Boundary | Required scenarios |
| --- | --- |
| No Git dependency | Git unavailable on PATH and no `.git` directories: file/stdin-list and empty seed; default/subset/addition capture, seed/candidate review, integration and update; no Git subprocess or ignore/tracking dependency |
| Tree algebra | Disjoint edits, identical edits, different edits to one file, add/add, edit/delete, rename-as-delete/add, binary/mode changes, file/directory collisions, empty seed, no-op composition, unchanged-file reuse across epochs |
| Authority | Unauthorized selection; stale head/epoch; wrong predecessor/goal; exact evidence; direct/nested source authors excluded; historical parent authors eligible; missing source closure; authority removal; unrelated rules change leaves epoch pinned; explicit handoff/reconciliation; governance fork |
| Epoch/replay | Permuted/duplicate delivery; missing event/policy evidence then arrival; pending checkpoint then later repair; explicit unseeded handoff then late old seed; no null checkpoint after retained revision boundary; old competing successors fenced; no branch-proof authority leakage; same-author/reviewer forks; idempotent lost response; restart/retraction |
| Content | Authority agreement without manifests; invalid manifest distinguished from missing; typed roots/opaque leaves; key epochs; wrong goal/hash/size; withdrawal; full-snapshot repair without readable parent or all ancestor manifests; no invalid content use or silent fallback; cache permission invalidation and measured scaling |
| Filesystem | Shared disposition permits Bob's integrated edit plus Alice's incoming path and compatible unpublished edits; retained edits remain dirty; target-equal additions adopted explicitly; conflicting edits refuse before mutation; full-layout collision/alias/file-directory checks; multiple checkouts and same-root concurrency; symlink/hardlink refusal; external-volume recovery placement; disk/write failure; kill before/after binding commit; external writer interference |
| Agent workflow | Two agent types start from H0, publish independent edits, integrate H1 then recomposed H2, read news, update explicitly, observe identical accepted files; conflicting proposals require an explicit new candidate |
| Receipts and restart | Session exits before publication acknowledgment; lost acknowledgment after durable publish; lost integration reply; retry uses the same captured candidate after local files change; completed task with dirty or publication-unknown checkout; unavailable inspection; restart preserves exact checkout/base; successful integration followed by failed local update |
| Explicit work boundaries | A new task uses its caller-selected accepted revision; unrelated head movement cannot retarget an active checkout; source changes pass separately but fail when composed; changing one checkout cannot change another's files or stored objects |
| Capture/publication | Large path lists, malformed/duplicate paths, explicit new files, subset visible, exclusions preserved, modified managed file gains private contents, candidate unchanged after preview despite live edits, no duplicate patch publication, task citation does not count as workspace evidence |
| Network qualification | Two real hosts, offline publication, authority unavailable, reconnect catch-up, daemon restart during object transfer; identical revision IDs and independently checked materialized bytes/modes |

For Rust implementation commits, run `cargo fmt --all --check`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`, and
`cargo test --locked --workspace`. Add tests for new behavior at the boundaries
above. Run the existing model/conformance and generated-contract checks relevant
to changed contracts as described in [testing](testing.md). If formation-editor
sources under `sites/locust.farm/` change, run its lint, check, test and build gates.
Stage documentation and indexes before `python3 scripts/check_docs.py`.

The first usable release completes shared head → ordinary checkout → previewed
proposal → exact integration → explicit update, with Git-free seed creation,
discovery, replication, conflict reporting and recovery. Alongside the deferred
workflow automation above, recursive Merkle directories, automatic text merging,
live editing, path-level private membership and multi-authority consensus are
subsequent decisions. Current object/manifest bounds must be reported honestly;
do not add new arbitrary agent, work, traversal or execution caps.

## Readiness

The storage and local file foundations are substantial enough to proceed without
replacing the transport. The highest-risk work is exact checkpoint authority,
epoch fencing and proof isolation, followed by content indexing/repair and
crash-safe checkout bookkeeping. Start the stage 1 executable scenarios and
independent stage 2 tree operations together. No mutable authoritative
`current_manifest` pointer or directory watcher is needed.

This planning change requires documentation/link review only. It does not prove
that any proposed shared-tree behavior works; implementation and the verification
matrix above remain future work.
