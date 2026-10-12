"""One frozen information exposure, carried by signed real-daemon contributions.

Every branch has its own goal and principals. Models receive only FrozenRecord's
review/source interface; the goal-wide daemon API is deliberately not a model
tool. Policy eligibility is enforced at that harness boundary, not claimed as a
per-recipient daemon ACL. Every read inspects explicit signed contribution IDs.
"""
import base64
import copy
import hashlib
import json
import math
from pathlib import Path
import re
import shutil
import sys
import threading
import time

from scoring import all_valid

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
from client_qualification.production import ProductionDaemon  # noqa: E402
from client_qualification.runtime import Profile  # noqa: E402
from check_shared_context_models import raw_call, enroll  # noqa: E402

# Protocol constraints, not experiment caps. See locust-proto/src/limits.rs and
# seal.rs: a payload includes format (1), epoch (4), nonce (24), and tag (16).
MAX_PAYLOAD_BYTES = 1024 * 1024
SEALED_OVERHEAD_BYTES = 45
MAX_TEXT_BYTES = MAX_PAYLOAD_BYTES - SEALED_OVERHEAD_BYTES
PRINCIPAL_NAME = re.compile(r'^[a-z0-9-]{1,32}$')
PACKET_SCHEMA = 'shared-record-packet-v1'
CHUNK_SCHEMA = 'shared-record-chunk-v1'


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'),
                      ensure_ascii=False, allow_nan=False)


def sha256(value):
    return hashlib.sha256(value).hexdigest()


def principal_name(title, agent):
    """A short stable identity; retain the complete input in the identity map."""
    if not isinstance(title, str) or not title or not isinstance(agent, str) or not agent:
        raise ValueError('principal title and agent must be nonempty strings')
    return 'sr-' + sha256(canonical([title, agent]).encode())[:29]


class Daemon:
    def __init__(self, output, binary):
        self.profile = Profile(output, 'record')
        self.daemon = ProductionDaemon(self.profile, binary, 120)
        self.lock = threading.Lock()
        self.events = []
        self.identity_map = {}
        self._goal_titles = set()

    def __enter__(self):
        self.daemon.__enter__()
        return self

    def binary_metadata(self):
        return self.daemon.binary_metadata

    def goal(self, title, agents):
        agents = list(agents)
        if not agents or len(agents) != len(set(agents)):
            raise ValueError('a goal needs distinct agents')
        names = {agent: principal_name(title, agent) for agent in agents}
        with self.lock:
            if title in self._goal_titles:
                raise ValueError('branch goal identity already used: ' + title)
            for agent, name in names.items():
                if name in self.identity_map:
                    raise ValueError('principal identity collision: ' + name)
            # A failed setup cannot be silently retried under the same identity.
            self._goal_titles.add(title)
            for agent, name in names.items():
                self.identity_map[name] = dict(title=title, agent=agent, name=name)
            created = raw_call(self.daemon, ['--agent', 'qualification', 'goal', 'create',
                                             '--title', title, '--formation', 'open'], owner=True)
            goal = created['goal_created']['goal']
            self.daemon.goal = goal
            roles = {agent: enroll(self.daemon, self.profile, names[agent]) for agent in agents}
            for agent, role in roles.items():
                self.identity_map[names[agent]].update(goal=goal, principal=role['principal'])
        identities = {agent: copy.deepcopy(self.identity_map[name]) for agent, name in names.items()}
        return Goal(self, goal, roles, identities)

    def call(self, args, role, stdin=None):
        with self.lock:
            self.daemon.goal = None
            return raw_call(self.daemon, args, role=role, stdin=stdin)

    def __exit__(self, *args):
        try:
            self.daemon.__exit__(*args)
        finally:
            self.profile.close()


