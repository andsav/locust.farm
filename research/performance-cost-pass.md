# Performance and agent cost pass

Measured 2026-10-04 against `be7c076`, after the
[real-model collaboration pilot](shared-context-real-model-pilot.md). These are
paired local measurements of the same API-3 behavior. No model or provider was
called in this pass; token usage and dollar savings are not measured.

## Changes and results

The 48-tool MCP catalog previously copied every request definition into every
tool, including unrelated definitions, and regenerated the request schema for
each operation. The [schema generator](../crates/locust-proto/src/api/schema.rs)
now caches immutable generated schemas and retains each operation's transitive
reference closure. The [MCP bridge](../crates/locust/src/mcp/schema.rs) caches the
tool list. Every tool, field and referenced type remains available. Unsupported
reference forms conservatively retain all definitions.

The [CLI](../crates/locust/src/cli/mod.rs) avoids rendering a terminal summary for
JSON output. The [context reader](../crates/locust-core/src/node/context.rs)
reuses each event version and availability result for the page and its pending
summary. [Pending work](../crates/locust-core/src/node/views.rs) collects the
caller's effective reviews once through the author index, replacing a complete
history scan per review candidate. No read acknowledgment, content, permission
or pagination semantics changed.

The [engine](../crates/locust-core/src/goal/fold.rs) indexes candidate decisions
by exact author, context/purpose and predecessor for each fold. It still applies
the same authorization and conflict checks. [Author prefix lookup](../crates/locust-core/src/goal/history.rs)
uses the existing contiguous-prefix invariant to test the exact sequence and
event identifier. These indexes do not retain eligibility across folds.

| Measurement | Before | After | Change |
| --- | ---: | ---: | ---: |
| Compact MCP tool catalog, all 48 tools | 366,531 B | 32,300 B | 91.2% smaller |
| Authenticated warm MCP `tools/list` | 13.979 ms | 0.432 ms | 96.9% less time |
| New CLI process: `--version` | 22.819 ms | 6.096 ms | 73.3% less time |
| New CLI process: `--help` | 22.799 ms | 6.294 ms | 72.4% less time |
| New CLI process: JSON runtime contract | 25.707 ms | 8.849 ms | 65.6% less time |
| New CLI process: JSON blueprint contract | 22.974 ms | 6.332 ms | 72.4% less time |
| New CLI process: authenticated JSON pending | 22.988 ms | 6.158 ms | 73.2% less time |
| Context page, 256 findings | 6.209 ms | 2.771 ms | 55.4% less time |
| Pending unread, 256 findings | 5.887 ms | 2.746 ms | 53.4% less time |
| Healthy history replay, 512 tasks | 27.046 ms | 20.433 ms | 24.5% less time |
| Healthy decoded batches, 512 tasks | 263.560 ms | 205.570 ms | 22.0% less time |
| Healthy wire decode and batches, 512 tasks | 326.907 ms | 265.141 ms | 18.9% less time |

Forked histories retain their accepted proof branches and also improve: the
512-task case measures 23.394 → 17.523 ms for replay, 265.572 → 200.205 ms for
decoded batches, and 321.568 → 261.112 ms including wire verification.

The [operating skill](../skills/locust/SKILL.md) and MCP initialization guidance
now ask for full context at start, task changes or context recovery; incremental
updates at collaboration checkpoints; and a freshness check before publication
or a shared-state decision. Agents retain context during local work. The skill
also tells them to use their configured CLI or MCP transport. These instruction
changes have not been qualified in a new real-model campaign, so no reduction in
model turns is claimed.

## Evidence and measurement boundaries

The [machine-readable evidence](evidence/performance-cost-2026-10-04.json)
retains sample timings, response sizes and fingerprints, source digests and exact
binary SHA256 identities. The baseline comes from an archive of `be7c076`; the
candidate binary was built from the recorded working source before the commit,
so its version label deliberately retains `be7c07686751-dirty`. Both use the
repository release profile and Rust 1.96.1, on macOS 26.4 / Apple M5 Pro / arm64.

- CLI and MCP use 3 warmups and 31 samples, alternating before/after order, with
  warm filesystem caches. Each CLI observation starts a new process. Both
  clients use the same isolated baseline SQLite daemon to measure client work.
  The daemon and both MCP bridges exit cleanly; temporary profiles are removed.
  Help and all measured JSON responses match exactly. Version strings differ
  by build identity. Initial MCP startup is recorded as one observation per
  binary, baseline first; it is not a repeated cold-cache measurement.
