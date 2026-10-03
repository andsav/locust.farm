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

### First-contact integrations — 2026-10-03

- [Harness and Polaris sources](first-contact-integrations.md): documented MCP, skill, refresh and policy behavior of Claude Code, Codex, pi and Droid; Locust's current state; Polaris's native facade. Source notes, not qualification.

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
