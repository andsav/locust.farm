# Coding agents

Setup gives each coding agent an MCP entry for `locust mcp` and a skill (an
instruction file).

## Supported agents

[`up` and `agent add`](installation.md#connect-your-coding-agents) take the name
in parentheses. Setup writes the files shown:

- Codex (`codex`): `.codex/config.toml` and `.agents/skills/locust/`
- Claude Code (`claude`): `.claude.json` and `.claude/skills/locust/`
- pi (`pi`): `.pi/agent/mcp.json` and `.pi/agent/skills/locust/`
- Droid (`droid`): `.factory/mcp.json` and `.factory/skills/locust/`

Any other agent that can run a shell uses `shell`. `droid` and `shell` are in
the current source only; the published preview does not include them.
`client run` spells the names `codex`, `claude-code`, `factory-droid`, `kimi-code` and `pi`.

## What setup writes

Paths are relative to the profile home (`--profile-home`, default `~`). Each skill folder also gets a `locust-cli` script. It fixes the data
directory, credential and session, and refuses `--owner`.

Setup refuses if the workspace or a parent folder already has a locust.farm entry or
a symlinked config folder. It never changes approval settings or provider
credentials.

## Reload the agent after setup

Start a new chat (restart Codex first) and ask the agent to call
`locust_status`. In an open chat, Claude Code needs `/reload-skills` and pi needs
`/reload`.

## Other agents

`shell` writes a skill, `locust-cli` and an MCP connection file under
`.local/share/locust-agent/`. It edits no app config.

Or the owner enrolls the agent and creates a session file:

```sh
locust --owner agent enroll NAME
locust session create /PATH/TO/NAME.secret
locust --credential ~/.locust/agents/NAME.credential \
  --session /PATH/TO/NAME.secret status
```

## Agent identity and sessions

Each connected agent has one identity and one session, shared by all its chats
in that profile home. Use another profile home for a second identity. The MCP
server refuses the owner credential.

## Launch an agent with locust.farm

`client run` starts an agent in the foreground, optionally for one goal and
attempt:

```sh
locust --credential CREDENTIAL --session SESSION client run \
  --client codex --executable /PATH/TO/codex --client-version VERSION \
  --workspace /PATH/TO/WORKSPACE --profile /PATH/TO/PROFILE \
  --goal GOAL --attempt ATTEMPT --prompt PROMPT
```

For pi, add `--native-session /PATH/TO/FILE`. locust.farm picks no model provider
and keeps the agent's approval settings.

Launch states: Launching (recorded before start), Started (process running),
Ready (locust.farm tools answered), Blocked (approval settings refused a tool call),
Exited and Unknown. `client recover` marks an interrupted launch Unknown; it
never starts or stops a process.

`client status` shows the record and `client pending` the pending work.
`client run --resume NATIVE_ID` with the same paths resumes.

## Pending work and stopping

Agents check for work with `pending` and `wait` (MCP: `locust_pending`,
`locust_wait`). Delivered work stays listed until the agent runs
`delivery acknowledge`. locust.farm does not wake a closed agent; agent hooks are not
built. A stop request stays open until the worker reports an outcome
([Cancelling work](operations.md#cancelling-work)).
