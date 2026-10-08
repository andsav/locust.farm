# Luna evidence collaboration study

Implemented runner for the
[evidence collaboration design](../../evidence-collaboration-study-design-2026-10-08.md).
The study compares full-evidence solo investigation, independent attempts,
divided work with private rechecking, and divided work with evidence exchange.
It uses direct API calls, not Locust participants. No runtime/networking or market
claim follows from this experiment.

## Amendments frozen before live calls

The complete official MuSiQue archive was downloaded, all ZIP entries passed CRC
verification, and both JSONL input files were hashed. The archive SHA-256 is
`98f839bf2fd5319f5c688aed77901a6d5c30b3b9f9f691ab9a8ecafb045ee0cd`.
MuSiQue reuses many single-hop questions in composed questions. The proposed
balanced 500-family sample was not attainable under the strict no-reused-component
rule. A preliminary balanced selection produced only 50 main families.

Before any model outputs, selection was revised to prioritize scarce four-hop
families and low-frequency components, using a fixed hash seed to break ties.
Development uses 40 families from the official development split, 20 at each
hop count. Main evaluation reserves **479 untouched training families: 400
three-hop and 79 four-hop**. Calibration component IDs and all development
component IDs are excluded from main; no selected families share a component ID.
Normalized duplicate questions are excluded as well. This is a deliberately
constructed investigation sample, not the official leaderboard distribution.
The original calibration used training data; its component IDs remain excluded.

The primary main result weights each selected family equally. Development
baseline selection weights its two strata to the frozen main proportions,
including quality and cost estimated from actual token usage at the fixed
conservative rates below. Results also report each stratum. With
479 families, the approximate detectable difference at 80% power is 7.0 points
at 30% paired discordance, or 8.1 points at 40% discordance. These are planning
calculations; development discordance and source-overlap sensitivity are retained.
The practical target stays eight percentage points.

The model is `gpt-6-luna`, reasoning `high`, on every arm and stage. Initial
budgets are 6,000 and 12,000 output tokens per arm/case. Development compares
both solo policies and every method at both budgets. Calls are fresh and
stateless, use the default service tier, and expose no tools to the models.
An evidence case is the complete or insufficient-document version of one question;
a question family includes both versions. No-document diagnostics were already
run during calibration and are not added to the primary experiment.

The API timeout is 300 seconds instead of the calibration runner's 180 seconds.
No failed or ambiguous paid attempt is retried. Isolated transport failures count
as failures; dispatch stops on an HTTP error, three consecutive transport errors,
or five transport errors among the last 50 calls. Inspection can resume only
unattempted jobs. This prevents one failed call from silently being replaced by a
better answer. An interrupted reservation remains charged at its conservative
reservation until known usage is available; it is never resubmitted.

The study ceiling is $250 across all phases through one SQLite ledger. Initial
phase ceilings are $60 for development, $150 for main, and $30 for repeat; the
whole-study ceiling still takes precedence. Each case/configuration has a $0.25
ceiling. Every request reserves UTF-8 input bytes plus 4,096 overhead tokens and
its output cap before dispatch. Accounting uses $0.125 per million input tokens
and $0.50 per million output tokens without cache discounts. These are conservative
estimates, not invoices. No credentials, HTTP authorization headers, or hidden
reasoning text enter the records.

The [preregistration](preregistration.json) retains the development manifest,
source hashes, main/repeat IDs, and preflight verification. The complete selected
[cohort](../../evidence/evidence-study-cohort-2026-10-08.json.gz) is retained as
UTF-8 JSON compressed with gzip; decompression requires no paid calls. Its source
attribution and input hashes are included. The main manifest is frozen only after
the development gate and baseline selection, before any main outputs.

## Method and enforced boundaries

[protocol.py](protocol.py) builds the request dependency graph. D and E reuse the
same three initial records. D's revision sees its own record; E's revision sees
all three initial records. Their revision instructions, shard, token cap, and own
record identification otherwise match. All final synthesizers see the full raw
corpus and the relevant initial/revised records. Initial and revised records have
stage/worker labels without the arm name. Shards partition the source corpus by a
seeded ordering independent of answer/support annotations.

S-direct uses one investigation. S-review has draft, private verification, and
synthesis stages. I has three independent full-corpus investigations and synthesis.
Every stage uses the same structured evidence schema. A malformed or truncated
record causes dependent jobs to fail without further spending; another arm cannot
see that arm's revisions. The evaluator alone reads answer/support annotations.
[Tests](test_experiment.py) exercise these boundaries, including byte-for-byte
comparison of D/E requests apart from their delivered earlier findings.

[transport.py](transport.py) implements durable reservations and records.
[experiment.py](experiment.py) snapshots source and prompts per run, refuses changed
manifests and cohorts, locks against duplicate runners, executes the dependency
graph, and supports offline verification. Actual execution uses the snapshotted
source. The ledger and all files live under this study's own output directory;
they do not reuse the concurrently running Nous experiment's state.

[analysis.py](analysis.py) reports exact answer/support outcomes, both-variant
family success, cost, stage validity/truncation, initial disagreement, damaged
and repaired answers, confidence, and literal evidence-quote checks. Shared calls
are billed once in experimental spending and attributed in full to each of D/E
for deployable-method cost comparisons. A recheck's answer correctness is compared
to the full-case gold answer; it does not establish that an initially fragmented
worker had sufficient information. Self-reported changes are descriptive.

