export const meta = {
  name: 'review-k1-and-experiments',
  description: 'Review the landed phase K1 code along five lines with each finding tried for refutation, and assess the three parallel experiments',
  phases: [
    { title: 'Find', detail: 'five readers of the K1 change, three assessors of the experiments' },
    { title: 'Refute', detail: 'each K1 reader\'s findings are tried for refutation by a second reader' },
  ],
}

const REPO = '.'

const COMMON = `
You are reviewing work that just landed in Locust, a Rust system in which several people's coding agents work on one goal; every member's computer keeps a full signed copy of the goal as per-author append-only logs, and what counts is computed by every computer from the records it holds. The repository is at ${REPO}. You are READ-ONLY there: do not edit, create or delete any file in the repository. Do not run cargo or npm; the full test suites are being run separately and they pass or fail on their own account. You may read files and run read-only shell commands (rg, sed -n, wc, git log, git show, git diff).

Background, short. The plan is docs/master-plan.md (read its opening table and "Decided by the owner"; 27 numbered answers are fixed: a person meets no friction; no migration; agents' work never waits for a human; the failure to design for is a host that disappears). Phases 1 to 3 of the build order are built. Phase K1, fourth in the build order, landed in commit cb1acaa with its model change in 0a4bbc2 and its build notes in a6664a1 (research/v2-phase-k1-build-notes-2026-10-06.md). K1 gives each goal a signing key of its own for members and rules (the governance key; text a person reads says only "host"), makes the agent that started the goal an ordinary member (the host's agent), raises the protocol version from 6 to 7, and adds one command that connects a disconnected agent again. Its specification is in docs/host-safety-and-ending-plan.md: the section "What the host's computer signs by itself", the K1-* terminal texts, and "### K1: A goal's governance has its own key" (Goal, Depends on, Changes, Tests, Exit criteria, Risks and notes); and in docs/host-safety-and-ending-plan-details.md, "The signing key (K1)".

Write plainly: short declarative sentences, no dashes used as punctuation. Cite file and line for every claim. Say what you verified by reading and what you inferred.
`

const FINDINGS = {
  type: 'object',
  properties: {
    read: { type: 'string', description: 'what you read in full, what you skimmed, what you did not open' },
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          file: { type: 'string', description: 'path relative to the repository root' },
          line: { type: 'integer' },
          summary: { type: 'string', description: 'the defect in one sentence' },
          failure_scenario: { type: 'string', description: 'concrete: which computers, records, commands and order lead to the wrong result, and what the wrong result is' },
          severity: { type: 'string', enum: ['a goal stops for good', 'computers disagree', 'security or trust boundary', 'wrong result for a person', 'work waits on a human', 'departs from the plan', 'dead or leftover code', 'test gap', 'efficiency', 'wording'] },
          smallest_fix: { type: 'string' },
        },
        required: ['file', 'line', 'summary', 'failure_scenario', 'severity', 'smallest_fix'],
      },
    },
    holds: { type: 'array', items: { type: 'string' }, description: 'what you attacked and could not break' },
  },
  required: ['read', 'findings', 'holds'],
}

const VERDICTS = {
  type: 'object',
  properties: {
    verdicts: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          index: { type: 'integer', description: 'the finding\'s position in the list you were given, from 0' },
          verdict: { type: 'string', enum: ['confirmed', 'partly', 'refuted', 'cannot tell'] },
          evidence: { type: 'string', description: 'the trace as you rebuilt it or the code that makes it impossible, with file and line' },
          corrected_summary: { type: 'string', description: 'if the finding holds in a narrower or different form, state it; otherwise empty' },
        },
        required: ['index', 'verdict', 'evidence', 'corrected_summary'],
      },
    },
  },
  required: ['verdicts'],
}

