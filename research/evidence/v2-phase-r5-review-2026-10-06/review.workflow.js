export const meta = {
  name: 'review-phase-6-and-r4-fixes',
  description: 'Read-only review of phase 6 (the one view and refusals, 9c337df) and of the phase 5 review fixes (726de21..cffd2cf): find, then a second reader per finding',
  phases: [
    { title: 'Find', detail: 'one reader per region or lens' },
    { title: 'Verify', detail: 'a second reader tries to refute each finding' },
    { title: 'Merge', detail: 'duplicates grouped' },
  ],
}

const COMMON = `
You are reviewing code that other sessions built and already verified. Repository: . (Rust workspace + SvelteKit site). Project "Locust": a per-person daemon; agents have signing keys; a goal is a signed, append-only, per-author hash-linked set of logs synced peer to peer; replay is a pure function of the records a computer holds; every work record names a host-signed record as its position (its "anchor"). A person (the owner) types commands with --owner; agents call the same daemon through an MCP tool with their own credential.

TWO THINGS LANDED AND ARE UNDER REVIEW (the working tree equals HEAD 9c337df for all code)
A. Phase 6 of the build order: the roles plan's "Phase 5: The one view and refusals" (also called R5), commit 9c337df. Plan: docs/roles-and-permissions-plan.md lines 3255-3563 (Goal, Depends on, Changes from 3266, Tests from 3449, Exit criteria from 3494, Risks and notes from 3526). Terminal mockups P5-1 (line 519), P5-2 (line 547) and P5-3 (line 575) in the same file. Companion: docs/roles-and-permissions-plan-details.md lines 331-384. Builder's notes, with its declared departures: research/v2-phase-r5-build-notes-2026-10-06.md. See the change with: git show 9c337df -- <path>
B. The fixes for the review of the previous phase: commits 726de21, 974ea33, e801764, 2de65f0, a9992d8, ec1e078, cffd2cf (see them with: git diff 0ed4dc5 cffd2cf -- <path>, or git show <commit> -- <path>). The review they answer: research/v2-phase-r4-review-2026-10-06.md (table with a "Who acts" column, then "The findings in full"). The fix session's record: research/v2-phase-r4-fixes-2026-10-06.md (a table of 40 dispositions). The plan they build to: docs/roles-and-permissions-plan.md "Phase 4: Names and roles in the goal", lines 2274-3254.
The two sessions worked in the same checkout at the same time; B's commits landed first and A's commit landed last.

HARD RULES
- Read only. Do not edit, create or delete any file. Do not run cargo, npm, node, python test suites, or any build or test. Format, clippy, the full test suite, the model checker and the site checks already ran and pass; do not report "this might not compile" or "tests may fail".
- Use git show, git diff, git grep, grep, sed -n and reading files. Nothing else.

THE OWNER'S PRINCIPLES (a violation is a finding)
- A person meets no friction: the option that asks least of the person wins.
- Agents' work never waits for a human.
- No computer decides something shared from its own clock, arrival order, or by comparing identifiers; the host's computer may choose by signing one record.
- Greenfield: no migration, no compatibility reader, no dead code.
- A hostile HOST is outside the design. A hostile or buggy ordinary member is inside it: it can sign and send any record with its own key, and it chooses its own name, its titles and its text.
- No text a person reads shows the goal's governance key. No text from another computer (a member's name, a role name, a title, a host name) is printed unsanitized, and none of it may change what a printed command does when the person runs it as printed.

WHAT COUNTS AS A FINDING
1. Concrete wrong behaviour with a scenario: the exact state and inputs, what the code does, what it should do, with the lines.
2. A sentence a person or an agent reads that is untrue in some reachable state, or a printed command that does not do what the line beside it says.
3. A place where the code differs from the plan in a way that changes behaviour (say whether code or plan should change).
4. A behaviour the plan names a test for, where the test is missing or would still pass with the behaviour removed.
5. For the fixes (B): a finding the record says is fixed that is not, is fixed only in part, or whose fix broke something else.
Not findings: style, naming, speculation without a scenario, and what the builder's notes or the fix record already declare as left out (unless it is harmful and you can show how).

Report at most 6 findings, most serious first. Give file and line as they are at HEAD. In "person_sees" say in plain everyday words what a person using Locust would see or lose ("nothing visible" if internal). In "covered" say in two or three sentences what you read and what you did not get to.
`

