# Harnesses and managed sessions

Bring the agent you already use. Source integration provides reviewed local
profile setup and explicit managed sessions. The published terminal preview
installs CLI software; model-visible discovery and complete client journeys
require their own qualification. Respect the selected client's native policy.

## Choose the supported local profile path

The current development source supports `up --client codex`, `claude`, `pi`,
`droid` and `shell`. Codex uses `.codex/config.toml` with a skill under
`.agents/skills/locust`; Claude uses `.claude.json` and `.claude/skills/locust`;
Pi uses `.pi/agent/mcp.json` and `.pi/agent/skills/locust`; Droid uses
`.factory/mcp.json` and `.factory/skills/locust`. The `shell` route creates a
portable bound CLI connection without editing an app-specific profile.

Custom config overrides can make these layouts unsuitable. Review
[onboarding](../onboarding.md) for explicit profile selection and refusal rules.
Inspect the installed `up --help`: the published API-4 preview and current API-5
source have different capabilities. A configured route does not establish
native model discovery or a successful full workflow.

An unlisted harness needs a demonstrated local transport, instruction-loading
mechanism and approval path. Shell access alone does not qualify integration.

## Refresh and prove discovery

Codex configuration changes require a restart; missing skills may also require
one. Claude watches existing skill directories, but a new top-level directory
needs its documented reload. Pi's MCP capability depends on native version/mode
and extension ownership; outside changes require reload. Droid config/skill
refresh and managed organization restrictions are separately checked.

These are client discovery expectations recorded in the [first-contact source](../first-contact.md),
not qualification of a model. Inspect the client's MCP view, load current
instructions and complete a harmless authenticated real tool roundtrip. An
accepted configuration file or scripted test reply is insufficient.

## Keep identity explicit

One enrolled profile binds a principal and a fixed Locust execution session.
Several native chats with that profile can share the binding. Use separate
profiles/bindings when independent executions are intended. A managed native
session ID and a Locust session are separate identifiers.

Foreground launch records intent before spawn. Observed owned-child start,
structured native identity and authenticated tool readiness are distinct lifecycle
stages. Resume must keep the exact native identity and local selections. Recovery
of an uncertain session must not spawn another worker merely because no receipt
was seen. A historical PID is correlation, not process authority.

## Pending work and stop requests

The daemon durably materializes configured ready work without an agent
requesting every transition. Closed-client wake and native launch/resume remain
adapter-specific capabilities requiring local grants and separate qualification.
An active session can inspect durable pending state even if a notification was
lost or duplicated.

Cancellation remains requested until an authorized current execution reports its
observed outcome. Native child exit, remote request or notification does not
assert every descendant stopped. Existing harness tool access is not a Locust
sandbox. See [managed-client evidence](../managed-clients.md) and
[recovery](recovery.md) for the current launcher limits.

## Qualification boundaries

[Client harnesses](../client-qualification.md) distinguish scripted-provider,
real-provider, managed-session and installation experiments. The
[live collaboration records](../../research/collaboration-followups.md) retain
Codex/Merak and Pi/Claude observations against identified API-4 development
binaries, including failures and permissive synthetic workspace policies.
Those records do not qualify all API-5 changes or default interactive approvals.

Droid has a source setup route; full native workspace behavior remains
unqualified. Linux client execution, independent accounts, physical networking
and worker confinement require separate evidence. Use
[availability](status.md) and the exact candidate's matching manual before making
a support claim.