const K1 = [
  { key: 'replay', prompt: `YOUR LINE: the signed format and replay. Read the K1 diff of crates/locust-proto (git show cb1acaa -- crates/locust-proto) and of crates/locust-core/src/goal (chain.rs, fold.rs, history.rs, screen.rs, state.rs, mod.rs, flow.rs, standing.rs, projection.rs and their tests), then the enclosing functions. Attack it with concrete traces, each with named keys, records and arrival orders: the first record (who may author it, what the goal identifier commits to, a first record whose host equals its governance key, two first records); the governance key signing anything outside the kinds it may sign, and whether that work can ever count or be selected; the governance key appearing as a member, a role holder, a task creator, a reviewer or the only member; an admission or a removal that names the governance key or the host's agent; a fork of the host's agent's log (it must cost what any member's fork costs and leave members and rules alone); a fork of the governance key's log; stage steps signed by the governance key (task creator, rules that name the task's creator, subtasks, start rules, offers); records anchored before and after; a goal in which the host's agent is removed, has left or is disconnected; sync screening of records by a key that is no member. Look for any place where two honest computers holding the same records could reach different answers, and any place where replay accepts something K1's text says it refuses.` },
  { key: 'node', prompt: `YOUR LINE: the daemon's own paths. Read the K1 diff of crates/locust-core/src/node (git show cb1acaa -- crates/locust-core/src/node), then the enclosing functions: founding a goal (requests/goals.rs), host() and hosts() and the one place that enforces the rule for what the host's computer signs by itself (K1's text calls it author_alone; find what was built), next_place and authoring, the store record that holds the goal's key and how it is loaded, written and never leaked into a view or a sync frame, admission on an invitation (peers.rs), stage steps (flow.rs), invitations and tickets (requests/invitations.rs and crates/locust-proto/src/invite.rs), publication (farm.rs), removal and the content key, identity.rs and requests/daemon.rs for disconnecting and connecting an agent again, and commit.rs. Check against "What the host's computer signs by itself": each unattended signature must come out the same if made twice from the same records, and must be passed over, never an error, when it cannot be made, so that it never fails inside start or inside landing a received batch. Look for: a path that still signs members or rules with the agent's key, or work with the goal's key; a host command that fails or waits because the host's agent is disconnected, left or missing; a key that can be read by an agent or sent to a peer; a second record at a used position of the goal's key after a retry, a restart or a failed commit; reconnecting an agent that was never disconnected, that left, or that is the host's agent of a goal; the replayed copy that sign_for hands to the commit (access.rs, authoring.rs, commit.rs) still being adopted correctly.` },
  { key: 'plan', prompt: `YOUR LINE: conformance to the plan. Read K1's text in docs/host-safety-and-ending-plan.md in full (Changes bullet by bullet, Tests, Exit criteria), the companion's lists for the signing key (owns, removed or rewritten, the table of behaviours with the test that shows each), and the build notes' section "Departures from the plan, and corrections of it". For every Changes bullet, every removal and every named test, find it in the tree (rg) and say: built as written, built differently (how, and whether the build notes say so), or missing. Check that what K1 supersedes is gone and not left beside the new code: AGENTS.md at the repository root says to remove superseded code, APIs, flags, tests and documentation in the same change and to leave no dead code or dormant fallback. Check the frozen vectors, generated contracts, the guide's executable recipes, the skill text under skills/, the scripts that the K1 commit touched and docs/site.json. Check each departure the build notes declare: is it a sound reading of the plan and the owner's answers, or does it change what a person sees or what every computer must agree on without saying so. Report as findings only real gaps: something the plan requires that is missing or contradicted, an undeclared departure, a leftover, or a test the plan names that does not exist or does not test what its name says.` },
  { key: 'surface', prompt: `YOUR LINE: what a person and an agent see. Read the K1 diff of crates/locust/src (cli, mcp, daemon, installation), of skills/, docs/guide, docs/reference and sites/locust.farm/src, and the K1-* terminal texts in the plan. Check: no text a person reads names the governance key or prints it (status, plans, refusals, event lists, the join plan, invitations, the public page data), and JSON for scripts carries it only where K1 says; "Host: you" and the host lines read as the plan's texts; disconnecting an agent applies at once and prints the command that connects it again, and that command works in every state it can be typed in (never disconnected, disconnected twice, the host's agent, an agent in no goal, an agent that left a goal) and says something true in each; an agent cannot call the new request and it is not offered as a tool; refusals that used to say the host agent is disconnected; the formation check that refuses rules naming a task's creator where a stage's task would leave it with no possible actor, in Rust (crates/locust-core/src/organization) and in its mirror in the site's formation editor (sites/locust.farm/src/lib/formation-editor/contract/rules.ts): do the two agree case by case, and does the refusal tell an author what to write instead. Count, for a host who disconnects the agent they started a goal with and later wants it back, the commands and the confirmations.` },
  { key: 'removed-and-tests', prompt: `YOUR LINE: what K1 took away, and what its tests really show. For every line the K1 commit deletes or replaces in crates/ (git show cb1acaa), name the behaviour or guard it enforced and find where the new code re-establishes it; a guard you cannot find again is a finding. Pay attention to: checks on the endpoint an admission names, the test that a record's author is a member, anything keyed by the old creator's key (historical endpoints, sync peers, who a daemon dials and whom it answers, the responder's test for members in crates/locust-core/src/sync), invitation checks, the publication's required consents, and the simulator scenarios under crates/locust-core/src/node/sim. Then read the tests K1 added or rewrote (git show cb1acaa --stat lists them): for each one named in the plan's Tests list, say whether it would fail if the behaviour in its name were broken, or whether it only asserts what the fixture set up. Name any test that was deleted or weakened without a replacement, and any behaviour K1 changed that no test covers. Also read the model change (git show 0a4bbc2): do the two new scenarios and their invariants say what K1's text claims, and does anything in the Rust replay differ from what the model assumes.` },
]

