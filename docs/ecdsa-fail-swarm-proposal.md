# ecdsa.fail swarm demonstration: proposal

Date: 2026-10-03. **Status: proposal for the owner's review. Nothing in it has been built or run. Statements about Locust describe the code at `1738150` and the lane documents at `8b5dd7b` (2026-10-03 19:55 PDT), which start the physical test on two Macs, and say what is tested and what is not. Revised the same day after an adversarial review.** Evidence is in two research notes: the [benchmark, its rules and the state of the field](../research/ecdsa-fail-benchmark.md), and [prior art for agent swarms on it](../research/ecdsa-fail-swarm-prior-art.md), including the owner's July attempt in Merak.

"Swarm" here means the agents working one Locust goal. The protocol's unit remains the goal.

## 1. Recommendation

Use [ecdsa.fail](https://ecdsa.fail/) for the demonstration, with a different target from the obvious one.

**Do not promise a leaderboard record.** The record (944,620 Toffoli × 1,173 qubits) is a circuit that, by its submitter's own measurement, fails on about one input in 425. That is inside the 99% correctness the organisers said they aim for. It passes the official check because its no-op padding, the *nonce*, was searched until the 9,024 test inputs derived from it all happened to pass. By our arithmetic from the reported failure rate, about one variant in 1.7 billion passes, and every changed circuit must win that lottery again. The solver who made 34 of the last 49 improvements does this on consumer hardware with tooling built for this circuit family: paired measurement of every change on shared seeds, machine-proved rewrites, and a GPU model that found a passing variant in 13 minutes on a September circuit with a lower failure rate. At the record's rate the same reported throughput implies a few hours per landing. Their typical step is tens of Toffolis, a few thousandths of a percent. A day-old swarm will not out-tool that.

**Demonstrate what the public process lacks.** 142 solvers have submitted since May, 14 in the last fortnight. What they share is unstructured: the winning branch, about 1,300 free-text notes, rejected branches and chat. Their one shared memory directory was deleted repeatedly, by our reading six times, by submissions built from trees that lacked it. Negative results and measurements exist only as unindexed prose, mostly about circuits since replaced. The failure rates that decide what is landable are self-reported in three different units. Other solvers have occasionally re-measured a circuit before building on it, but nothing indexes those checks, and we found no outside measurement of the four circuits now at the frontier. Yet the benchmark has the property that makes collaboration between strangers cheap: the score is a deterministic function of an artifact, so a claim can be checked without trusting the claimant.

The proposed demonstration:

> Several agents work one goal. Every claim is a typed, attributed record. Every result is re-measured by a coordinator that no agent controls, and again by an agent that did not produce it. Dead ends are recorded with the circuit they were measured on. The output is a ledger: the public frontier's self-reported failure rates independently confirmed or refuted, and each change the swarm tried with its measured effect on Toffolis, qubits and failure rate.

**One limit, stated first on the day.** With one person's machines this is a rehearsal of the protocol, not collaboration between independent parties. Locust's own real-collaboration gate needs two people with their own machines and accounts. A second person running the verification role would be the first evidence toward it, but that means handing them the unpublished candidate binary and making this the first cross-machine Locust run, so it comes after the lanes' own two-Mac test, not before. Tomorrow's substitute is a second agent client on the same machine verifying the first one's results, which shows vendor diversity and not operator independence.

| Level | Meaning | Status |
|---|---|---|
| 0. Verifier | The record's score is reproduced from source inside a sandbox | Tonight's gate |
| 1. Process | The full cycle on real candidates: propose, assign, take, submit, re-measure, accept or reject, verify. One claim the coordinator does not reproduce. One dead end recorded and later cited | The commitment |
| 2. Verified ledger | The public frontier circuits re-measured with sample size and standard error, each compared with its author's claim; plus the swarm's own changes priced on shared seeds | The aim |
| 3. Priced improvement | A change whose paired measurement beats the record circuit beyond noise without raising its failure rate. Expected size: tens of Toffolis | Possible, not expected |
| 4. Landed | That change passes the official check with a searched nonce | Owner's decision; needs a screen model |
| 5. Submitted | It is on the public leaderboard | Owner's decision; irreversible and public |

### Needed from the owner tonight

1. **Approve running the challenge code in a sandbox on one machine.** Everything else waits on this. It includes installing a container runtime there, fetching the Rust 1.93.0 toolchain and crates before the network is cut, and fetching two other solvers' rejected branches for the audit. (Decision 1 in section 9.)
2. Accept or change the defaults in section 9: no nonce search, no public submission, a local seeded evaluator treated as a permitted measurement tool, Claude Code workers, no dollar ceiling on subscription seats.

The campaign uses one of the owner's two Macs, the one that is not put to sleep in the lanes' test, and none of the lanes' time until that test is recorded.

## 2. The problem as the swarm sees it

Details and sources are in the [benchmark note](../research/ecdsa-fail-benchmark.md).

- **Objective.** Minimise the rounded average executed Toffoli count × the number of qubit indices used. Only `src/point_add` may change. Public acceptance is strict improvement of that scalar; the site's Pareto view is a display.
- **Verifier.** `build_circuit` turns source into `ops.bin`; `eval_circuit` scores `ops.bin`. A candidate costs about a minute of one core, and each further evaluation of the same build about 13 seconds.
- **Three axes.** Toffolis (T), qubits (Q) and the failure rate λ. One qubit is worth 805 Toffolis. Solvers report λ in three different units: failing 64-shot batches per run, failing shots per run, or minus the logarithm of the pass chance. Every λ in the swarm's records states its unit, sample size and standard error. A change that cuts T by raising λ is a loan. Reported prices on the current circuit run from about 150 to 1,300 Toffolis per unit of λ depending on the setting, and the leading solver's steps are packages that buy where it is cheap and sell where it is dear.
- **Noise.** A *draw* is one 9,024-input test set. The Toffoli average varies between draws because some Toffolis are guarded by measurement coins. Failures depend on the test inputs. Two circuits measured on the same seed see the same inputs, so their classically failing shots can be compared one by one. Phase failures and Toffoli averages pair only when both circuits make the same sequence of measurements.
- **Orientation cost.** The record circuit is 876 KB of dense Rust with stale comments: more than one context window. In July the owner's agents never got past reading. A budget rule stopped them while they were still inspecting files, and with the cap removed one agent read for ten rounds before its first hypothesis.
- **Coupling.** Settings interact, and the circuit's exact rewrite rows are addressed by operation index, so a change early in the stream invalidates rows after it. The build checks each row's gate and stops when the index has moved. A change that keeps the gate but alters the condition the row was proved under is not detected. Two agents' edits do not compose by file.

## 3. Why several agents should help here, and where they will not

Published evidence points one way, with a caveat. Groups of agents win on work that is parallel, context-isolated and checkable, at several times the token cost, and lose on coupled work; two of the studies put coding tasks on the losing side. One preprint, CORAL, reports agents with shared memory beating the best of the same number of independent agents at equal wall-clock time on three scored optimisation tasks. Nothing has been measured on this benchmark. See the [prior art note](../research/ecdsa-fail-swarm-prior-art.md).

The favourable conditions hold for the work proposed here. Re-measuring one public circuit is independent of re-measuring another. Re-keying one rewrite row is independent of the next. Every claim can be re-measured from its artifact. Verification and negative results, which a single agent skips, are separable tasks.

The unfavourable one holds for composition. Combining changes is coupled work and needs tooling nobody has built for this circuit, so the demonstration does not attempt it.

The demonstration claims only what it measures (section 7). "A swarm beats one agent" is not among the claims.

## 4. Swarm design

### 4.1 Roles

| Role | What it is | Authority |
|---|---|---|
| Coordinator | A script, not a model. Acts as the goal's coordinator principal and runs the pinned evaluator | The only source of assignments, acceptances and rejections. Ignores self-reported numbers |
| Workers | One agent per task, each in its own checkout, fresh context per task | Propose, take, submit, add notes |
| Verifiers | Workers taking verification tasks on results they did not author; ideally a different client or a different person | Add verification records |
| Launcher | A script on each machine. Starts a fresh agent session per taken assignment and makes the board calls and builds on its behalf | None of its own; runs under the local owner's approval |
| Owner | The person | Approves work on each machine, sets the task list and any budget, and is the only one who can submit publicly |

There is no pipeline of software roles and workers do not converse. They read the board. A planning agent, a composing integrator and a search service are deferred to section 10.

### 4.2 Records

Every record names its base, meaning the upstream commit and the `ops.bin` hash, and its author. The public dead-end claims that went stale are the ones that named no base.

| Record | Required content |
|---|---|
| Proposal | Mechanism, the part of the circuit it touches, expected ΔT, ΔQ and Δλ, the result that would disprove it, the ledger entries it builds on or contradicts |
| Result | Base, diff, claimed `ops.bin` hash, Q, emitted T (Toffolis in the stream), executed T (the scored average that fire per shot), failure counts with their seeds, and the number of measurement operations |
| Decision | The coordinator's own measurements, whether the hash and edit boundary held, accept or reject with reason |
| Verification | A non-author's reproduction from the artifact hash, or a refutation |
| Negative result | What was tried, on which base, what was measured, and the condition under which to reopen it |

Every number carries its evidence class: exact count, paired measurement on N seeds, or inference. A λ without a unit, N and a standard error is not a measurement.

### 4.3 Task lifecycle

1. **Propose.** A worker's first action on a task is a short proposal record, before deep reading. This is the direct fix for July, when publishing came after reading: the capped runs published nothing, and the uncapped one published its first hypothesis in its eleventh round, to a peer that had already exited.
2. **Assign.** The coordinator assigns the task to a named member, normally its proposer. Locust has no open claim pool.
3. **Take.** In Locust an assignment is an offer. Work starts only when the local owner has authorized it: one assignment at a time, or once per worker and goal with a standing grant. The per-assignment path is the tested one. For this run the owner sets the standing grant for the campaign's workers if it passes tonight's scripted cycle, and otherwise authorizes each assignment. A second person's approval is theirs to give.
4. **Work.** The worker edits only `src/point_add` in its own checkout and uses the cheap evaluation stages itself.
5. **Submit.** The only way to get a trusted measurement is to submit a result, so an agent cannot finish with nothing on the board.
6. **Decide.** The coordinator re-measures in its own sandbox and records an acceptance or a rejection with its numbers.
7. **Verify.** Each accepted result produces a verification task for a different member.
8. **Stops are not successes.** A worker that hits a named budget pauses and asks the owner. A worker that crashes or times out is shown as interrupted with its claim held, and the owner chooses resume or reassign. Nothing downstream runs on an empty input, and no model summarises work that did not happen.

### 4.4 Measurement

| Stage | What it measures | Cost | Run by |
|---|---|---|---|
| 0. Boundary and build | Only `src/point_add` changed; builds with a scrubbed environment; `ops.bin` hash | under a minute | worker, coordinator |
| 1. Count | Q and emitted Toffolis, exactly; the per-phase ledger the build already prints | seconds | worker, coordinator |
| 2. Paired screen | Failing shots of base and candidate on the same seeds, compared shot by shot; executed T paired only when both make the same measurements | about 48 evaluations, ten minutes of one core, for exact changes | worker, coordinator |
| 3. Priced change or absolute rate | A setting's Δλ on about 2,000 paired seeds, or one circuit's λ with a standard error of 0.1 | about seven core-hours each | coordinator, selected candidates only |
| 4. Landing | A nonce that passes the official derivation | see section 9 | not in the demonstration |

"Failure rate not raised" is accepted only when the paired comparison's upper bound on Δλ is at or below zero. Comparing two separate estimates cannot show it: at the record's rate, 128 draws per circuit resolve a difference of about 1, which is a threefold change in landing cost, while reported single changes on the current circuit move λ by 0.03 to 0.12. A 48-seed screen therefore decides only changes whose failing shots are identical, such as rewrite rows, or moves of about 0.5 and more. Pricing one setting takes about 2,000 paired seeds, which is what the leading solver's notes report using.

Stages 2 and 3 use a **seeded evaluator**. The official evaluator already simulates every shot of a failing run and prints its failure counts and Toffoli average. What it lacks is a way to choose the seed. The seeded evaluator is a separate small program that links the unmodified simulator modules from upstream `main` and takes the seed as an argument. With no argument it must reproduce the official number. It is never submitted. The Terms forbid modifying the scoring code; treating a separate local measurement tool as permitted is our reading, and it is on the owner's list in section 9.

### 4.5 Work for the demonstration

**Setup tasks**

| Task | Output |
|---|---|
| Baseline | The record's score reproduced from upstream `main` in the sandbox. Nothing else starts until this passes |
| Map | Parsed from the build's own output, not from reading 876 KB: the 19 sub-phases with their Toffolis and peak qubits, the pinned settings and their types, and the position of every rewrite row. A liveness pass over `ops.bin` is added if the reader from step 7 exists |
| Ledger | Tried changes and their reported prices, mined from the public notes, pull requests and issues. Every entry carries its source id, date and base circuit, and is marked unverified |

**Lanes**

| Lane | Work | Why it suits a swarm |
|---|---|---|
| Frontier audit | Rebuild each public frontier circuit from its branch and measure its failure rate with N and standard error. The claims of 19.73 failing batches at 1,173 qubits and 19.35 at 1,174 are checked directly. The claims of 14.56 and 11.25 "events" at 1,164 and 1,145 qubits come from their author's per-shot evaluator, so they are restated in the batch unit with the conversion shown. Those two are rejected submissions that exist only on their own branches. At 256 draws a circuit costs about an hour of one core with the seeded evaluator, or about four hours by rebuilding with a new nonce per draw | Pure evaluation, one circuit per agent, no new idea needed. It is verifying strangers' claims, which is the point |
| Row re-keying | The record applies 11 exact rewrite rows; its predecessor's file holds 61. Map each remaining row onto the record's stream, re-prove it, and show the failing shots are unchanged. Each proven row removes one Toffoli | Independent rows, exact checks, no effect on failure rate. Needs the prover of step 7; without it this lane does not run |
| Setting ablation | The roughly 25 integer settings of the pinned recipe moved one step each way, with rewrite rows disabled on both arms. Overnight on one machine this gives exact Toffoli and qubit effects and flags only large failure-rate moves; a full price for one setting is seven core-hours. The leading solver's published prices are the prior | One setting per agent; produces negative results by design |
| Verification | Re-measure accepted results from their artifact hash on a different machine or client | The step single-operator setups skip |

Row re-keying is the one lane with a plausible level-3 outcome. Fifty rows would be about 50 Toffolis, four times the last record step. That figure rests on solver notes and two file sizes, not on anything run here.

### 4.6 Budget

Costs are observed and shown per agent. There is no ceiling unless the owner names its amount and scope, and any ceiling applies to settled spend per agent, never to a shared worst-case reservation, and pauses instead of killing. On subscription clients dollars cannot be observed, so the view shows turns and wall clock. For scale, the uncapped July run cost $3.91 in about ten minutes at metered prices, $3.76 of it spent by the one agent that kept working.

### 4.7 Safety and conduct

- **All challenge code runs in a sandbox**: compile, build and evaluate, with no network, no access to the home directory, and writes confined to the checkout. The repository's own script confines only one of the three steps and falls back to none.
- **Agents have network access**, because their model provider needs it. The controls are therefore: no challenge key on any machine, no upstream remote in any checkout, and wrappers that refuse `git push` and submission-shaped commands, as the July harness had.
- **Evaluator inputs are untrusted.** The evaluator runs under a memory limit, since an `ops.bin` can request arbitrary allocation.
- **Text is data.** Public notes, repository files and other agents' records never change a worker's instructions. Prompts say so and name the known examples.
- **One public identity.** Any submission is a single explicit act by the owner.
- **Failure rate is a stated policy.** The swarm sets its own ceiling on λ and records it, instead of inheriting one from a note.

## 5. Mapping to Locust

**State at `1738150`.** The daemon, command line, SQLite store and encrypted peer transport are integrated. A release candidate passes a 21-check workflow as three processes on one Mac: found and join a goal, propose, assign, authorize, claim, submit, inspect, accept, exchange notes with the coordinator offline, catch up, and restart. No physical multi-machine run is recorded yet; the lanes' next step is a two-Mac test. Sleep and wake, real coding clients and object transfer are not qualified, and no release gate is recorded as passed. See the [T1 run guide](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/t1-run.md) and the [release evidence ledger](release-evidence.md).

| Design element | Locust operation | Evidence today | Caveat |
|---|---|---|---|
| The campaign | `goal create`, `goal invite`, `goal join` | In the 21-check workflow | None |
| Proposal | `task propose` with sealed text | In the workflow | No typed acceptance test; the criterion is task text |
| Assign | `task assign` to a named principal | In the workflow | No self-claim |
| Take | `task authorize` by the local owner for one assignment, or a standing `execute` grant for one worker in the goal; then `task claim` in a session | `task authorize` and `task claim` are in the workflow. The standing grant is implemented but has no named command and is outside the workflow check | Per-assignment authorization is the tested path |
| Result | `task submit` with sealed text | In the workflow | The tested path is text up to 1 MiB: the diff, the claimed hash and the counts. `ops.bin` is 33.7 MB. Object transfer up to 64 MiB has component tests but no named command and no qualification, so the campaign does not send it through Locust |
| Accept | `result accept`, plus a note about the result carrying the coordinator's measurements | `result accept` is in the workflow | An acceptance carries no text; a rejection carries its reason |
| Reject | `result reject` | Implemented; outside the workflow check | Needs its own test tonight |
| Interrupted worker | Resume: the same session claims again and gets its held claim back. Give up: `task fail` from that session, or a new `task assign` by the coordinator | Implemented; outside the workflow check | Only the claim holder can report failure, a different session needs a separately authorized takeover, and nothing expires by itself |
| Negative result, verification | `note add`, `notes` | In the workflow | Free text; the campaign's record format is a convention on top |
| Coordinator offline, catch-up | Daemon stop and restart | In the workflow, one machine | It passed with the default public relays; a relay-free run on the same Mac failed at this step. It needs internet from the demonstration network. Physical machines pending |

**Where the artifact goes.** On one machine the workers, coordinator and verifiers share a directory of `ops.bin` files named by hash, so a result is checked by re-scoring the artifact, which runs none of its author's code. Across machines the artifact needs a side channel until Locust carries objects, or the coordinator rebuilds from the diff inside the sandbox and compares hashes.

**Substrate.** The board calls go through one small wrapper.

- **Primary: the Locust candidate binary on one machine**, with one daemon home per participant as its own workflow check does. This is the evidence level that exists.
- **Fallback: a file board in Git.** One branch per writer that only that writer pushes, one file per record, a coordinator branch as the only authority, and agents never run Git themselves. It uses Locust's operation names and is said plainly not to be Locust.

The choice is made tonight, when the scripted cycle of section 6 either passes on the candidate or does not. It is not changed on the day. Moving the campaign to a second machine follows the lanes' own two-Mac test and does not precede it.

**Clients.** In Codex's default sandbox a shell command cannot open the daemon's socket and has no network, as the [independent review](../research/implementation-plan-independent-review.md) measured. Workers are therefore Claude Code sessions, or the launcher makes the board calls. Codex reaches the daemon through an MCP server, which is a later milestone.

**Ownership.** The campaign's scripts live outside this repository's lane-owned paths. Anything it needs from Locust is a request in the lane logs. Nothing here gates the lanes' two-Mac test.

## 6. Run plan

### Tonight, in order, each step time-boxed

| Step | Box | Gate |
|---|---|---|
| 1. Owner approves the sandbox on one machine | minutes | |
| 2. Baseline. A shallow fetch of upstream `main` at `3161bd20` and of the four branches named in the audit lane, pinned by commit, into a network-less Linux container with Rust 1.93.0 and the crates fetched beforehand. No remote remains in any checkout afterwards | 90 min | Average 944,619.796 at 1,173 qubits and an `ops.bin` of 33,738,224 bytes, as in the official log |
| 3. Ledger and map start in parallel. The ledger needs no sandbox | alongside | |
| 4. Board wrapper, coordinator loop, launcher and a one-table status view, tested with a scripted fake agent: an accepted result, a rejected mismatch, an interrupted worker | 3 h | All three cases pass on the Locust candidate; otherwise switch to the file board and repeat |
| 5. One real worker completes one cycle with no permission prompts | 1 h | |
| 6. Seeded evaluator | 45 min | Reproduces the official number with no seed. If skipped, the audit varies the nonce instead and other results are labelled "counted on own draw" |
| 7. Row prover: a reader for `ops.bin` and an exact check that a Toffoli at a given index acts as a simpler gate under the preceding stream's known-zero qubits | 2 h | Reproduces one of the record's 11 rows. If it misses its box, the row lane is dropped, the ledger has no re-keyed rows and level 3 is off the table for the night |
| 8. Go or no-go for an overnight run: three workers on one machine, frontier audit first | | |

If the container does not come up, the fallback for step 2 is the path the owner already used in July: the native script with the network off, which confines only `build_circuit`. That is labelled on the day.

Layout that decides concurrency: one checkout per candidate at a stable path, because the circuit reads data files from paths fixed at compile time; one build directory per worker; the evaluator built once.

### On the day

| Segment | What is shown |
|---|---|
| Start | One live cycle is started before anything is said. It needs several minutes of compile and evaluation |
| The problem | The leaderboard, the record, and the deleted memory directory |
| The verifier | The record's score reproduced from source in the sandbox |
| The overnight ledger | The frontier audit against the authors' claims, measured settings, re-keyed rows if the prover was built, dead ends, verification coverage |
| A rejected claim | A result whose reported numbers the coordinator does not reproduce |
| The live cycle lands | Proposal, assignment, the owner's authorization, result, the coordinator's decision, and verification by another agent |
| Resilience | The coordinator's daemon is stopped, notes continue, it restarts and catches up. Shown from the night's record if the venue's network blocks the relays |

A known-good candidate from the night is kept ready to replay if the live edit fails. The view is one table: tasks by state, one line per agent, the last decisions.

### Fallbacks

- The baseline does not reproduce in the container: use the native path. If that fails too, stop; nothing else is meaningful.
- The Locust candidate fails the scripted cycle: file board, and Locust's own workflow check is shown separately and labelled.
- Nothing priced overnight: the demonstration is level 1 with the audit and the ledger. Say so.

## 7. What will be measured

| Measurement | Definition |
|---|---|
| Audit coverage | Public frontier claims re-measured, and how many were confirmed within their stated error |
| Verification coverage | Share of accepted results reproduced by a different member; false claims caught |
| Time to first verified result | Wall clock and turns from start to the first result accepted and verified |
| Duplicate work | Share of submitted candidates whose `ops.bin` hash matches an earlier one from another agent |
| Reuse of negative results | Proposals that cite a negative record, and proposals rejected as repeats of one |
| Evaluator efficiency | Evaluations per accepted result |
| Effort | Turns and wall clock per agent; tokens where the client reports them |

**The control.** A claim that sharing helps needs three arms at equal budget: one agent with all of it, k agents without sharing, and k agents with sharing. One run of each is an anecdote. This is the first follow-up, not part of the demonstration.

## 8. Risks

| Risk | Response |
|---|---|
| The sandboxed baseline takes longer than its box | Native fallback; it is the first step and the gate for everything else |
| Tonight's preparation competes with the lanes' two-Mac test | One machine, one owner decision, fixed boxes, and a go or no-go before the overnight run |
| Agents drown in the code, as in July | The map comes from the build's output; tasks are scoped to a module; fresh context per task; one-line harness output |
| A "gain" that is really a failure-rate increase or noise | Paired measurement at shared seeds by the coordinator, compared against its own measurement of the base, never against the public number |
| A change invalidates rewrite rows, loudly or not | Rows are disabled on both arms unless the task is row re-keying |
| The coordinator's machine sleeps or fails on the day | On power with sleep disabled, on the Mac that the lanes' test does not put to sleep |
| The demonstration network blocks the public relays, so restarted daemons do not reconnect | Run Locust's own workflow check on that network beforehand; if it fails, restart no daemon live |
| The live edit does not produce a valid result | A replay candidate from the night |
| An agent acts on instructions found in a note | Prompts treat mined text as data; no challenge key anywhere; push and submission wrappers |
| The demonstration is read as a Locust release, or as collaboration between independent parties | Level, substrate and single ownership are stated at the start |

The base is pinned to `3161bd20` for the demonstration. Upstream movement is ignored until afterwards.

## 9. Decisions for the owner

1. **Who runs third-party code, and in what sandbox?** The baseline requires executing code written by anonymous contributors. The audit also builds and measures two other solvers' rejected branches. The Terms cite a licence for the repository that is not in it and publish none for unpromoted submissions; our reading is that local build and measurement without redistribution is use the public repository invites, and the ledger would report only measurements. *Recommendation: a network-less Linux container on one machine, set up by the owner or with explicit approval; include the two rejected branches unless the owner reads the Terms differently.* **Needed tonight.**
2. **Is nonce search in scope?** The organisers have treated it as acceptable and the Terms now prohibit circumventing validity gates; the tension is unresolved. Landing needs a classical model of the walk for the specific circuit, which is section 10 work: a GPU model at the record's failure rate, or a CPU model for the 1,164-qubit variant, which is 2% behind the record. Without one, full evaluations at 13 seconds each would take hundreds of core-days. Landing is out of scope for the night either way. *Default: no. Report measured changes and their landing cost.*
3. **Will anything be submitted publicly?** A submission is permanent, carries the owner's GitHub handle, and requires a public note naming the model and the agent harness. *Default: no.*
4. **Is the seeded evaluator acceptable under the Terms?** It links unmodified simulator code and is never submitted. *Default: yes, as a local measurement tool.*
5. **Which clients and how many agents?** *Default: Claude Code, three workers on one machine, more only after the cycle is stable. Public notes do not settle model choice.*
6. **Is there a spending ceiling?** *Default: none on subscription seats; for any metered agent, a per-agent ceiling on settled spend that pauses and asks.*
7. **Is a second person invited as a verifier?** It is the one change that turns a rehearsal into collaboration. It needs the candidate binary handed over privately and a sandbox on their machine. *Default: after the lanes' two-Mac test, not before.*

## 10. After the demonstration

- **Composition.** A script that applies accepted changes in order and re-measures. It needs tooling to re-key rewrite rows and regenerate tables, which must be built first, and its queue should be watched: one published team removed its integrator as a bottleneck.
- **A planning agent, a regional archive and diversity rules**, once there are enough workers for herding to be a risk: best candidate per qubit band and technique family, reserved slots for proposals that match nothing in the ledger, and effort allocated by improvement over a lane's own parent. The last is this proposal's adaptation, not a published method.
- **A classical model of the walk**, which is both the instrument for failure rate and the precondition for any landing. Then, if approved, nonce search as a queued service with non-overlapping ranges handed out as tasks.
- **Longer research**: a new walk recurrence with fewer steps, and cheaper failure-rate purchases that do not raise the qubit peak. Several attractive-looking ideas are ruled out by the circuit; the benchmark note lists them.
- **The matched arms**, published whichever way they fall.
- **A second person with their own machine and account.** A Locust goal is a direct replacement for the memory directory the public process keeps losing.
- **Feedback to Locust** from what the campaign needed. Missing from version 0: a typed acceptance test on a task, an open claim pool, and more than one accepted head per goal. Implemented but without a named command or a place in the workflow check: object transfer for artifacts and the standing grant for a campaign's assignments.
