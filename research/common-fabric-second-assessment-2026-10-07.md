# Common Fabric and Locust: second, independent assessment

Status: research, 2026-10-07. This note reviews
[the first assessment](common-fabric-assessment-2026-10-07.md) of the same day
and records what it missed. It is not an accepted plan or a security audit.
Every recommendation below is a proposal. No Common Fabric code was run. No
Locust suite was run for this note: Locust claims come from reading the code at
`a71495d`, and the four marked "checked by hand" were re-read line by line
after the verifiers had confirmed them.

Common Fabric (CF) paths are in `commonfabric/labs` at
[`66ea881`](https://github.com/commonfabric/labs/tree/66ea88177e51e7afc554dd8b6698a6d0fb78b4d1),
the commit the first assessment read. It was still the remote head on
2026-10-07. CF history since 2026-06-01 (4,165 commits) was fetched into the
disposable clone under `output/`.

## Method

Eleven readers each took one area of CF and compared it with Locust: agent
harness, coordination products, storage and sync, identity, information flow
and sandboxing, files, verification, development process, trajectory,
integrations, and a line-by-line fact-check of the first assessment. A
skeptical verifier per area re-opened every citation on both sides, checked
Locust for what a finding said it had or lacked, and checked whether the first
assessment already covered the point. Verifiers marked a finding refuted when
they could not confirm it themselves. A completeness critic then named five
gaps, each read and verified the same way: authority separation across both
systems, a full read of CF's agents connector, CF's records of its own
many-agent build, CF's server-side execution against Locust's host automation,
and the agent call contract.

Of 116 findings, 52 were confirmed as written and 64 confirmed with
corrections; none was refuted. The text below uses the corrected versions.

## Verdict on the first assessment

**Its citations are accurate; its centre of gravity is not.** Every CF link and
line range says what the report says. The counts hold (10,357 tracked files;
39 packages; 3,350 test files, of which 3,322 end in `.test.ts(x)` and 28 in
`_test.ts(x)`; no submodules). The five private repositories still return 404,
and so does `weaver`. Locust changed only by the report itself since its
baseline `2e5b954`, so its Locust claims still hold. Three paraphrases need
small fixes: delivery receipts disclaim claiming work or launching a client
(not "start work"); `Why::OnlyYou` is person-only, owner or host (not
owner-only); and in detached mode with no receipt, CF's CLI does commit a
same-id call again (`packages/cli/commands/piece.ts:1193-1199`).

Corrections of substance:

| The first assessment says | What the code shows |
| --- | --- |
| `agent-runner` is the "directly overlapping" component | It is one user's queue for runs a pattern requests from CF's own harness. Its first code landed 2026-09-21 and it became a package on 2026-10-06. Each runner judges lease expiry by its own wall clock (`packages/agent-runner/src/agent-runner.ts:376-383, 420-428`). A refused renewal does not stop the losing run (`:562-567`), and a test asserts a silent but live run is executed a second time. The component that overlaps Locust is `packages/connectors/agents` (below). |
| CF's runtime and Locust's file proposals "solve a different problem" | For code changes, CF's piece source lifecycle is the same problem, solved with the opposite choices (below). |
| Information-flow control is framed as confidentiality; the default sink ceiling map is empty | What transfers to Locust is the integrity half: who influenced a record, and which inputs carry authority. CF's write-side enforcement in the pattern runtime is on by default (`packages/runner/src/runtime-presets.ts:389-405`). The real gaps sit elsewhere: harness tool calls check authority, not labels (`packages/cf-harness/src/prompt-loop.ts:2918-3034`); the products run in observe mode; llm sinks and `sqliteQuery` are published as ungated; `/api/ai/*` is unauthenticated by CF's own access-control spec. |
| The sink inventory "requires an explicit policy entry for every registered runtime sink" (lesson 4) | Literally true, but the totality check compares two hand-written lists in one file, and it has already drifted. Served `compileAndRun` stages a sink request under a name absent from `KNOWN_SINKS` and from the posture report (`packages/runner/src/builtins/compile-and-run.ts:502-507`, added eight days after the registry). Locust's `operations!` macro in [api.rs](../crates/locust-proto/src/api.rs) derives the operation list and the match from one list with no wildcard, which is the stronger form. |
| Lesson 2: CF "omits usage when it is unavailable", so incomplete totals are not treated as complete | True for cost, not for tokens. `sumHarnessModelUsage` sums token fields over whichever turns reported them, with no coverage marker (`packages/cf-harness/src/model/usage.ts:96-136`); the typed contract is `client.ts:95-141`. Locust's [acceptance evidence script](../scripts/client_qualification/acceptance_evidence.py) already handles per-client usage more carefully: per-field missing reasons, and cumulative counters differenced only within one native thread. CF's real additions are a typed vocabulary for why a cost estimate is withheld, no dollar figure on subscription runs, and a 429 shown as "usage limit reached". |
| Lesson 5 credits Locust with a network harness and TLA models | Locust also has `node::sim`: production nodes over an in-memory store with seeded partitions, restarts, clock offsets and restores, exact seed replay, and automatic shrinking of failing seeds ([sim](../crates/locust-core/src/node/sim/mod.rs)). The [TLA case registry](tla/cases.json) has 48 reachability witnesses and 22 deliberate mutations, each required to fail a named property. The real gaps are elsewhere: sweeps print what they exercised but assert no floors, and nothing runs unattended. |
| "How much is public" lists what is missing | It omits what is present: 184 public spec files (about 95,000 lines), covering memory v2 with two TLA+ models, server-side execution, sandboxing, the FUSE filesystem and the agent harness. The private part is the CFC prose, its Lean development and a paper (`docs/development/cfc-spec-correspondence.md:15-22`). |

The architecture table also needs a trust-base row. CF commits carry no
signature. Authority comes from a signed session open, after which the server
attributes commits itself (`packages/memory/v2/session-open-auth.ts`). Client
pending writes live in memory and are lost when the process ends. The server
runs WAL with `synchronous=NORMAL` for ordinary commits. CF's own security
roadmap (`docs/plans/security-privacy-roadmap.md`) says operators can reach
production data. Locust signs every record, checks it on receipt and trusts no
operator.

## What the first assessment missed about Common Fabric

### The coding-agent connector

`packages/connectors/agents` (first commit 2026-08-17, 45 commits) is one
person's control plane for the coding agents they already use. One driver
interface sits over the Claude Agent SDK, the Codex app server (JSON-RPC over
stdio) and ACP, the Agent Client Protocol. Each driver advertises what it
supports. Commands arrive on queues only the owner can write
(`host/README.md:141-158`). Paths in this section are under
`packages/connectors/agents/`.

- Before calling the provider, the worker fsyncs an in-flight receipt locally
  and publishes it. If publication fails, the provider is never called
  (`connector/src/commands.ts:583-623`; `command-ledger.ts:233-272`).
- On restart, an in-flight receipt becomes `unknown` and is never retried
  automatically (`command-ledger.ts:474-504`). Unknown is scoped to one command;
  the next command for the session runs normally.
- One session's commands run in order; a cancel skips the queue once the driver
  says the turn can be interrupted.
- With no person present, Codex approval requests are declined and ACP
  permission requests are cancelled. Only an explicit `allowDangerFullAccess`
  switches to approval `never` and `dangerFullAccess`
  (`drivers/codex-app-server.ts:20-43`; `drivers/acp.ts:166-167`).
- No driver gives the agent tools: Claude gets no `mcpServers`, hooks or
  `settingSources`, and ACP gets an empty MCP server list.
- Two machines can claim one command; CF says this needs outside coordination
  and leaves it unsolved (`connector/docs/interfaces.md:235-241`).

Locust already has the execution half:
[managed launch](../crates/locust-adapter/src/managed.rs) resumes Codex, Claude
Code, Droid, Kimi and pi headlessly, records `Launching` before the spawn, and
turns an interrupted launch into `Unknown`. What Locust lacks, and CF has not
built either, is the trigger. Every CF command producer is a person's click: the
debug view's confirmed submission, or a workbench Start button that shows the
kickoff prompt before sending. So the connector is prior art for how to run a
woken turn safely, not for deciding when to wake. That decision is owner
question 5 of the [public-goals plan](../docs/joinable-farms-plan.md), and the
master plan leaves starting or waking an agent out of v2. Locust has no
research on ACP or the Codex app server as launch routes.

### CF's own dogfood board

The closest CF analogue to a Locust goal is internal: Topics, a multi-user
tracker started 2026-07-09 as "the team's own issue-tracker replacement"
(`packages/patterns/topics/README.md`), plus the agents connector and the topic
and person workbenches (2026-09-16 to 22) that start a Claude Code session with
a topic's context. People hand out the work; agents act through the `cf` CLI
under their person's key, and the Topics skill tells them not to mint an agent
key (`skills/topics/SKILL.md:27-46`). There are no claims, attempts, review
rules or file proposals; code lands through GitHub PRs. Activity is cooling: in
the last three weeks commits touching Topics fell from 24 to 8, and those
touching the connector from 23 to 16, against the three weeks before.

CF's own team coordinated its largest many-agent build (server execution,
2026-07-28 to 08-02) with one orchestrator, a 127 KB markdown state file on the
branch, a cap of three subagents and per-item commits. Its team tools held
almost none of it. That is the realistic baseline Locust has to beat.

