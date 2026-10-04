# October 4 release evidence ledger

**Status: production-client campaigns and managed-session implementation have local evidence; no complete release gate is recorded as passed.** Original target: October 4, 2026, at night, America/Los_Angeles; publication is deferred by the owner. Operational workflows have retained local results; native macOS packaging and prioritized local installed-client campaigns have passed and precede the final two-Mac, sleep/wake and independent-account acceptance pass, deferred until the owner is available. Droid follow-up is lower priority. This ledger tracks the [implementation plan](implementation-plan.md); its summary rows do not replace detailed M0–M6 exit evidence or the conformance matrix.

The repository owner decides release go/no-go and changes to required scope or support claims. Agents record failures and remediate them; they cannot waive requirements. The [review response](implementation-plan-review-response.md) explains the changes that introduced this register.

## Gate status

| Gate | Required evidence and plan mapping | Status | Evidence records |
|---|---|---|---|
| R1 — Protocol and identity | M0–M3: byte/signature fixtures, authority, goal scoping, restore/copy and per-goal conflict behavior | Partial; gate open | A-C1; A-C3 |
| R2 — Authorization and confidentiality | M1/M2: scoped local API, membership, event/blob admission, encryption/key changes and malformed/member-input controls | Partial; gate open | A-C1; A-C3 |
| R3 — Durable state and transfer | M1/M2: commit/crash/replay, pending delivery, retained/resumable blobs and lost acknowledgments | Partial; gate open | A-C1; A-C3; A-C5/B-C9 (content graph); A-C7/B-C11 (pending/cancellation survives daemon restart); A-C8/B-C12 (interrupted retained-content recovery) |
| R4 — Work ownership and cancellation | M3: claim recovery/takeover, stale fencing, authored policy, cancellation and three-instance outage/ancestry tests | Partial; gate open | A-C1; A-C3; A-C5/B-C9; A-C6/B-C10; A-C7/B-C11 (actual-client claims, explicit cancellation acknowledgment and conservative crash recovery); A-C8/B-C12 (competing-worker and stale-acceptance workflows) |
| R5 — Workspace and integration | M4: reviewed export, safe materialization, no automatic hooks/filters, dirty-work preservation, accepted/integrated distinction | Partial; gate open | A-C1; A-C5/B-C9; A-C6/B-C10 (four real-model mixed-client pairs, preserved dirty work and separately observed acceptance/application); A-C8/B-C12 (resumable patches and conflicting workers) |
| R6 — Real client behavior | M1/M5/M6: default-profile Codex, Claude Code, Factory Droid and Pi, CLI/MCP, skill setup, wait/interruption/manual resume and own-account authentication | Partial; gate open | B-C2; B-C5; B-C8; A-C5/B-C9; A-C6/B-C10 (earlier real-model production flows); A-C10/B-C14 (installed Codex/Claude/Pi discovery and task flow; Droid installed/resume, interactive approval and independent accounts remain open) |
| R6L — Locust client lifecycle | M1/M3/M5/M6: all four baseline clients; Locust-owned launch/configuration, readiness, attempt/session binding, launch/restart recovery, cancellation and active-session delivery; hooks where claimed; optional Merak-only wake qualified separately | Partial; gate open | B-C2; B-C5; B-C8; A-C7/B-C11 (four earlier scripted clients); A-C10/B-C14 (three clients refreshed on installed native artifact) |
| R7 — Packaging and platform | M5: claimed macOS arm64/Linux x86_64 installs and the four-client matrix; fresh-DB write/read/restart; repeat install, migration, service and uninstall, including owned client-configuration cleanup | Partial; gate open | B-C7; A-C2; A-C4 (identified protocol-1 T1 candidate); A-C9/B-C13 (ten native macOS install/upgrade/launchd cases); A-C10/B-C14 (installed client setup/removal) |
| R8 — Real collaboration | M6: two people/machines, mixed-client flows covering all four baseline clients, independent accounts, no shared forge, actual artifact and restart/reconnect | Not run | None |
| R9 — Network and operations | M0/M2: direct/relayed paths, named relay/discovery operators, alternate relay, no-overlap status and redacted diagnostics | Partial; gate open | B-C1; B-C3; B-C4; A-C1; A-C2; A-C3/A-C4 (same-host component/artifact evidence); M1 observation and T1-M2-S1/S2 (mixed-build two-Mac join, note and assignment); A-C8/B-C12 (clock and same-host operational hardening) |
| R10 — Release integrity | M5/M6: owner-selected license, signing custody, manifest/withdrawal handling, exact tested artifact and public download verification | Partial; gate open | A-C2; A-C4; A-C9/B-C13 (explicit test trust and signed withdrawal implementation; production custody/public distribution open) |

