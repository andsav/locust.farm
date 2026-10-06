#!/usr/bin/env python3
"""Two real coding clients exercise one shared workspace as distinct principals.

Synthetic arithmetic fixture; one production daemon and one physical host. Native
clients have permissive tools and separate profiles, not OS filesystem isolation.
There is no model execution deadline, token ceiling or tool-call ceiling.
"""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import time

from check_clients import session_identifier
from check_shared_context_models import (ROOT, await_agent, enroll, entries, raw_call,
                                         retain_codex_sessions, save, sha)
from check_t2_models import BASE_CODE, TEST_CODE, model_completion, native_calls, skill_read
from client_qualification.acceptance_evidence import hash_evidence, review_identity, usage
from client_qualification.collaboration_evidence import _calls
from client_qualification.production import ProductionDaemon, redact
from client_qualification.real_models import (PROVIDERS, RedactingProcess,
                                              configure_real_provider, install_locust_skill)
from client_qualification.runtime import Profile, private_write, records


NOTES = 'Synthetic arithmetic fixture. Preserve this managed note.\n'
DIRTY_NOTES = NOTES + 'Local coordinator notes in progress.\n'
UNTRACKED = 'Unrelated coordinator local work.\n'
FILES = ('calculator.py', 'test_calculator.py', 'notes.md')


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def json_values(value):
    """Decode complete native outputs, including text blocks and CLI prefixes."""
    if isinstance(value, dict):
        yield value
        for nested in value.values():
            yield from json_values(nested)
    elif isinstance(value, list):
        for nested in value:
            yield from json_values(nested)
    elif isinstance(value, str):
        decoder = json.JSONDecoder()
        for match in re.finditer(r'\{', value):
            try:
                decoded, _ = decoder.raw_decode(value[match.start():])
            except ValueError:
                continue
            if isinstance(decoded, dict):
                yield decoded


def completed_calls(run):
    yield from (call for call in run.get('native_calls', []) if call.get('success') is True)
    # Current Codex exec JSON sometimes omits stdout that its native session
    # retains. Pair function outputs to their real call IDs, never prompt text.
    for snapshot in run.get('native_sessions', []):
        pending = {}
        for event in records(snapshot['path'], strict=True):
            payload = event.get('payload', {})
            if event.get('type') != 'response_item':
                continue
            if payload.get('type') == 'function_call':
                try:
                    arguments = json.loads(payload.get('arguments', '{}'))
                except ValueError:
                    continue
                command = arguments.get('cmd', arguments.get('command', arguments.get('code', '')))
                if isinstance(command, list):
                    command = shlex.join(command)
                if isinstance(command, str):
                    pending[payload.get('call_id')] = (command, arguments.get('workdir', arguments.get('cwd')))
            elif payload.get('type') == 'function_call_output':
                found = pending.pop(payload.get('call_id'), None)
                if found:
                    output = payload.get('output', '')
                    successful = bool(re.search(r'Process exited with code 0\b|exit_code[\"\s:]+0\b', str(output)))
                    yield {'arguments': {'command': found[0], 'cwd': found[1]}, 'output': output,
                           'success': successful}


def shell_invocations(command, cwd=None):
    """Read simple native shell invocations, never quoted commands as data.

    This deliberately accepts individual commands, shell -c wrappers and command
    lists. Substitution, pipelines and redirection are not execution evidence.
    A cd supplies test-directory evidence only when its successor uses &&.
    """
    try:
        words = shlex.split(command)
        if words and Path(words[0]).name in ('sh', 'bash', 'zsh'):
            index = next((i for i, word in enumerate(words[1:], 1) if word in ('-c', '-lc')), None)
            if index is not None and index + 1 < len(words):
                yield from shell_invocations(words[index + 1], cwd)
                return
        # Split syntax before unquoting words: a quoted ";" is an argument,
        # not a second command. shlex alone loses that distinction.
        pieces, start, index, quote, escaped = [], 0, 0, None, False
        while index < len(command):
            character = command[index]
            if escaped:
                escaped = False
            elif character == '\\' and quote != "'":
                escaped = True
            elif quote:
                if character == quote:
                    quote = None
            elif character in ("'", '"'):
                quote = character
            elif character in '|<>':
                return
            elif character in ';&\n':
                delimiter = character
                if character == '&':
                    if command[index:index + 2] != '&&':
                        return
                    delimiter = '&&'
                pieces.append((command[start:index], delimiter))
                index += len(delimiter) - 1
                start = index + 1
            index += 1
        pieces.append((command[start:], ';'))
        commands = [(shlex.split(piece), delimiter) for piece, delimiter in pieces]
    except ValueError:
        return
    for segment, delimiter in commands:
        if segment:
            if segment[0] == 'cd' and len(segment) == 2:
                cwd = segment[1] if delimiter == '&&' else None
            elif not any('$(' in word or '`' in word for word in segment):
                yield segment, cwd


