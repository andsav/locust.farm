# Evidence for the review of phase R4 as built

Kept for the [review of phase R4](../../v2-phase-r4-review-2026-10-06.md),
6 October 2026.

- [results.json](results.json) holds everything the readers returned.
  `distinct` is the 52 findings of the note, each with the reports folded
  into it. `reports` is every report as its first reader wrote it, with each
  second reader's verdict, corrected statement, steps, fix and reasons.
  `coverage` is what each reader said it read. `critic` is the reader that
  named what nobody had read. `k1_findings_after_phase_5` is one reader's
  pass over the [K1 review](../../v2-phase-k1-review-2026-10-06.md)'s
  findings as they stood in `8c086c1`, before the K1 fixes landed.
- [review.workflow.js](review.workflow.js) is the script that ran the
  eighteen readers, their second readers, the critic and the cost reader.
- [cost-checks.workflow.js](cost-checks.workflow.js) is the script that ran
  one second reader for each of the three cost findings.

The first script was stopped twice near its end and resumed. The resume ran
the second readers again, which is why each of the first 94 reports has two
verdicts; the note keeps the stricter one. The step that was to fold
duplicates together never ran, so the reports were grouped by hand; the
grouping is the `reported_by` list of each distinct finding.
