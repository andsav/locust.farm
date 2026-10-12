# MASS and Locust: information handoffs, learning and validity

Status: source assessment, 11 October 2026, updated after the 12 October pilot.
No upstream reproduction was run. This note does not change the accepted
collaboration thesis or runtime behavior.

Follow-up, 12 October: the [concrete information-policy experiment plan](../docs/shared-record-information-policy-plan.md)
specifies a Luna mechanism study and a frontier solo benchmark. It separates
peer exposure from private review and from independent search. Its
[harness has offline verification](shared-record-information-policy-verification-2026-10-12.md).
The [completed calibration and development pilot](shared-record-information-policy-synthesis-2026-10-12.md)
did not establish an incremental sharing benefit; evaluator timing instability
prevented the scored phase from qualifying. It does not reproduce or refute
MASS's combined workflow-search and training procedure.

Sakana's [12 October announcement](https://sakana.ai/mass/) uses the title
“Recursive Self-Improvement through Collective Intelligence.” The owner's
objection is causal: a successful combined workflow-search/training procedure
does not isolate communication or diversity as the source of its gain.
The plan therefore distinguishes those claims, treats diversity as a measured
mediator rather than an established cause, and makes no RSI claim for a
fixed-weight experiment. Homogeneous agents can still produce different
searches; provider diversity is not a prerequisite for this mechanism test.

