"""Immutable study manifests and sealed, independently analyzable archives."""
import hashlib
import json
import math
from pathlib import Path
import platform
import random
import re
import subprocess
import sys

from problems import FAMILIES, instances, reference_costs

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SCHEMA = 'shared-record-information-policy.v1'
IDENTIFIER = re.compile(r'^[a-zA-Z0-9_-]+$')


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'),
                                     allow_nan=False).encode()).hexdigest()


def file_digest(path):
    with Path(path).open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def save(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + '.tmp')
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + '\n')
    temporary.replace(path)


def source_hashes():
    files = list(HERE.glob('*.py'))
    # Daemon helper imports are part of the tested runtime, not an untracked dependency.
    files += list((ROOT / 'scripts/client_qualification').glob('*.py'))
    files += [ROOT / 'scripts' / name for name in ('check_shared_context_models.py', 'owner_plans.py',
              'check_t2_models.py', 'check_t2_clients.py', 'check_clients.py')]
    return {str(path.relative_to(ROOT)): file_digest(path) for path in sorted(files)}


def task_prompt(block, group, allowance, evaluation):
    problem = FAMILIES[block['family']]
    arrangement = ('Three workers attempt this problem independently. The public selector considers all three workers.'
                   if group else 'You work alone; the public selector considers your candidates.')
    return (f'Problem: {problem.title}\n{problem.description}\nGenerator: {problem.shape}\n{arrangement}\n'
            f'Your total task allowance: ${allowance:.9g}.\n'
            f'Evaluator CPU allowance per instance: {evaluation["cpu_seconds"]} seconds.\n'
            f'Public instances: {len(block["public"]["instances"])}; evaluations per instance: {evaluation["repetitions"]}.\n'
            f'Public validity fixtures outside the sampled generator: {block["public"].get("fixture_count", 0)}. '
            'These affect public selection only; hidden instances follow the stated generator.\n'
            f'Reference costs: {json.dumps(block["public"]["reference"])}.\n'
            'Inspect public INSTANCES with run_python; evaluate complete candidate sources.')


def runtime(binary):
    return dict(source_sha256=source_hashes(), python=sys.version,
                executable=sys.executable, platform=platform.platform(),
                source_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                binary_sha256=file_digest(binary))


def validate(manifest):
    if manifest.get('schema') != SCHEMA:
        raise ValueError('unsupported study schema')
    if manifest['stage'] not in ('scripted', 'development', 'scored'):
        raise ValueError('unknown study stage')
    if not IDENTIFIER.fullmatch(manifest['study_id']):
        raise ValueError('study_id must be a safe identifier')
    for value in manifest['budget'].values():
        if not isinstance(value, (int, float)) or not math.isfinite(value) or value <= 0:
            raise ValueError('budgets must be finite and positive')
    arms = manifest['arms']
    if len({k for k in arms}) != len(arms) or not arms:
        raise ValueError('empty or duplicated arms')
    for name, arm in arms.items():
        if not IDENTIFIER.fullmatch(name) or arm['kind'] not in ('S', 'I', 'C', 'P', 'R'):
            raise ValueError('invalid arm')
        if arm['kind'] in ('C', 'P') and arm.get('policy') not in ('E', 'D', 'F'):
            raise ValueError('missing information policy')
    if manifest['stage'] == 'scored':
        if set(arms) != {'S', 'I', 'C', 'P', 'R'}:
            raise ValueError('scored study requires exactly five arms')
        if any(arms[name]['kind'] != name for name in arms):
            raise ValueError('scored arm labels must match their actual procedures')
        if arms['C']['policy'] != arms['P']['policy'] or arms['P']['policy'] not in ('D', 'F'):
            raise ValueError('freeze one matched D/F policy before scoring')
        readiness = manifest.get('readiness', {})
        required = ('selector', 'delivery', 'accounting', 'solo', 'precision', 'development_selection')
        if not all(readiness.get(key) for key in required):
            raise ValueError('scored study requires recorded development/readiness evidence')
    identities = set()
    for block in manifest['blocks']:
        if not IDENTIFIER.fullmatch(block['id']) or block['id'] in identities:
            raise ValueError('invalid or duplicate block identity')
        identities.add(block['id'])
        if block['family'] not in FAMILIES:
            raise ValueError('unknown problem family')
        if not block['pack'] or type(block['repetition']) is not int or block['repetition'] < 0:
            raise ValueError('invalid pack or repetition')
        if block['public']['digest'] != digest(block['public']['instances']):
            raise ValueError('public instance digest mismatch')
        if len(block['public']['reference']) != len(block['public']['instances']):
            raise ValueError('public reference count mismatch')
        if sorted(block['launch_order']) != sorted(arms):
            raise ValueError('launch order does not contain each assigned arm once')
    if not identities:
        raise ValueError('study needs at least one block')
    for model in ('luna', 'frontier'):
        if model not in manifest['models']:
            raise ValueError('missing model configuration')
        if manifest['continuation'][model] not in ('continue', 'verify', 'restart'):
            raise ValueError('unknown private continuation procedure')
        from luna import validate_model_config
        config = validate_model_config(manifest['models'][model])
        if config['input_accounting']['method'] not in ('context_bound', 'provider_count'):
            raise ValueError('runner requires a documented context bound or exact provider input count')
    if manifest['evaluation']['repetitions'] < 1 or manifest['analysis']['bootstrap_samples'] < 1:
        raise ValueError('evaluation repetitions and resamples must be positive')


