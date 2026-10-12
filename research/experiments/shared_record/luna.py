"""Metered, archived Responses sessions. Network calls are disabled by default.

Live calls need explicit opt-in and a frozen manifest with qualified accounting.
Injected transports support scripted tests; no tokenizer heuristic is a bound.
"""
import copy
import hashlib
import json
import math
import os
from pathlib import Path
import threading
import time
import urllib.error
import urllib.request
import uuid


# Responses API schema minimum, not an experiment-specific output cap:
# https://developers.openai.com/api/reference/resources/responses/methods/create
RESPONSES_MIN_OUTPUT_TOKENS = 16


class BudgetStop(Exception):
    pass


class TransportFailure(Exception):
    pass


class PaidCallsDisabled(Exception):
    pass


class AccountingFailure(Exception):
    pass


def atomic_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + '.tmp')
    with temporary.open('w') as stream:
        json.dump(value, stream, indent=1, sort_keys=True, allow_nan=False)
        stream.write('\n')
        stream.flush()
        os.fsync(stream.fileno())
    temporary.replace(path)
    descriptor = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _number(value, name, positive=False):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise ValueError(f'{name} must be a finite number')
    if value < 0 or (positive and value == 0):
        raise ValueError(f'{name} must be {"positive" if positive else "nonnegative"}')
    return value


def _error_text(error):
    text = f'{type(error).__name__}: {error}'
    key = os.environ.get('OPENAI_API_KEY')
    return text.replace(key, '<redacted>') if key else text


def validate_model_config(config):
    value = copy.deepcopy(config)
    if not isinstance(value.get('model'), str) or not value['model']:
        raise ValueError('model must be explicit')
    rates = value['rates']
    for field in ('input', 'cached_input', 'cache_write', 'output'):
        _number(rates[field], f'rates.{field}', positive=field == 'output')
    for field in ('context_tokens', 'max_input_tokens', 'max_output_tokens'):
        if type(value[field]) is not int or value[field] < 1:
            raise ValueError(f'{field} must be a documented positive integer limit')
    if max(value['max_input_tokens'], value['max_output_tokens']) > value['context_tokens']:
        raise ValueError('input or output limit exceeds context limit')
    if value['max_output_tokens'] < RESPONSES_MIN_OUTPUT_TOKENS:
        raise ValueError('output limit is below the Responses API minimum of 16 tokens')
    if value['service_tier'] != 'default':
        raise ValueError('only explicitly priced default service tier is supported')
    long_context = value.get('long_context_pricing')
    if long_context is not None:
        if type(long_context['threshold_tokens']) is not int or long_context['threshold_tokens'] < 1:
            raise ValueError('long-context threshold must be a positive integer')
        for field in ('input_multiplier', 'output_multiplier'):
            _number(long_context[field], f'long_context_pricing.{field}', positive=True)
            if long_context[field] < 1:
                raise ValueError('long-context multipliers must not reduce the reservation bound')
    if value['input_accounting']['method'] not in ('context_bound', 'qualified_counter', 'provider_count'):
        raise ValueError('unsupported input accounting method')
    return value


def effective_rates(config, input_tokens):
    """Frozen prices for the whole request, selected by total input tokens."""
    rates = copy.deepcopy(config['rates'])
    long_context = config.get('long_context_pricing')
    if long_context is not None and input_tokens > long_context['threshold_tokens']:
        for field in ('input', 'cached_input', 'cache_write'):
            rates[field] *= long_context['input_multiplier']
        rates['output'] *= long_context['output_multiplier']
    return rates


