export const meta = {
  name: 'review-phase-5-roles',
  description: 'Read-only review of phase 5 (names and roles in the goal, commit 8c086c1) against its plan: find, adversarially verify, merge',
  phases: [
    { title: 'Find', detail: 'one reader per region or lens of the commit' },
    { title: 'Verify', detail: 'a second reader tries to refute each finding' },
    { title: 'Gaps', detail: 'a critic names what nobody read; extra readers cover it' },
    { title: 'Merge', detail: 'duplicates folded into one ranked list' },
  ],
}

const COMMON = `
You are reviewing code another session built and already verified. Repository: . (Rust workspace + SvelteKit site). Project "Locust": a per-person daemon; agents have signing keys; a goal is a signed, append-only, per-author hash-linked set of logs synced peer to peer; evaluation ("replay") is a pure function of the events a computer holds; every work event names a host-signed governance record as its position (its "anchor").

WHAT IS UNDER REVIEW
- Commit 8c086c1 "feat: add signed names and goal role holders" (129 files, +6560/-1051). This is the roles plan's "Phase 4: Names and roles in the goal" (fifth phase in the overall build order, also called R4). The formal-model change came first as commit 3a56930.
- The plan: docs/roles-and-permissions-plan.md lines 2274-3215 (Goal, Depends on, Changes from line 2301, Tests from 2821, Exit criteria from 3032, Risks and notes from 3097). Terminal mockups P4-1 and P4-2 are near lines 467-540 of the same file. The companion: docs/roles-and-permissions-plan-details.md lines 229-330.
- The builder's own notes: research/v2-phase-r4-build-notes-2026-10-06.md. It lists 15 "Departures and readings of the plan". Read them; a declared departure is not automatically fine and not automatically a finding.
- The code in the working tree equals commit 8c086c1 for every code file. See what changed with: git show 8c086c1 -- <path>   and the whole file with: git show 8c086c1:<path>   (or just read the file).

HARD RULES
- Read only. Do not edit, create or delete any file. Do not run cargo, npm, node, python test suites, or any build or test. The full test suite, clippy, the model checker and the site checks already ran and pass; do not report "this might not compile" or "tests may fail".
- Use git show, git grep, grep, sed -n, and reading files. Nothing else.

THE OWNER'S PRINCIPLES (a violation is a finding)
- A person meets no friction: the option that asks least of the person wins.
- Agents' work never waits for a human.
- No computer decides something shared from its own clock, arrival order, or by comparing identifiers; the host's computer may choose by signing one record.
- Greenfield: no migration, no compatibility reader, no dead code.
- A hostile HOST is outside the design. A hostile or buggy ordinary member is inside it: a member's computer can sign and send any record with its own key.
- No text a person reads shows the goal's governance key, and no untrusted name is printed unsanitized.

WHAT COUNTS AS A FINDING
1. Concrete wrong behaviour, with a scenario: the exact state and inputs, what the code does, what it should do. You must be able to point at the lines.
2. A sentence a person or an agent reads that is untrue in some reachable state.
3. A place where the code differs from the plan in a way that changes behaviour. Say whether the code or the plan should change.
4. A behaviour the plan names a test for, where the test is missing or would still pass with the behaviour removed.
Not findings: style, naming, "could be cleaner", speculation without a scenario, and anything already listed in research/v2-phase-k1-review-2026-10-06.md unless this commit made it worse.

Report at most 8 findings, most serious first. For each, give the file and line as they are in commit 8c086c1. In "person_sees", say in plain everyday words what a person using Locust would see or lose (or "nothing visible" if it is internal). In "covered", list in two or three sentences what you read and what you did not get to.
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
          kind: { type: 'string', enum: ['bug', 'untrue-text', 'plan-departure', 'test-gap', 'model-gap', 'doc'] },
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
  { key: 'signed-format', files: [P + 'event.rs', P + 'invite.rs', P + 'sync.rs', P + 'limits.rs', P + 'vectors.rs', P + 'testkit.rs'],
    focus: 'The signed bytes. Field order and canonical encoding of MemberAdmitted (name, role), RoleHolders (new last variant, discriminant 25), RulesBinding without holders. Bounds and character rules for a member name and a role name (is_member_name and the limits): are they enforced on decode for every path a hostile member can reach (admission, role record, ticket, join request)? Unsorted, duplicate, empty or oversized holder lists. Whether the ticket signature and the join request signature really cover the new fields. Whether the frozen vectors exercise the new fields or would still pass if a field were dropped.' },
  { key: 'chain-snapshot', files: [C + 'goal/chain.rs', C + 'goal/mod.rs', C + 'goal/state.rs', C + 'goal/projection.rs'],
    focus: 'Role holders kept in each governance snapshot. A declared role starts with the host agent; removing the last holder falls back to the host agent; a member removed from the goal leaves every role; RoleHolders validation (who may sign it, holders must be admitted at that position, what about a role no binding declared, what about two holders for a deciding role: does replay accept it, and does the plan put that check in replay or only in the host daemon?). An admission that carries a role. Is the result the same on two computers that received the same records in different orders or with a fork of the governance log? Cost: is a role map cloned per governance record, and what does that do to a goal with thousands of governance records and members (the project demands ruthless performance)?' },
  { key: 'rules-counting', files: [C + 'goal/rules.rs', C + 'goal/fold.rs', C + 'goal/delegation.rs'],
    focus: 'Rules resolution at the record\'s anchor. Role holders and the only-member case read at the anchor of the act; a task keeps its pinned rules but sees the holders at each later act. Latest effective review per member decides completion; selection evidence pinned by a decision stays valid; a role change does not erase reviews signed while eligible; reviews under rules with no review requirement are opinions. Author exclusion. A reviewer removed from the goal later. Symbolic narrowing in delegation.rs: can a subtask now get rules that widen authority, or is a legitimate narrowing now refused? Look for an act judged with holders from the wrong position (too early or too late), and for a result that counts on one computer and not on another.' },
  { key: 'flow-closure-workspace', files: [C + 'goal/flow.rs', C + 'goal/closure.rs', C + 'goal/workspace.rs', C + 'goal/workspace_tests.rs'],
    focus: 'Departures 5 and 6 in the build notes. How is "chronological" order after a lead change defined without a clock, and is it the same on every computer? Does the observed-closure ancestry change alter outcomes for goals that never change lead? Review requests created for new reviewers: only for results still needing review; none for approved or selected work. Opinions excluded from stage review prerequisites: can a stage now stall forever, or advance without the review its rules ask for? The host\'s first source-free files in an empty workspace epoch count without review: exactly once per empty epoch? Can anyone but the host agent get files accepted without review through this door? What happens to later proposals.' },
  { key: 'node-roles-rules-bind', files: [C + 'node/requests/goals.rs', C + 'node/requests/mod.rs', C + 'node/access.rs', C + 'node/requests/levels.rs', C + 'node/requests/workspace.rs', C + 'node/local.rs', C + 'node/commit.rs', C + 'node/authoring.rs'],
    focus: 'role.give and role.take: host only; expected-holder comparison and what a retry or a concurrent second command does; deciding role has one holder and cannot be taken from the host agent; taking a group role from its sole host holder is a no-op; missing historical definitions block role changes (does that make a person or an agent wait with no way forward?); kind enforced across every binding. rules bind: carries an enabled workspace into the new rules as two governance records in one durable transaction at adjacent positions. Is it really atomic (crash between the two, a refusal of the second after the first is signed)? Does the trial path (commit.rs Trial, access.rs allowed_keeping, authoring.rs sign_for; advance adopts a trial only when revision and event ids match) still hold for a two-record transaction, or can a stale trial state be adopted? Agent credentials cannot perform the owner\'s first capture, including by replacing a registered folder.' },
  { key: 'invitations-join', files: [C + 'node/requests/invitations.rs', C + 'node/farm.rs', C + 'node/peers.rs', P + 'invite.rs', P + 'api/invitations.rs', L + 'daemon/network.rs', L + 'daemon/worker.rs', L + 'cli/invitations.rs'],
    focus: 'A joining member\'s name: signed into the join request, persisted through retries, used in the admission. What if two members ask for the same name, or a joiner asks for the host\'s name, or a name that looks like another member\'s key prefix, or a name with control characters or look-alike letters? Is uniqueness decided anywhere, and by whom? The automatic reviewer role on admission ("where the rule needs reviewers the host agent cannot supply alone, members the host adds or invites become reviewers"): when exactly is it given, lexically first role when several qualify, never a deciding role, --no-role. The goal-add retry digest now includes the role. The ticket\'s host name shown before the founding transcript arrives: is it presented as unverified, and sanitized? Does any of this make joining wait for a person?' },
  { key: 'views-api', files: [C + 'node/views.rs', C + 'node/context_views.rs', P + 'api.rs', P + 'api/context.rs', P + 'api/level.rs', 'docs/reference/generated/runtime.contract.json'],
    focus: 'What people and agents are shown. Pending review counts use the same latest-review lookup as replay, opinions excluded: find a state where the view and replay disagree (a member told to review something that no longer needs it, or not told about something that does). Names and roles in views and in structured refusal fields. New API requests and fields: are they consistent, bounded, and present in the generated contract? The API version stayed 7 although shapes changed: does the plan allow that?' },
  { key: 'cli-texts', files: [L + 'cli/roles.rs', L + 'cli/only_you.rs', L + 'cli/presentation.rs', L + 'cli/selectors.rs', L + 'cli/workspace.rs', L + 'cli/args.rs', L + 'cli/mod.rs'],
    focus: 'Everything a person types and reads, against mockups P4-1 and P4-2 and the plan text. Sentences that are untrue in some state. The printed inverse command: does running it really restore the previous holders (deciding role, group role, final-holder fallback "Give it back:")? Shell quoting of role names and member names. Member resolution by signed name or unique key prefix: ambiguity, a name equal to another member\'s key prefix, the same name twice. Every place a signed name or role (untrusted text from another computer) is printed: sanitized? Any command that stops to ask the person when it could act. Any output that shows the goal\'s governance key. Removal of --integrator.' },
  { key: 'formations-editor', files: [P + 'organization.rs', P + 'organization/presets.rs', C + 'organization.rs', C + 'organization/roles.rs', C + 'organization/explanation.rs', C + 'organization/validation.rs', S + 'contract/decode.ts', S + 'contract/explain.ts', S + 'contract/rules.ts', S + 'contract/types.ts', S + 'model/line.ts', S + 'model/presets.ts', S + 'model/words.ts', S + 'ui/diagrams.ts', 'examples/formations/', 'docs/reference/conformance/organization.cases.json'],
    focus: 'The six formations in plan order with directed replacing coordinator; default peer-review; peer-review and pipeline let the only member\'s contribution count; review-panel needs two other reviewers and gives joiners the reviewer role. The new only-member selector: its meaning in Rust validation and explanation versus the TypeScript mirror in the site editor; find any input where the two disagree (validation result, explanation sentence, canonical encoding, parentheses with a named check). organization/roles.rs: how a role\'s kind (deciding or group) is derived, and whether a custom formation can make that derivation wrong.' },
  { key: 'tests-vs-plan', files: [C + 'goal/tests.rs', C + 'goal/workspace_tests.rs', C + 'node/tests/roles.rs', C + 'node/tests/workspace_lifecycle.rs', C + 'node/tests/workspace.rs', C + 'organization/tests.rs', 'crates/locust/tests/cli.rs'],
    focus: 'Take the plan\'s **Tests.** list (plan lines 2821-3031) and the companion\'s Phase 4 section one entry at a time. For each named test or behaviour: is there a test, does it assert the behaviour, and would it still pass if the behaviour were removed or inverted? Do the "arrivals in different orders" tests really permute delivery, including the role record arriving before and after the acts that depend on it? Report only missing or too-weak tests, each naming the plan\'s test and the smallest assertion that would close it.' },
  { key: 'plan-changes-first-half', files: ['docs/roles-and-permissions-plan.md (lines 2301-2560)'],
    focus: 'Walk the plan\'s **Changes.** text from line 2301 to about line 2560 item by item. For each stated behaviour and each named file edit, find the code that does it (git grep) and check it does what the sentence says. Report only what is missing, different, or contradicted. Also judge departures 1-8 in the build notes: for each, is behaviour different from what the plan promises in a way a person or another computer would notice? Say whether the code or the plan should change.' },
  { key: 'plan-changes-second-half', files: ['docs/roles-and-permissions-plan.md (lines 2560-2820 and 3032-3215)'],
    focus: 'Walk the plan\'s **Changes.** text from about line 2560 to 2820, then **Exit criteria.** (3032-3096) and **Risks and notes.** (3097-3215), item by item. For each stated behaviour, named file edit and exit criterion, find the code and check it. Report only what is missing, different, or contradicted. Also judge departures 9-15 in the build notes the same way, especially 13 (the rules-change plan does not pin the head revision) and 14.' },
  { key: 'model-fidelity', files: ['research/tla/Organization.tla', 'research/tla/organization.md', 'research/tla/configs/organization-role-*.cfg', 'research/tla/configs/organization-lead-change*.cfg', 'research/tla/cases.json', C + 'goal/chain.rs', C + 'goal/fold.rs', C + 'goal/closure.rs'],
    focus: 'The formal model added in commit 3a56930 (git show 3a56930 -- research/tla/Organization.tla research/tla/organization.md) against the Rust. Does the model\'s rule for role holders equal the code\'s: start with the host agent, read at the act\'s anchor, fall back to the host agent when the last holder is removed, one holder for a deciding role? Is the code\'s ordering of decisions after a lead change (build-notes departure 5) in the model at all? Is the only-member rule in the model? Would the role-anchor mutation really be caught for the reasons claimed? Name each behaviour the code has that the model does not check, and each claim in organization.md that no case checks.' },
  { key: 'guides-scripts', files: ['docs/formations.md', 'docs/formation-editor.md', 'docs/guide/apply.md', 'docs/guide/collaboration.md', 'docs/guide/concepts.md', 'docs/guide/formation-authoring.md', 'docs/guide/formations.md', 'docs/guide/sharing.md', 'docs/site.json', 'scripts/check_t1.py', 'scripts/live_farm_demo.py', 'scripts/client_qualification/production.py'],
    focus: 'Guide and reference sentences that are now false against the code in this commit and would mislead a person today (a later phase rewrites the guides wholesale, so report only plain falsehoods and commands that no longer exist or behave differently, such as --integrator, coordinator, the old default formation, author declaration). The change to the T1 harness (it now waits for the exact offer): does it hide a real race in the product where an agent sees a task before its offer and is refused?' },
  { key: 'lens-hostile-member', files: ['whole commit'],
    focus: 'Cross-cutting lens: an ordinary member (not the host) with its own valid key signs and sends whatever it likes, or a stranger sends a crafted join request. Try each of these against the replay and node code and report what is wrongly accepted or what breaks: a RoleHolders record or a role-bearing admission not signed by the goal\'s governance key; a review, check attestation, selection or close from a member who held the role at a different position than the anchor it names; a member choosing a stale anchor on purpose to act under holders it has since lost (what stops backdating to an old governance position?); a join request with a huge or hostile name or a role it should not be able to ask for; a flood of records that makes the per-snapshot role data blow up; a name crafted to make a person give a role to the wrong member.' },
  { key: 'lens-convergence', files: ['whole commit'],
    focus: 'Cross-cutting lens: two computers that hold the same set of records must compute the same state, whatever order the records arrived in and whatever each computer\'s clock says. In the code this commit added or changed, look for: a decision that depends on arrival order, on a wall clock, on HashMap/HashSet iteration order, or on comparing identifiers or keys as a tie-break for something shared (the owner forbids the last unless the host signs the choice); "latest review" and "chronological" orderings and exactly what they are computed from; state computed incrementally in fold that differs from a full replay; anything cached in the node (views, trial state, pending lists) that can go stale when a role record arrives after the acts it affects.' },
  { key: 'lens-earlier-phases', files: [C + 'node/access.rs', C + 'node/authoring.rs', C + 'node/commit.rs', C + 'node/flow.rs', C + 'node/requests/levels.rs', C + 'goal/flow.rs', P + 'event.rs'],
    focus: 'Interplay with the earlier phases. Levels (read/ask/auto) and the one check: a new signing path added by this commit that skips sign_for, or that signs with the governance key for something Body::host_may_sign should not allow. RoleHolders and role-bearing admissions as records the host\'s computer signs: are any signed unattended (without a command from the person), and if such a step cannot be signed (host agent disconnected, level below auto) does it stop the daemon, fail a join, or quietly wait forever? Automatic reviewer roles on admission combined with a member at level ask or read: does the swarm end up waiting for a human? Anything this commit did that makes agents\' work wait for a person where the plan says it must not.' },
]

function finderPrompt(r) {
  return COMMON + `
