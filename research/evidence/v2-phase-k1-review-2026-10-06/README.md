# Evidence for the review of phase K1 and three experiments

Raw results behind
[the review of phase K1](../../v2-phase-k1-review-2026-10-06.md), collected
on 6 October 2026.

- `results.json`: `k1` holds one entry per line of review, each finding with
  its scenario, its proposed fix and, under `check`, the verdict of the
  reader who tried to refute it. `experiments` holds the three assessments.
- `review.workflow.js`: the instructions each reader was given.

Every scenario is a hand trace against the code as read. The assessor of the
restore guard's model re-ran its registered cases, and the assessor of the
file-identity measurements re-ran the macOS script.
