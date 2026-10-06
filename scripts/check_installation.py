#!/usr/bin/env python3
"""Qualify a local installation with an explicitly trusted bootstrap executable.

The unsigned candidate is copied as inert files, signed with disposable test
keys, and executed only by the trusted installer's verified version probe and
through the installed path. This never establishes production release trust.
All profiles, credentials and daemon state are private synthetic fixtures.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import plistlib
import re
import shutil
import signal
import stat
import subprocess
import tempfile
import time

from check_t1 import CheckFailure, redact, variant

PAYLOADS = ("locust", "skills/locust/SKILL.md", "manual.tar", "manifest.json")


def require(condition, message):
    if not condition:
        raise CheckFailure(message)


def digest(path):
    result = hashlib.sha256()
    with Path(path).open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def private_write(path, contents):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    with path.open("wb") as target:
        path.chmod(0o600)
        target.write(contents.encode() if isinstance(contents, str) else contents)


def plain_file(root, relative):
    """Read only the fixed inert package members, never manifest-selected paths."""
    root = Path(root)
    require(stat.S_ISDIR(root.lstat().st_mode), "bundle root must be a plain directory")
    path = root
    parts = Path(relative).parts
    require(parts and not Path(relative).is_absolute() and all(p not in (".", "..") for p in parts),
            "bundle path must be relative without traversal")
    for part in parts[:-1]:
        path = path / part
        require(stat.S_ISDIR(path.lstat().st_mode), "bundle parent must be a plain directory")
    path = path / parts[-1]
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), "rb") as source:
        metadata = os.fstat(source.fileno())
        require(stat.S_ISREG(metadata.st_mode) and metadata.st_nlink == 1,
                "bundle payload must be a singly linked regular file")
        return source.read(), stat.S_IMODE(metadata.st_mode)


def copy_bundle(source, destination, signed=False):
    destination.mkdir(mode=0o700)
    for relative in PAYLOADS + (("manifest.sig",) if signed else ()):
        data, mode = plain_file(source, relative)
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        private_write(target, data)
        target.chmod(mode)


def environment(root):
    """No ambient provider credentials, Git settings, HOME or client profiles."""
    return {"HOME": str(root / "home"), "XDG_CONFIG_HOME": str(root / "config"),
            "XDG_CACHE_HOME": str(root / "cache"), "XDG_DATA_HOME": str(root / "data"),
            "TMPDIR": str(root / "tmp"), "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
            "LANG": "en_US.UTF-8", "TERM": "dumb", "NO_COLOR": "1",
            "LOCUST_RELAY": "none", "LOCUST_LOOKUP": "none", "LOCUST_BIND": "127.0.0.1:0"}


def cpu_seconds(value):
    """Parse macOS/Linux ps accumulated CPU time, not lifetime-average %CPU."""
    days, clock = value.split("-", 1) if "-" in value else ("0", value)
    parts = clock.split(":")
    require(len(parts) in (2, 3), "unrecognized ps CPU time")
    result = 0.0
    for part in parts:
        result = result * 60 + float(part)
    return int(days) * 86400 + result


def data_fingerprint(root):
    """Compare every fixture entry without following symbolic links."""
    root = Path(root)
    result = {}
    for directory, folders, files in os.walk(root, followlinks=False):
        for path in [Path(directory), *(Path(directory) / name for name in folders + files)]:
            metadata = path.lstat()
            if stat.S_ISLNK(metadata.st_mode):
                value = ("symlink", stat.S_IMODE(metadata.st_mode), os.readlink(path))
            elif stat.S_ISDIR(metadata.st_mode):
                value = ("directory", stat.S_IMODE(metadata.st_mode))
            elif stat.S_ISREG(metadata.st_mode):
                value = ("file", stat.S_IMODE(metadata.st_mode), metadata.st_nlink, digest(path))
            else:
                value = ("special", metadata.st_mode, metadata.st_rdev)
            result[str(path.relative_to(root))] = value
    return result


class InstallationCheck:
    def contribution_grant(self, goal, principal):
        self.cli(["goal", "grant", "--goal", goal, "--agent", principal, "--grants",
            json.dumps({"contribute": True, "execute": False, "review": False,
                "select": False, "flow": False, "takeover": False})], installed=True, owner=True)

    def __init__(self, bootstrap, bundle, output, timeout=60, sample_interval=1, samples=3, baseline_bundle=None):
        self.input_bootstrap, self.input_bundle = Path(bootstrap), Path(bundle)
        self.input_baseline = Path(baseline_bundle) if baseline_bundle is not None else None
        self.output, self.timeout = Path(output), timeout
        self.sample_interval, self.samples = sample_interval, samples
        self.output.mkdir(parents=True, mode=0o700, exist_ok=False)
        self.output.chmod(0o700)
        self.transcript = self.output / "transcript.jsonl"
        private_write(self.transcript, b"")
        self.daemon = None
        self.daemon_log = None
        self.service_owned = None
        self.summary = {"status": "running", "evidence_level": "local test-signed native installation",
            "started_at": datetime.now(timezone.utc).isoformat(), "platform": platform.platform(),
            "architecture": platform.machine(), "production_trust_selected": False,
            "harness_sha256": digest(Path(__file__)), "cases": {}, "failures": [],
            "resource_sampling": {"interval_seconds": sample_interval, "samples": samples,
                                  "metric": "ps cumulative CPU delta and sampled resident bytes",
                                  "performance_verdict": "measurement only; no thresholds authored",
                                  "network_bytes": "unmeasured"},
            "service": {"status": "not_run", "reason": "native service campaign has not run"}}

    def record(self, kind, **fields):
        value = redact({"kind": kind, "time": datetime.now(timezone.utc).isoformat(), **fields})
        with self.transcript.open("a") as output:
            output.write(json.dumps(value, sort_keys=True) + "\n")

    def passed(self, case, **facts):
        self.summary["cases"][case] = {"status": "passed", **facts}
        self.record("case_passed", case=case, **facts)

    def command(self, argv):
        """Bound and reap the command's owned process group on a harness timeout."""
        process = subprocess.Popen(list(map(str, argv)), env=self.env, cwd=self.root,
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            start_new_session=True, text=True)
        try:
            out, err = process.communicate(timeout=self.timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate()
            raise CheckFailure("installation qualification command timed out") from None
        return process.returncode, out, err

    def cli(self, args, *, installed=False, owner=False, agent=False, expected_error=None, expected_message=None):
        binary = self.installed if installed else self.bootstrap
        argv = [binary, "--home", self.daemon_home, "--json"]
        if owner:
            argv.append("--owner")
        elif agent:
            argv.extend(["--credential", self.daemon_home / "agents" / "qualification.credential"])
        argv.extend(map(str, args))
        code, out, err = self.command(argv)
        try:
            body = json.loads(out)
        except (json.JSONDecodeError, UnicodeError):
            raise CheckFailure("CLI did not return one JSON envelope") from None
        require(isinstance(body, dict) and isinstance(body.get("ok"), bool), "invalid CLI envelope")
        self.record("cli", executable_role="installed" if installed else "trusted_bootstrap",
                    operation=list(map(str, args[:2])), exit_status=code, response=body,
                    expected_error=expected_error)
        require(not err, "JSON CLI wrote unexpected diagnostics")
        if expected_error is not None:
            require(not body["ok"] and body.get("error", {}).get("code") == expected_error and code != 0,
                    f"operation did not fail with expected {expected_error}")
            if expected_message is not None:
                require(expected_message in body.get("error", {}).get("message", ""), "refusal came from an unexpected failure path")
            return None
        require(body["ok"] and code == 0 and "result" in body,
                "CLI operation failed: " + str(body.get("error", {}).get("code", "invalid envelope")))
        return body["result"]

    def registry(self, name, sequence, withdrawn=(), key=None):
        path = self.trust / (name + ".json")
        private_write(path, json.dumps({"format": "locust-withdrawals-v1", "sequence": sequence,
                                       "withdrawn_manifest_sha256": list(withdrawn)}))
        self.cli(["package", "sign-withdrawals", "--registry", path, "--secret-key", key or self.secret])
        return path

    def inputs(self, bundle=None, registry=None, public=None, prefix=None):
        return ["--prefix", prefix or self.prefix, "--bundle", bundle or self.bundle,
                "--trust-key", public or self.public, "--withdrawals", registry or self.current_registry]

    def plan(self, **kwargs):
        return self.cli(["install", "plan", *self.inputs(**kwargs)])

    def apply(self, plan=None, **kwargs):
        plan = plan or self.plan(**kwargs)
        return self.cli(["install", "apply", *self.inputs(**kwargs), "--expect-plan", plan["plan_sha256"]])

    def status(self):
        return self.cli(["install", "status", "--prefix", self.prefix])

    def unchanged(self, manifest):
        require(self.status()["manifest_sha256"] == manifest, "failed install changed the selected release")

    def unchanged_trust(self, state):
        current = self.status()
        require(current["manifest_sha256"] == state["manifest_sha256"] and
                current["withdrawals_sequence"] == state["withdrawals_sequence"] and
                digest(self.prefix / "trust-state.json") == state["trust_state_sha256"],
                "refused operation changed active release or retained trust policy")

    def wait(self, description, predicate):
        deadline = time.monotonic() + self.timeout
        while time.monotonic() < deadline:
            if predicate():
                return
            time.sleep(0.05)
        raise CheckFailure(description + " timed out")

    def healthy_doctor(self):
        checks = self.cli(["doctor"], installed=True, owner=True)["checks"]
        require(isinstance(checks, list) and bool(checks) and all(check.get("ok") is True for check in checks),
                "installed doctor reported unhealthy or missing checks")

    def start_daemon(self):
        started = time.monotonic()
        self.daemon_log = (self.output / "daemon.stderr").open("ab")
        self.daemon = subprocess.Popen([str(self.installed), "--home", str(self.daemon_home), "daemon", "run"],
            env=self.env, cwd=self.root, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=self.daemon_log, start_new_session=True)

        def ready():
            require(self.daemon.poll() is None, "installed daemon exited before readiness")
            code, stdout, _ = self.command([self.installed, "--home", self.daemon_home, "--json", "--owner", "status"])
            if code:
                return False
            body = json.loads(stdout)
            return bool(body.get("ok") and body.get("result", {}).get("status", {}).get("endpoint"))

        self.wait("installed daemon readiness", ready)
        elapsed = time.monotonic() - started
        self.healthy_doctor()
        return elapsed

    def stop_daemon(self):
        if self.daemon is None:
            return
        self.cli(["daemon", "stop"], installed=True, owner=True)
        self.daemon.wait(timeout=self.timeout)
        require(self.daemon.returncode == 0, "installed daemon failed to stop cleanly")
        self.daemon = None
        self.daemon_log.close()
        self.daemon_log = None

    def service_cli(self, operation, *extra, **kwargs):
        return self.cli(["service", operation, *self.service_args, *extra], **kwargs)

    def service_ready(self):
        code, out, _ = self.command([self.installed, "--home", self.daemon_home, "--json", "--owner", "status"])
        if code:
            return False
        body = json.loads(out)
        return bool(body.get("ok") and body.get("result", {}).get("status", {}).get("endpoint"))

    def cleanup_service(self):
        if self.service_owned is None:
            return
        self.stop_service_observed()
        plan = self.service_cli("remove-plan")
        removed = self.service_cli("remove", "--expect-plan", plan["plan_sha256"])
        require(removed["removed"], "owned service unit was not removed")
        require(not self.service_owned["unit"].exists() and not self.service_owned["record"].exists(),
                "owned service removal left ownership state")
        self.service_owned = None

    def service_control_observed(self, operation):
        # Native activation/removal can complete after the accepted manager call.
        expected_state, transient = {
            "start": ("running", "service has not reached running state"),
            "stop": ("stopped", "service remains loaded after stop"),
        }[operation]
        code, out, err = self.command([self.bootstrap, "--home", self.daemon_home, "--json",
                                      "service", operation, *self.service_args])
        body = json.loads(out)
        require(not err and isinstance(body.get("ok"), bool), "invalid service control envelope")
        self.record("service_" + operation, exit_status=code, response=body)
        if body["ok"]:
            require(code == 0 and body["result"]["state"] == expected_state and
                    body["result"]["daemon_api_readiness_observed"] is False, "invalid service control observation")
        else:
            require(code != 0 and body.get("error", {}).get("code") == "unavailable" and
                    body.get("error", {}).get("message") == transient,
                    "service control failed outside the allowed manager state race")
            self.record("native_manager_transient", operation=operation, message=transient)

    def start_service_observed(self):
        self.service_control_observed("start")

    def stop_service_observed(self):
        self.service_control_observed("stop")
        self.wait("owned native service removal from manager", lambda: self.service_cli("status")["state"] == "stopped")

    def service_failure_recovery(self, profile):
        original_home = self.daemon_home
        broken = self.root / "service-failed-home"
        private_write(broken, "synthetic file prevents daemon-home directory creation")
        logs = self.root / "service-failed-logs"
        self.service_args = ["--prefix", self.prefix, "--kind", "launchd", "--profile-home", profile,
                             "--daemon-home", broken, "--log-dir", logs]
        plan = self.service_cli("plan")
        result = self.service_cli("apply", "--expect-plan", plan["plan_sha256"])
        unit = Path(result["unit"])
        record = self.prefix / ("service-" + result["label"] + ".json")
        self.service_owned = {"unit": unit, "record": record, "label": result["label"]}
        original_unit, original_record = digest(unit), digest(record)
        self.daemon_home = broken
        try:
            self.start_service_observed()
            observed = {}

            def failed():
                code, out, _ = self.command(["/bin/launchctl", "print", f"gui/{os.getuid()}/{result['label']}"])
                require(code == 0, "failed daemon service disappeared from manager")
                found = re.search(r"^\s*last exit code = ([0-9]+)\s*$", out, re.MULTILINE)
                if found is None or int(found.group(1)) == 0:
                    return False
                observed["launchd_last_exit_code"] = int(found.group(1))
                return True

            self.wait("native daemon launch failure", failed)
            require(not self.service_ready(), "invalid daemon-home unexpectedly became ready")
            require(digest(unit) == original_unit and digest(record) == original_record,
                    "native daemon failure discarded its owned service")
            log_evidence = [{"name": path.name, "sha256": digest(path),
                             "text": path.read_text(errors="replace")}
                            for path in logs.iterdir() if path.is_file() and path.stat().st_size]
            require(bool(log_evidence), "native failed daemon produced no owned log evidence")
            self.record("native_failed_daemon", **observed, owned_logs=log_evidence)
            self.stop_service_observed()
            require(broken.read_text() == "synthetic file prevents daemon-home directory creation",
                    "failed start modified synthetic obstruction")
            broken.unlink()
            broken.mkdir(mode=0o700)
            self.start_service_observed()
            self.wait("native service recovery API readiness", self.service_ready)
            self.healthy_doctor()
            self.cleanup_service()
            self.passed("native_launchd_process_failure_and_retry", **observed,
                        ownership_retained=True, retry_ready=True, explicit_cleanup=True)
        finally:
            self.daemon_home = original_home

    def service_lifecycle(self, goal, note, endpoint):
        if platform.system() != "Darwin":
            self.summary["service"] = {"status": "not_run", "reason": "launchd requires macOS; systemd native run is separate"}
            return
        code, _, _ = self.command(["/bin/launchctl", "print", f"gui/{os.getuid()}"])
        if code:
            self.summary["service"] = {"status": "not_run", "reason": "current account GUI launchd domain unavailable"}
            return
        self.summary["service"] = {"status": "running", "manager": "launchd"}
        profile, logs = self.root / "service-home", self.root / "service-logs"
        profile.mkdir(mode=0o700)
        self.service_failure_recovery(profile)
        self.service_args = ["--prefix", self.prefix, "--kind", "launchd", "--profile-home", profile,
                             "--daemon-home", self.daemon_home, "--log-dir", logs]
        plan = self.service_cli("plan")
        result = self.service_cli("apply", "--expect-plan", plan["plan_sha256"])
        require(result["installed"], "native service was not installed")
        unit = Path(result["unit"])
        record = self.prefix / ("service-" + result["label"] + ".json")
        self.service_owned = {"unit": unit, "record": record, "label": result["label"]}
        require(unit.is_relative_to(profile), "service unit escaped synthetic profile")
        config = plistlib.loads(unit.read_bytes())
        for key, expected in (("HOME", profile), ("XDG_CONFIG_HOME", profile / ".config"),
                              ("XDG_DATA_HOME", profile / ".local/share")):
            require(config["EnvironmentVariables"][key] == str(expected), "service inherited an ambient profile")
        plan = self.service_cli("plan")
        require(self.service_cli("apply", "--expect-plan", plan["plan_sha256"])["changed"] is False,
                "native service repeat apply changed its ownership")
        original = digest(record)
        # A real CLI refusal before manager activation, not a simulated launchd failure.
        logs.chmod(0o755)
        try:
            self.service_cli("start", expected_error="denied",
                             expected_message="service log directory must be private mode 0700")
        finally:
            logs.chmod(0o700)
        require(digest(record) == original, "failed service start discarded ownership")
        started = time.monotonic()
        self.start_service_observed()
        self.wait("launchd daemon API readiness", self.service_ready)
        self.unchanged(self.summary["candidate"]["manifest_sha256"])
        require(digest(self.installed) == self.summary["candidate"]["binary_sha256"], "launchd installed candidate identity changed")
        start_seconds = time.monotonic() - started
        self.healthy_doctor()
        plan = self.service_cli("remove-plan")
        self.service_cli("remove", "--expect-plan", plan["plan_sha256"], expected_error="conflict")
        self.cli(["daemon", "stop"], installed=True, owner=True)
        self.wait("launchd loaded stopped process", lambda: self.service_cli("status")["state"] == "loaded")
        plan = self.service_cli("remove-plan")
        self.service_cli("remove", "--expect-plan", plan["plan_sha256"], expected_error="conflict")
        require(digest(record) == original, "loaded service removal refusal changed ownership")
        self.stop_service_observed()
        self.start_service_observed()
        self.wait("launchd restarted daemon API readiness", self.service_ready)
        status = variant(self.cli(["status"], installed=True, owner=True), "status")
        notes = variant(self.cli(["contributions", "--goal", goal], installed=True, agent=True), "contributions")
        require(status["endpoint"] == endpoint and any(item["contribution"] == note for item in notes),
                "native service restart lost identity or durable data")
        self.cleanup_service()
        self.summary["service"] = {"status": "passed", "manager": "launchd", "synthetic_profile": True,
            "startup_to_authenticated_status_seconds": start_seconds,
            "failed_preflight_boundary": "insecure private log directory refused before manager activation",
            "loaded_job_removal_refused": True, "running_job_removal_refused": True,
            "explicit_stop_and_remove": True, "persistent_note_after_restart": True}
        self.passed("native_launchd_lifecycle", **self.summary["service"])

    def sample_idle(self):
        samples = []
        for index in range(self.samples):
            if index:
                time.sleep(self.sample_interval)
            require(self.daemon.poll() is None, "daemon exited during resource sample")
            code, out, _ = self.command(["/bin/ps", "-p", str(self.daemon.pid), "-o", "rss=", "-o", "time="])
            require(code == 0 and len(out.split()) == 2, "owned daemon resource sample unavailable")
            rss, cpu = out.split()
            require(int(rss) > 0, "resident memory sample was zero or negative")
            samples.append({"at_monotonic": time.monotonic(), "rss_bytes": int(rss) * 1024,
                            "cpu_seconds": cpu_seconds(cpu)})
        elapsed = samples[-1]["at_monotonic"] - samples[0]["at_monotonic"]
        cpu = samples[-1]["cpu_seconds"] - samples[0]["cpu_seconds"]
        require(elapsed > 0 and cpu >= 0, "resource counters did not advance consistently")
        return {"wall_seconds": elapsed, "cpu_seconds_delta": cpu,
                "cpu_percent_one_core": 100 * cpu / elapsed,
                "rss_bytes_samples": [sample["rss_bytes"] for sample in samples],
                "sampled_max_rss_bytes": max(sample["rss_bytes"] for sample in samples),
                "interval_seconds": self.sample_interval, "sample_count": len(samples),
                "limitation": "sampled resident set, not peak transfer memory; idle startup background work may remain"}

    def prepare(self):
        self.env = environment(self.root)
        for key in ("HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME", "TMPDIR"):
            Path(self.env[key]).mkdir(mode=0o700)
        self.daemon_home = self.root / "daemon"
        self.prefix = self.root / "software"
        self.installed = self.prefix / "current" / "locust"
        self.bootstrap = self.root / "trusted-bootstrap"
        bootstrap_hash = digest(self.input_bootstrap)
        shutil.copyfile(self.input_bootstrap, self.bootstrap)
        self.bootstrap.chmod(0o700)
        require(digest(self.bootstrap) == bootstrap_hash, "trusted bootstrap changed during copy")
        code, version, stderr = self.command([self.bootstrap, "--version"])
        require(code == 0 and not stderr, "trusted bootstrap version check failed")
        self.summary["bootstrap"] = {"sha256": bootstrap_hash, "version_line": version.strip()}
        self.bundle = self.root / "candidate"
        copy_bundle(self.input_bundle, self.bundle)
        manifest = json.loads((self.bundle / "manifest.json").read_text())
        self.summary["candidate"] = {"manifest_sha256": digest(self.bundle / "manifest.json"),
            "binary_sha256": digest(self.bundle / "locust"), "binary_bytes": (self.bundle / "locust").stat().st_size,
            "manifest": manifest, "execution_policy": "source bundle is inert input; copied candidate executes only after verification",
            "independent_execution_trace": False}
        self.trust = self.output / "disposable-test-trust"
        self.trust.mkdir(mode=0o700)
        self.secret, self.public = self.trust / "secret.key", self.trust / "public.key"
        self.cli(["package", "keygen", "--secret-key", self.secret, "--public-key", self.public])
        require(stat.S_IMODE(self.secret.stat().st_mode) == 0o600, "test private key is not private")
        self.summary["test_trust_key_sha256"] = digest(self.public)
        self.cli(["package", "sign", "--bundle", self.bundle, "--secret-key", self.secret])
        self.current_registry = self.registry("initial", 1)

    def upgrade(self):
        if self.input_baseline is None:
            self.summary["upgrade"] = {"status": "not_run", "reason": "no distinct baseline bundle supplied"}
            return
        baseline = self.root / "baseline"
        copy_bundle(self.input_baseline, baseline)
        baseline_metadata = json.loads((baseline / "manifest.json").read_text())
        baseline_hash = digest(baseline / "manifest.json")
        candidate_metadata = self.summary["candidate"]["manifest"]
        require(all(baseline_metadata[field] == candidate_metadata[field] for field in ("api_version", "protocol_version")),
                "persistent upgrade qualification requires the current API and protocol; cross-cutover stores are refused")
        require(baseline_metadata["source_commit"] != self.summary["candidate"]["manifest"]["source_commit"],
                "upgrade requires a baseline from a distinct source commit")
        self.cli(["package", "sign", "--bundle", baseline, "--secret-key", self.secret])
        original_prefix, original_installed, original_home = self.prefix, self.installed, self.daemon_home
        self.prefix, self.daemon_home = self.root / "upgrade-software", self.root / "upgrade-daemon"
        self.installed = self.prefix / "current/locust"
        try:
            self.apply(bundle=baseline)
            require(self.status()["manifest_sha256"] == baseline_hash, "upgrade baseline did not become active")
            require(digest(self.installed) == digest(baseline / "locust"), "baseline installed bytes differ")
            self.start_daemon()
            principal = variant(self.cli(["agent", "enroll", "qualification"], installed=True, owner=True), "agent_enrolled")["agent"]
            goal = variant(self.cli(["--as", "qualification", "goal", "create", "--title", "Cross-commit upgrade"], installed=True, owner=True), "goal_created")["goal"]
            self.contribution_grant(goal, principal)
            note = variant(self.cli(["contribution", "publish", "--goal", goal, "created by baseline release"], installed=True, agent=True), "recorded")["event"]
            before = variant(self.cli(["status"], installed=True, owner=True), "status")["endpoint"]
            self.stop_daemon()
            result = self.apply()
            require(result["changed"], "cross-commit upgrade did not change active release")
            self.unchanged(self.summary["candidate"]["manifest_sha256"])
            require(digest(self.installed) == self.summary["candidate"]["binary_sha256"], "upgraded executable differs from candidate")
            self.start_daemon()
            after = variant(self.cli(["status"], installed=True, owner=True), "status")["endpoint"]
            notes = variant(self.cli(["contributions", "--goal", goal], installed=True, agent=True), "contributions")
            require(before == after and any(item["contribution"] == note and item["text"] == "created by baseline release" for item in notes),
                    "cross-commit upgrade lost daemon identity or durable note")
            self.stop_daemon()
            self.summary["upgrade"] = {"status": "passed", "baseline_manifest_sha256": baseline_hash,
                "baseline_binary_sha256": digest(baseline / "locust"), "baseline_manifest": baseline_metadata,
                "candidate_manifest_sha256": self.summary["candidate"]["manifest_sha256"],
                "real_native_version_probes": True, "persistent_note": True, "persistent_identity": True}
            self.passed("real_cross_commit_upgrade", **self.summary["upgrade"])
        finally:
            if self.daemon is not None:
                self.stop_daemon()
            self.prefix, self.installed, self.daemon_home = original_prefix, original_installed, original_home

    def flow(self):
        verified = self.cli(["package", "verify", "--bundle", self.bundle, "--trust-key", self.public,
                             "--withdrawals", self.current_registry])
        require(verified["verified"] and not verified["executed"], "verification executed the candidate")
        manifest = verified["manifest_sha256"]
        require(manifest == self.summary["candidate"]["manifest_sha256"], "verified manifest identity differs from copied bytes")
        for name, relative, expected in (("signature", "manifest.sig", "denied"),
                                        ("payload", "skills/locust/SKILL.md", "corrupted"),
                                        ("manual", "manual.tar", "corrupted")):
            bad = self.root / ("bad-" + name)
            copy_bundle(self.bundle, bad, signed=True)
            (bad / relative).write_bytes(b"x" * 64 if name == "signature" else b"tampered skill")
            self.cli(["package", "verify", "--bundle", bad, "--trust-key", self.public,
                      "--withdrawals", self.current_registry], expected_error=expected)
        plan = self.plan()
        require(not self.prefix.exists(), "installation plan wrote the target")
        self.cli(["install", "apply", *self.inputs(), "--expect-plan", "0" * 64], expected_error="conflict")
        require(not self.prefix.exists(), "wrong plan created an installation")
        self.apply(plan)
        self.unchanged(manifest)
        require(digest(self.installed) == self.summary["candidate"]["binary_sha256"], "installed bytes differ from candidate")
        import build_release
        installed_manual = self.prefix / "current/manual.tar"
        identity = build_release.verify_manual(installed_manual.read_bytes(),
            verified["manifest"]["source_commit"], verified["manifest"]["protocol_version"],
            verified["manifest"]["api_version"])
        self.passed("installed_manual_read_without_checkout", source_commit=identity["source_commit"],
                    files=len(identity["files"]), sha256=digest(installed_manual))
        require(self.apply()["changed"] is False, "repeat install was not idempotent")
        self.passed("verified_native_install_repeat", manifest_sha256=manifest,
                    real_version_probe=True, installed_binary_bytes=self.installed.stat().st_size)
        self.passed("signature_payload_and_plan_refusal")

        old_registry = self.current_registry
        stale_plan = self.plan()
        withdrawn_other = "e" * 64
        self.current_registry = self.registry("advanced", 2, [withdrawn_other])
        self.apply()
        trust_state = self.status()
        trust_state["trust_state_sha256"] = digest(self.prefix / "trust-state.json")
        require(trust_state["withdrawals_sequence"] == 2, "advanced withdrawal policy was not retained")
        self.cli(["install", "apply", *self.inputs(), "--expect-plan", stale_plan["plan_sha256"]], expected_error="conflict")
        self.unchanged_trust(trust_state)
        self.cli(["install", "plan", *self.inputs(registry=old_registry)], expected_error="denied")
        self.unchanged_trust(trust_state)
        equivocation = self.registry("equivocation", 2, ["d" * 64])
        self.cli(["install", "plan", *self.inputs(registry=equivocation)], expected_error="denied")
        self.unchanged_trust(trust_state)
        dropped = self.registry("dropped-withdrawal", 3)
        self.cli(["install", "plan", *self.inputs(registry=dropped)], expected_error="denied")
        self.unchanged_trust(trust_state)
        withdrawn = self.registry("withdrawn-candidate", 3, [withdrawn_other, manifest])
        self.cli(["install", "plan", *self.inputs(registry=withdrawn)], expected_error="denied")
        self.unchanged_trust(trust_state)
        alternate = self.root / "alternate-signer"
        copy_bundle(self.bundle, alternate)
        other_secret, other_public = self.trust / "other-secret.key", self.trust / "other-public.key"
        self.cli(["package", "keygen", "--secret-key", other_secret, "--public-key", other_public])
        self.cli(["package", "sign", "--bundle", alternate, "--secret-key", other_secret])
        other_registry = self.registry("other-signer", 4, [withdrawn_other], key=other_secret)
        self.cli(["install", "plan", *self.inputs(bundle=alternate, public=other_public, registry=other_registry)], expected_error="denied")
        self.unchanged_trust(trust_state)
        self.unchanged(manifest)
        self.passed("withdrawal_and_trust_rollback_refusal", retained_sequence=self.status()["withdrawals_sequence"],
                    trust_state_sha256=trust_state["trust_state_sha256"])

        for name, error in (("wrong-version", "corrupted"), ("unstartable", "unavailable")):
            bad = self.root / name
            copy_bundle(self.bundle, bad)
            metadata = json.loads((bad / "manifest.json").read_text())
            if name == "wrong-version":
                metadata["version"] = "999.0.0"
            else:
                binary = bad / "locust"
                binary.write_bytes(binary.read_bytes()[:64])
                record = next(item for item in metadata["files"] if item["path"] == "locust")
                record.update(size=64, sha256=digest(binary))
            (bad / "manifest.json").write_text(json.dumps(metadata))
            self.cli(["package", "sign", "--bundle", bad, "--secret-key", self.secret])
            bad_plan = self.plan(bundle=bad)
            self.cli(["install", "apply", *self.inputs(bundle=bad), "--expect-plan", bad_plan["plan_sha256"]], expected_error=error,
                     expected_message="verified executable version does not match" if name == "wrong-version" else "verified candidate could not start")
            self.unchanged(manifest)
        require(self.apply()["changed"] is False, "valid retry after failed native probe changed the release")
        self.passed("real_candidate_probe_failure_preserves_active_release", valid_retry=True)

        # Reproduce the durable filesystem boundary of an interrupted initial
        # copy. This is a staged-state fixture, not a claim of a timed SIGKILL.
        interrupted = self.root / "interrupted-software"
        partial = interrupted / ".stage-interrupted-fixture"
        partial.mkdir(parents=True, mode=0o700)
        interrupted.chmod(0o700)
        private_write(partial / "locust", b"incomplete candidate; must never be executed")
        partial_before = digest(partial / "locust")
        recovered = self.apply(prefix=interrupted)
        require(recovered["installed"] and digest(interrupted / "current/locust") == self.summary["candidate"]["binary_sha256"],
                "interrupted staging fixture prevented a complete verified install")
        require(digest(partial / "locust") == partial_before, "installer rewrote unknown partial staging")
        self.passed("partial_staging_fixture_recovery", partial_state_preserved=True,
                    evidence_boundary="preexisting partial directory; not timed process interruption")

        startup = self.start_daemon()
        idle = self.sample_idle()
        principal = variant(self.cli(["agent", "enroll", "qualification"], installed=True, owner=True), "agent_enrolled")["agent"]
        goal = variant(self.cli(["--as", "qualification", "goal", "create", "--title", "Installed synthetic goal"], installed=True, owner=True), "goal_created")["goal"]
        self.contribution_grant(goal, principal)
        note = variant(self.cli(["contribution", "publish", "--goal", goal, "persist across installed daemon restart"], installed=True, agent=True), "recorded")["event"]
        endpoint = variant(self.cli(["status"], installed=True, owner=True), "status")["endpoint"]
        self.stop_daemon()
        restarted = self.start_daemon()
        status = variant(self.cli(["status"], installed=True, owner=True), "status")
        notes = variant(self.cli(["contributions", "--goal", goal], installed=True, agent=True), "contributions")
        require(status["endpoint"] == endpoint and any(a["agent"] == principal for a in status["agents"]),
                "installed daemon identity changed after restart")
        require(any(item["contribution"] == note and item["text"] == "persist across installed daemon restart" for item in notes),
                "installed daemon lost persisted note")
        self.stop_daemon()
        self.passed("installed_daemon_fresh_database_restart_doctor", goal=goal, note=note, endpoint=endpoint,
                    startup_to_status_seconds=startup, restart_to_status_seconds=restarted, idle_resources=idle)

        self.service_lifecycle(goal, note, endpoint)
        self.upgrade()

        before = data_fingerprint(self.daemon_home)
        require(bool(before), "no daemon data existed for preservation check")
        release = self.prefix / "releases" / manifest
        (release / "skills/locust/SKILL.md").write_text("local user modification\n")
        private_write(self.prefix / "unrelated.txt", "preserve unrelated user file\n")
        unplan = self.cli(["install", "uninstall-plan", "--prefix", self.prefix])
        removed = self.cli(["install", "uninstall", "--prefix", self.prefix, "--expect-plan", unplan["plan_sha256"]])
        require(removed["uninstalled"] and str(release) in removed["retained_modified_or_unknown"], "modified release was not retained")
        require((release / "skills/locust/SKILL.md").read_text() == "local user modification\n", "modified skill was deleted")
        require((self.prefix / "unrelated.txt").read_text() == "preserve unrelated user file\n", "unrelated file changed")
        require(before == data_fingerprint(self.daemon_home), "uninstall changed daemon data")
        self.cli(["install", "plan", *self.inputs(registry=old_registry)], expected_error="denied")
        require(self.status()["installed"] is False, "uninstall left current release selected")
        clean = self.root / "clean-software"
        self.apply(prefix=clean)
        unplan = self.cli(["install", "uninstall-plan", "--prefix", clean])
        removed = self.cli(["install", "uninstall", "--prefix", clean, "--expect-plan", unplan["plan_sha256"]])
        require(not removed["retained_modified_or_unknown"] and not (clean / "releases" / manifest).exists(),
                "unchanged owned release was not removed")
        self.passed("uninstall_preserves_data_modified_files_and_trust", daemon_entry_count=len(before),
                    unchanged_owned_release_removed=True)

    def run(self):
        try:
            self.root = Path(tempfile.mkdtemp(prefix="lci-", dir="/tmp")).resolve()
            try:
                self.prepare()
                self.flow()
                self.summary["status"] = "passed"
            finally:
                if self.daemon is not None:
                    try:
                        self.stop_daemon()
                    except Exception:
                        os.killpg(self.daemon.pid, signal.SIGKILL)
                        self.daemon.wait()
                        self.daemon = None
                        self.summary["failures"].append("owned daemon required forced cleanup")
                        self.summary["status"] = "failed"
                if self.daemon_log is not None:
                    self.daemon_log.close()
                try:
                    self.cleanup_service()
                except Exception as error:
                    self.summary["failures"].append("native service cleanup failed: " + str(error))
                    self.summary["status"] = "failed"
                if self.service_owned is None:
                    shutil.rmtree(self.root)
                else:
                    self.summary["retained_fixture_for_service_cleanup"] = str(self.root)
        except Exception as error:
            if self.summary["service"]["status"] == "running":
                self.summary["service"]["status"] = "failed"
            self.summary["status"] = "failed"
            self.summary["failures"].append(str(error))
        finally:
            trust = self.output / "disposable-test-trust"
            if trust.exists():
                shutil.rmtree(trust)
            self.summary["test_private_keys_removed"] = not trust.exists()
            self.summary["finished_at"] = datetime.now(timezone.utc).isoformat()
            private_write(self.output / "summary.json", json.dumps(redact(self.summary), indent=2, sort_keys=True) + "\n")
        return self.summary["status"] == "passed"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bootstrap", required=True, type=Path, help="Explicitly trusted bootstrap executable")
    parser.add_argument("--bundle", required=True, type=Path, help="Extracted unsigned candidate; never run directly")
    parser.add_argument("--baseline-bundle", type=Path, help="Optional inert bundle from a distinct earlier commit for native upgrade")
    parser.add_argument("--timeout-seconds", type=float, default=60)
    parser.add_argument("--sample-interval", type=float, default=1)
    parser.add_argument("--samples", type=int, default=3)
    args = parser.parse_args(argv)
    if not args.bootstrap.is_absolute() or not args.bootstrap.is_file() or not os.access(args.bootstrap, os.X_OK):
        parser.error("--bootstrap must name an absolute executable path whose execution you trust")
    if not args.bundle.is_absolute() or not args.bundle.is_dir():
        parser.error("--bundle must name an absolute extracted candidate directory")
    if args.baseline_bundle is not None and (not args.baseline_bundle.is_absolute() or not args.baseline_bundle.is_dir()):
        parser.error("--baseline-bundle must be an absolute extracted baseline directory")
    if any(not math.isfinite(v) or v <= 0 for v in (args.timeout_seconds, args.sample_interval)) or args.samples < 2:
        parser.error("positive finite timeout/interval and at least two resource samples are required")
    name = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + os.urandom(4).hex()
    output = Path(__file__).resolve().parents[1] / "output" / "installation" / name
    check = InstallationCheck(args.bootstrap, args.bundle, output, args.timeout_seconds, args.sample_interval, args.samples, args.baseline_bundle)
    passed = check.run()
    print(json.dumps({"ok": passed, "summary": str(output / "summary.json"), "production_release_qualified": False}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
