# Evidence for the review of phase R5 and the R4 fixes

Kept for the [review of phase R5 and the R4 fixes](../../v2-phase-r5-review-2026-10-06.md),
6 October 2026.

- [results.json](results.json) holds everything the readers returned.
  `distinct` is the 47 findings of the note, each with the reports grouped
  into it. `reports` is every report as its first reader wrote it, with the
  second reader's verdict, corrected statement, steps, fix and reasons.
  `coverage` is what each reader said it read.
- [review.workflow.js](review.workflow.js) is the script that ran the fifteen
  readers, one second reader per report, and the reader that grouped
  duplicates.
