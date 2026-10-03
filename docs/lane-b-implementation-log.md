# Lane B implementation log

Date started: 2026-10-03. **Status: active implementation log. Component checks, real-client behavior and complete collaboration are recorded separately.** Contract requests and cross-review remain in the [lane B log](lane-b-log.md); release qualification remains in the [evidence ledger](release-evidence.md).

## Product objective

The owner assigned lane B the product area of making coordination between agents easier. The motivating example is this project's own development: a person currently assigns lanes, carries findings between chats and writes handoff prompts. Locust should let agents discover their work, exchange attributable reviews, resolve dependencies and retain continuity when sessions end, with the person involved where a decision needs their authority.

The first useful demonstration is a worker submitting a concrete change, a coordinator inspecting it, a review finding reaching the responsible agent, and that finding being resolved and verified without the person relaying each message. Preserve local execution authority and the common CLI/MCP path across clients. A successful connection, emitted notification or completed model turn alone does not establish this workflow.

## How to use this log

For each meaningful implementation slice, record what changed and why, links to code or retained evidence, what was actually verified, useful observations, and the next unresolved boundary. Mark assumptions and proposals explicitly. Keep transient build noise out unless it exposes a repeatable problem. Research that warrants a longer investigation belongs in `research/` with a link here. Never retain credentials, private source or unredacted transcripts.

## 2026-10-03 — First independent client and transport components

**Implementation:** committed as `48cf79b` (peer links) and `7d1a207` (client configuration/fixture). Lane A cross-review, daemon integration and complete task flow are still pending.

- [Per-run MCP configuration](../crates/locust-adapter/src/config.rs) generates one named Codex TOML override or Claude JSON registration. It performs no profile writes and emits no approval, sandbox, network or trust overrides. Alias collision checks use names supplied by the caller; discovering the effective configuration is not implemented here. Credential provisioning and launch/session persistence await the daemon contract.
- [MCP socket probe](../crates/locust-adapter/examples/stdio_probe.rs) provides initialization, tool discovery and a zero-argument read-only fixture tool. The trusted harness selects its socket and I/O timeout. It speaks a dedicated ping/pong fixture protocol, not Locust's daemon API. It can establish whether an MCP process reaches that socket without exposing arbitrary path or command tools.
- [Transport](../crates/locust-net/src/lib.rs) accepts caller-selected Iroh identity, relay/discovery settings and frame admission limits. It exposes authenticated remote endpoint identity and carries protocol messages. [Framing](../crates/locust-net/src/framing.rs) is shared with in-memory tests. Membership, reconciliation and durable acknowledgments remain the daemon's responsibility.

### Observations and evidence boundaries

1. **The installed clients differ from the prior-art test binaries.** Read-only `--version` checks found Codex 0.153.4 and Claude Code 2.1.280. The earlier hcom tests used 0.159.2 and 2.1.283. Qualification must record the actual executable/version used; the earlier tests prove neither these installed clients nor Locust.
2. **Configuration prepared is not client ready.** A registration must still initialize, expose its tools and authenticate to the daemon. Native delivery, idle wake and session recovery have separate gates. Current [Codex MCP documentation](https://learn.chatgpt.com/docs/extend/mcp?surface=cli) describes optional-server startup behavior and tool timeouts; qualify the installed client rather than assuming those defaults apply universally.
3. **Per-run configuration needs argument boundaries.** Insert registration arguments before any existing `--`, preserving option/value pairs. Claude's variadic `--mcp-config` needs a boundary before the positional prompt. Build argv directly; quotes, control characters and shell-looking text must remain literal argument data. [Claude MCP reference](https://code.claude.com/docs/en/mcp).
4. **Profiles and credentials need separate treatment.** Installed Codex help describes profile layering, not isolated state; skipping base configuration still leaves authentication under `CODEX_HOME`. Installed Claude help says `--bare` also changes authentication and hook behavior, and `--strict-mcp-config` discards other MCP configuration. None is an ordinary launch default. Claude's documented environment allowlist can also prevent ambient values from reaching MCP children; the bridge's credential path must be explicit and qualified. [Environment reference](https://code.claude.com/docs/en/env-vars). These are help/documentation observations, not live authentication results.
5. **Codex parsed our actual generated configuration.** The [optional installed-client test](../crates/locust-adapter/tests/codex_config.rs) uses a disposable profile and no credentials/provider calls. It checks exact argument recovery, an unrelated server remaining present and an unchanged configuration file. This verifies configuration parsing/preservation for that invocation, not effective sandbox enforcement or successful model tool use. Claude's generated JSON has component tests but no corresponding real-client run yet.
6. **Iroh's minimal preset is not a promise of no auxiliary networking.** At the pinned source version it disables relay/lookup, but router port mapping is separately configured. The [local endpoint test](../crates/locust-net/src/tests.rs) disables port mapping and optional probes and uses loopback; it establishes same-machine authenticated transport, not separate-network or relay connectivity.
7. **QUIC stream setup depends on the first write.** Opening a bidirectional stream does not make it remotely acceptable until bytes are sent. Waiting for the remote to accept an empty stream before writing can stall setup. The endpoint test sends a frame before awaiting the corresponding receive path.
8. **Cancellation can damage framing unless progress is owned outside the future.** Receive progress survives a canceled read. A canceled write makes that sending half unusable so a later frame cannot follow a partially written one. Send success means local transport acceptance, not remote processing or persistence.

### Reproduction commands

Use the pinned toolchain and separate build directories:

```sh
CARGO_TARGET_DIR=target/lane-b-net cargo test --locked -p locust-net
CARGO_TARGET_DIR=target/lane-b-adapter cargo test --locked -p locust-adapter
CARGO_TARGET_DIR=target/lane-b-adapter cargo test --locked -p locust-adapter --example stdio_probe
LOCUST_CODEX_BIN=/opt/homebrew/bin/codex CARGO_TARGET_DIR=target/lane-b-adapter cargo test --locked -p locust-adapter --test codex_config -- --ignored --nocapture
```

The last command intentionally requires an explicitly selected installed executable and reports its version. Its default ignored status prevents an ordinary crate test run from depending on a local client installation. The MCP fixture also passed a process-level initialize → initialized → list → call → EOF smoke against a local fixture socket, with no diagnostic output on stdout or stderr.

**Verified on macOS arm64 / Rust 1.96.1:** workspace formatting, workspace Clippy with warnings denied, and workspace tests passed on the tree containing these two changes and lane A's `6a0fd23` contract scaffold. The workspace run passed 57 tests (40 protocol, 11 transport, 6 adapter), with the optional installed-client test ignored in that run. Explicitly running the MCP example passed its 9 tests; explicitly running the installed-client test passed 1 test and reported `codex-cli 0.153.4: overlay parsed; unrelated server and profile preserved`. The test-only fixture has caller-selected read/write timeouts; Unix connection establishment is still blocking, so it is not a general production IPC implementation.

### Remaining product work

The first task flow still needs the daemon and bridge, protected claims/session persistence, result inspection and integration with peer reconciliation. Track those interfaces through B-1/B-2/B-3 in the [lane B log](lane-b-log.md). Real-client default-policy and own-account authentication tests, interrupted wait/resume, hooks/wake, two-machine connectivity, alternate relay, packaging and the complete review handoff remain unverified. These component results do not close any release gate.
