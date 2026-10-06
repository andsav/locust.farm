# Rust quality review and remediation

Status: implemented and source-verified, 2026-10-06. This note records the eleven
findings from the broad Rust review and the checks of their fixes. It does not
qualify a deployed or packaged release.

## Scope and baseline

The review covered production code across the eight Rust crates, with selective
test inspection and focused behavioral probes. Remediation began from `ae85baf`
with a clean working tree. That revision includes the separate task-creator fix
in `c5201dd`; the findings below are not regressions attributed to that fix.
Baseline formatting, strict workspace Clippy and the ordinary Rust suite passed:
911 tests passed and 14 were ignored. Green baseline checks did not cover the
newly reproduced cases.

The accepted [crate boundaries](../docs/crates.md) remain unchanged. This work
addresses specific correctness and performance defects rather than replacing
the runtime, changing wire formats or introducing compatibility paths.

## Findings and remediation

| Priority | Finding | Implementation and regression location |
| --- | --- | --- |
| P1 | Task revision used the contribution grant instead of the administration grant. | [Task requests](../crates/locust-core/src/node/requests/tasks.rs) |
| P1 | An editor save after composition could be adopted as an authorized preimage and silently overwritten. | [Tree planning](../crates/locust-workspace/src/tree.rs) |
| P1 | Reverse-ordered farm stage dependencies caused quadratic work before enrollment refusal, under the database permit. | [Snapshot validation](../crates/locust-proto/src/farm.rs), [farm service](../crates/locust-farm/src/lib.rs) |
| P2 | Removed-member cutoff ancestry was traversed separately for each retained event. | [Author-chain authorization](../crates/locust-core/src/goal/chain.rs) |
| P2 | Disposable layout probes synchronized every empty file and directory before deletion. | [Tree planning](../crates/locust-workspace/src/tree.rs), [filesystem primitives](../crates/locust-workspace/src/safe_fs.rs) |
| P2 | Update preparation rejected preserved opaque `.git` directories. | [Update transactions](../crates/locust-workspace/src/transaction.rs) |
| P2 | Historical contacts repeatedly received the first halt proof, starving later proofs. | [Sync driver](../crates/locust-core/src/sync/driver.rs) |
| P2 | Scope decisions embedded all approved contributions and could exceed the event-header limit. | [Task requests](../crates/locust-core/src/node/requests/tasks.rs) |
| P2 | Gallery reads did not apply ended-farm retention before returning snapshots. | [Farm service](../crates/locust-farm/src/lib.rs) |
| P2 | Publication suspension cleared an ended farm's original retention deadline. | [Farm service](../crates/locust-farm/src/lib.rs) |
| P2 | Bare relative package output paths failed directory synchronization after writing the file. | [Package files](../crates/locust/src/package.rs), [CLI regressions](../crates/locust/tests/cli.rs) |

The implementation was split into independent tracks and committed locally:

| Commit | Verified change |
| --- | --- |
| `0ac87c4` | Relative package output persistence |
| `43f03b4` | Task-revision grants and close/reopen evidence |
| `fa4a6ff` | Per-fold cutoff ancestry reuse |
| `ab388e9` | Workspace preimage safety, opaque Git preservation and disposable probes |
| `5e2864d` | Fair historical proof delivery and refusal/close handling |
| `67e8ac1` | Farm graph validation, admission, bounded validation and retention |

### Core authorization, evidence and replay

- Task revision uses the existing administrator helper, including its explicit
  owner-on-behalf exception. The [permission regression](../crates/locust-core/src/node/tests/permissions.rs)
  checks contribution-only refusal, administration-only success and owner action.
- Close/reopen decisions carry no extraneous contribution evidence. These
  decisions do not require a contribution witness; the proof builder still
  retains the context round and previous decision. The
  [lifecycle regression](../crates/locust-core/src/node/tests/lifecycle.rs) closes
  and reopens a task with 600 approved contributions, checks the header limit,
  and confirms those contributions remain readable.
- Cutoff ancestry is cached per admission within one fold. Cache hits borrow the
  ancestor set rather than cloning it, and rebuilding the chain invalidates the
  cache. [Goal regressions](../crates/locust-core/src/goal/tests.rs) count one
  traversal per removed tenure and check missing-ancestor arrival. Membership
  lookups retain the ordered set's logarithmic cost; the change removes repeated
  full traversal and copying rather than making all replay work linear.
- Historical-contact proof delivery rotates after definitive outcomes and
  retries the same proof after transport abort. Cursor state is discarded with
  the contact. [Driver regressions](../crates/locust-core/src/sync/tests/driver.rs)
  cover two proofs over repeated exchanges, aborted-delivery retry, and a received
  refusal followed by close before local finish, including a refusal that arrives
  while finish is already pending.

### Workspace planning and durable updates

The planner refuses a captured preimage whose bytes, executable bit or size
disagree with the composition inventory. Deterministic
[planning tests](../crates/locust-workspace/src/tree.rs) inject stale content and
mode without relying on thread timing.

Preserved directories must have been observed. A reserved Git basename is allowed
only for an opaque observed directory beneath an otherwise safe parent; paths
inside Git metadata remain forbidden. [Transaction tests](../crates/locust-workspace/src/transaction.rs)
exercise full updates beside `.git/config`, preserve metadata content, mode and
directory identity, and reject forged unsafe directory observations.

