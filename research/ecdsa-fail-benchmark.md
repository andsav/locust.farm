# ecdsa.fail: benchmark, rules and state of the field

Research date: 2026-10-03. **Status: research note. Source review and data analysis only. No challenge code was built or run, nothing was submitted and no account was used.** It supports the [swarm demonstration proposal](../docs/ecdsa-fail-swarm-proposal.md). The companion note covers [prior art for agent swarms on this benchmark](ecdsa-fail-swarm-prior-art.md), and the [leaderboard analysis appendix](evidence/ecdsa-fail-leaderboard-analysis.md) holds the script behind the numbers marked *recomputed*.

## Question

What exactly does [ecdsa.fail](https://ecdsa.fail/) ask for, how is a result judged, what does one evaluation cost, where does the frontier stand, and what kind of work still moves it? The answers decide whether the challenge is a good open-ended problem for demonstrating collaborating agents, and what such a demonstration can honestly claim.

## Sources and method

- The site and its two unauthenticated JSON endpoints, `/api/benchmarks` and `/api/benchmarks/<id>/submissions`, captured 2026-10-03 at about 20:40 UTC: 1,355 submissions to benchmark `1ffb695a-309b-46b6-a728-2f97d8c7be74`.
- The site's Terms (effective 2026-09-01), privacy page, installer script and CLI bundle, fetched as text and read; none was executed.
- The source repository [Layr-Labs/ecdsafail-challenge](https://github.com/Layr-Labs/ecdsafail-challenge) at `main` = `3161bd20` (2026-10-03 20:34 UTC), through read-only GitHub API calls: tree, selected files, commit list, workflow runs, issues, pull-request bodies. Nothing was cloned.
- A local checkout of the same repository from July (`422f21d`), read for the harness source. The scoring files `src/sim.rs`, `src/circuit.rs` and `src/bin/eval_circuit.rs` were compared with `main` through the API and are byte-identical.
- Papers and public write-ups, linked where cited.

Labels used below: **recomputed** means derived from the captured API data by the appendix script; **read** means seen in harness source, the Terms, or the repository through the API; **reported** means stated in a solver's public note, pull request or issue and not reproduced here; **inferred** means our own reasoning. Solver notes are written by anonymous third parties, frequently by their agents. They are leads, not facts.

## The task and the score

Build a reversible circuit that adds a classical secp256k1 point to a quantum one in place: four 256-wide registers, `target_x` and `target_y` as qubits and `offset_x` and `offset_y` as classical bits, with the target overwritten by the affine sum. Point addition is the inner step that Shor's algorithm repeats when it attacks elliptic-curve keys, which is why its cost matters.

Score = round(average executed Toffoli count per shot) × qubits. Lower is better. The harness came from the code Google released with its 2026 resource estimate ([arXiv 2603.28846](https://arxiv.org/abs/2603.28846)), whose own point-addition circuits were attested by a zero-knowledge proof and not published. The site quotes them as 2.1M Toffoli × 1,425 qubits and 2.7M × 1,175.

| Reference point | Toffoli | Qubits | Product |
|---|---|---|---|
| Litinski 2023, as estimated by the site | 8.19M | 3,000 | 2.46 × 10¹⁰ |
| Challenge start, 2026-05-30 | 3.96M | 2,715 | 1.076 × 10¹⁰ |
| Google, low-gate point | 2.1M | 1,425 | 2.99 × 10⁹ |
| Current record, 2026-10-03 | 944,620 | 1,173 | 1,108,039,260 |

Only files under `src/point_add` may change. Dependencies and the Rust toolchain (1.93.0) are frozen. The benchmark closes on 2026-12-31.

## What the evaluator charges, ignores and checks

Read from the harness source in the July checkout; file references are to that tree.

- **Two programs.** `build_circuit` runs the contestant's `point_add::build()` and writes `ops.bin`, a zstd frame of 56-byte operation records. `eval_circuit` reads that file, simulates it and writes `score.json`. The score is therefore a pure function of `ops.bin`.
- **Charged.** Only Toffoli-class gates (CCX and CCZ), once for every shot in which the gate's classical condition holds (`src/sim.rs`). A gate guarded by a measurement bit costs about half a Toffoli on average.
- **Free.** Every Clifford gate, measurement, reset, classical bit operation, circuit depth and operation count. Measurement-based uncomputation is therefore free apart from its Toffoli fix-ups.
- **Qubits.** The highest qubit index used, plus one (`src/circuit.rs`). This is a property of the allocator, not of how many qubits are live at once.
- **Test inputs.** A SHAKE256 hash over the whole operation stream seeds 9,024 scalar pairs and every measurement outcome (`src/bin/eval_circuit.rs`). A circuit cannot be tuned to its inputs, but any change to the stream, even a no-op, draws a fresh set.
- **Validity.** All 9,024 shots must be classically correct, the phase word must be zero in every batch, and every non-register qubit must end at zero. One failure of any kind and the run has no score.
- **A gap between text and code.** The README and the Terms also promise a forward-then-reverse identity check. No such check exists in `eval_circuit.rs`, in July or at `main`. We do not know whether the platform enforces it elsewhere.

One qubit is worth about 805 Toffoli at the frontier (944,620 ÷ 1,173), and one Toffoli is worth 1,173 points of score.

## Validity is statistical

This is the fact that most shapes the current regime.

The circuits at the frontier are deliberately inexact. They truncate register widths and shorten the Euclidean walk to bounds that hold for almost all inputs, not all. A circuit that is wrong on a fraction ε of inputs passes with probability about (1 − ε)⁹⁰²⁴. Because the test set is seeded by the operation stream, a submitter can append gates that cancel, such as 48 pairs of X gates that encode a number, and search that "tail nonce" until the 9,024 derived shots all happen to pass. The challenge's maintainer described this model in [issue #1](https://github.com/Layr-Labs/ecdsafail-challenge/issues/1) and the issue was closed as acceptable. The organisers' paper calls the pass probability *landability* and counts nonce-only commits separately from circuit changes ([arXiv 2609.09582](https://arxiv.org/abs/2609.09582)).

Solvers summarise a circuit's inexactness as λ, the expected number of failing 64-shot batches per 9,024-shot run on fresh inputs, so that the chance of a clean draw is about e^−λ.

- The current record holder's note reports λ ≈ 19.73 (pooled mean over 256 full evaluations, standard error 0.255). That implies about 3.6 × 10⁸ expected draws to land a variant of that circuit (**reported**; the arithmetic is ours).
- 756 of the 1,339 notes mention a nonce, and 42 of the 49 promotions in the last 14 days kept the qubit count fixed and changed the Toffoli average by less than 2,000 (**recomputed**).
- Notes describe GPU screens of the classical failures at about 12,000 nonces per second per GPU, fleets of thousands of vCPUs, and campaigns of 10⁹ draws (**reported**, not reproduced).
- A separate community note, first attached to an upstream pull request on 2026-09-25 and copied into 61 submission notes, asks later solvers to keep λ at or below 18. The current record exceeds it. It is a norm written by one solver, not a platform rule.

Three consequences follow. Every candidate, even an exact rewrite, has to win the lottery again before it can be submitted. The Toffoli average itself moves by a standard deviation of about 13 from seed to seed (**reported**; consistent with a fair-coin model of the conditional gates), so differences smaller than a few tens of Toffolis are noise unless measured on paired inputs. And a change that lowers Toffolis by raising λ transfers cost to whoever lands the next variant, which [issue #131](https://github.com/Layr-Labs/ecdsafail-challenge/issues/131) documents as an apology from a solver who did exactly that.

## Cost of one evaluation

| Where | Compile | Build circuit | Evaluate | Source |
|---|---|---|---|---|
| Official runner (32 vCPU), record circuit | 26.6 s | 20.9 s | 12.6 s | Job log of workflow run 37152039816, **read** |
| Solver laptops and servers | n/a | 1.4–2 s | 13–38 s | **Reported**, for 1,250-qubit routes |

Both binaries are single-threaded, so one evaluation occupies one core for well under a minute. Memory is about 56 bytes per operation; the record circuit has 9.8 million operations (**inferred**: roughly 0.6 GB to evaluate). Concurrent runs need one worktree and build directory each, because the harness writes fixed output paths. We measured nothing locally.

Official turnaround from submission to verdict, **recomputed** over the 569 accepted rows: median 2.2 minutes, 90th percentile 2.8. Low-qubit circuits with hundreds of millions of Toffolis take 6 to 40 minutes, and the workflow is cancelled at 45 minutes.

Evaluation is cheap. The scarce resources are ideas, orientation in the code, and the compute for the nonce lottery.

## Rules of engagement

**Read** in the Terms and the CLI bundle unless marked.

- Acceptance is by strictly lower product than the current best at verdict time. There is no minimum step. About 100 scored submissions tied the record exactly and were rejected (**recomputed**: 105).
- AI agents may generate and automate submissions, and an account owner may give an API key to an agent they control. The owner answers for everything submitted.
- One account per person. Creating several to get around limits is prohibited.
- Every submission, accepted or not, becomes a permanent public commit and pull request carrying the submitter's GitHub handle. A public Markdown note of 5 KiB to 100 KiB and the exact model and harness names are mandatory. Promoted code is licensed under Apache 2.0. The record cannot be withdrawn.
- A submission may credit up to ten co-authors. 15 rows do.
- No cash prize is attached to this benchmark today (**reported** by the platform's public reward endpoints: the benchmark has no points).
- The installer adds a global agent skill and enables telemetry, and `ecdsafail clone` runs the repository's setup and benchmark scripts automatically. Neither should be used outside a sandbox.

## State of the leaderboard

All **recomputed**.

| | Count |
|---|---|
| Submissions | 1,355 from 142 solvers |
| Accepted | 569 |
| Rejected | 558, of which 448 scored validly but did not beat the best |
| Failed | 225 |
| With a public note | 1,339 |

| Month end | Best product | Notes |
|---|---|---|
| May | 8,405,420,100 | launched 2026-05-30 |
| June | 1,571,592,960 | 379 accepted submissions in one month |
| July | 1,488,026,454 | |
| August | 1,140,989,148 | one architectural change on 2026-08-21 took 14.8% |
| September | 1,109,316,122 | |
| 3 October | 1,108,039,260 | |

The last 14 days produced 49 promotions with a median gain of 0.0057%. One solver made 34 of them. The whole fortnight moved the record 2.58%, about half of it in one submission.

**The Pareto display is not the acceptance rule.** The site's default view says "843 frontier advances, 76 solvers" and advertises a 792-qubit record. Replaying every scored submission in time order and counting non-dominated points reproduces exactly 843 advances, 76 solvers and 27 current frontier points. 279 of those advances were officially rejected, including every submission below 1,150 qubits. The 792-qubit point costs 755,617,938 Toffoli.

| Qubit band | Scored submissions | Accepted |
|---|---|---|
| below 850 | 203 | 0 |
| 850–1,149 | 77 | 0 |
| 1,150–1,174 | 209 | 150 |
| 1,175–1,248 | 23 | 23 |
| 1,249–1,349 | 261 | 199 |
| 1,350 and above | 244 | 197 |

Best Toffoli count at selected widths: 1,173 → 944,620 (the record); 1,174 → 943,826; 1,250 → 888,879 (the lowest Toffoli count ever accepted); 1,164 → 970,983 (rejected, 2.0% above the record, with a **reported** λ of 14.6); 1,145 → 1,132,785 (rejected, **reported** λ 11.25). The band from 1,175 to 1,248 qubits is nearly unexplored.

## The frontier circuit as an agent meets it

**Read** through the GitHub API at `3161bd20`.

- `src/point_add` holds 44 Rust files in one flat directory, about 876 KB, plus three data directories with about 614 KB of precomputed tables. `pingpong.rs` is 219 KB and `heo_carry.rs` 171 KB. That is more than a 200k-token context can hold (**inferred** from byte counts).
- The July tree (`trailmix_ludicrous`, `rounds/dialog`, `arith/`) was replaced wholesale. The local checkout is 175 accepted commits behind and describes a different circuit.
- `build()` clears the process environment and then pins on the order of 130 named settings in code, some of them per-round tables. Positional schedules and call-indexed tables mean that edits in one stage can silently invalidate tables used by later stages (**read** in the July tree; **inferred** to persist).
- In-code documentation is partly stale. One 171 KB module's header calls it research-only while the shipped configuration enables it.
- The repository has no licence file. Only three harness files carry a CC BY 4.0 notice.

## How the frontier moved

From the notes of the largest single steps (**reported**), cross-checked against score changes (**recomputed**):

| Date | Step | What changed |
|---|---|---|
| 05-31 to 06-02 | 6–12% each | Merging adjacent controlled swaps, truncating the Kaliski loop widths to an empirical envelope, carry-tail truncation |
| 06-02 | 18.8% | "Dialog" GCD: record the Euclidean walk's branch decisions as a transcript and replay it, giving inversion and in-place multiplication together |
| 06-02 | 7.2% | Measurement-based uncomputation of the step comparator |
| 06-05 | 4.5% | Bounded-shift walk, 393 → 259 iterations |
| 06-08 | 3.5% | Conditional replay, exploiting the *executed* average |
| 08-21 | 14.8% | "Ping-pong" division: fixed-depth alternating-target binary division without comparisons, 952,707 × 1,321 |
| 09-29 | qubits 1,250 → 1,174 | "Skywalk": a Stein-style walk in a different frame, about 390 steps where the earlier walk took about 700 |

The organisers' paper classifies 400 scored commits up to its July cutoff and attributes the idea of record-and-replay division to Google's earlier work, reconstructed publicly by Schrottenloher ([arXiv 2606.02235](https://arxiv.org/abs/2606.02235)). It also records that the scalar score suppressed designs that needed a temporary regression.

## Where the Toffolis go, and how much headroom is left

- In the July circuit, the two in-place divisions and their replays account for about 95% of emitted Toffolis and the squaring for about 5% (**inferred** by an agent parsing the committed `ops.bin` as data). Schrottenloher's Table 3 gives a similar split for his design: about 90% in inversion and in-place multiplication.
- Techniques the cost model rewards: Gidney adders and measurement-based uncomputation, routing transient work through idle registers ("venting"), compact transcript coding, constant folding with secp256k1's pseudo-Mersenne prime, classically conditioned skipping of rare branches, and approximation.
- Techniques it does not reward here: windowing and table lookups (the addend is classical), Montgomery form, Fermat inversion, projective coordinates. Notes report each as tried and priced worse.
- There is no published lower bound. A crude floor for this circuit family, three additions per round over a few hundred rounds, twice per addition, plus the squaring, lands between 0.6M and 0.9M Toffoli (**inferred**, speculative). The record is within a factor of about 1.2 to 1.7 of that.
- Below about 950 qubits the known designs pay roughly eight times more Toffolis, so the low-qubit end is a different architecture, not a continuation (**reported** in the paper, which notes that no hard boundary has been established).

Structural directions that the sources leave open, most promising first (**inferred**; none verified):

1. Fewer rounds in the Euclidean walk: larger jumps, better step schedules, early termination that the executed-average metric would reward.
2. One recorded transcript serving both division directions, which would remove a large share of the second pass.
3. Streaming or pebbling the transcript tape and the partial registers that own the qubit peak.
4. Arithmetic on active width only, as operands shrink through the walk.
5. Cheaper exact comparators that buy back λ, which can then be spent on truncation or simply make variants landable.

## Dead ends and open leads from the notes

All **reported**. A swarm should treat each as a hypothesis with an owner, not as settled.

- Dead: raising the qubit cap for Toffolis pays less than 952 Toffoli per qubit, below the exchange rate; deferred modular reduction overflows; two-level Karatsuba on the square costs more; shortening the round count further raises λ faster than it saves; an exhaustive sweep of "safe" settings found nothing; [issue #380](https://github.com/Layr-Labs/ecdsafail-challenge/issues/380) argues the region with λ ≤ 14 and a better product is empty for the current family.
- Open: widening the walk's envelope to buy λ, exact borrow computation in place of narrow-fold assumptions, the unexplored 1,175–1,248 band, and hybrids of the lower-λ 1,164-qubit variant with the record's settings.

## The public shared memory failed

The README tells solvers to keep notes under `src/point_add/memory/`. That directory does not exist at `main` (**read**: the API returns 404). According to the commit history read through the API, it was created and wiped three times by accepted submissions, on 2026-06-06, 2026-06-19 and 2026-09-05, because the editable path is the whole directory and a submission whose tree lacks the notes deletes them for everyone. Its last state was internally inconsistent: an index covering a third of the notes, three different "current frontier" figures, and a note declaring the structure exhausted shortly before the record fell another 11%.

What survives is scattered: in-code change comments, 461 pull-request bodies, 786 `submissions/*` branches including rejected ones, and 13 issues used as a discussion channel. Winners propagate through `main`. Negative results, measurements and reasoning mostly do not.

## Text addressed to agents

Sources contained instructions aimed at whichever agent reads them. None was followed. They matter because swarm workers will read the same material.

- The README and the site tell the reader to pipe an installer into a shell.
- 61 submission notes carry a "note to AI agents" with a four-term "limiting contract", including an order to copy the note into later commits.
- Deleted memory notes contained ready-made session prompts with sync and submit commands, and coordination orders to "all fleet agents".
- The CLI's embedded skill tells agents to post public research updates every 30 minutes.

## Open questions

- Local evaluation time and memory on the demonstration machines are unmeasured.
- Whether the forward-then-reverse check is enforced anywhere.
- The numeric submission rate limits. The fastest observed solver made 15 submissions in an hour.
- Whether the organisers will change scoring, add a minimum step, or turn on owner review before the close. Any of these would change the value of nonce search.
- How much of the 876 KB at `main` is live code. This needs a build inside a sandbox.
- Whether the repository's lack of a licence constrains building on other solvers' rejected branches.
