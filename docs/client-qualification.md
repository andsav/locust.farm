# Client qualification harness

Date: 2026-10-03. **Status: implemented test harness for actual client binaries with a scripted local provider and a fixture MCP server. It does not establish a working Locust daemon, real-model collaboration or release support.** The [evidence ledger](release-evidence.md) tracks those separate requirements; [retained findings](../research/client-qualification.md) record measured behavior.

## What it does

The [Python harness](../scripts/check_clients.py) launches explicitly selected Codex, Claude Code, Factory Droid and Pi executables. The [Rust configuration generator](../crates/locust-adapter/examples/config_probe.rs) prepares each client's real MCP registration. A [stdio fixture](../crates/locust-adapter/examples/stdio_probe.rs) validates protected paths and dummy proof files before calling a local Unix socket. The scripted provider selects fixture tools; it never forwards a request to a model service.

The sequence checks initialization and tool discovery, read and write behavior under default headless policy, then read/write/held wait, SIGINT, explicit native-session resume and a new bridge process. Assertions require protocol events and authenticated fixture receipts. Generated prose, process startup and provider completion are insufficient. A native session identifier must match across resume; Pi's session history must also extend the original file.

The harness uses a private temporary HOME, configuration, workspace and runtime directory, clears inherited authentication and supplies dummy provider keys. Bridge paths are scoped to the MCP child's environment. macOS `sandbox-exec` permits loopback and Unix sockets while denying other network traffic; an actual local-connect/external-denial preflight is required before client calls. Other platforms currently report `not_run` rather than launch unguarded clients. These controls isolate the experiment; they are not Locust's future worker sandbox.

## Run

From the repository root, build the test executables with the pinned compiler:

```sh
CARGO_TARGET_DIR=target/lane-b-fixture cargo build --locked -p locust-adapter --examples
python3 -m unittest discover -s scripts/tests
python3 scripts/check_clients.py \
  --timeout-ms 30000 \
  --output output/client-qualification \
  --config-probe "$PWD/target/lane-b-fixture/debug/examples/config_probe" \
  --stdio-probe "$PWD/target/lane-b-fixture/debug/examples/stdio_probe" \
  --codex /ABSOLUTE/PATH/TO/codex \
  --claude-code /ABSOLUTE/PATH/TO/claude \
  --factory-droid /ABSOLUTE/PATH/TO/droid \
  --pi /ABSOLUTE/PATH/TO/pi
```

Replace executable placeholders with actual paths. Omitted or missing clients get explicit `not_run` rows. The harness does not install clients or authenticate accounts. `--timeout-ms` is an operator-selected budget per operation and cleanup, not a global run limit; cleanup can add intervals. Node must be resolvable from the isolated PATH when the selected Pi executable requires it.

Each run writes a private JSON report and protocol logs under the output directory. Existing reports are preserved by hash before replacement. Temporary client profiles and dummy proof files are removed after each client; process groups and identified detached fixture children are cleaned up. Keep local raw output private and retain reviewed, redacted evidence under `research/`. A report records versions, executable hashes, invocation/policy, provider mode, individual assertion outcomes and cleanup state. An overall zero exit means no assertion failed; it can still contain required scenarios marked `not_run` and does not mean all release gates passed.

## Interpret the policy and recovery checks

- **Default headless:** measures the client's noninteractive default without a permission bypass. A denied call is recorded as a denial and `not_run` for successful execution; interactive approval was not exercised.
- **Permissive lifecycle:** Codex uses `-a never -s danger-full-access`, Claude uses `--dangerously-skip-permissions`, and Droid uses `--skip-permissions-unsafe` inside the isolated experiment. These opt-ins are never emitted by the production configuration adapter. Pi retains its default policy with no permission extension installed and is labeled accordingly.
- **Droid backend fixture:** native resume first queries Factory session storage. The harness routes both Factory API URL settings to loopback and explicitly returns a recorded 404 to allow local-session fallback. This does not establish account authentication or cloud session retrieval.
- **Claude isolation opt-in:** `--bare` changes hooks and authentication/profile discovery. Both Claude scenarios record it; their results cannot establish ordinary interactive hook behavior.
- **Interruption:** the client must reach the held wait, receive SIGINT, exit before forced leader cleanup and leave no live owned processes after cleanup. The report records forced cleanup separately. The fixture handles requests sequentially, so this does not prove MCP cancellation-notification handling.
- **Resume:** the same client-native session must perform a fresh successful read through a new bridge process. This is client continuation and bridge restart, not durable daemon recovery or protection against duplicate task execution.

Actual daemon authentication/task flow, interactive approval, own-account sign-in, real models, active-session delivery and packaged installation remain separate `not_run` assertions. Automatic wake is restricted to Merak and is outside this harness.