def native_results(run, operation):
    for call in completed_calls(run):
        arguments = call.get('arguments', {})
        invocations = shell_invocations(arguments.get('command', ''), arguments.get('cwd'))
        if not any((argv[0] == './locust-scoped' or argv[0] == run.get('cli_wrapper'))
                   and argv[1:1 + len(operation.split())] == operation.split() for argv, _ in invocations):
            continue
        for value in json_values(call.get('output')):
            if value.get('ok') is True and isinstance(value.get('result'), dict):
                yield value['result']


def native_event(run, operation, event):
    for result in native_results(run, operation):
        for key in ('operation', 'workspace_operation'):
            if result.get(key, {}).get('state', {}).get('recorded', {}).get('event') == event:
                return True
        if result.get('recorded', {}).get('event') == event:
            return True
    return False


def exact_review_read(run, expected):
    # ProductionDaemon redacts every "bytes" field, including public size
    # counts. Compare under that same policy; manifest hashes and exact diff
    # remain bound, and independent materialization verifies actual file bytes.
    return any(review_identity(redact(result)) == review_identity(redact(expected))
               for result in native_results(run, 'workspace review'))


def native_tests(run, directory):
    for call in completed_calls(run):
        args = call.get('arguments', {})
        output = str(call.get('output', ''))
        if call.get('success') is not True or 'Ran 5 tests' not in output or not re.search(r'\bOK\b', output):
            continue
        for argv, cwd in shell_invocations(args.get('command', ''), args.get('cwd')):
            if (re.fullmatch(r'python(?:\d+(?:\.\d+)*)?', Path(argv[0]).name)
                    and any(argv[i:i + 2] == ['-m', 'unittest'] for i in range(len(argv) - 1))
                    and cwd == str(directory)):
                return True
    return False


def author(event, principal, kind):
    view = event.get('view', {})
    return (view.get('author') == principal and view.get('kind') == kind
            and view.get('standing') == 'effective')


def coordinator_files(root, calculator):
    root = Path(root)
    expected = {'calculator.py': calculator, 'test_calculator.py': TEST_CODE.encode(),
                'notes.md': DIRTY_NOTES.encode(), 'unrelated.txt': UNTRACKED.encode()}
    return (not (root / '.git').exists() and all((root / name).is_file()
        and not (root / name).is_symlink() and (root / name).read_bytes() == content
        for name, content in expected.items()))


def tests(profile, root):
    result = subprocess.run([sys.executable, '-B', '-m', 'unittest', 'discover', '-s', '.', '-v'],
        cwd=root, env=profile.environment(sys.executable), capture_output=True, text=True, check=False)
    return {'exit_code': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr}


def retain_tree(output, label, root):
    destination = output / label
    destination.mkdir(mode=0o700)
    for name in (*FILES, 'unrelated.txt'):
        if (root / name).is_file():
            private_write(destination / name, (root / name).read_bytes())
    return str(destination)


def install_wrapper(role, daemon):
    role['wrapper'] = role['profile'].workspace / 'locust-scoped'
    prefix = [str(daemon.binary), '--home', str(daemon.home), '--credential', str(role['credential']),
              '--session', str(role['session']), '--json']
    private_write(role['wrapper'], '#!/bin/sh\nexec ' + shlex.join(prefix) + ' "$@"\n')
    role['wrapper'].chmod(0o700)


