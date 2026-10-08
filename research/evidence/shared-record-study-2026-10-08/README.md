# Shared-record study: run records

Status: verified run records, 8 October 2026, of the amended study
(calibration 2 and the scored phase). Protocol, amendments and every
deviation: [shared_record/README.md](../../experiments/shared_record/README.md).
Results and interpretation:
[results note](../../shared-record-study-results-2026-10-08.md). The first,
saturated calibration is in
[shared-record-calibration-1-2026-10-08](../shared-record-calibration-1-2026-10-08/README.md).

Total ledger $6.5254 (calibration $1.2294, scored $5.2960) over 1,805
entries, 15 failed requests kept at their full reservation. Study total with
calibration 1: $7.22.

Files, written by `analyze.py --evidence` from `output/shared-record-2026-10-08b`.
Large files are gzipped (`gzip -d`, or `gzip.open` in Python); small ones are
verbatim.

- `manifest.json`: configuration with the amendment tag, source hashes of the
  last launch, daemon binary hash, a SHA-256 digest of each family's frozen
  public and hidden instance lists (regenerate from the study seed and
  `problems.py`), reference costs, and total spend.
- `ledger.json`: every API request's reservation, settlement and cost, across
  all four launches of the output directory, including voided runs.
- `calibration-gate.json`: the verdict under Amendment A2 (passed) with facts.
  `calibration-gate-preregistered-rule.json`: the verdict under the
  preregistered rule (failed on vertex-cover spread).
- `trials/calibration/*.json.gz` and `trials/scored/*.json.gz`: the final
  trials. Each holds its agents' full visible conversations, every candidate's
  source with public and hidden scores and timestamps, requests, reads, posts,
  goal IDs, principals and receipts, and the trial's `started`/`finished`
  times. The `spent_usd` fields of the rerun trials (`calibration/vertex_cover`,
  `scored/tsp`, `scored/qap`) include spend inherited from the voided runs;
  `analysis.json` splits it out.
- `voided/calibration-attempt-2-network-outage/`: the vertex-cover trials,
  gate, manifest, ledger snapshot and daemon log of the attempt lost to the
  11:50 outage.
- `voided/scored-outage-1352/`: the six `tsp` and `qap` trials voided after
  the 13:52 outage, with that launch's manifest. Never analyzed as results;
  used only in the sensitivity analysis.
- `daemon-events-launch-1.jsonl.gz` (calibration attempt 2: the maxcut shared
  trial; identical to the copy under `voided/`), `daemon-events-launch-3.jsonl.gz`
  (coloring and the voided tsp and qap shared trials) and `daemon-events.jsonl.gz`
  (launch 4: unrelated_machines, tsp, qap, mkp and setcover shared trials):
  the Locust daemon's operation logs. Each launch copies its daemon log over
  `daemon-events.jsonl` when it ends; launch 2 ran only the vertex-cover
  rerun, which has no shared arm, so its overwritten log held no record
  operations. Credential file paths appear; the files were in a temporary
  directory and are gone.
- `analysis.json`: `analyze.py --exclude tsp` output: per-trial summaries with
  own and inherited spend, record diagnostics, paired contrasts over all six
  tasks and without `tsp`, matched-spend tables.
- `analysis-preregistered-treatment.json`: the same analysis over a directory
  assembled from this evidence with the voided `tsp` and `qap` trials in place
  of the reruns (sensitivity analysis; no `--exclude`).

No API key, daemon secret or hidden reasoning is in these files.
