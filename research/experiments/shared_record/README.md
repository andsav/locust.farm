# Shared-record information-policy harness

Status: implemented with offline verification, 12 October 2026. No paid
model calls were made for the initial implementation. The later authorized
[$30 exploratory execution](../../shared-record-information-policy-nightly-2026-10-12.md)
completed calibration and policy development, then stopped before scored
execution at the evaluator-noise readiness gate. This is the current runner for
the [information-policy plan](../../../docs/shared-record-information-policy-plan.md).
The [October 8 protocol](historical-protocol-2026-10-08.md) and its
[results](../../shared-record-study-results-2026-10-08.md) are historical evidence;
reproduce that runtime at its recorded commits, not with this replacement.

## Current method

The scored study has five arms: continued Luna solo (S), three uninterrupted
independent attempts (I), matched private review (C), selected peer policy (P),
and frontier solo (R). Model identities, prices and usable allowances require
funded development; none is qualified by the scripted fixtures.

The development policies are early full (E, 25% stage allocation), delayed
full (D, 50%), and early filtered (F, 25%). The filter retains failed and
non-improving public attempts; it excludes the first valid baseline and strict
improvements. Such records can still convey useful methods. A recipient needs
its own fully public-valid candidate to receive peers. Otherwise it receives
exactly the private-review input and stays assigned to P. Empty packets stay
empty. E/F reuse their exact early prefix; C/P fork exact histories and are
charged the complete prefix individually while the study bills it once.

Candidate IDs are opaque `candidate-` UUID4 identifiers, generated once per
evaluated attempt and retained unchanged in forks, parent links, source
references and private-review histories. Repeated submissions remain distinct
attempts. Internal trial, worker and accounting IDs stay auditable in the
archive without entering model-facing candidate objects. The
[runner tests](test_runner.py) check identity stability and exact source/parent
authorization; the [request tests](test_interruptions.py) and scripted replay
inspect dispatched payloads for leaked internal execution labels.

This controls avoidable arm-name cues; it does not blind workers to receiving
peer evidence. Truthful worker attribution and the policy field remain visible.
Peer context length, reading costs and later choices are part of the treatment.
Frozen studies made before this ID change retain their original identifiers
and runtime hashes; these checks do not retroactively qualify their labels.

Each branch uses an isolated real Locust goal. The harness publishes frozen
metadata and full sources as signed contributions. Model-visible source
retrieval is restricted to that recipient's delivered candidate IDs. Documents
are losslessly chunked to respect the actual 1 MiB signed payload limit,
including sealing overhead; neither sources nor tool output are truncated.
The daemon enforces goal isolation. Own-versus-peer filtering within a goal is
enforced by the harness tool boundary, not a new daemon authorization rule.

Live evaluation sees public instances only. The selector requires validity on
every public repetition, then minimizes the unrounded public score, then source
hash. No valid deliverable selects the fixed fallback. All hidden evaluation
runs separately, after every assigned output and selector is sealed. Failures,
non-triggered exposure and no-candidate observations remain in the analysis.
An explicit `public_fixtures` map in the spec appends declared per-family
validity instances to the public pack and freezes their digest. They never
enter the hidden pack. The
[historical coloring regression](selector_regression.py) supplies a deterministic
K12 fixture exposing the old source defect; include qualified fixtures in the
paid study's concrete selector configuration.

## Files and enforced boundaries

| File | Responsibility |
| --- | --- |
| [protocol.py](protocol.py) | Explicit pack/repetition/attempt identities, full prompts, source/helper/interpreter/binary provenance, immutable manifest receipt and completed-output seal |
| [run.py](run.py) | Public-only agent tools, uninterrupted I, stage boundaries, C/P forks, private continuation, fail-preserving finalization and scripted daemon replay |
| [record.py](record.py) | Deterministic eligibility, real signed delivery, complete source retrieval and branch/recipient isolation |
| [luna.py](luna.py) | Explicit model/rates, durable dispatch reservations, cache-aware settlement, prefix attribution, full request/response archives and default-denied live HTTP |
| [campaign.py](campaign.py) | Locked campaign allocations across separate studies, sealed settlement and retained uncertain charges |
| [problems.py](problems.py), [scoring.py](scoring.py) | Seeded packs, validators, fallback costs, repeated sandbox evaluation and public-only selection |
| [analyze.py](analyze.py) | Offline hidden scoring, archive-only analysis, failures, pending costs, paired contrasts and diagnostics |

There is no old/new runner switch or historical result conversion. The tests
retain still-required generator, sandbox and accounting invariants while
replacing tests for superseded behavior.

## Offline verification

Python 3.12 or newer and macOS `sandbox-exec` are required by the current local
sandbox and daemon helpers. No API key is needed. From this directory:

```sh
/opt/homebrew/bin/python3.12 -m unittest discover -p 'test_*.py'
/opt/homebrew/bin/python3.12 selector_regression.py --output ../../../output/coloring-regression.json
/opt/homebrew/bin/python3.12 run.py replay \
  --output ../../../output/shared-record-policy-replay \
  --binary ../../../target/debug/locust
```

