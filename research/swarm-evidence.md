# Published evidence on groups of agents versus one

Research date: 2026-10-04. **Status: research note. Source review of published studies; nothing was run.** It sits beside the [prior art for an agent swarm on ecdsa.fail](ecdsa-fail-swarm-prior-art.md), which covers that benchmark, Merak's July run and the design rules drawn from them; this note does not repeat them.

## Question

Is there published evidence that many participants working on one problem do better than one, as in the wisdom of crowds or mixture of experts? Under which conditions, and where does it fail? The site says "Unleash collective intelligence on your hardest problems", and the founder wants to be able to argue for that choice. This note makes the strongest honest case and states where it breaks.

Short answer: yes, but only under conditions. Many independent attempts beat one on hard problems when the attempts are genuinely different and a trustworthy check can pick the good one. Without independence, diversity or a good check, the gain shrinks, and at equal compute it often disappears.

## Pitch

A pitch for demos, built only on the evidence below:

> Hard problems are the ones where a single attempt usually fails. On them, whether and how fast an attempt succeeds varies enormously from one try to the next, so the best of many attempts beats the typical one by a wide margin. This is why constraint solvers restart and run portfolios ([Gomes et al. 2000](https://www.cs.cornell.edu/selman/papers/pdf/00.jar.heavytails.pdf)), why open contests win on uncertain problems ([Boudreau et al. 2011](https://doi.org/10.1287/mnsc.1110.1322)), and why a coding model that solves 16% of SWE-bench Lite tasks in one try has a passing patch for 56% of them after 250 tries ([Brown et al. 2024](https://arxiv.org/abs/2407.21787)). Two things decide whether that potential is realised: the attempts must be genuinely different, and something trustworthy must recognise the right one. Locust is built around both. People and agents from different models make independent attempts, a shared record keeps what was tried and what failed, and someone other than the author reviews every result. Systems that keep a population of scored attempts and build on the record have found new results in mathematics and algorithms ([FunSearch](https://www.nature.com/articles/s41586-023-06924-6), [AlphaEvolve](https://arxiv.org/abs/2506.13131)), and in the one wall-clock-matched test so far, agents sharing a record beat the best of the same number of isolated agents on all three tasks ([CORAL](https://arxiv.org/abs/2604.01658)).

Say alongside it: this is for hard problems whose answers can be checked. For easy or tightly coupled work, one strong agent is usually better, and Locust has not yet measured its own gain.

## Sources

- Web searches in six areas: human collective intelligence; hard search problems and many attempts; machine learning ensembles and mixture of experts; many samples from language models; multi-agent language model systems; and failure modes. Each area looked for counter-evidence and published critiques as well as support.
- A second, independent pass checked 122 claims against their primary sources: the paper, an author copy, or the publisher abstract. All 122 were confirmed or corrected; 27 needed a corrected figure or citation, and this note uses the corrected version. Where only an abstract was read, the note says so.
- Sources suggested by the checking pass but not themselves checked are cited only where useful, and are marked "not checked".
- Evidence types: **theorem** (a proof or exact identity under stated assumptions), **controlled experiment**, **field study** (real settings, often observational), **benchmark** (scores on a fixed test set), **engineering report** (a team describing its own system). Posts by vendors about their own systems are self-reports. Many 2025 and 2026 sources are unreviewed preprints; the note says so where it matters.
- For each result the note says whether compute, tokens, calls or wall clock were matched. Most headline gains compare N attempts with one, and are not matched.

Most human evidence comes from estimation, forecasting and classification, not open-ended engineering. Applying it to coding agents is analogy. The language model evidence is closer, but mostly comes from benchmarks of short tasks.

## Evidence by mechanism

Five mechanisms explain most published gains. Each has its own conditions, so the case for Locust has to rest on each separately.

### 1. Pooling independent judgments cancels errors

The formal core is old and exact. It is also narrower than the popular version.

- **Condorcet jury theorem (theorem).** If voters are independent and each is right more often than wrong, majority accuracy rises toward certainty as the group grows. Once voters share evidence, majority accuracy converges to what that evidence supports and no further: the group "cannot beat the facts" ([Dietrich and Spiekermann](https://plato.stanford.edu/entries/jury-theorems/)). With correlated votes, the conditions for small groups are severe (Ladha 1992).
- **Diversity prediction theorem (theorem).** For averaged numeric predictions under squared error, crowd error equals average member error minus the spread of predictions ([Page 2007](https://depts.washington.edu/ctu/wordpress/wp-content/uploads/2019/07/Making-the-Difference_Applying-a-Logic-of-Diversity_Page_AcadMgtPerp-2011.pdf)). It is the same identity as the ambiguity decomposition for ensembles ([Krogh and Vedelsby 1995](https://proceedings.neurips.cc/paper_files/paper/1994/file/b8c37e33defde51cf91e1e03e51657da-Paper.pdf)). It guarantees beating the average member, not the best. Code cannot be averaged, so for code the analogue is selection.
- **Correlation multiplies error (theorem).** Breiman's [random forest bound](https://www.stat.berkeley.edu/~breiman/randomforest2001.pdf) grows with the mean correlation between members and shrinks with their individual strength.
- **Independence fails in practice (controlled experiments).** A vote of neural networks beat the best network, but by far less than independence predicts, because some inputs are hard for every network ([Hansen and Salamon 1990](https://machine-learning.martinsewell.com/ensembles/HansenSalamon1990.pdf)). Twenty-seven independently written versions of one program failed together on the same tests far more often than chance ([Knight and Leveson 1986](https://doi.org/10.1109/TSE.1986.6312924), publisher abstract only).

| Source | Type | Result | Matched? |
|---|---|---|---|
| [Galton 1907, re-analysed by Wallis 2014](https://arxiv.org/abs/1410.3989) | Field, one question | Median of 787 guesses 1,208 lb against a true 1,197 lb; the mean was exact | No: many guesses against one |
| [Kurvers et al. 2016](https://doi.org/10.1073/pnas.1601827113) | Observational, simulated groups from real diagnoses | Pooled doctors beat the best doctor only when their accuracy differed by less than 0.1 Youden's J; they always beat the average doctor | No |
| [Mannes, Soll and Larrick 2014](https://doi.org/10.1037/a0036677) | Archival data and simulation | Averaging the top 5 judges, ranked on a cue to ability, was accurate and robust; whole-crowd averaging and the single best judge each won only in some settings (abstract) | Not applicable |
| [Mellers et al. 2014](https://doi.org/10.1177/0956797614524255) | Field study, randomized conditions | In a forecasting tournament, teams beat independent forecasters, and aggregation weighted by track record and recency, then extremized, added more | People against people |
| [Atanasov et al. 2017](https://doi.org/10.1287/mnsc.2015.2374) | Controlled experiment | Team polls with that aggregation beat prediction markets, which beat a simple average (indexed abstract only) | People against people |
| [Wang et al. 2023](https://arxiv.org/abs/2203.11171), self-consistency | Controlled experiment | PaLM-540B on GSM8K: 56.5% to 74.4% with a vote over 40 sampled reasoning paths; 5 to 10 paths capture most of it; a weak model gained little | No: 40 times the decoding |
| [Lakshminarayanan et al. 2017](https://arxiv.org/abs/1612.01474) | Controlled experiment | ImageNet top-1 error 22.17% with 1 network, 19.10% with 5, 18.68% with 10 | No: M times the compute |
| [Schoenegger et al. 2024](https://doi.org/10.1126/sciadv.adp1528) | Field study, 31 questions | Median of 12 LLMs: Brier score 0.20, not significantly different from 0.19 for 925 human forecasters; GPT-4 alone scored 0.15 | No |

Two refinements matter for design. First, how the pool is formed: pooling peers of similar competence, or weighting by measured accuracy, beats counting heads (Kurvers; Mannes; Mellers). Second, exchange can help when first answers are independent. Estimates improved after exchange in decentralized networks, and got worse when a central node was wrong ([Becker et al. 2017](https://doi.org/10.1073/pnas.1615978114)). Averaging the consensus answers of four separate five-person discussions beat the average of thousands of independent answers ([Navajas et al. 2018](https://arxiv.org/abs/1703.00045)).

Two popular claims here are contested. The "collective intelligence factor" of [Woolley et al. 2010](https://doi.org/10.1126/science.1193147) has a mixed record: an independent meta-analysis found it correlated with group performance at r = .26, and few studies controlled for members' IQ ([Rowe et al. 2021](https://doi.org/10.1186/s41235-021-00285-2)). Even the originating group's pooled analysis found individual skill dominated on Sudoku-like tasks ([Riedl et al. 2021](https://doi.org/10.1073/pnas.2005737118)). The Good Judgment Project's teaming and training effects shrink or reverse in a 2025 re-analysis ([Hauenstein et al.](https://doi.org/10.1177/09567976241266481)); its aggregation result is the more robust lesson.

### 2. On hard problems the best attempt matters, not the average

When the effort one attempt needs varies wildly, the best of many attempts beats the typical one by a wide margin. This is extreme-value logic, not averaging. Hard problems are where such heavy tails show up.

- **Heavy tails and restarts (controlled experiment, equal compute).** On hard SAT and constraint problems, randomized solver runtimes were heavy-tailed, and many short restarts beat one long run by up to two orders of magnitude ([Gomes et al. 2000](https://www.cs.cornell.edu/selman/papers/pdf/00.jar.heavytails.pdf)). Later work showed good heuristics can remove the tail, so it depends on the method as well as the problem.
- **Restart schedules (theorem).** For any randomized algorithm whose answers can be checked, one universal restart schedule comes within a log factor of the optimum without knowing how hard the problem is ([Luby, Sinclair and Zuckerman 1993](https://www.cs.utexas.edu/~diz/Sub%20Websites/Research/optimal_speedup_of_las_vegas_algorithms.pdf)).
- **Portfolios (controlled experiment).** With 20 processors, 20 copies of the riskier, high-variance strategy gave the lowest expected runtime. Interleaved on one processor, 20 attempts made things worse ([Gomes and Selman 2001](https://www.cs.cornell.edu/selman/papers/pdf/01.aij.portfolios.pdf)). The large gain needs extra compute; at fixed compute, more attempts help only up to a point.
- **The ceiling (theorem with measurements).** Independent parallel runs give linear speedup only when runtimes are memoryless. A fixed minimum cost per attempt caps it ([Truchet et al. 2013](https://arxiv.org/abs/1212.4287)); for an agent, setup and reading the codebase are such a cost. At 3,072 cores, parallel SAT reached a median speedup of 41 and about 14% efficiency on the hardest instances, and the gains came from sharing learned clauses, not from independence alone ([MallobSat 2024](https://jair.org/index.php/jair/article/view/15827)).
- **Contests (field study and theorem).** Across 9,661 software contests, more competitors improved the best result on uncertain problems and hurt on well-understood ones, because each competitor put in less effort ([Boudreau et al. 2011](https://doi.org/10.1287/mnsc.1110.1322), abstract only). A contest model agrees: open entry is optimal for ideation and trial-and-error problems, less so for expertise problems ([Terwiesch and Xu 2008](https://faculty.wharton.upenn.edu/wp-content/uploads/2012/04/6-Innovation-Contests-and-Open-Innovation.pdf)).
- **Broadcast search (field study).** InnoCentive's outside solvers cracked 49 of 166 problems (29.5%) that firms' labs had often failed on, and winners tended to come from distant fields ([Jeppesen and Lakhani 2010](https://dash.harvard.edu/bitstream/handle/1/3351241/Jeppesen_Marginality.pdf)). About 70% stayed unsolved.
- **Language models (benchmarks).** On SWE-bench Lite, one open model went from 15.9% with 1 attempt to 56% with 250, against a 43% single-attempt state of the art ([Large Language Monkeys](https://arxiv.org/abs/2407.21787)). That 56% is coverage: some attempt passed the benchmark's own tests, so selection was an oracle. Matched on API cost, 5 attempts of the cheaper model solved more than one attempt of GPT-4o or Claude 3.5 Sonnet at under a third of the cost. Per problem, failure falls exponentially with attempts; the slower power law across a benchmark comes from a heavy tail of very hard problems ([Schaeffer et al. 2025](https://proceedings.mlr.press/v267/schaeffer25a.html)).

Human cases fit the pattern but are selected for success, and none is matched on effort. [Foldit](https://pmc.ncbi.nlm.nih.gov/articles/PMC2956414/) players beat the Rosetta protocol on 5 of 10 blind puzzles, and got stuck where the starting models lacked diversity. [Polymath](https://www.cs.cmu.edu/~jcransh/papers/cranshaw_kittur.pdf) found a new proof of the k = 3 case of density Hales-Jewett in about six weeks, but the top 10 of 39 contributors wrote nearly 90% of the comments. The [Netflix Prize](https://www2.seas.gwu.edu/~simhaweb/champalg/cf/papers/KorenBellKor2009.pdf) was won by blending hundreds of predictors; Netflix did not deploy the final blend, citing engineering cost and a shift in focus.

The limit is the quality of each attempt. When no attempt has a real chance, more attempts add little. At matched FLOPs on the hardest maths problems, a larger model beat extra test-time compute ([Snell et al. 2025](https://arxiv.org/abs/2408.03314)). AlphaCode 2 needed about 100 samples to match AlphaCode at a million, with a better model and scorer ([technical report](https://storage.googleapis.com/deepmind-media/AlphaCode2/AlphaCode2_Tech_Report.pdf), a self-report).

### 3. Checking is cheaper than solving

Mechanism 2 pays only if something can pick the good attempt. Where a cheap, trustworthy check exists, generate many and verify. The checker sets the ceiling.

| Source | Type | Result | Matched? |
|---|---|---|---|
| [Cobbe et al. 2021](https://arxiv.org/abs/2110.14168) | Controlled experiment | A 6B model with 100 samples and a trained verifier slightly beat a fine-tuned 175B model on GSM8K; past 400 samples accuracy fell, as search found answers that fooled the verifier | No |
| [Lightman et al. 2023](https://arxiv.org/abs/2305.20050) | Controlled experiment | Same 1,860 candidates per MATH problem: step-level judge 78.2%, final-answer judge 72.4%, majority vote 69.6%; the two judges were trained on different data | Matched on samples |
| [AlphaCode 2022](https://doi.org/10.1126/science.abq1158) | Benchmark | Up to a million programs per problem; example tests removed about 99%; clustering by behaviour chose 10 submissions; solve rate grew log-linearly with samples | No |
| [Large Language Monkeys](https://arxiv.org/abs/2407.21787) | Benchmark | On MATH, coverage rose from 82.9% to 98.4% between 100 and 10,000 samples, while voting and reward-model selection rose only from 40.5% to 41.4% | No |
| [CodeMonkeys 2025](https://arxiv.org/abs/2501.14723) | Benchmark, preprint | SWE-bench Verified: some attempt correct on 69.8%; random pick 45.8%; vote with model-written tests 53.0%; selector 57.4%, about half the gap. Pooling five systems' patches: coverage 80.8%, selector 66.2%, best single system 62.8% | Cost reported; no matched baseline |
| [Anthropic, Claude 4](https://www.anthropic.com/news/claude-4) | Self-report | Opus 4 on SWE-bench Verified: 72.5% with one attempt, 79.4% with parallel attempts filtered by visible tests and ranked by an internal scorer | Attempts and compute not disclosed |
| [Stroebl, Kapoor and Narayanan](https://arxiv.org/abs/2411.17501) | Controlled experiment, ICLR 2026 | When unit tests pass some wrong answers, resampling weaker models did not reach GPT-4o's single-attempt accuracy; with a cost on false accepts, the best number of attempts was often under 10 | Compute treated as free |

Benchmark tests are themselves imperfect. Large Language Monkeys reports flaky tests on 11.3% of SWE-bench Lite problems. A study of SWE-bench found patches counted correct that fail the developers' full test suite ([Wang, Pradel and Liu](https://arxiv.org/abs/2503.15223), not checked directly). Read the absolute numbers above with that in mind.

### 4. Populations that share an archive and keep diversity

On open-ended optimisation, a population of attempts that records scored results and keeps different lines alive beats a single trajectory. This is the mechanism closest to Locust's design, and its matched evidence is thin.

- **Keeping diversity (controlled experiments, equal evaluations).** On a deceptive maze, searching for new behaviour solved 39 of 40 runs against 3 of 40 for objective-driven search ([Lehman and Stanley 2011](https://www.cs.swarthmore.edu/~meeden/DevelopmentalRobotics/lehman_ecj11.pdf)). Keeping the best solution in each region of a feature space found a better single best on a deceptive task, and no better one on a less deceptive task ([MAP-Elites](https://arxiv.org/abs/1504.04909), an unreviewed draft).
- **Independent starts reach different solutions (controlled experiment, preprint).** Networks from different random starts disagree on many predictions, while variations around one training run stay within one solution ([Fort et al. 2019](https://arxiv.org/abs/1912.02757)). Rephrasing one plan is not an independent attempt.
- **Different strategies over copies of the best (theorem and simulation, contested).** In a search model, a random group of solvers beat a group of the individually best, because the best were alike ([Hong and Page 2004](https://doi.org/10.1073/pnas.0403723101)). [Thompson 2014](https://www.ams.org/notices/201409/rnoti-p1024.pdf) argues the theorem restates its assumptions. [Singer 2019](https://doi.org/10.1086/701074) re-ran the model over a million groups and found maximally diverse groups only slightly ahead of random ones (96.33 against 96.04). The safe reading: mixing strategies can help on rugged problems, the effect may be small, and ability still matters.
- **Program search with LLMs.** [FunSearch](https://www.nature.com/articles/s41586-023-06924-6) sampled about a million programs per run into islands and reseeded the worst islands from the best; only 4 of 140 runs found the record cap set, and the main text has no matched single-trajectory comparison. In [AlphaEvolve](https://arxiv.org/abs/2506.13131)'s compute-matched ablation on two tasks, building on the archive beat repeatedly prompting from the initial program (a white paper, not peer reviewed; curves only). [ShinkaEvolve](https://arxiv.org/abs/2509.19349) beat a fixed model mix and a single model by letting a bandit pick which model proposes next, at matched samples.
- **The closest matched test (controlled experiment).** In [CORAL](https://arxiv.org/abs/2604.01658), four agents in isolated workspaces with shared persistent memory beat the best of four independent runs at equal wall clock on all three tasks. Disabling shared notes and skills cost 18.6% on one task. It is one paper and three tasks, matched on wall clock and not tokens.
- **Shared context in coding (benchmark, preprint).** [DeLM](https://arxiv.org/abs/2606.10662) agents sharing a task queue and findings beat single-agent harnesses and a central orchestrator on Terminal-Bench and SWE-bench Verified. The Terminal-Bench runs use 10-task subsets with spreads near ±13 points, and there is no baseline of independent agents plus selection at matched cost.
- **A peer swarm without a planner herds (observational).** In [Agora](https://arxiv.org/abs/2609.18094), 13 agents over 12 days made steady progress on one task, but search concentrated on one lineage until the operators added views of neglected branches. There was no control arm.

The public crowd on ecdsa.fail is a field study of this kind, with no counterfactual; see the [prior art note](ecdsa-fail-swarm-prior-art.md).

### 5. Parallel breadth with isolated contexts

Splitting work across agents with separate context windows wins on breadth-first, decomposable tasks. Much of that gain is extra spending.

- [Anthropic's research system](https://www.anthropic.com/engineering/multi-agent-research-system) (self-report) beat a single agent by 90.2% on an internal eval. On BrowseComp, token use alone explained 80% of the variance in performance. Multi-agent runs used about 15 times the tokens of a chat. Upgrading the model gained more than doubling the token budget.
- A later [Anthropic post](https://claude.com/blog/building-multi-agent-systems-when-and-how-to-use-them) (self-report) puts multi-agent cost at 3 to 10 times a single agent, and says splitting agents by software role spent more tokens on coordination than on the work.
- With reasoning tokens matched across 260 configurations, the mean multi-agent effect was −0.3% (95% interval −58.7% to +77.2%): +80.8% on a decomposable finance task and −39% to −70% on sequential planning. Agents whose outputs nobody checked amplified errors 17.2 times, against 4.4 times with central checking ([Kim et al., Google and MIT](https://arxiv.org/abs/2512.08296), preprint). At equal thinking tokens on multi-hop questions, a single agent matched or beat every multi-agent design ([Tran and Kiela 2026](https://arxiv.org/abs/2604.02460), preprint).
- In vision, at matched inference FLOPs (training not counted), two separately trained models matched a larger one with about half the FLOPs, but only once single models were large; at small budgets single models won, and cascades won at every budget ([Wang et al. 2022](https://arxiv.org/abs/2012.01988)). A single larger network can match what an ensemble provides ([Abe et al. 2022](https://arxiv.org/abs/2202.06985)).
- Large agent builds worked only once the work split into independent, checkable pieces. Sixteen agents building a C compiler all stalled on the same kernel bug until a reference compiler was used to give each a different set of files ([Carlini 2026](https://www.anthropic.com/engineering/building-c-compiler), self-report, just under $20,000). Flat locking cut 20 agents to the throughput of 2 or 3, and planners with isolated workers fixed it ([Cursor 2026](https://cursor.com/blog/scaling-agents), self-report; the quality of its output was disputed).

### Mixture of experts

Mixture of experts is weak support for this argument.

The original [mixture of experts](https://www.cs.toronto.edu/~hinton/absps/jjnh91.pdf) (Jacobs et al. 1991) trains several small networks together with a gate that sends each case to one of them. On a vowel task it learned in about half the training time of one network, with the same accuracy. Modern sparse mixture of experts ([Shazeer et al. 2017](https://arxiv.org/abs/1701.06538); [Switch Transformer](https://arxiv.org/abs/2101.03961)) puts many feed-forward blocks inside one model and routes each token to one or two of them. It buys parameter capacity at fixed compute per token.

It does not show that many agents beat one:

- The experts are parts of one jointly trained model. None attempts the whole problem, and none is checked on its own.
- They are not subject specialists. In [Mixtral](https://arxiv.org/abs/2401.04088), routing followed syntax, not topic.
- Their ceiling is one big model. [DeepSeekMoE](https://aclanthology.org/2024.acl-long.70/) treats a dense model with the same total parameters as the upper bound. The gain is cost.

One lesson transfers: the router collapses onto a few favoured experts unless the system forces it to spread the load (Shazeer). Any task assignment layer faces the same pull.

The closer machine learning analogues are ensembles (mechanism 1), sampling with a verifier (mechanism 3), and routing or fusing whole models:

| Source | What it does | Result | Matched? |
|---|---|---|---|
| [FrugalGPT](https://lingjiaochen.com/papers/2024_FrugalGPT_TMLR.pdf) | Cascade across 14 commercial models with a learned answer scorer | Matches the best model at up to 98% lower cost, or beats GPT-4 by up to 4% at equal cost | Cost |
| [RouteLLM](https://arxiv.org/abs/2406.18665) | Sends each query to a strong or a weak model | Mainly saves cost; quality is bounded by the strong model | Cost |
| [LLM-Blender](https://arxiv.org/abs/2306.02561) | Ranks and fuses the outputs of 11 open models | The model most often ranked first was first on only 21% of inputs; ranking plus fusion beat every member | No: all 11 run |
| [Mixture-of-Agents](https://arxiv.org/abs/2406.04692) | Layers of different models refine each other's answers | 65.1% against 57.5% for GPT-4o, on a benchmark judged by GPT-4 | No |
| [Self-MoA](https://arxiv.org/abs/2502.00674) | Same design, but six samples from the single best model | 65.7 against 59.1 for the six-model mix; mixing helped only slightly, and only when models were comparably strong and complementary | Matched on proposals |

## When groups lose

Each condition below has direct evidence. Most failures come from members being alike or from how they interact, not from headcount.

### Correlated errors

- When two LLMs are both wrong, they pick the same wrong answer about 60% of the time on HELM, against one in three by chance. More accurate models are more correlated, even across providers ([Kim et al. 2025](https://arxiv.org/abs/2506.07962)).
- Mistakes grow more similar with capability, and LLM judges favour models like themselves ([Goel et al. 2025](https://arxiv.org/abs/2502.04313)).
- With correlated cues, a small group is often the most accurate size ([Kao and Couzin 2014](https://doi.org/10.1098/rspb.2013.3305), theorem).
- If many decision-makers adopt the same more accurate algorithm, overall decision quality can fall ([Kleinberg and Raghavan 2021](https://doi.org/10.1073/pnas.2018340118), theorem).
- On queries where a wrong answer is the most likely output, more votes make the group more surely wrong ([Chen et al. 2024](https://arxiv.org/abs/2403.02419), theorem and experiments).

### Herding and social influence

- Seeing others' estimates narrowed a crowd's spread without improving accuracy, and raised confidence ([Lorenz et al. 2011](https://doi.org/10.1073/pnas.1008636108)).
- In sequential choices on hard questions, wrong majorities persisted and did not self-correct ([Frey and van de Rijt 2021](https://doi.org/10.1287/mnsc.2020.3713), abstract only).
- Informational cascades follow when only actions are visible ([Bikhchandani et al. 1992](https://doi.org/10.1086/261849), theorem). Lab cascades proved shorter-lived than the theory predicts.
- Groups discuss what everyone already knows and miss facts held by one member ([Stasser and Titus 1985](https://doi.org/10.1037/0022-3514.48.6.1467)). LLM groups do the same: 30.1% accuracy, against 80.7% for one agent given all the information ([HiddenBench](https://arxiv.org/abs/2505.11556)).
- Agora's peer swarm herded onto one lineage (mechanism 4).

### Coupled tasks and coordination cost

- Two coding agents, each implementing one of two interacting features, did worse than one agent doing both, despite having two budgets. For the strongest models success fell by about half, and by 30% on average ([CooperBench](https://arxiv.org/abs/2601.13295), preprint).
- Sequential planning lost 39% to 70% at matched tokens (Google and MIT).
- Distributed software work items took about 2.5 times as long, mostly because more people touched them ([Herbsleb and Mockus 2003](https://doi.org/10.1109/tse.2003.1205177), field study).
- Interacting human groups matched their fastest member, and beat their most efficient one, only on complex tasks. On solution quality the top individual still won ([Almaatouq et al. 2021](https://doi.org/10.1073/pnas.2101062118), labour-matched).
- Seven multi-agent frameworks failed on 41% to 86.7% of tasks, mostly from unclear specification, misalignment between agents and missing verification ([MAST](https://arxiv.org/abs/2503.13657)).

### Equal-compute comparisons erase gains

- The founding debate results gave debate more calls than the baselines ([Du et al.](https://proceedings.mlr.press/v235/du24e.html); [Liang et al.](https://arxiv.org/abs/2305.19118)).
- At an equal number of responses, self-consistency beat debate on GSM8K, 88.2 to 83.0 ([Huang et al. 2024](https://arxiv.org/abs/2310.01798)).
- A vote over five answers beat debate at lower cost. The authors prove that debate alone does not raise expected correctness under their model ([Choi et al. 2025](https://arxiv.org/abs/2508.17536)).
- Debate rarely beat plain chain of thought across 36 settings ([Zhang et al. 2025](https://arxiv.org/abs/2502.08788), preprint). One agent with a worked example matched the best discussion ([Wang et al. 2024](https://aclanthology.org/2024.acl-long.331.pdf)).
- With equal tuning effort, debate was significant on no benchmark, while best-of-N gained a few points at 2 to 4 times the tokens ([Leins et al. 2026](https://arxiv.org/abs/2608.00685), preprint).
- Add the token-matched results of Google and MIT and of Tran and Kiela (mechanism 5).

### Imperfect or gameable verifiers

- Optimising against a proxy grader first improves and then degrades true quality ([Gao et al. 2023](https://arxiv.org/abs/2210.10760)).
- Resampling cannot fix a checker's false positives (Stroebl et al.).
- One frontier model reward-hacked in 30.4% of runs on tasks where it could see the scoring function, against 0.7% on other tasks. The task sets differ in more than visibility ([METR](https://metr.org/blog/2025-06-05-recent-reward-hacking/), engineering report).
- The more attempts one grader selects among, the more chances a wrong one has to pass.

### Process loss

- People put in less effort when their output is pooled and anonymous ([Latané et al. 1979](https://web.mit.edu/curhan/www/docs/Articles/15341_Readings/Group_Dynamics/required_reading/4Latane_et_al_1979_Many_hands_make_light_the_work.pdf)). A meta-analysis of 78 studies found this robust, and smaller when individual contributions can be evaluated ([Karau and Williams 1993](https://doi.org/10.1037/0022-3514.65.4.681)).
- Interacting brainstorming groups produced far fewer ideas than the same number of people working alone, mainly because of turn-taking ([Diehl and Stroebe 1987](https://homepages.se.edu/cvonbergen/files/2013/01/Productivity-Loss-In-Brainstorming_Toward-the-Solution-of-a-Riddle.pdf)).
- Large teams tend to develop existing ideas, and small teams to disrupt ([Wu, Wang and Evans 2019](https://doi.org/10.1038/s41586-019-0941-9), observational).
- Weaker members dilute the pool: see Kurvers's competence gap and Self-MoA's six-model mix.

### A stronger single member

- When a better model exists, it often wins. GPT-4 alone beat a GPT-3.5 debate by 14 points (Liang et al.).
- See also AlphaCode 2, Abe et al., and Snell et al. on the hardest problems.

Groupthink is not on this list. Its experimental record is thin ([Esser 1998](https://doi.org/10.1006/obhd.1998.2758)), and the mechanisms above are better supported.

## How to argue it

### The defensible claim

On hard problems with a trustworthy check, many independent attempts by capable and genuinely different participants find solutions that one attempt usually misses, because success is rare and a correct minority answer can be recognised by checking rather than by vote. Recording every scored attempt, including failures, lets later attempts build on earlier ones, and the few matched tests (AlphaEvolve's ablation, CORAL) favour this over isolated attempts. The gain depends on independence, real diversity and strong review, and it shrinks or reverses on easy, tightly coupled or uncheckable work.

### Claims to avoid

- "The wisdom of crowds proves more agents are better." The theorems promise beating the average member, not the best, and only with independent errors.
- "Diversity trumps ability." The theorem is contested and the defended effect is small.
- "Mixture of experts shows many models beat one." It is one model, and its ceiling is a dense model of the same size.
- "Groups have a measurable collective intelligence." The general factor has a weak independent record.
- "Agents that debate reach better answers." At matched calls, voting or one better prompt does as well.
- "More agents always help." Correlated cues, voting on hard queries and imperfect checks all reverse it.
- "250 attempts solve 56% of SWE-bench Lite." That is coverage, with the benchmark's tests as an oracle.
- "The ecdsa.fail crowd beat a single team." There is no counterfactual.
- Any claim that Locust has shown a gain. Nothing has been measured.

The site's line can carry the defensible claim if "hardest problems" means hard and checkable, and "collective intelligence" means pooled attempts and findings rather than a group IQ.

### Common objections

| Objection | Short answer |
|---|---|
| "It is just more compute." | Partly true: token use explained 80% of the variance in Anthropic's system. But some gains survive matching: restarts at equal compute (Gomes 2000), committees at matched FLOPs (Wang 2022), a cheaper model with more attempts at under a third of the cost (Large Language Monkeys), the AlphaEvolve ablation, and CORAL at equal wall clock. The honest pitch is spending compute well, not free gains. |
| "One bigger model does the same." | Often true when a bigger model exists (Abe; AlphaCode 2; Snell). At the frontier there is none, and committees pay most once single models are large (Wang 2022). |
| "LLM agents all think alike." | True (Kim; Goel). Diversity has to be built: different models, tools, starting points and information. Mixing model families was the change that consistently helped debate, though not always above the stronger model alone (Zhang). Independent starts reach different solutions (Fort). |
| "Multi-agent systems fail more often." | True of concurrent edits to coupled code and of free chat (CooperBench; MAST; Google and MIT). Independent attempts scored by an outside check are a different design, still unmeasured for Locust. |
| "A vote will lock in a wrong answer." | Yes, on hard queries (Chen 2024). Do not vote; select by check and review. |
| "The scorer will be gamed." | It will be tried (METR). Keep scoring outside participants' write access, and have non-authors review. This lowers the risk; it does not remove it. |
| "Human crowd studies do not transfer to code." | Agreed; they are analogy. The direct evidence for code is sampling with verification and program search. The human work informs structure. |

## What this means for Locust

These are implications from the evidence, not measured results. Locust has not shown any gain.

| Condition from the evidence | Protocol feature that would hold it | Basis |
|---|---|---|
| First answers must be independent | Independent attempts, recorded before a participant reads peers' results | Lorenz; Frey and van de Rijt; Becker; Diehl and Stroebe; Choi |
| Select by check, not by headcount | Harness-owned scoring; model verdicts are advisory | Large Language Monkeys; Chen 2024; Stroebl; CodeMonkeys |
| Checkers are fooled and gamed; similar judges favour similar work | Review by a non-author, ideally another model family or a person; scorer outside participants' write access | METR; Gao; Goel; MAST |
| Unique evidence must reach the decision | Shared record of findings, including negative results, as typed entries rather than chat | Stasser and Titus; HiddenBench; AlphaEvolve; CORAL |
| Diversity must be kept alive | Keeping diverse attempts alive; showing concentration and neglected branches | Lehman and Stanley; MAP-Elites; FunSearch; Agora |
| Errors correlate across similar models | Mixing models and people, tools and starting points; weighting contributions by demonstrated quality, not by count | Kim; Goel; Zhang; Fort; Self-MoA; ShinkaEvolve |
| Coupled work loses | Attempts on isolated branches; one coupled change has one writer | CooperBench; Google and MIT; Herbsleb and Mockus; Cursor |
| Pooled, anonymous effort shrinks | Every attempt, finding and review attributed and evaluated | Latané; Karau and Williams |
| Returns diminish fast | A formation sets the number of attempts, small by default; estimate single-attempt success early | Schaeffer; Stroebl; Mannes (top 5); deep ensembles (5 members); Kao and Couzin |
| Task fit decides | A formation picks one strong agent for easy, well-specified work, and many attempts for hard, uncertain, checkable work | Boudreau; Terwiesch and Xu; Almaatouq; Snell |

Where no trustworthy check exists, most of this case falls away. Locust's value then rests on the shared record and on human review, which this evidence does not measure.

A fair test needs three arms at equal budget: one agent, k independent agents with selection, and k agents sharing a record. The [prior art note](ecdsa-fail-swarm-prior-art.md) sets out such a design for ecdsa.fail.

## Open questions

- Does a shared record beat independent attempts plus selection at matched tokens on coding work? CORAL matched wall clock, DeLM had no such baseline, and AlphaEvolve's ablation covers two tasks.
- How fast should findings spread? In simulation, efficient networks did better in the short run and worse in the long run ([Lazer and Friedman 2007](https://doi.org/10.2189/asqu.52.4.667)); web experiments found the opposite ([Mason and Watts 2012](https://doi.org/10.1073/pnas.1110069108)); intermittent contact got the best of both ([Bernstein et al. 2018](https://doi.org/10.1073/pnas.1802407115)). None of these was checked.
- Can an aggregator recover a correct minority when most participants share a mistake? The "surprisingly popular" method claims to ([Prelec et al. 2017](https://doi.org/10.1038/nature21054), not checked).
- How correlated are errors on open-ended coding attempts? Kim et al. measured multiple-choice questions.
- Is a reviewer from another model family independent enough, or are people needed? Goel et al. suggest similarity bias grows with capability.
- How do mixed groups of people and agents perform? Almost nothing measures them. In one small, less controlled study, a simple average of human and machine forecasts beat machines that updated on the human median (Schoenegger et al.).
- What baseline should Locust beat? Simple retry baselines outperformed complex agent designs at lower cost in [Kapoor et al. 2025](https://arxiv.org/abs/2407.01502) (not checked).