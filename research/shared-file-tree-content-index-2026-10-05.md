# Shared tree content indexing measurements

Status: measured source-test fixture, 2026-10-05. This is structural indexing
and decode evidence, not a paired end-to-end latency benchmark or network result.

The [implemented contract](../docs/workspace.md) separates signed workspace
authority from content readiness. This experiment measures whether new ordinary
events revisit an accumulating collection of old workspace manifests.

## Method

The test
`growing_workspace_history_does_not_redecode_or_rescan_on_unrelated_work` in
[content_graph_tests.rs](../crates/locust-core/src/node/content_graph_tests.rs)
creates two in-memory peers, founds a goal and stages 128 distinct complete
workspace manifests. Manifest number N contains N paths referencing one shared
four-byte opaque file. At N = 8, 32 and 128, three unrelated signed generic
contribution events are delivered. Test-only counters compare manifest decodes,
root visits and complete-history rebuilds immediately before and after those
three events.

Reproduce from the repository root using the pinned Rust toolchain:

```sh
cargo test --locked -p locust-core --lib growing_workspace_history_does_not_redecode_or_rescan_on_unrelated_work -- --nocapture
```

The 2026-10-05 local macOS run passed and printed:

| Distinct manifests | Cumulative manifest entries | Total authenticated manifest decodes | New root visits for three unrelated events | Old manifest decodes for those events | History rebuilds for those events | Cumulative elapsed time |
| --- | --- | --- | --- | --- | --- | --- |
| 8 | 36 | 8 | 3 | 0 | 0 | 21 ms |
| 32 | 528 | 32 | 3 | 0 | 0 | 94 ms |
| 128 | 8,256 | 128 | 3 | 0 | 0 | 614 ms |

Each row's elapsed time starts before the first snapshot and includes fixture
publication, staging and earlier samples. It is not the latency of the three
unrelated events. Two initial history rebuilds remained unchanged at each sample.
The full test completed in 0.67 seconds, excluding the Cargo startup/build step.

After the growth samples, removal of a member triggers a reference rebuild on the
source peer. Cache hits increase and authenticated manifest decode count remains
unchanged. Companion tests verify that a dropped physical object, a replaced key,
withdrawal and a reader removed from a newer key epoch cannot gain access through
cached plaintext. Restart reconstructs typed references and readiness from
history and stored bytes.

## What this establishes

The [content index](../crates/locust-core/src/node/content_graph.rs) decodes each
new canonical manifest once in this fixture. A new unrelated ordinary event
visits its own root without rescanning old history or redecoding old manifests.
Reference rebuilding after an authorization change can reuse authenticated decode
facts while current serving authorization remains independently checked.

This does not measure SQLite startup, memory consumption, growing signed replay
cost, catch-up throughput, wire transfer, checkout file writes or two-host latency.
The manifests share one small file and the test uses in-memory stores. There is
no pre-change paired timing baseline, so these timings do not establish a causal
speedup. Large trees and retained caches still consume space proportional to the
stored data; no pruning or arbitrary new runtime cap was introduced.
