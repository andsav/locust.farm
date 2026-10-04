# Prior art for an agent swarm on ecdsa.fail

Research date: 2026-10-03. **Status: research note. Source review of the owner's earlier Merak work, public solver write-ups and published studies. Nothing was run. Revised the same day after an adversarial review.** It supports the [swarm demonstration proposal](../docs/ecdsa-fail-swarm-proposal.md) and sits beside the [benchmark note](ecdsa-fail-benchmark.md).

## Question

Who has already pointed several agents at this benchmark, what happened, and what do published studies say about when a group of agents beats one? The proposal should repeat what worked, and should make the known failures impossible, not merely unlikely.

## Sources

- The owner's Merak repository (`merak10`, a sibling checkout, at `677b18cbd`; July code at `7d979ffc`): `docs/archive/ECDSA_RESEARCH_DUET_BLUEPRINT_DESIGN_2026-07-09.md`, the four `ecdsa-*.json` built-in blueprints, `scripts/ecdsa_research_experiment.sh` and its tests, `docs/AGENT_SWARMS_IMPLEMENTATION_PLAN_2026-10-03.md` and `docs/LOCUST_POLARIS_PRODUCT_PLAN_2026-10-03.md`.
- The July experiment directory `ecdsa-experiments/20260710-031409-pubsub`: `RUN_REPORT.md`, `experiment.json` and the exported agent traces.
- Public submission notes on the leaderboard and public write-ups by solvers, linked below. These are self-reports. Counts over the notes are in the [leaderboard analysis appendix](evidence/ecdsa-fail-leaderboard-analysis.md).
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
| Workspace preflight | $0.03 | Stopped by the budget rule after 2 rounds |
| Corrected solo | $0.05 | Stopped after 3 rounds, nothing proposed |
| Staged duet, partial | $0.34 | One agent ended on a protocol error, the other on its budget; stopped by the operator |
| Live duet, own cap of $4.00 | $0.55 | 0 messages published, 0 dequeued; both researchers ended with status `cost_budget` |
| Total in the run report | $1.12 | No candidate was implemented or benchmarked |
| Uncapped rerun, not in the report | $3.91 | One researcher published 3 messages to a peer that had already exited; none was read; no candidate |

In the live run the editor then dequeued an empty inbox, wrote two generic experiments with itself as proposer, and the run ended as `completed`.

The incumbent itself was benchmarked once, by hand, on the owner's laptop the evening before (the last row of `results.tsv` in the July checkout). No benchmark ran inside the experiment's sandbox wrapper or on any candidate.

### Root cause

**Budget admission, not budget size.** Under a hard cap, every provider call first reserved a worst-case bound: six tokens per request byte at the cache-write rate, plus an output allowance that started at 128,000 tokens and was shrunk to fit what was left. Once the input bound alone exceeded the remaining cap, the call was refused. In the live run that happened after $0.44 of settled spend, about 11% of that run's $4.00 cap. The solo rerun stopped at 2% of its cap. This is reconstructed from Merak's code at the commit used and from trace arithmetic, because the refusal message was not kept. Merak's own later fix describes the old behaviour as rejecting capped requests "6x early".

Four more things turned waste into failure:

1. **Publishing came after reading.** The first publication sat behind an open-ended instruction to inspect the implementation, so an early stop left nothing durable.
2. **Aggregation failed open.** A budget stop was an ordinary node result, the edge to the editor was unconditional, and the editor was a model that was allowed to invent work.
3. **One agent was lost to an adapter defect.** One model's thinking-only responses were accepted as empty turns until its node ended in a protocol error: in the staged run, in three of its nine capped live rounds, and in the uncapped rerun, where it exited after three turns. Merak fixed this the same day. The uncapped rerun is therefore one agent alone, not a duet.
4. **Researchers could not falsify anything.** They had read-only file tools and no scorer.

The uncapped rerun shows what the fourth point costs. With money removed, the remaining researcher took 11 rounds to publish its first hypothesis, grew to 180,000-token prompts, retracted that hypothesis three rounds later because the code it targeted was unreachable, and never produced a candidate. It is one trajectory, so it illustrates the limit and does not rank the causes.

