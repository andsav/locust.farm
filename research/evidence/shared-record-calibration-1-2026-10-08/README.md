# Shared-record study: first calibration (gate failed)

Status: verified run records, 8 October 2026. Calibration phase only; no
scored trial ran. Protocol and amendment:
[shared_record/README.md](../../experiments/shared_record/README.md).

The gate failed on criterion 3: `maxcut/independent` hidden range 0.0000 (all
three agents at 0.9352). Everything else passed: 0 transport failures, every
agent produced a fully valid candidate, the shared trial published 13 findings
with receipts and 7 reads returned peer findings, one candidate beat the
reference on hidden instances (best 0.9352). Vertex cover's independent range
of 0.0058 came from a public-score tie-break; every vertex-cover agent reached
hidden 0.9595. Total spend $0.6967 across 180 settled ledger entries.

| Task | Arm | Selected hidden | Public | Oracle hidden | Spent $ | Turns | Elapsed s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| maxcut | independent | 0.9352 | 0.9363 | 0.9352 | 0.258 | 58 | 619 |
| maxcut | shared | 0.9352 | 0.9363 | 0.9352 | 0.232 | 56 | 457 |
| vertex_cover | independent | 0.9595 | 0.9524 | 0.9595 | 0.184 | 54 | 589 |
| vertex_cover | solo | 0.9595 | 0.9524 | 0.9595 | 0.023 | 12 | 128 |

Files, copied unchanged from `output/shared-record-2026-10-08` by
`analyze.py --evidence`:

- `manifest.json`: configuration as committed in `e493d60`, source hashes,
  binary hash, the frozen instances (this run's manifest still embeds them)
  and reference costs.
- `ledger.json`: every API request's reservation, settlement and cost.
- `calibration-gate.json`: the gate verdict, reasons and facts.
- `trials/calibration/*.json`: each trial's agents, full visible
  conversations, candidates with public and hidden scores, reads, posts,
  goal IDs, principals and receipts.
- `daemon-events.jsonl`: the Locust daemon's operation log (paths to
  credential files appear; the files were in a temporary directory and are
  gone).
- `analysis.json`: `analyze.py` output for this run.
