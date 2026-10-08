# Nous: profile transfer and communication

Status: **Luna run complete: 900/900 calls, 899 valid responses; exploratory results**.
The [2026-10-08 run report](results-2026-10-08.md) and
[completed response evidence](completed-evidence-2026-10-08.json) preserve all
900 responses and 903 attempts, including recovery from three network errors.
This is separate from the
[evidence-collaboration study](../../evidence-collaboration-study-design-2026-10-08.md).
It tests behavioral-profile injection in probabilistic forecasting, not retrieval
or multi-hop question answering.

## Question

Do the released Nous profiles improve forecasts compared with a neutral prompt?
Does direct structured injection behave differently from the released narrative
prompts? Does peer exchange amplify or erase those differences?

The primary source is Haowei Qian's
[Nous paper](https://arxiv.org/abs/2606.13038), with
[reproduction artifacts](https://github.com/WillChienT/nous-paper/blob/3ff0b996784cdc8f4db40210c0505371e667ee1e/artifacts/REPRODUCE.md)
pinned at `3ff0b996784cdc8f4db40210c0505371e667ee1e`.
The [existing independent arithmetic audit](../audit_nous_frozen_outputs.py)
recomputes released forecasts. This experiment generates new forecasts when run.

This is a **protocol extension**, not exact reproduction: it uses GPT-6 Luna,
a different output schema, strict probability parsing without clipping, matched
private rechecking, and a communication intervention. The original raw-wallet
extractor and profile-to-prompt translator are unavailable. We read their frozen
profiles and prompts; no claim about regenerating either pipeline is possible.

## Conditions and actual calls

Ten agents receive the same frozen evidence. Each profile condition branches from
its exact saved initial forecasts into private rechecking and peer exchange:

| Profile condition | Private recheck | One simultaneous exchange round |
| --- | --- | --- |
| Neutral | Released placebo text plus neutral padding | Same, with peers' initial forecasts |
| Published Nous | Released persona narrative, unchanged, plus neutral padding | Same, with peers' initial forecasts |
| Structured | An original direct rendering of six released proxy values and z-scores | Same, with peers' initial forecasts |

Each question uses 30 initial calls and 60 revision calls: **90 actual calls**.
Each final condition is charged its ten initial plus ten revision calls for
method-cost comparisons. The shared initial calls are therefore attributed to
both branches, although paid only once. Final probabilities are arithmetic means
of all ten agents; no model judge or additional synthesizer is used.

Both branches receive the same initial self-forecast, raw brief, revision
instruction, output cap and reasoning setting. Exchange additionally receives the
other nine initial records, with participant IDs. Private rechecking receives an
empty peer list. Neither branch sees revisions from the other or from its peers.
If any initial response is invalid, both downstream branches of that profile
condition are blocked symmetrically. The planned denominator is retained.

Profile-to-agent assignment is shuffled identically across conditions per question.
Condition and branch execution order are seeded and randomized. These seeds
control scheduling/assignment, **not provider sampling**: no common-random-number
claim is made for OpenAI calls. Agent identities are local to each question.
This does not estimate long-lived agent error correlations across questions.

### Limits on interpreting the manipulations

All profile blocks have equal UTF-8 byte lengths. This is **not tokenizer-exact
matching**. The analysis reports actual initial input-token means by condition.
Before making a length-controlled claim, inspect token balance (target within
5%) and revise matching on development data if needed. Peer inputs also cost
more than private inputs; output caps alone do not establish equal realized cost.

The published prompt is reproduced as a contiguous unedited block, but the common
forecast instruction and padding alter its surrounding context. The direct arm
has six selected dimensions while the published narrative contains additional
content. A difference between these arms bundles content and encoding changes;
it cannot identify the unavailable translator as the sole cause.

Different answers or higher JSD do not prove faithful transmission of a behavioral
profile. On development data, inspect paired responses to evidence updates and
consensus changes against the intended update-rate/crowd-sensitivity ordering.
Any dedicated directional-probe prompts and pass criteria must be frozen separately
before using them as a compliance test; the current harness does not implement
such a test. Do not describe visible explanations or self-reported peer use as
validated cognition measurements.

## Cohorts

### Historical smoke and replay

Preparation verifies each used upstream file against its pinned Git object. It
rejects empty/missing briefs, upstream `leak_prone` cases, and inconsistent or
future dates relative to the declared as-of date. It does not silently replace a
missing brief with an empty one.

Related questions are provisionally grouped by category and resolution date.
The first two clusters in a deterministic hash ordering form the smoke cohort;
the other clusters form the disjoint replay cohort. Keep whole clusters together.
These coarse clusters require substantive review before scientific confirmation.

At 2026-10-08, the pinned source produces **20 eligible historical questions**:
10 in two smoke clusters and 10 in the remaining replay clusters. Thirty source
questions are excluded. These filters are implementation checks, not proof that
briefs lack leaked outcomes. Retrospective model knowledge remains a major limit.
No number of calls on these old cases establishes prospective forecasting value.

### Prospective cohort

The runner also accepts a user-curated cohort of unresolved questions with explicit
event clusters, dated evidence and source URLs. No outcome fields are accepted.
Every source publication date must be on/before its information cutoff; cutoff
must be on/before preparation as-of date; resolution must be later. Live dispatch
stops at a case's declared resolution date, including during a long run.
These are date-level guards, not independent verification that an event is still
unresolved or that a source's publication metadata is truthful.

Choose related-event clusters before forecasts, include all their questions, and
freeze the cohort and protocol in a timestamped commit before generation. Use
separate development and evaluation event clusters. Outcomes are supplied later
as a separate file with resolution dates and source URLs, without editing the
frozen input manifest. The evaluator records its hash. A human must verify the
resolution sources; the script validates their schema, not their truth.

## Scoring and interpretation

The closest paper comparison is the initial published-profile versus neutral
ensemble Brier difference. Lower is better. Retain this as the **single primary
contrast** for a future preregistered prospective study. The six revised cells,
direct-profile contrasts and interaction are secondary/exploratory here.

The interaction is `(profile_exchange - profile_private) -
(neutral_exchange - neutral_private)` in Brier loss. Negative means exchange
helps that profile condition more than it helps the neutral condition. The code
also reports initial/private/exchange profile contrasts separately.

Per-question output includes ensemble Brier, mean individual Brier, pairwise
error product, JSD, completion counts and each group's attributable cost. Group
Brier and contrasts weight event clusters equally, and questions equally within
clusters. Exploratory intervals resample clusters 10,000 times. Intervals are
suppressed with fewer than five clusters, under 95% valid observations, unresolved
cases, or simulated data. They are not multiplicity-adjusted; do not treat all
printed contrasts as independent confirmatory discoveries.

An unavailable or invalid agent forecast is replaced by **0.5**, a predeclared
fallback policy. This keeps every planned question in the deployment score. It is
not the proper score of a nonexistent forecast, nor guaranteed to penalize failure.
Always report missing and invalid counts alongside scores. No live results means
`not_run` or `awaiting_resolutions`; dry-run artifacts are marked simulated.

For a future powered study, choose a practical Brier reduction before evaluation
(for example 0.01), estimate the standard deviation of paired cluster differences
on development clusters, and use approximately
`N = ((1.96 + 0.84) * SD / 0.01)^2` clusters. SD 0.05 implies about 196 clusters;
SD 0.10 implies about 784. These are planning approximations, not a power guarantee.
Freeze the attainable sample size and stop rule before evaluation. The historical
replay is not large enough to establish a small effect. Never select a favorable
model, profile representation or subset using evaluation outcomes.

Interpretations:

- More dispersion without lower Brier reproduces the distinction Nous highlights.
- Lower Brier for published profiles supports that intervention on the tested model
  and cohort; it does not establish the profiles measure human cognition.
- A direct-arm gain motivates a content/encoding-controlled follow-up, not a claim
  that we repaired the original translator.
- Exchange helping neutral and profile groups equally is a communication benefit,
  not evidence that profiles made communication better.
- A historical null or gain remains provisional until checked prospectively.

## Commands

Run from the Locust repository root. Preparation, inspection, dry-run, and analysis
are offline. Only `run` calls the model API. Source artifacts stay in ignored
`output/`; they are governed by the upstream
[CC BY-NC-ND 4.0 license](https://github.com/WillChienT/nous-paper/blob/3ff0b996784cdc8f4db40210c0505371e667ee1e/artifacts/LICENSE).
This directory contains original experiment code, not redistributed prompt data.

```sh
python3 -m unittest discover -s research/experiments/nous_transfer -v
python3 research/experiments/nous_transfer/experiment.py prepare output/nous-transfer-smoke --upstream output/nous-paper-review-2026-10-07 --phase smoke --as-of 2026-10-08 --ceiling-usd 10
python3 research/experiments/nous_transfer/experiment.py inspect output/nous-transfer-smoke
python3 research/experiments/nous_transfer/experiment.py dry-run output/nous-transfer-smoke
```

`dry-run` creates a separate `dry-run/` child folder with deterministic simulated
responses. It cannot mix those records with a live run. To make the actual model
calls, with `OPENAI_API_KEY` set locally:

```sh
python3 research/experiments/nous_transfer/experiment.py run output/nous-transfer-smoke
python3 research/experiments/nous_transfer/experiment.py analyze output/nous-transfer-smoke
```

Prepare another folder with `--phase replay` for the disjoint historical cohort.
The frozen manifests include code and input hashes, exact prompt blocks and case
inputs, model `gpt-6-luna`, high reasoning and a 4,096-token cap per call. Resumption
checks hashes and recorded request bodies; it refuses ambiguous pending/transport
failures. There are no automatic paid retries. Output truncation/invalid schemas
are retained rather than silently repaired by extra calls.

An explicitly requested recovery can use [recover.py](recover.py). It preserves
the original run directory and creates a separate continuation directory with
an amendment, all attempt records, and a selected-response view. Connection
failures can receive up to four total attempts per logical request with 5/15/30
second backoff. Completed responses, including schema-invalid responses, are
never retried. The first completed response is retained. Every attempt's usage or
full reservation counts against the original aggregate ceiling. Authentication,
request-shape and accounting errors stop further dispatch.

```sh
python3 research/experiments/nous_transfer/recover.py output/nous-transfer-verified-smoke-2026-10-08 output/nous-transfer-completed-2026-10-08 --evidence research/experiments/nous_transfer/smoke-evidence-2026-10-08.json --upstream output/nous-paper-review-2026-10-07
```

Use the same Python runtime as the original evidence export for exact floating
point recomputation (the saved 2026-10-08 evidence uses Python 3.9.7). Recovery
exports retain the full attempt ledger and report its aggregate cost separately
from selected-response scoring costs. Verification checks that selection keeps
the first completed answer and that every retry has the same request hash.

The runner reuses the existing [bounded API transport](../luna_decision_pilot/run.py)
with reservations before each request, a process lock, and four concurrent cases.
The $10 smoke ceiling is a hard accounting ceiling, not a spending target. The
runner rejects requests whose conservative input bound crosses 272,000 tokens,
where the standard price assumptions no longer apply. Input
accounting conservatively uses $0.125/million tokens and output $0.50/million, with
no cache-read discount; these are the rates used by the existing transport and
should be rechecked before a later run against the
[model documentation](https://developers.openai.com/api/docs/models/gpt-6-luna).
Actual invoices are not inferred from those estimates. The model alias can change
behind the same name; retain returned model IDs and run dates, and freeze a dated
snapshot when the provider offers one.

### Prospective input and delayed labels

A cohort is JSONL, one question per line, with exactly these keys (illustrative
schema only; this is not a supplied real forecasting dataset):

```json
{"id":"question-1","cluster":"event-1","question":"Will the specified event occur?","resolution_date":"2026-12-01","information_cutoff":"2026-10-08","evidence":[{"id":"source-1","text":"Dated evidence text.","published_at":"2026-10-07","url":"https://example.org/report"}]}
```

```sh
python3 research/experiments/nous_transfer/experiment.py prepare output/nous-transfer-prospective --upstream output/nous-paper-review-2026-10-07 --phase prospective --cohort path/to/cohort.jsonl --as-of 2026-10-08 --ceiling-usd 100
```

After outcomes resolve, provide a JSON object mapping question IDs to records:

```json
{"question-1":{"outcome":1,"resolved_at":"2026-12-01","source_url":"https://example.org/resolution"}}
```

```sh
python3 research/experiments/nous_transfer/experiment.py analyze output/nous-transfer-prospective --resolutions path/to/resolutions.json
```

Unknown labels are rejected; absent labels remain unresolved. No resolved-outcome
file is used by the generation code. Preserve the final analysis output, original
records, manifests and resolution sources when reporting results.

## Verification record

[Offline verification metadata](verification.json) records the checked upstream
pin, preparation fingerprint, simulated call count and test scope at creation.
The subsequent [live run report](results-2026-10-08.md) records the completed
run, recovery and verifier repair. Historical replay, prospective data collection and
directional behavioral probes remain **unrun**. No scientific effect estimate is
reported from the simulated plumbing check; live results remain exploratory.

Saved live runs can be exported without redistributing upstream prompt or brief
bodies. The exporter retains original model responses, source/request hashes,
minimal scoring labels, costs and diagnostics. Verification recomputes the
statistics; supplying the pinned upstream checkout also rebuilds every request:

```sh
python3 research/experiments/nous_transfer/evidence.py export output/nous-transfer-smoke path/to/evidence.json
python3 research/experiments/nous_transfer/evidence.py verify path/to/evidence.json --upstream output/nous-paper-review-2026-10-07
```

Exports record a Git revision whose four generation-code blobs match the frozen
hashes. Use `export --code-revision REV` when exporting after a code change.
Historical request verification checks those original blobs, rebuilds every
other manifest field from upstream, and uses the transport's actual JSON hash
format. This allows the corrected verifier to audit an earlier run without
changing its frozen manifest or making new API calls. A regression test exercises
the real transport with a mocked HTTP response; simulated requests use their
separately recorded hash format.

For prospective runs, the original frozen cohort is additionally needed;
`evidence.py verify` currently supports upstream request reconstruction only for
historical smoke/replay runs. It still recomputes prospective scores and costs
without `--upstream`. Pending or simulated records are rejected by the live exporter.
