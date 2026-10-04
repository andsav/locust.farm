# Bound CLI launcher qualification

Date: 2026-10-04. **Measured:** Codex CLI 0.153.4 and Claude Code 2.1.280
passed the installed launcher workflow on macOS arm64. This qualifies the X3
step in the [last-mile plan](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/last-mile-implementation-plan.md), using
actual native clients and scripted loopback providers. No real model ran.

## Candidate and method

The candidate came from committed source
`4030795cd04d1fc03201e4bcbf8f024df16eda73`, pinned Rust 1.96.1, API 1 and
protocol 1. The [release builder](../scripts/build_release.py) compiled an
isolated archived source snapshot. The initial fresh registry fetch was stopped
because it was slow; the successful build copied the local registry index and
dependency archives into the isolated Cargo home and compiled offline with the
unchanged lockfile. No existing compiled target was reused.

- Executable SHA-256: `770a897378023d05894a0f12d6bd7cc145e3fc8d4e3d3d331d125040fdade132`.
- Manifest SHA-256: `8b41834abc54f5f5af7a20eab62be83232fa5c09ad4dbd28b106c6ccca48252d`.
- Archive SHA-256: `59b4bf23fb140719a7ac14c64941910df04e82ace02af0b4adfb4bf28072dcfb`.

Archive, manifest and each extracted payload hash were independently checked
before running the candidate. The locally built executable was the explicitly
trusted bootstrap. Disposable signatures test installation mechanics, not
production publisher identity.

The [installed-client harness](../scripts/check_installed_clients.py) retained
its [existing campaign method](installed-client-qualification.md). Each client
used a fresh private profile, real installed daemon, fixed enrolled agent and
session, synthetic Git workspace, dummy provider authentication and an OS
external-network guard. Setup generated the launcher and installed skill.
The native workspace driver received only `[launcher, "--json"]`; it did not
receive a harness-constructed executable/home/credential/session prefix.

## Observations

| Check | Codex | Claude Code |
|---|---|---|
| Assertions | 22 pass, 4 not run, 0 fail | 21 pass, 5 not run, 0 fail |
| Skill description in first provider request | Pass | Pass |
| Exact installed skill body read by native tool | Pass in permissive run | Pass in permissive run |
| Default MCP goal read | Pass | Policy denial |
| Default MCP note write | Policy denial | Policy denial |
| Bound launcher snapshot/patch/review/submit/accept/apply | Pass | Pass |
| Acceptance observed before integration | Pass | Pass |
| Unrelated dirty work and original Git HEAD preserved | Pass | Pass |
| Daemon restart and fresh native chat | Pass | Pass |
| Repeat setup unchanged; removal preserves unrelated settings | Pass | Pass |
| Natural process cleanup and private profile removal | Pass | Pass |

The successful workspace runs explicitly used Codex's `-a never -s
danger-full-access` and Claude's `--dangerously-skip-permissions`. Default
denials remain `not_run`, not successful writes. Setup does not change grants
or client approval policy. Pi was unavailable for this rerun; its generated
launcher is covered by component tests only.

The [retained evidence](evidence/bound-cli-qualification-2026-10-04.json)
records source/payload/client/harness identities, launcher and installed-skill
hashes, assertion reasons, policy denials, task/contribution identifiers,
initial metadata observations and cleanup results. The signed source skill
and generated installed skill have distinct hashes. Credential bytes, skill
bodies and provider payloads are not retained in this projection.

## Limits and component checks

This demonstrates native execution through setup's launcher. Scripted tool
selection does not demonstrate a real model choosing the skill or following
its instructions. Interactive human approval, production publisher trust,
independent-member collaboration and Linux native clients were not tested.
The launcher is a convenience binding, not same-user credential isolation.

Before the implementation commit, workspace formatting, strict Clippy and
578 Rust tests passed (11 existing ignored tests). The focused installed-client
and workspace-driver Python suites passed all 21 tests. Setup tests cover
quoted paths/arguments, binding overrides, legacy ownership upgrade,
interrupted apply/removal, edited-file refusal and all three client targets.
