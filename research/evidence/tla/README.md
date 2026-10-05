# TLA+ evidence

The [current organization verification record](organization/README.md) names the
bounded organization, attempt-session and durable-effect checks. Their executable
sources and case configurations are under [research/tla](../../tla/README.md).
The model map states the covered subset; newer API fields are not automatically
covered by the earlier modeled baseline.

[Runner fixtures](stage0-fixtures.json) and the
[expected fixture violation](traces/fixture-violation.json) retain the checker
completion/violation behavior. They are reproducible through the current fixture
suite. Source and log hashes identify the original observations; they are not
claims of current arbitrary-size Rust safety or a new hosted CI run.
