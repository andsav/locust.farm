# Documentation

This folder holds current documentation. Dated investigations and experiments
belong in [research](../research/README.md).

- Give each document a one-line status and keep it in step with the code.
- Delete documents that are superseded. Git keeps them; point any remaining
  links at a GitHub permalink.
- List every tracked Markdown file here exactly once. After staging new files,
  run `python3 scripts/check_docs.py` from the repository root.
- [`site.json`](site.json) chooses the public manual's pages, order and titles.

## Public manual

The pages published on locust.farm under `/docs`. See the
[manual's README](guide/README.md).

- [locust.farm overview](guide/overview.md): what locust.farm is, what works today and where to start.
- [How locust.farm works](guide/concepts.md): goals, members, tasks, contributions, permissions and how the parts connect.
- [Install locust.farm](guide/installation.md): install the macOS preview, connect your coding agents and check they work.
- [Start a goal and invite others](guide/collaboration.md): run two agents on one computer, then invite a person.
- [Sharing and privacy](guide/sharing.md): what members can read, sharing code, removing members and network traffic.
- [Review and apply a patch](guide/apply.md): review a contribution and apply its patch to your own checkout.
- [Formations](guide/formations.md): the rules a goal follows, presets, when results count and who decides.
- [Write a formation](guide/formation-authoring.md): check, draft and publish a formation with the CLI, your agent or the editor.
- [Coding agents](guide/agents.md): supported coding agents, what setup writes, sessions and launching agents.
- [Public farm pages](guide/farm-publication.md): publish a public view of a goal with every member's consent.
- [Run the daemon](guide/operations.md): services, the data directory, settings, restarts, conflicts and cancelling work.
- [Troubleshooting and glossary](guide/help.md): common problems, short answers and the terms the manual uses.
- [Formation schema reference](guide/schema-reference.md): generated formation fields, offline commands and example files.
- [Command, API and MCP reference](guide/runtime-reference.md): versions, command basics, exit codes, the local API, MCP and events.

## Design

- [Formation design](formations.md): how formations, tasks, reviews, selection and rule changes work.
- [Formation editor](formation-editor.md): the `/formations` editor on locust.farm and the rules it follows.
- [Formation prompt contract](formation-prompt.md): the exact prompt the formation editor copies for an agent.
- [First contact](first-contact.md): the setup prompt a person pastes into their coding agent.
- [Public farm architecture](swarm-visualization-plan.md): how farm pages, consent and the farm service fit together.
- [Live four-client farm demo](live-farm-demo.md): record of the farm demo run with four real coding agents on one Mac.

## Building and releasing

- [Crates](crates.md): what each Rust crate does and how they depend on each other.
- [How installation works](installation.md): package checks, installation, services and connecting agents.
- [Packaging and releases](packaging.md): building, signing and publishing release packages.
- [Published macOS terminal preview](public-preview-release.md): record of the 0.1.0 developer preview for macOS on Apple Silicon.
- [Testing](testing.md): the checks to run for each change and the tests with real coding agents.
- [Public manual](manual.md): how the manual is built, its routes and the tests that check it.

## Status

- [Project status](status.md): what is published, what is built, what is tested and what is not built yet.
