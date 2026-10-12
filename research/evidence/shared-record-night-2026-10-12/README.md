# Shared-record $30 exploratory campaign evidence

Git retains this report, the [compact result summary](summary.json), Markdown
reviews and diagnostic Python source. Raw manifests, study archives, traces,
full audit reports and receipts are local-only files excluded by `.gitignore`.
Filenames marked “local only” below refer to files retained beside this README,
not downloadable repository artifacts. A fresh clone cannot independently
reproduce the paid results without those local records. The
[findings synthesis](../../shared-record-information-policy-synthesis-2026-10-12.md)
is the recommended entry point.

The experiment's 18 original mainline commits and isolated opaque-ID repair were
preserved before consolidation in the ignored local
`output/shared-record-squash-2026-10-12/original-experiment-history.bundle`.
Its SHA-256 is `8d868a9250bd35dcafcc58018d46937efdde10515ec7dbb4df25ba91fd4c4911`.
Restoration into a fresh bare repository containing only the base ancestry
verified all 19 commits and trees, 11 manifest artifacts and 385 source checks.
The adjacent local `original-experiment-history-receipt.json` contains restoration
commands and detailed identities. The compact summary retains the hashes and
base commit. Original manifest, seal and source IDs remain unchanged; their
historical Git objects are available from that local backup. The bundle and raw
evidence are not part of the consolidated Git history.

These records belong to the [12 October execution protocol](../../shared-record-information-policy-nightly-2026-10-12.md).
The calibration manifests were frozen before any generation request. They
share calibration task packs, differ in the declared private continuation
procedure, and each reserve at most $1.68 from the same $30 campaign.

- Initial campaign allocation (`campaign-initial.json`; local only)
- Continued solo calibration manifest (`calibration-continue-manifest.json.gz`; local only)
- Private verification calibration manifest (`calibration-verify-manifest.json.gz`; local only)
- Private restart calibration manifest (`calibration-restart-manifest.json.gz`; local only)

The original verification/restart freezes above were cancelled before any
execution after the Responses minimum-output fix. The following replacements
preserve their exact task packs and initial prompts:

- Replacement verification calibration manifest (`calibration-verify-min16-manifest.json.gz`; local only)
- Replacement restart calibration manifest (`calibration-restart-min16-manifest.json.gz`; local only)
- Campaign with cancellation and replacement proofs (`campaign-after-preflight-replacement.json`; local only)

The manifest evidence is stored as deterministic gzip archives (`mtime=0`).
Decompression preserves the exact original JSON bytes and raw SHA-256 values
listed below; for example, `gzip -dc calibration-continue-manifest.json.gz > calibration-continue-manifest.json`
reconstructs that original manifest. The live output originals and their freeze
receipts remain unchanged.

| Archive | Original bytes | Archive bytes | Original SHA-256 |
| --- | ---: | ---: | --- |
| `calibration-continue-manifest.json.gz` | 3194032 | 159527 | `2aa7342cbaf385dad1e50095e3cd1bc64f50a71c33bde256f8f7cd5637e72b84` |
| `calibration-restart-manifest.json.gz` | 3194027 | 159533 | `3a6f8a6c07471107540fff5971dd206ca006e750001f3fee243045a09e18f1f0` |
| `calibration-restart-min16-manifest.json.gz` | 3194045 | 159538 | `ca976b39f1f636bca73e8b59a8a74f9b49d3dfbd43ed21ce9abbf4e7300dd10e` |
| `calibration-verify-manifest.json.gz` | 3194022 | 159534 | `ed79303b3c79bdeb13b2ca60861e5df52dca1d644fdac7cb8436f42edd5902a1` |
| `calibration-verify-min16-manifest.json.gz` | 3194040 | 159534 | `c33e2f3517061c29c64424e36a3d742455928408bdc017d42ece9cae0da8e191` |

## Completed continued-solo calibration

- Sealed study archive (`calibration-continue-sealed.tar.gz`; local only)
- Archive receipt and per-file SHA-256 inventory (`calibration-continue-sealed-receipt.json`; local only)
- Read-only qualification report (`qualification-continue.json`; local only), copied separately outside the study seal