Sources: Lee et al., [Recursive Self-Improvement through Multi-Agent
Self-Supervision, arXiv v1](https://arxiv.org/pdf/2610.12176v1), submitted
8 October 2026; its [HTML version](https://arxiv.org/html/2610.12176v1); and
the official repository at
[`fb403581bed2c974b3f99e7236224169da643079`](https://github.com/SakanaAI/mass/tree/fb403581bed2c974b3f99e7236224169da643079).

## Assessment

MASS makes the design of information handoffs a more concrete research target
for Locust. Our next useful question is whether sharing a selected piece of
evidence at a useful moment preserves independent search while improving the
next attempt. The paper does not resolve that question for our product.

Keep three interventions separate: changing the information available during
an attempt, optimizing the working procedure across attempts, and changing the
model's weights. A benefit from their combination does not identify which
intervention Locust needs.

## What the paper establishes within its setting

These are author-reported results, not Locust measurements:

- MASS alternates same-model orchestrator/worker workflow search with
  fine-tuning on selected trajectories. Two cycles improve score per output
  token by 1.2–1.6 times on four research benchmarks; coding scores decline
  (§§3–4).
- Paper-v1 MLR evaluation uses 12 tasks, five trials. Higher scores accompany
  more completed papers; none of 180 runs delegates (Appendix C.5).
- The 68.3% versus 64.0% training comparison pools four training tasks and one
  test task. Training coverage, sampling and checkpoint selection differ
  (Appendix D).
- Workflow-text information estimates do not establish causal effects of
  executed communication (Appendix F).
- A simulator revision changes physical success into direct repositioning,
  yet receives five favorable judgments out of six (Appendix G.3, p29).
- First-cycle training alone uses six H100s for about 19 hours, excluding
  workflow search, collection and evaluation (Appendix B.2).

Read these together: useful evidence for an investigable mechanism, limited
evidence for generality, and a direct reason to retain independent validity
checks. Higher evaluator agreement is not a substitute for them.

## Fit with our actual research

### The closest connection is the shared-record result

The [shared-record study](shared-record-study-results-2026-10-08.md) tested
solo, independent attempts and agents sharing findings through a real Locust
daemon. Its six tasks and single trial per arm were exploratory; two task
comparisons had substantial measurement problems. Sharing showed no actionable
advantage over independent attempts. Reading the record early made agents
converge, cost approximately 60% more per candidate across the five non-TSP
tasks, and once spread a defect that the public selector could not distinguish.

That result is already a reason to test later or filtered exposure, as the
study's own follow-up proposes. MASS strengthens the motivation to study the
handoff policy explicitly. It does not overturn the observation or establish
that our original sharing procedure would improve with a different model.

The [collaboration thesis](../docs/collaboration-thesis.md) now centers on
independent attempts, trustworthy checks and the incremental value of retained
findings. Its introduction records the completed study; some body passages
still use pre-study wording. The completed results above are the authority for
what has actually been measured.

### Model mixture remains a separate question

The [mixture synthesis](model-mixtures-and-communication-synthesis-2026-10-08.md)
found no reliable general benefit from changing model families or increasing
exchange. Our [negotiation pilot](negotiation-pilot-results-2026-10-07.md)
saturated before exchange; [forecasting mixtures](experiments/nous_models/results-2026-10-08.md)
did not consistently beat both pure teams; the
[evidence-collaboration study](evidence-collaboration-development-results-2026-10-08.md)
failed qualification. These remain distinct negative or inconclusive results.

For a new handoff experiment, use the same model first. This isolates the
effect of the information policy and gives heterogeneous teams a credible
comparator if they are studied later. Model identity should not stand in for
measured complementarity.

### Procedure learning and model learning need different comparisons

The [literature review](self-organizing-agents-literature-2026-10-07.md)
already treats GPTSwarm and AFlow as comparators for optimizing a procedure
across executions. It distinguishes that process from peers choosing their
organization during an execution, and charges search costs separately.

My inference is that Locust has two potential research uses here. First, its
record can make an information policy testable: what existed, who received it,
what changed afterward, and which independent check passed. Second, those
records could eventually become training examples. The second use requires
additional work on data selection, complete trajectories, permission to use
the data, and training/evaluation isolation. Existing signed records do not
by themselves establish a suitable training corpus or a training advantage.

Keep any trainer or workflow optimizer outside the collaboration protocol
until evidence justifies tighter integration. The
[self-organization plan](../docs/self-organizing-collaboration-plan.md)
separates working method from local execution and acceptance authority. A
method that works for one executor is still compatible with that separation.

### Validation must protect the original question

Our annotation problems in the evidence study and the propagated hidden bug
in the shared-record study already show that completing a workflow and
improving its measured score are different from satisfying its intended task.
The simulator example makes this failure especially concrete.

For Locust, a useful training or evaluation record should distinguish:

| Observation | What it establishes |
| --- | --- |
| A finding was delivered | The proposed information treatment reached its recipient |
| An artifact was read or edited | Access or modification occurred; useful uptake remains unproven |
| An output was completed | The expected deliverable exists |
| A reviewer preferred it | A particular reviewer judged it better |
| An independent check passed under the original task | Evidence of task validity within that check's coverage |

Only the last row directly supports the intended quality claim, and its
coverage must still be stated. Preserve failed hypotheses and unfavorable
scientific findings; an optimizer must not gain credit by changing the task
or its acceptance evidence.

## A narrow follow-up worth considering

This is a research proposal, not a paid-run authorization or a frozen protocol.
Keep weights fixed and reuse the real-daemon harness where appropriate. Retain
a strong solo reference and independent attempts, then compare the current
early-sharing method with one policy selected on development families only.
Candidate policies include exposing findings after a plateau or exposing
failed approaches and counterexamples before complete solutions.

Choose the policy and its trigger on development data; freeze them before
held-out evaluation. Recheck the public selector's discriminating power and
task validity first. Use repeated task-arm trials and quality against actual
spend; keep the selector, tools, model and continuation procedure comparable.
Hidden acceptance evidence must remain inaccessible to every working agent,
policy optimizer and selector.

Measure whether useful candidate diversity survives, whether information was
actually delivered, and whether subsequent work repairs an error or damages
a correct result. Charge retrieval, communication, selection and failures.
Report policy-development cost separately and show how often the resulting
policy would need to be reused to recover it. A result where independent
attempts remain preferable is useful and should remain visible.

If an information policy produces a credible gain, a separate experiment can
ask whether training preserves it. That comparison should match training-task
coverage and selection procedures, distinguish data generation from processed
training tokens, and evaluate frozen held-out families. There is no reason to
make weight training a prerequisite for the smaller information-policy test.

## Upstream reuse and reproduction boundary

The pinned [README](https://github.com/SakanaAI/mass/blob/fb403581bed2c974b3f99e7236224169da643079/README.md)
reports newer MLR results on 107 tasks by three trials (1.71 to 2.25), distinct
from paper v1. Its packaged configuration still covers the earlier subset;
several benchmark adapters are absent. Do not combine the two result sets.

The [implementation notes](https://github.com/SakanaAI/mass/blob/fb403581bed2c974b3f99e7236224169da643079/docs/reproduction.md)
disclose changed search rules, an incomplete historical run ledger and
reconstructed tournament/second-cycle settings. They report offline/mock
validation, not a validated full GPU and benchmark reproduction.
The [notice](https://github.com/SakanaAI/mass/blob/fb403581bed2c974b3f99e7236224169da643079/NOTICE.md)
states that the MASS-specific code has no assigned public open-source license
and excludes weights, datasets and generated artifacts from distribution.

The practical reuse today is the experimental decomposition and documented
interfaces. Direct code adoption or exact reproduction would require separate
qualification. This assessment inspected paper text, rendered relevant PDF
pages, pinned upstream documentation and current local research; it did not
execute upstream code, reconstruct our archived runs or launch model calls.
