# Research

Keep investigations, experiments, source references, and findings here. Research
is tracked in Git so its evidence and rationale remain available.

Record the question, sources and dates, method where relevant, findings, and open
questions. Distinguish hypotheses from measured results. Link adopted findings to
their decisions in [`../docs/`](../docs/README.md), and retain useful rejected or
deferred approaches.

Index each tracked Markdown document below using a relative inline link. After
staging new notes and index updates, run `python3 scripts/check_docs.py` from the
repository root. Untracked drafts are excluded from the check.

## Index

- [Local demo qualification](demo-qualification.md): exact API-4 candidate, fresh Codex/Claude through up, selected doctor, named administration, native workspace/restart and owned cleanup.

- [Separate-goal subgroup acceptance](subgroup-qualification.md): authenticated Engine tests for explicit selected-context export, membership isolation and fresh parent review of returned findings.

- [Organization local and default-network qualification](organization-local-discovery.md): exact-candidate T1 and six operations cases pass with production defaults; relay-free mDNS failure remains separate.

- [Protocol-1 replay and ingestion baseline](organization-protocol1-performance.md): measured healthy and pinned-member-fork histories before the organization engine replacement; exact harness, results and measurement limits.

- [Installed onboarding qualification](onboarding-qualification.md): committed macOS candidate, launchd and existing-daemon onboarding, identity/grant preservation and verified cleanup.
- [Formations](formations.md): research behind the accepted organization and authoring direction, with Merak comparison, primary sources, open collaboration and review/pool examples; detailed protocol mechanisms and migration remain proposals.
- [Formation authoring on locust.farm](formation-authoring.md): research for the web editor plan: later owner decisions (TypeScript checks, renamed contract fields, six ways of working, stage-only map) and Locust decoder quirks, then the earlier contract, Polaris canvas, site, prompt and ecosystem findings, compared designs and review records.
- [Formation editor review: first-time understanding](formation-editor-review.md): walkthrough, two model-played cold reads, a runtime source check and a word inventory of the built editor; where the page and the runtime disagree, and a proposal to show the rules as one line with four points; not accepted.
- [Exclusive claim on a task: proposal](exclusive-task-claim.md): what the accepted documents already require of a reservation, a third start rule kind, the goal's administrator as the one who hands tasks out, the events and replay rules, results posted without the task, release, replacement, offline and fork cases, the editor's new answer and ten questions for the owner. Proposal; nothing built.

- [T2 integration findings](t2-integration-review.md): nested-content replication, materialization races, CLI error/session boundaries and local verification scope.
- [Production T2 qualification](t2-production-qualification.md): production daemon/client experiments, real-model and scripted-provider boundaries, managed sessions and discovered compatibility defects.
- [Real-model T2 qualification](t2-real-model-qualification.md): all four mixed-client roles, skill and native workspace operations, exact Pi resume, Droid continuation failure and evidence hygiene.

- [T1 remediation](t1-remediation.md): disposition of all 36 independent-review findings, protocol-1 decisions, additional build/client-evidence repairs and verification boundaries.

### Last-mile experience — 2026-10-04

- [Collaboration follow-ups and simulation findings](collaboration-followups.md): short receipt references, explicit context pages, signed sources, invitation races, permission self-inspection and retained live-client observations.

- [Performance and agent cost pass](performance-cost-pass.md): paired API-3 schema, CLI/MCP, context and history measurements with exact behavior checks and explicit provider-cost boundaries.

- [Real-model shared-context pilot](shared-context-real-model-pilot.md): Merak/GPT Luna finding to Codex/GPT Luna implementation, independent patch/oracle evidence, session acknowledgment and retained citation/receipt friction.

- [First-user journey after formations](first-user-journey-review.md): current API-2 source and local-browser review separating completed setup/authoring work from remaining first-goal, human-view, shared-context, invitation and natural-language qualification gaps.

- [Bound CLI launcher qualification](bound-cli-launcher-qualification.md): committed candidate, fresh Codex and Claude Code profiles, native workspace commands through setup's launcher, and explicit scripted-provider limits.

