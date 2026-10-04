# Linux x86_64 build evidence and stopped emulation attempt

Status on 2026-10-04: **Linux x86_64 release compilation passed in 92.032
seconds**, using the host's native ARM compiler and existing Zig toolchain.
The owner selected build-only Linux scope. No Linux execution, installation,
service or runtime tests were performed for this artifact. The earlier emulated
campaign was stopped and its task-owned container removed.

## Completed cross-build

| Field | Recorded value |
| --- | --- |
| Source commit | `95d986045f4f711527d335012d94f1776f2b4498` |
| Source archive SHA-256 | `eb418e76a8b4472b60c7ac2cfe9ad15e246230bec8982a7251233b0eb317faf3` |
| Build host | `aarch64-apple-darwin`; no Docker or QEMU |
| Target | `x86_64-unknown-linux-gnu`, glibc 2.28 target |
| Tools | Rust 1.96.1, cargo-zigbuild 0.23.0, Zig 0.16.0 |
| Profile / result | Release, eight jobs, exit 0, 92.032 seconds |
| Binary size | 19,795,544 bytes |
| Binary SHA-256 | `c6b71ecd00382173c8bf27b592ee81a0804660d6b4d80a89d9078f2cf59ef2e4` |
| Archive SHA-256 | `1f7d52bd21470d8291221127e74027832cfb4b8603dc5366d475f22788d2d48b` |
| Build log SHA-256 | `6c3daec13a8ef75f80c9a4c8a768bcc5131f76bb565bcb9e8f8796d02af26330` |

The build used a frozen `git archive`, isolated HOME/Cargo/cache/target paths,
an explicit pinned compiler and `LOCUST_BUILD_COMMIT=95d986045f4f`:

```sh
cargo zigbuild --locked --release \
  --target x86_64-unknown-linux-gnu.2.28 \
  --package locust --bin locust --jobs 8
```

Static header inspection and `file` identified an executable, stripped ELF64
x86-64 PIE with interpreter `/lib64/ld-linux-x86-64.so.2`. The binary was not
executed. Source identity, ELF architecture, hashes and archive contents were
checked; these are build checks, not runtime qualification.

The local artifact is
`output/linux-cross-95d986045f4f/locust-x86_64-unknown-linux-gnu-95d986045f4f-build.tar.gz`,
with a `.sha256` sidecar. It contains the executable, Apache-2.0 `LICENSE`,
operating skill and `build-info.json`. This is a cross-build archive, not the
native builder's signed-install manifest format. No signing or publication was
performed. Exact build inputs, log and artifact identity remain beside it.

## Earlier stopped emulation attempt

The stopped attempt asked whether the exact committed source could produce and
run a local Linux x86_64 candidate, then pass the [installation qualification
harness](../scripts/check_installation.py) with disposable test signing. It used
**Docker Desktop on an Apple-Silicon Mac emulating x86_64**, not a physical
x86_64 host or native Linux user session.

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
  Its `service` case was not run on Linux.

## Checks in the earlier emulated attempt

| Check | Observation |
| --- | --- |
| `cargo fmt --all --check` | Passed on the committed checkout. |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Passed, exit 0, in 10m 49s. The log contained jemalloc warnings explicitly identifying QEMU; no Clippy warnings were accepted. |
| `cargo test --locked --workspace` | **Unverified.** No full-suite completion or exit 0 was observed. |
| Emulated committed `scripts/build_release.py` | Stopped at the user's direction before completion; no archive, manifest or ELF result. |
| Installation harness and systemd service | Not run on Linux; outside the current build-only scope. |


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

The emulated release helper was a distinct build attempt with its committed
two-job setting. It was still compiling dependencies when stopped. The
task-owned container was removed; pre-existing Docker images and containers
were left alone. Neither that partial build nor a later cross-build can turn
the incomplete emulated workspace test into a pass.

## Evidence boundary

The cross-build completes the owner's current Linux request. Execution,
installed behavior, systemd, physical x86_64, production signing and
distribution remain unverified, and are not blockers for this build-only scope.
