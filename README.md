# Locust

Locust is a peer-to-peer collaboration protocol and local Rust daemon for people
and coding agents. Participants keep their own harnesses, accounts and execution
controls while sharing goals, immutable contributions and explicit organization
rules. Each goal has a membership/rule administrator; ordinary work and scoped
decisions follow the goal's pinned formation.

The development runtime implements API 3 / protocol 3: offline formation
inspection, private drafts and publication, taskless findings, independent
attempts, exact review/completion, scoped selection and daemon-driven flow. CLI,
MCP, workspace patches and managed-session adapters use the same typed contract.
There is no published installer or public release.

Start with the [reader manual](docs/guide/overview.md), the executable
[two-participant tutorial](docs/guide/collaboration.md), and the
[current verification ledger](docs/formations-status.md). The frozen
[implementation plan](docs/formations-implementation-plan.md)
defines delivery scope. Historical protocol-1 client and transport campaigns do
not qualify the replacement; current native-client, physical-machine and release
boundaries are recorded separately.

## Layout

```text
AGENTS.md       Agent workflow and eager-commit rules
Cargo.toml      Rust workspace and shared package settings
crates/         The locust binary and its library crates; see docs/workstreams.md
docs/           Project documentation, decisions, and implementation plans
research/       Investigations, experiments, sources, and findings
scripts/        Repository checks
sites/          Websites; each is a self-contained npm project
output/         Disposable local artifacts (ignored by Git)
```

Both `docs/` and `research/` are tracked in Git. Start with their respective
[documentation](docs/README.md) and [research](research/README.md) indexes.
The [first-contact contract](docs/first-contact.md) describes the intended first
session, from one prompt pasted into your own agent to agents working together on
a hard task; it is a target. Its dated source-review snapshot predates the runtime
integration; use the release evidence above for current implementation status.

## Development

Install Rust through rustup. `rust-toolchain.toml` pins Rust 1.96.1 and includes
`rustfmt` and `clippy`; rustup installs them when a Cargo command needs them.
The documentation checks use Python 3 with no third-party packages.

```sh
cargo run -p locust -- --help
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 -m unittest discover -s scripts/tests
python3 scripts/check_docs.py
```

The workspace holds the `locust` binary and library crates behind the shared
contract crate, `locust-proto`. Start a foreground daemon with
`cargo run -p locust -- --home /absolute/path/to/a/private/home daemon run`.
The reader tutorial covers enrollment and authenticated commands. New dependencies are
added through the root `Cargo.toml` by the integration owner.

GitHub Actions runs these checks on pushes to `main` and pull requests. The
documentation checker checks Git-tracked Markdown in `docs/` and `research/` for
index coverage, duplicate entries, and broken local link paths. Stage new files
before checking. Untracked drafts, heading anchors, and external URLs are excluded;
use ordinary inline Markdown links for local references.

The [locust.farm](sites/locust.farm/README.md) SvelteKit site includes the homepage,
first-contact guide and versioned manual with raw Markdown, search and generated
references. Run `npm install` and `npm run dev` in that directory. Site CI checks
lint, types, tests, production prerender, required routes and exact downloadable
assets. A successful local build does not claim public deployment.

Keep disposable logs and experiment output in `output/`. Commit useful findings
and supporting evidence under `research/` or `docs/` so they are preserved.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).

## License

GNU Affero General Public License v3.0 only (AGPL-3.0-only). See [LICENSE](LICENSE).
