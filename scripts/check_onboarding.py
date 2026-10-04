#!/usr/bin/env python3
"""Qualify installed onboarding with disposable trust and private synthetic profiles.

This runs Locust administration and generated CLI launchers only. It does not run
a model or native AI client, prove native tool discovery, or establish production
release trust. The selected native manager is the only managed-service boundary
tested. Every deadline is supplied by the caller. Cleanup failures retain the
private fixture so an active service never loses its executable or state files.
"""

import argparse
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import tempfile
import time
import tomllib

from check_installation import copy_bundle, data_fingerprint, digest, private_write
from client_qualification.runtime import Profile


class QualificationError(Exception):
    pass


def require(condition, message):
    if not condition:
        raise QualificationError(message)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


class PrivateProfile(Profile):
    """Reuse the isolated environment and cleanup, with a short socket-safe root."""
    def __init__(self):
        self.root = Path(tempfile.mkdtemp(prefix="lup-", dir="/tmp")).resolve()
        self.root.chmod(0o700)
        for name in ("home", "config", "tmp", "workspace", "fixture", "logs"):
            setattr(self, name, self.root / name)
            getattr(self, name).mkdir(mode=0o700)


class OnboardingCheck:
    def __init__(self, args):
        self.args = args
        self.timeout = args.timeout_ms / 1000
        args.output.mkdir(parents=True, mode=0o700, exist_ok=False)
        args.output.chmod(0o700)
        self.receipts = args.output / "operations.jsonl"
        private_write(self.receipts, b"")
        self.profile = None
        self.installed = None
        self.foreground = None
        self.service_attempted = False
        self.unit = None
        self.stage = "prepare"
        self.summary = {
            "status": "running", "started_at": datetime.now(timezone.utc).isoformat(),
            "harness_sha256": digest(Path(__file__)), "timeout_ms": args.timeout_ms,
            "platform": platform.system(), "architecture": platform.machine(),
            "service_kind": args.service, "model_ready": False,
            "limits": {"native_client_execution": False, "native_tool_discovery": False,
                       "real_model": False, "production_release_trust": False,
                       "trust": "disposable test signer", "service_scope": "selected manager on this host"},
            "checks": {},
        }

    def record(self, label, **projection):
        with self.receipts.open("a") as target:
            target.write(json.dumps({"operation": label, **projection}, sort_keys=True) + "\n")

    def cli(self, label, arguments, *, binary=None, owner=False, bound=False, optional=False):
        self.stage = label
        executable = binary or self.installed
        command = [str(executable), "--json"]
        if not bound:
            command += ["--home", str(self.home)]
        if owner:
            command += ["--owner"]
        command += list(map(str, arguments))
        try:
            result = subprocess.run(command, cwd=self.profile.workspace, env=self.env,
                                    stdin=subprocess.DEVNULL, capture_output=True, timeout=self.timeout)
        except subprocess.TimeoutExpired as error:
            self.record(label, timed_out=True, stdout_sha256=sha256(error.stdout or b""),
                        stderr_sha256=sha256(error.stderr or b""))
            raise QualificationError("caller deadline expired: " + label) from None
        try:
            body = json.loads(result.stdout)
        except (ValueError, UnicodeError):
            body = {}
        code = body.get("error", {}).get("code") if isinstance(body, dict) else None
        self.record(label, exit_code=result.returncode, timed_out=False,
                    stdout_sha256=sha256(result.stdout), stderr_sha256=sha256(result.stderr),
                    error_code=code if code in {"denied", "invalid", "conflict", "unavailable", "corrupted",
                                               "unsupported_version", "not_found", "internal"} else None)
        success = isinstance(body, dict) and body.get("ok") is True and result.returncode == 0
        if optional and not success:
            return None
        require(success and "result" in body, "CLI operation failed: " + label)
        # up deliberately writes review information to stderr. Only its hash is retained.
        return body["result"]

    def install(self):
        bundle = self.profile.root / "bundle"
        copy_bundle(self.args.bundle, bundle)
        secret = self.profile.fixture / "signing.secret"
        public = self.profile.fixture / "signing.public"
        bootstrap = lambda label, arguments: self.cli(label, arguments, binary=self.args.bootstrap)
        bootstrap("keygen", ["package", "keygen", "--secret-key", secret, "--public-key", public])
        bootstrap("sign", ["package", "sign", "--bundle", bundle, "--secret-key", secret])
        registry = self.profile.fixture / "withdrawals.json"
        private_write(registry, json.dumps({"format": "locust-withdrawals-v1", "sequence": 1,
                                            "withdrawn_manifest_sha256": []}))
        bootstrap("sign-withdrawals", ["package", "sign-withdrawals", "--registry", registry, "--secret-key", secret])
        self.prefix = self.profile.root / "software"
        inputs = ["--prefix", self.prefix, "--bundle", bundle, "--trust-key", public, "--withdrawals", registry]
        proposed = bootstrap("install-plan", ["install", "plan", *inputs])
        bootstrap("install-apply", ["install", "apply", *inputs, "--expect-plan", proposed["plan_sha256"]])
        self.installed = self.prefix / "current/locust"
        status = self.cli("install-status", ["install", "status", "--prefix", self.prefix])
        require(status["installed"] and not status["withdrawn"]
                and status["manifest_sha256"] == digest(bundle / "manifest.json")
                and digest(self.installed) == digest(bundle / "locust"), "installed artifact provenance mismatch")
        self.summary["artifact"] = {"bootstrap_sha256": digest(self.args.bootstrap),
            "installed_binary_sha256": digest(self.installed), "manifest_sha256": status["manifest_sha256"],
            "test_trust_public_key_sha256": digest(public), "prefix_inference": "up and agent add omit --prefix"}
        # The private signer is not needed by any daemon or service.
        secret.unlink()
        self.summary["checks"]["test_signed_install"] = True

    def service(self, operation, *extra, optional=False):
        arguments = ["service", operation, "--prefix", self.prefix, "--kind", self.args.service,
                     "--profile-home", self.service_home, "--daemon-home", self.home, "--log-dir", self.logs]
        return self.cli("service-" + operation, [*arguments, *extra], optional=optional)

    def wait(self, label, check):
        deadline = time.monotonic() + self.timeout
        while not check():
            require(time.monotonic() < deadline, "caller deadline expired: " + label)
            time.sleep(min(0.02, max(0, deadline - time.monotonic())))

    def lock_held(self):
        path = self.home / "daemon.lock"
        if not path.exists():
            return False
        with path.open("rb") as source:
            try:
                fcntl.flock(source, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                return True
            fcntl.flock(source, fcntl.LOCK_UN)
        return False

    def pid(self):
        result = self.cli("doctor", ["doctor"], owner=True)
        check = next(row for row in result["checks"] if row["name"] == "daemon_lock")
        matched = re.fullmatch(r"held by process ([1-9][0-9]*)", check["detail"])
        require(check["ok"] and matched is not None and self.lock_held(), "daemon PID not independently observed")
        pid = int(matched.group(1))
        require(int((self.home / "daemon.lock").read_text().strip()) == pid, "doctor and lock PID disagree")
        return pid

    def owner_agents(self):
        return self.cli("owner-status", ["status"], owner=True)["status"]["agents"]

    def flow(self):
        codex = self.profile.home / ".codex/config.toml"
        claude = self.profile.home / ".claude.json"
        private_write(codex, 'model = "qualification-preserve"\napproval_policy = "on-request"\n'
                      '[qualification]\nmarker = "codex-unrelated"\n')
        private_write(claude, json.dumps({"qualification_marker": "claude-unrelated",
                                         "permissions": {"defaultMode": "default"}}))
        unselected = self.profile.root / "unselected-profile"
        private_write(unselected / ".codex/config.toml", 'marker = "unselected-profile"\n')
        pi = self.profile.home / ".pi/agent/mcp.json"
        private_write(pi, '{"qualification_marker":"unselected-client"}\n')
        unselected_before, pi_before = data_fingerprint(unselected), digest(pi)
        up = ["up", "--client", "codex,claude", "--profile-home", self.profile.home,
              "--workspace", self.profile.workspace, "--service", self.args.service,
              "--service-profile-home", self.service_home, "--log-dir", self.logs,
              "--wait-ms", self.args.timeout_ms]
        before = data_fingerprint(self.profile.root)
        proposed = self.cli("up-plan", [*up, "--plan"])
        require(proposed["changed"] is False and before == data_fingerprint(self.profile.root)
                and not self.home.exists(), "selected plan modified fixture or created daemon state")
        self.summary["checks"]["selected_plan_read_only_before_daemon"] = True
        if self.args.service != "none":
            self.unit = Path(proposed["service"]["plan"]["unit_path"])
            require(self.unit.is_relative_to(self.service_home) and not os.path.lexists(self.unit),
                    "service plan escaped the new private manager profile or collided")
            self.summary["service_label"] = proposed["service"]["plan"]["label"]
            self.service_attempted = True
        else:
            self.foreground = subprocess.Popen([str(self.installed), "--home", str(self.home), "daemon", "run"],
                cwd=self.profile.workspace, env=self.env, stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        first = self.cli("up", [*up, "--yes"])
        require(first["configuration_ready"] is True and first["daemon_api_ready"] is True
                and first["model_ready"] is False and first["grants_added"] is False,
                "up overstated readiness or changed grants")
        clients = {row["client"]: row for row in first["clients"]}
        require(set(clients) == {"codex", "claude"}, "up did not configure exactly the selected clients")
        require(len({row["principal"] for row in clients.values()}) == 2
                and len({row["instance"] for row in clients.values()}) == 2, "selected clients share an identity or session")
        initial_pid = self.pid()
        if self.foreground is not None:
            require(initial_pid == self.foreground.pid, "foreground daemon lock belongs to another process")
        if self.args.service != "none":
            require(self.service("status")["state"] == "running" and self.unit.is_file(), "native task service is not running")
        agents = {row["agent"]: row for row in self.owner_agents()}
        require(set(agents) == {row["principal"] for row in clients.values()}, "owner status found missing or extra principals")
        self.summary["clients"] = {}
        for client, row in clients.items():
            require(row["model_ready"] is False and row["grants_added"] is False
                    and not agents[row["principal"]]["grants"]["manage_goals"], "agent enrolled with work permission")
            launcher = Path(row["launcher"])
            require(launcher.is_relative_to(self.profile.home), "launcher escaped selected profile")
            observed = self.cli(client + "-launcher-status", ["status"], binary=launcher, bound=True)["status"]["agents"]
            require(len(observed) == 1 and observed[0]["agent"] == row["principal"], "launcher authenticated wrong principal")
            files = {key: Path(row[key]) for key in ("credential_file", "session_file")}
            require(all(path.is_relative_to(self.home) and not path.is_symlink()
                        and path.stat().st_size == 32 and path.stat().st_mode & 0o7777 == 0o600
                        and path.stat().st_uid == os.getuid() and path.stat().st_nlink == 1
                        for path in files.values()), "identity files are not private fixture secrets")
            hashes = {key + "_sha256": digest(path) for key, path in files.items()}
            repeated = self.cli(client + "-agent-add-repeat", ["agent", "add", client, "--yes",
                "--profile-home", self.profile.home, "--workspace", self.profile.workspace])["clients"][0]
            require(repeated["changed"] is False and repeated["principal"] == row["principal"]
                    and repeated["instance"] == row["instance"]
                    and all(digest(files[key]) == hashes[key + "_sha256"] for key in files), "agent add replaced identity or session")
            self.summary["clients"][client] = {"principal": row["principal"], "instance": row["instance"],
                                               "launcher_principal_verified": True, "model_ready": False, **hashes}
        repeated = self.cli("up-repeat", [*up, "--yes"])
        require(all(row["changed"] is False and row["principal"] == clients[row["client"]]["principal"]
                    and row["instance"] == clients[row["client"]]["instance"] for row in repeated["clients"]), "up repeat changed identity")
        require(self.pid() == initial_pid, "repeating onboarding restarted the daemon")
        principal = clients["codex"]["principal"]
        self.cli("owner-grant", ["agent", "grant", "--agent", principal, "--manage-goals", "true"], owner=True)
        after_grant = self.cli("agent-add-after-grant", ["agent", "add", "codex", "--yes", "--profile-home", self.profile.home,
                                           "--workspace", self.profile.workspace])["clients"][0]
        require(after_grant["changed"] is False and after_grant["principal"] == principal
                and after_grant["instance"] == clients["codex"]["instance"], "agent add after a grant replaced identity")
        granted = {row["agent"]: row for row in self.owner_agents()}
        require(len(granted) == 2 and granted[principal]["grants"]["manage_goals"] is True
                and granted[clients["claude"]["principal"]]["grants"]["manage_goals"] is False,
                "agent add reset later owner grants or granted another agent")
        require(self.pid() == initial_pid, "agent add after a grant restarted the daemon")
        require(all(digest(Path(row[key])) == self.summary["clients"][client][key + "_sha256"]
                    for client, row in clients.items() for key in ("credential_file", "session_file")),
                "repeated up or agent add replaced secret files")
        document = tomllib.loads(codex.read_text())
        claude_document = json.loads(claude.read_text())
        require(document["model"] == "qualification-preserve" and document["approval_policy"] == "on-request"
                and document["qualification"]["marker"] == "codex-unrelated"
                and claude_document["qualification_marker"] == "claude-unrelated"
                and claude_document["permissions"] == {"defaultMode": "default"}, "onboarding changed unrelated client settings")
        require(unselected_before == data_fingerprint(unselected) and digest(pi) == pi_before, "unselected profile or client changed")
        for key in ("installed_prefix_inference", "distinct_principals_and_sessions", "no_initial_grants",
                    "bound_launchers", "repeat_preserves_identity", "repeat_preserves_daemon_pid",
                    "agent_add_preserves_owner_grant", "unrelated_settings_preserved", "unselected_profiles_preserved"):
            self.summary["checks"][key] = True
        self.summary["daemon_pid"] = initial_pid

    def cleanup(self):
        if self.service_attempted:
            # Control and remove only the exact unit recorded by the private plan.
            self.service("stop", optional=True)
            self.wait("native service stopped", lambda: self.service("status")["state"] == "stopped")
            proposed = self.service("remove-plan")
            self.service("remove", "--expect-plan", proposed["plan_sha256"])
            require(not os.path.lexists(self.unit), "task service unit remains after removal")
            self.summary["checks"]["owned_service_stopped_and_removed"] = True
        if self.foreground is not None:
            if self.foreground.poll() is None:
                self.cli("daemon-stop", ["daemon", "stop"], owner=True, optional=True)
            try:
                self.foreground.wait(timeout=self.timeout)
            except subprocess.TimeoutExpired:
                self.foreground.kill()
                self.foreground.wait(timeout=self.timeout)
                raise QualificationError("owned foreground daemon required forced cleanup") from None
            self.summary["checks"]["owned_foreground_reaped"] = True
        self.wait("daemon lock released", lambda: not self.lock_held())
        require(not (self.home / "daemon.sock").exists(), "daemon socket remains after cleanup")
        self.summary["checks"]["cleanup_verified"] = True

    def run(self):
        try:
            self.profile = PrivateProfile()
            self.home = self.profile.root / "daemon"
            self.service_home = self.profile.root / "service-home"
            self.service_home.mkdir(mode=0o700)
            self.logs = self.profile.logs
            self.env = self.profile.environment(self.args.bootstrap)
            self.env.update({"LOCUST_RELAY": "none", "LOCUST_LOOKUP": "none", "LOCUST_BIND": "127.0.0.1:0"})
            if self.args.service == "systemd":
                # Transport address of the current per-user manager, never provider credentials.
                for name in ("XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS"):
                    if name in os.environ:
                        self.env[name] = os.environ[name]
                self.env["XDG_CONFIG_HOME"] = str(self.service_home / ".config")
            self.install()
            self.flow()
            self.summary["status"] = "passed"
        except Exception as error:
            self.summary.update(status="failed", failure={"stage": self.stage, "type": type(error).__name__,
                "detail_sha256": sha256(str(error).encode())})
        finally:
            if self.profile is not None:
                try:
                    (self.profile.fixture / "signing.secret").unlink(missing_ok=True)
                    self.summary["checks"]["disposable_signer_removed"] = True
                    self.cleanup()
                    self.profile.close()
                except Exception as error:
                    self.summary.update(status="failed", retained_private_fixture=str(self.profile.root),
                        cleanup_failure={"type": type(error).__name__, "detail_sha256": sha256(str(error).encode())})
            self.summary["finished_at"] = datetime.now(timezone.utc).isoformat()
            private_write(self.args.output / "summary.json", json.dumps(self.summary, indent=2, sort_keys=True) + "\n")
        return self.summary["status"] == "passed"


def positive(value):
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be a positive caller-authored number of milliseconds")
    return parsed


def absolute(value):
    path = Path(value)
    if not path.is_absolute():
        raise argparse.ArgumentTypeError("must be an absolute path")
    return path.resolve()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bootstrap", required=True, type=absolute, help="Trusted locally built bootstrap executable")
    parser.add_argument("--bundle", required=True, type=absolute, help="Inert local release bundle to test-sign and install")
    parser.add_argument("--output", required=True, type=absolute, help="New private evidence directory")
    parser.add_argument("--timeout-ms", required=True, type=positive, help="Caller deadline for each command and readiness wait")
    parser.add_argument("--service", required=True, choices=("none", "launchd", "systemd"))
    args = parser.parse_args()
    if not args.bootstrap.is_file() or not os.access(args.bootstrap, os.X_OK):
        parser.error("--bootstrap must be an executable file")
    if not args.bundle.is_dir():
        parser.error("--bundle must be a directory")
    if args.output.exists():
        parser.error("--output must be new")
    if (args.service == "launchd" and platform.system() != "Darwin"
            or args.service == "systemd" and platform.system() != "Linux"):
        parser.error("selected native service manager is unsupported on this host")
    passed = OnboardingCheck(args).run()
    print(json.dumps({"status": "passed" if passed else "failed", "summary": str(args.output / "summary.json"),
                      "model_ready": False}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
