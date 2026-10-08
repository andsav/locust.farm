# Luna decision pilot: live results

Date: 2026-10-08. Status: completed live, direct-API proof of concept.

## Result

The cheap experiment works end to end, but these tasks are too easy to establish
a dependable advantage for personas, procedures, or a crowd. Luna produced valid
answers for all 240 scored calls. There were two probability errors greater than
one percentage point, both in the same related task pair, and only one wrong
action. The narrative and procedural conditions were both essentially perfect.

The complete run, including one connectivity smoke, took **303.75 seconds** and
used **$0.048965** under conservative token accounting, below the self-imposed
$2 ceiling. This cost covers the experiment API calls, not the surrounding Codex
conversation. It is an estimate from returned usage and published rates, not a
billing invoice reconciliation.

The [protocol](experiments/luna_decision_pilot/README.md) and runner were committed
as `d002336` before any scored requests. The
[evidence bundle](evidence/luna-decision-pilot-2026-10-08.json) retains the frozen
design, all 241 sanitized responses and usage records, source hashes, request
hashes, and the original analysis. No cases, prompts, or scoring rules were
changed after observing results. The run was not expanded to chase a difference.

## What ran

`gpt-6-luna`, low reasoning effort, Responses API, default service tier, no tools
or retrieval, with up to four simultaneous calls. All returned model IDs were
`gpt-6-luna`. Every scored request completed; there were no transport failures,
truncated answers, paid retries, or skipped scored requests.

Twenty synthetic decisions form ten related pairs: three pairs changing the
base rate, three changing whether an additional report repeats an observation or
provides independent evidence, two changing evidence diagnosticity, and two
changing the cost of acting. Every condition receives the same facts and has
access to all evidence. A local exact-rational oracle computes the posterior
and expected utility; model outputs do not supply ground truth.

Four prompting conditions each receive three calls per case:

- **Default:** the common task instructions.
- **Filler:** common instructions plus generic text matching treatment length.
- **Persona:** narrative descriptions emphasizing base rates, evidence sources,
  or consequences, one emphasis per call.
- **Procedure:** explicit computational steps with those same three emphases.

Filler, persona, and procedure prompts have equal character lengths, not equal
token counts. Filler is not guaranteed to be semantically inert. The procedural
condition supplies computational guidance; this is a concrete instruction
intervention, not an isolated test of an intrinsic cognitive trait.

## Prespecified outcomes

The primary error is squared deviation from the exact posterior. It equals
expected excess Brier loss over that posterior, not a Brier score on real
resolved events. Lower is better. Each row contains 60 individual responses.

| Condition | Valid | Probability within 1 percentage point | Correct action | Mean squared probability error | Mean regret, task points |
| --- | ---: | ---: | ---: | ---: | ---: |
| Default | 60/60 | 59/60 | 60/60 | 0.000251850 | 0 |
| Filler | 60/60 | 59/60 | 59/60 | 0.002941024 | 0.891486 |
| Persona | 60/60 | 60/60 | 60/60 | 1.7653e-9 | 0 |
| Procedure | 60/60 | 60/60 | 60/60 | 8.7266e-10 | 0 |

All 240 responses listed the correct distinct evidence IDs. All four conditions
passed the ten pair-direction diagnostics under the protocol's definition.
Those diagnostic successes did not prevent the individual errors below.

Exploratory paired intervals resample the ten related pairs, not the 240 calls.
These are percentile bootstrap intervals without a multiple-comparison
correction. The designed pairs are not a representative population sample.

| Primary contrast | Mean error difference | 95% pair-bootstrap interval |
| --- | ---: | ---: |
| Persona minus filler | -0.002941022 | [-0.008823071, +2.9545e-9] |
| Procedure minus persona | -8.9261e-10 | [-3.2226e-9, +4.4169e-10] |
| Procedure minus default | -0.000251849 | [-0.000755545, +6.7563e-10] |

All intervals include zero. More importantly, the apparent gains against default
and filler are driven by two isolated responses within one pair. The minute
persona/procedure difference is not a practically useful result.

## The two errors are more informative than the leaderboard

The pair used a 7% prior success probability and three distinct observations.
One variant included a fourth report repeating the first observation. The other
made that fourth report an independent observation with the same likelihoods.