def context(role, daemon, work):
    return f'''You are the {role['name']} in an authorized real-model LOCAL synthetic Locust qualification.
FIRST read the Locust operating skill at {role['skill']['path']} using a native read tool.
Your principal is {role['principal']}, protected session instance {role['instance']}, goal {daemon.goal}.
Use actual registered Locust MCP tools for shared status/task/claim/review operations. Discover them if deferred.
Use {role['wrapper']} for Locust filesystem CLI operations. Its --help and contract commands describe the installed API.
Your assigned ordinary workspace is {role['profile'].workspace}. The folder returned by your own checkout registration and the review path {work['native_review']} are also authorized.
Credential/session paths in the wrapper are capabilities to pass to Locust only. Never read, print or copy their bytes.
Never inspect process environments, owner credentials, other profiles, real projects or secrets. No external research is needed.
The person authorizes this synthetic fix, exact publication, independent review, integration and explicit local update.
Use native calls and individual Locust commands, not a driver script. Preserve supplied tests and unrelated local work.
Do not create Git repositories, commit, push, change levels or allowances or broaden authority. Report actual failures.
Task {work['task']}; offer {work['offer']}; accepted seed revision {work['seed_revision']}.
Use {sys.executable} -B -m unittest discover -s . -v for the supplied five tests, after reading the files.
'''


def worker_prompt(role, daemon, work):
    return context(role, daemon, work) + f'''
1. Read locust_status, locust_goal_status, locust_task_show and locust_pending. Inspect your exact offer and authority.
2. Start task {work['task']} using offer {work['offer']} with locust_attempt_start; retain attempt/generation and report progress with locust_attempt_report.
3. Call locust_checkout_register with goal {daemon.goal}, a fresh random 32-character lowercase hex checkout ID, revision {work['seed_revision']}, task {work['task']} and your returned attempt. The daemon makes your own new folder. Bind your session with workspace bind --goal {daemon.goal} --checkout RETURNED_CHECKOUT_ID.
4. Read calculator.py, test_calculator.py and notes.md in that checkout. Run baseline tests, repair only calculator.py with the smallest change, then run all five tests there.
5. Freeze only calculator.py with workspace propose --goal {daemon.goal} --checkout RETURNED_CHECKOUT_ID --only --path calculator.py. Inspect the returned frozen preview; workspace publish --goal {daemon.goal} --operation OPERATION_ID without recapturing. Read that exact proposal using workspace review.
6. Publish a task report with locust_contribution_publish using your attempt/generation and sources=[EXACT_PROPOSAL_EVENT], artifacts=[]. The daemon derives its task from the attempt. The report is advisory and does not integrate files.
STOP after publication and reporting. Do not approve, integrate or update coordinator files. Report exact proposal and task report IDs.
'''


def review_prompt(role, daemon, work):
    return context(role, daemon, work) + f'''
You are a different principal from the worker. Task report {work['result']}; exact proposal {work['proposal']}; manifest {work['manifest']}.
1. Inspect shared status, task and both exact events using MCP. Confirm the worker's signed task report cites its proposal and attempt.
2. Run {role['wrapper']} workspace review --goal {daemon.goal} --proposal {work['proposal']} --destination {work['native_review']}.
3. Inspect the returned complete exact diff and materialized files. Require only calculator.py changed. Read the materialized calculator.py and test_calculator.py, then run the five tests in {work['native_review']}. The worker's prose is not independent test proof.
4. Inspect your own checkout {work['source']}: it must still contain the buggy calculator plus your dirty notes.md and unrelated.txt. Preserve them.
5. If the exact candidate is correct, record locust_review_record approve separately for proposal {work['proposal']} and task report {work['result']}, citing your inspection/tests.
6. Run workspace integrate --goal {daemon.goal} --proposal {work['proposal']} --expected-head {work['seed_revision']}, then scope select --goal {daemon.goal} --subject {work['result']}.
7. Inspect workspace head and workspace status --goal {daemon.goal} --checkout {work['source_checkout']}. Accepted head must name the proposal while your local base and bytes remain at the seed.
STOP BEFORE workspace update. Report the accepted revision and unchanged checkout. No local application during this turn.
'''


def update_prompt(role, daemon, work):
    return context(role, daemon, work) + f'''
Resume the same coordinator session. The harness independently verified accepted proposal {work['proposal']} at revision {work['revision']}; your checkout remains at the seed.
Reconcile locust_goal_status, then explicitly run {role['wrapper']} workspace update --goal {daemon.goal} --checkout {work['source_checkout']} --revision {work['revision']}.
Inspect the updated calculator and unchanged tests. Run the five tests in {work['source']}. Verify dirty notes.md still includes "Local coordinator notes in progress." and unrelated.txt is preserved.
Read workspace status for the checkout: base_revision must now equal {work['revision']}. Report actual results.
'''


