export const meta = {
  name: 'v2-complexity-ledger',
  description: 'Count what the v2 plan adds to and removes from Locust, then attack the count from two sides',
  phases: [
    { title: 'Count', detail: 'one reader per piece of the plan, plus a baseline of the code as it is today' },
    { title: 'Attack', detail: 'one reader looks for complexity the count missed, one for complexity that can be cut' },
  ],
}

const REPO = '.'
const SCRATCH = 'scratch'

const COMMON = `
You are measuring how much complexity a planned redesign ("v2") adds to an existing Rust system called Locust. The repository is at ${REPO}. You are read-only there: do not edit, create or delete any file in the repository, do not run cargo, npm or any build or test. You may run read-only shell commands (rg, wc, sed -n, python3 for reading JSON). Nothing in v2 is built yet; the plans are proposals.

Read ${REPO}/docs/master-plan.md first. It is short. It lists the pieces, the owner's 25 decisions and a sixteen-phase build order (R1..R10 roles and permissions, K1 signing key, G1 G2 restore guard, E1..E3 ending a goal), then public goals (J0..J8), then replacing a host (after v2).

Count complexity on these seven measures and no others, so that the readers of the other pieces count the same way:
1. signed_format: kinds of signed record, fields inside signed records, keys that sign, version numbers.
2. local_state: files, stores, markers and secrets a computer keeps on disk.
3. states: distinct states a goal, a member, or one computer-in-one-goal can be in, and standings such as halted or superseded.
4. surface: commands, flags, API requests, exit codes, refusal kinds, and words a person must learn.
5. background: things the daemon does by itself without being asked (signing, dialing, revoking, holding).
6. invariants: properties that must hold across computers and that the plan says need a formal model or a dedicated test family.
7. code: files touched and lines added and removed.

For every item say whether the plan ADDS it, REMOVES it (it exists in the code today and the plan deletes it) or CHANGES it. For a removal, find the thing in the code today and cite file and line; a removal you cannot find in the code does not count. For an addition, cite the plan location (file and line or heading). Do not count the same item twice.

For the code measure, base the estimate on the files the plan names and on the present size of those files (use wc -l), and say what the estimate rests on. Give a low and a high figure. Test code counts separately from non-test code. Today the workspace holds about 103,000 lines of Rust, of which about 34,500 are in test files.

Also name the three places in your piece where bugs are most likely, and for each larger addition say which of these it is: required by a numbered owner decision in the master plan (give the number), required for safety even though the owner did not ask for it, or something that could be cut or deferred without contradicting an owner decision.

Be exact and plain. No praise, no hedging words. If the plan is silent or contradicts itself on something you need, say so in notes.
`

const LEDGER = {
  type: 'object',
  properties: {
    piece: { type: 'string' },
    one_line: { type: 'string', description: 'What this piece does, one plain sentence' },
    items: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          measure: { type: 'string', enum: ['signed_format', 'local_state', 'states', 'surface', 'background', 'invariants'] },
          direction: { type: 'string', enum: ['adds', 'removes', 'changes'] },
          name: { type: 'string' },
          what: { type: 'string' },
          cite: { type: 'string', description: 'plan location for adds and changes; code file:line for removes' },
          why: { type: 'string', enum: ['owner_decision', 'safety', 'cuttable'] },
          owner_decision: { type: 'string', description: 'number in the master plan, or empty' },
        },
        required: ['measure', 'direction', 'name', 'what', 'cite', 'why'],
      },
    },
    code: {
      type: 'object',
      properties: {
        files_touched: { type: 'integer' },
        nontest_added_low: { type: 'integer' },
        nontest_added_high: { type: 'integer' },
        nontest_removed_low: { type: 'integer' },
        nontest_removed_high: { type: 'integer' },
        test_added_low: { type: 'integer' },
        test_added_high: { type: 'integer' },
        test_removed_low: { type: 'integer' },
        test_removed_high: { type: 'integer' },
        basis: { type: 'string' },
      },
      required: ['files_touched', 'nontest_added_low', 'nontest_added_high', 'nontest_removed_low', 'nontest_removed_high', 'basis'],
    },
    hardest: { type: 'array', items: { type: 'string' }, description: 'three places bugs are most likely, each one sentence' },
    notes: { type: 'string' },
  },
  required: ['piece', 'one_line', 'items', 'code', 'hardest', 'notes'],
}