def build_manifest(spec, binary, prompts):
    """Freeze concrete settings. No model calls or outcome-based choices occur here."""
    result = json.loads(json.dumps(spec))
    result['schema'] = SCHEMA
    result['runtime'] = runtime(binary)
    result['prompts'] = prompts
    blocks = []
    # A shuffled base order, rotated over blocks, counterbalances launch position.
    order = list(spec['arms'])
    random.Random(spec['seed']).shuffle(order)
    for i, assignment in enumerate(spec['blocks']):
        block = dict(assignment)
        block['id'] = 'b-' + digest([spec['study_id'], assignment])[:24]
        pack = block['pack']
        public = instances(spec['seed'], block['family'], 'public', pack=pack,
                           count=spec['public_instances'])
        fixtures = result.get('public_fixtures', {}).get(block['family'], [])
        if not isinstance(fixtures, list):
            raise ValueError('public fixtures must be an explicit instance list')
        public.extend(fixtures)
        hidden = instances(spec['seed'], block['family'], 'hidden', pack=pack,
                           count=spec['hidden_instances'])
        block['public'] = dict(instances=public, reference=reference_costs(block['family'], public),
                               digest=digest(public), fixture_count=len(fixtures),
                               fixture_digest=digest(fixtures))
        block['hidden'] = dict(seed=spec['seed'], pack=pack, count=len(hidden), digest=digest(hidden))
        block['prompts'] = {name: task_prompt(block, group, spec['budget']['task_arm_usd'] / (3 if group else 1),
                                            spec['evaluation'])
                            for name, group in [('solo', False), ('group', True)]}
        shift = i % len(order)
        block['launch_order'] = order[shift:] + order[:shift]
        blocks.append(block)
    result['blocks'] = blocks
    validate(result)
    return result


def freeze(output, manifest, binary):
    from record import copy_binary
    output = Path(output)
    if (output / 'manifest.json').exists():
        raise ValueError('manifest already exists; freeze into a new study directory')
    if output.exists() and any(output.iterdir()):
        raise ValueError('freeze requires an empty output directory')
    validate(manifest)
    output.mkdir(parents=True, exist_ok=True)
    copy_binary(binary, output / 'bin/locust')
    if file_digest(output / 'bin/locust') != manifest['runtime']['binary_sha256']:
        raise ValueError('binary changed while freezing')
    save(output / 'manifest.json', manifest)
    save(output / 'freeze.json', dict(schema=SCHEMA, manifest_sha256=file_digest(output / 'manifest.json')))


def load_manifest(output, validate_runtime=True):
    output = Path(output)
    manifest = json.loads((output / 'manifest.json').read_text())
    receipt = json.loads((output / 'freeze.json').read_text())
    if receipt != dict(schema=SCHEMA, manifest_sha256=file_digest(output / 'manifest.json')):
        raise ValueError('manifest differs from its frozen configuration receipt')
    validate(manifest)
    if validate_runtime:
        current = runtime(output / 'bin/locust')
        # The source commit records provenance. A later documentation-only
        # commit may track this very manifest without changing executed code.
        # Exact source/helper, interpreter and binary hashes remain mandatory.
        frozen = dict(manifest['runtime'])
        current.pop('source_commit')
        frozen.pop('source_commit')
        if current != frozen:
            raise ValueError('frozen runtime differs from current source, interpreter or binary')
    return manifest


def trial_paths(manifest):
    return [f'trials/{block["id"]}/{arm}.json' for block in manifest['blocks'] for arm in manifest['arms']]


def seal(output):
    output = Path(output)
    manifest = load_manifest(output)
    paths = trial_paths(manifest)
    for path in paths:
        trial = json.loads((output / path).read_text())
        if not trial.get('final'):
            raise ValueError('cannot seal incomplete assigned trial')
        if trial.get('manifest_sha256') != file_digest(output / 'manifest.json'):
            raise ValueError('trial belongs to another manifest')
    if set(output.glob('trials/*/*.json')) != {output / p for p in paths}:
        raise ValueError('unexpected trial file in study')
    paths += ['ledger.json', 'freeze.json']
    if (output / 'ledger.counts.json').exists():
        paths.append('ledger.counts.json')
    paths += [str(p.relative_to(output)) for p in sorted((output / 'prefixes').glob('*.json'))]
    if (output / 'daemon-events.jsonl').exists():
        paths.append('daemon-events.jsonl')
    value = dict(schema=SCHEMA, manifest_sha256=file_digest(output / 'manifest.json'),
                 files={p: file_digest(output / p) for p in paths})
    target = output / 'seal.json'
    if target.exists() and json.loads(target.read_text()) != value:
        raise ValueError('sealed archive cannot be changed')
    save(target, value)
    return value


def verify_seal(output):
    """Offline readers need only the archive, not the live source or daemon."""
    output = Path(output)
    manifest = load_manifest(output, validate_runtime=False)
    value = json.loads((output / 'seal.json').read_text())
    if value.get('schema') != SCHEMA or value['manifest_sha256'] != file_digest(output / 'manifest.json'):
        raise ValueError('manifest does not match seal')
    required = set(trial_paths(manifest)) | {'ledger.json', 'freeze.json'}
    if not required.issubset(value['files']):
        raise ValueError('seal omits assigned trials or ledger')
    if set(output.glob('trials/*/*.json')) != {output / p for p in trial_paths(manifest)}:
        raise ValueError('archive trial set differs from assignment')
    for name, expected in value['files'].items():
        path = output / name
        if not path.resolve().is_relative_to(output.resolve()) or file_digest(path) != expected:
            raise ValueError('sealed file changed: ' + name)
    for name in trial_paths(manifest):
        trial = json.loads((output / name).read_text())
        if not trial.get('final') or trial.get('manifest_sha256') != value['manifest_sha256']:
            raise ValueError('unsealed or mismatched trial: ' + name)
    return manifest
