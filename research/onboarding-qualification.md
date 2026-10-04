# Installed onboarding qualification

Date: 2026-10-04. **Measured:** the installed `locust up` and `locust agent add`
routes passed on macOS arm64, using both launchd and an existing foreground
daemon. Each run configured disposable Codex and Claude profiles and passed 14
grouped checks. These are daemon/configuration checks; no native AI client or
model ran. See the [onboarding contract](../docs/onboarding.md).

## Candidate and method

Source commit: `d15e51f9269be988c2f2b54d8eb98f88fa2711d2`, built with pinned Rust
1.96.1, API 1 and protocol 1. The [release builder](../scripts/build_release.py)
compiled a verified Git archive with an isolated target and Cargo home. The
local registry index and dependency archives were copied into that Cargo home
for an offline locked build; no compiled target was reused.

- Executable SHA-256: `d1398e6fafd06e4e3e2cfaba8dd6013683442c679416bbf5665d562502ad2895`.
- Manifest SHA-256: `81aa71ee2eaad0bb197bbe6a903d2baae53c733dd1d24124fe49d46b24217111`.
- Archive SHA-256: `907cdf6e72f5325a90f5a691e4d7c88d6b9800ad8962542c3daa93e4d96420cb`.

The archive, manifest, payload hashes, file sizes and modes were independently
checked before execution. The locally built binary was the explicitly trusted
bootstrap. The [qualification helper](../scripts/check_onboarding.py) created
disposable signing keys, signed the candidate and withdrawal registry, reviewed
installation, and ran the activated executable. It removed the private signer
before daemon startup. Disposable test trust does not establish publisher trust.

Run once for each `SERVICE` value, `launchd` and `none`, with a distinct new output
directory and an absolute path to the inert extracted candidate:

```sh
python3 scripts/check_onboarding.py --bootstrap /BUNDLE/locust --bundle /BUNDLE --output /NEW-OUTPUT --timeout-ms 120000 --service SERVICE
```

The timeout is caller-authored. Every run uses its own private temporary home,
workspace, software prefix and daemon home. Launchd uses a task-specific label
and unit under the synthetic service profile. The `none` run starts its own
foreground daemon before onboarding. No personal client configuration or
provider account is used.

## Observations

Both runs established:

- A selected `--plan` left the full fixture unchanged and created no daemon state.
- The installed executable inferred its prefix without a supplied `--prefix`.
- Codex and Claude received distinct enrolled principals and fixed sessions,
  with no initial work grants. Their credential/session files were owned regular
  files, mode 0600, single-link and 32 bytes long.
- Generated launchers authenticated as their exact agents using only the launcher
  path and `--json status`; the harness supplied no authority flags or protected
  credential/session environment variables.
- Repeated `up` and `agent add` preserved identity/session hashes and daemon PID.
- Repeating `agent add` after a deliberate owner grant preserved that grant and
  did not grant the other agent.
- Unrelated Codex/Claude settings, an unselected client and a separate unselected
  profile remained unchanged.
- Cleanup removed the owned unit or reaped the owned foreground child, observed
  the daemon lock released and socket absent, then deleted the private fixture.

The first launchd stop command returned `unavailable`; subsequent status observed
`stopped`, and reviewed removal succeeded. This nonzero receipt is retained.
The helper does not infer shutdown from the stop command alone. After both runs,
an independent process query found neither daemon PID, and a separate launchctl
query confirmed the exact task label was absent.

The [retained evidence](evidence/onboarding-qualification-2026-10-04.json)
contains candidate/build identities, each run's checks and public principal/session
IDs, credential/session fingerprints, command exit codes and output hashes,
and cleanup observations. It contains no credential bytes, configuration bodies
or provider payloads.

## Component checks and limits

Formatting, strict workspace Clippy and 608 Rust tests passed; 11 existing tests
were ignored. The existing Python suite passed 177 tests and the new qualification
harness suite passed 10 tests. Documentation checks passed. The Rust tests cover
all three generated client layouts, interrupted enrollment/setup checkpoints,
restart recovery, lost enrollment responses, modified or missing secrets,
revocation, profile collisions and preservation of later grants. Harness tests
reject false success and preserve a fixture when cleanup cannot be established.

Linux/systemd and the installed Pi route were not exercised here. Interactive
terminal prompts, fresh native client discovery and real-model selection were
not exercised. Earlier [bound-launcher client evidence](bound-cli-launcher-qualification.md)
has its own candidate and does not establish those observations for this build.
Physical-machine collaboration, production signing custody and public distribution
remain separate release gates. W3 display metadata/renaming, managed-session
metadata and onboarding-specific `doctor` integration remain deferred.
