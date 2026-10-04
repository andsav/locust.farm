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

- [Locust implementation plan](implementation-plan.md): proposed architecture, coordination and task contracts, Locust-owned Rust client adapters, one-prompt installation, milestones and acceptance tests.
- [Protocol contract, version 0](protocol-v0.md): encoding, hashing and signature decisions, the signed event header and event kinds, invitations, sync frames, local API and storage seam; types implemented, rules not yet.
- [Crates and workstreams](workstreams.md): accepted crate split, the two lanes and their ownership, cross-review, and the rules for sharing one checkout.
- [Lane A log](lane-a-log.md): contract notices, answers to lane B and reviews of lane B's commits.
- [Lane B log](lane-b-log.md): requests to lane A, reviews of lane A's commits and replies.
- [Lane B implementation log](lane-b-implementation-log.md): product objective, implemented slices, verification boundaries and useful client/transport observations.
- [T1 build](t1-build.md): prepare one identified Apple Silicon artifact; publication and first-run download remain open for the three-Mac test.
- [Client qualification harness](client-qualification.md): isolated four-client scripted-provider checks, policy distinctions and recovery evidence boundaries.
- [Transport probe](transport-probe.md): runnable direct, relay-only and automatic transport qualification, local process checks and two-machine instructions.
- [Implementation plan review](implementation-plan-review.md): adversarial review of the proposed plan with prioritized objections, strengths and pre-implementation gates.
- [Second review response](implementation-plan-review-response.md): adopted plan refinements, qualified simplifications and claims not adopted.
- [Release evidence ledger](release-evidence.md): October 4 gate status and partial component evidence; complete release gates remain open.
- [First contact](first-contact.md): proposed first-session journey from one pasted prompt, harness routing, readiness and approval states, and the Polaris handoff; lists current status.
- [Lane C log](lane-c-log.md): lane C's requests to lanes A and B and the Polaris work, and its source reviews.
- [ecdsa.fail swarm demonstration proposal](ecdsa-fail-swarm-proposal.md): proposed roles, records, evaluation cascade and lanes for agents working the ecdsa.fail circuit benchmark as one goal; substrate tiers, run plan and owner decisions. Nothing built or run.
