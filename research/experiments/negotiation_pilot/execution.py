"""Execute candidate Python in a private macOS sandbox, without credentials."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

RUNNER = '''import json, sys, resource
resource.setrlimit(resource.RLIMIT_CPU, (30, 30))

data=json.load(sys.stdin)
namespace={}
try:
 exec(compile(data["source"], "candidate.py", "exec"), namespace)
 results=[]
 for item in data["inputs"]:
  try: results.append({"value": namespace["solve"](item)})
  except BaseException as error: results.append({"error":type(error).__name__+": "+str(error)})
 print(json.dumps(results))
except BaseException as error:
 print(json.dumps({"error":type(error).__name__+": "+str(error)}))
'''


def execute(source, inputs):
    with tempfile.TemporaryDirectory(prefix='lnp-') as directory:
        root = str(Path(directory).resolve())
        guard = ('(version 1)(allow default)(deny network*)'
                 '(deny file-read* (subpath "/Users") (subpath "/private/tmp")'
                 '(subpath "/private/var/folders"))'
                 '(allow file-read* (subpath '+json.dumps(root)+'))'
                 '(deny file-write*)(allow file-write* (subpath '+json.dumps(root)+'))'
                 '(deny process-fork)')
        script = Path(root)/'runner.py'
        script.write_text(RUNNER)
        env = {'PATH':'/usr/bin:/bin','HOME':root,'TMPDIR':root,'LANG':'en_US.UTF-8'}
        try:
            result = subprocess.run(['/usr/bin/sandbox-exec','-p',guard,sys.executable,'-I',str(script)],
                input=json.dumps({'source':source,'inputs':inputs}),text=True,capture_output=True,
                cwd=root,env=env,timeout=40)
            if result.returncode:
                return {'error':'candidate process failed', 'exit_code':result.returncode,'stderr':result.stderr}
            return json.loads(result.stdout)
        except subprocess.TimeoutExpired:
            return {'error':'candidate exceeded the declared 30 CPU / 40 wall second evaluator limit'}
        except ValueError:
            return {'error':'candidate stdout was not valid JSON'}


def evaluate(source, fixtures, reveal=True):
    actual = execute(source, [f['input'] for f in fixtures])
    if not isinstance(actual, list) or len(actual) != len(fixtures):
        return {'passed':0,'total':len(fixtures),'error':actual}
    failures = []
    passed = 0
    for index, (case, result) in enumerate(zip(fixtures, actual)):
        okay = isinstance(result, dict) and 'value' in result and json.dumps(result['value'],sort_keys=True) == json.dumps(case['expected'],sort_keys=True)
        passed += okay
        if not okay:
            failure = {'index':index,'actual':result}
            if reveal:
                failure.update(case)
            failures.append(failure)
    return {'passed':passed,'total':len(fixtures),'failures':failures}