def execute(role, daemon, args, prompt, label, report, resume=None):
    profile = role['profile']
    events, lifecycle = profile.logs / (label + '.mcp.jsonl'), profile.logs / (label + '.bridge.jsonl')
    private_write(events, '')
    private_write(lifecycle, '')
    observer = [str(ROOT / 'scripts/client_qualification/observe_mcp.py'), '--events-file', str(events),
        '--executable', str(daemon.binary), '--argument=mcp', '--argument=--lifecycle-receipt', '--argument=' + str(lifecycle)]
    command = [args.config_probe, '--client', role['client'], '--executable', sys.executable,
        '--locust-home', str(daemon.home), '--credential', str(role['credential']), '--session', str(role['session']),
        *['--argument=' + value for value in observer]]
    config = json.loads(subprocess.run(command, env=profile.environment(role['binary']), cwd=profile.workspace,
        capture_output=True, text=True, check=True, timeout=args.rpc_timeout).stdout)
    profile.apply(config)
    provider = role['provider']
    argv = provider.invocation(prompt, config['arguments'], resume=resume, policy='deliberately-permissive')
    private_write(profile.logs / (label + '.prompt.txt'), prompt)
    report['timeline'].append({'phase': label, 'event': 'started', 'utc': datetime.now(timezone.utc).isoformat()})
    save(args.output / 'report.json', report)
    print(json.dumps({'progress': label + '_started', 'model': provider.model}), flush=True)
    process = RedactingProcess(argv, provider.environment, profile.workspace, profile.logs, label,
                               args.rpc_timeout, [PROVIDERS[provider.provider][0]])
    def observe(child):
        for event in records(events):
            if event.get('event') == 'bridge_started':
                child.register_child(event.get('pid'), str(daemon.binary), lifecycle)
                child.register_child(event.get('observer_pid'), sys.executable, events)
    try:
        if args.farm_service:
            from client_qualification.live_farm import report_session
            report_session(daemon, role, 'started', 'Native client executing ' + label)
        run = await_agent(process, label, observe)
    finally:
        # Reporting may fail before await_agent installs its own cleanup guard.
        # Process.close is idempotent and targets only this owned process tree.
        process.close()
    if args.farm_service:
        report_session(daemon, role, 'exited', 'Native client exited after ' + label)
    run.update(client=role['client'], phase=label, principal=role['principal'], instance=role['instance'],
        model=provider.model, provider=provider.metadata, mcp_events=str(events), lifecycle_receipts=str(lifecycle),
        prompt=str(profile.logs / (label + '.prompt.txt')), native_calls=native_calls(role['client'], run['stdout']),
        resume_thread=resume, policy='deliberately-permissive', cli_wrapper=str(role['wrapper']))
    run['native_session'] = session_identifier(role['client'], run, profile)
    if role['client'] == 'codex':
        run.update(retain_codex_sessions(profile, label, provider.environment['OPENAI_API_KEY']))
        previous = next((item['accounting'] for item in reversed(report['runs']) if item['client'] == 'codex'), None)
        run['accounting'] = usage(run, provider.model, previous=previous)
    else:
        stream = records(run['stdout'], strict=True)
        result = next((item for item in reversed(stream) if item.get('type') == 'result'), {})
        selected = sorted({item['model'] for item in stream if item.get('type') == 'system' and item.get('model')} |
                          {item['message']['model'] for item in stream if item.get('message', {}).get('model')})
        run['native_selected_models'] = selected
        run['accounting'] = {'requested_model': provider.model, 'observed_models': selected,
            'raw_client_usage': result.get('usage', {}), 'raw_client_usage_scope': 'Claude invocation counters',
            'output_tokens': result.get('usage', {}).get('output_tokens'),
            'client_reported_cost_usd': result.get('total_cost_usd'), 'billing_evidence': 'not collected'}
    run['evidence_files'] = hash_evidence(run)
    report['runs'].append(run)
    report['timeline'].append({'phase': label, 'event': 'completed', 'utc': datetime.now(timezone.utc).isoformat()})
    save(args.output / 'report.json', report)
    require(run['exit_code'] == 0 and not run['timed_out'], 'Native client failed: ' + label)
    return run