YOUR REGION: ${r.key}
FILES TO START FROM: ${r.files.join(', ')}
WHAT TO LOOK FOR: ${r.focus}

Read the relevant plan lines first, then the diff of your files, then the full functions around every change. Follow calls into other files when you need to. Prefer three findings you are sure of to eight guesses.`
}

function verifyPrompt(f, r) {
  return `
A reviewer of commit 8c086c1 in . (the roles plan's "Phase 4: Names and roles in the goal"; plan at docs/roles-and-permissions-plan.md lines 2274-3215, mockups P4-1/P4-2 near lines 467-540, companion docs/roles-and-permissions-plan-details.md lines 229-330, builder's notes with 15 declared departures at research/v2-phase-r4-build-notes-2026-10-06.md) reported the finding below. You are the second reader. Your job is to try to REFUTE it by reading the code yourself.

HARD RULES: read only. Do not edit or create files. Do not run cargo, npm, tests or builds (they already ran and pass). Use git show 8c086c1:<path>, git show 8c086c1 -- <path>, git grep, grep, sed -n, and reading files.

CONTEXT YOU NEED: a goal is a signed append-only set of per-author logs; replay is a pure function of held records; each work record names a host-signed governance record as its anchor. A hostile host is outside the design; a hostile ordinary member is inside it. The owner's principles: a person meets no friction; agents' work never waits for a human; no computer decides something shared from its own clock, arrival order or identifier comparison unless the host signs the choice; greenfield with no migration.