Use a new empty output directory for each replay. The replay uses synthetic
model responses and accounting values through the same runner and ledger. It
covers E/D/F and their private controls, both solo configurations, independent
attempts, repeated packs, invalid initial candidates, empty private filtered
packets and an injected uncertain request failure. Its synthetic scores are
not collaboration-quality evidence. It also checks opaque candidate identities
across archived copies and the absence of internal arm/prefix identifiers in
every dispatched scripted request. The compiled binary is privately copied
and hashed; the replay neither rebuilds nor changes production Rust code.

Offline analysis can be reproduced from the completed archive:

```sh
/opt/homebrew/bin/python3.12 analyze.py \
  --output ../../../output/shared-record-policy-replay
```

`--score-hidden` explicitly performs offline scoring if it is still outstanding;
it never invokes a model. Completed source evaluations are journaled. An
interrupted source schedule blocks automatic re-evaluation and requires explicit
recovery; it cannot silently obtain a different noisy outcome. `--evidence DIR`
copies the sealed archive and completed scores to an empty evidence directory.
The packager copies only the declared evidence files and excludes daemon state
and credentials. Raw experiment outputs remain local-only and are excluded
from Git; tracked summaries, Markdown reports and audit helpers are indexed in
the [offline evidence](../../evidence/shared-record-information-policy-offline-2026-10-12/README.md)
and [paid-campaign evidence](../../evidence/shared-record-night-2026-10-12/README.md)
directories. Archive reproduction requires the corresponding local raw files.

## Freeze and execution

An explicit JSON spec supplies study ID/seed/stage, models, budgets, arms,
continuation procedures, block assignments, instance counts, evaluator schedule
and analysis settings. `scripted_spec()` in [run.py](run.py) is an executable
fixture, not a paid-study template or calibrated configuration. A development
spec must replace its synthetic model/price/limit values and identify the
funded ceiling. A scored spec additionally freezes one D/F policy and records
selector, delivery, accounting, solo, precision and development-selection
readiness evidence.

```sh
/opt/homebrew/bin/python3.12 run.py freeze --spec concrete-spec.json \
  --output ../../../output/new-study --binary ../../../target/debug/locust
```

Freezing makes no model calls. It refuses an existing output directory. The
runner checks the separate manifest receipt and all runtime hashes before
execution, so editing a budget, policy, prompt, helper or binary cannot silently
continue an older study. The execution command requires `--allow-paid` and
qualified model/rate/limit evidence in the manifest. **No paid command was run
for this implementation.**

The runner supports a documented model-context bound or the provider's exact
full-input counter. The latter counts the same instructions, history, reasoning
configuration and tool definitions before reserving generation cost. Count
requests and responses are archived, including counts that do not lead to a
generation. There is no byte/token heuristic or silent fallback. Input,
cache-read, cache-write, long-context and output rates are frozen explicitly.
An accounting-bound violation, unexpected model identity or service tier halts
further study dispatch. Paid runs require a matching active campaign allocation;
separate directories cannot each claim the whole campaign allowance.
An explicitly cancelled, frozen study releases its cap only if it has no
execution marker, request or counter ledger, or output seal. Its identity and
original manifest remain recorded. Started studies require sealed settlement.
Responses requests stop before dispatch if their remaining context or budget
cannot fund the documented minimum of 16 output tokens.

Source/helper hashes, interpreter and binary identity must still match the
freeze. The recorded source commit remains provenance, while a later commit
may track the frozen manifest without changing any executed source.

Requests have no automatic retries. Pending and uncertain failed requests keep
their reservation; known charges use returned input/output/cache usage. The
recorded output limit is derived from remaining allowance and the declared
provider limit. There are no incidental turn caps, fixed output caps, history
deletion, forced-spend loops or source truncation.

## Interruption and evidence

An execution marker prevents an interrupted run from silently dispatching the
same work again. Completed sealed runs can be read again without calls. To
close an interrupted run without retrying or paying for missing work:

```sh
/opt/homebrew/bin/python3.12 run.py finalize-interrupted \
  --output ../../../output/new-study
```

Finalization retains partial candidates and dispatched reservations, assigns
fallbacks where there is no deliverable, and attributes completed prefix work
to every assigned C/P continuation, including one never launched. The original
study stays in the dataset. Any authorized repeat needs a separately identified
attempt and funding; it cannot replace the interrupted observation.

The archive retains full visible prompts/responses/tool results, frozen
candidate sources, source hashes and parent IDs, request IDs and costs, policy
receipts and withholding decisions, public selectors, offline hidden scores,
and the frozen analysis. Encrypted provider reasoning state may be retained for
exact continuation; no private credential is recorded. The analyzer reports
known charges separately from uncertain reservations and counts pending work at
its original stopping cut. A later settlement cannot retroactively make an
earlier candidate affordable.

The two scored contrasts are C−P and I−P, with fixed-family, pack-then-paired-
repetition bootstrap intervals. Other comparisons and lower-budget curves are
descriptive. Changes in disagreement, repairs and source lineage are diagnostics;
neither communication gains nor diversity as their cause nor RSI are established
by these offline checks.
An explicitly exploratory manifest, or fewer than two packs in any family,
prevents threshold-based confirmatory verdicts. Small-pack intervals stay
visible with their descriptive limitation.

See the [initial implementation verification note](../../shared-record-information-policy-verification-2026-10-12.md)
for its historical offline checks, and the
[nightly report](../../shared-record-information-policy-nightly-2026-10-12.md)
for the paid observations and final readiness stop.
