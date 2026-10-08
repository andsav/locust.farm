# Coding agents

Setup gives each coding agent an MCP entry for `locust mcp` and a skill (an
instruction file).

## Supported agents

[`up` and `agent add`](installation.md#connect-your-coding-agents) take the name
in parentheses. Setup writes the files shown:

- Codex (`codex`): `.codex/config.toml`, `.codex/hooks.json` and `.agents/skills/locust/`
- Claude Code (`claude`): `.claude.json`, `.claude/settings.json` and `.claude/skills/locust/`
- pi (`pi`): `.pi/agent/mcp.json`, `.pi/agent/extensions/locust.ts` and `.pi/agent/skills/locust/`
- Droid (`droid`): `.factory/mcp.json`, `.factory/hooks.json` and `.factory/skills/locust/`

Any other agent that can run a shell uses `shell`.
`client run` spells the names `codex`, `claude-code`, `factory-droid`, `kimi-code` and `pi`.

## What setup writes

Paths are relative to the profile home (`--profile-home`, default `~`). Each skill folder also gets a `locust-cli` script. It fixes the data
directory, credential and session, and refuses `--owner` and `--agent`.

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

Or you enroll the agent and creates a session file:

```sh
locust --owner agent enroll NAME
locust session create /PATH/TO/NAME.secret
locust --credential ~/.locust/agents/NAME.credential \
  --session /PATH/TO/NAME.secret status
```

## Agent identity and sessions

Each connected agent has one identity and one session, shared by all its chats
in that profile home. Use another profile home for a second identity. The MCP
server refuses your personal credential.

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
`delivery acknowledge`. locust.farm does not wake a closed agent.
A stop request stays open until the worker reports an outcome
([Cancelling work](operations.md#cancelling-work)).

## Hooks

Setup installs hooks by default for Codex, Claude Code, Droid and pi. The default profile
is your own client profile; use `--profile-home` to select another one. Review
native hook trust prompts and restart the client after setup. Installing the
files alone does not prove the client has loaded them.

A chat that has never used Locust never hears from these hooks, whatever
fails, and neither does a subagent's tool call: a subagent is not its parent
chat. A chat becomes a Locust chat at its first successful Locust tool call.

A chat that started, asked to start or took over an attempt, or called `locust_wait`, can receive
one `Locust:` line when it stops with work waiting. The line contains counts,
IDs and tool names. The chat acts through its MCP tools. If it ignores the line,
stopping again goes through until a new work ID arrives or that chat writes to
Locust. Reading context, acknowledging it and a start without a task that found
nothing to take are not writes for this rule, and
an attempt the chat was already told it holds does not hold it again after a
progress note, so a worker can always end its turn to ask its owner. Other
chats are only reminded of attempts held by their session.

When no work is waiting, a worker chat in your own client is told once that it
can call `locust_wait`, and its next turn end goes through: a hook never keeps
your chat waiting while you might type. A worker that no person types in may
wait for work at its turn end instead, for at most 270 seconds, one chat per
execution session. A harness says so with `LOCUST_HOOKS=unattended` in its
environment; pi says so itself in print and JSON runs. Start and tool hooks give
up after five seconds when the daemon does not answer.

Reopening or compacting a chat that already used Locust shows its held attempts
and the context tool to resume them; read acknowledgments are not reset by these hooks yet.

After a tool call, a hook can name a cancellation or a lost claim. Use
`locust_pending` before doing more work on that attempt. A successful terminal
report or cancellation acknowledgment from any chat of the same execution
session, made through its MCP tools, explains the matching claim's
disappearance; an unrelated write or progress report does not. A terminal
report made with the CLI in a shell is not seen, so the chat may hear that
claim was lost.

Set `LOCUST_HOOKS=off` in the harness environment to silence hooks. `client run`
sets it because its lifecycle is managed separately. Without a stop hook, use
`locust_wait`; without a tool hook, use `locust_pending`; after context loss,
start with `locust_status`.

`setup remove` removes only owned hook entries and restores the original bytes
when the file is otherwise unchanged. Applying setup again brings the hook
entries it installed, and pi's extension, up to the installed release, and
leaves out an entry removed by hand. A hook settings file kept as a link, such
as a dotfile manager's, is never written through: setup goes on without hooks
for that client and says so in one line of its plan and status. Setup records
from before hooks are refused with instructions
to remove their entries using the release that created them before setting up
again. Local hook marks are private files under `HOME/hook-marks` within the
Locust data directory. They are never signed or synced. When a hook fails in a
Locust chat, that chat sees `Locust context was NOT injected` once, and no more
until a hook works again; its fallback tools remain available.