class Ledger:
    """One durable billed record per dispatch; consuming arms reference its ID."""

    def __init__(self, path, ceiling):
        self.path = Path(path)
        self.ceiling = _number(ceiling, 'study ceiling')
        self.lock = threading.RLock()
        self.entries = json.loads(self.path.read_text()) if self.path.exists() else []
        self.count_path = self.path.with_suffix('.counts.json')
        self.counts = json.loads(self.count_path.read_text()) if self.count_path.exists() else []
        ids = [entry['id'] for entry in self.entries]
        if len(set(ids)) != len(ids):
            raise ValueError('duplicate ledger request IDs')
        count_ids = [entry['id'] for entry in self.counts]
        if len(set(count_ids)) != len(count_ids):
            raise ValueError('duplicate provider count IDs')

    def save(self):
        with self.lock:
            atomic_json(self.path, self.entries)

    def assert_qualified(self):
        with self.lock:
            if any(entry['status'] == 'accounting_error' for entry in self.entries + self.counts):
                raise AccountingFailure('study accounting qualification failed; no further dispatch is allowed')

    def start_count(self, label, phase, payload, session_id):
        """Durable audit for the non-generating provider input-count endpoint."""
        with self.lock:
            self.assert_qualified()
            item = dict(id='count-' + uuid.uuid4().hex, session_id=session_id,
                        label=label, phase=phase, endpoint='responses/input_tokens',
                        payload=copy.deepcopy(payload), status='pending', started_at=time.time())
            self.counts.append(item)
            atomic_json(self.count_path, self.counts)
            return item

    def finish_count(self, item, *, response=None, error=None, accounting_error=False):
        with self.lock:
            if item['status'] != 'pending':
                raise AccountingFailure('provider count is already settled')
            if response is not None:
                item['response'] = copy.deepcopy(response)
            item.update(status=('accounting_error' if accounting_error else 'failed')
                        if error is not None else 'complete', settled_at=time.time())
            if error is not None:
                item['error'] = str(error)
            atomic_json(self.count_path, self.counts)

    def snapshot(self, request_ids=None, at=None):
        """Cost committed at a cut; then-pending requests keep their reservation.

        IDs are deduplicated. A later settlement cannot lower an earlier cut's
        committed amount. Unknown IDs are errors, never free dependencies.
        """
        with self.lock:
            by_id = {entry['id']: entry for entry in self.entries}
            selected = list(by_id) if request_ids is None else list(dict.fromkeys(request_ids))
            missing = set(selected) - by_id.keys()
            if missing:
                raise ValueError(f'unknown request IDs: {sorted(missing)}')
            known = unknown = 0.0
            included, pending, failed = [], [], []
            for ident in selected:
                entry = by_id[ident]
                if at is not None and entry['reserved_at'] > at:
                    continue
                included.append(ident)
                settled = entry.get('settled_at')
                complete_at_cut = settled is not None and (at is None or settled <= at)
                if complete_at_cut and 'cost' in entry:
                    known += entry['cost']
                else:
                    unknown += entry['reserved']
                    (failed if complete_at_cut and entry['status'] in ('failed', 'accounting_error')
                     else pending).append(ident)
            return dict(request_ids=included, known_cost=known, unknown_reserved=unknown,
                        committed_cost=known + unknown, pending_ids=pending, failed_ids=failed, at=at)

    def spent(self, label=None, prefix=None):
        with self.lock:
            ids = [entry['id'] for entry in self.entries
                   if (label is None or entry['label'] == label)
                   and (prefix is None or entry['label'].startswith(prefix))]
            return self.snapshot(ids)['committed_cost']

    @property
    def failures(self):
        with self.lock:
            return sum(entry['status'] in ('failed', 'accounting_error') for entry in self.entries)

    def reserve(self, label, phase, amount, phase_ceiling, *, model_config, payload,
                input_bound, max_output_tokens, session_id, input_count_id=None,
                require_service_tier=False):
        with self.lock:
            self.assert_qualified()
            _number(amount, 'reservation')
            _number(phase_ceiling, 'phase ceiling')
            if self.spent() + amount > self.ceiling + 1e-12:
                raise BudgetStop('study ceiling')
            phase_ids = [entry['id'] for entry in self.entries if entry['phase'] == phase]
            if self.snapshot(phase_ids)['committed_cost'] + amount > phase_ceiling + 1e-12:
                raise BudgetStop('phase ceiling')
            item = dict(id='request-' + uuid.uuid4().hex, session_id=session_id, label=label,
                        phase=phase, model=model_config['model'], rates=copy.deepcopy(model_config['rates']),
                        reserved=amount, status='pending', reserved_at=time.time(),
                        input_bound=input_bound, max_output_tokens=max_output_tokens,
                        service_tier=model_config['service_tier'],
                        require_service_tier=require_service_tier,
                        long_context_pricing=copy.deepcopy(model_config.get('long_context_pricing')),
                        reservation_rates=effective_rates(model_config, input_bound),
                        input_count_id=input_count_id,
                        input_accounting=copy.deepcopy(model_config['input_accounting']),
                        payload=copy.deepcopy(payload))
            self.entries.append(item)
            self.save()
            return item

    def settle(self, item, response):
        with self.lock:
            if item['status'] != 'pending':
                raise AccountingFailure('request is already settled')
            item['response'] = copy.deepcopy(response)
            try:
                usage = response['usage']
                item['usage'] = copy.deepcopy(usage)
                incoming, outgoing = usage['input_tokens'], usage['output_tokens']
                rates = item['rates']
                details = usage.get('input_tokens_details', {})
                if rates['cached_input'] != rates['input'] and 'cached_tokens' not in details:
                    raise AccountingFailure('missing cached-token usage for distinct cache pricing')
                if rates['cache_write'] != rates['input'] and 'cache_write_tokens' not in details:
                    raise AccountingFailure('missing cache-write usage for distinct cache-write pricing')
                cached = details.get('cached_tokens', 0)
                cache_write = details.get('cache_write_tokens', 0)
                for name, count in (('input', incoming), ('output', outgoing),
                                    ('cached', cached), ('cache_write', cache_write)):
                    if type(count) is not int or count < 0:
                        raise AccountingFailure(f'invalid provider {name} usage')
                if cached + cache_write > incoming:
                    raise AccountingFailure('cached and cache-write tokens exceed input tokens')
                item.update(usage=copy.deepcopy(usage), response_id=response.get('id'),
                            returned_model=response.get('model'), response_status=response.get('status'),
                            returned_service_tier=response.get('service_tier'))
                if response.get('model') != item['model']:
                    # This model's frozen rates cannot price another identity.
                    raise AccountingFailure('unexpected model identity')
                if item['require_service_tier'] and 'service_tier' not in response:
                    raise AccountingFailure('missing provider service tier for live request')
                if 'service_tier' in response and response['service_tier'] != item['service_tier']:
                    raise AccountingFailure('unexpected provider service tier')
                rates = effective_rates(item, incoming)
                cost = ((incoming - cached - cache_write) * rates['input']
                        + cached * rates['cached_input'] + cache_write * rates['cache_write']
                        + outgoing * rates['output']) / 1e6
                item.update(cost=cost, charged_rates=rates, usage=copy.deepcopy(usage), settled_at=time.time(),
                            response_id=response.get('id'), returned_model=response.get('model'),
                            response_status=response.get('status'), status='complete')
                violations = []
                if incoming > item['input_bound']:
                    violations.append('input usage exceeded qualified bound')
                if outgoing > item['max_output_tokens']:
                    violations.append('output usage exceeded request maximum')
                if cost > item['reserved'] + 1e-12:
                    violations.append('cost exceeded reservation')
                if violations:
                    raise AccountingFailure('; '.join(violations))
            except (KeyError, TypeError, AttributeError, AccountingFailure) as error:
                item.update(status='accounting_error', error=str(error), settled_at=time.time())
                self.save()
                raise AccountingFailure(str(error)) from error
            self.save()
            return item

    def fail(self, item, error):
        with self.lock:
            if item['status'] != 'pending':
                raise AccountingFailure('request is already settled')
            item.update(status='failed', error=str(error), settled_at=time.time())
            self.save()


