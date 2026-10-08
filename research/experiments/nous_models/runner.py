"""Bounded two-provider execution with a durable attempt journal and retries."""
import argparse
from concurrent.futures import ThreadPoolExecutor
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

from study import (STAGES, TEAMS, analyze, code_hashes, digest, label, nt, planned_labels,
                   prepare, read_attempts, request_body, selected, verify_requests)


def now():
    return datetime.now(timezone.utc).isoformat()


def cost(provider, usage, settings):
    incoming, outgoing = usage['input_tokens'], usage['output_tokens']
    cache_write = usage.get('cache_creation_input_tokens', 0)
    cache_read = usage.get('cache_read_input_tokens', 0)
    if any(type(v) is not int or v < 0 for v in (incoming, outgoing, cache_write, cache_read)):
        raise ValueError('Invalid provider token counts')
    if provider == 'luna':
        return (incoming*settings['input_rate']+outgoing*settings['output_rate'])/1e6, incoming
    creation = usage.get('cache_creation') or {}
    hour = creation.get('ephemeral_1h_input_tokens', 0)
    if type(hour) is not int or not 0 <= hour <= cache_write:
        raise ValueError('Invalid cache usage')
    return (incoming*settings['input_rate']+(cache_write-hour)*2.5+hour*4+
            cache_read*.1+outgoing*settings['output_rate'])/1e6, incoming+cache_write+cache_read


