"""Durable whole-campaign funding across sequential, separately sealed studies.

Active allocations reserve their entire study ceiling. Settling a sealed study
replaces that cap with known charges plus every uncertain request reservation.
Only the unused difference becomes available to another study. Frozen studies
that never started can be explicitly cancelled, retaining their preflight proof.
Run one execution process at a time; concurrent execute/cancel is unsupported.
No keys or model transports are accessed here.
"""
import argparse
from contextlib import contextmanager
from decimal import Decimal
import fcntl
import hashlib
import json
import math
import os
from pathlib import Path
import tempfile
import time
import uuid

SCHEMA = 'shared-record-campaign.v1'


def _digest(value):
    data = json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()
    return hashlib.sha256(data).hexdigest()


def _money(value, name, positive=False):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise ValueError(f'{name} must be finite dollars')
    amount = Decimal(str(value))
    if amount < 0 or (positive and amount == 0):
        raise ValueError(f'{name} must be {"positive" if positive else "nonnegative"}')
    return amount


def _as_float_not_above(amount):
    """Expose a numeric remaining allowance without rounding it above the cap."""
    value = float(amount)
    if Decimal(str(value)) > amount:
        value = math.nextafter(value, -math.inf)
    return value


def _atomic(path, value):
    path = Path(path)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode='w', prefix='.' + path.name + '-',
                                         suffix='.tmp', dir=path.parent, delete=False) as output:
            temporary = Path(output.name)
            os.chmod(temporary, 0o600)
            json.dump(value, output, sort_keys=True, indent=2, allow_nan=False)
            output.write('\n')
            output.flush()
            os.fsync(output.fileno())
        temporary.replace(path)
        directory = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if temporary is not None and temporary.exists():
            temporary.unlink()


@contextmanager
def _locked(path):
    path = Path(path).resolve()
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.with_name(path.name + '.lock').open('a+') as lock:
        os.chmod(lock.name, 0o600)
        fcntl.flock(lock, fcntl.LOCK_EX)
        try:
            yield path
        finally:
            fcntl.flock(lock, fcntl.LOCK_UN)


def _header(value):
    return {key: value[key] for key in ('schema', 'campaign_id', 'ceiling_usd', 'created_at')}


def _binding(value):
    return {key: value[key] for key in ('allocation_id', 'name', 'output', 'ceiling_usd',
                                       'parameters', 'created_at')}


def _load(path):
    value = json.loads(path.read_text())
    if value.get('schema') != SCHEMA or value.get('campaign_sha256') != _digest(_header(value)):
        raise ValueError('campaign identity or ceiling differs from its immutable receipt')
    _money(value['ceiling_usd'], 'campaign ceiling', positive=True)
    outputs, ids = set(), set()
    for name, allocation in value['allocations'].items():
        if allocation['name'] != name or allocation['binding_sha256'] != _digest(_binding(allocation)):
            raise ValueError('allocation differs from its immutable receipt')
        if allocation['output'] in outputs or allocation['allocation_id'] in ids:
            raise ValueError('duplicate campaign output or allocation identity')
        outputs.add(allocation['output'])
        ids.add(allocation['allocation_id'])
        _money(allocation['ceiling_usd'], 'allocation ceiling', positive=True)
        if allocation['status'] not in ('allocated', 'settled', 'cancelled'):
            raise ValueError('unknown campaign allocation status')
        if allocation['status'] == 'settled':
            committed = Decimal(allocation['settlement']['committed_decimal'])
            if not committed.is_finite() or committed < 0:
                raise ValueError('invalid settled campaign commitment')
        if allocation['status'] == 'cancelled':
            cancellation = allocation['cancellation']
            proof = {key: item for key, item in cancellation.items() if key != 'proof_sha256'}
            if (cancellation.get('proof_sha256') != _digest(proof)
                    or cancellation.get('committed_decimal') != '0'
                    or not isinstance(cancellation.get('reason'), str)
                    or not cancellation['reason'].strip()):
                raise ValueError('cancelled allocation differs from its retained proof')
    if _committed(value) > Decimal(str(value['ceiling_usd'])) and not value.get('halted'):
        raise ValueError('campaign commitments exceed ceiling without a recorded accounting halt')
    return value