### What to keep, change and forbid

**Keep**

- Harness-owned scoring; model verdicts are advisory.
- The edit-boundary check, including ignored files.
- One worktree per candidate from a pinned base; no remotes; no network for challenge code; wrappers that refuse `git push` and submission-shaped commands.
- The experiment card: hypothesis, mechanism, files, expected Toffoli and qubit deltas, disconfirming result, novelty class.
- The validator verdict: valid, score, Toffoli, qubits, improved, boundary clean.
- Implementer is not validator. As run, proposer was not critic either; the design document paired them in two lanes.
- A solo control arm, and fake-agent tests before spending.

**Change**

- No shared worst-case reservation. Track settled spend per agent.
- Every worker gets a sandboxed scorer.
- A shared code map and technique ledger replace per-agent rereading.
- Durable claims and results on a board replace a two-agent dialogue that needs both alive.
- A deterministic reducer replaces the model editor.
- Many small scored iterations replace one shot.

**Make impossible**

- An agent ending with nothing published.
- An aggregator inventing work.
- A stopped agent reported as success.
- An empty model turn counted as progress.
- One agent's reservation starving another.
- Waiting on, or messaging, a dead peer.

Caveat on reuse: the safety wrapper is macOS-only and has only ever run against a fake fixture. The real `benchmark.sh` itself calls `sandbox-exec`, which commonly fails when nested, and the wrapper's profile denies reads under `/Users`, where the Rust toolchain lives. It needs one supervised test before anything depends on it.

## Merak's current swarm doctrine, and the split with Locust

Merak's swarm plan of 2026-10-03 describes a tree with owners, not a room of agents talking. A planner adds durable work items; a pool keeps a few fresh workers busy, one item each; the runtime, not the worker, records claims and completion; each item's check is part of the work; the human watches by exception. Costs are observed and shown, and a ceiling exists only when the owner names its amount and scope. Work whose worker stopped does not restart by itself: it stops and asks. Of this, the monitoring slices landed on Merak's main branch on 2026-10-03, with the agents view behind a flag that is off by default. The pool, the work items and the removal of built-in cost ceilings are still planned; three of the four ECDSA blueprints still carry a $20 ceiling.

The product plan of the same date puts the boundary where this proposal needs it. Locust owns identity and membership, assignment and claims, durable history, shared context, artifacts and authoritative acceptance. Merak is one optional local executor. Polaris is the product surface for creating and observing swarms. Other clients bring their own execution through Locust adapters. A swarm is the product view of a Locust goal. The plan defers Polaris integration until Locust works. Of the state distinctions it lists, the proposal relies on two: task versus attempt, and submitted versus accepted versus applied.

Merak has no result showing several agents beating one on this benchmark.

## How public solvers organise their agents

All from the solvers' own notes and posts.