def _verify_live(model_config, live_manifest):
    if not live_manifest:
        raise PaidCallsDisabled('live calls require a verified frozen manifest')
    data = Path(live_manifest['path']).read_bytes()
    if hashlib.sha256(data).hexdigest() != live_manifest['sha256']:
        raise PaidCallsDisabled('live manifest hash changed')
    manifest = json.loads(data)
    if manifest['models'][live_manifest['model_key']] != model_config:
        raise PaidCallsDisabled('model configuration differs from the frozen manifest')
    qualification = model_config.get('qualification', {})
    evidence = [qualification.get(field) for field in ('model', 'rates', 'limits')]
    evidence.append(model_config['input_accounting'].get('qualification'))
    if not all(isinstance(item, str) and item.strip() for item in evidence):
        raise PaidCallsDisabled('live model, rates, limits and input accounting need qualification evidence')


def _http(payload, *, endpoint='responses'):
    key = os.environ['OPENAI_API_KEY']
    headers = {'Authorization': 'Bearer ' + key, 'Content-Type': 'application/json'}
    request = urllib.request.Request('https://api.openai.com/v1/' + endpoint,
                                     json.dumps(payload).encode(), headers)
    try:
        # No historical experiment-specific timeout is imposed here.
        with urllib.request.urlopen(request) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        try:
            message = str(json.load(error).get('error', {}).get('message', ''))
        except (ValueError, AttributeError):
            message = 'unreadable provider error'
        raise TransportFailure(f'HTTP {error.code}: {message.replace(key, "<redacted>")}') from None
    except (urllib.error.URLError, TimeoutError, OSError, ValueError) as error:
        raise TransportFailure(type(error).__name__) from None


