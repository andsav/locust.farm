# Self-organizing agents: literature and reuse decisions

Status: selective primary-source review, 7 October 2026. This informs the
[comprehensive plan](../docs/self-organizing-collaboration-plan.md) and complements
the broader [groups-versus-one evidence review](swarm-evidence.md). It is not a
systematic review, novelty proof, reproduction or endorsement of a framework.

There is substantial prior art for almost every proposed mechanism. Locust
should reuse coordination concepts, method recipes, evaluation designs and
failure categories. The research opportunity is to establish which combinations
help current agents under Locust's actual authority, information, lifecycle and
resource constraints. A useful negative result is preferable to a new framework
whose benefit is assumed.

## Scope and reading boundary

The review covers 23 works in three groups: six classical coordination sources,
eight LLM organization/skills systems, and nine evaluation/diversity studies.
We searched those mechanisms, followed primary references, and checked author
papers, proceedings or publisher records. Versions are identified where material;
published and preprint titles of the same study are not counted twice. Full-text
methods/results were inspected except the explicitly limited reads below.
No framework was installed and no paid model experiment was performed.

Descriptions of a paper's mechanism/results are source claims. Every **Reuse**
sentence and the final mapping are our proposed application to Locust. The
[source assessment](merak10-locust-organization-2026-10-07.md) establishes the
current Locust/Merak side of those comparisons. No paper below by itself
qualifies independently governed Locust participants or proves their advantage.

## Classical foundations worth keeping

### 1. Contract Net — negotiated allocation

