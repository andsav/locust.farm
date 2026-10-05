# Command, API and MCP reference

On the website, tables of every command, operation, response, event and error
code follow. `locust contract` prints the same contract as JSON,
without a daemon.

## Versions

The runtime uses API 6, protocol 6, formation schema 2 and store schema 6.
`locust --version` prints the version, API and
protocol.

## Command line basics

Global flags:

- `--home PATH`: the data directory.
- `--owner`: use the owner credential.
- `--as NAME`: with `--owner`, act for the enrolled agent `NAME`.
- `--credential FILE`: use an agent, author or viewer credential.
- `--session FILE`: use an agent's session secret.
- `--json`: print one JSON envelope.
- `--idempotency-key HEX`: a 16-byte key that makes a retry safe.

A daemon command without a credential fails; it never falls back to the owner.
Named commands accept a goal title, a full ID or a unique ID prefix.
`locust call OPERATION JSON` calls any operation; it needs full hex IDs.

With `--json`, the output is `{"ok":true,"result":...}` or
`{"ok":false,"error":{"code":...,"message":...,"details":...}}`.

## Exit codes

- 0: success
- 1: `internal`
- 2: a command line usage error
- 3: `denied`
- 4: `authorization_required`
- 5: `not_found`
- 6: `invalid`
- 7: `conflict`, `claim_held`, `superseded` or `idempotency_mismatch`
- 8: `unavailable`, such as a stopped daemon
- 9: `halted` or `read_only`
- 10: `unsupported_version`
- 11: `corrupted`
- 12: `limit_exceeded`
- 20: `wait` saw no change before its timeout
- 21: `wait` saw no change before its timeout and no peer of the goal is reachable

Codes 20 and 21 are not errors. The JSON output is still `{"ok":true,...}`, with
`"waited":"no_event"` for 20 and `"waited":"disconnected"` for 21.
`call wait` exits the same way. `watch` reports the same outcomes and exits
with 0.

A reader that closes the output early, as in `locust contract | head`, does not
change the exit code: the command ends quietly with the code it earned. If the
output cannot be written for another reason, such as a full disk, standard
error says so and a command that succeeded exits with 1.

## Local API

The CLI and the MCP server reach the daemon through a Unix socket at
`HOME/daemon.sock`, not HTTP. Each frame is a 4-byte little-endian length
followed by postcard bytes. The client sends a hello, then the daemon answers
each request once, matched by `id`. The hello's credential decides the caller for
the whole connection: the owner, an agent or a viewer.

## MCP server

`locust mcp` serves MCP over standard input and output. It needs absolute data
directory and credential paths, usually set through `LOCUST_HOME`,
`LOCUST_CREDENTIAL` and `LOCUST_SESSION`. It refuses the owner credential. It supports MCP versions 2025-11-25, 2025-06-18 and 2025-03-26.

A tool name is `locust_` plus the operation name with `_` for `.`, so
`goal.status` becomes `locust_goal_status`. The generated operation table identifies
which operations are tools.
Not tools: invitations, permission changes, enrollment, grants, `goal.join`,
`goal.invite`, `task.authorize`, `blob.put`, `blob.get`, sessions, `inbox`,
`daemon.stop` and the farm commands.

`tools/list` shows the tools the credential's kind can call at all: every tool
for an agent, the `locust_formation_*` tools for an author, and the read-only
tools outside the formation catalog for a viewer. The daemon still decides each
call.

## Reading context

`context read --goal GOAL --view full --limit N` returns goal context in pages.
The full view includes rules, inputs, task state and pending work; `compact`
keeps counts and news. Both expose workspace authority and the checkout explicitly
bound to this session. Pass the previous page's `next` as `--after`.

A read in a session returns a `ctx:` reference.
`context acknowledge --goal GOAL --receipt REF` marks that content read. Only
complete text counts. The reference works only with the same credential and
session. Any other reference, whether mistyped or read by another session,
answers `not_found`: read context again and acknowledge the reference that
read returns.

`pending --goal GOAL` lists all pending work; `pending page` adds `--limit` and
`--after`.

When publishing a generic finding, name assessed evidence with
`contribution publish --sources '["EVENT"]'`. Workspace composition records its
exact source proposals; those sources do not approve the combined candidate.
`contribution inspect --goal GOAL --contribution EVENT` shows a contribution with
its sources, attempt and task.

## Shared workspace operations

`workspace.head`, `workspace.tree`, `workspace.read`, `workspace.proposals`,
`workspace.proposal` and `workspace.revision` inspect signed authority and verified
content. MCP exposes the corresponding `locust_workspace_*` tools. Received file
bytes are inert. File capture, copies and update journals run through the CLI,
which durably registers exact candidates and recovery descriptors with the daemon.
The daemon never opens a supplied host path.

Bind a session with `checkout.bind_session` (MCP:
`locust_checkout_bind_session`; CLI: `workspace bind --goal GOAL --checkout ID`).
Context and pending responses then include this checkout's exact base and recovery
state. `workspace.operation.show` and `workspace.operations` expose durable handles
and receipts. Publication uses a prepared candidate; integration uses a prepared
expected epoch/head and proposal; completion commits verified file disposition
and the checkout base together. See the [workflow](apply.md).

## Events

Every change to a goal is a signed event. The signature covers:

- the goal and the signer
- the signer's sequence number, its previous event and causal parents
- the membership or rule change it builds on
- the event's type and fields
- the payload hash

The event's time is for display only.

The payload is encrypted with the goal's content key, which changes each time a
member is removed. Headers are signed but not encrypted.
`event show --goal GOAL --event EVENT` shows one event. See
[What leaves your computer](sharing.md#what-leaves-your-computer).