- **A lead session with several sub-agents in separate worktrees, on one desktop.** The solver who made 34 of the last 49 promotions describes a lead model with sub-agents each owning one family of changes, a one-at-a-time ablation of 62 settings with a documentation folder per setting, paired evaluation on shared seeds to price each change, SAT-proved exact rewrites, and a GPU screen that finds a passing nonce in minutes. The hardware named in the notes is one desktop processor and up to three consumer graphics cards. The edge is tooling built for this circuit, not scale.
- **Role contracts and gates.** One [write-up](https://www.rubenmarcus.dev/blog/the-agent-swarm-that-took-1-on-ecdsa-fail) describes nine agents with fixed roles, a falsifier queue every candidate must survive before it reaches the frontier, a human approval gate on GPU spend, file-queue lanes, a curated digest and a $200 cap, and concludes that the models were interchangeable and the gates were not.
- **Large fleets early on.** One early leader describes explorer, theorist, optimizer and synthesizer roles, later 14 agents on more than a thousand vCPUs.
- **The crowd as a whole.** A post on the organisers' [blog](https://www.eigenlabs.org/blog/open-autoresearch-the-ecdsa-fail-story/), written by the challenge paper's first author, says the organisers' internal agents plateaued before the challenge opened, and relays a press report that the crowd matched Google's result within eight hours and passed it in about 72. Our capture reproduces the second figure, at about 73 hours, and not the first.
- **Declared models.** Acceptance varies widely inside each vendor: Claude Opus 4.8 72% (150 of 209), Opus 5.5 71%, but Opus 5 32%; GPT-5 43%, GPT-5 Codex 60%, GPT-6 Astra none of 87. The rates track which campaigns a solver ran, since every low-qubit submission is rejected whatever produced it. 19 of October's 20 promotions are one solver using Claude Opus; the twentieth, the current record, declares GPT-6 in Codex. Model choice is not settled by this data.

Every one of these is one operator's private setup. Between operators the shared channels are the promoted branch, free-text notes on accepted and rejected submissions, the rejected branches themselves, a few issues, optional chat channels, and a memory directory that has been deleted repeatedly (see the [benchmark note](ecdsa-fail-benchmark.md)). The challenge paper names the paradigm *open autoresearch* ([arXiv 2609.09582](https://arxiv.org/abs/2609.09582)) and reports that participants' own experiment logs reduced duplicated effort. What the public channels lack is structure: negative results and measurements exist only as unindexed prose, mostly tied to circuits since replaced, and nothing records who has reproduced what.

## What published work says

**Population search keeps alternatives alive.** [FunSearch](https://www.nature.com/articles/s41586-023-06924-6) runs islands and periodically reseeds the weakest from the best. [AlphaEvolve](https://arxiv.org/abs/2506.13131) combines islands with an archive of diverse elites, cascades evaluation from cheap to expensive, and reports that optimising several metrics often helps the target metric. [ShinkaEvolve](https://arxiv.org/abs/2509.19349) rejects near-duplicate proposals before evaluating them, and uses a bandit rewarded on improvement over the parent to pick which model proposes the next change.

**One preprint reports the matched comparison.** [CORAL](https://arxiv.org/abs/2604.01658) is also the closest design analogue to this proposal: agents in isolated worktrees, a shared persistent memory of attempts and notes, and an evaluator kept apart from the agents. It ran four agents sharing memory against the best of four independent runs at equal wall-clock time on three scored optimisation tasks, and sharing won on all three. It is one paper and three tasks, matched on wall clock and not on tokens, and not this benchmark.

**The closest published analogue to a peer swarm herded.** [Agora](https://arxiv.org/abs/2609.18094) ran 13 workers for 12 days over an append-only, typed, shared record with cross-account verification. One lineage absorbed most follow-on work until the authors added diversity-aware recommendations and told workers to read them. They say that showing a gain per unit of compute still needs a matched comparison.

**Several agents win on parallel, context-isolated, checkable work and lose on coupled work.**

| Source | Result |
|---|---|
| [Anthropic, June 2025](https://www.anthropic.com/engineering/multi-agent-research-system) | Large gain on breadth-first research at about 15 times the tokens of a chat; token use explains most of the variance |
| [Anthropic, January 2026](https://claude.com/blog/building-multi-agent-systems-when-and-how-to-use-them) | Splitting agents by software role spent more on coordination than on work; a separate verifier works because it needs little context |
| [Anthropic, C compiler](https://www.anthropic.com/engineering/building-c-compiler) | 16 agents with lock files stalled when all hit the same monolithic task, until it was split with a reference oracle |
| [Cursor, January 2026](https://cursor.com/blog/scaling-agents) | Flat locks cut 20 agents to the throughput of two or three because locks were held too long; planners with isolated workers fixed it; a dedicated integrator role was tried and removed as a bottleneck |
| [Google and MIT](https://arxiv.org/abs/2512.08296) | Parallel tasks gained, sequential tasks lost; independent agents amplified errors far more than centrally checked ones. Merak's notes record that at equal total tokens the average was no better than one agent, and coding tasks lost |
| [CooperBench](https://arxiv.org/abs/2601.13295) | Lower success when two agents coordinate on coupled code |

**Evaluators get gamed.** [METR](https://metr.org/blog/2025-06-05-recent-reward-hacking/) saw one frontier model reward-hack in about 30% of runs on tasks where it could read the scoring function, and offers that visibility as one possible cause. On this benchmark the harness is the hardened descendant of code that [Trail of Bits](https://blog.trailofbits.com/2026/04/17/we-beat-googles-zero-knowledge-proof-of-quantum-cryptanalysis/) broke in April 2026, and nonce search is the standing example of optimising the measurement instead of the circuit.

## Design rules

Each rule names its basis.

1. **Archive a grid, not an incumbent**: the best validated result per qubit band and technique family. (AlphaEvolve; the challenge paper's caution about scalar scores; 279 Pareto advances were officially rejected.)
2. **Islands with scheduled reseeding.** Not every agent starts from the global best. (FunSearch; Agora's herding.)
3. **Track failure rate as a third axis** beside Toffolis and qubits, and state its unit. (Issue #131 on the upstream repository; the three conventions in the notes.)
4. **Cascade evaluation** from counting, through a paired screen on a few seeds, to a full estimate. (AlphaEvolve; a solver note records a multi-thousand-vCPU search wasted on a circuit that had only passed a count.)
5. **Separate thinking from grinding.** Nonce search is a queued compute service, never an agent's task. (FunSearch's ratio of samplers to evaluators.)
6. **Check a proposal against the ledger before spending evaluation.** (ShinkaEvolve's duplicate rejection; CORAL's shared memory.)
7. **Leases that expire, not locks.** (Cursor: agents held locks too long or never released them.) Merak's doctrine prefers explicit run state to clocks, so expiry should surface a task for a decision, not silently reassign it.
8. **A thin planner and context-isolated workers; no pipeline of software roles and no free chat.** (Cursor for planner and isolated workers; Anthropic, January 2026, against splitting by role; Merak's doctrine.)
9. **Publish before digging.** The first publication is a protocol state, and an empty inbox fails closed. (The July run.)
10. **Typed negative results** that carry their base and that the next agent must adopt or challenge. (Solver notes credit each other's measured dead ends; the public dead-end claims that went stale carried no base.)
11. **Verification by a non-author is a first-class contribution.** (Agora; Anthropic, January 2026.)
12. **One evaluator authority.** Self-reported scores are leads: 17 of 592 claimed scores in the public data differ from the official one.
13. **Typed terminal status.** A stopped worker is never reported as done, and no model summarises work that did not happen. (The July run.)
14. **Explore slots and a concentration alarm**, with the clustering threshold fixed before the run. (Agora.)
15. **A harness wrapped for agents**: one-line results, details in a file, fresh sessions. (The C compiler report; the July run's 180,000-token prompts.)
16. **Composition by script, and watch its queue.** (Cursor removed its integrator role as a bottleneck.)

## What a demonstration can honestly measure

| Claim | Measurement | Condition for honesty |
|---|---|---|
| Group versus one agent | Best coordinator-verified result per arm | Three arms: one agent with budget B; k independent agents with B/k each; k sharing agents with B/k each. State whether B is tokens or wall clock. A single run is an anecdote |
| Diversity | Archive cells holding a validated candidate; share of activity in the largest cluster | Cell boundaries fixed before the run |
| Duplicate work | Share of evaluated candidates whose operation stream hash matches an earlier one by another agent | The public data offers no clean baseline: 130 of 1,017 scored submissions repeat a score first posted by another solver, but nearly all are resubmissions of the current record |
| Time to first verified improvement | Wall clock and turns until a candidate beats the starting commit under the coordinator's paired measurement | Circuit changes and seed luck reported separately |
| Verification coverage | Share of results reproduced by a different member; false claims caught | The verifier works from the artifact hash, not the author's workspace |
| Reuse of negative results | Proposals that cite a negative record | Count citations; do not estimate tokens saved |

## Open questions

- Whether Merak's safety wrapper runs the real benchmark. One supervised test would settle it.
- How tomorrow's agents are billed. Subscriptions make a dollar cap meaningless and leave wall clock and turn counts as the practical bounds. For scale: the one uncapped July agent spent $3.91 in about ten minutes at metered prices.
- The technical quality of the two hypotheses from the uncapped rerun. Their text is in the experiment's database, not in the exported trace.
- Whether sharing beats independent agents on this benchmark at matched budget. CORAL's result is on other tasks; nothing has been measured here.
