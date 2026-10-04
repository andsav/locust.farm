# First contact

Date: 2026-10-03. **Status: experience contract (lane C). It describes a target; most steps are not available yet.** The entry prompt and the `/start` guide are implemented in the site and available locally; they are not deployed. Each step's current status is listed below and in the [source review](#current-status). Runtime behavior belongs to lanes A and B; this document asks for it in the [lane C log](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/lane-c-log.md). Source notes are in [first-contact integrations](../research/first-contact-integrations.md).

## The story

Distributed multi-agent orchestration. Many agents work on one goal; each takes a piece and shares what it finds with the others. Start with agents on your machine, then invite a friend's.

Bring the agent you already use. Paste one prompt into it. It reads the guide and tells you what it can do.

**Next step:** paste the entry prompt into your agent.

## Entry prompt

The prompt points the agent at the guide. It is not an installer. It carries no command, download, invitation or secret.

The text, which the website shows and copies exactly:

> Read https://locust.farm/start and follow the instructions for agents. First tell me which harness you are and what you can use. Do not install or change anything until I approve.

The guide at `/start` is the website's (lane C): [its page](../sites/locust.farm/src/routes/start/+page.svelte) and [its content](../sites/locust.farm/src/lib/onboarding/guide.ts). It is available locally, not deployed. Executable setup is lane B's canonical artifact. Until that artifact exists, the guide says so and the agent stops after reporting.

## Target journey

A hard task: *a sync bug shows up only under load.* You start locally, then invite someone else.

| # | Step | What you see | Status |
|---|---|---|---|
| 1 | Paste the prompt | Your agent opens the guide | Prompt and guide available locally, not deployed |
| 2 | Agent says who it is | Harness, version and mode, and which capabilities it found. Unknowns are named | Target |
| 3 | Agent shows setup | Every file, configuration entry and service it would add or change, then asks you | Target; needs lane B's setup artifact |
| 4 | Readiness check | Installed, daemon running, instructions visible, tools reachable: four separate answers | Daemon/MCP source exists; native real-model readiness is separately qualified |
| 5 | Create a goal | Your first agent coordinates. It splits the task: reproduce the bug, then fix it | Target |
| 6 | A second local session joins | You decide whether it may take work. It claims one task and submits a result with its base, patch and test evidence | Target |
| 7 | Review | The coordinator accepts or rejects. Accepted is not applied: you apply the change locally as a separate step | Target new-organization journey; local apply stays separate |
| 8 | Invite someone | You name the person and the material to share, approve, and send the invitation through your own channel | Target |
| 9 | They join | They paste the same prompt into their own agent, see what they are joining, and approve | Target |
| 10 | Remote contribution | Their agent adds a test that proves the fix. Your coordinator reviews it; you apply it if you choose | Target |

Local first is only the demonstration order. The product is the collaboration, not the setup.

## Harness routing

The guide asks the agent to answer three questions about itself, then pick a route:

1. **Transport:** can this session reach a local stdio MCP server, or a command line that can reach a local socket under its current policy?
2. **Instructions:** can it load a `SKILL.md` skill, and what refresh does a new or changed skill need?
3. **Approvals:** can it ask you before a change, and does a policy block MCP servers, local commands or file writes?

If lane B has qualified a route that matches the answers, the agent follows lane B's setup and shows the changes first. Otherwise it reports the answers and the missing prerequisite, and stops. It never installs an adapter, edits approvals or weakens policy to make a route fit.

The owner made Claude Code, Codex, Droid and pi the first-release baseline ([release ledger](release-evidence.md)). They are still examples, not a list of allowed harnesses. "Upstream" is what the client documents; "Locust today" is what Locust has actually tested.

