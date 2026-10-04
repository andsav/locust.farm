# Prior art for an agent swarm on ecdsa.fail

Research date: 2026-10-03. **Status: research note. Source review of the owner's earlier Merak work, public solver write-ups and published studies. Nothing was run.** It supports the [swarm demonstration proposal](../docs/ecdsa-fail-swarm-proposal.md) and sits beside the [benchmark note](ecdsa-fail-benchmark.md).

## Question

Who has already pointed several agents at this benchmark, what happened, and what do published studies say about when a group of agents beats one? The proposal should repeat what worked, and should make the known failures impossible, not merely unlikely.

## Sources

- The owner's Merak repository (`merak10`, a sibling checkout, at `677b18cbd`): `docs/archive/ECDSA_RESEARCH_DUET_BLUEPRINT_DESIGN_2026-07-09.md`, the four `ecdsa-*.json` built-in blueprints, `scripts/ecdsa_research_experiment.sh` and its tests, `docs/AGENT_SWARMS_IMPLEMENTATION_PLAN_2026-10-03.md` and `docs/LOCUST_POLARIS_PRODUCT_PLAN_2026-10-03.md`.
- The July experiment directory `ecdsa-experiments/20260710-031409-pubsub`: `RUN_REPORT.md`, `experiment.json` and the exported agent traces.
- Public submission notes on the leaderboard and public write-ups by solvers, linked below. These are self-reports.
- Published studies and engineering reports, linked below.

Paths outside this repository are named, not linked. Figures quoted from Merak's notes about external studies are Merak's transcription unless a primary link is given.

## Merak's July attempt

### Design

A read-only *director* stage in which two models analyse the code independently, cross-examine each other, and a portfolio editor writes four experiments. Each experiment then runs in its own Git worktree through a critic, an implementer and a validator. A human picks the winner. The design document already listed what full autonomy would need: a host-owned evaluator, a candidate state that never merges by itself, and an archive of non-dominated (Toffoli, qubit) points.

The paid run compared three arms from one pinned commit under a single $20 cap: one agent alone; a staged duet connected only by graph edges; and a live duet exchanging messages on three publish/subscribe topics.

Authority and safety were sound and are worth keeping. The harness, never a model, decided validity. It listed every changed, untracked and ignored path against the base and refused anything outside `src/point_add/`. Agents worked in a clone with no remotes, `git push` and submission-shaped commands were wrapped to fail, and the benchmark was to run under a macOS sandbox profile with no network.

### What happened

| Run | Spend | Outcome |
|---|---|---|
| First three-arm run | $0.15 | The solo agent saw an empty workspace; both duets died within seconds on a provider error |
| Corrected solo | $0.05 | Stopped after 3 rounds, nothing proposed |
| Staged duet, partial | $0.34 | One agent ended on a protocol error, the other on its budget; stopped by the operator |
| Live duet | $0.55 | 0 messages published, 0 dequeued; both researchers ended with status `cost_budget` |
| Total in the run report | $1.12 | No candidate implemented; the benchmark never ran |
| Uncapped rerun, not in the report | $3.91 | 3 peer messages, no candidate |

In the live run the editor then dequeued an empty inbox, wrote two generic experiments with itself as proposer, and the run ended as `completed`.

### Root cause

**Budget admission, not budget size.** Under a hard cap, every provider call first reserved its worst-case cost: six tokens per request byte plus the full 128,000-token output allowance. Once a request passed roughly 47 KB, the reservation alone exceeded the remaining headroom and the call was refused, with about 11% of the cap actually spent. This is reconstructed from Merak's code at the commit used and from trace arithmetic, because the refusal message was not kept. Merak's own later fix describes the old behaviour as rejecting capped requests "6x early".

Three protocol properties turned waste into failure:

1. **Publishing came after reading.** The first publication sat behind an open-ended instruction to inspect the implementation, so an early stop left nothing durable.
2. **Aggregation failed open.** A budget stop was an ordinary node result, the edge to the editor was unconditional, and the editor was a model that was allowed to invent work.
3. **Researchers could not falsify anything.** They had read-only file tools and no scorer.

The uncapped rerun shows the third point is the deep one. With money removed, one researcher still took 11 rounds to publish its first hypothesis, grew to 180,000-token prompts, retracted that hypothesis three rounds later because the code it targeted was unreachable, and never produced a candidate.

### What to keep, change and forbid

