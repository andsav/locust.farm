"""Arm 3's shared record: signed findings in a real local Locust goal.

One daemon serves the study. Each shared-record trial gets its own goal with
one enrolled principal per agent. Findings are goal-wide contributions; reads
go through the daemon, never through harness memory.
"""
import json
from pathlib import Path
import re
import shutil
import sys
import threading

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
from client_qualification.production import ProductionDaemon  # noqa: E402
from client_qualification.runtime import Profile  # noqa: E402
from check_shared_context_models import raw_call, enroll  # noqa: E402

MAX_FINDING_BYTES = 12000
PRINCIPAL_NAME = re.compile(r'^[a-z0-9-]{1,32}$')  # The daemon's rule for `agent enroll` names.


def principal_name(title, agent):
    """A daemon-valid principal name for an agent of a trial; task names may contain underscores."""
    name = f'{title}-{agent}'.replace('_', '-')
    for long, short in (('-agent-', '-a'), ('calibration-', 'cal-'), ('scored-', 'sc-')):
        if len(name) <= 32:
            break
        name = name.replace(long, short, 1)
    if not PRINCIPAL_NAME.match(name):
        raise ValueError(f'{name!r} is not a valid principal name')
    return name


class Daemon:
    def __init__(self, output, binary):
        self.profile = Profile(output, 'record')
        self.daemon = ProductionDaemon(self.profile, binary, 120)
        self.lock = threading.Lock()
        self.events = []

    def __enter__(self):
        self.daemon.__enter__()
        return self

    def binary_metadata(self):
        return self.daemon.binary_metadata

    def goal(self, title, agents):
        names = {agent: principal_name(title, agent) for agent in agents}  # Validate before touching the daemon.
        with self.lock:
            created = raw_call(self.daemon, ['--agent', 'qualification', 'goal', 'create',
                                              '--title', title, '--formation', 'open'], owner=True)
            goal = created['goal_created']['goal']
            self.daemon.goal = goal
            roles = {agent: enroll(self.daemon, self.profile, names[agent]) for agent in agents}
        return Goal(self, goal, roles)

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
    def __init__(self, daemon, goal, roles):
        self.daemon, self.id, self.roles = daemon, goal, roles
        self.receipts = []
        self.lock = threading.Lock()

    def principal(self, agent):
        return self.roles[agent]['principal']

    def publish(self, agent, finding):
        text = json.dumps(finding, sort_keys=True)
        if len(text.encode()) > MAX_FINDING_BYTES:
            raise ValueError(f'finding exceeds {MAX_FINDING_BYTES} bytes')
        receipt = self.daemon.call(['contribution', 'publish', '--goal', self.id, '-'],
                                   self.roles[agent], stdin=text)
        with self.lock:
            self.receipts.append(dict(agent=agent, principal=self.principal(agent), receipt=receipt,
                                      text_sha256=__import__('hashlib').sha256(text.encode()).hexdigest()))
        return receipt

    def read(self, agent):
        """Every contribution the daemon returns to this principal, in daemon order."""
        listing = self.daemon.call(['contributions', '--goal', self.id], self.roles[agent])
        out = []
        for record in listing['contributions']:
            try:
                finding = json.loads(record['text'])
            except (ValueError, TypeError, KeyError):
                continue
            out.append(dict(contribution=record['contribution'], author=record['author'], finding=finding))
        return out


def copy_binary(source, destination):
    """A private copy so a rebuild during the study cannot change the daemon under test."""
    destination = Path(destination)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    destination.chmod(0o700)
    return destination
