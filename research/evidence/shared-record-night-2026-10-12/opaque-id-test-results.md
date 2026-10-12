# Opaque candidate-ID repair: offline verification

Verified 12 October 2026 in the isolated worktree at
`/Users/andrei/.codex/worktrees/opaque-candidate-ids/locust`.
Implementation commit: `1eb43fc60cdc09ba2ee0df38177284045ceed708`.
This evidence was preserved before applying that change to the primary
checkout. Existing paid studies retain their frozen runtime and original
model-visible identifiers.

Raw reports and archives named below remain local-only and are excluded from
Git. The tracked [evidence index](README.md) records their inventory and the
retained summary; reproducing archive checks requires those local files.

The change generates each candidate handle once as `candidate-` followed by
UUID4 hexadecimal digits. Stored handles remain unchanged through common
prefix forks, parent references, source retrieval and private-review
serialization. Internal trial/agent accounting labels remain archived;
truthful worker attribution and the policy field remain visible. This removes
avoidable execution-label cues without claiming that receipt of peer evidence,
attribution or context costs are blinded.

## Completed checks

From `research/experiments/shared_record` in that isolated worktree:

```sh
/opt/homebrew/bin/python3.12 -m unittest discover -p 'test_*.py'
/opt/homebrew/bin/python3.12 run.py replay --output ../../../output/opaque-candidate-id-replay --binary ../../../target/debug/locust
```

The full offline suite passed **128 tests in 13.624 seconds**. The new cases
exercise distinct handles for repeated submissions and different workers,
stable stored identities, exact parent/source authorization, unchanged prefix
reuse, and actual scripted request payloads across all arms and the continue,
verify and restart procedures.

The scripted replay verification, `opaque-id-replay-verification.json`, passed:
four blocks, 36 trials, 24 isolated real Locust goals, 208 signed publication
receipts, 148 distinct opaque candidate handles and 306 archived generation
payloads. Two injected uncertain transport failures, four empty filtered
deliveries and 12 ineligible peer recipients remain in the archive. No paid
model calls occurred. Its synthetic responses and fixture scores establish
protocol behavior, not candidate quality or a communication benefit.

The replay used an exact copy of the existing primary debug binary, SHA-256
`74e8a05a061489dda54ac931dd95704ee99b9577d0a24fed4e107dea9babf01c`;
no Rust source was changed or rebuilt. The staged documentation check and
`git diff --cached --check` passed before the implementation commit.

The separate model-label audit, `opaque-id-model-label-audit.json`, found no
registered internal execution labels in 306 distinct request payloads,
1,054 archived history items, 336 tool results or four private-review history
items. Responses confirm receipt for 304 requests; two are deliberately
failed attempts. This checks the auditor's declared literal/structured label
scope, not inferred treatment, model attention or semantic use. The
audit verification receipt, `model-label-audit-verification-receipt.json`,
records 18 passing auditor tests, positive controls that detect the former
label exposure and exact reproduction using the preserved helper.

## Preserved archive

The local sealed replay archive, `opaque-id-replay-sealed.tar.xz`, contains the exact
73 files selected by the isolated `analyze.package`, totaling 11,106,760
payload bytes. Deterministic sorted USTAR metadata with zero timestamps and
ownership, mode `0644`, and XZ preset 6/CRC64 produced 194,916 bytes. Archive
SHA-256: `df51584153436e7424fda8287b8dd5dd3a1d75e2f0e1336dc8aeb595e43aabc9`.

The archive verification receipt, `opaque-id-replay-sealed-receipt.json`,
records every member hash, both seals, exact extracted-byte comparisons,
identical archive regeneration, reproduced scripted verification and
unchanged sealed source files. Every frozen source/helper hash was also
matched to its blob in the implementation commit. The manifest records the
pre-commit HEAD `ea17cbb55180eefd6848219785ae1611b8cc3494` because verification
preceded committing; the executed source hashes match the final commit.

With the local archive present in this evidence directory, extract into an
empty destination:

```sh
tar -xJf opaque-id-replay-sealed.tar.xz -C /path/to/empty-directory
```

This restores `opaque-id-replay/`, including the existing output and hidden
seals, request/candidate archives, completed hidden journals and analysis.
The binary and private daemon state are excluded. Packaging and its read-only
checks made no provider requests and performed no evaluator reruns.
