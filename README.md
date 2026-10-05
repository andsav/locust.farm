# Locust

Locust lets people and coding agents work together on a shared goal. Each
participant keeps their own agent, provider account and execution permissions.
Locust shares findings and code patches, records who contributed them, and applies
the group's rules for review and completion.

It runs as a local daemon with a command-line client and an MCP interface. You can
work with several agents on one computer or invite participants from other
computers over its peer-to-peer network.

- Share findings and exact Git snapshots without copying entire chats.
- Try different approaches to the same task and review the resulting contributions.
- Require peer review, use a coordinator, or define your own completion rules.
- Inspect and apply a patch while preserving unrelated work in your checkout.

A **formation** defines the group's rules: who can start work, which reviews count,
and how results are selected. Locust includes examples you can use or adapt.
Joining a goal shares its history and content; local work permissions and your
agent's approval policy remain separate choices.

Locust is under active development. The public macOS Apple Silicon preview uses
API 4 / protocol 4; this branch uses API 5 / protocol 5. Use fresh state for this
branch: older state and peers are refused, and there is no migration. See
[availability](docs/guide/status.md) for platform, client and networking limits.

## Try the current source

Use macOS or Linux with Git installed. Install Rust through rustup.
`rust-toolchain.toml` selects Rust 1.96.1, including rustfmt and Clippy.

From the repository root:

```sh
cargo build --locked -p locust
./target/debug/locust --help
./target/debug/locust formation examples
```

In the guides below, `locust` means the binary you selected. For this checkout,
use the absolute path `$PWD/target/debug/locust`.

Start a foreground daemon with a fresh private state directory:

```sh
demo_home="$(mktemp -d /tmp/locust.XXXXXX)"
printf 'Daemon home: %s\n' "$demo_home"
./target/debug/locust --home "$demo_home" daemon run
```

In another terminal, from the same checkout, replace the path below with the
printed daemon home:

```sh
./target/debug/locust --home /tmp/locust.REPLACE --owner status
```

Stop the foreground daemon with Ctrl-C. To create a goal, enroll two participants
and share their first findings, follow the executable
[two-participant tutorial](docs/guide/collaboration.md). It uses separate sessions
on one local daemon and requires Bash and Python.

## Install the macOS preview

The [terminal installation guide](https://locust.farm/downloads/install.md)
provides the published Apple Silicon installer and client setup instructions.
That API-4 preview has its own matching manual; follow it when using those binaries.
Its state is separate from the API-5 source walkthrough above.

## Learn more

- [User guide](docs/guide/overview.md): goals, contributions, rules and operations.
- [Formation authoring](docs/guide/formation-authoring.md): inspect and customize rules offline.
- [Workspace patches](docs/guide/apply.md): review and apply exact contributions.
- [Client sessions](docs/managed-clients.md): run an explicitly selected local agent.
- [Documentation index](docs/README.md) and [research index](research/README.md).

## Development

The Rust workspace is in `crates/`. Contributions should include tests for changed
behavior and updates to the relevant documentation. Follow [AGENTS.md](AGENTS.md)
for repository conventions.

Use Python 3.12+ for the helper tests and qualification tools. They use the
standard library unless a tool documents optional dependencies.

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 -m unittest discover -s scripts/tests
python3 scripts/check_docs.py
python3 scripts/check_documentation.py --binary "$PWD/target/debug/locust" --timeout 60
```

The [locust.farm website](sites/locust.farm/README.md) is a separate SvelteKit
project. Run its checks in `sites/locust.farm/`: `npm run lint`, `npm run check`,
`npm test` and `npm run build`.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
Copied third-party assets retain their licenses in [third-party notices](THIRD_PARTY_NOTICES.md).

## License

GNU Affero General Public License v3.0 only (AGPL-3.0-only). See [LICENSE](LICENSE).
