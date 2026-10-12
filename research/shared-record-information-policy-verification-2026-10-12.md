# Shared-record information policy: implementation and offline verification

Status: historical offline verification snapshot, 12 October 2026, before the
paid campaign. **No paid model calls were made for this verification.** At this
checkpoint the [experiment plan](../docs/shared-record-information-policy-plan.md)
had a runnable replacement harness, with model calibration, policy selection
and scored execution still outstanding. The later
[nightly report](shared-record-information-policy-nightly-2026-10-12.md)
records completed calibration and policy development, followed by the stop
before scored execution at the evaluator-noise readiness gate.

The implementation is in [shared_record](experiments/shared_record/README.md).
It replaces the October 8 runner in place; its
[historical protocol](experiments/shared_record/historical-protocol-2026-10-08.md)
and [outcomes](shared-record-study-results-2026-10-08.md) remain historical
research. The old protocol text is preserved verbatim at its new path.

## Implemented controls

- S/I/C/P/R and development E/D/F policies use explicit model, task-pack,
  repetition, attempt, budget and branch identities. I receives its complete
  uninterrupted B/3 allowance. C/P fork exact initial histories and use the
  same review instruction and tool definitions; peer evidence is the treatment.
- Public evaluation is the only live evaluator. Validity precedes unrounded
  score and source-hash tie-breaking. Hidden scores are computed only after all
  assigned live outputs and selectors are sealed. The hidden journal reuses
  completed evaluations and refuses to silently repeat an unresolved one.
- Frozen record packets are deterministic. F excludes first valid baselines
  and strict improvements. Invalid recipients receive the private packet,
  with no later exposure; empty packets remain empty. Signed source retrieval
  accepts only that recipient's eligible candidate IDs and never truncates.
- Manifests freeze complete initial prompts, tools, instances/digests, models,
  settings, source and transitive helper hashes, interpreter and binary identity.
  A separate configuration receipt detects edits even before the first launch.
  Resume checks runtime hashes and refuses interrupted redispatch.
- Ledger reservations precede dispatch. Shared prefix work is charged once to
  actual study billing and once to each consuming arm's attributed total.
  Pending and unknown failed requests remain reserved at their original cost
  cuts. Incorrect model identity or accounting bounds halt later dispatches.
- Explicit interruption finalization retains all assigned arms, available
  prefix/continuation candidates and pending requests. No-candidate arms keep
  their fallback. C/P latency includes its independent prefix critical path.
- Analysis retains paired blocks and fixed-family weighting, implements the
  prescribed D/F development tie rule, and uses 97.5% intervals for the two
  primary scored contrasts. Lower-budget curves and diversity remain
  descriptive. Scripted dollars are explicitly labeled synthetic.

These boundaries are exercised by [runner tests](experiments/shared_record/test_runner.py),
[interruption tests](experiments/shared_record/test_interruptions.py),
[record tests](experiments/shared_record/test_record.py),
[ledger tests](experiments/shared_record/test_luna.py), and
[analysis tests](experiments/shared_record/test_analysis.py). Within-goal
own/peer filtering is enforced by the harness's only model-visible tool
interface; branch isolation is enforced by actual daemon goals.

## Executed verification

From `research/experiments/shared_record`:

```sh
/opt/homebrew/bin/python3.12 -m unittest discover -p 'test_*.py'
```

**88 tests passed in 8.559 seconds.** This includes actual sandbox execution,
a CPU-overrun kill, exact filter eligibility, first-valid gating, unrounded
selection, source-hash ties, full outputs, immutable freeze/resume, prefix
attribution, historical pending-cost cuts, accounting failures and archive
reconstruction. The problem invariants from the earlier suite were retained;
tests for its superseded selector, spending and voluntary record protocol
were replaced.

The full runner replay used `run.replay` with `luna._http` and
`urllib.request.urlopen` patched to raise on use. It used the same runner,
sandbox, ledger and real production daemon, with synthetic provider responses.
Its assertions are implemented in
[verify_scripted_replay](experiments/shared_record/run.py).