THE FINDING (from the reader of region "${r.key}")
Title: ${f.title}
Where: ${f.file}:${f.line}
Kind: ${f.kind}   Claimed severity: ${f.severity}
Scenario: ${f.scenario}
Evidence: ${f.evidence}
Proposed fix: ${f.fix}
What a person would see: ${f.person_sees}

DO THIS
1. Open the cited lines and the functions around them. Check every step of the scenario against the code: is the state reachable, does an earlier check stop it, does a test already pin the opposite, does the plan or a declared departure say this is intended?
2. Decide: "confirmed" (the scenario happens as described), "partly" (something real, but the scenario, severity or fix as written is wrong; restate it correctly), or "refuted" (it does not happen, or it is intended and harmless). If you cannot build the scenario from the code, answer "refuted" and say which step fails.
3. In "statement" write the finding as it should stand after your reading, one or two sentences. In "scenario" the corrected concrete steps. In "fix" the smallest change that closes it (name the function), or for a plan departure whether the plan or the code should change. In "person_sees" plain everyday words for what a person using Locust would see or lose. In "reason" what you checked and what decided it, with file:line.`
}

async function findAndVerify(regions, phaseFind) {
  const out = await pipeline(
    regions,
    r => agent(finderPrompt(r), { label: `find:${r.key}`, phase: phaseFind, schema: FINDINGS_SCHEMA }),
    (res, r) => {
      if (!res) return { region: r.key, covered: 'reader returned nothing', findings: [] }
      return parallel(res.findings.map((f, i) => () =>
        agent(verifyPrompt(f, r), { label: `verify:${r.key}:${i + 1}`, phase: 'Verify', schema: VERDICT_SCHEMA })
          .then(v => ({ ...f, region: r.key, second: v }))
      )).then(vs => ({ region: r.key, covered: res.covered, findings: vs.filter(Boolean) }))
    }
  )
  return out.filter(Boolean)
}

phase('Find')
const K1_SCHEMA = {
  type: 'object',
  properties: {
    items: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          heading: { type: 'string' },
          status: { type: 'string', enum: ['unchanged', 'fixed-by-phase-5', 'moved', 'worse', 'cannot-tell'] },
          where_now: { type: 'string' },
          note: { type: 'string' },
        },
        required: ['heading', 'status', 'where_now', 'note'],
      },
    },
  },
  required: ['items'],
}

const k1Promise = agent(`
In ., research/v2-phase-k1-review-2026-10-06.md lists the findings of a review of the previous phase (K1, commit cb1acaa); see its section "The findings in full". None were fixed at the time. Since then commit 8c086c1 (phase 5, names and roles) changed many of the same files. A session is about to be asked to fix the K1 findings.

