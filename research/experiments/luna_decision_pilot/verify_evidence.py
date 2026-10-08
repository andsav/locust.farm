"""Verify a complete tracked pilot bundle offline; no API key or network needed."""

import argparse
import json
from pathlib import Path

from study import analyze, digest


def verify(bundle):
    frozen, records = bundle["design"], bundle["records"]
    if digest(frozen) != bundle["design_sha256"]:
        raise ValueError("Design hash mismatch")
    cases = {case["case_id"]: case for case in frozen["cases"]}
    expected = {f"{c}/{a}/{s}" for c in cases for a in frozen["arms"] for s in frozen["styles"]} | {"smoke"}
    labels = [record["label"] for record in records]
    if len(set(labels)) != len(labels) or set(labels) != expected:
        raise ValueError("Missing, duplicate, or unexpected attempt")
    for record in records:
        if record["label"] == "smoke":
            instructions = 'Return only the JSON object {"ok":true}.'
            public = {"check": "connectivity"}
        else:
            c, a, s = record["case_id"], record["arm"], record["style"]
            if record["label"] != f"{c}/{a}/{s}" or record["kind"] != "scored":
                raise ValueError("Attempt identity mismatch")
            instructions = frozen["prompts"][a][s]
            public = cases[c]["public_case"]
        request = dict(model=frozen["model"], instructions=instructions,
                       input=json.dumps(public, sort_keys=True), reasoning={"effort": frozen["effort"]},
                       max_output_tokens=frozen["max_output_tokens"], service_tier="default", store=False)
        if digest(request) != record["request_sha256"]:
            raise ValueError("Request hash mismatch")
        if record.get("returned_model", frozen["model"]) != frozen["model"]:
            raise ValueError("Unexpected returned model")
    calculated = analyze(records, frozen)
    if calculated != bundle["summary"]:
        raise ValueError("Saved analysis does not reproduce")
    if calculated["accounted_usd"] > frozen["ceiling_usd"]:
        raise ValueError("Budget exceeded")
    return dict(attempts=len(records), design_sha256=digest(frozen),
                accounted_usd=calculated["accounted_usd"], verification="passed")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(json.loads(args.bundle.read_text())), indent=2))