## Partial component evidence — October 3

**B-C1, relevant to R9:** commit `48cf79b` implements authenticated framed Iroh links. On macOS arm64 / Rust 1.96.1, 11 transport tests passed, including two loopback endpoints, authenticated identities, framing admission and interrupted I/O. This is local component evidence; separate machines/networks, public/alternate relays, route diagnostics, membership and reconnect/reconciliation are not qualified. Lane A cross-review is requested as B-4 in the [lane B log](lane-b-log.md).

**B-C2, relevant to R6/R6L:** commit `7d1a207` implements configuration generation and a test-only stdio MCP socket probe. Six configuration tests, nine MCP-fixture tests and one explicit installed-Codex configuration test passed. The real executable was Codex 0.153.4 with a disposable profile; no provider calls, credentials or user-profile changes were involved. This does not prove real-model tool use, daemon authentication, default approvals, launch/session recovery or wake. Claude configuration has component coverage only. Lane A cross-review is pending under B-4.

Exact reproduction commands, observations and remaining boundaries are in the [implementation log](lane-b-implementation-log.md). For B-C1/B-C2, workspace formatting, Clippy and tests also passed on the combined tree. These source/component checks are not packaged-candidate checks; no release artifact hash or public-download assertion exists yet, and the gate table remains open.

**B-C3, relevant to R9:** commits `b4daf3f` and `905f31a` add address-free route snapshots, acknowledged stream shutdown and a runnable transport probe with direct/relay/automatic modes. On macOS arm64, all 25 Rust component/example tests, 21 Python tests and five process smoke checks passed. Two same-host processes also completed authenticated exchanges through n0's default relay selection and a custom-only n0 east-region relay; automatic mode with an explicit loopback hint selected direct paths. Both sides' route snapshots, success records, exit codes and identity comparisons were checked. [Retained measurements](../research/evidence/transport-probe-2026-10-03.json) include the local debug binary hash; [findings](../research/iroh-transport-probe.md) explain the initial relay-probe and blocked-output bugs that were fixed. This is not two-machine/separate-network evidence, independent-operator evidence, or a packaged release check. Final workspace-wide checks were blocked by concurrent lane-A workspace code; focused checks passed. Lane A cross-review is requested in B-5.

**B-C4, relevant to R9:** `28dcfdd` corrects lane A findings A-R1–A-R8 and updates the endpoint/framing API for revision 2. Whole-workspace formatting, Clippy and 174 tests passed; 2 explicit installed-client tests were ignored in that run. Transport coverage includes 26 library tests and 11 testkit-enabled probe tests. Five process checks passed. The [new retained transport record](../research/evidence/transport-review-2026-10-03.json) records same-host default-n0, custom-n0-region and automatic-direct runs against the rebuilt probe, with both identities, route kinds and successful exits checked. This does not qualify independent relay operators, peer discovery, daemon replication, three machines or durable state. The [lane B replies](lane-b-log.md) request integration review.

**B-C5, relevant to R6/R6L:** `87f8a42` implements configuration for Codex, Claude Code, Factory Droid and Pi plus protected-path read/write/wait fixtures; `d4dbe7c` uses the shared revision-2 environment constants. The adapter passed 24 component/example tests, formatting and Clippy. Explicit installed configuration tests passed for Codex 0.153.4 and Claude Code 2.1.280. Registration generation never selects a permissive policy or writes owner profiles. Actual readiness and session continuation are covered separately below; real-daemon credentials are not qualified by the dummy fixture proofs.