| Attempt | Exact probability | Returned probability | Oracle action | Returned action |
| --- | ---: | ---: | --- | --- |
| `pair-03-0/default/base_rate` | 0.298045 | 0.175119 | WAIT | WAIT |
| `pair-03-1/filler/base_rate` | 0.718116 | 0.298043 | ACT | WAIT |

The second answer is almost exactly the posterior of the duplicate-evidence
variant, despite correctly listing all four distinct evidence IDs. That is
consistent with failing to incorporate the fourth independent observation. It
does not reveal the model's hidden mechanism; this study retains final answers,
not hidden reasoning. The wrong action lost 53.489 expected points on that trial.

The practical lesson is that **listing evidence correctly does not establish
that it affected the calculation correctly**. A product should check the actual
consequences of evidence use where possible, rather than treating a neat source
list as proof of reasoning quality.

## What averaging did

All 20 cases had three valid probabilities in every condition. Their mean had
the following squared error:

| Condition | Mean of individual squared errors | Squared error of three-response mean |
| --- | ---: | ---: |
| Default | 0.000251850 | 0.000083949 |
| Filler | 0.002941024 | 0.000980342 |
| Persona | 1.7653e-9 | 5.6423e-10 |
| Procedure | 8.7266e-10 | 2.8887e-10 |

Averaging diluted the isolated outliers. This alone is not strong evidence of
crowd wisdom: squared loss guarantees that the mean cannot be worse than the
average individual squared loss on the same complete cases. It does not
guarantee beating the best individual or a stronger single-call policy.

A post-hoc check applying the action payoff rule to each mean probability gives
20/20 correct actions for every condition. This is descriptive, separate from
the prespecified primary analysis. There was no discussion, trading, or evidence
exchange, and no comparison with three repeats of the best fixed procedure.

## What this suggests for Locust and the broader product

1. **Cheap model trials are practical.** This fixed task, metered runner, and
   exact grader can collect a useful diagnostic record for cents. That is an
   operational result, not a claim that all useful agent research will be cheap.
2. **The next benchmark should stress evidence structure.** Both sizeable errors
   arose in one dependence pair. A separate development set with more sources,
   duplicated/rephrased reports, contradictory evidence, and shared upstream
   sources could determine whether that is a repeatable weakness. Freeze a fresh
   evaluation set before comparing methods; this pair is now development data.
3. **Preserve a calculator baseline.** In these fully specified tasks, the exact
   oracle is the appropriate computational tool. The interesting product problem
   is extracting and validating the evidence structure, then handing suitable
   arithmetic to a deterministic calculator. Ask whether an agent correctly
   identifies inputs and missing assumptions, not whether it can replace arithmetic.
4. **Do not build a persona or market engine from this result.** The pilot cannot
   distinguish the two treatments reliably and does not test market interaction.
   A useful next comparison would hold evidence and cost fixed across one
   explicit procedure, repeated samples, distinct procedures, and evidence pooling.

These findings support the evaluation approach in the
[paper review](agent-crowds-and-decision-markets-2026-10-07.md), not a new core
Locust policy. This was a one-machine direct-API study. It did not exercise a
Locust daemon, native agent clients, peer authorization, or network collaboration.

## Usage and verification

- 241 completed requests: 240 scored and one smoke.
- 97,543 input tokens and 73,544 output tokens, including 64,241 reasoning tokens
  as reported by the provider. Hidden reasoning content was not retained.
- Median request latency: 4.742 seconds; total elapsed run: 303.755 seconds.
- Conservative accounting uses $0.125/M for all input and $0.50/M for output,
  without cache discounts, from the official
  [Luna model page](https://developers.openai.com/api/docs/models/gpt-6-luna).
- Nine unit tests passed before the live run. All 20 exact-rational oracle answers
  also agreed with a separate floating-point log-odds calculation to 1e-12.
- The offline verifier checks all expected attempt identities, absence of
  duplicates, exact request reconstruction hashes, returned model IDs, frozen
  design hash, budget, and deterministic reproduction of the saved analysis.

Recompute the evidence without model calls:

```sh
python3 research/experiments/luna_decision_pilot/verify_evidence.py research/evidence/luna-decision-pilot-2026-10-08.json
python3 -m unittest discover -s research/experiments/luna_decision_pilot -v
```

The frozen design digest is
`d95b9fcd47c1ad670efe323da15410111252468b888e83c151511b3d48bf8e1a`.
The current model is an API alias; this run does not guarantee identical future
responses or performance on real decisions.
