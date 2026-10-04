# Installed-client qualification

## Scope and evidence boundary

This records the installed-client campaign completed on 2026-10-04. All
exercised final scenarios passed; policy denials and untested claims remain
explicit below. The implementation is
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

## Verified artifact and client results — 2026-10-04

The final macOS artifact was built from clean source commit
`5bb254d97504209c1ee4277e74c1365c2d8620e0`. The bootstrap was explicitly trusted
through that audited local source/build provenance; using the same bytes as
bootstrap and candidate does not make a candidate signature self-authenticating.
The disposable signer verifies installation mechanics, not publisher identity.

| Artifact | SHA-256 / size |
|---|---|
| Installed Locust executable | `abe1c0271de5c8fdbd8145d35b6b0932233d02eee7b5957fc99fc3211eac8580` / 13,678,896 bytes |
| Signed manifest contents | `3793145c1aa0aa7aae24e8572d4b60683ff8d5205d1ce26202d4d01843c4d8c7` |
| Installed operating skill | `b7a14462adbd345ba925c4b1b44a9009c5b52fdab7215c453be18be32724fe7b` |
| Skill description | `cd7dbf1f7edae51bb8910763480c3db5b45c7ca106cb8681905d9a81a6c835f3` |

| Actual client | Client executable SHA-256 | Final assertions |
|---|---|---|
| Codex CLI 0.153.4 | `b973d440acac501fd2594a43e7ca9ce41e0a65b9dfb28d0d7a7837c99e1261e3` | 21 pass, 4 not run, 0 fail |
| Claude Code 2.1.280 | `387a5c5dcdbb815085edf0baf79591f9d8894efe922bceaf3d75b1b08055229d` | 20 pass, 5 not run, 0 fail |
| Pi 1.0.1 | `e79626f2dd6f94aa45d30f3fa63cd84319a6eefcd150b353cfaf274366926774` | 22 pass, 3 not run, 0 fail |

Pi's executable hash covers its resolved JavaScript CLI entry, not the Node
runtime or every transitive package. Client versions came from guarded native
`--version` invocations. The full report records launch/resolved paths.

All three first provider requests contained the exact installed skill
description and none contained its body. Pi also exposed the exact installed
skill path at that point. Codex and Claude did not expose that full path in
their first request. This verifies metadata discovery independently of the
subsequent scripted native read; it does not show a real model choosing to use
the skill.

| Scenario | Codex | Claude Code | Pi |
|---|---|---|---|
| Native installed skill read | Pass | Pass in explicit permissive run | Pass |
| Default MCP goal read | Completed | Explicit native policy denial | Completed |
| Default MCP note write | Explicit native policy denial | Explicit native policy denial | Completed |
| Separately permissive MCP read/write | Pass | Pass | Pass under unchanged Pi default |
| Authorized claim and durable progress | Pass | Pass | Pass |
| Native preview/export/materialize/edit/contribution/review/submit/accept/apply | Pass | Pass | Pass |
| Accepted before local integration; unrelated dirty file and original Git HEAD retained | Pass | Pass | Pass |
| Installed daemon restart and fresh native chat with new committed note | Pass | Pass | Pass |
| Setup repeat unchanged; reviewed removal preserves unrelated settings | Pass | Pass | Pass |
| Native process cleanup and private profile removal | Pass | Pass | Pass |

Codex's exact observed default write refusal was: “MCP tool call requires
approval, but approval policy is never.” Claude's native `permission_denials`
identified its `Read`, `mcp__locust__locust_goal_status` and
`mcp__locust__locust_note_add` calls. No default denied operation is reported as
executed. Permissive Codex used `-a never -s danger-full-access`; permissive
Claude used `--dangerously-skip-permissions`. These are explicit qualification
opt-ins, not an assertion that the skill or setup changes a client's policy.
Pi has no permission extension in this fixture.

All profiles were removed. No skill body or real authentication material is
retained in the projected reports. Interactive human approval, real models,
production publisher trust and independent-account collaboration remain unrun.

## Managed lifecycle and recovery on the same installed artifact

