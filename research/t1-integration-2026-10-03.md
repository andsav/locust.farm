# T1 runtime integration and discovery findings

Date: 2026-10-03 (America/Los_Angeles). **Status: implemented runtime and local integration evidence; no published three-Mac qualification.** This records the takeover of Lane A following the [review](lane-a-review-2026-10-03.md), the runtime now committed in `885b372`, and the workspace exclusion fix in `98dbb9c`. The [lane A log](../docs/lane-a-log.md) maps each review finding to its regression; the [T1 run guide](../docs/t1-run.md) is the operator sequence.

## What is now real

The executable runs `Node<SqliteStore, OsEntropy>` through the authenticated Unix API and the Iroh peer transport. CLI calls found and join goals, exchange sealed text, claim and submit work, inspect and accept results, append notes and rebuild after restart. Stand-in engines are test-only. All local API variants dispatch to implementations; MCP/client launch and the later packaging workflow remain outside this T1 slice.

The transport reports successful stream delivery separately from aborted close and supplies one frame's writable capacity at a time. The receiver waits for the engine's admission decision before reading another prefix. Membership is checked again for requests and outgoing chunks. Payload metadata is checked before promotion and independently for each text view; restart cannot bypass a split-prefix check. Tests cover the reviewed authority, projection, invitation, claim, withdrawal and failed-commit cases.

The integrated source passed `cargo fmt --all --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, and `cargo test --locked --workspace` on macOS 26.4 arm64 / Rust 1.96.1: **391 passed, zero failed, five explicit ignores**. Two ignores require installed coding clients, two require real network environments, and one is the crash-test child entry point invoked by the parent tests. These checks do not establish physical power-loss survival or the complete release matrix.

## First local run: multicast-only discovery failed

The [retained failed run](evidence/t1-local-2026-10-03-failed-discovery.json) identifies the frozen debug executable, public event/endpoint identifiers and observed routes. The harness used `LOCUST_RELAY=none` and `LOCUST_LOOKUP=local` to exercise local discovery. All three members joined, transferred sealed content, completed a task and retained the full history on M3. After M1 stopped and M3 restarted, M2 and M3 did not exchange their notes before the 40-second qualification deadline. All child daemons were reaped.

An independent empty-hints mDNS test reproduced no addressing results. Temporary dependency tracing showed successful registration of one mDNS provider per endpoint and publication of four contact addresses per endpoint, but repeated IPv4 multicast send failures: `No route to host (os error 65)`. Both address books stayed empty. Source inspection showed that Locust registers lookup providers in the right order and the pinned mDNS library advertises by default; its `Mode::Any` uses IPv4 when the IPv4 socket exists. The host reported an `en0` multicast route and group membership. This does not distinguish routing from macOS local-network permission behavior. No host routes, firewall or privacy settings were changed.

This failure remains open for multicast-only operation on this host. It is not evidence that discovery is absent from Locust, and it is not a passing local-network test.

## Default-network run: complete workflow passed

The owner's accepted configuration uses discovery across networks, with both lookup services and n0 relays enabled by default. The [retained default-network run](evidence/t1-local-2026-10-03-default-network.json) ran that configuration through the CLI on three private homes and one frozen debug image. It passed all 21 checks in roughly ten seconds:

- All three principals became members and decrypted the goal title.
- M1 proposed and assigned; M2 explicitly authorized, claimed and submitted; M1 read the actual decrypted result and accepted it; M3 held the entire effective task history.
- M1 stopped; M3 restarted without its live connection cache; M2 and M3 exchanged and decrypted notes through observed relay paths.
- M1 restarted and caught up; each daemon then restarted with stable endpoint/principal identity, accepted task, content, notes and history.
- All eight launched daemon generations stopped and were reaped cleanly.

The exact debug image was SHA-256 `f7eca1f112ff7678a53a7f698479cf69b37871e69d08770766de6842519bb8c3`, reporting `locust 0.1.0 (98956f1728a2-dirty) api 0 protocol 0`. The working tree was not yet committed at that run, and subsequent held-content and directory-retry guards received their own tests. This result therefore qualifies that captured debug executable. The separate release-artifact run below includes those guards.

Initial routes were direct and the coordinator-offline routes were relayed. Route snapshots describe the selected path at the time of observation; they do not independently identify the lookup service that supplied it. A separate Mainline-only component test, with no contact hints and n0 relays, completed an authenticated exchange in 6.41 seconds after an initial no-address retry. The finalized test at `885b372` passed again in 7.37 seconds and observed a selected relay path (73.49 ms RTT snapshot). That isolates a working Mainline path on this host; separate machines and networks remain unqualified.

The harness never reads credential or session-secret bytes. Redaction review of 231 default-run and 270 failed-run transcript records found no raw invitation strings or unredacted secret fields. Tracked summaries omit private local paths. The transient invitation is handed to its intended join command and removed from retained command/output records.

## Exact release-candidate run: all 21 checks passed

The [identified Apple Silicon candidate](../docs/t1-build.md) was built from clean source commit `3422c7b51948a409481cf2cd9df1cc3f3a1b4dd1` using Rust 1.96.1. It reports `locust 0.1.0 (3422c7b51948) api 0 protocol 0`; its SHA-256 is `299aafb3c473d7d1051317a636640dfd8c68a52f0d2fe3317cf685b6d56ffbcf`. The original bundle's hash was checked before and after qualification, and all subprocesses used one independently verified private copy.

The [release run summary](evidence/t1-release-local-2026-10-03.json) and [redacted command transcript](evidence/t1-release-local-2026-10-03.jsonl) record all 21 checks passing in 12.53 seconds with daemon-default networking. Three principals joined; the encrypted task reached acceptance and M3 held the effective history; M2 and restarted M3 exchanged notes with M1 absent; M1 caught up; sequential restarts retained identities, task state and content. Initial selected routes were direct, and coordinator-offline routes were relayed. All eight daemon generations exited cleanly, with no forced cleanup. Temporary homes and the private executable copy were removed.

Redaction review covered all 259 transcript records and the summary: no raw invitation strings, unredacted secret fields or private local paths remained. This is qualification of the exact release executable on three processes on one Mac. It does not establish public download, three physical Macs, OS sleep/wake or signed installation. The immutable build metadata keeps `qualification: not_run` because it describes the build helper's scope; this separate run record supplies runtime evidence.

## Reproduction and boundaries

```sh
python3 scripts/check_t1.py --binary /absolute/path/to/locust
python3 scripts/check_t1.py --binary /absolute/path/to/locust --network local --timeout-seconds 40
CARGO_TARGET_DIR=target/lane-b-probe cargo test --locked -p locust-net mainline_finds_a_peer_by_key_without_contact_hints -- --ignored --nocapture
CARGO_TARGET_DIR=target/lane-b-probe cargo test --locked -p locust-net mdns_finds_a_peer_by_key_without_contact_hints -- --ignored --nocapture
```

The source repository is private and no release existed when inspected. Public binary hosting remains an owner decision, followed by explicit publication authorization. No source push or publication occurred. A passing local three-process workflow does not qualify downloaded bytes on three Macs, laptop sleep/wake, default-profile coding clients, a signed installer, Linux binaries or the full October 4 release.