Signals that would raise competitive risk: Topics gaining statuses or
assignees; connector commands issued by agents rather than people; sessions
shared beyond their owner; CF's planned `AgentActor` landing; an MCP surface in
cf-harness.

### Files: source revisions and a writable mount

CF has two paths from an edit to shared state. Neither is in the first
assessment.

- The piece source lifecycle (`docs/specs/piece-source-lifecycle.md`;
  `packages/piece/src/ops/piece-controller.ts:4795`) keeps an append-only,
  commit-guarded revision log. Agents reach it through `read_piece_source` and
  `revise_piece`. CF's own documents list its gaps: a revision keeps only the
  files reachable from the entry, so a revert loses the rest; the base check is
  optional and not atomic; a revision records no author; and every `.src` save
  through the mount is its own revision. Locust's
  [workspace contract](../docs/workspace.md) has complete manifests, an exact
  signed parent and a signed author, and makes a revert an ordinary proposal.
- The writable FUSE mount (`packages/fuse`) is mounted into agent sandboxes at
  `/fabric`. It replies success to flush, release, rename and others before the
  write commits, registers no fsync, has a stale-cache problem on macOS
  (FUSE-T ignores invalidations), and decides when a markdown save has settled
  with a 25 ms timer. A compile failure still looks like success to the
  editor. This is direct evidence for keeping Locust's explicit
  `workspace update` rather than a live mount.

