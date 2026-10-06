# Evidence for the check of the v2 plan review

Raw results behind
[checking the v2 plan review](../../v2-plan-review-verification-2026-10-06.md),
collected on 6 October 2026.

- `results.json`: what the seven readers returned, unedited apart from
  shortened paths. `verified` holds one entry per group of findings, each
  finding with its verdict, the trace as the reader rebuilt it, what the
  review got wrong, and the changes the reader named. `scope` is the study of
  recovering under a new key.
- `verify.workflow.js`: the instructions each reader was given.

Nothing was built or run. Every trace is a hand trace against the plan text
and the code as read.
