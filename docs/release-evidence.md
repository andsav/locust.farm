# October 4 release evidence ledger

**Status: component evidence is accumulating; no complete release gate is recorded as passed.** Target: October 4, 2026, at night, America/Los_Angeles. This ledger tracks the [implementation plan](implementation-plan.md); its summary rows do not replace the detailed M0–M6 exit evidence or conformance matrix.

The repository owner decides release go/no-go and changes to required scope or support claims. Agents record failures and remediate them; they cannot waive requirements. The [review response](implementation-plan-review-response.md) explains the changes that introduced this register.

## Gate status

| Gate | Required evidence and plan mapping | Status | Evidence records |
|---|---|---|---|
| R1 — Protocol and identity | M0–M3: byte/signature fixtures, authority, goal scoping, restore/copy and per-goal conflict behavior | Not run | None |
| R2 — Authorization and confidentiality | M1/M2: scoped local API, membership, event/blob admission, encryption/key changes and malformed/member-input controls | Not run | None |
| R3 — Durable state and transfer | M1/M2: commit/crash/replay, pending delivery, retained/resumable blobs and lost acknowledgments | Not run | None |
| R4 — Work ownership and cancellation | M3: claim recovery/takeover, stale fencing, authored policy, cancellation and three-instance outage/ancestry tests | Not run | None |
| R5 — Workspace and integration | M4: reviewed export, safe materialization, no automatic hooks/filters, dirty-work preservation, accepted/integrated distinction | Not run | None |
| R6 — Real client behavior | M1/M5/M6: default-profile Codex, Claude Code, Factory Droid and Pi, CLI/MCP, skill setup, wait/interruption/manual resume and own-account authentication | Partial; gate open | B-C2; B-C5; B-C6 (scripted fixture only) |
| R6L — Locust client lifecycle | M1/M3/M5/M6: all four baseline clients; Locust-owned launch/configuration, readiness, attempt/session binding, launch/restart recovery, cancellation and active-session delivery; hooks where claimed; optional Merak-only wake qualified separately | Partial; gate open | B-C2; B-C5; B-C6 (scripted fixture only) |
| R7 — Packaging and platform | M5: claimed macOS arm64/Linux x86_64 installs and the four-client matrix; fresh-DB write/read/restart; repeat install, migration, service and uninstall, including owned client-configuration cleanup | Partial; gate open | B-C7 (build helper only) |
| R8 — Real collaboration | M6: two people/machines, mixed-client flows covering all four baseline clients, independent accounts, no shared forge, actual artifact and restart/reconnect | Not run | None |
| R9 — Network and operations | M0/M2: direct/relayed paths, named relay/discovery operators, alternate relay, no-overlap status and redacted diagnostics | Partial; gate open | B-C1; B-C3; B-C4 (same-host direct/public relay) |
| R10 — Release integrity | M5/M6: owner-selected license, signing custody, manifest/withdrawal handling, exact tested artifact and public download verification | Not run | None |

## Partial component evidence — October 3

**B-C1, relevant to R9:** commit `48cf79b` implements authenticated framed Iroh links. On macOS arm64 / Rust 1.96.1, 11 transport tests passed, including two loopback endpoints, authenticated identities, framing admission and interrupted I/O. This is local component evidence; separate machines/networks, public/alternate relays, route diagnostics, membership and reconnect/reconciliation are not qualified. Lane A cross-review is requested as B-4 in the [lane B log](lane-b-log.md).

**B-C2, relevant to R6/R6L:** commit `7d1a207` implements configuration generation and a test-only stdio MCP socket probe. Six configuration tests, nine MCP-fixture tests and one explicit installed-Codex configuration test passed. The real executable was Codex 0.153.4 with a disposable profile; no provider calls, credentials or user-profile changes were involved. This does not prove real-model tool use, daemon authentication, default approvals, launch/session recovery or wake. Claude configuration has component coverage only. Lane A cross-review is pending under B-4.

Exact reproduction commands, observations and remaining boundaries are in the [implementation log](lane-b-implementation-log.md). For B-C1/B-C2, workspace formatting, Clippy and tests also passed on the combined tree. These source/component checks are not packaged-candidate checks; no release artifact hash or public-download assertion exists yet, and the gate table remains open.

**B-C3, relevant to R9:** commits `b4daf3f` and `905f31a` add address-free route snapshots, acknowledged stream shutdown and a runnable transport probe with direct/relay/automatic modes. On macOS arm64, all 25 Rust component/example tests, 21 Python tests and five process smoke checks passed. Two same-host processes also completed authenticated exchanges through n0's default relay selection and a custom-only n0 east-region relay; automatic mode with an explicit loopback hint selected direct paths. Both sides' route snapshots, success records, exit codes and identity comparisons were checked. [Retained measurements](../research/evidence/transport-probe-2026-10-03.json) include the local debug binary hash; [findings](../research/iroh-transport-probe.md) explain the initial relay-probe and blocked-output bugs that were fixed. This is not two-machine/separate-network evidence, independent-operator evidence, or a packaged release check. Final workspace-wide checks were blocked by concurrent lane-A workspace code; focused checks passed. Lane A cross-review is requested in B-5.