def _committed(value):
    return sum((Decimal(str(allocation['ceiling_usd'])) if allocation['status'] == 'allocated'
                else Decimal(0) if allocation['status'] == 'cancelled'
                else Decimal(allocation['settlement']['committed_decimal'])
                for allocation in value['allocations'].values()), Decimal(0))


def _receipt(path, campaign, allocation):
    return dict(path=str(path), campaign_id=campaign['campaign_id'],
                campaign_sha256=campaign['campaign_sha256'],
                allocation_id=allocation['allocation_id'], name=allocation['name'],
                output=allocation['output'], ceiling_usd=allocation['ceiling_usd'],
                binding_sha256=allocation['binding_sha256'])


def create(path, campaign_id, ceiling=30.0):
    """Create an immutable campaign identity and maximum authorized dollar spend."""
    _money(ceiling, 'campaign ceiling', positive=True)
    if not isinstance(campaign_id, str) or not campaign_id.strip():
        raise ValueError('campaign_id must be explicit')
    with _locked(path) as path:
        if path.exists():
            raise ValueError('campaign already exists')
        value = dict(schema=SCHEMA, campaign_id=campaign_id, ceiling_usd=ceiling,
                     created_at=time.time(), halted=None, allocations={})
        value['campaign_sha256'] = _digest(_header(value))
        _atomic(path, value)
        return value


def read(path):
    """Inspect funding; active caps and settled commitments remain separate."""
    with _locked(path) as path:
        value = _load(path)
        reserved = _committed(value)
        balance = max(Decimal(0), Decimal(str(value['ceiling_usd'])) - reserved)
        return dict(value, committed_usd=float(reserved),
                    remaining_usd=_as_float_not_above(balance), remaining_decimal=str(balance))


def remaining(path):
    return read(path)['remaining_usd']


def allocate(path, name, output, ceiling, parameters=None):
    """Reserve a distinct study slot before freeze; an exact retry is idempotent.

    Parameters are a nested subset of the resulting manifest, for example
    {'stage': 'development', 'budget': {'task_arm_usd': .42}}. Lists and scalar
    values match exactly. The study ceiling and output binding are mandatory.
    """
    amount = _money(ceiling, 'allocation ceiling', positive=True)
    if not isinstance(name, str) or not name.strip():
        raise ValueError('allocation name must be explicit')
    output = str(Path(output).resolve())
    parameters = {} if parameters is None else parameters
    if not isinstance(parameters, dict):
        raise ValueError('allocation parameters must be a manifest subset')
    parameters = json.loads(json.dumps(parameters, allow_nan=False))
    with _locked(path) as path:
        campaign = _load(path)
        if campaign.get('halted'):
            raise ValueError('campaign accounting is halted: ' + campaign['halted'])
        previous = campaign['allocations'].get(name)
        if previous is not None:
            if (previous['output'] == output and Decimal(str(previous['ceiling_usd'])) == amount
                    and previous['parameters'] == parameters and previous['status'] == 'allocated'):
                return _receipt(path, campaign, previous)
            raise ValueError('allocation name already belongs to another or inactive study')
        if any(slot['output'] == output for slot in campaign['allocations'].values()):
            raise ValueError('study output is already allocated')
        if _committed(campaign) + amount > Decimal(str(campaign['ceiling_usd'])):
            raise ValueError('allocation exceeds remaining campaign funding')
        allocation = dict(allocation_id='allocation-' + uuid.uuid4().hex, name=name, output=output,
                          ceiling_usd=ceiling, parameters=parameters, created_at=time.time(), status='allocated')
        allocation['binding_sha256'] = _digest(_binding(allocation))
        campaign['allocations'][name] = allocation
        _atomic(path, campaign)
        return _receipt(path, campaign, allocation)


def _matches(actual, expected, location='manifest'):
    if isinstance(expected, dict):
        if not isinstance(actual, dict):
            raise ValueError('allocated parameter differs: ' + location)
        for key, value in expected.items():
            if key not in actual:
                raise ValueError('allocated parameter missing: ' + location + '.' + key)
            _matches(actual[key], value, location + '.' + key)
    elif actual != expected:
        raise ValueError('allocated parameter differs: ' + location)