HARD RULES: read only. Do not edit or create files. Do not run cargo, npm, tests or builds. Use git show, git grep, grep, sed -n and reading files.

For EVERY finding in "The findings in full" (go heading by heading, do not skip any), open the code as it is in commit 8c086c1 and decide: "unchanged" (still wrong as described; give the file and line where it is now), "moved" (still wrong, but the code moved; give the new file and line), "fixed-by-phase-5" (the described scenario no longer happens; say what changed), "worse" (phase 5 widened it; say how), or "cannot-tell". Use the review's own heading text as "heading". In "note" give one or two sentences of evidence with file:line.`, { label: 'k1-findings-status', phase: 'Find', schema: K1_SCHEMA })

const round1 = await findAndVerify(REGIONS, 'Find')

phase('Gaps')
const CRITIC_SCHEMA = {
  type: 'object',
  properties: {
    tasks: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          key: { type: 'string' },
          files: { type: 'array', items: { type: 'string' } },
          focus: { type: 'string' },
        },
        required: ['key', 'files', 'focus'],
      },
    },
    reason: { type: 'string' },
  },
  required: ['tasks', 'reason'],
}
const summary = round1.map(r => `REGION ${r.region}\n  read: ${r.covered}\n  findings: ${r.findings.map(f => `[${f.second.verdict}] ${f.title} (${f.file}:${f.line})`).join(' | ') || 'none'}`).join('\n')
const critic = await agent(`
A review of commit 8c086c1 in . (the roles plan's "Phase 4: Names and roles in the goal"; plan at docs/roles-and-permissions-plan.md lines 2274-3215; builder's notes at research/v2-phase-r4-build-notes-2026-10-06.md) has finished a first round. Eighteen readers each took a region or a lens. Below is what each said it read and what it found.

HARD RULES: read only. Do not edit or create files. Do not run cargo, npm, tests or builds.

${summary}

YOUR JOB: find what nobody read. Run  git show --stat --format= 8c086c1  to list the 129 changed files. Compare with the regions above and with the plan's Changes, Tests and Exit criteria. Look for: changed non-test code files no reader named or plausibly covered; plan behaviours no reader checked; a verified finding that hints at a sibling problem nobody chased (the same mistake in a second place). Open the files yourself enough to be sure a gap is real and worth a reader.

Return at most 5 follow-up tasks, each with a short key, the files to start from, and a precise focus written as instructions to a reader (what to look for and why you think something may be there). Return an empty list if coverage is adequate. Do not repeat a region already covered.`, { label: 'critic', phase: 'Gaps', schema: CRITIC_SCHEMA })