CF also keeps "applied" apart from "verified" and makes `not-checked` an
explicit marker in what a subagent returns
(`packages/cf-harness/src/contracts/subagent.ts:262-296`): "omission does not
establish verification".

### Authority and identity

- **CF keeps keys away from one agent only: its own sandboxed model.** The
  coding agents CF runs sign as their person, by policy. Its plan lists "agent
  uses its human's key, declares itself" as the current arrangement and
  delegation as undelivered (`docs/plans/pattern-verb-contract.md:913-937`).
  CF's own conformance manifest admits that nothing limits which surface may
  mint the direct-command role (`packages/cf-harness/audit/conformance-manifest.ts:107-136`).
  CF is therefore no precedent for keeping a person's authority from an agent
  that has a shell. Its posture matches Locust's: instruction text plus the
  harness's own permission mode.
- **In CF an agent is a label** (`agentName`) on its person's principal.
  Attribution claims, writer policies and pattern roles bind only honest
  runtimes; the memory server checks space capability alone
  (`packages/memory/v2/server.ts:2867-2894`). CF cannot host Locust's public
  door, and a CF label or roster entry must never count as a Locust approval.
- **Keys:** a passphrase becomes a key through one unsalted SHA-256; derived
  keys are `SHA-256(sign(root, name))`; there is no rotation; a toolshed started
  without `IDENTITY` runs as the public "implicit trust" key with only a
  warning. If Locust ever exports or seals a governance key, it should use a
  salted memory-hard KDF and domain-separated derivation, as `seal.rs` already
  does.
