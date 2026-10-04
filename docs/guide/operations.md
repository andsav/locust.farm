# Services, fresh state and durable flow

**Status: implemented development runtime and user-session services.** A running process and a ready authenticated
API are separate observations. A committed flow record does not establish an executed task.

## Own only explicitly selected services

Candidate services use a user-session launchd domain on macOS or systemd user
manager on Linux. Select software prefix, daemon home, service profile and logs
explicitly. The service renderer invokes the binary directly, not a shell. It
creates no root daemon, system service or login account.

Inspect the reviewable service plan before apply. Existing unowned units or
altered owned units must be refused. Start/stop requests can be asynchronous;
inspect settled manager state and independently check authenticated daemon
readiness. A missing GUI domain or user bus is unavailable, not stopped.
Software upgrades need an explicit restart to replace running code. Use the
[canonical service commands](../installation.md) for the implemented route.

For a separately managed macOS service, use the same explicit selection on each
call; substitute `systemd` only on the supported Linux user-manager route:

```sh
locust --json service plan --prefix /SOFTWARE/locust --kind launchd --profile-home /PROFILE --daemon-home /DATA/locust --log-dir /LOGS/locust
locust --json service apply --prefix /SOFTWARE/locust --kind launchd --profile-home /PROFILE --daemon-home /DATA/locust --log-dir /LOGS/locust --expect-plan PLAN_SHA256
locust --json service start --prefix /SOFTWARE/locust --kind launchd --profile-home /PROFILE --daemon-home /DATA/locust --log-dir /LOGS/locust
locust --json service status --prefix /SOFTWARE/locust --kind launchd --profile-home /PROFILE --daemon-home /DATA/locust --log-dir /LOGS/locust
locust --home /DATA/locust --owner --json doctor
```

Use `service stop`, observe stopped/absent state, then `service remove-plan` and
`service remove --expect-plan PLAN_SHA256` with the same selection to remove an
owned unit. Removal preserves daemon state and logs. An unowned or modified unit
is a refusal requiring inspection.

## Initialize fresh current-model state

Select a new empty absolute state location explicitly and initialize it directly
with `locust --home /absolute/new-state daemon run`. The process stays in the
foreground; use a reviewed service plan for unattended operation. Starting with a
new path does not erase or convert an existing directory. Separate software, state, profile and workspace paths.

Restart reopens that same supported format and replays durable decisions and
proofs. Unsupported state, definitions, events or peers receive a clear refusal
before mutation. Do not convert files, load an old decoder, run a mixed-version
daemon or silently fall back. Preserve the rejected state for inspection.

## Advance configured transitions

The accepted D14 loop derives readiness as a pure projection. The goal
administrator's daemon, with its signing key and the local `flow` grant, signs
the child-task, review-request or handoff effect of a configured stage. The daemon drives this loop after
replay/ingestion/local action without asking an agent for every next step.

Its logical effect identity binds rule context, trigger, scope, action and target.
The signed effect, feed update and recipient outbox records commit atomically.
Replicas synchronize the signed evidence. The receiving daemon records one inbox
item durably before sending a transport receipt. The sender commits that receipt
before retiring its outbox entry. A lost receipt retries the same logical effect;
it does not create another task or inbox item. Restart resumes this process.

`pending --goal GOAL` exposes the recipient's stable delivery ID and separate
`received`, `available` and `acknowledged` fields. The recipient uses
`delivery acknowledge --goal GOAL --effect ID` for explicit agent acknowledgment.
Transport retries stop once durable receipt is confirmed; the agent's inbox item
remains visible until the agent acts. A receipt does not imply acknowledgment.

## Read delivery and execution separately

Equivalent threshold witnesses do not mint different logical actions. A received
item remains visible with `available: false` if its authority becomes disputed or
its recipient becomes ineligible. The daemon stops sending affected pending
outbox entries. Work offers, local attempt claims and managed-session process
observations are separate records. Neither transport receipt nor agent
acknowledgment proves that a remote process started.

An unavailable administrator blocks configured stage transitions; other replicas do
not impersonate it. A recipient without local permission retains ready work
instead of automatically executing it. A newly disputed proof stops affected
undelivered offers and reports the change; it cannot silently undo a filesystem
change or establish that a process stopped. [Recovery](recovery.md) covers these
failure observations.

## Configuration ownership and format IDs

Service/client configuration belongs to the exact selected paths and ownership
records. Refuse unowned collisions and preserve concurrent edits; a byte match is
not proof that Locust owns a pre-existing file. Provider/account credentials,
client approval rules and environment overrides stay outside implicit setup.

Software version, blueprint schema, local API, network protocol and persistent
state format are distinct identities. The development source context must name
its current markers. Unsupported state/protocol input is refused before mutation
or decoding with an old reader. A same-model restart is recovery, not migration.
