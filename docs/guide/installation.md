# Local installation and onboarding

**Status: verified macOS Apple Silicon terminal preview published; native agent
and real-model qualification are separate.** The [first-contact prompt](../first-contact.md)
uses the public [installation guide](https://locust.farm/downloads/install.md)
and [verified installer](https://locust.farm/downloads/install.sh) to install or
update existing software. It reviews plans, starts the user daemon and checks a
scoped CLI connection without requiring access to the authenticated website.
The candidate procedure below is also available for explicitly selected local
bundles; it requires an independently trusted executable. A verifier shipped
inside an untrusted download cannot establish that download's authenticity.

## Review the candidate

Select the candidate bundle, independent raw Ed25519 trust key, signed withdrawal
registry and an absolute software prefix. Keep software separate from daemon
identity/database state. The verifier checks exact manifest bytes, declared
payload digests, lengths, modes and native architecture. It refuses altered
payloads, unsupported formats and withdrawn manifests.

The registry's presence does not prove it is the publisher's latest. Once an
installation observes its sequence, rollback, changed bytes at the same sequence
and reinstatement of a withdrawn manifest are refused. A disposable test signing
key qualifies only that local test; it is not a production trust root.

Use the canonical [candidate procedure](../installation.md) to inspect its
read-only plan. Review every selected path and retain the plan digest. Applying
rechecks those inputs under a prefix lock; a stale plan requires fresh review.
The signed `manual.tar` payload carries the exact source-identified manual,
contract exports, examples and availability record. Read its `manual.json` and
raw Markdown directly with `tar -tf` / `tar -xOf`; no checkout or extraction is
required. Inspect install status afterward. Installation chooses a verified `current`
software link; it does not edit shell startup files or grant work permission.

Use independently trusted paths for the verifier, bundle, trust key and signed
withdrawal registry. These placeholders are deliberately not download URLs:

```sh
/TRUSTED/locust --json package verify --bundle /CANDIDATE --trust-key /TRUST/public.key --withdrawals /TRUST/withdrawals.json
/TRUSTED/locust --json install plan --prefix /SOFTWARE/locust --bundle /CANDIDATE --trust-key /TRUST/public.key --withdrawals /TRUST/withdrawals.json
/TRUSTED/locust --json install apply --prefix /SOFTWARE/locust --bundle /CANDIDATE --trust-key /TRUST/public.key --withdrawals /TRUST/withdrawals.json --expect-plan PLAN_SHA256
/TRUSTED/locust --json install status --prefix /SOFTWARE/locust
```

Read the plan before substituting its exact digest into `apply`. Reinstall uses
the same reviewed path: unchanged verified content is reused, while withdrawal,
tampering, ownership conflicts or changed plan inputs are refused. This procedure
activates software without erasing state or restarting an existing daemon.

## Select profile, workspace and service

The implemented `up` orchestration selects explicit clients, an existing profile
home and an existing workspace. `--plan` is read-only. Interactive apply shows
reviewable changes; `--yes` deliberately approves them and requires a selected
client. `--plan` and `--yes` cannot be combined. JSON mode does not prompt.

The default service choice is launchd on macOS and systemd on Linux. `none` uses
an existing daemon and creates no service. Software prefix, profile, daemon home,
service profile and logs are separate choices. Unsupported custom client layout
or owned-file collisions stop rather than silently relocating configuration.
Use the [canonical onboarding procedure](../onboarding.md) for exact commands.

Review onboarding with explicit existing profile/workspace directories:

```sh
/SOFTWARE/locust/current/locust --home /DATA/locust up --client codex --profile-home /PROFILE --workspace /WORKSPACE --plan
```

Repeat the same selections without `--plan` in an interactive terminal to review
and apply each change. Deliberate unattended application uses `--yes`; it must
still name the client. Select `claude`, `pi`, `droid` or `shell` for their supported installed routes.
For an already running daemon, add `--service none`. Readiness waits indefinitely
for a selected service unless you supply `--wait-ms N`; interrupting the command
does not erase its recovery journal. On Linux, service source exists but native
acceptance still needs its own evidence.

## Verify four separate observations

1. Installation status identifies verified, non-withdrawn software.
2. Service manager state shows the intended owned process state.
3. An authenticated daemon check confirms the local API answers.
4. A fresh native chat discovers the instructions and completes a harmless real
   tool roundtrip under its unchanged policy.

The first three do not establish the fourth. Service creation can be
asynchronous; a successful manager request is not observed readiness. A configured
skill or MCP server is not proof that the model has loaded it.

## Recover and remove

Retry onboarding with the same explicit selections. Its private journal preserves
enrollment identity and credentials across interruptions instead of duplicating
enrollment after an uncertain reply. Changed secrets or owned configuration
require inspection. Do not erase the journal to force a retry.

Remove owned client configuration and an observed stopped service before removing
software they reference. Review uninstall's plan separately. Modified releases
and unknown files remain; daemon data, credentials, sessions, logs and trust
watermarks are preserved. Software removal is not a data purge.

Software removal is separately reviewable:

```sh
/TRUSTED/locust --json install uninstall-plan --prefix /SOFTWARE/locust
/TRUSTED/locust --json install uninstall --prefix /SOFTWARE/locust --expect-plan PLAN_SHA256
```

[Services and state](operations.md) explains fresh-state selection and unsupported
state refusal. Consult [availability](status.md) for exact platform limits.

## macOS and Linux evidence

The reviewed native candidate evidence is macOS arm64, including launchd cases.
The source renderer also supports Linux x86_64 systemd user units. Native Linux
installation/service acceptance is not established by rendering or synthetic
probes; consult the exact platform's retained ledger before claiming readiness.
Unsupported binary architecture, service domain, user manager or profile layout
must be reported explicitly. Do not translate an unavailable manager into stopped.

## Fresh-state and removal choices

A new current-format state location must be explicitly selected and empty.
Existing identity/database state is not part of software activation and is not
authorized for erasure by install/uninstall. Unsupported state is refused before
mutation; select fresh current state instead of converting or falling back.

For removal, first stop/remove owned client and service bindings through their
reviewed procedures. Then review software uninstall. Preserve changed/unknown
files, state and trust watermarks. There is no implied deletion of learned remote
copies, provider accounts or project workspaces.
