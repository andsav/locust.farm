# How installation works

Status: built. The public installer supports macOS on Apple Silicon only. The Linux service code is not tested on Linux.

User steps are in [Install locust.farm](guide/installation.md). The setup prompt in
[first contact](first-contact.md#entry-prompt) runs them through an agent.

## Ways to install

- **The public installer** installs the published developer preview. Add
  `-s -- --plan` after `sh` to see the plan only.
- **By hand**: run a `locust` you already trust against an unpacked package.

```sh
curl -fsSL https://locust.farm/downloads/install.sh | sh
```

Both end with `install plan` and `install apply`. locust.farm never updates itself;
run the installer again to update.

## What the public installer checks

[`install.sh`](../scripts/install.sh) takes `--prefix` (default
`~/.local/share/locust`), `--bin-dir` (default `~/.local/bin`) and `--plan`. It
stops at the first failed step:

1. It refuses any host but macOS on arm64, and relative paths.
2. It refuses a bin directory that is a symlink or someone else's, and an
   existing `locust` there that points elsewhere.
3. It downloads `latest.json`. The target must be `aarch64-apple-darwin` and the
   trust key hash must equal the one in the script.
4. It downloads the archive, checks its SHA-256 and requires exactly nine files.
5. It checks the binary's SHA-256 and its timestamped Apple signature (team
   `P2Q3P9R6AT`). Only then does it run downloaded code.
6. It downloads the current withdrawal list from `/downloads/` and checks the
   hash of `trust.pub`.
7. It runs `package verify`. Commit, version and manifest hash must match
   `latest.json`.
8. It prints the `install plan`. With `--plan`, it stops here.
9. It runs `install apply` and links `BIN_DIR/locust` to `PREFIX/current/locust`.

It uses no sudo, edits no shell startup files, starts no daemon and connects no
agent. You trust the script itself through HTTPS.

## Install a package by hand

Use a `locust` you already trust: a verifier cannot vouch for the download it came
in. `BUNDLE` is an unpacked package. The withdrawal list's signature is its path
plus `.sig`.

```sh
locust --json install plan --prefix /PATH/TO/PREFIX --bundle /PATH/TO/BUNDLE --trust-key /PATH/TO/trust.pub --withdrawals /PATH/TO/withdrawals.json
locust --json install apply --prefix /PATH/TO/PREFIX --bundle /PATH/TO/BUNDLE --trust-key /PATH/TO/trust.pub --withdrawals /PATH/TO/withdrawals.json --expect-plan PLAN_SHA256
locust --json install status --prefix /PATH/TO/PREFIX
```

`package verify` runs the same checks without a prefix.
[Packaging and releases](packaging.md) explains building and signing.

## Package checks and activation

[package.rs](../crates/locust/src/package.rs) checks the Ed25519 signature in
`manifest.sig` over the exact `manifest.json` bytes, with the public key you name.
A package never chooses its own key. It then checks each file's SHA-256, size
and mode, the binary's machine format, and the signed withdrawal list. Links and
special files are refused. Verification never runs the binary.

[installation.rs](../crates/locust/src/installation.rs) activates a package:

- `install plan` changes nothing. It returns the plan and its `plan_sha256`.
- `install apply` locks the prefix, builds the plan again and refuses if the
  digest changed.
- The package must match the host, and its API and protocol must equal those of
  the `locust` running the command.
- A lower version needs `--allow-downgrade` in both plan and apply.
- Apply copies the files to a private staging folder and hashes them again. It
  runs the new binary's `--version` with an empty environment and compares it
  with the manifest.
- Apply saves the trust state before it switches `current`. A crash can leave the
  old release active, but never new code with an older withdrawal list.

| Prefix path | Contents |
| --- | --- |
| `current` | Symlink to the active release |
| `releases/MANIFEST_SHA256/` | One verified release per manifest |
| `trust-state.json` | Trust key and newest withdrawal list seen |
| `install.lock` | Lock file |
| `service-LABEL.json` | User service ownership record |
| `setup/` | Agent setup records and journals |

