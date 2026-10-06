"""Private, persistent production-daemon fixture for client qualification.

ProductionDaemon.call returns the CLI result with capability/secret fields
redacted. Errors carry a stable ``code`` and never include raw subprocess
output. The fixture creates only its enrolled coordinator/worker and private
goal; assignment authorization and all target worker actions remain the
orchestrator's responsibility. No existing account or client profile is used.
"""

import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import time

from .runtime import private_write


# The production endpoint currently enables port mapping. Disable discovery
# and relays AND deny non-loopback networking at the OS boundary; configuration
# alone is not evidence that no external network operation was attempted.
DAEMON_GUARD = (
    '(version 1)(allow default)(deny network*)'
    '(allow network-bind (local ip "localhost:*"))'
    '(allow network-inbound (local ip "localhost:*"))'
    '(allow network-outbound (remote ip "localhost:*"))'
    '(allow network-bind (local unix-socket))'
    '(allow network-inbound (local unix-socket))'
    '(allow network-outbound (remote unix-socket))'
)
NETWORK = {"LOCUST_RELAY": "none", "LOCUST_LOOKUP": "none", "LOCUST_BIND": "127.0.0.1:0"}
SECRET_FIELDS = {"credential", "session_secret", "secret", "secret_key", "ticket", "bytes"}
TICKET = re.compile(r"locust-invite-[A-Za-z0-9_-]+")
ERROR_CODES = {"denied", "not_found", "invalid", "conflict", "claim_held", "superseded",
               "idempotency_mismatch", "limit_exceeded", "unavailable", "halted",
               "unsupported_version", "corrupted", "internal", "level_required", "not_eligible", "read_only"}
PUBLIC_ID = re.compile(r"[0-9a-f]{64}\Z")


class ProductionError(RuntimeError):
    def __init__(self, message, code="unavailable"):
        super().__init__(message)
        self.code = code


