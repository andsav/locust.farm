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

- [Formations](formations.md): accepted product direction and agent/Polaris authoring requirements; API 4/protocol 4 runtime and authoring are implemented; current qualification is tracked separately.
- [Formation implementation status](formations-status.md): current package checks, code-enforced boundaries and remaining native, networking, performance and release qualification.
- [Formation implementation plan](formations-implementation-plan.md): phased greenfield runtime replacement, agent authoring, Polaris, qualification and release work; no migrations, backward compatibility or dead code; frozen authorized scope with explicit verification gates.
- [Formation semantics and removal inventory](formations-semantics.md): accepted administration and scoped-authority model, concrete scenario expectations, outstanding signed-proof obligations and source-based replacement inventory.
- [Public documentation implementation plan](public-documentation-plan.md): complete locust.farm manual inventory, versioned human/agent references, site architecture, checked examples, CI and live publication gates; required formation delivery workstream.
- [Formation editor on locust.farm plan](formation-authoring-plan.md): the `/formations` editor that opens with six ways of working, shows the rules as one line of four points with steps as rows under it, checks formations in TypeScript against Locust-generated test cases, and copies a prompt that has the agent check, draft and publish with Locust; implemented, not yet tried with real first-time users.
- [Formation prompt contract](formation-prompt.md): the exact fixed text of the prompt the formation editor copies, its data blocks and integrity values, its limits, and why it does not start a goal.

- [Locust implementation plan](implementation-plan.md): design and current implementation status, the two-Mac first pass, remaining client/workspace work and release acceptance tests.
- [Last-mile implementation plan](last-mile-implementation-plan.md): proposed work packages for distribution, recoverable setup, reliable task context, human permissions, a reviewed and applied change, two-person qualification and consented visualization; dependencies, decisions, tests and complete research coverage.
- [Live farm pages and the farms gallery](swarm-visualization-plan.md): proposed plan for a live swarm page the goal's creator turns on, relayed through the creator's daemon to locust.farm, with per-member consent and a public gallery; mockups only, nothing built.
- [Archived protocol contract, version 1](protocol-v1.md): historical authority, encoding and local API rules; superseded by the current protocol without an old reader.
- [Archived protocol contract, version 0](protocol-v0.md): historical encoding, event, invitation, synchronization and local API rules for interpreting version-0 artifacts.
- [TLA+ formal verification implementation plan](tla-verification-plan.md): current organization, attempt/session and durable-effect models; bounded checks, witnesses, mutation evidence and explicit proof limits.
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
- [Resumable client onboarding](onboarding.md): implemented owner-driven `up` and `agent add`, explicit profile selection, protected identity recovery and separate readiness/permission boundaries.
- [Local installation prompt](install-prompt.md): pasteable template for explicit local trust, reviewed software/service/client setup and separately observed readiness.

## Public reader guides

- [Reader guide index](guide/README.md): canonical public articles and publication boundaries.
- [Locust overview](guide/overview.md): Current source capabilities, public availability and the organization direction.
- [Goals and organizations](guide/concepts.md): The current goal, organization, participant and authority model.
- [Formation authoring](guide/formation-authoring.md): Offline checks, private publication and contextual runtime readiness.
- [Architecture and local boundaries](guide/architecture.md): Daemon, harness, transport and optional Polaris responsibilities.
- [Availability and evidence](guide/status.md): Publication, platform qualification and development capability boundaries.
- [Local installation and onboarding](guide/installation.md): Review trusted local candidates, profile/service choices and separate readiness observations.
- [First collaboration journeys](guide/collaboration.md): Concrete two-participant, invitation, snapshot and contribution journeys under the accepted model.
- [Apply a chosen patch](guide/apply.md): Separate review and selection from deliberate local application and observed checkout results.
- [Completion, review and selection](guide/completion.md): Exact candidate evidence, multiple qualifying outputs and explicitly scoped decision authority.
- [Presets, composition and lifecycle](guide/organization.md): Choose an arrangement and distinguish immutable definitions, contextual bindings and explicit revisions.
- [Harnesses and managed sessions](guide/agents.md): Client discovery, profile/session identity, approvals and lifecycle evidence.
- [Visibility, membership and trust](guide/sharing.md): Goal read boundaries, explicit exports, exact artifacts and removal/retention limits.
- [Services, fresh state and durable flow](guide/operations.md): Separate service readiness, current-format state and daemon-driven materialization/delivery.
- [Offline recovery and conflicts](guide/recovery.md): Interpret missing proof, uncertain outcomes, forks, revisions and cancellation without unsafe inference.
- [Polaris authoring and observation](guide/polaris.md): Optional visual editing, semantic/layout identity and native verification boundaries.
- [Troubleshooting, FAQ and glossary](guide/help.md): Symptom-specific next actions and exact collaboration terminology.
- [Authoring schema and command reference](guide/schema-reference.md): generated fields, offline operations and exact example downloads.
- [Local API, MCP and event reference boundaries](guide/runtime-reference.md): transport, effects, proof and generation boundaries.

- [Local Codex and Claude demo](demo.md): reviewed onboarding, named local participants, explicit permissions, shared findings and exact contribution application.