- **Worth taking: revocation that carries its evidence.** On an access change,
  CF drops sessions that lost read access and sends a terminal
  `session/revoked` with no reopen loop (`packages/memory/v2/server.ts:2951-3030`).
  It scopes a permanent refusal to the access-list revision, so a later grant
  lifts it without a timer. Locust leaves a removed computer dialing, signing
  records nobody takes, while hooks keep its agents working. The master plan
  left "telling a computer that it was removed" out of v2 on purpose, so this
  is input for later.

### Who, how big, and where it is going

- Common Fabric, Inc. was renamed from Common Tools on 2026-09-16 (commit
  `e61c76835`). labs has 11 human committers since June. Between 2026-06-01 and
  2026-10-07 it landed 4,165 commits (PRs #3776 to #8537): 543, 731, 1,240 and
  1,455 in June to September, and 196 in October's first week. 3,184 of them
  (76%) carry an AI co-author trailer.
- CF's own July survey lists 24 active organization repositories. The four that
  are public today hold about 26% of their tracked files
  (`docs/history/packages/cli/cf-view-language-coverage-2026-07.md:41-71`).
  Product code (Loom, the Weaver app, fabric-mobile, pattern-factory) is
  private.
- labs has been 0BSD only since 2026-07-13 (`6ddae9998`); before that it had no
  license file. Locust is AGPL-3.0-only. Locust may adapt CF code from after
  that date; CF cannot take Locust code into labs without AGPL terms.
- labs has no MCP or A2A surface, so no protocol interop path exists. CF's
  agent-facing surface is the `cf` CLI plus skills, the same shape as Locust's
  CLI plus skill. Both projects meet only at the session-driver layer, where
  both drive the same third-party agents.
- Direction, inferred with medium confidence: a Loom and Weaver launch from
  the waitlist, server execution and shared spaces across servers, finishing
  CFC, and social features. In the last three weeks, commits touching fabrichat
  went from 0 to 24 and space access from 0 to 19; the coding-agent pieces are
  cooling.
- Positioning: CF publicly treats prompt injection as a dataflow problem (its
  public `promptinjection-wtf` repository and `docs/how.md`). Locust's planned
  public door relies on prompt text and on agents with unsandboxed shells, and
  has no accepted threat model.

## Locust risks the comparison surfaced

These are Locust findings that the comparison brought to light, ranked by what
a person would lose. Each was confirmed in Locust code or documents by a
verifier.

### 1. An old chat's MCP bridge can send the wrong operation after an upgrade

Checked by hand. The local API is postcard, so a request is identified by its
position in the `Request` enum ([codec.rs](../crates/locust-proto/src/codec.rs)
lines 1-5). The handshake refuses only a different API number. The number has
stayed 7 since `65aecf1`, while the enum was edited mid-list: `3196be8` removed
and inserted variants, and `8c086c1` inserted `role.give` and `role.take` before
`rules.bind` and added a field to `goal.invite`. The codec's "only ever
appended" rule is already broken, as the plan intends before release. But a
chat's MCP bridge lives as long as the chat, and hooks and the CLI run a newer
binary against an older daemon.

The concrete case: R9 removes `workspace.integrate`, which precedes
`contribution.inspect` and `completion.declare`. Those two have identical field
layouts ([api.rs](../crates/locust-proto/src/api.rs)), so an old bridge's
inspect would decode on the new daemon as a completion declaration and could
commit. A decode failure closes the socket and reads as "not answering", which
invites retries. S3 leaves the API out of its release gate.

Proposal: put a digest of the request and response layout (variant order plus
`request_schema()`) in the client hello, refuse a mismatch with a fixed line
("restart this chat's Locust MCP server or start a new chat"), and add the
digest to S3. Use the layout, not the build string: `<commit>-dirty` is the same
for different dirty trees. Land it before R9.

### 2. The public door: a stranger's text can reach owner authority

Locust's docs admit that an agent with a shell can run owner commands and call
the coding agent's approval prompt the guard
([concepts](../docs/guide/concepts.md), "Only you"). The public-goals plan
also admits that trusted agents read strangers' text. No document joins these
into one chain:

- A trusted agent reads a door member's finding, task or file change. The host's
  own agent is one of them, by design.
- That agent's shell can read `owner.credential`, a mode-0600 file of the same
  user ([local.rs](../crates/locust-proto/src/local.rs) lines 84, 244-246).
  Nothing checks the peer, a terminal or the environment. Confirmation is a CLI
  step the agent can complete itself; the daemon has no notion of it. The
  installed skill and the agent's pending view print ready-to-run `--owner`
  lines.
- From there, `role give` makes the stranger trusted at once. A plain approval
  is enough to make a stranger's result count and auto-land. `goal continue`,
  which lifts the restore guard, is not even confirm-class today.
- The guard the docs name is usually off for unattended agents. Both real
  trials ran Claude Code with `--permission-mode acceptEdits` and Bash allowed,
  and Codex with approvals `never`
  ([runner](evidence/role-free-board-2026-10-05/runner.py) line 108;
  [live farm demo](../scripts/live_farm_demo.py)). CF states this constraint
  outright: a headless session in default mode cannot pass an approval.
- Shared trees can land `AGENTS.md`, `CLAUDE.md`, `.claude/`, `.codex/`,
  `.mcp.json` and `.github/workflows` in members' checkouts after one trusted
  approval. Harnesses grant these files authority without the model judging
  them. Locust's path deny list is checked on every incoming manifest but does
  not include them.
- A trusted agent can restate a stranger's suggestion as its own task. The plan
  accepts this as a limit.

Per-agent keys do not help on one computer: every agent credential, the owner
credential and every signing seed are files of one OS user. Locust's gain over
CF is attribution that honest daemons check and an MCP surface that refuses the
owner credential, not containment.

The owner has since ruled that guarding owner commands against the person's
own agent is out of Locust's scope (master plan answer 31). No presence check
or other local enforcement is proposed. What remains are proposals that
enforce nothing against the person, to settle in the public-goals plan before
the door ships: accept a one-page threat model in `docs/`; state plainly in
the agents and concepts guides that owner commands are not a boundary against
an agent with a shell, and that an unattended agent usually runs with
approvals off; show harness instruction paths in review and `workspace update`
output; and keep peer text out of everything Locust writes into a prompt
(fixed words, IDs and counts only).

### 3. Approvals carry no evidence that a check ran

CF accepted no subagent's "green" until the orchestrator ran the item's verify
command itself. Its records show agents reporting green with a gate red, a
runner that exited 0 on a red suite, and a type-check leg that checked nothing.
In Locust, `ReviewRecorded` is subject and verdict; `CheckAttested` is a name
and `passed: bool`, and counts when the rule names the attester, the author
included ([event.rs](../crates/locust-proto/src/event.rs); fold.rs lines
1213-1226). In v2 an approved change lands by itself (answer 15), and a later
reject does not undo it (answer 6). Only skill text says "Approve only what you
checked".

Answered by the owner (master plan answer 33): Locust requires no check. A
check stays an option a formation may name, as it can today, and Locust should
not impose guardrails on how agents work. No change is proposed here.

### 4. Catching up refolds the whole goal once per received batch

Checked by hand. `Goal::apply` always calls `refresh`, which rebuilds the chain
and folds the whole history ([goal/mod.rs](../crates/locust-core/src/goal/mod.rs)
lines 109-150). Every received `Events` frame lands as its own transaction, so a
catch-up of N events costs about N/256 full refolds of a growing history, on
the one engine thread that serves every goal
([worker.rs](../crates/locust/src/daemon/worker.rs)). Measured: 512 tasks replay
in 20.4 ms and ingest in 128-event batches in 205.6 ms
([performance pass](performance-cost-pass.md)). About 25 s at 41,000 events is
an extrapolation, not a measurement. S2 stops its batch modes at 2,048 tasks and
does not measure this case.

Proposal: add a sync catch-up mode to S2 (total time and longest frame). If it
is near J6's bounds, try in order: refold once when an exchange's stream drains;
an incremental path when every inserted event is a work author's tip (history
inserts are already classified, but nothing reads the class); then the deferred
fold checkpoint.

