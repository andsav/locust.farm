# Research and experiments

These notes retain public source assessments, current design questions and
reproducible experiments. Each experiment names its method, candidate and limits;
an earlier measured artifact does not qualify every later API/protocol change.
Current accepted behavior belongs in [the manual](../docs/guide/README.md).

## Investigations and experiments

- [A2A and Locust](a2a-assessment.md)
- [Agent ergonomics audit and proposals](agent-ergonomics-2026-10-05.md)
- [Agent integration and local execution boundaries](agent-agnostic-integration.md)
- [Collaboration follow-ups and simulation findings](collaboration-followups.md)
- [ecdsa.fail: benchmark, rules and state of the field](ecdsa-fail-benchmark.md)
- [Public prior art for agent research on ecdsa.fail](ecdsa-fail-swarm-prior-art.md)
- [Exclusive claim on a task: proposal](exclusive-task-claim.md)
- [Farm publication usability audit](farm-publication-ux.md)
- [Farm development qualification](farm-qualification.md)
- [Joinable public farms: proposal](joinable-public-farms-2026-10-05.md)
- [Joinable farms plan: what the rewrite must change, and its contract](joinable-farms-rewrite-contract-2026-10-05.md)
- [Roles and permissions: what exists and a simpler model](roles-and-permissions-2026-10-05.md)
- [Roles and permissions implementation plan review](roles-and-permissions-plan-review-2026-10-05.md)
- [Gas Town lessons for shared workspaces](gastown-shared-workspace-assessment.md)
- [Organization design research](formations.md)
- [Host-key failure characterization](host-key-failure-characterization-2026-10-05.md)
- [Replacing a host that is gone: groundwork, four designs and what survived](replacing-a-host-2026-10-05.md)
- [Ending a goal and cleaning up: what exists, one design and what broke](ending-a-goal-2026-10-05.md)
- [hcom dissection and reuse assessment](hcom-dissection.md)
- [Iroh transport probe findings](iroh-transport-probe.md)
- [Distributed agent collaboration landscape](landscape.md)
- [Live farm rehearsal — October 4, 2026](live-farm-demo.md)
- [Real agents on a board without work roles — October 5, 2026](role-free-board-2026-10-05.md)
- [Live shared workspace with Luna and Haiku — October 5, 2026](live-shared-workspace-models-2026-10-05.md)
- [MoltMesh architecture and consensus](moltmesh-architecture-and-consensus.md)
- [MoltMesh networking, artifacts, and offline behavior](moltmesh-networking-and-storage.md)
- [MoltMesh security and trust-boundary review](moltmesh-security.md)
- [MoltMesh task lifecycle and SDK reliability review](moltmesh-tasks-and-sdk.md)
- [MoltMesh validation and reproducibility](moltmesh-validation.md)
- [Reproducible multi-machine simulation](multi-machine-simulation.md)
- [Network recovery constraints and experiments](network-hardening-final.md)
- [Public source review](open-source-review.md)
- [Performance and agent cost pass](performance-cost-pass.md)
- [Real-model shared-context pilot](shared-context-real-model-pilot.md)
- [Shared file tree architecture investigation](shared-file-tree-2026-10-04.md)
- [Shared tree local daemon loop](shared-file-tree-local-loop-2026-10-05.md)
- [Shared tree content indexing measurements](shared-file-tree-content-index-2026-10-05.md)
- [Shared file tree design review and accepted revisions](shared-file-tree-review-2026-10-05.md)
- [Separate-goal subgroup acceptance](subgroup-qualification.md)
- [Published evidence on groups of agents versus one](swarm-evidence.md)

## Formal verification

- [Executable model guide](tla/README.md)
- [Organization property and implementation map](tla/organization.md)
- [Workspace authority model and recovery contract](tla/workspace.md)
- [Evidence and reproducible helpers](evidence/README.md)

## Evidence appendices

- [Three-agent board trial evidence](evidence/role-free-board-2026-10-05/README.md)

- [ecdsa.fail leaderboard analysis](evidence/ecdsa-fail-leaderboard-analysis.md)
- [hcom validation evidence](evidence/hcom-validation.md)
- [Unsnapshotted Raft restart characterization probe](evidence/raft-restart-probe.md)
- [Task lifecycle characterization probes](evidence/task-probes.md)
- [TLA+ evidence](evidence/tla/README.md)
- [Organization protocol model verification evidence](evidence/tla/organization/README.md)