let round2 = []
if (critic && critic.tasks.length) {
  log(`critic named ${critic.tasks.length} gaps: ${critic.tasks.map(t => t.key).join(', ')}`)
  round2 = await findAndVerify(critic.tasks.slice(0, 5).map(t => ({ key: 'gap-' + t.key, files: t.files, focus: t.focus })), 'Gaps')
} else {
  log('critic named no gaps')
}

phase('Merge')
const all = [...round1, ...round2].flatMap(r => r.findings)
const kept = all.filter(f => f.second.verdict !== 'refuted')
const refuted = all.filter(f => f.second.verdict === 'refuted')
log(`${all.length} reported, ${kept.length} survived a second reader, ${refuted.length} refuted`)

const MERGE_SCHEMA = {
  type: 'object',
  properties: {
    distinct: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          title: { type: 'string' },
          file: { type: 'string' },
          line: { type: 'integer' },
          kind: { type: 'string' },
          severity: { type: 'string', enum: ['high', 'medium', 'low'] },
          verdict: { type: 'string', enum: ['confirmed', 'partly'] },
          statement: { type: 'string' },
          scenario: { type: 'string' },
          fix: { type: 'string' },
          person_sees: { type: 'string' },
          owner: { type: 'string', enum: ['code', 'plan', 'model', 'tests', 'docs'] },
          sources: { type: 'array', items: { type: 'integer' } },
        },
        required: ['title', 'file', 'line', 'kind', 'severity', 'verdict', 'statement', 'scenario', 'fix', 'person_sees', 'owner', 'sources'],
      },
    },
  },
  required: ['distinct'],
}
const numbered = kept.map((f, i) => ({
  n: i + 1, region: f.region, title: f.title, file: f.file, line: f.line, kind: f.kind,
  verdict: f.second.verdict, severity: f.second.severity, statement: f.second.statement,
  scenario: f.second.scenario, fix: f.second.fix, person_sees: f.second.person_sees,
}))
const merged = await agent(`
Below are ${numbered.length} findings from a review of one commit, each already checked by a second reader (the text is the second reader's corrected version). Several readers looked at overlapping code, so some findings are the same problem reported twice.

Fold duplicates together: two findings are the same when one code change would close both. Keep the clearest statement, the most concrete scenario and the smallest fix; when verdicts differ keep "partly"; when severities differ keep the one the better-argued finding gives. Do not drop any finding that is not a duplicate. Do not invent anything and do not soften or strengthen what the second readers wrote. Do not read the repository; work only from this list.

For each distinct finding set "owner" to who must act: "code" (a code fix), "tests" (only a test is missing or weak), "plan" (the plan text should change to match the code), "model" (the formal model), "docs" (guide or reference text). List in "sources" the numbers n you folded into it. Order the result most serious first: wrong shared outcomes and anything that makes work wait for a person first, then untrue texts, then plan and test gaps.

${JSON.stringify(numbered)}`, { label: 'merge', phase: 'Merge', schema: MERGE_SCHEMA })

const k1 = await k1Promise
return {
  counts: { reported: all.length, kept: kept.length, refuted: refuted.length, distinct: merged ? merged.distinct.length : null },
  distinct: merged ? merged.distinct : null,
  kept: numbered,
  refuted: refuted.map(f => ({ region: f.region, title: f.title, file: f.file, line: f.line, why: f.second.reason })),
  coverage: [...round1, ...round2].map(r => ({ region: r.region, covered: r.covered })),
  critic: critic ? { reason: critic.reason, tasks: critic.tasks.map(t => t.key) } : null,
  k1_status: k1 ? k1.items : null,
}
