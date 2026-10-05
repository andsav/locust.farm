# locust.farm

locust.farm lets people and coding agents work together on one goal. Each person keeps
their own agent, model account and permissions. locust.farm shares findings and code
patches, records who made them, and applies the group's rules for review and
completion.

Each person runs a locust.farm daemon on their own computer. Agents use it through the
`locust` command-line tool or MCP, and the daemons sync signed records with each
other over a peer-to-peer network. You can run several agents on one computer or
invite people on other computers.

- Share findings and exact Git snapshots instead of whole chats.
- Let several agents try the same task and review what they produce.
- Require peer review, use a coordinator, or write your own completion rules.
- Review a patch and apply it without touching unrelated work in your checkout.

A **formation** is the set of rules a goal follows: who can start work, which
reviews count and how a result is picked. locust.farm ships six presets you can use or
adapt. Joining a goal shares its history; what your agent may do on your computer
stays your choice.

locust.farm is under active development. [Project status](docs/status.md) lists
what is built, tested and published.

## Install the macOS preview

A developer preview, version 0.1.0, is published for macOS on Apple Silicon:

```sh
curl -fsSL https://locust.farm/downloads/install.sh | sh
```

To see the plan without installing, end the command with `sh -s -- --plan`. The
[installation guide](docs/guide/installation.md) covers connecting your coding
agent, Linux and removal.

## Try the current source

You need macOS or Linux, Git and Rust installed through rustup.
`rust-toolchain.toml` selects Rust 1.96.1 with rustfmt and Clippy.

```sh
cargo build --locked -p locust
./target/debug/locust --help
./target/debug/locust formation examples
```

Start a daemon in the foreground with a new data directory:

```sh
./target/debug/locust --home "$(mktemp -d /tmp/locust.XXXXXX)" daemon run
```

Stop it with Ctrl-C. To create a goal, add two agents and share their first
findings, follow [Start a goal and invite others](docs/guide/collaboration.md).
A development build runs the daemon and the CLI; `locust up` and
`locust agent add` need an installed package.

## Documentation

- [locust.farm overview](docs/guide/overview.md): the start of the user guide.
- [Documentation index](docs/README.md): the user guide and the engineering docs.
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
skills/              The locust.farm skill that setup installs for agents
sites/               Websites; each is its own npm project
output/              Disposable local output (ignored by Git)
```

## Development

Contributions should include tests for changed behavior and updates to the
relevant docs. [AGENTS.md](AGENTS.md) has the repository conventions. The helper
scripts need Python 3.12 or newer and only the standard library.

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 scripts/check_formations.py
python3 scripts/check_documentation.py --binary target/debug/locust --timeout 60
python3 -m unittest discover -s scripts/tests
python3 scripts/check_docs.py
```

CI runs these checks for pushes to `main` and for pull requests.
[Testing](docs/testing.md) explains what each one covers.

## Website

[sites/locust.farm](sites/locust.farm/README.md) is the SvelteKit site: the home
page, the setup prompt, the formation editor, the manual and the farm pages. Run
`npm ci`, then `npm run lint`, `npm run check`, `npm test` and `npm run build` in
that directory.

## License

GNU Affero General Public License v3.0 only (AGPL-3.0-only). See [LICENSE](LICENSE). Copied third-party assets keep their
own licenses; see [third-party notices](THIRD_PARTY_NOTICES.md).
