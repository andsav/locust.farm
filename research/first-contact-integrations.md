# First-contact integrations: harness and Polaris sources

Date: 2026-10-03. **Status: source research for the [first-contact experience](../docs/first-contact.md). Nothing here is a Locust qualification result.** No coding client was installed, launched or tested for this note, no profile or configuration was changed, and Polaris was read but not built or run.

The question: what do Claude Code, Codex, pi and Droid document today about loading tools and instructions, and what does Polaris offer as a place to show Locust? The answers decide how one pasted prompt can route an agent by its identity and capabilities without promising support that lane B has not qualified.

## Method

- Official documentation was fetched or searched on 2026-10-03. The pages track their latest versions, so recheck them, and the exact client version, when lane B qualifies a route.
- Locust's own state was read from the repository at `9fbbf74`, the source files and lane logs linked below.
- Polaris was inspected read-only from its source tree in the separate `merak10` repository (`crates/polaris`). That repository is not linked here because it is not part of this checkout.

## Harness capabilities, as documented upstream

| Harness | Tool transport | Instructions | Refresh after a change | Policy and approval notes |
|---|---|---|---|---|
| Claude Code | Local stdio MCP servers; per-run `--mcp-config` registration ([MCP](https://code.claude.com/docs/en/mcp)) | `SKILL.md` skills in `~/.claude/skills/`, project `.claude/skills/` and other scopes ([skills](https://code.claude.com/docs/en/skills)) | Watches skill directories and picks up `SKILL.md` changes in the running session. A top-level skills directory created after start needs `/reload-skills`. Bare mode does not watch. | Lane A measured that Claude expands `${NAME}` inside `--mcp-config` values ([A-R9](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/lane-a-log.md)). Its environment allowlist can keep ambient values from MCP children ([lane B observations](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/lane-b-implementation-log.md)). |
| Codex | Stdio and streamable HTTP MCP in `~/.codex/config.toml` or a trusted project's `.codex/config.toml`; `codex mcp add`, `/mcp` ([MCP](https://learn.chatgpt.com/docs/extend/mcp.md)) | `SKILL.md` skills in `.agents/skills/` from the working directory up to the repository root, and `~/.agents/skills/` ([skills](https://developers.openai.com/codex/skills)) | Detects skill changes automatically; restart if one does not appear. Restart after changing `config.toml`. | Per-server and per-tool approval modes exist. Optional servers have a short startup grace. MCP servers get a minimal environment unless variables are named ([A-R10](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/lane-a-log.md)). |
| pi | Built-in MCP over stdio or streamable HTTP; `pi mcp add`, `pi mcp list`, `/mcp` ([MCP](https://pi.dev/docs/latest/mcp)). Release notes introduce built-in MCP in [v0.99.0](https://github.com/earendil-works/pi/releases/tag/v0.99.0). | Task-matched `SKILL.md` skills ([skills](https://pi.dev/docs/latest/skills)) | Run `/reload` after a server is changed outside the session. | Project `.pi/mcp.json` is read only after project trust. An extension that registers `/mcp` (for example `pi-mcp-adapter`) replaces built-in MCP; built-in MCP can also be disabled in settings. SDK sessions do not load built-in extensions. Default `codemode` exposure hides tools from the model's declared list, and the first prompt does not wait for those servers. |
| Droid | Stdio, HTTP and legacy SSE MCP; `droid mcp add` writes user config; project `.factory/mcp.json` ([MCP](https://docs.factory.com/harness/mcp.md)) | `SKILL.md` skills in `.factory/skills/`, `~/.factory/skills/` and compatible `.agents/skills/` paths; `/skills` shows status ([skills](https://docs.factory.com/harness/skills.md)) | MCP: reloads automatically when an `mcp.json` changes. Skills: start a new session if a new skill is not visible. These are different. | `${NAME}` expands only in env, headers and OAuth fields, not in command or args. Persistent tool approvals are bound to the server's command or URL. Organization `mcpPolicy` can filter a server out. `allowed-tools` is metadata, not a grant. Headless `droid exec` defaults to read-only ([droid exec](https://docs.factory.com/droid-exec/overview.md)). |

### Findings that shape routing

1. **Identity means the harness, not the model.** The same model can run in several harnesses, and pi alone documents interactive, print/JSON, RPC and SDK modes with different behavior. The agent should report its harness, version and mode, and say "unknown" when it cannot tell. That report is a Locust design choice; none of these clients documents an identity API for it.
2. **Native MCP cannot be assumed or ruled out by brand.** Current pi has built-in MCP, so "pi has no MCP" is stale. An older pi, an extension that owns `/mcp`, an SDK session or a disabled built-in still changes the answer. Route by what the session can actually reach.
3. **A visible server is not a working tool.** pi's default exposure and non-blocking startup, Codex's optional-server grace period and Droid's organization policy all mean that a configured entry, or its absence from a listing, does not prove the tool works. Lane B's observation agrees: configuration prepared is not client ready.
4. **Refresh differs per client and per surface.** Claude Code watches skills; Codex detects skills but restarts for configuration; pi reloads with `/reload`; Droid hot-reloads MCP but may need a new session for a skill. Setup must report which refresh step is needed, if any.
5. **Approvals belong to the client and its owner.** Each client has its own approval and policy layer. Locust's setup can name the approvals it needs; it must not edit stored approvals, weaken policy or suggest a bypass.

### pi detail retained from earlier research

Sources read on 2026-10-03 through targeted search and the official MCP page: the [pi site](https://pi.dev/), the [v0.99.0 release](https://github.com/earendil-works/pi/releases/tag/v0.99.0) ("Codemode and MCP"), the [v0.99.2 release](https://github.com/earendil-works/pi/releases/tag/v0.99.2) (codemode servers no longer block the first prompt), the [MCP documentation](https://pi.dev/docs/latest/mcp) and its [repository source](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/mcp.md), and the [skills documentation](https://pi.dev/docs/latest/skills). Legacy `badlogic/pi-mono` paths resolve to `earendil-works/pi`.

- Stdio servers take `command`, `args`, `env` and `cwd`; a relative `cwd` resolves against the session directory. This supports a possible lane B stdio route. It does not settle the Locust executable, arguments, credential path or configuration scope.
- `pi mcp` shell commands do not load extensions, so they see only file-configured servers. A shell check can therefore disagree with what a session that has an MCP extension sees.
- A skill gives workflow context; it is not daemon access, execution consent or readiness.
- Open for lane B: the pi version and mode in scope, configuration precedence in a real session, instruction refresh, daemon authentication under default policy, and interruption and resume.

## Locust state at `9fbbf74`

- The owner made Codex, Claude Code, Factory Droid and pi the required first-release baseline and scoped automatic wake to Merak only; the four clients use active sessions and explicit resume ([release ledger](https://github.com/andsav/locust.farm/blob/ddb2db1e609652a1de766b453d8e86b45b25a1f3/docs/release-evidence.md)). This is required scope, not support evidence.

- The [`locust` binary](../crates/locust/src/main.rs) prints `locust`. There is no daemon, CLI or `locust mcp` bridge yet.
- The [local API contract](../crates/locust-proto/src/api.rs) defines the operations and read types (goal status, members, peers, board, task detail, pending work, wait outcomes, events). These are types, not a running service, and lane A's [log](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/lane-a-log.md) says revision 2 of `api.rs` is still to land.
- [Workspace export and materialization](../crates/locust-workspace/src/lib.rs) are implemented; manifest-bound patches are not.
- Lane B has [per-run client configuration](../crates/locust-adapter/src/config.rs) for Codex and Claude Code, an MCP socket fixture, and authenticated [peer links](../crates/locust-net/src/lib.rs). Codex 0.153.4 parsed the generated configuration in a disposable profile; Claude has component tests only ([implementation log](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/lane-b-implementation-log.md)).
- No installer, install prompt, operating skill or release exists. The [release ledger](https://github.com/andsav/locust.farm/blob/ddb2db1e609652a1de766b453d8e86b45b25a1f3/docs/release-evidence.md) records partial component evidence and no passed gate.

## Polaris

Read-only inspection of `crates/polaris` in `merak10`:

- It is a Tauri 2 desktop app with a static SvelteKit front end. The front end reaches the Rust side through one Tauri command (`tauri_api_fetch`) that routes requests natively and returns typed adapter payloads; DTOs are generated from Rust (`frontend/src/lib/api/transport.ts`, `frontend/src/lib/generated/`).
- `frontend/src/lib/desktopMirror.ts` is a development mirror: a browser tab on the Vite server proxies to the running app's loopback automation bridge. It is a development tool, not a product API.
- The `@33ccff/galaxy` `GalaxyHost` seam is a code-view data source. It is not a Locust collaboration view.
- No Locust connector, bundle, download or stable deep link exists. At `9fbbf74` Locust had no viewer permission either; `2157ca1` added a typed one.

Implication: the native facade is the right shape for Locust. A Rust-side connector can hold the daemon credential, make typed reads and hand the webview only display data. The development mirror, direct reads of Locust's database and fake Merak sessions are not acceptable substitutes. The repository owner has said Polaris is the complete offering that includes Locust and its preferred view; it is built separately, and Locust must also work without it.

## Open questions

- Which versions and modes of the four baseline clients lane B will qualify, and what the minimum capability set for any other harness is.
- Whether a read-only viewer principal belongs in the local API (requested in the [lane C log](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/lane-c-log.md)). Answered at `2157ca1`: the API defines a typed viewer credential; nothing serves it yet.
- How Polaris will run Locust: attach to a user's existing daemon, or start one it manages. Either way there must be only one daemon per state directory.