const FINDINGS_SCHEMA = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          title: { type: 'string' },
          file: { type: 'string' },
          line: { type: 'integer' },
          kind: { type: 'string', enum: ['bug', 'untrue-text', 'plan-departure', 'test-gap', 'fix-incomplete', 'cost', 'doc'] },
          severity: { type: 'string', enum: ['high', 'medium', 'low'] },
          scenario: { type: 'string' },
          evidence: { type: 'string' },
          fix: { type: 'string' },
          person_sees: { type: 'string' },
        },
        required: ['title', 'file', 'line', 'kind', 'severity', 'scenario', 'evidence', 'fix', 'person_sees'],
      },
    },
    covered: { type: 'string' },
  },
  required: ['findings', 'covered'],
}

const VERDICT_SCHEMA = {
  type: 'object',
  properties: {
    verdict: { type: 'string', enum: ['confirmed', 'partly', 'refuted'] },
    statement: { type: 'string' },
    scenario: { type: 'string' },
    fix: { type: 'string' },
    severity: { type: 'string', enum: ['high', 'medium', 'low'] },
    person_sees: { type: 'string' },
    reason: { type: 'string' },
  },
  required: ['verdict', 'statement', 'scenario', 'fix', 'severity', 'person_sees', 'reason'],
}

const P = 'crates/locust-proto/src/'
const C = 'crates/locust-core/src/'
const L = 'crates/locust/src/'
const S = 'sites/locust.farm/src/lib/formation-editor/'

