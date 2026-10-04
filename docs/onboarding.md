# Resumable client onboarding

Status: implemented and component-tested. The installed `up` route passed
[macOS launchd and existing-daemon qualification](../research/onboarding-qualification.md)
with disposable Codex and Claude profiles. This page describes the
[CLI](../crates/locust/src/cli/onboarding.rs),
[enrollment journal](../crates/locust/src/installation/onboarding.rs) and its
[recovery tests](../crates/locust/src/installation/onboarding/tests.rs).
Configuration, authenticated daemon readiness and native model discovery are
separate observations. Earlier [bound-launcher qualification](../research/bound-cli-launcher-qualification.md)
does not qualify this new orchestration route.

## Start with verified software

Complete the reviewed [local installation](installation.md) first. Onboarding
requires an installed, verified, non-withdrawn candidate; it does not download a
release or establish a trust key. The installed executable infers its software
prefix from `current/locust` or `releases/<manifest>/locust`. When using another
trusted executable, select the installed prefix with `--prefix /SOFTWARE/locust`.

Review explicit selections without writing files:

```sh
/SOFTWARE/locust/current/locust --home /DATA/locust up --client codex --client claude --profile-home /PROFILE --workspace /WORKSPACE --plan
```

Repeat without `--plan` in an interactive terminal to review the selections,
service creation/recovery and client setup. The final client setup review uses
the actual protected credential and session files created during enrollment.
For a deliberate unattended application, use the same selections with `--yes`.
`--yes` approves the displayed changes and requires an explicit `--client`.
Noninteractive execution without `--yes` shows a read-only review. `--json`
also disables interactive prompts. `--plan` and `--yes` cannot be combined.

With no selected clients, `up` presents command/profile candidates; an interactive
run asks which clients to configure. Discovery alone selects and edits no
profile. Repeated `--client` flags or a comma-separated list select clients in
one profile home. `agent add` selects one client explicitly:

```sh
/SOFTWARE/locust/current/locust --home /DATA/locust agent add claude --profile-home /PROFILE --workspace /WORKSPACE --plan
```

`agent add` requires the existing daemon to answer; it does not install or start
a service. Repeat interactively or with `--yes` to apply its reviewed onboarding.

## Profiles, service and readiness

The client profile home defaults to `HOME`, the workspace to the current
directory, and daemon data to `--home`, `LOCUST_HOME`, or `HOME/.locust`, in that
order. Profile home and workspace must already be directories. Paths must be
absolute and free of traversal components. The supported standard layouts are:

| Client | Registration under profile home | Skill and launcher directory |
|---|---|---|
| Codex | `.codex/config.toml` | `.agents/skills/locust/` |
| Claude Code | `.claude.json` | `.claude/skills/locust/` |
| Pi | `.pi/agent/mcp.json` | `.pi/agent/skills/locust/` |

When using the default profile home, a custom `CODEX_HOME` or
`PI_CODING_AGENT_DIR` that differs from the supported layout blocks onboarding
for that client. Any nonempty `CLAUDE_CONFIG_DIR` also blocks default-profile
selection: it changes the config-file layout even when it names `HOME/.claude`.
Unset the override or select `--profile-home` explicitly to choose a supported standard
layout. This does not rewrite those environment variables or move custom client
configuration; use the selected profile when starting the client.

`up` defaults to launchd on macOS and systemd on Linux. The service profile home
defaults independently to `HOME`; select `--service-profile-home` to change it.
Logs default to the daemon home's `logs` directory, or an explicit `--log-dir`.
Service writes retain existing ownership, collision and reviewed-plan checks.
An already-running owned service is observed without restarting it. Software
upgrades still require an explicit restart to replace a running daemon's code.

`--service none` uses an existing daemon and creates no service. Without a
selected service, readiness is checked once unless `--wait-ms` requests a wait.
For a managed service, `up` waits for authenticated owner API readiness until it
is ready or the command is interrupted. `--wait-ms N` supplies an optional maximum
readiness wait in milliseconds; `--wait-ms 0` checks once. Waiting only observes
service/API state and never loops service restarts. Manager state alone does not
prove API readiness. Final client readiness checks authenticate with the saved
agent credential and session. Model discovery remains unverified until a fresh
native client chat loads Locust and completes a harmless status roundtrip.

## Identity and recovery

The versioned private journal lives at
`DAEMON_HOME/onboarding/<client-and-profile-hash>/state.json`; its protected
credential and session files are beside it. It reserves one identity for that
client/profile before enrollment. An optional `--name` selects a file-safe local
agent name for one client; otherwise a client kind, generated word and random
suffix are saved once. Retry with the same prefix, daemon home, client, profile,
workspace and name selections. A retry without `--name` reuses the recorded name.

Interruptions preserve those files and stages. Recovery reuses the credential
and checks its enrolled principal instead of duplicating enrollment, including
after a lost enrollment response or a later owner grant change. Missing or
modified recorded secrets, altered owned configuration and profile collisions
stop recovery for inspection. A declined final client setup review leaves the
enrolled identity saved; repeat the same command to review and finish setup.
The setup component retains its own reviewed file transaction journal.

Each selected profile binds one principal and one fixed Locust execution session.
Multiple native chats using that profile share this Locust session; they are not
independently identified executions. Use separate profiles for independent
bindings. [Managed clients](managed-clients.md) have separate native-session and
lifecycle contracts.

Onboarding grants no goal membership, sharing or work permission. It keeps client
approval policies and provider/account credentials outside setup. Owner-only
administration stays outside the bound launcher, which is convenience and
attribution rather than isolation from another process able to read the owner
credential. Software removal preserves daemon identity/data and this onboarding
journal, credentials and sessions. Remove the selected owned client setup and
service before removing software they reference; see [removal](installation.md).

W3 remains partial: separate display metadata and renaming and atomic managed-session
client metadata are deferred. Selected-profile `doctor` integration is implemented. No
end-to-end real-model workflow, physical-machine pass or public-distribution
qualification is implied by these component and local installation tests.

## Inspect a configured profile

Run `locust doctor --client codex` (or `claude`/`pi`) with the same daemon home
and selected `--profile-home`. The protected journal supplies the recorded
software prefix and workspace; explicit overrides must agree. Checks cover
installation signatures, the chosen service, authenticated daemon access, saved
identity, MCP registration, skill, launcher and effective workspace configuration.
Use `--service none` for a foreground daemon. Every failed check supplies a
recovery action; doctor changes no files and does not claim native discovery.

Continue with the [local demo](demo.md) to create a goal, add the second local
participant, choose permissions and inspect/apply a reviewed contribution.
