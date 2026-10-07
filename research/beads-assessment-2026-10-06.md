# beads and Locust: storage, agent memory and ideas to adapt

Status: source assessment, 2026-10-06; recommendations are proposed, not
implemented Locust behavior.

Inspected [beads](https://github.com/gastownhall/beads) at commit
[`bb9b9e502023f8bbd115961f55f8ff724e07d78b`](https://github.com/gastownhall/beads/commit/bb9b9e502023f8bbd115961f55f8ff724e07d78b),
whose recorded commit date is 2026-10-07 (UTC). `git describe` gives
`v1.3.0-402-gbb9b9e502`: the commit is 402 commits after v1.3.0, and the
v1.3.1 tag sits on a release branch, not on this commit. Locust baseline is
`756c896f9eb671e40b1f0c95a547fff6583c9b27`, read with other sessions'
uncommitted edits in place. This investigation read beads source, docs, tests,
git history, and GitHub issues and pull requests through `gh`. It did not
build, install or run beads, and it ran nothing in Locust. All beads code
links pin the inspected commit. **Proposal** marks something not built;
"today" means what the cited Locust code does at the baseline.

## Conclusion

beads (`bd`) is a Go issue tracker for coding agents. Every clone holds a whole
Dolt database, a SQL store with git-style versioning, and syncs it with Dolt
push and pull ([dolt.md:158-228](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/docs/architecture/dolt.md#L158-L228)).
Rows are mutable state, the author is a free string, and a cell both clones
changed is settled last-writer-wins by `updated_at`
([automerge.go:28-36](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/versioncontrolops/automerge.go#L28-L36)).
Locust differs on each point: a signed append-only log per goal, folded in
memory, where no clock or arrival order decides anything shared
([event.rs](../crates/locust-proto/src/event.rs) line 32;
[master-plan.md](../docs/master-plan.md) lines 80-90).

So beads offers Locust little on storage or trust. Its value is a year of
reversals that ended near where Locust started, and a set of measured
agent-facing failures. The strongest recommendations:

1. Give every agent tool a real description and cap a context page in the
   daemon (idea 1). beads measured that an unnamed read verb is never used
   ([#6626](https://github.com/gastownhall/beads/issues/6626)).
2. Let `attempt.start` with no task pick and start in one step on this
   computer (idea 2), as `bd ready --claim` does on one store.
3. Show a capped index of current goal findings, and later let an author
   retire their own finding (idea 3). beads' own memory redesign points the
   same way ([#5877](https://github.com/gastownhall/beads/issues/5877)).
4. If the owner accepts hooks, inject only facts at session start, under
   beads' output contract (idea 4).
5. At the first real release, make the release script refuse a published
   version whose signed bytes changed (idea 11). beads shipped a wire-shape
   change under an unchanged version ([PR #7182](https://github.com/gastownhall/beads/pull/7182)).

## What beads is

**Storage.** Embedded Dolt runs inside `bd` behind an exclusive file lock, or
`dolt sql-server` serves many writers
([store_factory.go:127-141](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/cmd/bd/store_factory.go#L127-L141);
[dolt.md:158-228](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/docs/architecture/dolt.md#L158-L228)). Each write command
makes one Dolt commit ([main.go:1964-1975](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/cmd/bd/main.go#L1964-L1975)).
Sync is Dolt push and pull, usually to `refs/dolt/data` on the git remote
([dolt.md:427-430](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/docs/architecture/dolt.md#L427-L430)), with a per-table
auto-resolve policy for conflicts
([mergesettle.go:380-430](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/versioncontrolops/mergesettle.go#L380-L430)).

**IDs.** Since v0.20.1 (2025-10-31) issue IDs are short base36 hashes whose
length grows with the database by the birthday bound
([CHANGELOG.md:9927-9953](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/CHANGELOG.md#L9927-L9953);
[adaptive-ids.md:17-46](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/docs/core-concepts/adaptive-ids.md#L17-L46)).
Child IDs still come from a replicated counter and collide across machines
([#4796](https://github.com/gastownhall/beads/issues/4796), open).

**Work graph.** Ready work is a SQL filter that reads a stored `is_blocked`
column ([ready.go:117-118](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/sqlbuild/ready.go#L117-L118)),
recomputed inside writes and after merges
([blocked_state.go:141-200](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/issueops/blocked_state.go#L141-L200)).
A claim is a compare-and-set on a random `row_lock` cell in one transaction
([claim.go:119-140](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/issueops/claim.go#L119-L140)). Leases
last 5 minutes and are local to the granting clone
([lease.go:26](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/issueops/lease.go#L26);
[0055:1-18](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/schema/migrations/0055_move_leases_to_table.up.sql#L1-L18)).

**Memory and `bd prime`.** `bd remember` writes flat key/value rows into the
replicated config table ([kvkeys.go:18-28](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/kvkeys/kvkeys.go#L18-L28))
with no author or timestamp ([prime.go:634-635](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/cmd/bd/prime.go#L634-L635));
a conflicting memory resolves to the remote copy
([mergesettle.go:648-656](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/versioncontrolops/mergesettle.go#L648-L656)).
`bd prime` prints rules plus memories, and a Claude Code SessionStart hook runs
it, which also fires after compaction
([claude.go:283-296](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/cmd/bd/setup/claude.go#L283-L296)). Semantic
compaction replaces closed issues' text with a summary
([compact.go:46-48](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/cmd/bd/compact.go#L46-L48)).

**Reversals.**

- 2026-01-14: Dolt backend added ([`1dc36098a`](https://github.com/gastownhall/beads/commit/1dc36098a)); the default
  for new projects from v0.50.0, 2026-02-14
  ([CHANGELOG.md:6086-6090](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/CHANGELOG.md#L6086-L6090)).
- 2026-02-09 to 02-18: the daemon ([`326976614`](https://github.com/gastownhall/beads/commit/326976614),
  [`b136996b9`](https://github.com/gastownhall/beads/commit/b136996b9)), the three-way merge engine
  ([`510251291`](https://github.com/gastownhall/beads/commit/510251291)), tombstones ([`7eed8b8a1`](https://github.com/gastownhall/beads/commit/7eed8b8a1)),
  JSONL sync ([`8e10fd09d`](https://github.com/gastownhall/beads/commit/8e10fd09d)) and the sync branch
  ([`ff7244b61`](https://github.com/gastownhall/beads/commit/ff7244b61)) were deleted.
- 2026-02-08 to 02-24: one Dolt branch per worker, then one shared branch
  again ([`764ad0ce2`](https://github.com/gastownhall/beads/commit/764ad0ce2);
  [dolt-concurrency.md:3-40](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/engdocs/design/dolt-concurrency.md#L3-L40)).
- 2026-02-21/22: embedded Dolt removed ([`e2e341053`](https://github.com/gastownhall/beads/commit/e2e341053),
  [`4766a4f3d`](https://github.com/gastownhall/beads/commit/4766a4f3d)); default again in v0.63.0 on 2026-03-29
  ([`d1f4784d3`](https://github.com/gastownhall/beads/commit/d1f4784d3)).
- 2026-04-14 to 08-08: per-clone metadata (0028), leases (0055), the audit
  table (0062) and the journal (0064) left the versioned plane because of
  merge conflicts and history growth
  ([0055:4-11](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/schema/migrations/0055_move_leases_to_table.up.sql#L4-L11);
  [0062:4-6](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/schema/migrations/0062_events_dolt_ignore.up.sql#L4-L6)).
- 2026-05-10 to 07-29: auxiliary row IDs went from auto-increment to random
  UUID to content-derived (migrations 0037, 0050, 0061; [#4259](https://github.com/gastownhall/beads/issues/4259)).
- 2026-07-16/18: Postgres, MySQL and SQLite backends removed
  ([`cafb6888b`](https://github.com/gastownhall/beads/commit/cafb6888b), [`6fd9dbda7`](https://github.com/gastownhall/beads/commit/6fd9dbda7)).
- 2026-07-31: an HTTP server, `bd serve`, added ([`e988fd002`](https://github.com/gastownhall/beads/commit/e988fd002)).
- 2026-08-11 to 08-15: v1.2.0 and v1.2.1 were published untested; one run of
  v1.2.1 moved databases to schema v65; v1.2.2 re-shipped the v1.1.2 tree
  ([v1.2.2 gate:3-8](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/release-gates/v1.2.2-recovery-release-gate.md#L3-L8)).
- 2026-10-04: a golden wire-shape digest tied to a revision counter
  ([`5439be97b`](https://github.com/gastownhall/beads/commit/5439be97b), PR #7182).

## Parallels and differences

| Topic | beads | Locust | Note |
| --- | --- | --- | --- |
| Full copy per participant | Whole Dolt database per clone ([dolt.md:427-430](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/docs/architecture/dolt.md#L427-L430)) | Whole signed log per member ([sync.rs](../crates/locust-proto/src/sync.rs) 85-107) | Both grow on every machine. |
| One local writer | Embedded flock ([store_factory.go:127-141](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/cmd/bd/store_factory.go#L127-L141)) | SQLite `locking_mode=EXCLUSIVE` ([connection.rs](../crates/locust-store/src/connection.rs) 20-32) | Outside tools cannot read a store in use. |
| Content-derived identity | Short hashes, collision retries ([hash.go:55-84](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/idgen/hash.go#L55-L84)) | 256-bit BLAKE3 IDs ([event.rs](../crates/locust-proto/src/event.rs) 856-858) | Locust stores full IDs and shortens only for display. |
| Local versus replicated plane | `dolt_ignore` tables, added over five migrations | 13 local spaces from the start ([store.rs](../crates/locust-proto/src/store.rs) 69-102) | Same line, drawn earlier. |
| Claims and liveness | Node-local leases ([0055:13-17](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/schema/migrations/0055_move_leases_to_table.up.sql#L13-L17)) | Session claims in `Space::Claim` ([sessions.rs](../crates/locust-core/src/node/sessions.rs) 87-114) | Both enforce only where granted. |
| Unknown format | Refused, then migrated ([schema.go:230-307](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/schema/schema.go#L230-L307)) | Refused, never converted ([lib.rs](../crates/locust-store/src/lib.rs) 95-98) | Locust has no migration path at all. |
| Contested value | Last writer wins by `updated_at` | Separate signed facts; a fork halts one stream ([fold.rs](../crates/locust-core/src/goal/fold.rs) 961-991) | beads stays live and loses data; Locust can stop a scope. |
| Derived state | Stored `is_blocked` | Full refold per batch ([goal/mod.rs](../crates/locust-core/src/goal/mod.rs) 97-139) | Stale caches versus linear cost. |
| Durable knowledge | Unattributed key/value memories | Signed goal-scope contributions with no kind or retirement ([event.rs](../crates/locust-proto/src/event.rs) 465-471) | Opposite gaps. |
| Session-start context | Pushed by hook | Pulled by the agent ([SKILL.md](../skills/locust/SKILL.md) 43-71); no hooks ([agents.md](../docs/guide/agents.md) 81-82) | |
| Ending work | Free-text close reason parsed by regex ([#5577](https://github.com/gastownhall/beads/issues/5577)) | Typed attempt status ([event.rs](../crates/locust-proto/src/event.rs) 284-290) | Locust already has typed outcomes. |

The differences that matter:

- **Trust.** No beads record or commit is signed, and the actor is a
  caller-chosen string ([main.go:869-901](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/cmd/bd/main.go#L869-L901)).
  Locust events carry Ed25519 signatures checked with `verify_strict`
  ([crypto.rs](../crates/locust-proto/src/crypto.rs) 113-138).
- **Conflict.** beads stays live by dropping one side
  ([automerge.go:28-36](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/versioncontrolops/automerge.go#L28-L36));
  Locust keeps every fact and can halt a scope
  ([fold.rs](../crates/locust-core/src/goal/fold.rs) 961-991), so halts must
  be visible to the person.
- **Format change.** beads has 69 migrations and still forked
  ([#4259](https://github.com/gastownhall/beads/issues/4259), [#6727](https://github.com/gastownhall/beads/issues/6727)). Locust ends goals on a signed-format
  change instead ([master-plan.md](../docs/master-plan.md) 80-82), so an
  unnoticed byte change is costlier there.
- **Context delivery.** beads pushes a digest through a hook
  ([claude.go:283-296](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/cmd/bd/setup/claude.go#L283-L296)); Locust can say exactly what a
  session has not read, but pays in bookkeeping calls, 51% of recorded MCP
  calls ([agent-ergonomics](agent-ergonomics-2026-10-05.md) 150-156).

## Ideas to adapt

Ordered by value for size. Each is a **proposal**. Two reviewers checked each
idea independently: one confirmed its claims against both codebases and the
plans, and one argued against adopting it. Where they disagreed, the choice
and its reason are stated.

### 1. Name every agent tool and cap the context page

beads: `bd prime` never named `bd recall`, so agents never used it and read
120-character previews as empty entries ([#6626](https://github.com/gastownhall/beads/issues/6626)). Its MCP layer
bounds results while the CLI JSON path stays unbounded, 454 KB for one
`bd ready --json` ([#5546](https://github.com/gastownhall/beads/issues/5546)).

Locust today: 31 of the 57 tools listed to an agent have a description equal
to the operation name ([api.rs](../crates/locust-proto/src/api.rs) 891-933).
A test already checks the tool names in the MCP instructions
([mcp/tests.rs](../crates/locust/src/mcp/tests.rs) 287). The skill sends
agents to the board for `attempting` and `verdicts`
([SKILL.md](../skills/locust/SKILL.md) 103), which live on pending items
([api.rs](../crates/locust-proto/src/api.rs) 1601, 1617). `context.read`
refuses only a zero limit ([context.rs](../crates/locust-core/src/node/context.rs)
173-178), while `events` caps pages at 256 (api.rs 124). This is already Track
5 of the [ergonomics research](agent-ergonomics-2026-10-05.md) (lines 472-484).

Proposal: for each bare tool, set `tool: false` if it only repeats another
read, or write one sentence saying what it returns and when to use it. Add one
assertion to the existing test: no listed description equals its name with
dots as spaces, and every `locust_*` token in SKILL.md is a listed tool. Fix
SKILL.md line 103. Clamp a `context.read` limit to a new maximum beside
`MAX_FEED_PAGE`. The cut must stay in the daemon, because the daemon signs the
receipt over the delivered items (context.rs 255-299).

Reviewers: one wanted a refusal, page caps on `board` and budget constants; this
note takes a quiet clamp and no constants, because no recorded run showed an
oversized page or board ([agent-ergonomics](agent-ergonomics-2026-10-05.md)
136-160).

Cost S, mostly writing. No signed-format change. No owner decision.

### 2. Start the next task in one call

beads: claim-next uses the ready list's own predicate, skips candidates it
loses, and treats an empty list as an ordinary answer
([claim.go:300-309](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/issueops/claim.go#L300-L309);
[claim_next.go:56-59](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/issueops/claim_next.go#L56-L59)).
Its refusal names the holder, never a steal command
([claim.go:184-196](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/issueops/claim.go#L184-L196)).

Locust today: `attempt.start` requires a task
([api.rs](../crates/locust-proto/src/api.rs) 574-578). `to_start` lists
attended tasks too, least-attended first
([views.rs](../crates/locust-core/src/node/views.rs) 639). Its `attempting`
leaves out the caller's own principal (views.rs 428-440), so a second session
of the same agent sees the first session's attempt as unattended. The trial's
duplicate came from a 13.7 s stale read
([role-free-board](role-free-board-2026-10-05.md) 146-150).

Proposal: make `task` optional. With none, the daemon takes the first
`to_start` item that no other member and no other session of the caller
attempts, inside the plan that signs `AttemptStarted`
([claims.rs](../crates/locust-core/src/node/requests/claims.rs) 87-148). If
none qualifies it signs nothing and returns the existing pending answer, which
needs the response check at api.rs 1170 widened. The skill says this holds on
this computer only. Naming a task still allows a deliberate second attempt.

Reviewers: one kept a hashed tiebreak and a new empty-answer type; this note
drops both, because no herding was seen and the order is an unproven
heuristic (role-free-board 237-240).

Cost S. No signed-format change. No owner decision: a local pick decides
nothing shared.

### 3. Current findings, then author supersession

beads: memories carry no author or time; 2 of 5 checked were stale after a
day, and an agent followed one over guidance it had loaded
([#5153](https://github.com/gastownhall/beads/issues/5153)). Before caps, about 80% of 102 memories never reached the
agent ([#4569](https://github.com/gastownhall/beads/pull/4569)). A key index was about 14 times smaller than full
bodies ([#7208](https://github.com/gastownhall/beads/issues/7208)).

Locust today: goal-scope contributions are signed and attributed, but have no
kind, index or way to retire one
([event.rs](../crates/locust-proto/src/event.rs) 465-471). J5 will stop
counting records from before a member's admission as unread
([joinable-farms-plan.md](../docs/joinable-farms-plan.md) 4027-4045), so a
newcomer's unread view will not show current knowledge either. A contribution kind is
proposed, not accepted ([agent-ergonomics](agent-ergonomics-2026-10-05.md)
495-498).

Proposal, step 1, no signed change: the first compact context page lists up
to 20 goal-scope findings not tied to an attempt (event ID, author name, first
line up to 120 characters), newest first, with shown and total counts and a
line naming `locust_event_show`. Step 2, with the next signed-format change and
the kind field: `supersedes` on a contribution, valid only for the same
author's goal-scope finding. Concurrent supersessions all stay current, with
no tie-break ([master-plan.md](../docs/master-plan.md) 83-90). Nothing is
deleted.

Cost S for step 1; M for step 2. Step 2 changes the signed format. Owner
decision for step 2: "Should an agent be able to mark one of its own earlier
findings as replaced, so other agents stop seeing it as current? It changes
the record format, so it waits for the next format change."

### 4. A session-start hook that injects facts

beads: the first output line tells the agent to read a truncated hook output
in full ([prime.go:522](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/cmd/bd/prime.go#L522)); caps stop at whole entries
with the notice before them ([prime.go:676-690](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/cmd/bd/prime.go#L676-L690));
a failure prints "NOT injected" with the error cut to 160 characters
([prime.go:733-750](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/cmd/bd/prime.go#L733-L750)). A forceful injected
close protocol made agents push against users' instructions
([#3451](https://github.com/gastownhall/beads/issues/3451)).

Locust today: no hooks ([agents.md](../docs/guide/agents.md) 81-82).
`status` carries claims with generation
([level.rs](../crates/locust-proto/src/api/level.rs) 131-143), but the CLI
text prints only name, roles, level and standing
([presentation.rs](../crates/locust/src/cli/presentation.rs) 705-747). The
research proposes `status` as the resume card and an opt-in hook
([agent-ergonomics](agent-ergonomics-2026-10-05.md) 419-423, 533-536); no
compaction was ever recorded (622-623).

Proposal: no new operation. The CLI `status` prints each held claim (escaped
first-line title, short attempt ID, generation) and a fixed line naming the
next reads. Setup adds one Claude Code SessionStart entry with no matcher,
running `status` through a wrapper that always exits 0, capped near 4 KiB,
with an elision line naming `locust status` and a "Locust context was NOT
injected" line on failure. No participant text beyond escaped titles (line
531). Leave Codex until a Codex compaction is observed. Land it with the
compaction scenario (655-658).

Reviewers: one folded in beads' Codex marker design; this note defers it,
because nothing has measured the need.

Cost S for the renderer, M for setup owning a second config file. No signed
change. Owner decision, since only the owner's answers are accepted
([master-plan.md](../docs/master-plan.md) 72-73): "When you set up an agent,
should Locust also make Claude Code show the agent its Locust work again when
a chat starts or is compacted?"

### 5. Short IDs in MCP, resolved only against what was shown

beads: `bd comment` refuses abbreviations after the stray word "list" matched
an unrelated hash prefix and 15 or more sessions wrote comments there
([comment.go:15-30](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/cmd/bd/comment.go#L15-L30);
[hash-ids.md:121-131](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/docs/core-concepts/hash-ids.md#L121-L131)).

Locust today: `short` already emits at least 8 hex and grows until unique
([level.rs](../crates/locust-proto/src/api/level.rs) 551). The CLI resolves
prefixes and refuses ambiguity
([selectors.rs](../crates/locust/src/cli/selectors.rs) 40-48); MCP takes exact
IDs. Hex was 51% of a 31-event read, and the research plans one resolver and
asks "12 hex or 16?" ([agent-ergonomics](agent-ergonomics-2026-10-05.md) 143,
438-443, 585).

Already planned; beads adds two points. First, by the birthday bound beads
uses ([adaptive-ids.md:17-46](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/docs/core-concepts/adaptive-ids.md#L17-L46)), 8 hex keeps a collision chance under 1 in 1,000 up to about 2,900 IDs,
and `short` lengthens only on a real collision, so a fixed 12 or 16 buys
nothing. Use 8, grown as needed, and replace the fixed cuts such as
presentation.rs 261. Second, let the MCP bridge expand a prefix only to an ID
it has shown this session, as it already keeps receipts per session
([context_receipts.rs](../crates/locust/src/context_receipts.rs) 119-205). A
prefix copied from another computer can otherwise match a different event
where the intended one has not synced, and a member can grind a matching
prefix ([R5 review](v2-phase-r5-review-2026-10-06.md) 364).

Reviewers: one called this already planned, the other added the session table;
both parts are kept.

Cost S, plus S-M for the bridge. No signed change. No owner decision.

### 6. Recovery text where the store refuses to open

beads: keeps recovery runbooks per failure
([database-corruption.md](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/docs/recovery/database-corruption.md)).

Locust today: every start reads every event and refuses bytes that no longer
hash to their ID ([goal/mod.rs](../crates/locust-core/src/goal/mod.rs) 77-95;
[lib.rs](../crates/locust-store/src/lib.rs) 89-94). A failure becomes an error
code with no next step ([daemon/mod.rs](../crates/locust/src/daemon/mod.rs)
267-279), and the guide says restoring is untested
([operations.md](../docs/guide/operations.md) 88-91).

Proposal: give each open failure one recovery sentence: in use, wrong version,
or damaged (name the record; move the folder aside, do not delete it). Add a
matching "If the daemon will not start" section to the guide, and one test per
case using the existing fault points.

Reviewers: one proposed a `locust store verify` command; this note drops it,
because each start already checks the store and a stale but intact copy, G1's
real question, passes every check
([host-safety-and-ending-plan.md](../docs/host-safety-and-ending-plan.md)
1675-1686).

Cost S. No signed change. No owner decision.

### 7. Delete the dormant `drop_blobs` path

beads: its store reached 2.3 GB for a 2.9 MB export, from Dolt history and a
backup copy ([#4625](https://github.com/gastownhall/beads/issues/4625)).

Locust today: `Commit::drop_blobs` ([store.rs](../crates/locust-proto/src/store.rs)
128-133) has no production producer; only tests fill it, to fake lost
content. The plans promise a copy stays readable after leaving or ending
([host-safety-and-ending-plan.md](../docs/host-safety-and-ending-plan.md) 168,
713, 810).

Proposal: delete the field, its store and node handling, and the conformance
case; rebuild the affected tests on content that never arrived. Build no
reclaim or size report until a measured store shows growth.

Reviewers: one kept an optional size report; this note drops it, as no Locust
growth has been measured.

Cost S. No signed change. No owner decision.

### 8. Last sync refusal on goal status

beads: typed `bd sync` exit codes
([bucket-federation.md:113-140](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/docs/multi-agent/bucket-federation.md#L113-L140)),
and a push that reports success while commits stopped ([#5433](https://github.com/gastownhall/beads/issues/5433)).

Locust today: `exchange_ended` keeps completed sync times and join refusals
only ([peers.rs](../crates/locust-core/src/node/peers.rs) 269-297).
`PeerView` has endpoint, connected and last sync
([api.rs](../crates/locust-proto/src/api.rs) 1556-1564). J1 moves sync times
into memory, written at most once a minute
([joinable-farms-plan.md](../docs/joinable-farms-plan.md) 1984-1995).

Proposal, inside J1: keep the last refusal a peer sent on an exchange this
daemon dialed, in memory per goal and peer, cleared on a completed sync.
Persist nothing. Show it on the `goal status` peer line and in JSON.

Reviewers: one added plain-status lines for a computer every peer refuses; this
note leaves them out, because telling a removed computer is left out of v2
([master-plan.md](../docs/master-plan.md) 468-471).

Cost XS on top of J1. No signed change. No owner decision.

### 9. Measure a long goal's fold cost

beads: found after release that leases and audit rows drove history growth
([0055:4-11](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/schema/migrations/0055_move_leases_to_table.up.sql#L4-L11)).

Locust today: every batch refolds the whole history
([goal/mod.rs](../crates/locust-core/src/goal/mod.rs) 97-139). The harness
stops at 512 tasks and is "measurement only"
([organization_performance.rs](../crates/locust-core/tests/organization_performance.rs)
137; [performance-cost-pass](performance-cost-pass.md) 43-46).

Proposal: add 2,048 and 8,192 tasks and a one-event apply mode, and record the
numbers in research. If one apply at about 40,000 events nears J6's 250 ms
status bound ([joinable-farms-plan.md](../docs/joinable-farms-plan.md)
4827-4829), raise a local fold checkpoint as a design question.

Reviewers: one wanted a release threshold; this note keeps measurement only,
because J6 already gates start time under churn (4796-4847). Ending a goal is
no retention answer: it deletes nothing
([ending-a-goal](ending-a-goal-2026-10-05.md) 165-167).

Cost S. No signed change. No owner decision.

### 10. Ordering between tasks as a written convention first

beads: its blocking bugs come from storing readiness
([#6716](https://github.com/gastownhall/beads/issues/6716); [#5427](https://github.com/gastownhall/beads/issues/5427), where the recompute took 81.9% of
`bd close` and finding affected issues 11.5%) and from tying it to the
hierarchy ([#6817](https://github.com/gastownhall/beads/issues/6817), [#6506](https://github.com/gastownhall/beads/issues/6506)). `discovered-from` never
blocks ([types.go:1480-1483](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/types/types.go#L1480-L1483)).

Locust today: tasks have no edges; ordering comes from formation stages
([organization.rs](../crates/locust-proto/src/organization.rs) 238-262), and
advisory ordering was dropped
([agent-ergonomics](agent-ergonomics-2026-10-05.md) 553-554). No recorded run
shows an early start ([role-free-board](role-free-board-2026-10-05.md) 36).

Proposal: in the skill, a follow-up task sets `parent` where allowed, and a
dependent task's first line starts "After task:<handle>", which the ready list
already prints ([presentation.rs](../crates/locust/src/cli/presentation.rs)
466-468). Ask the owner only if a real-agent trial shows early starts.

Reviewers: one asked the owner now; this note waits for evidence.

Cost S, docs only. A later edge would change the signed format. Owner question
if needed: "Should an agent be able to mark a task 'do after task X', so other
agents see it as not ready yet but can still take it?"

### 11. A release gate for signed formats

beads: [#6053](https://github.com/gastownhall/beads/pull/6053) turned revisions from integers into strings while
`api_version` stayed the same, so old clients passed the handshake and failed
to decode ([PR #7182](https://github.com/gastownhall/beads/pull/7182)). Its regeneration tool now refuses to record a
changed entry unless the revision moved
([bd-serve-v0.md:589-625](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/engdocs/design/bd-serve-v0.md#L589-L625)).

Locust today: a test freezes the signed bytes
([vectors.rs](../crates/locust-proto/src/vectors.rs) 153-175), and
[crates.md](../docs/crates.md) lines 47-50 call the version bump "a
convention", in words that conflict with the plan: bytes may change inside 7
until a release ([master-plan.md](../docs/master-plan.md) 353-363). The API
golden is drift-checked in CI but `--write` re-records it silently
([check_formations.py](../scripts/check_formations.py) 84-112).

Proposal: now, align crates.md with the plan. At the first release, have
[build_release.py](../scripts/build_release.py) keep one tracked row per
published build (protocol, store version, a digest of the vector constants and
the store DDL) and refuse to build when a released version's digest differs.
Leave the API out: CLI, daemon and MCP ship as one binary
([public-preview-release.md](../docs/public-preview-release.md) 3-6).

Reviewers: one wanted a test ledger with a hand-set `released` flag; this note
puts the check where release state already lives, so nobody has to remember
the flag.

Cost XS now, S at release. No signed change. Timing follows the owner's open
version question (master-plan.md 450).

### 12. The last release's data folder as a fixture

beads: keeps an upgrade target per reviewed release
([BUILD.bazel:5-12](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/tests/migration/BUILD.bazel#L5-L12)). One day of
differential testing logged about 54 bugs and decisions, mostly by hand
exploration ([DISCOVERY.md:11-23](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/tests/regression/DISCOVERY.md#L11-L23)).

Locust today: refusal tests leave files unchanged
([locust-store tests.rs](../crates/locust-store/src/tests.rs) 143, 983), but
nothing opens a folder written by an earlier build
([phases 1-3 build notes](v2-phases-1-3-build-notes-2026-10-06.md) 101).

Proposal, at the first release: one fixture, replaced each release, written by
the released binary through normal commands with throwaway keys, plus the
daemon's normalized read answers. The current build must reproduce them, or
refuse with every file unchanged.

Reviewers: one kept a fixture per release with a hash fingerprint; this note keeps
one, because without migration older fixtures only retest refusal, and the
existing fingerprint hashes a Debug dump that changes on refactors
([organization_performance.rs](../crates/locust-core/tests/organization_performance.rs)
175-180).

Cost S, once. No signed change. No owner decision: the fixture is a test
input written by a released binary, not build output under
[AGENTS.md](../AGENTS.md).

## Lessons from beads' reversals

1. **Two representations kept in step.** JSONL in git as truth, SQLite as
   cache: stale JSONL overwrote fresh changes ([#1623](https://github.com/gastownhall/beads/issues/1623),
   [#911](https://github.com/gastownhall/beads/issues/911)); a later automatic export dropped 89% of rows
   ([#4069](https://github.com/gastownhall/beads/issues/4069)). Locust: any export is manual, never authoritative and
   never imported automatically.
2. **Replicated chatter.** Each claim and heartbeat was a Dolt commit, and
   audit rows touched about 80% of commits
   ([0062:4-6](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/schema/migrations/0062_events_dolt_ignore.up.sql#L4-L6)).
   Locust: liveness, presence and read receipts stay local
   ([store.rs](../crates/locust-proto/src/store.rs) 69-102); progress reports
   stay human-paced.
3. **Random values as replicated identity.** Random keys forked primary keys
   across clones ([#4259](https://github.com/gastownhall/beads/issues/4259)); child counters still collide
   ([#4796](https://github.com/gastownhall/beads/issues/4796)). Locust: every replicated identifier is content-derived,
   like `EffectId` ([event.rs](../crates/locust-proto/src/event.rs) 374-386),
   and display prefixes are never signed.
4. **Clock wins on contested values.** Across clones, `assignee` merges
   last-writer-wins and nothing tells the loser
   ([automerge.go:28-36](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/versioncontrolops/automerge.go#L28-L36));
   that loss is read from code, not from an issue. The open double claims
   ([#3575](https://github.com/gastownhall/beads/issues/3575), [#5998](https://github.com/gastownhall/beads/issues/5998)) are races on one shared database, and
   v1.2.2, where #5998 was reported, is the v1.1.2 tree and carries no
   `row_lock` code ([v1.2.2 gate:5-8](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/release-gates/v1.2.2-recovery-release-gate.md#L5-L8)). Locust: keep
   principle 3; exclusive claims stay out of v2
   ([master-plan.md](../docs/master-plan.md) 473).
5. **Stored derived state.** `is_blocked` went stale after merges and racing
   closes ([#6716](https://github.com/gastownhall/beads/issues/6716), [#6608](https://github.com/gastownhall/beads/issues/6608)). Locust: derive readiness in
   the fold; never store a flag that needs a repair command.
6. **A process removed, then rebuilt.** The daemon went in February partly so
   agents would stop assuming one ([`326976614`](https://github.com/gastownhall/beads/commit/326976614)); in July
   `bd serve` returned for startup cost and a stdout contract
   ([bd-serve-v0.md:1-7](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/engdocs/design/bd-serve-v0.md#L1-L7)). Locust:
   keep one small daemon, with CLI and MCP as thin clients.
7. **Summaries written over the original.** Only the `--auto` path snapshots
   first ([compactor.go:133-138](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/compact/compactor.go#L133-L138));
   undo was impossible until [#4464](https://github.com/gastownhall/beads/pull/4464). Locust: a summary is a new
   record that cites, never a rewrite.
8. **Rules nothing checks.** A 30-command warning
   ([UI_PHILOSOPHY.md:212](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/engdocs/UI_PHILOSOPHY.md#L212)) sits beside 109
   files in its [CLI reference](https://github.com/gastownhall/beads/tree/bb9b9e502023f8bbd115961f55f8ff724e07d78b/docs/cli-reference), and a charter that bars orchestration from core
   ([PROJECT_CHARTER.md:29-34](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/engdocs/PROJECT_CHARTER.md#L29-L34)) beside
   Gas Town identity rules in core
   ([identity.go:5-50](https://github.com/gastownhall/beads/blob/bb9b9e502023f8bbd115961f55f8ff724e07d78b/internal/storage/issueops/identity.go#L5-L50)).
   Locust: turn limits that matter into tests (idea 1).
9. **Injected commands.** A close protocol in every session overrode users'
   instructions ([#3451](https://github.com/gastownhall/beads/issues/3451)). Locust: a hook states facts, not
   workflow orders.

## This repository's own memory practice

The agents building Locust keep memory in prose: tracked `docs/` and
`research/`, `AGENTS.md`, and the owner's local agent memory, which is outside
Git. Phase state is written in three places that disagree: the status lines
say phases 1 to 6 are built ([master-plan.md](../docs/master-plan.md) 3-6), the
pieces table says only roles phases 1 to 3 (297) while the build order
marks R4 and R5 built (322-323), and R6 is unmarked (324) although
`9499cde` recorded its verification. A K1 fix record left the stale status as
"plan ownership" ([K1 fixes](v2-phase-k1-fixes-2026-10-06.md) 48). Findings
use three ID schemes: bare headings in K1 (14-15), a `#` column in
[R4 fixes](v2-phase-r4-fixes-2026-10-06.md) (21), and numbered headings in the
[R5 review](v2-phase-r5-review-2026-10-06.md) (137). The local memory records
two sessions duplicating a fix and a history rewrite mid-task
(`concurrent-sessions-stage-by-file.md`, lines 20-21).

beads' answer is a tracker; this repository needs less. Two process ideas:

- **One place for phase state.** Mark phases only in the build-order table as
  "Built (`commit`)", strip counts and states from the status paragraph and
  pieces table, and let a build session edit its own row. Number findings as
  the R5 review does. The reviewers differed on a `check_docs.py` rule; this note
  drops it, because an exactly-once rule would have passed the R4 finding
  that was moved and then half built (R5 review 640).
- **Real work for R7's early run.** Seed the planned run
  ([roles-and-permissions-plan.md](../docs/roles-and-permissions-plan.md)
  4064-4083) with three to five open findings from this repository. Each
  agent uses its own home, worktree and build directory, and results land by
  hand outside the run. Record tasks with more than one attempt and the work
  each duplicate wasted, the evidence the exclusive-claim question lacks. Do
  not credit beads' self-hosting: outside users reported its claim races
  ([#3575](https://github.com/gastownhall/beads/issues/3575), [#5998](https://github.com/gastownhall/beads/issues/5998)).

## Considered and not recommended

- **Goal bundle export and import.** Members on one network already sync
  over mDNS ([locust-net lib.rs](../crates/locust-net/src/lib.rs) 126-131), and
  a file cannot end the hold that a whole-computer restore produces
  ([host-safety-and-ending-plan.md](../docs/host-safety-and-ending-plan.md)
  1734-1745). Reviewers split; rejected because it removes no person step.
- **A Summary that names the events it covers.** J5 already spares newcomers
  the backlog ([joinable-farms-plan.md](../docs/joinable-farms-plan.md)
  4027-4045), and a Summary needs a review and a decision before it counts
  ([projection.rs](../crates/locust-core/src/goal/projection.rs) 464-478).
  Reviewers split; rejected because no agent writes one: `Doc::Summary`
  appears only in a fold test
  ([goal/tests.rs](../crates/locust-core/src/goal/tests.rs) 866-876).
- **Importing a beads export.** An agent can open each issue with
  `locust_task_open` ([mcp/schema.rs](../crates/locust/src/mcp/schema.rs)
  45-47). Reviewers split; rejected as a person command for an act the owner gives
  to agents ([master-plan.md](../docs/master-plan.md) 171-174).
- **`locust store verify`, reference-aware reclaim, a storage report, board
  paging, surface budget constants, a hashed start tiebreak, a schema-based
  field checker, plain-status refusal lines, a finding-ID checker:** reasons
  in ideas 1, 2, 6, 7, 8 and above.
- **Locust as a beads backend.** No plan proposes it, and it would drop
  signatures.

## Open questions

1. Is the private part of v2 released before the door
   ([master-plan.md](../docs/master-plan.md) 450)? Ideas 11 and 12 start at
   that release.
2. Are hooks accepted, and as part of setup or a separate command
   ([agent-ergonomics](agent-ergonomics-2026-10-05.md) 598)?
3. Does finding supersession ride the next signed-format change?
4. Does `bd ready --claim` still double-fill in server mode at this commit
   ([#5998](https://github.com/gastownhall/beads/issues/5998))? It was not run.
5. Is beads' memory write path fixed at this commit ([#5976](https://github.com/gastownhall/beads/issues/5976))? It
   was not checked.