const PIECES = [
  {
    key: 'roles-person-side',
    prompt: `Your piece: phases 1, 2, 3, 5, 6 and 7 of ${REPO}/docs/roles-and-permissions-plan.md (called R1, R2, R3, R5, R6, R7 in the master plan), with its companion ${REPO}/docs/roles-and-permissions-plan-details.md. These cover the person's side: which commands are the person's alone, the one command grammar and when a command asks first, one level per agent per goal (read, ask, auto), one task allowance, the single check that says who refused, the status view, the documentation sweep and the qualification runs. The plan is about 4,000 lines; read the top sections (intended behavior, decisions) and then your phases in full. This piece is expected to remove existing machinery (grants, permission kinds, delegated commands, older views). Look hard in crates/locust-proto/src/api.rs, crates/locust-core/src/node and crates/locust/src for what exists today and is deleted, and count it.`,
  },
  {
    key: 'roles-goal-side',
    prompt: `Your piece: phases 4, 8, 9 and 10 of ${REPO}/docs/roles-and-permissions-plan.md (called R4, R8, R9, R10 in the master plan), with its companion ${REPO}/docs/roles-and-permissions-plan-details.md. These cover the goal's side: members with names, roles read at the moment of an act, the default rule (peer approval, a lone member needs none, a member's latest review counts, first files need no approval), and the host's computer recording approved plan changes (R8) and approved file changes (R9) by itself, then the final qualification (R10). The plan is about 4,000 lines; read the top sections (intended behavior, decisions) and then your phases in full. Compare against crates/locust-proto/src/organization.rs and crates/locust-core/src/goal today. Count what is removed (for example an integrator role, presets that are renamed or dropped, manual selection commands) as well as what is added.`,
  },
  {
    key: 'key-guard-end',
    prompt: `Your piece: the six host-safety phases K1 (a signing key per goal for membership and rules), G1 and G2 (a computer restored from an old copy signs nothing in the affected goals until it has caught up), E1, E2, E3 (the host ends a goal; leave becomes visible; idle goals dial less). They are not yet in the repository as a document. Their checked text is in ${SCRATCH}/safety.json. Structure: top-level "pieces" is a list of three objects, each with "key" and "piece" (an object of about 14 fields holding the phase text); top-level "seams" holds "verdict", "seams" (28 entries with between, severity, problem, fix), "order", "version_ladder" and "owner_questions". Strings contain HTML entities; read it with python3 and html.unescape. Count the 28 seam fixes as part of the piece where they add state or behavior. Later owner answers that apply (master plan numbers 9, 13, 14, 23): the key is stored with no passphrase; after a whole-computer restore with nobody else to ask, Locust signs nothing until the person runs one command; a member whose agent signs a leave is removed automatically by the host's computer. Background reading if needed: ${REPO}/research/host-key-failure-characterization-2026-10-05.md, ${REPO}/research/goal-lifecycle-characterization-2026-10-05.md, ${REPO}/research/ending-a-goal-2026-10-05.md.`,
  },
  {
    key: 'public-goals',
    prompt: `Your piece: public goals, phases J0 to J8. The phases are not written yet. What exists: the rewrite contract ${REPO}/research/joinable-farms-rewrite-contract-2026-10-05.md (about 1,700 lines, with its own delta map of what survives from the old plan), its review ${REPO}/research/joinable-farms-rewrite-contract-review-2026-10-05.md (13 findings, all accepted) and the old plan ${REPO}/docs/joinable-farms-plan.md. The owner's answers 17 to 24 in the master plan override anything in those three that disagrees. Count what the public door will add once the contract is corrected by its review and those answers: the door record, seats, how a member came in (door or invitation), the public preset, the farm service's new duties (page, gallery, ended pages kept 30 days, helping a restored host notice), the join command and page. Some of this exists in the code today (crates/locust-farm, crates/locust-proto/src/farm.rs, sites/locust.farm); separate what is already built from what is new. Because the phases are unwritten, your code estimate is necessarily rougher than the other pieces; say so and give a wider range.`,
  },
  {
    key: 'backup-host',
    prompt: `Your piece: replacing a host, which the master plan places AFTER v2. It is not designed to the level of phases. What exists: ${REPO}/research/replacing-a-host-2026-10-05.md (about 1,100 lines, the result of a design round with four competing designs and attacks on each) and owner answers 10, 11, 12 and 19 in the master plan: one optional named backup host who can take over alone; a takeover is one record naming the last host record the backup holds; whatever the old host signed after it is void; if the host removed the backup and the backup takes over without having heard, the removal wins. Count what the first version (one optional backup) will add, as far as the note lets you, and separately what the later step (a threshold among several named people) would add on top. Mark every figure as provisional. Say plainly which open problems named in the note (a fresh content key at takeover, telling the old host, removals the backup never received, two takeovers) add state or protocol that the count cannot yet size.`,
  },
]