def prepare(daemon, coordinator, worker, setup, output):
    seed_root = setup.workspace / 'seed'
    seed_root.mkdir(mode=0o700)
    for name, text in zip(FILES, (BASE_CODE, TEST_CODE, NOTES)):
        private_write(seed_root / name, text)
    completion = {'kind': 'reviews', 'by': {'kind': 'role', 'name': 'coordinator'}, 'count': 1, 'exclude_author': False}
    capture = daemon.call(['workspace', 'init', '--goal', daemon.goal, '--root', seed_root,
        '--completion', json.dumps(completion), '--publish', *[value for name in FILES for value in ('--path', name)]], owner=True)
    seed_proposal = capture['operation']['state']['recorded']['event']
    daemon.call(['review', 'record', '--goal', daemon.goal, '--subject', seed_proposal,
                 '--verdict', 'approve', 'Harness inspected the explicit synthetic starter tree'])
    daemon.call(['workspace', 'integrate', '--goal', daemon.goal, '--proposal', seed_proposal, '--expected-empty'])
    seed = daemon.call(['workspace', 'head', '--goal', daemon.goal])['head']
    source = coordinator['profile'].workspace / 'checkout'
    checkout = raw_call(daemon, ['--agent', coordinator['principal'], 'workspace', 'connect', '--goal', daemon.goal, '--folder', source], owner=True)['checkout']
    raw_call(daemon, ['workspace', 'bind', '--goal', daemon.goal, '--checkout', checkout['id']], role=coordinator)
    coordinator['profile'].workspace = source
    install_wrapper(coordinator, daemon)
    private_write(source / 'notes.md', DIRTY_NOTES)
    private_write(source / 'unrelated.txt', UNTRACKED)
    task = 'task:' + daemon.call(['task', 'open', '--goal', daemon.goal, '--inputs', json.dumps({'snapshot': seed['result_manifest']}),
        'Fix add(a,b) in calculator.py so the supplied five tests pass. Change only calculator.py.'])['recorded']['event']
    offer = daemon.call(['work', 'offer', '--goal', daemon.goal, '--task', task, '--member', worker['principal']])['recorded']['event']
    daemon.call(['--agent', worker['name'], 'allow', '--goal', daemon.goal, '--task', task], owner=True)
    baseline = tests(coordinator['profile'], source)
    require(baseline['exit_code'] != 0, 'Synthetic baseline unexpectedly passes')
    return {'goal': daemon.goal, 'task': task, 'offer': offer, 'seed_revision': seed['revision'],
        'seed_manifest': seed['result_manifest'], 'source': str(source), 'source_checkout': checkout['id'],
        'native_review': str(coordinator['profile'].root / 'review-candidate'), 'baseline_tests': baseline,
        'baseline_artifact': retain_tree(output, 'baseline', source)}


