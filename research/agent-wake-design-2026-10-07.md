# Waking the person's own agent: a design

Status: research, 7 October 2026. This is a proposal, not an accepted plan,
and nothing in it is built. The owner wants a wake ([master plan](../docs/master-plan.md)
answer 32: "Ideally, yes"). Waking is not in the build order, and "starting or
waking an agent from the daemon" stays under "Left out of v2 on purpose" until
a design is approved. Locust claims come from reading the code at `d542413`.
Common Fabric (CF) claims come from reading `packages/connectors/agents` at
[`66ea881`](https://github.com/commonfabric/labs/tree/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1),
in the disposable clone under `output/`. No code was run for this note. Claude
Code flags were checked against its current
[CLI reference](https://code.claude.com/docs/en/cli-reference.md) and
[headless guide](https://code.claude.com/docs/en/headless.md). Codex flags
come from `codex exec --help` of Codex 0.153.4.

## What this does, today against after

| | Today | After |
| --- | --- | --- |
| Work arrives for your agent while none of its chats is open | It waits until you open a chat | Locust on your computer starts one short headless turn of your agent to deal with it |
| A stranger's task in your public goal, when only your agent can approve it | It waits while your agent is closed (public goals, question 5) | Your agent is woken to look at it, while your computer is on |
| Who can start your agent | Only you: you open a chat, or you run `client run` | Also a small Locust process on your computer. It wakes an agent only in a goal where that agent works on its own (level auto) |
| What the woken agent is told | Nothing; it is not running | One fixed line of counts, IDs and tool names. Never another member's words |
| What the woken agent may do | Nothing | What a chat of yours may do: use Locust's tools, and anything else its own approval settings allow. A refused approval ends that turn as "blocked", status says so, and nothing waits for you |
| Which chats are woken | None | Only chats that Locust itself started headless. Never a chat you opened |
| Who pays | Not applicable | Your agent's sign-in or key, as recorded when you set Locust up. Never a key found in the background process's environment, and never a fallback to another payer |
| A crash in the middle of a launch | Your agent's launch record becomes Unknown, and every later launch is refused until you clear it | Only that one turn is unknown and it is never repeated. The next wake waits only until that turn's process has ended |
| Spending limits | Not applicable | None unless you set one. A limit you set stays on your computer and nobody else sees it |
| Turning it off | Not applicable | One command, or setting your agent to a level other than auto |

In short: a small waker process, outside the daemon, watches this computer's
own view of pending work. When the goal's work waits for an agent at level
auto and none of that agent's chats can take it, the waker starts one turn of
that agent. It reuses the existing `client run` route, with a fixed-word
prompt, the agent's own MCP entry and hooks, and an environment built from
what the person recorded at setup. The turn either ends, ends blocked, or
becomes unknown. Each of those outcomes belongs to that one wake, and none is
retried.

## Constraints

Binding on this design:

- **Answer 32.** Waking is wanted. It is not designed, and it is not in v2's
  build order.
- **Answer 26.** The swarm never waits for a human. A wake removes one such
  wait: someone keeping a session open. It must not add another. A blocked or
  failed wake leaves the work where other agents can take it, and it asks
  nobody.
- **Principle 1.** The design that asks least of the person wins. Protective
  features are optional and nothing prompts for them. Spend limits are
  protective, so they are optional, off by default, and never prompted.
- **Principle 3.** Nothing shared is decided by a clock, a timeout or arrival
  order. The wake decision is local and signs nothing. What the woken agent
  signs counts by the goal's rules, like the work of any chat. A local clock
  may therefore pace wakes. For every local time bound, the test is CF's
  question "is firing early safe?". A wake that fires early costs one turn
  and changes no result.
- **Answer 28.** Hooks are in, MCP tools are the only way agents act, and both
  stay harness- and model-agnostic. A woken turn acts only through Locust's
  MCP tools. It hears from the same hooks as any other chat. The wake
  decision contains no branch for a particular harness.
- **Answer 31.** Locust does not guard owner commands against the person's
  own agent. A woken agent with a shell can reach whatever any chat of it can
  reach. The wake adds no guard and claims none.
- **Answer 33.** Locust imposes no guardrails. It offers options.
- **Public goals, question 5** ([plan](../docs/joinable-farms-plan.md) lines
  436-438 and 5136-5143). Strangers' tasks and results wait while no trusted
  agent is running.

Statements this design would replace if approved. They stand today:

- [`managed.rs`](../crates/locust-adapter/src/managed.rs) line 4: "No peer
  event initiates a launch". Lines 81-82: permission flags are "never
  synthesized by this adapter".
- [`client.rs`](../crates/locust/src/cli/client.rs) line 30: `client run` is
  "never triggered by peer events". Line 730 returns `"automatic_wake":false`.
- [agents.md](../docs/guide/agents.md) line 81: "locust.farm does not wake a
  closed agent".
- The [ergonomics audit](agent-ergonomics-2026-10-05.md) dropped resuming a
  closed agent on a peer event, because remote events are "insufficient
  launch/signal authority" (lines 550-552). It called a closed agent that is
  never woken "a boundary, not a gap" (line 564).
- The [store plan](../docs/agent-memory-and-store-plan.md) lists "Waking a
  closed chat" and "hooks for `client run`" under "Not in this plan" (line
  939).

The narrower rule proposed in their place: **a peer's record can cause a wake,
but it chooses nothing about it.** Every record that leads to a wake has
already been checked and folded by the local daemon, like any record. The
person's setup fixes everything else locally: what runs, where, with which
arguments, environment, model and payer. The prompt carries only IDs and
counts. Each agent runs at most one woken turn at a time.

## What already exists

### In Locust

- **Managed launch** ([`managed.rs`](../crates/locust-adapter/src/managed.rs))
  - It has headless routes for Codex (`exec --json`, `resume ID`), Claude
    Code (`-p --output-format stream-json`, `--resume ID`), Droid (`exec`,
    `--session-id ID`), Kimi (`--session ID`) and pi (a session file)
    (lines 234-294).
  - It records `Launching` before the spawn and refuses to spawn if that
    report fails (606). The child gets a cleared environment plus an explicit
    one (607-616).
  - `Started` follows. If the report after the spawn fails, the launch becomes
    `Unknown` (631-637).
  - It reads the native session ID only from structured events (416-450). If
    that identity changes, the launch becomes `Unknown` (454-469).
  - `recover` turns anything not `Exited` into `Unknown` and never retries
    (339-350).
- **`client run`** ([`client.rs`](../crates/locust/src/cli/client.rs))
  - It needs an agent credential and a session, and refuses the owner
    (117-136). It takes a launcher lock per session (175-180).
  - It refuses to launch unless the last record is `Exited` (196-204). A
    resume must match the exact native session, paths and attempt that
    exited (276-303).
  - It writes a lifecycle receipt per launch and syncs it to disk (304-321).
    It passes the MCP bridge per launch (322-335).
  - It marks a launch `Blocked` from structured refusal events for Claude
    Code, Codex and Droid (1003-1051).
- **Hooks** ([core](../crates/locust-adapter/src/hooks/core.rs))
  - The stop line holds fixed words, counts, IDs and tool names, and never a
    title (676-692).
  - An idle worker parks for at most 270 s, and only where no person types
    (443-462; `MAX_WAIT` in [hook.rs](../crates/locust/src/hook.rs) line 41).
    `LOCUST_HOOKS=unattended` turns that on ([cli/hook.rs](../crates/locust/src/cli/hook.rs)
    lines 28-55).
  - Free tasks are counted with the daemon's
    [`Goal::unattended`](../crates/locust-core/src/goal/mod.rs) (lines
    452-467), through `WorkItem.unattended` (core.rs 641-650).
- **API** ([api.rs](../crates/locust-proto/src/api.rs))
  - `Wait` answers when a goal changes (lines 676, 932). Hooks already wait on
    it.
  - `SessionState` has `Blocked` and `Unknown` (1864-1880).
  - `SessionCapabilities.idle_wake` exists (1890-1891). No adapter ever sets
    it to true.

What is missing:

1. **A trigger.** Nothing in Locust decides to start an agent.
2. **Uncertainty scoped to one launch.** `Unknown` lives on the execution
   session's one record. Any `Unknown` refuses every later launch of that
   session until a person acts (client.rs 196-204). That is a wait on a person.
3. **A known outcome survives a lost report.** If the final `Exited` report
   fails, the record stays `Started`. The next run is refused, and `client
   recover` writes `Unknown` (client.rs 706-727). The launcher knew the exit
   code and dropped it.
   ([CF assessment](common-fabric-second-assessment-2026-10-07.md) line 387.)
4. **A chosen environment.** The child gets the caller's environment minus
   `LOCUST_*`, with `LOCUST_HOOKS=off` (client.rs 336-339). That is right for
   a command a person types in their own shell, and wrong for a background
   process.
5. **A fence.** After a launcher dies, nothing tells whether the turn it
   started is still running.
6. **A blocked signal everywhere.** Kimi and pi have no structured refusal
   detector.
7. **Room for setup's own entry.** `client run` refuses a profile that already
   has an MCP server named `locust` (`occupied_names`, client.rs 499-551). So
   today it cannot run in the profile that setup configured.

### In Common Fabric's connector

The [second CF assessment](common-fabric-second-assessment-2026-10-07.md)
(lines 71-108) found the connector to be prior art for running a woken turn,
and none for deciding when to wake. A full read of its source confirms this.
Paths below are under `packages/connectors/agents/`.

Worth taking:

- **An in-flight receipt before the provider call.** Under the session's
  queue, the worker writes an `in-flight` receipt and syncs it to disk:
  temporary file, fsync, rename, fsync of the directory
  (`connector/src/command-ledger.ts:217-256`). It then publishes the receipt.
  If publication fails, it never calls the provider. It writes `failed` with
  `claim-publish-failed` instead (`connector/src/commands.ts:583-623`).
- **Unknown is never retried.** On restart, every `in-flight` entry becomes
  `unknown` (`orphaned-in-flight`, `retryable: false`;
  `command-ledger.ts:474-503`). Admission skips any command already in the
  ledger (`commands.ts:431-434`). "Callers must not treat it as safe to repeat
  automatically" (`connector/docs/interfaces.md:339-340`).
- **Unknown belongs to one command.** The next command for the same session
  runs normally.
- **Nobody present, nothing approved.** Codex command and file-change approval
  requests get `decline` (`drivers/codex-app-server.ts:31-43`). ACP permission
  requests get `cancelled` (`drivers/acp.ts:166-167`). Only an explicit
  `allowDangerFullAccess` switches Codex to approval `never` with
  `dangerFullAccess`.
- **A session ID chosen by the sender.** A Claude start passes the sender's
  UUID as the SDK's `sessionId`, and it fails if that session already exists
  (`drivers/claude-agent-sdk.ts:519-528, 590-613`).
- **A paying account the person connected.** For CF's own harness, the plan
  `docs/plans/cf-harness-codex-subscription-auth.md` makes the person who
  starts the work pay with an account they connected. Never a service-wide or
  ambient credential (lines 16-19, 135-139). No automatic fallback to another
  payer (500-502). No choice of a personal subscription "through ambient
  process state" (621-622).

Missing or not to copy:

- **No trigger.** Every command producer is a click: the debug view's
  confirmed "Send command", and the workbench Start buttons
  (`packages/patterns/topic-workbench/main.tsx:368-447`;
  `packages/patterns/person-workbench/main.tsx:461-540`). Even that is a
  convention. The host accepts any configured producer that declares a
  verified writer (`host/README.md:141-158`).
- **No fence after unknown.** The per-session queue and every driver's
  "already active" guard live in memory (`commands.ts:361, 499-504`;
  `claude-agent-sdk.ts:804-829`). After a restart, a turn marked unknown may
  still be running when the next prompt for its session arrives.
- **Refusals mostly vanish.** A declined Codex approval lets the turn continue
  and complete, and the receipt says `succeeded`. Claude's
  `permission_denials` are dropped (`claude-agent-sdk.ts:291-292`).
- **No usage capture and no spend limit.** The SDK's `maxTurns` and
  `maxBudgetUsd` are never passed.
- **No tools for the agent.** Claude gets no `mcpServers`, hooks or
  `settingSources`. ACP gets `mcpServers: []`. Locust needs the opposite:
  exactly its own tools.
- **The whole host environment is inherited**
  (`claude-agent-sdk.ts:345, 869-875`). CF accepts this only because the
  connector runs as the person. Its own harness plan forbids an ambient payer.
- **Two machines, one command.** CF leaves this unsolved and says it needs
  outside coordination (`connector/docs/interfaces.md:235-241`). Locust does
  not have the problem: a wake belongs to one computer and one agent, and an
  agent's identity is per computer.

## The design

### 1. What triggers a wake

**Author's choice: the waker decides alone, from this computer's own view.** For each
agent with wake on, it holds one `Wait` per goal on the local daemon, as
hooks do. When an answer changes, it reads `pending`. It wakes the agent when
all of these hold:

- the agent's local level in that goal is **auto**;
- `pending` lists work of a kind in the table below that no earlier wake has
  covered;
- no chat of that agent can take the work (see below);
- no woken turn of that agent is running or fenced (section 5);
- no limit the person set has been reached (section 9).

| Pending for the agent | Wakes at auto | Why |
| --- | --- | --- |
| An attempt the agent holds, whose chat has closed | Yes, by resuming that work | Only this agent can finish or drop it. Until then it keeps the task out of automatic pickup ([CF assessment](common-fabric-second-assessment-2026-10-07.md) line 380) |
| A cancellation of an attempt the agent holds | Yes | Other agents wait on the acknowledgment |
| A result the agent may review | Yes | Under peer approval, a review is how work counts |
| A delivery | Yes | Only this agent can acknowledge it |
| A free task that nobody attempts (`WorkItem.unattended`) | Yes | Taking tasks on its own is what auto means |
| After J2: a door member's task waiting for a trusted approval | Yes, for a trusted agent | Public goals, question 5 |

Nothing wakes at level ask or read. At ask, the person has already chosen
that a task waits for them (answer 26 counts this as the person's own choice).
At read, the agent takes no new work. Nothing wakes where the agent is not a
member, or in a goal that is halted, catching up after a restore, or ended.
The waker checks the goal's status first, as the stop hook does (member, not
halted), and status already tells the person why such a goal is quiet.

**Loop guard.** The waker keeps the set of work identities each wake covered,
as the stop hook's marks do (core.rs 443-471). A new wake needs at least one
identity that no earlier wake covered. A turn that ends blocked, or that does
nothing, is therefore not repeated for the same work. One case needs a rule:
a wake that ends unknown. Once its fence has cleared (section 5), work it
covered that is still pending may cause one new wake. That is not a retry of
the lost turn. The new turn starts from the daemon's current state, which
already includes whatever the lost turn signed.

**Batching comes for free.** A woken turn runs with
`LOCUST_HOOKS=unattended`. Its stop hook keeps it working while work waits,
and parks it for up to 270 s when nothing does
([agents.md](../docs/guide/agents.md) lines 105-111). Work arriving during the
turn reaches that turn, not a new wake. A busy goal therefore makes one long
turn rather than a stream of wakes. Parking costs no model tokens.

**When a chat of the agent can already take the work.** The decision needs
to know whether such a chat exists. Before H3, Locust only knows whether some
connection holding the session's secret is open (`SessionView.attached`,
api.rs 1956-1957). That stays true for a chat left idle in a window for days.
The rule, before H3:

- **Work tied to an attempt the session holds** (the attempt itself, its
  cancellation) wakes only when no chat of that session is attached. An open
  chat holds that attempt, and two chats must not work it at once.
- **All other work** (reviews, deliveries, free tasks, door approvals) wakes
  whether or not a chat is open. An idle chat is not working. The worst
  overlap is an active chat and a woken turn both starting a free task. Under
  option A below they share one execution session, and A2 gives a second
  start with no task named the same claim ("Asking twice returns the same
  claim"). Under option B they are two sessions, as two open chats in two
  profiles are today.

After H3, "attached" becomes H3's presence: a chat that is active or waiting
takes its own work, and a closed one does not. The waker's own connection to
the daemon must not count as a chat. W2 has to mark it, or count bridges
rather than raw connections.

**What a wake cannot fix.** A wake needs the computer on and awake. A laptop
with its lid closed wakes nobody, and Locust does not keep a computer awake.
With wakes, public-goals question 5 shrinks from "while no trusted agent is
running" to "while every trusted member's computer is off or asleep".

**Two computers of one person.** Each computer wakes only its own agents. If
both see the same free task, both may start on it, exactly as two open chats
can today. The goal's rules decide what counts. One trusted approval of a
stranger's task is enough, so a second one is harmless.

### 2. Which sessions may be woken

Two terms: Locust's **execution session** (a session file and its record),
and a harness's **native chat** (the ID the harness resumes).

**Author's choice:**

- **The waker learns native chat IDs only from its own launches.** The ID comes
  from the structured event that `observe_native` reads (managed.rs 416-450),
  and the waker records it in its ledger (section 5). A native ID that a hook
  saw, or that a person's chat used, is never a candidate. A person's
  interactive chat is never resumed, by construction.
- **Resume, or start new.** A wake resumes the native chat of an earlier wake
  only when both hold:
  - that chat holds the attempt this wake is for, so its context is the
    work's context;
  - that chat's last turn ended with a known exit, and its fence is clear.

  Every other wake starts a new native chat. Reviews, deliveries, free tasks
  and door approvals need no earlier context: the
  [A3](../docs/agent-memory-and-store-plan.md) first page and `locust_status`
  give it. A short new chat is also cheaper than resuming a long transcript
  after its prompt cache has expired.
- **A woken chat the person opens becomes theirs.** Under option A below, the
  person can open a woken chat in their own client. The waker gives each
  woken turn a marker in its environment. Codex, Claude Code and Droid hook
  payloads carry the native session ID. A hook that sees a woken chat's ID
  without that marker records, in the private marks it already keeps, that a
  person opened it. The waker never resumes that chat again, so no woken turn
  types into a chat the person is reading.

**Open: which profile woken turns run in.** This is owner question 2.

| | A. Your agent's own profile (recommended) | B. A separate wake profile |
| --- | --- | --- |
| Setup | Nothing more: setup already wrote Locust's MCP entry and hooks there, tied to the agent's credential and session ([setup.rs](../crates/locust/src/installation/setup.rs) 320-346) | Locust writes the profile. The person signs the agent in once more there, or names a key. In Codex, hook trust must be granted there too |
| Who pays | As the agent already pays in that profile, recorded at setup (section 7) | Whatever the person signs in with there |
| The person's own instructions and approval settings | Apply to woken turns | Do not apply, unless the person repeats them there |
| Woken chats | Appear in the agent's own chat history, where the person can read them | Kept apart; the person sees them only through Locust's status |
| Execution session | The one setup made, shared with the person's chats. A woken turn can pick up an attempt that a closed chat of the agent left | Its own. An attempt held by the person's chats needs an explicit takeover |
| Risk | The person opens a woken chat while it runs. Covered by the rule above, and by section 5's one-turn-at-a-time | None between chats. The cost is the second sign-in |

Recommended: A, because it asks nothing more of the person (principle 1).

### 3. Where the waker lives

| Option | Verdict |
| --- | --- |
| Inside the daemon | No. The master plan leaves "starting or waking an agent from the daemon" out of v2. A network-facing process would gain the power to start processes, and the daemon is meant to stay minimal |
| A separate per-user process, run by the same service manager as the daemon (launchd or systemd) | **Recommended** for normal use. `up` installs it next to the daemon's service |
| The same command in a terminal, in the foreground | Also offered: for people who run no service, and for tests. Same code |

The waker is a client of the daemon's local API, as `client run` is. It holds
agent credentials and sessions, never the owner credential, and refuses
`--owner` as `client run` does (client.rs 117-136). The daemon gains no code
that starts a process. Its only new input is the session reports the waker
already sends through `SessionReport`.

One waker process serves every agent whose wake is on. Each agent's work uses
only that agent's credential and session. Per-agent processes would add no
real separation: every agent credential, the owner credential and every
signing seed are already files of one OS user ([CF assessment](common-fabric-second-assessment-2026-10-07.md)
risk 2).

### 4. The wake prompt

**Author's choice: fixed words, counts, IDs and tool names only.** The prompt never
contains a task title, a finding, a review or any other text a member wrote.
That includes the person's own other agents, because their text may restate
a stranger's. It is rendered by the same code as the hooks' stop line
(core.rs 676-692), so one census of model-facing text covers both. For
example:

```text
Locust woke you: 1 held attempts, 0 cancellations, 2 reviews, 0 deliveries, 3 free tasks; attempt 3f9a1c2e in goal 8b20d417. Use locust_context_read and locust_pending. Act in the goal only through Locust's tools; report or drop each attempt you hold before you end your turn.
```

The wording is the phase's to tune. It must read the same to any model,
without vendor markup (answer 28). The prompt travels as an argument, as in
`client run`, so other processes of the same user can read it. It holds only
IDs, so that exposes nothing new. The agent gets everything else through the
context tools, which mark other members' words as material and never as
instructions.

### 5. Exactly once, and Unknown per wake

**Author's choice: each wake has its own ID, its own record, and its own uncertainty.**

**The ledger.** The waker keeps a private, unsigned ledger in the Locust data
folder, keyed by wake ID. The ID is the launch ID that `client run` already
mints (client.rs 304-308; `Metadata.launch_id`). The states:

```text
planned -> launching -> started(pid, native id when seen) -> exited(code, blocked?) -> reported
                                     \-> unknown (waker restarted before it saw the exit)
```

1. Before reporting `Launching` to the daemon, write the entry as `launching`
   and sync it to disk. If that write fails, nothing is spawned. This is CF's
   order: the receipt is durable before the side effect.
2. Report `Launching` to the daemon. If that fails, nothing is spawned. This
   is already the rule (managed.rs 606).
3. Spawn. Record `started`.
4. On exit, write `exited` with the code, and `blocked` if a refusal was seen,
   and sync it to disk. Only then report to the daemon. A failed report is
   sent again from the ledger, on the next loop or after a restart. A known
   exit therefore never turns into Unknown. That fixes the lost exit report
   (missing item 3), with or without wakes.
5. On restart, every entry still `launching` or `started` becomes `unknown`.
   It is never retried, as in CF.

**The fence.** CF has no fence. Locust needs one, so that an unknown turn and a
new one never run at once. The waker opens a lock file per wake, takes an
exclusive `flock` on it, and passes that descriptor to the child without
close-on-exec (Rust opens files with it set). The lock then outlives the
waker. The kernel releases it when the last descriptor is closed: when the
turn, and every process it started that kept the descriptor, have exited.
Before any wake of that agent, the waker tries each lock left by an
`unknown` wake:

- **Held:** the earlier turn, or something it started, is still running.
  There is no wake for that agent. Status says "a woken turn from before
  Locust restarted is still running".
- **Free:** that process has ended. The wake stays `unknown` in the history,
  because its outcome was never observed, but it no longer blocks anything.

The fence uses no PID and no clock, and it never signals anything. That keeps
managed.rs's rule that recorded PIDs "never authorize signaling or
respawning" (lines 5-6). It does depend on each harness keeping inherited
descriptors open. W4 checks this per harness. Where a harness closes them,
the fallback is the recorded PID together with the process's start time read
from the operating system. That is evidence only for not waking, never for
signaling.

**What changes in today's records.** The daemon's session record stays one
record per execution session. The gate in `client run` (client.rs 196-204)
changes from "the last record is `Exited`" to "no launch of this session is
`launching` or `started`, and no unknown launch's fence is held". `Unknown`
becomes a fact about one launch ID. It does not refuse every later launch
until a person acts. `client recover` keeps its meaning: it marks a launch it
cannot account for, and never starts or stops a process.

**Exactly once, more precisely.** Locust guarantees at most one turn per wake,
possibly none. It never repeats a turn. The work itself stays exactly-once in
the usual way: a woken agent's writes are ordinary signed records, with the
idempotency keys and claims every chat uses.

### 6. Locust's tools and hooks, passed explicitly

CF gives a woken agent no tools at all. Locust gives it exactly its own: the
MCP server and the hooks, both tied to the right daemon and session.

Under option A, the profile already holds setup's entries.

- **Check before each wake.** The waker verifies that the profile still holds
  setup's own MCP entry and hook entries, byte for byte as setup's record has
  them. If not, there is no wake. Status says "wake off: Locust's entry in
  this profile was changed or removed".
- **Pass the server per run where a harness allows it.** For Codex,
  `-c mcp_servers.locust=…` overrides that one key for the run, as `client
  run` already does (config.rs 136-162). For Claude Code, `--mcp-config`
  exists, but the CLI reference does not say which wins when the profile has
  a server of the same name. `--strict-mcp-config` would settle it, but it
  drops the person's other MCP servers. W2 relies on the verified entry and
  checks the precedence. For Droid, Kimi and pi, Locust passes the server
  through a profile file today, so setup's verified entry is the explicit
  one.
- **Accept setup's own entry.** `client run`'s refusal of an occupied
  `locust` name (missing item 7) changes to accept setup's own identical
  entry.
- **Hooks on, set explicitly.** The child's environment sets
  `LOCUST_HOOKS=unattended`, where `client run` sets `off`. Setup installed
  the hooks, and the person granted Codex's hook trust at setup (agents.md
  87-90). Where a harness has no stop hook, the skill's fallback
  (`locust_wait`) applies. Claude Code's `--settings` can carry hooks per run.
  The reference does not say whether they merge with the profile's, so W2
  does not use it.

Under option B, Locust owns the whole profile. The MCP server comes through
`client run`'s existing per-launch route, and setup's code writes the hooks
there.

### 7. Who pays

**Author's choice: the environment the person chose at setup. Never the
background process's environment, and no fallback.**

Why it matters: Claude Code in `-p` mode always uses `ANTHROPIC_API_KEY` when
it is present, ahead of the subscription sign-in
([authentication](https://code.claude.com/docs/en/authentication.md)). Take a
person whose shell exports a key. Their chats pay by that key. A waker
started by launchd would not see that export, and would pay by the
subscription instead. A service environment that happens to hold some key
would do the reverse. Today `client run` copies the caller's environment
(client.rs 336-339). That is fine for a command typed in a shell, and wrong
here.

The design:

- **At setup** (`up` or `agent add`), Locust records the wake route: harness,
  executable, version, profile, `PATH`, and the payer. The payer is one of
  two:
  - **"your agent's own sign-in in this profile"**, when the setup shell holds
    none of the provider-key variables that harness documents;
  - **"the key in `NAME`"**, when it holds one. The value is stored in a
    private file in the Locust data folder, like the credentials. It is never
    synced, signed or printed.

  Setup asks nothing. It says the result in one line, for example "Woken
  turns: Codex, paid by your ChatGPT sign-in", and one command changes it.
  CF's harness plan refuses to pick a subscription automatically because a
  credential exists. That plan is for a shared service. Here the computer and
  the agent are the person's own, and the record shows exactly what they
  already use.
- **At each wake**, the child environment is built from nothing. `env_clear`
  is already applied (managed.rs 610). Into it go: the profile and harness
  folders, as `client run` sets them; the recorded `PATH`;
  `LOCUST_HOOKS=unattended`; and the recorded payer's variables. No
  provider-key variable the person did not choose is passed, even if the
  service environment holds one.
- **If the payer fails** (sign-in expired, key revoked), the turn ends. Status
  says "your agent's last woken turn could not sign in". No other payer is
  tried. The loop guard allows at most one more try, when new work arrives.
  Nothing waits for the person: other members' agents carry on.

In a public goal, strangers can cause wakes. A stranger's task wakes the
trusted agents, and each wake is paid by that agent's person. Nothing limits
how many tasks a door member opens ([public-goals plan](../docs/joinable-farms-plan.md)
line 445). The loop guard and one-turn-at-a-time keep the number of wakes
growing with how often new work arrives, not with how much work there is.
Each woken turn sees everything waiting. The door plan should say this in
plain words, and name the optional limit in section 9.

### 8. Approval prompts with nobody present

**Author's choice: refused approvals show as Blocked; the waker never answers
one.**

On the CLI routes, a headless harness does not wait for an answer. It refuses
the tool call:

- Claude Code `-p` lists denials in the final result's `permission_denials`
  (headless guide).
- Codex, with approval policy `never`, fails the MCP call with a fixed
  message.
- Droid `exec` says the tool "requires higher autonomy".

Locust already reads all three as `Blocked` (client.rs 1003-1051). W4 adds
detectors for Kimi and pi. A blocked woken turn:

- records `blocked` in its ledger entry and `Blocked` in the session record;
- runs on or ends by itself. The waker sends no answer and no signal;
- shows one status line, for example "your agent's woken turn was refused a
  tool by its own settings";
- leaves its work identities covered (no loop). Free work stays free for other
  agents. The prompt tells the agent to report or drop an attempt it cannot
  finish.

CF answers approvals itself because its drivers sit on the approval channel.
Locust's routes do not, so there is nothing to decline. This is also why a
woken turn shows its refusals when CF's mostly vanish.

**Author's choice: a woken turn may use Locust's own tools.** A turn
whose Locust tool calls are all refused can do nothing. It would cost a turn
for every new piece of work. So where a harness has a per-run way to allow
one MCP server's tools, the wake allows Locust's tools and nothing else. For
Claude Code that is `--allowedTools`. Codex and Droid have no such way
established yet; W4 checks. Everything else, including the shell and file
edits, follows the person's own settings. Without them a woken agent can
review, approve and report, but not edit. That is the person's choice, shown
as blocked. Level auto already means "takes eligible tasks on its own", and
answer 31 puts the agent's reach out of Locust's scope. This changes
managed.rs lines 81-82 for this one allowance.

### 9. Optional spend limits

**Author's choice: off by default, local, unsigned, never prompted.** Like levels,
limits live on the person's computer. The host cannot see them, and they are
never synced. Three kinds are proposed:

- **Wakes per day** for an agent. The day is the computer's local calendar
  day. A clock may decide this because it is not shared, and firing early is
  safe.
- **A budget per turn**, passed to harnesses that take one. Claude Code takes
  `--max-budget-usd` and `--max-turns`. Others get nothing, and Locust does
  not interrupt their turns.
- **No dollar limit computed by Locust.** Codex reports tokens, not dollars,
  and subscription turns have no dollar figure. CF withholds cost on
  subscription runs for the same reason.

When a limit is reached there is no wake, and status shows one line until the
limit resets. Nobody is asked.

### 10. The launch route: CLI resume, ACP, the Codex app server, or the Claude Agent SDK

| | CLI headless routes (today) | ACP | Codex app server | Claude Agent SDK |
| --- | --- | --- | --- | --- |
| Harnesses | All five that Locust sets up | Agents with an ACP server or adapter. For Locust's five, not established | Codex only | Claude Code only |
| Built in Locust | Yes: managed.rs, qualified per harness | No | No | No |
| Process | One owned child per turn; its exit ends the turn | A long-lived agent process over JSON-RPC on stdio | A long-lived JSON-RPC process on stdio. `codex app-server --help` calls it experimental | A Node or Python library. It needs that runtime beside Locust's Rust binary |
| Passing Locust's MCP server | Arguments for Codex and Claude Code; a profile file for Droid, Kimi and pi | `mcpServers` per session, on new, load and resume | Configuration overrides on thread start and resume | `mcpServers` option |
| Nobody to approve | The harness refuses; Locust reads the refusal for three of five | The client answers `request_permission` (CF: cancelled) | The client answers approval requests (CF: decline, and the turn continues) | `canUseTool`, `permissionMode` |
| Session ID before start | Claude Code `--session-id`; pi takes a session file path; the others report it after start | Created by the agent | Thread ID after start | `sessionId` |
| Resume | All five | `session/load` or resume, if the agent advertises it | `thread/resume` | `resume` |
| Usage | From result events (Claude Code reports cost and usage) | Stop reason only, as CF reads it | Token usage notifications | Cost and usage |
| Who pays | The person's installed official client, with their own sign-in or key | Depends on the adapter | The person's Codex sign-in | The SDK overview: third parties may not offer claude.ai login "unless previously approved", so in practice an API key |

**Recommended: the CLI routes for the first wake.** They exist, are qualified
for all five harnesses, need no second runtime, and run the person's own
official client. That makes "pay the way your agent already pays" literally
true.

- **ACP.** Its real gain is MCP servers passed per session, with no profile
  file to overlay and restore. It is also one protocol across harnesses.
  Revisit it if Locust adds a harness with no headless resume, or if the
  Droid, Kimi or pi overlays cause trouble.
- **The Codex app server.** It offers exact approval requests and a mid-turn
  interrupt. A wake needs neither: the turn ends by itself, and the waker
  answers no approvals.
- **The Claude Agent SDK.** It adds a runtime. Its login terms push toward an
  API key, against the person's own subscription. The one SDK feature the CF
  assessment flagged was a session ID chosen by the sender (line 467). The
  Claude Code CLI already has that as `--session-id`. `prepare` refuses that
  flag from the person's arguments (managed.rs 166-192). The waker would set
  it itself for a new Claude Code chat, so the chat's ID is in the ledger
  before the spawn.

## What a person sees

- **Setup, one added line:** "Woken turns: Claude Code in ~/, paid by your
  claude.ai sign-in. Turn off: `locust --owner wake off`."
- **`status`, only when there is something to say:** "Your agent was woken 3
  times today; the last turn ended normally"; "…the last woken turn was
  refused a tool by its own settings"; "…could not sign in"; "a woken turn
  from before Locust restarted is still running"; "wake off: Locust's entry
  in this profile was changed or removed"; "daily wake limit reached".
- **Commands** (the person's own; they apply at once, like setting a level):
  `wake on|off [--agent NAME]`, and an optional `wake limit`. Changing the
  payer reuses setup.
- **The agent's chat history (option A):** woken chats appear there, opened by
  the fixed prompt above.
- **Session view:** a session with wake on reports `idle_wake: true`, which no
  session reports today.

## Tests that would show it works

- **The fake harness** already used for hooks: work arrives with no chat
  open, and exactly one turn starts. Then:
  - a second piece of work during the turn reaches that turn through its stop
    hook;
  - level ask or read, a halted, catching-up or ended goal, and an open chat
    holding the attempt each start nothing;
  - a turn that ends blocked is not repeated for the same work.
- **Crash points**, each ending with no turn repeated and nothing waiting on a
  person:
  - between the ledger write and the `Launching` report;
  - between the report and the spawn;
  - after the spawn, with the child still alive. The fence holds, and nothing
    wakes until the child exits;
  - after the exit, with the daemon unreachable. The exit is reported after
    restart, and nothing becomes Unknown.
- **The census of model-facing text** covers the wake prompt. No title or
  member text can reach it.
- **Environment:** a provider-key variable that is in the waker's environment
  but was not chosen at setup never reaches the child. With no payer
  recorded, the agent's own sign-in is used and no variable is passed.
- **Per harness, with real models:** one wake each; a refused tool reads as
  Blocked, Kimi and pi included; descriptors inherited for the fence; Claude
  Code's same-name `--mcp-config` precedence; any per-run allowance of one
  MCP server's tools.

## Phases, if approved

| Phase | What works afterwards | Needs |
| --- | --- | --- |
| W1 | A launch's uncertainty is its own. Unknown is per launch ID, a fence shows whether an interrupted launch's process still runs, and a known exit survives a lost report. Useful without wakes: it fixes the lost exit report | nothing |
| W2 | A foreground waker: the trigger, loop guard, prompt, route recorded at setup, environment built from nothing, hooks on, setup's entry accepted, `wake on/off`, the status lines | W1, H1a and H1b (built) |
| W3 | The waker runs as a service next to the daemon, installed by `up`, with the setup line | W2 |
| W4 | Blocked detection for Kimi and pi; optional limits; per-harness checks with real models (fence descriptors, MCP precedence, per-run tool allowance) | W2 |
| After J2 | A door member's task waiting for approval wakes a trusted agent. Public-goals question 5 is restated | W2, J2 |

H3's presence improves the "a chat can already take it" rule, but no phase
waits for it.

If approved, these documents change with the phases:

- master plan: the "left out" list and answer 32's note;
- public-goals plan: lines 436-438, the passage at 2737, and question 5;
- agents.md: lines 81 and 124-125;
- the store plan: line 939;
- the managed.rs header and the about-text of `client run`.

## Decided here by the author

These follow from the owner's principles. Each is reported here in case the
owner objects.

- Only level auto wakes. Ask and read never do.
- The waker lives outside the daemon. It is one per-user process and holds no
  owner credential.
- The prompt is fixed words, counts, IDs and tool names, rendered by the
  hooks' code.
- A wake resumes only a woken chat that holds the attempt it is for.
  Otherwise it starts a new chat. A chat the person opened is never resumed.
- Unknown belongs to one wake and is never retried. A fence that uses no PID
  or clock keeps an unknown turn and a new one from overlapping.
- The payer is recorded at setup with one line and no question. The child's
  environment is built from nothing, and there is no fallback.
- Refused approvals are Blocked and never answered. A woken turn may use
  Locust's own tools where the harness allows a per-run allowance.
- Limits are off by default, local, unsigned, and never prompted.
- The existing CLI routes, not ACP, the Codex app server or the Claude Agent
  SDK.

## Questions for the owner

1. **Should Locust wake your agent without you turning it on?** Recommended:
   yes, wherever your agent works on its own (level auto). Setup says so in
   one line, and one command turns it off. Under yes, your agent's sign-in or
   key is used while you are away, including to look at strangers' tasks in a
   public goal. The other choice is off until you run one command, once.
   Under that choice, work that waits for your agent waits until you open a
   chat or turn waking on.
2. **Where should woken chats live?** Recommended: in your agent's own chat
   history, paid the way your agent already pays. Nothing to do; you can open
   any woken chat to see what your agent did, and your own instructions and
   settings apply to it. The other choice keeps woken chats apart from yours,
   where your own instructions and settings do not apply. For that, you sign
   your agent in once more, or name a key, for Locust to use.
