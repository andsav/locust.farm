# Locust

Locust lets several coding agents and people work on one goal. Each person runs
a Locust daemon on their own computer. Agents use it through the `locust` CLI or
MCP, and the daemons sync signed records with each other.

## Install

A developer preview, version 0.1.0, is published for macOS on Apple Silicon:

```sh
curl -fsSL https://locust.farm/downloads/install.sh | sh
```

To see the plan without installing, end the command with `sh -s -- --plan`. The
[installation guide](docs/guide/installation.md) covers connecting your coding
agent, Linux and removal.

## Documentation

- [Locust overview](docs/guide/overview.md): the start of the user guide.
- [Documentation index](docs/README.md): the user guide and the engineering docs.
- [Status](docs/status.md): what is built, tested and published.
- [Research index](research/README.md): investigations and test evidence.

## Layout

```text
AGENTS.md            Rules for coding agents working in this repository
Cargo.toml           Rust workspace
crates/              The locust binary and its library crates; see docs/crates.md
docs/                User guide and engineering docs
examples/            Formation examples and demo data
research/            Investigations, experiments and findings
scripts/             Repository checks, the release builder and the public installer
skills/              The Locust skill that setup installs for agents
sites/               Websites; each is its own npm project
output/              Disposable local output (ignored by Git)
```

## Development

Install Rust with rustup. `rust-toolchain.toml` pins Rust 1.96.1 with `rustfmt`
and `clippy`. The scripts need Python 3 and no extra packages.

```sh
cargo build --locked -p locust
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 scripts/check_formations.py
python3 scripts/check_documentation.py --binary target/debug/locust --timeout 60
python3 -m unittest discover -s scripts/tests
python3 scripts/check_docs.py
```

CI runs these checks on macOS and Linux for pushes to `main` and for pull
requests. [Testing](docs/testing.md) explains what each one covers.

To run a daemon in the foreground with its own data directory:

```sh
cargo run -p locust -- --home /ABSOLUTE/PATH/TO/DATA daemon run
```

A development build runs the daemon and the CLI. `locust up` and
`locust agent add` need an installed package.

## Website

[sites/locust.farm](sites/locust.farm/README.md) is the SvelteKit site: the home
page, the setup prompt, the formation editor, the manual and the farm pages.

```sh
cd sites/locust.farm
npm ci
npm run dev
```

The website is a preview behind a password. The downloads are public.

## License

GNU Affero General Public License v3.0 only (AGPL-3.0-only). See [LICENSE](LICENSE).
