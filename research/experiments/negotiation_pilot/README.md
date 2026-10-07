# Strong solo versus heterogeneous negotiation: pilot protocol

Status: protocol frozen before scored requests, 2026-10-07. This is a synthetic
methodology pilot, not evidence of research-level mathematical discovery.
The user authorized up to **$50 total API usage**, including smoke checks and
failed requests. Results will be recorded separately, without changing this
protocol in response to scores.

## Question and comparison

At a matched API-dollar ceiling, does an exchange between two different model
families produce more correct solutions than a strong, fully informed solo
model? Is any difference attributable to heterogeneous negotiation, rather than
independent sampling, one-way synthesis, or additional context/tools?

This operationalizes the proposal in [the OpenAI mathematics assessment](../../openai-math-methodology-2026-10-07.md)
and the earlier [swarm evidence review](../../swarm-evidence.md).
The earlier live board trials established collaboration mechanics; they did not
measure this solo-versus-group question.

Both model families receive the same complete written specification, public
fixtures, Python execution, evaluator and implementation interface. There are
no private specialist facts, model-specific hints, native coding-client
advantages, retrieval differences, or hidden context withheld from solo runs.
Different training priors are a hypothesis behind heterogeneity; this experiment
does not observe or establish that those priors are independent.

## Frozen design

Three task blocks, seven arms each, one independent trial per task/arm: 21 trials.
Each trial has a **$2 ceiling**, with at most $42 allocated to scored runs. The
remaining $8 covers smoke checks, uncertain failed charges, and unused headroom;
it is not an instruction to spend the full amount. Unused stage allocations are
not transferred to another stage or arm. Actual usage must be reported alongside
quality because an equal ceiling is not an equal realized spend.

Models are `gpt-6-astra` (A) and `claude-fable-5-1` (B), both with effort `high`.
The account advertised these IDs and both passed live API smoke checks. Use the
same custom direct-API harness for both; no native CLI client, hosted tool or
web search is involved. API model aliases and provider randomness remain limits
to exact reproducibility; retain returned model IDs and response usage.

| Arm | Initial phase | Review or exchange | Final choice | Total ceiling |
| --- | --- | --- | --- | --- |
| Solo A | A independently, $1 | A self-review, $0.50 | A checks, $0.50 | $2 |
| Solo B | B independently, $1 | B self-review, $0.50 | B checks, $0.50 | $2 |
| Independent A+B | Each independently, $0.75 | Each self-review, $0.25; no exchange | Select by public fixture score; predetermined tie break | $2 |
| One-way synthesis A+B | Each independently, $0.75 | No replies | Designated participant reads both and synthesizes, $0.50 | $2 |
| Homogeneous A+A | Each independently, $0.50 | Each sees the other's initial candidate and critiques/revises, $0.25 | Designated A resolves exchange, $0.50 | $2 |
| Homogeneous B+B | Each independently, $0.50 | Each sees the other's initial candidate and critiques/revises, $0.25 | Designated B resolves exchange, $0.50 | $2 |
| Heterogeneous A+B | Each independently, $0.50 | Each sees the other's initial candidate and critiques/revises, $0.25 | Designated participant resolves exchange, $0.50 | $2 |

The designated participant is index 0 (A in mixed groups) for replay and flow,
and index 1 (B in mixed groups) for scheduling. Independent selection uses public
score, then this same designated index. Hidden tests never select a candidate.
This is an imperfect 2:1 counterbalance; three blocks cannot perfectly balance
final model identity. Compare mixed negotiation to both homogeneous controls and
both solo baselines, not just to a convenient weaker baseline.

The critique phase freezes both initial candidates before either reply. Each
participant sees the other's initial candidate, not its as-yet-unwritten reply.
The final participant then sees both revised candidates and the response to its
own initial work. Thus there is a bounded reciprocal exchange, not unlimited
conversation. Phase structure and communication budgets are experimental
interventions, not proposed mandatory Locust organization.

The trial order is shuffled with seed 871023. Three independent trials may run
concurrently. Participants within a trial execute sequentially; elapsed time is
a harness observation, not a fair parallel-speedup benchmark. No results from a
trial enter any other trial.

## Tasks and scoring

The complete specifications and reference graders are frozen in [tasks.py](tasks.py):

- Causal observed-remove replay: unordered dependencies, missing predecessors,
  cycles, equivocation, duplicate bodies, concurrent additions and removal scope.
- Optional-job scheduling: release times, deadlines, prerequisites, setup delays,
  overlapping blackouts, global value maximization and deterministic tie breaks.
- Integral minimum-cost flow: lower bounds, supplies, parallel edges, self loops,
  negative cycles and lexicographic ties.

Every task provides four generated public fixtures. Each task is scored on 60
separate generated hidden fixtures, seed 773177; public seed is 4103. Inputs and
oracles are fixed before scored requests. Hidden labels are not accessible in
model tools; the source module lives outside the sandbox. Generated candidates
receive hidden inputs only during post-run evaluation, without another model turn.
Hidden expected values are never passed to candidate code.

Primary outcome: exact hidden-case correctness for the selected final candidate,
reported by task and arm. Also report all-tests-passed tasks and conservative
cost estimates. Three tasks are the experimental units: 180 fixtures are not 180
independent trials. No statistical significance or broad superiority claim is
warranted from this design. Ceiling/floor effects, absent disagreement, skipped
phases and provider failures are results to report, not reasons to retune a task
and silently replace the run.

