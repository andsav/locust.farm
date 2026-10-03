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
| R6 — Real client behavior | M1/M5/M6: default-profile Codex and Claude, CLI/MCP, skill setup, wait/interruption/manual resume and own-account authentication | Partial; gate open | B-C2 (configuration only) |
| R6L — Locust client lifecycle | M1/M3/M5/M6: Locust-owned launch/configuration, readiness, attempt/session binding, launch/restart recovery, cancellation and each claimed hook/wake capability | Partial; gate open | B-C2 (configuration only) |
| R7 — Packaging and platform | M5: claimed macOS arm64/Linux x86_64 installs; fresh-DB write/read/restart; repeat install, migration, service and uninstall, including owned client-configuration cleanup | Not run | None |
| R8 — Real collaboration | M6: two people/machines, mixed clients, independent accounts, no shared forge, actual artifact and restart/reconnect | Not run | None |
| R9 — Network and operations | M0/M2: direct/relayed paths, named relay/discovery operators, alternate relay, no-overlap status and redacted diagnostics | Partial; gate open | B-C1 (loopback component) |
| R10 — Release integrity | M5/M6: owner-selected license, signing custody, manifest/withdrawal handling, exact tested artifact and public download verification | Not run | None |

## Partial component evidence — October 3

**B-C1, relevant to R9:** commit `48cf79b` implements authenticated framed Iroh links. On macOS arm64 / Rust 1.96.1, 11 transport tests passed, including two loopback endpoints, authenticated identities, framing admission and interrupted I/O. This is local component evidence; separate machines/networks, public/alternate relays, route diagnostics, membership and reconnect/reconciliation are not qualified. Lane A cross-review is requested as B-4 in the [lane B log](lane-b-log.md).

**B-C2, relevant to R6/R6L:** commit `7d1a207` implements configuration generation and a test-only stdio MCP socket probe. Six configuration tests, nine MCP-fixture tests and one explicit installed-Codex configuration test passed. The real executable was Codex 0.153.4 with a disposable profile; no provider calls, credentials or user-profile changes were involved. This does not prove real-model tool use, daemon authentication, default approvals, launch/session recovery or wake. Claude configuration has component coverage only. Lane A cross-review is pending under B-4.

Exact reproduction commands, observations and remaining boundaries are in the [implementation log](lane-b-implementation-log.md). Workspace formatting, Clippy and tests also passed on the combined tree. These source/component checks are not packaged-candidate checks; no release artifact hash or public-download assertion exists yet, and the gate table remains open.

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

## Client lifecycle qualification

The accepted direction is to implement client integration in Locust's Rust application, using [hcom's patterns](../research/hcom-dissection.md) as references. There is no hcom runtime, fork or source-transplant adoption gate. R6L qualifies Locust's own implementation and must record, per supported client/profile:

- Effective configuration and permission preservation, required authentication/approval actions, and Locust skill/MCP readiness after launch.
- Persisted launch intent and attempt/session binding; crash after spawn, ambiguous launch recovery, resume/fork/rebind, and stale claim rejection.
- Notification loss/duplication and exit before claim, with pending work recovered from Locust task state.
- Cancellation requested, observed process/descendant outcome and any uncertain effects; no launch triggered directly by peer assignment.
- Active-session delivery, idle wake and explicit resume as separate capabilities. An unqualified optional wake path is reported as unsupported; the common manual-resume path still requires R6 evidence.
- Exact packaged Locust artifact and client versions for shipped claims. Independently written scripted-provider tests, default-profile checks and real-model task runs are separate evidence records.

The earlier hcom Codex/Claude runs only identify useful scenarios. In particular, the retained intermittent Claude approval-resume failure motivates testing delivery around approval prompts and active user input; neither that failure nor a later passing repeat establishes Locust behavior. See the [original evidence](../research/evidence/hcom-validation.md).

## Decisions still requiring an owner

- Repository license and distribution/signing-key custody: repository owner.
- Pinned transport/blob/crypto implementation and default relay operator: assigned implementation/integration owner, with M0 evidence.
- Final release go/no-go or explicit scope/support revision: repository owner after reviewing the candidate evidence.

The first two rows assign decisions rather than resolving them. Do not invent a license, configure services or publish artifacts merely to complete this planning register.
