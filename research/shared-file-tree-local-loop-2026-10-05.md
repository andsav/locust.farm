# Shared tree local daemon loop

Status: passing model-free source fixture on macOS, 2026-10-05. This is one
loopback-only production SQLite daemon with separate local principals and
ordinary directories. Separate same-host transport evidence is recorded below.
It does not establish two-physical-host, real-agent or packaged
release qualification.

## Reproduction and source

The test
`test_actual_seed_review_integration_update_preserves_dirty_ordinary_checkout` in
[test_collaboration_acceptance.py](../scripts/tests/test_collaboration_acceptance.py)
uses the current helper setup from
[check_shared_context_models.py](../scripts/check_shared_context_models.py).
It does not invoke a coding agent or spend provider credits.

Build the current binary with the pinned Rust toolchain, then run:

```sh
cargo build --locked -p locust
/opt/homebrew/bin/python3.12 -m unittest discover -s scripts/tests -p test_collaboration_acceptance.py
```

Python 3.12 or newer is required. The real-daemon case requires macOS,
`/usr/bin/sandbox-exec` and the built executable; it explicitly skips when those
prerequisites are absent. The helper guard permits loopback and Unix socket
traffic only. The test copies the selected binary into its private temporary
output so a concurrent Cargo rebuild cannot change its bytes during restart.
It retains the production fixture's identity and binary-change refusal checks.

## Observed loop

The passing run used an API 6 / protocol 6 development binary. The fixture:

1. Created a fresh goal and explicitly pinned a one-peer-review workspace policy.
2. Captured two explicitly selected public starter files without Git, supplied
   a distinct peer's seed approval and integrated the exact seed.
3. Created and session-bound separate builder and integrator checkouts at that
   revision. Neither required a repository or worktree.
4. Published a new exact workspace proposal for `safe_member.py`; published a
   separate generic report citing both a finding and the proposal as advisory
   sources, and inspected the returned signed references.
5. Independently materialized the exact proposal for review. Integration before
   peer approval was refused; after approval it accepted that exact proposal.
6. Updated the separate integrator checkout, preserving an unpublished README
   edit and an unrelated ordinary file byte for byte.
7. Restarted the daemon and verified the exact completed base revision and the
   still-dirty README through checkout status.

The first run reached the file update, then correctly refused restart because
another build changed `target/debug/locust`. After pinning an exact executable
copy, the full case passed. This failure was not reclassified as a passing
restart and the binary-integrity check was not relaxed.

Companion false-pass tests require exact proposal parent/result identity, a
complete successful native review, independent reviewed bytes, an exact native
publication receipt, finding delivery/acknowledgment and a generic report's
explicit proposal citation. Reviewing the generic task report does not approve
the workspace proposal. Another proposal, altered review bytes, missing receipts,
substituted source details, different session receipts and lost dirty files fail
their corresponding checks. Review destinations and later approval evidence may
differ without changing the immutable candidate and exact authenticated diff.

## Separate two-daemon transport smoke

A second run at 2026-10-05 09:23:59–09:24:06 UTC used two native production
daemons on the same macOS ARM64 host. Local Iroh route snapshots selected direct
paths with `LOCUST_LOOKUP=local` and `LOCUST_RELAY=none`. The
[retained summary](evidence/shared-file-tree-local-smoke-2026-10-05.json) names
binary SHA-256
`021d547409435e43966a5d5bb6e6de1ee31623e9972e557d0c49ea70aab998f0`,
embedded identity `98d03af6ce6a-dirty`, API 6 / protocol 6, and harness SHA-256
`7fdc1ecb0b0f4be1d2fdda814faa4031c7988020677caa7be7caa4a7fd6aac5b`.
Absolute local paths are redacted; original summary hash and changed fields are
retained. These checks identify that development artifact, not every later build.

The exact invocation was `python3 output/workspace-smoke/run.py`, an ignored
one-off using the tracked [operations harness](../scripts/check_operations.py)
helpers and the existing explicit 90-second per-wait/child campaign deadlines.
The supported reproduction now exists in that harness, with a separate identified
passing artifact described below.
It checked membership convergence, replicated two-file ordinary seed, a worker
proposal with its exact completion declaration, lead review/integration,
replicated accepted head and content, update preserving compatible managed dirt
and an unrelated file, both SQLite daemon restarts, verified peer file readback,
and identical integration receipt on keyed retry. Private unselected worker bytes
were not included in the proposal.

The accepted peer readback contained the worker's new code and the accepted base
notes; compatible unpublished notes remained dirty only in the local checkout.
This distinction checks shared versus local disposition. The report contains no
failures and all four named case groups passed. CPU, network bytes, sleep/wake and
maximum RSS were not measured; there is no performance threshold or latency win.

