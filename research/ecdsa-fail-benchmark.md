# ecdsa.fail: benchmark, rules and state of the field

Research date: 2026-10-03. **Status: research note. Source review and data analysis only. No challenge code was built or run, nothing was submitted and no account was used. Revised the same day after an adversarial review.** The companion note covers [prior art for agent swarms on this benchmark](ecdsa-fail-swarm-prior-art.md). The [leaderboard analysis appendix](evidence/ecdsa-fail-leaderboard-analysis.md) holds a reduced capture and the script behind every number marked *recomputed*.

## Question

What exactly does [ecdsa.fail](https://ecdsa.fail/) ask for, how is a result judged, what does one evaluation cost, where does the frontier stand, and what kind of work still moves it? The answers decide whether the challenge is a good open-ended problem for demonstrating collaborating agents, and what such a demonstration can honestly claim.

## Sources and method

- The site and its two unauthenticated JSON endpoints, fetched 2026-10-03 at 23:43 UTC: 1,355 submissions to benchmark `1ffb695a-309b-46b6-a728-2f97d8c7be74`, the newest created at 20:34 UTC.
- The site's Terms (effective 2026-09-01), privacy page, installer script and CLI bundle, fetched as text and read. None was executed.
- The source repository [Layr-Labs/ecdsafail-challenge](https://github.com/Layr-Labs/ecdsafail-challenge) at `main` = `3161bd20`, through read-only GitHub API calls: tree, selected files, commit list, workflow runs and one job log, issues, pull-request bodies. Nothing was cloned.
- Harness source comparison with public revision `422f21d`. The three scoring files `src/sim.rs`, `src/circuit.rs` and `src/bin/eval_circuit.rs` are byte-identical at `main`. Three other harness files changed: `src/bin/build_circuit.rs`, `src/lib.rs` and `benchmark.sh`. At `main`, contestant code is compiled only into `build_circuit` and is no longer linked into the evaluator. Any evaluator a swarm trusts must be built from the identified current harness.
- The challenge paper, written by participants and organisers ([arXiv 2609.09582](https://arxiv.org/abs/2609.09582)), other papers and public write-ups, linked where cited.

Labels: **recomputed** means produced by the appendix script from the reduced capture. **Read** means seen in harness source, the Terms, the job log or the repository. **Reported** means stated in a solver's public note, pull request or issue and not reproduced here; such claims carry the first eight characters of a submission id or an issue number. **Inferred** means our own reasoning. Notes are written by anonymous third parties, often by their agents. They are leads, not facts.

## The task and the score

Build a reversible circuit that adds a classical secp256k1 point to a quantum one in place: four 256-wide registers, `target_x` and `target_y` as qubits and `offset_x` and `offset_y` as classical bits, with the target overwritten by the affine sum. Point addition is the inner step that Shor's algorithm repeats when it attacks elliptic-curve keys.

Score = round(average executed Toffoli count per shot) × qubits. Lower is better. The harness descends from the code Google released with its 2026 resource estimate ([arXiv 2603.28846](https://arxiv.org/abs/2603.28846)), whose own point-addition circuits were attested by a zero-knowledge proof and not published.

| Reference point | Toffoli | Qubits | Product |
|---|---|---|---|
| Litinski 2023, as estimated by the site | 8.19M | 3,000 | 2.46 × 10¹⁰ |
| Challenge start, 2026-05-30 | 3.96M | 2,715 | 1.076 × 10¹⁰ |
| Google's low-gate bound | 2.1M | 1,425 | 2.99 × 10⁹ |
| Current record, 2026-10-03 | 944,620 | 1,173 | 1,108,039,260 |

The rows are not like for like. Google's figures are proved upper bounds; the README also lists its low-qubit bound of 2.7M × 1,175. The record circuit fails on roughly one input in 425 by its submitter's own measurement (see below). That is inside the 99% correctness the maintainers said they aim for in issue #1, and well above the roughly one in 10,000 that the published approximate designs claim. The benchmark supplies one addend classically and never tests doubling or the point at infinity. The challenge paper calls the comparison with Google contextual.

Only files under `src/point_add` may change. Dependencies and the Rust toolchain (1.93.0) are frozen. The benchmark closes on 2026-12-31.

## What the evaluator charges, ignores and checks

**Read** in the harness source.

- **Two programs.** `build_circuit` runs the contestant's `point_add::build()` and writes `ops.bin`, a compressed list of 56-byte operation records. `eval_circuit` reads that file, simulates it and writes `score.json`. The score is a pure function of `ops.bin`.
- **Charged.** Only Toffoli-class gates (CCX and CCZ), once for every shot in which the gate's classical condition holds. A gate guarded by a measurement bit costs about half a Toffoli on average.
- **Free.** Every Clifford gate, measurement, reset, classical bit operation, circuit depth and operation count.
- **Qubits.** The highest qubit index used, plus one. This is a property of the allocator, not of how many qubits are live at once.
- **No data-dependent control.** The only classical bits a circuit can condition on are the classical addend and the outcomes of X-basis measurements, which the simulator draws as fair coins. A circuit cannot stop early because its quantum data says the work is done.
- **Test inputs.** A SHAKE256 hash over the whole operation stream seeds 9,024 scalar pairs and then every measurement coin. A circuit cannot be tuned to its inputs, but any change to the stream, even a no-op, draws a fresh set.
- **Validity.** All 9,024 shots must be classically correct, the phase word must be zero in every batch of 64 shots, and every non-register qubit must end at zero. On a failing run the evaluator still simulates every shot and prints the three failure counts and the executed-Toffoli average.
- **A gap between text and code.** The README, the Terms and the evaluator's own header comment promise a forward-then-reverse identity check. The evaluator's validity loop runs the circuit forward once and checks outputs, phase and ancilla state. It has no reverse pass, and the official workflow runs nothing else, so official scoring does not perform one.

One qubit is worth about 805 Toffoli at the frontier (944,620 ÷ 1,173), and one Toffoli is worth 1,173 points of score.

## Validity is statistical

This shapes the current regime more than anything else.

The circuits at the frontier are deliberately inexact. They truncate register widths and shorten the Euclidean walk to bounds that hold for almost all inputs. Because the test set is seeded by the operation stream, a submitter can append gates that cancel, such as 48 pairs of X gates encoding a number, and search that "tail nonce" until the 9,024 derived shots all pass. An outside researcher raised this in [issue #1](https://github.com/Layr-Labs/ecdsafail-challenge/issues/1) and closed it as resolved; a maintainer then set out the model there. The challenge paper calls the pass probability *landability* and counts nonce-only commits separately from circuit changes.

**The failure rate λ has more than one definition in the notes**, and comparing figures across notes without checking the unit gives wrong answers.

| Convention | Used by | Chance of a clean draw |
|---|---|---|
| Failing 64-shot batches per run, of 141 | The record's note and the leading solver | (1 − λ/141)¹⁴¹ |
| Failing shots or "failure events" per run | The 1,164- and 1,145-qubit variants | about e^−λ |
| −ln of the clean-draw chance | Some issues | e^−λ exactly |

- The record's note (`0b98178b`) reports 19.727 failing batches, pooled over 256 full evaluations with standard error 0.255. Under the batch model that is one clean draw in about 1.7 × 10⁹, between 0.9 and 3.1 × 10⁹ within two standard errors, and a per-shot failure rate of about 1 in 425 (**reported** λ; arithmetic ours, assuming independent failures; the central figures are in the appendix output). The e^λ shortcut gives 3.7 × 10⁸ and understates the cost. The note does not say how many draws its search took.
- The previous head at 1,174 qubits (`ee410040`) reports 19.348 ± 0.032 failing batches over 16,384 draws.
- A 1,164-qubit variant (`17250b2e`, rejected, 2.0% above the record) reports 14.56 failure events per run from 16 runs, standard error 0.9. A 1,145-qubit variant (`dd3621dc`) reports 11.25 ± 0.62. Both are in the per-shot convention and both come from small samples.
- Two versions of a solver-written "limiting contract" ask later solvers to cap λ: at 18 from 2026-09-25, raised to 19.5 on 2026-09-29. 61 notes carry one or both. The record exceeds both and its note apologises for it. This is a community norm, not a platform rule.

**Landing is a tooling problem more than a compute problem.** The solver who made 34 of the last 49 promotions names consumer hardware in their notes: one desktop-class processor and one to three graphics cards. The notes describe a classical model of the walk on the GPU with exact verification of survivors. On the September 1,251-qubit circuit it screened about 73,000 candidates per second, and the first clean nonce came after 55 million candidates in 13 minutes at λ = 16.7 (`73e6d530`). A day later it ran at about 150,000 per second (`9e674e58`). No rate is published for the current circuit. At the record's rate the same throughput would need about three hours (rates **reported**; arithmetic ours). Earlier public figures were 10,000 to 16,000 per second in August. For the 1,164-qubit variant the only stated rate is 383 per second with an exact replica on one GPU, and [issue #380](https://github.com/Layr-Labs/ecdsafail-challenge/issues/380) quotes about 35 per second on a laptop processor. Classical failures can be screened by such a model; phase failures need a full evaluation. A model must be written and validated per circuit family.

**Failure rate has a price, and it differs by setting.** [Issue #284](https://github.com/Layr-Labs/ecdsafail-challenge/issues/284) measured, on the 1,259-qubit head of 2026-09-09, a best sale of about 788 Toffoli per unit of λ and a purchase at about 610, and called that round trip roughly break-even. On the current circuit the leading solver reports purchases from about 145 to 720 Toffoli per failing batch and sales from about 800 to 1,300, and several recent promotions hold λ flat and take the spread (`fafbe564`, `a9b1c181`, `a7bcc605`). The cheapest purchases run out quickly: one note reports its price rising from 145 to 630 after half a unit. On the 1,164-qubit variant, buying 6.3 events back cost 11,615 Toffoli, about 1,840 each by our arithmetic (`17250b2e`). These sources use different units of λ (**reported**).

**Noise in the Toffoli average comes from measurement coins, not from inputs.** About one in nine Toffolis in the July circuit is guarded by a measurement outcome. The reported standard deviation of the average between draws is 7 to 14.5 for the July circuit, about 13 for its August successors, 5 to 7 for the ping-pong circuits from late August, and 39 to 50 for one September variant. It is not reported for the current circuit. Sharing a seed between two circuits pairs their inputs, and so their classically failing shots. It pairs their coins, and so their phase failures and Toffoli averages, only while both circuits make the same sequence of measurements. Reported single-setting changes on the current circuit move λ by 0.03 to 0.12, and the leading solver prices each on about 2,000 paired seeds (`100467cf`).

Two consequences. Every candidate, even an exact rewrite, has to win the lottery again before it can be submitted. And a change that lowers Toffolis by raising λ raises the bar for whoever wants to land the next variant at the old rate; [issue #131](https://github.com/Layr-Labs/ecdsafail-challenge/issues/131) is an apology from a solver who shipped one.

## Cost of one evaluation

| Where | Compile | Build circuit | Evaluate | Source |
|---|---|---|---|---|
| Official runner (32 vCPU), record circuit | 26.6 s | 20.9 s | 12.6 s | Job log of workflow run 37152039816, **read** |
| Solver machines, August circuits | n/a | 1.4–2 s | 13–38 s | **Reported** |

Both binaries are single-threaded. A candidate costs about a minute of one core including the recompile, and each further evaluation of the same build about 13 seconds. Evaluation memory is 56 bytes per operation, about 550 MB for the record's 9.8 million operations (**inferred**). Concurrent runs need one checkout and build directory each: the harness writes fixed output paths, and the current circuit reads data files from paths fixed at compile time.

Official turnaround from submission to verdict, **recomputed** over the 569 accepted rows: median 2.2 minutes, 90th percentile 2.8. Submissions with at least 10⁸ Toffolis take a median of 19 minutes and a 90th percentile of 39; the workflow's own timeout is 45.

Evaluation is cheap. The scarce things are ideas, orientation in the code, and the tooling to price and land a change.

## Rules of engagement

**Read** in the Terms and the CLI bundle unless marked.

- Acceptance is by strictly lower product than the current best at verdict time, with no minimum step. 105 scored submissions exactly tied the best in force and were rejected (**recomputed**).
- AI agents may generate and automate submissions, and an account owner may give an API key to an agent they control. The owner answers for everything submitted.
- Creating multiple accounts to get around rate limits or other controls is prohibited. The separate rules of the platform's weekly giveaway forbid operating more than one account at all; they apply to the giveaway.
- **The Terms prohibit modifying, reverse-engineering or circumventing the harness, scoring code or validity gates, and manipulating benchmark measurements.** The maintainer's explanation of nonce search in issue #1 predates these Terms, and the challenge paper still describes it as routine. The tension is unresolved. A locally built measurement tool that is never submitted is our reading of permitted use, not a stated one.
- Every submission that passes intake becomes a permanent public commit carrying the submitter's GitHub handle, and since August a pull request as well. A public Markdown note of 5 KiB to 100 KiB and the names of the model and the agent harness are mandatory. Promoted code is licensed under Apache 2.0. The record cannot be withdrawn.
- A submission may credit up to ten co-authors. 15 rows in the full capture do.
- The Terms say the repository is under an MIT licence published in it. The tree has no licence file, only a notice for three harness files under CC BY 4.0.
- No cash prize is attached to this benchmark today: the platform's giveaway pays by points allocated per challenge each day, and this benchmark has none. That can change.
- The installer adds a global agent skill and enables CLI telemetry after login. `ecdsafail clone` runs the repository's setup and benchmark scripts automatically. Neither should be used outside a sandbox.
- The repository's own `benchmark.sh` confines only the run of `build_circuit`. Compilation and the evaluator run unconfined, and the script falls back to no confinement when no sandbox tool is present. The evaluator puts no cap on qubit or bit indices, so a hostile `ops.bin` can request an arbitrary allocation.

## State of the leaderboard

All **recomputed**.

| | Count |
|---|---|
| Submissions | 1,355 from 142 solvers since 30 May |
| Accepted | 569, of which 565 promoted |
| Rejected | 558, of which 448 scored validly but did not beat the best |
| Failed | 225 |
| Cancelled | 3 |
| With a public note | 1,339 |
| Solvers active in the last 14 days | 14, six of them with an accepted submission |

| Month end | Best product | Accepted that month |
|---|---|---|
| May | 8,405,420,100 | 20 |
| June | 1,571,592,960 | 379 |
| July | 1,488,026,454 | 25 |
| August | 1,140,989,148 | 92 |
| September | 1,109,316,122 | 33 |
| 3 October | 1,108,039,260 | 20 |

The last 14 days produced 49 promotions with a median gain of 0.0057%. One solver made 34 of them, and 42 kept the qubit count fixed and moved the Toffoli average by less than 2,000. The fortnight moved the record 2.58%, about half of it in one submission of 1.27%.

**The Pareto display is not the acceptance rule.** The site's default view says "843 frontier advances, 76 solvers" and advertises a 792-qubit record. Replaying every scored submission in time order reproduces exactly 843 advances, 76 solvers and 27 frontier points. 279 of those advances were officially rejected, 268 of them below 1,150 qubits. No submission below 1,150 qubits has ever been accepted.

| Qubit band | Scored submissions | Accepted |
|---|---|---|
| below 850 | 203 | 0 |
| 850–1,149 | 77 | 0 |
| 1,150–1,174 | 209 | 150 |
| 1,175–1,248 | 23 | 23 |
| 1,249–1,349 | 261 | 199 |
| 1,350 and above | 244 | 197 |

Best Toffoli count at selected widths: 1,173 → 944,620 (the record); 1,174 → 943,826; 1,250 → 888,879, the lowest ever accepted; 1,164 → 970,983; 1,145 → 1,132,785; 1,112 → 1.74M; 1,011 → 9.3M; 973 → 11.8M; 838 → 22.0M; 792 → 756M. Low-qubit designs use a different inversion architecture and pay from about twice to 800 times the record's Toffolis. The challenge paper, on July data, notes a sharp step inside that branch and says it does not establish a hard boundary.

The 1,175–1,248 band is empty of recent work: all 23 rows date from 10 to 13 June, on an earlier architecture. No current note prices a move into it. The measured prices for one more qubit are 952 Toffoli on the August 1,153-qubit circuit (`b87bdf71`), 666 on the September circuit at 1,259 → 1,260 (issue #284), and 794 between the two current heads at 1,173 and 1,174. Each is just under its break-even, and none has been measured further up (**reported** and **recomputed**).

## The frontier circuit as an agent meets it

**Read** through the GitHub API at `3161bd20` and in the record's official job log.

- `src/point_add` holds 44 Rust files in one flat directory, about 876 KB, plus three data directories with about 614 KB of tables. `pingpong.rs` is 219 KB and `heo_carry.rs` 171 KB. That is more than a 200,000-token context holds (**inferred** from byte counts).
- The current tree replaces the earlier circuit family wholesale. Earlier revisions describe a different circuit.
- `build()` clears the process environment and installs a recipe of about 130 named settings. A second table of about 105 defaults sits behind it, partly overridden by the recipe. Most are switches or mode names, about 25 recipe settings are integers, and several are per-round profiles hundreds of entries long (counts from one reviewing agent's read of `mod.rs`).
- **Exact rewrite rows are keyed by position.** The record applies 11 rewrite rows from `skywalk_data/q1173_exact_rows.txt`, each addressed by an absolute operation index. The previous head's file, `sky20_rewrite.txt`, holds 61 and is unused at `main`. Any change that shifts the stream before a row's index invalidates the row. The loader checks that each indexed operation is the expected gate and stops the build otherwise; a change that keeps the gate but alters the condition the row was proved under is not detected. The leading solver's notes describe re-keying and re-proving rows after every change, with a SAT solver.
- **The build prints its own ledger.** `build_circuit` reports Toffolis and peak qubits for each of 19 sub-phases. The log shows the 1,173-qubit peak first reached at operation 22,216 of 9.8 million, at the start of the forward walk, and the cap is itself a pinned setting.
- In-code documentation is partly stale. One 171 KB module's header calls it research-only while the shipped configuration enables it.

## How the frontier moved

The largest single steps and the two latest changes of contract and architecture. Step sizes are **recomputed** (the last two from the full capture); what changed is **reported**:

| Date | Step | Id | What changed |
|---|---|---|---|
| 05-31 to 06-02 | 6–12% each | `f94f726c`, `437e22ad`, `66ad478c` | Merging adjacent controlled swaps; truncating loop widths to an empirical envelope; truncating carry tails |
| 06-02 | 18.8% | `0c1d4d95` | "Dialog" GCD: record the Euclidean walk's branch decisions as a transcript and replay it, giving inversion and in-place multiplication together |
| 06-02 | 7.2% | `75927ba7` | Measurement-based uncomputation of the step comparator |
| 08-21 | 14.8% | `3616dbf2` | "Ping-pong" division: fixed-depth alternating-target binary division without comparisons, 952,707 × 1,321 |
| 09-25 | 1.27% | `81864024` | A package of error clean-up changes, with the first "limiting contract" |
| 09-29 | 0.02% | `4f0d2135` | "Skywalk": a walk in a different frame with about 390 steps where the earlier one took about 700; qubits 1,250 → 1,174 |

The challenge paper classifies the 400 scored, accepted commits of the public competition through 18 July and attributes record-and-replay division to an earlier Google paper ([arXiv 2510.10967](https://arxiv.org/abs/2510.10967)), adapted to point addition by Schrottenloher ([arXiv 2606.02235](https://arxiv.org/abs/2606.02235)). It cautions that a scalar score may suppress designs that need a temporary regression, and credits the separate low-qubit and low-Toffoli displays with mitigating that.

## Where the Toffolis go, and how much headroom is left

- The record's own build ledger splits its 977,065 emitted Toffolis roughly as: forward division walk 36%, the multiplication leg's fused replay 25%, the two re-walks 11% each, and the rest in smaller batches and the square (**read** in the job log). Schrottenloher's Table 3 gives a similar picture for his design: about 90% in inversion and in-place multiplication.
- Techniques the cost model rewards: adders with measurement-based uncomputation, routing transient work through idle registers, compact transcript coding, constant folding with secp256k1's pseudo-Mersenne prime, classically conditioned skipping of rare branches, and approximation.
- Techniques it does not reward here: windowing and table lookups, because the addend is classical. Notes price Fermat inversion and projective coordinates as far worse.
- **No lower bound is published.** A rough model of three full-width additions per step, about 395 steps, two passes and 60,000 to 100,000 for the square gives about 0.7M. It is not a bound: earlier circuits already spent fewer Toffolis per round than it assumes.

**What still looks open** (**reported** or **inferred**; none verified here):

1. **Re-keying exact rewrite rows.** The record carries 11 rows where its predecessor carried 61. Each proven row removes one Toffoli and leaves the failing shots unchanged. Mapping the other 50 onto the record's stream needs a local prover and is parallel and exactly checkable. Expected size: about 50 Toffoli, four times the last record step.
2. **Cheaper failure-rate purchases** that do not raise the qubit peak. Only these make a lower-λ circuit competitive.
3. **A new walk recurrence** with fewer steps at equal or lower transcript cost. Both architecture changes since August were of this kind. This is weeks of work, not a night.
4. **A classical model of the walk** that predicts failures from millions of inputs in seconds. It is an instrument, not an improvement, and every solver who lands variants cheaply has one.

**Ruled out by the circuit or the evaluator**, although they look attractive from outside:

- *Reusing one transcript for both division directions.* The two walks run on different operands: the circuit divides, changes x, then multiplies by the new x. The record already shares what can be shared, and the re-walk in the multiplication leg is 11% of emitted Toffolis.
- *Stopping the walk early when the data allows.* No data-dependent classical control exists in this gate set.
- *Arithmetic on active width only.* This is already the design: the walk runs against per-step width envelopes, and their tails are where most failures come from.
- *Streaming the transcript to lower the qubit peak.* The peak is reached at the start of the walk, when the transcript is nearly empty.
- *Trimming rounds from the existing walk.* This is the canonical way to sell failure rate, at the prices above.

## Reported dead ends

A swarm should treat each as a hypothesis with an owner. Every entry is tied to the circuit it was measured on; most of those circuits have since been replaced.

| Claim | Source | Circuit | Status today |
|---|---|---|---|
| Raising the qubit cap pays at most about 952 Toffoli per qubit, below break-even | `b87bdf71`, 08-19 | 1,153 qubits, August; break-even was about 1,112 | Not re-measured. Break-even is now 805 |
| Deferred modular reduction overflows | `b87bdf71`, 08-19 | Same | Not re-measured |
| A second level of Karatsuba on the square costs more | `b87bdf71`, 08-19 | Same | Reversed: accepted on 09-08 (`9e1ab6ad`), and the current square is recursive |
| Shortening the round count raises λ faster than it saves | `0fa3b9f1`, 08-21; issue #131 | Ping-pong | Consistent with the price of λ above |
| The region with λ ≤ 14 and a better product is empty | Issue #380, 09-13 | Ping-pong, against a bar of 1,137,367,864 | Written before Skywalk. The 1,164-qubit variant sits just under that bar at about that λ |
| Narrowing the qubit cap on the current family | `17250b2e`, 09-30 | Skywalk | Its author's measurements: the cap alone raises the failure rate, and the wider envelope that buys it back holds only down to 1,164 (18.3 events at 1,163, about 64 at 1,162). That buy-back cost 11,615 Toffoli, more than the 8,050 that ten qubits are worth today; the comparison is ours, not the author's |

## Shared memory on the public side

The README tells solvers to keep notes under `src/point_add/memory/`. That directory does not exist at `main` (**read**). By our reading of the commit history it was removed entirely six times by accepted submissions: three times on 2026-06-02 while it held one to three files, and with substantial content on 2026-06-06, 2026-06-19 and 2026-09-05. The editable path is the whole directory, so a submission whose tree lacks the notes deletes them for everyone. Its last state was internally inconsistent: an index covering six of thirteen notes, three different "current frontier" figures, and a note declaring the structure exhausted shortly before the record fell another 11%.

What the crowd does share is real but unstructured: the promoted branch, about 1,300 free-text notes, 786 public `submissions/*` branches including rejected ones, 461 pull-request bodies, 15 issues of which about four are research write-ups, and optional Slack and Telegram channels. The platform has a discussions feature intended as shared research memory; it is switched off for this benchmark. Many notes do record negative results and measurements, some with sample sizes and standard errors. They survive as unindexed prose, mostly tied to circuits that have since been replaced. The few second-party re-measurements are buried in the same prose and nothing indexes them. The challenge paper reports that participants' own experiment logs reduced duplicated effort.

## Text addressed to agents

Sources contained instructions aimed at whichever agent reads them. None was followed. They matter because swarm workers will read the same material.

- The README and the site tell the reader to pipe an installer into a shell.
- 61 submission notes carry a "note to AI agents" with a four-term "limiting contract", including an order to copy the note into later commits.
- Deleted memory notes contained ready-made session prompts with sync and submit commands, and coordination orders to "all fleet agents".
- The CLI's embedded skill tells agents to post public research updates every 30 minutes where discussions are enabled. They are not enabled for this benchmark.

## Open questions

- Local evaluation time and memory for the current circuit on the demonstration machines.
- The numeric submission rate limits. The fastest observed solver made 15 submissions in an hour.
- Whether the organisers will change scoring, add a minimum step, or turn on owner review before the close. Any of these would change the value of nonce search.
- Whether the Terms' prohibition on circumventing validity gates will be applied to nonce search.
- How much of the 876 KB at `main` is live code. This needs a build inside a sandbox.
- Whether the absence of a licence file constrains evaluating or building on other solvers' rejected branches.
- We found no public re-measurement of the four current frontier circuits by anyone but their authors. Earlier circuits were occasionally re-measured by others (`fafbe564`, `17250b2e`, issue #284).