**B-C6, relevant to R6/R6L (historical; interruption/cleanup superseded by B-C8):** `d45a3f5` and `0e7b150` implement the [actual-client harness](client-qualification.md). The [corrected retained record](../research/evidence/client-qualification-corrected-2026-10-03.json) records passing read/write, held wait, SIGINT, same-native-session resume and bridge restart for Codex 0.153.4, Claude Code 2.1.280, Factory Droid 0.218.1 and Pi 1.0.1 on macOS arm64. Provider/model responses and Droid's backend session lookup are scripted loopback fixtures; no real account or model was used. Default headless denials remain explicit (Claude read/write, Droid write); the lifecycle runs' permission overrides and Claude `--bare` are recorded, while Pi keeps default policy. The [findings](../research/client-qualification.md) retain the initial Droid failure, independent diagnosis and controlled correction. All 54 Python tests and the latest whole-workspace Rust checks passed. Interactive approval, real daemon/task flow, accounts/models, operating-skill refresh, active-session delivery, wake and packaged installation remain unverified. Cross-review was requested as B-7; remediation found that group-wide interruption could mask descendant cleanup failure.

**B-C7, relevant to R7 and T1:** `31ca555` adds the [identified Apple Silicon build helper](t1-build.md). Fifteen helper tests pass. An actual pinned release compile passed; publication returned `version_contract_missing` for the name-only scaffold, so no T1 bundle was produced. This is verified refusal and build-tooling evidence, not daemon readiness, a signed release or three-machine behavior.

**A-C1, relevant to R1–R5/R9 and T1:** `98dbb9c` fixes workspace export ancestry; `885b372` integrates the real core, SQLite daemon, CLI and Iroh peer synchronization. Workspace formatting, strict Clippy and 391 Rust tests pass (five explicit ignores); 63 Python helper tests pass. The [integration findings](../research/t1-integration-2026-10-03.md) retain a complete default-network three-process workflow and its earlier multicast-only failure, with exact debug hashes and event/route evidence. A separate key-only Mainline exchange passed. This is source/component and same-host multiprocess evidence, not public-artifact, three-Mac, sleep/wake, physical-power-loss or real-client qualification. The [lane A log](lane-a-log.md) maps the reviewed fixes and their enforcing tests.

**A-C2, relevant to R7/R9/R10 and T1:** the pinned Apple Silicon release build from `3422c7b51948a409481cf2cd9df1cc3f3a1b4dd1` reports `locust 0.1.0 (3422c7b51948) api 0 protocol 0` and SHA-256 `299aafb3c473d7d1051317a636640dfd8c68a52f0d2fe3317cf685b6d56ffbcf`. After independent identity/checksum verification, this exact artifact passed all 21 CLI workflow checks in 12.53 seconds. The [summary](../research/evidence/t1-release-local-2026-10-03.json) and [redacted transcript](../research/evidence/t1-release-local-2026-10-03.jsonl) retain joining, sealed task completion, coordinator-offline relay exchange, catch-up and restart evidence. All eight daemon generations exited cleanly. This is three processes on one Mac; public download, three Macs, OS sleep/wake, Developer ID signing and notarization remain unqualified. The [candidate record](t1-build.md) has the proposed download/start command.

**B-C8, relevant to R6/R6L:** `0391da8` repairs leader-only interruption, natural observed cleanup and HTTP evidence redaction in the qualification harness. The [remediation rerun](../research/evidence/client-qualification-remediation-2026-10-03.json) uses the same four installed client versions and records passing lifecycle, native-session resume and bridge restart checks across 12 runs; all observed bridges exited naturally and private profiles were removed. Default Claude read/write and Droid write remain denied. Pi's 30.391-second exit is consistent with the fixture timeout, so immediate cancellation is unqualified. Probe/source hashes and permission overrides are explicit. This supersedes B-C6's interruption/cleanup claim, which group-wide signaling could falsely satisfy. These are scripted-provider fixture results; production Locust lifecycle, real accounts/models and unobserved fully detached descendants remain outside the evidence.

**A-C3, relevant to R1–R4/R9 and T1:** `d253a07` implements the [36-finding remediation](../research/t1-remediation.md): local/remote read authority, invitation lifecycle, deterministic committed branches, screening, bounded reconciliation, transfer recovery, durable storage, daemon failure handling and CLI corrections. API/protocol 1 explicitly refuse old peers, tickets and event-bearing homes; preserve old state and use a fresh home. Formatting, strict Clippy and all 439 workspace Rust tests passed (nine explicit ignores); all 76 Python tests passed. Two additional real Iroh one-way regressions passed beyond the production idle interval, and the macOS WAL recovery fault regression passed. Independent cross-review verified the closure mapping and found no remaining actionable defect in the accepted scope. Source/component and syscall-injection evidence do not establish physical power-loss recovery or close any complete release gate. The build helper's `9c01986` provenance repair is covered by 25 focused tests; artifact runtime evidence is recorded separately below.

