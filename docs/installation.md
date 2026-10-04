# Local verified installation

Status: signed package verification and software activation implemented; native
installation qualification in progress. Publication, production
signing custody, distribution origin and license remain owner decisions. This
procedure starts with an independently trusted Locust executable and an extracted
candidate made by the [native builder](packaging.md). A verifier downloaded with
an untrusted candidate does not establish its authenticity.

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
