# Installed-client qualification

## Scope and evidence boundary

This is the tracked protocol for the installed-client campaign. Actual client
results are pending until recorded below. The implementation is
[`check_installed_clients.py`](../scripts/check_installed_clients.py), with
[false-pass and privacy regressions](../scripts/tests/test_installed_clients.py).
The accepted installation interface is described in
[installation and setup](../docs/installation.md).

The campaign uses actual Codex, Claude Code and Pi executables, a real Locust
production daemon and durable SQLite state, a synthetic Git workspace, and a
local scripted provider. A trusted bootstrap verifies a disposable test
signature and payload manifest before activating the candidate. The MCP server
and native workspace commands execute the installed binary. This does not
establish production publisher trust, model competence, independent-account
collaboration, or interactive human approval.

Each client has a fresh private HOME, configuration root, temporary directory
and workspace. Existing client profiles and real provider authentication are
excluded. The caller selects the operation timeout. The macOS network guard
must permit loopback and reject an external raw-IP connection before the client
campaign proceeds; platforms without that guard remain `not_run`.

## Discovery and execution protocol

1. Copy the candidate bundle as inert files. Use the trusted bootstrap to create
   disposable signing material, sign and verify the manifest and withdrawal
   registry, review the installation plan, and activate the verified release.
   Record exact manifest, executable, skill and client executable hashes and the
   client version observed under the private profile.
2. Start the installed production daemon. Explicit fixture provisioning creates
   the authorized local agent and fixed protected session. This is a dedicated
   profile/session binding; a fresh native chat does not create a new Locust
   identity automatically.
3. Configure only the synthetic provider and unrelated baseline settings. Run
   the installed `setup plan` and `setup apply` to install the persistent MCP
   entry and skill. Repeat the reviewed apply and require `changed=false`.
4. Launch a default-policy native process, without MCP argument overlays,
   `--bare`, or skill-disabling flags. Inspect the first provider request before
   issuing any scripted tool call. Exact installed skill-description presence
   establishes metadata discovery; later scripted references cannot satisfy
   that assertion. Retain only booleans, paths and hashes from this inspection.
5. Request a native skill read, a registered MCP goal read, and a note write.
   Require completed native tool receipts, not provider call selection. Match
   the exact skill body transiently, retaining only a digest and match boolean.
   Independently read the durable note event and check its author and text. An
   exact observed default-policy refusal remains an explicit denial with the
   operation `not_run`; an absent result without refusal is a failure.
6. Run a separate fresh process with the client's explicit permissive flags.
   Pi keeps its default policy because this campaign installs no Pi permission
   extension. Claim an authorized task, check its durable generation, write
   progress, and ask the native command tool to execute the existing synthetic
   workspace driver. That driver previews, exports, materializes, edits one
   selected file, creates/reviews/submits/accepts/applies its contribution, and
   records the acceptance-before-integration observation. Independently verify
   task/event/head/binding/file agreement and preservation of unrelated local
   work. The harness does not perform the worker operations in place of a
   native client.
7. Restart the installed daemon, compare persisted state and endpoint, and
   launch a fresh native chat against the existing persistent setup. Require
   both a new native session identifier and a newly committed note.
8. Review and apply setup removal. Require owned skill/entry cleanup and
   preservation of unrelated provider settings or the baseline sentinel.

Full native streams containing skill reads exist only inside the disposable
runtime profile. Retained evidence contains projected completed native
receipts, hashes, policy denials, public identifiers and independent daemon
observations. The profile and its native histories are removed on completion,
including failure. Scripted provider requests retain tool-name projections,
not prompts, skill bodies or authentication material.

## Verification status

- Harness regression tests: seven passed before the first actual campaign.
- Actual Codex, Claude Code and Pi campaign: not yet run against the final bundle.
- Real models and interactive human approval: intentionally not run.

The first source-level prerequisite check found a generated-command mismatch:
setup emitted `locust mcp serve`, while the CLI accepts `locust mcp`. This is a
source finding, not an actual client result. The campaign must use a rebuilt
bundle containing the corrected command before discovery can be qualified.
