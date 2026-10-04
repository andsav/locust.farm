# Documentation

Keep project documentation, accepted design decisions, and implementation plans
here. Exploratory work belongs in [`../research/`](../research/README.md).

Give each document a clear status, keep it aligned with the implementation, and
link to the research behind decisions. Preserve superseded decisions with a
supersession note rather than deleting their rationale.

Index each tracked Markdown document below using a relative inline link. After
staging new documents and index updates, run `python3 scripts/check_docs.py` from
the repository root to check coverage and local link paths.

## Index

- [Locust implementation plan](implementation-plan.md): design and current implementation status, the two-Mac first pass, remaining client/workspace work and release acceptance tests.
- [Protocol contract, version 1](protocol-v1.md): current authority, deterministic fork selection, reconciliation, local API and recovery rules, with enforcing code/tests and explicit version-0 incompatibility.
- [Archived protocol contract, version 0](protocol-v0.md): historical encoding, event, invitation, synchronization and local API rules for interpreting version-0 artifacts.
- [TLA+ formal verification implementation plan](tla-verification-plan.md): Stages 0 and 1 implemented with historical version-0 and bounded version-1 models; replication, durability, full model CI and optional safety proofs remain proposed.
- [Crates and workstreams](workstreams.md): accepted crate split, lane responsibilities and current shared A/B ownership, cross-review, and the rules for sharing one checkout.
- [Lane A log](lane-a-log.md): contract notices, answers to lane B and reviews of lane B's commits.
- [Lane B log](lane-b-log.md): requests to lane A, reviews of lane A's commits and replies.
- [Lane B implementation log](lane-b-implementation-log.md): product objective, implemented slices, verification boundaries and useful client/transport observations.
- [T1 CLI run](t1-run.md): two-Mac command sequence, optional third-peer extension, network operators, restart/sleep evidence and the local three-process harness.
- [T1 build](t1-build.md): identified Apple Silicon artifact for local transfer to the two Macs; publication and public-download qualification are deferred.
- [Native release candidate packaging](packaging.md): unsigned macOS arm64 and Linux x86_64 candidate format, source identity, signature boundary and manual CI declarations.
- [Client qualification harness](client-qualification.md): isolated four-client scripted-provider checks, policy distinctions and recovery evidence boundaries.
- [Explicit managed client sessions](managed-clients.md): local launch, readiness, exact binding, native resume, pending work and conservative crash recovery.
- [Transport probe](transport-probe.md): runnable direct, relay-only and automatic transport qualification, local process checks and two-machine instructions.
- [Implementation plan review](implementation-plan-review.md): adversarial review of the proposed plan with prioritized objections, strengths and pre-implementation gates.
- [Second review response](implementation-plan-review-response.md): adopted plan refinements, qualified simplifications and claims not adopted.
- [Release evidence ledger](release-evidence.md): October 4 gate status and partial component evidence; complete release gates remain open.
- [First contact](first-contact.md): proposed first-session journey from one pasted prompt, harness routing, readiness and approval states, and the Polaris handoff; lists current status.
- [Lane C log](lane-c-log.md): lane C's requests to lanes A and B and the Polaris work, and its source reviews.
- [ecdsa.fail swarm demonstration proposal](ecdsa-fail-swarm-proposal.md): proposed roles, records, measurement stages and lanes for agents working the ecdsa.fail circuit benchmark as one goal; success levels, mapping to the current Locust commands, time-boxed run plan and owner decisions. Nothing built or run.
- [T2 coding-agent workflow](t2-workflow.md): MCP, operating skill, snapshots and contribution review/application.

- [Local verified installation](installation.md): explicit trust, withdrawal registry, reviewed activation and conservative software removal.
- [Local installation prompt](install-prompt.md): pasteable template for explicit local trust, reviewed software/service/client setup and separately observed readiness.
