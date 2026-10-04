# Lane A implementation review — 2026-10-03

**Status: review findings and proposed takeover sequence, not an accepted change
of lane ownership or release qualification.** Requested by the owner after the
second transport review. The architecture is suitable to continue; the current
implementation is not yet a working T1 daemon.

## Scope and evidence

Reviewed committed HEAD `bc4c25f2c2da44df74e288eafbe8ed76687ca409` separately from
the working files captured at `2026-10-04T00:51:17.900431Z` (October 3 locally).
Three independent reviewers covered authority, storage/workspace, and sync; the
orchestrating reviewer covered daemon integration and checked their findings.
All experiments used isolated scratch copies. Lane A's source was not changed.

The [capture manifest](evidence/lane-a-review-snapshot.json) records SHA-256 hashes
of the working sources. A second comparison found no changes to any captured
file. WIP-only findings below must not be described as bugs in a released or
already integrated execution path. The base commit alone does not contain those
uncommitted modules. The [reproduction appendix](evidence/lane-a-review-probes.md)
preserves test source, setup changes, commands, and observed results; reproducing
the WIP findings requires the matching sources identified by the manifest.

| Layer | Implemented and tested at review | Still missing |
|---|---|---|
| Protocol | Signed/canonical events, sealed objects, API/store/engine contracts; 155 tests | Enforcement of the complete contract by the running daemon |
| Storage/workspace | SQLite and object storage, shared store conformance, crash tests, export and materialization | Findings B-R5 and B-R14 below; patches explicitly remain unimplemented |
| Node | Opening, identity/credentials, enrollment, status and stop; 17 tests | Goal lifecycle, sessions/claims and replay exercised through public operations |
| Goal | WIP history, fold, chain and transition implementations | [Goal](../crates/locust-core/src/goal/mod.rs) still has `todo!()` entry points; rule modules are not declared and compiled |
| Sync | WIP initiator/responder/driver and synthetic replica tests | Production Host/Replica, join/key/content persistence, [Node peer wiring](../crates/locust-core/src/node/peers.rs) |
| Daemon | WIP local framing, connection/wait/stop shell; 39 tests | Daemon constructor (`crates/locust/src/daemon/mod.rs`) uses `MinimalEngine`; real Node/store, network shell and complete CLI flow |

## Findings

Priorities describe the consequence once the affected feature is used. Several
features, including member removal and large-object transfer, are outside T1.

### B-R5 — P1: an export root beneath a denied directory bypasses exclusions

**Committed defect; reproduced.** [Export selection](../crates/locust-workspace/src/export.rs)
passes already-rebased paths to [the deny rules](../crates/locust-workspace/src/select.rs).
Exporting the repository root excludes fake `.aws/credentials`, but exporting
the `.aws` directory uploads `credentials` and `nested/credentials`. Exporting
`.aws/nested` uploads `credentials`, and exporting `.kube` uploads `config`.
Every affected report has an empty `left_out` list. This defeats the documented
directory exclusions for precisely the files they are intended to catch.

Preserve repository-relative ancestry for classification, while keeping manifest
paths relative to the requested root, or refuse roots beneath denied directories.
The appendix's desired-behavior regression fails in all three cases. It uses only
fake credentials in temporary repositories.

### B-R6 — P1: the waiting limit can suppress the evidence that halts authority

**Unwired WIP rule; reproduced in the scratch harness.**
Screening (`crates/locust-core/src/goal/screen.rs`) rejects a fork variant when
1,024 events already wait beyond a gap. Hold genesis and self-admission, omit
sequence 2, retain sequences 3–1026, then deliver another validly signed event at
sequence 1: the conflicting event is dropped and the chain remains unhalted.
The [contract](../docs/protocol-v0.md) requires retaining both variants and halting
conflicting decisions. Repeated reconciliation cannot fix this while the waiting
set remains full.

Make first fork evidence admissible independently of waiting capacity, and test
both delivery orders. Resource limits must not silently override the safety rule.

### B-R7 — P1: a removed member's open assignment can still finalize