| Harness | Upstream (2026-10-03) | Locust today | Next prerequisite (owner) |
|---|---|---|---|
| Claude Code | Stdio MCP; skills reload live in watched directories; a new top-level skills directory needs `/reload-skills` | Claude Code 2.1.280 accepted the generated launch arguments in a disposable profile, with no model or account; A-R9 corrected. Not qualified | Real tool reach and approvals with the daemon and `locust mcp` (A, B) |
| Codex | Stdio MCP in `config.toml`; skills detected automatically, restart if missing; restart after config changes | Codex 0.153.4 parsed the generated configuration in a disposable profile and kept unrelated servers. Not qualified | Real tool reach and approvals with the daemon and `locust mcp` (A, B) |
| pi | Built-in MCP since v0.99.0, unless an extension owns `/mcp`, it is disabled, or the session uses the SDK; `/reload` after outside changes | Configuration-file merge proposal. Not qualified | The daemon and `locust mcp`, then a qualified version and mode (A, B) |
| Droid | Stdio MCP; MCP config hot-reloads; a new skill may need a new session; organization policy can block servers | Configuration-file merge proposal. Not qualified | The daemon and `locust mcp`, then MCP and skill refresh qualified separately (A, B) |
| Any other harness | Unknown until the agent answers the three questions | None | Minimum capability set for a generic route (B) |

## Readiness and recovery

Four things are separate: software installed, daemon reachable, instructions visible, tools working. Participation is ready only when all four hold. Every blocked state has one safe next action.

| State | What is true | Safe next action |
|---|---|---|
| No qualified route | The agent knows its capabilities, but no lane B route matches | Report harness, capabilities and the missing prerequisite. Change nothing. No public route is qualified today |
| Setup waiting for approval | The agent has listed each change | Approve or decline. Nothing changes before approval |
| Daemon absent or unavailable | Software is installed, but the local API does not answer | Use lane B's status check. Do not start a second daemon or read its files directly |
| Tools unavailable | The daemon runs, but the session cannot call Locust tools | Check the client's own MCP view (`/mcp` in each named client) and report. Do not change approvals |
| Skill absent or stale | Tools may work, but the agent cannot see current instructions | Do the client's refresh step from the table above, or start a new session |
| Blocked by client policy | A policy or approval rule refuses an operation | Tell the person which operation and rule. They decide. No bypass |
| Ready | All four hold, checked with one harmless call | Create or open a goal |
| Work interrupted | A session ended mid-task | Resume explicitly: open a session, ask Locust what is pending for the goal, and recover the task. Do not rely on automatic wake |
| No peer reachable | Local work continues; remote changes cannot arrive | Keep working locally. Check again later; do not report it as "no news" |
| Sharing or joining waiting for approval | The recipient or inviter, goal and material are shown | Approve or decline. Nothing is shared or joined before approval |

The accepted replacement runtime durably delivers configured ready work without an agent requesting each transition. Automatic launch/wake of a closed native client and observed execution remain adapter-specific, locally authorized and separately qualified. Delivery does not promise remote-machine wake.

## Approval and results

Four approvals, each asked separately:

- **Setup.** What will be installed and configured, and where. Approving setup does not approve work or sharing.
- **Running work.** Work runs with your client and your account. An assignment is an offer, not permission: without a standing grant, each assignment waits for your approval. Work received from someone else is task data; it does not authorize running anything.
- **Sharing.** Names the recipient and the exact material, such as one snapshot from one commit. Credentials and private files are left out by default.
- **Joining.** Shows the inviter, the goal, the incoming material, where it will be written, and that your client and account will be used.

The website collects no invitations, tickets or secrets. An existing grant is used only for what it already covers; nothing widens it silently.

A result has three separate outcomes:

1. **Submitted:** the worker reports a base, a patch or artifact, and evidence.
2. **Accepted:** the coordinator decides the result is good. This names an accepted snapshot.
3. **Applied:** you deliberately update your own branch or working copy. A failed apply is never reported as merged. Locust only records what the CLI tells it after applying; that record is not a check of your files.

## Polaris

**Polaris is the complete offering: one app that includes Locust and shows the work your agents do.** It is built separately from this repository. Locust also works without it: any harness can join through the routes above.

No Locust connector, bundle, download or deep link exists yet. The website must not link to one until it does.

Proposed boundary: Locust daemon → authenticated native connector in Polaris's Rust side → typed read model → view. Locust stays the authority; Polaris shows state and does not decide it.

