# Shared workspace implementation

Status: implemented in source, 2026-10-05. Repository gates and same-host native
daemon campaigns have passed. Two-host and release qualification are separate
checks; this document does not claim a deployed or packaged release.

The [implementation plan](shared-file-tree-plan.md) records the accepted design
and remaining qualification. The [manual](guide/apply.md) describes the commands.

## Authority and content

A `WorkspaceProposed` event identifies a complete canonical manifest, an optional
accepted parent and explicit source proposals. Workspace `ScopeDecided` events
accept exact proposals under a pinned workspace epoch. The goal evaluator derives
retained lineage, head and disputes from signed history and evidence. There is no
mutable authoritative manifest pointer. Epoch checkpoints distinguish an unseeded
boundary, a retained revision and recovery before an earlier epoch.

Authority is independent of manifest availability. Missing event or policy
evidence blocks authority; missing, invalid, withdrawn or unreadable objects block
content use. A complete replacement can repair content without decoding its
parent, but still needs normal proposal evidence and integration authority.
Transitive source authors participate in author exclusion; historical parent
authorship alone does not make a person an author of a new proposal.

The [goal evaluator](../crates/locust-core/src/goal/workspace.rs) implements replay
and authority. The [signed request handlers](../crates/locust-core/src/node/requests/workspace.rs)
enforce local grants, identity, expected epoch/head and exact revision/manifest
pairs. The [protocol types](../crates/locust-proto/src/event.rs) define the signed
contract. Generic task contributions retain text and opaque artifacts; they do
not carry serialized workspace patches.

The [content graph](../crates/locust-core/src/node/content_graph.rs) treats workspace
proposal manifests as typed roots and their file contents as opaque leaves.
Canonical decoded manifests are cached by goal, sealed object hash, epoch and
content-key fingerprint. Authenticated file sizes use the same immutable-fact
boundary, so repeated readiness checks do not reload, hash and decrypt all file
bytes. Each manifest still checks its own declared sizes, and each request
rechecks object existence, withdrawal and current reader/key authorization.
The cache never substitutes for missing stored bytes or
current read authorization. Key changes, withdrawal, membership and governance
changes invalidate the relevant reference state. New ordinary events add their
own roots without decoding or scanning all older trees. Restart reconstructs
reachability and cache from durable history and stored objects. Readiness reports
manifest missing, key missing, invalid manifest, missing files, invalid file,
withdrawal or complete content separately.

## Files and local ownership

The [tree library](../crates/locust-workspace/src/tree.rs) captures selected regular
files, composes exact candidates and plans updates from base B, local L and target
C. A default proposal captures managed modifications/deletions and explicit new
paths; a subset proposal leaves omitted changes local. Manifest entries include
file bytes and executable bits. Rename is delete plus add. Composition does not
perform text merging or choose a winner for conflicting contents. The accepted
mixed update preserves compatible local edits and unrelated ordinary files.
Local inventories stream file hashes, so a preserved untracked file need not fit
the publication object limit. Durable plans keep digests, not copies of preserved
file contents; changed files still have exact recovery copies.

The [CLI](../crates/locust/src/cli/workspace.rs) owns filesystem reads and writes.
The daemon stores inert root paths, root identities, base revision/manifest pairs,
frozen candidates and operation receipts in its durable local store. A checkout
belongs to one local principal in one goal. Root overlap checks cover bindings
and active recovery locations across goals and principals. No metadata request
authorizes the daemon to read or mutate a host path.

Checkout and review materialize into fresh ordinary directories. Git is optional
for a named-commit seed import; ordinary capture, composition, review, integration
and update do not use Git history, branches, tracking or ignore rules. Excluded
private paths are reported. Exclusion cannot establish that selected bytes contain
no secrets. Explicit task/attempt attachment records the caller's work boundary;
unrelated head movement does not update a directory automatically.

## Durable update ordering

The [transaction primitive](../crates/locust-workspace/src/transaction.rs) opens
absolute ancestors without following symlinks, inventories the full layout and
records filesystem identities. It refuses symlinks, hardlinks and unsupported
objects. A lock on the checkout directory inode serializes operations across
processes and principals, including operations using different journal folders.

Before mutation, prepare writes a durable plan, copies originals, stages replacement
files and directory objects, and validates the intended mixed layout. Recovery
lives in a private same-device sibling directory outside every managed root. A
mount whose parent cannot host same-device recovery is refused before mutation.
Prepared descriptors bind exact root/recovery paths, device/inode identities and
the plan digest. An incomplete older operation blocks fresh preparation.
An in-process preparation failure cleans only the staging directory it just
created. Missing authorization markers do not prove an older journal is
unregistered, so unknown or interrupted journals remain intact for recovery.