The existing [managed-client harness](../scripts/check_managed_clients.py) and
[managed-recovery harness](../scripts/check_managed_recovery.py) were also run
against a stable test-signed installation of the exact executable above. Every
exercised client result reports the matching executable hash and an immutable
release path whose directory is the manifest hash above. This independently
links the managed run to the installed candidate. The older managed report's
canned `packaged_install=not_run` label remains unchanged; that harness does not
itself execute the package-install scenario. The separate
[artifact-binding record](evidence/installed-client-artifact-bindings.json)
links the unchanged raw report hashes, runtime identities, installation evidence
and subsequent owned uninstall evidence. After both managed campaigns finished,
the temporary prefix was removed through a reviewed uninstall; status confirmed
no selected release, no modified/unknown retained files, and a retained trust
watermark.

Codex, Claude and Pi each passed all 13 exercised managed lifecycle assertions:
ready state, binding, claim, ordinary pending observation, held wait, natural
interrupt/exit, durable cancellation, daemon restart, exact explicit resume,
cancellation acknowledgment, profile restoration and owned-process cleanup.
The recovery campaign separately passed intentional parent-crash observation,
unknown-state reconciliation, duplicate-launch refusal and fault cleanup for
all three. Codex and Claude also passed observed default blocking. Pi default
blocking was explicitly `not_run` because its default policy allowed the write.
Intentional fault cleanup is not counted as natural cleanup.

Droid was omitted from these final installed-artifact refreshes by priority;
its earlier results are not silently promoted to this exact artifact. Managed
Claude uses the older harness's explicit `--bare` scenario, so automatic skill
discovery is established by the separate persistent setup campaign above,
which used no `--bare`, MCP override, or skill-disabling flag.

## Retained failures, corrections and reproducibility

Before native qualification, source review found setup generating
`locust mcp serve` although the actual CLI accepts `locust mcp`. The source fix
and regression through the real CLI parser are included in the qualified
`5bb254d` artifact. Source-level setup tests passed all 13 cases before the
release build.

Initial installed campaigns v1 and v2 completed Claude's native read but failed
the harness's exact-body matcher; all its MCP/workflow/cleanup checks passed.
The matcher incorrectly assumed arrow line numbering and initially stripped
real indentation. A projection-only diagnostic campaign v3 observed the header
prefix codepoints `[54, 9]` (decimal line number followed by a tab), without
retaining body text. The parser now strips either observed tab numbering or
legacy arrow numbering while preserving content indentation. An explicit
regression covers both formats and indented lines. All eight installed-harness
unit tests passed before the v4 run; v4 reran all three actual clients and
returned zero failed assertions under those predicates. This was a harness
correction, not a change to the qualified runtime artifact.

An independent review then strengthened two evidence predicates. Ordinary note
verification now requires the returned event ID, exact `note` kind, text and
author. Progress additionally requires `progress` kind and the exact task
assignment from the durable event body; generation is not present in that
protocol body and is checked separately through claim evidence. Removal now
independently parses the remaining JSON/TOML and requires the `locust` MCP key
to be absent, together with no ownership record, skill or pending journal.
Earlier ownership status alone could not prove entry removal. New regressions
reject wrong event kind/assignment/text/author/ID, leftover MCP entries, pending
journals and dangling skill links. All ten focused tests passed. The final v5
campaign reran every actual client with these stronger predicates and again
returned zero failed assertions; the result tables above refer to v5. No raw
v1–v4 report was rewritten.

The ignored receipt directories remain available locally. The tracked
[qualification verdicts](evidence/installed-client-qualification-2026-10-04.json)
retain every final assertion verdict, native session and public task/event
identifier, artifact identities, receipt hashes and cleanup observations.
These findings do not depend solely on disposable output; the projection
excludes skill bodies, prompts, authentication, provider payloads and tool
arguments.

| Local report under `output/` | Report SHA-256 |
|---|---|
| `installed-clients-final-v1/report.json` | `fb9c065ca7d767ceb42da16069a41111799f8502b77c0bbb64db345693946558` |
| `installed-clients-final-v2/report.json` | `14da7b2720bc9376e4909d2982e7763c5b6b22f804b609f89ea454adc6684425` |
| `installed-clients-claude-diagnostic-v3/report.json` | `2229fcef33585b64b54b1f84aac749306967dc70f9888d7dd2eeb97d13ad250d` |
| `installed-clients-final-v4/report.json` | `e09b4767df556187e82fa1981b7dbc59d7a593dbcd836258fec4d67a07f3d746` |
| `installed-clients-final-v5/report.json` | `f3e2f926646c5d6a4f0510567b235ed3854f1167d27451c38c045aa0b4fd8e0b` |
| `managed-clients-installed-final/report.json` | `bf4cc011ee2425c4b8738da26de1822d8c0bee3d2e7fb48b025e797cb89b1932` |
| `managed-recovery-installed-final/report.json` | `41b2da7f05ba0cdb2a1cf164fd12d6a32b7ee28e52c0f19f7f1ccc35a19ca15d` |