def campaign(daemon, coordinator, worker, args, report):
    work, checks = report['work'], report['assertions']
    wrun = execute(worker, daemon, args, worker_prompt(worker, daemon, work), 'worker', report)
    wevents = records(wrun['mcp_events'], strict=True)
    claims = [value['claimed'] for _, value in _calls(wevents, 'locust_attempt_start')
        if value.get('claimed', {}).get('task') == work['task'] and value['claimed'].get('instance') == worker['instance']]
    require(len(claims) == 1, 'Expected one exact worker MCP claim')
    claim = claims[0]
    registrations = [value['checkout'] for _, value in _calls(wevents, 'locust_checkout_register') if 'checkout' in value]
    own_checkouts = raw_call(daemon, ['checkouts', '--goal', daemon.goal], role=worker)['checkouts']
    worker_checkouts = [row for row in own_checkouts if row.get('task') == work['task']
        and row.get('attempt') == claim['attempt']
        and any(item.get('id') == row['id'] and item.get('root') == row['root'] for item in registrations)]
    require(len(worker_checkouts) == 1, 'Worker did not register one own daemon-created checkout for this attempt')
    work.update(destination=worker_checkouts[0]['root'], worker_checkout=worker_checkouts[0]['id'])
    task = daemon.call(['task', 'show', '--goal', daemon.goal, '--task', work['task']])['task']
    require(len(task['view']['contributions']) == 1, 'Expected one worker task report')
    work['result'] = task['view']['contributions'][0]
    report_event = daemon.call(['event', 'show', '--goal', daemon.goal, '--event', work['result']])['event']
    sources = report_event.get('body', {}).get('contribution_published', {}).get('sources', [])
    candidates = [candidate for candidate in daemon.call(['workspace', 'pending', '--goal', daemon.goal])['workspace_proposals']
                  if candidate['proposal'] in sources]
    require(len(candidates) == 1, 'Worker task report must cite one pending exact workspace proposal')
    candidate = candidates[0]
    work.update(proposal=candidate['proposal'], manifest=candidate['result_manifest'], attempt=claim['attempt'])
    proposal_event = daemon.call(['event', 'show', '--goal', daemon.goal, '--event', work['proposal']])['event']
    frozen = args.output / 'candidate'
    review = daemon.call(['workspace', 'review', '--goal', daemon.goal, '--proposal', work['proposal'], '--destination', frozen])
    report.update(worker_task_report=report_event, worker_proposal=proposal_event,
                  independent_review=review, candidate_artifact=str(frozen), candidate_tests=tests(coordinator['profile'], frozen))
    report_body = report_event.get('body', {}).get('contribution_published', {})
    checks['two_distinct_principals'] = worker['principal'] != coordinator['principal']
    checks['worker_signed_claim_proposal_and_report'] = (author(report_event, worker['principal'], 'contribution_published')
        and author(proposal_event, worker['principal'], 'workspace_proposed')
        and report_body.get('attempt') == claim['attempt'] and report_event.get('task') == work['task']
        and claim.get('generation') is not None and work['proposal'] in sources)
    checks['frozen_candidate_independently_tested'] = (report['candidate_tests']['exit_code'] == 0
        and (frozen / 'test_calculator.py').read_text() == TEST_CODE and (frozen / 'notes.md').read_text() == NOTES
        and (frozen / 'calculator.py').read_bytes() == (Path(work['destination']) / 'calculator.py').read_bytes()
        and review['proposal']['parent'] == work['seed_revision'] and len(review['changes']) == 1
        and review['changes'][0]['path'] == 'calculator.py' and not (frozen / '.git').exists())
    checks['worker_native_publication'] = native_event(wrun, 'workspace publish', work['proposal'])
    checks['worker_native_skill'] = skill_read(wrun['native_calls'], worker['skill']['path'])
    checks['worker_mcp_inspection_and_progress'] = all(_calls(wevents, name) for name in
        ('locust_status', 'locust_goal_status', 'locust_task_show', 'locust_pending', 'locust_attempt_report'))
    before = daemon.call(['workspace', 'head', '--goal', daemon.goal])['head']
    require(before['revision'] == work['seed_revision'] and task['view']['selected'] is None
            and coordinator_files(work['source'], BASE_CODE.encode()), 'Worker altered acceptance or coordinator files')
    require(all(checks.values()), 'Worker evidence failed; approval withheld')
    crun = execute(coordinator, daemon, args, review_prompt(coordinator, daemon, work), 'coordinator-review', report)
    accepted = daemon.call(['workspace', 'head', '--goal', daemon.goal])['head']
    work['revision'] = accepted['revision']
    state = raw_call(daemon, ['workspace', 'status', '--goal', daemon.goal, '--checkout', work['source_checkout']], role=coordinator)
    final_context = entries(coordinator, daemon)
    approval_subjects = {item['event']['body']['review_recorded']['subject'] for item in final_context
        if author(item['event'], coordinator['principal'], 'review_recorded')
        and item['event']['body']['review_recorded'].get('verdict') == 'approve'}
    decision = daemon.call(['event', 'show', '--goal', daemon.goal, '--event', work['revision']])['event']
    checks['coordinator_exact_native_review'] = exact_review_read(crun, review)
    checks['coordinator_signed_approval_and_integration'] = ({work['proposal'], work['result']} <= approval_subjects
        and author(decision, coordinator['principal'], 'scope_decided')
        and native_event(crun, 'workspace integrate', work['revision']))
    checks['accepted_before_local_update'] = (accepted['proposal'] == work['proposal']
        and accepted['result_manifest'] == work['manifest'] and state['checkout']['base_revision'] == work['seed_revision']
        and coordinator_files(work['source'], BASE_CODE.encode()))
    native_review = Path(work['native_review'])
    checks['coordinator_candidate_materialization_and_tests'] = (native_review.is_dir()
        and all((native_review / name).read_bytes() == (frozen / name).read_bytes() for name in FILES)
        and native_tests(crun, native_review))
    report.update(accepted_before_update={'head': accepted, 'checkout': state, 'integration_event': decision},
                  context_after_review=final_context)
    require(all(checks.values()), 'Coordinator review evidence failed; local update withheld')
    finalize_campaign(daemon, coordinator, args, report)


