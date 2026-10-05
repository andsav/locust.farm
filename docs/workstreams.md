# Workspace architecture

Locust is a Rust workspace with one collaboration contract and a local daemon
that connects its components. The [architecture guide](guide/architecture.md)
explains the responsibilities visible to users.

| Crate | Responsibility |
| --- | --- |
| [locust-proto](../crates/locust-proto/src/lib.rs) | Identifiers, signed events, organization definitions, local API, wire format and storage/engine seams |
| [locust-core](../crates/locust-core/src/lib.rs) | Organization validation, event evaluation, authenticated requests and synchronization state machines |
| [locust-store](../crates/locust-store/src/lib.rs) | SQLite persistence, retained objects, staged transfers and recovery |
| [locust-net](../crates/locust-net/src/lib.rs) | Iroh connections, address lookup, relay configuration and authenticated framing |
| [locust-workspace](../crates/locust-workspace/src/lib.rs) | Explicit snapshot export, materialization and patch application |
| [locust-adapter](../crates/locust-adapter/src/lib.rs) | Client configuration, native-session binding and managed launcher support |
| [locust](../crates/locust/src/main.rs) | Daemon, CLI, stdio MCP, installation and typed client API |
| [locust-farm](../crates/locust-farm/src/lib.rs) | Optional public farm service, signed uploads, stored projections and live event streams |

The library components share `locust-proto`; the application wires them together.
The public farm service is separate from a participant's daemon. The website is
an independent SvelteKit project under
[sites/locust.farm](../sites/locust.farm/README.md).

Keep root dependency declarations and `Cargo.lock` consistent when changing a
crate. This is a contribution convention; Cargo enforces the dependency graph.
Follow [AGENTS.md](../AGENTS.md) for scoped edits, verification and local commits.
