# Research evidence and reproducible helpers

Retained reports describe identified experiments and public source assessments.
The displayed publication copies redact machine-specific absolute paths and
private deployment addresses. Source, artifact and original-output hashes retain
the original measurement identity; they do not hash sanitized displayed
transcripts. Redaction metadata identifies changes where the report format allows
it. Sanitization does not imply rerunning a check or qualifying a newer artifact.

## Experiment records

- [Agents without work roles](role-free-board-2026-10-05/README.md): two real three-client trials, signed task choices, automatic review requests, duplicate work and permission friction; [findings](../role-free-board-2026-10-05.md).

- [Agent lifecycle model checks](agent-lifecycle-2026-10-05.json): all 11 bounded session-model cases matched expectations after adding durable stopped acknowledgment; [implementation scope](../agent-ergonomics-2026-10-05.md).
- [Published terminal preview](public-preview-release-2026-10-04.json): exact API-4 package/download identity; [release record](../../docs/public-preview-release.md).
- [Paired performance](performance-cost-2026-10-04.json): workload, candidate and response/evaluation fingerprints; [method](../performance-cost-pass.md).
- [Shared-context model pilot](shared-context-models-2026-10-04.json): exact native findings and independent patch oracle; [method](../shared-context-real-model-pilot.md).
- [Collaboration runtime](collaboration-runtime-2026-10-04.json), [Codex/Merak experiment](collaboration-live-codex-merak-2026-10-04.json) and [Pi/Claude experiment](collaboration-live-pi-claude-2026-10-04.json): retained failures and exact client/candidate limits; [methods](../collaboration-followups.md).
- [Public farm rehearsal](live-farm-demo-2026-10-04.json): four actual native clients, persisted projection and public streaming; [method](../live-farm-demo.md).
- [Farm pipeline](farm/local-pipeline.json), [discovery failures](farm/ephemeral-discovery-failures.json) and [Nginx routes](farm/nginx-routes.json): separate local transport and public UI boundaries; [method](../farm-qualification.md).
- [Transport measurements](transport-probe-2026-10-03.json) and [follow-up measurements](transport-review-2026-10-03.json): identified direct/relay probe observations; [method](../iroh-transport-probe.md).
- [Shared tree operations campaigns](../shared-file-tree-local-loop-evidence-2026-10-05.json): supported local two-daemon and default-network three-daemon runs, exact build fingerprints and retained failures; [method](../shared-file-tree-local-loop-2026-10-05.md).
- [Shared tree local transport smoke](shared-file-tree-local-smoke-2026-10-05.json): two ordinary-directory participants on two local production daemons, exact development binary and receipt/restart checks; [method](../shared-file-tree-local-loop-2026-10-05.md).
- [Formal model evidence](tla/README.md): executable current models, finite bounds, witnesses and deliberate mutations.

## Public prior art

- [ecdsa.fail leaderboard analysis](ecdsa-fail-leaderboard-analysis.md)
- [hcom validation evidence](hcom-validation.md)
- [Unsnapshotted Raft restart characterization probe](raft-restart-probe.md)
- [Task lifecycle characterization probes](task-probes.md)

## macOS recovery fault helper

[The flush interposer](t1-candidate-review/verify-store-flush-interposer.c) is used
by the current [SQLite recovery regression](../../crates/locust-store/tests/recovery_flush.rs).
It restricts faults to armed owned test children. Compile and run on macOS:

```sh
mkdir -p output/recovery
cc -dynamiclib research/evidence/t1-candidate-review/verify-store-flush-interposer.c -o output/recovery/flush-interposer.dylib
LOCUST_STORE_TEST_INTERPOSER="$PWD/output/recovery/flush-interposer.dylib" cargo test --locked -p locust-store --test recovery_flush recovery_flushes -- --ignored
```

This tests syscall ordering and failed barriers, not physical power-loss safety.
