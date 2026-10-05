# Shared file tree design review

Status: accepted design revisions, 2026-10-05; protocol and runtime work remain
unimplemented. Updates the [implementation plan](../docs/shared-file-tree-plan.md)
after a team review of Locust commit `ae9d077` and the
[initial architecture investigation](shared-file-tree-2026-10-04.md).

This review inspected source and the proposed contract. It did not run Rust tests,
TLA models or performance benchmarks. Existing implementations support the
observations below; they do not prove the new workspace protocol. In particular,
checkpoint validation and epoch fencing still require modeling before implementation.

## 1. One result identity; separate authority from usable content

Drop `base_manifest` and the serialized `patch` from workspace proposals. The
proposal's parent identifies its base, and its result manifest determines the
diff. Today's [`changes` and `load`](https://github.com/andsav/locust.farm/blob/ae9d077/crates/locust-workspace/src/contribution.rs)
already derive that diff and then compare it with the redundant stored delta.
Workspace proposals retain their context, parent, result manifest and typed source
proposal references. Review derives changes from those exact manifests.

Project selected workspace history from signed events and verified formation
definitions, without requiring every ancestor's manifest bytes. This deliberately
separates an authorized selection from content validity and usability. Report
missing objects, missing keys, withdrawal, invalid manifest, incomplete files and
usable content distinctly; never substitute an older tree under the selected
revision's identity. [`Manifest::decode`](../crates/locust-proto/src/manifest.rs),
goal/epoch authentication and [content-path permissions](../crates/locust-core/src/node/requests/content.rs)
remain mandatory at reading, verification and materialization boundaries.

An invalid immutable manifest cannot be repaired in place. Allow an explicit full
snapshot replacement against the selected parent, with normal policy evidence and
full-tree review when the parent cannot be decoded. Preserve the broken revision's
history and say that no parent/result diff was available. This is a repair workflow,
not permission to call malformed content valid.

The performance concern is structural, not measured: [`Definitions::load`](../crates/locust-core/src/node/definitions.rs)
rescans goal history from [`advance`](../crates/locust-core/src/node/commit.rs).
Do not copy that loading strategy for every large manifest. Moving manifest
validation out of the fold does not resolve all scaling work: the
[content graph](../crates/locust-core/src/node/content_graph.rs) rebuilds roots on
event/key/withdrawal changes, although blob arrivals expand incrementally. Plan
incremental indexing and immutable-object validation caching separately, preserving
permission and withdrawal invalidation.

## 2. Reuse scoped decisions; open workspace epochs explicitly

Use `ScopeDecided` with `Scope::Workspace` and `Select { subject: proposal }` for
integration. The existing [decision evaluator](../crates/locust-core/src/goal/fold.rs)
already checks named authority, same-context predecessor, successor disputes and
exact [proof closure](../crates/locust-core/src/goal/commitments.rs). Add workspace
rules relating the proposal parent to the previous decision or epoch checkpoint;
these are extensions, not already verified behavior. The selection event ID is
the workspace revision and integration receipt.

Use one administrator-authored epoch event, modeled on `TaskRevised`, with a
compare-and-swap predecessor, pinned rules binding and optional checkpoint. Its
identity is the workspace context round. Ordinary rules rebinding must not silently
replace the workspace epoch or invalidate its in-flight reviews. Membership and
authority-removal checks still apply; pinning policy does not preserve revoked
authority.

Keep work-scope proof validation out of [`Chain::build`](../crates/locust-core/src/goal/chain.rs),
whose documented boundary prevents pending work from stopping governance. Normal
handoff and dispute reconciliation can share the epoch/checkpoint mechanism.
Model checkpoint validation along its chosen predecessor chain while excluding
sibling-successor disputes, without waiving authority, evidence or other intrinsic
validation. Also model late old-epoch forks, reviewer forks, invalid checkpoints and
administrator forks. No claim that this extension is safe follows merely from
reusing the event shape.