- [What people and agents meet, and what to change](last-mile-experience.md): whether agents are prompted to communicate, what the log enforces, identity at joining, local and public views of a swarm, the first ten minutes for a person and for an agent; a measured hands-on run and real-model tool tally, proposals reviewed by seven critics and a follow-up source review, revised acceptance and sequencing, and a linked implementation plan. Most recommendations remain proposals; the licence decision and X3 bound-launcher implementation are separately recorded.

### TLA+ verification

- [Historical version-1 bounded models](tla/version-1.md): retained canonical branch, accepted-display and local-departure evidence with pinned superseded source links.

- [Model and runner guide](tla/README.md): pinned tools, reproducible bounded checks, input snapshots, result classification and evidence limits.
- [Historical property and implementation map](tla/property-map.md): protocol-0 rules, original findings and pinned superseded source links.
- [Upstream version-1 modeling impact](tla/upstream-impact-2026-10-04.md): merge assessment, changed rules, historical model boundaries and required rebaselining.
- [Stages 0 and 1 evidence](evidence/tla/README.md): bounded result summaries, retained counterexamples/witnesses, Rust fixture correspondence, development failures and unverified CI qualification.

### Simulating multi-machine tests — 2026-10-03

- [Simulation on one computer](multi-machine-simulation.md): the in-process machine simulator and the multi-process scenario runner now in the repository, how to run them, what each can and cannot show, nine findings with their verdicts, and the fixes applied in `24ddd21`.

### Independent review of the T1 candidate — 2026-10-03

- [Independent review of the release candidate](t1-candidate-independent-review.md): reproduction of the integration claims, 36 verified findings (10 at P2, none blocking the T1 guide), what was tested and held, and the earlier cross-review of the build helper and client harness. Probes are kept as patches under `evidence/t1-candidate-review/`, listed in the evidence index.
- [Orchestrator assessment](t1-candidate-review-response.md): independently reproduced high-priority defects, qualifications to fork-policy and durability fixes, a mismatched store verifier, and proposed repair order; no runtime fixes applied.

### T1 runtime integration — 2026-10-03

- [Runtime integration and discovery findings](t1-integration-2026-10-03.md): reviewed runtime fixes, the failed multicast-only run, passing default-network three-process flow and exact evidence boundaries.
- [M2 preparation and two-Mac join](t1-m2-smoke-2026-10-03.md): verified worker readiness, matching Rust/build inputs across different artifacts and a successful owner-authorized mixed-build relay join; task and recovery checks remain pending.

### Lane A implementation review — 2026-10-03

- [Implementation and takeover assessment](lane-a-review-2026-10-03.md): committed versus unfinished behavior, reproduced correctness gaps, verification results and a proposed integration sequence.
- [Reproduction appendix](evidence/lane-a-review-probes.md): workspace export, authority and sync probes with exact setup and evidence limits.

### ecdsa.fail as a swarm demonstration — 2026-10-03

- [Benchmark, rules and state of the field](ecdsa-fail-benchmark.md): scoring contract, statistical validity, failure-rate units and nonce search, evaluation cost, leaderboard figures, the frontier circuit, technique lineage, open and ruled-out directions. Source review and data analysis; no challenge code run.
- [Prior art for an agent swarm](ecdsa-fail-swarm-prior-art.md): the July Merak attempt and its root cause, Merak's swarm doctrine, how public solvers organise agents, published multi-agent evidence and derived design rules.
- [Leaderboard analysis appendix](evidence/ecdsa-fail-leaderboard-analysis.md): a reduced capture of the public submissions data, the script and its output behind the recomputed figures.

### Groups of agents versus one — 2026-10-04

- [Published evidence on groups of agents versus one](swarm-evidence.md): a demo pitch, then the published evidence for and against many participants on one hard problem, grouped by mechanism (pooling, best of many attempts, checking, shared archives, parallel breadth), what mixture of experts does and does not show, when groups lose, claims to avoid and answers to common objections. Source review; nothing run.

### First-contact integrations — 2026-10-03