const REGIONS = [
  { key: 'a-refusal-wording', files: [P + 'api/level.rs', L + 'failure.rs', C + 'node/access.rs', C + 'node/callers.rs'],
    focus: 'Part A. The one wording for every refusal: render(&Refused, Voice) in both voices, for every side (level, rules, state, only-you) and every variant of Why, Rule, Act and Attempted. Compare with mockup P5-2 line by line. Find a variant whose sentence is untrue, names the wrong party, or tells the reader to do something that cannot work. The agent voice must quote nothing another member wrote; the person voice must pass everything through safe(). Check safe() itself (what it escapes, how it cuts, can a cut land inside an escape), short() (shortest unique prefix of at least eight characters: what if two identifiers share a long prefix, what if the list of others is incomplete), shell_word, allow_command and level_command.' },
  { key: 'a-status-daemon', files: [C + 'node/views.rs', P + 'api.rs', C + 'node/requests/daemon.rs', C + 'goal/mod.rs', C + 'node/tests/daemon.rs'],
    focus: 'Part A, the daemon side of the one view. DaemonStatus.waiting and Node::waiting_for: one entry per task an agent wanted while its level is below auto, oldest first. Find states where an entry is shown that the printed command cannot settle (task since closed, taken by someone else, agent removed from the goal, level since raised), or where something that does wait on the person is missing. GoalSummary additions (name in the goal, host name, open invitations and latest expiry): owner only, hosted goals only; is "open" computed the same way everywhere. wants_review lists a result only while its round is open; latest_reviews called once for all subjects. What does an agent credential see of another agent of the same owner, and of other people\'s members.' },
  { key: 'a-presentation', files: [L + 'cli/presentation.rs'],
    focus: 'Part A. presentation.rs was rewritten around a Reader. Compare status with mockup P5-1 byte for byte and goal status and pending with the plan text. Walk every branch: goal hosted here or elsewhere, joining, halted, catching up, agent disconnected, agent in no goal, zero goals, an agent credential reading the view about its owner. Find a line that is untrue in some branch, a printed command that is wrong for that branch, a place the governance key or a full 64-hex key is printed, a name or title printed without member_label or safe, and anything the old presentation showed that a person needs and the rewrite dropped by accident.' },
  { key: 'a-cli-plumbing', files: [L + 'cli/mod.rs', L + 'cli/watch.rs', L + 'cli/only_you.rs', L + 'cli/roles.rs', L + 'failure.rs'],
    focus: 'Part A. The CLI now pre-reads status, and a goal\'s members and board, before printing some views, only outside --json. Find: a command that now fails or prints nothing because a pre-read failed (daemon busy, agent not in the goal, goal still joining); how many extra daemon requests an ordinary command now makes and whether any is made on a hot path such as wait or watch on every wake-up; --json output that changed shape or content by accident; Failure::for_person applied to the wrong caller (an agent credential getting the person voice, or the person getting the agent voice); print_failure appending Role and Roles here lines with text from another computer. The Undo lines of level and allow: does running the printed Undo restore exactly the earlier state.' },
  { key: 'a-agent-surface', files: [L + 'mcp.rs', L + 'mcp/schema.rs', L + 'mcp/tests.rs', 'skills/locust/SKILL.md', 'docs/reference/generated/runtime.contract.json', P + 'api.rs'],
    focus: 'Part A, what an agent\'s tool receives (mockup P5-3). The refusal as data: code, message, details.why.side and the rest; is every field the plan names present for every side; does the exported refusal_schema match what the daemon really sends (field names, tagging; note the builder\'s departure about WaitingKind being externally tagged). INSTRUCTIONS and SKILL.md: a sentence that tells an agent to do something that no longer works, names a command or code that does not exist, or would lead an agent to stop and wait for its owner when it could go on. Summaries of status, goal.status, attempt.start, attempt.takeover, review.record, pending and wait. Anything that could lead an agent to run a command with --owner by itself.' },
  { key: 'a-tests-vs-plan', files: [P + 'api/level.rs', C + 'node/tests/daemon.rs', C + 'node/tests/lifecycle.rs', C + 'node/tests/levels.rs', L + 'mcp/tests.rs', 'crates/locust/tests/cli.rs', 'crates/locust/tests/t2_flow.rs'],
    focus: 'Part A. Take the plan\'s **Tests.** list (plan lines 3449-3493), the companion\'s Phase 5 section and the **Exit criteria.** (3494-3525) one entry at a time. For each named test or criterion: is there a test, does it assert the behaviour, and would it still pass if the behaviour were removed or inverted? Does every_printed_command_parses_as_printed really cover every command the views print, including the waiting line, the Undo lines and the role lines, with names that need quoting? Report only missing or too-weak tests, each with the smallest assertion that would close it.' },
  { key: 'a-plan-conformance', files: ['docs/roles-and-permissions-plan.md (lines 3266-3448 and 3526-3563)', 'research/v2-phase-r5-build-notes-2026-10-06.md'],
    focus: 'Part A. Walk the plan\'s **Changes.** text (3266-3448) and **Risks and notes.** item by item; for each stated behaviour and named file edit find the code (git grep) and check it. Then judge every departure the builder declared: is behaviour different from the plan in a way a person or an agent would notice, and should the code or the plan change. Then check the ten findings of research/v2-phase-r4-review-2026-10-06.md marked "phase 6 (R5)": is each really fixed at HEAD, including the second half of the name finding (two members whose names read the same)? Report only what is missing, different or contradicted.' },
  { key: 'b-replay-fixes', files: [C + 'goal/chain.rs', C + 'goal/fold.rs', C + 'goal/flow.rs', C + 'goal/delegation.rs', C + 'goal/tests.rs', C + 'goal/workspace_tests.rs'],
    focus: 'Part B, commit 726de21 (git show 726de21). Fix record rows 1, 5, 7, 9, 13, 15, 18, 28, 36-39. For each: is the described problem closed, and did the fix change anything else? Look hard at: (row 7) a review, check or declaration must not be anchored earlier than its subject: can an honest daemon ever sign such a record (its head behind the subject\'s anchor, a subject from a fork, a document revision, shared-file proposals, selection evidence already pinned by an old decision that now becomes ineffective and so un-picks something); (row 1) offers only for an open round; (row 9) review requests for a departed author; (row 15) wanted effects read the projection\'s map: is the map always built before, and the same set as the scan it replaced, in every replay order.' },
  { key: 'b-caches', files: [C + 'goal/fold.rs', C + 'goal/rules.rs', C + 'goal/projection.rs', C + 'goal/mod.rs'],
    focus: 'Part B, commit e801764 (git show e801764). Fix record rows 12, 14, 16. The rule-resolution cache is keyed by context again and fills roles and only-member per call from the anchor\'s snapshot; latest_review is cached per subject and check per fold. Prove or break soundness: is there any path where a cached value computed under one condition is returned under another (the pinned-evidence path with a proof, a status that changes during the same fold because of memoised recursion order, an error result cached for a context that would succeed at another anchor, a snapshot missing for the anchor)? Does behaviour differ between forward, reversed and reloaded replay? Is test-only counting code compiled into the production build? Do not claim a speed-up or slowdown you cannot show by reading.' },
  { key: 'b-names-selectors-join', files: [L + 'cli/selectors.rs', L + 'cli/only_you.rs', C + 'node/requests/invitations.rs', C + 'node/peers.rs', P + 'invite.rs', C + 'node/tests/roles.rs', C + 'node/replica_tests.rs'],
    focus: 'Part B, commit 2de65f0 (git show 2de65f0). Fix record rows 2, 10, 17, 25, 26, 30, 34, 35. Member selection: exact key wins, otherwise key-prefix, signed-name and enrolled-name candidates combined and deduplicated by key; find an input where the wrong member is chosen silently, where a member can still capture a selector aimed at someone else, or where a person can no longer name a member at all (two members with one name: is there always a way to pick each). A waiting join may change its requested name before admission: what if the host already signed the admission with the old name, what does the retry do and say. Repeated add or join is a no-op that prints the existing name and level: is it always true.' },
  { key: 'b-workspace-validation', files: [L + 'cli/workspace.rs', C + 'organization/validation.rs', C + 'organization/explanation.rs', S + 'contract/rules.ts', S + 'contract/explain.ts', S + 'ui/PointBox.svelte', 'crates/locust/tests/workspace.rs', 'docs/formations.md'],
    focus: 'Part B, commit a9992d8 (git show a9992d8). Fix record rows 3, 4, 21, 22, 32. workspace init now preserves a formation\'s own workspace part and rebinds only when it is missing or a completion override is given; "the reviewed rules revision is carried through to the write". A role named as the one who accepts file changes is refused in Rust and in the site\'s validator. Find: a goal already bound to rules that name a role there (made before this change in the same greenfield store or received from another computer): does replay now exclude rules that were effective, and is that the same on every computer; an input where the Rust validator and the TypeScript mirror disagree; a workspace init path that leaves the files with no usable rule or with the wrong accepting member; sentences in explanation.rs and its mirror that are untrue.' },
  { key: 'b-rules-bind-roles', files: [C + 'node/requests/goals.rs', L + 'cli/only_you.rs', L + 'cli/roles.rs', C + 'organization/roles.rs', C + 'node/tests/roles.rs', 'crates/locust/tests/cli.rs', 'crates/locust/tests/t2_flow.rs'],
    focus: 'Part B, commit ec1e078 (git show ec1e078). Fix record rows 6, 8, 11, 19, 20, 31, 33. rules bind now gives the counting role to the members already in the goal in the same durable commit as the rules and the optional shared-files epoch (plan lines around 2700-2712), with --no-role. Find: is it really one atomic commit of up to three host records at adjacent positions, and what happens if signing the second or third fails; who is in the list (the host agent, an agent that asked to leave, a disconnected agent, a member mid-removal); is a deciding role ever assigned; what if the role already has a list the host edited by hand (does bind silently overwrite the host\'s earlier role take); is the printed Undo correct for each member; does the plan a person confirms describe exactly what is then signed if a member joins between plan and yes; the limit_exceeded refusal for a list that no longer fits; a formation that lists a role no rule uses.' },
  { key: 'b-scripts-model-docs', files: ['scripts/live_farm_demo.py', 'scripts/check_operations.py', 'scripts/tests/test_live_farm_demo.py', 'research/tla/organization.md', 'research/tla/Organization.tla', 'research/evidence/tla/organization/README.md', 'docs/formations.md'],
    focus: 'Part B, commits 974ea33 and cffd2cf, fix record rows 23, 24, 27, 28, 29. Are the two scripts consistent with the commands as they are at HEAD (after part A changed status and refusal texts: do the scripts or their helpers parse any text or field that part A changed)? Does the property map now say only true things about what the model checks and what only Rust tests check? The fix record says an operations qualification run failed at startup ("workers establish their independent peer link"): read the script around that step and say whether the script, the product or the environment is the likelier cause, by reading only.' },
  { key: 'x-two-sessions', files: [L + 'cli/only_you.rs', L + 'cli/roles.rs', C + 'goal/mod.rs', C + 'node/views.rs', 'crates/locust/tests/cli.rs', 'crates/locust/tests/t2_flow.rs', 'crates/locust/tests/workspace.rs'],
    focus: 'Cross-cutting. Two sessions edited the same checkout at once and part A committed last with a large rewrite. Look for damage from that: for each file both changed (git diff --stat 0ed4dc5 cffd2cf and git show --stat 9c337df; the overlap includes cli/only_you.rs, cli/roles.rs, goal/mod.rs, tests/cli.rs, tests/t2_flow.rs, tests/workspace.rs, node/requests/mod.rs, node/tests/levels.rs, node/tests/lifecycle.rs, runtime.contract.json), compare git show cffd2cf:<path> with HEAD and check that every part-B change is still there and still wired up: a text part B fixed that part A\'s rewrite put back, a function one session added and the other stopped calling, two helpers that now do the same job, a test that asserts the other session\'s old wording, a member printed through one label in one command and another label elsewhere. Also check the fix record\'s claims against HEAD for rows 17, 19, 20 and 31, whose output goes through code part A rewrote.' },
  { key: 'x-hostile-text', files: [P + 'api/level.rs', L + 'cli/presentation.rs', L + 'cli/only_you.rs', L + 'cli/roles.rs', L + 'cli/selectors.rs', L + 'cli/mod.rs'],
    focus: 'Cross-cutting lens: a member on another computer chooses its own name, and any member writes titles, task text and (through a formation the host accepts) role names. Every command line the views print is meant to be run as printed by the person. Trace every place such text reaches a printed line or a printed command: can a name, title or role name make a printed command do something else when pasted into a shell (quotes, $(), backticks, newlines, a leading dash read as a flag, a name equal to a flag or to another member\'s key prefix), make a status line read as a different fact (a name that looks like "host", "you", a level, a role list, or the label of another member), hide itself (invisible or look-alike characters that safe() does not escape, right-to-left marks), or overflow a cut so that the cut hides the part that matters? Check member_label, safe, shell_word, quote_role and each caller.' },
]

