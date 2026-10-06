#!/usr/bin/env python3
"""Exercise real Merak/Codex collaboration, recovery, review and local application.

Uses the pilot's private profiles, production daemon and client runners. No model
execution deadline or token/tool ceiling is imposed. RPC/startup/cleanup timeouts
are explicit. Provider billing is not inferred from client accounting.
"""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import subprocess
import sys

from check_shared_context_models import (ROOT, codex_run, common, enroll, entries,
                                        checkout_role, seed_workspace, review_tree, merak_run, raw_call, save, sha)
from client_qualification import acceptance_case as case
from client_qualification.acceptance_evidence import (denial, evaluate, hash_evidence, permission_block,
                                                     thread_id, usage)
from client_qualification.production import ProductionDaemon
from client_qualification.real_models import install_locust_skill, provider_model_ids
from client_qualification.runtime import Profile, private_write


INITIAL_OBJECTIVE = """Implement the archive member path helper from the public project brief. Inspect the current project, decide the smallest useful implementation supported by the requirements you have, and freeze and publish your result as an exact workspace proposal, then publish a task contribution citing its event ID. A collaborator is separately investigating deployment behavior; their findings are not yet available. Report any missing local permission to the person and stop the turn so they can resolve it. Preserve unrelated work. Do not broaden your own permissions."""

REVISION_OBJECTIVE = """Your collaborator has now published their deployment investigation in the shared Locust goal. Assess the new evidence against your current implementation and revise the implementation when warranted. Publish a new exact workspace proposal, then a task contribution with advisory source references to that proposal and the findings you used, so another participant can inspect its provenance. Workspace proposal sources may contain only workspace proposals. Preserve unrelated work; capture only safe_member.py. Report what changed and why."""


def snapshot(output, label, workspace):
    destination = output / label
    destination.mkdir(mode=0o700)
    for name in ('README.md', 'safe_member.py'):
        private_write(destination / name, (workspace / name).read_bytes())
    return str(destination / 'safe_member.py')