def _verify(path, campaign, output, manifest, require_active):
    receipt = manifest.get('campaign')
    if not isinstance(receipt, dict):
        raise ValueError('paid study requires a campaign allocation receipt')
    allocation = campaign['allocations'].get(receipt.get('name'))
    if allocation is None or receipt != _receipt(path, campaign, allocation):
        raise ValueError('manifest campaign allocation receipt does not match')
    if str(Path(output).resolve()) != allocation['output']:
        raise ValueError('study output differs from allocated output')
    if _money(manifest['budget']['study_usd'], 'study ceiling') != Decimal(str(allocation['ceiling_usd'])):
        raise ValueError('study ceiling differs from allocated cap')
    _matches(manifest, allocation['parameters'])
    if require_active and (allocation['status'] != 'allocated' or campaign.get('halted')):
        raise ValueError('campaign allocation is settled/cancelled or accounting is halted')
    return allocation


def verify_allocation(output, manifest, require_active=True):
    """Guard a paid launch against its frozen whole-campaign allocation."""
    receipt = manifest.get('campaign')
    if not isinstance(receipt, dict) or not receipt.get('path'):
        raise ValueError('paid study requires a campaign allocation receipt')
    with _locked(receipt['path']) as path:
        campaign = _load(path)
        allocation = _verify(path, campaign, output, manifest, require_active)
        return _receipt(path, campaign, allocation)


def cancel(path, name, reason):
    """Release an explicitly unstarted frozen study, keeping its original proof.

    This is not failed-request recovery. Any execution marker, request ledger,
    token-count ledger or output seal prevents cancellation, even if empty.
    An exact repeated cancellation is idempotent. Its name/output stay used.
    """
    from protocol import load_manifest
    if not isinstance(reason, str) or not reason.strip():
        raise ValueError('cancellation requires an explicit reason')
    with _locked(path) as path:
        campaign = _load(path)
        allocation = campaign['allocations'].get(name)
        if allocation is None:
            raise ValueError('unknown campaign allocation')
        if allocation['status'] == 'settled':
            raise ValueError('a settled study cannot be cancelled')
        output = Path(allocation['output'])
        for filename in ('execution.json', 'ledger.json', 'ledger.counts.json', 'seal.json'):
            if os.path.lexists(output / filename):
                raise ValueError('study may have started; cannot cancel with ' + filename)
        # A source repair can invalidate an unused runtime while its original
        # manifest/freeze receipt remains valid preflight evidence.
        manifest = load_manifest(output, validate_runtime=False)
        _verify(path, campaign, output, manifest, require_active=False)
        proof = dict(manifest_sha256=hashlib.sha256((output / 'manifest.json').read_bytes()).hexdigest(),
                     freeze_sha256=hashlib.sha256((output / 'freeze.json').read_bytes()).hexdigest())
        if allocation['status'] == 'cancelled':
            previous = allocation['cancellation']
            if previous['reason'] != reason or previous['proof'] != proof:
                raise ValueError('repeated cancellation differs from its original reason or proof')
            return previous
        cancellation = dict(reason=reason, proof=proof, cancelled_at=time.time(), committed_decimal='0')
        cancellation['proof_sha256'] = _digest(cancellation)
        allocation.update(status='cancelled', cancellation=cancellation)
        _atomic(path, campaign)
        return cancellation


