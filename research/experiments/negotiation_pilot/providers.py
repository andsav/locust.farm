"""Direct, metered APIs. No SDK, native client, automatic retry or hosted tools."""
import json
import math
import os
import time
import threading
from functools import wraps
import urllib.error
import urllib.request
from pathlib import Path

MODELS = {'a': 'gpt-6-astra', 'b': 'claude-fable-5-1'}
# USD per million tokens, official standard pricing read 2026-10-07.
INPUT, OUTPUT = 10, 50


class BudgetStop(Exception):
    pass


def locked(method):
    @wraps(method)
    def invoke(self, *args, **kwargs):
        with self.lock:
            return method(self, *args, **kwargs)
    return invoke


class Ledger:
    def __init__(self, path, ceiling=50.0):
        self.lock = threading.RLock()
        self.path = Path(path)
        self.ceiling = ceiling
        self.entries = json.loads(self.path.read_text()) if self.path.exists() else []

    @locked
    def save(self):
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.write_text(json.dumps(self.entries, indent=2) + '\n')

    @locked
    def spent(self, label=None):
        return sum(e.get('cost', e['reserved']) for e in self.entries
                   if label is None or e['label'] == label)

    @locked
    def reserve(self, label, model, amount):
        if amount < 0 or self.spent() + amount > self.ceiling + 1e-9:
            raise BudgetStop('Global dollar ceiling')
        item = dict(label=label, model=model, reserved=amount, status='pending',
                    time=time.time())
        self.entries.append(item)
        self.save()  # Persist worst-case charge BEFORE a billable request.
        return item

    @locked
    def settle(self, item, response, provider):
        usage = response['usage']
        if provider == 'a':
            incoming, outgoing = usage['input_tokens'], usage['output_tokens']
            cached = usage.get('input_tokens_details', {}).get('cached_tokens', 0)
            # Conservatively price all uncached input at the cache-write rate.
            cost = ((incoming - cached) * 12.5 + cached + outgoing * OUTPUT) / 1e6
        else:
            cost = (usage['input_tokens'] * INPUT + usage['output_tokens'] * OUTPUT
                    + usage.get('cache_read_input_tokens', 0) * .25
                    + usage.get('cache_creation_input_tokens', 0) * 20) / 1e6
        item.update(status='complete', cost=cost, usage=usage,
                    response_id=response.get('id'), returned_model=response.get('model'))
        self.save()
        if cost > item['reserved'] + 1e-6:
            raise RuntimeError('Provider usage exceeded reserved charge; stop experiment')


def post(provider, endpoint, body):
    if provider == 'a':
        base = 'https://api.openai.com/v1/'
        headers = {'Authorization': 'Bearer ' + os.environ['OPENAI_API_KEY']}
    else:
        base = 'https://api.anthropic.com/v1/'
        headers = {'x-api-key': os.environ['ANTHROPIC_API_KEY'], 'anthropic-version': '2023-06-01'}
    headers['Content-Type'] = 'application/json'
    request = urllib.request.Request(base + endpoint, json.dumps(body).encode(), headers)
    try:
        with urllib.request.urlopen(request, timeout=600) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        # No retry: an ambiguous billable failure keeps its full reservation.
        try:
            detail = json.load(error).get('error', {})
            message = str(detail.get('message', ''))
            for key in ('OPENAI_API_KEY', 'ANTHROPIC_API_KEY'):
                if os.environ.get(key):
                    message = message.replace(os.environ[key], '<redacted>')
        except (ValueError, AttributeError):
            message = 'unreadable provider error'
        raise RuntimeError(f'Provider {provider} HTTP {error.code}: {message}') from None



class Session:
    def __init__(self, provider, instructions, tools, ledger, label):
        self.provider, self.instructions, self.tools = provider, instructions, tools
        self.ledger, self.label = ledger, label
        self.history = []

    def user(self, text):
        self.history.append({'role': 'user', 'content': text})

    def payload(self):
        if self.provider == 'a':
            definitions = [dict(type='function', name=t['name'], description=t['description'],
                                parameters=t['input_schema'], strict=True) for t in self.tools]
            return dict(model=MODELS['a'], instructions=self.instructions,
                        input=self.history, tools=definitions)
        return dict(model=MODELS['b'], system=self.instructions,
                    messages=self.history, tools=self.tools)

    def request(self, ceiling):
        payload = self.payload()
        endpoint = 'responses/input_tokens' if self.provider == 'a' else 'messages/count_tokens'
        counted = post(self.provider, endpoint, payload)['input_tokens']
        # Token-count preflight, 20% + 1024 token margin, and twice the GPT
        # standard cache-write price. Reservations intentionally overestimate.
        bound = math.ceil(counted * 1.2) + 1024
        if counted > 272000:
            raise BudgetStop('This pilot does not authorize long-context pricing')
        reserve_input = bound * 25 / 1e6
        remaining = min(ceiling - self.ledger.spent(self.label),
                        self.ledger.ceiling - self.ledger.spent())
        maximum = min(128000, math.floor((remaining - reserve_input) * 1e6 / OUTPUT))
        if maximum < 1:
            raise BudgetStop('Phase dollar ceiling')
        reserved = reserve_input + maximum * OUTPUT / 1e6
        if self.provider == 'a':
            payload.update(max_output_tokens=maximum, reasoning={'effort': 'high'},
                           store=False, include=['reasoning.encrypted_content'], service_tier='default')
        else:
            payload.update(max_tokens=maximum, output_config={'effort': 'high'})
        item = self.ledger.reserve(self.label, MODELS[self.provider], reserved)
        response = post(self.provider, 'responses' if self.provider == 'a' else 'messages', payload)
        self.ledger.settle(item, response, self.provider)
        calls, visible = [], []
        if self.provider == 'a':
            self.history.extend(response['output'])  # Preserve opaque reasoning for API continuity.
            for value in response['output']:
                if value['type'] == 'function_call':
                    calls.append((value['call_id'], value['name'], json.loads(value['arguments'])))
                if value['type'] == 'message':
                    visible.extend(c.get('text', '') for c in value.get('content', []) if c['type'] == 'output_text')
            stop = response.get('status')
        else:
            self.history.append({'role': 'assistant', 'content': response['content']})
            for value in response['content']:
                if value['type'] == 'tool_use':
                    calls.append((value['id'], value['name'], value['input']))
                if value['type'] == 'text':
                    visible.append(value['text'])
            stop = response.get('stop_reason')
        return calls, visible, stop

    def results(self, results):
        if self.provider == 'a':
            self.history.extend(dict(type='function_call_output', call_id=ident, output=json.dumps(value))
                                for ident, value in results)
        else:
            self.history.append({'role': 'user', 'content': [
                dict(type='tool_result', tool_use_id=ident, content=json.dumps(value))
                for ident, value in results]})
