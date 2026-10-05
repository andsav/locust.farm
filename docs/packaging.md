# Packaging and releases

Status: building and signing are scripted here. Apple signing, notarization and publishing are not.

[How installation works](installation.md) covers checking and installing a
package. [The preview release record](public-preview-release.md) records the one
published release.

## Build a package

```sh
python3 scripts/build_release.py
```

[build_release.py](../scripts/build_release.py):

- builds for the host only: `aarch64-apple-darwin` on macOS arm64, or
  `x86_64-unknown-linux-gnu` on Linux x86_64;
- uses the compiler pinned in `rust-toolchain.toml` and
  `cargo build --locked --release`;
- builds the committed `HEAD` from a clean copy of the source, so other changes in
  the checkout are left out;
- checks the binary's machine format, executable bit and `--version` output
  against the commit, API and protocol;
- writes `output/release/locust-TARGET-COMMIT-unsigned.tar.gz` and a `.sha256`
  file, where `COMMIT` is 12 hex digits.

It refuses to build when:

- the host has no supported target, or the pinned compiler or target is missing;
- Rust sources, the build script, the skill, `LICENSE` or a manual file have
  uncommitted changes;
- the source changes during the build;
- the binary or `docs/site.json` reports other versions than the source;
- `output/release/` already holds a different file with the same name.

The archive holds four files in this order, with fixed owners, times and modes:

| File | Contents |
| --- | --- |
| `locust` | The binary, mode 0755 |
| `skills/locust/SKILL.md` | The skill that agents read |
| `manual.tar` | The offline manual |
| `manifest.json` | The unsigned manifest |

The archive layout is fixed, but two builds of one commit can still produce
different binaries.

## The manifest

`manifest.json` is JSON with sorted keys, no spaces and one final newline.

| Field | Value |
| --- | --- |
| `format` | `locust-release-v2` |
| `source_commit` | Full commit hash |
| `version` | Package version, such as `0.1.0` |
| `target` | `aarch64-apple-darwin` or `x86_64-unknown-linux-gnu` |
| `machine_format` | `mach-o-arm64` or `elf-x86_64` |
| `toolchain` | Pinned compiler version |
| `api_version`, `protocol_version` | Taken from the source |
| `files` | `path`, `sha256`, `size` and `mode` of each file |

`files` lists exactly `locust` (mode 493, that is 0755), `skills/locust/SKILL.md`
and `manual.tar` (mode 420, that is 0644). Locust refuses unknown fields and any
other set of files. The manifest's SHA-256 identifies the package; the version
alone does not.

## The offline manual

`manual.tar` starts with `manual.json`, which records the commit, the versions
and each file's hash, size and mode. Then it holds:

- `LICENSE` and `docs/site.json`;
- every page and artifact that `docs/site.json` names;
- every file in its `sourceLinks`.

An installed manual is at `PREFIX/current/manual.tar`. Read it without unpacking:

```sh
tar -tf ~/.local/share/locust/current/manual.tar
tar -xOf ~/.local/share/locust/current/manual.tar docs/guide/overview.md
```

## Sign a package

Unpack the archive into a folder, the bundle. Then:

```sh
locust package keygen --secret-key /PATH/TO/signing.key --public-key /PATH/TO/trust.pub
locust package sign --bundle /PATH/TO/BUNDLE --secret-key /PATH/TO/signing.key
locust package sign-withdrawals --registry /PATH/TO/withdrawals.json --secret-key /PATH/TO/signing.key
locust --json package verify --bundle /PATH/TO/BUNDLE --trust-key /PATH/TO/trust.pub --withdrawals /PATH/TO/withdrawals.json
```

- `keygen` writes a new 32-byte secret key (mode 0600) and public key. It refuses
  paths that exist.
- `sign` checks every file against the manifest, then writes `manifest.sig`.
- `sign-withdrawals` writes `withdrawals.json.sig`.
- Both refuse a secret key that is not yours or not mode 0600.

A withdrawal list that withdraws nothing:

```json
{"format":"locust-withdrawals-v1","sequence":1,"withdrawn_manifest_sha256":[]}
```

To withdraw a release, add its manifest SHA-256, raise `sequence`, sign the list
and publish it as `/downloads/withdrawals.json`. Installed copies refuse a lower
sequence or a list that drops an earlier withdrawal.

The publisher key lives on the owner's computer, as the
[preview release record](public-preview-release.md) describes. Only the public
key is published. `install.sh` pins its SHA-256, and installed copies refuse any
other key. No command moves an installed copy to a new key.

## Publish a release

The web server serves `https://locust.farm/downloads/` from
`/var/www/locust.farm/downloads/`:

| Path | Contents |
| --- | --- |
| `install.sh`, `install.md` | The installer and its notes |
| `latest.json` | The current release: identity, hashes and file list |
| `withdrawals.json`, `withdrawals.json.sig` | The current signed withdrawal list |
| `RELEASE_ID/` | One folder per release, never changed after publishing |

A release folder holds the raw `locust` binary, the archive, a DMG, `INSTALL.md`,
`manifest.json`, `manifest.sig`, `trust.pub`, the withdrawal list and its
signature, `release.json` and `SHA256SUMS`. The published archive has nine
files: the builder's four, plus `manifest.sig`, `trust.pub`, `withdrawals.json`,
`withdrawals.json.sig` and `INSTALL.md`. The published binary carries an Apple
Developer ID signature.

Releases are staged outside the public folder. A release folder is checked on the
server, then moved into place. `latest.json` is replaced last, under a lock.
Website deploys and rollbacks never touch `/downloads/`
([site README](../sites/locust.farm/README.md)).

## Not in this repository

No script or record of these release steps is in this repository:

- Apple Developer ID signing of the binary;
- notarization;
- building the DMG;
- building the nine-file archive;
- writing `INSTALL.md`, `latest.json`, `release.json` and `SHA256SUMS`;
- uploading to the server and replacing `latest.json`.

## CI

[release-build.yml](../.github/workflows/release-build.yml) runs only when started
by hand. On macOS 15 and Ubuntu 24.04 it runs formatting, Clippy, the Rust and
Python tests and the docs check, then `build_release.py`. It uploads the unsigned
packages as CI artifacts. It does not sign or publish. No Linux package has been
published.

[test_build_release.py](../scripts/tests/test_build_release.py) tests the builder:

```sh
python3 -m unittest discover -s scripts/tests -p test_build_release.py
```
