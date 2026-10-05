# Shared file tree in the current Locust architecture

Status: source-backed research, 2026-10-04. No runtime implementation or
qualification is claimed. The user chose a versioned shared tree with publication
and integration, rather than immediate synchronization of local edits.

Inspected baseline: `67c50ffbc7df01d150bd6b6e59e5e27c18b1a24f`, protocol/API 5.
The unrelated modification to `sites/locust.farm/scripts/deploy-production.sh`
was outside this investigation. The [implementation plan](../docs/shared-file-tree-plan.md)
records the proposed design; only the product direction above is accepted.

## Finding

Locust already transports immutable trees and exact changes. It lacks a canonical
workspace revision, integration authority spanning tasks, and a checkout lifecycle
that connects an agent's local files to that revision. This is primarily a change
to the collaboration state machine and workspace workflow, not a new transport.

| Current capability | Source evidence | Consequence for a shared tree |
| --- | --- | --- |
| Canonical sorted manifest of relative paths, sealed file identifiers, sizes and executable bits | [Manifest](../crates/locust-proto/src/manifest.rs), `Entry`, `Manifest::check` | Reuse the format initially. It is a flat path map, not a recursive directory Merkle tree. Directories are implied by file paths. |
| Goal- and epoch-specific encrypted objects with deterministic sealing | [Sealing](../crates/locust-proto/src/seal.rs) | Reuse object transfer and deduplication. Equal plaintext can have different object identifiers across key epochs. |
| Committed snapshots and capture of explicit local paths without requiring Git | [Export](../crates/locust-workspace/src/export.rs), [capture](https://github.com/andsav/locust.farm/blob/67c50ffbc7df01d150bd6b6e59e5e27c18b1a24f/crates/locust-workspace/src/capture.rs) | Agents can already exchange code from Git or ordinary materialized directories. No new Git hosting service is needed. |
| Exact base/head manifests plus before/after entries | [Contribution format](https://github.com/andsav/locust.farm/blob/67c50ffbc7df01d150bd6b6e59e5e27c18b1a24f/crates/locust-proto/src/contribution.rs), [delta validation](https://github.com/andsav/locust.farm/blob/67c50ffbc7df01d150bd6b6e59e5e27c18b1a24f/crates/locust-workspace/src/contribution.rs) | Reuse inert changes, including additions, deletions, binary replacement and executable-bit changes. |
| Fresh-directory materialization with no-follow path handling and no-replace publication | [Materializer](../crates/locust-workspace/src/materialize.rs), [tests](../crates/locust-workspace/tests/materialize.rs) | Good foundation for reproducible revision checkouts. This does not provide atomic multi-file updates to an existing live directory. |
| Recoverable application with exact affected-file checks | [Apply](https://github.com/andsav/locust.farm/blob/67c50ffbc7df01d150bd6b6e59e5e27c18b1a24f/crates/locust-workspace/src/apply.rs), [tests](https://github.com/andsav/locust.farm/blob/67c50ffbc7df01d150bd6b6e59e5e27c18b1a24f/crates/locust-workspace/tests/contribution.rs) | Preserve this protection when adding checkout update. Current application can retain unrelated local changes. |
| Per-principal local workspace metadata | [WorkspaceBinding](../crates/locust-proto/src/api.rs), [local records](../crates/locust-core/src/node/local.rs) | `exported` and `integrated` identify artifacts, not a shared accepted revision. One binding per principal/goal cannot represent concurrent independent checkouts well. |
| Signed scoped decisions, exact review evidence, conflict detection | [Decision validation](../crates/locust-core/src/goal/fold.rs), [proof closure](../crates/locust-core/src/goal/commitments.rs) | Reuse proof concepts, but current selection chains are keyed by scope and rules/task round. They are not a goal-wide tree history. |
| Typed graph traversal for task manifests and patch base/head manifests | [Content graph](../crates/locust-core/src/node/content_graph.rs), [graph tests](../crates/locust-core/src/node/content_graph_tests.rs) | Add explicit workspace roots. An arbitrary hash in `artifacts` is opaque and does not automatically authorize traversal as a tree. |
| Pure state evaluation separated from filesystem code | [Goal](../crates/locust-core/src/goal/mod.rs), [crate boundaries](../docs/crates.md) | Keep deterministic tree validation in protocol/core and local file I/O in the CLI workspace library. |
| Session-specific coherent context and explicit read acknowledgment | [Context contract](../crates/locust-proto/src/api/context.rs), [context implementation](../crates/locust-core/src/node/context.rs) | Surface the accepted revision and checkout base without claiming an agent has read every file. |

## Architectural issues that an implementation must resolve

### A selected result is not a composited workspace

[Scope selection](../crates/locust-core/src/node/requests/tasks.rs) selects an exact
contribution or document revision in its context. Two tasks can legitimately select
two different complete head manifests derived from the same base. Choosing the
last manifest would discard the other task's changes. The workspace must integrate
the deltas into an explicitly reviewed result.

`State.head` is the governance head; its documentation explicitly says it is never
an accepted workspace head in [state.rs](../crates/locust-core/src/goal/state.rs).
Do not repurpose it. Likewise, the per-daemon context revision is a local read
cursor, not a portable workspace revision.

### Existing review rules do not validate a patch's contents in the pure fold

The [goal evaluator](../crates/locust-core/src/goal/mod.rs) receives authenticated
events and a `DefinitionLookup`. Exact base/head/delta equality is checked by
`locust-workspace::load` when reviewing/applying a patch. The existing
[content graph test](../crates/locust-core/src/node/content_graph_tests.rs) also
distinguishes content reachability from contribution validation.

It would be unsafe to add a head pointer that trusts only CLI validation. Every
receiving daemon needs independently verified candidate manifests and deltas,
including candidates authored by a custom or hostile client. Missing content must
wait, rather than be treated as empty or valid. Build a verified content lookup
along the lines of [definition loading](../crates/locust-core/src/node/definitions.rs),
then feed that data into the pure evaluator.

### Compare-and-swap is local admission, not distributed consensus

Current formations bind each decision authority to one participant;
[binding validation](../crates/locust-core/src/goal/chain.rs) rejects multi-member
authority roles. [Decision evaluation](../crates/locust-core/src/goal/fold.rs)
detects multiple authenticated successors instead of choosing by timestamp.

Use one formation-bound integration authority for the first shared-tree version.
This still allows parallel participants and offline proposals. It trades integration
availability for a simple ordered accepted tree when that authority is offline.
Two disconnected copies of its signing identity can still equivocate: local
expected-head checks alone do not prevent this. A late fork can retract an earlier
view. Expose dispute and recovery explicitly; do not describe local success as
irreversible distributed finality.

### Rules changes must carry the workspace forward

Current decision predecessor validation requires the same exact context. A new
`RulesBound` changes the goal/document context. Adding a workspace selection scope
without an explicit handoff would create unrelated head chains after a policy
change. Authority rotation needs a governance-anchored workspace checkpoint and
fencing of the old integration epoch. The plan treats this as a protocol gate.

### Stored identity and byte equality differ

`Manifest::plain_digest` hashes the manifest encoding, whose entries themselves
contain sealed object identifiers. Despite its broad comparison wording, it is
not a general cross-epoch digest of the entire plaintext file tree if descendants
have been resealed. [Capture](https://github.com/andsav/locust.farm/blob/67c50ffbc7df01d150bd6b6e59e5e27c18b1a24f/crates/locust-workspace/src/capture.rs) already
retains old file identifiers when the bytes are unchanged.

Preserve those identifiers where valid. For three-way composition across epochs,
compare verified plaintext bytes and executable modes when identifiers differ.
Do not publish plaintext content hashes in unencrypted event headers merely to
make this comparison easier. Fix the misleading digest comment with implementation.

### Local application is not shared acceptance

[The CLI](../crates/locust/src/cli/workspace.rs) separately checks selection,
applies files, rechecks authority and records a local binding. Its `--local-choice`
path intentionally has no distributed selection effect. That is useful local
behavior, but it cannot become a route for advancing the shared tree.

The new workflow must distinguish accepted head, locally available objects,
checkout base, dirty files, and interrupted updates. Background blob fetching is
compatible with this model; rewriting active workdirs on every incoming event is
not part of the chosen product direction.

## Public source comparison

Sources were opened on 2026-10-04. These are design references, not dependencies
installed or benchmarked in this investigation.

| Approach | Primary-source behavior | Fit for Locust |
| --- | --- | --- |
| Immutable versioned tree | Git separates file blobs, named trees and commits linking trees to history. [Git objects](https://git-scm.com/book/en/v2/Git-Internals-Git-Objects) | Adopt the separation conceptually. Locust already has its own manifests, encrypted objects, signed history and membership. Git commit identifiers need not become protocol authority. |
| Three-way merge as a local helper | `git merge-tree --write-tree` computes a merge without changing the working tree or index. It reports content, mode, rename and directory conflicts; an output tree alone is not proof of success. [Git merge-tree](https://git-scm.com/docs/git-merge-tree) | Potential later helper for preparing proposals. Do not make the replicated validator depend on installed Git versions or custom merge drivers. First implement deterministic path-level composition. |
| Live folder synchronization | Syncthing scans/watches files and distributes changes. Concurrent modifications can produce propagated conflict copies; path case differences require explicit handling. [Syncthing synchronization](https://docs.syncthing.net/users/syncing.html) | Useful evidence about filesystem hazards, but its conflict-copy semantics do not express reviewed multi-file integration. Do not put a live folder synchronizer underneath the canonical tree. |
| Collaborative CRDT documents | Automerge retains concurrent property values while presenting a deterministic winner; applications can inspect conflicts. [Automerge conflicts](https://automerge.org/docs/reference/documents/conflicts/) | Suitable to investigate if collaborative editing becomes a requirement. Convergence alone does not establish that a code change was reviewed or builds. It would add a second document model for the present task. |

Recommendation: reuse Locust's manifests and object network; add explicit workspace
revisions, integration proposals and authority; keep ordinary local files for
agent tools. No FUSE mount, CRDT engine, Git server or new transport is needed for
the first implementation.

## Evidence limits and next measurements

This investigation read current source and existing test definitions. It did not
run Rust tests, a network experiment, or a shared-tree prototype. Existing tests
are relevant regression coverage, not evidence for the proposed behavior.

Before optimizing storage, measure manifest encoding size, cold and warm tree
listing, changed-byte transfer, composition cost, replay after restart, and
checkout update with representative repository trees. The existing flat manifest
rewrites metadata per revision but reuses unchanged file objects. Introduce
recursive tree objects or chunked file storage only if these measurements justify
them. Preserve explicit pagination and report current format limits; do not add
arbitrary participant, iteration or execution-time caps.