**Unwired WIP rule; reproduced through the fold.** A worker submits within its
removal cutoff; the coordinator removes it and subsequently accepts that result.
Removal (`crates/locust-core/src/goal/transition.rs`) deletes membership, while
result decision eligibility checks cancellation/current assignment but no
revocation. The resulting task is accepted even though the member is removed.
The [contract](../docs/protocol-v0.md) explicitly prevents removed members' open
assignments from finalizing.

Preserve historical submissions while invalidating finalization for affected
assignments. A mere current-membership check also needs a remove/readmit test so
readmission does not resurrect an old assignment. Resolve before enabling removal;
removal is outside T1.

### B-R8 — P2: progress after submission hides undecided work from review

**Unwired WIP rule; fold reproduced, public API consequence source-backed.**
Assignment acceptance → result submission → progress changes the task from
`Submitted` back to `Taken`, retaining the undecided result. The
claim-bound request check (`crates/locust-core/src/node/requests/claims.rs`)
allows that write, the transition (`crates/locust-core/src/goal/transition.rs`)
changes the state, and pending review (`crates/locust-core/src/node/views.rs`)
includes only submitted tasks. The coordinator loses automatic discovery of work
awaiting its decision. The contract gives progress no state effect.

Keep progress from changing submission state, or reject it when the attempt is no
longer taken. Add the regression through public API operations after wiring Goal.

### B-R9 — P2: duplicate admission silently rebinds an active principal

**Unwired WIP rule; reproduced through the fold.** Self-admission followed by a
second admission for the same active principal makes the second event effective
and changes its endpoint. Chain validation (`crates/locust-core/src/goal/chain.rs`)
and admission (`crates/locust-core/src/goal/transition.rs`) omit the contract's
“principal is not a member” precondition. Enforce removal before readmission, or
explicitly change the accepted contract before implementing endpoint migration.

### B-R10 — P2: sync completion is recorded before final delivery

**WIP machine defect; reproduced with a synthetic host.**
[Driver advancement](../crates/locust-core/src/sync/driver.rs) emits `Finish` and
immediately removes the exchange, clears backoff, frees its pair and reports
`Completed`. No shell delivery confirmation has arrived. A failed final write or
acknowledgment therefore cannot correct that outcome; subsequent `Closed` is
ignored. The [engine seam](../crates/locust-proto/src/engine.rs) also conflates
acknowledged finish and transport failure in one `Closed` input.

Keep a finishing state and report success after the shell confirms delivery.
Give successful finish and failed/aborted closure distinguishable outcomes. This
is relevant to T1 status, join completion and backoff, not merely logging polish.

### B-R11 — P2: responses collect complete transfers before backpressure applies

**WIP source finding; allocation/latency impact not benchmarked.**
[Blob serving](../crates/locust-core/src/sync/responder.rs) reads every chunk into
an output vector before returning, up to the 64 MiB object limit. Frontier
responses and [initiator pushes](../crates/locust-core/src/sync/initiator.rs) also
collect an entire missing suffix. Per-frame limits do not bound these vectors or
the synchronous engine work, contrary to [the module's stated backpressure
property](../crates/locust-core/src/sync/mod.rs).

Emit incrementally using a cursor and transport capacity. Large blobs are outside
T1, but event histories can grow during ordinary operation. Do not substitute an
unreported truncation cap for complete reconciliation.

### B-R12 — P2: admission can become stale during an exchange

**WIP machine defect; reproduced with a synthetic host.** After `Hello` admits an
endpoint, removing its membership still lets it request events, inventories and
ciphertext. [The responder](../crates/locust-core/src/sync/responder.rs) rechecks
current membership only for `KeyRequest`. The probe receives `Events` where it
expects `NotAMember`. This does not demonstrate disclosure of a new epoch's
plaintext: the key request is rechecked.

Recheck authority for requests or close exchanges when their admission becomes
invalid. Fix before member removal is enabled; that feature is outside T1.

### B-R13 — P2: bounded object selection has no fairness contract

**WIP integration hazard; reproduced with the supplied test replica.**
[The initiator](../crates/locust-core/src/sync/initiator.rs) requests up to 256
wanted objects afresh each exchange, with no cursor or unavailable-object
feedback. In the probe, 256 unavailable objects occupy every slot for five
anti-entropy periods, and a later available object is never requested.