**A-C4, relevant to R7/R9/R10 and T1:** the identified protocol-1 artifact from `d253a07bf26cef2b59172df16297383fd286e369` reports `locust 0.1.0 (d253a07bf26c) api 1 protocol 1`, SHA-256 `4231754e08f1b4b5fb77ae4e56c6a92c4212e4968659f9b1cea94682d6834f25`. The isolated archived-source build, independent checksum/version/format checks and all 21 local three-process checks passed (24.69 seconds). The [verification](../research/evidence/t1-remediation-verification-2026-10-03.json), [workflow summary](../research/evidence/t1-remediation-local-2026-10-03.json) and [redacted transcript](../research/evidence/t1-remediation-local-2026-10-03.jsonl) retain exact identity, direct/relay observations, task acceptance, coordinator-offline exchange and restarts. This is one physical Mac and an ad hoc signed executable; physical-machine qualification, real-client collaboration, installation and public distribution remain separate. The owner is handling further live testing separately.

**A-C5 / B-C9, relevant to T2 and R3/R4/R5/R6:** `8a7d170` implements the
production stdio MCP bridge, packaged operating-skill source, typed referenced
content replication, and snapshot/contribution preview, export, materialize,
create, review, submit, accept and apply commands. Whole-workspace formatting,
strict Clippy and **483 Rust tests** passed (zero failures, nine explicit
ignores); documentation/link checks and skill validation passed. The tests
include real OS-pipe interruption/output-closure cleanup, nine Node/Driver
content-graph regressions, conflict/recovery and concurrent materialization
checks, and one production Node/MemStore workflow driven by real MCP and CLI
subprocesses. Independent Sol review found and helped close a materialization
race, CLI error-code loss and operating-skill omissions; the
[retained findings](../research/t2-integration-review.md) and
[workflow/verification record](t2-workflow.md) link enforcing code and tests.
This is local source/component integration on macOS arm64, not an identified
release artifact, installed-client real-model collaboration, physical-machine
qualification or Linux execution proof. The owner handles live qualification
separately; a parallel lane can pin this commit for production-client checks.
Managed launch, installation and publication remain outside this checkpoint.

**A-C6 / B-C10, relevant to T2 and R4/R5/R6:** `040da11` implements the
[production client campaign](../research/t2-production-qualification.md) exercises
actual Codex 0.153.4, Claude Code 2.1.280, Droid 0.218.1 and Pi 1.0.1 with scoped
production daemon/MCP sessions. Scripted-provider MCP lifecycle checks passed in
all four; native workspace execution under the external-network guard failed in
Droid and passed in the other three. Separate real-provider pairs completed the
workspace task with every client as coordinator and worker, including native
skill reads, independent acceptance-before-integration checks and preserved
HEAD/unrelated work. Droid's provider-key-only native coordinator continuation
failed; an explicitly fresh session applied the accepted result successfully.
The [scripted record](../research/evidence/t2-production-clients-2026-10-03.json)
and [real-model record](../research/evidence/t2-real-model-qualification-2026-10-03.json)
retain exact differing debug artifacts, clients, public task/session IDs,
permissions and failed attempts. `2ec3f13` fixes unsafe MCP numeric-schema maxima
discovered by the real OpenAI/Droid run. Pi's harness native-session path and
qualification receipt/exit predicates were also corrected. A real-model native
tool exposed its provider key; retained files were scrubbed, but this is not
worker confinement or prevention of provider-visible tool disclosure. These
same-host, same-principal experiments do not qualify independent accounts,
automatic skill discovery, interactive approvals, installation or real networks.

**A-C7 / B-C11, relevant to R3/R4/R6L:** the
[managed launcher](managed-clients.md) implements locally selected foreground
launch, authenticated readiness, exact session/claim binding, native resume,
ordinary-tool pending delivery and explicit cancellation handling. Durable
uncertain launch recovery refuses a duplicate and never signals a historical
PID. The [managed campaign record](../research/evidence/managed-client-qualification-2026-10-03.json)
records thirteen passing normal checks per actual client and deliberate
launcher-crash checks for all four. Default headless blocking was observed in
Codex, Claude and Droid; Pi allows the tested operation. Normal runs had natural
cleanup; Codex, Claude and Droid recovery experiments required forced cleanup
after observations. Pi's observed processes exited after the deliberate fault without
additional forced cleanup. Final normal and recovery campaigns both used debug
artifact SHA-256 `205169864dc78dca8d7c51c484b7d44bafab1cb69b619816bc577344b5c38c51`,
reporting `locust 0.1.0 (040da1187719-dirty) api 1 protocol 1`. Its recorded source
matches the final verified tree, including repeated-interrupt and retained-pipe
fixes. Earlier campaign artifacts and failures remain separate. Formatting,
strict Clippy and all **511 Rust tests** passed (nine explicit ignores); all
**137 Python tests** and documentation checks passed. This evidence uses scripted
providers, one host and isolated profiles. Active hooks, automatic wake,
confinement, physical/account and installed-artifact qualification remain open.