const BASELINE = {
  type: 'object',
  properties: {
    signed_record_kinds: { type: 'integer' },
    signed_record_list: { type: 'array', items: { type: 'string' } },
    signing_keys_per_goal: { type: 'string' },
    api_requests: { type: 'integer' },
    cli_commands: { type: 'integer', description: 'leaf commands a person or agent can type' },
    cli_command_list: { type: 'array', items: { type: 'string' } },
    permission_machinery: { type: 'string', description: 'what exists today for grants, permissions, delegation: types, how many kinds, where, roughly how many lines' },
    goal_and_member_states: { type: 'array', items: { type: 'string' } },
    halts_and_refusals: { type: 'array', items: { type: 'string' } },
    local_files: { type: 'array', items: { type: 'string' }, description: 'what one computer keeps on disk today' },
    background_behaviors: { type: 'array', items: { type: 'string' } },
    formal_models: { type: 'array', items: { type: 'string' } },
    lines: { type: 'string', description: 'non-test and test line counts per crate, counting inline test modules as test code where you can' },
    notes: { type: 'string' },
  },
  required: ['signed_record_kinds', 'signed_record_list', 'api_requests', 'cli_commands', 'permission_machinery', 'goal_and_member_states', 'halts_and_refusals', 'local_files', 'background_behaviors', 'formal_models', 'lines', 'notes'],
}

phase('Count')
const counted = await parallel([
  () => agent(`You are taking an inventory of an existing Rust system called Locust as it is TODAY, so that a planned redesign can be compared against it. The repository is at ${REPO}. You are read-only: do not edit anything, do not run cargo, npm or any build or test. Use rg, wc, sed -n and reading.

Count, with file and line for each count:
- the kinds of signed record (the event body enum in crates/locust-proto/src; list every variant by name);
- which keys sign what in a goal today;
- the API requests (the Request enum in crates/locust-proto/src/api.rs);
- the leaf commands of the locust command-line program (crates/locust/src; list them);
- the machinery that exists today for permissions, grants and delegation between a person and their agents: the types, how many kinds, where they live, and roughly how many lines of non-test code serve them;
- the distinct states a goal, a member and a computer-in-a-goal can be in today, and the halt, stall and refusal kinds;
- what one computer keeps on disk (stores, key files, markers), from crates/locust-store and crates/locust/src/daemon;
- what the daemon does by itself today without being asked;
- the formal models under research/tla and what each covers;
- lines of Rust per crate, non-test and test, counting inline cfg(test) modules as test code where you can do so cheaply.

Be exact. Where a count depends on a judgement (what is a leaf command), say which judgement you made.`, { label: 'baseline:today', phase: 'Count', schema: BASELINE }),
  ...PIECES.map(p => () => agent(COMMON + '\n' + p.prompt, { label: 'count:' + p.key, phase: 'Count', schema: LEDGER })),
])

const baseline = counted[0]
const ledgers = counted.slice(1).filter(Boolean)
log('ledgers returned: ' + ledgers.length + ' of ' + PIECES.length + (baseline ? ', baseline returned' : ', baseline MISSING'))

const pack = JSON.stringify({ baseline, ledgers })

const HIDDEN = {
  type: 'object',
  properties: {
    verdict: { type: 'string', description: 'three or four plain sentences: is the count fair, too low or too high, and where' },
    missed: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          what: { type: 'string' },
          between: { type: 'string', description: 'the piece or pieces it sits in or between' },
          evidence: { type: 'string', description: 'plan or code location' },
          size: { type: 'string', enum: ['small', 'medium', 'large'] },
        },
        required: ['what', 'between', 'evidence', 'size'],
      },
    },
    wrong: {
      type: 'array',
      items: {
        type: 'object',
        properties: { claim: { type: 'string' }, why_wrong: { type: 'string' }, evidence: { type: 'string' } },
        required: ['claim', 'why_wrong', 'evidence'],
      },
    },
    state_space: { type: 'string', description: 'how many states one computer-in-one-goal can be in today and after v2, and which combinations the plans do not address' },
    top_risks: { type: 'array', items: { type: 'string' }, description: 'the five places v2 is most likely to go wrong in the building, most serious first' },
  },
  required: ['verdict', 'missed', 'wrong', 'state_space', 'top_risks'],
}

