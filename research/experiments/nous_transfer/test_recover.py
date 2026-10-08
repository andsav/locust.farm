import io
import json
from pathlib import Path
import tempfile
import threading
import unittest
from unittest.mock import patch
import urllib.error

from recover import Recovery, selected_records
from test_experiment import fixture


class RecoveryTests(unittest.TestCase):
    def test_failed_attempt_reservation_counts_against_retry_budget(self):
        with tempfile.TemporaryDirectory() as tmp:
            runner=object.__new__(Recovery);runner.folder=Path(tmp);runner.frozen=fixture()
            runner.frozen['ceiling_usd']=.005
            runner.attempts=[];runner.lock=threading.Lock();runner.stop=threading.Event();runner.simulated=False
            with patch.dict('os.environ',{'OPENAI_API_KEY':'unit-test-placeholder'}), \
                 patch('urllib.request.urlopen',side_effect=urllib.error.URLError('fixture')) as request, \
                 patch.object(runner.stop,'wait',return_value=False):
                runner.call(runner.frozen['cases'][0],'neutral','initial',0)
            self.assertEqual(request.call_count,1)
            self.assertEqual(len(runner.attempts),1)
            self.assertTrue(runner.stop.is_set())

    def test_first_completed_retained_even_if_schema_invalid(self):
        attempts=[{'label':'a','status':'transport_error'},
                  {'label':'a#attempt2','logical_label':'a','status':'completed','text':'invalid'},
                  {'label':'a#attempt3','logical_label':'a','status':'completed','text':'different'}]
        self.assertEqual(selected_records(attempts)[0]['text'],'invalid')

    def test_transient_error_retried_and_both_attempts_accounted(self):
        with tempfile.TemporaryDirectory() as tmp:
            runner=object.__new__(Recovery);runner.folder=Path(tmp);runner.frozen=fixture()
            runner.attempts=[];runner.lock=threading.Lock();runner.stop=threading.Event();runner.simulated=False
            response={'status':'completed','usage':{'input_tokens':100,'output_tokens':20},'output':[]}
            with patch.dict('os.environ',{'OPENAI_API_KEY':'unit-test-placeholder'}), \
                 patch('urllib.request.urlopen',side_effect=[urllib.error.URLError('fixture'),io.StringIO(json.dumps(response))]), \
                 patch.object(runner.stop,'wait',return_value=False):
                runner.call(runner.frozen['cases'][0],'neutral','initial',0)
                runner.call(runner.frozen['cases'][0],'neutral','initial',0)
            self.assertEqual(len(runner.attempts),2)
            self.assertEqual(runner.records[0]['status'],'completed')
            runner.verify_records()
            self.assertEqual(len(json.loads((runner.folder/'attempts.json').read_text())),2)


if __name__=='__main__':unittest.main()
