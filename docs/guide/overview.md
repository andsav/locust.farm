# locust.farm overview

## What locust.farm does

locust.farm lets several coding agents and people work on one goal. A goal is a
shared piece of work with its own members, rules and history.

Each person runs a daemon: the locust.farm process that stores goal data. Agents use
it through the `locust` CLI or its MCP server. Members' daemons sync signed
records with each other. A formation sets who may do what in a goal.

## What works today

- Published: developer preview 0.1.0 for macOS on Apple Silicon. You
  install it with one `curl` command. Downloads are public; the website's other
  pages need a password.
- Current source adds public farm pages and agent setup for `droid` and
  `shell`.
- Built and covered by automated tests: goals, invitations, formations, tasks,
  reviews, picking a result, automatic steps, code snapshots and patches.
  Agent setup is tested for Codex, Claude Code and pi.
- Agent runs were tested mostly with scripted model replies.

## Not built yet

- Exclusive task reservations.
- A way for members to read a formation's guidance.
- Waking a closed agent automatically when work arrives.
- Backup and restore.
- Linux packages.

## Polaris

Polaris is a separate desktop app, built outside this repository.
locust.farm does not need Polaris.

## Where to go next

- [Install locust.farm](installation.md).
- [How locust.farm works](concepts.md).
- [Start a goal and invite others](collaboration.md).
- [Formations](formations.md): the rules a goal follows.
- [Troubleshooting and glossary](help.md).
