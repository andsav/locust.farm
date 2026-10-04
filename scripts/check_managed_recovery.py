#!/usr/bin/env python3
"""Actual-client default blocking and explicit recovery after owned parent crash.

Fault cleanup is intentional and separately reported; it never qualifies natural
cleanup. Uses private profiles and scripted loopback providers, no user account.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import signal
import sys
import threading
import time

import check_clients as fixture
from check_managed_clients import flags, metadata, native_wait_ids, ready_evidence
from check_t2_clients import step
from client_qualification.production import ProductionDaemon, ProductionError
from client_qualification.provider import Provider
from client_qualification.runtime import Process, Profile, private_write, records

CHECKS = ("default_blocked", "crash_observed", "unknown_recovery", "duplicate_launch_refused", "fault_cleanup")


def unknown_evidence(before, after, response):
    return (after["record"]["state"] == "unknown" and
            metadata(before)["launch_id"] == metadata(after)["launch_id"] and
            metadata(before) == metadata(after) and
            before["record"].get("client_session") == after["record"].get("client_session") and
            response.get("spawned") is False and response.get("signaled") is False)


def qualify(client, binary, args):
    result = {"client":client, "assertions":{key:fixture.assertion("not_run","Scenario not executed") for key in CHECKS}, "runs":[]}
    if not binary:
        return result
    profile = Profile(args.output,client)
    checks = result["assertions"]
    timeout = args.timeout_ms/1000
    try:
        env = profile.environment(binary)
        version = Process([binary,"--version"],env,profile.workspace,profile.logs,"version",timeout).wait()
        if version["exit_code"] != 0:
            raise ProductionError("Client version unavailable")
        observed_version = Path(version["stdout"]).read_text().strip()
        result["version"] = observed_version
        result["binary_sha256"] = hashlib.sha256(Path(binary).read_bytes()).hexdigest()
        with ProductionDaemon(profile,args.locust,timeout) as daemon, Provider([]) as provider:
            result["locust_artifact"] = daemon.binary_metadata
            env.update(fixture.provider_settings(client,profile,provider.url))
            native = profile.home/".pi/agent/sessions/recovery.jsonl"
            native.parent.mkdir(mode=0o700,parents=True,exist_ok=True)

            def command(permissive):
                global_args,run_args = flags(client,permissive)
                argv = daemon.command(["client","run","--client",client,"--executable",binary,
                    "--workspace",profile.workspace,"--profile",profile.home,"--client-version",observed_version,
                    "--goal",daemon.goal,"--prompt","Execute the authored private recovery qualification calls."])
                argv += ["--global-arg="+value for value in global_args]+["--arg="+value for value in run_args]
                if client=="pi":
                    argv += ["--native-session",str(native)]
                return argv

            # Readiness observation releases the scripted provider; a second
            # callback holds the client active until Blocked is independently read.
            ready = threading.Event(); blocked = threading.Event()
            def await_event(event):
                if not event.wait(timeout):
                    raise ProductionError("Expected lifecycle state not observed")
                return {"goal":daemon.goal}
            with provider.lock:
                provider.plan = [step("locust_contribution_publish",lambda:dict(await_event(ready),summary="default-policy-recovery",artifacts=[])),
                                 step("locust_goal_status",lambda:await_event(blocked))]
                provider.index = 0
            parent = Process(command(False),env,profile.workspace,profile.logs,"default",timeout)
            default_views=[]
            def observe_default(child):
                try: view=daemon.call(["client","status"])["session"]
                except ProductionError as error:
                    if error.code=="not_found": return
                    raise
                default_views.append(view)
                if ready_evidence(view,parent.stderr_path): ready.set()
                if view["record"]["state"]=="blocked": blocked.set()
                if client=="pi" and ready.is_set(): blocked.set()
            try:
                run=parent.wait(observe=observe_default)
            finally:
                ready.set(); blocked.set(); parent.close()
            result["runs"].append(run)
            denials=fixture.permission_denials(dict(run,stdout=run["stderr"]))
            proved=bool(denials) and any(view["record"]["state"]=="blocked" for view in default_views)
            checks["default_blocked"]=fixture.assertion("pass" if proved else "not_run" if client=="pi" and not denials else "fail",
                "Structured policy denial and independently observed parent Blocked required; Pi default allowed writes do not prove blocking")
            private_write(profile.logs/"default.sessions.json",json.dumps(default_views,indent=2))

            # Use another fresh protected Locust session for the fault scenario.
            fault_session=daemon.home/"sessions/fault.secret"
            daemon.call(["session","create",str(fault_session)],owner=True)
            daemon.session=fault_session
            if client=="pi":
                native=profile.home/".pi/agent/sessions/fault.jsonl"
            ready=threading.Event()
            def wait_args():
                await_event(ready)
                revision=daemon.call(["pending","--goal",daemon.goal])["pending"]["revision"]
                return {"goal":daemon.goal,"seen":revision,"timeout_ms":args.timeout_ms}
            with provider.lock:
                provider.plan=[step("locust_wait",wait_args)]
                provider.index=0
            parent=Process(command(True),env,profile.workspace,profile.logs,"fault",timeout)
            before=None;deadline=time.monotonic()+timeout
            try:
                while time.monotonic()<deadline and parent.process.poll() is None:
                    try: before=daemon.call(["client","status"])["session"]
                    except ProductionError as error:
                        if error.code!="not_found":raise
                        time.sleep(0.02);continue
                    receipt=metadata(before).get("lifecycle_receipt")
                    if receipt:
                        for event in records(receipt):
                            if event.get("event")=="tools_ready":parent.register_child(event.get("pid"),args.locust,receipt)
                    if ready_evidence(before,parent.stderr_path):ready.set()
                    if ready.is_set() and native_wait_ids(parent.stderr_path):break
                    time.sleep(0.02)
                else:
                    raise ProductionError("Actual client did not reach ready outstanding wait")
                parent.process.kill()  # ONLY the retained owned managed parent.
                parent.process.wait(timeout=timeout)
                checks["crash_observed"]=fixture.assertion("pass" if parent.process.returncode==-signal.SIGKILL else "fail","Owned managed parent SIGKILL observed; child cleanup intentionally deferred")
                before=daemon.call(["client","status"])["session"]
                response=daemon.call(["client","recover"])
                after=daemon.call(["client","status"])["session"]
                checks["unknown_recovery"]=fixture.assertion("pass" if unknown_evidence(before,after,response) else "fail","Recovery preserves launch/process identity and explicitly neither spawns nor signals")
                try:
                    retry=command(True)
                    daemon.call(retry[retry.index("client"):])
                except ProductionError as error:refused=error.code=="conflict"
                else:refused=False
                latest=daemon.call(["client","status"])["session"]
                unchanged=metadata(latest)["launch_id"]==metadata(after)["launch_id"]
                checks["duplicate_launch_refused"]=fixture.assertion("pass" if refused and unchanged else "fail","Unknown launch rejects explicit retry with Conflict and unchanged launch identifier")
                private_write(profile.logs/"fault.sessions.json",json.dumps({"before":before,"recovery":response,"after":after,"latest":latest},indent=2))
            finally:
                ready.set();parent.close()
                cleaned=not parent.live_owned_members() and not any(parent.owned_child_alive(pid) for pid in parent.children)
                checks["fault_cleanup"]=fixture.assertion("pass" if cleaned else "fail","Intentional owned-session forced fault cleanup; no natural-cleanup claim")
                result["fault_cleanup"]={"forced":parent.forced_cleanup,"verified":cleaned}
            result["provider_errors"]=provider.errors
            result["provider_request_count"]=len(provider.requests)
    except Exception as error:
        result["harness_error"]=type(error).__name__+": "+str(error)
        checks["fault_cleanup"]=fixture.assertion("fail","Qualification did not finish; inspect retained evidence")
    finally:
        profile.close()
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--locust",required=True);parser.add_argument("--timeout-ms",type=int,required=True)
    parser.add_argument("--output",type=Path,required=True)
    for client in fixture.CLIENTS:parser.add_argument("--"+client)
    args=parser.parse_args()
    if args.timeout_ms<=0:parser.error("timeout must be positive")
    args.locust=fixture.checked_binary(args.locust)
    if not args.locust:parser.error("Locust executable unavailable")
    args.output=args.output.resolve();args.output.mkdir(mode=0o700,parents=True,exist_ok=True)
    report={"schema":"locust-managed-recovery-qualification","created_at":datetime.now(timezone.utc).isoformat(),
            "timeout_ms":args.timeout_ms,"scope":"actual client/scripted provider/private profile; default blocking and intentional parent fault; no natural fault-cleanup claim","clients":[]}
    for client in fixture.CLIENTS:
        report["clients"].append(qualify(client,fixture.checked_binary(getattr(args,client.replace("-","_"))),args))
    sources=[Path(__file__),Path(fixture.__file__),Path(sys.modules["check_managed_clients"].__file__)]
    report["harness_sources"]=[{"path":str(path.resolve()),"sha256":hashlib.sha256(path.read_bytes()).hexdigest()} for path in sources]
    destination=args.output/"report.json"
    if destination.exists():
        previous=destination.read_bytes()
        private_write(args.output/("report-"+hashlib.sha256(previous).hexdigest()+".json"),previous)
    private_write(destination,json.dumps(report,indent=2)+"\n")
    return int(any(check["status"]=="fail" for result in report["clients"] for check in result["assertions"].values()))

if __name__=="__main__":sys.exit(main())
