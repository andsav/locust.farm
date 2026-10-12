#!/usr/bin/env python3
"""Read-only audit of literal internal labels on recorded model-facing surfaces.

No harness imports, network, daemon calls, candidate execution, or hidden-score
reads. Unsealed studies require --allow-partial and cannot receive a pass verdict.
The audit detects registered internal IDs, explicit treatment arm tokens, and
structured arm/prefix assignments; it is not proof against inferential unblinding.
Truthful author agent-N attribution, policy E/D/F, and opaque candidate handles
(candidate- plus 32 lowercase hexadecimal characters) are permitted.
"""
import argparse
from collections import Counter, defaultdict
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re

SCHEMA = 'shared-record-independent-model-label-audit.v1'
OPAQUE_CANDIDATE = re.compile(r'candidate-[0-9a-f]{32}\Z')
AUTHOR = re.compile(r'agent-[0-9]+\Z')
IDENTITY_KEYS = {'arm', 'trial_arm', 'prefix_kind', 'branch_kind', 'condition', 'treatment'}
PRIVATE_MARKER = 'Own evaluated artifacts:\n'
PACKET_MARKER = 'Frozen evidence:\n'


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()


def contains_marker(value, marker):
    if isinstance(value, str):
        return marker in value
    if isinstance(value, dict):
        return any(contains_marker(child, marker) for child in value.values())
    if isinstance(value, list):
        return any(contains_marker(child, marker) for child in value)
    return False


def pointer(key):
    return str(key).replace('~', '~0').replace('/', '~1')


class Scanner:
    def __init__(self, manifest, documents, ledger):
        self.tokens = defaultdict(set)
        self.arms = set(manifest['arms'])
        self.candidate_ids, self.opaque_ids = set(), set()
        self.add(manifest.get('study_id'), 'internal_study_id')
        for block in manifest['blocks']:
            self.add(block['id'], 'internal_block_id')
            for arm in self.arms | {'early', 'delayed'}:
                self.add(f'{block["id"]}/{arm}', 'internal_trial_id')
        for arm in self.arms:
            if len(arm) > 1:
                self.add(arm, 'treatment_arm_token')
        for row in ledger:
            self.add(row.get('label'), 'internal_agent_id')
        for document in documents:
            for agent in document.get('agents', []):
                self.add(agent.get('id'), 'internal_agent_id')
            candidates = document.get('candidates', []) + [
                candidate for agent in document.get('agents', [])
                for candidate in agent.get('candidates', [])]
            for candidate in candidates:
                ident = candidate['id']
                self.candidate_ids.add(ident)
                if OPAQUE_CANDIDATE.fullmatch(ident):
                    self.opaque_ids.add(ident)
                else:
                    self.add(ident, 'internal_candidate_id')
        alternatives = '|'.join(re.escape(token) for token in sorted(self.tokens, key=lambda s: (-len(s), s)))
        self.pattern = re.compile(r'(?<![A-Za-z0-9_-])(?:' + alternatives + r')(?![A-Za-z0-9_-])') if alternatives else None

    def add(self, token, kind):
        if isinstance(token, str) and token and not AUTHOR.fullmatch(token):
            self.tokens[token].add(kind)

    def scan(self, value):
        findings = []

        def walk(item, path, key=None, surface_part=None):
            if isinstance(item, dict):
                for child_key, child in item.items():
                    walk(str(child_key), path + '/' + pointer(child_key) + '#key', surface_part=surface_part)
                    walk(child, path + '/' + pointer(child_key), child_key, surface_part)
            elif isinstance(item, list):
                for index, child in enumerate(item):
                    walk(child, path + '/' + str(index), surface_part=surface_part)
            elif isinstance(item, str):
                part = surface_part
                if PRIVATE_MARKER in item:
                    part = 'private_review_serialization'
                elif PACKET_MARKER in item:
                    part = 'record_review_packet'
                decoded = None
                for marker in (PRIVATE_MARKER, PACKET_MARKER):
                    if marker in item:
                        before, after = item.split(marker, 1)
                        try:
                            decoded = json.loads(after)
                        except (ValueError, TypeError):
                            pass
                        if decoded is not None:
                            walk(before, path + '#preamble', surface_part=part)
                            walk(decoded, path + '#json', surface_part=part)
                            return
                try:
                    parsed = json.loads(item)
                    if isinstance(parsed, (dict, list, str)) and parsed != item:
                        walk(parsed, path + '#json', surface_part=part)
                        return
                except (ValueError, TypeError):
                    pass
                matches = [] if self.pattern is None else list(self.pattern.finditer(item))
                for match in matches:
                    token = match.group()
                    cues = sorted(set(re.findall(r'(?:^|/)(early|delayed|[CP]_[EDF])(?:/|$)', token)))
                    findings.append(dict(path=path or '/', literal=token,
                                         kinds=sorted(self.tokens[token]), cues=cues,
                                         offset=match.start(), value_sha256=digest(item.encode()),
                                         surface_part=part))
                if key in IDENTITY_KEYS and item in self.arms | {'early', 'delayed'} and not matches:
                    findings.append(dict(path=path or '/', literal=item,
                                         kinds=['structured_assignment_label'], cues=[item],
                                         offset=0, value_sha256=digest(item.encode()), surface_part=part))
        walk(value, '')
        return findings


