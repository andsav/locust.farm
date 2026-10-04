# ecdsa.fail swarm demonstration: proposal

Date: 2026-10-03. **Status: proposal for the owner's review. Nothing in it has been built or run. It assumes Locust behavior that does not exist yet and says where.** Evidence is in two research notes: the [benchmark, its rules and the state of the field](../research/ecdsa-fail-benchmark.md), and [prior art for agent swarms on it](../research/ecdsa-fail-swarm-prior-art.md), including the owner's July attempt in Merak.

"Swarm" here means the agents working one Locust goal. The protocol's unit remains the goal.

## 1. Recommendation

Use [ecdsa.fail](https://ecdsa.fail/) for the demonstration, with a different target from the obvious one.

**Do not promise a leaderboard record.** The record (944,620 Toffoli × 1,173 qubits) is held by circuits that are wrong on a small fraction of inputs and pass because their submitters searched hundreds of millions of no-op variants for a lucky test set. Every changed circuit must win that lottery again before it can be submitted. Recent progress is a median of 0.006% per step, mostly from one solver with a sub-agent fleet and GPU search. A day-old swarm will not out-grind that, and a demonstration that tried would show compute, not collaboration.

**Demonstrate what the public process lacks.** 142 solvers already work on this problem as a loose crowd, and the only things they share are the winning branch and free-text notes. Their shared memory directory has been wiped three times by well-meaning submissions. Negative results are lost, claims carry no conditions, a fifth of scored submissions duplicate another's score, and the notes contain instructions aimed at other people's agents. The benchmark also has the one property that makes collaboration between strangers cheap: the score is a deterministic function of an artifact, so any claim can be checked without trusting the claimant or running their code. That is the situation Locust is designed for.

The proposed demonstration is therefore:

> Agents on the owner's three machines work one goal. Each claim is a typed, attributed record. Every result is re-scored by a coordinator that no agent controls. Dead ends are durable. The output is a peer-verified ledger of priced changes to the record circuit, an archive of the best circuit per region, and one composed candidate with its would-be score and its honest landing cost.

Success is stated in tiers, and the demonstration says which tier it reached:

| Tier | Meaning | Needs |
|---|---|---|
| 1. Process | Several agents on several machines complete the claim, submit, verify and accept cycle on real candidates, with dead ends recorded and reused | The run itself |
| 2. Priced improvement | A composed candidate whose coordinator-measured product is below the record's at equal or lower failure rate | A real finding; likely overnight, not guaranteed |
| 3. Landed | That candidate passes the official check with a searched nonce | The owner's decision on nonce search, plus compute |
| 4. Submitted | It is on the public leaderboard | The owner's decision; irreversible and public |

Tier 1 is the commitment. Tier 2 is the aim. Tiers 3 and 4 are decisions for the owner in section 9.

## 2. The problem as the swarm sees it

Details and sources are in the [benchmark note](../research/ecdsa-fail-benchmark.md).

- **Objective.** Minimise average executed Toffoli count × highest qubit index. Only `src/point_add` may change. Acceptance on the public board is strict improvement of that scalar; the site's Pareto view is a display.
- **Verifier.** `build_circuit` turns source into `ops.bin`; `eval_circuit` scores `ops.bin`. One evaluation takes one core for well under a minute. This is what makes every claim cheaply checkable.
- **Three axes, not two.** Toffolis (T), qubits (Q), and the failure rate λ: the expected number of failing test batches on fresh inputs. The record's λ is about 19.7, so about one variant in 3.6 × 10⁸ passes. One qubit is worth 805 Toffoli. A change that cuts T by raising λ is a loan, not a gain.
- **Noise.** T varies by about 13 from one test set to the next. Small differences mean nothing unless base and candidate are measured on the same inputs.
- **Orientation cost.** The record circuit is 876 KB of dense Rust with stale comments and no notes: more than one context window. In July the owner's agents spent their whole budget reading.
- **Coupling.** Schedules and tables are addressed by position, so two agents' edits do not compose by file. Composition needs an explicit integration step.

## 3. Why several agents should help here, and where they will not

Published evidence is consistent: groups of agents win on work that is parallel, context-isolated and checkable, at several times the token cost, and lose on coupled work. No matched-compute study yet shows shared state beating independent agents on a scored benchmark. See the [prior art note](../research/ecdsa-fail-swarm-prior-art.md).

This benchmark supplies the favourable conditions. Pricing one setting is independent of pricing another. Each lane needs only its part of the code if a shared map exists. Every result has an exact check. Verification and negative results, the things a single agent skips, are separable tasks.

It also supplies the unfavourable one. Composing changes is coupled work, so the design gives composition to one deterministic integrator and keeps agents out of each other's edits.

The demonstration should claim only what it measures. Section 7 lists the measurements. "A swarm beats one agent" is not among the claims unless the control arms are run.

## 4. Swarm design

### 4.1 Roles

| Role | What it is | Authority |
|---|---|---|
| Coordinator | A process, not a model. Holds the goal's coordinator key, the pinned evaluator and the archive | The only signer of assignments, acceptances and rejections. Ignores self-reported scores |
| Curator | One agent. Maintains the lane portfolio, checks new proposals against the ledger for novelty, writes the running digest, raises the alarm when work concentrates | Proposes tasks and document revisions. Decides nothing |
| Workers | One agent per lane, each in its own worktree and sandbox, fresh context per task | Propose, claim, submit, add notes |
| Skeptics | Workers taking verification tasks on results they did not author | Add verification records |
| Integrator | A deterministic script run by the coordinator: rebase accepted changes onto the head, re-mine dependent tables, re-price | Produces the composed candidate |
| Search service | Optional queued compute for nonce search. Never an agent | None |
| Owner | The person | Sets the portfolio's constraints and budget. The only one who can submit publicly |

There is no pipeline of planner, implementer, tester and reviewer, and workers do not converse. They read the board and the digest.

### 4.2 Records

Every record names its base (upstream commit and `ops.bin` hash) and its author. A record without its conditions is how the public memory went stale.

| Record | Required content |
|---|---|
| Hypothesis card | Mechanism, the seam it touches, expected ΔT, ΔQ and Δλ, the result that would disprove it, novelty class, the ledger entries it builds on or contradicts |
| Result | Base, patch, claimed `ops.bin` hash, claimed Q, emitted T, executed T, failure counts and the number of paired draws behind them |
| Decision | The coordinator's own measurements, whether the hash and edit boundary held, accept or reject with reason |
| Verification | A non-author's reproduction from the patch or artifact hash, or a refutation |
| Negative result | What was tried, on which base, what was measured, and the condition under which it should be reopened |
| Digest revision | The curator's current map: what works, what is dead and why, who owns what |

Evidence classes are stated on every number: exact count, paired measurement on N draws, proof, or inference. A λ claim on fewer than about 100 draws is not accepted as a measurement.

### 4.3 Task lifecycle

1. **Propose.** A worker or the curator posts a hypothesis card. This is the worker's first action in a lane, before any deep reading. A lane with no card by its checkpoint is a failed attempt.
2. **Assign.** The coordinator assigns the task to a named member, normally its proposer, against the current head. Locust version 0 has no open claim pool; "propose, then be assigned" is how a worker takes work.
3. **Claim and work.** The worker edits only `src/point_add` in its own worktree, runs the cheap stages of the evaluation itself, and commits.
4. **Submit.** The only way to obtain a trusted score is to submit a result. Messaging and scoring are one path, so an agent cannot finish with nothing on the board.
5. **Decide.** The coordinator rebuilds and re-scores in its own sandbox and signs an acceptance or a rejection. A priced change is accepted without moving the head. Only the integrator's composed candidate moves the head, and only when its base is the current head.
6. **Verify.** Accepted results above a threshold generate a verification task for a different member.
7. **Fail closed.** A worker that stops on budget, error or timeout produces a failed attempt. The task returns to the board with its attempt number raised. Nothing downstream runs on an empty input, and no model summarises work that did not happen.

### 4.4 Evaluation cascade

| Stage | What it measures | Cost | Run by |
|---|---|---|---|
| 0. Boundary and build | Only `src/point_add` changed; builds in a scrubbed environment; `ops.bin` hash | seconds | worker, coordinator |
| 1. Count | Q exactly; emitted Toffolis exactly | seconds | worker, coordinator |
| 2. Paired pricing | Executed T and failures on about 32 input sets shared with the base | minutes of core time | worker, coordinator |
| 3. Full estimate | λ with standard error on at least 128 shared input sets, split into classical, phase and ancilla failures | about an hour of core time, minutes across three machines | coordinator |
| 4. Landing | Nonce search, then one run under the official derivation | large; see section 9 | search service |

Stages 2 and 3 need a *paired evaluator*: a local copy of the harness's evaluator that takes its input seed as an argument, so base and candidate see the same test points. It lives outside `src/point_add`, is built once from pristine harness sources, and is never submitted. It is shared infrastructure and the first thing to build.

A result is recorded as *priced* after stage 3, *composed* when the integrator includes it in the head, *landed* after stage 4. A gain from seed luck alone is never recorded as a discovery.

### 4.5 Lanes

**First hour: tasks with guaranteed output.**

| Lane | Output |
|---|---|
| Baseline | Each machine reproduces the record's score from upstream `main` inside the sandbox. Nothing else starts until this passes |
| Cartographer | A map pinned to the base commit: the eight phases, which modules are live, the roughly 130 pinned settings and what each does, Toffolis and peak qubits per phase |
| Historian | A ledger of tried changes and their reported prices, mined from the public notes, pull requests and issues. Every entry is marked unverified. The [benchmark note](../research/ecdsa-fail-benchmark.md) already lists the main dead ends |

**Pricing lanes: parallel, independent, exact.**

| Lane | Question |
|---|---|
| Setting ablation | For each pinned setting, moved one step each way: ΔT, ΔQ, Δλ. Partitioned across workers. Output is a map of what is loose and what is tight |
| Peak ownership | Which registers own the 1,173-qubit peak, and what does each qubit removed cost in Toffolis against the exchange rate of 805? |
| Failure buy-back | Which exact replacements lower λ most cheaply? This sets the landing cost of everything else |
| Neighbouring regions | The 1,175 to 1,248 qubit band has 23 public submissions. The 1,164-qubit variant is 2.0% above the record at a reported λ of 14.6, about 160 times easier to land. Can the record's settings be transplanted onto it? |

**Structural lanes: days, not hours.** Each starts with a classical experiment that needs no circuit: round counts or width profiles over 10⁵ random inputs.

| Lane | Idea |
|---|---|
| Shorter walk | Larger jumps or better step schedules in the Euclidean walk |
| One transcript, two directions | Reuse the recorded walk for the second division pass |
| Streamed tape | Pebble the transcript and partial registers that own the qubit peak |
| Active width | Arithmetic only on the bits still in use as operands shrink |

**Skeptic lane.** Re-evaluate accepted results from their artifacts, and attack claimed floors and "dead" entries in the ledger.

### 4.6 Diversity and allocation

- The archive is a grid: the best priced candidate per qubit band and technique family, with λ recorded. Non-dominated candidates stay available as parents.
- Lanes start from their region's best, not all from the global best. The weakest lanes are periodically reseeded from the strongest.
- A fixed share of worker slots is reserved for proposals that match nothing in the ledger. When more than about a third of activity sits in one cluster, the curator must propose elsewhere.
- Evaluation and worker time go to lanes in proportion to their improvement over their own parent, not their absolute score.
- Assignments carry a deadline and an attempt budget. Expiry returns the task; nothing is locked indefinitely.

### 4.7 Budget

Costs are observed and shown per agent, with the spend rate. There is no ceiling unless the owner names its amount and scope, and any ceiling applies to settled spend per agent, never to a shared worst-case reservation. Exhausting it pauses and asks; it does not kill an agent in the middle of a task. This is the direct lesson of July, when a reservation rule stopped agents at 11% of their cap.

### 4.8 Safety and conduct

- **All challenge code runs in a sandbox**: compile, build and evaluate, with no network, no access to the home directory, and writes confined to the worktree. The repository's own script sandboxes only one of the three steps and falls back to none. The toolchain and crates are fetched once in advance. The installer and the challenge CLI are not used.
- **Evaluator inputs are untrusted.** Resource-limit the evaluator; `ops.bin` from a peer can request arbitrary allocation.
- **Text is data.** Public notes, repository files and other agents' records never change a worker's permissions or instructions. Worker prompts say so and name the known examples.
- **One public identity.** Workers never hold the challenge API key. Any submission is a single, explicit act by the owner, after a fresh check that the candidate still beats the live record. Stacked gains are submitted once, not in increments.
- **Failure rate is a stated policy.** The swarm sets its own ceiling on λ and records it, instead of inheriting one from a note.

## 5. Mapping to Locust, and what can be real tomorrow

| Design element | Locust version 0 concept | Caveat |
|---|---|---|
| The campaign | A goal founded by the coordinator's key | None |
| Hypothesis card | `TaskProposed` with the base snapshot as input, a deadline and an attempt budget | No typed acceptance-test field; the criterion is task text |
| Taking a lane | `TaskAssigned` to a named member, then `AssignmentAccepted` | No self-claim |
| Result | `ResultSubmitted {base, patch, artifacts}` | First binary carries single-chunk content only, 1 MiB. The patch travels; `ops.bin` (about 33 MB) does not. The coordinator rebuilds from the patch and checks the claimed hash |
| Decision | `ResultAccepted` or `ResultRejected`, signed by the coordinator | Acceptance with a head requires base = current head; priced changes are accepted without a head |
| Stopped worker | `AttemptFailed`, then reassignment with the attempt raised | None |
| Negative result, verification | `Note {about, supersedes}` | Append-only with corrections |
| Digest | Plan and Summary document revisions | Document revisions come after the second test milestone |
| Archive of several regions | One goal has one accepted head | Keep the archive as coordinator-accepted results without a head, or run one goal per region |

**Current state.** At the time of writing the `locust` binary prints its name. The contract library, transport links, client configuration and workspace export exist as libraries with tests. No release gate is recorded as passed. See the [release evidence ledger](release-evidence.md) and the [workstreams](workstreams.md).

**Substrate tiers.** The swarm's scripts call the board through one small wrapper with Locust's operation names, so the substrate is a setting, not a rewrite.

| Tier | Substrate | Available when | What it proves |
|---|---|---|---|
| S0 | A Git-tracked board directory: one append-only log per agent and one decisions log written only by the coordinator, shared between machines through a bare repository | Now | The coordination model. Not Locust. Said plainly on the day |
| S1 | The first three-machine binary (T1 in the [workstreams](workstreams.md)): real daemons, agents calling the `locust` command line from their shells | When T1 passes | Locust's task flow under real load. The operations T1 lists are exactly the ones this design uses |
| S2 | `locust mcp` and workspace snapshots (T2) | When T2 passes | The intended client path |

Choose the tier at a fixed time on the day from what has passed its own test. Running this campaign on S1 would also exercise T1's own steps under real load, including stopping the coordinator's daemon while the other two machines keep exchanging notes, then restarting it and watching it catch up.

Two limits to state on the day. Three machines belonging to one person are not evidence for the real-collaboration gate, which requires two people with independent accounts. And on S0 nothing enforces who may sign a decision; the coordinator being the only writer of the decisions log is a convention.

## 6. Run plan

### Tonight: preparation

1. Decide the questions in section 9.
2. Build the sandbox and fetch upstream `main` and the pinned toolchain into it. **Gate: one machine reproduces 944,620 × 1,173 from source.** In July this step never happened. The existing Merak wrapper has only run against a fake fixture and may not work with the real script; a Linux container with networking disabled, calling the three build and evaluate commands directly, avoids the nested-sandbox problem.
3. Measure evaluation time and memory, and derive how many evaluations each machine runs at once.
4. Build the paired evaluator and the one-line harness wrapper.
5. Write the board wrapper, the coordinator loop and the worker prompt. Test the whole cycle with a scripted fake agent before any model is involved, as the July harness did.
6. Run the cartographer and the historian. Their outputs seed every later worker.
7. Start the pricing lanes and let them run overnight, three workers per machine at most, beginning with three in total.

### On the day

| Segment | What is shown |
|---|---|
| The problem, two minutes | The public leaderboard, the record, and the wiped memory directory |
| The verifier | Three machines produce the same `ops.bin` hash and the same score from the same commit |
| The overnight ledger | Priced settings, the archive grid, dead ends, verification coverage, spend per agent |
| One live cycle | A proposal is posted, assigned, worked, submitted, re-scored by the coordinator, accepted or rejected, and verified by an agent on another machine |
| A rejected claim | A result whose self-reported score the coordinator does not reproduce, or a proposal the curator matches to a recorded dead end |
| Resilience, on S1 only | Stop the coordinator's daemon; workers keep posting; restart; it catches up and clears its queue |
| The result | The composed candidate: its measured T, Q and λ, its would-be product, and its landing cost stated as a number of draws |

The view should be read by exception: task counts by state, one "doing now" line per agent, the archive, a short list of problems with known causes, and the spend rate. Not streaming transcripts.

### Fallbacks

- The baseline does not reproduce: stop and fix. Nothing else is meaningful.
- No priced improvement overnight: the demonstration is tier 1, shown with the ablation map and the ledger. Say so.
- Locust's binary is not ready: run on S0 and show Locust's real components separately and labelled, such as the contract tests and the transport probe.

## 7. What will be measured

| Measurement | Definition |
|---|---|
| Time to first verified priced improvement | Wall clock and tokens from start to the first accepted result that beats the base at equal or lower λ |
| Verification coverage | Share of accepted results reproduced by a different member; false claims caught |
| Duplicate work | Share of submitted candidates whose `ops.bin` hash matches an earlier one from another agent. Public upper bound for comparison: about a fifth of scored submissions share a score |
| Reuse of negative results | Proposals that cite a negative record, and proposals rejected as duplicates of one |
| Diversity | Archive cells holding a priced candidate; share of activity in the largest cluster |
| Evaluator efficiency | Evaluations per accepted result |
| Cost | Tokens and wall clock per agent, including the curator |

**The control.** The claim that sharing helps needs three arms at equal total budget: one agent with all of it, k agents without sharing, and k agents with sharing. One run of each is an anecdote and several are an experiment. This is the first follow-up after the demonstration, not part of it, unless the owner wants the claim made on the day.

## 8. Risks

| Risk | Response |
|---|---|
| The sandboxed baseline takes longer than expected to get working | It is the first task tonight and the gate for everything else |
| Agents drown in 876 KB of code, as in July | Cartographer first; lanes scoped to a module; fresh context per task; one-line harness output |
| Every lane converges on the same idea | Lanes assigned by seam; novelty check; reserved explore slots; concentration alarm |
| A "gain" that is really a λ increase | λ is measured by the coordinator and the comparison is at equal or lower λ |
| Edits that do not compose | Only the integrator composes, and it re-prices the composition |
| The upstream record moves during the run | A watcher polls the public endpoint sparingly from one place and posts base changes; the integrator rebases |
| An agent acts on instructions found in a note | Prompts treat all mined text as data; workers have no network and no key |
| The demonstration is read as a Locust release | Tier and substrate are stated on the first slide |

## 9. Decisions for the owner

1. **Is nonce search in scope?** The organisers accept it, and landing anything requires it. It measures compute, not insight, and at the record's λ it needs GPU tooling that only exists as third-party code. *Recommendation: no for the demonstration. Report priced results and their landing cost. Revisit with a lower-λ lineage.*
2. **Will anything be submitted publicly?** A submission is permanent, carries the owner's GitHub handle, and requires a public note of at least 5 KiB naming the exact model and harness, which would name Locust. *Recommendation: not during the demonstration.*
3. **Who runs third-party code, and in what sandbox?** The baseline requires executing code written by anonymous contributors. *Recommendation: a network-less Linux container on each machine, set up by the owner or with the owner's explicit approval.*
4. **Which clients and how many agents?** *Recommendation: Claude Code and Codex, the pair with configuration evidence today; begin with three workers and grow to nine across three machines. Opus-class models for workers, on the evidence of the public notes.* Codex's default sandbox may block the daemon's socket on S1; the file board on S0 lives inside the workspace.
5. **Is there a spending ceiling, and at what scope?**
6. **When is the substrate chosen?** *Recommendation: a fixed time on the morning of the demonstration.*
7. **Is the control arm part of the demonstration?** *Recommendation: afterwards.*

## 10. After the demonstration

- Run the matched arms and publish the result whichever way it falls.
- Give the structural lanes the days they need.
- Invite a second person with their own machine and account. A Locust goal is a direct replacement for the memory directory the public process keeps losing, and that run would be the first real-collaboration evidence.
- If nonce search is approved, run it as the search service: non-overlapping ranges handed out as tasks, peers contributing compute instead of tokens.
- Feed back what the campaign needed and version 0 lacks: a typed acceptance test on a task, a claim pool, more than one accepted head per goal, and content larger than one chunk.
