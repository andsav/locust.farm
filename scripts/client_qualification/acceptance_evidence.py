"""Behavioral evidence for the real collaboration acceptance workflow.

Acknowledgment and declared sources establish inspectable provenance, not a
counterfactual claim that collaboration caused a benchmark improvement.
"""
import hashlib
import json
from pathlib import Path
import re

from . import acceptance_case as case
from . import collaboration_case as pilot
from .collaboration_evidence import _calls, _tree_matches, _native_publication
from .runtime import records


def thread_id(run):
    ids = {event.get('thread_id') for event in records(run['stdout'], strict=True)
           if event.get('type') == 'thread.started' and event.get('thread_id')}
    if len(ids) != 1:
        raise ValueError('Codex did not expose exactly one native thread identity')
    return ids.pop()


def usage(run, requested_model, previous=None):
    """Keep client accounting separate from provider billing and missing values."""
    if run.get('client') == 'merak':
        raw = run.get('response', {}).get('totals', {})
        phase = dict(raw)
        scope = 'Merak run totals'
        native_thread = None
        missing = {}
        cached = raw.get('cache_read_tokens') if raw.get('cache_read_tokens_reported') else None
        availability = raw.get('availability', {})
        if availability.get('cache_read_tokens') is True:
            cached = raw.get('cache_read_tokens')
        observed = raw.get('models') or ([raw['model']] if raw.get('model') else [])
    else:
        rows = [event.get('usage', {}) for event in records(run['stdout'], strict=True)
                if event.get('type') == 'turn.completed']
        # Native Codex exec resume exposes cumulative thread counters, not
        # isolated invocation totals. Never add resumed cumulative receipts.
        raw = rows[-1] if rows else {}
        scope = 'Codex cumulative native thread counters'
        try:
            native_thread = thread_id(run)
            identity_error = None
        except ValueError as error:
            native_thread, identity_error = None, str(error)
        phase, missing = {}, {}
        prior = previous.get('raw_client_usage', {}) if previous else {}
        same_thread = previous and previous.get('native_thread') == native_thread
        for key, value in raw.items():
            if not isinstance(value, int):
                continue
            if identity_error:
                phase[key], missing[key] = None, identity_error
            elif previous and not same_thread:
                phase[key], missing[key] = None, 'previous receipt belongs to another native thread'
            elif run.get('resume_thread') and previous is None:
                phase[key], missing[key] = None, 'resumed thread has no retained preceding counter'
            elif previous and not isinstance(prior.get(key), int):
                phase[key], missing[key] = None, 'preceding cumulative category unavailable'
            elif previous and value < prior[key]:
                phase[key], missing[key] = None, 'cumulative counter decreased'
            else:
                phase[key] = value - prior.get(key, 0)
        cached = phase.get('cached_input_tokens')
        observed = run.get('native_selected_models', [])
    total = phase.get('input_tokens')
    uncached = total - cached if isinstance(total, int) and isinstance(cached, int) and 0 <= cached <= total else None
    return {'requested_model': requested_model, 'observed_models': observed,
            'raw_client_usage': raw, 'input_tokens': total,
            'raw_client_usage_scope': scope, 'native_thread': native_thread,
            'phase_usage': phase, 'phase_usage_unavailable': missing,
            'cached_input_tokens': cached, 'uncached_input_tokens': uncached,
            'output_tokens': phase.get('output_tokens'),
            'reported_cost_usd': raw.get('reported_cost_usd'),
            'client_cost_usd': raw.get('cost_usd', raw.get('cost')),
            'client_cost_micros': raw.get('cost_micros'),
            'client_cost_micros_reported': raw.get('cost_micros_reported'),
            'cost_source': raw.get('cost_source'), 'billed_cost_usd': None,
            'billing_evidence': 'not collected', 'elapsed_ms': run.get('elapsed_ms')}