Reid G. Smith, **The Contract Net Protocol: High-Level Communication and Control
in a Distributed Problem Solver** (1980), IEEE Transactions on Computers.
[Primary paper](https://cse-robotics.engr.tamu.edu/dshell/cs631/papers/smith80contract.pdf),
sections III–IV.

Tasks are announced, participants bid, contracts are awarded and progress is
reported. Manager/contractor responsibilities are task-specific; directed
contracts can be accepted or refused. This is negotiation over who does work,
not proof that debating answers improves correctness.

**Reuse:** existing tasks/offers/declines first. A skill can request ability,
availability, a bounded output and notice of changed commitments. Do not build
an auction service without a measured need, or let an award override local
execution authorization.

### 2. Blackboard architectures — shared evidence and control are different

H. Penny Nii, **The Blackboard Model of Problem Solving and the Evolution of
Blackboard Architectures** (1986), AI Magazine.
[Official article](https://ojs.aaai.org/aimagazine/index.php/aimagazine/article/view/537),
[author's Stanford report](https://i.stanford.edu/pub/cstr/reports/cs/tr/86/1123/CS-TR-86-1123.pdf).

Shared problem state lets different knowledge sources contribute, with a
separate control design deciding what runs. A blackboard can still have strong
central strategic control. **Read limit:** journal abstract and indexed report
control discussion; scanned journal text was not fully readable.

**Reuse:** Locust already has the shared surface. Study relevance, visibility
and attention policy rather than creating another board or equating shared
storage with decentralized decisions.

### 3. GPGP — coordination methods chosen for the situation

Keith S. Decker and Victor R. Lesser, **Designing a Family of Coordination
Algorithms** (ICMAS 1995), expanded author report revised August 1995.
[Primary report](https://mas.cs.umass.edu/Documents/lesser/decker-94-14.pdf),
sections 2–3 and 4.3–4.4.

Generalized Partial Global Planning combines modular coordination mechanisms
with local scheduling. Task interactions determine which mechanisms help;
commitments can change and peers are informed. Paired simulation episodes and
mechanism combinations evaluate tradeoffs. This is a close conceptual precedent
for conditional organizing methods, not an LLM result.

**Reuse:** a small repertoire, evidence-driven activation and mechanism ablation.
Keep voluntary commitments explicit. Do not import the complete task-utility
and scheduling formalism merely because it exists.

### 4. MOISE+ — structure, goals and obligations

Jomi F. Hübner, Jaime S. Sichman and Olivier Boissier, **A Model for the
Structural, Functional, and Deontic Specification of Organizations in Multiagent
Systems** (SBIA 2002).
[Primary paper](https://moise.sourceforge.net/doc/publications/Hubner-sbia2002.pdf),
[publisher](https://link.springer.com/chapter/10.1007/3-540-36127-8_12).

The model distinguishes roles/groups, goals/plans/missions, and permissions or
obligations linking them. Organizational specification differs from its actual
enactment. The illustrative domain is soccer, not LLM teamwork.

**Reuse:** separate a temporary behavioral observer/lead from a formal permission
role, and the planned team from the observed team. Locust needs no second
organization language to study that distinction.

### 5. Electronic institutions — autonomy within enforceable rules

Marc Esteva, Juan A. Rodríguez-Aguilar, Carles Sierra, Pere García and Josep Ll.
Arcos, **On the Formal Specification of Electronic Institutions** (2001).
[Author-institution paper](https://www.iiia.csic.es/media/filer_public/2b/3e/2b3e434e-b7d5-4a3f-bdd7-acad526b46eb/iiia-2001-286.pdf),
[DOI](https://doi.org/10.1007/3-540-44682-6_8).

An explicit institutional framework constrains interactions among heterogeneous
participants while separating rules from players. **Read limit:** indexed
abstract/introduction and institutional description; PDF extraction was garbled.
This is a formal coordination precedent, not comparative quality evidence.

**Reuse:** distinguish permissible acts from useful acts. A daemon suggestion is
not a new obligation, and institutional acceptance is not external correctness.

### 6. Stigmergy — coordination through persistent traces

Francis Heylighen, **Stigmergy as a universal coordination mechanism I: Definition
and components** (Cognitive Systems Research, 2016).
[Author paper](https://pespmc1.vub.ac.be/Papers/StigmergyICognSystems.pdf),
definition and sections 7–8.

Actions leave traces in a shared environment that influence later actions,
supporting asynchronous coordination without continuous direct conversation.
The account combines positive reinforcement and corrective negative feedback
under assumptions about agents recognizing useful actions. This is a theoretical
account across domains, not an LLM benchmark.

**Reuse:** shared findings/artifacts are a coordination mechanism in their own
right. Measure useful reuse, duplicated work and anchoring; provenance and
visibility do not make a popular finding true.

## LLM organization, adaptation and skills

### 7. AutoGen — conversation as executable control

Qingyun Wu et al., **AutoGen: Enabling Next-Gen LLM Applications via Multi-Agent
Conversation** (2023), inspected v2.
[Primary paper](https://arxiv.org/html/2308.08155v2), section 2.2 and group-chat appendix.

Conversable agents combine models, tools and humans. Code and prompts jointly
determine control; a group manager can choose speakers dynamically. The dynamic
group example uses twelve manually crafted tasks, so it is infrastructure and
application evidence rather than a general self-organization result.

**Reuse:** explicit communication/control patterns and a centrally adaptive
baseline. This historical paper is not current API documentation, and runtime
speaker choice does not establish independent owner authority.

### 8. AgentVerse — recruited roles inside a prescribed outer loop

Weize Chen et al., **AgentVerse: Facilitating Multi-Agent Collaboration and
Exploring Emergent Behaviors** (2023 preprint; ICLR 2024), inspected v3.
[Primary paper](https://arxiv.org/html/2308.10848v3), sections 2–4 and Appendix A.

A recruiter describes experts; evaluation can cause recomposition within the
recruit/discuss/execute/evaluate sequence. Some expert counts and speaker orders
are prescribed. Tests span reasoning, code and generation, with a small manually
assessed tool task set. The paper also observes destructive behavior and erroneous
reviewer criticism.

**Reuse:** task-conditioned recruitment and environment feedback; measure
right-to-wrong as well as wrong-to-right changes. Emergent behavior is not a
synonym for useful behavior.

### 9. DyLAN — initial selection and online pruning

Zijun Liu, Yanzhe Zhang, Peng Li, Yang Liu and Diyi Yang, **A Dynamic LLM-Powered
Agent Network for Task-Oriented Agent Collaboration** (2023 preprint; COLM 2024),
inspected v2, November 2024.
[Primary paper](https://arxiv.org/html/2310.02170v2), sections 3.3–4.

Preliminary team selection is distinct from solving, during which an LLM ranker
can deactivate agents and agreement can end inference. HumanEval, WebShop,
MMLU and math experiments study this algorithmically controlled adaptation.
Ranked contribution and agreement remain proxies for correctness.

**Reuse:** separate available, activated and retained participants; account for
selection/ranker costs. Test whether a selector suppresses useful dissent.

### 10. GPTSwarm — optimization across executions

Mingchen Zhuge et al., **GPTSwarm: Language Agents as Optimizable Graphs**
(ICML 2024).
[Proceedings](https://proceedings.mlr.press/v235/zhuge24a.html),
[full paper v2](https://arxiv.org/html/2402.16823v2).

Node prompts and graph edges are optimizable using utility feedback. These
experiments distinguish node and edge optimization from a supplied graph's
execution. The GAIA demonstration explicitly uses neither optimization, so its
score cannot establish their benefit. Other experiments address MMLU,
crosswords and HumanEval.

**Reuse:** reusable operators and an optimized fixed comparator. Distinguish
learning a graph across trials from peers changing organization during work;
charge optimizer evaluations separately.

### 11. AFlow — learned workflow baseline

Jiayi Zhang et al., **AFlow: Automating Agentic Workflow Generation**
(2024 preprint; ICLR 2025), inspected v4.
[Primary paper](https://arxiv.org/html/2410.10762v4), sections 3–5 and Appendix D.

An LLM searches code-based workflows with Monte Carlo Tree Search, reusable
operators and execution feedback. Validation drives search; held-out splits of
six benchmarks evaluate the selected workflows. Headline cheap-model comparisons
concern test execution cost, not free workflow search.

**Reuse:** calibration-only optimization of fixed methods and explicit search
cost/amortization. An adaptive Locust arm should not win merely because its
fixed comparator was poorly chosen.

### 12. MorphAgent — close prior for supported self-organization

Siyuan Lu, Jiaqi Shao, Bing Luo and Tao Lin, **MorphAgent: Empowering Agents
through Self-Evolving Profiles and Decentralized Collaboration** (2024 preprint),
inspected v2, September 2025.
[Primary paper](https://arxiv.org/html/2410.15048v2), sections 3.2–5.

Agents revise profiles using peer interactions and feedback on clarity,
differentiation and task alignment, alternating updates with execution.
BigCodeBench, BBH and MATH tests principally use three same-model agents and
include synthetic nonresponse. This directly precedes feedback-supported role
adaptation; it does not qualify distinct owners, partition recovery or revoked
permissions. Linguistic profile measures are capability proxies.

**Reuse:** a role-feedback comparator for factual cues. Do not claim that Locust
invents supported self-organization or optimize role diversity instead of results.

### 13. Voyager — reusable skills without a team

Guanzhi Wang et al., **Voyager: An Open-Ended Embodied Agent with Large Language
Models** (2023), inspected v1.
[Primary paper](https://arxiv.org/html/2305.16291v1), sections 2–3.4.

A Minecraft agent builds and retrieves executable skills, chooses curriculum
tasks and responds to environmental/error feedback. Ablations and transfer
experiments examine those components. This is a single agent in a specific
environment, not evidence that prose organizing skills improve teams.

**Reuse:** methods with prerequisites, outputs and evidence-linked revision;
separate skill retrieval from feedback in ablations.

### 14. CORAL — closest system-level precedent

Ao Qu et al., **CORAL: Towards Autonomous Multi-Agent Evolution for Open-Ended
Discovery** (2026), inspected v3, September 2.
[Primary paper](https://arxiv.org/html/2604.01658v3), sections 3–4, Table 3 and
appendices C/E; [author code](https://github.com/Human-Agent-Society/CORAL).

Autonomous agents share persistent notes/skills, with reflection, consolidation
and redirection prompts. Four-agent collaboration beats best-of-four independent
runs on three reported ablation tasks. The comparison matches wall time, not
exact spend. Its evaluated teams are homogeneous; shared storage and process
control are centralized. Prompt and heartbeat components are not fully isolated.

**Reuse:** study its tasks, prompts and shared-memory control before inventing
equivalents. Locust should test independent ownership and support components
separately; periodic reminders are precedent, not a mandatory daemon policy.

## Evidence that changes the experimental controls

### 15. Capability and task structure can dominate topology

Yubin Kim et al., **Capable language models can outgrow the benefits of
collaboration** (Nature Machine Intelligence, July 2026), published version of
**Towards a Science of Scaling Agent Systems**.
[Published paper](https://www.nature.com/articles/s42256-026-01268-y),
[preprint v3](https://arxiv.org/html/2512.08296v3).

Detailed methods were read in preprint v3; publication metadata and the abstract
were checked against the publisher and author record. The inspected preprint
evaluates 260 configurations, six benchmarks, five architectures and
three model families. Results range from helpful coordination on decomposable
work to substantial degradation on sequential planning. Strong solo performance
predicts diminished gains. Its threshold is an empirical selection rule, not a
universal law; spontaneous self-organization remains outside the main comparison.
Older 180-configuration descriptions refer to an earlier version.

**Reuse:** stratify by task dependence and baseline ability. Preserve strong
homogeneous controls and actual coordination cost. Do not encode its numerical
threshold as Locust policy.

### 16. Debate needs an independent aggregation control

Hyeong Kyu Choi, Xiaojin Zhu and Sharon Li, **Debate or Vote: Which Yields Better
Decisions in Multi-Agent Large Language Models?** (NeurIPS 2025), inspected v2.
[Proceedings](https://proceedings.neurips.cc/paper_files/paper/2025/hash/934252acd87f254d5d4672fbde283bd2-Abstract-Conference.html),
[full paper](https://arxiv.org/html/2508.17536v2), sections 3–5.

Across seven NLP benchmarks, majority voting explains most measured debate
gains. Main models are 7B/8B with larger-model extensions. The accompanying
argument assumes a specific belief-update model, not arbitrary evidence-bearing
collaboration. Answer extraction can also distort results.

**Reuse:** independent generation plus a public selector, standardized extraction,
and correction/subversion counts. An oracle-assisted improvement is an upper
bound, not a usable deployed method.

### 17. A poorly tuned fixed baseline is misleading

Andries Petrus Smit et al., **Should we be going MAD? A Look at Multi-Agent Debate
Strategies for LLMs** (ICML 2024).
[Proceedings](https://proceedings.mlr.press/v235/smit24a.html),
[full paper v3](https://arxiv.org/html/2311.17371v3), sections 3–4.

Original debate implementations do not reliably beat self-consistency and other
ensembles, usually using more calls. Tuning debate and agreement parameters can
change rankings; transfer across model families is imperfect. These are QA
experiments, not a universal result for tool-using teams.

**Reuse:** calibrate fixed protocols fairly and record tuning cost. Reuse
DebateLLM definitions where applicable rather than inventing weak lookalikes.

### 18. Repeated samples of a strong model are a serious comparator

Wenzhe Li, Yong Lin, Mengzhou Xia and Chi Jin, **Rethinking Mixture-of-Agents: Is
Mixing Different Large Language Models Beneficial?** (2025), inspected v1.
[Primary paper](https://arxiv.org/html/2502.00674v1), sections 3–4.

Self-MoA often outperforms mixed-model aggregation in the tested generation,
knowledge, math and code tasks. Adding weaker proposers can lose more quality
than diversity gains; some mixtures of comparably strong complementary models
remain useful. This is output aggregation, not a complete distributed agent
experiment, and proposal counts do not ensure equal dollar cost.

**Reuse:** compare A+B with strong A+A/B+B sampling and synthesis. Distinguish
cross-model complementarity from the benefit of another sample.

### 19. Diversity can help, but the metric and controls matter

Yingxuan Yang et al., **Understanding Agent Scaling in LLM-Based Multi-Agent
Systems via Diversity** (2026), inspected v1.
[Primary paper](https://arxiv.org/html/2602.03794v1), sections 5.1–5.4 and Appendix B.

The study separates persona diversity, model diversity and both; small diverse
teams can beat larger homogeneous teams in its aggregation. Main experiments
use 7B/8B models and seven reasoning/knowledge benchmarks. Matching agent-call
counts does not match tokens or dollars. Its embedding-based diversity measure
is not a direct measure of useful knowledge.

**Reuse:** vary model and prompt diversity separately; measure complementary
errors and verified contributions, not brand count or semantic spread alone.

### 20. Provider diversity does not guarantee independent errors

Elliot Kim, Avi Garg, Kenny Peng and Nikhil Garg, **Correlated Errors in Large
Language Models** (ICML 2025).
[Primary paper](https://arxiv.org/abs/2506.07962).

The study examines more than 350 models across leaderboards and a resume task,
finding substantial error correlation even across architectures/providers.
Larger, more accurate models can remain strongly correlated. **Read scope for
this entry:** verified author abstract; no additional numerical claim from the
full analysis is used.

**Reuse:** measure disagreement on actual claims and tests. Different priors are
a hypothesis about useful behavior, not an inference justified by different
model names.

### 21. Equal requested budgets do not establish equal computation

Dat Tran and Douwe Kiela, **Single-Agent LLMs Outperform Multi-Agent Systems on
Multi-Hop Reasoning Under Equal Thinking Token Budgets** (2026), inspected v2.
[Primary paper](https://arxiv.org/html/2604.02460v2), sections 4–5 and Appendix C.

On FRAMES and MuSiQue 4-hop, solos generally match or beat tested teams under
matched requested thinking allowances; severe context corruption can reverse
the result. Requested caps do not force equal realized use, and Gemini token
accounting is approximate. Text-only reasoning does not settle tool-based or
cross-owner collaboration.

**Reuse:** same-information solo controls, actual usage accounting and a distinct
context-management control. Do not generalize the title beyond its tasks.

### 22. Reuse the existing failure taxonomy

Mert Cemri et al., **Why Do Multi-Agent LLM Systems Fail?** (2025), inspected v3.
[Primary paper](https://arxiv.org/html/2503.13657v3), sections 3–5 and Appendix A.

MAST groups fourteen failure modes under system design, inter-agent misalignment
and verification. The final dataset has 1,642 traces; taxonomy development used
150 expert-annotated traces. Intervention case studies are not a randomized
estimate of self-organization benefit, and some failures also affect solos.

**Reuse:** start with these labels and human-checked evidence; add explicit Locust
delivery, generation, authorization and lifecycle codes only where needed.
Failure classification remains separate from artifact scoring.

### 23. Evaluate useful systems, not just benchmark scores

Sayash Kapoor, Benedikt Stroebl, Zachary S. Siegel, Nitya Nadgir and Arvind
Narayanan, **AI Agents That Matter** (2024), inspected v1.
[Primary paper](https://arxiv.org/html/2407.01502v1), sections 3–5.

The paper identifies cost-blind evaluation, inadequate holdouts and inconsistent
benchmark practice. A holdout must match the intended generality: fresh examples
alone do not establish transfer to new tasks or domains.

**Reuse:** report quality/cost jointly, freeze genuine held-out problems and
disclose calibration/search cost. The paper supplies evaluation discipline,
not evidence that any particular team topology wins.

## What we should reuse, test and avoid building

| Design question | Reuse now | Test before product changes |
| --- | --- | --- |
| Who takes work? | Contract-Net-style bounded offers/refusals through existing tools | Whether capability/availability statements improve allocation enough to need structured bids |
| How can peers coordinate asynchronously? | Blackboard/stigmergic shared findings and exact artifacts | Shared-record reuse versus direct reciprocal exchange; provenance, freshness and attention |
| How should agents organize? | GPGP-style conditional methods; MorphAgent-style role revision as a comparator | Fact-grounded cues versus role/profile feedback; useful adaptation rather than role churn |
| Is a fixed workflow competitive? | Established debate, portfolio and observer recipes; calibration-only selection/optimization | Adaptive peers versus a competent fixed team, then versus a centrally adaptive manager |
| Do skills or prompts help? | Versioned method recipes, explicit prerequisites and outcomes | Skills/cues ablation; discovery, delivery, use and effect separately |
| Does diversity matter? | Strong same-model sampling/aggregation controls | Complementary mistakes, harmful persuasion and actual costs under current models |
| Why did a run fail? | MAST vocabulary plus existing Locust lifecycle evidence | Human-checked causal evidence; do not diagnose from node labels alone |

Do not introduce a new graph optimizer, auction language, agent society schema,
board or central scheduler for the first study. Do not copy another framework's
private defaults, stopping rules or process authority into Locust. If source code
is reused later, inspect its pinned implementation and license first; reading a
paper is not a software compatibility review.

Describe adaptation by **when** it happens (before a trial, between attempts,
during work), **who** chooses it (designer, optimizer, coordinator, participants),
and **whose execution authority** applies. These distinctions prevent a learned
fixed graph, a central adaptive manager and an independent peer organization
from being treated as the same experimental condition.

The closest useful reading sequence is GPGP, MorphAgent, CORAL, the capability
scaling study, Self-MoA, and MAST. Together they challenge both easy stories:
that coordination requires a rigid pipeline, and that more autonomous or diverse
agents necessarily solve problems better. Locust's plan should preserve that
uncertainty and make the comparison reproducible.
