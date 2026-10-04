# Published macOS terminal preview

Date: 2026-10-04. **Status: published developer preview; production readiness is
not claimed.** The owner authorized publication of the current binary. Locust is
a terminal CLI, local daemon and stdio MCP bridge. The primary installation route
is the public HTTPS shell bootstrap:

```sh
curl -fsSL https://locust.farm/downloads/install.sh | sh
```

To review the verified plan without activating software:

```sh
curl -fsSL https://locust.farm/downloads/install.sh | sh -s -- --plan
```

[Terminal installation instructions](https://locust.farm/downloads/install.md)
describe the selected harness setup. Installation places software at
`$HOME/.local/share/locust` and the CLI link at `$HOME/.local/bin/locust`.
`--prefix` and `--bin-dir` select other absolute destinations. It uses no sudo,
changes no shell startup files and starts no service or client. The person chooses
the harness and workspace with `locust up`; work permissions remain separate.

## Exact artifact

| Field | Published value |
| --- | --- |
| Version | `0.1.0`, developer preview |
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

[Latest metadata](https://locust.farm/downloads/latest.json) identifies the
installer, immutable artifact URLs, hashes and qualification scope. The
[raw CLI](https://locust.farm/downloads/0.1.0-cd65921d8a0f/locust) and
[signed package archive](https://locust.farm/downloads/0.1.0-cd65921d8a0f/locust-0.1.0-cd65921d8a0f-aarch64-apple-darwin.tar.gz)
are public. An optional notarized DMG contains the same CLI and supporting files;
it contains no GUI application. The source-matching manual records the
prepublication availability snapshot; this record supersedes its download status.

## Verification and trust

The [bootstrap](../scripts/install.sh) checks the archive checksum and exact
member list, streams known members into new regular files, and verifies the
downloaded executable's Apple-rooted publisher requirement and trusted timestamp
before executing it. The authenticated verifier then checks the pinned Ed25519
publisher key, exact signed manifest and payload bytes, current signed withdrawal
registry, source identity and version. Activation rechecks the exact plan digest.
Existing foreign CLI links/files and symlinked bin directories are refused.

The dedicated publisher key is retained in the owner's private
`~/.config/locust-release/signing.key` under a mode-0700 directory; the key is
mode 0600. Only its public key and signatures are distributed. The signed registry
starts at sequence 1. Keep its continuity and the [installer's policy
watermark](installation.md) when publishing later releases. The current registry
is served separately from immutable archives so the bootstrap can observe later
withdrawals. HTTPS establishes trust in the shell bootstrap itself.

The pinned source passed formatting, strict workspace Clippy, workspace tests,
Python helper tests and documentation checks. The final signed executable passed
six formation exports/conformance checks, all four executable tutorials,
installation cases and launchd onboarding. Native Codex and Claude workflows
passed with scripted loopback providers: 25/24 passing assertions and 4/5
explicitly unrun assertions, respectively. These do not qualify real-model use or
interactive approvals on these bytes. Native installation/client harnesses use
disposable test signers; production trust is verified separately by the package
check and public curl route.

Unauthenticated HTTPS fetches matched every artifact's exact size and SHA-256,
plus release and checksum metadata. The public shell route was tested in an
explicit disposable prefix: read-only planning, installation, CLI link/hash/version
and repeat installation. The [retained evidence](../research/evidence/public-preview-release-2026-10-04.json)
records these boundaries and identities. Five bootstrap preflight tests protect
path selection and existing files; the Python suite at the installer commit passes
243 tests. The release status/site changes pass lint, type checks, 178 tests with
one explicit skip, and the static build in an isolated staged copy. Concurrent
Farm dashboard work in the shared checkout failed formatting/type checks and was
excluded from this publication and commit. Documentation links pass.

Apple accepted the signed disk image and its stapled ticket validates. On the
qualification host, legacy `spctl` assessments of both the DMG and standalone
executable returned invalid-parameter errors. `syspolicy_check` reported that a
standalone executable has no stapled application ticket; the ticket belongs to
the DMG. No independent successful Gatekeeper assessment is claimed, and no
assessment bypass was installed. The terminal bootstrap uses the explicit Apple
publisher and Locust package verification described above.

Only macOS arm64 is published. The formation editor's exact copied prompt has no
real-agent qualification, and `/start` and other website preview pages remain
authenticated. This publication does not establish a complete public one-prompt
onboarding journey or close the full production release gates.

## Host and promotion

The existing host serves public `/downloads/` files from
`/var/www/locust.farm/downloads/`, separately from the authenticated website's
`current` symlink. Release staging remains outside that public directory. Remote
SHA-256 and exact inventory checks precede an immutable directory promotion;
`latest.json` is promoted last under a publication lock. Website deploys and
rollback preserve binary releases. The owner-authorized public scope is the
download path; the private GitHub repository and website authentication are
unchanged.