Other operations campaigns are separate evidence. A three-daemon local attempt
stopped before workspace work when the independent worker link did not converge.
A three-daemon default-network attempt observed a 12 MiB seed transfer/resume,
then stopped on a harness argument collision. Neither is relabeled as a complete
passing workflow. The two-daemon smoke establishes one same-host direct transport
scenario; it does not erase those failures or establish WAN connectivity.

## Supported operations campaigns

The [combined retained evidence](shared-file-tree-local-loop-evidence-2026-10-05.json)
contains five identified summaries: two supported two-daemon local workspace
passes, a full three-daemon default-network pass, the earlier independent-worker-link
failure, and the earlier fixed harness argument-collision failure. Each summary
retains its original checksum, development binary and harness identity. Different
builds are reported separately. Machine paths/private addresses are redacted;
this does not change outcomes or qualify another artifact.

The supported two-daemon invocation is:

```sh
python3 scripts/check_operations.py --binary /ABSOLUTE/locust/target/debug/locust --network local --workflow workspace --timeout-seconds 90
```

Its run `20261005T092652Z-8bd2fb56` passed the same four workspace case groups,
with binary SHA-256
`eb0ec8f8ea9725fb7bcb9ec9e97ce89dc87ee90bd49ef065767331bca5481454`
and harness SHA-256
`948ca9c3acd6e5b43d97750ea964d62389df88ade52befc556075776c9cdc1df`.
The smoke does not sample resources; its additive qualification note corrects an
inherited broader operations resource label. Transport was explicitly local,
relay-free and direct in the recorded route snapshots.

The final supported run, `20261005T093615Z-14f726f7`, repeated all four case groups
with Git unavailable through the daemon and CLI child command PATH. The harness
used an isolated nonexistent tool directory, asserted that Git lookup returned
no executable, and kept its own PATH unchanged. The matching unit test also
requires an attempted `git --version` launch in that child environment to fail.
This run passed the complete ordinary-directory loop and both SQLite restarts
with binary SHA-256
`502976754cdb8b7c5a0092cd86c75dc33d458d509edbef1444ad77442c2717e9`
and harness SHA-256
`a2b294e6bf58bf9faf34825a892ecb52a9742ae8eec339cc0cc73aa180be2f0e`.
The retained summary records the unavailable Git lookup and its child-process
scope. Optional named-commit import was not exercised. Original summary checksums
and exact path-redaction field lists identify each retained copy.

The full three-daemon invocation is:

```sh
python3 scripts/check_operations.py --binary /ABSOLUTE/locust/target/debug/locust --network default --timeout-seconds 90
```

Run `20261005T092455Z-60a2d2d2` passed all six operations case groups, with binary
SHA-256 `ebf748b8fe15424ef03082701fd0a12479d59c41885aa82266ca5382eb3c634b`
and harness SHA-256
`5b73a95792733fe33a5d557e5acb963b4bce5bf3f17ebd62209f0723ca6f2523`.
The workspace part interrupted both a 12 MiB seed transfer and a published result
at an observed durable 1 MiB prefix, then resumed while the originating source
was offline. It checked a stale prepared integration refused without a new event
receipt, a conflicting update preserved complete local bytes/modes/binding,
compatible managed edits and private untracked files survived a successful update,
and keyed integration receipts and accepted peer readback survived SQLite restart.
Existing document, cancellation, withdrawal/leave and offline member-rotation
scenarios also passed. See the retained per-case summary for their exact scope.

This full campaign used default discovery/relay configuration on one host; a
configured relay or observed direct route is not evidence of a WAN crossing.
The earlier local convergence and harness failures remain in the evidence bundle.
There is still no two-Mac, cross-architecture, live coding-agent or sleep/wake
qualification. Resource samples are measurements with their recorded scope, not
maximum-memory or throughput guarantees.

## Limits

These are harness-authored actions and deterministic artifacts. The migrated
real-model drivers and prompts are implemented, but were not run with provider
models for this protocol. No natural-language agent understanding, OS separation
between agents, two-machine convergence, disk exhaustion, process-kill recovery
during checkout mutation or actual power-loss durability is claimed. The native
campaigns do exercise restart and catch-up during object transfer. Temporary
fixture directories are removed after the runs; retained development binary
fingerprints identify those tests, not a packaged release or deployment.

See the [implementation contract](../docs/workspace.md) and the separate
[content-index measurements](shared-file-tree-content-index-2026-10-05.md).