| Keep | Change | Make impossible |
|---|---|---|
| Harness-owned scoring; model verdicts advisory | No shared worst-case reservation; track settled spend per agent | An agent ending with nothing published |
| Edit-boundary check that includes ignored files | Every worker gets a sandboxed scorer | An aggregator inventing work |
| One worktree per candidate from a pinned base; no remotes; no network | A shared code map and technique ledger replace per-agent rereading | A stopped agent reported as success |
| The experiment card: hypothesis, mechanism, files, expected Toffoli and qubit deltas, disconfirming result, novelty class | Durable claims and results on a board replace a two-agent dialogue that needs both alive | One agent's reservation starving another |
| The validator verdict: valid, score, Toffoli, qubits, improved, boundary clean | A deterministic reducer replaces the model editor | Agents starting before the incumbent score has been reproduced |
| Proposer is not critic; implementer is not validator | Many small scored iterations replace one shot | Waiting on, or messaging, a dead peer |
| A solo control arm; fake-agent tests before spending | | |

Caveat on reuse: the safety wrapper is macOS-only and has only ever run against a fake fixture. The real `benchmark.sh` itself calls `sandbox-exec`, which commonly fails when nested, and the wrapper's profile denies reads under `/Users`, where the Rust toolchain lives. It needs one supervised test before anything depends on it.

## Merak's current swarm doctrine, and the split with Locust

Merak's swarm plan of 2026-10-03 describes a tree with owners, not a room of agents talking. A planner adds durable work items; a pool keeps a few fresh workers busy, one item each; the runtime, not the worker, records claims and completion; each item's check is part of the work; the human watches by exception. Costs are observed and shown, and a ceiling exists only when the owner names its amount and scope. Of this, the monitoring half has shipped in Merak. The pool and work items are still planned.

The product plan of the same date puts the boundary where this proposal needs it: Locust owns identity and membership, assignment and claims, durable history, shared context, artifacts and authoritative acceptance. Merak is one optional local executor, Polaris is a view, and other clients bring their own execution through Locust adapters. A swarm is the product view of a Locust goal. That plan defers Polaris integration until Locust works, and it keeps four distinctions that this proposal reuses: task versus attempt, cancel request versus actual stop, submitted versus accepted versus applied, and reported versus missing usage.

Merak has no result showing several agents beating one on this benchmark.

## How public solvers organise their agents

All from the solvers' own notes and posts.