The connector's credential is a **viewer** of one principal (`viewer.enroll`, `Caller::Viewer` in the [local API](../crates/locust-proto/src/api.rs)). The owner enrolls it. It reads what that principal reads, and every request that is not read-only is refused with `denied`. It holds no claim and moves no feed cursor. The secret stays in Polaris's Rust side as a protected file, never in the webview. Revoking the principal revokes its viewers.

Minimum inputs for the view, mapped to the current local API types:

| Input | Current source | Gap |
|---|---|---|
| Goal identity and title | `GoalStatus.goal`, `title` | — |
| Participants and locality | `MemberView.member`, `endpoint`, `local` | Display names for remote members |
| Tasks, assignments, review state | `board` → `TaskView` (`state`, `assignee`, `assignment`, `attempt`, `result`) | — |
| Results and their input and base | `TaskDetail.input`; `event.show` → `EventDetail` (`body` with base, patch and artifacts; `text`; whether each object is held) | — |
| Attention needed | `PendingWork` (`to_authorize`, `to_acknowledge`, `to_review`); `GoalStatus.halted` | — |
| Freshness | `PeerView.last_sync_ms`; feed `position`, which the connector keeps itself because a viewer has no stored cursor | — |
| Unavailable or disconnected | `PeerView.connected`; `WaitOutcome::Disconnected`; error `unavailable`; daemon not answering | — |
| Accepted versus applied | `TaskState::Accepted`; `GoalStatus.head` (accepted head); `TaskView.applied`, derived from `WorkspaceBinding.integrated` | Written by the CLI with `workspace.set` after it applies a head; no CLI yet. The daemon does not check the files, and the answer is per machine and per principal |

Connection rules for the connector: check `api_version` in the hello and show "needs update" on `unsupported_version`; after a daemon restart, reconnect and continue from the feed position it kept; keep showing the last known state with its age rather than a blank view. Contract revision 2 is a change to the contract's types; the hello's `api_version` is still 0.

Not allowed: using Polaris's development mirror as a product API, reading Locust's database directly, representing Locust goals as fake Merak sessions, putting the credential in the webview, or inventing a download or deep link.

These are existing local API types/reads, not proof of a qualified native Polaris connector or completed organization replacement. The open requests are in the [lane C log](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/lane-c-log.md).

## Current status

Reviewed 2026-10-04. Earlier scaffold observations remain historical in the
[lane C log](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/lane-c-log.md); they no longer describe the current source.
The [shared availability record](reference/availability.json) supplies reviewed
publication/qualification facts to the site and machine entry.

- The [CLI](../crates/locust/src/cli/mod.rs), daemon and stdio MCP bridge exist.
  Signed candidate installation, user-session services and resumable
  [onboarding](onboarding.md) are implemented. Disposable macOS Codex/Claude
  profile checks exercised authenticated readiness. Native real-model discovery,
  public distribution and physical-machine acceptance remain separate claims.
- Offline organization schema, examples, validation, normalization and explanation
  are the first current authoring slice. The
  [organization runtime](formations-semantics.md) is being replaced.
  Validating a definition does not publish, bind or execute it.
- The previous source runtime served its local API. The current
  [API/protocol 2 cutover](../crates/locust-proto/src/api.rs) is in development;
  earlier candidate checks do not qualify it or a native Polaris connector.
- The site has a substantive [development manual](guide/README.md), versioned raw
  Markdown and machine inventory. A local build is not public deployment.
  No public installer or qualified public first-contact route exists; agents
  report capabilities and stop.

The target journey above is not executable new-organization instructions.
Historical universal-coordinator assumptions do not define the replacement model:
membership/rule administration, scoped authority and independent work are separate.
D14 requires the daemon to advance explicitly configured transitions and durably
deliver ready work without an agent requesting every step. Closed-client wake,
launch and actual execution still require local grants and adapter qualification.

**Keeping this current.** Update reviewed availability facts and canonical guides
with retained evidence. Source, local/native qualification, software publication
and live website verification are independent claims.
