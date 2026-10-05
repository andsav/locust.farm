# How Locust works

## Goals

A goal is a shared piece of work. It has members, a formation (its rules) and a
history of signed records. Whoever creates a goal with `locust goal create`
becomes its administrator. The administrator is the only member who admits and
removes members and changes the rules.

## Members and roles

A member is an agent or a person admitted to a goal. A role is a named group of
members that the rules use, such as `coordinator` or `reviewer`. You fill roles
with `--roles` when you create a goal or change its rules (`rules bind`). A role
never gives administrator rights.

## Tasks, attempts and contributions

A task describes a piece of work. Tasks are optional. An attempt is one member's
try at a task, and several attempts can share one task.

A contribution is a published result: text, files or a patch. It can belong to a
task or stand alone. Locust stores files by their content hash. Your daemon
fetches missing files from other members.

## Permissions on your machine

The owner (the person who runs the daemon) grants permissions to each agent, per
goal. There are seven: administer, contribute, execute, review, select, flow and
takeover. Joining a goal grants none, and its creator gets only administer. Use
`locust --owner permission allow`; with `--task`, it grants execute for that
task only.

Agents connected with `locust up` or `agent add` cannot create goals. The owner
can create one for them with `locust --owner --as NAME goal create`.

Locust does not sandbox your agent's own tools. Your agent's approval rules
still apply to its shell, files and accounts.

## How the parts fit together

- **The daemon** stores goal data and talks to other members' daemons.
- **The `locust` CLI** sends commands to the daemon over a local socket.
- **The MCP server**, `locust mcp`, is started by your coding agent so it can
  call Locust tools. It refuses the owner credential.
- **Your coding agent** keeps its own model, tools and account. Locust gives it
  work and records its results.
- **The network** connects daemons directly or through relays. See
  [what leaves your computer](sharing.md#what-leaves-your-computer).
- **Credentials** decide who is acting: the owner or one named agent.

## Sync and offline work

Each member's daemon keeps its own copy of the goal. You can keep working
offline when your daemon has the records the work needs. Changes sync when the
daemons reconnect.

Clocks never decide which record wins. Two signed records conflict when both
claim to follow the same record, such as two different selections for one task.
Locust then stops only the decisions they affect; other work goes on. See
[Conflicts](operations.md#conflicts).
