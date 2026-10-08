# Evidence-investigation task calibration

This is the small diagnostic for the
[evidence collaboration study design](../../evidence-collaboration-study-design-2026-10-08.md).
It does not implement or estimate the four-arm collaboration experiment.

Data: **MuSiQue: Multi-hop Questions via Single-hop Question Composition**, Harsh
Trivedi, Niranjan Balasubramanian, Tushar Khot and Ashish Sabharwal, TACL 2022.
[Authors' repository](https://github.com/StonyBrookNLP/musique);
[data license: CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
Our frozen subset and conversion are provided with the same attribution. Model
outputs and local analysis are additional experiment records.

The official archive download was slow. Calibration used the first 4,440 complete
JSONL rows recovered from the partially downloaded `musique_full_v1.0_train.jsonl`
ZIP entry. This is a convenience development pool, not a random sample of the
whole dataset or a verified complete archive. No full-archive checksum is claimed.
The frozen evidence retains every selected source row, exact model input,
request hash, visible response, usage and summary. No API secrets or hidden
reasoning are retained. Selection uses hashed IDs within the pool, six complete
three-hop pairs and six four-hop pairs. Future main data must be fully downloaded
and validated before selection.

The first run used low reasoning, a 4,096 output-token cap and the prompt in
`calibrate.py`. After seeing excessive abstention, the second run reused the same
12 development families with an explicit chain-evidence prompt, high reasoning,
and an 8,192 cap. Its exact changed prompt is retained in its manifest. These
changes are confounded; no causal claim about reasoning level alone is warranted.
No-document calls were omitted in the second run. Neither run is held-out evidence
for a chosen method. The $10 ceilings apply separately to each manifest and are
maximum reservations, not actual spending.

## Reproduce

Run from the repository root. Preparation consumes complete MuSiQue-Full JSONL
rows and needs at least six complete question pairs of each hop stratum:

```sh
python3 -m unittest discover -s research/experiments/evidence_calibration -v
python3 research/experiments/evidence_calibration/calibrate.py prepare output/evidence-check --source path/to/musique_full_v1.0_train.jsonl
python3 research/experiments/evidence_calibration/calibrate.py run output/evidence-check
python3 research/experiments/evidence_calibration/calibrate.py analyze output/evidence-check
```

Only `run` uses the network and requires `OPENAI_API_KEY`. The runner reuses the
[previous pilot's bounded API transport](../luna_decision_pilot/run.py), including
pre-dispatch reservations, no automatic retries and stopping after transport
failure. It refuses to resume when existing recorded request hashes differ from
the supplied manifest. It does not enforce every proposed main-study guardrail.
Do not present this calibration runner as the confirmatory experiment runner.

For exact saved-run analysis, use the offline evidence verifier described below.
No repeat API calls are needed. Costs use the transport's conservative rate of
$0.125 per million input tokens and $0.50 per million output tokens, without
claiming cache-read discounts; these are estimated accounting totals, not invoices.

## Recorded results

[Frozen evidence](../../evidence/evidence-calibration-2026-10-08.json) contains both
runs. Recompute scores, validate delivered-input boundaries and request hashes,
and check usage accounting without network access:

```sh
python3 research/experiments/evidence_calibration/verify_evidence.py
```

| Diagnostic | Initial low-effort prompt | Revised high-effort prompt |
| --- | --- | --- |
| Planned / valid responses | 36 / 36 | 24 / 23 |
| Complete evidence: answer exact match | 2 / 12 | 6 / 12 |
| Complete evidence: answer plus exact support | 1 / 12 | 4 / 12 |
| Missing evidence: correct abstention | 12 / 12 | 11 / 12 |
| No documents: abstention | 12 / 12 | Not repeated |
| Strict paired family success | 1 / 12 | 3 / 12 |
| Conservative accounted dollars | $0.0103755 | $0.0277890 |

The revised run had one transport timeout on an insufficient-evidence case. It
counts as a failure, was not retried, and retains its $0.006166875 reservation in
the cost total because actual provider usage is unknown. The two runs therefore
made 60 attempts with 59 valid responses and $0.0381645 conservatively accounted.
This is not a final invoice. The exact prompts changed along with reasoning
setting and token cap; the difference is not an isolated treatment effect.

The initial prompt abstained on eight answerable cases. The revised prompt has
useful answer-level headroom, but strict paired family success remains only 25%
including the timeout. **The large study is not cleared to launch.** The next
40-family development phase must qualify the stronger solo policy, scoring and
stage reliability before any confirmatory run. This is a concrete calibration
finding, not evidence that teams will help.

The sample is small and contains reused component questions. Treat all counts as
descriptive; do not infer statistical significance, domain generalization or
training-data cleanliness. Exact-match scoring can also reject semantically
reasonable variants: the frozen evaluator is intentionally not an LLM judge.
The main study's blinded alternative-support/answer review remains necessary.
