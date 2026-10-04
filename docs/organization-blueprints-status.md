# Organization blueprint implementation status

Updated 2026-10-04. This records execution of the frozen
[implementation plan](organization-blueprints-implementation-plan.md); it does
not change its scope. The current runtime is API 2 / protocol 2, introduced by
`c88e3bc960de79eb990b3185a653bd982e587ed6`. It is a local development candidate,
not a published or fully qualified release.

## Current package status

| Package | Status | Evidence and remaining boundary |
| --- | --- | --- |
| O0: signed semantics, models, baseline | Verified within stated model/test scope | [Concrete contract](organization-blueprints-semantics.md), frozen vectors, adversarial replay tests and 54 current finite-model cases; bounded evidence is not an unbounded implementation proof |
| O1: authoring contract | Verified locally | Six presets, normalization, validator, explanations and generated schemas; [authoring tests](../crates/locust-core/src/organization/tests.rs), [installed CLI discovery](../crates/locust/tests/blueprints.rs) |
| O2: catalog and current persistence | Verified locally | Private drafts, source/presentation CAS, immutable publication, direct schema initialization and unsupported-format refusal; [catalog tests](../crates/locust-core/src/organization/catalog/tests.rs), [store tests](../crates/locust-store/src/tests.rs) |
| O3: governance and work separation | Verified in deterministic and local daemon tests | One shared evaluator at authoring, receive and replay; [goal tests](../crates/locust-core/src/goal/tests.rs), [replica tests](../crates/locust-core/src/node/replica_tests.rs); key-only peer discovery qualification blocked on this host |
| O4: contributions and attempts | Verified locally | Taskless findings, independent attempts, explicit offers, cancellation and local generation fencing; [API acceptance](../crates/locust-core/tests/organizations.rs) |
| O5: completion and scoped decisions | Verified in deterministic tests | Exact distinct eligible evidence, competing approved candidates, scope-specific selection and retained proof; [goal tests](../crates/locust-core/src/goal/tests.rs), [read projection regression](../crates/locust-core/src/node/tests/content.rs) |
| O6: composition and daemon flow | Audit corrections in progress | Materialization, logical effect identity and acknowledgment tests pass. Independent audit found missing parent-rule narrowing/inheritance and explicit durable receipt-driven outbox/inbox delivery; these are being completed before qualification. [Flow evaluator](../crates/locust-core/src/goal/flow.rs), [daemon flow](../crates/locust-core/src/node/flow.rs) |
| O7: CLI, MCP and agent guidance | Implemented; installed-client qualification pending | Current typed CLI/MCP, offline contract/diff, private authoring and updated skill pass; exact installed/client campaigns remain O11 |
| O8: workspace and managed clients | Implemented; package qualification in progress | Exact selected-patch application, owner local choice for unselected Open contributions, attempt/session lifecycle and policy guards pass; manual snapshot packaging and installed current-model checks are being completed |
| O9: full public manual | Standalone articles implemented; Polaris article pending | Versioned site, generated references and current operating articles; three executable Markdown tutorials; site checks and manual snapshot qualification tracked separately |
| O10: Polaris | In progress | Typed native adapter and forms-first library/editor under development in Merak; native/package qualification outstanding; dependency revision has not been published |
| O11: qualification | In progress | Rust gates pass; current Python recipes and formal models in progress; native four-client, physical-machine and authorized provider campaigns outstanding |
| O12: release | Not started | Public hosting, artifact signing/publication and independent download/live journey remain separate authorized release gates |
| O13: exclusive reservations | Deferred | Not selected for this delivery; local attempt claims are not distributed reservations |

## Enforced boundaries

- [Goal evaluation](../crates/locust-core/src/goal/fold.rs) uses exact signed
  contexts, member tenures, cutoffs and positive evidence. Missing proofs remain
  pending; a discovered fork can retract ordinary standing.
- [Scoped selection projection](../crates/locust-core/src/goal/projection.rs)
  retains only the exact accepted proof branch in its decision scope. It does not
  make unrelated forked work effective or eligible to start.
- [The commit path](../crates/locust-core/src/node/commit.rs) persists event,
  feed, local claim and request receipt atomically. Failed commits stop further
  writes until restart. Revision exhaustion is refused before changing memory.
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
The six blueprint exports and documentation checks passed.

The generated public runtime reference at `e95e277` passed site lint, Svelte checks
(zero errors/warnings), 37 tests and production prerender checks. This proves a
local build, not a deployed website.

The [paired performance measurement](../research/organization-protocol2-performance.md)
records a remaining healthy replay/batch regression after removing repeated proof
work. Its changed authorization workload and host contention are explicit. There
is no production capacity or general speed claim.

## Current qualification failure

On this host, the isolated `locust-net` test
`mdns_finds_a_peer_by_key_without_contact_hints` failed with `transport: Connect`
after about ten seconds on 2026-10-04. It contains no goal engine. Three-process
local recipes established invitation-hint connections and replicated reviewed
work, then failed to discover a worker-to-worker path with the administrator
offline. Increasing the recipe deadline did not establish that path.

This is a present local discovery limitation. Deterministic transport simulation
and explicit-address loopback tests cover different boundaries. No system network
or privacy setting was changed, and no physical-machine or public-route success
is inferred. Current-model package and native-client qualification must retain
their exact candidate and environment evidence before their claims are promoted.

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
retains the exact local binary and three recipe hashes and their passing results.