class Goal:
    def __init__(self, daemon, goal, roles, identities=None):
        self.daemon, self.id, self.roles = daemon, goal, roles
        self.identity_map = identities or {}
        self.receipts = []
        self.lock = threading.Lock()

    def principal(self, agent):
        return self.roles[agent]['principal']

    def publish(self, agent, finding):
        text = canonical(finding)
        length = len(text.encode())
        if length + SEALED_OVERHEAD_BYTES > MAX_PAYLOAD_BYTES:
            raise ValueError(f'sealed contribution exceeds {MAX_PAYLOAD_BYTES} bytes')
        started = time.time()
        receipt = self.daemon.call(['contribution', 'publish', '--goal', self.id, '-'],
                                   self.roles[agent], stdin=text)
        with self.lock:
            self.receipts.append(dict(agent=agent, principal=self.principal(agent), receipt=receipt,
                                      text_sha256=sha256(text.encode()), plaintext_bytes=length,
                                      sealed_bytes=length + SEALED_OVERHEAD_BYTES,
                                      started_at=started, finished_at=time.time()))
        return receipt

    def read(self, agent, receipt_ids):
        """Read exact IDs one at a time, so an aggregate reply cannot hit frame limits."""
        out = []
        for receipt_id in receipt_ids:
            reply = self.daemon.call(['contribution', 'inspect', '--goal', self.id,
                                     '--contribution', receipt_id], self.roles[agent])
            event = reply['contribution_inspected']['contribution']
            if event['view']['event'] != receipt_id or event['view']['standing'] != 'effective':
                raise RuntimeError('contribution is not the expected effective signed event')
            out.append(dict(contribution=receipt_id, author=event['view']['author'],
                            finding=json.loads(event['text'])))
        return out


def fully_valid(candidate):
    """Use the selector's exact validity convention, including repeated outcomes."""
    return all_valid(candidate['public'])


def eligible_candidates(candidates, policy):
    """Public-only chronological filter, using exact unrounded float comparisons.

    Return (eligible metadata, withheld reasons). Failed public validation always
    qualifies for F. First valid establishes a baseline; a strict improvement is
    then excluded, including improvements too small to survive display rounding.
    """
    if policy not in ('E', 'D', 'F'):
        raise ValueError('unknown information policy: ' + str(policy))
    ordered = sorted(candidates, key=lambda candidate: (candidate['sequence'], candidate['id']))
    best = None
    eligible, withheld = [], []
    sequences = set()
    for candidate in ordered:
        if candidate['sequence'] in sequences:
            raise ValueError('candidate sequence must be unique within an author')
        sequences.add(candidate['sequence'])
        score = candidate['public']['normalized']
        if not isinstance(score, (int, float)) or not math.isfinite(score):
            raise ValueError('public normalized score must be finite')
        valid = fully_valid(candidate)
        before = best
        improving = valid and (best is None or score < best)
        if improving:
            best = score
        reason = ('failed_public_validation' if not valid else
                  'first_valid_baseline' if before is None else
                  'strict_public_improvement' if improving else 'non_improving')
        metadata = dict(schema=PACKET_SCHEMA, candidate_id=candidate['id'],
                        parent_id=candidate.get('parent_id'), author=candidate['agent'],
                        sequence=candidate['sequence'], source_sha256=candidate['sha256'],
                        source_reference=dict(candidate_id=candidate['id'], sha256=candidate['sha256'],
                                              utf8_bytes=len(candidate['source'].encode())),
                        public=copy.deepcopy(candidate['public']), fully_public_valid=valid,
                        public_best_before=before,
                        public_best_delta=None if before is None else before - score,
                        eligibility_reason=reason)
        if policy != 'F' or not improving:
            eligible.append(metadata)
        else:
            withheld.append(dict(candidate_id=candidate['id'], author=candidate['agent'], reason=reason))
    return eligible, withheld