function finderPrompt(r) {
  return COMMON + `
YOUR REGION: ${r.key}
FILES TO START FROM: ${r.files.join(', ')}
WHAT TO LOOK FOR: ${r.focus}

Read the relevant plan or review text first, then the diff of your files, then the full functions around every change. Follow calls into other files when needed. Prefer three findings you are sure of to six guesses.`
}

function verifyPrompt(f, r) {
  return `
A reviewer in . reported the finding below against HEAD 9c337df. Two things landed and are under review: (A) the roles plan's "Phase 5: The one view and refusals", commit 9c337df (plan: docs/roles-and-permissions-plan.md lines 3255-3563, mockups P5-1/P5-2/P5-3 at lines 519, 547, 575; companion docs/roles-and-permissions-plan-details.md lines 331-384; builder's notes research/v2-phase-r5-build-notes-2026-10-06.md); (B) fixes for the previous phase's review, commits 726de21..cffd2cf (review: research/v2-phase-r4-review-2026-10-06.md; fix record: research/v2-phase-r4-fixes-2026-10-06.md; plan: same file, "Phase 4", lines 2274-3254). You are the second reader. Try to REFUTE the finding by reading the code yourself.

HARD RULES: read only. Do not edit or create files. Do not run cargo, npm, tests or builds (they already ran and pass). Use git show, git diff, git grep, grep, sed -n and reading files.

CONTEXT: a goal is a signed append-only set of per-author logs; replay is a pure function of held records; each work record names a host-signed record as its anchor. A hostile host is outside the design; a hostile ordinary member is inside it and chooses its own name and text. The owner's principles: a person meets no friction; agents' work never waits for a human; no computer decides something shared from its own clock, arrival order or identifier comparison unless the host signs the choice; greenfield with no migration; no text from another computer is printed unsanitized or can change what a printed command does.

THE FINDING (from the reader of "${r.key}")
Title: ${f.title}
Where: ${f.file}:${f.line}
Kind: ${f.kind}   Claimed severity: ${f.severity}
Scenario: ${f.scenario}
Evidence: ${f.evidence}
Proposed fix: ${f.fix}
What a person would see: ${f.person_sees}

DO THIS
1. Open the cited lines and the functions around them. Check every step of the scenario against the code: is the state reachable, does an earlier check stop it, does a test already pin the opposite, does the plan, the builder's notes or the fix record say this is intended?
2. Decide: "confirmed" (it happens as described), "partly" (something real, but the scenario, severity or fix as written is wrong; restate it correctly), or "refuted" (it does not happen, or it is intended and harmless). If you cannot build the scenario from the code, answer "refuted" and say which step fails.
3. "statement": the finding as it should stand, one or two sentences. "scenario": the corrected concrete steps. "fix": the smallest change that closes it (name the function), or for a plan departure whether plan or code should change. "person_sees": plain everyday words. "reason": what you checked and what decided it, with file:line.`
}

