# Organization protocol model verification evidence

Historical record, 2026-10-04. **All 54 cases registered then matched their expected outcomes.**
This comprises 19 completed bounded safety cases, 22 requested reachability
counterexamples, 11 deliberate mutation counterexamples and two runner fixtures.
An expected counterexample establishes its named scenario or broken mutation;
it is not passing safety verification of the violated property.

The modeled source is `c88e3bc960de79eb990b3185a653bd982e587ed6`.
[Fast-suite results](fast.json) and [general action-system results](action-safety.json)
record actual checkout commit/dirty paths, source hashes, all model/configuration
hashes, exact configuration text, tool pins, commands, fingerprint estimates,
counts, traces and resource observations. Both runs report unchanged checked
source hashes from start to finish. Their union is exactly the registry at that date;
there are no skipped registered cases. Historical/parallel checkout work is
recorded rather than represented as a pristine release checkout.

The complete [raw checker output](checker-output.txt), [machine summary](summary.json)
and [Python checker/test output](python-tests.txt) are retained. The Python suite
completed 192 tests; the focused TLA runner suite completed 21 tests, including
explicit timeout failure and absence of an implicit deadline. No Rust source was
changed in this formal-model change, so no new Rust rebuild is claimed here.
The baseline engine's separate Rust verification is recorded in its commit.

Reproduction commands (Python 3.12+, pinned private tools already bootstrapped):

```sh
python3 scripts/check_tla.py --suite fast
python3 scripts/check_tla.py --case attempt-single-safety --case attempt-independent-safety --case effects-safety
python3 -m unittest discover -s scripts/tests
```

TLC used one worker, breadth-first search, fingerprint 0 and seed 1, with a
4096-MB configured Java heap. Neither run requested a wall-time deadline. Tool
identity was verified against pinned archives on macOS Arm64. There was no
simulation, symmetry reduction, remote CI qualification, fairness theorem or
proof of arbitrary-size behavior. See the [model/property map](../../../tla/organization.md)
for exact finite bounds and implementation/abstraction boundaries.

The general session cases exhausted 149,201 (one attempt) and 174,761 (independent
attempts) distinct states. The effect action system exhausted 3,643 states. The
largest organization transcript exhausted 2,048 held subsets. Safety queues were
empty at normal completion. Witness/mutation runs terminate at their registered
counterexample; their remaining queues are not represented as exhaustive safety
results.

## Case outcomes

| Case | Expected outcome | Distinct states | Seconds |
| --- | --- | --- | --- |
| `fixture-pass` | complete | 4 | 0.705 |
| `fixture-violation` | counterexample | 3 | 0.680 |
| `organization-base` | complete | 64 | 1.722 |
| `organization-taskless` | complete | 4 | 0.738 |
| `organization-fork` | complete | 128 | 2.781 |
| `organization-review-fork` | complete | 128 | 3.117 |
| `organization-scope-conflict` | complete | 128 | 3.440 |
| `organization-authority-fork` | complete | 128 | 3.050 |
| `organization-duplicate-review` | complete | 32 | 1.177 |
| `organization-rule-change` | complete | 32 | 1.194 |
| `organization-cutoff` | complete | 512 | 17.051 |
| `organization-remove-empty` | complete | 256 | 6.252 |
| `organization-governance-fork` | complete | 128 | 1.834 |
| `organization-forged-selection` | complete | 32 | 1.046 |
| `organization-wrong-scope` | complete | 64 | 1.480 |
| `organization-two-scopes` | complete | 1,024 | 28.485 |
| `organization-incompatible` | complete | 2,048 | 64.454 |
| `organization-missing-definition` | complete | 128 | 2.329 |
| `organization-taskless-witness` | counterexample | 4 | 0.742 |
| `organization-fork-witness` | counterexample | 122 | 2.183 |
| `organization-review-fork-witness` | counterexample | 122 | 2.327 |
| `organization-scope-conflict-witness` | counterexample | 28 | 0.863 |
| `organization-two-scopes-witness` | counterexample | 1,024 | 27.827 |
| `organization-incompatible-witness` | counterexample | 2,048 | 64.521 |
| `organization-missing-definition-witness` | counterexample | 122 | 1.885 |
| `organization-cutoff-witness` | counterexample | 506 | 15.335 |
| `organization-remove-empty-witness` | counterexample | 92 | 1.241 |
| `organization-pin-mutation` | counterexample | 122 | 1.862 |
| `organization-isolation-mutation` | counterexample | 639 | 8.589 |
| `organization-cutoff-mutation` | counterexample | 221 | 2.478 |
| `organization-distinct-mutation` | counterexample | 32 | 1.040 |
| `organization-scope-mutation` | counterexample | 64 | 1.364 |
| `organization-authority-mutation` | counterexample | 32 | 1.077 |
| `organization-rules-mutation` | counterexample | 32 | 1.015 |
| `organization-definition-mutation` | counterexample | 2 | 0.739 |
| `attempt-aba-witness` | counterexample | 7 | 0.726 |
| `attempt-replay-witness` | counterexample | 10 | 0.718 |
| `attempt-failure-before-witness` | counterexample | 10 | 0.722 |
| `attempt-failure-after-witness` | counterexample | 10 | 0.719 |
| `attempt-cancel-witness` | counterexample | 6 | 0.722 |
| `attempt-principal-witness` | counterexample | 4 | 0.714 |
| `attempt-independent-witness` | counterexample | 4 | 0.710 |
| `attempt-permissions-witness` | counterexample | 5 | 0.717 |
| `attempt-generation-mutation` | counterexample | 7 | 0.728 |
| `effects-ack-witness` | counterexample | 5 | 0.693 |
| `effects-duplicate-witness` | counterexample | 6 | 0.698 |
| `effects-failure-before-witness` | counterexample | 5 | 0.723 |
| `effects-failure-after-witness` | counterexample | 5 | 0.705 |
| `effects-retraction-witness` | counterexample | 5 | 0.708 |
| `effects-duplicate-mutation` | counterexample | 4 | 0.702 |
| `effects-ack-mutation` | counterexample | 5 | 0.669 |
| `attempt-single-safety` | complete | 149,201 | 11.209 |
| `attempt-independent-safety` | complete | 174,761 | 13.903 |
| `effects-safety` | complete | 3,643 | 0.823 |


