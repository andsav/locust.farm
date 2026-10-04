# Formation implementation status

Updated 2026-10-04. This records execution of the frozen
[implementation plan](formations-implementation-plan.md); it does
not change its scope. The current runtime is API 3 / protocol 3 (store schema 3). Signed invitation
previews and local context acknowledgments change the format; older homes are
refused without migration. Qualification records below retain the exact earlier
artifacts they exercised and do not qualify these additions. The integrated
runtime and exported contract are committed at `1b81bef7a3caafa219f5a4096a01b3a49d505c56`,
following the initial engine cutover at `c88e3bc`. The exact native candidate
`0a295cdabe6a878cc733c791ca73863933cfa45a` includes subsequent setup/package
cleanup and the matching manual. It is locally qualified within the boundaries
below, not a published or fully qualified release.

## Current package status

| Package | Status | Evidence and remaining boundary |
| --- | --- | --- |
| O0: signed semantics, models, baseline | Verified within stated model/test scope | [Concrete contract](formations-semantics.md), frozen vectors, adversarial replay tests and 54 current finite-model cases; bounded evidence is not an unbounded implementation proof |
| O1: authoring contract | Verified locally | Six presets, normalization, validator, explanations and generated schemas; [authoring tests](../crates/locust-core/src/organization/tests.rs), [installed CLI discovery](../crates/locust/tests/formations.rs) |
| O2: catalog and current persistence | Verified locally | Private drafts, source/presentation CAS, immutable publication, direct schema initialization and unsupported-format refusal; [catalog tests](../crates/locust-core/src/organization/catalog/tests.rs), [store tests](../crates/locust-store/src/tests.rs) |
| O3: governance and work separation | Verified in deterministic and local daemon tests | One shared evaluator at authoring, receive and replay; [goal tests](../crates/locust-core/src/goal/tests.rs), [replica tests](../crates/locust-core/src/node/replica_tests.rs); relay-free local mDNS qualification remains failed on this host |
| O4: contributions and attempts | Verified locally | Taskless findings, independent attempts, explicit offers, cancellation and local generation fencing; [API acceptance](../crates/locust-core/tests/organizations.rs) |
| O5: completion and scoped decisions | Verified in deterministic tests | Exact distinct eligible evidence, competing approved candidates, scope-specific selection and retained proof; [goal tests](../crates/locust-core/src/goal/tests.rs), [read projection regression](../crates/locust-core/src/node/tests/content.rs) |
| O6: composition and daemon flow | Verified in deterministic and local API tests | Parent delegation cannot widen; children retain pinned rules. Durable event/outbox/inbox/receipt commits, retries and withdrawal pass fault tests. Separate-goal export preserves membership and parent review. [Delegation](../crates/locust-core/src/goal/delegation.rs), [delivery](../crates/locust-core/src/node/delivery.rs), [peer tests](../crates/locust-core/src/node/tests/delivery.rs), [subgroup evidence](../research/subgroup-qualification.md) |
| O7: CLI, MCP and agent guidance | Verified installed discovery; real-model qualification blocked | Exact installed CLI/MCP exports, six examples and offline authoring material pass. Three supported persistent native setups discover skill/tools and exercise work; natural-language four-client campaigns remain O11. [Native evidence](../research/organization-native-qualification.md) |
| O8: workspace and managed clients | Implemented; native qualification partial | Exact candidate passes ten installation cases, three supported persistent client workflows, all four managed lifecycle/recovery runs, selected and owner-chosen application, and packaged offline manual checks. Droid native Execute prevents its full workspace flow; test signing does not establish publisher custody. [Exact evidence](../research/organization-native-qualification.md) |
| O9: full public manual | Implemented and verified locally | Eighteen articles cover standalone use and Polaris; generated references, six formation examples and four executable Markdown tutorials pass. The production build verifies 88 routes and ten exact assets. Publication remains O12 |
| O10: Polaris | Verified in isolated signed native package | Merak `aaaf62ae3` pins Locust `1b81bef`; `b0fb5d9d5` records the packaged native create/edit/explain/publish/goal/restart/revocation journey against Locust `0a295cd`. QA identity differs from the canonical app; broader Merak gates and remote dependency publication remain open. [Native evidence](guide/polaris.md) |
| O11: qualification | Partial; remaining blockers recorded | 624 Rust tests pass with 12 explicit ignores; 195 Python tests, 54 finite-model cases, four installed tutorials, same-host default-network T1 and six operational cases pass. Native package and isolated Polaris evidence are identified; Droid workspace, local-only mDNS, physical-machine and real-provider gates remain open |
| O12: release | Local candidate prepared; publication blocked | Exact native software/skill/manual candidate, test signing and installation rehearsal are complete. Production signing custody, artifact origin, continuity policy, publication authorization and independent public download/live journey remain required |
| O13: exclusive reservations | Deferred | Not selected for this delivery; local attempt claims are not distributed reservations |