The archive contains all 54 files selected by `analyze.package`: the exact
manifest, eight sealed live-evidence files, output seal, completed hidden scores
and seal, 41 hidden-journal files, and existing analysis. It omits the binary and
private daemon state. The 13,938,945 payload bytes compress to a 4,325,252-byte
archive with SHA-256 `86dd049f174fe636fdaa3eefd1171abc6af7ea5e4ff9a2edfbd6346c028f6dc2`.
Sorted USTAR members use mode `0644`, zero ownership and timestamps; gzip uses
`mtime=0`. Every extracted file was checked byte-for-byte against the source,
and both live and hidden seals were verified after extraction. Packaging made
no model calls or evaluator runs and left the source study unchanged.

From this evidence directory, `tar -xzf calibration-continue-sealed.tar.gz -C /path/to/empty-directory`
restores the `calibration-continue/` study directory. Its existing `analysis.json`
contains the results below; the qualification report records ties, repeat
variation, continuation coverage and retained failures.

| Family | Solo arm/model | Selected hidden normalized cost | Candidates | Attributed cost |
| --- | --- | ---: | ---: | ---: |
| Max cut | S / gpt-6-luna | 0.969490 | 3 | $0.004861 |
| Max cut | R / gpt-6-astra | 0.945162 | 5 | $0.354808 |
| Vertex cover | S / gpt-6-luna | 0.955765 | 8 | $0.019988 |
| Vertex cover | R / gpt-6-astra | 0.951777 | 4 | $0.419999 |

Lower normalized cost is better. All four public-selected deliverables were
hidden-valid, and none used the fallback. The study retains 31 dispatched
requests: $0.710231735 in known usage cost plus $0.089425 reserved for one failed
request with unknown charge, totaling $0.799656735 accounted against the
campaign. The failed Astra vertex-cover request received HTTP 400 because its
remaining allowance produced `max_output_tokens=1`, below the provider minimum
of 16. Its failure and full reservation remain in the sealed ledger.

All four trials continued after their first candidate. The two frozen evaluator
repeats showed no cost variation or validity flips among the 20 source records;
invalid candidate evaluations are still retained. Four distinct-source public
tied pairs occurred, and maximum selected-versus-best hidden regret was
0.0014503263. These are descriptive observations from one pack per family and
two evaluator repeats; they do not establish stable evaluator noise, general
selector quality, a selected continuation procedure, or a collaboration effect.

## Completed private-verification calibration

- Sealed study archive (`calibration-verify-min16-sealed.tar.gz`; local only) and SHA-256 receipt (`calibration-verify-min16-sealed-receipt.json`; local only)
- Qualification report (`qualification-verify.json`; local only), private-stage execution report (`private-stage-verify.json`; local only), and independent accounting audit (`accounting-verify.json`; local only), all outside the study seal

The archive preserves 76 exact files, including 63 hidden-journal files and the
existing analysis: 19,957,575 payload bytes in an 8,478,325-byte deterministic
archive. Its SHA-256 is `754655e9a1ce37df0f2904603c6674dea2dc5b432e3d13b412c437dff92c02bb`.
It uses the same packaging, metadata and round-trip seal verification as above.
Extracting it restores `calibration-verify-min16/`.

| Family | Solo arm/model | Selected hidden normalized cost | Candidates | Usage cost | Private verification executed |
| --- | --- | ---: | ---: | ---: | --- |
| Max cut | S / gpt-6-luna | 0.949579 | 9 | $0.012756 | Yes, completed response and candidate |
| Max cut | R / gpt-6-astra | 0.948367 | 6 | $0.357109 | No, private prompt not queued |
| Vertex cover | S / gpt-6-luna | 0.953590 | 12 | $0.023985 | Yes, completed response and candidate |
| Vertex cover | R / gpt-6-astra | 0.948151 | 4 | $0.362942 | No, private prompt not queued |

