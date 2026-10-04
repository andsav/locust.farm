# Organization protocol model verification evidence

Date: 2026-10-04. **All 54 registered cases matched their expected outcomes.**
This comprises 19 completed bounded safety cases, 22 requested reachability
counterexamples, 11 deliberate mutation counterexamples and two runner fixtures.
An expected counterexample establishes its named scenario or broken mutation;
it is not passing safety verification of the violated property.

The modeled source is `c88e3bc960de79eb990b3185a653bd982e587ed6`.
[Fast-suite results](fast.json) and [general action-system results](action-safety.json)
record actual checkout commit/dirty paths, source hashes, all model/configuration
hashes, exact configuration text, tool pins, commands, fingerprint estimates,
counts, traces and resource observations. Both runs report unchanged checked
source hashes from start to finish. Their union is exactly the current registry;
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