### 5. A second running copy of the host's data folder is adopted silently

Checked by hand. When a daemon receives records signed by a key it holds, above
that key's mark, it raises the mark and moves on
([commit.rs](../crates/locust-core/src/node/commit.rs) lines 255-300). The
comment explains why: catch-up after a restore. No hold covers a live twin
([guard.rs](../crates/locust-core/src/node/guard.rs)). The host-safety plan
calls two running copies undetectable, but this is the point where a copy can
see its twin, whenever the twin's record arrives before it signs at that
position. CF fences a second server process with a lease judged by the memory
server's clock, which Locust cannot copy.

Proposal: for the governance key only, when a received batch carries a record
above its mark and the goal has no open restore record, hold that key and say
that another computer is signing as this host. This uses no clock and decides
nothing shared. Add a two-holder case to the restore-guard model. It cannot
fence two copies signing at the same moment.

### 6. S3 freezes bytes, not judgement

S3 digests the signed vectors and the store layout. Its fixture test checks that
every recorded field is present ([store plan](../docs/agent-memory-and-store-plan.md),
S3). The plans change replay rules many times within protocol 7. After release,
two builds with the same protocol number could sync cleanly while disagreeing
about standings, landings and membership, and the host's computer auto-lands on
its own judgement. CF's invariant catalog requires "identical apply semantics"
on every side (`docs/specs/memory-v2/09-invariants.md:383-400`), though only
within one build; across builds it negotiates feature flags.

