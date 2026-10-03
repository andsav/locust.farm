# MoltMesh validation and reproducibility

Research date: 2026-10-03. Reviewed source: default branch `actor-model`, exact commit [`707c870e3188df243e5aea3c4662daa3253270bc`](https://github.com/sahilpohare/MoltMesh/commit/707c870e3188df243e5aea3c4662daa3253270bc). Host: macOS, Apple Silicon (`darwin/arm64`); Go 1.26.5. The repository was cloned separately from Locust, and its tracked files were left unchanged. Research probes and harness adaptations ran in a separate archive copy of that commit, with synthetic identities and temporary data.

## Results

| Check | Observed result | What it establishes |
|---|---|---|
| Upstream full Go race suite | **Failed overall**: 273 test/subtest passes, 1 failure, 1 skipped soak test | Broad exercised baseline; not an all-green checkout |
| Failed test, three focused repeats | All three passed | Intermittent failure; no root cause established |
| Five local task characterization probes | All five reproduced their expected gaps | Specific store/RPC semantics; not full SDK or WAN tests |
| Unsnapshotted Raft restart probe | Same payload applied twice | Reproduced component-level replay defect |
| Existing multi-process thread demonstration | **Passed** | Real local P2P discovery, task/result flow, replication and fresh-reader recovery |
| Release-configuration native build | Built successfully | Compilation only |
| Starting that CGO-disabled binary | **Failed**, SQLite driver stub | Release configuration incompatible with required runtime database path |

“PASS” on a characterization probe means that the described defect/limitation was observed. Those tests intentionally assert the current undesirable behavior; they are not correctness gates that should remain green after a fix.

## 1. Upstream test suite

Executed against the unmodified pinned checkout:

```sh
/opt/homebrew/bin/go test -race -timeout 15m -json ./...
```

JSON event totals: 19 package passes, 1 package failure, 7 packages without tests; 273 test/subtest passes, 1 failure and 1 test skip. Counts include subtests, not 273 independently designed scenarios. No `WARNING: DATA RACE` diagnostic was observed, but the suite failed functionally:

```text
TestExecutorReacquireAfterRelease
executor_race_internal_test.go:71: iter 135: dispatch on concurrently acquired executor: actor is not alive
FAIL github.com/sahilpohare/p2p-a2a/daemon/deliver
```

`TestSoakHundredThousandDormantThreads` was skipped. The passing `e2e` package uses in-process local libp2p nodes; its fixture deliberately omits DHT/bootstrap. It is not WAN or mass-scale evidence. [E2E fixture](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/e2e/e2e_test.go#L40-L67).

Focused follow-up:

```sh
/opt/homebrew/bin/go test -race -count=3 \
  -run '^TestExecutorReacquireAfterRelease$' -v ./daemon/deliver
```

All three repetitions passed. This does not erase the original failure or prove actor reacquisition reliable; it classifies the result as intermittent in this environment. No fix was attempted.

## 2. Task characterization probes

Five new research-only tests used temporary SQLite stores, in-memory outboxes, and direct calls to actual MoltMesh store/RPC methods. No remote deployment was contacted. For the two-sided cases, `HandleIncoming` received the first server's queued message directly.

```sh
/opt/homebrew/bin/go test -race -v ./researchprobe
```

Observed:

1. Two claimants sharing the same assignee identity receive the same live lease token. The lease is identity-idempotent; it does not distinguish worker processes.
2. Advancing a subscription cursor before any claim leaves the task `SUBMITTED`, but reconnecting after that cursor returns no delivery. The test emulates the SDK checkpoint order; it does not kill a real Python SDK process.
3. Reusing an idempotency key with another assignee returns the original task while enqueuing another request to the new assignee with the old task ID. This crosses the storage/dispatch boundary missed by a store-only idempotency test.
4. An initiator asking for seven attempts and a 60-second deadline produces an assignee record with the default three attempts and no deadline.
5. Canceling at the initiator leaves the assignee's task `SUBMITTED` and enqueues no cancellation message.

The complete probe source and captured output are preserved in [task probe appendix](evidence/task-probes.md). Analysis and exact source references are in [tasks and SDKs](moltmesh-tasks-and-sdk.md).

## 3. Unsnapshotted Raft restart

A focused in-package probe used one synthetic voter and a persistent temporary SQLite database. It committed one payload, confirmed the pending queue was empty, stopped the backend, closed/reopened the database, and reconstructed the backend **without taking a snapshot**. Then it pumped the restarted backend without new ticks, enqueues or network input.

```sh
/opt/homebrew/bin/go test -race -count=1 \
  -run '^TestResearchRestartReplaysCommittedApplicationEntry$' -v ./daemon/thread
```

```text
before restart: height=1 payload_count=1 snapshot_index=0 raft_commit=3 raft_log_entries=3 pending=0
after restart without enqueue/tick/network: height=2 payload_count=2 applied_index=3
PASS
```

This confirms a duplicate application-history entry after component reconstruction. It does not demonstrate a Raft agreement violation; the same committed Raft entry is applied again by MoltMesh. `Config.Applied` is not set, and application append allocates another block height. See [architecture analysis](moltmesh-architecture-and-consensus.md).

Scope matters: this is a component-level model of a crash before snapshot, not a process-kill or multi-node failover test. Graceful actor shutdown calls `Snapshot`; the probe deliberately bypasses that mitigation to exercise an unsnapshotted persistent state. Complete [source and output](evidence/raft-restart-probe.md) are retained.

## 4. Existing multi-process demonstration

Executed the repository's [`e2e/manual-thread/run.sh`](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/e2e/manual-thread/run.sh) in the separate copy. Adaptations were environmental: remove the scripts' `HOME` overrides and use their explicit `--data-dir`/`--config` locations instead; select `/opt/homebrew/bin/go` for the build; rename the shell's local `home` variable. Production Go source was unchanged. gRPC endpoints were loopback; the daemons used a local bootstrap peer and the standard P2P listener configuration. All processes ran on one laptop, not separate physical hosts or WAN networks.

The harness starts four named agent daemons plus a bootstrap daemon, and later a recovery daemon. The calculator is deterministic shell arithmetic, not a language model. The thread uses `f=0`: one voter plus observers. Storage holders remain online for recovery.

Captured successful milestones:

```text
discovering calculator and text-generation capabilities
created encrypted recoverable thread 9e508ddd-1103-4a84-8428-8abafdc9ffc2
calculator and observer joined the committed thread
submitting calculator task
task 0e8a0383-4f5c-4e9d-a3bd-8241a8605ac9 completed with answer 4
verified durable task-event replay
verified terminal result replication on all three members
verified cold recovery with the capability secret
PASS thread=9e508ddd-1103-4a84-8428-8abafdc9ffc2 task=0e8a0383-4f5c-4e9d-a3bd-8241a8605ac9 answer=4
```

Harness exit status was 0 and its processes were cleaned up. The recovery capability/private keys were not copied into Locust. A cleanup `Terminated: 15` line followed PASS as expected.

This is meaningful real-process happy-path evidence. It does not establish multi-voter fault tolerance, unattended model collaboration, cross-NAT connectivity, a retained third-party mailbox, or retrieval while every content holder is offline.

## 5. Release build and startup

The pinned [GoReleaser configuration](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/.goreleaser.yaml#L8-L22) specifies `CGO_ENABLED=0`. The application depends on `github.com/mattn/go-sqlite3`, whose non-CGO build is a runtime error stub.

Built a native binary using the relevant release settings:

```sh
CGO_ENABLED=0 /opt/homebrew/bin/go build -ldflags '-s -w' \
  -o /tmp/moltmesh-release-config ./cmd/moltmesh
```

The build succeeded and produced a 52,757,266-byte binary (about 50.3 MiB). That is the size of this stripped, unusable CGO-disabled build, not a usable-daemon footprint benchmark. Starting it with a fresh temporary data directory, empty bootstrap configuration and loopback listener failed with exit code 1:

```text
daemon error: inbox: migrate inbox: Binary was compiled with 'CGO_ENABLED=0', go-sqlite3 requires cgo to work. This is a stub
```

This verifies the current source release-configuration defect on macOS/arm64. We did not download or execute the historical public v0.1.0 artifacts, run the entire GoReleaser pipeline, or test each OS/architecture. The Homebrew package test checks only `version`, which would miss database startup failure. Production artifact validation should exercise initialization and a basic write/read, not just executable launch/version output.

## Repository and CI snapshot

GitHub API observations on 2026-10-03:

- Default branch: `actor-model`; pinned HEAD dated 2026-09-17 with subject `wip: log every RPC arrival and completion`.
- Latest CI run at that exact SHA was marked **failure**: [run 35242266172](https://github.com/sahilpohare/MoltMesh/actions/runs/35242266172). The observation is status only; this report does not attribute its failure to the locally observed intermittent test.
- The release list contained [v0.1.0](https://github.com/sahilpohare/MoltMesh/releases/tag/v0.1.0), published 2026-05-31, targeting `main`. It is not the reviewed actor-model snapshot.
- The checked-in [LICENSE](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/LICENSE) is Apache 2.0, while GoReleaser's package metadata still says MIT. Treat that as a metadata inconsistency when evaluating reuse.

The checked-in CI workflow includes Go, Python and TypeScript jobs, with benchmarks gated to manual workflow dispatch. This research ran the full Go suite and the listed local probes/demo; **it did not run the Python/TypeScript suites, benchmarks or scale soak**. Workflow configuration is not evidence that every job currently passes. [CI definition](https://github.com/sahilpohare/MoltMesh/blob/707c870e3188df243e5aea3c4662daa3253270bc/.github/workflows/ci.yml).

## Important unverified boundaries

No internet/NAT matrix, malicious-peer test, formal consensus verification, long-running load/soak, resource benchmark, all-holder-offline retrieval, or complete multi-platform package validation was performed. The source review identifies several concerns in those areas; they are labeled as source findings or proposed experiments in their respective documents. Passing the local demo does not resolve them.

All published research findings remain tied to the pinned commit and date. Before implementing a borrowed approach or filing upstream issues, recheck current source and turn the relevant characterization into a focused correctness regression test.
