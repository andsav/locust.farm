# Organization protocol 2 native package qualification

Verified local candidate on 2026-10-04: source commit
`0a295cdabe6a878cc733c791ca73863933cfa45a`, macOS 26.4 arm64,
Rust 1.96.1, API 2 / protocol 2. This is development qualification,
not a published release or production signing trust.

The exact optimized binary SHA256 is
`b577709eb981544b4bcf5bdcc5efd677a6133d24e474ab274a1fe500215450b3`.
The unsigned archive SHA256 is
`f77bf689e860e63a99126006e85d713d4d843bfa43b0e7704191c13badcaec5d`.
The manifest and 40-source manual identities, native client versions and hashes,
assertion verdicts, recipe hashes and input-report hashes are retained in
[structured evidence](evidence/organization-native-qualification/results.json).
The manual includes the repository license and matches the selected source snapshot.

## Executed checks

[Installation qualification](../scripts/check_installation.py) passed all ten
cases: verified repeat installation, authenticated fresh database/restart,
manual reading without checkout, payload/signature/plan refusal, failed candidate
probe preservation, withdrawal/trust rollback refusal, staged recovery,
uninstall preservation and two native launchd lifecycle/failure cases.
Disposable signing keys were removed. No cross-version upgrade was attempted.

[Installed client qualification](../scripts/check_installed_clients.py) used
actual Codex 0.153.4, Claude Code 2.1.280 and Pi 1.0.1 executables with private
profiles and scripted loopback providers. All executed persistent setup,
idempotence, skill reading, registered MCP, bound CLI, permissive task/workspace/
contribution/selection, dirty-work preservation, daemon restart and removal
assertions passed. Unproved default-policy cases remain `not_run` in the evidence.
Pi used existing package files with Homebrew Node 22.22.0; no client installation
or account configuration was changed.

[Managed recovery](../scripts/check_managed_recovery.py) used the exact installed
candidate through `ProductionDaemon`, actual native Codex, Claude Code, Droid
0.218.1 and Pi processes, and scripted loopback providers. All four passed
intentional parent crash observation, durable unknown recovery, duplicate-launch
refusal and explicit owned fault cleanup. Codex, Claude and Droid proved default
blocking; Pi did not produce a denial, so its blocking assertion is `not_run`.
Droid's recovery result covers managed lifecycle/recovery, not persistent installed setup
or a full contribution workflow. Intentional fault cleanup does not establish
natural cleanup.

All four [executable manual recipes](../scripts/check_documentation.py) passed
against the installed binary: open patch application, private authoring, local
collaboration and separate-goal export. Installed runtime and blueprint contracts
were retrieved with `HOME=/dev/null`; the installed manual was validated without
checkout access. Campaign watchdogs were explicitly 300 seconds to accommodate
previously observed host loader delays; no product execution budget changed.

## Ordinary lifecycle and Droid workflow boundary

[Ordinary managed qualification](../scripts/check_managed_clients.py) passed all
13 executed lifecycle assertions for all four native clients: ready/binding,
claim/pending, held wait/interruption/exit, durable cancellation, daemon restart,
explicit resume, exact effective cancellation acknowledgment, profile restoration
and natural owned cleanup. The initial campaign exposed an obsolete harness
`task.state` assertion after successful resume. Commit `6c93b3f` replaces it with
independently read exact cancellation/acknowledgment context and obligation
removal. Four focused and all 194 Python tests passed; the corrected campaign
passed without modifying candidate bytes.

[Temporary-registration Droid T2](../scripts/check_t2_clients.py) passed
configuration/handshake, scoped authentication, claim/progress, held wait,
interruption/resume, bridge/daemon restart and receipt checks. Native Droid
`Execute` reported that the authored Python workspace driver terminated by `SIGKILL` about
66 ms after invocation. There was no workspace receipt; workspace execution and
client execution failed, and contribution/selection/dirty-preservation assertions
remain `not_run`. Scripted providers reported no errors. The exact command used
the existing Homebrew Python interpreter and synthetic driver/settings paths;
report hashes retain the original temporary path identities without publishing
profile contents. A separately observed repeat retained the same failure.

A narrow diagnostic executed that same authored driver directly against the
installed candidate in a fresh equivalent synthetic profile with the same
loopback-only macOS sandbox. It exited successfully in 579 ms and produced the
workspace receipt. This narrows the failure to native Droid child execution;
it does not distinguish Droid internal policy from host child-launch policy or
qualify Droid's full workflow. No security settings or permissive fallback were
changed.

## Boundaries

No real model calls, interactive human approvals, production publisher trust,
publication, multiple physical machines or sleep/wake qualification occurred.
The [key-only local discovery limitation](organization-local-discovery.md)
remains separate from these local loopback and explicit-route checks.
Full four-client production workflow qualification remains partial because
Droid's native workspace step failed before contribution, and real model
execution was not run.