The CLI persists that descriptor with the expected checkout base and target in the
daemon before executing. Each destructive filesystem step has a durable intent
and completion record. Originals and removed directories move into recovery;
replacement installation uses no-replace renames. Files, records and directories
are synchronized, including macOS full-file synchronization. The entire preimage
and layout are checked repeatedly. Reopening verifies the descriptor, immutable
plan, backup bytes, phase records and known artifact identities.
Each inventory lists a directory once and uses sorted name lookups. Revalidation
reuses a file's digest only while its device, inode, size, mode, modification time
and change time match; changes force a fresh read before mutation.

After verifying the mixed result, the CLI requests one atomic daemon commit that
updates the binding and operation receipt together, then marks the filesystem
journal completed. The receipt records whether the target remained in accepted
lineage at completion; applied files are not hidden by a later authority dispute.
A lost response is reconciled through the existing operation. A durable completed
receipt remains authoritative for completion after subsequent user edits; optional
journal-marker repair verifies journal identity without rescanning those new files. Completed journals
retain recovery artifacts; no automatic cleanup policy is implemented.

These steps provide detectable, resumable per-file transitions, not multi-file
atomicity. An uncooperative editor can race a write; unknown identities, contents
or phases stop recovery and preserve the available originals and outside edits.
The library relies on its caller to persist daemon authorization before `execute`.

## Verification boundaries

The 2026-10-05 integrated source checks passed on macOS: formatting, strict workspace
Clippy, 884 Rust tests (14 explicitly ignored), a locked binary build, formation
exports/conformance, four executable guide recipes, Python helper tests, and
documentation links. The site's lint, type check, 208 tests, 24 browser tests and
build passed; the build verified 88 routes, 14 raw articles and 10 exact assets.
The workspace crate also passed Linux-target compilation and Clippy, not Linux
runtime qualification. The previously checked 54 bounded workspace model outcomes
matched their expectations. These are source checks,
not a signature or deployment qualification for a packaged release.

- [Tree tests](../crates/locust-workspace/tests/tree.rs) exercise canonical capture,
  composition, conflicts, private-path behavior and mixed per-path updates.
- Journal tests in [transaction.rs](../crates/locust-workspace/src/transaction.rs)
  exercise deterministic interruption points before/after write-ahead phases,
  file/directory transitions, descriptor and backup tampering, incomplete journals,
  cross-process locks, outside edits, and receipt-based marker repair after new
  user edits. They also check failed-preparation retry, preservation of unknown
  journals, digest-only plans and bounded directory listings. They do not simulate
  an actual power loss.
- [Content tests](../crates/locust-core/src/node/content_graph_tests.rs) exercise
  typed roots, opaque leaves, missing/invalid/withdrawn content, key and reader
  authorization, restart, unchanged-file reuse, file-validation caching and
  growing-history/refold counters.
  [Measurements](../research/shared-file-tree-content-index-2026-10-05.md) describe
  the fixture and limits.
- [Authority tests](../crates/locust-core/src/goal/workspace_tests.rs),
  [request tests](../crates/locust-core/src/node/tests/workspace.rs) and
  [lifecycle tests](../crates/locust-core/src/node/tests/workspace_lifecycle.rs)
  exercise signed state and local metadata independently of filesystem mutation.
- [CLI tests](../crates/locust/tests/workspace.rs) use an isolated typed Unix API
  fixture. They exercise commands and file behavior without proving signed daemon
  replay or network replication. The [bounded model](../research/tla/workspace.md)
  establishes only its stated model properties.
- The [local daemon loop](../research/shared-file-tree-local-loop-2026-10-05.md)
  exercises accepted seed, peer review, separate ordinary checkouts, publication,
  preserved dirty update and restart through one real SQLite daemon. A separate
  retained two-daemon direct local-Iroh smoke checks replication, peer readback
  and receipt retention. A three-daemon campaign also resumes interrupted large
  transfers from a retained replica while the original source is offline and
  checks stale integration and conflicting update refusal. These use harness
  actions and do not invoke provider models.

Real process termination during an update, disk exhaustion, actual power loss,
two real hosts, and a complete workflow run by real coding
agents remain qualification work. No WAN, packaged-runtime or causal latency-win
claim follows from the focused tests above.
