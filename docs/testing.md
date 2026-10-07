# Testing

Status: built. Tests with real coding agents run on macOS only.

## Checks for every change

For Rust changes, run the [AGENTS.md](../AGENTS.md) checks, then the formation,
recipe and Python checks:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked -p locust
python3 scripts/check_formations.py
python3 scripts/check_documentation.py --binary target/debug/locust --timeout 60
python3 -m unittest discover -s scripts/tests
```

For documentation, run `python3 scripts/check_docs.py`. It checks indexes and
local links in `docs/` and `research/`.

For the website, run these in `sites/locust.farm/`: `npm run lint`,
`npm run check`, `npm test` and `npm run build`. `npm run test:e2e` builds the
site and runs the Playwright browser tests; CI does not run it.

Use Python 3.11 or newer, as CI does. `check_tla.py`,
`check_collaboration_acceptance.py` and `check_shared_context_models.py` need
3.12.

| Workflow | Runs on | What it runs |
| --- | --- | --- |
| [ci.yml](../.github/workflows/ci.yml) | Push to `main`, pull requests | On macOS 15 and Ubuntu 24.04: formatting, Clippy, Rust tests, `check_formations.py`, `check_documentation.py`, Python helper tests, `check_docs.py` |
| [site.yml](../.github/workflows/site.yml) | Push to `main`, pull requests | On Ubuntu with Node 24.14.1: `npm ci`, lint, check, test, build |
| [release-build.yml](../.github/workflows/release-build.yml) | Started by hand | On both platforms: the Rust, Python and link checks, then `build_release.py`; uploads the unsigned package |
| [tla-bootstrap.yml](../.github/workflows/tla-bootstrap.yml) | By hand, or when TLA files change | On Ubuntu: runner tests and the fixture models |

No workflow signs, publishes or deploys anything.

## Executable guide recipes

[check_documentation.py](../scripts/check_documentation.py) runs the shell recipes
in `docs/guide/*.md` against the binary given with `--binary`.

- A recipe is a closed, non-empty `bash` fence whose first line is
  `# locust-doc-test: NAME`. `NAME` uses `a-z`, `0-9` and `-` and is unique in
  its page. The marker may not appear anywhere else in a guide page.
- Each recipe runs with `bash -c` in a new temporary directory. The script removes
  every `LOCUST_*` variable, then sets `LOCUST_BIN` (the binary) and
  `LOCUST_DOC_DIR` (the directory).
- The recipes set `LOCUST_RELAY=none LOCUST_LOOKUP=none LOCUST_BIND=127.0.0.1:0`.
  They need `python3`; shared workspace recipes use ordinary directories without Git.
- `--timeout` limits each recipe, in seconds. `--output FILE` writes a JSON
  record with hashes, exit codes and output. The run fails if the binary changes.

The recipes live in collaboration.md, apply.md, sharing.md and
formation-authoring.md. Run the script after you change those pages.

## Formation checks

[check_formations.py](../scripts/check_formations.py) compares committed files
with what the binary prints:

- the contracts and schemas in `docs/reference/generated/`;
- the six presets in `examples/formations/`. Each must validate; an extra file
  fails;
- the conformance vectors. Each case in
  `docs/reference/conformance/organization.cases.json` runs through
  `formation validate`; the results must match
  `docs/reference/generated/organization.vectors.json`, which the site's
  TypeScript checks read. A diagnostic code in the Rust source without a case
  fails.

`--binary` defaults to `target/debug/locust`. `--write` rewrites the files
instead. Use it after changing the contract, schema, presets or diagnostics, and
commit the result.

## Formal models

TLA+ models in `research/tla/` check a small part of the goal rules: membership
and rule changes, pinned rules, review counting, selection, attempt sessions and
automatic steps. [check_tla.py](../scripts/check_tla.py) `--bootstrap` downloads
pinned tools into `output/tla/`; `--suite` picks the cases. The models check
small examples exhaustively; they do not check the Rust code. See the
[models README](../research/tla/README.md).

## Tests with real coding agents

These scripts start real coding-agent programs. You pass each program's absolute
path; a program you leave out is reported as `not_run`. Each run uses private
temporary profiles and never copies your own client settings. Scripted runs
replace the model with a local fake provider. Most of them also block other
network traffic with macOS `sandbox-exec`.

Some runs use permission-bypassing modes, such as Claude Code's
`--dangerously-skip-permissions`. They are used only inside these isolated test
runs. locust.farm's setup never writes them into a profile.

| Script | What it runs | What it needs |
| --- | --- | --- |
| [check_clients.py](../scripts/check_clients.py) | Generated MCP config against a stub server: tool discovery, default permissions, wait, interrupt, resume | The `config_probe` and `stdio_probe` examples, `--timeout-ms` |
| [check_t2_clients.py](../scripts/check_t2_clients.py) | A workspace task through a real daemon and MCP, with scripted models | `--locust`, `--config-probe`, `--output`, `--timeout-ms` |
| [check_t2_models.py](../scripts/check_t2_models.py) | The same task with real models, in client pairs (`--pair`) | `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`, `--openai-model`, `--anthropic-model` |
| [check_managed_clients.py](../scripts/check_managed_clients.py) | `locust client run`: readiness, interrupt, daemon restart, resume, cancellation | `--locust`, `--output`, `--timeout-ms` |
| [check_managed_recovery.py](../scripts/check_managed_recovery.py) | Kills the launcher; recovery must report `Unknown` and refuse a second launch | Same as above |
| [check_installed_clients.py](../scripts/check_installed_clients.py) | Installs a test-signed package, runs `up`, then a full task with Codex, Claude Code or pi | `--bootstrap`, `--bundle`, `--output`, `--timeout-ms` |
| [check_collaboration_acceptance.py](../scripts/check_collaboration_acceptance.py) | Codex and Merak (a separate coding agent) with real models: permission stop, frozen workspace proposal, revision, peer review, integration and update | `--merak`, `--codex`, `--model`, `--output` |
| [check_shared_context_models.py](../scripts/check_shared_context_models.py) | Codex and Merak with real models choosing their own locust.farm calls | `--merak`, `--model`, `--output` |

Build the probes with `cargo build --locked -p locust-adapter --examples`. Runs
with real models cost money.

## Install, network and operations checks

| Script | What it checks | Main flags |
| --- | --- | --- |
| [check_installation.py](../scripts/check_installation.py) | Installs a test-signed package: repeat install, refused signatures and plans, launchd, uninstall | `--bootstrap`, `--bundle` |
| [check_onboarding.py](../scripts/check_onboarding.py) | `up` and `agent add` from an installed package, without agent programs | `--bootstrap`, `--bundle`, `--output`, `--timeout-ms`, `--service` |
| [check_operations.py](../scripts/check_operations.py) | Three daemons on one machine: interrupted transfers, shared workspace updates, cancellation, leaving, offline removal; `--workflow workspace` runs the focused two-daemon loop | `--binary`, `--network`, `--workflow` |
| [check_t1.py](../scripts/check_t1.py) | Three daemons on one machine: join, task, review, offline host, restarts | `--binary`, `--network` |
| [check_farm.py](../scripts/check_farm.py) | Two daemons and the farm service: consent, private text kept out, restart, revocation, deletion | `--binary`, `--service-binary`, `--output` |
| [simulate_machines/run.py](../scripts/simulate_machines/run.py) | Daemons on one Mac as simulated machines; `--list` shows the scenarios | `--binary`, `--quick` |
| [check_performance_cost.py](../scripts/check_performance_cost.py) | CLI and MCP time and bytes for two release binaries | `--before`, `--after`, `--samples`, `--output` |
| [check_transport_probe.py](../scripts/check_transport_probe.py) | Two `transport_probe` processes on loopback | `--binary` |

[build_release.py](../scripts/build_release.py) builds an unsigned package from
the committed `HEAD` into `output/release/`. The install checks take that
extracted package as `--bundle` and a locally built binary as `--bootstrap`.
[build_t1.py](../scripts/build_t1.py) builds one Apple Silicon binary from
committed sources and records its identity. `check_farm.py` needs
`cargo build --locked -p locust -p locust-farm` first.

An in-process simulator runs three simulated machines with random faults:
`cargo test --locked -p locust-core --lib node::sim`. Set `LOCUST_SIM_SEEDS=N`
and add `-- --ignored` to run the long `sim_many` test.

### Transport between two machines

The [transport_probe](../crates/locust-net/examples/transport_probe.rs) example
connects two endpoints and exchanges one small message. It joins no goal and
saves nothing.

```sh
cargo build --locked -p locust-net --example transport_probe
python3 scripts/check_transport_probe.py --binary "$PWD/target/debug/examples/transport_probe"
```

To test two machines, build the same commit on both. On machine A:

```sh
target/debug/examples/transport_probe listen --mode direct --timeout-ms 300000
```

Copy A's endpoint ID and a reachable address from its `record=contact` line. On
machine B:

```sh
target/debug/examples/transport_probe connect --mode direct --timeout-ms 60000 --peer A_ENDPOINT_ID --peer-addr A_IP:PORT
```

Both must print `record=result status=success`. For a relay, use
`--mode relay --n0-relays` on both sides and give B `--peer-relay A_RELAY_URL`.

## Recording results

- Copy the binary under test into `output/` first, so a rebuild cannot replace it
  during the run.
- Raw output goes in `output/`, which Git ignores.
- Reviewed results go in `research/`, with redacted JSON in `research/evidence/`.
  Remove credentials, tickets and IP addresses.
- Name the exact binary (version line and SHA-256), and say whether the run used
  scripted or real models.
- Keep failed runs next to later passing runs.
- Add each note to `research/README.md`.