- **A lead with sub-agents in separate worktrees, one family of changes each.** The solver who made 34 of the last 49 promotions describes a lead model with five worktree sub-agents, a one-at-a-time ablation of 62 settings to find which were loose, paired evaluation on identical inputs to price each change, and SAT-proved exact rewrites.
- **Role contracts and gates.** One [write-up](https://www.rubenmarcus.dev/blog/the-agent-swarm-that-took-1-on-ecdsa-fail) describes nine agents with fixed roles, a falsifier that must pass before compute or submission, file-queue lanes, a curated digest and a $200 cap, and concludes that the models were interchangeable and the gates were not.
- **Large private fleets.** One early leader describes explorer, theorist, optimizer and synthesizer roles, later 14 agents on a fleet of more than a thousand vCPUs. The organisers' [blog post](https://www.eigenlabs.org/blog/open-autoresearch-the-ecdsa-fail-story/) says their internal agents plateaued and the open crowd matched Google's figure in 8 hours.
- **Declared models and harnesses.** Of notes that declare a model, Claude Opus and Fable lines are accepted 70–77% of the time and the GPT-5 family 43–60%; October's promotions are almost all Claude Opus. Counts are confounded by who chose to submit what.

Every one of these is one operator's private fleet. Between operators the only shared channels are the promoted `main` branch, public notes, and a memory directory that has been wiped three times (see the [benchmark note](ecdsa-fail-benchmark.md)). The organisers' paper names the paradigm *open autoresearch* and describes independent teams building on verified results through a public leaderboard ([arXiv 2609.09582](https://arxiv.org/abs/2609.09582)). That channel carries winners. It does not carry negative results, measurements with their conditions, or who verified what.

## What published work says

**Population search keeps alternatives alive.** [FunSearch](https://www.nature.com/articles/s41586-023-06924-6) runs islands and periodically reseeds the weakest from the best. [AlphaEvolve](https://arxiv.org/abs/2506.13131) combines islands with an archive of diverse elites, cascades evaluation from cheap to expensive, and reports that optimising several metrics often helps the target metric. [ShinkaEvolve](https://arxiv.org/abs/2509.19349) rejects near-duplicate proposals before evaluating them and allocates effort with a bandit rewarded on improvement over the parent.

**The closest published analogue herded.** [Agora](https://arxiv.org/abs/2609.18094) ran 13 workers for 12 days over an append-only, typed, shared record with cross-account verification. One lineage absorbed most follow-on work until humans deployed a diversity-aware allocator, and the authors say that showing a gain per unit of compute still needs a matched comparison.

**Several agents win on parallel, context-isolated, checkable work and lose on coupled work.**

| Source | Result |
|---|---|
| [Anthropic, June 2025](https://www.anthropic.com/engineering/multi-agent-research-system) | Large gain on breadth-first research at about 15 times the tokens of a chat; token use explains most of the variance |
| [Anthropic, C compiler](https://www.anthropic.com/engineering/building-c-compiler) | 16 agents with lock files stalled when all hit the same bug, until the task was split |
| [Cursor, January 2026](https://cursor.com/blog/scaling-agents) | Flat locks cut 20 agents to the throughput of two or three; a planner with workers fixed it |
| [Google and MIT](https://arxiv.org/abs/2512.08296) | Parallel tasks gained, sequential tasks lost; independent agents amplified errors far more than centrally checked ones |
| [CooperBench](https://arxiv.org/abs/2601.13295) | Lower success when two agents coordinate on coupled code |

**Evaluators get gamed.** [METR](https://metr.org/blog/2025-06-05-recent-reward-hacking/) observed reward hacking in a large share of runs where the scoring code was visible. On this benchmark the harness itself is the hardened descendant of code that [Trail of Bits](https://blog.trailofbits.com/2026/04/17/we-beat-googles-zero-knowledge-proof-of-quantum-cryptanalysis/) broke in April 2026, and nonce search is the standing example of optimising the measurement instead of the circuit.

No published matched-compute study shows that shared state beats independent agents on a scored optimisation task.

## Design rules

Each rule names its basis.

1. **Archive a grid, not an incumbent**: the best validated result per qubit band and technique family. (AlphaEvolve; the scalar leaderboard's suppression of designs that regress first.)
2. **Islands with scheduled reseeding.** Not every agent starts from the global best. (FunSearch; Agora's herding.)
3. **Track failure rate as a third axis** beside Toffolis and qubits. (Issue #131 on the upstream repository.)
4. **Cascade evaluation** from counting, through paired pricing on a few inputs, to a full estimate. (AlphaEvolve; a solver note records a multi-thousand-vCPU search wasted on a circuit that had only passed a count.)
5. **Separate thinking from grinding.** Nonce search is a queued compute service, never an agent's task. (FunSearch's ratio of samplers to evaluators.)
6. **Check novelty before spending evaluation.** (ShinkaEvolve; 215 of 1,017 scored public submissions share a score with another.)
7. **Leases that expire, not locks.** (Cursor; the C compiler team.)
8. **A thin planner and context-isolated workers; no role pipeline and no free chat.** (Cursor; Anthropic; Merak's doctrine.)
9. **Publish before digging.** The first publication is a protocol state, and an empty inbox fails closed. (The July run.)
10. **Typed negative results** that the next agent must adopt or challenge. (Solver notes credit each other's measured dead ends.)
11. **Verification by a non-author is a first-class contribution.** (Agora; Anthropic's verifier pattern.)
12. **One evaluator authority.** Self-reported scores are leads: 17 of 592 claimed scores in the public data differ from the official one.
13. **Typed terminal status.** A stopped worker is a failed attempt. (The July run.)
14. **Mandatory explore slots and a concentration alarm.** (Agora.)
15. **A harness wrapped for agents**: one-line results, details in a file, fresh sessions. (The C compiler report; the July run's 180,000-token prompts.)

## What a demonstration can honestly measure

| Claim | Measurement | Condition for honesty |
|---|---|---|
| Group versus one agent | Best coordinator-verified result per arm at equal total tokens | Three arms: one agent with budget B; k independent agents with B/k each; k sharing agents with B/k each. A single run is an anecdote |
| Diversity | Archive cells holding a validated candidate; share of activity in the largest cluster | Cell boundaries fixed before the run |
| Duplicate work | Share of evaluated candidates whose operation stream matches an earlier one by another agent | Public upper bound for comparison: 215 of 1,017 |
| Time to first verified improvement | Wall clock and tokens until a candidate beats the starting commit under the coordinator's evaluation | Circuit changes and seed luck reported separately |
| Verification coverage | Share of results reproduced by a different peer; false claims caught | The verifier works from the artifact hash, not the author's workspace |
| Reuse of negative results | Proposals that cite a negative record | Count citations; do not estimate tokens saved |

## Open questions

- Whether Merak's safety wrapper runs the real benchmark. One supervised test would settle it.
- How tomorrow's agents are billed. Subscriptions make a dollar cap meaningless and leave wall clock and turn counts as the practical bounds.
- The technical quality of the two hypotheses from the uncapped rerun. Their text is in the experiment's database, not in the exported trace.
- Whether any matched-compute comparison of sharing against independent agents exists for a scored benchmark. We found none.