**A-C8 / B-C12, relevant to R3/R4/R5/R9:** `8cfb489` integrates
[network hardening](../research/network-hardening-final.md) and the
[operational campaign](../research/operational-qualification.md). Monotonic retry
scheduling survives wall-clock steps; founding proof/key retrieval precedes bulk
content; asynchronous invitation readiness leaves other daemon work responsive.
The deterministic clock campaign passed 1,000 seeds with 9,674 faults and 403,955
frames. The identified `c74e451` same-host daemon campaign passed six scenarios:
conflicting documents, 12 MiB snapshot and patch recovery after partial receiver
termination, retained-source recovery with the origin offline, competing-worker
stale acceptance and dirty-apply refusal, durable cancellation, withdrawal/leave,
and offline-member rotation. The six cases group some related observations; they
do not represent six hosts. Revocation preserves its explicit offline boundary:
an unaware excluded peer may author old-epoch local work, but remaining members
refuse it and do not give that peer future keys/content. Source integration passed
formatting, strict Clippy and 533 Rust tests with 11 explicit ignores. A later
candidate rerun and resource observations must retain their own artifact identity.

**A-C9 / B-C13, relevant to R7/R10:** `ab1f422`, `6757d75` and `5bb254d`
implement [native candidate packaging](packaging.md), [signed verification and
reviewed installation](installation.md), user-service ownership and persistent
Codex/Claude/Pi setup. Manifest verification precedes execution; signed policy
rejects rollback, same-sequence equivocation, reinstatement and trust-key
replacement. Activation uses reverified private staging and an atomic current
link. Removal preserves data and modified/unowned files. Interrupted client
writes are journaled; cleanup remains possible after software removal. Native
service definitions are checked before label control, and configuration writes
never claim observed client/API readiness. Cross-review corrected special-file
reads, interrupted cleanup, ambiguous systemd ownership and generated MCP argv.
Final macOS source checks passed formatting, strict all-target Clippy and 571
Rust tests (11 explicit ignores). CI declarations have not been run remotely.
Native installer, service, client and emulated Linux campaign results are
separate qualification records; production key custody and publication remain
open. The owner selected the [Apache License 2.0](../LICENSE) on 2026-10-04;
release bundles do not carry its text yet. The [local prompt](install-prompt.md) requires explicitly selected
trust and paths and does not invent a public release origin.

**A-C9 / B-C13 native macOS result:** the exact candidate from
`5bb254d97504209c1ee4277e74c1365c2d8620e0`, executable SHA-256
`abe1c0271de5c8fdbd8145d35b6b0932233d02eee7b5957fc99fc3211eac8580`, passed
all ten [installation cases](../research/installation-qualification.md). This
includes actual native version probes, repeat/stale-plan/signature/policy refusal,
failed startup and retry, same-schema upgrade from `6757d75`, launchd lifecycle,
conservative uninstall and preserved identity/data. The same executable passed
all six [operational workflows](../research/operational-qualification.md),
including partial snapshot/patch recovery from a retained peer while the origin
was offline. Its [identity record](../research/evidence/local-candidate-5bb254d-2026-10-03.json)
separates executable, manifest and unsigned-archive hashes. Disposable test
signing is not production custody; this upgrade did not change the database
schema. Three sampled idle RSS values were 15,745,024 bytes; transfer checkpoints
ranged from 24,788,992 to 58,130,432 bytes. These are measurements under concurrent
local qualification load, not peak-memory or zero-CPU/network claims.

