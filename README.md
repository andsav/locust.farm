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
```

Both `docs/` and `research/` are tracked in Git. Start with their respective
[documentation](docs/README.md) and [research](research/README.md) indexes.

## Development

Use a Rust toolchain supporting edition 2024, with `rustfmt` and `clippy` installed.

```sh
cargo run -p locust
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The binary currently prints `locust`; no application behavior is implemented yet.
Add crates and dependencies when a concrete feature needs them.

## License

GNU Affero General Public License v3.0 only (AGPL-3.0-only). See [LICENSE](LICENSE).
