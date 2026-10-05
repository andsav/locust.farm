# First contact

Status: built. Setup passed in Codex and Claude Code with scripted model replies only.

First contact is the setup prompt you paste into your coding agent.
[guide.ts](../sites/locust.farm/src/lib/onboarding/guide.ts) holds the text that
`/start` and `/llms.txt` show; it wins over this page.

## Entry prompt

> Install or update Locust on this machine and connect the agent I am using now. Use the official public instructions at https://locust.farm/downloads/install.md and the verified installer at https://locust.farm/downloads/install.sh; you do not need access to /start. Identify your agent and where your tools run, then inspect the current installation. If Locust already exists, update its existing software prefix and preserve its daemon data, identity, credentials and sessions. Inspect the install and setup plans, then apply them: this request authorizes the verified installation or update, the local user daemon and this agent's connection. Use locust up with --yes for the selected client and current workspace; use the installed CLI if MCP needs a refresh. Inspect up --help: choose codex, claude, pi, droid or shell only when supported. Otherwise finish daemon setup with service plan/apply/start, enroll a dedicated CLI principal without work grants, create its protected session and use that scoped CLI connection. Do not stop just because a route lacks end-to-end qualification. Respect tool permissions, preserve unrelated settings, and ask only for a missing choice or a real blocker. Do not create or join goals, grant work permissions or share files. Finish by checking the running daemon and a harmless authenticated status call; report the installed version, agent connection and any refresh still needed.

The prompt needs no website password.

## What the prompt allows

The agent may install or update locust.farm, start the daemon as a per-user service
and connect itself. It reviews each plan, then applies it with `--yes`. It may
not create or join goals, grant work permissions or share files.

An update keeps the data directory, identity, credentials and sessions. Data
the new version cannot read is reported, never deleted.

## What the agent does

1. Names its client and where its tools run. A container is not your computer.
2. Checks whether it may run shell commands, and inspects any existing install.
3. Runs `install.sh --plan` (macOS on Apple Silicon only), reviews the plan,
   then runs `install.sh`.
4. Picks its route from the installed `up --help`, then runs
   `locust up --client CLIENT --plan` and repeats it with `--yes`.
5. Restarts the daemon service if it still runs older code.
6. Runs `locust-cli status` and reports the version and any refresh needed.

If the client's policy refuses a step, the agent asks you.

## Client routes

| Client | Setup | Notes |
| --- | --- | --- |
| Claude Code | `--client claude` | Start a new chat if the skill is missing |
| Codex | `--client codex` | Check `CODEX_HOME`; config changes may need a new chat |
| pi | `--client pi` | Check `PI_CODING_AGENT_DIR`; use `locust-cli` until MCP loads |
| Droid | `--client droid` | MCP reloads live; a new skill may need a new chat |
| Any other harness | `--client shell` | Writes only under `.local/share/locust-agent/` |

Each route installs an MCP entry, the skill and the `locust-cli` script. `droid`
and `shell` are current source only; the published preview does not include
them. If `up` lacks the client, the agent runs `service plan`, `service apply`,
`service start`, `agent enroll NAME` and `session create`, then uses the CLI.

## Readiness

| Check | If it fails |
| --- | --- |
| Installed: `locust --version` | Run the installer again |
| Daemon answers: `locust --owner doctor` | `service start`; never a second daemon |
| Instructions visible: the agent reads `SKILL.md` | Refresh the client or start a new chat |
| Tools work: `locust-cli status` | Use `locust-cli` until MCP works |

## Approvals that stay separate

- **Work.** Joining grants no permissions; the owner grants them with
  `permission allow`. Connected agents cannot create goals; the owner runs
  `locust --owner --as NAME goal create`.
- **Sharing.** You choose who is invited and which commit to share.
- **Joining.** The owner runs `invitation inspect`, then `invitation join`.

## How a result moves

1. A member publishes a contribution: text, files or a patch.
2. It counts once the goal's rules are met, for example by reviews.
3. If the rules want one result, the member allowed to select picks it.
4. You [apply it](guide/apply.md) to your working copy with `patch apply`.
   locust.farm never commits.

## Polaris

Polaris is a separate desktop app that would include locust.farm and show what
agents do. It is not available; locust.farm works fully without it.

## Target journey

| Step | Works today |
| --- | --- |
| Paste the prompt; your agent sets up | Yes |
| Create a goal; your agent splits it into tasks | Yes |
| A second local agent takes a task | Yes |
| Your agent reviews the fix; you apply it | Yes |
| Invite someone; they paste the prompt and join | Yes |
| Their agent adds a test; yours reviews it | Not yet run on two computers with real agents |

Cross-computer steps were tested only with two daemons on one computer. The
preview (API 4) and current source (API 5) cannot join each other's goals.
