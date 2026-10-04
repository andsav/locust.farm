# Local verified installation

Status: signed package verification and software activation implemented; the
[identified macOS candidate](packaging.md) passed ten local native installation,
upgrade and launchd cases. Client and other-platform evidence is recorded
separately. Publication, production
signing custody, distribution origin and license remain owner decisions. This
procedure starts with an independently trusted Locust executable and an extracted
candidate made by the [native builder](packaging.md). A verifier downloaded with
an untrusted candidate does not establish its authenticity.

The [local installation prompt](install-prompt.md) composes these commands for
an explicitly selected candidate, service and dedicated client profile.

## Trust and software activation

The [package verifier](../crates/locust/src/package.rs) verifies the exact manifest
bytes with a separately selected raw 32-byte Ed25519 public key. The detached
signature is `manifest.sig` (64 bytes). The candidate has exactly two declared
payload paths: `locust` and `skills/locust/SKILL.md`. Their SHA-256 digests, lengths,
Unix modes and binary architecture must match. Symlinks, hardlinks and special
files in the declared payload paths are refused. Verification does not execute
the candidate.

Every verification and install takes an explicit signed withdrawal registry:

```json
{"format":"locust-withdrawals-v1","sequence":1,"withdrawn_manifest_sha256":[]}
```

Its detached signature is `<registry-path>.sig`, signed with the same independent
key. A locally supplied registry is not evidence that it is the publisher's
latest registry. Once an installation has seen a sequence, it refuses rollback,
changed bytes at the same sequence, reinstatement of a previously withdrawn
manifest, and replacement of its trust key. Uninstall retains this watermark.
There is no automatic network update or trust-root migration.

For local qualification only, a participant can explicitly create a disposable
key, sign an inspected candidate and sign its registry. This does not select a
production signing identity:

```sh
/TRUSTED/locust package keygen --secret-key /TEST/signing.key --public-key /TEST/trust.pub
/TRUSTED/locust package sign --bundle /TEST/bundle --secret-key /TEST/signing.key
/TRUSTED/locust package sign-withdrawals --registry /TEST/withdrawals.json --secret-key /TEST/signing.key
/TRUSTED/locust --json package verify --bundle /TEST/bundle --trust-key /TEST/trust.pub --withdrawals /TEST/withdrawals.json
```

The [installer](../crates/locust/src/installation.rs) takes an explicit absolute
software prefix, separate from the daemon's identity/database home. Review the
plan and pass its digest back to apply with the same inputs:

```sh
/TRUSTED/locust --json install plan --prefix /SOFTWARE/locust --bundle /TEST/bundle --trust-key /TEST/trust.pub --withdrawals /TEST/withdrawals.json
/TRUSTED/locust --json install apply --prefix /SOFTWARE/locust --bundle /TEST/bundle --trust-key /TEST/trust.pub --withdrawals /TEST/withdrawals.json --expect-plan PLAN_SHA256
/TRUSTED/locust --json install status --prefix /SOFTWARE/locust
```

Planning is read-only. Apply rechecks the plan under an exclusive prefix lock,
copies into a private staging directory, rehashes the copied bytes and executes
only the verified candidate's `--version` under a cleared environment. Its
reported version, 12-character source prefix, API and protocol must match the
signed manifest. The full source commit remains in the manifest. Installation
currently requires the same API/protocol as the running installer and the native
macOS arm64 or Linux x86_64 target. Semver downgrades require an explicit
`--allow-downgrade` in both plan and apply; a new commit at the same version has
a distinct signed manifest and still requires a fresh plan.

A verified directory lives under `releases/<manifest-sha256>`. An atomic relative
`current` symlink selects it; the executable is `/SOFTWARE/locust/current/locust`.
The installer does not edit shell startup files or the user's PATH. It persists
the signed policy before changing `current`. An interrupted operation can leave
a private `.stage-*` directory or a completed inactive release; it never selects
a partial staged payload. A retry re-verifies a completed release before using
it. Unrecognized staging directories remain for inspection; they are not
recursively erased on a guess. Existing daemons require an explicit service
restart to run new code. Installer file/directory syncs are implemented, but no
power-loss durability qualification is claimed.

## Removal

Stop and remove the selected service and client configuration before removing
software that they reference. Software removal is separately reviewable:

```sh
/TRUSTED/locust --json install uninstall-plan --prefix /SOFTWARE/locust
/TRUSTED/locust --json install uninstall --prefix /SOFTWARE/locust --expect-plan PLAN_SHA256
```

It removes the selected link and unchanged signed software payloads. Modified
releases and unknown files remain and are reported. The private trust state,
daemon identity/database, logs, credentials, sessions and client configuration
are preserved. No data-purge operation is implied by software uninstall.

## Evidence boundaries

The [installation tests](../crates/locust/src/installation/tests.rs) exercise
read-only plans, stale-plan rejection, repeat install, upgrade, downgrade and
registry policy, mutated source bytes, failed executable probe, lock contention
and conservative removal. Their synthetic executable probes do not establish a
real native install. Native package/service/client results must be recorded
separately in the [release evidence ledger](release-evidence.md), with candidate
and harness hashes. A configured CI job is not a completed Linux/macOS run;
same-host installation is not the deferred physical-machine acceptance pass.

## User-session services

The [service renderer](../crates/locust/src/installation/service.rs) supports a
macOS arm64 launchd GUI service, Linux x86_64 systemd user service, or explicit
`none`. The label derives from the selected absolute daemon home. The unit uses
the installed `current/locust`, the explicit daemon home, a private log directory,
and HOME/XDG paths under the selected profile. It never invokes a shell.

