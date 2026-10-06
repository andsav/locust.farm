"""Use installed onboarding identities in the shared guarded native-client fixture."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

from .production import ProductionDaemon, ProductionError, PUBLIC_ID
from owner_plans import confirmation_arguments


class OnboardedDaemon(ProductionDaemon):
    def __init__(self, profile, binary, timeout_seconds, client):
        super().__init__(profile, binary, timeout_seconds)
        self.client = "claude" if client == "claude-code" else client
        self.onboarding = {}

    def _onboarding_command(self, arguments, operation):
        response = subprocess.run(self._guard() + self.command(arguments, owner=True),
            cwd=self.profile.workspace, env=self.environment(), capture_output=True,
            timeout=self.timeout_seconds)
        self._record("onboarding", operation=operation, exit_code=response.returncode,
                     stdout_sha256=hashlib.sha256(response.stdout).hexdigest(),
                     stderr_sha256=hashlib.sha256(response.stderr).hexdigest())
        try:
            envelope = json.loads(response.stdout)
        except (ValueError, UnicodeError):
            raise ProductionError("onboarding returned invalid JSON") from None
        if response.returncode != 0 or envelope.get("ok") is not True:
            raise ProductionError("installed onboarding failed")
        return envelope["result"]

    def onboard(self, operation="up"):
        arguments = (["up", "--client", self.client, "--service", "none",
                      "--wait-ms", str(int(self.timeout_seconds * 1000))]
                     if operation == "up" else ["agent", "add", self.client])
        arguments += ["--profile-home", str(self.profile.home), "--workspace",
                      str(self.profile.workspace)]
        if self.onboarding.get("name"):
            arguments += ["--name", self.onboarding["name"]]
        planned = self._onboarding_command(arguments, operation + "_plan")
        if not isinstance(planned, dict) or planned.get("action") != "review_required":
            raise ProductionError("onboarding did not show a reviewable owner plan")
        confirmed = confirmation_arguments(arguments, planned)
        result = self._onboarding_command(confirmed, operation)
        if (result.get("configuration_ready") is not True or result.get("daemon_api_ready") is not True
                or result.get("model_ready") is not False
                or len(result.get("clients", [])) != 1):
            raise ProductionError("onboarding readiness or enrollment scope mismatch")
        row = result["clients"][0]
        if row.get("client") != self.client:
            raise ProductionError("onboarding selected a different client")
        if not isinstance(row.get("name"), str) or not row["name"]:
            raise ProductionError("onboarding did not return the selected agent name")
        self.onboarding["name"] = row["name"]
        return row

    def binding(self, row):
        result = {key: row[key] for key in ("principal", "instance", "launcher")}
        if (not PUBLIC_ID.fullmatch(result["principal"])
                or not re.fullmatch(r"[0-9a-f]{32}", result["instance"])):
            raise ProductionError("onboarding returned an invalid principal or instance")
        launcher = Path(result["launcher"])
        if not launcher.is_relative_to(self.profile.home) or not os.access(launcher, os.X_OK):
            raise ProductionError("onboarding launcher escaped selected profile or is not executable")
        result["launcher_sha256"] = hashlib.sha256(launcher.read_bytes()).hexdigest()
        for key in ("credential_file", "session_file"):
            path = Path(row[key])
            if (not path.is_relative_to(self.home) or path.is_symlink() or not path.is_file()
                    or path.stat().st_mode & 0o7777 != 0o600 or path.stat().st_size != 32
                    or path.stat().st_uid != os.getuid() or path.stat().st_nlink != 1):
                raise ProductionError("onboarding identity is not a private fixture secret")
            result[key] = str(path)
            result[key + "_sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
        return result

    def retry(self, *, after_native=False):
        for operation in ("up", "agent add"):
            row = self.onboard(operation)
            if self.binding(row) != self.onboarding["binding"]:
                raise ProductionError("onboarding retry changed enrolled identity or binding")
            # Native clients write unrelated profile metadata. A reviewed up can
            # refresh setup's saved original/config images while preserving the
            # exact binding. The immediately following agent add must be a no-op.
            if after_native and operation == "up":
                self.onboarding["native_profile_reconciled"] = row.get("changed") is True
            elif row.get("changed") is not False:
                raise ProductionError("onboarding retry changed an unused or reconciled profile")
        agents = self.call(["status"], owner=True)["status"]["agents"]
        if len(agents) != 1 or agents[0]["agent"] != self.principal:
            raise ProductionError("onboarding retry created an extra principal")
        self.onboarding["retry_verified"] = True

    def doctor(self):
        result = self.call(["doctor", "--client", self.client, "--profile-home",
                            self.profile.home, "--service", "none"], owner=True)
        checks = {row["name"]: row.get("ok") for row in result.get("checks", [])}
        required = {"installation", "service", "onboarding_journal", "onboarding_complete",
                    "agent_identity", "mcp_registration", "skill", "launcher", "profile_binding",
                    "profile_transaction", "profile_configuration", "hello", "status",
                    "credential_file", "daemon_lock", "socket_connection"}
        profile = result.get("profile", {})
        if (not required.issubset(checks) or not all(value is True for value in checks.values())
                or result.get("model_ready") is not False
                or profile.get("discovery") != "unverified"
                or profile.get("identity", {}).get("principal") != self.principal
                or profile.get("identity", {}).get("instance") != self.instance):
            raise ProductionError("selected onboarding doctor did not verify the exact binding")
        return {"checks": checks, "model_ready": False, "discovery": "unverified",
                "principal": self.principal, "instance": self.instance}

    def _enroll_identity(self):
        row = self.onboard()
        binding = self.binding(row)
        self.principal, self.instance = binding["principal"], binding["instance"]
        self.credential, self.session = Path(binding["credential_file"]), Path(binding["session_file"])
        self.onboarding["binding"] = binding
        agents = self.call(["status"], owner=True)["status"]["agents"]
        if (len(agents) != 1 or agents[0]["agent"] != self.principal
                or agents[0].get("author_only") is not False):
            raise ProductionError("onboarding enrolled the wrong kind or number of agents")
        self.onboarding["active_agent_enrolled"] = True
        self.retry()
