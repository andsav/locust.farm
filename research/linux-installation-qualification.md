# Linux x86_64 installation qualification under Docker emulation

Status on 2026-10-04: **partial**. The committed source passed formatting and
strict workspace Clippy in an emulated x86_64 container. The full workspace
test suite did not complete because repeated Cargo subprocess launches stopped
making progress under this Docker Desktop/QEMU environment. The release build
and installation campaign are in progress; this note does not yet establish a
Linux release artifact or installed behavior.

## Scope and provenance

The question is whether the exact committed source can produce and run a local
Linux x86_64 candidate, then pass the [installation qualification harness](../scripts/check_installation.py)
with disposable test signing. This is **Docker Desktop on an Apple-Silicon Mac
emulating x86_64**, not a physical x86_64 host, a native Linux user session, a
systemd service test, production signing, or a Linux distribution matrix.

- Source commit: `5bb254d97504209c1ee4277e74c1365c2d8620e0`. A complete Git
  bundle advertising that commit as `HEAD` was copied into the container and
  cloned to a clean detached checkout. Bundle SHA-256:
  `c3d414dd6f6f616f4725a5ab7f9c176489993da290df4c73973d36fe0b6457be`.
- Container image: `rust@sha256:a339861ae23e9abb272cea45dfafde21760d2ce6577a70f8a926153677902663`,
  locally inspected as Linux `amd64`, image ID
  `sha256:55c395291aeff1aadcfe3f4cb4238a3ffd2e36388c74a4638febbe7688892fb8`.
  A new task-owned container had no host mounts and no privileged mode. It
  reported `x86_64` for `uname -m` and Rust `1.96.1` with the
  `x86_64-unknown-linux-gnu` target. Python was 3.11.2, Git 2.39.5,
  pkg-config 1.8.1, and GCC 12.2.0. The process executable reported
  `qemu-x86_64` 6.2.0. Pinned rustfmt and Clippy components were installed
  inside that container.
- The qualification harness was copied **separately** from the candidate source
  commit; its exact bytes are tracked in later commit `ce46102`. Its SHA-256
  was checked on both sides:
  `29b88365b33fd533d2a1dfe7373d3a14ac5ff5c43df35eb0c36ba48758558b51`.
  It is a test tool, not an input to the committed [release builder](../scripts/build_release.py).
  Its `service` case explicitly remains unrun on Linux in this campaign.

## Checks observed so far

| Check | Observation |
| --- | --- |
| `cargo fmt --all --check` | Passed on the committed checkout. |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Passed, exit 0, in 10m 49s. The log contained jemalloc warnings explicitly identifying QEMU; no Clippy warnings were accepted. |
| `cargo test --locked --workspace` | **Unverified.** No full-suite completion or exit 0 was observed. |
| Committed `scripts/build_release.py` | In progress; its exact-source, isolated build result is pending. |
| Native candidate installation harness | Not run yet; pending a built candidate and independently selected local bootstrap. |

The first two-job test build was intentionally interrupted after its dependency
cache was populated, to retry with eight jobs using available container CPUs.
The eight-job and four-job runs then stopped making progress after compiling
many crates. After clearing only this container's processes between attempts,
one final two-job retry stopped after `Compiling locust`. In each stalled run,
CPU use dropped near zero and no `rustc` or test child remained. For the final
retry, a read-only `/proc` snapshot showed a parent Cargo/QEMU process and a
child with the same Cargo command line, inherited target/cache lock descriptors,
compiler pipe descriptors, and a socket pair. The parent's spawning thread was
waiting on the socket; no second process was waiting for the Cargo target lock.
The child had no CPU progress. This strongly localizes the observed stall to
subprocess launch before the expected exec, but **does not establish the exact
QEMU, Cargo, kernel, or library cause**. There was no observed container OOM or
resource ceiling. Retained diagnostic logs and the `/proc` snapshot are under
ignored `output/linux-native/`; their important findings are recorded here so
they do not exist only in disposable output.

The release helper is a distinct build attempt and intentionally retains its
committed two-job setting. A successful build would still not retroactively
turn the incomplete workspace test into a pass.

## Remaining evidence boundary

Record the release-builder exit and exact ELF, archive, manifest, toolchain and
runtime-library identities if it completes. Then run the frozen harness only
with a separately selected local bootstrap, report each actual case and its
resource samples, and preserve explicit `not_run` states for systemd, physical
x86_64 and production trust. If this emulator cannot complete the builder or
harness, retain the failure as an environment-limited result and require a
separate x86_64 Linux host for those gates.
