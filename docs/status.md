# Project status

Last reviewed 2026-10-04.

## Published

- locust.farm 0.1.0, a developer preview. It runs on macOS on Apple Silicon only.
- Install it with the first command. The second shows the plan without
  installing.

  ```sh
  curl -fsSL https://locust.farm/downloads/install.sh | sh
  curl -fsSL https://locust.farm/downloads/install.sh | sh -s -- --plan
  ```

- The installer puts the software in `~/.local/share/locust` and links
  `~/.local/bin/locust`. It does not start a daemon or connect an agent.

Signing, hashes, key storage and hosting are in the
[preview release record](public-preview-release.md).

## Features

The runtime contract has 81 operations; 51 of them are MCP tools.

- A daemon that stores goals in SQLite and syncs them with other members' daemons
  over iroh.
- A command-line client, a local API on a Unix socket, and an MCP server
  (`locust mcp`).
- Goals with members, signed single-use invitations, member removal and a new
  content key after each removal.
- Seven per-goal permissions that the owner grants to local agents.
- Formations: six presets, offline validation and explanation, private drafts and
  published versions.
- Tasks, attempts, contributions with or without a task, reviews, selection and
  staged flows.
- Code snapshots of one Git commit, patches, exact diff review and guarded apply.
- Shared context: agents read new findings and acknowledge them with short
  references. Pending work comes in pages.
- Agent setup with `locust up` and `locust agent add` for Codex, Claude Code, pi,
  Droid and a portable shell route.
- Managed launch of a coding agent (`locust client run`) with readiness and
  recovery records.
- Signed packages, a signed withdrawal list, install, uninstall, and launchd or
  systemd services.
- Farm pages: an owner-approved, read-only public view of one goal, served by the
  `locust-farm` service. See [farm publication](guide/farm-publication.md).
- The formation editor at `/formations` on the website.

## How it is tested

- CI runs formatting, Clippy, Rust tests, formation exports, the guide recipes,
  Python helper tests and link checks on macOS and Ubuntu. A second workflow
  checks and builds the website. See [testing](testing.md).
- Four guide pages contain shell recipes that CI runs against the built binary.
- Codex, Claude Code and pi ran on macOS with scripted model replies, from setup
  on an installed package through a full task with a patch. Droid passed the
  managed launch and recovery checks, but its scripted task run failed (see
  below). Setup for `droid` and `shell` has unit tests only.
- Before publishing, the preview's bytes passed the package checks, the four
  recipes, installation, launchd setup, and Codex and Claude Code runs with
  scripted model replies.
- A few trials used real models on one machine. See the build-specific results
  in [collaboration follow-ups](../research/collaboration-followups.md).
- The farm check runs two daemons and the farm service on one machine. See the
  [farm check](../research/farm-qualification.md).
- A live demo ran Codex, Claude Code, Kimi Code and pi on one Mac through a
  five-stage goal, published as a public farm. See the
  [live farm demo](live-farm-demo.md).
- TLA+ models check a small part of the goal rules. They do not cover farms. See
  the [model map](../research/tla/organization.md).
- The owner reports that goals work across physical machines. There is no
  record of that here yet.

## Known problems

- Droid's scripted task run failed at the workspace step. Under the test's macOS
  sandbox, Droid's command children were killed (SIGKILL), even `/bin/echo`. The
  same commands ran without that sandbox. The cause is not known.
- In the farm check, with random ports and local discovery, the peer got no new
  events from a restarted daemon within 45 seconds. The passing run used fixed
  ports.
- Discovery limited to the local network (`LOCUST_LOOKUP=local`, relays off)
  failed in earlier runs on the test machine and passed in a later one. The cause
  is not known. With default network settings the same tests passed.
- Nothing has run on Linux except the CI checks on Ubuntu. Linux binaries were
  cross-compiled on a Mac and never run. No install or systemd service was tried.
- Real models and interactive approval prompts were not tested on the preview's
  bytes.
- Farm pages show Droid agents as an unknown coding agent.

## Not built yet

- Exclusive task reservations. Two members can work on the same task at once.
- A way for members to read a formation's `context.guidance`. The daemon stores it
  but no command returns it.
- Hooks, and automatic wake of a closed agent. An agent sees new work only when it
  calls locust.farm.
- Backup and restore.
- Linux packages.
- Closing the whole goal. `scope close` on the goal is recorded and changes
  nothing. Closing a task blocks only new attempts.

## Release work still open

- A test with two people on two machines, each with their own accounts and coding
  agents, including sleep and wake. It has not been run.
- Apple signing, notarization, the DMG, `latest.json` and the upload are not
  scripted in this repository. See [packaging](packaging.md).
- The website is behind a password. `/downloads/`, the farm gallery `/farms` and
  farm pages are public. The farm service runs on the website host.
- Decisions waiting on the owner:
  - whether the website stays behind a password;
  - whether to script the signing and publishing steps;
  - whether invitations get a default expiry (today they never expire);
  - whether agents keep `goal join --ticket`, which has no review step;
  - what closing a goal should do;
  - whether switching a farm from link-only to listed needs new consent.

## Test records

Research notes keep measured results, including failures. Each names the build it
tested.

- [Real-model trials](../research/collaboration-followups.md)
- [Farm check](../research/farm-qualification.md)
- [Preview downloads](../research/evidence/public-preview-release-2026-10-04.json)