class Audit:
    def __init__(self, study, allow_partial=False):
        self.study = Path(study).resolve()
        self.allow_partial = allow_partial
        self.files, self.errors, self.warnings = {}, [], []
        self.started = datetime.now(timezone.utc).isoformat()

    def read(self, name):
        path = (self.study / name).resolve()
        if not path.is_relative_to(self.study):
            raise ValueError('evidence path escapes study')
        raw = path.read_bytes()
        self.files[name] = dict(sha256=digest(raw), bytes=len(raw))
        return json.loads(raw)

    def run(self):
        sealed = (self.study / 'seal.json').is_file()
        if not sealed and not self.allow_partial:
            raise ValueError('unsealed study requires --allow-partial; it cannot receive a pass verdict')
        seal = self.read('seal.json') if sealed else None
        manifest = self.read('manifest.json')
        ledger = self.read('ledger.json')
        freeze = self.read('freeze.json') if (self.study / 'freeze.json').is_file() else None
        manifest_hash = self.files['manifest.json']['sha256']
        if freeze is None or freeze.get('manifest_sha256') != manifest_hash:
            self.errors.append('missing or mismatched manifest freeze receipt')
        if seal and seal.get('manifest_sha256') != manifest_hash:
            self.errors.append('manifest does not match seal')
        names = sorted(str(p.relative_to(self.study)) for pattern in ('prefixes/*.json', 'trials/*/*.json')
                       for p in self.study.glob(pattern))
        documents = [(name, self.read(name)) for name in names]
        required_trials = {f'trials/{block["id"]}/{arm}.json' for block in manifest['blocks'] for arm in manifest['arms']}
        required_prefixes = {f'prefixes/{block["id"]}-{kind}.json'
                             for block in manifest['blocks']
                             for kind in ({'delayed' if arm.get('policy') == 'D' else 'early'
                                           for arm in manifest['arms'].values() if arm['kind'] in ('C', 'P')})}
        missing_prefixes = sorted(required_prefixes - set(names))
        missing_trials = sorted(required_trials - set(names))
        if not ledger:
            self.errors.append('no archived generation requests')
        if sealed and missing_prefixes:
            self.errors.append('sealed study lacks assigned prefix files')
        for name, document in documents:
            if sealed and not document.get('final'):
                self.errors.append('sealed document is not final: ' + name)
            if not document.get('agents'):
                self.errors.append('no agents in archived document: ' + name)
            for agent in document.get('agents', []):
                if not {'history', 'requests', 'trace', 'candidates'}.issubset(agent):
                    self.errors.append('incomplete agent archive: ' + name)
        if sealed and missing_trials:
            self.errors.append('sealed study lacks assigned trial files')
        if sealed:
            for name, info in self.files.items():
                if name not in ('manifest.json', 'seal.json') and seal.get('files', {}).get(name) != info['sha256']:
                    self.errors.append('read evidence not bound to seal: ' + name)
        else:
            self.warnings.append('Partial live snapshot: files were read separately, not as an atomic study snapshot.')
        scanner = Scanner(manifest, [doc for _, doc in documents], ledger)
        requests, ledger_ids = {}, set()
        aliases = defaultdict(list)

        def add_request(row, origin, label=None, from_ledger=False):
            ident = row['id']
            if from_ledger:
                if ident in ledger_ids:
                    self.errors.append('duplicate ledger request identity: ' + ident)
                ledger_ids.add(ident)
            payload = row.get('payload')
            if not isinstance(payload, dict):
                self.errors.append('missing request payload: ' + origin)
                return
            key = (ident, digest(canonical(payload)))
            if key not in requests:
                requests[key] = dict(id=ident, payload=payload, payload_sha256=key[1], origins=[], labels=set(),
                                     ledger_status=None, response_confirmed=False, dispatch_failure=False)
            entry = requests[key]
            entry['origins'].append(origin)
            if label:
                entry['labels'].add(label)
            if from_ledger:
                entry['ledger_status'] = row.get('status')
            entry['response_confirmed'] |= isinstance(row.get('response'), dict)
            entry['dispatch_failure'] |= row.get('status') == 'failed'

        for index, row in enumerate(ledger):
            add_request(row, f'ledger.json#/{index}/payload', row.get('label'), True)
        for name, document in documents:
            if document.get('manifest_sha256') != manifest_hash:
                self.errors.append('document manifest mismatch: ' + name)
            for ai, agent in enumerate(document.get('agents', [])):
                for ri, request in enumerate(agent.get('requests', [])):
                    add_request(request, f'{name}#/agents/{ai}/requests/{ri}/payload', agent.get('id'))
        variants = defaultdict(set)
        for ident, payload_hash in requests:
            variants[ident].add(payload_hash)
        for ident, hashes in variants.items():
            if len(hashes) != 1:
                self.errors.append('different payload copies for request: ' + ident)
        findings, coverage = [], Counter()
        surfaces = []
        request_summary = []
        for (ident, _), entry in sorted(requests.items()):
            if entry['response_confirmed']:
                dispatch = 'response_confirmed'
            elif entry['dispatch_failure']:
                dispatch = 'dispatch_attempt_failed_or_unknown'
            else:
                dispatch = 'reservation_or_archive_only'
            surface_id = 'request:' + ident + ':' + entry['payload_sha256']
            matched = scanner.scan(entry['payload'])
            for finding in matched:
                findings.append(dict(surface_id=surface_id, surface='request_payload',
                                     request_id=ident, dispatch_evidence=dispatch, **finding))
            coverage['request_payloads'] += 1
            coverage['requests_' + dispatch] += 1
            coverage['request_findings_' + dispatch] += len(matched)
            request_summary.append(dict(id=ident, payload_sha256=entry['payload_sha256'],
                                        labels=sorted(entry['labels']), origins=entry['origins'],
                                        ledger_status=entry['ledger_status'], dispatch_evidence=dispatch,
                                        findings=len(matched)))
            for index, item in enumerate(entry['payload'].get('input', [])):
                aliases[digest(canonical(item))].append(dict(request_id=ident, input_index=index,
                                                           dispatch_evidence=dispatch))
        # Final history and trace are separate from dispatch: the last tool result
        # or private review can exist without a later model request.
        for name, document in documents:
            for ai, agent in enumerate(document.get('agents', [])):
                for hi, item in enumerate(agent.get('history', [])):
                    surface_id = f'{name}#/agents/{ai}/history/{hi}'
                    matched = scanner.scan(item)
                    history_hash = digest(canonical(item))
                    coverage['archived_history_items'] += 1
                    if contains_marker(item, PRIVATE_MARKER):
                        coverage['private_review_history_items'] += 1
                    if matched:
                        submitted = [row for row in aliases[history_hash]
                                     if row['request_id'] in agent.get('request_ids', [])]
                        surfaces.append(dict(id=surface_id, surface='archived_history', value_sha256=history_hash,
                                             matching_payload_items=submitted, findings=len(matched)))
                        for finding in matched:
                            findings.append(dict(surface_id=surface_id, surface='archived_history', **finding))
                for ti, event in enumerate(agent.get('trace', [])):
                    if event.get('event') != 'tool_results':
                        continue
                    for ri, pair in enumerate(event.get('results', [])):
                        if not isinstance(pair, list) or len(pair) != 2:
                            self.errors.append('malformed tool result pair: ' + name)
                            continue
                        surface_id = f'{name}#/agents/{ai}/trace/{ti}/results/{ri}/1'
                        matched = scanner.scan(pair[1])
                        coverage['archived_tool_results'] += 1
                        if matched:
                            surfaces.append(dict(id=surface_id, surface='tool_result', call_id=pair[0],
                                                 value_sha256=digest(canonical(pair[1])), findings=len(matched)))
                            for finding in matched:
                                findings.append(dict(surface_id=surface_id, surface='tool_result', **finding))
        changed = []
        for name, info in self.files.items():
            path = self.study / name
            if not path.is_file() or digest(path.read_bytes()) != info['sha256']:
                changed.append(name)
        names_after = sorted(str(p.relative_to(self.study)) for pattern in ('prefixes/*.json', 'trials/*/*.json')
                             for p in self.study.glob(pattern))
        appeared = sorted(set(names_after) - set(names))
        if changed or appeared:
            self.warnings.append('Study files changed during this read; only individually hashed bytes support findings.')
            if sealed:
                self.errors.append('sealed study changed during audit')
        counts = Counter(finding['surface'] for finding in findings)
        cue_counts = Counter(cue for finding in findings for cue in finding['cues'])
        kinds = Counter(kind for finding in findings for kind in finding['kinds'])
        status = ('leaks_found' if findings else 'invalid_evidence' if self.errors else
                  'pass' if sealed else 'no_leaks_found_in_partial_snapshot')
        return dict(schema=SCHEMA, status=status, study=str(self.study),
                    started_at=self.started, completed_at=datetime.now(timezone.utc).isoformat(),
                    diagnostic_sha256=digest(Path(__file__).read_bytes()),
                    manifest_sha256=manifest_hash, seal_sha256=self.files.get('seal.json', {}).get('sha256'),
                    synthetic=all(str(config.get('model', '')).startswith('scripted-') for config in manifest['models'].values()),
                    snapshot=dict(sealed=sealed, allow_partial=self.allow_partial,
                                  changed_during_read=changed, appeared_during_read=appeared,
                                  missing_assigned_trials=missing_trials, missing_assigned_prefixes=missing_prefixes,
                                  file_hashes=self.files),
                    allowed=dict(candidate_handle='candidate-[0-9a-f]{32}', author='agent-[0-9]+',
                                 record_policy=['E', 'D', 'F']),
                    detection_scope=['Exact registered internal candidate, agent, trial, block and study IDs.',
                                     'Standalone multicharacter arm labels and explicit structured arm/prefix assignments.',
                                     'JSON embedded in tool output, private-review prose and record packets is decoded.',
                                     'Incidental prose words early/delayed and one-letter arm characters are not forbidden.',
                                     'No claim about inferred treatment, encrypted reasoning, model attention or study outcomes.'],
                    inventory=dict(forbidden_literals=len(scanner.tokens), candidate_ids=len(scanner.candidate_ids),
                                   opaque_candidate_ids=len(scanner.opaque_ids)),
                    coverage=dict(coverage), summary=dict(findings=len(findings), by_surface=dict(counts),
                                                         by_kind=dict(kinds), by_cue=dict(cue_counts),
                                                         affected_request_payloads=sum(row['findings'] > 0 for row in request_summary)),
                    errors=self.errors, warnings=self.warnings, requests=request_summary,
                    archive_surfaces_with_findings=surfaces, findings=findings)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('study', type=Path)
    parser.add_argument('--allow-partial', action='store_true')
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    study, output = args.study.resolve(), args.output.resolve()
    if output.is_relative_to(study):
        parser.error('report must be outside the audited study')
    report = Audit(study, args.allow_partial).run()
    output.write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print(json.dumps({key: report[key] for key in ('status', 'summary', 'coverage', 'errors', 'warnings')}, sort_keys=True))
    return 1 if report['summary']['findings'] or report['errors'] else 0


if __name__ == '__main__':
    raise SystemExit(main())
