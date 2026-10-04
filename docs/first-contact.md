# First contact

Date: 2026-10-04. **Status: installation/update instructions implemented in source.** The verified macOS Apple Silicon terminal preview is public. Native agent discovery and real-model behavior remain separate qualification claims. The website preview requires authentication, so the copied prompt uses the public download instructions directly. See [installation](installation.md), [onboarding](onboarding.md) and [release evidence](release-evidence.md). Source notes are in [first-contact integrations](../research/first-contact-integrations.md).

## The story

Distributed multi-agent orchestration. Many agents work on one goal; each takes a piece and shares what it finds with the others. Start with agents on your machine, then invite a friend's.

Bring the agent you already use. Paste one prompt into it. It installs or updates verified software, starts the local user daemon and checks an authenticated connection.

**Next step:** paste the entry prompt into your agent.

## Entry prompt

The prompt is self-contained and names public HTTPS instructions. Pasting it authorizes setup and updates for the selected agent; the agent reviews plans and uses `--yes` within that scope. It asks only for a missing selection, a policy/ownership block or a change beyond that scope. Setup grants no work permission, goal membership or sharing authority.

The text, which the website shows and copies exactly:

> Install or update Locust on this machine and connect the agent I am using now. Use the official public instructions at https://locust.farm/downloads/install.md and the verified installer at https://locust.farm/downloads/install.sh; you do not need access to /start. Identify your agent and where your tools run, then inspect the current installation. If Locust already exists, update its existing software prefix and preserve its daemon data, identity, credentials and sessions. Inspect the install and setup plans, then apply them: this request authorizes the verified installation or update, the local user daemon and this agent's connection. Use locust up with --yes for the selected client and current workspace; use the installed CLI if MCP needs a refresh. Inspect up --help: choose codex, claude, pi, droid or shell only when supported. Otherwise finish daemon setup with service plan/apply/start, enroll a dedicated CLI principal without work grants, create its protected session and use that scoped CLI connection. Do not stop just because a route lacks end-to-end qualification. Respect tool permissions, preserve unrelated settings, and ask only for a missing choice or a real blocker. Do not create or join goals, grant work permissions or share files. Finish by checking the running daemon and a harmless authenticated status call; report the installed version, agent connection and any refresh still needed.

The [page](../sites/locust.farm/src/routes/start/+page.svelte) and [shared content](../sites/locust.farm/src/lib/onboarding/guide.ts) use the same instructions as `/llms.txt`. An unqualified native journey is a result to report, not a prohibition on installing published software. `/start` authentication must not block the copied prompt.

Existing software is updated at its owned prefix; activation preserves daemon data and identity. An already running daemon needs an explicit owned service restart to use updated code. Incompatible state is preserved for an explicit fresh-state choice. The agent must not delete state, silently create a replacement identity or stop an unrelated process.


## Target journey

A hard task: *a sync bug shows up only under load.* You start locally, then invite someone else.

| # | Step | What you see | Status |
|---|---|---|---|
| 1 | Paste the prompt | Your agent reads public installation instructions | Self-contained prompt implemented in source |
| 2 | Agent checks its host | Harness, execution host, existing installation and selected workspace | Implemented instructions; unknown version alone does not block setup |
| 3 | Agent reviews setup | Verified installation/update and selected service/profile plans; applies within the prompt authorization | Public software installer; resumable onboarding implemented in source |
| 4 | Readiness check | Installed, daemon running, instructions visible, tools reachable: four separate answers | Daemon/MCP source exists; native real-model readiness is separately qualified |
| 5 | Create a goal | Your first agent coordinates. It splits the task: reproduce the bug, then fix it | Target |
| 6 | A second local session joins | You decide whether it may take work. It claims one task and submits a result with its base, patch and test evidence | Target |
| 7 | Review | The coordinator accepts or rejects. Accepted is not applied: you apply the change locally as a separate step | Target new-organization journey; local apply stays separate |
| 8 | Invite someone | You name the person and the material to share, approve, and send the invitation through your own channel | Target |
| 9 | They join | They paste the same prompt into their own agent, see what they are joining, and approve | Target |
| 10 | Remote contribution | Their agent adds a test that proves the fix. Your coordinator reviews it; you apply it if you choose | Target |

Local first is only the demonstration order. The product is the collaboration, not the setup.

## Harness routing

The agent checks where its tools run, whether shell or stdio MCP is permitted, and whether it can load or directly read the installed skill. Cloud/container execution must not be described as installation on the person's computer. Unknown versions and skill-refresh behavior are recorded without forcing another approval round.

| Harness | Current source route | Discovery |
|---|---|---|
| Claude Code | `up --client claude` | Read the bound skill/CLI now; refresh native discovery if needed |
| Codex | `up --client codex` | Inspect `CODEX_HOME`; refresh native discovery if needed |
| pi | `up --client pi` | Inspect `PI_CODING_AGENT_DIR`; CLI works independently of native MCP availability |
| Droid | `up --client droid` | `.factory/mcp.json` and `.factory/skills/locust`; preserve unrelated entries and ancestor collisions |
| Other shell-capable agents | `up --client shell` | Portable skill, bound CLI and connection descriptor; no app-specific profile is edited |

Droid and shell onboarding are source additions; inspect the installed `up --help` before choosing them. If the published binary lacks them, its existing `service plan/apply/start`, `agent enroll` and `session create` operations can establish a scoped CLI connection without an adapter. The [shared guide](../sites/locust.farm/src/lib/onboarding/guide.ts) provides this portable procedure. Owner administration is confined to setup; the operating prefix uses the enrolled credential/session.

The four named harnesses remain the first-release baseline in the [release ledger](release-evidence.md). Configuration and component tests do not prove a real-model journey. Local CLI access is sufficient for active-session use when permitted; it must not be used to bypass a denied operation or organization policy.

## Readiness and recovery

Four things are separate: software installed, daemon reachable, instructions visible, tools working. Participation is ready only when all four hold. Every blocked state has one safe next action.

| State | What is true | Safe next action |
|---|---|---|
| Native route unqualified or absent | Published software and permitted CLI access are available | Install/update the daemon and use the portable CLI route; report native discovery separately |
| Setup reviewed | Plans fit the pasted prompt authorization | Apply those selections with `--yes`; ask only when scope or required choices differ |
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

- **Setup.** The entry prompt authorizes reviewed installation/update, the user daemon and the selected agent connection. Additional destinations or ambiguous existing state need a decision. Setup does not approve work or sharing.
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
  The [macOS terminal preview](public-preview-release.md) has a public curl
  installer. The copied setup prompt proceeds using public download instructions; native
  agent and real-model readiness are still checked separately.

The target journey above is not executable new-organization instructions.
Historical universal-coordinator assumptions do not define the replacement model:
membership/rule administration, scoped authority and independent work are separate.
D14 requires the daemon to advance explicitly configured transitions and durably
deliver ready work without an agent requesting every step. Closed-client wake,
launch and actual execution still require local grants and adapter qualification.

**Keeping this current.** Update reviewed availability facts and canonical guides
with retained evidence. Source, local/native qualification, software publication
and live website verification are independent claims.