Secondary evidence: initial-to-final score changes, disagreement in candidate
outputs, confidence changes, concrete proposed counterexamples, accepted/rejected
peer corrections, and correct-to-incorrect regressions. Different source hashes
alone do not establish substantive disagreement. An oracle best-of-independent
score may be reported only as a diagnostic upper bound, never as the selected
independent arm's result.

## Execution, budget and Locust boundaries

[run.py](run.py) exposes the same `evaluate`, `probe` and `submit` tools to every
participant. `evaluate` retains complete candidates, `probe` runs self-authored
checks, and `submit` ends a phase. If a phase exhausts its allocation or finishes
without submitting, retain the candidate with the highest public score, breaking
ties by latest candidate. With no emitted candidate, score `return None` as the
explicit failure fallback. A submitted candidate is selected as submitted even
if another public candidate was better. Record all candidates for audit.

[providers.py](providers.py) counts input tokens before each billable request,
reserves input at $25/million with a 20% plus 1,024-token counting margin, and
derives maximum output from the remaining stage/global dollar allocation at
$50/million. Persist the worst-case reservation before dispatch. Reconcile usage
after a successful response; ambiguous failures retain their full reservation.
No automatic paid retries. Stop if reported usage exceeds the reservation. There
is no separate tool-call or agent-turn cap; output limits implement the explicit
user dollar budget. Token counting and pricing are provider reports, not an
independent billing invoice audit. The $8 headroom further separates the planned
scored spend from the user's ceiling.

Current standard prices were checked in the official [OpenAI pricing guide](https://developers.openai.com/api/docs/pricing)
and [Claude Fable 5.1 overview](https://platform.claude.com/docs/en/models/fable-5-1/overview):
$10/million ordinary input and $50/million output for each selected model.
OpenAI accounting conservatively prices all uncached input at $12.50/million
(the cache-write price), and Claude cache creation at its higher $20 rate if
present. No cache controls are requested. Use the official
[OpenAI token counter](https://developers.openai.com/api/docs/guides/token-counting)
and [Claude token counter](https://platform.claude.com/docs/en/build-with-claude/token-counting).

[execution.py](execution.py) runs generated Python with no API credentials, no
network, no access to user homes or other temporary profiles, and writes only
inside a disposable candidate directory. System/runtime reads remain available.
The evaluator gives each call 30 CPU seconds and 40 wall seconds. These are
explicit benchmark execution limits, equal for all arms. It is a local macOS
sandbox, not a general proof of adversarial containment.

[board.py](board.py) starts a real local Locust daemon and creates a separate
open goal with two enrolled principals per trial. Candidate exchanges are actual
signed `contribution publish` and authenticated `contributions` reads, retaining
returned event IDs, author IDs and SHA-256 of exact source bytes. The harness
publishes phase snapshots and schedules exchanges. Models author candidates and
critiques; they do not autonomously choose the formation, participants or timing.
Solo and independent runs cannot see peer board content through their tools.
No completion predicate or peer agreement is treated as correctness: the external
frozen grader decides scores. This is one-machine direct-API evidence, not LAN
recovery, native-client interoperability, decentralized scheduling or a release
qualification.

The copied daemon was built from `a19a395a62aa5a8110a38178c07250bba8cdf9eb` with
`cargo build --locked -p locust`, reporting API 7 / protocol 7. Its build string
contains `dirty`: an unrelated research-index reorder and this untracked pilot
were present, with no Rust source changes. Preserve the binary hash in the run
manifest. Do not imply an exact clean release build.

## Reproduction and evidence

On macOS, with a current copied Locust binary and the two API keys in the parent
environment:

```sh
python3 -m unittest discover -s research/experiments/negotiation_pilot -v
python3 research/experiments/negotiation_pilot/run.py \
  --output output/negotiation-pilot-2026-10-07 \
  --binary output/negotiation-pilot-2026-10-07/locust
```

Use a new output directory for another experiment; the runner refuses to replace
existing trial directories. The ledger persists reservations across restarts.
Do not delete it to circumvent the authorized budget. Retain public prompts,
model-visible text, tool calls/results, candidate code, scores, usage and signed
publication/readback receipts. Do not publish API keys, local capabilities,
provider hidden reasoning, encrypted reasoning blocks or private daemon stores.
The tracked results package should retain the material evidence, not just a
summary pointing at ignored scratch output.


## Implementation erratum during the first batch

The initial runner raised `JSONDecodeError` when an OpenAI response reached its
output allowance inside a function-call argument. This violated the already
specified phase fallback: a truncated candidate must not abort the trial. The
repair drops incomplete calls from executable/replayed history, preserves their
visible partial arguments in the trace, and ends an incomplete response's phase
with the best existing public candidate. It retains the original token-counting
margin, dollar allocations, model settings, task inputs, graders and prompts.
An added regression test covers accounting and non-execution of partial calls.
The first batch remains intact. Explicit technical reruns of failed trials use
`--only`, a separate `--output`, `--prefix repair-`, and the **same global
--ledger**. Their charges and results must be reported separately, alongside all
original failures. These reruns are not fresh preregistered replications and must
not silently replace failed trials in the original-batch denominator.


## Recorded outcome

See [the first pilot results](../../negotiation-pilot-results-2026-10-07.md) for
the original batch, disclosed technical reruns, costs, evidence and limitations.
[analyze.py](analyze.py) supports both runtime output and the tracked public
evidence bundles; it makes no model calls.