All four selected deliverables were hidden-valid without fallback. All 40
generation requests settled, totaling $0.75679152 in provider-usage charges and
zero uncertain reservations. The independent audit matched every generation
input count to usage and found no accounting failures; these charges are not an
independently reconciled invoice. Luna executed private verification on both
families. Astra did not execute its declared private stage on either family,
so this run does not establish Astra verification quality. These remain sparse
calibration observations, with no collaboration-effect claim.

## Completed private-restart calibration

- Sealed study archive (`calibration-restart-min16-sealed.tar.xz`; local only) and SHA-256 receipt (`calibration-restart-min16-sealed-receipt.json`; local only)
- Qualification report (`qualification-restart.json`; local only), private-stage execution report (`private-stage-restart.json`; local only), and independent accounting audit (`accounting-restart.json`; local only), all outside the study seal

The deterministic XZ archive uses preset 6 and CRC64 over the same sorted USTAR
format described above. It preserves 68 exact files, including 55 hidden-journal
files: 17,728,186 payload bytes in a 769,576-byte archive, with SHA-256
`260647166caff92872f0315689b337bf5ec34f4b42db6291106feff7a37723bd`.
Every extracted file and both seals were verified. Extract with
`tar -xJf calibration-restart-min16-sealed.tar.xz -C /path/to/empty-directory`.
Earlier local gzip archives remain unchanged.

| Family | Solo arm/model | Selected hidden normalized cost | Candidates | Usage cost | Private restart executed |
| --- | --- | ---: | ---: | ---: | --- |
| Max cut | S / gpt-6-luna | 0.950727 | 13 | $0.018348965 | Yes, completed response and candidate |
| Max cut | R / gpt-6-astra | 0.945400 | 5 | $0.341708500 | No, prompt queued without generation |
| Vertex cover | S / gpt-6-luna | 0.956490 | 5 | $0.023120430 | Yes, completed response and candidate |
| Vertex cover | R / gpt-6-astra | 0.951777 | 4 | $0.363947500 | No, private prompt not queued |

All four selected deliverables were hidden-valid without fallback. All 46
generation requests settled, totaling $0.747125395 in provider-usage charges,
with no uncertain reservations or failed requests. The independent audit
matched all 46 generation input counts to usage. Luna executed the restart
stage on both families; Astra did not execute it on either family.

## Calibration selection and development freeze

The consolidated selection report (`calibration-selection.json`; local only) applies the
frozen rule and selects the declared `verify` procedure for both models.
Astra's verification stage did not execute, so this selects an observed
comparator without establishing a benefit from private verification. The
report explicitly leaves group-prefix qualification unresolved by these
full-allowance solo observations.

The policy-development prelaunch manifest (`policy-development-manifest.json.gz`; local only)
freezes seven arms on fresh max-cut and vertex-cover packs, with `verify`
continuation and a $5.88 allocation. Its exact 3,172,510 raw bytes have SHA-256
`89cda826c1782407bac261854e83c5d2bf9acf93317b9b0625b138dd65402421`;
the deterministic gzip copy is 158,855 bytes. The
campaign snapshot after calibration and development allocation (`campaign-after-calibration.json`; local only)
is an exact 28,948-byte copy with SHA-256
`e9df80ecc1c9446b70759c218496f6759c536f97a8c1d25ee712e175703a331a`.
These copies were verified against the frozen manifest receipt before any
development execution marker or request ledger existed. They are prelaunch
records, not development results.

## Secondary diagnostics and helper source

The continued-study accounting audit (`accounting-continue.json`; local only),
continued-study private-stage report (`private-stage-continue.json`; local only), and
continued-study qualification recheck (`qualification-continue-recheck.json`; local only)
are follow-up reports outside its original seal and archive receipt. The
verification accounting reproduction (`accounting-verify-reproduced.json`; local only) and
scripted-prefix accounting audit (`accounting-scripted-prefixes.json`; local only) preserve
the reusable audit helper's checks; the scripted report is synthetic evidence.
The restart archive receipt inventories these added reports and helper copies
without changing either earlier archive receipt.

