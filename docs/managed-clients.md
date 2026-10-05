# Explicit managed client sessions

Status: implemented local foreground launcher for Codex, Claude Code, Factory
Droid and Pi. The current binding uses task/attempt identities. The [client harnesses](client-qualification.md) provide reproducible lifecycle
and recovery checks.
This is not unattended activation, a worker sandbox, or release installation.

## Run a selected client

Use an enrolled agent credential and an existing private session file. Select an
existing absolute workspace and client HOME. Runtime provider authentication comes
from that selected profile and the invoking environment. Supply the actual client
version and any native policy/model flags explicitly:

```sh
locust --home /ABSOLUTE/DAEMON/HOME \
  --credential /ABSOLUTE/AGENT/CREDENTIAL \
  --session /ABSOLUTE/SESSION/SECRET \
  client run --client codex --executable /ABSOLUTE/PATH/TO/codex \
  --workspace /ABSOLUTE/WORKSPACE --profile /ABSOLUTE/CLIENT/HOME \
  --client-version OBSERVED_VERSION --goal GOAL_ID --attempt ATTEMPT_EVENT_ID \
  --arg=--skip-git-repo-check --prompt 'Inspect the bound Locust attempt.'
```

`--arg` repeats client run arguments; `--global-arg` repeats arguments placed
before the native run verb. Pi also requires `--native-session` with an absolute
native session-file path; for Pi 1.0.1, choose a nested path under
`.pi/agent/sessions/` so startup migration does not move the selected file.
The launcher does not select a provider, weaken native
permission policy, or supply a credential. A default headless policy may deny a
write; deliberate permissive qualification is recorded separately.

Native stdout and stderr stream to Locust's stderr. `--json` leaves stdout for one
result envelope after the managed invocation ends. A successful lifecycle report
does not mean the client completed its task: inspect the native exit evidence and
the daemon's attempt/contribution state.

## Identity and recovery

The [managed adapter](../crates/locust-adapter/src/managed.rs) records `Launching`
through the authenticated session API before spawning. `Started` requires an
owned child process. `Ready` requires both a structured native session identifier
and a matching authenticated production MCP tools-ready receipt. A configured
server or prose startup message is insufficient. Structured native permission
denials can record `Blocked`; owned-child exit records `Exited`.

The binding contains the principal, Locust session, optional goal and exact task/attempt identity. A claim generation is copied only from the daemon's
authenticated session view. Launch intent, native identity, notification state,
claim ownership, contribution publication, rule completion and scoped selection
remain separate facts.

Use `client status` to inspect that record. Resume requires `client run --resume
NATIVE_SESSION_ID` with the same client, paths, goal and attempt. Pi uses the
same `--native-session` file as well. A known exited session with no outstanding
claim can be explicitly launched afresh with a new binding. A stale or occupied
attempt is rejected before spawn.

`client recover` marks an interrupted non-exited record `Unknown` without
spawning or signaling anything. A persisted PID is historical correlation, not
process authority. An uncertain record blocks another launch under that session;
the participant must reconcile it locally. No crash recovery infers that an
unobserved launch never happened. Session/profile locks prevent concurrent local
launchers. A local foreground interrupt is forwarded only to the retained,
unreaped child; repeated explicit signals are forwarded until that child is
reaped. Native exit is observed independently of output-pipe EOF. Readers then
drain the bytes already buffered and close, so a descendant retaining a pipe
does not keep the launcher alive. This does not prove descendant termination;
no remote offer or cancellation signals a process.

## Pending work and cancellation

[Delivery handling](../crates/locust-adapter/src/delivery.rs) uses daemon-authored
identifiers and status. Ordinary `locust_pending`/`locust_wait` MCP tools remain
the active client's path; `client pending` exposes the managed binding and the
same authoritative pending state to local callers. It is read-only, including
when a previous notification is a duplicate or the client has exited. Qualified
hooks and automatic wake remain separate capabilities and are not claimed here.

Only the managed lifecycle owner writes its session metadata. Hook/pending
readers do not rewrite it: the existing `session.report` API has no compare and
swap. Saved notification metadata is advisory. Lost, duplicate or late delivery
does not hide durable pending work, create a claim or acknowledge cancellation.
Claim takeover fences the old generation. Cancellation stays requested until a
current authorized caller explicitly reports `stopped`, `completed` or
`uncertain`; emitting a notification or observing a client exit supplies none of
those task outcomes.

## Profile and evidence boundaries

The [CLI](../crates/locust/src/cli/client.rs) uses the existing
[configuration adapter](../crates/locust-adapter/src/config.rs). Codex/Claude
registration is passed on argv. Droid/Pi use a scoped file overlay that retains
the original bytes and mode, refuses a conflicting `locust` server, and restores
only the owned overlay after exit. Concurrent edits are preserved and recovery
paths are reported. The [profile helpers](../crates/locust/src/cli/client/profile.rs)
reject a symlink at the selected profile root or in configuration components
beneath it, and use private recovery files. Ancestor aliases such as `/tmp` are
accepted. If a launcher crashes, inspect retained originals and the
session record before manually resolving its configuration.

The manual-resume capability remains false until an explicit resumed invocation
observes the same native identity and authenticated tool readiness. A native ID
alone does not establish account/profile continuation support.

Native permission policy, tool access, ordinary pending recovery, active hooks,
automatic wake, manual native resume and confinement are distinct capabilities.
This launcher reports no confinement or automatic wake. Processes running as the
same OS user can access that user's files and credentials; private profile paths
are not a sandbox. Prompts, environment values and private transcripts are not
stored in the daemon's managed record.

Adapter tests live alongside the implementation; the
[CLI integration fixtures](../crates/locust/tests/managed.rs) cover launch and
failure boundaries. The [actual-client harness](../scripts/check_managed_clients.py)
tests separately against the production daemon and retained client binaries.