**A-C10 / B-C14, relevant to R4/R5/R6/R6L/R7:** the
[installed-client campaign](../research/installed-client-qualification.md)
qualified actual Codex 0.153.4, Claude Code 2.1.280 and Pi 1.0.1 against that exact
installed macOS executable. Persistent setup alone registered the skill and MCP
server; first provider requests independently showed skill metadata, native tools
read the skill and completed the authorized workspace flow, and fresh clients
worked after daemon restart. Configuration removal was independently parsed and
unrelated settings/dirty files remained. Codex default reads passed and writes
were denied; Claude default reads/writes were denied; Pi allowed both. Deliberate
permissive runs are separate. The final v5 run has zero failed assertions, with
21/20/22 passes and 4/5/3 explicit not-run outcomes respectively. Ten focused
harness tests enforce exact event kind/ID/assignment and actual registration
removal; earlier parser/predicate runs and their hashes remain recorded.

The same installed bytes passed all 13 exercised managed lifecycle assertions
per client. Crash recovery passed four common checks, plus default blocking for
Codex and Claude; Pi's default blocking remains not run because its policy allows
the operation. The [artifact binding](../research/evidence/installed-client-artifact-bindings.json)
links those unchanged managed reports to the verified installation and subsequent
owned uninstall. Scripted providers, private profiles and one Mac remain the
boundary. No real model or interactive human approval ran in these installed
campaigns. Droid was omitted by priority; its prior record and failed cases
remain unchanged. No hooks, automatic wake or worker confinement are qualified.

## T1 preparation and run status

**Owner update, October 3:** start the physical run on the two available Apple Silicon Macs. Publication remains deferred; AirDrop or a shared folder can carry the executable bundle. Each Mac initializes independent state. The [T1 run guide](t1-run.md) defines the first pass and optional third-peer extension. The owner subsequently authorized a mixed-build smoke test when M2 lacked the original bundle and verified matching Rust/build inputs; identical-artifact qualification remains pending.

**M1 preparation, October 3:** the exact `3422c7b` candidate from A-C2 was rechecked for version and SHA-256 on macOS 26.4 arm64, then started with default discovery/relay configuration and a new private state directory. Participant `m1` was enrolled; `status` succeeded and all seven `doctor` checks passed (state directory, socket path, daemon lock, socket connection, authenticated hello, status and credential file). Goal `19a6ceda4b41a80860177835c74626e4a0d1f5b7218214a73ebb92a91b6cf623`, titled `Two Mac T1`, was created, with proposed task `8570524049313563b9b2a3e48637077f0dbabff09f00dcc65e209a4d333f0445`. A one-use invitation was prepared for M2 and retained only in protected local handoff files. At preparation, M1 was left running with one goal member and an unassigned task. This is local preparation, not peer-connectivity, two-Mac or sleep/wake qualification. At that stage, M2's candidate identity remained to be checked; independently built bytes must be recorded as a mixed-build smoke test if used, without closing the identical-artifact requirement below.

**M2 joined, October 3:** after the owner reported joining from the second Mac, M1's authenticated goal view showed both members, no halted state and M2 connected. The latest address-free route snapshot selected a relay path (57.727625 ms RTT at that observation). M1 assigned the proposed task to M2 principal `a979795f5f99fed3a619a8f8d612dcb876685888495dd39c245b54d7056e265f`, producing assignment `50024601853742ae22a7a3b33d3a33bee8e89f265a31fb003a6776c195a285ef`; the board reports `assigned`. The [retained M1 observation](../research/evidence/two-mac-t1-2026-10-03.json) includes both member endpoints, the route and task state. At that observation, M2's exact build/host evidence, decrypted result, acceptance, restart and sleep/wake were pending; M2's subsequent build and readiness records appear below. This observation does not close the two-Mac test or the matching-artifact requirement.

**T1-M2-S1, relevant to R9:** on macOS 26.2 / arm64, M2 built commit `8ef9dbc2bcee86dc5e505cd486f38acf9b3e5c02` with pinned Rust 1.96.1, independently verified version `locust 0.1.0 (8ef9dbc2bcee) api 0 protocol 0` and SHA-256 `f0f719f80a5205a8758be1e0e4c689af3e7595fa9880c0d138c24ffa98650299`, and confirmed an empty Rust/build-input diff against M1's owner-reported original `3422c7b` candidate. Fresh private M2 state, `m2 --manage-goals` enrollment, authenticated status and all seven doctor checks passed. After the owner's explicit mixed-build authorization, M2 joined M1's existing goal and observed both principals, decrypted title `Two Mac T1`, no halted state and a selected relay path (about 57 ms RTT). The expected task remained proposed and unassigned. The [worker findings](../research/t1-m2-smoke-2026-10-03.md) and [redacted results](../research/evidence/t1-m2-smoke-2026-10-03.json) retain exact identities, commands and responses without the invitation or credential bytes. All recorded runtime checks passed. At the initial M2 snapshot, M1-side corroboration, task execution/acceptance, offline catch-up, restarts and OS sleep/wake were pending; this does not qualify identical artifacts or close a release gate.

