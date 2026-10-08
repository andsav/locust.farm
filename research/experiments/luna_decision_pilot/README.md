# Luna decision pilot: personas, procedures, and aggregation

Date: 2026-10-08. Status: protocol fixed before scored model requests.
This implements the first small follow-up proposed in the
[agent-crowds paper review](../../agent-crowds-and-decision-markets-2026-10-07.md).
The user requested an experiment that could run tonight using cheap models.
This pilot uses `gpt-6-luna`, with a self-imposed **$2 total ceiling** including
the connectivity smoke, failed calls, and uncertain charges.

## Question

Can narrative personas or explicit procedures improve Luna's use of known
probabilities and action payoffs? Does averaging three responses help? This is
a synthetic behavioral proof of concept, not a replication of Nous, a prediction
market, real-world forecasting, or a test of Locust's distributed runtime.

## Frozen design

- Twenty synthetic cases in ten related pairs: three base-rate pairs, three
  duplicate-versus-independent evidence pairs, two diagnostic-strength pairs,
  and two action-cost pairs. Pair variants change one designated factor.
- Four conditions: default task instructions, filler control, narrative persona,
  and explicit procedure. The last three conditions have equal character length.
  Character matching does not ensure equal token counts or semantic neutrality.
- Three calls per condition per case. Persona/procedure calls use three fixed
  emphases: base rates, evidence provenance, and utility. Default/filler calls
  repeat the same prompt. These are independent API requests, not three different
  models or guaranteed independent errors. No model sampling seed is claimed.
- 240 scored requests plus one connectivity smoke; request order shuffled with
  local seed 20261008; up to four requests in flight. Paired variants and arms
  do not see one another's answers. The smoke has no scored task content.
- Responses API, `gpt-6-luna`, low reasoning effort, default service tier,
  4,096 output tokens per call including hidden reasoning, no tools or retrieval.
  Truncated/non-completed answers are failures even if they contain readable JSON.

Every condition receives identical facts and the same requested JSON schema.
Observed evidence likelihoods and conditional independence are specified. Repeated
reports with the same observation ID carry no additional information. The task
asks for the posterior success probability and the action maximizing expected
points. [study.py](study.py) computes exact rational posteriors locally and
expected utility. No sampled event outcome or model judge is used.

The procedural arm supplies computational guidance the other arms do not receive.
A benefit would therefore support that concrete instruction intervention; it
would not isolate an innate personality or establish that diverse procedures
outperform three repeats of the single best procedure. That comparison is a
possible next experiment. There is no strong-model baseline in this cheap pilot.

## Outcomes fixed in advance

Primary metric: mean squared deviation from the exact posterior, averaged across
the three calls and then equally across cases. This equals expected excess Brier
loss over the Bayesian oracle; it is not a Brier score on real resolved events.
Invalid/failed outputs receive squared-error penalty 1.0. Report valid-only
details separately if useful, never silently discard failures.

Primary exploratory contrasts: persona minus filler; procedure minus persona;
procedure minus default. Compute paired differences per related pair, then a
10,000-resample percentile bootstrap over the ten pairs. No multiple-comparison
correction or confirmatory significance claim. Ten pairs from four designed
families are not a representative sample of real decisions.

Secondary outcomes: probability within one percentage point, correct action,
expected regret in task points, correct distinct evidence IDs, and the error of
the mean of three valid probabilities. Regret is the oracle value minus chosen
value; a failed response receives the worse-action regret. Ensemble results use
complete cases only and explicitly report that denominator.

Directional sensitivity compares pair means for changed probabilities against
the oracle direction. Utility pairs instead require all six actions to be correct
across the two variants. This combined diagnostic is stricter for utility and
is not the primary metric. Report it with that definition.

Report actual usage and cost estimates, incomplete responses, and all attempted
conditions. Keep ceiling/floor effects as findings. Do not modify cases, prompts,
grading, or effort after seeing scores and call the replacement the same study.

## Cost and execution

Official [Luna model pricing](https://developers.openai.com/api/docs/models/gpt-6-luna)
checked 2026-10-08 lists $0.10/M input, $0.125/M cache writes, $0.01/M cache reads,
and $0.50/M output. The account's read-only model list confirmed `gpt-6-luna`.
The runner conservatively charges all reported input at $0.125/M and output at
$0.50/M; this is usage-based accounting, not an invoice reconciliation.

[run.py](run.py) reserves a worst-case charge before each request. Input is bounded
by serialized ASCII request bytes plus 4,096 tokens of overhead, and output by
the explicit cap. No tool calls can add context. Usage beyond the reservation or
an unexpected service tier stops further dispatch. Uncertain/failed requests
retain the reservation. There are no automatic paid retries. A transport error
stops new dispatch; already in-flight calls may finish. A process lock prevents
two runners writing/spending against the same folder simultaneously.

Resume skips every previously attempted label, including errors and interrupted
pending attempts. It requires the saved design to match the current code. A
stopped smoke does not start scored calls. Outputs exclude API keys and provider
hidden reasoning. The model receives synthetic public case content only, with
`store: false` and no repository or filesystem access.

## Reproduce

With `OPENAI_API_KEY` in the invoking environment:

```sh
python3 -m unittest discover -s research/experiments/luna_decision_pilot -v
python3 research/experiments/luna_decision_pilot/run.py prepare output/luna-decision-pilot-2026-10-08
python3 research/experiments/luna_decision_pilot/run.py run output/luna-decision-pilot-2026-10-08
python3 research/experiments/luna_decision_pilot/run.py analyze output/luna-decision-pilot-2026-10-08
```

Retain the frozen design, hashes, sanitized response text, request metadata,
usage, failures, summary, and interpretation under tracked research evidence
when complete. A rerun of the API is stochastic; recomputation of the saved
outputs is deterministic. Different API model aliases or versions may behave
differently even with identical prompts.