Proposal: a third S3 digest over a stable projection of `Goal::load` on frozen
multi-author transcripts (standings, selections, refusal codes, members), keyed
by protocol number. Do not hash `Debug` output, as the existing fingerprint in
`organization_performance.rs` does.

### 7. Smaller defects found on the way

- **A replacement proposal silently takes the current head as its parent.**
  Checked by hand. `workspace propose --replace` without `--parent` falls back
  to the head ([workspace.rs](../crates/locust/src/cli/workspace.rs) lines
  625-631), although the guide documents `--parent` as required and the error
  text says so. The daemon skips the frozen-base check for replacements. A
  replacement captured from an older folder therefore becomes a proposal on the
  new head and, if approved, reverts what landed in between. Shared documents
  have the same gap: `doc.revise` takes an optional base that the fold does not
  check against the selected text.
- **A stale attempt blocks automatic pickup indefinitely.** `Goal::unattended`
  counts any attempt with no status or `Progress` as attending its task
  ([goal/mod.rs](../crates/locust-core/src/goal/mod.rs) lines 452-466). If the
  author's chat closes without reporting, the task drops out of automatic
  selection and hook nudges. Ephemeral peer presence, unsigned and advisory as
  in CF's presence relay, would let this local choice see that the author is
  gone.
- **A lost exit report erases a known outcome.** If `client run`'s final
  `Exited` report fails, the record stays `Started`, the next run is refused,
  and `client recover` writes `Unknown` ([client.rs](../crates/locust/src/cli/client.rs)
  lines 196-204, 706-724). Unknown also blocks every later launch of the
  session; CF scopes it to one command. Any wake would turn that into a wait on
  a person.