const CUTS = {
  type: 'object',
  properties: {
    verdict: { type: 'string', description: 'three or four plain sentences: how much of v2 is fixed by the owner decisions and how much is the plan author choosing' },
    share: { type: 'string', description: 'rough share of the added complexity that is owner_decision, safety and cuttable, and how you weighed it' },
    cuts: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          cut: { type: 'string', description: 'what to drop, defer or merge' },
          saves: { type: 'string', description: 'what it removes on the seven measures' },
          person_loses: { type: 'string', description: 'what a person using Locust would notice' },
          conflicts_with: { type: 'string', description: 'owner decision number it conflicts with, or none' },
          checked: { type: 'string', description: 'where in the plans or code you confirmed the cut is possible' },
          recommend: { type: 'string', enum: ['cut', 'defer', 'keep'] },
        },
        required: ['cut', 'saves', 'person_loses', 'conflicts_with', 'checked', 'recommend'],
      },
    },
    simplifications_already_in_plan: { type: 'array', items: { type: 'string' }, description: 'where v2 makes Locust simpler than it is today, checked against the code' },
  },
  required: ['verdict', 'share', 'cuts', 'simplifications_already_in_plan'],
}

phase('Attack')
const attacks = await parallel([
  () => agent(`A planned redesign of a Rust system called Locust ("v2") has been counted for complexity by five readers, one per piece, plus an inventory of the code as it is today. Their results are below as JSON. The repository is at ${REPO}; you are read-only there (no edits, no cargo, no npm, no builds). The plans: ${REPO}/docs/master-plan.md (read it first), ${REPO}/docs/roles-and-permissions-plan.md, ${SCRATCH}/safety.json (pieces and a list of 28 seams between them; strings hold HTML entities, read with python3 and html.unescape), ${REPO}/research/joinable-farms-rewrite-contract-2026-10-05.md with its review beside it, ${REPO}/research/replacing-a-host-2026-10-05.md.

Your job is to show the count is TOO LOW. Each reader saw one piece, so what sits between pieces is the likely gap. Look for:
- interactions between pieces that add states or rules no single ledger lists (the 28 seams in safety.json are a start; find others, especially between the roles plan and public goals, and between automatic recording by the host's computer and the restore guard);
- combinations of states: a goal can be ended, a computer can be catching up, a host can be superseded, a member can have left, a door can be open. Which combinations do the plans define behavior for and which do they not;
- removals claimed that are not really removals (check a sample of them in the code);
- additions listed as small that the code shows are large (check a sample against the files the plans name);
- work the plans assign to "a formal model" or "a test family" without sizing it.

Also say where the count is too HIGH, if it is. Finish with the five places v2 is most likely to go wrong when it is built, most serious first. Plain and exact; cite locations.

COUNT:
${pack}`, { label: 'attack:hidden-complexity', phase: 'Attack', schema: HIDDEN }),
  () => agent(`A planned redesign of a Rust system called Locust ("v2") has been counted for complexity by five readers, one per piece, plus an inventory of the code as it is today. Their results are below as JSON. The repository is at ${REPO}; you are read-only there (no edits, no cargo, no npm, no builds). The plans: ${REPO}/docs/master-plan.md (read it first; its section "Decided by the owner" lists 25 numbered decisions that are fixed, and "Assumed until the owner objects" lists the plan author's own choices, which are not), ${REPO}/docs/roles-and-permissions-plan.md, ${SCRATCH}/safety.json (strings hold HTML entities, read with python3 and html.unescape), ${REPO}/research/joinable-farms-rewrite-contract-2026-10-05.md with its review beside it, ${REPO}/research/replacing-a-host-2026-10-05.md.

Your job is to find what can be CUT, DEFERRED or MERGED without contradicting any of the 25 owner decisions. The owner's first principle is that a person should meet no friction, and the project's rule is no dead code and no migrations. For each candidate, say what it saves on the seven measures (signed format, local state, states, surface, background behavior, invariants, code), what a person using Locust would notice, and whether it conflicts with a numbered decision. Confirm each candidate in the plans or code before listing it; do not list a cut you have not checked is separable. Give a recommendation for each: cut, defer, or keep (keep means you examined it and it earns its place). Aim for the ten candidates that matter most, not a long list.

Then say what share of the added complexity is fixed by owner decisions, what share is there for safety, and what share is the plan author's choice, and how you weighed it. Last, list where v2 makes Locust simpler than it is today, checked against the code.

COUNT:
${pack}`, { label: 'attack:what-can-be-cut', phase: 'Attack', schema: CUTS }),
])

return { baseline, ledgers, hidden: attacks[0], cuts: attacks[1] }