Disposable layout probes share exclusive no-follow creation with actual writes
but skip file/directory synchronization. Recovery artifacts and actual
replacements retain the durable path. [Filesystem tests](../crates/locust-workspace/src/safe_fs.rs)
and planning tests check sync-path counts and collision behavior. This is not a
power-loss test.

### Farm validation and retention

Stage cycle detection visits each stage and edge once after reference validation;
100,000-stage reverse-chain and cycle tests guard the adversarial graph shape.
Other identity/reference checks still use ordered sets.

Body limits and enrollment admission precede signature/snapshot validation.
Async validation runs on a blocking thread under its own semaphore, not the
database permit; cancellation cannot release capacity while that work continues.
The request is moved rather than copied. The database phase rechecks authority,
replay and expiration after validation, so passing a deadline during validation
cannot reopen an expired farm.

Public reads check expiration under the database lock, after waiting for capacity.
Suspension preserves an ended farm's closure timestamp, and expiration includes
suspended ended farms. Only a timely actual reopen clears the deadline.
[Farm regressions](../crates/locust-farm/src/lib.rs)
cover admission before validation, oversized inputs, queued-validation database
access, gallery and individual-read expiration after queuing, suspension/resume,
deadline crossing and exact receipt replay.

### Package outputs

Committed as `0ac87c4`.

Package publication now treats an empty parent path as the current directory,
matching the existing secret-file behavior. It still creates files exclusively,
sets the required mode, synchronizes the file and synchronizes its parent.

The CLI regressions exercise key generation and withdrawal signing with bare
relative filenames in isolated subprocess working directories. They verify
signatures, permissions and refusal to overwrite existing outputs without changing
the test process's working directory. Both tests first reproduced the original
`cannot persist release directory` failure, then passed with the fix.

## Performance observations

The review identified these local bottlenecks:

- A release-mode, 4,500-stage farm upload of roughly 246 KB took about 343 ms
  with reverse-ordered dependencies, versus 1.5 ms for ascending dependencies.
  Four concurrent rejected uploads delayed a health request by about 1.39 seconds.
- Development-artifact replay of 256, 512, 1,024 and 2,048 retained contributions
  after member removal took approximately 32, 115, 487 and 2,058 ms. Without the
  removal, the same sizes took roughly 4, 3, 5 and 10 ms.
- Planning an update with 1,001 resulting ordinary files issued 2,002 macOS
  full-file synchronizations for disposable probes and took about 4.2 seconds.
  A diagnostic experiment bypassing only those probe flushes took about 86 ms.

These are review-probe observations, not throughput guarantees or post-fix
benchmark results. Regression tests check traversal counts, adversarial graph
shape and durability paths without asserting latency thresholds.

The farm track reran the original local release-mode probe after its initial
fixes: the unenrolled reverse-chain upload returned 403 in roughly 4 ms, the
health request behind four such uploads had no measured millisecond delay,
the expired gallery was empty, and resuming an ended farm beyond its original
deadline failed. Subsequent integration added bounded validation concurrency
and a deadline recheck. No paired post-fix replay or filesystem timing campaign
has been run; their checks establish implementation behavior, not a capacity
claim.

## Verification

Final integrated source verification passed on macOS with pinned Rust 1.96.1:

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Passed |
| `cargo test --locked --workspace` | 944 passed, 14 ignored |
| `cargo build --locked -p locust -p locust-farm` | Passed |
| `python3.12 scripts/check_formations.py` | Contract, schema, example and conformance parity passed |
| `python3.12 scripts/check_documentation.py --binary target/debug/locust --timeout 60` | Four guide recipes passed |
| `python3.12 -m unittest discover -s scripts/tests` | 288 tests run successfully, including three skips |
| `python3 scripts/check_docs.py` | Documentation indexes and local links passed |

There are 33 more ordinary Rust tests than at the review baseline. The ignored
tests were not run as a group. The website was unchanged and its checks were not
rerun.

The package fix also passed a complete pre-commit run independently: 913 Rust
tests passed and 14 were ignored. Its focused CLI suite passed 33 tests and the
package unit suite passed eight tests.

### Review follow-ups and failed runs

The first integration binary build ran while parallel sync edits were incomplete
and failed on a missing `Dialed` initializer field. It passed once those edits
stabilized; the final implementation derives proof-delivery state instead of
retaining that extra field.

An integration pass then passed 942 Rust tests, with 14 ignores, plus formatting,
Clippy, builds, formation contracts and four guide recipes. Separate reuse and
efficiency reviews found no remaining actionable issues. Correctness review
identified two gaps still present in that passing version:

- Evidence-only refusal handling lost an observed refusal if the remote closed
  before local finish, preventing fair cursor advancement.
- Gallery maintenance could precede a wait for the database permit, allowing a
  deadline crossed during that wait to escape the read check.

Both new regressions first failed against that version. The refusal regression
observed `Aborted` instead of `Refused(NotAMember)`; the gallery regression returned
an expired entry. The fixes retain definitive refusals through teardown and move
expiration checks into the database critical section. Focused regressions, all
39 sync tests and all 39 farm tests then passed. The public-read regression also
covers individual reads. The final whole-workspace rerun is recorded above.

## Remaining qualification

Fixing these defects does not supply backup/restore, complete goal termination,
two-person/two-host qualification, Linux installation qualification, power-loss
testing or sustained production load evidence. The [project status](../docs/status.md)
and [workspace qualification limits](../docs/workspace.md) continue to apply.