phase('Find')
const rounds = await pipeline(
  REGIONS,
  r => agent(finderPrompt(r), { label: `find:${r.key}`, phase: 'Find', schema: FINDINGS_SCHEMA }),
  (res, r) => {
    if (!res) return { region: r.key, covered: 'reader returned nothing', findings: [] }
    return parallel(res.findings.slice(0, 6).map((f, i) => () =>
      agent(verifyPrompt(f, r), { label: `verify:${r.key}:${i + 1}`, phase: 'Verify', schema: VERDICT_SCHEMA })
        .then(v => ({ ...f, region: r.key, n: i + 1, second: v }))
    )).then(vs => ({ region: r.key, covered: res.covered, findings: vs.filter(Boolean) }))
  }
)
const done = rounds.filter(Boolean)
const all = done.flatMap(r => r.findings)
const kept = all.filter(f => f.second && f.second.verdict !== 'refuted')
const refuted = all.filter(f => f.second && f.second.verdict === 'refuted')
log(`${all.length} reported, ${kept.length} survived, ${refuted.length} refuted`)

phase('Merge')
const GROUP_SCHEMA = {
  type: 'object',
  properties: {
    groups: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          ids: { type: 'array', items: { type: 'string' } },
          sentence: { type: 'string' },
          kind: { type: 'string', enum: ['wrong result', 'work waits on a human', 'untrue text', 'cost', 'fix incomplete', 'departs from the plan', 'leftover', 'test gap', 'scripts and guides'] },
          best: { type: 'string' },
        },
        required: ['ids', 'sentence', 'kind', 'best'],
      },
    },
  },
  required: ['groups'],
}
const compact = kept.map(f => ({ id: `${f.region}:${f.n}`, where: `${f.file}:${f.line}`, severity: f.second.severity, statement: f.second.statement.slice(0, 700), fix: f.second.fix.slice(0, 300) }))
const merged = await agent(`
Below are ${compact.length} findings from a code review, each already checked by a second reader. Several readers looked at overlapping code, so some are the same problem reported twice. Do not read the repository; work only from this list.

Group them: two findings belong together when one code change would close both. Every id must appear in exactly one group; a finding with no duplicate is a group of one. For each group give: "ids" (the ids exactly as written), "sentence" (the problem in one plain sentence a non-programmer could follow, no more than 35 words, no file names), "kind" (one of the allowed values), and "best" (the one id whose statement is clearest). Order the groups most serious first: wrong shared outcomes and anything that makes work wait for a person, then untrue texts and commands, then the rest. Do not invent, soften or strengthen anything.

${JSON.stringify(compact)}`, { label: 'merge', phase: 'Merge', schema: GROUP_SCHEMA })

return {
  counts: { reported: all.length, kept: kept.length, refuted: refuted.length, groups: merged ? merged.groups.length : null },
  groups: merged ? merged.groups : null,
  kept: kept.map(f => ({ id: `${f.region}:${f.n}`, title: f.title, file: f.file, line: f.line, kind: f.kind, verdict: f.second.verdict, severity: f.second.severity, statement: f.second.statement, fix: f.second.fix, person_sees: f.second.person_sees })),
  refuted: refuted.map(f => ({ id: `${f.region}:${f.n}`, title: f.title, why: f.second.reason.slice(0, 400) })),
  coverage: done.map(r => ({ region: r.region, covered: r.covered })),
}