- Context uses deterministic 32/128/256-finding fixtures, each with five request
  cases and 15 samples. Before/after/after/before runs match all 15 serialized
  response hashes and byte counts. At 256 findings, the two runs improve
  2.11–2.24×; smaller fixtures improve 1.32–1.77×. The context binary isolates
  the node changes; later engine edits do not participate in these read-only
  timed paths. No returned text or pending items were removed.
- Engine uses 16/128/512 tasks, healthy and pinned-member-fork histories, one
  warmup and seven samples. All six full evaluation fingerprints match across
  revisions. Timed modes compare complete evaluation, including standing and
  pending dependencies, against replay. This is `Goal`/`MemStore`, excluding
  SQLite durability, network, encrypted content and daemon scheduling. Full
  batch refresh remains; this does not remove all historical scaling costs.

Builds and this task's other tests were stopped during measured runs. Ordinary
host scheduling and thermal conditions remain uncontrolled. The native sampling
attempt produced no usable stacks and supplies no profiler evidence. These
finite fixtures and sample counts are measurement parameters, not product limits
or capacity guarantees. Static schema caches retain immutable schemas per
process; the decision-candidate index uses memory proportional to held decisions
and is rebuilt per fold.

The pilot's million aggregate input tokens included extensive cached input.
Catalog byte reduction cannot be translated directly into that run's tokens,
latency or bill. Receipt-copying ergonomics, inherited source visibility,
late-arriving findings and further reduction of repeated context metadata remain
separate work. In the 256-finding fixture even an acknowledged context reply is
about 103 KB because the complete pending review list remains present; any
smaller view should be explicit and tested, with no silently omitted obligations.

## Reproduction

Build and preserve each release binary from its identified source. Then run the
[paired client harness](../scripts/check_performance_cost.py) with an empty output
directory:

```sh
python3.12 scripts/check_performance_cost.py \
  --before /absolute/path/before/locust \
  --after /absolute/path/after/locust \
  --output output/performance-comparison \
  --samples 31 --warmups 3 --operation-timeout-seconds 30
```

This harness uses the existing macOS loopback-only production fixture. Its
explicit operation timeout covers benchmark RPC/startup/cleanup, not agent work.
For each revision, copy the exact current measurement tests into that source,
compile first, and run them while the other build is idle:

```sh
cargo test --locked -p locust-core --release --lib context_read_performance --no-run
cargo test --locked -p locust-core --release --lib context_read_performance -- --ignored --nocapture
cargo test --locked -p locust-core --release --test organization_performance --no-run
cargo test --locked -p locust-core --release --test organization_performance -- --ignored --nocapture
```

The context harness checks every timed response against the first response.
Compare its response hashes across revisions. The engine harness compares full
evaluation on each sample and emits six cross-revision fingerprints. The schema
unit tests independently compare original fields and all reference targets,
including transitive cycles and escaped JSON pointers. New regression tests
cover late competing decisions, author gaps/forks/duplicate arrival, caller-only
effective reviews, changed standing across restart and task-scoped context news.

## Verification

Formatting and strict workspace Clippy pass. The complete workspace rerun passes
**672 Rust tests, with 14 explicit ignores**; the separate measurements exercise
the context/engine benchmark ignores. All **204 Python tests** pass, the runtime
contract export is unchanged, and all six blueprint examples validate.

The first Rust run stopped at the existing two-daemon reviewed-invitation test
after its 20-second convergence deadline (161 passed, one failed, three ignored
in that binary). The isolated recheck passed in 11.56 seconds, then the full
workspace rerun passed without source or deadline changes. The initial timeout's
cause is unestablished; the successful rerun does not erase it. Independent
source review found no blocker in the indexes, schema closure or CLI changes.

These are source and local release-binary checks. No new installed-client,
physical-machine, packaged-release or real-model qualification is claimed.

## Subsequent collaboration work

The [collaboration follow-up](collaboration-followups.md) implements short client
receipt references, explicit compact context and pending pages, signed source
inspection and invitation lifecycle fixes. It also records a later live-model
campaign and its failures. These new observations do not change the API-3 paired
measurements above or establish a paired model-cost improvement.
