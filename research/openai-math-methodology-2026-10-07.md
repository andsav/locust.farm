# OpenAI mathematics methodology and its relevance to Locust

Research date: 7 October 2026. **Status: source assessment and experiment
proposal. No mathematical proofs, agent experiments or Lean builds were run.**

The assessment inspected OpenAI's release announcement, the repository README
and catalogue, the structure and selected passages of all ten released reasoning
summaries, formalization metadata, representative Comparator configurations and
challenge/solution files, Comparator's documentation, and the earlier research
report linked by the announcement. Locust source was inspected at
`8f274192272bdf1a7ce5207a4a59995406f471fe`.

The mathematics repository snapshot is
[`adc7f1241b42e322a6451854ab7e4b4c146bf78a`](https://github.com/openai/math/tree/adc7f1241b42e322a6451854ab7e4b4c146bf78a),
dated 6 October 2026. This is a methodology assessment, not an independent
certification of the announced theorems. The recursive GitHub tree response was
truncated; targeted directory requests and file retrieval were used instead. No
whole-repository file count or exhaustive source audit is claimed.

## Assessment

There is a substantial connection to Locust, with an essential distinction:
the October collection and the earlier September swarm experiment have different
disclosures. The latter explicitly used communicating agents; the former does
not specify its per-problem agent topology. A shared internal model and a link
between announcements do not establish an identical orchestration procedure.

The transferable method is sustained search across alternatives, explicit
criticism, reuse of intermediate results, selective synthesis, and verification
of a precise artifact. Locust can carry that collaboration. This release does
not establish how much performance comes from coordination, model capability,
longer reasoning, human steering, or total compute.

## Two disclosed workflows

### October: broad evaluation and publication

The [repository README](https://github.com/openai/math/blob/adc7f1241b42e322a6451854ab7e4b4c146bf78a/README.md)
reports approximately 4,000 problems posed to an unreleased internal model,
722 manuscripts grouped into 372 result families, and roughly three hours of
ChatGPT Pro thinking compute per result on average. Most results followed one
procedure, with exceptions including zeta zero-free-region and CM-abelian Hodge
work. Some outputs build on earlier model results; one specified zeta writeup
was human edited. Verification status varies, and unformalized results may
contain issues.

Those counts do not yield a success rate: problems, families and manuscripts
are different units. The compute equivalence is neither a wall-clock guarantee
nor an accounting of every failed attempt, formalization and human contribution.
The public description leaves agent counts, scheduling, restarts, tool access,
selection rules and full per-problem resource accounting unspecified. The
repository publishes mathematical outputs and checking artifacts, not a
reproducible solver harness.

The [October announcement](https://openai.com/index/sharing-ai-progress-in-mathematics/)
also identifies continued formalization and community understanding as work
following the release.

### September: an explicitly coordinated swarm

The [linked Navier–Stokes report](https://openai.com/index/navier-stokes-solution/)
describes communicating agent groups using an internal model, cached web access
and code execution. Separate groups explored different problem variants.
Approximately 100 agents worked for 50 hours on an Euler result; researchers
then redirected effort and supplied that result to other groups. Codex
consolidated useful findings for cross-group prompts. The successful
Navier–Stokes group had roughly 10,000 concurrent agents. The report gives
88 hours to a resolution and another 17 for Lean formalization and verification
with GPT-6 Astra, with 130 billion output tokens attributed to the Navier–Stokes
effort. Agents were also upgraded to a further-trained model during the effort.

This is direct evidence of a reported multi-agent method, including human
steering. It is not a controlled comparison against one agent at equal compute.
Its scale and budget cannot be assigned to the October collection.

## What the reasoning summaries reveal

These are abridged, selected narratives, not complete execution logs. Their
first-person snippets and descriptions of “the assistant” cannot determine
process count, communication topology, or whether a referenced paper was
retrieved with a tool or recalled. They do expose useful research behavior.

1. **Specify the actual mathematical obligation.** The
   [Kaplansky summary](https://github.com/openai/math/blob/adc7f1241b42e322a6451854ab7e4b4c146bf78a/reasoning_traces/kaplansky-direct-finiteness-characteristic-two.pdf)
   distinguishes unrestricted groups from special classes, positive from zero
   characteristic, and direct from stable finiteness. This is a task contract
   designed to prevent solving an easier neighboring problem.
2. **Search alternative representations and attack their bottlenecks.** The
   [Mahler summary](https://github.com/openai/math/blob/adc7f1241b42e322a6451854ab7e4b4c146bf78a/reasoning_traces/symmetric-and-general-mahler-conjectures.pdf)
   moves through geometric, analytic and probabilistic representations. Failed
   constants, normalization issues and matrix dependencies drive revisions.
   It explicitly separates three attempts with different premises; the
   equality-classification attempt is supplied a lower bound, and the general
   case has its own argument.
3. **Try to break an attractive argument.** In the
   [pi summary](https://github.com/openai/math/blob/adc7f1241b42e322a6451854ab7e4b4c146bf78a/reasoning_traces/irrationality-exponent-of-pi.pdf),
   candidate approximation arguments are tested against Liouville-type examples
   that would make an overgeneralized claim impossible. Denominator costs,
   nonvanishing and rank requirements repeatedly undermine proposed routes.
   The later exponent-two attempt borrows an earlier technique without assuming
   its earlier numerical conclusion.
4. **Turn gaps into specific repair obligations.** The
   [Vlasov–Maxwell summary](https://github.com/openai/math/blob/adc7f1241b42e322a6451854ab7e4b4c146bf78a/reasoning_traces/relativistic-vlasov-maxwell.pdf)
   identifies a mismatch between smooth finite-energy data and a continuation
   theorem's Sobolev hypotheses. It also revises an estimate that fails on short
   intervals and repeatedly checks constants and bootstrap dependencies.
5. **Reuse results and ask a stronger or different question.** The
   [arithmetic-progression summary](https://github.com/openai/math/blob/adc7f1241b42e322a6451854ab7e4b4c146bf78a/reasoning_traces/quasipolynomial-arithmetic-progressions.pdf)
   includes a follow-up asking for stronger bounds using the previous work.
   The Kaplansky companion investigation explicitly assumes earlier algebraic
   witnesses and derives a cellular-automaton construction from them. Reusing
   a technique and assuming a theorem are different dependency relationships.

The [Mézard–Parisi prompt and summary](https://github.com/openai/math/blob/adc7f1241b42e322a6451854ab7e4b4c146bf78a/reasoning_traces/mezard-parisi-formula.tex)
also show that this is not simply brute-force numerical search: its prompt
discourages brute-force and computer-assisted proofs in the final writeup.
That restriction on presentation does not prove that tools were unavailable.

**Inference:** a useful high-level reconstruction is precise problem statement,
multiple candidate approaches, counterexamples and hypothesis checks, repaired
lemmas, assembled proof, and checking of the resulting artifact. Some sequences
then seed further problems. The summaries support this cognitive workflow;
they do not disclose the software architecture implementing it, nor prove that
each claimed repair is mathematically sound.

## Verification is a distinct part of the method

The [Lean catalogue](https://github.com/openai/math/blob/adc7f1241b42e322a6451854ab7e4b4c146bf78a/lean/formalization.yaml)
maps papers to declarations and Comparator configurations. It records automation
method `agent` and review status `unchecked`. These fields should not be read as
either a complete multi-agent architecture or a claim that every supplied Lean
proof fails. At this snapshot the `sources` section lists 162 paper entries;
that is a catalogue count, not a count of independently certified theorems or a
coverage percentage for the 722 manuscripts.

[Comparator](https://github.com/leanprover/comparator) checks that a solution
proves the challenge statement, uses only permitted axioms, and is accepted by
the kernel, subject to its documented trusted-input and execution assumptions.
Representative [Vlasov–Maxwell](https://github.com/openai/math/blob/adc7f1241b42e322a6451854ab7e4b4c146bf78a/lean/ComparatorChallenges/VlasovMaxwell.json)
and [zeta](https://github.com/openai/math/blob/adc7f1241b42e322a6451854ab7e4b4c146bf78a/lean/ComparatorChallenges/QuasiRiemannHypothesis.json)
configurations permit `propext`, `Quot.sound` and `Classical.choice` and disable
the optional Nanoda kernel. They are checking configurations, not execution
receipts.

The [Vlasov–Maxwell challenge](https://github.com/openai/math/blob/adc7f1241b42e322a6451854ab7e4b4c146bf78a/lean/ComparatorChallenges/VlasovMaxwell.lean)
intentionally contains a theorem placeholder. Its
[solution module](https://github.com/openai/math/blob/adc7f1241b42e322a6451854ab7e4b4c146bf78a/lean/OAI/Analysis/VlasovMaxwell/Main.lean)
assembles the result from imported lemmas. Finding `sorry` in a challenge is
therefore not evidence of a missing submitted proof; inspecting the final
solution theorem is also insufficient to certify its imported dependency tree.

There are separate questions: did a checker accept the exact formal statement;
does that statement faithfully represent the intended problem; and have people
understood the argument and its significance? The
[advisory group's release recommendations](https://agmai.org/general-sep29/)
explicitly distinguish formalization, disclosure and human understanding.
Consultation with that group should not be described as its certification of
this collection. This assessment did not run Comparator or resolve these
mathematical questions.

## Mapping to Locust's current implementation

| Research need | Locust surface | Boundary |
| --- | --- | --- |
| Try different approaches to one problem | Multiple attempts at a task | Independent execution does not guarantee different ideas. |
| Retain intermediate work | Contributions and content-addressed files | A published claim can still be wrong or conditional. |
| Criticize a candidate | Reviews of an exact subject | Another model's approval is not a proof checker. |
| Require external validation | Named-check attestations combined with reviews | Locust verifies the signed assertion and eligibility; it does not run or independently establish the check. |
| Build a combined result | Exact tree proposals, composition, review and integration | Previously approved inputs do not automatically approve new combined bytes. |
| Share across people and tools | Local CLI/MCP plus replicated signed records | The external reports do not establish a decentralized or heterogeneous execution design. |

Implementation evidence:

- [Formation presets](../crates/locust-proto/src/organization/presets.rs) include
  independent attempts, peer review, a review panel and directed work. A
  mandatory central planner is not required to run a research workflow.
- [Start eligibility and latest reviews](../crates/locust-core/src/goal/mod.rs)
  and the [independent-attempt regression](../crates/locust-core/tests/organizations.rs)
  establish the separate-attempt mechanism.
- [Completion predicates](../crates/locust-core/src/goal/fold.rs) match reviews
  and named-check attestations against the exact subject, applicable rule and
  eligible signer. They support conjunctions of evidence requirements.
- The [formation manual](../docs/guide/formations.md) explicitly says Locust
  does not run an attested check. The [workspace manual](../docs/guide/apply.md)
  separates exact-artifact review, composition and integration; reviewers run
  their own checks in the materialized directory.
- [Concepts](../docs/guide/concepts.md) describes independent local levels and
  shared history. [The master plan](../docs/master-plan.md) identifies proposed
  work separately from built phases. Proposed automatic integration and agent
  hooks are not assumed to be available merely because they appear in plans.

Locust addresses coordination, durable evidence and participant control. The
mathematical strategy is an experiment running on that substrate. Peer-to-peer
transport is useful for the intended ownership model, but is not shown here to
cause mathematical breakthroughs. Model strength also remains a separate
variable: orchestration cannot be credited for capabilities that were never
measured with a fixed model and budget.

## A useful experiment, without changing the core design

This is a proposal, not an accepted implementation phase or a running trial.
Start with a small set of difficult, checkable tasks. Record the task statement,
starting artifacts, allowed assumptions and evaluator before starting work.

1. Give attempts materially different approaches or subclaims. Preserve some
   initial exploration without seeing other candidates; sharing every tentative
   idea immediately is not necessary for collaboration.
2. Publish concise artifacts stating the claim, exact assumptions, dependencies,
   failed approaches, counterexamples and remaining proof obligations. Distinguish
   a borrowed technique from an assumed result. These can initially be document
   conventions, without adding protocol types.
3. Make falsification a useful task: reproduce a check, find a counterexample,
   expose a missing hypothesis, or show that a proposed lemma merely restates
   the unsolved target. Use fresh-context reviewers where practical.
4. Share selected findings across approaches at defined points. A shared goal
   provides full history, so strict experimental isolation needs separate goals
   or separately controlled contexts; it is not a claimed access property of
   the ordinary board.
5. Use a trusted local evaluator for the exact candidate. For mathematics this
   may be Lean plus statement review; for code, hidden tests, properties and
   reproducible benchmarks provide narrower evidence. Publish the artifact hash,
   checker version, invocation and result. Bind any signed attestation to that
   candidate. A research formation can require both review and a named check.
6. Recheck a composed artifact and its dependencies. When a premise changes,
   request new review of dependent claims. This is a proposed research practice,
   not a claim that Locust currently propagates mathematical invalidation.

Compare three arms on the same task set and model:

| Arm | Allocation | Question |
| --- | --- | --- |
| One strong agent | Total budget B, with its own tools and iterative criticism | Does extended individual work suffice? |
| Isolated portfolio | k agents dividing B, no shared discoveries | What does independent search buy? |
| Sharing group on Locust | k agents dividing B, with the same evaluator | What does communication and reuse add? |

Include synthesis, review and verification in the accounting. Fix the budget
unit in advance, report wall time separately, and record actual usage. Across
repeated tasks/runs, measure verified success, time to first verified result,
false acceptance, duplicated work, reuse of findings, and total cost. Evaluate
candidate artifacts without giving the evaluator an arm label. The sharing arm
must beat the isolated portfolio to establish a benefit from collaboration.
Testing heterogeneous models is a subsequent variable, not a substitute for
that comparison.

This follows the unresolved question already recorded in
[the swarm-evidence assessment](swarm-evidence.md) and
[the ecdsa.fail prior-art assessment](ecdsa-fail-swarm-prior-art.md).
The OpenAI material strengthens the case that coordinated research is worth
testing. It does not yet supply Locust's missing measurement.