## Phase 4 roles, model-first checks (2026-10-06)

The [seven-case role gate](phase4-role-gate.json) matched all three new safety
scenarios, three requested witnesses and the deliberately broken own-anchor
lookup. It completed before any Phase 4 Rust code was edited. The model and
configuration hashes identify the actual inputs; its modeled source baseline
is K1 commit `a6664a15682534bdaa94ac149a759b2bea0a8ea3` with the explicitly
labeled Phase 4 amendment, protocol/API 7. A completed Phase 4 implementation
commit cannot be named until it exists; the case registry will be pinned to it
in a follow-up evidence commit.

The [full model-first run](phase4-model-first.json) then matched **all 42 case
outcomes**: 21 completed safety checks, 12 reachability witnesses and nine
expected mutation violations. This preserved all 35 previous outcomes. Its
checker process nevertheless exited 1 because concurrent Phase 4 Rust edits
changed tracked source hashes while it ran. The frozen formal inputs did not
change. This run is evidence for the model outcomes, not a passing stable-source
final gate; the stable-tree rerun below supplies that gate.
No timeout was requested and none of these cases timed out.

The role-change and lead-change safety cases each exhaust 128 distinct held
subsets; role-removal exhausts 64. The anchor mutation reaches an ordinary
review by identity 4 signed before it held the reviewer role, with the role
change present. That is exactly `RoleHeldAtAnchor`'s required counterexample.
Neither these checks nor the older results establish Rust refinement,
unbounded behavior or transport liveness. Review rejects, unpinned completion,
only-member completion, opinions, admission-carried roles and first files are
outside the formal subset, as the [property map](../../../tla/organization.md)
and [workspace boundary](../../../tla/workspace.md) describe.


### Final stable-source run

The [final Phase 4 run](phase4.json),
`output/tla/runs/20261006T203500Z-6f425b43`, completed with checker exit 0:
all **42 cases matched**, no timeout, and `source_changed_during_run: false`.
The 21 safety cases exhausted their queues; the 12 witnesses and nine deliberate
mutations reached their registered violations and required trace predicates.
The same frozen organization model hash was checked before Rust implementation
and in this final run. The per-case source-baseline pin is updated to the completed
implementation commit separately; this retained record preserves the exact
baseline and checkout hashes observed when the checker ran.