def finalize_campaign(daemon, coordinator, args, report):
    """Resume only local application after independently retained review evidence.

    Callers recovering a harness-only failure should write a new report, retain
    the original failure, and restart the same private daemon and native profile.
    No worker or completed coordinator-review turn is repeated here.
    """
    work, checks = report['work'], report['assertions']
    crun = next(run for run in reversed(report['runs']) if run['phase'] == 'coordinator-review')
    crun['cli_wrapper'] = str(coordinator['wrapper'])
    checks['coordinator_exact_native_review'] = exact_review_read(crun, report['independent_review'])
    require(all(checks.values()), 'Retained coordinator evidence failed; local update withheld')
    head = daemon.call(['workspace', 'head', '--goal', daemon.goal])['head']
    require(head['revision'] == work['revision'] and head['proposal'] == work['proposal']
            and coordinator_files(work['source'], BASE_CODE.encode()), 'Retained accepted revision or local preimage changed')
    frozen = Path(report['candidate_artifact'])
    require(crun['native_session'], 'Coordinator native session identity missing')
    arun = execute(coordinator, daemon, args, update_prompt(coordinator, daemon, work), 'coordinator-update', report,
                   resume=crun['native_session'])
    final_state = raw_call(daemon, ['workspace', 'status', '--goal', daemon.goal, '--checkout', work['source_checkout']], role=coordinator)
    final_task = daemon.call(['task', 'show', '--goal', daemon.goal, '--task', work['task']])['task']
    report.update(final_checkout=final_state, final_task=final_task,
        final_artifact=retain_tree(args.output, 'applied', Path(work['source'])), final_tests=tests(coordinator['profile'], Path(work['source'])))
    checks['same_native_coordinator_resumed'] = arun['native_session'] == crun['native_session'] and bool(
        _calls(records(arun['mcp_events'], strict=True), 'locust_goal_status'))
    checks['accepted_candidate_updated_and_dirty_work_preserved'] = (coordinator_files(work['source'], (frozen / 'calculator.py').read_bytes())
        and report['final_tests']['exit_code'] == 0 and final_state['checkout']['base_revision'] == work['revision']
        and final_state['checkout']['base_manifest'] == work['manifest']
        and bool(list(native_results(arun, 'workspace update'))))
    checks['task_completed_with_exact_report'] = final_task['view']['completed'] is True and final_task['view']['selected'] == work['result']
    checks['real_models_completed_and_cleaned_up'] = all(model_completion(run['client'], run['stdout'])
        and run['accounting'].get('output_tokens', 0) > 0 and run['model'] in run['native_selected_models']
        and run['cleanup_verified'] and run['natural_cleanup'] and not run['forced_cleanup'] for run in report['runs'])
    require(all(checks.values()), 'Final evidence assertions failed')


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--locust', required=True)
    parser.add_argument('--config-probe', required=True)
    parser.add_argument('--codex', required=True)
    parser.add_argument('--claude', required=True)
    parser.add_argument('--openai-model', required=True)
    parser.add_argument('--anthropic-model', required=True)
    parser.add_argument('--rpc-timeout', type=float, default=30)
    parser.add_argument('--farm-service')
    parser.add_argument('--wait-for-start', action='store_true')
    args = parser.parse_args(argv)
    if sys.version_info < (3, 12):
        parser.error('Use Python 3.12 or newer')
    if args.rpc_timeout <= 0:
        parser.error('--rpc-timeout must be positive')
    args.output = args.output.resolve()
    if args.output.exists() and any(args.output.iterdir()):
        parser.error('--output must be empty; prior attempts are never overwritten')
    args.output.mkdir(mode=0o700, parents=True, exist_ok=True)
    for name in ('locust', 'config_probe', 'codex', 'claude'):
        setattr(args, name, str(Path(getattr(args, name)).expanduser().resolve()))
    original_locust = args.locust
    pinned = args.output / 'locust'
    shutil.copy2(original_locust, pinned)
    pinned.chmod(0o700)
    args.locust = str(pinned)
    report = {'started': datetime.now(timezone.utc).isoformat(), 'passed': False, 'runs': [], 'timeline': [], 'assertions': {},
        'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        'source_dirty': bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True)),
        'binary_sha256': {name: sha(getattr(args, name)) for name in ('locust', 'config_probe', 'codex', 'claude')},
        'original_locust': original_locust, 'pinned_locust': args.locust, 'harness_sha256': sha(__file__),
        'execution_deadline': None, 'rpc_startup_cleanup_timeout_seconds': args.rpc_timeout,
        'boundary': 'One host, one real SQLite daemon, distinct principals and private native profiles; permissive tools, no OS filesystem isolation, no remote peer or packaged release claim'}
    profiles = []
    try:
        profiles = [Profile(args.output, name) for name in ('setup', 'coordinator', 'worker')]
        setup, cp, wp = profiles
        daemon_class = ProductionDaemon
        if args.farm_service:
            from client_qualification.live_farm import FarmDaemon, configure_farm, finish_farm
            daemon_class = FarmDaemon
        with daemon_class(setup, args.locust, args.rpc_timeout) as daemon:
            contract = daemon.call(['contract'])
            report['api_version'], report['protocol_version'] = contract['api_version'], contract['protocol_version']
            require((report['api_version'], report['protocol_version']) == (7, 7), 'Requires API 7 and protocol 7')
            coordinator = {'name': 'coordinator', 'profile': cp, 'principal': daemon.principal,
                'credential': daemon.credential, 'session': daemon.session, 'instance': daemon.instance,
                'client': 'codex', 'binary': args.codex}
            worker = enroll(daemon, wp, 'worker', level='ask')
            worker.update(client='claude-code', binary=args.claude)
            report['work'] = prepare(daemon, coordinator, worker, setup, args.output)
            report['principals'] = {role['name']: {'principal': role['principal'], 'instance': role['instance'],
                'client': role['client']} for role in (coordinator, worker)}
            report['goal'] = daemon.goal
            for role, model in ((coordinator, args.openai_model), (worker, args.anthropic_model)):
                role['provider'] = configure_real_provider(role['client'], role['profile'], role['binary'], model)
                role['skill'] = install_locust_skill(role['client'], role['profile'], ROOT / 'skills/locust/SKILL.md')
            if args.farm_service:
                farm = configure_farm(daemon, [dict(name=name, principal=role['principal'], client=role['client'])
                    for role, name in ((coordinator, 'Codex / Luna'), (worker, 'Claude Code / Haiku'))],
                    args.farm_service, 'Live Luna and Haiku workspace test')
                report['farm'] = farm
                save(args.output / 'farm.json', farm)
                print(json.dumps({'farm': farm}), flush=True)
            save(args.output / 'report.json', report)
            if args.wait_for_start:
                print(json.dumps({'progress': 'waiting_for_start', 'path': str(args.output / 'start-agents')}), flush=True)
                while not (args.output / 'start-agents').exists():
                    time.sleep(.2)
            campaign(daemon, coordinator, worker, args, report)
            if args.farm_service:
                report['final_farm_publication'] = finish_farm(daemon)
            report['passed'] = True
    except BaseException as error:
        report['error'] = type(error).__name__ + ': ' + str(error)
    finally:
        report['finished'] = datetime.now(timezone.utc).isoformat()
        if report['passed']:
            for profile in reversed(profiles):
                profile.close()
        report['private_profiles_removed'] = all(not profile.root.exists() for profile in profiles)
        report['retained_private_runtime'] = [str(profile.root) for profile in profiles if profile.root.exists()]
        save(args.output / 'report.json', report)
    print(json.dumps({'report': str(args.output / 'report.json'), 'passed': report['passed'],
        'assertions': report['assertions'], 'error': report.get('error')}), flush=True)
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
