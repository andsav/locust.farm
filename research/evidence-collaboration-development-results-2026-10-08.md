# Evidence collaboration: development results, October 8, 2026

**Status: implemented and executed; development gate failed. No held-out main
or repeat evaluation was dispatched.**

The [study design](evidence-collaboration-study-design-2026-10-08.md) and
[implemented protocol](experiments/evidence_collaboration/README.md) compare
Luna investigations with full-evidence solo work, independent attempts, divided
work, and reciprocal evidence exchange. The completed development phase does
not justify a confirmatory collaboration claim. Its cost-matching gate failed,
and source inspection exposed a mismatch between the experiment's strict
evidence-sufficiency instructions and some MuSiQue annotations.

This is a failed development qualification, not evidence that collaboration
generally cannot help. The earlier 12-family calibration did not catch these
problems sufficiently. Increasing sample size under the same scoring assumptions
would not resolve them.

## What ran

All calls used `gpt-6-luna`, reasoning `high`, without tools, persistent conversation
state, or gold annotations in their inputs. This is a direct-API experiment, not
a test of Locust's runtime or networking. The separate Nous experiment was not
modified by this work.

- **Allocation diagnostic:** two families, 102 paid attempts at 6,000/12,000
  total output-token caps per method/case. Nineteen calls truncated and blocked
  50 downstream jobs. This phase was discontinued and retained.
- **Development v2:** all 40 frozen development families, each with complete and
  insufficient-evidence variants; five methods at each of 24,000/48,000 caps.
  All 3,040 graph jobs resolved: 2,979 paid attempts, 2,954 valid completions,
  25 truncations, and 61 downstream jobs skipped because required inputs failed.
  There were no transport errors and no paid retries.
- **Spending:** $2.541337125 for v2 and $0.078684750 for the initial allocation
  diagnostic, **$2.620021875 total** at the frozen conservative token rates.
  These are cache-normalized estimates, not invoices. Earlier design calibration
  is outside this run's ledger.
- **Untouched evaluation:** the 479-family main partition and 50-family repeat
  subset remain reserved. No main or repeat results exist.

The development sample has 20 three-hop and 20 four-hop families. Its raw rates
describe that sample. Baseline selection reweights strata to the planned main
composition of 400 three-hop and 79 four-hop families. This constructed sample
is not the official MuSiQue leaderboard distribution.

## Observed development outcomes

Family success requires a correct normalized answer **and exact annotated support
set** on the complete case, plus abstention on its insufficient-evidence variant.
Technical failures count as failures. Answer-only family success omits the exact
support-set requirement but retains both variants.

| Configuration | Successful families / 40 | Answer-only families / 40 | Weighted family success | Weighted estimated cost / case | Valid final cases / 80 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Direct solo, 24k | 6 | 12 | 18.4% | $0.000946 | 80 |
| Solo with review, 24k | 8 | 13 | 26.7% | $0.002530 | 80 |
| Independent attempts, 24k | 5 | 10 | 17.5% | $0.003548 | 79 |
| Divided work, 24k | 5 | 9 | 14.2% | $0.004421 | 73 |
| Evidence exchange, 24k | 4 | 6 | 16.7% | $0.005316 | 62 |
| Direct solo, 48k | 8 | 12 | 26.7% | $0.000881 | 80 |
| Solo with review, 48k | 4 | 9 | 13.4% | $0.002542 | 80 |
| Independent attempts, 48k | 7 | 11 | 19.2% | $0.003614 | 80 |
| Divided work, 48k | 5 | 9 | 14.2% | $0.004547 | 79 |
| Evidence exchange, 48k | 5 | 10 | 14.2% | $0.005785 | 79 |

At the preselected 48k allocation, exchange and divided work each succeeded on
five families, with one exchange-only success and one divided-only success.
Against independent attempts, exchange had one win and three losses; against
direct solo, one win and four losses. These are descriptive development counts,
not preregistered main-effect estimates or evidence of equivalence.

Exchange cost 1.60 times independent attempts, 1.27 times divided work, and
6.56 times direct solo under the weighted, cache-normalized accounting. Shared
initial work is billed once in the experiment's spending but fully attributed
to each deployable method. Larger output caps did not force solo methods to
consume more tokens, so equal caps did not produce equal actual resource use.

The independent 48k investigators disagreed on 13 of 40 complete-evidence cases
even after known answer aliases were collapsed. Eight groups mixed correct and
incorrect answers under the frozen labels. There was variation for collaboration
to act on; its existence alone does not show useful information exchange.

## Why the gate failed