## Enforced boundaries

- [Goal evaluation](../crates/locust-core/src/goal/fold.rs) uses exact signed
  contexts, member tenures, cutoffs and positive evidence. Missing proofs remain
  pending; a discovered fork can retract ordinary standing.
- [Scoped selection projection](../crates/locust-core/src/goal/projection.rs)
  retains only the exact accepted proof branch in its decision scope. It does not
  make unrelated forked work effective or eligible to start.
- [The commit path](../crates/locust-core/src/node/commit.rs) persists event,
  feed, recipient delivery records, local claim and request receipt atomically.
  Failed commits stop further writes until restart. Revision exhaustion is refused
  before changing memory.
- [Goal-scoped definition loading](../crates/locust-core/src/node/definitions.rs)
  checks signed sealed-object hash, size, epoch, decryption and semantic identity;
  a private catalog entry is not a substitute for the referenced goal object.
- [Local access](../crates/locust-core/src/node/access.rs) keeps author-only
  credentials, read-only viewers, replicated work eligibility and owner execution
  grants separate. A template cannot grant filesystem or provider permissions.

## Verification at the engine commit

`cargo fmt --all --check`, workspace clippy with `-D warnings`, and
`cargo test --locked --workspace` passed: 601 tests passed and 12 tests were
explicitly ignored. The ignored set includes extended campaigns and tests needing
external networking or a platform interposer; it is not a claim that those passed.
The six formation exports and documentation checks passed.

The generated public runtime reference at `e95e277` passed site lint, Svelte checks
(zero errors/warnings), 37 tests and production prerender checks. This proves a
local build, not a deployed website.

The [paired performance measurement](../research/organization-protocol2-performance.md)
records a remaining healthy replay/batch regression after removing repeated proof
work. Its changed authorization workload and host contention are explicit. There
is no production capacity or general speed claim.

## Current qualification failures and network boundary

On this host, the isolated `locust-net` test
`mdns_finds_a_peer_by_key_without_contact_hints` failed with `transport: Connect`
after about ten seconds on 2026-10-04. It contains no goal engine. Three-process
local recipes established invitation-hint connections and replicated reviewed
work, then failed to discover a worker-to-worker path with the administrator
offline. Increasing the recipe deadline did not establish that path.

This is a present relay-free local discovery limitation. A later campaign on the
exact `0a295cd` candidate with normal lookup/relay defaults passed all three-node
collaboration and six operational cases. Worker-to-worker relay paths were
observed while the administrator was offline; the resolving discovery service
was not identified. Durable partial transfer, dirty apply refusal, cancellation,
withdrawal and offline key rotation were exercised. See the
[network evidence](../research/organization-local-discovery.md). Same-host success
does not qualify physical machines, sleep/wake or public distribution.

Droid's full workspace campaign separately failed when its native Execute child
received SIGKILL before producing a workspace receipt. The identical authored
driver passed directly in an equivalent fresh private profile with the same
network guard. Its cause
remains unresolved; the four-client managed lifecycle passes do not replace that
missing full-workflow proof. [Native results](../research/organization-native-qualification.md)
also retain untested real models, human approvals and production trust.

## Follow-up implementation checks

`f985cf9` adds the offline normalized semantic diff; `b08d797` adds owner-only
local choice of an unselected Open contribution, current-model qualification
recipes and the installed operating skill. Their Rust gates passed with 606 tests
and 12 explicit ignores. The actual production fixture's nine cases passed,
including selected application and unselected taskless application without a
distributed approval/selection. Python helper checks passed 192 tests at that
point, including concurrent documentation/model-runner tests.

The public collaboration, private-authoring and Open-patch tutorials execute
directly from their marked Markdown fences. They use an isolated local daemon
with discovery and relay disabled; assertions inspect persistent state, exact
errors and actual file bytes. Their checker records the binary and recipe hashes.
They do not stand in for native model or multi-machine evidence.

The current finite-model lane is committed at `d08606e`: all 54 registered cases
matched their declared outcomes (19 completed safety cases, 22 reachability
witnesses, 11 deliberate safety mutations and two runner fixtures). See the
[model/property map](../research/tla/organization.md) and
[retained raw evidence](../research/evidence/tla/organization/README.md). Historical
executable models were removed; source-history references preserve prior findings.

The [manual qualification record](../research/documentation-qualification.md)
retains both the historical three-recipe campaign and all four current recipes
passing against the exact installed `0a295cd` candidate, with binary and recipe
hashes. The matching bundled manual contains 40 selected source files.

