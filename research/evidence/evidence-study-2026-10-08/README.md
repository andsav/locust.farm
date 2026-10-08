# Luna evidence-study records — October 8, 2026

Records for the [implemented study](../../experiments/evidence_collaboration/README.md).
The [complete frozen cohort](../evidence-study-cohort-2026-10-08.json.gz) contains
source rows, component exclusions and split IDs. MuSiQue data is CC BY 4.0;
its authors, source URL and attribution are retained in the execution protocol.

## Initial allocation diagnostic

- [Development v1 results and frozen source](development-v1-results.json)
- [Development v1 exact requests and visible responses](development-v1-records.jsonl.gz)

Only the first two development families were executed at 6,000/12,000 total
output-token caps per arm/case. Of 102 paid attempts, 83 completed with valid
records and 19 were truncated. This also blocked 50 dependent jobs. The other
2,888 planned jobs were never dispatched. Conservative accounted spending was
$0.07868475. No transport error occurred and no paid attempt was retried.

These two-family diagnostics are not 40-family performance estimates. The full
planned denominator remains explicit in the saved summary. The allocation was
discontinued because it prevented the evidence-exchange intervention from being
delivered, and a new development manifest raised allocations to 24,000/48,000.
See the [development amendment](../../experiments/evidence_collaboration/development-amendment.json).

Each records archive is UTF-8 JSONL compressed with gzip. The results JSON includes
its compressed and uncompressed SHA-256, exact manifest, frozen executable source,
summary and independent offline-verification result. Request headers, credentials
and hidden reasoning text are excluded. The records include public evidence and
visible model outputs; support annotations remain in the separate cohort/evaluator.

The exporter in [evidence.py](../../experiments/evidence_collaboration/evidence.py)
verifies each run with that run's frozen implementation before archiving it.
Record filenames/labels are experiment identifiers, not assertions that a case
passed. Missing, failed and truncated cases remain in their planned denominators.