The frozen [selection result](evidence/evidence-study-2026-10-08/development-v2-results.json)
reports `No cost-qualified valid solo/independent baseline`. A baseline needed
to consume at least 90% of exchange's weighted estimated cost. The most expensive
eligible S/I configuration consumed about 62.5% of exchange's cost.

The 48k exchange, divided, and independent configurations passed their technical
completion/truncation checks. Exchange and divided work each delivered 79/80 final
cases; one shared initial truncation prevented their remaining case. No 48k
revision truncated. The 24k exchange configuration failed technical readiness,
including 15 revision truncations and only 62/80 final cases.

The quality threshold would also need attention: every S/I configuration's
weighted family success was below 30%, the lower bound for a selected baseline.
The gate reports cost first because no eligible baseline exists; this does not
mean the quality range was satisfied.

No 96k escalation was run. The frozen amendment permits it for technical failure
at 48k, whereas that allocation passed technical checks. A larger cap would not
repair annotation ambiguity or guarantee a cost-matched control.

## Source and annotation audit

The [retained qualitative audit](evidence/evidence-study-2026-10-08/development-v2-annotation-audit.json)
records seven diagnostic examples. It is a Codex assistant review, not independent
human adjudication, and it makes **no changes to primary labels or scores**.

First, the three leading candidates in the fixed hash-ordered queue of unique
nonmatching affirmative answers were reviewed without their arm or gold mapping.
The mapping was disclosed only after recording the judgments:

1. A Barcelona answer names the documented 2009 sextuple and its six competitions.
   The annotation accepts only “continental treble.” The question asks broadly
   for the team's series of wins, so exact matching excludes a plausible supported
   answer.
2. An Israel population answer estimates about eight million from six million
   Jewish residents constituting 75.4% of the population. The annotation gives
   six million for the question about the country's population. The source
   supports distinguishing the subgroup from the total.
3. A Security Council answer names the permanent members and cites a paragraph
   explicitly establishing their veto. The annotation uses different answer
   phrasing and another support paragraph. A supported alternate citation path
   fails the exact-support requirement.

Second, the first four complete-case abstentions from direct solo 48k, ordered
by case-ID hash, were inspected with gold annotations visible. Direct solo
abstained on 22 of the 40 annotated-answerable cases, so the problem is not just
exact citation matching. The four inspected cases include:

- A question about a country whose annotated answer is Kaliningrad Oblast, with
  an additional inference from being Queen of Hungary to citizenship.
- A hometown inference from Andy Roddick playing for the Austin Aces.
- A birthplace inference from Stanton Moore being described as a New Orleans
  drummer.
- A Soviet political-control/agreement question where the model's interpretation
  is arguably too literal; this remains an interpretation-sensitive example,
  not a definitive annotation-error finding.

The models often located relevant documents but rejected relations that the
annotations appeared to assume. The audit therefore supports a narrower conclusion:
**some labels and question formulations conflict with this study's instructions**.
Seven selected examples cannot estimate a dataset-wide error rate or yield
corrected performance. Filtering cases after seeing arm outcomes would also
invalidate the intended held-out comparison.

## Decision and next requirements

The protocol stops before main evaluation. Neither a collaboration gain nor a
precise null result has been established. The defensible empirical observations
are that this implementation executes and can be audited, larger allocations
largely removed truncation, and the tested development comparison did not show
an exchange advantage while consuming more resources.

A successor study needs source-validated questions and answerability labels
before any treatment outcomes, an explicit policy for equivalent answer spans
and alternate support, and a stronger independent-attempt control whose **used**
compute matches exchange. It should qualify those choices on new development
material before freezing a confirmatory sample. Simply increasing calls, relaxing
the gate after seeing outcomes, or changing the primary labels to favor an arm
would not answer the user's original question about useful collaboration.

## Verification and retained evidence

- All 20 experiment tests passed, covering gold-label isolation, D/E message
  differences, shared accounting, durable reservations, resume behavior, scoring,
  paired statistics, archive reconstruction, and metrics.
- Offline verification reproduced all 3,040 v2 request bodies, parsed visible
  outputs, token-price calculations, and the complete summary using frozen source.
- The [records index](evidence/evidence-study-2026-10-08/README.md) links both phases'
  full visible requests/responses, frozen manifests, executable snapshots, usage
  metrics, and hashes. Archived reconstruction requires no API calls.
- Documentation checks and staged whitespace checks passed. No Rust application
  code changed, so no Rust rebuild was required.

Development v2 ran with 16 concurrent requests and its original frozen source.
The current runner permits up to 64 for future phases; that scheduling change
was committed separately and was not used to modify the running experiment.
Observed case wall times include scheduler waits. The retained critical-path
estimates exclude those waits and are not measurements of Locust or production
deployment latency. Public-dataset training contamination also remains unresolved.
