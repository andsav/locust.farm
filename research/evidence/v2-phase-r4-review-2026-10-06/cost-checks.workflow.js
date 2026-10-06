export const meta = {
  name: 'review-phase-5-cost-checks',
  description: 'Second readers for the three replay-cost findings of the phase 5 review',
  phases: [{ title: 'Verify', detail: 'one second reader per cost finding' }],
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
phase('Verify')
const out = await parallel(args.map((f, i) => () => agent(`
A reviewer of commit 8c086c1 in . (the roles plan's "Phase 4: Names and roles in the goal"; plan at docs/roles-and-permissions-plan.md lines 2274-3215; builder's notes at research/v2-phase-r4-build-notes-2026-10-06.md) reported the performance finding below. The project's stated engineering priority is ruthless performance of replay. You are the second reader. Try to REFUTE it by reading the code yourself.

HARD RULES: read only. Do not edit or create files. Do not run cargo, npm, tests, benchmarks or builds. Use git show 8c086c1:<path>, git show 8c086c1 -- <path>, git grep, grep, sed -n and reading files. Compare with the code before the commit (git show 8c086c1^:<path>) to decide whether this commit added the cost.

THE FINDING
Title: ${f.title}
Where: ${f.file}:${f.line}
Claimed severity: ${f.severity}
Scenario: ${f.scenario}
Evidence: ${f.evidence}
Proposed fix: ${f.fix}
What a person would see: ${f.person_sees}

DO THIS
1. Open the cited lines and every function on the cited path. Check each count in the scenario: what is cached and for how long, how often the enclosing function runs (per replay, per request, per record), and whether the cost existed before this commit.
2. Decide: "confirmed" (the cost is as described and this commit added it), "partly" (real, but the growth, the cause or the fix as written is wrong; restate it), or "refuted" (the cost is not there, was there before, or is bounded small). Nothing was measured, so say plainly that the numbers are by reading.
3. "statement": the finding as it should stand, one or two sentences. "scenario": corrected concrete numbers. "fix": the smallest change (name the function). "person_sees": plain everyday words. "reason": what you checked, with file:line.`, { label: `verify:cost:${i + 1}`, phase: 'Verify', schema: VERDICT_SCHEMA }).then(v => ({ ...f, second: v }))))
return out.filter(Boolean)
