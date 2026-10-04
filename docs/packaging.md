# Native release candidate packaging

Date: 2026-10-03. **Status: native candidate builder and CI declarations implemented; one identified macOS arm64 candidate has passed local installation, upgrade and launchd qualification with disposable test signing.** Production signing and publication remain open. This extends the narrower [T1 identified build](t1-build.md); the [release evidence ledger](release-evidence.md) keeps platform, client and physical-machine checks separate.

## Current local macOS candidate

The [retained identity record](../research/evidence/local-candidate-5bb254d-2026-10-03.json)
identifies the exact native candidate used by the
[installation campaign](../research/installation-qualification.md). It includes
T2 and the installed service/profile commands. This is a local candidate, not a
published release or a completed release gate.

| Field | Observed value |
|---|---|
| Source | `5bb254d97504209c1ee4277e74c1365c2d8620e0` |
| Target | `aarch64-apple-darwin` |
| Version | `locust 0.1.0 (5bb254d97504) api 1 protocol 1` |
| Executable size | 13,678,896 bytes |
| Executable SHA-256 | `abe1c0271de5c8fdbd8145d35b6b0932233d02eee7b5957fc99fc3211eac8580` |
| Manifest SHA-256 | `3793145c1aa0aa7aae24e8572d4b60683ff8d5205d1ce26202d4d01843c4d8c7` |
| Unsigned archive SHA-256 | `5f9a51ec62cc4d34cfe8dfdda0cffc945c10caf946d267a19bc3146fb9e413af` |
| Archive filename | `locust-aarch64-apple-darwin-5bb254d97504-unsigned.tar.gz` |

The archive and checksum are retained under local `output/final-native/`, with
the fixed-file extraction under `output/final-bundle/`. Independent checks
confirmed all three archive members, file modes, payload hashes, Mach-O arm64
format and exact embedded version. `codesign` reports a linker-generated ad hoc
signature and no TeamIdentifier; Developer ID signing and notarization were not
performed. Qualification made private copies and used disposable Ed25519 keys;
the archive itself remains unsigned by a publisher. The separately selected
trust key and registry required by the [installer](installation.md) cannot be
inferred from a checksum sidecar.

## Build and source identity

Run [`python3 scripts/build_release.py`](../scripts/build_release.py) on macOS arm64 or Linux x86_64. It uses the exact Rust version in `rust-toolchain.toml` and builds only the native target (`aarch64-apple-darwin` or `x86_64-unknown-linux-gnu`) with `cargo build --locked --release`. The helper builds a verified archive of the captured Git `HEAD`, reusing the committed-blob check in [`build_t1.py`](../scripts/build_t1.py), and refuses dirty Rust/build and packaging inputs. Unrelated checkout changes are excluded from the archive. It checks the resulting Mach-O or ELF architecture, executable bit, exact `locust --version` output, embedded source commit, and source protocol/API constants. The archived operating skill is included. The helper uses isolated home, Cargo home and target directories, a pinned absolute compiler, and two build jobs. It does not claim an independently reproducible binary: host SDK, native libraries, build scripts, and dependency resolution remain relevant.

The output is `output/release/locust-<target>-<12-character-commit>-unsigned.tar.gz`, with a sibling `.sha256` checksum file. The archive has exactly three regular file entries, all at flat relative paths: `locust`, `skills/locust/SKILL.md`, and `manifest.json`. Tar ownership and timestamps, gzip timestamp, entry order, and permissions are fixed. No symlink or absolute path is included. Rebuilding the same committed source can only reuse an existing output name when the candidate bytes match; a different binary under the same name is reported as a collision. `output/` is ignored and disposable.

## Candidate manifest and trust boundary

`manifest.json` is UTF-8 JSON with sorted keys, compact separators, and one trailing newline. The format marker is `locust-release-v1`. Its fields are `format`, full `source_commit`, package `version`, Rust `target`, `machine_format` (`mach-o-arm64` or `elf-x86_64`), pinned `toolchain`, numeric `protocol_version` and `api_version`, and `files`. `files` lists `locust` then `skills/locust/SKILL.md`, each with relative `path`, SHA-256 hex `sha256`, byte `size`, and integer permission `mode` (493 for executable `0755`, 420 for skill `0644`). The archive checksum is separate; the manifest identifies its contained files.

This builder emits **no signing key or signature**. A signed candidate requires a detached `manifest.sig` containing a raw 64-byte Ed25519 signature over the **exact `manifest.json` bytes**, with a separately supplied, explicit 32-byte public trust root. The installer must verify that signature before trusting the manifest or installing any bytes, then check the target, format, paths, sizes, modes, and file hashes. The signed manifest hash is the candidate content identity; package version alone is not an upgrade order. A `.sha256` sidecar checks transport integrity, not release authenticity. The unsigned archive is for inspection and signing; it is not install-trusted. Key custody, distribution license, and release publication remain owner decisions.

## CI and qualification

[`ci.yml`](../.github/workflows/ci.yml) declares pinned-toolchain Rust formatting, Clippy, tests, Python helper tests and documentation checks on both macOS arm64 and Linux x86_64. [`release-build.yml`](../.github/workflows/release-build.yml) is manual only: each native runner repeats those checks, builds an unsigned archive from identified source, and uploads it as a CI artifact for inspection. It has read-only repository permission and no signing or release publication step. These are CI declarations, not a claim that either hosted job has run or passed.

The [focused builder tests](../scripts/tests/test_build_release.py) run with `python3 -m unittest discover -s scripts/tests -p test_build_release.py`. A local native build establishes one host's package creation only. Installed CLI, service lifecycle, repeat install/uninstall, configuration cleanup, physical network and independent-account checks remain separate gates in the [evidence ledger](release-evidence.md).
