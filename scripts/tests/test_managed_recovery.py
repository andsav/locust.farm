"""Unknown recovery cannot qualify implicit launch, signaling or identity drift."""
import copy
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import check_managed_recovery as harness

class RecoveryTests(unittest.TestCase):
    def test_unknown_requires_explicit_no_side_effects_and_unchanged_metadata(self):
        detail={"launch_id":"launch","process":{"pid":123,"exited":False},"binding":{"instance":"instance"}}
        before={"record":{"state":"ready","client_session":"native","detail":list(json.dumps(detail).encode())}}
        after=copy.deepcopy(before);after["record"]["state"]="unknown"
        response={"spawned":False,"signaled":False}
        self.assertTrue(harness.unknown_evidence(before,after,response))
        for changed in ({"spawned":True},{"signaled":True},{"spawned":None}):
            self.assertFalse(harness.unknown_evidence(before,after,dict(response,**changed)))
        for field in ("launch_id","process","binding"):
            modified=copy.deepcopy(after);other=copy.deepcopy(detail);other[field]="different"
            modified["record"]["detail"]=list(json.dumps(other).encode())
            self.assertFalse(harness.unknown_evidence(before,modified,response))
        after["record"]["client_session"]="forked"
        self.assertFalse(harness.unknown_evidence(before,after,response))

if __name__=="__main__":unittest.main()