def _charges(entries):
    seen, requests = set(), []
    known = uncertain = Decimal(0)
    accounting_errors = []
    for entry in entries:
        ident = entry['id']
        if ident in seen:
            raise ValueError('duplicate ledger request ID')
        seen.add(ident)
        reserved = _money(entry['reserved'], 'request reservation')
        status = entry['status']
        if status not in ('complete', 'pending', 'failed', 'accounting_error'):
            raise ValueError('unknown sealed ledger request status')
        cost = None if entry.get('cost') is None else _money(entry['cost'], 'request cost')
        if status == 'complete':
            if cost is None or entry.get('settled_at') is None:
                raise ValueError('completed request lacks settled cost')
            known += cost
            charged = cost
            if cost > reserved:
                accounting_errors.append(ident + ': settled cost exceeded reservation')
        elif status == 'accounting_error':
            # A pricing/identity/bound error cannot create reusable funding.
            charged = max(reserved, cost or Decimal(0))
            uncertain += charged
            accounting_errors.append(ident + ': accounting qualification failed')
        else:
            if cost is not None:
                raise ValueError('uncertain request unexpectedly has a settled cost')
            charged = reserved
            uncertain += charged
        requests.append(dict(id=ident, status=status, committed_decimal=str(charged)))
    return dict(committed_decimal=str(known + uncertain), known_usage_decimal=str(known),
                uncertain_reserved_decimal=str(uncertain), committed_cost_usd=float(known + uncertain),
                known_usage_cost_usd=float(known), uncertain_reserved_usd=float(uncertain),
                requests=requests, accounting_errors=accounting_errors)


def settle(path, name):
    """Release unused capacity only from a sealed study; uncertain charges persist."""
    from protocol import verify_seal
    with _locked(path) as path:
        campaign = _load(path)
        if name not in campaign['allocations']:
            raise ValueError('unknown campaign allocation')
        allocation = campaign['allocations'][name]
        if allocation['status'] == 'cancelled':
            raise ValueError('a cancelled unstarted study cannot be settled')
        output = Path(allocation['output'])
        manifest = verify_seal(output)
        _verify(path, campaign, output, manifest, require_active=False)
        seal_bytes = (output / 'seal.json').read_bytes()
        sealed = json.loads(seal_bytes)
        ledger_bytes = (output / 'ledger.json').read_bytes()
        ledger_sha = hashlib.sha256(ledger_bytes).hexdigest()
        if sealed['files']['ledger.json'] != ledger_sha:
            raise ValueError('ledger changed while settling campaign allocation')
        proof = dict(seal_sha256=hashlib.sha256(seal_bytes).hexdigest(), ledger_sha256=ledger_sha,
                     manifest_sha256=sealed['manifest_sha256'])
        if allocation['status'] == 'settled':
            if allocation['settlement']['proof'] != proof:
                raise ValueError('settled study evidence changed')
            return allocation['settlement']
        charges = _charges(json.loads(ledger_bytes))
        if Decimal(charges['committed_decimal']) > Decimal(str(allocation['ceiling_usd'])):
            charges['accounting_errors'].append('study commitment exceeded its allocated cap')
        settlement = dict(charges, proof=proof, settled_at=time.time())
        allocation.update(status='settled', settlement=settlement)
        if charges['accounting_errors']:
            campaign['halted'] = '; '.join(charges['accounting_errors'])
        if _committed(campaign) > Decimal(str(campaign['ceiling_usd'])):
            campaign['halted'] = 'campaign commitment exceeded its authorized ceiling'
        _atomic(path, campaign)
        return settlement


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--campaign', type=Path, required=True)
    sub = parser.add_subparsers(dest='command', required=True)
    create_args = sub.add_parser('create')
    create_args.add_argument('--id', required=True)
    create_args.add_argument('--ceiling', type=float, default=30.0)
    sub.add_parser('status')
    allocation = sub.add_parser('allocate')
    allocation.add_argument('--name', required=True)
    allocation.add_argument('--output', type=Path, required=True)
    allocation.add_argument('--ceiling', type=float, required=True)
    allocation.add_argument('--parameters', type=Path)
    completion = sub.add_parser('settle')
    completion.add_argument('--name', required=True)
    cancellation = sub.add_parser('cancel')
    cancellation.add_argument('--name', required=True)
    cancellation.add_argument('--reason', required=True)
    args = parser.parse_args()
    if args.command == 'create':
        value = create(args.campaign, args.id, args.ceiling)
    elif args.command == 'allocate':
        parameters = json.loads(args.parameters.read_text()) if args.parameters else None
        value = allocate(args.campaign, args.name, args.output, args.ceiling, parameters)
    elif args.command == 'settle':
        value = settle(args.campaign, args.name)
    elif args.command == 'cancel':
        value = cancel(args.campaign, args.name, args.reason)
    else:
        value = read(args.campaign)
    print(json.dumps(value, sort_keys=True, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
