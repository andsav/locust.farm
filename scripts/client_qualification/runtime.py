"""Private profiles, authenticated fixture receipts, and owned-process cleanup."""

import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import tempfile
import threading
import time


GUARD = '(version 1)(allow default)(deny network*)(allow network-outbound (remote ip "localhost:*"))(allow network-outbound (remote unix-socket))'
PROTECTED = ("LOCUST_HOME", "LOCUST_SESSION", "LOCUST_CREDENTIAL")


def records(path, strict=False):
    if not Path(path).exists():
        return []
    result = []
    for line in Path(path).read_text().splitlines():
        try:
            result.append(json.loads(line))
        except json.JSONDecodeError:
            if strict:
                raise ValueError("Malformed completed protocol receipt log")
            continue
    return result


def private_write(path, content):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    with path.open("wb") as file:
        os.chmod(path, 0o600)
        file.write(content.encode() if isinstance(content, str) else content)


class Profile:
    def __init__(self, output, client):
        # macOS Unix socket paths have a small fixed limit. Runtime profiles
        # live under a short system temp path; evidence is retained in output.
        self.root = Path(tempfile.mkdtemp(prefix="lq-", dir="/tmp")).resolve()
        for name in ("home", "config", "tmp", "workspace", "fixture", "logs"):
            setattr(self, name, self.root / name)
            getattr(self, name).mkdir(mode=0o700)
        self.logs = Path(tempfile.mkdtemp(prefix=client + "-", dir=output)).resolve()
        self.session = self.fixture / "session.bin"
        self.credential = self.fixture / "credential.bin"
        private_write(self.session, os.urandom(32))
        private_write(self.credential, os.urandom(32))

    def close(self):
        import shutil
        if self.root.exists():
            shutil.rmtree(self.root)

    def environment(self, binary):
        env = {"HOME": str(self.home), "XDG_CONFIG_HOME": str(self.config),
               "XDG_CACHE_HOME": str(self.home / ".cache"), "XDG_DATA_HOME": str(self.home / ".local/share"),
               "TMPDIR": str(self.tmp), "TMP": str(self.tmp), "TEMP": str(self.tmp),
               "CODEX_HOME": str(self.home / ".codex"), "CLAUDE_CONFIG_DIR": str(self.home / ".claude"),
               "PI_CODING_AGENT_DIR": str(self.home / ".pi/agent"),
               "PATH": str(Path(binary).parent) + ":/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
               "LANG": "en_US.UTF-8", "TERM": "dumb", "NO_COLOR": "1"}
        assert not any(key in env for key in PROTECTED)
        return env

    def apply(self, proposal):
        if any(key in proposal.get("environment", {}) for key in PROTECTED):
            raise ValueError("Protected bridge environment must not reach client")
        for file in proposal["files"]:
            path = Path(file["relative_path"])
            if path.is_absolute() or ".." in path.parts:
                raise ValueError("Generated file escapes isolated HOME")
            private_write(self.home / path, file["content"])