```sh
/TRUSTED/locust --json service plan --prefix /SOFTWARE/locust --kind launchd --profile-home /PROFILE --daemon-home /DATA/locust --log-dir /LOGS/locust
/TRUSTED/locust --json service apply --prefix /SOFTWARE/locust --kind launchd --profile-home /PROFILE --daemon-home /DATA/locust --log-dir /LOGS/locust --expect-plan PLAN_SHA256
/TRUSTED/locust --json service start --prefix /SOFTWARE/locust --kind launchd --profile-home /PROFILE --daemon-home /DATA/locust --log-dir /LOGS/locust
/TRUSTED/locust --json service status --prefix /SOFTWARE/locust --kind launchd --profile-home /PROFILE --daemon-home /DATA/locust --log-dir /LOGS/locust
/SOFTWARE/locust/current/locust --home /DATA/locust --owner --json doctor
```

On Linux select `--kind systemd` and the profile whose user manager owns the
configuration. A missing GUI domain or user bus is unavailable, not a stopped
service. `service start` starts an absent service or restarts an already loaded
owned service. The reported manager state is separate from a successful daemon
API roundtrip; run `doctor` with the appropriate scoped credential as well.
`loaded` means launchd knows the job but does not currently report it running.
Native service changes can be asynchronous: a successful manager request can
briefly be followed by an `unavailable` result because the requested running or
stopped state has not been observed. Inspect `service status` until the state
settles, then independently check the API after start. Removal still refuses a
loaded or running service. Do not infer startup failure or completed shutdown
from the request alone.

The [ownership wrapper](../crates/locust/src/installation/service_install.rs)
writes a private intent before creating a nonce-marked unit. It never adopts a
pre-existing unit without that record, even if its bytes match. Retry can finish
an interrupted owned write; edits or an unrelated label collision are refused.
Start checks installed trust under the prefix lock, and control verifies the
loaded unit definition before acting on its label. A failed start retains the
unit and ownership record for inspection and retry.

Use the same selection arguments with `service stop`, then `service remove-plan`
and `service remove --expect-plan PLAN_SHA256`. Removal requires an unchanged
owned unit and an observed stopped/absent service. It preserves daemon data and
logs. No root daemon, system service or login account is created.

## Client skill and MCP setup

The [setup implementation](../crates/locust/src/installation/setup.rs) installs the
signed operating skill and one `locust` stdio server into an explicitly selected
client profile. Supported targets are Codex (`.codex/config.toml` and
`.agents/skills/locust`), Claude Code (`.claude.json` and `.claude/skills/locust`),
and Pi (`.pi/agent/mcp.json` and `.pi/agent/skills/locust`). Those are the clients'
user-profile locations; project/ancestor collisions are checked for the selected
workspace. Managed organization policy remains authoritative. See the official
[Codex MCP](https://developers.openai.com/codex/mcp),
[Codex skills](https://developers.openai.com/codex/skills),
[Claude MCP](https://code.claude.com/docs/en/mcp),
[Claude skills](https://code.claude.com/docs/en/skills),
[Pi MCP](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/mcp.md)
and [Pi skills](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/skills.md)
documentation for the clients' discovery rules.

Enroll a dedicated principal and create its explicit session through existing
owner/session commands. Setup consumes their existing private 32-byte files;
it does not silently enroll, grant execution permission, change client tool
approval/sandbox policy, copy provider authentication or expose secret bytes in
configuration. The MCP definition contains only protected-file paths. This is a
fixed profile/session binding: use a dedicated profile for this session, and do
not treat multiple native conversations sharing that profile as independently
identified Locust sessions. [Managed launch](managed-clients.md) has separate
native-session binding and lifecycle checks.

Close clients that write the selected profile while applying or removing setup.
Review the actual paths and generated Locust registration, then apply its digest:

```sh
/TRUSTED/locust --json setup plan --prefix /SOFTWARE/locust --client codex --profile-home /PROFILE --workspace /WORKSPACE --daemon-home /DATA/locust --credential-file /DATA/locust/agents/worker.credential --session-file /DATA/locust/sessions/worker.secret
/TRUSTED/locust --json setup apply --prefix /SOFTWARE/locust --client codex --profile-home /PROFILE --workspace /WORKSPACE --daemon-home /DATA/locust --credential-file /DATA/locust/agents/worker.credential --session-file /DATA/locust/sessions/worker.secret --expect-plan PLAN_SHA256
```

Unowned name/skill collisions are refused. An interrupted write retains a private
journal; a retry recognizes only the reviewed before/after states and refuses
unrelated modifications. Reapplying can update owned content while preserving
unrelated settings. `setup remove-plan` and `setup remove --expect-plan ...` use
the same selection arguments. Removal restores the exact original configuration
when the installed document is otherwise unchanged, or removes just the owned
entry from a document with unrelated edits. A modified owned entry or skill is
preserved and reported as a conflict. Removal remains available after software
uninstall; credentials and session files remain.

Restart the client so it discovers the registration and skill. Setup reports
`reload_required`, with discovery and API readiness unobserved. Only the actual
client can establish that its policy permits loading the bridge and calling it.
Use `locust_status` in that client, then perform the intended task under the
participant's selected permissions. A written configuration is not proof of
client readiness, and a successful read is not execution authorization.
