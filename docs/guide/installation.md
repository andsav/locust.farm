# Install locust.farm

## Install the developer preview

The preview runs on macOS on Apple Silicon only. Install it:

```sh
curl -fsSL https://locust.farm/downloads/install.sh | sh
```

To see the plan without installing anything:

```sh
curl -fsSL https://locust.farm/downloads/install.sh | sh -s -- --plan
```

The installer puts the software in `~/.local/share/locust` and links
`~/.local/bin/locust`. Before it runs the downloaded program, it checks Apple's
publisher signature. That program then checks locust.farm's own package signature
and the signed withdrawal list.

The installer needs no sudo and does not edit your shell startup files. It does
not start a daemon or connect an agent. Add `~/.local/bin` to your `PATH` if it
is not there.

To update, run the installer again, then
[restart the service](operations.md#run-it-as-a-service).

## Connect your coding agents

Review what setup will change:

```sh
locust --home "$HOME/.locust-api4" up --client codex --workspace "$PWD" --plan
```

Then run the same command without `--plan` to review and apply each change. Or
use `--yes` to apply without prompts; it needs `--client`. `up` installs the
daemon as a user service (launchd on macOS) and connects the agents you name. If
`up` stops partway, run it again to resume.

To add another agent while the daemon runs:

```sh
locust --home "$HOME/.locust-api4" agent add claude --workspace "$PWD"
```

The preview accepts `codex`, `claude` (Claude Code) and `pi`. Current source
also accepts `droid` and `shell`; the published preview does not include them.

Use the same `--home` (the data directory) in every command.

The [setup prompt](../first-contact.md#entry-prompt) asks your agent to run
these steps for you.

## Check that it works

```sh
locust --home "$HOME/.locust-api4" doctor --client codex
```

Then start a fresh chat with your agent. Ask it to read its locust.farm skill (the
instruction file that setup installed) and report its locust.farm status. A connected
agent has no work permissions yet;
[Start a goal and invite others](collaboration.md#two-agents-on-one-computer)
shows how to grant them.

## Install from a package by hand

Run these with a `locust` executable you already trust. `/PATH/TO/PACKAGE` is
an unpacked package folder.

```sh
locust package verify --bundle /PATH/TO/PACKAGE --trust-key /PATH/TO/trust.pub --withdrawals /PATH/TO/withdrawals.json
locust install plan --prefix /PATH/TO/PREFIX --bundle /PATH/TO/PACKAGE --trust-key /PATH/TO/trust.pub --withdrawals /PATH/TO/withdrawals.json
locust install apply --prefix /PATH/TO/PREFIX --bundle /PATH/TO/PACKAGE --trust-key /PATH/TO/trust.pub --withdrawals /PATH/TO/withdrawals.json --expect-plan PLAN_SHA256
locust install status --prefix /PATH/TO/PREFIX
```

`PLAN_SHA256` is the `plan_sha256` that `install plan` prints. The trust key is
the publisher's public key; get it from a source you trust, not from the same
download. The withdrawal list, signed with that key, names packages that must
not be installed. Its signature goes beside it as `withdrawals.json.sig`.

## Build from source

```sh
cargo build --locked -p locust
```

A development build runs the daemon, the CLI and this manual's scripts. `up`
and `agent add` refuse to run from it; they need an installed package. A source
build refuses the preview's data, so give it a new directory with `--home`.

## Linux

No Linux package is published. The source builds on Linux x86_64. Its systemd
user-service code has not been tested on Linux.

## Remove locust.farm

1. Remove each agent's setup with `locust setup remove-plan`, then
   `locust setup remove --expect-plan PLAN_SHA256`. Both take `--client`,
   `--prefix`, `--profile-home`, `--workspace`, `--daemon-home`,
   `--credential-file` and `--session-file`, with the values `up` used. The
   credential and session files are under `onboarding/` in the data directory.
2. Stop and remove the service with `locust service stop`,
   `service remove-plan` and `service remove`. They take the flags in
   [Run it as a service](operations.md#run-it-as-a-service).
3. Remove the software:

   ```sh
   locust install uninstall-plan --prefix "$HOME/.local/share/locust"
   locust install uninstall --prefix "$HOME/.local/share/locust" --expect-plan PLAN_SHA256
   ```

Uninstalling leaves the `~/.local/bin/locust` link; delete it yourself. The data
directory, credentials and logs stay.
