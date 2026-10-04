# T1: one binary, starting on two Apple Silicon Macs

Date: 2026-10-03. **Status: the historical protocol-0 Apple Silicon candidate passed all 21 local three-process workflow checks. The two-Mac mixed-build run recorded joining, a decrypted note and task readiness; claim/submission, acceptance, physical restart and sleep/wake remain pending.** Publication remains deferred. The [current T1 direction](workstreams.md) and [run guide](t1-run.md) retain a third-participant extension for later. The prepared public-download command below is historical preparation and has not been executed against a published release.

## Build identity

The identified candidate below preserves the original two-Mac test identity. The independent review found defects in that candidate; a new remediation build needs its own qualification. Preserve existing protocol-0 homes and binaries. To prepare a new candidate from committed source, use **Python 3.11 or newer** from the repository root:

```sh
python3 scripts/build_t1.py
```

The [helper](../scripts/build_t1.py) requires committed Rust/build inputs and compiles a private `git archive` of the captured commit. It checks every exported regular file against the committed Git blob identifier. Hidden `skip-worktree` / `assume-unchanged` edits and checkout writes during compilation cannot enter the archived source; visible dirty build inputs and a changed HEAD still block publication of the local bundle. Unrelated documentation and website edits may remain dirty. Symlinks, submodules and archive substitutions/omissions are refused.

The build uses the exact compiler pinned in the archived `rust-toolchain.toml`, resolved to an absolute executable and used for `cargo build --locked --release --target aarch64-apple-darwin`. Fresh private home, Cargo-home and target directories exclude user build configuration and cached outputs. Cargo config files in the invocation ancestry are refused; inherited compiler wrappers, flags, loader hooks and SDK overrides are removed. The helper records the selected Apple toolchain/SDK, compiler tool paths and hashes, effective environment and Cargo arguments; it rechecks tool hashes after compilation. It does not install a missing target or toolchain. Python below 3.11 produces a structured error naming the requirement.

The helper verifies a thin arm64 Mach-O executable and requires `locust --version` to identify the package version and captured source commit. The archive supplies the existing `LOCUST_BUILD_COMMIT` build-script input; unrelated tracked edits no longer make the archived binary appear dirty. Path remapping removes the temporary build prefix from Rust paths. This is controlled provenance, not a hermetic or reproducible-build guarantee: system SDK/library contents, build scripts and dependency downloads still matter, and no independent byte-for-byte rebuild has been established.

After building and checking identity, the helper writes `output/t1/<full-commit>/locust`, `SHA256SUMS` and `metadata.json`. It hashes the copied bytes, records the verified Git tree, compiler/target/version/commit and build settings, and checks source identity again. Existing identical bundles can be reused; differing or incomplete output is refused. The metadata explicitly says `qualification: not_run`: build identity does not establish daemon behavior, signing, notarization or a successful installation.

## Local candidate

The pinned release build succeeded for source commit `3422c7b51948a409481cf2cd9df1cc3f3a1b4dd1`:

- Version: `locust 0.1.0 (3422c7b51948) api 0 protocol 0`.
- Target/compiler: `aarch64-apple-darwin`, Rust 1.96.1.
- SHA-256: `299aafb3c473d7d1051317a636640dfd8c68a52f0d2fe3317cf685b6d56ffbcf`.
- Local bundle: `output/t1/3422c7b51948a409481cf2cd9df1cc3f3a1b4dd1/` with `locust`, `SHA256SUMS` and build-only `metadata.json`.

The checksum and embedded identity were verified independently after the helper completed. The executable has an ad hoc linker signature, without a Developer ID signature or notarization. Its metadata's `qualification: not_run` describes the build helper's scope; subsequent runtime checks are recorded separately in the [evidence ledger](release-evidence.md).

## Copy to the second Mac

Use AirDrop or a shared folder to copy the local bundle's `locust`, `SHA256SUMS` and `metadata.json` together into a folder on the second Mac. If the bundle arrives as an archive, extract it first. A copy of the run guide may travel with it. Only the binary bundle is shared: leave each Mac's `~/.locust-t1` daemon state on that Mac so each creates a distinct identity.

On each Mac, open Terminal in the bundle folder and verify before starting:

```sh
shasum -a 256 -c SHA256SUMS
chmod u+x ./locust
./locust --version
uname -m
sw_vers -productVersion
```

Both copies must match the version, source commit and SHA-256 above, and `uname -m` must report `arm64`. Keep those results with the run evidence. Set `LOCUST` to that copy's absolute executable path and continue with the [two-Mac CLI sequence](t1-run.md). No public release, Rust installation or independent rebuild on the second Mac is needed.

## Deferred publication draft

The owner has said publication is not needed now. The following is a retained draft, not an action awaiting immediate approval or a prerequisite for local work. The proposed public destination is a separate release-only repository, `andsav/locust-releases`, with tag `v0.1.0-t1.3422c7b`. It has not been created or approved. This keeps source-repository visibility unchanged. The following download/start command is prepared for that destination and will fail until publication; it must not be used until the owner later requests publication and chooses the destination:

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

The digest is pinned in the command rather than fetched from a peer. Publication still needs a fresh public fetch/check before this becomes a verified first-contact path. Once downloaded, continue with the [T1 CLI sequence](t1-run.md).

## Later public-download qualification

The owner has [deferred publication](workstreams.md). The earlier first-contact scenario remains unqualified: a published pre-release and a command that downloads, verifies and starts it. The destination above and its command are unapproved and untested over public download. Local runtime verification can proceed independently; it does not satisfy the deferred public-download test.

Once that path is implemented and the release is published, compare the downloaded artifact from a second terminal on each Mac:

```sh
cd "$HOME/.local/share/locust/t1/3422c7b51948"
shasum -a 256 -c SHA256SUMS
./locust --version
uname -m
sw_vers -productVersion
```

Record the same commit and SHA-256 on every participating machine before running the [T1 CLI sequence](t1-run.md). Retain the number of physical machines and daemon identities, event identifiers, observed routes, restart/sleep results and every failure in the [evidence ledger](release-evidence.md). The build helper does not run that sequence or attest to its outcomes.

## Verification and current boundary

The helper has 25 focused tests covering archive/blob verification, hidden index edits, a checkout race, hostile compiler settings, Cargo configuration, changed compiler bytes, exact copy/hash evidence, Python version errors and collision refusal. An actual minimal Rust fixture also compiled for arm64 with pinned Rust 1.96.1 while its hidden checkout source contained `compile_error!` and inherited compiler/SDK variables were deliberately invalid: the verified committed source and selected toolchain produced the artifact. That fixture establishes the build path, not Locust runtime behavior. The full Locust remediation release build remains a separate verification step. Run the tests with:

```sh
python3 -m unittest discover -s scripts/tests -p test_build_t1.py
```

The runtime and version contract are committed in `885b372`; publication and the public first-run path are deferred. The [release-candidate local workflow](../research/t1-integration-2026-10-03.md) passed all 21 checks against this exact artifact in 12.53 seconds. The [two-Mac findings](../research/t1-m2-smoke-2026-10-03.md) record mixed-build joining, decrypted content and task readiness. Two-Mac execution/acceptance, physical restart, sleeping-laptop recovery and the later third-participant extension remain unqualified. The 21-check completion result remains three processes on one Mac. No public-download qualification is claimed.