**T1-M2-S2, relevant to R9:** M2 subsequently received M1's decrypted coordination note and confirmed effective assignment `50024601853742ae22a7a3b33d3a33bee8e89f265a31fb003a6776c195a285ef` to its existing principal. Local execution authorization and protected session creation both exited 0. With the session selected, all eight doctor checks passed. Final status retained M2's principal/endpoint and membership; pending revision 9 listed the assignment in `to_claim`, and the board remained `assigned`, attempt 1, result null. The owner requested evidence-only handoff before claim/submission; neither was attempted and no coordination reply was sent. The [worker findings](../research/t1-m2-smoke-2026-10-03.md) and [raw task-readiness results](../research/evidence/t1-m2-task-2026-10-03.json) preserve the note, commands and actual results without credentials, the session secret or invitation. M2's binary identity and the mixed-build boundary are unchanged; task acceptance and all recovery checks remain pending.

| Required record | Current evidence |
|---|---|
| One identified `aarch64-apple-darwin` binary | Current protocol-1 `5bb254d` candidate includes T2 and passed local native installation, operations and three installed clients (A-C9/A-C10); the older T1 and mixed-build physical records retain their own identities. See [current candidate](packaging.md) |
| Matching candidate identity on two Macs | Pending; M1 and M2 have different commits/hashes with verified matching Rust/build inputs and explicit owner authorization for a smoke test |
| Two members, observed routes and encrypted task completion | M1 and M2 both observe two members, decrypted title, selected relay and assignment; M2 received M1's note and authorized execution/session locally; claim/submission and acceptance pending |
| Offline note authored with coordinator stopped; catch-up on restart | Two-Mac first pass pending; must not be described as exchange between two surviving peers |
| Restart each daemon and OS sleep/wake reconnect | Candidate passes local process restarts; physical-machine restart/reconnect and OS sleep/wake pending |
| Passive third replica and surviving-peer exchange with coordinator offline | Passed with three local processes; optional three-daemon/two-Mac extension or later third-Mac run remains separate and unqualified |
| Published pre-release and first-run fetch/verify/start | Deferred by owner; no host selection or publication required now |

A successful two-Mac run will not close the complete release gates, the four-client baseline or the independent-accounts collaboration requirement. Preserve the original three-process evidence and label every new result with its actual process/host topology.


## Evidence record format

Add one record per actual check; use it from the corresponding gate row. A record contains:

- Gate ID and exact assertion tested.
- Candidate commit and packaged artifact hash, where applicable.
- Evidence level: source review; deterministic/component runtime; multiprocess local; real-client/network; packaged installation; public-artifact verification.
- Environment: OS/architecture, client/version, permission/sandbox configuration, integration opt-ins and authentication mode without credentials. Distinguish an existing session using CLI/MCP from a session launched/managed by Locust.
- Exact command or reproducible interaction sequence, result/exit status and elapsed time when useful.
- Relative link to retained raw evidence, observer and scoped review reference. Redact credentials/private source before tracking evidence; disposable `output/` alone is not sufficient.
- Outstanding failures and unverified boundaries. Keep failed observations when a later rerun passes.

Prior-art tests, reviewer-reported probes and documentation-check success do not satisfy Locust runtime gates. Linux/macOS CI, a packaged install and a public artifact check establish different evidence levels.

## Required client baseline

**Owner decision, October 3:** Codex, Claude Code, Factory Droid and Pi are required first-release clients. Pi is no longer deferred as a participation/lifecycle target. Automatic wake is restricted to Merak for now; the four baseline clients use active sessions and explicit resume. This changes the required scope; it does not turn documentation or prior-art results into verified support.

