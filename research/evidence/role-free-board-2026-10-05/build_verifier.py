#!/usr/bin/env python3
"""Build the evidence verifier using already-built workspace dependencies.
Rust artifacts may have several profiles; select a serde_json compatible with
locust_proto instead of accidentally mixing different serde crate identities.
"""
from pathlib import Path
import subprocess
import sys
HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
deps=ROOT/'target/debug/deps'
proto=max(deps.glob('liblocust_proto-*.rlib'),key=lambda p:p.stat().st_mtime)
output=Path(sys.argv[1]).resolve()
for serde in sorted(deps.glob('libserde_json-*.rlib')):
    command=['rustc','--edition=2024',str(HERE/'verify_events.rs'),'-L','dependency='+str(deps),'--extern','locust_proto='+str(proto),'--extern','serde_json='+str(serde),'-o',str(output)]
    result=subprocess.run(command,capture_output=True,text=True,cwd=ROOT)
    if result.returncode==0:
        print(output)
        break
else:
    raise RuntimeError('No compatible serde_json artifact; build the pinned workspace first')