- **Idempotency keys exist only if the model invents one.** Generic writes take
  an optional key and nothing asks for it on the first attempt. Workspace writes
  and `goal add` already mint keys. Stored responses are kept forever, keyed by
  principal, and a replay carries no mark. CF marks a deduplicated call.
- **Signed farm requests are not bound to the receiving service.** The signing
  digest has no service origin. Fix before J3 freezes the format.
- **Anti-entropy cost grows with every author ever admitted.** Both sides send
  the whole frontier at least every 30 s; at 1,024 door seats that is about
  68 KB/s per goal, and frontiers above 4,096 authors are refused. J6 does not
  re-measure the idle bound at large author counts.

## How Locust is built

CF lands about 1,000 commits a month with agents doing most of the writing.
Each change is a PR tested on its exact commit, behind about 25 named checks,
and each check says what a pass does not prove (`AGENTS.md:307-460`). Findings
about Locust's own process:

- **A known intermittent test is untracked.**
  `a_diverged_author_log_reconciles_between_two_real_daemons` failed in five
  recorded sessions (R4 fixes, R5 fixes, R6 build, hooks qualification, LAN
  sync) and was put down to timing each time. It fails at the first publish
  after a restart ([reconcile_tests.rs](../crates/locust/src/daemon/reconcile_tests.rs)
  line 413, with no readiness wait). R6 recorded the error, "the candidate cannot
  be applied yet". G1's notes list exactly that conflict as a known product gap
  and say the helper retries. The LAN note reproduced it in 7 of 7 crash runs and
  then dismissed the cargo failure as unrelated. `a0dff5e` and `5e71dc7` have
  since fixed the new-member part of that conflict; no document tracks the
  restart case. CF bans waits that the starting
  state already satisfies and tracks flakes as defects
  (`docs/development/waiting-in-tests.md`).
- **Build state is restated in at least seven live places that disagree.** The
  master plan's header and build-order table, its pieces table and its review
  section disagree with each other. The roles and host-safety plan headers say
  less is built than has landed. [status.md](../docs/status.md) still lists
  hooks under "Not built yet". CF keeps one live status block per plan.
- **The shipped skill grew 74%** (11,678 to 20,349 bytes since `ff1686d`) after
  the ergonomics audit proposed a short skill. It grew in each of 13 commits and
  has no size test. CF's skill rule is "write the map, minimize the procedure".
- **Retired words survive in daemon and CLI strings** ("principal",
  "participant", "permissions") that agents read. The word tests cover the guide
  and the skill only.
- **Verification runs on a checkout other sessions are editing.** Binaries
  report `-dirty`, and sessions have fallen back to snapshots. The G1-fixes lane
  (own worktree, gates there, fast-forward landing) is a working pattern to make
  the default.
- **The two README indexes are the main merge hotspot.** In 9 of 16 merge
  commits, both sides had changed the same index file. CF merges its busiest
  index with git's union driver and checks it is one line per entry
  (`.gitattributes:4-9`).
- **The models never run unattended.** Locust has no scheduled workflow. The
  TLA fast suite, `sim_many` and the scripted-client runs run only by hand.
  Measured wins (the compact catalog dropped from 366,531 to 32,300 bytes) have
  no regression budget. CF gates deterministic counts in CI and pairs each
  limit with a negative control (`docs/development/BENCHMARKS.md:1329-1380`).
