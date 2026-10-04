# T1: one binary for three Apple Silicon Macs

Date: 2026-10-03. **Status: runtime integrated and the local three-process workflow passes; identified release packaging is next. The published three-Mac run remains unqualified.** The accepted [T1 sequence](workstreams.md) comes before real-client integration. The owner now requires each machine to download the same published `aarch64-apple-darwin` build through a first-time-user path. This helper prepares the local artifact; it does not publish a release or implement that fetch/verify/start path. Network routes are observations of the run.

## Build identity

From the repository root:

```sh
python3 scripts/build_t1.py
```

The [helper](../scripts/build_t1.py) requires committed Rust/build inputs, uses the exact compiler in `rust-toolchain.toml`, and builds `locust` with `--locked --release --target aarch64-apple-darwin`. The helper permits unrelated documentation and website edits at the input check, but the binary must still report a clean commit; the current build script marks any tracked edit dirty. It does not install a missing target. It verifies a thin arm64 Mach-O executable and requires `locust --version` to print the package version and at least seven hexadecimal characters matching the source commit. The integrated CLI implements that version contract. A dirty or unknown embedded commit is refused; commit tracked changes before building if the binary would otherwise mark itself dirty.

After building and checking identity, the helper writes `output/t1/<full-commit>/locust`, `SHA256SUMS` and `metadata.json`. It hashes the copied bytes, records compiler/target/version/commit and checks source identity again. Existing identical bundles can be reused; differing or incomplete output is refused. The metadata explicitly says `qualification: not_run`: build identity does not establish daemon behavior, signing, notarization or a successful installation.

## Publish, download and compare

The owner's [updated T1 direction](workstreams.md) requires a published pre-release and a first-run command or entry prompt that downloads, verifies and starts it on each Mac. Publishing requires the owner's explicit instruction. The download location and fetch/verify/start implementation are still pending; a local bundle or a file copied between Macs does not satisfy this first-contact test.

Once that path is implemented and the release is published, compare the downloaded artifact on each Mac:

```sh
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

The runtime and version contract are committed in `885b372`; publication and the first-run download path are additional T1 requirements. The [local workflow](../research/t1-integration-2026-10-03.md) passes with a frozen debug binary. It must repeat against the release artifact, then against the same published download on all three Macs, including sleeping-laptop recovery.