def hash_evidence(run):
    result = {}
    for name in ('stdout', 'stderr', 'events', 'transcript', 'mcp_events', 'prompt'):
        path = Path(run[name]) if run.get(name) else None
        if path and path.is_file() and not path.is_symlink():
            result[name] = {'path': str(path), 'bytes': path.stat().st_size,
                            'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
    return result


def denial(run):
    events = records(run['mcp_events'], strict=True)
    requests = {event.get('id'): event for event in events
                if event.get('direction') == 'client_request' and event.get('tool') == 'locust_attempt_start'}
    found = [dict(request=requests[event['id']], response=event)
            for event in events if event.get('direction') == 'bridge_response'
            and event.get('id') in requests
            and event.get('result', {}).get('ok') is False
            and event.get('result', {}).get('error', {}).get('code') == 'level_required']
    for call in run.get('native_calls', []):
        if not re.search(r'\battempt\s+start\b', call.get('arguments', {}).get('command', '')):
            continue
        try:
            envelope = json.loads(call.get('output', ''))
        except (ValueError, TypeError):
            continue
        if envelope.get('ok') is False and envelope.get('error', {}).get('code') == 'level_required':
            found.append({'native_call': call, 'response': envelope})
    return found


def ask_level_block(run, goal, person):
    """An ask-level task is a block even without a denied start.

An agent should stop when its abilities already show that it needs to ask.
Do not require it to attempt work it knows it cannot take yet.
"""
    def blocked(status):
        if status.get('goal') != goal:
            return False
        return any(item.get('agent') == person.get('principal') and item.get('level') == 'ask'
                   and not item.get('allowed_tasks') for item in status.get('abilities', []))

    found = []
    for request, response in _calls(records(run['mcp_events'], strict=True), 'locust_goal_status'):
        status = response.get('goal_status', {})
        if blocked(status):
            found.append({'request': request, 'goal_status': status})
    for call in run.get('native_calls', []):
        if call.get('success') is not True or not re.search(r'\bgoal\s+status\b', call.get('arguments', {}).get('command', '')):
            continue
        # A shell command can contain several help/read operations. Each JSON
        # line is retained and validated independently; echoed prose is ignored.
        for line in str(call.get('output', '')).splitlines():
            try:
                envelope = json.loads(line)
            except ValueError:
                continue
            status = envelope.get('result', {}).get('goal_status', {})
            if envelope.get('ok') is True and blocked(status):
                found.append({'native_call': call, 'goal_status': status})
    return found


def successful_start(run, person):
    for _, response in _calls(records(run['mcp_events'], strict=True), 'locust_attempt_start'):
        if response.get('claimed', {}).get('instance') == person.get('instance'):
            return True
    for call in run.get('native_calls', []):
        if call.get('success') is not True or not re.search(r'\battempt\s+start\b', call.get('arguments', {}).get('command', '')):
            continue
        try:
            envelope = json.loads(call.get('output', ''))
        except (ValueError, TypeError):
            continue
        if (envelope.get('ok') is True
                and envelope.get('result', {}).get('claimed', {}).get('instance') == person.get('instance')):
            return True
    return False


def finding_receipts(run, findings, goal, person):
    """Only exact delivered text and an accepted opaque receipt can qualify."""
    events = records(run['mcp_events'], strict=True)
    known = {item['event']['view']['event']: item['event']['text'] for item in findings}
    delivered = []
    for request, response in _calls(events, 'locust_context_read'):
        page = response.get('context', {})
        receipt = page.get('receipt')
        if request.get('arguments', {}).get('goal') != goal or not isinstance(receipt, str) or not receipt:
            continue
        for item in page.get('items', []):
            event = item.get('event', {})
            ident = event.get('view', {}).get('event')
            if ident in known and item.get('text_complete') is True and event.get('text') == known[ident]:
                delivered.append((ident, receipt))
    acknowledged = set()
    for request, response in _calls(events, 'locust_context_acknowledge'):
        args = request.get('arguments', {})
        ack = response.get('context_acknowledged', {})
        if (args.get('goal') != goal or ack.get('goal') != goal
                or ack.get('principal') != person.get('principal') or ack.get('session') != person.get('instance')):
            continue
        for ident, receipt in delivered:
            if args.get('receipt') == receipt and any(entry.get('event') == ident and entry.get('version')
                    for entry in ack.get('entries', [])):
                acknowledged.add(ident)
    return delivered, acknowledged


def review_identity(review):
    """Bind exact candidate/diff while allowing destination and later approval.

    Review copies and native review stdout name different local destinations.
    Approval evidence can advance after the peer reads the same candidate.
    These fields do not change immutable content identity or authenticated diff.
    """
    proposal = dict(review.get('proposal', {}))
    for field in ('approved', 'evidence'):
        proposal.pop(field, None)
    return {'proposal': proposal, 'review_mode': review.get('review_mode'),
            'changes': review.get('changes')}


def peer_review_read(run, proposal, expected):
    """Bind completed native stdout to independently authenticated tree review data."""
    if not run.get('transcript'):
        return False
    transcript = json.loads(Path(run['transcript']).read_text())
    # Transcript pagination can omit unrelated effects. The retained review
    # effect must itself be complete and equal the authenticated full review.
    decoder = json.JSONDecoder()
    for turn in transcript.get('turns', []):
        for effect in turn.get('effects', []):
            outcome = effect.get('outcome', '')
            if (effect.get('tool') != 'exec.run' or effect.get('status') != 'completed'
                    or effect.get('tool_truncated') or 'workspace review ' not in outcome
                    or 'exit_code: 0,' not in outcome or 'success: true' not in outcome
                    or 'stdout_truncated: false' not in outcome or ', stdout: ' not in outcome):
                continue
            stdout = outcome.split(', stdout: ', 1)[1]
            for match in re.finditer(r'\{', stdout):
                try:
                    value, _ = decoder.raw_decode(stdout[match.start():])
                except ValueError:
                    continue
                result = value.get('result') if isinstance(value, dict) and value.get('ok') is True else None
                if (isinstance(result, dict) and result.get('proposal', {}).get('proposal') == proposal
                        and review_identity(result) == review_identity(expected) and any(change.get('path') == 'safe_member.py'
                                                      for change in result.get('changes', []))):
                    return True
    return False


def evaluate(report):
    by_phase = {run.get('phase'): run for run in report.get('runs', [])}
    phases = ('ask', 'initial', 'research', 'revision', 'review')
    if any(phase not in by_phase for phase in phases):
        return {'workflow_completed': False}
    person = report.get('principals', {}).get('builder', {})
    researcher = report.get('principals', {}).get('researcher', {})
    findings = report.get('research_findings', [])
    ids = {item.get('event', {}).get('view', {}).get('event') for item in findings}
    ids.discard(None)
    valid_findings = [item for item in findings if
        item.get('event', {}).get('view', {}).get('author') == researcher.get('principal')
        and item.get('event', {}).get('view', {}).get('standing') == 'effective'
        and item.get('event', {}).get('view', {}).get('kind') == 'contribution_published'
        and item.get('event', {}).get('task') is None
        and '.archive-index' in (item.get('event', {}).get('text') or '').lower()]
    delivered, acknowledged = finding_receipts(by_phase['revision'], valid_findings, report.get('goal'), person)
    final = report.get('final_contribution', {}).get('event', {})
    body = final.get('body', {}).get('contribution_published', {})
    review = report.get('independent_tree_review', {})
    proposal_event = report.get('final_proposal', {}).get('event', {})
    proposal_body = proposal_event.get('body', {}).get('workspace_proposed', {})
    proposal = proposal_event.get('view', {}).get('event')
    # An explicit direct source declaration is stronger than an unrelated task's
    # prose citation. The API inspector is separately retained for review.
    direct_sources = set(body.get('sources', []))
    inspection = report.get('source_inspection', {}).get('contribution_inspected', {})
    inspected = inspection.get('contribution', {})
    inspected_sources = {entry.get('event') for entry in inspection.get('declared_sources', [])
        if (entry.get('detail') or {}).get('view', {}).get('author') == researcher.get('principal')
        and (entry.get('detail') or {}).get('view', {}).get('standing') == 'effective'}
    final_source = bool(direct_sources & acknowledged & ids & inspected_sources)
    final_source &= inspected == final
    final_source &= bool(body.get('attempt') and final.get('task') == report.get('task'))
    final_source &= (inspection.get('attempt') or {}).get('event') == body.get('attempt')
    final_source &= bool(proposal and proposal in direct_sources)
    peer = [item for item in report.get('final_context', []) if
            item.get('event', {}).get('view', {}).get('kind') == 'review_recorded'
            and item.get('event', {}).get('view', {}).get('author') == researcher.get('principal')
            and item.get('event', {}).get('view', {}).get('standing') == 'effective'
            and item.get('event', {}).get('body', {}).get('review_recorded', {}).get('subject') == proposal
            and item.get('event', {}).get('body', {}).get('review_recorded', {}).get('verdict') == 'approve']
    initial = Path(report['initial_artifact'])
    revised = Path(report['revised_artifact'])
    applied = Path(report.get('applied_artifact', revised.parent / 'not-applied' / 'safe_member.py'))
    changed = initial.is_file() and revised.is_file() and initial.read_bytes() != revised.read_bytes()
    initial_event = report.get('initial_proposal', {}).get('event', {})
    initial_body = initial_event.get('body', {}).get('workspace_proposed', {})
    initial_review = report.get('initial_tree_review', {})
    initial_task = report.get('initial_contribution', {}).get('event', {})
    initial_task_body = initial_task.get('body', {}).get('contribution_published', {})
    initial_id = initial_event.get('view', {}).get('event')
    initial_public_implementation = (pilot.verify(initial.parent)['passed'] is True
        and initial_event.get('view', {}).get('author') == person.get('principal')
        and initial_event.get('view', {}).get('kind') == 'workspace_proposed'
        and initial_event.get('view', {}).get('standing') == 'effective'
        and bool(initial_task.get('task') == report.get('task') and initial_task_body.get('attempt'))
        and initial_id in initial_task_body.get('sources', [])
        and initial_id == initial_review.get('proposal', {}).get('proposal')
        and initial_body.get('parent') == report.get('base_revision')
        and initial_body.get('result_manifest') == initial_review.get('proposal', {}).get('result_manifest')
        and _tree_matches(initial_review, initial, report.get('base_revision'), report.get('initial_reviewed_artifact'))
        and _native_publication(by_phase['initial'].get('native_calls', []), initial_id))
    same_thread = len({thread_id(by_phase[phase]) for phase in ('ask', 'initial', 'revision')}) == 1
    native_commands = [call.get('arguments', {}).get('command', '')
                       for phase in ('ask', 'initial', 'revision')
                       for call in by_phase[phase].get('native_calls', [])]
    no_owner = all(not re.search(r'(^|\s)--owner(?:\s|$)|owner\.credential', command)
                   for command in native_commands)
    application = report.get('application', {})
    allowance = report.get('task_allowance', {})
    granted = allowance.get('result', {})
    person_allowed_task = (allowance.get('actor') == 'person-harness'
        and allowance.get('principal') == person.get('principal') and allowance.get('instance') == person.get('instance')
        and granted.get('agent') == person.get('principal') and granted.get('goal') == report.get('goal')
        and granted.get('task') == allowance.get('task') == report.get('task')
        and granted.get('allowed') is True)
    return {
        'ask_level_stop_then_same_agent_recovery': bool(ask_level_block(by_phase['ask'], report.get('goal'), person))
            and report.get('ask_artifact_unchanged') is True
            and not successful_start(by_phase['ask'], person)
            and same_thread and successful_start(by_phase['initial'], person)
            and person_allowed_task and no_owner,
        'builder_work_preceded_private_finding': report.get('initial_context_has_research_finding') is False
            and by_phase['initial'].get('completed_sequence', 0) < by_phase['research'].get('started_sequence', 0)
            and initial_public_implementation,
        'attributed_finding_read_and_acknowledged': bool(valid_findings) and bool(delivered) and bool(acknowledged),
        'direct_source_provenance_declared': final_source,
        'artifact_changed_and_private_behavior_recovered': changed and case.verify(initial.parent)['passed'] is False
            and case.verify(revised.parent)['passed'] is True,
        'submitted_tree_matches_revised_artifact': proposal_event.get('view', {}).get('author') == person.get('principal')
            and proposal_event.get('view', {}).get('kind') == 'workspace_proposed'
            and proposal_event.get('view', {}).get('standing') == 'effective'
            and proposal == review.get('proposal', {}).get('proposal')
            and proposal_body.get('parent') == report.get('base_revision')
            and proposal_body.get('result_manifest') == review.get('proposal', {}).get('result_manifest')
            and _tree_matches(review, revised, report.get('base_revision'), report.get('reviewed_artifact'))
            and _native_publication(by_phase['revision'].get('native_calls', []), proposal),
        'real_peer_reviewed_and_approved_exact_submission': bool(peer)
            and peer_review_read(by_phase['review'], proposal or '<missing>', review),
        'explicit_application_independently_verified': application.get('actor') == 'person-harness'
            and application.get('subject') == proposal
            and application.get('result_manifest') == proposal_body.get('result_manifest')
            and bool(application.get('integration')) and bool(application.get('revision'))
            and bool(application.get('result')) and applied.is_file() and revised.is_file()
            and applied.read_bytes() == revised.read_bytes() and case.verify(applied.parent)['passed'] is True
            and application.get('distinct_workspace') is True and application.get('dirty_work_preserved') is True
            and application.get('ordinary_directory') is True,
        'real_model_runs_completed_and_cleaned_up': all(run.get('exit_code') == 0
            and run.get('natural_cleanup') is True and run.get('cleanup_verified') is True
            and isinstance(run.get('accounting', {}).get('output_tokens'), int)
            and run['accounting']['output_tokens'] > 0 for run in by_phase.values()),
    }
