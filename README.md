# Locust

A Rust project. The initial workspace contains a single binary crate with no
external dependencies.

## Layout

```text
AGENTS.md       Agent workflow and eager-commit rules
Cargo.toml      Rust workspace and shared package settings
crates/locust/  Application crate
docs/           Project documentation, decisions, and implementation plans
research/       Investigations, experiments, sources, and findings
scripts/        Repository checks
output/         Disposable local artifacts (ignored by Git)
```

Both `docs/` and `research/` are tracked in Git. Start with their respective
[documentation](docs/README.md) and [research](research/README.md) indexes.

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

The binary currently prints `locust`; no application behavior is implemented yet.
Add crates and dependencies when a concrete feature needs them.

GitHub Actions runs these checks on pushes to `main` and pull requests. The
documentation checker checks Git-tracked Markdown in `docs/` and `research/` for
index coverage, duplicate entries, and broken local link paths. Stage new files
before checking. Untracked drafts, heading anchors, and external URLs are excluded;
use ordinary inline Markdown links for local references.

Keep disposable logs and experiment output in `output/`. Commit useful findings
and supporting evidence under `research/` or `docs/` so they are preserved.

## License

GNU Affero General Public License v3.0 only (AGPL-3.0-only). See [LICENSE](LICENSE).
