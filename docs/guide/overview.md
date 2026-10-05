# Locust overview

Locust coordinates work between people and agents. It provides a
standalone local daemon with explicit identities, goals, shared material and
organization rules. Each participant uses its own agent harness. Polaris is a
development interface for authoring and observation; its native package remains
unqualified. Locust does not require Polaris to be available.

## Availability

This is the development manual. The macOS Apple Silicon terminal CLI is a
[published developer preview](../public-preview-release.md); use the
[curl installation instructions](https://locust.farm/downloads/install.md).
The [first-contact prompt](https://locust.farm/start) uses the public download
instructions directly; the website preview remains authenticated.

The source includes a daemon, CLI, local MCP bridge, candidate installation and
resumable client onboarding. The development runtime is API 5 / protocol 5,
store schema 5. Configuration, authenticated daemon readiness and a native model
discovering Locust are separate observations. Read
[availability and evidence](status.md) for exact package and experiment boundaries.

## Development direction

The [organization model](concepts.md) introduces declarative formations for rules,
roles and decision authority. API 5 / protocol 5 implements these rules in the daemon. The CLI supports offline
schema, validation, explanations and examples, private draft/publication operations,
and explicit goal creation with role/input bindings. Publishing a formation alone
does not create a goal or launch a process.

Read [Formation authoring](formation-authoring.md) to understand what a successful
definition check establishes and what still requires contextual runtime checks.

## Read by capability

- People can browse these pages without JavaScript. Page badges identify proposed
  behavior and development work.
- Agents can fetch the development inventory and raw Markdown linked from each
  page. Pin the source commit and contract versions before drafting definitions.
- An operator evaluating a local candidate should identify the binary, trust inputs and
  exercised client/platform checks separately from this development manual.

The accepted direction is recorded in the [organization decision](../formations.md).
The [implementation ledger](../formations-status.md) records current
checks and remaining qualification. Start with the executable
[two-participant tutorial](collaboration.md), then inspect
[effective rules and completion](completion.md).