def run_phase(report, output, role, daemon, args, phase, objective, runner, resume=None, base=None):
    """Checkpoint before and after every paid invocation, including failures."""
    sequence = len(report['timeline']) + 1
    report['timeline'].append({'sequence': sequence, 'phase': phase, 'event': 'started',
                               'utc': datetime.now(timezone.utc).isoformat()})
    save(output / 'report.json', report)
    print(json.dumps({'progress': phase + '_started', 'model': args.model}), flush=True)
    skill = report['skill_paths'][role['name']]
    prompt = common(role, daemon, skill, base) + objective
    if runner is codex_run:
        result = runner(role, daemon, args, prompt, phase, resume=resume)
    else:
        result = runner(role, daemon, args, prompt, phase)
    result.update(phase=phase, principal=role['principal'], instance=role['instance'],
                  requested_model=args.model, resume_thread=resume,
                  started_sequence=sequence, completed_sequence=len(report['timeline']) + 1,
                  prompt=str(role['profile'].logs / (phase + '.prompt.txt')))
    previous = next((run['accounting'] for run in reversed(report['runs']) if run.get('client') == 'codex'), None)
    result['accounting'] = usage(result, args.model, previous=previous if result.get('client') == 'codex' else None)
    result['evidence_files'] = hash_evidence(result)
    report['runs'].append(result)
    report['timeline'].append({'sequence': result['completed_sequence'], 'phase': phase,
                              'event': 'completed', 'utc': datetime.now(timezone.utc).isoformat()})
    save(output / 'report.json', report)
    if result.get('exit_code') != 0:
        raise RuntimeError('Real client failed in ' + phase + '; retained exact stdout/stderr')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--locust', default=str(ROOT / 'target/debug/locust'))
    parser.add_argument('--merak', required=True)
    parser.add_argument('--codex', default='/opt/homebrew/bin/codex')
    parser.add_argument('--config-probe', default=str(ROOT / 'target/debug/examples/config_probe'))
    parser.add_argument('--model', default='gpt-6-luna')
    parser.add_argument('--rpc-timeout', type=float, default=30,
                        help='Per-RPC/startup/cleanup seconds; never a model execution deadline')
    args = parser.parse_args()
    if sys.version_info < (3, 12):
        parser.error('Use Python 3.12 or newer')
    for name in ('locust', 'merak', 'codex', 'config_probe'):
        setattr(args, name, str(Path(getattr(args, name)).expanduser().resolve()))
    output = args.output.resolve()
    if output.exists() and any(output.iterdir()):
        parser.error('--output must be empty; prior attempts and failures are never overwritten')
    output.mkdir(mode=0o700, parents=True, exist_ok=True)
    report = {'started': datetime.now(timezone.utc).isoformat(), 'model': args.model,
              'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'source_dirty': bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True)),
              'binary_sha256': {name: sha(getattr(args, name)) for name in ('locust', 'merak', 'codex', 'config_probe')},
              'harness_sha256': {str(path.relative_to(ROOT)): sha(path) for path in (
                  Path(__file__), ROOT / 'scripts/check_shared_context_models.py',
                  ROOT / 'scripts/client_qualification/acceptance_case.py',
                  ROOT / 'scripts/client_qualification/acceptance_evidence.py',
                  ROOT / 'scripts/client_qualification/collaboration_case.py',
                  ROOT / 'scripts/client_qualification/collaboration_evidence.py',
                  ROOT / 'scripts/client_qualification/runtime.py',
                  ROOT / 'scripts/client_qualification/real_models.py',
                  ROOT / 'scripts/client_qualification/production.py')},
              'fixture_environment': {'temporary_profiles': 'Owned private /tmp directories; no ambient client profiles copied',
                  'public_python_minimum': '3.12', 'supplied_python': str(Path(sys.executable).resolve()),
                  'cli_guidance': './locust-scoped; default assigned cwd',
                  'native_codex_evidence': 'Per-phase private session snapshots, provider key redacted before retention'},
              'execution_deadline': None, 'rpc_startup_cleanup_timeout_seconds': args.rpc_timeout,
              'boundary': 'One host, production SQLite daemon, private client profiles and separate role workspaces; permissive native tools, no OS filesystem isolation; behavior evidence only',
              'causal_claim': 'No paired ablation or benchmark improvement claim',
              'runs': [], 'timeline': [], 'assertions': {}, 'passed': False}
    profiles = []
    try:
        report['client_versions'] = {name: subprocess.check_output([getattr(args, name), '--version'], text=True).strip()
                                     for name in ('merak', 'codex', 'locust')}
        contract = json.loads(subprocess.check_output([args.locust, '--json', 'contract'], text=True))['result']
        report['api_version'], report['protocol_version'] = contract['api_version'], contract['protocol_version']
        if (report['api_version'], report['protocol_version']) != (7, 6):
            raise RuntimeError('Acceptance workflow requires API 7 / protocol 6')
        report['model_available'] = args.model in provider_model_ids('openai', args.rpc_timeout)
        if not report['model_available']:
            raise RuntimeError('Selected model is absent from provider metadata')
        profiles = [Profile(output, name) for name in ('setup', 'researcher', 'builder', 'integration')]
        setup, rp, bp, ip = profiles
        fixture = case.seed(rp.workspace, bp.workspace)
        report['case'] = fixture
        report['input_sha256'] = {name: sha(bp.workspace / name) for name in fixture['builder_files']}
        private_fixture = Path(fixture['researcher_only']['private_fixture'])
        report['input_sha256']['private_operational_note'] = sha(private_fixture)
        private_write(output / 'harness-only-input' / 'archive-operations.md', private_fixture.read_bytes())
        with ProductionDaemon(setup, args.locust, args.rpc_timeout) as daemon:
            formation = raw_call(daemon, ['formation', 'example', 'peer-review'])
            report['review_policy'] = formation['decisions']['completion']
            daemon.goal = raw_call(daemon, ['--as', daemon.principal, 'goal', 'create', '--title', 'Portable archive member paths',
                '--formation-json', json.dumps(formation)], owner=True)['goal_created']['goal']
            researcher = enroll(daemon, rp, 'researcher')
            builder = enroll(daemon, bp, 'builder', permissions=('contribute', 'review'))
            integrator = enroll(daemon, ip, 'integrator', permissions=('contribute',))
            for role in (researcher, builder):
                role['python_executable'] = str(Path(sys.executable).resolve())
            report['goal'] = daemon.goal
            report['principals'] = {r['name']: {'principal': r['principal'], 'instance': r['instance']}
                                    for r in (researcher, builder, integrator)}
            report['initial_permissions'] = raw_call(daemon, ['permission', 'inspect', '--goal', daemon.goal,
                                                           '--agent', builder['name']], owner=True)
            rskill = rp.workspace / 'LOCUST_SKILL.md'
            private_write(rskill, (ROOT / 'skills/locust/SKILL.md').read_bytes())
            bskill = install_locust_skill('codex', bp, ROOT / 'skills/locust/SKILL.md')['path']
            report['skill_paths'] = {'researcher': str(rskill), 'builder': bskill}
            report['skill_sha256'] = sha(rskill)
            seed = seed_workspace(daemon, bp.workspace, fixture['builder_files'],
                                  completion=report['review_policy'], reviewer=researcher)
            bound = checkout_role(builder, daemon, seed['revision'])
            integration_bound = checkout_role(integrator, daemon, seed['revision'])
            base = seed['result_manifest']
            report['base'] = base
            report['base_revision'] = seed['revision']
            report['checkout'] = bound
            report['integration_checkout'] = integration_bound
            private_write(bp.workspace / 'unrelated.txt', 'builder local work\n')
            private_write(ip.workspace / 'README.md', (ip.workspace / 'README.md').read_text()
                          + '\nLocal integration notes in progress.\n')
            private_write(ip.workspace / 'unrelated.txt', 'integration local work\n')
            integration_dirty = {name: (ip.workspace / name).read_bytes() for name in ('README.md', 'unrelated.txt')}
            report['baseline'] = case.verify(bp.workspace)
            if report['baseline']['passed']:
                raise RuntimeError('Starter unexpectedly passes private oracle')
            private = fixture['researcher_only']['rule_summary']
            public = INITIAL_OBJECTIVE + common(builder, daemon, bskill, base) + ''.join(
                (bp.workspace / name).read_text() for name in fixture['builder_files'])
            if private in public or 'assets\\..\\outside.txt' in public or '.archive-index' in public:
                raise RuntimeError('Private constraint leaked into builder setup')
            denied = run_phase(report, output, builder, daemon, args, 'permission', INITIAL_OBJECTIVE,
                               codex_run, base=base)
            report['permission_block'] = permission_block(denied, daemon.goal, report['principals']['builder'])
            report['permission_artifact_unchanged'] = sha(bp.workspace / 'safe_member.py') == report['input_sha256']['safe_member.py']
            if not report['permission_block'] or not report['permission_artifact_unchanged']:
                raise RuntimeError('Builder did not inspect missing execution authority and stop before implementation')
            report['permission_denials'] = denial(denied)
            thread = thread_id(denied)
            report['permission_grant'] = {'actor': 'person-harness', 'permission': 'execute',
                'principal': builder['principal'], 'instance': builder['instance'],
                'result': raw_call(daemon, ['permission', 'allow', '--goal', daemon.goal,
                                          '--agent', builder['name'], 'execute'], owner=True)}
            run_phase(report, output, builder, daemon, args, 'initial',
                'The person has granted local execution permission. Reconcile current Locust state and continue the implementation you began.\n' + INITIAL_OBJECTIVE,
                codex_run, resume=thread, base=base)
            report['initial_artifact'] = snapshot(output, 'initial-artifact', bp.workspace)
            report['initial_verification'] = case.verify(Path(report['initial_artifact']).parent)
            before = entries(builder, daemon)
            save(output / 'context-before-research.json', before)
            initial_submissions = [item for item in before if item['event']['view']['author'] == builder['principal']
                and item['event']['view']['kind'] == 'workspace_proposed']
            if not initial_submissions:
                raise RuntimeError('Builder did not publish its initial exact workspace proposal')
            report['initial_proposal'] = initial_submissions[-1]
            initial_id = initial_submissions[-1]['event']['view']['event']
            report['initial_tree_review'], report['initial_reviewed_artifact'] = review_tree(
                daemon, initial_id, builder, output, 'initial-review')
            initial_reports = [item for item in before if item['event']['view']['author'] == builder['principal']
                and item['event']['view']['kind'] == 'contribution_published'
                and initial_id in item['event']['body']['contribution_published'].get('sources', [])]
            if not initial_reports:
                raise RuntimeError('Builder task report did not cite its exact initial workspace proposal')
            report['initial_contribution'] = initial_reports[-1]
            report['initial_context_has_research_finding'] = any(
                item['event']['view']['author'] == researcher['principal']
                and item['event']['view']['kind'] == 'contribution_published' for item in before)
            run_phase(report, output, researcher, daemon, args, 'research', fixture['prompts']['researcher'], merak_run)
            report['research_findings'] = [item for item in entries(builder, daemon)
                if item['event']['view']['author'] == researcher['principal']
                and item['event']['view']['kind'] == 'contribution_published']
            if not report['research_findings']:
                raise RuntimeError('Researcher did not publish a signed finding')
            previous = {item['event']['view']['event'] for item in before}
            run_phase(report, output, builder, daemon, args, 'revision', REVISION_OBJECTIVE,
                      codex_run, resume=thread, base=base)
            report['revised_artifact'] = snapshot(output, 'revised-artifact', bp.workspace)
            after = entries(builder, daemon)
            proposals = [item for item in after if item['event']['view']['author'] == builder['principal']
                and item['event']['view']['kind'] == 'workspace_proposed'
                and item['event']['view']['event'] not in previous]
            if not proposals:
                raise RuntimeError('Builder did not publish a new final workspace proposal')
            report['final_proposal'] = proposals[-1]
            proposal = proposals[-1]['event']['view']['event']
            submissions = [item for item in after if item['event']['view']['author'] == builder['principal']
                and item['event']['view']['kind'] == 'contribution_published'
                and item['event']['view']['event'] not in previous
                and proposal in item['event']['body']['contribution_published'].get('sources', [])]
            if not submissions:
                raise RuntimeError('Builder task report did not cite its exact final workspace proposal')
            report['final_contribution'] = submissions[-1]
            subject = submissions[-1]['event']['view']['event']
            report['independent_tree_review'], report['reviewed_artifact'] = review_tree(
                daemon, proposal, researcher, output, 'final-review')
            report['source_inspection'] = raw_call(daemon, ['contribution', 'inspect', '--goal', daemon.goal,
                                                          '--contribution', subject], role=researcher)
            review_objective = f'''Review workspace proposal {proposal} and its task report {subject} through Locust. Run workspace review --goal {daemon.goal} --proposal {proposal} and inspect the actual before/after diff and the report's declared sources against the deployment requirement you investigated. Record a signed Locust review of this exact workspace proposal with your justified approve or reject verdict. Do not modify collaborator workspaces, integrate or update. A local person will decide integration after your review.'''
            run_phase(report, output, researcher, daemon, args, 'review', review_objective, merak_run)
            report['final_context'] = entries(builder, daemon)
            approvals = [item for item in report['final_context'] if
                item['event']['view']['author'] == researcher['principal']
                and item['event']['view']['kind'] == 'review_recorded'
                and item['event']['view']['standing'] == 'effective'
                and item['event']['body']['review_recorded'].get('subject') == proposal
                and item['event']['body']['review_recorded'].get('verdict') == 'approve']
            if not approvals:
                raise RuntimeError('Real peer did not approve the exact final submission; local application withheld')
            integrated = raw_call(daemon, ['workspace', 'integrate', '--goal', daemon.goal,
                '--proposal', proposal, '--expected-head', seed['revision']])
            accepted = raw_call(daemon, ['workspace', 'head', '--goal', daemon.goal])['head']
            if accepted['proposal'] != proposal:
                raise RuntimeError('Integration did not retain the exact reviewed proposal')
            applied = raw_call(daemon, ['workspace', 'update', '--goal', daemon.goal,
                '--checkout', integration_bound['id'], '--revision', accepted['revision']], role=integrator)
            report['applied_artifact'] = snapshot(output, 'applied-artifact', ip.workspace)
            report['application'] = {'actor': 'person-harness', 'subject': proposal,
                'result_manifest': accepted['result_manifest'], 'integration': integrated,
                'revision': accepted['revision'], 'result': applied,
                'distinct_workspace': ip.workspace != bp.workspace,
                'ordinary_directory': not (ip.workspace / '.git').exists(),
                'dirty_work_preserved': all((ip.workspace / name).read_bytes() == content
                    for name, content in integration_dirty.items())}
            for name, content in integration_dirty.items():
                private_write(output / 'application-dirty-work' / name, content)
            report['builder_ordinary_directory'] = not (bp.workspace / '.git').exists()
            report['builder_unrelated_preserved'] = (bp.workspace / 'unrelated.txt').read_text() == 'builder local work\n'
            report['assertions'] = evaluate(report)
            report['assertions']['builder_existing_work_preserved'] = report['builder_ordinary_directory'] and report['builder_unrelated_preserved']
            report['passed'] = all(report['assertions'].values())
    except BaseException as error:
        report['error'] = type(error).__name__ + ': ' + str(error)
        report['passed'] = False
    finally:
        if not report['assertions']:
            try:
                report['assertions'] = evaluate(report)
            except (OSError, ValueError, KeyError, TypeError) as error:
                report['evidence_evaluation_error'] = type(error).__name__ + ': ' + str(error)
        report['finished'] = datetime.now(timezone.utc).isoformat()
        save(output / 'report.json', report)
        # If a client fails before returning structured metadata, retain its
        # process logs and hashes instead of losing the failed attempt.
        report['retained_files'] = [{'path': str(path), 'bytes': path.stat().st_size, 'sha256': sha(path)}
            for path in sorted(output.rglob('*')) if path.is_file() and not path.is_symlink() and path.name != 'report.json']
        for profile in profiles:
            profile.close()
        report['private_profiles_removed'] = all(not profile.root.exists() for profile in profiles)
        save(output / 'report.json', report)
    print(json.dumps({'report': str(output / 'report.json'), 'passed': report['passed'],
                      'assertions': report['assertions'], 'error': report.get('error')}), flush=True)
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