- [Harness and Polaris sources](first-contact-integrations.md): documented MCP, skill, refresh and policy behavior of Claude Code, Codex, pi and Droid; Locust's current state; Polaris's native facade. Source notes, not qualification.

### Client qualification — 2026-10-03

- [Four-client qualification findings](client-qualification.md): real client binaries, scripted-provider evidence, default-policy differences and native session/bridge recovery checks.

### Transport qualification — 2026-10-03

- [Iroh transport probe findings](iroh-transport-probe.md): pinned-source routing behavior, relay/discovery operators, metadata boundaries and measured probe evidence.

### hcom client integration and reuse — 2026-10-03

- [hcom dissection](hcom-dissection.md): pinned-source client-integration patterns, permission boundaries and test evidence; runtime reuse was assessed and superseded by the decision to implement the ideas independently in Rust.
- [hcom validation evidence](evidence/hcom-validation.md): isolated CLI/delivery results, real Codex/Claude with scripted providers (including an intermittent Claude approval failure), and a storage-error probe.

### Implementation plan review — 2026-10-03

- [Independent review of the implementation plan](implementation-plan-independent-review.md): re-baselined objections, cut candidates, default-configuration client probes for Codex and Claude Code, opening-pass decisions, and prior-art projects worth a teardown.

### Agent integration and local execution — 2026-10-03

- [Agent-agnostic integration](agent-agnostic-integration.md): daemon transport, Codex/Claude/Pi bridges, native adapters and separate sandbox qualification.
- [Local sandbox integration](merak-native-local-sandbox.md): source-backed assessment of Merak's existing controls, sandbox gaps and a proposed macOS worker experiment.

### Distributed agent collaboration and MoltMesh — 2026-10-03

Start with the Locust implementation proposal for the recommendation, or MoltMesh architecture and consensus for the implementation review.

| Document | Contents |
|---|---|
| [Landscape](landscape.md) | Iroh, rust-libp2p, OpenDHT, p2panda, Willow, Radicle, A2A, MCP and MoltMesh |
| [A2A assessment](a2a-assessment.md) | Current standard and Rust SDK; concrete interoperability value, durable-coordination gaps and release recommendation |
| [MoltMesh architecture and consensus](moltmesh-architecture-and-consensus.md) | Component map, actor lifecycle, Raft integration, custom Tendermint and documentation drift |
| [MoltMesh networking and storage](moltmesh-networking-and-storage.md) | Discovery, NAT/relay assumptions, Bitswap, recovery, replication and offline availability |
| [MoltMesh task lifecycle and SDKs](moltmesh-tasks-and-sdk.md) | Leases, cursors, retries, idempotency, cancellation and durable notifications |
| [MoltMesh security](moltmesh-security.md) | Identity, authorization, trust boundaries, encryption and local API exposure |
| [Validation and reproducibility](moltmesh-validation.md) | Tests actually executed, observed results, release smoke and unverified boundaries |
| [Locust implementation proposal](locust-implementation-proposal.md) | Suggested Rust architecture, task/workspace contracts and staged acceptance experiments |
| [Evidence appendices](evidence/README.md) | Original local characterization probe source and captured results, preserved as Markdown |
| [Task probe appendix](evidence/task-probes.md) | Five local task lifecycle characterizations and reproduction instructions |
| [Raft restart probe appendix](evidence/raft-restart-probe.md) | Unsnapshotted component restart, duplicate application and reproduction instructions |

### Main conclusions

MoltMesh is close to the intended product shape and contains real working components. Its decomposition is useful prior art; its correctness and availability claims need substantial qualification. The most consequential findings are:

- **Reproduced locally:** five task lifecycle gaps; duplicate application of a committed entry after an unsnapshotted Raft component restart; database startup failure under the current CGO-disabled release configuration.
- **Source-established:** recovery checks hashes without proving authorized authorship/finality; several admission/authorization paths are incomplete; artifact transfer lacks a full workspace-sync and retained-replica contract; the custom Tendermint path has unresolved safety requirements.
- **Positive runtime evidence:** the existing multi-process discovery/task/result/replication/recovery demo passed on one machine. It uses one voter and online storage holders.
- **Mixed baseline:** the full Go race suite recorded 273 passing tests/subtests, one intermittent failure and one skipped soak test. Three focused repeats of the failure passed. The full suite was not green.

