# Nous: Luna, Sonnet and mixed teams

Status: **live execution in progress; formatting amendment recorded**. This extends the
[completed Luna profile experiment](../nous_transfer/results-2026-10-08.md) to
separate model strength, model mixing, and communication effects. It does not
change that experiment or discard its outputs.

## Frozen comparison

Twenty eligible historical questions from the pinned Nous artifacts are used:
the ten previously tested questions in two provisional clusters, and ten
additional questions in three other clusters. The latter have not been used in
our previous generation, but remain historical public outcomes. Report previous,
additional and pooled results separately; none are prospective evidence.

Every question retains neutral, published Nous and structured-proxy profiles.
Each profile condition compares these ten-member teams:

| Team | Members | Stages |
| --- | --- | --- |
| Luna | 10 `gpt-6-luna` calls | Initial, private recheck, peer exchange |
| Sonnet | 10 `claude-sonnet-5-5` calls | Initial, private recheck, peer exchange |
| Mixed | 5 Luna + 5 Sonnet | Initial, private recheck, peer exchange |

Both APIs use high effort and a 4,096-token total output cap. Those settings do
not establish equal internal compute or equal dollar cost. Prompt/evidence text
is identical across providers, with provider-specific API envelopes. All inputs
are frozen evidence; there are no tools, live searches or model judges.

Initial forecasts and private revisions do not contain team membership, so their
exact saved responses are shared between pure and mixed teams. Mixed-team
independent aggregation requires no extra generation. Each exchange response
receives its own exact initial response and the other nine selected teammates'
initial responses; it never sees peer revisions. Peer messages have anonymous
participant IDs and no provider labels. Models can still infer stylistic clues.

The profile-to-participant rotation is unchanged from the original protocol.
Within each ten-question cohort, each profile is assigned to Sonnet five times
and Luna five times in the mixed team. Every question has exactly five of each.
The assignment is frozen before new outputs and shared across profile conditions.

There are **5,400 logical forecast slots**, supported by **4,200 unique model
requests** through response sharing. The original 900 Luna responses are imported
without regeneration after checking their request hashes against reconstructed
inputs. The extension needs **3,300 new calls**: 1,200 Luna and 2,100 Sonnet.
Reusing Luna introduces a run-time/provider-drift limitation; timestamps and
returned model IDs are retained. Same-day reuse is not simultaneous execution.

## Scoring and failure policy

The primary contrasts are mixed-minus-Luna and mixed-minus-Sonnet initial
ensemble Brier, separately for each profile condition. A mixture beating only
Luna does not establish complementarity beyond Sonnet. Exchange-minus-private
within each team and profile effects within each team are secondary. Also
compare revisions with initials: an exchange advantage over private rechecking
can coexist with both being worse than the initial forecast.

The ten probabilities are averaged. Brier scores weight event clusters equally
and questions equally within each cluster. Per-question Brier, dispersion,
pairwise error products and valid-response counts are retained. Report actual
method-attributed costs and total spending; no equal-dollar claim is made.
Five provisional clusters and multiple exploratory contrasts do not justify a
confirmatory claim; the report omits inferential intervals.

The first non-transport result is retained, including truncated or schema-invalid
outputs. Parsing accepts strict JSON or exactly one forecast-schema JSON object
surrounded by prose or Markdown fences. It does not coerce values, choose among
multiple forecast objects, or regenerate an answer. The rule applies equally to
both providers. Invalid forecasts receive the declared 0.5 scoring fallback. An invalid
initial becomes an explicit unavailable marker in both revision branches, which
can still produce forecasts from the supplied evidence. This is an explicit
extension to the original downstream-blocking rule; it does not affect reused
Luna requests because every reused initial was valid. No answer is retried to
improve its probability, explanation or schema.

