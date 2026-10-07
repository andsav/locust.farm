"""Actual signed Locust contributions; experimenter controls exchange timing."""
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT/'scripts'))
from client_qualification.production import ProductionDaemon
from client_qualification.runtime import Profile
from check_shared_context_models import raw_call, enroll


class Board:
    def __init__(self, output, binary):
        self.profile = Profile(output, 'board')
        self.daemon = ProductionDaemon(self.profile, binary, 60)
        self.roles = []
        self.receipts = []

    def __enter__(self):
        self.daemon.__enter__()
        return self

    def trial(self, name):
        created = raw_call(self.daemon, ['--agent','qualification','goal','create',
                          '--title',name,'--formation','open'], owner=True)
        self.daemon.goal = created['goal_created']['goal']
        self.roles = [enroll(self.daemon, self.profile, name+'-'+str(i)) for i in range(2)]
        return self.daemon.goal

    def publish(self, role, phase, candidate):
        payload = dict(phase=phase, candidate=candidate)
        summary = json.dumps(payload, sort_keys=True)
        receipt = raw_call(self.daemon, ['contribution','publish','--goal',self.daemon.goal,'-'],
                           role=self.roles[role], stdin=summary)
        self.receipts.append(dict(goal=self.daemon.goal,principal=self.roles[role]['principal'],
                                 phase=phase,source_sha256=candidate['sha256'],receipt=receipt))
        return summary

    def read(self, role):
        return raw_call(self.daemon,['contributions','--goal',self.daemon.goal],role=self.roles[role])

    def __exit__(self, *args):
        try:
            self.daemon.__exit__(*args)
        finally:
            self.profile.close()