Production Replica is not implemented, so this is not evidence of an actual-node
failure. Define fair progress across exchanges at that seam and test mixed
availability. A per-exchange work budget must not permanently starve later work.

### B-R14 — P2: first-start directory durability is incomplete

**Source-backed durability gap; no power-loss experiment performed.**
[Store directory creation](../crates/locust-store/src/files.rs) recursively creates
the state and blob directories without syncing each new directory entry in its
parent. Later file/database syncs do not establish durability of the new state
directory's entry in its own parent. The WIP credential creator (`crates/locust/src/secret.rs`)
likewise syncs a staging inode, installs its final hard link, and returns without
syncing that containing directory. These are narrower guarantees than the
unconditional power-loss language in [the store seam](../crates/locust-proto/src/store.rs)
and credential helper.

Make directory creation and final credential-name publication durable, with
errors surfaced. Existing process-kill tests do not prove physical power-loss
behavior.

## Verification actually performed

On the frozen **committed** tree, with pinned Rust 1.96.1:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

All passed: **265 tests passed, 0 failed, 3 intentionally ignored**. Targets were
isolated from active lane work. Passing includes scaffold crates and therefore
does not demonstrate a complete daemon. Store/workspace scoped fmt, clippy and
tests also passed in their byte-identical working snapshot.

The unmodified **working** snapshot did not pass full formatting or compile its
core tests: `Founded::clone_goal` is missing in `sync/tests/machines.rs`. WIP
daemon-only tests passed, **39/39**, exercising the shell with stand-in engines.
These unfinished-file failures are maturity evidence, not allegations that Lane
A committed a broken build.

For the authority harness, declaring the orphan modules exposed three borrow
errors. After only test module wiring, three task-ID copies to shorten borrows,
and disabling unrelated unfinished sync tests, all four assertions of the
undesired behavior passed. Existing node tests also passed, **17/17**. The harness
does not exercise production Goal entry points, Node lifecycle or persistence.

For sync, adding the missing test helper and fixing an incorrect existing chunk
count expectation yielded **17 passing tests**. Expanding a fork case to 8,292
variants made one replica actually exceed the 4,096-point inventory page size;
pagination converged. Three extra desired-behavior tests failed as expected for
B-R10, B-R12 and B-R13. Reconciliation evidence covers synthetic replicas, not
transport, actual nodes or durable join/content handling.

## Takeover assessment and proposed order

**Comfortable taking over and orchestrating this architecture.** Preserve the
pure planning interface, single durable commit boundary, separation of retained
history from effective state, session/generation fencing, store conformance suite
and transport-independent sync machines. These are understandable boundaries with
meaningful existing tests; there is no evidence here that a redesign is needed.

The risk is integration and insufficient exercised behavior across those
boundaries. The following is a proposal, not a claim that ownership has already
changed or that the current writers were stopped:

1. Checkpoint active Lane A work and establish one integration owner with explicit
   file ownership. Keep parallel implementation limited to disjoint areas.
2. Wire and compile Goal, fix the authority/export findings, and test deterministic
   fold under arrival permutations, replay and incremental append. Keep removal
   findings attached to that feature even if T1 does not enable it.
3. Prove one complete local public-API transcript: create/admit/propose/assign,
   claim/submit/inspect/accept, takeover with a stale generation, cancellation,
   restart and failed commit. Use Node with SqliteStore rather than stand-ins.
4. Wire production Host/Replica and the real transport shell; resolve completion,
   incremental output and fair content fetching. Lane B also owns the outstanding
   address-lookup dependency raised as **A-R15** in [Lane A's log](../docs/lane-a-log.md):
   key-only dialing must work for non-coordinator peers and after port changes.
5. Connect CLI and client adapters to that real daemon, then run three independent
   local processes before the [published-build, three-Mac T1 run](../docs/t1-build.md).
   T1 still needs a first-user fetch/verify/start path and an identified published
   artifact. Publication and separate-machine evidence have not happened here.

Ownership would be practical as three parallel scopes—goal/node correctness,
daemon/CLI integration, and sync/transport integration—with one orchestrator
controlling contract changes and reviewing complete transcripts at each boundary.
Do not use unit-test totals, source review, scripted-provider client checks or
same-host transport results as substitutes for the published three-Mac evidence.