## 3. Update by path while preserving independent local edits

Replace blanket dirty-checkout refusal with three-way path selection. For checkout
base B, local L and target T: write T when L equals B; keep L when L equals T or
T equals B; otherwise report a conflict and mutate nothing. Compare bytes and
executable mode, with absence representing deletion. This follows the useful
before/after matching in [`apply_contribution`](https://github.com/andsav/locust.farm/blob/ae9d077/crates/locust-workspace/src/apply.rs)
without introducing a text merge engine.

Preflight the complete transition, including file/directory prefix conflicts and
untracked collisions. After success, bind the checkout to T and report any surviving
local differences as dirty; the directory need not equal T exactly. Recovery must
verify the planned mixed result and retained edits before updating the binding.
Test the worked example where a participant's own edit already matches T while
another participant's independent path still needs updating.

## 4. Exclude source authors from composition review

Current [`exclude_author` evaluation](../crates/locust-core/src/goal/fold.rs) excludes
only the subject event's signer. A composed proposal could therefore let an author
approve their own included change when someone else authored the composition.

For workspace proposals, exclude the candidate author and authors of its transitive
typed source-proposal DAG. Do not traverse accepted revision ancestry: historical
contributors are not automatically authors of every subsequent candidate. Apply
the same rule during review eligibility and completion counting, and test nested
composition. This covers recorded provenance; copying changes while omitting their
source is not something source-author traversal can independently detect.

## 5. Proposals carry tree changes; task results link to them

Use workspace proposals as the sole first-version carrier of tree changes. Generic
task contributions remain useful for findings and task results, but link to an
exact proposal through a typed relationship when workspace evidence depends on it.
Today's contribution [`sources`](../crates/locust-proto/src/event.rs) are advisory
citations, explicitly not causal proof. Preserve task/attempt ownership and local
claim-generation checks from [contribution publication](../crates/locust-core/src/node/requests/claims.rs)
when reporting task work.

The ordinary path is: claim a task, edit its bound checkout, publish proposal P,
report the task result linked to P, review/check P against its exact parent, and
integrate P. Task outcome rules remain separate. The previous design did not force
two review rounds under every policy, but left two file-publication paths and an
ambiguous common workflow. Required workspace review belongs on the final composed
candidate; a changed parent requires a new proposal and applicable evidence.

Remove superseded public `patch create/review/submit/select/apply` paths and unused
serialized patch fields/formats once their consumers are replaced. Retain only
delta/review/application primitives that serve current workspace operations. Do
not add `workspace_base` to a parallel patch-bearing task contribution workflow.

## 6. Preview practical path selections

Support path lists from a file or stdin for Git-free seed capture, with preview,
explicit path interpretation and existing exclusions. Repeated command-line flags
alone are unsuitable for large directories; this review did not benchmark the
host's argument limit. Existing [workspace CLI selection](../crates/locust/src/cli/workspace.rs)
provides exact-path capture but has no bulk-list input.

Proposal capture defaults to modified/deleted managed paths shown in the preview;
new files require explicit selection. A deliberate subset remains possible and
must be visible in the preview. Previously shared paths can still acquire private
bytes, so managed status does not justify skipping exclusions or preview. Bind
checks to the published result, including when a subset differs from what a worker
tested locally.

## Ownership and implementation implications

The daemon owns durable checkout bindings and operation/receipt metadata. The CLI
owns authorized filesystem mutation and recoverable file operations; record their
operation identity and progress through the daemon rather than creating competing
authorities. Missing metadata or failed inspection must not authorize destructive
reuse. Crash cases must cover bytes changed before binding completion and committed
publication before a response is received.

Pure composition and Git-free capture work can proceed independently of protocol
modeling when their interfaces do not freeze event semantics. Epochs, checkpoints,
decision replay and evidence rules require the stage-1 contract. The plan's first
version remains a Git-independent proposal/integration/update loop; deeper adapter
lifecycle automation, scheduling and task integration prerequisites stay deferred.
