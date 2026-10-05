# Published macOS terminal preview

Status: published 2026-10-04 as a developer preview. Not production ready.

This is the record of one release: locust.farm 0.1.0 for macOS on Apple Silicon, API 4.
It holds the CLI, the daemon and the MCP server. The current source is newer
(API 5). Install it with:

```sh
curl -fsSL https://locust.farm/downloads/install.sh | sh
```

To see the plan without installing:

```sh
curl -fsSL https://locust.farm/downloads/install.sh | sh -s -- --plan
```

The installer puts the software in `~/.local/share/locust` and links
`~/.local/bin/locust`. `--prefix` and `--bin-dir` choose other absolute paths. It
uses no sudo, edits no shell startup files and starts no daemon or agent. You
then connect agents with `locust up`, as the
[install notes](https://locust.farm/downloads/install.md) describe.

## Release details

| Field | Published value |
| --- | --- |
| Release | `0.1.0-cd65921d8a0f`, developer preview |
| Binary source | `cd65921d8a0f9c7de3a64a2c63b28a38d557e496` |
| Target | `aarch64-apple-darwin`, macOS Apple Silicon |
| API / protocol | `4` / `4` |
| Signed binary SHA-256 | `e9729960ddd3d3944b8b8b82ecd6bcaf3e86653479ce1a07fdcc6b0389680884` |
| Signed manifest SHA-256 | `f8306c552c362c8d47918159370eeb11ec31c451e5281536d2daef1baf09d0cc` |
| Public key SHA-256 | `ce02bb70439f130406ea1d7febc6ced4c273427fde0a1bbbfe6126abb1b45cd3` |
| Installer source | `6f5b7d47effd8adfcab78ae2dea814d38dcccc26` |
| Installer SHA-256 | `29cf0b0e2cb7612df477d2e89053fc3f0f6346ce07615a2511a43c4a89a38117` |
| Apple publisher | Andrei Savin, team `P2Q3P9R6AT` |
| Notarization | `Accepted`, submission `daf3cbe5-5f98-4db4-9411-99a7e962c083`; DMG ticket stapled and validated |

[latest.json](https://locust.farm/downloads/latest.json) lists every file with
its URL, size and hash. The
[raw binary](https://locust.farm/downloads/0.1.0-cd65921d8a0f/locust) and the
[signed archive](https://locust.farm/downloads/0.1.0-cd65921d8a0f/locust-0.1.0-cd65921d8a0f-aarch64-apple-darwin.tar.gz)
are public. An optional notarized DMG holds the same CLI and files; it is not an
app.

The binary's source commit is not in this repository's history. The installer's
source commit is. The manual inside the package was built before publishing, so
its availability data still calls the software unpublished.

## Verification

The [installer](../scripts/install.sh) checks the archive hash and file list, the
binary hash and its timestamped Apple signature before it runs the binary. The
binary then checks the pinned Ed25519 key, the signed manifest and files, and the
current signed withdrawal list. Apply checks the plan digest again. The installer
refuses a symlinked bin directory or a `locust` that points elsewhere.
[How installation works](installation.md) lists each step.

Before publishing, the signed bytes passed the package and withdrawal checks,
formation exports and the guide's four recipes. Installation, launchd setup, and
Codex and Claude Code workflows with scripted model replies also passed. Real
models and interactive approvals were not tested on these bytes.

After publishing, plain HTTPS downloads matched every file's size and SHA-256.
The public installer ran in a throwaway prefix: plan, install, link, version and
a repeat install. The [evidence file](../research/evidence/public-preview-release-2026-10-04.json)
records the hashes and results.

Apple accepted the DMG, and its stapled ticket validates. The ticket belongs to
the DMG; the standalone binary has none. Local `spctl` checks returned
invalid-parameter errors, so no Gatekeeper pass is claimed. The installer relies
on its own Apple signature check and the package signature instead.

## Keys

The Ed25519 publisher key stays on the owner's computer. Only the public key
and signatures are published.

The withdrawal list starts at sequence 1 and withdraws nothing. The server keeps
the current list at `/downloads/withdrawals.json`, apart from the release folders,
so the installer sees later withdrawals. Later releases must use the same key and
never lower the sequence, because installed copies refuse both. You trust the
installer script itself through HTTPS.

## Hosting

The server serves `/downloads/` from `/var/www/locust.farm/downloads/`, apart
from the password-protected website. Releases are staged outside that folder,
checked by SHA-256 and file list, then moved in. `latest.json` is replaced last,
under a lock. Website deploys and rollbacks keep the downloads.

## Limits

- Only macOS on Apple Silicon is published.
- The website's pages, including `/start`, need a password. The downloads do not.
- The formation editor's copied prompt has not been tried with real agents.