## Correctness audit and final runtime gates

The audit corrections are committed independently:

- `9d0487f` enforces inherited parent work/decision constraints and rejects
  widening or weaker completion. `ee9ede9` retains child rule pins across future
  default changes and binds attempt starts to authenticated closure positions.
  Concurrent offline starts remain valid; observing Close forbids a new start
  until an eligible Reopen. [Delegation tests](../crates/locust-core/src/goal/tests.rs)
  and [public API tests](../crates/locust-core/tests/organizations.rs) cover these rules.
- `0feae33` verifies different memberships, chosen-byte export and fresh parent
  review in [separate goals](../research/subgroup-qualification.md).
- `1b81bef` commits one effect and its recipient records atomically, records the
  recipient inbox before a transport receipt, and retires retries only after
  durably storing that receipt. Agent acknowledgment and process start remain
  separate. Tests inject failure before and after all three commit boundaries,
  lose receipts, restart both nodes and retract forked authority. Failed nodes
  expose no replica and sign nothing until reopen resolves durable state.

At `1b81bef`, formatting, strict workspace clippy and all workspace tests passed:
**623 passed, zero failed, 12 explicitly ignored**. The separate ignored release
performance harness also passed and records current replay/batch costs against the
historical baseline. Its [final measurements](../research/organization-protocol2-performance.md)
retain the remaining regressions and exact source digests. No latency or capacity
budget is inferred.

The final greenfield cleanup (`4bc417d`, `c731e71`) removes the unused proposal
limit and older setup ownership/journal readers. Unsupported setup formats are
refused before mutation. Its combined gates passed **624 Rust tests with 12
explicit ignores**, strict workspace Clippy, formatting and **193 Python tests**.
The package now includes Apache-2.0 license text inside its hashed manual payload.
`6c93b3f` subsequently removes a stale Python qualification assertion about the
old task-state shape. It verifies the exact effective cancellation and
acknowledgment, principal and current obligation removal; four focused tests and
all 194 Python tests pass. The exact installed candidate then passes all 13
ordinary lifecycle assertions in each of four native clients.
`19ced9e` corrects an operational harness expectation: export alone leaves
local integration unset, so a refused apply must preserve the exact earlier
workspace binding. Six focused tests and all **195 Python tests** pass; the
corrected same-candidate operations campaign passes all six cases.

The final public API export includes typed causal closure and separate durable
receipt/availability fields. Six exported formations validate. Site lint, Svelte
checks (zero errors/warnings), 37 site tests and all 88 prerendered routes passed.
The four exact tutorial fences cover private authoring, collaboration, Open patch
application and separate-goal context export. This is local development evidence;
it does not close physical-machine, provider-account or public-release gates.

## Verification scenario ledger

The implementation commits and package owners above apply to this map. A
`verified` row names its exercised boundary; it does not promote deterministic or
same-host evidence to a physical-machine, real-provider or public-release claim.
O13 was not selected, so V04 is conditional and deferred.