For Locust, the proposed direction is a small Rust daemon with explicit invitations, signed durable collaboration events, encrypted immutable artifacts, separate participant workspaces and an explicit authority for task/result acceptance. Investigate Iroh first and rust-libp2p as an alternative. Keep execution and sandbox selection with participants; keep protocol authorization and persistence inside the daemon. These are research recommendations, not accepted decisions.

### Provenance and interpretation

MoltMesh findings target default branch `actor-model`, exact commit [`707c870e3188df243e5aea3c4662daa3253270bc`](https://github.com/sahilpohare/MoltMesh/commit/707c870e3188df243e5aea3c4662daa3253270bc), dated 2026-09-17 and inspected on 2026-10-03. Links into that repository are pinned to the reviewed commit. Landscape documentation and GitHub status observations are dated snapshots and should be refreshed before dependency selection.

Documents distinguish implemented behavior, source-based findings, locally reproduced observations, inferred risks and proposed work. This research does not certify the protocol or claim a production penetration test, WAN validation, BFT proof, scale benchmark or language-model collaboration trial. Accepted decisions should be recorded separately in [`docs/`](../docs/README.md), with links back to this evidence.

- [Final network hardening](network-hardening-final.md): retained founding proof recovery, monotonic retry time, asynchronous invite address discovery and offline removal boundaries.
- [Operational qualification](operational-qualification.md): pinned three-daemon document, transfer, workspace, cancellation, withdrawal and rotation workflows.
- [Operational qualification evidence](operational-qualification-evidence.json): retained structured results for the six operational cases.

- [Local installation qualification](installation-qualification.md): trusted bootstrap, signed native install, upgrade, launchd lifecycle, failure/retry, data preservation and resource measurement harness.
- [Installed-client qualification](installed-client-qualification.md): persistent skill/MCP discovery, native task/workspace execution, policy denials and cleanup through test-signed installed candidates.
- [Current local macOS candidate identity](evidence/local-candidate-5bb254d-2026-10-03.json): source, binary, manifest and archive identities independently checked before the local installation/client campaigns.
- [Local installation qualification evidence](installation-qualification-evidence.json): exact native candidate, ten passing installation/upgrade/launchd cases and sampled resource observations.
- [Installed managed-client artifact binding](evidence/installed-client-artifact-bindings.json): unchanged managed-run hashes correlated with the verified installation, manifest, executable and subsequent uninstall.
- [Installed-client qualification verdicts](evidence/installed-client-qualification-2026-10-04.json): final native-client verdicts, public task/session/event identifiers, artifact identities and cleanup observations without retained skill bodies or provider payloads.
- [Linux build-only qualification](linux-installation-qualification.md): current organization-runtime and earlier Linux x86_64 cross-build identities, static artifact checks, and the stopped emulation attempt; no Linux runtime or service qualification.

- [Organization runtime performance comparison](organization-protocol2-performance.md): paired task/selection replay and ingestion measurements, exact proof indexing, and the remaining regression.

- [Organization formal models](tla/organization.md): current governance, completion, scoped proof, local attempt and daemon-effect safety/mutation checks with explicit finite limits.

- [Organization model verification evidence](evidence/tla/organization/README.md): all54 registered outcomes, full checker traces, bounded safety counts and precise verification limits.

- [Current manual recipe qualification](documentation-qualification.md): exact executable Markdown journeys, binary/recipe hashes and local daemon/application evidence.

- [Organization protocol 2 native package qualification](organization-native-qualification.md): exact native candidate installation, three supported persistent setups, four managed recovery clients, executable manual and installed offline discovery, with explicit unrun boundaries.

- [Farm development qualification](farm-qualification.md): local daemon/service recovery, browser/proxy evidence, and explicit discovery/real-harness boundaries.