| Observed replay property | Result |
| --- | ---: |
| Paired blocks: two fresh packs, two repetitions | 4 |
| Trial outputs: S/I/R plus E/D/F private/peer controls | 36 |
| Independent real daemon goals | 24 |
| Signed contribution receipts | 208 |
| Scripted provider requests | 306 |
| Invalid P recipients correctly kept private | 12 |
| Empty filtered deliveries retained | 4 |
| Injected uncertain request failures retained | 2 |
| Paid model calls | 0 |
| Packaged archive analysis equals original JSON | Pass |

Archive reconstruction ran with `scoring.score` patched to raise, so it could
not silently recompute candidate outcomes. The package does not require the
daemon or its private state. The local-only verification record,
`verification.json`, contains exact source/helper hashes and manifest/output
seals. Raw reports named here are excluded from Git; the tracked
[offline evidence index](evidence/shared-record-information-policy-offline-2026-10-12/README.md)
records their inventory and the retained summary. The full local
replay is disposable output at
`output/shared-record-policy-replay-2026-10-12-final`; its packaged copy is at
`output/shared-record-policy-replay-2026-10-12-final-archive`.

Both daemon checks used a private copy of the existing local binary with
SHA-256 `74e8a05a061489dda54ac931dd95704ee99b9577d0a24fed4e107dea9babf01c`.
A focused transport smoke additionally reconstructed a 1,440,000-byte Unicode
source from two signed source chunks, denied private-control access to a peer
source, and rejected a direct cross-goal inspect. Its local condensed receipt,
`large-source-smoke.json`, is retained. The final runner replay qualifies the
integrated code at this checkpoint;
the large-source smoke specifically exercises transport capacity and isolation.
No Rust source changed and no Rust workspace build/test result is claimed.

## Historical coloring regression

The archived shared-arm candidate `agent-1`, index 2, assumes too few initial
colors and indexes a ten-column counter with color 10. A complete graph K12
exposes this deterministically. The archived repair at index 5 returns a valid
12-color solution. Both receive normalized cost 1.0 on this fixture because
the defect receives fallback cost; a score/hash-only selector picks the broken
source, while the implemented validity-first selector chooses the repair in
either arrival order.

[selector_regression.py](experiments/shared_record/selector_regression.py)
embeds the exact archived sources and their provenance hashes, so no historical
runtime reader is needed. The standalone sandbox replay passed and its local
full report, `coloring-selector-regression.json`, is retained.
`public_fixtures` in a concrete spec can append these declared
instances to public selection only; a test verifies the hidden pack is unchanged.

K12 is a source-derived public validity fixture outside the sampled benchmark
population. It qualifies the known defect check, not arbitrary hidden-bug
detection, public-selector regret on new instances or forecasting quality.

## Readiness at this historical checkpoint

No real model's response behavior, current deployment name, rate table or
context limit was qualified here. Live execution still needs concrete funding,
verified model/rate/limit evidence, useful B and private continuation calibration,
public/hidden counts and evaluator-noise measurements, a D/F selection based on
development results, and a final powered or explicitly exploratory sample.
The paid spec must include qualified public validity fixtures or equivalent
recorded selector evidence. Readiness fields require evidence references;
their empirical sufficiency remains a review requirement, not a claim that
nonempty JSON strings prove readiness.

At this checkpoint the runner used a conservative declared model-context input
bound, which could reserve much more input cost than the request needed; an
exact full-input counter was not yet qualified or integrated. The later paid
campaign integrated and verified exact provider counts, as described in the
nightly report. The initial synthetic accounting tests alone did not establish
provider limits or count behavior.

Useful disagreements, repairs and copying can now be recorded and analyzed.
No communication advantage, diversity-mediated gain, frontier-model comparison
or recursive self-improvement result has been measured by this work.
