# Locust TLA+ models and checks

Date: 2026-10-03. **Status: Stages 0 and 1 have separate historical
version-0 and current version-1 bounded models.** Start with the
[version-1 mapping and checks](version-1.md) for GoalLogV1 and SessionsV1.
The [implementation plan](../../docs/tla-verification-plan.md) defines later
replication, durability, CI and proof work. These models do not prove the Rust
implementation or qualify the running M1/M2 test.

The [upstream impact assessment](upstream-impact-2026-10-04.md) explains why the
historical GoalLog and Sessions models could not qualify version 1. Their
models, configurations and evidence are preserved. The revised models check
canonical branch commitments, accepted display and the scoped local session
contract; invitations, read epochs, transport and physical durability remain
outside these checks.

## Run the checks

Use Python 3.12 or newer. The runner downloads tools only with `--bootstrap`,
verifies their SHA-256 and keeps them under ignored `output/tla/tools/`. It uses
an isolated Temurin 21.0.8+9 JDK, not the system Java installation. macOS Arm64
and Linux x86_64 archives are pinned in [toolchain.json](toolchain.json).
Extracted runtime files and links are checked against the verified archive on
every invocation; a matching version string alone cannot qualify a changed JDK.

```sh
python3 -m unittest discover -s scripts/tests -p test_check_tla.py
python3 scripts/check_tla.py --bootstrap --suite fixtures
python3 scripts/check_tla.py --suite v1
python3 scripts/check_tla.py --suite fast
python3 scripts/check_tla.py --suite extended
```

The default Java heap limit is 4096 MB (`--memory-mb` overrides it); the
historical session case has a 900-second deadline; the version-1 session
safety case allows 1,800 seconds. The extended suite checks
all configurations, including the fast cases.

The [case registry](cases.json) names configurations, checked properties, expected
outcomes, deadlines and counterexample state requirements. `--case <id>` selects
a case. `--timeout <seconds>` overrides its deadline; a timed-out check fails.
The historical cases comprise two runner fixtures, 18 GoalLog configurations
and eight Sessions configurations. Version-1 cases are separately identified
with `v1-` IDs and explicit per-case baselines; `v1` selects only that revision.
The fast and extended suites include both revisions. The version-1 session
safety and 13-event cancellation/removal cases run in `v1` and `extended`. Each run snapshots its
model/config inputs before checking. See the [stage evidence](../evidence/tla/README.md)
for measured resources, failures and results.

The [runner](../../scripts/check_tla.py) requires normal completion and an empty
queue before reporting a passing exhaustive check. A known counterexample must
have the expected invariant, a usable trace and matching final-state predicates,
plus an ordered prefix where the registry requires it.
Parser failures, unrelated violations, deadlocks, missing completion, checksum
errors and resource exhaustion do not count as reproducing a finding. Expected
witnesses and deliberate mutations are labeled separately from product
findings in the registry.

TLC v1.7.4 does not implement the newer `-dumpTrace json` flag. The runner
preserves its text counterexample and normalizes state numbers, action labels
and variables into JSON. `variables_tla` contains TLA text, including records
and sequences; these values are not represented as decoded mathematical JSON.
Actual checking uses one worker, breadth-first exploration, fingerprint 0 and
seed 1, with no simulation or symmetry reduction. TLC uses fingerprints; the
runner retains its collision probability estimates. A completed bounded check
is conditional on that checker limitation. Optional coverage instrumentation is
disabled: pinned TLC exhausted its heap before initial-state computation on
recursive GoalLog operators when that instrumentation was enabled. Named
witnesses check reachability separately.

Full logs, input snapshots, checker state and result JSON live under ignored
`output/tla/runs/`. Useful summaries and selected traces belong in tracked
research evidence. No real identities, credentials, invitations or daemon homes
are used in any specification or fixture.

## Baseline and evidence boundaries

The initial source baseline is `86980594acee708b2f9bf303afc9ee636dc55337`.
The [historical property map](property-map.md) and [version-1 map](version-1.md)
distinguish intended rules, implementation seams, bounded models and limits. The tool pins came from the
[TLC release](https://github.com/tlaplus/tlaplus/releases/tag/v1.7.4) and
[Temurin release](https://github.com/adoptium/temurin21-binaries/releases/tag/jdk-21.0.8%2B9).
The JDK hashes agree with the publisher's GitHub release asset digests. The TLC
SHA-256 was measured from the downloaded release jar.

[Bootstrap CI](../../.github/workflows/tla-bootstrap.yml) runs runner tests and
the two real TLC fixtures on Ubuntu 24.04. No remote CI run or Linux execution has been verified in this task. This limitation
does not qualify Linux by inference. The Stage 4 full model CI gate remains
separate from this bootstrap job.

Review and preserve current-rule counterexamples before considering semantic
changes. Passing a proposed model never certifies the current implementation.
Source fixes, replication/retention models, storage flush models, liveness and
deductive proofs are outside Stages 0 and 1.

## Historical models and Rust mapping

[GoalLog.tla](GoalLog.tla) checks incremental projection against canonical replay
for finite authenticated transcripts. Safety families vary event delivery order,
gaps and duplicates, including forks, cancellation, reassignment and
removal/readmission. Scripted cases preserve the reviewed causal prefixes for
IR-5 and IR-12. Desired acceptance permanence and display binding fail under
the modeled version-0 rules. Upstream corrects those runtime cases; passing
historical invariants does not verify the version-1 fixes.

[Sessions.tla](Sessions.tla) models one local assignment, two principals, two
sessions, three generations, three events and two request keys. It checks
binding, local claim fencing, authorization, idempotency and uncertain atomic
commit outcomes. Directed witnesses reach A-to-B-to-A stale-write rejection,
restart/replay, both failed-commit outcomes, cancellation and principal refusal.
The mutation configuration disables generation checking and must produce a
stale signed write. Generation/holder fields attached to abstract events are
signing-time audit metadata, not fields of Rust wire events.

Three deterministic Rust fixtures replay IR-5, IR-12 and the session takeover/
retry/reopen scenario in [goal tests](../../crates/locust-core/src/goal/tests.rs)
and [session tests](../../crates/locust-core/src/node/tests/formal.rs).
This begins trace-to-Rust mapping with synthetic identities and MemStore; it
is not a generic trace importer or a proof of refinement. The original Stage 1
implementation changed no runtime behavior. After upstream
integration, these Rust fixtures assert version-1 behavior; their outcomes no
longer match the historical GoalLog findings. The [property map](property-map.md)
records omitted cases and the shared GoalLog transition algebra.
