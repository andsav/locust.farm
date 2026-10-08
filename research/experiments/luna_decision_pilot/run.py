"""Run the frozen study with bounded direct API calls and no model tools."""

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
import fcntl
import json
import os
from pathlib import Path
import random
import threading
import time
import urllib.error
import urllib.request

from study import ARMS, STYLES, analyze, design, digest


INPUT_RATE = .125  # Conservative cache-write rate; no cache discount claimed.
OUTPUT_RATE = .5


def now():
    return datetime.now(timezone.utc).isoformat()


class Runner:
    def __init__(self, folder):
        self.folder = Path(folder)
        self.frozen = json.loads((self.folder / "design.json").read_text())
        if self.frozen != design():
            raise ValueError("Frozen design differs from current code; do not silently resume")
        self.records_path = self.folder / "records.json"
        self.records = json.loads(self.records_path.read_text()) if self.records_path.exists() else []
        self.lock = threading.Lock()
        self.stop = threading.Event()

    def save(self):
        temporary = self.folder / "records.tmp"
        temporary.write_text(json.dumps(self.records, indent=2) + "\n")
        temporary.replace(self.records_path)

    def invoke(self, label, instructions, public, **metadata):
        body = dict(model=self.frozen["model"], instructions=instructions,
                    input=json.dumps(public, sort_keys=True), reasoning={"effort": self.frozen["effort"]},
                    max_output_tokens=self.frozen["max_output_tokens"], service_tier="default", store=False)
        encoded = json.dumps(body).encode()
        # All inputs are short ASCII text, with no tools or retrieved content.
        # Bytes plus a 4096-token overhead is conservative, not a token-counter measurement.
        input_bound = len(encoded) + 4096
        reserve = (input_bound * INPUT_RATE + body["max_output_tokens"] * OUTPUT_RATE) / 1e6
        with self.lock:
            if self.stop.is_set():
                return None
            if any(r["label"] == label for r in self.records):
                return None  # Never retry an ambiguous or failed paid attempt.
            spent = sum(r.get("cost_usd", r["reserved_usd"]) for r in self.records)
            if spent + reserve > self.frozen["ceiling_usd"]:
                self.stop.set()
                return None
            item = dict(label=label, started_at=now(), status="pending", reserved_usd=reserve,
                        input_bound=input_bound, request_sha256=digest(body), **metadata)
            self.records.append(item)
            self.save()  # Persist reservation before network activity.
        started = time.monotonic()
        try:
            request = urllib.request.Request("https://api.openai.com/v1/responses", data=encoded,
                headers={"Content-Type": "application/json", "Authorization": "Bearer " + os.environ["OPENAI_API_KEY"]})
            with urllib.request.urlopen(request, timeout=180) as response:
                value = json.load(response)
            usage = value["usage"]
            incoming, outgoing = usage["input_tokens"], usage["output_tokens"]
            if type(incoming) is not int or type(outgoing) is not int or min(incoming, outgoing) < 0:
                raise ValueError("Invalid provider usage")
            cost = (incoming * INPUT_RATE + outgoing * OUTPUT_RATE) / 1e6
            text = "\n".join(c["text"] for o in value.get("output", []) if o.get("type") == "message"
                             for c in o.get("content", []) if c.get("type") == "output_text")
            with self.lock:
                item.update(status=value.get("status", "unknown"), text=text, usage=usage, cost_usd=cost,
                            response_id=value.get("id"), returned_model=value.get("model"),
                            service_tier=value.get("service_tier"), incomplete_details=value.get("incomplete_details"))
                if incoming > input_bound or outgoing > body["max_output_tokens"] or cost > reserve:
                    self.stop.set()
                    item["budget_error"] = "Provider usage exceeded reservation; further dispatch stopped"
                if value.get("service_tier") not in (None, "default"):
                    self.stop.set()
                    item["budget_error"] = "Unexpected tier; further dispatch stopped"
        except Exception as error:
            # Do not persist arbitrary provider bodies, credentials, or hidden reasoning.
            with self.lock:
                item.update(status="transport_error", error_type=type(error).__name__)
                if isinstance(error, urllib.error.HTTPError):
                    item["http_status"] = error.code
                self.stop.set()  # Inspect first transport failure instead of repeating it 240 times.
        finally:
            with self.lock:
                item.update(finished_at=now(), elapsed_seconds=time.monotonic() - started)
                self.save()
        return item

    def run(self):
        smoke = self.invoke("smoke", "Return only the JSON object {\"ok\":true}.", {"check": "connectivity"}, kind="smoke")
        smoke = smoke or next(r for r in self.records if r["label"] == "smoke")
        if smoke["status"] != "completed" or json.loads(smoke.get("text", "null")) != {"ok": True}:
            raise RuntimeError("Smoke did not complete; scored requests not started")
        jobs = [(case, arm, style) for case in self.frozen["cases"] for arm in ARMS for style in STYLES]
        random.Random(self.frozen["order_seed"]).shuffle(jobs)
        with ThreadPoolExecutor(max_workers=4) as executor:
            futures = [executor.submit(self.invoke, f"{c['case_id']}/{a}/{s}", self.frozen["prompts"][a][s],
                        c["public_case"], kind="scored", case_id=c["case_id"], arm=a, style=s) for c, a, s in jobs]
            for i, future in enumerate(as_completed(futures), 1):
                future.result()
                if i % 20 == 0:
                    with self.lock:
                        completed = sum(r["status"] == "completed" for r in self.records)
                        print(json.dumps(dict(checked_jobs=i, completed_requests=completed,
                            accounted_usd=sum(r.get("cost_usd", r["reserved_usd"]) for r in self.records))), flush=True)
        summary = analyze(self.records, self.frozen)
        (self.folder / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
        print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["prepare", "run", "analyze"])
    parser.add_argument("folder", type=Path)
    args = parser.parse_args()
    if args.command == "prepare":
        args.folder.mkdir(parents=True, exist_ok=False)
        frozen = design()
        (args.folder / "design.json").write_text(json.dumps(frozen, indent=2) + "\n")
        print(json.dumps(dict(design_sha256=digest(frozen), cases=len(frozen["cases"]), scored_requests=240)))
    elif args.command == "run":
        with (args.folder / "run.lock").open("w") as lockfile:
            fcntl.flock(lockfile, fcntl.LOCK_EX | fcntl.LOCK_NB)
            Runner(args.folder).run()
    else:
        runner = Runner(args.folder)
        print(json.dumps(analyze(runner.records, runner.frozen), indent=2))