**B-C4, relevant to R9:** `28dcfdd` corrects lane A findings A-R1–A-R8 and updates the endpoint/framing API for revision 2. Whole-workspace formatting, Clippy and 174 tests passed; 2 explicit installed-client tests were ignored in that run. Transport coverage includes 26 library tests and 11 testkit-enabled probe tests. Five process checks passed. The [new retained transport record](../research/evidence/transport-review-2026-10-03.json) records same-host default-n0, custom-n0-region and automatic-direct runs against the rebuilt probe, with both identities, route kinds and successful exits checked. This does not qualify independent relay operators, peer discovery, daemon replication, three machines or durable state. The [lane B replies](lane-b-log.md) request integration review.

**B-C5, relevant to R6/R6L:** `87f8a42` implements configuration for Codex, Claude Code, Factory Droid and Pi plus protected-path read/write/wait fixtures; `d4dbe7c` uses the shared revision-2 environment constants. The adapter passed 24 component/example tests, formatting and Clippy. Explicit installed configuration tests passed for Codex 0.153.4 and Claude Code 2.1.280. Registration generation never selects a permissive policy or writes owner profiles. Actual readiness and session continuation are covered separately below; real-daemon credentials are not qualified by the dummy fixture proofs.

**B-C6, relevant to R6/R6L:** `d45a3f5` and `0e7b150` implement the [actual-client harness](client-qualification.md). The [corrected retained record](../research/evidence/client-qualification-corrected-2026-10-03.json) records passing read/write, held wait, SIGINT, same-native-session resume and bridge restart for Codex 0.153.4, Claude Code 2.1.280, Factory Droid 0.218.1 and Pi 1.0.1 on macOS arm64. Provider/model responses and Droid's backend session lookup are scripted loopback fixtures; no real account or model was used. Default headless denials remain explicit (Claude read/write, Droid write); the lifecycle runs' permission overrides and Claude `--bare` are recorded, while Pi keeps default policy. The [findings](../research/client-qualification.md) retain the initial Droid failure, independent diagnosis and controlled correction. All 54 Python tests and the latest whole-workspace Rust checks passed. Interactive approval, real daemon/task flow, accounts/models, operating-skill refresh, active-session delivery, wake and packaged installation remain unverified. Cross-review is requested as B-7.

**B-C7, relevant to R7 and T1:** `31ca555` adds the [identified Apple Silicon build helper](t1-build.md). Fifteen helper tests pass. An actual pinned release compile passed; publication returned `version_contract_missing` for the name-only scaffold, so no T1 bundle was produced. This is verified refusal and build-tooling evidence, not daemon readiness, a signed release or three-machine behavior.

## T1 preparation and run status

The first integrated run uses one binary on the owner's three Apple Silicon Macs; the full sequence is in [workstreams](workstreams.md).

| Required record | Current evidence |
|---|---|
| One identified `aarch64-apple-darwin` binary | Build helper implemented/tested; current scaffold refused for missing version/commit identity |
| Published pre-release and first-run fetch/verify/start path | Not implemented or published; owner must instruct publication |
| Matching downloaded version, commit and SHA-256 on all three Macs | Not run; no identified download candidate |
| Three members; observed peer routes | Not run |
| Propose → assign → claim → submit → inspect → accept; third peer observes history | Not run |
| Coordinator offline while other peers exchange notes; catch-up | Not run |
| Restart each daemon and sleeping-laptop reconnect | Not run |


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
| Codex | B-C5/B-C6: 0.153.4 configuration, actual MCP fixture and scripted native resume passed | Not run | Not run |
| Claude Code | B-C5/B-C6: 2.1.280 actual MCP fixture and scripted resume passed; `--bare` and policy opt-ins explicit | Not run | Not run |
| Factory Droid | B-C5/B-C6: 0.218.1 actual MCP fixture and scripted local-session fallback passed | Not run | Not run |
| Pi | B-C5/B-C6: 1.0.1 actual MCP fixture and scripted native resume passed under default policy | Not run | Not run |

The [lane A review](lane-a-log.md) records configuration findings against B-C2; the [lane B replies](lane-b-log.md) map corrections to later commits. The current client rows report scripted fixture behavior only; they do not qualify daemon task flow or Locust-managed lifecycle.

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

- Repository license and distribution/signing-key custody: repository owner.
- Pinned transport/blob/crypto implementation and default relay operator: assigned implementation/integration owner, with M0 evidence.
- Final release go/no-go or explicit scope/support revision: repository owner after reviewing the candidate evidence.

The first two rows assign decisions rather than resolving them. Do not invent a license, configure services or publish artifacts merely to complete this planning register.