Exact copies of the read-only post-run helpers remain outside the frozen
harness and study seals:

- [private_stage_diagnostics.py](diagnostics/private_stage_diagnostics.py): distinguish queued prompts from dispatched private work and candidate creation.
- [frozen_qualification.py](diagnostics/frozen_qualification.py): summarize recorded evaluator repeats, selector ties and regret.
- [audit_accounting.py](diagnostics/audit_accounting.py) and [test_audit_accounting.py](diagnostics/test_audit_accounting.py): recompute usage charges, dispatch reservations and shared-prefix attribution.
- [consolidate_calibration.py](diagnostics/consolidate_calibration.py): apply the frozen continuation-selection rule and retain first-valid-candidate observations.

These helpers read archived records without model calls or evaluator reruns;
write their reports outside extracted study directories. The exact accounting
tests retain their original fixture-relative paths: they expect the continued
and verification study directories plus `accounting-verify.json` beside the
test script, and the `shared-record-policy-paid-preflight-2026-10-12` scripted
study one directory above. In the retained original output layout, run
`python3 -m unittest discover -s output/shared-record-night-2026-10-12 -p test_audit_accounting.py`
from the repository root. The copied test source can be run after restoring
that same layout in a disposable directory. All original output-only helpers
remain unchanged.

## Group-delivery diagnostic verification

- [Delivery auditor](diagnostics/audit_delivery.py) and [13 offline tests](diagnostics/test_audit_delivery.py)
- [Development qualification helper](diagnostics/development_qualification.py)
- Scripted delivery audit (`delivery-scripted.json`; local only), structural qualification replay (`development-qualification-structural-replay.json`; local only), and exact-copy receipt (`diagnostics-verification-receipt.json`; local only)

These are post-run helpers outside the frozen harness. The delivery audit
passed on the completed scripted archive: 24 record trials, 72 recipients,
208 publication receipts and four empty deliveries. It checks exact prefix
ancestry, recipient eligibility, E/D/F filtering, daemon receipt/view evidence,
and complete source bytes. The tests include resealed corruptions and separate
internal parent-authority fetches from source actually returned to a model.
Both synthetic reports bind to the same manifest and seal; the structural
qualification report predates attachment of the separate delivery audit.
They verify audit logic and report structure, not paid-study outcomes or
collaboration quality.

From this evidence directory, run the helpers against a completed extracted
study, writing fresh reports outside that study:

```sh
python3 diagnostics/audit_delivery.py /path/to/sealed-study --output /tmp/delivery.json
python3 diagnostics/development_qualification.py /path/to/sealed-study --delivery-audit /tmp/delivery.json --output /tmp/qualification.json
```

The delivery auditor refuses unsealed studies. It distinguishes delivered
metadata, actual request submission, full-source tool returns, later source
submission and internal parent validation. None alone proves semantic use or
benefit. The delivery tests retain the original fixture layout, with
`shared-record-policy-paid-preflight-2026-10-12` one directory above the scripts;
they run in the retained output layout with
`python3 -m unittest discover -s output/shared-record-night-2026-10-12 -p test_audit_delivery.py`.
No live study was read or changed to preserve this diagnostic evidence.

## Sealed policy-development live audits

- Accounting audit (`accounting-development.json`; local only) and settled campaign snapshot (`campaign-after-development.json`; local only)
- Delivery audit (`delivery-development.json`; local only)
- Sealed model-label audit (`model-labels-development-sealed.json.gz`; local only)
- Exact-copy receipt, raw/compressed hashes and shared source identities (`development-audits-verification-receipt.json`; local only)

The completed generation archive contains 14 trials and 447 settled requests.
Accounting passed with $1.402139390 in provider-usage charges, no unknown
reservations and no failures. All 447 generation input counts matched usage;
456 completed count requests include nine count-only stops where the remaining
allowance could not fund the provider minimum of 16 output tokens.