| Scenario | Status | Owner / implementation | Evidence and remaining boundary |
| --- | --- | --- | --- |
| V01: open templates and taskless findings | Verified locally | O1/O4/O7; `c88e3bc`, `b08d797` | [Authoring tests](../crates/locust-core/src/organization/tests.rs), [API journeys](../crates/locust-core/tests/organizations.rs) and [Open patch tutorial](guide/apply.md) |
| V02: scoped coordinator behavior | Verified in deterministic/local tests | O3/O5; `c88e3bc` | [Goal tests](../crates/locust-core/src/goal/tests.rs) include coordinator assignment, exact selection and shared-document evidence; [production workflows](../scripts/client_qualification/production.py) use current primitives |
| V03: independent disconnected attempts | Verified in deterministic replay | O4; `c88e3bc` | [Goal tests](../crates/locust-core/src/goal/tests.rs) preserve attempts/contributions under reverse arrival and restart; physical transport qualification remains V18 |
| V04: exclusive reservations | Deferred, not selected | O13 | Independent attempts and local generation fencing do not claim distributed exclusivity |
| V05: distinct eligible review | Verified in deterministic tests | O5; `c88e3bc` | [Goal tests](../crates/locust-core/src/goal/tests.rs) and [finite models](../research/tla/organization.md) enforce exact subjects, distinct eligible principals, tenure cutoffs and pinned rules |
| V06: competing approvals and scoped selection | Verified in deterministic tests | O5; `c88e3bc` | [Goal tests](../crates/locust-core/src/goal/tests.rs) retain exact accepted proof branches while keeping unrelated scopes separate |
| V07: offline administrator | Verified in evaluator and same-host default-network campaign | O3/O5/O11; `c88e3bc`, candidate `0a295cd` | [Goal tests](../crates/locust-core/src/goal/tests.rs) need no administrator decision for authorized Open work. [Network results](../research/organization-local-discovery.md) verify worker exchange over observed relay routes while the administrator is offline; local-only mDNS still fails |
| V08: membership, forks and missing proofs | Verified in deterministic tests | O3/O5; `c88e3bc` | [Goal tests](../crates/locust-core/src/goal/tests.rs), [replica tests](../crates/locust-core/src/node/replica_tests.rs) and [models](../research/tla/organization.md) |
| V09: composition and durable daemon flow | Verified in deterministic/local peer tests | O6; `9d0487f`, `ee9ede9`, `1b81bef` | [Delegation](../crates/locust-core/src/goal/delegation.rs), [delivery/restart tests](../crates/locust-core/src/node/tests/delivery.rs) and [commit failures](../crates/locust-core/src/node/tests/failure.rs) |
| V10: separate-member explicit export | Verified with a local daemon | O6; `0feae33` | [Subgroup qualification](../research/subgroup-qualification.md) and [executable export tutorial](guide/sharing.md); no topic-level isolation claim |
| V11: shared evaluation and atomic state | Verified in deterministic/local tests | O2–O8; `c88e3bc`, `1b81bef` | [Core API tests](../crates/locust-core/tests/organizations.rs), [failure injection](../crates/locust-core/src/node/tests/failure.rs) and [catalog tests](../crates/locust-core/src/organization/catalog/tests.rs) |
| V12: stale local session | Verified in local API tests | O4/O8; `c88e3bc` | [Independent attempt and ABA takeover tests](../crates/locust-core/tests/organizations.rs); cancellation and fencing do not assert physical process termination |
| V13: private draft and publication races | Verified in local API tests | O2/O7; `c88e3bc` | [Catalog tests](../crates/locust-core/src/organization/catalog/tests.rs) and [private author capability tests](../crates/locust-core/tests/organizations.rs) |
| V14: agent/visual/source round trip | Verified with local native package and typed API caller | O7/O10; Merak `aaaf62ae3`, `b0fb5d9d5` | [Polaris evidence](guide/polaris.md): exact schema parity, native adapter and isolated signed native UI against candidate `0a295cd`; restart and revocation pass. This is an automated API/UI journey, not real-model or canonical public-release qualification |
| V15: fresh/current-only persistence | Verified locally | O2/O8; `c88e3bc`, `c731e71` | [Store format tests](../crates/locust-store/src/tests.rs) and [setup refusal tests](../crates/locust/src/installation/setup/tests.rs) refuse unsupported formats without conversion or mutation |
| V16: reviewed and owner-chosen application | Verified with a local daemon/checkout | O8; `b08d797` | [Production workflows](../scripts/client_qualification/production.py) and [executable application tutorial](guide/apply.md); dirty files, conflicts and idempotent receipts remain explicit |
| V17: installed discovery and four clients | Partial; full Droid workflow and real models blocked | O7/O8/O11; candidate `0a295cd` | [Native evidence](../research/organization-native-qualification.md): three supported persistent setups/full scripted workflows, all four managed lifecycle/recovery cases, offline contracts and bundled manual pass. Droid native Execute fails before its workspace receipt; natural-language/provider campaigns need authorized accounts and spending |
| V18: physical transport and recovery | Blocked on external qualification inputs | O11 | [Current discovery limitation](../research/organization-local-discovery.md); two physical machines, independent accounts and sleep/wake evidence are required |
| V19: manual routes, examples and tutorials | Verified locally and against exact installed candidate | O9/O11; candidate `0a295cd` | Eighteen articles, 88 routes, six examples and four executable fences pass. [Manual evidence](../research/documentation-qualification.md) identifies the exact installed binary, 40-source manual and recipe hashes. No deployment claim |
| V20: independent public downloads | Blocked on release authorization and inputs | O12 | Software and website remain unpublished; signing custody, public origin and continuity policy remain owner decisions |
| V21: superseded executable removal | Verified by source inventory and refusal tests | All implementation owners; `4bc417d`, `c731e71`, `6c93b3f`, `19ced9e` | [Semantics removal inventory](formations-semantics.md), [current setup refusal tests](../crates/locust/src/installation/setup/tests.rs) and [runtime contract](reference/generated/runtime.contract.json); historical evidence stays explicitly historical |
| V22: measured replay and ingestion costs | Verified measurements; regression retained | O0/O3/O5; `10e5872` | [Paired performance record](../research/organization-protocol2-performance.md) contains exact inputs, source digests and remaining workload/host caveats; it supplies no invented capacity budget |
