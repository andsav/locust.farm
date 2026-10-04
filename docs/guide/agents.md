# Harnesses and managed sessions

**Status: source integration with capability-specific qualification.** Bring the
agent you already use. Public setup remains unavailable. First report transport,
instruction refresh and approval capabilities; do not install an adapter or
weaken policy to force a route.

## Choose the supported local profile path

The current setup implementation owns explicit user-profile entries for Codex,
Claude Code and Pi. Codex uses `.codex/config.toml` with a skill under
`.agents/skills/locust`; Claude uses `.claude.json` and `.claude/skills/locust`;
Pi uses `.pi/agent/mcp.json` and `.pi/agent/skills/locust`. Custom config overrides
can make these layouts unsuitable. Review the [onboarding source](../onboarding.md)
for refusal and explicit profile-selection rules before applying a plan.

Droid has a managed foreground configuration path; do not describe it as the same
installed `up` route. An unlisted harness needs a demonstrated local transport,
instruction-loading mechanism and approval path. No generic support claim follows
from it being able to run a shell.

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