- **Hook delivery to the model is proved only by paid runs.** The scripted
  provider already observes each request body, but the scripted harnesses set
  `LOCUST_HOOKS=off`. A scripted native-hooks mode would prove delivery for
  free.

## Smaller ideas worth keeping

| Idea | Source in CF | Use in Locust |
| --- | --- | --- |
| Follow the shared head at a natural moment and record the outcome | piece source lifecycle (follow-on-open) | A fixed hook line: "files moved: update clean" or "update blocked" |
| Rotating seeds and floors on what a random run exercised | `docs/development/TESTING.md:167-232`; differential harness floors | Assert floors per fault and restore kind in `sim_guide_under_faults`; derive the seed window from the commit |
| A seeded metamorphic fold check | differential consistency harness | Every delivery order, batch split and duplicate of a generated history folds to one evaluation; today 2n hand-picked orders are checked |
| Correcting a finding by someone other than its author | many-agent build records | Show on the A3 page the newest finding that cites a headline in `sources` |
| Standing knowledge that progress notes cannot push off | build records ("do not rediscover") | A pinned or "known trap" class of finding, listed apart |
| A readable host direction | owner rulings at the top of the state file | Return `context.guidance`; consider a short host-signed direction |
| "Is firing early safe?" for every remaining local time bound | fetch deadline docs | Add to the reading of principle 3 |
| Whose account pays | `docs/plans/cf-harness-codex-subscription-auth.md` | A wake records the billing environment the person chose and never falls back to the service's; the door plan names quota use by strangers |
| An admission hint | `session/admissible` | The host dials endpoints it refused at the door when a seat frees |
| A census of every text that reaches the model | prompt-stack audit | Extend the model-text test to hook lines and daemon refusals; tie numbers in the skill to their constants |
| A sender-minted session ID | Claude driver | Would give a woken launch a known session before it starts |

## What not to copy

Additions to the first assessment's list: CF's test-records and test-selection
machinery, built for a large flaky browser suite Locust does not have; a
writable live mount; any receipt claim that needs cross-machine coordination;
copying agent transcripts into shared state (CF keeps them owner-only, as
Locust keeps them out); server-held webhooks and OAuth integrations; per-event
lease writes; and borrowing a vendor's OAuth client to use a subscription
outside the official client. Also CF's "clean break, no migration" storage
policy, which turned into ten migrations on every open and two value formats
at rest. That is a caution for Locust's principle 2, not a model.

## Questions for the owner

1. Should a change to the shared files land only after another agent has
   recorded that it actually ran the change's check? Today an approval lands it
   with no such record.
2. Before strangers can join, should the few acts that make a stranger trusted
   or lift a safety hold (giving a stranger a role, continuing after a restore,
   opening the door) need you present, for example by Touch ID? Or should Locust
   only state the risk? Answered: no enforcement; it is beyond Locust's scope
   (master plan answer 31).
3. Should Locust wake your own agent when work is waiting for it? CF's connector
   shows how to run such a turn safely, but not when to start one. Answered:
   "Ideally, yes" (master plan answer 32). Not designed yet.

Question 1 drew "what check?": it used Locust's word without saying what the
check is. Restated as running the project's tests on the exact proposed files,
it was answered: no requirement; a check is an option in a deterministic
formation (master plan answer 33).

## Suggested order

- Before R9: the API layout check (risk 1).
- Before the door: the threat model and the door items of risk 2, and the farm
  request binding.
- Cheap now: the replacement parent, the flaky test and its restart race,
  `status.md`, the union merge driver, retired words in daemon strings.
- With S2 and S3: the catch-up mode and the fold digest (risks 4 and 6).
- With G1's follow-ups: the twin hold (risk 5).
- Next for the wake (answer 32): a design note covering when to wake, which
  sessions may be woken, and ACP and the Codex app server as launch routes,
  with CF's connector as the reference for running a woken turn.
