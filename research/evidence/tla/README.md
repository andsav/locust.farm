# TLA+ Stages 0 and 1 evidence

**Organization cutover:** current protocol/API-2 checks are in the
[organization verification record](organization/README.md). Executable protocol-0/1
models and configurations were removed; records below remain historical and their
model maps link to pinned pre-cutover source. They do not qualify protocol 2.

**Historical version-1 revision:** see the [historical mapping](../../tla/version-1.md) and
[verification record](version-1.json). The original records below remain
historical version-0 evidence.

Date: 2026-10-03; runs continued into 2026-10-04 UTC. **Status: implemented and
locally checked on macOS Arm64. Linux bootstrap CI is configured but unobserved.**
These are bounded model checks and concrete Rust trace fixtures, not a proof of
the implementation. Start with the [model guide](../../tla/README.md),
[property map](../../tla/property-map.md) and [plan](../../../docs/tla-verification-plan.md).

**Integration update, 2026-10-04:** upstream `d253a07` introduces protocol/API
version 1 and corrects the two goal findings reproduced below. These retained
results and their hashes describe the version-0 baseline; they are not a claim
about the merged implementation. The models remain historical. Rust transcript
fixtures now assert corrected version-1 behavior. See the
[impact assessment](../../tla/upstream-impact-2026-10-04.md) and
[integration record](upstream-integration-2026-10-04.json) for the new checks.

## What was delivered

Stage 0 supplies the rule inventory, private pinned TLC/JDK cache, standard-library
runner, completion/violation/deadline checks, frozen inputs, runner fixtures and
Ubuntu bootstrap workflow. Stage 1 supplies GoalLog and Sessions, 26 model
configurations plus two runner fixtures, retained findings and action witnesses,
a deliberate generation-check mutation, and three deterministic Rust fixtures.
The bootstrap workflow has not run remotely; that Stage 0 qualification remains
pending. Full model CI remains Stage 4 work.

The initial source was `86980594acee708b2f9bf303afc9ee636dc55337`. Official Stage 1
runs use parent `9234f88` plus the recorded working-tree models/tests. Their
source and executed-input SHA-256 values identify the actual checked bytes;
read the dirty state as recorded, rather than treating a parent commit as the
complete checked tree. Every executed model/config, mapped Rust path, runner and
manifest hash was compared with the final files before committing.

## Reproduce and interpret

Use Python 3.12+ and the [pinned manifest](../../tla/toolchain.json): TLC v1.7.4
(TLC2 2.19, revision 5a47802), isolated Temurin 21.0.8+9. Both the archive and
extracted Java files are verified. macOS archive/JAR acquisition and real
execution were observed; the Linux archive is pinned but has not been run here.

```sh
python3 scripts/check_tla.py --bootstrap --suite fixtures
python3 scripts/check_tla.py --suite fast
python3 scripts/check_tla.py --suite extended
```

All runs use one worker, breadth-first exploration, fingerprint 0 and seed 1.
There is no simulation, symmetry reduction, state constraint, fairness or
liveness property. Fast uses a 4096 MB heap limit; the separately checked two
large goal cases used 1024 MB. Each large case has a 900-second deadline.
TLC uses state fingerprints, so its reported collision estimates are retained
as a checker limitation, not a proof of zero missed states. Text state values
are normalized losslessly as `variables_tla`, without pretending to decode all
TLA values into mathematical JSON.

[Stage 0 fixtures](stage0-fixtures.json) record the first committed runner's
completed fixture and expected invariant violation. [Stage 1 results](stage1-results.json)
record the final suite and additional large cases: tools, commands, deadlines,
source/input hashes, statuses, generated/distinct states and elapsed time.
Raw logs, input copies and TLC state databases remain under ignored
`output/tla/runs/`; their paths/hashes are retained in the summaries. Selected
full counterexample/witness states are tracked under `traces/`.

| Case | Outcome | Generated / distinct | Seconds |
|---|---|---:|---:|
| `fixture-pass` | complete | 5 / 4 | 0.86 |
| `fixture-violation` | expected runner-fixture | 3 / 3 | 0.77 |
| `goal-authority-anchor` | complete | 31,239 / 5,312 | 12.14 |
| `goal-broken-anchor` | complete | 5,163 / 1,055 | 2.33 |
| `goal-cancel-accept` | complete | 31,221 / 5,308 | 11.36 |
| `goal-cancel-reassign` | complete | 409,798 / 55,538 | 200.32 |
| `goal-canonical-order` | complete | 31,655 / 5,410 | 12.18 |
| `goal-coordinator-fork` | complete | 12,832 / 2,383 | 3.90 |
| `goal-ir12` | expected finding | 17 / 10 | 0.93 |
| `goal-ir12-current` | complete | 20 / 10 | 0.96 |
| `goal-ir5` | expected finding | 39 / 21 | 1.20 |
| `goal-ir5-current` | complete | 44 / 22 | 1.16 |
| `goal-member-fork` | complete | 12,827 / 2,382 | 4.32 |
| `goal-removal-readmission` | complete | 409,780 / 55,535 | 198.96 |
| `goal-safety` | complete | 5,163 / 1,055 | 2.30 |
| `goal-witness-acceptance` | expected witness | 15 / 9 | 0.96 |
| `goal-witness-append` | expected witness | 7 / 5 | 0.89 |
| `goal-witness-cancellation` | expected witness | 13 / 8 | 0.96 |
| `goal-witness-fork` | expected witness | 17 / 10 | 0.95 |
| `goal-witness-removal` | expected witness | 15 / 9 | 1.07 |
| `session-aba-witness` | expected witness | 8 / 7 | 0.82 |
| `session-cancellation-witness` | expected witness | 7 / 6 | 0.81 |
| `session-failure-after-witness` | expected witness | 12 / 10 | 0.83 |
| `session-failure-before-witness` | expected witness | 12 / 10 | 0.83 |
| `session-generation-mutation` | expected mutation | 8 / 7 | 0.84 |
| `session-principal-witness` | expected witness | 5 / 4 | 0.84 |
| `session-restart-replay-witness` | expected witness | 11 / 10 | 0.83 |
| `session-safety` | complete | 30,078,885 / 2,568,001 | 320.54 |