The primary score requires the correct normalized answer and exact annotated
support on the complete case plus abstention on the insufficient case. Annotation
matching is narrower than semantic correctness. Answer-only family scores, token
F1 and support F1 remain separate. No LLM judge changes primary labels.

Main inference uses one development-selected S/I baseline, paired family bootstrap
intervals, and exact McNemar tests. E−D and E−I tests receive Holm adjustment.
Supporting-source-title connected components provide a separate cluster-bootstrap
sensitivity check; few/large clusters are reported. Confidence Brier scores exclude
technical failures and state their denominator; primary success counts those
failures. A second-seed subset of 50 already-selected families is reserved before
outputs. It changes paragraph ordering as well as model sampling; it does not
create 50 new independent families.

## Run and inspect

From the repository root, with `OPENAI_API_KEY` available only in the environment:

```sh
python3 research/experiments/evidence_collaboration/download.py output/evidence-study-2026-10-08/data
python3 research/experiments/evidence_collaboration/experiment.py cohort output/evidence-study-2026-10-08 --calibration research/evidence/evidence-calibration-2026-10-08.json
python3 research/experiments/evidence_collaboration/experiment.py prepare output/evidence-study-2026-10-08 development-v1
python3 research/experiments/evidence_collaboration/experiment.py run output/evidence-study-2026-10-08/runs/development-v1 --workers 12
python3 research/experiments/evidence_collaboration/experiment.py gate output/evidence-study-2026-10-08/runs/development-v1 --budget 12000
```

Preparation refuses to replace an existing cohort or run. `run --first-families 2`
can perform an engineering check on the first two frozen development families;
a subsequent full run continues the remaining jobs without repeating the check.
`progress.json` is a compact live status. Records and manifests retain exact model
inputs and visible outputs. `--resume-after-stop` requires inspecting the recorded
reason first and only continues jobs that were never attempted.

A passed development gate is required before `prepare --phase main` or
`prepare --phase repeat`, with `--selection PATH` and `--development-summary PATH`.
The runner recomputes the gate. If the chosen primary baseline differs from the
standard four configurations, it is retained as an additional baseline rather
than silently substituted. No held-out outcome participates in baseline selection.

Readiness requires at least 95% valid attempted outputs and final case outputs,
no selected stage with more than 5% truncation, a cost-qualified baseline with
30–85% weighted family success, and at least 10% independent initial-answer
variation on complete cases. If initial caps fail, a separately frozen development
phase may raise caps; failed configurations and spending remain in the report.
A new phase cannot erase an earlier failure. No confirmatory result is claimed
unless the main cohort is completed under its frozen manifest.

```sh
python3 -W error::ResourceWarning -m unittest discover -s research/experiments/evidence_collaboration -v
python3 research/experiments/evidence_collaboration/experiment.py verify output/evidence-study-2026-10-08/runs/development-v1
```

## Data attribution and limits

MuSiQue: Multi-hop Questions via Single-hop Question Composition, Harsh Trivedi,
Niranjan Balasubramanian, Tushar Khot and Ashish Sabharwal, TACL 2022.
[Authors' repository](https://github.com/StonyBrookNLP/musique),
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
Selected data and conversion retain that attribution and license. Exact released
source files and archive hashes are recorded in `source.json` and the cohort.
The dataset is public and old; our held-out partition does not prove absence of
training contamination. Source-title overlap and alternative valid citations can
also limit conclusions. This study measures bounded evidence investigations,
not open-web research, forecasting profit, or autonomous team formation.

The [retained run records](../../evidence/evidence-study-2026-10-08/README.md)
include the failed initial allocation check. Its 19/102 truncated calls justified
[raising development allocations](development-amendment.json) to 24,000/48,000
before the 40-family development comparison. The 6,000/12,000 phase was discontinued
after two families; its missing remaining cases are not treated as observed scores.
The 48,000 allocation is the next candidate for the gate, chosen for completion
headroom before comparing quality. A further 96,000 development phase is permitted
only if its technical gate fails; it must be separately frozen before main.

[evidence.py](evidence.py) exports complete visible records and frozen source,
and can build an arm-blinded queue of unique nonmatching final answers/citations.
The primary scores are unaffected by any later annotation review. Model limits and
accounting rates were checked against the
[official Luna documentation](https://developers.openai.com/api/docs/models/gpt-6-luna)
on October 8, 2026. The 48,000 cap is below its documented output limit.
The current test suite has 20 passing tests, including archive reconstruction,
blinded-queue separation, concurrent budget reservations, and usage/timing metrics.

[metrics.py](metrics.py) supplements the frozen score report with input/output and
reasoning-token totals, cache-aware cost estimates when usage fields are available,
actual case spans, and reconstructed call critical paths. Case spans include
scheduler waits. Critical paths exclude those waits and are not real Locust timing.
The ledger and baseline selection use actual token quantities at a uniform
conservative price; they are cache-normalized estimates, not actual invoices.
Cache-aware estimates reflect this multi-arm run, whose cross-arm cache reuse can
differ from deploying one method alone. Known answer aliases are collapsed in a
separate initial-disagreement diagnostic so wording variation is visible.

Archived results can be reconstructed and checked without the original output
folder or any API calls:

```sh
python3 research/experiments/evidence_collaboration/evidence.py verify-archive research/evidence/evidence-study-cohort-2026-10-08.json.gz development-v1 research/evidence/evidence-study-2026-10-08
```

This is reproducibility using the frozen evaluator, not an independent ground-truth
audit. Blinded alternative-answer/citation review is reported separately.