def _http_count(payload):
    return _http(payload, endpoint='responses/input_tokens')


# These are the complete input-bearing fields emitted by Session.payload().
# Other fields affect storage, response representation or output, not its input.
COUNT_INPUT_FIELDS = ('model', 'instructions', 'input', 'tools', 'reasoning')
COUNT_NON_INPUT_FIELDS = ('store', 'include', 'service_tier', 'max_output_tokens')
_INHERIT = object()


class Session:
    def __init__(self, instructions, tools, ledger, label, phase, phase_ceiling, allowance,
                 effort='high', *, model_config, transport=None, input_counter=None,
                 count_transport=None, paid_opt_in=False, live_manifest=None, session_id=None):
        self.instructions, self.tools, self.ledger = instructions, copy.deepcopy(tools), ledger
        self.label, self.phase = label, phase
        self.phase_ceiling = _number(phase_ceiling, 'phase ceiling')
        self.allowance = _number(allowance, 'session allowance')
        self.effort = effort
        self.model_config = validate_model_config(model_config)
        self.transport, self.input_counter = transport, input_counter
        self.count_transport = count_transport
        self.paid_opt_in, self.live_manifest = paid_opt_in, copy.deepcopy(live_manifest)
        self.session_id = session_id or 'session-' + uuid.uuid4().hex
        self.history, self.archive = [], []
        self.count_archive = []
        self.request_ids, self.own_request_ids = [], []
        self.halted = None

    @property
    def requests(self):
        return self.archive

    def user(self, text):
        self.history.append({'role': 'user', 'content': text})

    def spent(self):
        return self.ledger.snapshot(self.request_ids)['committed_cost']

    def billing_spent(self):
        return self.ledger.snapshot(self.own_request_ids)['committed_cost']

    def payload(self):
        definitions = [dict(type='function', name=t['name'], description=t['description'],
                            parameters=copy.deepcopy(t['input_schema']), strict=True) for t in self.tools]
        return dict(model=self.model_config['model'], instructions=self.instructions,
                    input=copy.deepcopy(self.history), tools=definitions,
                    reasoning={'effort': self.effort}, store=False,
                    include=['reasoning.encrypted_content'], service_tier=self.model_config['service_tier'])

    def fork(self, label, allowance=None, *, phase=None, phase_ceiling=None,
             transport=_INHERIT, session_id=None):
        if self.ledger.snapshot(self.request_ids)['pending_ids']:
            raise ValueError('cannot fork while a prefix request is unsettled')
        child = Session(self.instructions, self.tools, self.ledger, label,
                        phase or self.phase, self.phase_ceiling if phase_ceiling is None else phase_ceiling,
                        self.allowance if allowance is None else allowance, self.effort,
                        model_config=self.model_config,
                        transport=self.transport if transport is _INHERIT else transport,
                        input_counter=self.input_counter, count_transport=self.count_transport,
                        paid_opt_in=self.paid_opt_in,
                        live_manifest=self.live_manifest, session_id=session_id)
        child.history = copy.deepcopy(self.history)
        child.archive = copy.deepcopy(self.archive)
        child.count_archive = copy.deepcopy(self.count_archive)
        child.request_ids = list(self.request_ids)
        child.halted = self.halted
        return child

    def _authorize_live(self):
        if self.paid_opt_in is not True:
            raise PaidCallsDisabled('no scripted transport and paid calls are disabled')
        _verify_live(self.model_config, self.live_manifest)
        if not os.environ.get('OPENAI_API_KEY'):
            raise PaidCallsDisabled('live calls require a configured API credential')

    def provider_count(self, payload):
        """Count exact request input without generation; never fall back to a guess.

        The provider count request and full response are durably archived before
        generation is reserved. Counts are separate from billed generation rows.
        """
        if self.halted:
            raise TransportFailure(f'session stopped: {self.halted}')
        self.ledger.assert_qualified()
        if self.count_transport is None:
            self._authorize_live()
        unknown = set(payload) - set(COUNT_INPUT_FIELDS + COUNT_NON_INPUT_FIELDS)
        if unknown:
            raise ValueError(f'input counter has unqualified payload fields: {sorted(unknown)}')
        if payload.get('model') != self.model_config['model']:
            raise ValueError('input count model differs from frozen configuration')
        projected = {key: copy.deepcopy(payload[key]) for key in COUNT_INPUT_FIELDS if key in payload}
        item = self.ledger.start_count(self.label, self.phase, projected, self.session_id)
        archived = copy.deepcopy(item)
        self.count_archive.append(archived)
        try:
            result = (self.count_transport or _http_count)(copy.deepcopy(projected))
        except Exception as error:
            self.halted = _error_text(error)
            self.ledger.finish_count(item, error=self.halted)
            archived.update(copy.deepcopy(item))
            raise TransportFailure(self.halted) from error
        try:
            count = result['input_tokens']
            if result.get('object') != 'response.input_tokens':
                raise AccountingFailure('unexpected provider input-count response object')
            if type(count) is not int or count < 0:
                raise AccountingFailure('invalid provider input-token count')
            if 'model' in result and result['model'] != self.model_config['model']:
                raise AccountingFailure('unexpected provider input-count model identity')
        except (KeyError, TypeError, AttributeError, AccountingFailure) as error:
            self.halted = str(error)
            self.ledger.finish_count(item, response=result, error=self.halted, accounting_error=True)
            archived.update(copy.deepcopy(item))
            raise AccountingFailure(self.halted) from error
        self.ledger.finish_count(item, response=result)
        archived.update(copy.deepcopy(item))
        return count

    def request(self):
        if self.halted:
            raise TransportFailure(f'session stopped: {self.halted}')
        self.ledger.assert_qualified()
        if self.transport is None:
            self._authorize_live()
        payload = self.payload()
        config = self.model_config
        method = config['input_accounting']['method']
        input_count_id = None
        if method in ('qualified_counter', 'provider_count'):
            if method == 'provider_count':
                bound = self.provider_count(payload)
                input_count_id = self.count_archive[-1]['id']
            else:
                if self.input_counter is None:
                    raise ValueError('qualified_counter requires an injected, qualified full-input counter')
                bound = self.input_counter(copy.deepcopy(payload))
            if type(bound) is not int or bound < 0:
                raise ValueError('input counter must return a nonnegative integer bound')
            if bound > config['max_input_tokens']:
                raise BudgetStop('request input exceeds documented provider input limit')
            output_room = config['context_tokens'] - bound
        else:
            # Qualified context bounds accepted input, not this request's size.
            # This overreserves and may let the provider reject insufficient
            # context, but does not pretend that a byte heuristic counted input.
            bound = config['max_input_tokens']
            output_room = config['max_output_tokens']
        rates = effective_rates(config, bound)
        reserve_input = bound * max(rates[field] for field in ('input', 'cached_input', 'cache_write')) / 1e6
        remaining = self.allowance - self.spent()
        affordable_output = math.floor((remaining - reserve_input) * 1e6 / rates['output'])
        maximum = min(config['max_output_tokens'], output_room, affordable_output)
        if maximum < RESPONSES_MIN_OUTPUT_TOKENS:
            raise BudgetStop('context or session allowance cannot fund the provider minimum of 16 output tokens')
        reserved = reserve_input + maximum * rates['output'] / 1e6
        payload['max_output_tokens'] = maximum
        if self.transport is None:
            # Counting may have waited on the provider. Recheck the frozen
            # authorization before reserving and dispatching generation.
            self._authorize_live()
        item = self.ledger.reserve(self.label, self.phase, reserved, self.phase_ceiling,
                                   model_config=config, payload=payload, input_bound=bound,
                                   max_output_tokens=maximum, session_id=self.session_id,
                                   input_count_id=input_count_id, require_service_tier=self.transport is None)
        self.request_ids.append(item['id'])
        self.own_request_ids.append(item['id'])
        archived = dict(id=item['id'], session_id=self.session_id, payload=copy.deepcopy(payload),
                        reserved=item['reserved'], reserved_at=item['reserved_at'], status='pending')
        if input_count_id is not None:
            archived['input_count'] = copy.deepcopy(self.count_archive[-1])
        self.archive.append(archived)
        started = time.time()
        try:
            response = (self.transport or _http)(copy.deepcopy(payload))
        except Exception as error:
            # Unknown dispatched responses keep their reservation. No retry.
            self.halted = _error_text(error)
            self.ledger.fail(item, self.halted)
            archived.update(status='failed', error=self.halted, settled_at=item['settled_at'],
                            elapsed=time.time() - started)
            raise TransportFailure(self.halted) from error
        archived['response'] = copy.deepcopy(response)
        try:
            self.ledger.settle(item, response)
        except AccountingFailure as error:
            self.halted = str(error)
            archived.update(status='accounting_error', error=str(error),
                            settled_at=item['settled_at'], elapsed=time.time() - started)
            raise
        archived.update(status=response.get('status'), usage=copy.deepcopy(response['usage']),
                        cost=item['cost'], response_id=response.get('id'), settled_at=item['settled_at'],
                        elapsed=time.time() - started, incomplete=response.get('incomplete_details'))
        calls, visible, retained = [], [], []
        for value in response.get('output', []):
            kind = value.get('type')
            if kind == 'function_call':
                try:
                    arguments = json.loads(value['arguments'])
                except (json.JSONDecodeError, TypeError):
                    visible.append('[incomplete tool call discarded: ' + value.get('name', '?') + ']')
                    continue
                calls.append((value['call_id'], value['name'], arguments))
            elif kind == 'message':
                visible.extend(c.get('text', '') for c in value.get('content', []) if c.get('type') == 'output_text')
            retained.append(copy.deepcopy(value))
        self.history.extend(retained)
        return calls, visible, response.get('status')

    def results(self, results):
        self.history.extend(dict(type='function_call_output', call_id=ident, output=json.dumps(value))
                            for ident, value in results)
