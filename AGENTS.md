# Locust agent guide

## Scope and layout

Locust is a Rust workspace. Keep the scaffold small and let actual features drive
new dependencies, crate boundaries, and tooling.

- `crates/`: Rust application and library crates.
- `docs/`: Project documentation, accepted decisions, and implementation plans.
- `research/`: Exploratory work, experiments, references, and findings.
- `scripts/`: Small repository checks and development helpers.
- `sites/`: Websites. `sites/locust.farm/` is the SvelteKit marketing site, a
  self-contained npm project with its own `package.json` and `node_modules`.
- `output/`: Ignored, disposable verification output and scratch artifacts.

Keep documentation and research separate and tracked in Git. When research leads
to an accepted decision, record the decision in `docs/` and link back to its
research. Update each directory's README index when adding a document. Clearly
label proposals, implemented behavior, and verified results.

Document important architectural constraints with links to the code or tests that
enforce them. Label unenforced guidance as a convention. Preserve useful findings
in `research/`; do not leave their only copy in ignored `output/` or agent state.

## Implementation and verification

- Inspect Git status and relevant code before editing; preserve unrelated work.
- Keep changes scoped. Prefer straightforward Rust and standard tooling.
- Add dependencies or abstractions only when the current task needs them.
- Use the compiler and components pinned in `rust-toolchain.toml`.
- Never commit secrets, local environment files, or build output. Track `Cargo.lock`.
- For Rust changes, run the following before committing:

  ```sh
  cargo fmt --all --check
  cargo clippy --locked --workspace --all-targets -- -D warnings
  cargo test --locked --workspace
  ```

- For changes under `sites/locust.farm/`, run `npm run lint`, `npm run check`, and
  `npm run build` in that directory. They do not need a Rust rebuild.
- Add meaningful tests for new behavior and bug fixes. Documentation-only changes
  need content and link review, not a Rust rebuild.
- Run `python3 scripts/check_docs.py` when changing documentation. It checks
  tracked Markdown under `docs/` and `research/`, including index coverage and
  local link paths. Stage new documents and their index entries before checking;
  untracked drafts are excluded. Use ordinary inline Markdown links with relative
  paths; heading anchors and external URLs are not validated.
- When changing the checker, run `python3 -m unittest discover -s scripts/tests`.
- Report any failing or unavailable checks accurately; do not claim verification
  that did not happen.

## Eager commits (required)

- Commit each completed, verified logical change promptly. Do not wait for the
  user to ask, and do not accumulate unrelated completed work into one commit.
  This is standing authorization for local `git commit`.
- Stage only the files or hunks belonging to the task, including its docs and
  research. Inspect the staged diff before committing. Preserve unrelated changes.
- Use concise messages such as `feat: ...`, `fix: ...`, `docs: ...`, or `chore: ...`.
- Before ending a task, commit its completed work and report the commit hash and
  verification results. If a blocker prevents verification or committing, state
  the blocker and describe the remaining changes explicitly.
- Commit authorization does not authorize pushing or rewriting Git history.
  Follow any explicit user instruction to leave work uncommitted.