class Run:
    def __init__(self, folder):
        self.folder = folder
        self.manifest = json.loads((folder/'manifest.json').read_text())
        if digest(self.manifest) != (folder/'manifest.sha256').read_text().strip():
            raise ValueError('Manifest hash mismatch')
        if self.manifest['code_hashes'] != code_hashes():
            raise ValueError('Generation code changed')
        self.attempts = read_attempts(folder/'attempts.jsonl')
        self.lock = threading.Lock()
        self.stop = threading.Event()
        self.last_progress = 0
        self.spent = self.manifest.get('carry_forward_accounted_usd', 0)+sum(r.get('cost_usd', r['reserved_usd']) for r in self.attempts if not r.get('imported'))
        verify_requests(self.manifest, self.attempts)
        if any(r['status'] == 'pending' or r.get('budget_error') for r in self.attempts):
            raise ValueError('Ambiguous pending or accounting-error attempt needs inspection')

    def save(self, record):
        with (self.folder/'attempts.jsonl').open('a') as stream:
            stream.write(json.dumps(record, allow_nan=False)+'\n')
            stream.flush()
            os.fsync(stream.fileno())

    def invoke(self, case, profile, population, stage, agent):
        name = label(case['id'], profile, population, stage, agent)
        for _ in range(self.manifest['maximum_attempts']):
            if self.stop.is_set() or (self.folder/'stop-requested').exists():
                return
            with self.lock:
                records = selected(self.attempts)
                previous = records.get(name)
                count = sum(r['label'] == name for r in self.attempts)
            if previous and previous['status'] != 'transport_error':
                return
            if count >= self.manifest['maximum_attempts']:
                return
            if count and self.stop.wait(self.manifest['retry_delays_seconds'][count-1]):
                return
            provider, body = request_body(self.manifest, records, case, profile, population, stage, agent)
            settings = self.manifest['models'][provider]
            encoded = json.dumps(body).encode()
            input_bound = len(encoded)+4096
            if input_bound > 200000:
                self.stop.set()
                raise ValueError('Input exceeds conservative price bound')
            reserve = (input_bound*settings['reservation_input_rate']+settings['max_tokens']*settings['output_rate'])/1e6
            with self.lock:
                if self.stop.is_set():
                    return
                if self.spent+reserve > self.manifest['new_spend_ceiling_usd']:
                    self.stop.set()
                    return
                record = dict(label=name, attempt_id=name+f'#attempt{count+1}', provider=provider,
                              status='pending', started_at=now(), reserved_usd=reserve,
                              request_sha256=digest(body), input_bound=input_bound)
                self.attempts.append(record)
                self.spent += reserve
                self.save(record)
            start = time.monotonic()
            try:
                if provider == 'luna':
                    url = 'https://api.openai.com/v1/responses'
                    headers = {'Authorization': 'Bearer '+os.environ['OPENAI_API_KEY']}
                else:
                    url = 'https://api.anthropic.com/v1/messages'
                    headers = {'x-api-key': os.environ['ANTHROPIC_API_KEY'], 'anthropic-version': '2023-06-01'}
                headers['Content-Type'] = 'application/json'
                request = urllib.request.Request(url, data=encoded, headers=headers)
                with urllib.request.urlopen(request, timeout=180) as response:
                    value = json.load(response)
                usage = value['usage']
                charge, incoming = cost(provider, usage, settings)
                if provider == 'luna':
                    status = value.get('status', 'unknown')
                    text = '\n'.join(c['text'] for o in value.get('output', []) if o.get('type') == 'message'
                                     for c in o.get('content', []) if c.get('type') == 'output_text')
                    tier = value.get('service_tier')
                    stop_reason = value.get('incomplete_details')
                else:
                    stop_reason = value.get('stop_reason')
                    status = 'completed' if stop_reason == 'end_turn' else 'incomplete'
                    text = '\n'.join(c['text'] for c in value.get('content', []) if c.get('type') == 'text')
                    tier = usage.get('service_tier')
                with self.lock:
                    record.update(status=status, text=text, usage=usage, cost_usd=charge,
                                  returned_model=value.get('model'), response_id=value.get('id'),
                                  stop_reason=stop_reason, service_tier=tier)
                    self.spent += charge-reserve
                    if (incoming > input_bound or usage['output_tokens'] > settings['max_tokens'] or charge > reserve
                            or tier not in (None, 'default', 'standard') or value.get('model') != settings['id']):
                        record['budget_error'] = 'Unexpected model, tier or usage bound'
                        self.stop.set()
            except Exception as error:
                with self.lock:
                    record.update(status='transport_error', error_type=type(error).__name__)
                    if isinstance(error, urllib.error.HTTPError):
                        record['http_status'] = error.code
                        if error.code in (400, 401, 403, 404):
                            self.stop.set()
                    elif isinstance(error, (KeyError, ValueError)):
                        self.stop.set()
            finally:
                with self.lock:
                    record.update(finished_at=now(), elapsed_seconds=time.monotonic()-start)
                    self.save(record)
                    completed = sum(r['status'] == 'completed' for r in selected(self.attempts).values())
                    if completed >= self.last_progress+25:
                        print(json.dumps({'completed_unique': completed, 'planned_unique': len(planned_labels(self.manifest)),
                                          'new_accounted_usd_with_reservations': self.spent}), flush=True)
                        self.last_progress = completed
            if record['status'] != 'transport_error':
                return

    def run_job(self, job):
        case, profile = job
        rng = random.Random(digest([case['id'], profile, 'model-team-order']))
        for stage in STAGES:
            populations = list(TEAMS if stage == 'exchange' else TEAMS[:2])
            rng.shuffle(populations)
            for population in populations:
                for agent in range(10):
                    self.invoke(case, profile, population, stage, agent)

    def run(self):
        jobs = [(c, p) for c in self.manifest['base']['cases'] for p in nt.CONDITIONS]
        random.Random(20261008).shuffle(jobs)
        with ThreadPoolExecutor(max_workers=self.manifest['max_concurrency']) as pool:
            list(pool.map(self.run_job, jobs))
        verify_requests(self.manifest, self.attempts)
        summary = analyze(self.manifest, self.attempts)
        (self.folder/'summary.json').write_text(json.dumps(summary, indent=2, allow_nan=False)+'\n')
        return {k: v for k, v in summary.items() if k not in ('groups', 'contrasts', 'rows')}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    p = sub.add_parser('prepare');p.add_argument('folder', type=Path)
    p.add_argument('--upstream', type=Path, required=True);p.add_argument('--baseline', type=Path, required=True)
    p.add_argument('--prior', type=Path)
    p = sub.add_parser('run');p.add_argument('folder', type=Path)
    args = parser.parse_args()
    if args.command == 'prepare':
        print(json.dumps(prepare(args.upstream, args.baseline, args.folder, prior=args.prior), indent=2))
    else:
        with (args.folder/'run.lock').open('w') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX|fcntl.LOCK_NB)
            print(json.dumps(Run(args.folder).run(), indent=2))


if __name__ == '__main__':
    main()
