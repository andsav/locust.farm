# Public prior art for agent research on ecdsa.fail

Source review dated 2026-10-03. Nothing was executed or submitted. This note
compares public solver self-reports and published multi-agent studies alongside
the [benchmark contract](ecdsa-fail-benchmark.md). Counts are reproducible from
the [reduced public leaderboard capture](evidence/ecdsa-fail-leaderboard-analysis.md).
A solver's report is evidence about its stated setup, not an independently
qualified Locust experiment.

## How public solvers organise their agents

All from the solvers' own notes and posts.

- **A lead session with several sub-agents in separate worktrees, on one desktop.** The solver who made 34 of the last 49 promotions describes a lead model with sub-agents each owning one family of changes, a one-at-a-time ablation of 62 settings with a documentation folder per setting, paired evaluation on shared seeds to price each change, SAT-proved exact rewrites, and a GPU screen for nonce search. The hardware named in the notes is one desktop-class processor at a time and one to three consumer graphics cards. The GPU screen found a passing nonce in 13 minutes on a September circuit; at the record's failure rate the same throughput implies hours. The edge is tooling built for this circuit family, not scale.
- **Role contracts and gates.** One [write-up](https://www.rubenmarcus.dev/blog/the-agent-swarm-that-took-1-on-ecdsa-fail) describes nine agents with fixed roles, a falsifier queue every candidate must survive before it reaches the frontier, a human approval gate on GPU spend, file-queue lanes, a curated digest and a $200 cap, and concludes that the models were interchangeable and the gates were not.
- **Large fleets early on.** One early leader describes explorer, theorist, optimizer and synthesizer roles, later 14 agents on more than a thousand vCPUs.
- **The crowd as a whole.** A post on the organisers' [blog](https://www.eigenlabs.org/blog/open-autoresearch-the-ecdsa-fail-story/), written by the challenge paper's first author, says the organisers' internal agents plateaued before the challenge opened, and relays a press report that the crowd matched Google's result within eight hours and passed it in about 72. Our capture reproduces the second figure, at about 73 hours, and not the first.
- **Declared models.** Acceptance varies widely inside each vendor: Claude Opus 4.8 72% (150 of 209), Opus 5.5 71%, but Opus 5 32%; GPT-5 43%, GPT-5 Codex 60%, GPT-6 Astra none of 87. The rates track which campaigns a solver ran, since every low-qubit submission is rejected whatever produced it. 19 of October's 20 promotions are one solver using Claude Opus; the twentieth, the current record, declares GPT-6 in Codex. Model choice is not settled by this data.

Every one of these is one operator's private setup. Between operators the shared channels are the promoted branch, free-text notes on accepted and rejected submissions, the rejected branches themselves, a few issues, optional chat channels, and a memory directory that has been deleted repeatedly (see the [benchmark note](ecdsa-fail-benchmark.md)). The challenge paper names the paradigm *open autoresearch* ([arXiv 2609.09582](https://arxiv.org/abs/2609.09582)) and reports that participants' own experiment logs reduced duplicated effort. What the public channels lack is structure: negative results and measurements exist only as unindexed prose, mostly tied to circuits since replaced, and nothing records who has reproduced what.

## What published work says

**Population search keeps alternatives alive.** [FunSearch](https://www.nature.com/articles/s41586-023-06924-6) runs islands and periodically reseeds the weakest from the best. [AlphaEvolve](https://arxiv.org/abs/2506.13131) combines islands with an archive of diverse elites, cascades evaluation from cheap to expensive, and reports that optimising several metrics often helps the target metric. [ShinkaEvolve](https://arxiv.org/abs/2509.19349) rejects near-duplicate proposals before evaluating them, and uses a bandit rewarded on improvement over the parent to pick which model proposes the next change.

**One preprint reports the matched comparison.** [CORAL](https://arxiv.org/abs/2604.01658) is also the closest design analogue to a shared-record experiment: agents in isolated worktrees, a shared persistent memory of attempts and notes, and an evaluator kept apart from the agents. It ran four agents sharing memory against the best of four independent runs at equal wall-clock time on three scored optimisation tasks, and sharing won on all three. It is one paper and three tasks, matched on wall clock and not on tokens, and not this benchmark.

**The closest published analogue to a peer swarm herded.** [Agora](https://arxiv.org/abs/2609.18094) ran 13 workers for 12 days over an append-only, typed, shared record with cross-account verification. One lineage absorbed most follow-on work until the authors added diversity-aware recommendations and told workers to read them. They say that showing a gain per unit of compute still needs a matched comparison.

**Several agents win on parallel, context-isolated, checkable work and lose on coupled work.**

| Source | Result |
|---|---|
| [Anthropic, June 2025](https://www.anthropic.com/engineering/multi-agent-research-system) | Large gain on breadth-first research at about 15 times the tokens of a chat; token use explains most of the variance |
| [Anthropic, January 2026](https://claude.com/blog/building-multi-agent-systems-when-and-how-to-use-them) | Splitting agents by software role spent more on coordination than on work; a separate verifier works because it needs little context |
| [Anthropic, C compiler](https://www.anthropic.com/engineering/building-c-compiler) | 16 agents with lock files stalled when all hit the same monolithic task, until it was split with a reference oracle |
| [Cursor, January 2026](https://cursor.com/blog/scaling-agents) | Flat locks cut 20 agents to the throughput of two or three because locks were held too long; planners with isolated workers fixed it; a dedicated integrator role was tried and removed as a bottleneck |
| [Google and MIT](https://arxiv.org/abs/2512.08296) | Parallel tasks gained, sequential tasks lost; independent agents amplified errors far more than centrally checked ones |
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
7. **No silent locks and no silent expiry.** A claim that goes quiet is shown and raised for a decision. It is never held invisibly and never reassigned by a clock. (Cursor: agents held locks too long or never released them.)
8. **A thin planner and context-isolated workers; no pipeline of software roles and no free chat.** (Cursor for planner and isolated workers; Anthropic, January 2026, against splitting by role.)
9. **Publish before digging.** The first publication is a protocol state, and an empty inbox fails closed. (A proposed experiment condition; not measured here.)
10. **Typed negative results** that carry their base and that the next agent must adopt or challenge. (Solver notes credit each other's measured dead ends; the public dead-end claims that went stale carried no base.)
11. **Verification by a non-author is a first-class contribution.** (Agora; Anthropic, January 2026.)
12. **One evaluator authority.** Self-reported scores are leads: 17 of 592 claimed scores in the public data differ from the official one.
13. **Typed terminal status.** A stopped worker is never reported as done, and no model summarises work that did not happen. (A proposed experiment condition; not measured here.)
14. **Explore slots and a concentration alarm**, with the clustering threshold fixed before the run. (Agora.)
15. **A harness wrapped for agents**: one-line results, details in a file, fresh sessions. (The C compiler report.)
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

## Open question

Whether sharing beats independent agents on this benchmark at matched budget
remains unmeasured here. Published results on other tasks do not answer it.