Delivery passed across 12 record trials, 36 recipients and 336 publication
receipts. Peer metadata reached submitted model requests for 17 recipients;
12 recipients received full peer-source tool returns, and 11 submitted those
sources in later model requests. The remaining recipient, max-cut `P_E`
`agent-0`, fetched peer source and then stopped on budget before another
request. Internal parent-authority fetches remain distinct from source returns.
There were two empty deliveries.

The sealed label audit found the expected exposed execution labels in 371 of
447 response-confirmed model payloads, with 12,664 payload occurrences. It
reported no integrity errors, warnings or changing source files. The reports
agree on the exact manifest, seal and overlapping source hashes. The sealed
audit removes the earlier partial-snapshot limitation; the original partial
report remains preserved below. Frozen development data were not rewritten.

The label report preserves 12,108,111 raw bytes in a 508,787-byte deterministic
gzip file (`mtime=0`); the other three reports are exact uncompressed copies.
This preservation step covers completed generation audits, before the full
study archive and hidden-scoring results. It makes no model-quality or
collaboration-benefit claim.

## Completed policy-development archive

- Sealed study archive (`policy-development-sealed.tar.xz`; local only) and archive receipt with all member hashes (`policy-development-sealed-receipt.json`; local only)
- Exact analysis copy (`analysis-development.json`; local only), completed qualification report (`qualification-development.json`; local only), and copy-verification receipt (`qualification-development-copy-receipt.json`; local only)
- [Streaming archive wrapper](diagnostics/package_completed_study.py) and [10 offline tests](diagnostics/test_package_completed_study.py)

The archive preserves the 500 exact files selected by the frozen
`analyze.package`: 22 sealed live-evidence files, 473 hidden-journal files,
the manifest, both seals, completed hidden scores and existing analysis.
Its 731,330,400 payload bytes compress to 24,245,692 bytes with SHA-256
`fad77076fa75e278d14492ddbca6a8c7594fde7e5bb6a6c205f4748b2ab3974a`.
The deterministic XZ archive uses preset 6 and CRC64 over sorted USTAR members
with mode `0644`, zero ownership and timestamps. Independent regeneration
produced identical compressed and uncompressed archive hashes.

Every extracted file matched its source bytes, both seals verified, and the
original study remained unchanged. Before the primary opaque-ID repair, all
35 frozen source files matched both the checkout and recorded commit
`6ec9c62963f8ee8066daa7f70de001aa71513bc6`; the frozen binary hash also matched.
The binary and private daemon state are omitted from the archive. The preserved
wrapper passed all ten tests and made no evaluator or model calls. Packaging
copied the existing analysis without recomputing it or editing frozen data.

From this evidence directory,
`tar -xJf policy-development-sealed.tar.xz -C /path/to/empty-directory`
restores the `policy-development/` study directory. The separate analysis copy
matches its archived member exactly; qualification and the other audits remain
outside the study seals. The [execution report](../../shared-record-information-policy-nightly-2026-10-12.md)
records the result interpretation and decision.

## Development readiness and scored-phase stop

- [Archive-only qualification review and scored-phase stop](qualification-development-readiness.md)
- Source and endpoint readiness report (`qualification-development-readiness.json`; local only)
- [Read-only readiness report builder](diagnostics/development_readiness.py)
- Proposed evaluator-noise diagnostic, not executed (`proposed-evaluator-noise-diagnostic.json`; local only)

The current paid pilot stops before scored allocation, freeze or calls because
evaluator-noise and selector readiness remain unestablished. These reports
preserve the frozen development outcomes. The proposed additional executions
are unexecuted design work; no candidate was rerun and no score was replaced.

## Opaque candidate-ID repair and model-label audits

- [Isolated implementation test results and limits](opaque-id-test-results.md)
- Sealed scripted replay archive (`opaque-id-replay-sealed.tar.xz`; local only), archive verification receipt and member hashes (`opaque-id-replay-sealed-receipt.json`; local only), and replay verification (`opaque-id-replay-verification.json`; local only)
- Independent opaque-ID replay label audit (`opaque-id-model-label-audit.json`; local only)
- [Model-label auditor](diagnostics/audit_model_labels.py), [auditor tests](diagnostics/test_audit_model_labels.py), and audit verification receipt (`model-label-audit-verification-receipt.json`; local only)
- Old scripted positive control (`model-labels-scripted-positive.json.gz`; local only) and partial development positive control (`model-labels-development-partial.json.gz`; local only)