def redact(value, secrets=()):
    """Remove capabilities, opaque bytes and known private file contents."""
    if isinstance(value, dict):
        return {key: "<redacted>" if key.lower() in SECRET_FIELDS else redact(item, secrets)
                for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        if any(list(secret) == list(value) for secret in secrets):
            return "<redacted>"
        return [redact(item, secrets) for item in value]
    if isinstance(value, str):
        value = TICKET.sub("<redacted-ticket>", value)
        for secret in secrets:
            value = re.sub(re.escape(secret.hex()), "<redacted>", value, flags=re.IGNORECASE)
        return value
    return value


def operation(args):
    """Record operation names, never arbitrary argument payloads."""
    names = list(map(str, args))
    if not names:
        return "missing"
    if names[0] in {"call", "agent", "goal", "task", "session", "workspace", "contribution", "attempt", "review", "scope", "work", "formation", "daemon"}:
        names = names[:2]
    else:
        names = names[:1]
    return ".".join(name if re.fullmatch(r"[a-z][a-z0-9_.-]*", name) else "invalid"
                    for name in names)


class ProductionDaemon:
    def __init__(self, profile, binary, timeout_seconds):
        timeout = float(timeout_seconds)
        if not math.isfinite(timeout) or timeout <= 0:
            raise ValueError("timeout_seconds must be positive and finite")
        self.profile = profile
        self.binary = Path(binary).resolve()
        if not self.binary.is_file() or not os.access(self.binary, os.X_OK):
            raise ProductionError("compiled Locust binary is unavailable")
        self.timeout_seconds = timeout
        self.home = Path(profile.root) / "daemon"
        self.credential = self.home / "agents" / "qualification.credential"
        self.session = self.home / "sessions" / "qualification.secret"
        self.principal = None
        self.goal = None
        self.instance = None
        self.endpoint = None
        self.binary_metadata = None
        self.events = Path(profile.logs) / "production-daemon.jsonl"
        private_write(self.events, b"")
        self._process = None
        self._generation = 0

    def environment(self):
        """Isolated process environment; no owner or bridge credentials."""
        env = self.profile.environment(self.binary)
        env.update(NETWORK)
        return env

    def _secrets(self):
        result = []
        for path in (self.home / "owner.credential", self.credential, self.session):
            # Read only helper-owned private files for output redaction. Never
            # follow a substituted path or read an ambient credential.
            try:
                stat = path.lstat()
                if path.is_symlink() or not path.is_file() or stat.st_mode & 0o777 != 0o600:
                    continue
                contents = path.read_bytes()
            except OSError:
                continue
            if len(contents) == 32:
                result.append(contents)
        return result

    def _record(self, event, **fields):
        record = redact({"event": event, "generation": self._generation, **fields}, self._secrets())
        with self.events.open("a", encoding="utf-8") as output:
            output.write(json.dumps(record, sort_keys=True) + "\n")
        return record

    def _guard(self):
        guard = Path("/usr/bin/sandbox-exec")
        if platform.system() != "Darwin" or not guard.is_file() or not os.access(guard, os.X_OK):
            self._record("unavailable", reason="macOS external-network guard unavailable")
            raise ProductionError("macOS external-network guard unavailable")
        return [str(guard), "-p", DAEMON_GUARD]

    def command(self, args, owner=False):
        """CLI argv for this fixture only; callers retain process ownership."""
        command = [str(self.binary), "--home", str(self.home), "--json"]
        if owner:
            command.append("--owner")
        else:
            command += ["--credential", str(self.credential), "--session", str(self.session)]
        return command + list(map(str, args))

    def _invoke(self, args, owner=False, timeout=None, startup=False):
        name = operation(args)
        try:
            output = subprocess.run(self._guard() + self.command(args, owner),
                                    cwd=self.profile.root, env=self.environment(),
                                    capture_output=True, text=True,
                                    timeout=self.timeout_seconds if timeout is None else timeout,
                                    check=False)
        except subprocess.TimeoutExpired:
            self._record("cli_timeout", operation=name, owner=owner)
            raise ProductionError("production CLI operation timed out") from None
        except (OSError, UnicodeError):
            self._record("cli_unavailable", operation=name, owner=owner)
            raise ProductionError("production CLI operation could not run") from None
        try:
            body = json.loads(output.stdout)
            if not isinstance(body, dict) or not isinstance(body.get("ok"), bool):
                raise ValueError
            if body["ok"]:
                if "result" not in body or output.returncode not in (0, 20, 21):
                    raise ValueError
            elif (not isinstance(body.get("error"), dict)
                  or not isinstance(body["error"].get("code"), str)
                  or body["error"]["code"] not in ERROR_CODES):
                raise ValueError
        except (json.JSONDecodeError, ValueError):
            self._record("cli_invalid_response", operation=name, owner=owner, exit_code=output.returncode)
            raise ProductionError("production CLI returned an invalid envelope") from None
        self._record("cli", operation=name, owner=owner, exit_code=output.returncode,
                     response=body, stderr_bytes=len(output.stderr.encode()))
        if output.stderr:
            raise ProductionError("production JSON CLI wrote unexpected stderr")
        if not body["ok"]:
            code = body["error"]["code"]
            if startup and code == "unavailable":
                return None
            raise ProductionError("production CLI operation failed: " + code, code)
        return redact(body["result"], self._secrets())

    def call(self, args, owner=False):
        if self._process is None or self._process.poll() is not None:
            raise ProductionError("production daemon is not running")
        result = self._invoke(args, owner=owner)
        if owner and "--plan" not in args:
            from owner_plans import confirmation_arguments
            confirmed = confirmation_arguments(args, result)
            if confirmed is not None:
                return self._invoke(confirmed, owner=owner)
        return result

    def _verify_binary(self):
        stat = self.binary.stat()
        with self.binary.open("rb") as source:
            digest = hashlib.file_digest(source, "sha256").hexdigest()
        metadata = {"path": str(self.binary), "size": stat.st_size,
                    "mtime_ns": stat.st_mtime_ns, "sha256": digest}
        if self.binary_metadata is not None and self.binary_metadata != metadata:
            raise ProductionError("compiled Locust binary changed during qualification")
        self.binary_metadata = metadata
        self._record("binary", **metadata)

    def _start(self):
        self._verify_binary()
        guard = self._guard()
        self.home.mkdir(mode=0o700, exist_ok=True)
        if self.home.is_symlink() or not self.home.is_dir() or self.home.stat().st_mode & 0o777 != 0o700 or self.home.stat().st_uid != os.getuid():
            raise ProductionError("production daemon home must be an owned private directory")
        self._generation += 1
        self._process = subprocess.Popen(
            guard + [str(self.binary), "--home", str(self.home), "daemon", "run"],
            cwd=self.profile.root, env=self.environment(), stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
        self._record("daemon_started", pid=self._process.pid, network=NETWORK,
                     network_guard="macOS loopback_and_unix_only")
        deadline = time.monotonic() + self.timeout_seconds
        while self._process.poll() is None:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise ProductionError("production daemon startup timed out")
            result = self._invoke(["status"], owner=True, timeout=remaining, startup=True)
            if result is not None:
                status = result.get("status", {}) if isinstance(result, dict) else {}
                endpoint = status.get("endpoint")
                if isinstance(endpoint, str) and PUBLIC_ID.fullmatch(endpoint):
                    if self.endpoint is not None and endpoint != self.endpoint:
                        raise ProductionError("production endpoint identity changed on restart")
                    self.endpoint = endpoint
                    if not (self.home / "locust.db").is_file():
                        raise ProductionError("production daemon did not create its persistent SQLite store")
                    self._record("daemon_ready", endpoint=endpoint, sqlite=True)
                    return
            time.sleep(min(0.02, max(0, deadline - time.monotonic())))
        self._record("daemon_startup_exit", exit_code=self._process.returncode)
        raise ProductionError("production daemon exited before becoming ready")

    def _stop(self):
        process = self._process
        if process is None:
            return
        forced = False
        if process.poll() is None:
            # Signal only our unreaped child. Never kill by a historic PID,
            # socket path, name, process-group search or unrelated profile.
            process.send_signal(signal.SIGTERM)
            try:
                process.wait(timeout=self.timeout_seconds)
            except subprocess.TimeoutExpired:
                forced = True
                process.kill()
                process.wait(timeout=self.timeout_seconds)
        self._record("daemon_exit", exit_code=process.returncode, forced_cleanup=forced,
                     socket_removed=not (self.home / "daemon.sock").exists())
        self._process = None
        if forced or process.returncode != 0 or (self.home / "daemon.sock").exists():
            raise ProductionError("production daemon did not exit cleanly")

    def _enroll_identity(self):
        enrolled = self.call(["agent", "enroll", "qualification"], owner=True)
        self.principal = enrolled.get("agent_enrolled", {}).get("agent")
        if not isinstance(self.principal, str) or not PUBLIC_ID.fullmatch(self.principal):
            raise ProductionError("production enrollment did not return a principal")
        session = self.call(["session", "create", str(self.session)], owner=True)
        self.instance = session.get("instance")
        if not isinstance(self.instance, str) or not re.fullmatch(r"[0-9a-f]{32}", self.instance):
            raise ProductionError("production session creation did not return an instance")

    def __enter__(self):
        try:
            self._start()
            self._enroll_identity()
            for path in (self.credential, self.session):
                if path.is_symlink() or not path.is_file() or path.stat().st_mode & 0o777 != 0o600 or path.stat().st_size != 32:
                    raise ProductionError("production authentication files are not private 32-byte secrets")
            formation = self.call(["formation", "example", "directed"])
            formation["context"]["inputs"] = {"snapshot": {"kind": "artifact", "required": False}}
            created = self.call(["--agent", "qualification", "goal", "create", "--title", "Production client qualification",
                "--formation-json", json.dumps(formation)], owner=True)
            self.goal = created.get("goal_created", {}).get("goal")
            if not isinstance(self.goal, str) or not PUBLIC_ID.fullmatch(self.goal):
                raise ProductionError("production goal creation did not return a goal")
            self.call(["--agent", "qualification", "level", "--goal", self.goal, "ask"], owner=True)
            self._record("fixture_ready", principal=self.principal, goal=self.goal,
                         role="same_principal_worker_and_host_agent", level="ask")
            return self
        except BaseException:
            try:
                self._stop()
            except (ProductionError, OSError, subprocess.TimeoutExpired):
                pass
            raise

    def restart(self):
        """Restart the owned production process and verify persisted identity."""
        self._stop()
        try:
            self._start()
            status = self.call(["goal", "status", "--goal", self.goal])
            persisted = status.get("goal_status", {})
            if persisted.get("goal") != self.goal or persisted.get("host") != self.principal:
                raise ProductionError("production goal did not survive restart")
            self._record("restart_verified", principal=self.principal, goal=self.goal,
                         endpoint=self.endpoint)
        except BaseException:
            try:
                self._stop()
            except (ProductionError, OSError, subprocess.TimeoutExpired):
                pass
            raise

    def __exit__(self, exc_type, _value, _traceback):
        try:
            self._stop()
        except (ProductionError, OSError, subprocess.TimeoutExpired):
            if exc_type is None:
                raise