Folders must belong to you and have mode 0700. Once a prefix has seen a
withdrawal list, it refuses a lower sequence or other bytes at the same sequence.
It refuses a list that drops an earlier withdrawal, and any other trust key. No
command changes the key of a prefix.

An interrupted install can leave a `.stage-*` folder or an inactive complete
release. locust.farm never activates a partial copy, and checks a complete one again
before using it. Unknown staging folders stay for you to inspect.

A running daemon keeps its old code. `install apply` reports
`service_restart_required`; restart the service to run the new code.

## Remove the software

Remove agent setup and the service first, because they point at the software.

```sh
locust --json install uninstall-plan --prefix /PATH/TO/PREFIX
locust --json install uninstall --prefix /PATH/TO/PREFIX --expect-plan PLAN_SHA256
```

Uninstall removes `current` and every unchanged release. It keeps and reports
modified or unknown files. It keeps the trust state, the data directory, logs,
credentials, sessions and agent configuration.

## User services

[service.rs](../crates/locust/src/installation/service.rs) writes a per-user
service that runs `PREFIX/current/locust --home DATA daemon run`.

```sh
locust --json service plan --prefix /PATH/TO/PREFIX --kind launchd --profile-home "$HOME" --daemon-home /PATH/TO/DATA --log-dir /PATH/TO/DATA/logs
```

`service apply --expect-plan PLAN_SHA256`, `start`, `status`, `stop`,
`remove-plan` and `remove --expect-plan PLAN_SHA256` take the same flags.

| `--kind` | Host | Unit file |
| --- | --- | --- |
| `launchd` | macOS arm64 | `PROFILE/Library/LaunchAgents/LABEL.plist` |
| `systemd` | Linux x86_64 | `PROFILE/.config/systemd/user/LABEL.service` |
| `none` | any | None; you run the daemon |

- `LABEL` is `farm.locust.` plus 16 hex digits of the SHA-256 of the data
  directory path, so each data directory gets its own service.
- The unit sets `HOME` and the XDG folders to the profile home and logs to
  `stdout.log` and `stderr.log`. launchd restarts the daemon unless it exits
  cleanly; systemd restarts it on failure.
- [service_install.rs](../crates/locust/src/installation/service_install.rs)
  saves an ownership record before it creates the unit. locust.farm never takes over a
  unit it did not create and refuses an edited one.
- `service start` restarts a loaded service. Use it after an upgrade.
- Right after start or stop, `service status` can say `unavailable`. Check again
  until it settles, then run `locust --owner doctor`.
- `service remove` needs a stopped service. It keeps the data and logs.

locust.farm creates no root daemon or system service.

## Agent setup

[setup.rs](../crates/locust/src/installation/setup.rs) connects one coding agent
profile to one enrolled agent. `up` and `agent add` run it for you. Run it
directly when you already have a credential file and a session file:

```sh
locust --json setup plan --prefix /PATH/TO/PREFIX --client codex --profile-home "$HOME" --workspace /PATH/TO/WORKSPACE --daemon-home /PATH/TO/DATA --credential-file /PATH/TO/DATA/agents/NAME.credential --session-file /PATH/TO/SESSION
```

`setup apply --expect-plan PLAN_SHA256`, `status`, `remove-plan` and `remove`
take the same flags. Paths are relative to the profile home:

| Agent | `--client` | MCP entry | Skill folder | Available in |
| --- | --- | --- | --- | --- |
| Codex | `codex` | `.codex/config.toml` | `.agents/skills/locust/` | preview, source |
| Claude Code | `claude` | `.claude.json` | `.claude/skills/locust/` | preview, source |
| pi | `pi` | `.pi/agent/mcp.json` | `.pi/agent/skills/locust/` | preview, source |
| Droid | `droid` | `.factory/mcp.json` | `.factory/skills/locust/` | source |
| Any shell agent | `shell` | `.local/share/locust-agent/mcp.json` | `.local/share/locust-agent/skills/locust/` | source |

Setup writes:

- an MCP server entry named `locust` that runs `PREFIX/current/locust mcp` with
  `LOCUST_HOME`, `LOCUST_CREDENTIAL` and `LOCUST_SESSION` set to file paths, not
  secrets. For `shell`, no app reads this file;