| Client | Current Locust configuration evidence | Daemon task flow and recovery | Locust-managed lifecycle |
|---|---|---|---|
| Codex | 0.153.4; current installed skill/MCP discovery; default read passed/write denied | Both real-model roles and native resume passed | Scripted normal/recovery passed; refreshed on installed `5bb254d` |
| Claude Code | 2.1.280; persistent discovery without `--bare`; separate managed `--bare` scenario; default read/write denied | Both real-model roles and native resume passed | Scripted normal/recovery passed; refreshed on installed `5bb254d` |
| Factory Droid | 0.218.1; explicit permission opt-in; default write denied | Both roles completed; real native coordinator resume failed, fresh-session apply passed; guarded scripted workspace execution failed | Scripted provider/backend normal/recovery campaign passed; this does not qualify real-account native resume |
| Pi | 1.0.1; current installed skill/MCP discovery and native default policy | Both real-model roles and exact nested-path resume passed | Scripted normal/recovery campaign passed; default-blocking scenario not run |

The [lane A review](lane-a-log.md) records early configuration findings; the
[lane B replies](lane-b-log.md) map their corrections. Current rows use A-C6/B-C10
and A-C7/B-C11, with A-C10/B-C14 updating the three prioritized clients on the
installed candidate. No row closes default interactive approval, independent
accounts or physical networks; Droid installed discovery remains unrun.

Apply the same required scenarios to each client:

1. Install/discover the operating skill and bridge in an isolated default profile; preserve unrelated configuration and local permission policy; authenticate to the daemon with protected credentials.
2. Inspect a goal and its context, assign or claim an exact task attempt, perform a concrete code change, submit evidence/patch, inspect a contribution and complete the review/fix/accept loop. Cover each client as coordinator and worker with another baseline client so a successful same-client run cannot stand in for interoperability.
3. Observe read/write permission behavior, including whether prompts exist, wait behavior, interruption, explicit resume, bridge/daemon restart and durable pending-work recovery without duplicate execution or lost work. Keep blocked authentication/approval and cancellation outcomes explicit.
4. Exercise locally authorized launch, readiness and session binding/recovery independently of unmanaged CLI/MCP access. Record hooks and active-session delivery separately from explicit resume. Automatic wake is outside the current scope for Codex, Claude Code, Factory Droid and Pi; qualify it only for Merak. Closed-session unattended activation remains deferred.

The harness uses actual client binaries and records exact version, OS/architecture, profile, permission and authentication/provider mode. Scripted-provider checks, real-model collaboration and packaged-install checks remain separate evidence levels. If a client cannot use a scripted provider, record that limitation and qualify the corresponding behavior through an available real-client path; do not silently skip it. The early runnable pair does not close R6, R6L or R8 for the four-client baseline.

## Client lifecycle qualification

The accepted direction is to implement client integration in Locust's Rust application, using [hcom's patterns](../research/hcom-dissection.md) as references. There is no hcom runtime, fork or source-transplant adoption gate. R6L qualifies Locust's own implementation and must record, per supported client/profile:

- Effective configuration and permission preservation, required authentication/approval actions, and Locust skill/MCP readiness after launch.
- Persisted launch intent and attempt/session binding; crash after spawn, ambiguous launch recovery, resume/fork/rebind, and stale claim rejection.
- Notification loss/duplication and exit before claim, with pending work recovered from Locust task state.
- Cancellation requested, observed process/descendant outcome and any uncertain effects; no launch triggered directly by peer assignment.
- Active-session delivery and explicit resume for each baseline client. Automatic wake has a separate Merak-only qualification record; no Merak wake result exists yet. An unqualified wake path is unsupported, and this optional Merak capability is not a fifth baseline-client gate.
- Exact packaged Locust artifact and client versions for shipped claims. Independently written scripted-provider tests, default-profile checks and real-model task runs are separate evidence records.

The earlier hcom Codex/Claude runs only identify useful scenarios. In particular, the retained intermittent Claude approval-resume failure motivates testing delivery around approval prompts and active user input; neither that failure nor a later passing repeat establishes Locust behavior. See the [original evidence](../research/evidence/hcom-validation.md).

## Decisions still requiring an owner

- Distribution and signing-key custody: repository owner. The repository license is decided: [Apache License 2.0](../LICENSE), selected by the owner on 2026-10-04.
- Pinned transport/blob/crypto implementation and default relay operator: assigned implementation/integration owner, with M0 evidence.
- Final release go/no-go or explicit scope/support revision: repository owner after reviewing the candidate evidence.

The first two rows assign decisions rather than resolving them. Do not change the license, configure services or publish artifacts merely to complete this planning register.
