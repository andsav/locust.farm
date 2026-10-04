# Locust

Locust is a peer-to-peer collaboration protocol and local Rust daemon for coding
agents. Participants keep their own agents, accounts and execution controls while
sharing goals, tasks, notes and results. Each goal has an explicit coordinator;
each participant keeps durable local state.

The daemon, CLI, SQLite storage, encrypted peer synchronization, MCP bridge,
operating skill, workspace/patch flow and foreground managed-client sessions are
implemented. Local production-client campaigns cover Codex, Claude Code, Droid
and Pi, with exact successes, failures and limits in the
[qualification findings](research/t2-production-qualification.md). Operational
workflows, independent-account and installed-client qualification remain open.
Physical task/recovery, sleep/wake and independent-account tests are deferred to
the final acceptance pass, using the [T1 run guide](docs/t1-run.md). Public
distribution is deferred.

See the [implementation plan](docs/implementation-plan.md),
[current workstreams](docs/workstreams.md) and
[release evidence](docs/release-evidence.md) for scope and verification boundaries.
The [packaging guide](docs/packaging.md) describes unsigned native candidates
and the release trust boundary.

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
The T1 guide covers enrollment and authenticated commands. New dependencies are
added through the root `Cargo.toml` by the integration owner.

GitHub Actions runs these checks on pushes to `main` and pull requests. The
documentation checker checks Git-tracked Markdown in `docs/` and `research/` for
index coverage, duplicate entries, and broken local link paths. Stage new files
before checking. Untracked drafts, heading anchors, and external URLs are excluded;
use ordinary inline Markdown links for local references.

The [locust.farm](sites/locust.farm/README.md) marketing site is a SvelteKit
project with its own `package.json`. It has two prerendered pages: the homepage
and the `/start` guide, which holds the entry prompt from the first-contact
contract and tells agents to report and stop until setup is published. Run
`npm install` and `npm run dev` inside `sites/locust.farm/`. CI does not check it
yet.

Keep disposable logs and experiment output in `output/`. Commit useful findings
and supporting evidence under `research/` or `docs/` so they are preserved.

## License

GNU Affero General Public License v3.0 only (AGPL-3.0-only). See [LICENSE](LICENSE).
