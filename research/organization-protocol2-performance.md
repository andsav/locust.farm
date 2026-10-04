# Organization runtime replay and batch-ingestion comparison

Measured 2026-10-04 during the organization runtime cutover. **Status: local
measurement with a remaining performance regression; no numerical release gate
was defined.** This is the replacement-side companion to the
[protocol-1 baseline](organization-protocol1-performance.md), not a claim that
new and old protocols implement identical work.

## Reproduction and evidence

The executable [current-protocol measurement harness](../crates/locust-core/tests/organization_performance.rs)
is an ignored integration test. It has correctness assertions and no numerical
pass/fail threshold. The [final raw output](evidence/organization-performance/protocol2-final-output.txt)
records every minimum, median and maximum. The
[source digest manifest](evidence/organization-performance/protocol2-source-sha256.txt)
identifies the measured files from the uncommitted runtime candidate; the
subsequent engine commit records those files in Git. The shared checkout had
other cutover work in progress, so this was not an isolated release build.

Use the repository's pinned Rust toolchain on the same macOS arm64 Apple M5 Pro
host as the baseline, with the release profile:

```sh
CARGO_TARGET_DIR=output/organization-baseline/target cargo test --locked -p locust-core --release --test organization_performance -- --ignored --nocapture
```

Construction, signing, initial store population, expected-state generation and
wire encoding occur outside timed samples. Each mode has one warmup and seven
samples; each sample includes state equality checking and temporary-goal
destruction. Histories are ingested in 128-event batches. Compilation is excluded.
The host was also doing development work; scheduling and thermal conditions
were not isolated. These finite workload sizes and sample counts are measurement
parameters, not runtime limits.

## What is comparable and what changed

Both workloads create, execute, publish and select 16, 128 or 512 tasks. Both
append a correctly signed timestamp-only sibling of the midpoint worker result
in the forked variant. Every final task must remain selected, including tasks
whose ordinary author prefix becomes pending after the fork. The current harness
checks the dedicated scoped accepted projection and compares complete terminal
state for every timed ingestion mode.

Both use five events per task. Protocol 1 used proposal, assignment, worker
acceptance, result and coordinator acceptance. The current protocol uses task
creation, independent attempt start, contribution, explicit completion
declaration and scoped selection. Founding adds one definition binding, yielding
84/644/2,564 healthy events and 85/645/2,565 forked events. Definitions are already
available in memory; their resolution and rule validation are included, fetching
and decryption are not. The new projection also exposes explicit completion,
scope decisions and retained accepted subjects instead of a global accepted task
state. These are materially different authorization workloads despite matching
per-task event counts.

The three modes retain the baseline boundaries: `MemStore` replay, decoded batch
ingestion, and canonical wire decode/signature verification plus batch ingestion.
They exclude SQLite/disk durability, network delivery, daemon scheduling, encrypted
content and fetching. They do not qualify those systems' performance.

## Results

Times are median milliseconds. Exact minimum/maximum values are in raw evidence.

| Tasks | History | Events | Replay (old → current) | Decoded batches (old → current) | Wire + batches (old → current) |
| --- | --- | --- | --- | --- | --- |
| 16 | Healthy | 84 | 0.038 → 0.570 | 0.030 → 0.602 | 1.982 → 2.763 |
| 16 | Member fork, exact selection | 85 | 0.184 → 0.781 | 0.185 → 0.894 | 2.144 → 4.817 |
| 128 | Healthy | 644 | 0.236 → 11.781 | 0.213 → 29.304 | 16.172 → 46.695 |
| 128 | Member fork, exact selection | 645 | 9.949 → 6.662 | 9.725 → 25.397 | 25.950 → 47.782 |
| 512 | Healthy | 2564 | 1.081 → 42.195 | 0.910 → 363.812 | 64.114 → 407.031 |
| 512 | Member fork, exact selection | 2565 | 170.567 → 29.203 | 170.401 → 282.169 | 234.646 → 353.113 |

## Investigation and limits

An [initial diagnostic run](evidence/organization-performance/protocol2-initial-output.txt)
measured 512-task healthy replay at 304.947 ms and decoded batch ingestion at
2,531.464 ms. That run preceded the final accepted-view projection and subsequent
optimizations; it has raw output but no frozen independent source revision, so
it is diagnostic evidence rather than a separately reproducible benchmark release.

The implementation now shares immutable exact proofs, indexes positive evidence
by exact subject, memoizes pin-independent author validation within a fold, and
stores exact closure membership in dynamically sized bitsets. An iterative
per-event closure index reuses common signed ancestry and typed dependencies;
every decision still checks its own incompatible branch groups, signer ordering,
governance anchor and complete semantic authorization. Successful structural
closures may survive append only while the exact governance ordering is unchanged.
Missing/invalid cached closures are discarded and fork groups are rebuilt on
refresh. All statuses, scope authority, rules, membership and cutoffs are still
re-evaluated. This introduces no history, traversal or execution cap.

The targeted regression suite checks cutoff non-bypass, incompatible closures,
scope-only acceptance, later fork retraction, and rotated/reversed arrival in
small batches. Clippy and these tests passed after the indexing changes. These
checks support the implementation invariants; they are not a profiler result.

Healthy replay and both decoded batch paths remain slower than protocol 1 at the
largest workload. Full rule/status/projection refresh still occurs per batch,
whereas the prior healthy engine had an incremental append path. This is a
remaining implementation limitation, not an accepted latency target. Further
incremental evaluation needs explicit dependency invalidation for new reviews,
late forks, tenure changes, rules changes and scope conflicts; skipping those
checks would invalidate the comparison. Forked replay is faster on this workload,
but the changed evidence/authorization contract prevents generalizing that to a
protocol-wide speed claim. No production latency, throughput or capacity claim
follows from these measurements.