- a copy of the skill with an "Installed Locust CLI" section;
- a `locust-cli` script next to the skill. It fixes `--home`, `--credential` and
  `--session`, and refuses those flags, `--owner` and `--as` in its arguments
  ([launcher.rs](../crates/locust/src/installation/setup/launcher.rs)). It does
  not isolate the agent from the owner credential.

Setup refuses when the workspace or a parent folder already has a locust.farm MCP
entry or skill. It also refuses when the profile has another locust.farm entry or a
project configuration folder is a symlink.

Records under `PREFIX/setup/` let an interrupted write resume. Removal restores
the original file if nothing else changed it; otherwise it removes only the
locust.farm entry. An edited locust.farm entry, skill or script is kept and reported.

Setup never grants permissions, changes approval settings or copies provider
credentials. It reports `reload_required`: the agent sees the change after a
restart or in a new chat.

## Connect agents with up and agent add

`locust up` ([CLI](../crates/locust/src/cli/onboarding.rs),
[journal](../crates/locust/src/installation/onboarding.rs)) runs these steps:

1. It finds the installed release from its own path or `--prefix`. The release
   must be verified and not withdrawn. A development build is refused.
2. Without `--client`, it lists the agents it finds and changes nothing. In a
   terminal it asks which to connect.
3. It plans the service (`launchd` on macOS, `systemd` on Linux, or
   `--service none` to use a running daemon) and the agent setup.
4. It asks for confirmation. `--plan` only shows the plan. `--yes` applies without
   asking and needs `--client`. `--json` turns off prompts.
5. It starts the service and waits for the daemon (`--wait-ms` sets a maximum;
   `0` checks once). A running service is not restarted.
6. For each agent, it writes a journal, saves a credential and a session file,
   enrolls the agent and applies setup.
7. It checks access with the agent's own credential and session.
8. It reports `model_ready:false` and `grants_added:false`.

`locust agent add CLIENT` runs the same steps without the service.

Defaults: `--home` falls back to `LOCUST_HOME`, then `~/.locust`.
`--profile-home` and `--service-profile-home` default to `HOME`, `--workspace`
to the current folder and `--log-dir` to `DATA/logs`. Without `--name`, names
look like `codex-maple-1a2b3c4d`. Enrolled agents get no goal permissions and
cannot create goals.

The journal is `DATA/onboarding/HASH/state.json`, with the `credential` and
`session` files beside it. `HASH` comes from the client and profile home. If `up`
stops partway, run it again: it reuses the saved name and credential. Changed
secrets or configuration stop the retry for you to inspect.

With the default profile home, setup stops when `CODEX_HOME` or
`PI_CODING_AGENT_DIR` points away from the standard folder, or when
`CLAUDE_CONFIG_DIR` is set. Unset it or pass `--profile-home`.

Each profile holds one agent identity and one session, shared by all its chats.

The published preview accepts only `codex`, `claude` and `pi`. For another agent,
the owner runs `service plan`, `apply` and `start`, then
`locust --owner agent enroll NAME` and `session create PATH`. The agent then uses
the CLI with `--credential` and `--session`.

## Check a connected agent

```sh
locust --home /PATH/TO/DATA doctor --client codex
```

`doctor` reads the journal and checks the release, service, daemon access,
saved identity, MCP entry, skill, script and workspace. Each failed check names a
fix. It changes nothing. To see whether the agent loaded locust.farm, start a new chat
and ask for its locust.farm status.

## How this is tested

Unit tests cover [installation](../crates/locust/src/installation/tests.rs),
[setup](../crates/locust/src/installation/setup/tests.rs) and
[connecting agents](../crates/locust/src/installation/onboarding/tests.rs).
[test_public_installer.py](../scripts/tests/test_public_installer.py) checks the
installer's refusals offline. [check_installation.py](../scripts/check_installation.py)
and [check_onboarding.py](../scripts/check_onboarding.py) install a local package
signed with throwaway keys. No test runs the public installer against the live
server. See [Testing](testing.md).