Implementation commit `1eb43fc60cdc09ba2ee0df38177284045ceed708` was verified in
an isolated worktree before any application to the primary checkout. Its full
offline suite passed 128 tests. The completed real-daemon scripted replay
contains 148 opaque candidate handles and 306 generation payloads across 36
trials. Its separate label audit passed, including all 304 response-confirmed
payloads and the two deliberately failed attempts. Worker attribution and the
pair-shared policy field remain visible. These checks establish the declared
execution-label control, not complete treatment blinding or model quality.

The 194,916-byte deterministic XZ archive preserves 73 exact packaged files
and both seals. Its SHA-256 is
`df51584153436e7424fda8287b8dd5dd3a1d75e2f0e1336dc8aeb595e43aabc9`.
Extraction, per-file byte comparisons, both seals and the scripted
verification were checked without provider calls or evaluator reruns. The
receipt matches the frozen source hashes to the implementation commit; the
manifest itself records the pre-commit HEAD used during verification.

The preserved auditor passed all 18 tests. The old scripted archive and the
partial live development snapshot are positive controls containing exposed
execution labels; the opaque replay is the passing negative control. The
partial snapshot changed during reading and is diagnostic only. Its raw input
snapshot was not separately preserved; retain the report's file/payload hashes
and see the completed sealed audit above for the final snapshot. Both positive
reports are lossless deterministic gzip copies (`mtime=0`), with exact original
and compressed hashes in the audit receipt.

From this evidence directory:

```sh
python3 diagnostics/audit_model_labels.py /path/to/sealed-study --output /tmp/labels.json
python3 -m unittest discover -s diagnostics -p test_audit_model_labels.py
```

The audit CLI exits 1 when it detects labels. Reading an unfinished study
requires `--allow-partial`; such a result never passes qualification. The
audit does not establish inferred treatment, attention, semantic use or
benefit. Existing paid studies retain their original frozen IDs and outcomes.

## Prospective scored-pilot review

The [prospective readiness review](prospective-scored-readiness.md) preserves
the completed read-only review written while policy development was
still running. It records the gates pending at that point and does not inspect
current development hidden outcomes or approve a scored launch. Its repair
status is historical; the separate isolated evidence above records later
completion of the opaque-ID change. At review time, remaining gates required
sealed development results, audits, funding and a concrete scored freeze.

## Host suspension during hidden scoring

The single-source host-pause diagnostic (`hidden-host-pause-diagnostic.json`; local only)
records the 03:44:10–07:41:23 PDT low-power sleep and wake on AC. The affected
max-cut delayed `agent-2/c4` source completed its ten hidden executions with
no errors: all were valid, CPU ranged from 1.217208 to 1.632349 seconds, and
both repeats returned identical cost and solution hash on each instance.
It was selected in neither C_D nor P_D at B, B/3 or 2B/3, nor as that worker's
prefix/final best, so this record cannot change the frozen D/F endpoint rule.

The 14,245.881526-second journal-file interval spans the full ten-execution
batch. Per-instance wall timestamps were not recorded, so the archive cannot
locate the suspended evaluation or distinguish a pause within a subprocess
from a pause between evaluations. The report preserves exact journal, source,
trial and selector proofs plus only the two relevant power-log lines. It does
not prove the counterfactual result without suspension or a 40-second elapsed
UTC bound across hibernation. No evaluator was rerun and no result replaced.
Full hidden qualification remained pending when this diagnostic was written.

Full live outputs remain local under `output/shared-record-night-2026-10-12`.
The campaign is complete at the readiness stop. No generation result is claimed
by a prelaunch manifest. No new executions were made during consolidation.