The registry now pins all 42 Organization cases to implementation commit
`8c086c1eb72ffa5ad0572777c8944d8d66b5c7ee`, with status
`current-organization-role-holder-subset`. All 100 source hashes retained in
the final run were compared with that committed tree and match. This pin
changes metadata only; the model, configurations, expected outcomes and
retained run are unchanged. Other model suites keep their existing baselines.


### Review corrections, 2026-10-06

The review fix changes only the header comment in `Organization.tla` to name
its Phase 4 protocol/API 7 subset. This changes the file hash; the retained
Phase 4 runs above still describe the earlier, semantically identical file.
The [property map](../../../tla/organization.md) now names the current Rust
tests and explicitly lists the unmodeled role-start, cardinality, validity and
close/reopen behavior. No formal coverage is added by these wording changes.

The [R4 fix verification](r4-fixes.json), run
`output/r4-rules-verification/output/tla/runs/20261007T002238Z-56bb2d91`,
completed with checker exit 0. All **42 cases matched**: 21 safety checks,
12 reachability witnesses and nine deliberate mutations. There were no
timeouts and `source_changed_during_run` is false.

This run used an isolated source snapshot while R5 was being edited in the
primary checkout. Its metadata records the primary checkout's then-current
`a9992d8` and dirty status; the recorded hashes describe the actual snapshot.
All 100 source hashes were independently compared with the completed R4
source commit `ec1e07882c237d95069ff1ace5b0015154cf8468` and match. The model
semantics, configurations and case registry are unchanged; the new model hash
includes the corrected header comment. The registry's Phase 4 baseline remains
`8c086c1` and is not a claim that it includes these later Rust fixes.


## G1 review fixes to the E1 and E2 models, 2026-10-07

The earlier records above predate E1's `end` and E2's `leave`. These three
runs check the models after the fixes for findings 16 to 22 of the
[G1 review](../../../v2-phase-g1-review-2026-10-07.md). All three ran on the
clean commit `e995af53679c22d1e966817707b6eb43a0f0b7e7` (empty dirty status,
`source_changed_during_run: false`, checker exit 0). That commit is the fix
branch before it was rebased onto `main` to land; its models, configurations,
case registry and runner are byte for byte those of the landed commits, and the
Rust source hashes it records are those of its base `e350a85`, not of the
landed tree. The runs used the pinned TLC
1.7.4 and Temurin 21.0.8+9 on macOS Arm64, one worker, no timeout and a
4096-MB heap.

| Command | Run | Outcome |
| --- | --- | --- |
| `python3.13 scripts/check_tla.py --suite organization` | [g1-review-organization.json](g1-review-organization.json), `20261007T104709Z-41ff4fca` | All **57 cases matched**: 31 completed safety checks, 17 reachability witnesses, 9 deliberate mutations. |
| `python3.13 scripts/check_tla.py --suite restore` | [g1-review-restore.json](g1-review-restore.json), `20261007T111600Z-e0cbe6b3` | All **33 cases matched**: 19 completed (including the two extended cases), 14 expected counterexamples. |
| `python3.13 scripts/check_tla.py --suite fast` | [g1-review-fast-summary.json](g1-review-fast-summary.json), `20261007T112509Z-9a02b33c` | All **163 cases matched**: 76 completed, 87 expected counterexamples. It holds the 57 organization cases and 31 of the 33 restore cases. The summary keeps each case's outcome, counts, time and input hashes; its traces are omitted, and the two runs above keep theirs. |

What the cases now check:

- The six E1 cases and four E2 cases each check `ScenarioOutcome`, the outcome
  the case is named for, in every reachable held set. Each E2 case also has a
  reachability witness. The largest new case, `organization-end-after-readmission`,
  exhausts 1,024 held subsets.
- `organization-leave-fork` forks identity 2's log below its leave;
  `organization-leave-readmission` admits identity 2 again; the leave rule
  follows Rust and E2's plan.
- `restore-leave` runs to its second removal, and `restore-leave-witness`
  reaches that state (phase 11, two removals), so `RemovalMatchesBefore`
  compares two records. The restore model no longer resets `didRemove` on a
  restore, so a second removal needs `Continue`.

Checked by hand before these runs, and not retained as runs: putting back the
old leave clause in `Eligible` makes `ScenarioOutcome` fail in leave-removal,
leave-readmission and leave-host-agent. Dropping `RetainedByCutoff` makes it
fail in leave-fork and end-after-readmission. Putting `settle` back into the
leave trace, or moving the first removal before the exchange with the leaver,
makes `restore-leave-witness` fail to reach its state.