Connection and transient HTTP failures receive up to six attempts total, with
5/15/30/60/60-second backoff. Authentication/request errors, unexpected model or
service tier, usage-bound errors, and the spending ceiling stop dispatch. Every
attempt is journaled and reserved before network activity. Failed attempts retain
their full reservation; all new attempts count against a **$100 ceiling**.
Completed calls are never regenerated on resume. Pending attempts require
inspection to avoid silently duplicating an ambiguous call.

### Formatting amendment during execution

The initial implementation required the entire response to be JSON. During the
first 894 new completed calls, Sonnet sometimes returned a valid forecast object
with surrounding prose. Marking those forecasts invalid would conflate format
obedience with forecasting, and pass unavailable markers to peers unnecessarily.
The run was interrupted and the symmetric unique-object parser above was frozen
before continuation. This is a disclosed amendment after observing outputs,
not a preregistered result. No probability or other field is repaired.

The original run and all its 1,802 attempts remain intact: 900 prior Luna
responses, 894 new completed calls and eight calls whose provider outcome became
unknown on interruption. Their full reservations remain in the accounting.
The first 900 responses and 746 further responses have identical reconstructed
requests under the amendment and are reused. The 148 revisions whose request
inputs change are retained as superseded attempts and regenerated with the
correct initial messages. No regeneration is selected by forecast score.
This leaves 2,554 calls to finish the amended 4,200-request comparison.
The earlier new-attempt cost/reservations of $7.087970625 count toward the same
$100 ceiling; the amended run does not reset the budget.

The amended folder is `output/nous-models-formatted-2026-10-08/`. A
`stop-requested` file now allows dispatch to stop between calls while in-flight
requests finish. The original run lacked this mechanism, so its eight interrupted
calls remain explicitly unknown rather than being claimed as failed or free.

## Provider settings and accounting

Sonnet 5.5 availability was checked through the account's Models API. Its official
[model documentation](https://platform.claude.com/docs/en/models/sonnet-5-5/overview)
lists $2 per million input tokens and $10 per million output tokens. The runner
uses adaptive thinking, explicit high effort and `standard_only` service. Its
[Messages API](https://platform.claude.com/docs/en/api/messages/create) supports
these fields. Cache reads and writes, if returned, are accounted separately;
reservations conservatively allow the $4/million one-hour cache-write input rate.
No cache breakpoint or tool is requested. Hidden thinking blocks are not saved.

Luna retains the original $0.125/million conservative input accounting and
$0.50/million output accounting, high reasoning and default tier. Current model
aliases and different provider tokenizers remain limitations. Equal byte padding
from the original study is retained, including its known token imbalance across
profile conditions. Within a profile condition, model-team comparisons use the
same text, but are not tokenizer-exact matches across providers.

## Run and verify

From the repository root, with credentials set locally:

```sh
python3 -m unittest discover -s research/experiments/nous_models -v
python3 research/experiments/nous_models/runner.py prepare output/nous-models-2026-10-08 --upstream output/nous-paper-review-2026-10-07 --baseline research/experiments/nous_transfer/completed-evidence-2026-10-08.json
python3 research/experiments/nous_models/runner.py run output/nous-models-2026-10-08
```

The current amended run was prepared with
`prepare output/nous-models-formatted-2026-10-08` and the same arguments plus
`--prior output/nous-models-2026-10-08`. Resume it using
`run output/nous-models-formatted-2026-10-08`; the old folder is archival.

Use Python 3.9.7 consistently for exact reproduction of the original exported
floating point summaries. Preparation is offline and verifies the pinned
upstream files against Git objects. Only `run` makes paid API calls. Inputs and
raw manifests remain in ignored output because they contain upstream licensed
prompt/brief text. Durable reports retain original model outputs, usage,
request/source/code hashes and minimal scoring labels instead.

The implementation uses [study.py](study.py) for the frozen design and scoring,
[runner.py](runner.py) for bounded calls, and [test_study.py](test_study.py) for
request-identity, mixed-peer routing, response-sharing, accounting and retry tests.