A named expected violation means different things in different cases:

- **Findings:** [IR-5 trace](traces/goal-ir5.json) loses two of three accepted
  heads when event 20 forks the middle proposal. [IR-12 trace](traces/goal-ir12.json)
  displays the later submitted result while preserving the accepted result.
  Both desired guarantees fail in the historical version-0 model. Upstream
  version 1 corrects these runtime cases; revised formal checks remain pending.
- **Witnesses:** intentionally negated reachability properties fail only after
  the requested action is observable. Traces reach acceptance, cancellation,
  removal, fork detection, incremental append, A-B-A stale-write rejection,
  restart/replay, principal refusal and recovery from both atomic failure outcomes.
  Their violations establish reachability, not product defects or liveness.
- **Mutation:** [generation mutation](traces/session-generation-mutation.json)
  deliberately disables generation checking and authors a stale A1 request
  under A3. It must violate `StaleWritesCannotAuthor` with both diagnostic
  flags true. This is a faulty specification variant, not a Rust finding.

An additional [ordering mutation patch](incremental-ordering-probe.patch) and
[exploratory result](incremental-ordering-probe.json) remove the incremental
ordering guard. They reproduce `IncrementalEqualsReplay` with delivery
1,2,3,6,4: incremental task order becomes `<<6,4>>`, canonical order `<<4,6>>`.
That probe is separate from the 28 registered cases and preserves a useful
adequacy check; the production model retains the guard.

## Rust correspondence and review

At the original version-0 checkpoint, `locust-core` passed formatting, Clippy
with warnings denied, and all 80 tests. Its original fixtures were:

- `tla_ir5_member_fork_rolls_back_dependent_accepted_heads` and
  `tla_ir12_later_submission_changes_display_without_changing_acceptance` in
  [goal tests](../../../crates/locust-core/src/goal/tests.rs). Signed symbolic
  events map directly to the model. IR-12 checks scrambled/duplicate delivery
  and MemStore reopen; IR-5 checks reverse delivery. Both compare incremental
  projection with full replay and characterize version-0 behavior. The merged
  tests retain these transcripts with corrected version-1 assertions and names.
- `tla_sessions_aba_delayed_write_and_idempotent_retry_survive_reopen` in
  [formal tests](../../../crates/locust-core/src/node/tests/formal.rs). Engine
  rejects stale A1/B2 writes, replays a committed keyed success after A3 and
  reopen, and permits a previously refused key to commit once under A3.

These are hand-mapped executions, not a generic trace importer or a proof of
Rust refinement. Runtime logic was not changed. The complete script test suite
passes 79 tests, including 16 runner tests; documentation/link checks were run
on staged documents. [Verification record](verification.json) retains commands,
results, source hashes and independent review boundaries.

Independent review compared both models with their mapped source, reviewed
runner classification/input integrity, and identified limits documented in the
property map: fixed transcripts and key order, absent removal cutoff only,
shared GoalLog transition algebra and a partial acceptance invariant, conservative
append coverage, and atomic local storage. Sessions does not model a distributed
lease; generation fields on abstract events are signing-time audit metadata.

## Failures, limitations and deferred work

[Development failures](development-failures.json) preserve actual unsuccessful
checks and their disposition. They include coverage-enabled Java exhaustion,
an overly permissive model monitor caused by Boolean-expression precedence,
strict rejection of that mutation trace, and intentionally incomplete checks.
The monitor was corrected with parentheses; the final model retains the stronger
requirements and was rechecked. Coverage instrumentation was disabled after the
same model completed without it; named witness configurations supply reachability.
The TLC `-dumpTrace json` option is unavailable at this pin, so text traces are
preserved instead. Parser errors, wrong hashes, unrelated violations, deadlocks,
truncation and deadlines are also covered by runner refusal tests.

Docker's daemon is unavailable locally and no remote bootstrap CI run has been
observed. Linux execution is unqualified. Replication/catch-up, retention,
flush/power-loss recovery, full model CI, arbitrary parameters and deductive
proofs remain later stages. Upstream implements fixes for IR-6, IR-9, IR-13 and
the blob findings; their formal verification remains outside these models.
No third physical Mac was needed. The existing M2 daemon, private state,
credentials and test goal were left in place; no new goal or client integration
was created, and this work does not qualify the physical two-Mac run.
