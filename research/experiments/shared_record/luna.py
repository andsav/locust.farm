"""Direct, metered Responses API calls to one model. No SDK, no automatic retry."""
import json
import math
import os
import threading
import time
import urllib.error
import urllib.request
from functools import wraps
from pathlib import Path

MODEL = 'gpt-6-luna'
# USD per million tokens. Conservative: every input token at the uncached rate,
# as on the official model page read for the 8 October decision pilot.
RATES = {'input': 0.125, 'output': 0.50}
MAX_OUTPUT_TOKENS = 24000
MIN_OUTPUT_TOKENS = 2000
TIMEOUT_SECONDS = 600


class BudgetStop(Exception):
    pass


class TransportFailure(Exception):
    pass


def locked(method):
    @wraps(method)
    def invoke(self, *args, **kwargs):
        with self.lock:
            return method(self, *args, **kwargs)
    return invoke


class Ledger:
    """Worst-case charge is persisted before every billable request."""

    def __init__(self, path, ceiling):
        self.lock = threading.RLock()
        self.path = Path(path)
        self.ceiling = ceiling
        self.entries = json.loads(self.path.read_text()) if self.path.exists() else []
        self.failures = 0

    @locked
    def save(self):
        self.path.parent.mkdir(parents=True, exist_ok=True)
        temp = self.path.with_suffix('.tmp')
        temp.write_text(json.dumps(self.entries, indent=1) + '\n')
        temp.replace(self.path)

    @locked
    def spent(self, label=None, prefix=None):
        total = 0.0
        for entry in self.entries:
            if label is not None and entry['label'] != label:
                continue
            if prefix is not None and not entry['label'].startswith(prefix):
                continue
            total += entry.get('cost', entry['reserved'])
        return total

    @locked
    def reserve(self, label, phase, amount, phase_ceiling):
        if amount < 0 or self.spent() + amount > self.ceiling + 1e-9:
            raise BudgetStop('study ceiling')
        if self.spent(prefix=phase + '/') + amount > phase_ceiling + 1e-9:
            raise BudgetStop('phase ceiling')
        item = dict(label=label, model=MODEL, reserved=amount, status='pending', reserved_at=time.time())
        self.entries.append(item)
        self.save()
        return item

    @locked
    def settle(self, item, response):
        usage = response['usage']
        incoming, outgoing = usage['input_tokens'], usage['output_tokens']
        if type(incoming) is not int or type(outgoing) is not int or min(incoming, outgoing) < 0:
            raise RuntimeError('invalid provider usage')
        cost = (incoming * RATES['input'] + outgoing * RATES['output']) / 1e6
        item.update(status='complete', cost=cost, usage=usage, settled_at=time.time(),
                    response_id=response.get('id'), returned_model=response.get('model'),
                    response_status=response.get('status'))
        self.save()
        if cost > item['reserved'] + 1e-9:
            raise RuntimeError('provider usage exceeded the reservation; stop the study')
        if response.get('model') != MODEL:
            raise RuntimeError('provider returned an unexpected model; stop the study')

    @locked
    def fail(self, item, error):
        # The full reservation stays charged: the provider outcome is unknown.
        item.update(status='failed', error=error, settled_at=time.time())
        self.failures += 1
        self.save()


def post(endpoint, body, opener=None):
    headers = {'Authorization': 'Bearer ' + os.environ['OPENAI_API_KEY'], 'Content-Type': 'application/json'}
    request = urllib.request.Request('https://api.openai.com/v1/' + endpoint, json.dumps(body).encode(), headers)
    try:
        with (opener or urllib.request.urlopen)(request, timeout=TIMEOUT_SECONDS) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        try:
            message = str(json.load(error).get('error', {}).get('message', ''))[:300]
        except (ValueError, AttributeError):
            message = 'unreadable provider error'
        message = message.replace(os.environ.get('OPENAI_API_KEY', '\0'), '<redacted>')
        raise TransportFailure(f'HTTP {error.code}: {message}') from None
    except (urllib.error.URLError, TimeoutError, OSError) as error:
        raise TransportFailure(type(error).__name__) from None


def input_token_bound(payload):
    # UTF-8 bytes at two bytes per token overestimate code and JSON; plus overhead.
    return math.ceil(len(json.dumps(payload).encode()) / 2) + 2048


class Session:
    def __init__(self, instructions, tools, ledger, label, phase, phase_ceiling, allowance, effort='high', opener=None):
        self.instructions, self.tools, self.ledger = instructions, tools, ledger
        self.label, self.phase, self.phase_ceiling, self.allowance = label, phase, phase_ceiling, allowance
        self.effort, self.opener = effort, opener
        self.history = []
        self.requests = []

    def user(self, text):
        self.history.append({'role': 'user', 'content': text})

    def spent(self):
        return self.ledger.spent(self.label)

    def payload(self):
        definitions = [dict(type='function', name=t['name'], description=t['description'],
                            parameters=t['input_schema'], strict=True) for t in self.tools]
        return dict(model=MODEL, instructions=self.instructions, input=self.history, tools=definitions)

    def request(self):
        payload = self.payload()
        bound = input_token_bound(payload)
        reserve_input = bound * RATES['input'] / 1e6
        remaining = self.allowance - self.spent()
        maximum = min(MAX_OUTPUT_TOKENS, math.floor((remaining - reserve_input) * 1e6 / RATES['output']))
        if maximum < MIN_OUTPUT_TOKENS:
            raise BudgetStop('agent allowance')
        reserved = reserve_input + maximum * RATES['output'] / 1e6
        payload.update(max_output_tokens=maximum, reasoning={'effort': self.effort}, store=False,
                       include=['reasoning.encrypted_content'], service_tier='default')
        item = self.ledger.reserve(self.label, self.phase, reserved, self.phase_ceiling)
        item.update(max_output_tokens=maximum, input_token_bound=bound)
        self.ledger.save()
        started = time.time()
        try:
            response = post('responses', payload, self.opener)
        except TransportFailure as error:
            self.ledger.fail(item, str(error))
            raise
        self.ledger.settle(item, response)
        calls, visible, retained = [], [], []
        for value in response['output']:
            kind = value.get('type')
            if kind == 'function_call':
                try:
                    arguments = json.loads(value['arguments'])
                except json.JSONDecodeError:
                    visible.append('[incomplete tool call discarded: ' + value.get('name', '?') + ']')
                    continue  # Never replay a cut-off call into the history.
                calls.append((value['call_id'], value['name'], arguments))
            elif kind == 'message':
                visible.extend(c.get('text', '') for c in value.get('content', []) if c.get('type') == 'output_text')
            retained.append(value)
        self.history.extend(retained)
        self.requests.append(dict(response_id=response.get('id'), status=response.get('status'),
                                  usage=response['usage'], cost=item['cost'], elapsed=time.time() - started,
                                  incomplete=response.get('incomplete_details')))
        return calls, visible, response.get('status')

    def results(self, results):
        self.history.extend(dict(type='function_call_output', call_id=ident, output=json.dumps(value))
                            for ident, value in results)
