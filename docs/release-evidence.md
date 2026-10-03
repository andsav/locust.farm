# October 4 release evidence ledger

**Status: planning register. No Locust runtime or release gate is recorded as passed.** Target: October 4, 2026, at night, America/Los_Angeles. This ledger tracks the [implementation plan](implementation-plan.md); its summary rows do not replace the detailed M0–M6 exit evidence or conformance matrix.

The repository owner decides release go/no-go and changes to required scope or support claims. Agents record failures and remediate them; they cannot waive requirements. The [review response](implementation-plan-review-response.md) explains the changes that introduced this register.

## Gate status

| Gate | Required evidence and plan mapping | Status | Evidence records |
|---|---|---|---|
| R1 — Protocol and identity | M0–M3: byte/signature fixtures, authority, goal scoping, restore/copy and per-goal conflict behavior | Not run | None |
| R2 — Authorization and confidentiality | M1/M2: scoped local API, membership, event/blob admission, encryption/key changes and malformed/member-input controls | Not run | None |
| R3 — Durable state and transfer | M1/M2: commit/crash/replay, pending delivery, retained/resumable blobs and lost acknowledgments | Not run | None |
| R4 — Work ownership and cancellation | M3: claim recovery/takeover, stale fencing, authored policy, cancellation and three-instance outage/ancestry tests | Not run | None |
| R5 — Workspace and integration | M4: reviewed export, safe materialization, no automatic hooks/filters, dirty-work preservation, accepted/integrated distinction | Not run | None |
| R6 — Real client behavior | M1/M5/M6: default-profile Codex and Claude, CLI/MCP, skill setup, wait/interruption/manual resume and own-account authentication | Not run | None |
| R7 — Packaging and platform | M5: claimed macOS arm64/Linux x86_64 installs; fresh-DB write/read/restart; repeat install, migration, service and uninstall | Not run | None |
| R8 — Real collaboration | M6: two people/machines, mixed clients, independent accounts, no shared forge, actual artifact and restart/reconnect | Not run | None |
| R9 — Network and operations | M0/M2: direct/relayed paths, named relay/discovery operators, alternate relay, no-overlap status and redacted diagnostics | Not run | None |
| R10 — Release integrity | M5/M6: owner-selected license, signing custody, manifest/withdrawal handling, exact tested artifact and public download verification | Not run | None |

## Evidence record format

Add one record per actual check; use it from the corresponding gate row. A record contains:

- Gate ID and exact assertion tested.
- Candidate commit and packaged artifact hash, where applicable.
- Evidence level: source review; deterministic/component runtime; multiprocess local; real-client/network; packaged installation; public-artifact verification.
- Environment: OS/architecture, client/version, permission/sandbox configuration, integration opt-ins and authentication mode without credentials.
- Exact command or reproducible interaction sequence, result/exit status and elapsed time when useful.
- Relative link to retained raw evidence, observer and scoped review reference. Redact credentials/private source before tracking evidence; disposable `output/` alone is not sufficient.
- Outstanding failures and unverified boundaries. Keep failed observations when a later rerun passes.

Prior-art tests, reviewer-reported probes and documentation-check success do not satisfy Locust runtime gates. Linux/macOS CI, a packaged install and a public artifact check establish different evidence levels.

## Decisions still requiring an owner

- Repository license and distribution/signing-key custody: repository owner.
- Pinned transport/blob/crypto implementation and default relay operator: assigned implementation/integration owner, with M0 evidence.
- Final release go/no-go or explicit scope/support revision: repository owner after reviewing the candidate evidence.

The first two rows assign decisions rather than resolving them. Do not invent a license, configure services or publish artifacts merely to complete this planning register.