const EXPERIMENTS = [
  { key: 'restore-guard-model', prompt: `YOUR SUBJECT: the formal model of the restore guard, written before its code. It landed in commits 0ba1f31 (the runner learned temporal properties), cb7cf1a (research/tla/RestoreGuard.tla, research/tla/restore-guard.md, research/restore-guard-model-2026-10-06.md, results) and 1bbea98 (cases). Its specification is the paragraph "The restore guard (G1, G2)" under "Models written first" in docs/host-safety-and-ending-plan.md, and phase G1's two tables in the same file. You MAY run the model: /opt/homebrew/bin/python3 scripts/check_tla.py with the options research/tla/README.md describes (it writes only under the ignored output/ directory); run the restore cases and say what you ran and what it answered. Assess: does the model's state and each action match G1's text (the two tables row by row), or does it model something easier; does each of the six properties in the plan have a case, stated as the plan states it; is the property that a hold ends really checked as a liveness property under a stated fairness assumption, and is that assumption one the real system could meet; does each named counterexample have a case that fails when its rule is removed; are the bounds too small to see the failures that matter. Then the findings the note reports that the plan does not say (it reports cases named "finding"): for each, rebuild the trace, say whether it is a real hole in G1 as written, and what the smallest change to G1's rules is. Say plainly whether phase G1 can be built as its text stands.` },
  { key: 'file-identity', prompt: `YOUR SUBJECT: the file-identity measurements, a gate for phase G1. They landed in commit e1e3e0d: research/restore-file-identity-2026-10-06.md with scripts and raw results under research/evidence/restore-file-identity-2026-10-06/. G1 decides whether a daemon was started from a copy of its data by comparing the database file's inode number and creation time with what it recorded (read "FileId" and the first table in "### G1" of docs/host-safety-and-ending-plan.md, and G1's "Risks and notes" and exit criteria). You MAY re-run the macOS script in a new directory under /tmp to see that its numbers reproduce (read it first; run it only if it writes nowhere else). Assess: do the scripts measure what the note claims, and does each row of the note's table follow from the raw results; which cases were not measured and does the note say so; for each case, is the note's reading of what G1's rule would conclude right against G1's text. Then the consequences: list every case where a restore would be taken for the same file (missed) and every case where an ordinary start would be taken for another file (a false alarm), say what G1's text already does about each, and what it does not. Say plainly whether G1's rule for file identity can stand as written, must change (and to what, at the smallest), or needs a measurement that is still missing.` },
  { key: 'public-goals-review', prompt: `YOUR SUBJECT: an independent review of the public-goals plan, research/public-goals-plan-review-2026-10-06.md (commit 96dd045), which reviews docs/joinable-farms-plan.md (six phases, J1 to J6, described by behaviour). Try to REFUTE each of its findings: read the plan text it cites and the code it cites, rebuild its trace, and look for a sentence elsewhere in the plan that already closes it, a code fact that makes it impossible, or a misreading. Use the findings schema this way: one entry per finding of the review, with "summary" the finding in your words, "failure_scenario" your rebuilt trace and your verdict in its first word (CONFIRMED, PARTLY, REFUTED or UNCLEAR), "severity" by its kind, "file" and "line" the place in docs/joinable-farms-plan.md that must change (or the review's own line if it is refuted), and "smallest_fix" the smallest change to the plan that closes it. Then say under "holds" what the review got right overall, what it missed that you saw while checking, and its verdict (build as written, build after named changes, or do not build) with whether you agree. Note for context: the owner's answers 18 (a door member's task becomes available once a trusted agent approves it; no person is asked) and 26 (agents' work never waits for a human) are fixed; and the plan deliberately has no file-by-file change lists.` },
]

phase('Find')
const k1 = pipeline(
  K1,
  line => agent(COMMON + '\n' + line.prompt + '\n\nReport at most ten findings, the most serious first. A finding needs a concrete scenario; do not report style. Under "holds" list what you attacked and could not break.', { label: 'find:' + line.key, phase: 'Find', schema: FINDINGS }),
  (found, line) => {
    if (!found || !found.findings.length) return Promise.resolve({ line: line.key, read: found ? found.read : '', holds: found ? found.holds : [], findings: [] })
    return agent(COMMON + `
YOUR TASK: try to REFUTE findings. A reviewer of the K1 change reported the findings below as JSON. For each one, read the code at the place it names and around it, rebuild its scenario step by step, and look for the code that makes it impossible: a check earlier in the path, a test that pins the opposite, a rule in replay, or a sentence in the plan or the build notes that makes the behaviour intended. Say "confirmed" only when you rebuilt the scenario and could not break it; "partly" when it holds in a narrower form (state that form); "refuted" when it does not hold (show the code); "cannot tell" when reading does not settle it. Do not defer to the reviewer.

FINDINGS:
${JSON.stringify(found.findings)}`, { label: 'refute:' + line.key, phase: 'Refute', schema: VERDICTS }).then(v => ({
      line: line.key,
      read: found.read,
      holds: found.holds,
      findings: found.findings.map((f, i) => ({ ...f, check: v ? v.verdicts.find(x => x.index === i) || null : null })),
    }))
  },
)
const experiments = parallel(EXPERIMENTS.map(e => () => agent(COMMON + '\n' + e.prompt, { label: 'assess:' + e.key, phase: 'Find', schema: FINDINGS }).then(r => r && ({ subject: e.key, ...r }))))

const [k1Result, expResult] = await Promise.all([k1, experiments])
return { k1: k1Result.filter(Boolean), experiments: expResult.filter(Boolean) }
