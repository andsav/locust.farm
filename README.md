# Locust

A Rust project. The workspace holds the `locust` binary and one library crate
per workstream behind a shared contract crate, `locust-proto`.

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
a hard task; it is a target, and it lists what works today.

## Development

Install Rust through rustup. `rust-toolchain.toml` pins Rust 1.96.1 and includes
`rustfmt` and `clippy`; rustup installs them when a Cargo command needs them.
The documentation checks use Python 3 with no third-party packages.

```sh
cargo run -p locust
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
python3 -m unittest discover -s scripts/tests
python3 scripts/check_docs.py
```

The binary currently prints `locust`; there is no daemon or CLI yet. The contract
crate has types, encodings, structural checks and golden vectors; the transport,
client-configuration and workspace crates have component behavior with tests (see
the lane logs indexed in `docs/`). New dependencies are added through the root
`Cargo.toml` by the integration owner.

GitHub Actions runs these checks on pushes to `main` and pull requests. The
documentation checker checks Git-tracked Markdown in `docs/` and `research/` for
index coverage, duplicate entries, and broken local link paths. Stage new files
before checking. Untracked drafts, heading anchors, and external URLs are excluded;
use ordinary inline Markdown links for local references.

The [locust.farm](sites/locust.farm/README.md) marketing site is a SvelteKit
project with its own `package.json`. Run `npm install` and `npm run dev` inside
`sites/locust.farm/`. CI does not check it yet.

Keep disposable logs and experiment output in `output/`. Commit useful findings
and supporting evidence under `research/` or `docs/` so they are preserved.

## License

GNU Affero General Public License v3.0 only (AGPL-3.0-only). See [LICENSE](LICENSE).
