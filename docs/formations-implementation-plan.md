# Formation delivery scope

The [accepted direction](formations.md) and
[signed semantics](formations-semantics.md) define the organization model.
The development implementation is API 5 / protocol 5. This document keeps the
accepted delivery requirements and remaining qualification work together;
[implementation status](formations-status.md) maps behavior to source and tests.

## Required behavior

- One declarative organization contract, deterministic validator/explainer and
  generated schema shared by offline authoring and the runtime.
- Optional tasks and roles, unattached findings, independent attempts and multiple
  contributions. Open collaboration does not require a coordinator decision.
- Narrow membership/rule administration, separate work eligibility and exact
  scope-local decision authority. Completion evidence and selecting one output
  are different operations.
- Pinned immutable definitions and rules. Task specialization cannot widen parent
  authority or weaken parent completion requirements.
- Explicit local grants and session binding. Receiving work, delivery, agent
  acknowledgment and observed process start remain separate facts.
- Durable configured flow: the daemon materializes authorized transitions and
  stores recipient work with retryable receipts. Agents need not request every
  transition; delivery does not promise that a remote machine wakes.
- Private revisioned drafts, separate presentation revisions, immutable publication
  and deliberate goal creation. Publishing a definition does not launch work.
- Explicit context/snapshot exports and contribution sources. Membership remains
  the read boundary; topics and roles are not private channels.
- Reviewed local patch application with exact artifacts, base/dirty-work protection
  and idempotent receipts. Shared selection never silently changes a checkout.
- Current-format persistence and setup only. Refuse unsupported formats rather
  than retaining migrations, fallback readers or another runtime.

## Package map

| Package | Delivery requirement | Current reference |
| --- | --- | --- |
| O0 | Signed semantics, modeled invariants and reproducible measurements | [Semantics](formations-semantics.md), [formal models](../research/tla/README.md), [performance experiment](../research/performance-cost-pass.md) |
| O1 | Offline contract, validator, normalization, explanation and examples | [Authoring guide](guide/formation-authoring.md), [generated contract](reference/generated/organization.contract.json) |
| O2 | Private drafts, presentation CAS and immutable catalog publication | [Catalog tests](../crates/locust-core/src/organization/catalog/tests.rs) |
| O3 | Separate governance and work evaluation | [Goal evaluator](../crates/locust-core/src/goal/fold.rs) |
| O4 | Independent attempts and taskless contributions | [Engine acceptance tests](../crates/locust-core/tests/organizations.rs) |
| O5 | Exact eligible completion evidence and scoped output selection | [Completion guide](guide/completion.md), [goal tests](../crates/locust-core/src/goal/tests.rs) |
| O6 | Parent attenuation, composition and durable flow | [Delegation](../crates/locust-core/src/goal/delegation.rs), [delivery tests](../crates/locust-core/src/node/tests/delivery.rs) |
| O7 | Agent CLI/MCP/skill discovery and actionable diagnostics | [Agent guide](guide/agents.md), [schema reference](guide/schema-reference.md) |
| O8 | Workspace, managed-session, installation and recovery boundaries | [Application](guide/apply.md), [managed clients](managed-clients.md), [installation](installation.md) |
| O9 | Complete versioned manual, references and checked tutorials | [Coverage contract](public-documentation-plan.md), [manual index](guide/README.md) |
| O10 | Optional visual authoring over the same typed contract | [Visual integration boundary](guide/polaris.md) |
| O11 | Identified source, native-client, provider and network qualification | [Release evidence](release-evidence.md), [client harnesses](client-qualification.md) |
| O12 | Matching signed public software/manual and independently verified downloads | [Published preview](public-preview-release.md), [package format](packaging.md) |
| O13 | Exclusive distributed reservations, if separately accepted | [Current proposal](../research/exclusive-task-claim.md); deferred and unimplemented |

## Qualification work

Deterministic checks cover invalid authority, missing proofs, forks, duplicate
eligible review identities, competing approved candidates, scope-conflicting
selection, stale sessions, private-catalog races, uncertain commits and current
format refusal. The [source map](formations-status.md) identifies those checks.

Each release must separately identify its binary/manual and exercise installation,
native-client discovery, cancellation/recovery, documentation recipes and actual
public fetches. Physical-network recovery, separate owner/accounts, interactive
approval and real-provider behavior need their own evidence. Bounded formal
checks and one-host process tests do not substitute for those campaigns.

A visual integration must preserve unsupported source, semantic/layout identity,
expected revisions and scoped credentials, then qualify actual packaged-native
round trips against a matching Locust API. Locust remains usable independently.

The source is newer than the published API-4 terminal preview. A completed package
check or local website build does not qualify current publication or general
production readiness. Keep those boundaries explicit in
[availability metadata](reference/availability.json).
