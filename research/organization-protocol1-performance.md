# Protocol-1 replay and batch-ingestion baseline

Measured 2026-10-04 before the organization runtime replacement. **Status: local
measurement, not a performance requirement or protocol-2 result.** This records
the O0/V22 baseline required by the
[organization implementation plan](../docs/organization-blueprints-implementation-plan.md).
The [semantics contract](../docs/organization-blueprints-semantics.md) identifies
the invariants the replacement must retain.

## Source, environment and reproduction

Runtime source revision: `6b9365f89b62b898a4bd4178889af280006ef48e`.
`crates/locust-core/src/goal/`, `crates/locust-proto/src/event.rs` and
`crates/locust-proto/src/testkit.rs` were unchanged from that revision. The shared
checkout also contained uncommitted offline authoring/dependency work; therefore
this is not a pristine whole-checkout release measurement. The harness uses the
existing protocol-1 event engine and testkit directly.

Host: macOS arm64, Apple M5 Pro. Compiler: pinned
`rustc 1.96.1 (31fca3adb 2026-06-26)`. Cargo release profile. No attempt was made
to isolate CPU scheduling or thermal state from other work on the host.

The exact [harness source](evidence/organization-performance/protocol1-baseline.rs.txt)
is retained as historical text. It was temporarily installed as
`crates/locust-core/tests/protocol1_baseline.rs`, run, then removed so a superseded
protocol-1 executable fixture does not remain after the cutover. To reproduce,
use the recorded runtime revision in a disposable checkout, copy this text to
that integration-test path, and run the same command:

```sh
CARGO_TARGET_DIR=output/organization-baseline/target cargo test --locked -p locust-core --release --test protocol1_baseline -- --ignored --nocapture
```

The [complete command output](evidence/organization-performance/protocol1-baseline-output.txt)
includes compilation and all measured minimum/median/maximum values. Compilation
was outside each timed sample. The measurement test passed all workload/state
assertions; total test execution was 5.99 seconds. No numeric pass/fail threshold
was configured.

## Workloads and measurement boundaries

Each history begins with genesis, administrator self-admission and worker
admission. Every task contributes five events: proposal, assignment, worker
acceptance, result submission, coordinator acceptance. Histories contain 16,
128 or 512 completed tasks: respectively 83, 643 or 2,563 events.

The forked variant appends one correctly signed sibling of the midpoint task's
worker result, differing only in diagnostic timestamp. Coordinator acceptance
pins the original exact result and its ancestry. This yields 84, 644 or 2,564
events and exercises proof reconstruction after discovery of a member fork.
Every final state must still contain the expected number of accepted tasks.

Each mode runs one untimed warmup and seven timed samples. Reported values are
medians; raw output retains minimum/maximum as well. Event construction/signing,
initial store population, the expected projection and wire encoding occur outside
the timed region. Each timed sample includes projection equality validation and
destruction of its temporary goal, so these are operation measurements, not
isolated function-instruction timings.

- **Store replay:** `Goal::load` over an already populated `MemStore`, using the
  existing store pagination and full fold. This measures in-memory stored-event
  reconstruction, not SQLite opening, disk reads, durability, or decryption.
- **Decoded batch ingestion:** fresh `Goal`, then `Goal::apply` in 128-event
  batches. The terminal fork arrives in the final batch. This includes the
  incremental engine and resulting refold, but excludes wire verification,
  admission screening, node/store commits, transport and blob fetching.
- **Wire decode plus batch ingestion:** same batches, with canonical decoding and
  signature verification through `Event::from_wire` before `Goal::apply`.
  Network transport, node screening and durable commits remain outside scope.

The fixed batch size and history counts specify finite measurement workloads;
they are not product limits. These histories carry no encrypted payload objects
and no task dependency graph. They do not establish performance for those cases.

## Measured medians

All times below are milliseconds; raw harness output uses integer microseconds.

| Tasks | Events | History | Store replay | Decoded batch ingestion | Wire decode + batch ingestion |
| --- | --- | --- | --- | --- | --- |
| 16 | 83 | Healthy | 0.038 | 0.030 | 1.982 |
| 16 | 84 | Member fork, exact branch pinned | 0.184 | 0.185 | 2.144 |
| 128 | 643 | Healthy | 0.236 | 0.213 | 16.172 |
| 128 | 644 | Member fork, exact branch pinned | 9.949 | 9.725 | 25.950 |
| 512 | 2,563 | Healthy | 1.081 | 0.910 | 64.114 |
| 512 | 2,564 | Member fork, exact branch pinned | 170.567 | 170.401 | 234.646 |

The healthy and forked paths differ substantially at the larger workload. Source
inspection supplies a plausible cause: `Commitments::build` validates successive
coordinator commitments by tentative folding when a member fork exists, whereas
a healthy replay can use the ordinary author prefixes. These timings are not a
profiler result and do not isolate that cost from allocation or other fold work.

Compare the replacement on equivalent task/result/selection and fork-retention
workloads, explicitly recording added rule/evidence work. Do not claim a speed
improvement merely from changing the event count, deleting proof checks, using a
different build profile, or timing a different measurement boundary. O0's other
protocol/model obligations are not completed by this baseline.
