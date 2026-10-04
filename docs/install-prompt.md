# Local installation prompt

Status: local onboarding template for the implemented [installer](installation.md).
It requires an independently trusted bootstrap and explicitly selected files.
There is no public release origin or production signing identity in this template.
The [qualification records](release-evidence.md) determine which client and
platform behavior has actually been exercised.

## Public install or update

The self-contained [first-contact prompt](first-contact.md#entry-prompt) uses the
public HTTPS installer and installation guide. It authorizes the verified local
software update, selected user service and agent connection in one request.
Inspect the concrete plans and continue within that authorization using `--yes`.
Reuse existing owned software/state/profile selections and preserve their identity.
Restart an owned running service after code changes and check authenticated
readiness; software activation alone does not replace a process.

Read the installed command help before selecting an adapter. Current source
supports `codex`, `claude`, `pi`, `droid` and portable `shell`; older public bytes
may lack the last two. The shared guide also gives a portable service and scoped
CLI enrollment procedure for those bytes. Agent/model qualification is reported
separately and does not prohibit installing published software.

## New client through `up`

For a fresh selected profile, the implemented [onboarding route](onboarding.md)
creates and preserves the enrolled credential and fixed session. It begins only
after verified software activation; no existing credential/session input is
needed. The installed route passed [macOS launchd and existing-daemon checks](../research/onboarding-qualification.md); native client/model discovery remains separate.

```text
Install this local Locust candidate, then onboard only my selected client:

Trusted bootstrap executable: [absolute path]
Extracted signed candidate directory: [absolute path]
Independent public trust key: [absolute path]
Signed withdrawal registry: [absolute path]
Software prefix: [absolute path]
Daemon data home: [absolute path]
Client: [codex, claude, pi, droid, or shell]
Client profile home: [absolute path]
Workspace: [absolute path]
Service: [launchd, systemd, or none]
Service profile home: [absolute path]
Private log directory: [absolute path]

Read docs/installation.md and docs/onboarding.md in this source checkout.
Verify with the trusted bootstrap and independent key/registry before executing
the candidate. Review install plan and apply its unchanged digest. Use the
installed current/locust --home [daemon data home] up with the explicit --client,
--profile-home, --workspace, --service, --service-profile-home and --log-dir inputs
above. Run --plan first and show the destinations. My authorization covers this
verified software, selected service and client, its enrollment, protected session,
operating skill, bound launcher and scoped MCP registration. Use --yes to apply
these selected changes without a terminal prompt. If inputs exceed these
destinations or permissions, stop for my decision.

With service none, require an existing daemon. Otherwise wait for authenticated
API readiness or interruption; use an explicit --wait-ms only if I request a
maximum wait. If interrupted or setup is declined, preserve its journal and
retry the same selections without recreating its identity. Do not change client
approval/provider settings, expose secrets, grant work permission or create/join
a goal. Start a fresh client chat and report configuration, daemon API readiness
and actual client discovery separately. Report the installed source/manifest,
profile and launcher paths, preserved identity, and exact stop/remove commands.
```

The selected profile has one fixed Locust session shared by its native chats.
Software removal retains its onboarding journal and protected identity files.
For an already-running daemon, `agent add CLIENT` supports the same explicit
profile/workspace review without managing its service.

## Existing explicit credential and session

Replace every bracketed input before pasting. A credential and session belong to
one dedicated client profile; obtain them through the daemon's explicit owner
enrollment/session commands. Do not paste their contents into a conversation.
For a fresh daemon, software and service installation can happen first, followed
by enrollment and the client-setup part once those protected files exist.

```text
Install this local Locust candidate and configure my selected dedicated client
profile. Use these exact inputs:

Trusted bootstrap executable: [absolute path]
Extracted signed candidate directory: [absolute path]
Independent public trust key: [absolute path]
Signed withdrawal registry: [absolute path]
Software prefix: [absolute path]
Daemon data home: [absolute path]
Client: [codex, claude, pi, droid, or shell]
Client profile home: [absolute path]
Workspace: [absolute path]
Service: [launchd, systemd, or none]
Private log directory: [absolute path]
Existing principal credential file: [absolute path, or pending enrollment]
Existing session secret file: [absolute path, or pending enrollment]

Read the local docs/installation.md shipped with this source checkout. Identify
the host, client version, current permissions and each selected destination.
Verify the candidate with the trusted bootstrap's package verify command and
the independently supplied key and signed withdrawal registry. Do not execute
the candidate before that verification. Treat a missing input, invalid signature,
withdrawn candidate, unsupported host or ownership conflict as an explicit block.

Use install plan, service plan and setup plan as applicable. Show their concrete
paths and changes. My authorization covers installing this verified candidate,
the selected user service, its operating skill, bound CLI launcher and one scoped MCP registration
at the listed destinations. Apply each unchanged plan using its returned digest.
If a plan changes, inspect the fresh plan; stop for my decision only if it exceeds
these destinations or permissions. Do not replace another tool's files or alter
client approval, sandbox, authentication or organization policy.

If a user service was selected, start it and independently verify the daemon
with doctor. If service is none, give the exact foreground daemon command and
report whether that process has actually been started. If principal/session
files are pending, explain the required enrollment and session choices without
inventing grants or reading secret bytes into model-visible output, and finish
setup after those choices are supplied.

Restart or refresh the selected client as its interface permits. Report these
separately: verified software installed; daemon API healthy; operating skill
discovered; Locust tools discovered; harmless locust_status roundtrip completed.
Do not infer client readiness from configuration files. Preserve the client's
permission denials and state the action it requires. Do not create/join a goal,
share workspace files or execute an assignment under setup authorization alone.

Give the installed version, full source commit and manifest hash, the service and
profile paths, the bound launcher path, the observed readiness results, and exact stop/remove commands.
Retain my daemon identity and data during software/service/configuration removal.
```

This prompt composes separate reviewed operations. It does not claim an atomic
transaction across software activation, the OS service manager and a client's
own profile writer. Close clients that can rewrite the selected profile during
setup. A reload can require a new conversation; it does not transfer execution
authorization or make multiple conversations independent Locust sessions.
