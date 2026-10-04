# Services, fresh state and durable flow

**Status: user-session service source exists; new organization state and durable
flow implementation are pending.** A running process and a ready authenticated
API are separate observations. A proposed flow record does not establish an
executed task.

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

## Initialize fresh current-model state

For the replacement runtime, select a new empty absolute state location
explicitly, review its identity and initialize directly to the supported schema.
This is a future procedure until that runtime exists, not authorization to erase
an existing directory. Separate software, state, profile and workspace paths.

Restart reopens that same supported format and replays durable decisions and
proofs. Unsupported state, definitions, events or peers receive a clear refusal
before mutation. Do not convert files, load an old decoder, run a mixed-version
daemon or silently fall back. Preserve the rejected state for inspection.

## Advance configured transitions

The accepted D14 loop derives readiness as a pure projection. An explicitly
configured materializer with a signing key and applicable local grant signs the
child-task, review-request or handoff effect. The daemon drives this loop after
replay/ingestion/local action without asking an agent for every next step.

Its logical effect identity binds rule context, trigger, scope, action and target.
Event, retained proof, effect deduplication record and recipient outbox commit
atomically. Only committed effects are sent. A recipient durably records its
inbox entry before acknowledgment; lost acknowledgment causes a retry with the
same identity, not another task.

## Read delivery and execution separately

Restart resumes pending effects and unacknowledged deliveries. Equivalent
threshold witnesses do not mint different logical actions. Readiness,
materialization, delivery, receipt, acceptance, local session claim and observed
process start remain separately visible.

An unavailable materializer blocks its configured transition; other replicas do
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
