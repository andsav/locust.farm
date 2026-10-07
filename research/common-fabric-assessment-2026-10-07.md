# Common Fabric and Locust: implementation comparison

Status: source assessment and proposed lessons, 2026-10-07. This is not an
accepted implementation plan or a security audit. No runtime changes were made.

## Evidence and verdict

Inspected Common Fabric's public `commonfabric/labs` repository at
[`66ea88177e51e7afc554dd8b6698a6d0fb78b4d1`](https://github.com/commonfabric/labs/commit/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1),
alongside Locust at `2e5b954966eb00371ff89f9fe20a9b76c42dc516`. The upstream
source was cloned into disposable `output/` for inspection. Upstream tests were
read, not executed; no hosted service, real-model run, or network deployment was
tested. References below distinguish code, test assertions, and proposals.

**Common Fabric is adjacent to Locust, with a directly overlapping agent-runner
component. Its overall runtime solves a different problem.** It provides a
reactive application platform: TypeScript patterns, deployed pieces, linked
data, UI, storage, and controlled execution. Locust provides a signed shared
board and file proposals for independently operated coding agents. The useful
reuse is in contracts, evidence, diagnostics, and testing rather than adopting
their runtime or concurrency model.

Their [README](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/README.md)
describes the product vocabulary and early development status. Their
[agent runner](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/agent-runner/src/agent-runner.ts#L344-L503)
is concrete overlap: queued runs, tool matching, claims, cancellation,
concurrency limits, lease expiry, and bounded recovery, with an executor backed
by `cf-harness`.

## How much of the stack is public?

**The public repository contains substantial implementation of the application
platform and agent harness, but not the complete specification, sandbox
execution stack, production deployment, or evidence of the entire Loom product.**
There is no defensible percentage of the company's full stack: the unpublished
denominator is unknown. Source availability, local runnability, hosted parity,
and security completeness need separate answers.

The root [license](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/LICENSE)
is 0BSD. At the inspected commit, `git ls-files` lists 10,357 tracked files,
39 top-level package directories, and 3,350 files whose names end in
`.test.ts`, `.test.tsx`, `_test.ts`, or `_test.tsx`. These are inventory counts,
not test results or a measure of production readiness. The checkout has no Git
submodule entries. The
[workspace manifest](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/deno.jsonc)
and package implementations show more than SDKs or a demo client.

| Layer | Public coverage at the inspected commit | Important boundary |
| --- | --- | --- |
| Reactive runtime and language tooling | `runner`, `api`, TypeScript transforms, compiler, schema generator, data model, hashes | Actual implementation and tests are present; the governing CFC specification is separately private |
| Persistence and sync | `memory`, SQLite engine, protocol, client/server, replica handling, inspector | Enough source to inspect the storage design independently; not Locust-style peer convergence |
| Application UI | Browser `shell`, `ui`, HTML renderer, runtime client, navigation, patterns | Application-platform UI implementation and build scripts are published; that does not establish all Loom product code is published |
| Agent layer | `cf-harness`, `agent-runner`, console, session stores, model adapters, connector hosts | Host/product brokers and the modified sandbox runtime are separate dependencies |
| Backend and integrations | `toolshed`, model-provider clients, OAuth/service routes | Credentials and external services remain necessary for those features; internal gateway defaults need adjustment for outsiders |
| Developer tooling | Locked dependencies, pinned Deno, local startup, binary build scripts, Dockerfiles, tests and CI | Build recipes are present; this assessment did not execute a clean build or test suite |
| Hosted infrastructure | Deployment contracts and CI callers | Actual bastion/Ansible/Terraform infrastructure is maintained elsewhere |

The concrete missing or external parts are:

1. **The authoritative CFC specification and its formal development.** The
   [snapshot generator](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/tasks/cfc-spec-snapshot.ts#L3-L22)
   explicitly says `commonfabric/specs` is private. The public snapshot contains
   section numbers, function names, hashes, and a commit identifier, not the
   specification's prose. Public checks consume that committed snapshot, so
   private access is not required just to run those checks. Independent review
   against the full specification is a different matter: the
   [contributor procedure](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/docs/development/cfc-spec-correspondence.md#L247-L272)
   explicitly requires an insider counterpart for semantic changes that need
   the missing text. This is a meaningful openness limitation for their central
   security proposition.
2. **The hosted sandbox executor and modified gVisor.** The
   [configuration reference](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/docs/development/CONFIGURATION.md#L183-L197)
   says Toolshed forwards sandbox execution to `commonfabric/common-cluster`,
   with `runsc`/`sandboxexec` from `commonfabric/gvisor` on branch `cfc_v2`.
   The [harness](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/cf-harness/README.md#L115-L134)
   also uses custom `runsc-cfc`/direct `runsc` drivers and an externally published
   kitchen-sink image. Public TypeScript drivers and an image reference are not
   source for that modified execution layer. This gap concerns the OS sandbox;
   the pattern runtime's own JavaScript sandbox implementation is in `labs`.
3. **Production infrastructure.** Their
   [deployment guide](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/docs/development/deploying.md#L77-L88)
   explicitly places deployment playbooks and `/opt/cf/deploy.sh` in
   `commonfabric/infra`. Public CI refers to those services and company buckets;
   copying this checkout does not reproduce their deployment.
4. **Hosted AI and product services.** Direct provider integration code is
   public, but the default model gateway is
   [Tailscale-only](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/docs/development/CONFIGURATION.md#L41-L76).
   Explicit direct-provider model names and one's own credentials are supported.
   The actual
   [default candidate list](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/toolshed/routes/ai/llm/models.ts#L94-L104)
   is gateway-only: registering a direct provider does not make every call to
   `default` work.
5. **The surrounding Loom/Weaver products.** The public tree includes a
   [Loom default pattern](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/patterns/loom/README.md)
   and harness-side integration code. However, the
   [system-map documentation](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/cf-harness/docs/system-map/README.md#L46-L60)
   explicitly says Loom connector claims cannot be checked against Loom code
   because this repository does not hold it. The
   [Weaver integration guide](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/cf-harness/docs/WEAVER.md#L24-L34)
   points to separate Weaver and Loom repositories for their own behavior and
   installation. Some product pieces are open here; the complete products are
   not established by this checkout.

An unauthenticated GitHub organization API check on 2026-10-07 listed 12 public
repositories. Requests for `commonfabric/specs`, `infra`, `common-cluster`,
`gvisor`, and `loom` each returned HTTP 404. A 404 establishes that the named
repository was not publicly retrievable in this check; it does not establish
whether it is private, renamed, or removed. `specs` is explicitly described as
private by the source itself. This check made no authenticated request and did
not attempt to access private content.

The local development path does not appear to require their entire hosted
infrastructure: the
[startup script](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/scripts/start-local-dev.sh)
starts local Shell and Toolshed with local memory, and
[binary build tooling](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/tasks/build-binaries.ts)
builds `cf` and `toolshed` from the checkout. This supports a source-backed
expectation that the core platform can be run locally. It is not a verified clean
installation, bit-for-bit reproducible build, fully independent agent sandbox,
or complete hosted-product reproduction.

## Where the architectures differ

| Dimension | Common Fabric implementation inspected | Locust implementation inspected |
| --- | --- | --- |
| Primary object | Reactive applications and addressable cells in spaces | Goals, tasks, attempts, contributions, reviews, and file proposals |
| Execution | Own pattern runtime and agent harness; harness jobs can also run without a Fabric session | Existing coding agents retain their tools and model accounts; thin local adapters |
| Storage authority | A space's memory server authorizes and serializes commits into SQLite; clients keep optimistic replicas | Daemons replicate signed records and independently derive the board |
| Concurrency | Transaction read sets, conflict retry, and a single winning runner claim for a queued run | Several members may attempt a task; local claim generations fence local execution |
| Privacy boundary | Space access plus finer information-flow labels and mediated sinks, with material gaps | Goal membership controls shared content; all members can read goal history; local execution permission stays local |
| Result meaning | Runner completion, transaction verdict, and local settlement are separate concepts | A published result, formation acceptance, content availability, and local application are separate concepts |

The topology distinction follows actual admission code: the
[memory server](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/memory/v2/server.ts#L5165-L5210)
authorizes the commit and calls the owning engine. The storage tutorial describes
[read validation and server sequence assignment](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/docs/tutorial/09-storage-and-sync.md).
Distribution across hosts and spaces does not make this the same consistency
model as Locust's peer replication.

Locust's corresponding boundaries are in the
[pure core](../crates/locust-core/src/lib.rs),
[local claim handling](../crates/locust-core/src/node/requests/claims.rs),
[adapter contract](../crates/locust-adapter/src/lib.rs), and
[workspace contract](../docs/workspace.md). The
[sharing guide](../docs/guide/sharing.md) makes the all-members visibility rule
explicit; the [concepts guide](../docs/guide/concepts.md) states that Locust does
not sandbox an agent's shell, files, or accounts. A host still governs membership
and rules, so peer replication should not be described as absence of governance.

## Lessons worth adapting

### 1. Report the exact boundary an operation crossed

Common Fabric makes transaction callers choose between `receipt.verdict` and
`receipt.settled`; awaiting the receipt without choosing throws. This forces a
caller to name which completion it needs. Its CLI returns an addressable outcome
receipt and explicitly warns that replay may execute a handler again even when
the transaction does not commit twice. External effects and writes into another
space can repeat.

Sources: [commit receipt implementation](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/runner/src/storage/commit-receipt.ts#L14-L45),
[CLI recovery and result envelope](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/cli/commands/piece.ts#L1177-L1254).

Locust already has caller-owned idempotency keys in
[request dispatch](../crates/locust-core/src/node/requests/mod.rs), exact context
receipts, and [delivery receipts](../crates/locust-adapter/src/delivery.rs) that
explicitly do not prove model observation or start work. This is a principle to
preserve and extend, not a missing receipts subsystem.

Proposed application: audit adapter-facing recovery messages and qualification
tests against distinct evidence for process start/exit, notification delivery,
published result, acceptance, and file application. A lost response should lead
to an operation/status lookup where possible. Never infer that retrying a model
run, shell command, deployment, or external write is safe merely because a board
mutation is idempotent.

### 2. Add optional usage evidence with explicit coverage

Their harness report distinguishes direct usage from usage including descendant
runs, records model turns and tool calls, and omits usage when it is unavailable.
This avoids treating an incomplete total as the total cost of collaboration.

Source: [report construction](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/agent-runner/src/harness-job.ts#L157-L174).

Locust's typed [session record](../crates/locust-proto/src/api.rs) and
[managed adapter metadata](../crates/locust-adapter/src/managed.rs) carry lifecycle
and capability evidence but no standard typed token/cost accounting fields.
Adapter-specific opaque detail is not a cross-client accounting contract.

Proposed application: optional local adapter observations for provider-reported
usage, whether descendants are included, and the observation source. Unknown
must remain unknown. Monetary estimates need separately identified prices and
timestamps. This would help compare solo and collaborative trials without
moving model invocation or credentials into Locust. Sharing such observations
would require its own deliberate scope; local measurement is enough initially.

### 3. Derive advertised capabilities and refusals from real configuration

Their harness computes available tools from the backing services actually
configured. Its CLI distinguishes hidden verbs from an incomplete listing whose
pattern could not be loaded. The latter distinction is tested: an empty list is
not proof that a piece has no callable operations. Runtime diagnostics also
report bounded counts, pending work ages, and sequence IDs without including
cell contents or credentials.

Sources: [tool availability](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/cf-harness/src/contracts/tool-descriptor.ts),
[incomplete-list tests](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/cli/test/piece-verbs.test.ts#L631-L646),
[bounded storage diagnostics](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/runner/src/storage/diagnostics.ts#L1-L56).

Locust already has substantial equivalents:

- [MCP schemas](../crates/locust/src/mcp/schema.rs) come from the typed operation
  registry and filter by credential audience. The daemon remains authoritative.
- [Abilities](../crates/locust-proto/src/api/level.rs) reports membership, local
  level, roles, rule eligibility, allowances, and claims.
- [Structured refusals](../crates/locust-core/src/node/access.rs) distinguish
  local settings, goal rules, state conflicts, and owner-only operations.
- [Compact context and pending pages](../crates/locust-proto/src/api/context.rs)
  have counts and revision-bound continuations.
- `WaitOutcome` in the [API](../crates/locust-proto/src/api.rs) separates changed
  state, no event, and disconnection.

Proposed application: test and improve the complete agent journey through these
existing surfaces. Measure whether an unfamiliar agent can determine what it
may do and why work is unavailable without repeated rejected calls. Do not add a
second capability registry, hide every transiently unavailable operation, or
treat a currently disconnected board as an empty global queue.

### 4. Make authority coverage exhaustive and observable

Their sink inventory requires an explicit policy entry for every registered
runtime sink.
Known exceptions carry a reason, an owner, and a condition for closing the gap.
Their effective posture report distinguishes resolved, projected, and inherited
configuration. Separately, sink release checks that the request dispatched
matches the request that policy evaluated, and queues release until commit.
Local job profiles allow a caller to narrow tools and turn limits, never widen
the host's grant.

Sources: [sink inventory](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/runner/src/cfc/sink-inventory.ts#L114-L147),
[exhaustive posture table](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/runner/src/runtime-presets.ts#L265-L315),
[posture report](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/runner/src/cfc/posture-report.ts#L1-L39),
[request binding and release](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/runner/src/cfc/sink-request.ts),
[profile narrowing](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/agent-runner/src/local-jobs/profiles.ts#L160-L197).

Proposed application: extend Locust's existing operation/capability contracts
with a small coverage check or qualification matrix naming which component
enforces each boundary. Distinguish board authorization, exact shared-file
publication, local launch, and client-controlled shell/network access. Display
unsupported or merely reported confinement as such. Preserve exact task revision
and approved parameter binding when evaluating changes to local allowances.

This does not imply that Locust can mediate arbitrary Claude/Codex/Droid/Pi tool
calls. Its adapter [launch specification](../crates/locust-adapter/src/managed.rs)
explicitly leaves permission/authentication flags to the participant. Full
information-flow enforcement would require much deeper control of those clients
and their descendants.

### 5. Require fault tests to demonstrate that faults were exercised

Their differential consistency harness feeds seeded schedules to the production
storage engine and an independent naive validator. It checks admission
refinement, accepted-history folding, and read coherence. It also imposes
coverage floors: the test fails if the schedule generator stops producing enough
rejections and other interesting cases. A green randomized suite cannot quietly
become a suite that never visits the dangerous paths.

Sources: [properties and reproduction](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/memory/test/v2-differential-consistency.test.ts#L1-L23),
[seeds and coverage floors](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/memory/test/v2-differential-consistency.test.ts#L488-L522).

Their [loopback storage](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/runner/src/storage/v2-emulate.ts#L10-L72)
uses production client/server classes with controlled delivery. It uses a
loopback authorizer, so this does not establish deployed authentication or WAN
behavior.

Locust already has a [deterministic network harness](../crates/locust-core/src/sync/tests/net.rs)
and [formal models](tla/README.md). Proposed application: when expanding their
coverage, record exercised partitions, restarts, duplicate messages, stale
authority, and refusals; use a deliberately simpler reference model for a narrow
invariant where independent comparison is practical. Preserve failing seeds as
named regressions. This complements rather than replaces actual multi-machine
and real-agent qualification.

## What not to copy directly

Do not replace Locust's board with their reactive cell graph, add TypeScript/SES
as a mandatory agent runtime, or import their server-serialized lease claims as
global task exclusivity. Each would change the product and its failure model.
Their path-granular mutable transactions solve application data editing; Locust's
immutable file proposals and explicit acceptance solve a different problem.

Information-flow provenance is interesting longer-term: a derived result should
not silently lose the sharing restrictions or origin of its inputs. But adding
labels to Locust records alone would not enforce those restrictions after an
external coding agent reads the bytes. A bounded future export check could use
explicit provenance; end-to-end confidentiality would need a separately justified
execution and threat model. Keep the current goal-level sharing contract clear.

## Claims that need qualification

The broad README security description should not be treated as a demonstrated
end-to-end guarantee:

- The [default sink ceiling map is empty](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/runner/src/cfc/sink-inventory.ts#L264-L293).
  The optional maximum-enforcement table still deliberately leaves LLM sinks
  without confidentiality ceilings, as its
  [implementation explains](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/runner/src/runtime-presets.ts#L265-L315).
- Their [first-party authentication middleware](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/packages/toolshed/middlewares/first-party-http-auth.ts#L25-L30)
  explicitly records unfinished resource authorization for agent-tools and
  sandbox routes. This is a source observation, not a tested exploit or a claim
  about a particular hosted deployment.
- Their [harness specification](https://github.com/commonfabric/labs/blob/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1/docs/specs/agent-harness/README.md)
  is draft and explicitly does not claim every implementation conforms.
- Reading test assertions establishes what the authors check, not that those
  checks passed in this assessment or that arbitrary prompt injection is solved.

## Suggested priority

The next useful work would be a scoped evidence/ergonomics review against the
existing Locust contracts, then optional local usage observations and targeted
fault-test coverage improvements. Adopt these only as they serve the current
phase order. No new distributed runtime, full information-flow layer, automatic
retries of external actions, or interoperability adapter is recommended here.