class Process:
    def __init__(self, argv, env, cwd, logs, name, timeout, guarded=True):
        self.timeout = timeout
        self.stdout_path = Path(logs) / (name + ".stdout")
        self.stderr_path = Path(logs) / (name + ".stderr")
        self.argv = list(argv)
        self.timed_out = False
        self.interrupted = False
        self.started = time.monotonic()
        self.closed = False
        self.forced_cleanup = False
        self.children = {}
        actual = (["/usr/bin/sandbox-exec", "-p", GUARD] + self.argv) if guarded else self.argv
        with self.stdout_path.open("wb") as out, self.stderr_path.open("wb") as err:
            self.process = subprocess.Popen(actual, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                            stdout=out, stderr=err, start_new_session=True)
        self.pgid = self.process.pid

    def wait(self, condition=None, observe=None):
        deadline = self.started + self.timeout
        while self.process.poll() is None:
            if observe:
                observe(self)
            if condition and condition():
                self.interrupted = True
                try:
                    self.process.send_signal(signal.SIGINT)
                except ProcessLookupError:
                    pass
                break
            if time.monotonic() >= deadline:
                self.timed_out = True
                break
            time.sleep(0.02)
        if self.interrupted:
            grace = time.monotonic() + self.timeout
            while time.monotonic() < grace:
                if self.process.poll() is not None and not self.live_owned_members() and not any(
                        self.owned_child_alive(pid) for pid in self.children):
                    break
                if observe:
                    observe(self)
                time.sleep(0.02)
        if observe:
            observe(self)  # A detached bridge can publish just as its client exits.
        self.observed_exit = self.process.poll() is not None
        self.close()
        return {"argv": self.argv, "exit_code": self.process.returncode, "timed_out": self.timed_out,
                "interrupted": self.interrupted, "signal_scope": "client_leader" if self.interrupted else None,
                "natural_cleanup": self.observed_exit and not self.forced_cleanup, "elapsed_ms": round((time.monotonic() - self.started) * 1000),
                "observed_exit_before_cleanup": self.observed_exit, "forced_cleanup": self.forced_cleanup,
                "cleanup_verified": not self.live_owned_members() and not any(self.owned_child_alive(pid) for pid in self.children),
                "cleanup_scope": "owned_session_and_receipt_identified_bridges",
                "cleanup_limitation": "Fully detached descendants without a matching bridge receipt are outside cleanup evidence.",
                "stdout": str(self.stdout_path), "stderr": str(self.stderr_path)}

    def close(self):
        if self.closed:
            return
        self.closed = True
        self.forced_cleanup = (self.process.poll() is None or bool(self.live_owned_members()) or
                               any(self.owned_child_alive(pid) for pid in self.children))
        # A helper may create another process group while remaining in the
        # client's session. Clean every live member of that owned session.
        self.signal_owned(signal.SIGTERM)
        if self.process.poll() is None:
            try:
                self.process.wait(timeout=self.timeout)
            except subprocess.TimeoutExpired:
                pass
        if self.live_owned_members():
            self.signal_owned(signal.SIGKILL)
        self.process.wait()
        for pid in self.children:
            if self.owned_child_alive(pid):
                self.signal_child(pid, signal.SIGTERM)
                deadline = time.monotonic() + self.timeout
                while self.owned_child_alive(pid) and time.monotonic() < deadline:
                    time.sleep(0.02)
                if self.owned_child_alive(pid):
                    self.signal_child(pid, signal.SIGKILL)
        deadline = time.monotonic() + self.timeout
        while (self.live_owned_members() or any(self.owned_child_alive(pid) for pid in self.children)) and time.monotonic() < deadline:
            time.sleep(0.02)

    def register_child(self, pid, executable, events_file):
        if isinstance(pid, int) and pid > 1:
            self.children[pid] = (str(executable), str(events_file))

    def owned_child_alive(self, pid):
        # Confirm the unique per-run receipt path and exact fixture executable
        # in a live command before signaling a possibly reused PID. No unrelated
        # process is killed just because its PID appeared in a historic log.
        result = subprocess.run(["/bin/ps", "-p", str(pid), "-o", "stat=,command="],
                                capture_output=True, text=True, env={})
        text = result.stdout.strip()
        return bool(text and not text.startswith("Z") and
                    all(expected in text for expected in self.children[pid]))

    def signal_owned(self, sig):
        for pid in self.live_owned_members():
            try:
                # Revalidate the private session immediately before signaling;
                # a stale process listing alone never authorizes a signal.
                if os.getsid(pid) == self.pgid:
                    os.kill(pid, sig)
            except ProcessLookupError:
                pass

    def signal_child(self, pid, sig):
        if self.owned_child_alive(pid):
            try:
                os.kill(pid, sig)
            except ProcessLookupError:
                pass

    def live_owned_members(self):
        # start_new_session gives this invocation a private session. Helpers
        # may change their process group without leaving it. A fully detached
        # bridge remains observable through its unique receipt-path identity;
        # other descendants that create a new session are outside this proof.
        listing = subprocess.run(["/bin/ps", "-axo", "pid=,stat="], capture_output=True,
                                 text=True, env={}, check=True).stdout
        members = []
        for line in listing.splitlines():
            fields = line.split()
            if len(fields) != 2 or fields[1].startswith("Z"):
                continue
            pid = int(fields[0])
            try:
                if os.getsid(pid) == self.pgid:
                    members.append(pid)
            except ProcessLookupError:
                pass
        return members


class SocketFixture:
    def __init__(self, profile):
        self.profile = profile
        self.session = profile.session.read_bytes().hex()
        self.credential = profile.credential.read_bytes().hex()
        self.receipts = []
        self.sequence = 0
        self.hold = True
        self.release = threading.Event()
        self.stop = threading.Event()
        self.wait_started = threading.Event()
        self.server = socket.socket(socket.AF_UNIX)
        self.server.bind(str(profile.fixture / "daemon.sock"))
        self.server.listen()
        self.server.settimeout(0.1)
        self.connections = []
        self.workers = []
        self.lock = threading.Lock()
        self.thread = threading.Thread(target=self.serve, daemon=True)

    def serve(self):
        while not self.stop.is_set():
            try:
                connection, _ = self.server.accept()
            except socket.timeout:
                continue
            except OSError:
                break
            self.connections.append(connection)
            worker = threading.Thread(target=self.handle, args=(connection,), daemon=True)
            self.workers.append(worker)
            worker.start()

    def handle(self, connection):
        try:
            connection.settimeout(1)
            raw = bytearray()
            while not raw.endswith(b"\n") and not self.stop.is_set():
                chunk = connection.recv(4096)
                if not chunk:
                    return
                raw.extend(chunk)
            request = json.loads(raw)
            operation = request.get("operation")
            valid = (request.get("fixture") == "locust-probe-v1" and
                     request.get("session") == self.session and request.get("credential") == self.credential and
                     operation in ("read", "write", "wait"))
            with self.lock:
                self.receipts.append({"operation": operation, "authenticated_fixture": valid, "phase": "start"})
            if not valid:
                return
            if operation == "wait" and self.hold:
                self.wait_started.set()
                self.release.wait()
            if self.stop.is_set():
                return
            with self.lock:
                if operation == "write":
                    self.sequence += 1
                sequence = self.sequence
            connection.sendall((json.dumps({"fixture": "locust-probe-v1", "ok": True,
                                            "sequence": sequence}) + "\n").encode())
            self.receipts.append({"operation": operation, "authenticated_fixture": True,
                                  "phase": "complete", "sequence": sequence})
        except (OSError, json.JSONDecodeError):
            pass
        finally:
            connection.close()

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_args):
        self.stop.set()
        self.release.set()
        self.server.close()
        for connection in self.connections:
            try:
                connection.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
        self.thread.join()
        for worker in self.workers:
            worker.join()
