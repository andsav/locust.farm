# T1: one binary for three Apple Silicon Macs

Date: 2026-10-03. **Status: identified Apple Silicon candidate passes all 21 local workflow checks; public download and the three-Mac run remain unqualified.** The accepted [T1 sequence](workstreams.md) comes before real-client integration. The owner now requires each machine to download the same published `aarch64-apple-darwin` build through a first-time-user path. This helper prepares the local artifact; it does not publish a release or implement that fetch/verify/start path. Network routes are observations of the run.

## Build identity

From the repository root:

```sh
python3 scripts/build_t1.py
```

The [helper](../scripts/build_t1.py) requires committed Rust/build inputs, uses the exact compiler in `rust-toolchain.toml`, and builds `locust` with `--locked --release --target aarch64-apple-darwin`. The helper permits unrelated documentation and website edits at the input check, but the binary must still report a clean commit; the current build script marks any tracked edit dirty. It does not install a missing target. It verifies a thin arm64 Mach-O executable and requires `locust --version` to print the package version and at least seven hexadecimal characters matching the source commit. The integrated CLI implements that version contract. A dirty or unknown embedded commit is refused; commit tracked changes before building if the binary would otherwise mark itself dirty.

After building and checking identity, the helper writes `output/t1/<full-commit>/locust`, `SHA256SUMS` and `metadata.json`. It hashes the copied bytes, records compiler/target/version/commit and checks source identity again. Existing identical bundles can be reused; differing or incomplete output is refused. The metadata explicitly says `qualification: not_run`: build identity does not establish daemon behavior, signing, notarization or a successful installation.

## Prepared candidate

The pinned release build succeeded for source commit `3422c7b51948a409481cf2cd9df1cc3f3a1b4dd1`:

- Version: `locust 0.1.0 (3422c7b51948) api 0 protocol 0`.
- Target/compiler: `aarch64-apple-darwin`, Rust 1.96.1.
- SHA-256: `299aafb3c473d7d1051317a636640dfd8c68a52f0d2fe3317cf685b6d56ffbcf`.
- Local bundle: `output/t1/3422c7b51948a409481cf2cd9df1cc3f3a1b4dd1/` with `locust`, `SHA256SUMS` and build-only `metadata.json`.

The checksum and embedded identity were verified independently after the helper completed. The executable has an ad hoc linker signature, without a Developer ID signature or notarization. Its metadata's `qualification: not_run` describes the build helper's scope; subsequent runtime checks are recorded separately in the [evidence ledger](release-evidence.md).

The proposed public destination is a separate release-only repository, `andsav/locust-releases`, with tag `v0.1.0-t1.3422c7b`. It has not been created or approved. This keeps source-repository visibility unchanged. The following download/start command is prepared for that destination and will fail until publication; the owner must choose or approve the destination first:

```sh
(
  set -eu
  umask 077
  candidate="$HOME/.local/share/locust/t1/3422c7b51948"
  mkdir -p "$candidate"
  cd "$candidate"
  curl --fail --location --proto '=https' --tlsv1.2 \
    'https://github.com/andsav/locust-releases/releases/download/v0.1.0-t1.3422c7b/locust' \
    --output locust.download
  printf '%s  %s\n' \
    '299aafb3c473d7d1051317a636640dfd8c68a52f0d2fe3317cf685b6d56ffbcf' \
    'locust.download' | shasum -a 256 -c -
  chmod 700 locust.download
  mv -f locust.download locust
  printf '%s  %s\n' \
    '299aafb3c473d7d1051317a636640dfd8c68a52f0d2fe3317cf685b6d56ffbcf' \
    'locust' > SHA256SUMS
  ./locust --version
  exec ./locust --home "$HOME/.locust-t1" daemon run
)
```

The digest is pinned in the command rather than fetched from a peer. Publication still needs a fresh public fetch/check before this becomes a verified first-contact path. Once downloaded, continue with the [three-Mac CLI sequence](t1-run.md).

## Publish, download and compare

The owner's [updated T1 direction](workstreams.md) requires a published pre-release and a first-run command or entry prompt that downloads, verifies and starts it on each Mac. Publishing requires the owner's explicit instruction. The destination above and its fetch/verify/start command are prepared but unapproved and untested over public download; a local bundle or a file copied between Macs does not satisfy this first-contact test.

Once that path is implemented and the release is published, compare the downloaded artifact from a second terminal on each Mac:

```sh
cd "$HOME/.local/share/locust/t1/3422c7b51948"
shasum -a 256 -c SHA256SUMS
./locust --version
uname -m
sw_vers -productVersion
```

Record the same commit and SHA-256 on all three machines before running the [T1 CLI sequence](workstreams.md). Retain event identifiers, observed routes, restart/sleep results and every failure in the [evidence ledger](release-evidence.md). The build helper does not run that sequence or attest to its outcomes.

## Verification and current boundary

The helper's tests cover dirty/untracked input rejection, allowed unrelated edits, target/compiler checks, version/commit mismatch, source changes during build, exact copy/hash evidence and collision refusal. Fifteen tests passed using a fake builder and tiny Mach-O fixture. A separate actual release compile with the pinned toolchain passed on this Mac; the helper then correctly refused to publish the name-only scaffold with `version_contract_missing`. That earlier check produced no identified T1 bundle. Run them with:

```sh
python3 -m unittest discover -s scripts/tests -p test_build_t1.py
```

The runtime and version contract are committed in `885b372`; publication and the first-run download path are additional T1 requirements. The [release-candidate local workflow](../research/t1-integration-2026-10-03.md) passed all 21 checks against this exact artifact in 12.53 seconds. The same published download must still pass on all three Macs, including sleeping-laptop recovery.