def document_chunks(branch_id, author, candidate_id, kind, value):
    """Losslessly paginate a JSON document at the real sealed-payload boundary."""
    payload = canonical(value).encode()
    envelope = dict(schema=CHUNK_SCHEMA, branch_id=branch_id, author=author,
                    candidate_id=candidate_id, kind=kind, sha256=sha256(payload),
                    encoding='base64-json-utf8', part=0, parts=1, data='')
    parts = 1
    while True:
        envelope.update(part=parts - 1, parts=parts)
        available = MAX_TEXT_BYTES - len(canonical(envelope).encode())
        capacity = available // 4 * 3  # Base64 expands each three bytes to four.
        if capacity <= 0:
            raise ValueError('chunk identity fields exceed the daemon payload constraint')
        needed = max(1, (len(payload) + capacity - 1) // capacity)
        if needed == parts:
            break
        parts = needed
    chunks = []
    for part in range(parts):
        chunk = dict(envelope, part=part,
                     data=base64.b64encode(payload[part * capacity:(part + 1) * capacity]).decode('ascii'))
        if len(canonical(chunk).encode()) > MAX_TEXT_BYTES:
            raise AssertionError('chunk sizing exceeded the transport bound')
        chunks.append(chunk)
    return chunks


class FrozenRecord:
    """Immutable donor prefix and one review exposure per explicit recipient."""
    def __init__(self, goal, branch_id, policy, candidates_by_agent):
        if not isinstance(branch_id, str) or not branch_id:
            raise ValueError('branch_id must be a nonempty string')
        if policy not in ('E', 'D', 'F'):
            raise ValueError('unknown information policy')
        if set(candidates_by_agent) != set(goal.roles):
            raise ValueError('frozen donors must match the branch goal principals')
        self.goal, self.branch_id, self.policy = goal, branch_id, policy
        self._candidates = copy.deepcopy(candidates_by_agent)
        self._metadata, self._withheld, self._references = {}, {}, {}
        self._allowed, self._deliveries, self._source_reads = {}, [], []
        self._state, self._error = 'unfrozen', None
        self._freeze_started_at, self._freeze_finished_at = None, None
        self._lock = threading.RLock()
        self._valid_prefix = {}
        seen = set()
        for agent in sorted(self._candidates):
            candidates = self._candidates[agent]
            for candidate in candidates:
                candidate_id = candidate['id']
                if not isinstance(candidate_id, str) or not candidate_id or candidate_id in seen:
                    raise ValueError('candidate IDs must be nonempty and globally unique within a prefix')
                seen.add(candidate_id)
                if candidate['agent'] != agent:
                    raise ValueError('candidate author disagrees with donor identity')
                if sha256(candidate['source'].encode()) != candidate['sha256']:
                    raise ValueError('candidate source hash mismatch')
                if candidate['public']['source_sha256'] != candidate['sha256']:
                    raise ValueError('public evaluation source hash mismatch')
            eligible, withheld = eligible_candidates(candidates, policy)
            self._metadata[agent], self._withheld[agent] = eligible, withheld
            self._valid_prefix[agent] = any(fully_valid(candidate) for candidate in candidates)

    def _publish_document(self, agent, candidate_id, kind, document):
        chunks = document_chunks(self.branch_id, agent, candidate_id, kind, document)
        reference = dict(candidate_id=candidate_id, author=agent, kind=kind,
                         sha256=chunks[0]['sha256'], receipt_ids=[])
        # Retain partial receipts too if publication stops partway through.
        self._references.setdefault(candidate_id, {})[kind] = reference
        for chunk in chunks:
            receipt = self.goal.publish(agent, chunk)
            reference['receipt_ids'].append(receipt['recorded']['event'])
        reference['parts'] = len(chunks)

    def freeze(self):
        with self._lock:
            if self._state != 'unfrozen':
                raise RuntimeError('a frozen record may be published exactly once')
            self._state = 'publishing'
            self._freeze_started_at = time.time()
            try:
                for agent in sorted(self._metadata):
                    sources = {candidate['id']: candidate for candidate in self._candidates[agent]}
                    for metadata in self._metadata[agent]:
                        candidate_id = metadata['candidate_id']
                        source = sources[candidate_id]
                        self._publish_document(agent, candidate_id, 'source',
                                               dict(candidate_id=candidate_id, sha256=source['sha256'],
                                                    source=source['source']))
                        self._publish_document(agent, candidate_id, 'metadata', metadata)
            except Exception as error:
                self._state, self._error = 'failed', str(error)
                self._freeze_finished_at = time.time()
                raise
            self._state = 'frozen'
            self._freeze_finished_at = time.time()
        return self

    def _read_document(self, recipient, reference):
        records = self.goal.read(recipient, reference['receipt_ids'])
        if len(records) != reference['parts']:
            raise RuntimeError('incomplete frozen document delivery')
        pieces = []
        for part, (expected_id, record) in enumerate(zip(reference['receipt_ids'], records)):
            chunk = record['finding']
            expected = dict(schema=CHUNK_SCHEMA, branch_id=self.branch_id, author=reference['author'],
                            candidate_id=reference['candidate_id'], kind=reference['kind'],
                            sha256=reference['sha256'], encoding='base64-json-utf8', part=part,
                            parts=reference['parts'])
            if (record['contribution'] != expected_id
                    or record['author'] != self.goal.principal(reference['author'])
                    or {key: chunk.get(key) for key in expected} != expected):
                raise RuntimeError('signed frozen document identity mismatch')
            pieces.append(base64.b64decode(chunk['data'], validate=True))
        payload = b''.join(pieces)
        if sha256(payload) != reference['sha256']:
            raise RuntimeError('frozen document digest mismatch')
        return json.loads(payload)

    def review(self, agent, peers=False):
        with self._lock:
            if self._state != 'frozen':
                raise RuntimeError('review requires a successfully frozen record')
            if agent not in self._metadata:
                raise ValueError('unknown recipient')
            if agent in self._allowed:
                raise RuntimeError('recipient already received its one scheduled exposure')
            # Mark attempted before transport. Failure cannot become a fresh exposure.
            self._allowed[agent] = set()
            peer_access = bool(peers and self._valid_prefix[agent])
            donors = [agent] + ([donor for donor in sorted(self._metadata) if donor != agent]
                                if peer_access else [])
            eligible = [metadata['candidate_id'] for donor in donors for metadata in self._metadata[donor]]
            withheld = []
            for donor in sorted(self._metadata):
                if donor in donors:
                    withheld.extend(copy.deepcopy(self._withheld[donor]))
                else:
                    reason = 'recipient_has_no_valid_prefix' if peers else 'private_review'
                    withheld.extend(dict(candidate_id=candidate['id'], author=donor, reason=reason)
                                    for candidate in self._candidates[donor])
            delivery = dict(recipient=agent, recipient_id=self.branch_id + '/' + agent,
                            principal=self.goal.principal(agent), peers_requested=bool(peers),
                            peer_access=peer_access, eligible_candidate_ids=eligible,
                            withheld=withheld, delivered_candidate_ids=[], receipt_ids=[], status='reading',
                            started_at=time.time())
            self._deliveries.append(delivery)
            packet = dict(schema=PACKET_SCHEMA, policy=self.policy, candidates=[])
            try:
                for candidate_id in eligible:
                    reference = self._references[candidate_id]['metadata']
                    packet['candidates'].append(self._read_document(agent, reference))
                    delivery['delivered_candidate_ids'].append(candidate_id)
                    delivery['receipt_ids'].extend(reference['receipt_ids'])
            except Exception as error:
                delivery.update(status='failed', error=str(error), finished_at=time.time())
                raise RuntimeError('frozen review delivery failed: ' + str(error)) from error
            self._allowed[agent] = set(eligible)
            delivery.update(status='delivered', finished_at=time.time())
            return packet

    def source(self, agent, candidate_id):
        with self._lock:
            if candidate_id not in self._allowed.get(agent, set()):
                raise PermissionError('source is not in this recipient\'s frozen eligible record')
            reference = self._references[candidate_id]['source']
            receipt = dict(recipient=agent, recipient_id=self.branch_id + '/' + agent,
                           principal=self.goal.principal(agent), candidate_id=candidate_id,
                           receipt_ids=list(reference['receipt_ids']), status='reading', started_at=time.time())
            self._source_reads.append(receipt)
            try:
                result = self._read_document(agent, reference)
                if result['candidate_id'] != candidate_id or sha256(result['source'].encode()) != result['sha256']:
                    raise RuntimeError('source identity/hash mismatch')
            except Exception as error:
                receipt.update(status='failed', error=str(error), finished_at=time.time())
                raise RuntimeError('frozen source delivery failed: ' + str(error)) from error
            receipt.update(status='delivered', finished_at=time.time())
            return result

    def archive(self):
        with self._lock:
            return copy.deepcopy(dict(schema=PACKET_SCHEMA, branch_id=self.branch_id, goal=self.goal.id,
                                      policy=self.policy, state=self._state, error=self._error,
                                      freeze_started_at=self._freeze_started_at,
                                      freeze_finished_at=self._freeze_finished_at,
                                      identity_map=self.goal.identity_map, valid_prefix=self._valid_prefix,
                                      candidates=self._metadata, filtered_out=self._withheld,
                                      references=self._references, deliveries=self._deliveries,
                                      source_reads=self._source_reads, receipts=self.goal.receipts))


def copy_binary(source, destination):
    """A private copy so a rebuild during the study cannot change the daemon under test."""
    destination = Path(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    destination.chmod(0o700)
    return destination