The persistent invocation was:

```sh
python3 scripts/check_installed_clients.py \
  --bootstrap /Users/andrei/Projects26/locust/output/final-bundle/locust \
  --bundle /Users/andrei/Projects26/locust/output/final-bundle \
  --output /Users/andrei/Projects26/locust/output/installed-clients-final-v5 \
  --timeout-ms 60000 \
  --codex /opt/homebrew/bin/codex \
  --claude-code /Users/andrei/.local/bin/claude \
  --pi /Users/andrei/Projects26/locust/output/client-tools/pi-1.0.1/node_modules/.bin/pi
```

Managed invocations used the same client arguments and timeout, substituting
`check_managed_clients.py` or `check_managed_recovery.py`, their output paths
above, and `--locust` pointing to
`output/managed-installed-candidate/software/current/locust` as an absolute
path. The final report records each harness source hash, while the managed
reports record exact installed executable identity.

## Start prompt and published Codex/Claude recheck (2026-10-04)

The [entry prompt](../docs/first-contact.md) previously sent agents to an
HTTP-401 preview page and told them to report capabilities and stop even though
software had been published. The replacement names public download instructions,
authorizes reviewed setup in one request, updates an existing owned software
prefix, preserves identity/data and separately verifies the running daemon.
Native qualification is a reported boundary rather than an installation gate.
The installed command help selects released adapters; existing service/enrollment
commands provide a portable CLI route when newer adapters have not been released.

A new disposable-prefix run of the public HTTPS bootstrap passed read-only plan,
first installation and repeat installation. Both returned the published CLI
`0.1.0 (cd65921d8a0f) api 4 protocol 4`, with binary SHA-256
`e9729960ddd3d3944b8b8b82ecd6bcaf3e86653479ce1a07fdcc6b0389680884`.
The published manifest was
`f8306c552c362c8d47918159370eeb11ec31c451e5281536d2daef1baf09d0cc`.
This public bootstrap check exercised publisher/package verification; it did not
start a daemon or configure a normal user profile.

The installed-client campaign was rerun against the same published executable
bytes in separate disposable profiles, using its explicit test-signing route:

| Native client | Observed version | Passed assertions | Explicitly unrun |
|---|---|---|---|
| Codex | `0.153.4` | 25 | Default write approval, interactive approval, real model, production release trust |
| Claude Code | `2.1.280` | 24 | Default-policy read/write, interactive approval, real model, production release trust |

Both passed persistent setup, idempotent retry, no initial work grants,
authenticated selected-profile diagnostics, bound CLI use, native skill metadata
and body discovery, a registered MCP roundtrip, daemon restart, fresh native
client recovery, owned removal and preservation of unrelated settings. The
campaign also exercised its synthetic contribution workflow under separately
provisioned fixture grants. Private runtime profiles were removed after the run.
Pi and Droid native execution were not run in this focused recheck.

These checks use scripted loopback providers; they do not establish that a real
model follows the exact copied install prompt. The new Droid and portable-shell
onboarding adapters are source/component tested, not published binary additions.
The local production page's copy button was checked in Chromium: clipboard text
matched the displayed prompt, and mobile wrapping was checked at 390px. Live
website publication and real-account onboarding remain separate acceptance steps.

Formatting, strict workspace Clippy and all workspace tests first passed in a
clean source snapshot excluding concurrent farm work. After the separate farm
and typography changes were committed, the combined checkout based on `b916864`
passed these checks again: 743 Rust tests passed, with 14 explicitly ignored;
site lint, Svelte checking, all 186 site tests, production build and Markdown
checks passed. The combined production page also passed the clipboard equality,
390px wrapping and zero browser console error checks. No real user's agent
profile or existing daemon state was modified by these disposable qualification
runs.
